use std::fmt;
use std::io::{Read, Seek, SeekFrom};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use deskgraph_domain::ImageFormat;

mod image;
mod ocr;
mod ooxml;
mod pdf;
mod service;

pub use image::ImageMetadataExtractor;
pub use ocr::{
    ABSOLUTE_MAX_OCR_DIMENSION, ABSOLUTE_MAX_OCR_OBSERVATION_BYTES, ABSOLUTE_MAX_OCR_OBSERVATIONS,
    ABSOLUTE_MAX_OCR_OUTPUT_BYTES, ABSOLUTE_MAX_OCR_PIXELS, ABSOLUTE_MAX_OCR_SOURCE_BYTES,
    NativeOcrProvider, OcrBoundingBox, OcrCancellation, OcrControl, OcrObservation, OcrOutput,
    OcrProvider, OcrProviderLimits, OcrRequest, recognize_ocr_image_bytes,
};
pub use ooxml::OoxmlTextExtractor;
pub use pdf::PdfTextExtractor;
pub use service::{
    ExtractionServiceError, cancel_extraction_job_at, create_extraction_job_at,
    create_screenshot_ocr_job_at, extraction_job_at, extraction_stats_at,
    image_metadata_for_job_at, recent_extraction_jobs_at, resume_extraction_job_at,
    run_extraction_job_at,
};

