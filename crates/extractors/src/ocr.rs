#[cfg(any(target_os = "windows", test))]
use std::collections::HashSet;
#[cfg(any(target_os = "windows", test))]
use std::sync::mpsc::{Receiver, RecvTimeoutError};
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
#[cfg(any(target_os = "windows", test))]
const OCR_WORKER_POLL_INTERVAL: Duration = Duration::from_millis(10);

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

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
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
        encoded_image: Vec<u8>,
        request: OcrRequest,
        limits: OcrProviderLimits,
        control: &OcrControl,
    ) -> Result<OcrOutput, ExtractionError>;
}

#[cfg(target_os = "macos")]
#[derive(Clone, Copy, Debug, Default)]
pub struct NativeOcrProvider;

#[cfg(target_os = "windows")]
#[derive(Clone, Copy, Debug, Default)]
pub struct NativeOcrProvider;

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
#[derive(Clone, Copy, Debug, Default)]
pub struct NativeOcrProvider;

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
impl OcrProvider for NativeOcrProvider {
    fn provider_id(&self) -> &'static str {
        "deskgraph.native-ocr-unavailable"
    }

    fn provider_version(&self) -> &'static str {
        "1"
    }

    fn recognize(
        &self,
        _encoded_image: Vec<u8>,
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

/// Runs a bounded OCR provider invocation over core-owned image bytes.
///
/// Callers must obtain `encoded_image` through an authorized, bounded source.
/// This adapter validates the request and every provider-produced observation so
/// alternate callers cannot accidentally bypass the product OCR safety limits.
pub fn recognize_ocr_image_bytes(
    provider: &dyn OcrProvider,
    encoded_image: Vec<u8>,
    request: OcrRequest,
    limits: ExtractionLimits,
    control: &OcrControl,
) -> Result<OcrOutput, ExtractionError> {
    control.check()?;
    if u64::try_from(encoded_image.len()).ok() != Some(request.expected_source_bytes) {
        return Err(ExtractionError::SourceChanged);
    }
    let provider_limits = validate_ocr_request(request, limits)?;
    let output = provider.recognize(encoded_image, request, provider_limits, control)?;
    validate_ocr_output(&output, provider_limits, control)?;
    Ok(output)
}

fn validate_ocr_output(
    output: &OcrOutput,
    provider_limits: OcrProviderLimits,
    control: &OcrControl,
) -> Result<(), ExtractionError> {
    control.check()?;
    if output.observations.len() > provider_limits.max_observations {
        return Err(ExtractionError::OcrObservationLimitExceeded);
    }
    let mut output_bytes = 0_u64;
    for observation in &output.observations {
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
        output_bytes = output_bytes
            .checked_add(
                u64::try_from(observation.text.len())
                    .map_err(|_| ExtractionError::OutputTooLarge)?,
            )
            .ok_or(ExtractionError::OutputTooLarge)?;
        if output_bytes > provider_limits.max_output_bytes {
            return Err(ExtractionError::OutputTooLarge);
        }
    }
    control.check()
}

pub(crate) fn build_ocr_extraction_output(
    provider: &dyn OcrProvider,
    request: OcrRequest,
    limits: ExtractionLimits,
    control: &OcrControl,
    output: OcrOutput,
) -> Result<ExtractionOutput, ExtractionError> {
    let provider_limits = validate_ocr_request(request, limits)?;
    validate_ocr_output(&output, provider_limits, control)?;
    let mut chunks = Vec::new();
    let mut output_bytes = 0_u64;
    for (observation_index, observation) in output.observations.into_iter().enumerate() {
        control.check()?;
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

#[cfg(any(target_os = "windows", test))]
fn normalized_pixel_box(
    x: f64,
    y: f64,
    width: f64,
    height: f64,
    pixel_width: u32,
    pixel_height: u32,
) -> Result<OcrBoundingBox, ExtractionError> {
    let image_width = f64::from(pixel_width);
    let image_height = f64::from(pixel_height);
    if pixel_width == 0
        || pixel_height == 0
        || !x.is_finite()
        || !y.is_finite()
        || !width.is_finite()
        || !height.is_finite()
        || x < 0.0
        || y < 0.0
        || width <= 0.0
        || height <= 0.0
        || x + width > image_width
        || y + height > image_height
    {
        return Err(ExtractionError::OcrOutputInvalid);
    }

    let left = ((x.clamp(0.0, image_width) / image_width) * f64::from(NORMALIZED_SCALE)).round();
    let right =
        (((x + width).clamp(0.0, image_width) / image_width) * f64::from(NORMALIZED_SCALE)).round();
    let top = ((y.clamp(0.0, image_height) / image_height) * f64::from(NORMALIZED_SCALE)).round();
    let bottom = (((y + height).clamp(0.0, image_height) / image_height)
        * f64::from(NORMALIZED_SCALE))
    .round();
    if !left.is_finite()
        || !right.is_finite()
        || !top.is_finite()
        || !bottom.is_finite()
        || left < 0.0
        || top < 0.0
        || right > f64::from(NORMALIZED_SCALE)
        || bottom > f64::from(NORMALIZED_SCALE)
    {
        return Err(ExtractionError::OcrOutputInvalid);
    }

    let left = left as u32;
    let right = right as u32;
    let top = top as u32;
    let bottom = bottom as u32;
    let bounding_box = OcrBoundingBox {
        x_ppm: left,
        y_ppm: top,
        width_ppm: right
            .checked_sub(left)
            .filter(|value| *value > 0)
            .ok_or(ExtractionError::OcrOutputInvalid)?,
        height_ppm: bottom
            .checked_sub(top)
            .filter(|value| *value > 0)
            .ok_or(ExtractionError::OcrOutputInvalid)?,
    };
    if !bounding_box.is_valid() {
        return Err(ExtractionError::OcrOutputInvalid);
    }
    Ok(bounding_box)
}

#[cfg(any(target_os = "windows", test))]
fn push_unique_observation(
    observations: &mut Vec<OcrObservation>,
    seen: &mut HashSet<(String, OcrBoundingBox)>,
    output_bytes: &mut u64,
    candidate: OcrObservation,
    limits: OcrProviderLimits,
) -> Result<(), ExtractionError> {
    if candidate.text.is_empty()
        || candidate.text.len() > limits.max_observation_bytes
        || !candidate.bounding_box.is_valid()
        || candidate
            .confidence_basis_points
            .is_some_and(|value| value > 10_000)
    {
        return Err(ExtractionError::OcrOutputInvalid);
    }
    let key = (candidate.text.clone(), candidate.bounding_box);
    if seen.contains(&key) {
        return Ok(());
    }
    if observations.len() >= limits.max_observations {
        return Err(ExtractionError::OcrObservationLimitExceeded);
    }
    let candidate_bytes =
        u64::try_from(candidate.text.len()).map_err(|_| ExtractionError::OutputTooLarge)?;
    let next_output_bytes = output_bytes
        .checked_add(candidate_bytes)
        .ok_or(ExtractionError::OutputTooLarge)?;
    if next_output_bytes > limits.max_output_bytes {
        return Err(ExtractionError::OutputTooLarge);
    }
    seen.insert(key);
    *output_bytes = next_output_bytes;
    observations.push(candidate);
    Ok(())
}

#[cfg(any(target_os = "windows", test))]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum OcrAsyncStatus {
    Started,
    Completed,
    Canceled,
    Error,
}

#[cfg(any(target_os = "windows", test))]
trait OcrAsyncOperation<T> {
    fn status(&self) -> Result<OcrAsyncStatus, ExtractionError>;
    fn cancel(&self);
    fn close(&self);
    fn results(&self) -> Result<T, ExtractionError>;
}

#[cfg(any(target_os = "windows", test))]
fn wait_for_bounded_operation<T>(
    operation: &impl OcrAsyncOperation<T>,
    control: &OcrControl,
) -> Result<T, ExtractionError> {
    let mut pending_control_error = None;
    let mut cancel_requested = false;
    loop {
        if pending_control_error.is_none()
            && let Err(error) = control.check()
        {
            pending_control_error = Some(error);
            operation.cancel();
            cancel_requested = true;
        }

        let status = match operation.status() {
            Ok(status) => status,
            Err(error) => {
                if !cancel_requested {
                    operation.cancel();
                }
                // A failed status query gives no proof that the operation is terminal.
                // Dropping the interface is safer than calling Close prematurely.
                return Err(pending_control_error.unwrap_or(error));
            }
        };
        match status {
            OcrAsyncStatus::Started => std::thread::sleep(OCR_WORKER_POLL_INTERVAL),
            OcrAsyncStatus::Completed => {
                if let Some(error) = pending_control_error {
                    operation.close();
                    return Err(error);
                }
                if let Err(error) = control.check() {
                    operation.close();
                    return Err(error);
                }
                let result = operation.results();
                let control_result = control.check();
                operation.close();
                control_result?;
                return result;
            }
            OcrAsyncStatus::Canceled | OcrAsyncStatus::Error => {
                let control_error = pending_control_error.or_else(|| control.check().err());
                operation.close();
                return Err(control_error.unwrap_or(ExtractionError::OcrProviderFailed));
            }
        }
    }
}

#[cfg(any(target_os = "windows", test))]
fn receive_bounded_worker_result<T>(
    receiver: &Receiver<Result<T, ExtractionError>>,
    control: &OcrControl,
) -> Result<Result<T, ExtractionError>, ExtractionError> {
    loop {
        control.check()?;
        match receiver.recv_timeout(OCR_WORKER_POLL_INTERVAL) {
            Ok(result) => return Ok(result),
            Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => {
                return Err(ExtractionError::OcrProviderFailed);
            }
        }
    }
}

#[cfg(any(target_os = "windows", test))]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum RequiredOcrLanguage {
    TraditionalChinese,
    English,
}

#[cfg(any(target_os = "windows", test))]
fn resolved_language_satisfies(required: RequiredOcrLanguage, tag: &str) -> bool {
    let mut parts = tag.split('-');
    let Some(primary) = parts.next() else {
        return false;
    };
    match required {
        RequiredOcrLanguage::English => primary.eq_ignore_ascii_case("en"),
        RequiredOcrLanguage::TraditionalChinese => {
            if !primary.eq_ignore_ascii_case("zh") {
                return false;
            }
            let parts = parts.collect::<Vec<_>>();
            !parts.iter().any(|part| part.eq_ignore_ascii_case("hans"))
                && parts.iter().any(|part| {
                    part.eq_ignore_ascii_case("hant")
                        || part.eq_ignore_ascii_case("tw")
                        || part.eq_ignore_ascii_case("hk")
                        || part.eq_ignore_ascii_case("mo")
                })
        }
    }
}

#[cfg(any(target_os = "windows", test))]
fn validate_source_aligned_text_angle(angle: Option<f64>) -> Result<(), ExtractionError> {
    if angle.is_some_and(|value| !value.is_finite() || value != 0.0) {
        return Err(ExtractionError::OcrOutputInvalid);
    }
    Ok(())
}

#[cfg(target_os = "windows")]
mod windows_native {
    use std::collections::HashSet;
    use std::ptr;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::mpsc;
    use std::thread;

    use windows::{
        Globalization::Language,
        Graphics::Imaging::{BitmapDecoder, SoftwareBitmap},
        Media::Ocr::{OcrEngine, OcrResult},
        Storage::Streams::{DataWriter, InMemoryRandomAccessStream},
        Win32::System::WinRT::{RO_INIT_MULTITHREADED, RoInitialize, RoUninitialize},
        core::{HSTRING, RuntimeType},
    };
    use windows_future::{AsyncStatus, IAsyncOperation};
    use windows_sys::Win32::{
        Foundation::{APPMODEL_ERROR_NO_PACKAGE, ERROR_INSUFFICIENT_BUFFER},
        Storage::Packaging::Appx::GetCurrentPackageFullName,
    };

    use super::{
        NativeOcrProvider, OcrAsyncOperation, OcrAsyncStatus, OcrBoundingBox, OcrControl,
        OcrObservation, OcrOutput, OcrProvider, OcrProviderLimits, OcrRequest, RequiredOcrLanguage,
        normalized_pixel_box, push_unique_observation, receive_bounded_worker_result,
        resolved_language_satisfies, validate_source_aligned_text_angle,
        wait_for_bounded_operation,
    };
    use crate::ExtractionError;

    const PROVIDER_ID: &str = "deskgraph.windows-media-ocr";
    const PROVIDER_VERSION: &str = "1";
    const LANGUAGE_PASSES: [(&str, RequiredOcrLanguage); 2] = [
        ("zh-TW", RequiredOcrLanguage::TraditionalChinese),
        ("en-US", RequiredOcrLanguage::English),
    ];
    // A timed-out native operation may still be draining cancellation. Keep at
    // most one owned worker alive so a stuck OS operation cannot accumulate.
    static WINDOWS_OCR_WORKER_ACTIVE: AtomicBool = AtomicBool::new(false);

    impl OcrProvider for NativeOcrProvider {
        fn provider_id(&self) -> &'static str {
            PROVIDER_ID
        }

        fn provider_version(&self) -> &'static str {
            PROVIDER_VERSION
        }

        fn recognize(
            &self,
            encoded_image: Vec<u8>,
            request: OcrRequest,
            limits: OcrProviderLimits,
            control: &OcrControl,
        ) -> Result<OcrOutput, ExtractionError> {
            control.check()?;
            if u64::try_from(encoded_image.len()).ok() != Some(request.expected_source_bytes) {
                return Err(ExtractionError::SourceChanged);
            }
            u32::try_from(encoded_image.len()).map_err(|_| ExtractionError::SourceTooLarge)?;
            if WINDOWS_OCR_WORKER_ACTIVE
                .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
                .is_err()
            {
                return Err(ExtractionError::OcrProviderUnavailable);
            }

            let worker_control = control.clone();
            let (sender, receiver) = mpsc::sync_channel(1);
            let worker = match thread::Builder::new()
                .name("deskgraph-windows-ocr".to_string())
                .spawn(move || {
                    let _active_worker = ActiveWorkerGuard;
                    let result =
                        recognize_on_mta_worker(&encoded_image, request, limits, &worker_control);
                    let _ = sender.send(result);
                }) {
                Ok(worker) => worker,
                Err(_) => {
                    WINDOWS_OCR_WORKER_ACTIVE.store(false, Ordering::Release);
                    return Err(ExtractionError::OcrProviderUnavailable);
                }
            };

            match receive_bounded_worker_result(&receiver, control) {
                Ok(result) => {
                    worker
                        .join()
                        .map_err(|_| ExtractionError::OcrProviderFailed)?;
                    result
                }
                Err(error) => {
                    // The owned worker keeps the image, MTA, and async operation
                    // alive until cleanup finishes; dropping the handle detaches it.
                    drop(worker);
                    Err(error)
                }
            }
        }
    }

    struct ActiveWorkerGuard;

    impl Drop for ActiveWorkerGuard {
        fn drop(&mut self) {
            WINDOWS_OCR_WORKER_ACTIVE.store(false, Ordering::Release);
        }
    }

    fn recognize_on_mta_worker(
        encoded_image: &[u8],
        request: OcrRequest,
        limits: OcrProviderLimits,
        control: &OcrControl,
    ) -> Result<OcrOutput, ExtractionError> {
        control.check()?;
        if !has_package_identity() {
            return Err(ExtractionError::OcrProviderUnavailable);
        }
        let _apartment = MtaApartment::initialize()?;
        control.check()?;

        let stream = InMemoryRandomAccessStream::new()
            .map_err(|_| ExtractionError::OcrProviderUnavailable)?;
        let writer = DataWriter::CreateDataWriter(&stream)
            .map_err(|_| ExtractionError::OcrProviderFailed)?;
        writer
            .WriteBytes(encoded_image)
            .map_err(|_| ExtractionError::OcrProviderFailed)?;
        let store = writer
            .StoreAsync()
            .map_err(|_| ExtractionError::OcrProviderFailed)?;
        let stored_bytes = wait_for_operation(&store, control)?;
        if usize::try_from(stored_bytes).ok() != Some(encoded_image.len()) {
            return Err(ExtractionError::OcrProviderFailed);
        }
        writer
            .DetachStream()
            .map_err(|_| ExtractionError::OcrProviderFailed)?;
        stream
            .Seek(0)
            .map_err(|_| ExtractionError::OcrProviderFailed)?;

        let create_decoder =
            BitmapDecoder::CreateAsync(&stream).map_err(|_| ExtractionError::OcrProviderFailed)?;
        let decoder = wait_for_operation(&create_decoder, control)?;
        let decoder_width = decoder
            .PixelWidth()
            .map_err(|_| ExtractionError::OcrProviderFailed)?;
        let decoder_height = decoder
            .PixelHeight()
            .map_err(|_| ExtractionError::OcrProviderFailed)?;
        let oriented_width = decoder
            .OrientedPixelWidth()
            .map_err(|_| ExtractionError::OcrProviderFailed)?;
        let oriented_height = decoder
            .OrientedPixelHeight()
            .map_err(|_| ExtractionError::OcrProviderFailed)?;
        if decoder_width != request.pixel_width || decoder_height != request.pixel_height {
            return Err(ExtractionError::OcrOutputInvalid);
        }

        let engine_max_dimension =
            OcrEngine::MaxImageDimension().map_err(|_| ExtractionError::OcrProviderUnavailable)?;
        if decoder_width == 0
            || decoder_height == 0
            || oriented_width == 0
            || oriented_height == 0
            || decoder_width > engine_max_dimension
            || decoder_height > engine_max_dimension
            || oriented_width > engine_max_dimension
            || oriented_height > engine_max_dimension
        {
            return Err(ExtractionError::ImageDimensionLimitExceeded);
        }

        let get_bitmap = decoder
            .GetSoftwareBitmapAsync()
            .map_err(|_| ExtractionError::OcrProviderFailed)?;
        let bitmap = wait_for_operation(&get_bitmap, control)?;
        let bitmap_width = u32::try_from(
            bitmap
                .PixelWidth()
                .map_err(|_| ExtractionError::OcrProviderFailed)?,
        )
        .map_err(|_| ExtractionError::OcrOutputInvalid)?;
        let bitmap_height = u32::try_from(
            bitmap
                .PixelHeight()
                .map_err(|_| ExtractionError::OcrProviderFailed)?,
        )
        .map_err(|_| ExtractionError::OcrOutputInvalid)?;
        if bitmap_width != request.pixel_width
            || bitmap_height != request.pixel_height
            || bitmap_width == 0
            || bitmap_height == 0
            || bitmap_width > engine_max_dimension
            || bitmap_height > engine_max_dimension
        {
            return Err(ExtractionError::OcrOutputInvalid);
        }

        let engines = create_required_engines()?;
        let mut observations = Vec::new();
        let mut seen = HashSet::new();
        let mut output_bytes = 0_u64;
        let mut processed_words = 0_usize;
        for engine in engines {
            control.check()?;
            recognize_language_pass(
                &engine,
                &bitmap,
                bitmap_width,
                bitmap_height,
                limits,
                control,
                &mut observations,
                &mut seen,
                &mut output_bytes,
                &mut processed_words,
            )?;
        }
        control.check()?;
        Ok(OcrOutput { observations })
    }

    fn create_required_engines() -> Result<[OcrEngine; 2], ExtractionError> {
        let create = |tag: &str, required: RequiredOcrLanguage| {
            let language = Language::CreateLanguage(&HSTRING::from(tag))
                .map_err(|_| ExtractionError::OcrLanguageUnavailable)?;
            if !OcrEngine::IsLanguageSupported(&language)
                .map_err(|_| ExtractionError::OcrProviderUnavailable)?
            {
                return Err(ExtractionError::OcrLanguageUnavailable);
            }
            let engine = OcrEngine::TryCreateFromLanguage(&language)
                .map_err(|_| ExtractionError::OcrLanguageUnavailable)?;
            let resolved_tag = engine
                .RecognizerLanguage()
                .and_then(|language| language.LanguageTag())
                .map_err(|_| ExtractionError::OcrLanguageUnavailable)?
                .to_string();
            if !resolved_language_satisfies(required, &resolved_tag) {
                return Err(ExtractionError::OcrLanguageUnavailable);
            }
            Ok(engine)
        };
        Ok([
            create(LANGUAGE_PASSES[0].0, LANGUAGE_PASSES[0].1)?,
            create(LANGUAGE_PASSES[1].0, LANGUAGE_PASSES[1].1)?,
        ])
    }

    #[allow(clippy::too_many_arguments)]
    fn recognize_language_pass(
        engine: &OcrEngine,
        bitmap: &SoftwareBitmap,
        pixel_width: u32,
        pixel_height: u32,
        limits: OcrProviderLimits,
        control: &OcrControl,
        observations: &mut Vec<OcrObservation>,
        seen: &mut HashSet<(String, OcrBoundingBox)>,
        output_bytes: &mut u64,
        processed_words: &mut usize,
    ) -> Result<(), ExtractionError> {
        let operation = engine
            .RecognizeAsync(bitmap)
            .map_err(|_| ExtractionError::OcrProviderFailed)?;
        let result = wait_for_operation(&operation, control)?;
        validate_source_aligned_text_angle(read_text_angle(&result)?)?;
        let lines = result
            .Lines()
            .map_err(|_| ExtractionError::OcrProviderFailed)?;
        let line_count = lines
            .Size()
            .map_err(|_| ExtractionError::OcrProviderFailed)?;
        if usize::try_from(line_count)
            .ok()
            .is_none_or(|count| count > limits.max_observations)
        {
            return Err(ExtractionError::OcrObservationLimitExceeded);
        }

        for line_index in 0..line_count {
            control.check()?;
            let line = lines
                .GetAt(line_index)
                .map_err(|_| ExtractionError::OcrProviderFailed)?;
            let text = line
                .Text()
                .map_err(|_| ExtractionError::OcrProviderFailed)?
                .to_string();
            if text.is_empty() {
                continue;
            }
            if text.len() > limits.max_observation_bytes {
                return Err(ExtractionError::OcrOutputInvalid);
            }

            let words = line
                .Words()
                .map_err(|_| ExtractionError::OcrProviderFailed)?;
            let word_count = words
                .Size()
                .map_err(|_| ExtractionError::OcrProviderFailed)?;
            if word_count == 0
                || usize::try_from(word_count)
                    .ok()
                    .is_none_or(|count| count > limits.max_observations)
            {
                return Err(ExtractionError::OcrOutputInvalid);
            }
            let mut line_box = None;
            for word_index in 0..word_count {
                control.check()?;
                *processed_words = processed_words
                    .checked_add(1)
                    .ok_or(ExtractionError::OcrObservationLimitExceeded)?;
                let max_processed_words = limits
                    .max_observations
                    .checked_mul(LANGUAGE_PASSES.len())
                    .ok_or(ExtractionError::OcrObservationLimitExceeded)?;
                if *processed_words > max_processed_words {
                    return Err(ExtractionError::OcrObservationLimitExceeded);
                }
                let word = words
                    .GetAt(word_index)
                    .map_err(|_| ExtractionError::OcrProviderFailed)?;
                let rectangle = word
                    .BoundingRect()
                    .map_err(|_| ExtractionError::OcrProviderFailed)?;
                let word_box = normalized_pixel_box(
                    f64::from(rectangle.X),
                    f64::from(rectangle.Y),
                    f64::from(rectangle.Width),
                    f64::from(rectangle.Height),
                    pixel_width,
                    pixel_height,
                )?;
                line_box = Some(match line_box {
                    Some(current) => union_boxes(current, word_box)?,
                    None => word_box,
                });
            }
            push_unique_observation(
                observations,
                seen,
                output_bytes,
                OcrObservation {
                    text,
                    bounding_box: line_box.ok_or(ExtractionError::OcrOutputInvalid)?,
                    confidence_basis_points: None,
                },
                limits,
            )?;
        }
        Ok(())
    }

    fn union_boxes(
        first: OcrBoundingBox,
        second: OcrBoundingBox,
    ) -> Result<OcrBoundingBox, ExtractionError> {
        let left = first.x_ppm.min(second.x_ppm);
        let top = first.y_ppm.min(second.y_ppm);
        let right = first
            .x_ppm
            .checked_add(first.width_ppm)
            .and_then(|first_right| {
                second
                    .x_ppm
                    .checked_add(second.width_ppm)
                    .map(|second_right| first_right.max(second_right))
            })
            .ok_or(ExtractionError::OcrOutputInvalid)?;
        let bottom = first
            .y_ppm
            .checked_add(first.height_ppm)
            .and_then(|first_bottom| {
                second
                    .y_ppm
                    .checked_add(second.height_ppm)
                    .map(|second_bottom| first_bottom.max(second_bottom))
            })
            .ok_or(ExtractionError::OcrOutputInvalid)?;
        let bounding_box = OcrBoundingBox {
            x_ppm: left,
            y_ppm: top,
            width_ppm: right
                .checked_sub(left)
                .filter(|value| *value > 0)
                .ok_or(ExtractionError::OcrOutputInvalid)?,
            height_ppm: bottom
                .checked_sub(top)
                .filter(|value| *value > 0)
                .ok_or(ExtractionError::OcrOutputInvalid)?,
        };
        if !bounding_box.is_valid() {
            return Err(ExtractionError::OcrOutputInvalid);
        }
        Ok(bounding_box)
    }

    fn read_text_angle(result: &OcrResult) -> Result<Option<f64>, ExtractionError> {
        match result.TextAngle() {
            Ok(angle) => angle
                .Value()
                .map(Some)
                .map_err(|_| ExtractionError::OcrProviderFailed),
            Err(error) if error.code().is_ok() => Ok(None),
            Err(_) => Err(ExtractionError::OcrProviderFailed),
        }
    }

    struct WindowsAsyncOperation<'a, T: RuntimeType + 'static>(&'a IAsyncOperation<T>);

    impl<T> OcrAsyncOperation<T> for WindowsAsyncOperation<'_, T>
    where
        T: RuntimeType + 'static,
    {
        fn status(&self) -> Result<OcrAsyncStatus, ExtractionError> {
            match self
                .0
                .Status()
                .map_err(|_| ExtractionError::OcrProviderFailed)?
            {
                AsyncStatus::Started => Ok(OcrAsyncStatus::Started),
                AsyncStatus::Completed => Ok(OcrAsyncStatus::Completed),
                AsyncStatus::Canceled => Ok(OcrAsyncStatus::Canceled),
                AsyncStatus::Error => Ok(OcrAsyncStatus::Error),
                _ => Err(ExtractionError::OcrProviderFailed),
            }
        }

        fn cancel(&self) {
            let _ = self.0.Cancel();
        }

        fn close(&self) {
            let _ = self.0.Close();
        }

        fn results(&self) -> Result<T, ExtractionError> {
            self.0
                .GetResults()
                .map_err(|_| ExtractionError::OcrProviderFailed)
        }
    }

    fn wait_for_operation<T>(
        operation: &IAsyncOperation<T>,
        control: &OcrControl,
    ) -> Result<T, ExtractionError>
    where
        T: RuntimeType + 'static,
    {
        wait_for_bounded_operation(&WindowsAsyncOperation(operation), control)
    }

    fn has_package_identity() -> bool {
        let mut required_chars = 0_u32;
        let status = unsafe { GetCurrentPackageFullName(&mut required_chars, ptr::null_mut()) };
        if status == APPMODEL_ERROR_NO_PACKAGE {
            return false;
        }
        status == ERROR_INSUFFICIENT_BUFFER && required_chars > 0
    }

    struct MtaApartment;

    impl MtaApartment {
        fn initialize() -> Result<Self, ExtractionError> {
            unsafe { RoInitialize(RO_INIT_MULTITHREADED) }
                .map_err(|_| ExtractionError::OcrProviderUnavailable)?;
            Ok(Self)
        }
    }

    impl Drop for MtaApartment {
        fn drop(&mut self) {
            unsafe { RoUninitialize() };
        }
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
            encoded_image: Vec<u8>,
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

            let data = NSData::with_bytes(&encoded_image);
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
    use std::collections::VecDeque;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::{Arc, Mutex, mpsc};

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
            _encoded_image: Vec<u8>,
            _request: OcrRequest,
            _limits: OcrProviderLimits,
            _control: &OcrControl,
        ) -> Result<OcrOutput, ExtractionError> {
            unreachable!("core output tests provide fake output directly")
        }
    }

    #[derive(Clone, Debug)]
    struct CountingProvider {
        calls: Arc<AtomicUsize>,
        output: OcrOutput,
    }

    impl OcrProvider for CountingProvider {
        fn provider_id(&self) -> &'static str {
            "deskgraph.counting-fake-ocr"
        }

        fn provider_version(&self) -> &'static str {
            "1"
        }

        fn recognize(
            &self,
            _encoded_image: Vec<u8>,
            _request: OcrRequest,
            _limits: OcrProviderLimits,
            _control: &OcrControl,
        ) -> Result<OcrOutput, ExtractionError> {
            self.calls.fetch_add(1, Ordering::AcqRel);
            Ok(self.output.clone())
        }
    }

    fn valid_observation() -> OcrObservation {
        OcrObservation {
            text: "DeskGraph".to_string(),
            bounding_box: OcrBoundingBox {
                x_ppm: 0,
                y_ppm: 0,
                width_ppm: 100_000,
                height_ppm: 100_000,
            },
            confidence_basis_points: Some(9_000),
        }
    }

    struct FakeAsyncOperation<T> {
        statuses: Mutex<VecDeque<Result<OcrAsyncStatus, ExtractionError>>>,
        result: Mutex<Option<Result<T, ExtractionError>>>,
        events: Mutex<Vec<&'static str>>,
        cancel_during_results: Option<OcrCancellation>,
    }

    impl<T> FakeAsyncOperation<T> {
        fn new(
            statuses: impl IntoIterator<Item = Result<OcrAsyncStatus, ExtractionError>>,
            result: Option<Result<T, ExtractionError>>,
        ) -> Self {
            Self {
                statuses: Mutex::new(statuses.into_iter().collect()),
                result: Mutex::new(result),
                events: Mutex::new(Vec::new()),
                cancel_during_results: None,
            }
        }

        fn events(&self) -> Vec<&'static str> {
            self.events.lock().expect("events lock").clone()
        }
    }

    impl<T> OcrAsyncOperation<T> for FakeAsyncOperation<T> {
        fn status(&self) -> Result<OcrAsyncStatus, ExtractionError> {
            self.events.lock().expect("events lock").push("status");
            self.statuses
                .lock()
                .expect("statuses lock")
                .pop_front()
                .unwrap_or(Err(ExtractionError::OcrProviderFailed))
        }

        fn cancel(&self) {
            self.events.lock().expect("events lock").push("cancel");
        }

        fn close(&self) {
            self.events.lock().expect("events lock").push("close");
        }

        fn results(&self) -> Result<T, ExtractionError> {
            self.events.lock().expect("events lock").push("results");
            if let Some(cancellation) = &self.cancel_during_results {
                cancellation.cancel();
            }
            self.result
                .lock()
                .expect("result lock")
                .take()
                .unwrap_or(Err(ExtractionError::OcrProviderFailed))
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
    fn bounded_ocr_adapter_returns_a_valid_provider_output() {
        let calls = Arc::new(AtomicUsize::new(0));
        let provider = CountingProvider {
            calls: Arc::clone(&calls),
            output: OcrOutput {
                observations: vec![valid_observation()],
            },
        };

        let output = recognize_ocr_image_bytes(
            &provider,
            vec![0_u8; 64],
            request(),
            compact_limits(),
            &OcrControl::new(Duration::from_secs(1)),
        )
        .expect("a bounded valid provider output should pass");

        assert_eq!(calls.load(Ordering::Acquire), 1);
        assert_eq!(output.observations, vec![valid_observation()]);
    }

    #[test]
    fn bounded_ocr_adapter_rejects_length_mismatch_before_provider_call() {
        let calls = Arc::new(AtomicUsize::new(0));
        let provider = CountingProvider {
            calls: Arc::clone(&calls),
            output: OcrOutput {
                observations: vec![valid_observation()],
            },
        };

        let error = recognize_ocr_image_bytes(
            &provider,
            vec![0_u8; 63],
            request(),
            compact_limits(),
            &OcrControl::new(Duration::from_secs(1)),
        )
        .expect_err("mismatched bytes must fail closed before provider invocation");

        assert_eq!(error, ExtractionError::SourceChanged);
        assert_eq!(calls.load(Ordering::Acquire), 0);
    }

    #[test]
    fn bounded_ocr_adapter_rejects_invalid_provider_output() {
        let calls = Arc::new(AtomicUsize::new(0));
        let provider = CountingProvider {
            calls: Arc::clone(&calls),
            output: OcrOutput {
                observations: vec![OcrObservation {
                    bounding_box: OcrBoundingBox {
                        x_ppm: 900_000,
                        y_ppm: 0,
                        width_ppm: 200_000,
                        height_ppm: 1,
                    },
                    ..valid_observation()
                }],
            },
        };

        let error = recognize_ocr_image_bytes(
            &provider,
            vec![0_u8; 64],
            request(),
            compact_limits(),
            &OcrControl::new(Duration::from_secs(1)),
        )
        .expect_err("invalid provider observations must fail closed");

        assert_eq!(error, ExtractionError::OcrOutputInvalid);
        assert_eq!(calls.load(Ordering::Acquire), 1);
    }

    #[test]
    fn bounded_ocr_adapter_honors_cancellation_before_provider_call() {
        let calls = Arc::new(AtomicUsize::new(0));
        let provider = CountingProvider {
            calls: Arc::clone(&calls),
            output: OcrOutput {
                observations: vec![valid_observation()],
            },
        };
        let control = OcrControl::new(Duration::from_secs(1));
        control.cancellation().cancel();

        let error = recognize_ocr_image_bytes(
            &provider,
            vec![0_u8; 64],
            request(),
            compact_limits(),
            &control,
        )
        .expect_err("cancelled OCR must not invoke provider");

        assert_eq!(error, ExtractionError::Cancelled);
        assert_eq!(calls.load(Ordering::Acquire), 0);
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
    fn pixel_boxes_are_finite_top_left_and_bounded() {
        assert_eq!(
            normalized_pixel_box(64.0, 48.0, 320.0, 96.0, 640, 480)
                .expect("valid top-left pixel box should normalize"),
            OcrBoundingBox {
                x_ppm: 100_000,
                y_ppm: 100_000,
                width_ppm: 500_000,
                height_ppm: 200_000,
            }
        );
        assert!(normalized_pixel_box(f64::NAN, 0.0, 1.0, 1.0, 640, 480).is_err());
        assert!(normalized_pixel_box(-0.001, 0.0, 1.0, 1.0, 640, 480).is_err());
        assert!(normalized_pixel_box(0.0, -0.001, 1.0, 1.0, 640, 480).is_err());
        assert!(normalized_pixel_box(639.0, 0.0, 2.0, 1.0, 640, 480).is_err());
        assert!(normalized_pixel_box(0.0, 0.0, 0.0, 1.0, 640, 480).is_err());
        assert!(normalized_pixel_box(0.0, 0.0, 1.0, 1.0, 0, 480).is_err());
    }

    #[test]
    fn async_cancel_waits_for_terminal_status_before_close() {
        let control = OcrControl::new(Duration::from_secs(1));
        control.cancellation().cancel();
        let operation = FakeAsyncOperation::<u32>::new(
            [Ok(OcrAsyncStatus::Started), Ok(OcrAsyncStatus::Canceled)],
            None,
        );

        assert_eq!(
            wait_for_bounded_operation(&operation, &control),
            Err(ExtractionError::Cancelled)
        );
        assert_eq!(
            operation.events(),
            vec!["cancel", "status", "status", "close"]
        );
    }

    #[test]
    fn async_completion_race_never_publishes_after_cancel() {
        let control = OcrControl::new(Duration::from_secs(1));
        let mut operation =
            FakeAsyncOperation::new([Ok(OcrAsyncStatus::Completed)], Some(Ok(7_u32)));
        operation.cancel_during_results = Some(control.cancellation());

        assert_eq!(
            wait_for_bounded_operation(&operation, &control),
            Err(ExtractionError::Cancelled)
        );
        assert_eq!(operation.events(), vec!["status", "results", "close"]);
    }

    #[test]
    fn async_terminal_provider_error_closes_without_returning_results() {
        let control = OcrControl::new(Duration::from_secs(1));
        let operation = FakeAsyncOperation::<u32>::new([Ok(OcrAsyncStatus::Error)], None);

        assert_eq!(
            wait_for_bounded_operation(&operation, &control),
            Err(ExtractionError::OcrProviderFailed)
        );
        assert_eq!(operation.events(), vec!["status", "close"]);
    }

    #[test]
    fn async_deadline_requests_cancel_and_closes_only_after_terminal_status() {
        let control = OcrControl::new(Duration::ZERO);
        let operation = FakeAsyncOperation::<u32>::new([Ok(OcrAsyncStatus::Canceled)], None);

        assert_eq!(
            wait_for_bounded_operation(&operation, &control),
            Err(ExtractionError::TimeLimitExceeded)
        );
        assert_eq!(operation.events(), vec!["cancel", "status", "close"]);
    }

    #[test]
    fn async_status_failure_does_not_close_an_unknown_nonterminal_state() {
        let control = OcrControl::new(Duration::from_secs(1));
        let operation =
            FakeAsyncOperation::<u32>::new([Err(ExtractionError::OcrProviderFailed)], None);

        assert_eq!(
            wait_for_bounded_operation(&operation, &control),
            Err(ExtractionError::OcrProviderFailed)
        );
        assert_eq!(operation.events(), vec!["status", "cancel"]);
    }

    #[test]
    fn worker_result_wait_is_bounded_even_when_cleanup_has_not_finished() {
        let (_sender, receiver) = mpsc::sync_channel::<Result<u32, ExtractionError>>(1);
        let control = OcrControl::new(Duration::ZERO);
        let started = Instant::now();

        assert_eq!(
            receive_bounded_worker_result(&receiver, &control),
            Err(ExtractionError::TimeLimitExceeded)
        );
        assert!(started.elapsed() < Duration::from_millis(100));
    }

    #[test]
    fn worker_result_wait_preserves_completed_provider_results() {
        let (sender, receiver) = mpsc::sync_channel(1);
        sender
            .send(Ok(7_u32))
            .expect("bounded result channel should be open");

        assert_eq!(
            receive_bounded_worker_result(&receiver, &OcrControl::new(Duration::from_secs(1))),
            Ok(Ok(7))
        );
    }

    #[test]
    fn resolved_languages_must_preserve_traditional_chinese_and_english_capabilities() {
        assert!(resolved_language_satisfies(
            RequiredOcrLanguage::TraditionalChinese,
            "zh-TW"
        ));
        assert!(resolved_language_satisfies(
            RequiredOcrLanguage::TraditionalChinese,
            "zh-Hant-HK"
        ));
        assert!(!resolved_language_satisfies(
            RequiredOcrLanguage::TraditionalChinese,
            "zh-Hans-TW"
        ));
        assert!(!resolved_language_satisfies(
            RequiredOcrLanguage::TraditionalChinese,
            "zh-CN"
        ));
        assert!(resolved_language_satisfies(
            RequiredOcrLanguage::English,
            "en-GB"
        ));
        assert!(!resolved_language_satisfies(
            RequiredOcrLanguage::English,
            "fr-FR"
        ));
    }

    #[test]
    fn rotated_or_non_finite_windows_results_cannot_claim_source_aligned_boxes() {
        assert_eq!(validate_source_aligned_text_angle(None), Ok(()));
        assert_eq!(validate_source_aligned_text_angle(Some(0.0)), Ok(()));
        assert_eq!(
            validate_source_aligned_text_angle(Some(0.5)),
            Err(ExtractionError::OcrOutputInvalid)
        );
        assert_eq!(
            validate_source_aligned_text_angle(Some(f64::NAN)),
            Err(ExtractionError::OcrOutputInvalid)
        );
    }

    #[test]
    fn exact_text_and_spatial_duplicates_are_deterministic_and_not_double_counted() {
        let limits = OcrProviderLimits {
            max_output_bytes: 64,
            max_observations: 2,
            max_observation_bytes: 32,
        };
        let candidate = OcrObservation {
            text: "DeskGraph 桌面圖譜".to_string(),
            bounding_box: OcrBoundingBox {
                x_ppm: 10_000,
                y_ppm: 20_000,
                width_ppm: 300_000,
                height_ppm: 100_000,
            },
            confidence_basis_points: None,
        };
        let mut observations = Vec::new();
        let mut seen = HashSet::new();
        let mut output_bytes = 0;
        push_unique_observation(
            &mut observations,
            &mut seen,
            &mut output_bytes,
            candidate.clone(),
            limits,
        )
        .expect("first observation should be retained");
        let retained_bytes = output_bytes;
        push_unique_observation(
            &mut observations,
            &mut seen,
            &mut output_bytes,
            candidate,
            limits,
        )
        .expect("exact duplicate should be ignored");

        assert_eq!(observations.len(), 1);
        assert_eq!(output_bytes, retained_bytes);
        assert_eq!(observations[0].confidence_basis_points, None);

        let distinct_box = OcrObservation {
            text: observations[0].text.clone(),
            bounding_box: OcrBoundingBox {
                x_ppm: 20_000,
                ..observations[0].bounding_box
            },
            confidence_basis_points: None,
        };
        push_unique_observation(
            &mut observations,
            &mut seen,
            &mut output_bytes,
            distinct_box,
            limits,
        )
        .expect("same text at a distinct location must remain");
        assert_eq!(observations.len(), 2);
        assert_eq!(observations[0].bounding_box.x_ppm, 10_000);
        assert_eq!(observations[1].bounding_box.x_ppm, 20_000);
    }

    #[test]
    fn ocr_control_enforces_deadline_without_provider_cooperation() {
        let control = OcrControl::new(Duration::from_millis(1));
        std::thread::sleep(Duration::from_millis(10));
        assert_eq!(control.check(), Err(ExtractionError::TimeLimitExceeded));
    }
}
