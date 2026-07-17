use std::io::{self, BufRead, Cursor, Read, Seek, SeekFrom};
use std::time::Instant;

use deskgraph_domain::ImageFormat;
use imagesize::{ImageType, reader_type};

use crate::{
    CancellationSignal, ControlledSource, ExtractedImageMetadata, ExtractionError,
    ExtractionLimits, ExtractionOutput, ExtractionRequest, ExtractorProvider, MediaKind,
    check_control, validate_limits,
};

const PROVIDER_ID: &str = "deskgraph.image-metadata";
const PROVIDER_VERSION: &str = "1";
const MAX_PROBE_OPERATIONS: u32 = 65_536;

#[derive(Clone, Copy, Debug, Default)]
pub struct ImageMetadataExtractor;

impl ExtractorProvider for ImageMetadataExtractor {
    fn provider_id(&self) -> &'static str {
        PROVIDER_ID
    }

    fn provider_version(&self) -> &'static str {
        PROVIDER_VERSION
    }

    fn supports(&self, media_kind: MediaKind) -> bool {
        matches!(media_kind, MediaKind::Image(_))
    }

    fn extract(
        &self,
        source: &mut dyn ControlledSource,
        request: ExtractionRequest,
        limits: ExtractionLimits,
        cancellation: &dyn CancellationSignal,
    ) -> Result<ExtractionOutput, ExtractionError> {
        validate_limits(limits)?;
        let MediaKind::Image(expected_format) = request.media_kind else {
            return Err(ExtractionError::UnsupportedMediaKind);
        };
        if request.expected_source_bytes == 0 {
            return Err(ExtractionError::InvalidImage);
        }
        if request.expected_source_bytes > limits.max_image_source_bytes {
            return Err(ExtractionError::SourceTooLarge);
        }

        let started = Instant::now();
        let bytes = read_probe(source, request, limits, started, cancellation)?;
        let truncated = u64::try_from(bytes.len())
            .map_err(|_| ExtractionError::ImageMetadataLimitExceeded)?
            < request.expected_source_bytes;
        let mut reader =
            ProbeCursor::new(&bytes, started, limits.max_processing_time, cancellation);
        let image_type = match reader_type(&mut reader) {
            Ok(image_type) => image_type,
            Err(_) => return Err(reader.failure_or_parse_error(truncated)),
        };
        let format = format_for_image_type(image_type).ok_or(ExtractionError::InvalidImage)?;
        if format != expected_format {
            return Err(ExtractionError::ImageFormatMismatch);
        }
        validate_strict_header(format, &bytes, request.expected_source_bytes)?;
        let dimensions = match image_type.reader_size(&mut reader) {
            Ok(dimensions) => dimensions,
            Err(_) => return Err(reader.failure_or_parse_error(truncated)),
        };
        check_control(started, limits.max_processing_time, cancellation)?;
        let pixel_width = u32::try_from(dimensions.width)
            .map_err(|_| ExtractionError::ImageDimensionLimitExceeded)?;
        let pixel_height = u32::try_from(dimensions.height)
            .map_err(|_| ExtractionError::ImageDimensionLimitExceeded)?;
        let pixels = u64::from(pixel_width)
            .checked_mul(u64::from(pixel_height))
            .ok_or(ExtractionError::ImageDimensionLimitExceeded)?;
        if pixel_width == 0
            || pixel_height == 0
            || pixel_width > limits.max_image_dimension
            || pixel_height > limits.max_image_dimension
            || pixels > limits.max_image_pixels
        {
            return Err(ExtractionError::ImageDimensionLimitExceeded);
        }

        Ok(ExtractionOutput {
            provider_id: self.provider_id(),
            provider_version: self.provider_version(),
            media_kind: request.media_kind,
            source_bytes: request.expected_source_bytes,
            output_bytes: 0,
            modified_unix_ns: request.modified_unix_ns,
            chunks: Vec::new(),
            image_metadata: Some(ExtractedImageMetadata {
                format,
                pixel_width,
                pixel_height,
            }),
        })
    }
}

fn read_probe(
    source: &mut dyn ControlledSource,
    request: ExtractionRequest,
    limits: ExtractionLimits,
    started: Instant,
    cancellation: &dyn CancellationSignal,
) -> Result<Vec<u8>, ExtractionError> {
    source
        .seek(SeekFrom::Start(0))
        .map_err(|_| ExtractionError::SourceSeekFailed)?;
    let source_bytes =
        usize::try_from(request.expected_source_bytes).unwrap_or(limits.max_image_probe_bytes);
    let probe_bytes = source_bytes.min(limits.max_image_probe_bytes);
    let mut bytes = vec![0_u8; probe_bytes];
    let mut offset = 0_usize;
    while offset < bytes.len() {
        check_control(started, limits.max_processing_time, cancellation)?;
        let end = offset.saturating_add(64 * 1024).min(bytes.len());
        let read = source
            .read(&mut bytes[offset..end])
            .map_err(|_| ExtractionError::SourceReadFailed)?;
        if read == 0 {
            return Err(ExtractionError::SourceChanged);
        }
        offset = offset
            .checked_add(read)
            .ok_or(ExtractionError::ImageMetadataLimitExceeded)?;
    }
    check_control(started, limits.max_processing_time, cancellation)?;
    Ok(bytes)
}