pub const UNTRUSTED_TEXT: &str = "untrusted_extracted_text";
pub const ABSOLUTE_MAX_SOURCE_BYTES: u64 = 64 * 1024 * 1024;
pub const ABSOLUTE_MAX_OUTPUT_BYTES: u64 = 64 * 1024 * 1024;
pub const ABSOLUTE_MAX_CHUNKS: usize = 65_536;
pub const ABSOLUTE_MAX_CHUNK_BYTES: usize = 64 * 1024;
pub const ABSOLUTE_MAX_PROCESSING_TIME: Duration = Duration::from_secs(60);
pub const ABSOLUTE_MAX_DECOMPRESSED_BYTES: usize = 64 * 1024 * 1024;
pub const ABSOLUTE_MAX_PDF_PAGES: u32 = 4_096;
pub const ABSOLUTE_MAX_IMAGE_PROBE_BYTES: usize = 8 * 1024 * 1024;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MediaKind {
    PlainText,
    Markdown,
    SourceCode,
    Pdf,
    Docx,
    Pptx,
    Xlsx,
    Image(ImageFormat),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ExtractionLimits {
    pub max_source_bytes: u64,
    pub max_output_bytes: u64,
    pub max_chunks: usize,
    pub max_chunk_bytes: usize,
    pub chunk_overlap_bytes: usize,
    pub max_decompressed_bytes: usize,
    pub max_pdf_pages: u32,
    pub max_image_source_bytes: u64,
    pub max_image_probe_bytes: usize,
    pub max_image_dimension: u32,
    pub max_image_pixels: u64,
    pub max_processing_time: Duration,
}

impl Default for ExtractionLimits {
    fn default() -> Self {
        Self {
            max_source_bytes: 4 * 1024 * 1024,
            max_output_bytes: 8 * 1024 * 1024,
            max_chunks: 2_048,
            max_chunk_bytes: 4_096,
            chunk_overlap_bytes: 256,
            max_decompressed_bytes: 8 * 1024 * 1024,
            max_pdf_pages: 512,
            max_image_source_bytes: ABSOLUTE_MAX_SOURCE_BYTES,
            max_image_probe_bytes: 2 * 1024 * 1024,
            max_image_dimension: deskgraph_domain::MAX_IMAGE_DIMENSION_PIXELS,
            max_image_pixels: deskgraph_domain::MAX_IMAGE_TOTAL_PIXELS,
            max_processing_time: Duration::from_secs(5),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ExtractionRequest {
    pub media_kind: MediaKind,
    pub expected_source_bytes: u64,
    pub modified_unix_ns: Option<i64>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ChunkProvenance {
    ByteRange {
        start: u64,
        end: u64,
    },
    PdfPage {
        page_number: u32,
        fragment_index: u32,
    },
    DocxParagraph {
        paragraph_number: u32,
        fragment_index: u32,
    },
    PptxSlide {
        slide_number: u32,
        fragment_index: u32,
    },
    XlsxCell {
        sheet_number: u32,
        cell_reference: String,
        fragment_index: u32,
    },
    OcrObservation {
        observation_number: u32,
        fragment_index: u32,
        bounding_box: OcrBoundingBox,
        confidence_basis_points: Option<u16>,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExtractedChunk {
    pub ordinal: u32,
    pub text: String,
    pub provenance: ChunkProvenance,
    pub trust_class: &'static str,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ExtractedImageMetadata {
    pub format: ImageFormat,
    pub pixel_width: u32,
    pub pixel_height: u32,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExtractionOutput {
    pub provider_id: &'static str,
    pub provider_version: &'static str,
    pub media_kind: MediaKind,
    pub source_bytes: u64,
    pub output_bytes: u64,
    pub modified_unix_ns: Option<i64>,
    pub chunks: Vec<ExtractedChunk>,
    pub image_metadata: Option<ExtractedImageMetadata>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ExtractionError {
    UnsupportedMediaKind,
    InvalidLimits,
    SourceTooLarge,
    OutputTooLarge,
    SourceChanged,
    SourceSeekFailed,
    SourceReadFailed,
    InvalidUtf8,
    InvalidPdf,
    EncryptedPdfUnsupported,
    InvalidOoxmlArchive,
    UnsafeOoxmlArchive,
    EncryptedOoxmlUnsupported,
    UnsupportedOoxmlCompression,
    OoxmlEntryLimitExceeded,
    OoxmlCompressionRatioExceeded,
    MissingOoxmlPart,
    InvalidOoxmlXml,
    OoxmlStructureLimitExceeded,
    InvalidImage,
    ImageFormatMismatch,
    ImageMetadataLimitExceeded,
    ImageDimensionLimitExceeded,
    OcrProviderUnavailable,
    OcrLanguageUnavailable,
    OcrProviderFailed,
    OcrOutputInvalid,
    OcrObservationLimitExceeded,
    DecompressionLimitExceeded,
    PageLimitExceeded,
    ChunkLimitExceeded,
    Cancelled,
    TimeLimitExceeded,
}

impl ExtractionError {
    #[must_use]
    pub fn code(self) -> &'static str {
        match self {
            Self::UnsupportedMediaKind => "extraction_media_kind_unsupported",
            Self::InvalidLimits => "extraction_limits_invalid",
            Self::SourceTooLarge => "extraction_source_too_large",
            Self::OutputTooLarge => "extraction_output_too_large",
            Self::SourceChanged => "extraction_source_changed",
            Self::SourceSeekFailed => "extraction_source_seek_failed",
            Self::SourceReadFailed => "extraction_source_read_failed",
            Self::InvalidUtf8 => "extraction_invalid_utf8",
            Self::InvalidPdf => "extraction_pdf_invalid",
            Self::EncryptedPdfUnsupported => "extraction_pdf_encrypted_unsupported",
            Self::InvalidOoxmlArchive => "extraction_ooxml_archive_invalid",
            Self::UnsafeOoxmlArchive => "extraction_ooxml_archive_unsafe",
            Self::EncryptedOoxmlUnsupported => "extraction_ooxml_encrypted_unsupported",
            Self::UnsupportedOoxmlCompression => "extraction_ooxml_compression_unsupported",
            Self::OoxmlEntryLimitExceeded => "extraction_ooxml_entry_limit_exceeded",
            Self::OoxmlCompressionRatioExceeded => "extraction_ooxml_compression_ratio_exceeded",
            Self::MissingOoxmlPart => "extraction_ooxml_required_part_missing",
            Self::InvalidOoxmlXml => "extraction_ooxml_xml_invalid",
            Self::OoxmlStructureLimitExceeded => "extraction_ooxml_structure_limit_exceeded",
            Self::InvalidImage => "extraction_image_invalid",
            Self::ImageFormatMismatch => "extraction_image_format_mismatch",
            Self::ImageMetadataLimitExceeded => "extraction_image_metadata_limit_exceeded",
            Self::ImageDimensionLimitExceeded => "extraction_image_dimension_limit_exceeded",
            Self::OcrProviderUnavailable => "extraction_ocr_provider_unavailable",
            Self::OcrLanguageUnavailable => "extraction_ocr_language_unavailable",
            Self::OcrProviderFailed => "extraction_ocr_provider_failed",
            Self::OcrOutputInvalid => "extraction_ocr_output_invalid",
            Self::OcrObservationLimitExceeded => "extraction_ocr_observation_limit_exceeded",
            Self::DecompressionLimitExceeded => "extraction_decompression_limit_exceeded",
            Self::PageLimitExceeded => "extraction_page_limit_exceeded",
            Self::ChunkLimitExceeded => "extraction_chunk_limit_exceeded",
            Self::Cancelled => "extraction_cancelled",
            Self::TimeLimitExceeded => "extraction_time_limit_exceeded",
        }
    }
}

impl fmt::Display for ExtractionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.code())
    }
}

impl std::error::Error for ExtractionError {}

pub trait ControlledSource: Read + Seek {}

impl<T: Read + Seek> ControlledSource for T {}

pub trait CancellationSignal {
    fn is_cancelled(&self) -> bool;
}

#[derive(Debug, Default)]
pub struct NoCancellation;

impl CancellationSignal for NoCancellation {
    fn is_cancelled(&self) -> bool {
        false
    }
}

#[derive(Debug, Default)]
pub struct AtomicCancellation {
    cancelled: AtomicBool,
}

impl AtomicCancellation {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    pub fn cancel(&self) {
        self.cancelled.store(true, Ordering::Release);
    }
}

impl CancellationSignal for AtomicCancellation {
    fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::Acquire)
    }
}

pub trait ExtractorProvider {
    fn provider_id(&self) -> &'static str;
    fn provider_version(&self) -> &'static str;
    fn supports(&self, media_kind: MediaKind) -> bool;
    fn extract(
        &self,
        source: &mut dyn ControlledSource,
        request: ExtractionRequest,
        limits: ExtractionLimits,
        cancellation: &dyn CancellationSignal,
    ) -> Result<ExtractionOutput, ExtractionError>;
}

#[derive(Clone, Copy, Debug, Default)]
pub struct Utf8TextExtractor;

impl ExtractorProvider for Utf8TextExtractor {
    fn provider_id(&self) -> &'static str {
        "deskgraph.utf8-text"
    }

    fn provider_version(&self) -> &'static str {
        "1"
    }

    fn supports(&self, media_kind: MediaKind) -> bool {
        matches!(
            media_kind,
            MediaKind::PlainText | MediaKind::Markdown | MediaKind::SourceCode
        )
    }

    fn extract(
        &self,
        source: &mut dyn ControlledSource,
        request: ExtractionRequest,
        limits: ExtractionLimits,
        cancellation: &dyn CancellationSignal,
    ) -> Result<ExtractionOutput, ExtractionError> {
        validate_limits(limits)?;
        if !self.supports(request.media_kind) {
            return Err(ExtractionError::UnsupportedMediaKind);
        }
        if request.expected_source_bytes > limits.max_source_bytes {
            return Err(ExtractionError::SourceTooLarge);
        }

        let started = Instant::now();
        check_control(started, limits.max_processing_time, cancellation)?;
        let bytes = read_bounded_source(source, request, limits, started, cancellation)?;
        let source_bytes = request.expected_source_bytes;
        let content_offset = if bytes.starts_with(&[0xEF, 0xBB, 0xBF]) {
            3
        } else {
            0
        };
        let content_bytes = &bytes[content_offset..];
        let unique_output_bytes =
            u64::try_from(content_bytes.len()).map_err(|_| ExtractionError::OutputTooLarge)?;
        if unique_output_bytes > limits.max_output_bytes {
            return Err(ExtractionError::OutputTooLarge);
        }
        let content =
            std::str::from_utf8(content_bytes).map_err(|_| ExtractionError::InvalidUtf8)?;
        let (chunks, output_bytes) =
            chunk_utf8(content, content_offset, limits, started, cancellation)?;

        Ok(ExtractionOutput {
            provider_id: self.provider_id(),
            provider_version: self.provider_version(),
            media_kind: request.media_kind,
            source_bytes,
            output_bytes,
            modified_unix_ns: request.modified_unix_ns,
            chunks,
            image_metadata: None,
        })
    }
}

#[must_use]
pub fn media_kind_for_extension(extension: &str) -> Option<MediaKind> {
    let extension = extension.trim_start_matches('.').to_ascii_lowercase();
    match extension.as_str() {
        "txt" | "text" | "log" | "csv" | "tsv" => Some(MediaKind::PlainText),
        "md" | "markdown" | "mdown" | "mkd" | "mdx" => Some(MediaKind::Markdown),
        "rs" | "c" | "h" | "cc" | "cpp" | "cxx" | "hpp" | "cs" | "go" | "java" | "kt" | "kts"
        | "swift" | "js" | "jsx" | "mjs" | "cjs" | "ts" | "tsx" | "py" | "rb" | "php" | "sh"
        | "bash" | "zsh" | "fish" | "sql" | "html" | "htm" | "css" | "scss" | "sass" | "less"
        | "json" | "jsonl" | "toml" | "yaml" | "yml" | "xml" => Some(MediaKind::SourceCode),
        "pdf" => Some(MediaKind::Pdf),
        "docx" => Some(MediaKind::Docx),
        "pptx" => Some(MediaKind::Pptx),
        "xlsx" => Some(MediaKind::Xlsx),
        "png" => Some(MediaKind::Image(ImageFormat::Png)),
        "jpg" | "jpeg" => Some(MediaKind::Image(ImageFormat::Jpeg)),
        "gif" => Some(MediaKind::Image(ImageFormat::Gif)),
        "webp" => Some(MediaKind::Image(ImageFormat::Webp)),
        "bmp" => Some(MediaKind::Image(ImageFormat::Bmp)),
        "tif" | "tiff" => Some(MediaKind::Image(ImageFormat::Tiff)),
        _ => None,
    }
}

pub(crate) fn validate_limits(limits: ExtractionLimits) -> Result<(), ExtractionError> {
    if limits.max_source_bytes == 0
        || limits.max_source_bytes > ABSOLUTE_MAX_SOURCE_BYTES
        || limits.max_output_bytes == 0
        || limits.max_output_bytes > ABSOLUTE_MAX_OUTPUT_BYTES
        || limits.max_chunks == 0
        || limits.max_chunks > ABSOLUTE_MAX_CHUNKS
        || limits.max_chunk_bytes < 4
        || limits.max_chunk_bytes > ABSOLUTE_MAX_CHUNK_BYTES
        || limits.chunk_overlap_bytes >= limits.max_chunk_bytes
        || limits.max_decompressed_bytes == 0
        || limits.max_decompressed_bytes > ABSOLUTE_MAX_DECOMPRESSED_BYTES
        || limits.max_pdf_pages == 0
        || limits.max_pdf_pages > ABSOLUTE_MAX_PDF_PAGES
        || limits.max_image_source_bytes == 0
        || limits.max_image_source_bytes > ABSOLUTE_MAX_SOURCE_BYTES
        || limits.max_image_probe_bytes < 32
        || limits.max_image_probe_bytes > ABSOLUTE_MAX_IMAGE_PROBE_BYTES
        || u64::try_from(limits.max_image_probe_bytes)
            .map_or(true, |probe| probe > limits.max_image_source_bytes)
        || limits.max_image_dimension == 0
        || limits.max_image_dimension > deskgraph_domain::MAX_IMAGE_DIMENSION_PIXELS
        || limits.max_image_pixels == 0
        || limits.max_image_pixels > deskgraph_domain::MAX_IMAGE_TOTAL_PIXELS
        || limits.max_processing_time.is_zero()
        || limits.max_processing_time > ABSOLUTE_MAX_PROCESSING_TIME
    {
        return Err(ExtractionError::InvalidLimits);
    }
    Ok(())
}

pub(crate) fn check_control(
    started: Instant,
    max_processing_time: Duration,
    cancellation: &dyn CancellationSignal,
) -> Result<(), ExtractionError> {
    if cancellation.is_cancelled() {
        return Err(ExtractionError::Cancelled);
    }
    if started.elapsed() > max_processing_time {
        return Err(ExtractionError::TimeLimitExceeded);
    }
    Ok(())
}

pub(crate) fn read_bounded_source(
    source: &mut dyn ControlledSource,
    request: ExtractionRequest,
    limits: ExtractionLimits,
    started: Instant,
    cancellation: &dyn CancellationSignal,
) -> Result<Vec<u8>, ExtractionError> {
    source
        .seek(SeekFrom::Start(0))
        .map_err(|_| ExtractionError::SourceSeekFailed)?;
    let capacity = usize::try_from(request.expected_source_bytes)
        .map_err(|_| ExtractionError::SourceTooLarge)?;
    let mut bytes = Vec::with_capacity(capacity);
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        check_control(started, limits.max_processing_time, cancellation)?;
        let read = source
            .read(&mut buffer)
            .map_err(|_| ExtractionError::SourceReadFailed)?;
        if read == 0 {
            break;
        }
        let next_length = bytes
            .len()
            .checked_add(read)
            .ok_or(ExtractionError::SourceTooLarge)?;
        if u64::try_from(next_length).map_err(|_| ExtractionError::SourceTooLarge)?
            > limits.max_source_bytes
        {
            return Err(ExtractionError::SourceTooLarge);
        }
        bytes.extend_from_slice(&buffer[..read]);
    }
    check_control(started, limits.max_processing_time, cancellation)?;
    let source_bytes = u64::try_from(bytes.len()).map_err(|_| ExtractionError::SourceTooLarge)?;
    if source_bytes != request.expected_source_bytes {
        return Err(ExtractionError::SourceChanged);
    }
    Ok(bytes)
}

