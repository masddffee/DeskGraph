use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use std::time::{Duration, Instant};

use deskgraph_domain::ImageFormat;

use crate::{
    ChunkProvenance, ExtractedChunk, ExtractionError, ExtractionLimits, ExtractionOutput,
    MediaKind, UNTRUSTED_TEXT, check_control, validate_limits,
};

pub const ABSOLUTE_MAX_OCR_SOURCE_BYTES: u64 = 32 * 1024 * 1024;
pub const ABSOLUTE_MAX_OCR_OUTPUT_BYTES: u64 = 8 * 1024 * 1024;
pub const ABSOLUTE_MAX_OCR_OBSERVATIONS: usize = 4_096;
pub const ABSOLUTE_MAX_OCR_OBSERVATION_BYTES: usize = 256 * 1024;
pub const ABSOLUTE_MAX_OCR_DIMENSION: u32 = 16_384;
pub const ABSOLUTE_MAX_OCR_PIXELS: u64 = 64 * 1024 * 1024;
const NORMALIZED_SCALE: u32 = 1_000_000;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct OcrRequest {
    pub format: ImageFormat,
    pub expected_source_bytes: u64,
    pub modified_unix_ns: Option<i64>,
    pub pixel_width: u32,
    pub pixel_height: u32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct OcrProviderLimits {
    pub max_output_bytes: u64,
    pub max_observations: usize,
    pub max_observation_bytes: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct OcrBoundingBox {
    /// Normalized top-left X coordinate in millionths of the source width.
    pub x_ppm: u32,
    /// Normalized top-left Y coordinate in millionths of the source height.
    pub y_ppm: u32,
    pub width_ppm: u32,
    pub height_ppm: u32,
}

impl OcrBoundingBox {
    #[must_use]
    pub fn is_valid(self) -> bool {
        self.width_ppm > 0
            && self.height_ppm > 0
            && self
                .x_ppm
                .checked_add(self.width_ppm)
                .is_some_and(|right| right <= NORMALIZED_SCALE)
            && self
                .y_ppm
                .checked_add(self.height_ppm)
                .is_some_and(|bottom| bottom <= NORMALIZED_SCALE)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OcrObservation {
    pub text: String,
    pub bounding_box: OcrBoundingBox,
    pub confidence_basis_points: Option<u16>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OcrOutput {
    pub observations: Vec<OcrObservation>,
}

#[derive(Clone, Debug)]
pub struct OcrCancellation {
    cancelled: Arc<AtomicBool>,
}

impl OcrCancellation {
    pub fn cancel(&self) {
        self.cancelled.store(true, Ordering::Release);
    }

    #[must_use]
    pub fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::Acquire)
    }
}

#[derive(Clone, Debug)]
pub struct OcrControl {
    cancellation: OcrCancellation,
    started: Instant,
    deadline: Instant,
}

impl OcrControl {
    #[must_use]
    pub fn new(max_processing_time: Duration) -> Self {
        let started = Instant::now();
        let deadline = started.checked_add(max_processing_time).unwrap_or(started);
        Self {
            cancellation: OcrCancellation {
                cancelled: Arc::new(AtomicBool::new(false)),
            },
            started,
            deadline,
        }
    }

    #[must_use]
    pub fn cancellation(&self) -> OcrCancellation {
        self.cancellation.clone()
    }

    pub fn check(&self) -> Result<(), ExtractionError> {
        if self.cancellation.is_cancelled() {
            return Err(ExtractionError::Cancelled);
        }
        if Instant::now() > self.deadline {
            return Err(ExtractionError::TimeLimitExceeded);
        }
        Ok(())
    }

    #[must_use]
    pub fn deadline(&self) -> Instant {
        self.deadline
    }

    #[must_use]
    pub fn started(&self) -> Instant {
        self.started
    }
}

pub trait OcrProvider {
    fn provider_id(&self) -> &'static str;
    fn provider_version(&self) -> &'static str;
    fn recognize(
        &self,
        encoded_image: &[u8],
        request: OcrRequest,
        limits: OcrProviderLimits,
        control: &OcrControl,
    ) -> Result<OcrOutput, ExtractionError>;
}

#[cfg(target_os = "macos")]
#[derive(Clone, Copy, Debug, Default)]
pub struct NativeOcrProvider;

#[cfg(not(target_os = "macos"))]
#[derive(Clone, Copy, Debug, Default)]
pub struct NativeOcrProvider;

#[cfg(not(target_os = "macos"))]
impl OcrProvider for NativeOcrProvider {
    fn provider_id(&self) -> &'static str {
        "deskgraph.native-ocr-unavailable"
    }

    fn provider_version(&self) -> &'static str {
        "1"
    }

    fn recognize(
        &self,
        _encoded_image: &[u8],
        _request: OcrRequest,
        _limits: OcrProviderLimits,
        _control: &OcrControl,
    ) -> Result<OcrOutput, ExtractionError> {
        Err(ExtractionError::OcrProviderUnavailable)
    }
}

pub(crate) fn validate_ocr_request(
    request: OcrRequest,
    limits: ExtractionLimits,
) -> Result<OcrProviderLimits, ExtractionError> {
    validate_limits(limits)?;
    if !matches!(request.format, ImageFormat::Png | ImageFormat::Jpeg) {
        return Err(ExtractionError::UnsupportedMediaKind);
    }
    if request.expected_source_bytes == 0
        || request.expected_source_bytes > limits.max_image_source_bytes
        || request.expected_source_bytes > ABSOLUTE_MAX_OCR_SOURCE_BYTES
    {
        return Err(ExtractionError::SourceTooLarge);
    }
    let pixels = u64::from(request.pixel_width)
        .checked_mul(u64::from(request.pixel_height))
        .ok_or(ExtractionError::ImageDimensionLimitExceeded)?;
    if request.pixel_width == 0
        || request.pixel_height == 0
        || request.pixel_width > limits.max_image_dimension
        || request.pixel_height > limits.max_image_dimension
        || request.pixel_width > ABSOLUTE_MAX_OCR_DIMENSION
        || request.pixel_height > ABSOLUTE_MAX_OCR_DIMENSION
        || pixels > limits.max_image_pixels
        || pixels > ABSOLUTE_MAX_OCR_PIXELS
    {
        return Err(ExtractionError::ImageDimensionLimitExceeded);
    }
    let max_output_bytes = limits.max_output_bytes.min(ABSOLUTE_MAX_OCR_OUTPUT_BYTES);
    let max_observations = limits.max_chunks.min(ABSOLUTE_MAX_OCR_OBSERVATIONS);
    let max_observation_bytes = usize::try_from(max_output_bytes)
        .unwrap_or(ABSOLUTE_MAX_OCR_OBSERVATION_BYTES)
        .min(ABSOLUTE_MAX_OCR_OBSERVATION_BYTES);
    if max_output_bytes == 0 || max_observations == 0 || max_observation_bytes == 0 {
        return Err(ExtractionError::InvalidLimits);
    }
    Ok(OcrProviderLimits {
        max_output_bytes,
        max_observations,
        max_observation_bytes,
    })
}

pub(crate) fn build_ocr_extraction_output(
    provider: &dyn OcrProvider,
    request: OcrRequest,
    limits: ExtractionLimits,
    control: &OcrControl,
    output: OcrOutput,
) -> Result<ExtractionOutput, ExtractionError> {
    control.check()?;
    let provider_limits = validate_ocr_request(request, limits)?;
    if output.observations.len() > provider_limits.max_observations {
        return Err(ExtractionError::OcrObservationLimitExceeded);
    }
    let mut chunks = Vec::new();
    let mut output_bytes = 0_u64;
    for (observation_index, observation) in output.observations.into_iter().enumerate() {
        control.check()?;
        if observation.text.is_empty()
            || observation.text.len() > provider_limits.max_observation_bytes
            || !observation.bounding_box.is_valid()
            || observation
                .confidence_basis_points
                .is_some_and(|value| value > 10_000)
        {
            return Err(ExtractionError::OcrOutputInvalid);
        }
        let observation_number = u32::try_from(observation_index)
            .ok()
            .and_then(|index| index.checked_add(1))
            .ok_or(ExtractionError::OcrObservationLimitExceeded)?;
        let mut start = 0_usize;
        let mut fragment_index = 0_u32;
        while start < observation.text.len() {
            check_control(
                control.started(),
                limits.max_processing_time,
                &control.cancellation,
            )?;
            if chunks.len() >= limits.max_chunks {
                return Err(ExtractionError::ChunkLimitExceeded);
            }
            let mut end = start
                .saturating_add(limits.max_chunk_bytes)
                .min(observation.text.len());
            while end > start && !observation.text.is_char_boundary(end) {
                end -= 1;
            }
            if end == start {
                return Err(ExtractionError::InvalidLimits);
            }
            let chunk_bytes =
                u64::try_from(end - start).map_err(|_| ExtractionError::OutputTooLarge)?;
            output_bytes = output_bytes
                .checked_add(chunk_bytes)
                .ok_or(ExtractionError::OutputTooLarge)?;
            if output_bytes > provider_limits.max_output_bytes {
                return Err(ExtractionError::OutputTooLarge);
            }
            chunks.push(ExtractedChunk {
                ordinal: u32::try_from(chunks.len())
                    .map_err(|_| ExtractionError::ChunkLimitExceeded)?,
                text: observation.text[start..end].to_string(),
                provenance: ChunkProvenance::OcrObservation {
                    observation_number,
                    fragment_index,
                    bounding_box: observation.bounding_box,
                    confidence_basis_points: observation.confidence_basis_points,
                },
                trust_class: UNTRUSTED_TEXT,
            });
            if end == observation.text.len() {
                break;
            }
            let mut next = end.saturating_sub(limits.chunk_overlap_bytes);
            while next > start && !observation.text.is_char_boundary(next) {
                next -= 1;
            }
            start = if next > start { next } else { end };
            fragment_index = fragment_index
                .checked_add(1)
                .ok_or(ExtractionError::ChunkLimitExceeded)?;
        }
    }
    control.check()?;
    Ok(ExtractionOutput {
        provider_id: provider.provider_id(),
        provider_version: provider.provider_version(),
        media_kind: MediaKind::Image(request.format),
        source_bytes: request.expected_source_bytes,
        output_bytes,
        modified_unix_ns: request.modified_unix_ns,
        chunks,
        image_metadata: None,
    })
}

impl crate::CancellationSignal for OcrCancellation {
    fn is_cancelled(&self) -> bool {
        OcrCancellation::is_cancelled(self)
    }
}

#[cfg(target_os = "macos")]
mod macos {
    use std::ptr::NonNull;

    use block2::RcBlock;
    use objc2::AnyThread;
    use objc2_foundation::{NSArray, NSData, NSDictionary, NSError, NSString};
    use objc2_vision::{
        VNImageOption, VNImageRequestHandler, VNRecognizeTextRequest, VNRequest,
        VNRequestProgressProviding, VNRequestTextRecognitionLevel,
    };

    use super::{
        NORMALIZED_SCALE, NativeOcrProvider, OcrBoundingBox, OcrControl, OcrObservation, OcrOutput,
        OcrProvider, OcrProviderLimits, OcrRequest,
    };
    use crate::ExtractionError;

    const PROVIDER_ID: &str = "deskgraph.macos-vision-ocr";
    const PROVIDER_VERSION: &str = "1";

    impl OcrProvider for NativeOcrProvider {
        fn provider_id(&self) -> &'static str {
            PROVIDER_ID
        }

        fn provider_version(&self) -> &'static str {
            PROVIDER_VERSION
        }

        fn recognize(
            &self,
            encoded_image: &[u8],
            request: OcrRequest,
            limits: OcrProviderLimits,
            control: &OcrControl,
        ) -> Result<OcrOutput, ExtractionError> {
            control.check()?;
            if u64::try_from(encoded_image.len()).ok() != Some(request.expected_source_bytes) {
                return Err(ExtractionError::SourceChanged);
            }

            let vision_request = VNRecognizeTextRequest::new();
            vision_request.setRecognitionLevel(VNRequestTextRecognitionLevel::Accurate);
            vision_request.setUsesLanguageCorrection(true);
            let supported = unsafe { vision_request.supportedRecognitionLanguagesAndReturnError() }
                .map_err(|_| ExtractionError::OcrProviderUnavailable)?;
            let supported = supported
                .to_vec()
                .into_iter()
                .map(|language| language.to_string())
                .collect::<Vec<_>>();
            if !supported.iter().any(|language| language == "zh-Hant")
                || !supported.iter().any(|language| language == "en-US")
            {
                return Err(ExtractionError::OcrLanguageUnavailable);
            }
            let zh = NSString::from_str("zh-Hant");
            let en = NSString::from_str("en-US");
            vision_request.setRecognitionLanguages(&NSArray::from_slice(&[&*zh, &*en]));

            let cancellation = control.cancellation();
            let deadline = control.deadline();
            let progress = RcBlock::new(
                move |request: NonNull<VNRequest>, _fraction: f64, _error: *mut NSError| {
                    if cancellation.is_cancelled() || std::time::Instant::now() > deadline {
                        unsafe { request.as_ref().cancel() };
                    }
                },
            );
            unsafe { vision_request.setProgressHandler(RcBlock::as_ptr(&progress)) };

            let data = NSData::with_bytes(encoded_image);
            let options = NSDictionary::<VNImageOption, objc2::runtime::AnyObject>::new();
            let handler = VNImageRequestHandler::initWithData_options(
                VNImageRequestHandler::alloc(),
                &data,
                &options,
            );
            let requests = NSArray::<VNRequest>::from_slice(&[&vision_request]);
            if handler.performRequests_error(&requests).is_err() {
                control.check()?;
                return Err(ExtractionError::OcrProviderFailed);
            }
            control.check()?;

            let observations = vision_request
                .results()
                .ok_or(ExtractionError::OcrOutputInvalid)?;
            if observations.len() > limits.max_observations {
                return Err(ExtractionError::OcrObservationLimitExceeded);
            }
            let mut output = Vec::with_capacity(observations.len());
            let mut output_bytes = 0_u64;
            for observation in observations.to_vec() {
                control.check()?;
                let candidates = observation.topCandidates(1);
                let Some(candidate) = candidates.firstObject() else {
                    continue;
                };
                let text = candidate.string().to_string();
                if text.is_empty() || text.len() > limits.max_observation_bytes {
                    return Err(ExtractionError::OcrOutputInvalid);
                }
                output_bytes = output_bytes
                    .checked_add(
                        u64::try_from(text.len()).map_err(|_| ExtractionError::OutputTooLarge)?,
                    )
                    .ok_or(ExtractionError::OutputTooLarge)?;
                if output_bytes > limits.max_output_bytes {
                    return Err(ExtractionError::OutputTooLarge);
                }
                let confidence = candidate.confidence();
                if !confidence.is_finite() || !(0.0..=1.0).contains(&confidence) {
                    return Err(ExtractionError::OcrOutputInvalid);
                }
                let rectangle = unsafe { observation.boundingBox() };
                let bounding_box = normalized_top_left_box(
                    rectangle.origin.x,
                    rectangle.origin.y,
                    rectangle.size.width,
                    rectangle.size.height,
                )?;
                let confidence_basis_points =
                    Some((confidence * 10_000.0).round().clamp(0.0, 10_000.0) as u16);
                output.push(OcrObservation {
                    text,
                    bounding_box,
                    confidence_basis_points,
                });
            }
            Ok(OcrOutput {
                observations: output,
            })
        }
    }

    fn normalized_top_left_box(
        x: f64,
        vision_y: f64,
        width: f64,
        height: f64,
    ) -> Result<OcrBoundingBox, ExtractionError> {
        const EPSILON: f64 = 1.0e-6;
        if !x.is_finite()
            || !vision_y.is_finite()
            || !width.is_finite()
            || !height.is_finite()
            || x < -EPSILON
            || vision_y < -EPSILON
            || width <= 0.0
            || height <= 0.0
            || x + width > 1.0 + EPSILON
            || vision_y + height > 1.0 + EPSILON
        {
            return Err(ExtractionError::OcrOutputInvalid);
        }
        let left = x.clamp(0.0, 1.0);
        let right = (x + width).clamp(0.0, 1.0);
        let top = (1.0 - (vision_y + height)).clamp(0.0, 1.0);
        let bottom = (1.0 - vision_y).clamp(0.0, 1.0);
        let left = to_ppm(left)?;
        let right = to_ppm(right)?;
        let top = to_ppm(top)?;
        let bottom = to_ppm(bottom)?;
        let width_ppm = right
            .checked_sub(left)
            .filter(|value| *value > 0)
            .ok_or(ExtractionError::OcrOutputInvalid)?;
        let height_ppm = bottom
            .checked_sub(top)
            .filter(|value| *value > 0)
            .ok_or(ExtractionError::OcrOutputInvalid)?;
        let bounding_box = OcrBoundingBox {
            x_ppm: left,
            y_ppm: top,
            width_ppm,
            height_ppm,
        };
        if !bounding_box.is_valid() {
            return Err(ExtractionError::OcrOutputInvalid);
        }
        Ok(bounding_box)
    }

    fn to_ppm(value: f64) -> Result<u32, ExtractionError> {
        let scaled = (value * f64::from(NORMALIZED_SCALE)).round();
        if !scaled.is_finite() || scaled < 0.0 || scaled > f64::from(NORMALIZED_SCALE) {
            return Err(ExtractionError::OcrOutputInvalid);
        }
        Ok(scaled as u32)
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn vision_boxes_are_normalized_to_top_left_integer_coordinates() {
            let bounding_box = normalized_top_left_box(0.05, 0.43, 0.46, 0.15)
                .expect("valid Vision box should convert");
            assert_eq!(
                bounding_box,
                OcrBoundingBox {
                    x_ppm: 50_000,
                    y_ppm: 420_000,
                    width_ppm: 460_000,
                    height_ppm: 150_000,
                }
            );
            assert!(normalized_top_left_box(0.9, 0.0, 0.2, 0.1).is_err());
            assert!(normalized_top_left_box(0.0, 0.0, 0.0, 0.1).is_err());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Clone, Copy, Debug)]
    struct FakeProvider;

    impl OcrProvider for FakeProvider {
        fn provider_id(&self) -> &'static str {
            "deskgraph.fake-ocr"
        }

        fn provider_version(&self) -> &'static str {
            "1"
        }

        fn recognize(
            &self,
            _encoded_image: &[u8],
            _request: OcrRequest,
            _limits: OcrProviderLimits,
            _control: &OcrControl,
        ) -> Result<OcrOutput, ExtractionError> {
            unreachable!("core output tests provide fake output directly")
        }
    }

    fn request() -> OcrRequest {
        OcrRequest {
            format: ImageFormat::Png,
            expected_source_bytes: 64,
            modified_unix_ns: Some(7),
            pixel_width: 640,
            pixel_height: 480,
        }
    }

    fn compact_limits() -> ExtractionLimits {
        ExtractionLimits {
            max_source_bytes: 1_024,
            max_output_bytes: 1_024,
            max_chunks: 16,
            max_chunk_bytes: 12,
            chunk_overlap_bytes: 3,
            max_decompressed_bytes: 1_024,
            max_pdf_pages: 8,
            max_image_source_bytes: 1_024,
            max_image_probe_bytes: 1_024,
            max_image_dimension: 1_024,
            max_image_pixels: 1_048_576,
            max_processing_time: Duration::from_secs(1),
        }
    }

    #[test]
    fn ocr_output_is_bounded_chunked_and_keeps_spatial_provenance() {
        let provider = FakeProvider;
        let control = OcrControl::new(Duration::from_secs(1));
        let output = build_ocr_extraction_output(
            &provider,
            request(),
            compact_limits(),
            &control,
            OcrOutput {
                observations: vec![OcrObservation {
                    text: "DeskGraph 桌面圖譜".to_string(),
                    bounding_box: OcrBoundingBox {
                        x_ppm: 10_000,
                        y_ppm: 20_000,
                        width_ppm: 300_000,
                        height_ppm: 100_000,
                    },
                    confidence_basis_points: Some(9_000),
                }],
            },
        )
        .expect("bounded OCR output should build");

        assert!(output.chunks.len() > 1);
        assert_eq!(output.provider_id, "deskgraph.fake-ocr");
        for (index, chunk) in output.chunks.iter().enumerate() {
            let ChunkProvenance::OcrObservation {
                observation_number,
                fragment_index,
                bounding_box,
                confidence_basis_points,
            } = chunk.provenance
            else {
                panic!("OCR chunks must use spatial provenance");
            };
            assert_eq!(observation_number, 1);
            assert_eq!(fragment_index, index as u32);
            assert_eq!(bounding_box.x_ppm, 10_000);
            assert_eq!(confidence_basis_points, Some(9_000));
            assert_eq!(chunk.trust_class, UNTRUSTED_TEXT);
        }
    }

    #[test]
    fn ocr_output_preserves_absent_provider_confidence() {
        let output = build_ocr_extraction_output(
            &FakeProvider,
            request(),
            compact_limits(),
            &OcrControl::new(Duration::from_secs(1)),
            OcrOutput {
                observations: vec![OcrObservation {
                    text: "DeskGraph".to_string(),
                    bounding_box: OcrBoundingBox {
                        x_ppm: 10_000,
                        y_ppm: 20_000,
                        width_ppm: 300_000,
                        height_ppm: 100_000,
                    },
                    confidence_basis_points: None,
                }],
            },
        )
        .expect("a provider without a confidence API should remain honest");

        let ChunkProvenance::OcrObservation {
            confidence_basis_points,
            ..
        } = output.chunks[0].provenance
        else {
            panic!("OCR chunks must use spatial provenance");
        };
        assert_eq!(confidence_basis_points, None);
    }

    #[test]
    fn invalid_or_cancelled_ocr_output_publishes_nothing() {
        let provider = FakeProvider;
        let limits = compact_limits();
        let invalid = build_ocr_extraction_output(
            &provider,
            request(),
            limits,
            &OcrControl::new(Duration::from_secs(1)),
            OcrOutput {
                observations: vec![OcrObservation {
                    text: "text".to_string(),
                    bounding_box: OcrBoundingBox {
                        x_ppm: 900_000,
                        y_ppm: 0,
                        width_ppm: 200_000,
                        height_ppm: 1,
                    },
                    confidence_basis_points: Some(10_000),
                }],
            },
        )
        .expect_err("invalid provenance must fail closed");
        assert_eq!(invalid, ExtractionError::OcrOutputInvalid);

        let control = OcrControl::new(Duration::from_secs(1));
        control.cancellation().cancel();
        let cancelled = build_ocr_extraction_output(
            &provider,
            request(),
            limits,
            &control,
            OcrOutput {
                observations: Vec::new(),
            },
        )
        .expect_err("cancelled output must not build");
        assert_eq!(cancelled, ExtractionError::Cancelled);
    }

    #[test]
    fn ocr_request_rejects_unsupported_oversized_and_dimension_bomb_images() {
        let limits = compact_limits();

        let mut unsupported = request();
        unsupported.format = ImageFormat::Gif;
        assert_eq!(
            validate_ocr_request(unsupported, limits),
            Err(ExtractionError::UnsupportedMediaKind)
        );

        let mut oversized = request();
        oversized.expected_source_bytes = 1_025;
        assert_eq!(
            validate_ocr_request(oversized, limits),
            Err(ExtractionError::SourceTooLarge)
        );

        let mut oversized_dimension = request();
        oversized_dimension.pixel_width = 1_025;
        assert_eq!(
            validate_ocr_request(oversized_dimension, limits),
            Err(ExtractionError::ImageDimensionLimitExceeded)
        );

        let mut pixel_limits = limits;
        pixel_limits.max_image_dimension = 2_048;
        let mut pixel_bomb = request();
        pixel_bomb.pixel_width = 1_024;
        pixel_bomb.pixel_height = 1_025;
        assert_eq!(
            validate_ocr_request(pixel_bomb, pixel_limits),
            Err(ExtractionError::ImageDimensionLimitExceeded)
        );
    }

    #[test]
    fn ocr_output_rejects_observation_and_overlap_expansion_limits() {
        let provider = FakeProvider;
        let mut observation_limits = compact_limits();
        observation_limits.max_chunks = 1;
        assert_eq!(
            build_ocr_extraction_output(
                &provider,
                request(),
                observation_limits,
                &OcrControl::new(Duration::from_secs(1)),
                OcrOutput {
                    observations: vec![
                        OcrObservation {
                            text: "one".to_string(),
                            bounding_box: OcrBoundingBox {
                                x_ppm: 0,
                                y_ppm: 0,
                                width_ppm: 100_000,
                                height_ppm: 100_000,
                            },
                            confidence_basis_points: Some(9_000),
                        },
                        OcrObservation {
                            text: "two".to_string(),
                            bounding_box: OcrBoundingBox {
                                x_ppm: 100_000,
                                y_ppm: 0,
                                width_ppm: 100_000,
                                height_ppm: 100_000,
                            },
                            confidence_basis_points: Some(9_000),
                        },
                    ],
                },
            ),
            Err(ExtractionError::OcrObservationLimitExceeded)
        );

        let mut overlap_limits = compact_limits();
        overlap_limits.max_output_bytes = 12;
        overlap_limits.max_chunk_bytes = 8;
        overlap_limits.chunk_overlap_bytes = 4;
        assert_eq!(
            build_ocr_extraction_output(
                &provider,
                request(),
                overlap_limits,
                &OcrControl::new(Duration::from_secs(1)),
                OcrOutput {
                    observations: vec![OcrObservation {
                        text: "abcdefghijkl".to_string(),
                        bounding_box: OcrBoundingBox {
                            x_ppm: 0,
                            y_ppm: 0,
                            width_ppm: 100_000,
                            height_ppm: 100_000,
                        },
                        confidence_basis_points: Some(9_000),
                    }],
                },
            ),
            Err(ExtractionError::OutputTooLarge)
        );
    }

    #[test]
    fn ocr_control_enforces_deadline_without_provider_cooperation() {
        let control = OcrControl::new(Duration::from_millis(1));
        std::thread::sleep(Duration::from_millis(10));
        assert_eq!(control.check(), Err(ExtractionError::TimeLimitExceeded));
    }
}