fn format_for_image_type(image_type: ImageType) -> Option<ImageFormat> {
    match image_type {
        ImageType::Png => Some(ImageFormat::Png),
        ImageType::Jpeg => Some(ImageFormat::Jpeg),
        ImageType::Gif => Some(ImageFormat::Gif),
        ImageType::Webp => Some(ImageFormat::Webp),
        ImageType::Bmp => Some(ImageFormat::Bmp),
        ImageType::Tiff => Some(ImageFormat::Tiff),
        _ => None,
    }
}

fn validate_strict_header(
    format: ImageFormat,
    bytes: &[u8],
    source_bytes: u64,
) -> Result<(), ExtractionError> {
    let valid = match format {
        ImageFormat::Png => {
            bytes.len() >= 24
                && bytes.starts_with(b"\x89PNG\r\n\x1a\n")
                && bytes[8..12] == 13_u32.to_be_bytes()
                && &bytes[12..16] == b"IHDR"
        }
        ImageFormat::Jpeg => bytes.starts_with(b"\xff\xd8\xff"),
        ImageFormat::Gif => bytes.starts_with(b"GIF87a") || bytes.starts_with(b"GIF89a"),
        ImageFormat::Webp => {
            if bytes.len() < 16
                || &bytes[..4] != b"RIFF"
                || &bytes[8..12] != b"WEBP"
                || !matches!(&bytes[12..16], b"VP8 " | b"VP8L" | b"VP8X")
            {
                false
            } else {
                let riff_size = u32::from_le_bytes(
                    bytes[4..8]
                        .try_into()
                        .map_err(|_| ExtractionError::InvalidImage)?,
                );
                let minimum_file_bytes = match &bytes[12..16] {
                    b"VP8L" => 25,
                    b"VP8 " | b"VP8X" => 30,
                    _ => return Err(ExtractionError::InvalidImage),
                };
                u64::from(riff_size).checked_add(8).is_some_and(|declared| {
                    declared >= minimum_file_bytes && declared <= source_bytes
                })
            }
        }
        ImageFormat::Bmp => {
            bytes.len() >= 26
                && bytes.starts_with(b"BM")
                && u32::from_le_bytes(
                    bytes[14..18]
                        .try_into()
                        .map_err(|_| ExtractionError::InvalidImage)?,
                ) >= 40
        }
        ImageFormat::Tiff => {
            bytes.starts_with(b"II\x2a\x00")
                || bytes.starts_with(b"MM\x00\x2a")
                || bytes.starts_with(b"II\x2b\x00")
                || bytes.starts_with(b"MM\x00\x2b")
        }
    };
    if valid {
        Ok(())
    } else {
        Err(ExtractionError::InvalidImage)
    }
}

struct ProbeCursor<'a> {
    cursor: Cursor<&'a [u8]>,
    operations: u32,
    started: Instant,
    max_processing_time: std::time::Duration,
    cancellation: &'a dyn CancellationSignal,
    failure: Option<ExtractionError>,
}

impl<'a> ProbeCursor<'a> {
    fn new(
        bytes: &'a [u8],
        started: Instant,
        max_processing_time: std::time::Duration,
        cancellation: &'a dyn CancellationSignal,
    ) -> Self {
        Self {
            cursor: Cursor::new(bytes),
            operations: 0,
            started,
            max_processing_time,
            cancellation,
            failure: None,
        }
    }

    fn check_operation(&mut self) -> io::Result<()> {
        self.operations = self.operations.saturating_add(1);
        let result = if self.operations > MAX_PROBE_OPERATIONS {
            Err(ExtractionError::ImageMetadataLimitExceeded)
        } else {
            check_control(self.started, self.max_processing_time, self.cancellation)
        };
        if let Err(error) = result {
            self.failure = Some(error);
            return Err(io::Error::other(error.code()));
        }
        Ok(())
    }

    fn failure_or_parse_error(&self, truncated: bool) -> ExtractionError {
        self.failure.unwrap_or(if truncated {
            ExtractionError::ImageMetadataLimitExceeded
        } else {
            ExtractionError::InvalidImage
        })
    }
}

impl Read for ProbeCursor<'_> {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        self.check_operation()?;
        self.cursor.read(buffer)
    }
}