fn chunk_utf8(
    content: &str,
    content_offset: usize,
    limits: ExtractionLimits,
    started: Instant,
    cancellation: &dyn CancellationSignal,
) -> Result<(Vec<ExtractedChunk>, u64), ExtractionError> {
    let mut chunks = Vec::new();
    let mut output_bytes = 0_u64;
    let mut start = 0_usize;
    while start < content.len() {
        check_control(started, limits.max_processing_time, cancellation)?;
        if chunks.len() >= limits.max_chunks {
            return Err(ExtractionError::ChunkLimitExceeded);
        }
        let mut end = start
            .saturating_add(limits.max_chunk_bytes)
            .min(content.len());
        while end > start && !content.is_char_boundary(end) {
            end -= 1;
        }
        if end == start {
            return Err(ExtractionError::InvalidLimits);
        }
        let source_start = content_offset
            .checked_add(start)
            .ok_or(ExtractionError::OutputTooLarge)?;
        let source_end = content_offset
            .checked_add(end)
            .ok_or(ExtractionError::OutputTooLarge)?;
        let chunk_bytes =
            u64::try_from(end - start).map_err(|_| ExtractionError::OutputTooLarge)?;
        output_bytes = output_bytes
            .checked_add(chunk_bytes)
            .ok_or(ExtractionError::OutputTooLarge)?;
        if output_bytes > limits.max_output_bytes {
            return Err(ExtractionError::OutputTooLarge);
        }
        chunks.push(ExtractedChunk {
            ordinal: u32::try_from(chunks.len())
                .map_err(|_| ExtractionError::ChunkLimitExceeded)?,
            text: content[start..end].to_string(),
            provenance: ChunkProvenance::ByteRange {
                start: u64::try_from(source_start).map_err(|_| ExtractionError::OutputTooLarge)?,
                end: u64::try_from(source_end).map_err(|_| ExtractionError::OutputTooLarge)?,
            },
            trust_class: UNTRUSTED_TEXT,
        });
        if end == content.len() {
            break;
        }
        let mut next = end.saturating_sub(limits.chunk_overlap_bytes);
        while next > start && !content.is_char_boundary(next) {
            next -= 1;
        }
        start = if next > start { next } else { end };
    }
    Ok((chunks, output_bytes))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Cursor, Error, ErrorKind, Result as IoResult};
    use std::thread;

    fn request(media_kind: MediaKind, bytes: usize) -> ExtractionRequest {
        ExtractionRequest {
            media_kind,
            expected_source_bytes: bytes as u64,
            modified_unix_ns: Some(7),
        }
    }

    fn compact_limits() -> ExtractionLimits {
        ExtractionLimits {
            max_source_bytes: 1_024,
            max_output_bytes: 1_024,
            max_chunks: 32,
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
    fn extension_routing_is_explicit_and_case_insensitive() {
        assert_eq!(media_kind_for_extension(".TXT"), Some(MediaKind::PlainText));
        assert_eq!(media_kind_for_extension("md"), Some(MediaKind::Markdown));
        assert_eq!(media_kind_for_extension("Rs"), Some(MediaKind::SourceCode));
        assert_eq!(media_kind_for_extension("pdf"), Some(MediaKind::Pdf));
        assert_eq!(
            media_kind_for_extension("PNG"),
            Some(MediaKind::Image(ImageFormat::Png))
        );
        assert_eq!(
            media_kind_for_extension("jpeg"),
            Some(MediaKind::Image(ImageFormat::Jpeg))
        );
        assert_eq!(media_kind_for_extension("exe"), None);
    }

    #[test]
    fn caller_limits_cannot_disable_absolute_resource_caps() {
        let mut source = Cursor::new(b"small");
        let mut limits = compact_limits();
        limits.max_source_bytes = ABSOLUTE_MAX_SOURCE_BYTES + 1;

        let error = Utf8TextExtractor
            .extract(
                &mut source,
                request(MediaKind::PlainText, 5),
                limits,
                &NoCancellation,
            )
            .expect_err("unbounded caller policy must fail");

        assert_eq!(error, ExtractionError::InvalidLimits);
    }

    #[test]
    fn extracts_mixed_traditional_chinese_and_english_with_exact_offsets() {
        let text = "DeskGraph 連接本機檔案，local-first context。";
        let mut source = Cursor::new(text.as_bytes());
        let output = Utf8TextExtractor
            .extract(
                &mut source,
                request(MediaKind::PlainText, text.len()),
                compact_limits(),
                &NoCancellation,
            )
            .expect("mixed text should extract");

        assert!(output.chunks.len() > 1);
        assert_eq!(output.provider_id, "deskgraph.utf8-text");
        assert_eq!(output.modified_unix_ns, Some(7));
        for chunk in &output.chunks {
            let ChunkProvenance::ByteRange { start, end } = chunk.provenance else {
                panic!("text chunks must use byte provenance");
            };
            let start = start as usize;
            let end = end as usize;
            assert_eq!(chunk.text.as_bytes(), &text.as_bytes()[start..end]);
            assert_eq!(chunk.trust_class, UNTRUSTED_TEXT);
        }
    }

    #[test]
    fn utf8_bom_is_removed_but_source_offsets_remain_true() {
        let bytes = b"\xEF\xBB\xBFhello";
        let mut source = Cursor::new(bytes);
        let output = Utf8TextExtractor
            .extract(
                &mut source,
                request(MediaKind::Markdown, bytes.len()),
                compact_limits(),
                &NoCancellation,
            )
            .expect("BOM text should extract");

        assert_eq!(output.output_bytes, 5);
        assert_eq!(output.chunks[0].text, "hello");
        assert_eq!(
            output.chunks[0].provenance,
            ChunkProvenance::ByteRange { start: 3, end: 8 }
        );
    }

    #[test]
    fn chunk_overlap_is_preserved_and_counted_in_bounded_output() {
        let bytes = b"0123456789abcdef";
        let mut source = Cursor::new(bytes);
        let mut limits = compact_limits();
        limits.max_chunk_bytes = 8;
        limits.chunk_overlap_bytes = 2;
        let output = Utf8TextExtractor
            .extract(
                &mut source,
                request(MediaKind::PlainText, bytes.len()),
                limits,
                &NoCancellation,
            )
            .expect("overlapping chunks should extract");

        assert_eq!(
            output
                .chunks
                .iter()
                .map(|chunk| match chunk.provenance {
                    ChunkProvenance::ByteRange { start, .. } => start,
                    ChunkProvenance::PdfPage { .. }
                    | ChunkProvenance::DocxParagraph { .. }
                    | ChunkProvenance::PptxSlide { .. }
                    | ChunkProvenance::XlsxCell { .. }
                    | ChunkProvenance::OcrObservation { .. } => {
                        panic!("expected byte provenance")
                    }
                })
                .collect::<Vec<_>>(),
            vec![0, 6, 12]
        );
        assert_eq!(output.output_bytes, 20);
    }

    #[test]
    fn declared_oversized_source_is_rejected_before_reading() {
        let mut source = Cursor::new(b"small");
        let error = Utf8TextExtractor
            .extract(
                &mut source,
                request(MediaKind::PlainText, 2_048),
                compact_limits(),
                &NoCancellation,
            )
            .expect_err("oversized source must fail");

        assert_eq!(error, ExtractionError::SourceTooLarge);
    }

    #[test]
    fn invalid_utf8_is_a_per_file_error() {
        let bytes = [0x66, 0x80, 0x6f];
        let mut source = Cursor::new(bytes);
        let error = Utf8TextExtractor
            .extract(
                &mut source,
                request(MediaKind::PlainText, bytes.len()),
                compact_limits(),
                &NoCancellation,
            )
            .expect_err("invalid UTF-8 must fail");

        assert_eq!(error, ExtractionError::InvalidUtf8);
        assert_eq!(error.code(), "extraction_invalid_utf8");
    }

    #[test]
    fn cancellation_is_checked_before_source_access() {
        let cancellation = AtomicCancellation::new();
        cancellation.cancel();
        let mut source = Cursor::new(b"cancelled");
        let error = Utf8TextExtractor
            .extract(
                &mut source,
                request(MediaKind::SourceCode, 9),
                compact_limits(),
                &cancellation,
            )
            .expect_err("cancelled extraction must fail");

        assert_eq!(error, ExtractionError::Cancelled);
    }

    struct SlowSource(Cursor<Vec<u8>>);

    impl Read for SlowSource {
        fn read(&mut self, buffer: &mut [u8]) -> IoResult<usize> {
            thread::sleep(Duration::from_millis(5));
            self.0.read(buffer)
        }
    }

    impl Seek for SlowSource {
        fn seek(&mut self, position: SeekFrom) -> IoResult<u64> {
            self.0.seek(position)
        }
    }

    #[test]
    fn active_processing_time_is_bounded() {
        let mut source = SlowSource(Cursor::new(b"too slow".to_vec()));
        let mut limits = compact_limits();
        limits.max_processing_time = Duration::from_millis(1);
        let error = Utf8TextExtractor
            .extract(
                &mut source,
                request(MediaKind::PlainText, 8),
                limits,
                &NoCancellation,
            )
            .expect_err("slow source must time out");

        assert_eq!(error, ExtractionError::TimeLimitExceeded);
    }

    #[test]
    fn metadata_change_is_detected_before_publishing_output() {
        let mut source = Cursor::new(b"changed");
        let error = Utf8TextExtractor
            .extract(
                &mut source,
                request(MediaKind::PlainText, 6),
                compact_limits(),
                &NoCancellation,
            )
            .expect_err("changed size must fail");

        assert_eq!(error, ExtractionError::SourceChanged);
    }

    #[test]
    fn chunk_limit_returns_no_partial_output() {
        let bytes = b"0123456789abcdefghijklmnopqrstuvwxyz";
        let mut source = Cursor::new(bytes);
        let mut limits = compact_limits();
        limits.max_chunks = 1;
        limits.max_chunk_bytes = 8;
        limits.chunk_overlap_bytes = 2;
        let error = Utf8TextExtractor
            .extract(
                &mut source,
                request(MediaKind::PlainText, bytes.len()),
                limits,
                &NoCancellation,
            )
            .expect_err("chunk limit must fail atomically");

        assert_eq!(error, ExtractionError::ChunkLimitExceeded);
    }

    struct FailingSource;

    impl Read for FailingSource {
        fn read(&mut self, _buffer: &mut [u8]) -> IoResult<usize> {
            Err(Error::new(ErrorKind::InvalidData, "fixture failure"))
        }
    }

    impl Seek for FailingSource {
        fn seek(&mut self, _position: SeekFrom) -> IoResult<u64> {
            Ok(0)
        }
    }

    #[test]
    fn source_errors_are_reduced_to_fixed_codes() {
        let mut source = FailingSource;
        let error = Utf8TextExtractor
            .extract(
                &mut source,
                request(MediaKind::PlainText, 1),
                compact_limits(),
                &NoCancellation,
            )
            .expect_err("source error must not panic");

        assert_eq!(error, ExtractionError::SourceReadFailed);
        assert_eq!(error.to_string(), "extraction_source_read_failed");
    }
}