impl Seek for ProbeCursor<'_> {
    fn seek(&mut self, position: SeekFrom) -> io::Result<u64> {
        self.check_operation()?;
        self.cursor.seek(position)
    }
}

impl BufRead for ProbeCursor<'_> {
    fn fill_buf(&mut self) -> io::Result<&[u8]> {
        self.check_operation()?;
        self.cursor.fill_buf()
    }

    fn consume(&mut self, amount: usize) {
        self.cursor.consume(amount);
    }
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;
    use std::time::Duration;

    use super::*;
    use crate::{AtomicCancellation, NoCancellation};

    fn limits() -> ExtractionLimits {
        ExtractionLimits {
            max_source_bytes: 1024 * 1024,
            max_output_bytes: 1024 * 1024,
            max_chunks: 32,
            max_chunk_bytes: 4096,
            chunk_overlap_bytes: 0,
            max_decompressed_bytes: 1024 * 1024,
            max_pdf_pages: 8,
            max_image_source_bytes: 1024 * 1024,
            max_image_probe_bytes: 1024 * 1024,
            max_image_dimension: 100_000,
            max_image_pixels: 500_000_000,
            max_processing_time: Duration::from_secs(1),
        }
    }

    fn request(format: ImageFormat, bytes: &[u8]) -> ExtractionRequest {
        ExtractionRequest {
            media_kind: MediaKind::Image(format),
            expected_source_bytes: bytes.len() as u64,
            modified_unix_ns: Some(23),
        }
    }

    fn png(width: u32, height: u32) -> Vec<u8> {
        let mut bytes = vec![0_u8; 32];
        bytes[..8].copy_from_slice(b"\x89PNG\r\n\x1a\n");
        bytes[8..12].copy_from_slice(&13_u32.to_be_bytes());
        bytes[12..16].copy_from_slice(b"IHDR");
        bytes[16..20].copy_from_slice(&width.to_be_bytes());
        bytes[20..24].copy_from_slice(&height.to_be_bytes());
        bytes
    }

    fn jpeg(width: u16, height: u16) -> Vec<u8> {
        let mut bytes = vec![0_u8; 16];
        bytes[..2].copy_from_slice(b"\xff\xd8");
        bytes[2..4].copy_from_slice(b"\xff\xc0");
        bytes[4..6].copy_from_slice(&17_u16.to_be_bytes());
        bytes[6] = 8;
        bytes[7..9].copy_from_slice(&height.to_be_bytes());
        bytes[9..11].copy_from_slice(&width.to_be_bytes());
        bytes
    }

    fn gif(width: u16, height: u16) -> Vec<u8> {
        let mut bytes = vec![0_u8; 12];
        bytes[..6].copy_from_slice(b"GIF89a");
        bytes[6..8].copy_from_slice(&width.to_le_bytes());
        bytes[8..10].copy_from_slice(&height.to_le_bytes());
        bytes
    }

    fn webp(width: u32, height: u32) -> Vec<u8> {
        let mut bytes = vec![0_u8; 30];
        bytes[..4].copy_from_slice(b"RIFF");
        bytes[4..8].copy_from_slice(&22_u32.to_le_bytes());
        bytes[8..12].copy_from_slice(b"WEBP");
        bytes[12..16].copy_from_slice(b"VP8X");
        let width = width - 1;
        let height = height - 1;
        bytes[24..27].copy_from_slice(&width.to_le_bytes()[..3]);
        bytes[27..30].copy_from_slice(&height.to_le_bytes()[..3]);
        bytes
    }

    fn bmp(width: u32, height: u32) -> Vec<u8> {
        let mut bytes = vec![0_u8; 26];
        bytes[..2].copy_from_slice(b"BM");
        bytes[14..18].copy_from_slice(&40_u32.to_le_bytes());
        bytes[18..22].copy_from_slice(&width.to_le_bytes());
        bytes[22..26].copy_from_slice(&height.to_le_bytes());
        bytes
    }

    fn tiff(width: u32, height: u32) -> Vec<u8> {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(b"II\x2a\x00");
        bytes.extend_from_slice(&8_u32.to_le_bytes());
        bytes.extend_from_slice(&2_u16.to_le_bytes());
        for (tag, value) in [(0x100_u16, width), (0x101_u16, height)] {
            bytes.extend_from_slice(&tag.to_le_bytes());
            bytes.extend_from_slice(&4_u16.to_le_bytes());
            bytes.extend_from_slice(&1_u32.to_le_bytes());
            bytes.extend_from_slice(&value.to_le_bytes());
        }
        bytes
    }

    #[test]
    fn probes_six_allowlisted_formats_without_decoding_pixels() {
        let fixtures = [
            (ImageFormat::Png, png(1920, 1080)),
            (ImageFormat::Jpeg, jpeg(1600, 900)),
            (ImageFormat::Gif, gif(640, 480)),
            (ImageFormat::Webp, webp(1200, 800)),
            (ImageFormat::Bmp, bmp(800, 600)),
            (ImageFormat::Tiff, tiff(3000, 2000)),
        ];
        for (format, bytes) in fixtures {
            let output = ImageMetadataExtractor
                .extract(
                    &mut Cursor::new(&bytes),
                    request(format, &bytes),
                    limits(),
                    &NoCancellation,
                )
                .expect("allowlisted image metadata should parse");
            let metadata = output.image_metadata.expect("metadata should exist");
            assert_eq!(metadata.format, format);
            assert!(metadata.pixel_width > 0);
            assert!(metadata.pixel_height > 0);
            assert!(output.chunks.is_empty());
            assert_eq!(output.output_bytes, 0);
        }
    }

    #[test]
    fn rejects_corrupt_mismatched_and_dimension_bomb_headers() {
        let corrupt = b"not an image";
        let error = ImageMetadataExtractor
            .extract(
                &mut Cursor::new(corrupt),
                request(ImageFormat::Png, corrupt),
                limits(),
                &NoCancellation,
            )
            .expect_err("corrupt image should fail");
        assert_eq!(error, ExtractionError::InvalidImage);

        let mut fake_png = png(320, 240);
        fake_png[8..12].fill(0);
        let error = ImageMetadataExtractor
            .extract(
                &mut Cursor::new(&fake_png),
                request(ImageFormat::Png, &fake_png),
                limits(),
                &NoCancellation,
            )
            .expect_err("missing IHDR length should fail closed");
        assert_eq!(error, ExtractionError::InvalidImage);

        let png_bytes = png(320, 240);
        let mismatch = ImageMetadataExtractor
            .extract(
                &mut Cursor::new(&png_bytes),
                request(ImageFormat::Jpeg, &png_bytes),
                limits(),
                &NoCancellation,
            )
            .expect_err("extension and signature mismatch should fail");
        assert_eq!(mismatch, ExtractionError::ImageFormatMismatch);

        let bomb = png(25_000, 25_000);
        let over_limit = ImageMetadataExtractor
            .extract(
                &mut Cursor::new(&bomb),
                request(ImageFormat::Png, &bomb),
                limits(),
                &NoCancellation,
            )
            .expect_err("dimension bomb should fail before pixel decode");
        assert_eq!(over_limit, ExtractionError::ImageDimensionLimitExceeded);

        let mut undersized_webp = webp(320, 240);
        undersized_webp[4..8].copy_from_slice(&8_u32.to_le_bytes());
        let error = ImageMetadataExtractor
            .extract(
                &mut Cursor::new(&undersized_webp),
                request(ImageFormat::Webp, &undersized_webp),
                limits(),
                &NoCancellation,
            )
            .expect_err("undersized declared WebP container should fail closed");
        assert_eq!(error, ExtractionError::InvalidImage);
    }

    #[test]
    fn probe_limit_and_cancellation_fail_with_fixed_codes() {
        let mut delayed_jpeg = vec![0_u8; 80];
        delayed_jpeg[..2].copy_from_slice(b"\xff\xd8");
        delayed_jpeg[2..4].copy_from_slice(b"\xff\xe0");
        delayed_jpeg[4..6].copy_from_slice(&42_u16.to_be_bytes());
        delayed_jpeg[44..46].copy_from_slice(b"\xff\xc0");
        delayed_jpeg[46..48].copy_from_slice(&17_u16.to_be_bytes());
        delayed_jpeg[48] = 8;
        delayed_jpeg[49..51].copy_from_slice(&720_u16.to_be_bytes());
        delayed_jpeg[51..53].copy_from_slice(&1280_u16.to_be_bytes());
        let constrained = ExtractionLimits {
            max_image_probe_bytes: 32,
            ..limits()
        };
        let error = ImageMetadataExtractor
            .extract(
                &mut Cursor::new(&delayed_jpeg),
                request(ImageFormat::Jpeg, &delayed_jpeg),
                constrained,
                &NoCancellation,
            )
            .expect_err("metadata beyond the probe cap should fail");
        assert_eq!(error, ExtractionError::ImageMetadataLimitExceeded);

        let cancellation = AtomicCancellation::new();
        cancellation.cancel();
        let image = png(10, 10);
        let cancelled = ImageMetadataExtractor
            .extract(
                &mut Cursor::new(&image),
                request(ImageFormat::Png, &image),
                limits(),
                &cancellation,
            )
            .expect_err("cancelled image probe should stop");
        assert_eq!(cancelled, ExtractionError::Cancelled);
    }
}
