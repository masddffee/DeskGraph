use std::fmt;
use std::fs::{self, File, Metadata};
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use std::thread::{self, JoinHandle};
use std::time::Duration;
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use deskgraph_database::{
    ContentChunkProvenanceWrite, ContentChunkWrite, DatabaseError, ExtractableFile,
    ImageMetadataWrite, ManifestDatabase, ScopeExclusionMatcher, ScopeRevisionBinding,
};
use deskgraph_domain::{
    ExtractionJobProgress, ExtractionOperation, ExtractionStats, ExtractionStatus, ImageMetadata,
};
use deskgraph_identity::{
    IdentityNodeKind, comparison_key, fallback_identity, has_hidden_or_system_attribute,
    is_symlink_or_reparse_point, path_from_raw, platform_identity_for_open_file,
};

use crate::ocr::{ABSOLUTE_MAX_OCR_SOURCE_BYTES, build_ocr_extraction_output};
use crate::{
    CancellationSignal, ChunkProvenance, ExtractionError, ExtractionLimits, ExtractionRequest,
    ExtractorProvider, ImageMetadataExtractor, MediaKind, NativeOcrProvider, NoCancellation,
    OcrCancellation, OcrControl, OcrProvider, OcrRequest, OoxmlTextExtractor, PdfTextExtractor,
    Utf8TextExtractor, media_kind_for_extension, recognize_ocr_image_bytes,
};

// The provider's absolute processing cap is 60 seconds. Keep enough lease headroom for
// post-read identity validation and one atomic SQLite publish without permitting stale runners.
const RUNNER_LEASE_MS: i64 = 120_000;
const ROUTER_PROVIDER_ID: &str = "deskgraph.extractor-router";
const ROUTER_PROVIDER_VERSION: &str = "1";

#[derive(Debug)]
pub enum ExtractionServiceError {
    Database(DatabaseError),
    ScopePathDecodeFailed,
    ScopeCanonicalizationFailed,
    ScopeChanged,
    SourcePathDecodeFailed,
    SourceMetadataUnavailable,
    SourceExcluded,
    SourceCanonicalizationFailed,
    SourceScopeEscape,
    SourceNotFile,
    SourceOpenFailed,
    SourceMetadataChanged,
    SourceIdentityChanged,
    UnsupportedMediaKind,
    OcrCapacityBusy,
    OcrCancellationMonitorFailed,
    InvalidSystemTime,
    ScopePolicyChanged,
    Extraction(ExtractionError),
}

impl ExtractionServiceError {
    #[must_use]
    pub fn code(&self) -> &'static str {
        match self {
            Self::Database(error) => error.code(),
            Self::ScopePathDecodeFailed => "extraction_scope_path_decode_failed",
            Self::ScopeCanonicalizationFailed => "extraction_scope_canonicalization_failed",
            Self::ScopeChanged => "extraction_scope_changed",
            Self::SourcePathDecodeFailed => "extraction_source_path_decode_failed",
            Self::SourceMetadataUnavailable => "extraction_source_metadata_unavailable",
            Self::SourceExcluded => "extraction_source_excluded",
            Self::SourceCanonicalizationFailed => "extraction_source_canonicalization_failed",
            Self::SourceScopeEscape => "extraction_scope_escape_denied",
            Self::SourceNotFile => "extraction_source_not_file",
            Self::SourceOpenFailed => "extraction_source_open_failed",
            Self::SourceMetadataChanged => "extraction_source_metadata_changed",
            Self::SourceIdentityChanged => "extraction_source_identity_changed",
            Self::UnsupportedMediaKind => "extraction_media_kind_unsupported",
            Self::OcrCapacityBusy => "extraction_ocr_capacity_busy",
            Self::OcrCancellationMonitorFailed => "extraction_ocr_cancel_monitor_failed",
            Self::InvalidSystemTime => "system_time_invalid",
            Self::ScopePolicyChanged => "scope_policy_changed",
            Self::Extraction(error) => error.code(),
        }
    }

    fn invalidates_prior_content(&self) -> bool {
        matches!(
            self,
            Self::ScopeChanged
                | Self::SourceMetadataUnavailable
                | Self::SourceExcluded
                | Self::SourceCanonicalizationFailed
                | Self::SourceScopeEscape
                | Self::SourceNotFile
                | Self::SourceOpenFailed
                | Self::SourceMetadataChanged
                | Self::SourceIdentityChanged
                | Self::Extraction(ExtractionError::SourceChanged)
        )
    }
}

impl fmt::Display for ExtractionServiceError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.code())
    }
}

impl std::error::Error for ExtractionServiceError {}

impl From<DatabaseError> for ExtractionServiceError {
    fn from(error: DatabaseError) -> Self {
        Self::Database(error)
    }
}

impl From<ExtractionError> for ExtractionServiceError {
    fn from(error: ExtractionError) -> Self {
        Self::Extraction(error)
    }
}

pub fn create_extraction_job_at(
    database_path: &Path,
    scope_id: i64,
    node_id: i64,
) -> Result<ExtractionJobProgress, ExtractionServiceError> {
    let mut database = ManifestDatabase::open(database_path)?;
    let (binding, matcher) = bind_current_extraction_policy(&database, scope_id)?;
    let source = database.extractable_file(scope_id, node_id)?;
    ensure_extraction_source_not_excluded(&matcher, &source)?;
    let job = database
        .create_extraction_job_with_policy(binding, node_id)
        .map_err(ExtractionServiceError::from)?;
    assert_extraction_policy_current(&database, binding)?;
    Ok(job)
}

pub fn create_screenshot_ocr_job_at(
    database_path: &Path,
    scope_id: i64,
    node_id: i64,
) -> Result<ExtractionJobProgress, ExtractionServiceError> {
    let mut database = ManifestDatabase::open(database_path)?;
    let read_fence = database.acquire_scope_filesystem_read_fence(scope_id)?;
    let (binding, matcher) = bind_current_extraction_policy(&database, scope_id)?;
    if read_fence.binding().revision != binding.revision {
        return Err(ExtractionServiceError::ScopePolicyChanged);
    }
    let source = database.extractable_file(scope_id, node_id)?;
    ensure_extraction_source_not_excluded(&matcher, &source)?;
    let (mut file, media_kind) = validate_source(&database, &source)?;
    if !matches!(
        media_kind,
        MediaKind::Image(deskgraph_domain::ImageFormat::Png)
            | MediaKind::Image(deskgraph_domain::ImageFormat::Jpeg)
    ) {
        return Err(ExtractionServiceError::UnsupportedMediaKind);
    }
    let limits = ExtractionLimits::default();
    let image_metadata = ImageMetadataExtractor
        .extract(
            &mut file,
            ExtractionRequest {
                media_kind,
                expected_source_bytes: source.size_bytes,
                modified_unix_ns: source.modified_unix_ns,
            },
            limits,
            &NoCancellation,
        )?
        .image_metadata
        .ok_or(ExtractionError::OcrOutputInvalid)?;
    let MediaKind::Image(format) = media_kind else {
        return Err(ExtractionServiceError::UnsupportedMediaKind);
    };
    crate::ocr::validate_ocr_request(
        OcrRequest {
            format,
            expected_source_bytes: source.size_bytes,
            modified_unix_ns: source.modified_unix_ns,
            pixel_width: image_metadata.pixel_width,
            pixel_height: image_metadata.pixel_height,
        },
        limits,
    )?;
    validate_open_file(&file, &source)?;
    let job = database
        .low_level_insert_screenshot_ocr_job_with_policy_after_core_validation(binding, &source)
        .map_err(ExtractionServiceError::from)?;
    assert_extraction_policy_current(&database, binding)?;
    Ok(job)
}

pub fn extraction_job_at(
    database_path: &Path,
    job_id: i64,
) -> Result<ExtractionJobProgress, ExtractionServiceError> {
    ManifestDatabase::open(database_path)?
        .extraction_job(job_id)
        .map_err(Into::into)
}

pub fn recent_extraction_jobs_at(
    database_path: &Path,
) -> Result<Vec<ExtractionJobProgress>, ExtractionServiceError> {
    ManifestDatabase::open(database_path)?
        .recent_extraction_jobs()
        .map_err(Into::into)
}

pub fn extraction_stats_at(
    database_path: &Path,
) -> Result<ExtractionStats, ExtractionServiceError> {
    ManifestDatabase::open(database_path)?
        .extraction_stats()
        .map_err(Into::into)
}

pub fn image_metadata_for_job_at(
    database_path: &Path,
    job_id: i64,
) -> Result<ImageMetadata, ExtractionServiceError> {
    ManifestDatabase::open(database_path)?
        .image_metadata_for_job(job_id)
        .map_err(Into::into)
}

pub fn cancel_extraction_job_at(
    database_path: &Path,
    job_id: i64,
) -> Result<ExtractionJobProgress, ExtractionServiceError> {
    let mut database = ManifestDatabase::open(database_path)?;
    database
        .request_extraction_cancel(job_id)
        .map_err(Into::into)
}

pub fn resume_extraction_job_at(
    database_path: &Path,
    job_id: i64,
) -> Result<ExtractionJobProgress, ExtractionServiceError> {
    let mut database = ManifestDatabase::open(database_path)?;
    let progress = database.extraction_job(job_id)?;
    if progress.status != ExtractionStatus::Interrupted {
        return Err(DatabaseError::InvalidExtractionJobState.into());
    }
    let source = database.extractable_file_for_job(job_id)?;
    let read_fence = database.acquire_scope_filesystem_read_fence(source.scope_id)?;
    let (_, matcher) = bind_current_extraction_policy(&database, source.scope_id)?;
    if read_fence.binding().revision != matcher.revision {
        return Err(ExtractionServiceError::ScopePolicyChanged);
    }
    ensure_extraction_source_not_excluded(&matcher, &source)?;
    validate_source(&database, &source)?;
    database.resume_extraction_job(job_id).map_err(Into::into)
}

pub fn run_extraction_job_at(
    database_path: &Path,
    job_id: i64,
    limits: ExtractionLimits,
) -> Result<ExtractionJobProgress, ExtractionServiceError> {
    run_extraction_job_with_ocr_provider_at(database_path, job_id, limits, &NativeOcrProvider)
}

fn run_extraction_job_with_ocr_provider_at(
    database_path: &Path,
    job_id: i64,
    limits: ExtractionLimits,
    ocr_provider: &dyn OcrProvider,
) -> Result<ExtractionJobProgress, ExtractionServiceError> {
    let mut database = ManifestDatabase::open(database_path)?;
    let current = database.extraction_job(job_id)?;
    if current.is_terminal() || current.status == ExtractionStatus::Interrupted {
        return Ok(current);
    }
    let read_fence = database.acquire_scope_filesystem_read_fence(current.scope_id)?;
    let (policy_binding, matcher) = bind_current_extraction_policy(&database, current.scope_id)?;
    if read_fence.binding().revision != policy_binding.revision {
        return Err(ExtractionServiceError::ScopePolicyChanged);
    }
    let source = database.extractable_file_for_job(job_id)?;
    ensure_extraction_source_not_excluded(&matcher, &source)?;
    let runner_token = runner_token()?;
    database.claim_extraction_job(job_id, &runner_token, RUNNER_LEASE_MS)?;
    assert_extraction_policy_current(&database, policy_binding)?;
    let started = Instant::now();
    let attempt = extract_claimed_job(
        &database,
        database_path,
        job_id,
        current.operation,
        limits,
        ocr_provider,
    );
    let provider_id = attempt.provider_id;
    let provider_version = attempt.provider_version;
    let result = attempt.result;
    drop(read_fence);
    let elapsed_ms = u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX);
    match result {
        Ok(output) => {
            if database.extraction_cancel_requested(job_id)? {
                return database
                    .cancel_extraction_job_from_runner(
                        job_id,
                        &runner_token,
                        provider_id,
                        provider_version,
                        elapsed_ms,
                    )
                    .map_err(Into::into);
            }
            let chunks = output
                .chunks
                .into_iter()
                .map(|chunk| ContentChunkWrite {
                    ordinal: chunk.ordinal,
                    text: chunk.text,
                    provenance: match chunk.provenance {
                        ChunkProvenance::ByteRange { start, end } => {
                            ContentChunkProvenanceWrite::ByteRange { start, end }
                        }
                        ChunkProvenance::PdfPage {
                            page_number,
                            fragment_index,
                        } => ContentChunkProvenanceWrite::PdfPage {
                            page_number,
                            fragment_index,
                        },
                        ChunkProvenance::DocxParagraph {
                            paragraph_number,
                            fragment_index,
                        } => ContentChunkProvenanceWrite::DocxParagraph {
                            paragraph_number,
                            fragment_index,
                        },
                        ChunkProvenance::PptxSlide {
                            slide_number,
                            fragment_index,
                        } => ContentChunkProvenanceWrite::PptxSlide {
                            slide_number,
                            fragment_index,
                        },
                        ChunkProvenance::XlsxCell {
                            sheet_number,
                            cell_reference,
                            fragment_index,
                        } => ContentChunkProvenanceWrite::XlsxCell {
                            sheet_number,
                            cell_reference,
                            fragment_index,
                        },
                        ChunkProvenance::OcrObservation {
                            observation_number,
                            fragment_index,
                            bounding_box,
                            confidence_basis_points,
                        } => ContentChunkProvenanceWrite::OcrObservation {
                            observation_number,
                            fragment_index,
                            bbox_x_ppm: bounding_box.x_ppm,
                            bbox_y_ppm: bounding_box.y_ppm,
                            bbox_width_ppm: bounding_box.width_ppm,
                            bbox_height_ppm: bounding_box.height_ppm,
                            confidence_basis_points,
                        },
                    },
                    trust_class: chunk.trust_class,
                })
                .collect::<Vec<_>>();
            let image_metadata = output.image_metadata.map(|metadata| ImageMetadataWrite {
                format: metadata.format,
                pixel_width: metadata.pixel_width,
                pixel_height: metadata.pixel_height,
            });
            if let Err(error) = assert_extraction_policy_current(&database, policy_binding) {
                return fail_extraction_for_policy_change(
                    &mut database,
                    job_id,
                    &runner_token,
                    provider_id,
                    provider_version,
                    elapsed_ms,
                    error,
                );
            }
            match database.complete_extraction_job_with_image_metadata(
                job_id,
                &runner_token,
                output.provider_id,
                output.provider_version,
                output.source_bytes,
                output.modified_unix_ns,
                output.output_bytes,
                elapsed_ms,
                &chunks,
                image_metadata.as_ref(),
            ) {
                Ok(progress) => Ok(progress),
                Err(error) => {
                    if database
                        .extraction_cancel_requested(job_id)
                        .unwrap_or(false)
                    {
                        return database
                            .cancel_extraction_job_from_runner(
                                job_id,
                                &runner_token,
                                provider_id,
                                provider_version,
                                elapsed_ms,
                            )
                            .map_err(Into::into);
                    }
                    database
                        .fail_extraction_job(
                            job_id,
                            &runner_token,
                            provider_id,
                            provider_version,
                            error.code(),
                            elapsed_ms,
                        )
                        .map_err(Into::into)
                }
            }
        }
        Err(ExtractionServiceError::Extraction(ExtractionError::OcrCapacityBusy)) => {
            let progress =
                database.requeue_extraction_job_after_capacity_refusal(job_id, &runner_token)?;
            if progress.status == ExtractionStatus::Cancelled {
                Ok(progress)
            } else {
                Err(ExtractionServiceError::OcrCapacityBusy)
            }
        }
        Err(error) => {
            if matches!(
                error,
                ExtractionServiceError::Extraction(ExtractionError::Cancelled)
            ) || database
                .extraction_cancel_requested(job_id)
                .unwrap_or(false)
            {
                return database
                    .cancel_extraction_job_from_runner(
                        job_id,
                        &runner_token,
                        provider_id,
                        provider_version,
                        elapsed_ms,
                    )
                    .map_err(Into::into);
            }
            if error.invalidates_prior_content() {
                database.invalidate_content_for_node(current.scope_id, current.node_id)?;
            }
            database
                .fail_extraction_job(
                    job_id,
                    &runner_token,
                    provider_id,
                    provider_version,
                    error.code(),
                    elapsed_ms,
                )
                .map_err(Into::into)
        }
    }
}

fn bind_current_extraction_policy(
    database: &ManifestDatabase,
    scope_id: i64,
) -> Result<(ScopeRevisionBinding, ScopeExclusionMatcher), ExtractionServiceError> {
    let binding = database.bind_core_scope_policy_revision(scope_id)?;
    let matcher = database.scope_exclusion_matcher(scope_id)?;
    if matcher.revision != binding.revision
        || !database.is_core_scope_policy_binding_current(binding)?
    {
        return Err(ExtractionServiceError::ScopePolicyChanged);
    }
    Ok((binding, matcher))
}

fn assert_extraction_policy_current(
    database: &ManifestDatabase,
    binding: ScopeRevisionBinding,
) -> Result<(), ExtractionServiceError> {
    if database.is_core_scope_policy_binding_current(binding)? {
        Ok(())
    } else {
        Err(ExtractionServiceError::ScopePolicyChanged)
    }
}

fn ensure_extraction_source_not_excluded(
    matcher: &ScopeExclusionMatcher,
    source: &ExtractableFile,
) -> Result<(), ExtractionServiceError> {
    if matcher.is_excluded_path_key(&source.path_key)
        || matcher.is_excluded_identity(&source.identity_kind, &source.identity_key)
    {
        Err(ExtractionServiceError::ScopePolicyChanged)
    } else {
        Ok(())
    }
}

fn fail_extraction_for_policy_change(
    database: &mut ManifestDatabase,
    job_id: i64,
    runner_token: &str,
    provider_id: &str,
    provider_version: &str,
    elapsed_ms: u64,
    error: ExtractionServiceError,
) -> Result<ExtractionJobProgress, ExtractionServiceError> {
    let _ = database.fail_extraction_job(
        job_id,
        runner_token,
        provider_id,
        provider_version,
        error.code(),
        elapsed_ms,
    );
    Err(error)
}

fn extract_claimed_job(
    database: &ManifestDatabase,
    database_path: &Path,
    job_id: i64,
    operation: ExtractionOperation,
    limits: ExtractionLimits,
    ocr_provider: &dyn OcrProvider,
) -> ExtractionAttempt {
    let source = match database.extractable_file_for_job(job_id) {
        Ok(source) => source,
        Err(error) => return ExtractionAttempt::router_error(error.into()),
    };
    let (mut file, media_kind) = match validate_source(database, &source) {
        Ok(validated) => validated,
        Err(error) => return ExtractionAttempt::router_error(error),
    };
    if operation == ExtractionOperation::ScreenshotOcr {
        return extract_claimed_ocr_job(
            database_path,
            job_id,
            &source,
            &mut file,
            media_kind,
            limits,
            ocr_provider,
        );
    }
    let text_provider = Utf8TextExtractor;
    let pdf_provider = PdfTextExtractor;
    let ooxml_provider = OoxmlTextExtractor;
    let image_provider = ImageMetadataExtractor;
    let provider: &dyn ExtractorProvider = match media_kind {
        MediaKind::PlainText | MediaKind::Markdown | MediaKind::SourceCode => &text_provider,
        MediaKind::Pdf => &pdf_provider,
        MediaKind::Docx | MediaKind::Pptx | MediaKind::Xlsx => &ooxml_provider,
        MediaKind::Image(_) => &image_provider,
    };
    let provider_id = provider.provider_id();
    let provider_version = provider.provider_version();
    let cancellation = match DatabaseCancellation::open(database_path, job_id) {
        Ok(cancellation) => cancellation,
        Err(error) => {
            return ExtractionAttempt {
                provider_id,
                provider_version,
                result: Err(error),
            };
        }
    };
    let result = provider
        .extract(
            &mut file,
            ExtractionRequest {
                media_kind,
                expected_source_bytes: source.size_bytes,
                modified_unix_ns: source.modified_unix_ns,
            },
            limits,
            &cancellation,
        )
        .map_err(ExtractionServiceError::from)
        .and_then(|output| {
            validate_open_file(&file, &source)?;
            Ok(output)
        });
    ExtractionAttempt {
        provider_id,
        provider_version,
        result,
    }
}

fn extract_claimed_ocr_job(
    database_path: &Path,
    job_id: i64,
    source: &ExtractableFile,
    file: &mut File,
    media_kind: MediaKind,
    limits: ExtractionLimits,
    provider: &dyn OcrProvider,
) -> ExtractionAttempt {
    let provider_id = provider.provider_id();
    let provider_version = provider.provider_version();
    let MediaKind::Image(format) = media_kind else {
        return ExtractionAttempt {
            provider_id,
            provider_version,
            result: Err(ExtractionServiceError::UnsupportedMediaKind),
        };
    };
    let control = OcrControl::for_job(limits.max_processing_time, database_path, job_id);
    let mut monitor = match DurableOcrCancellationMonitor::start(
        database_path.to_path_buf(),
        job_id,
        control.cancellation(),
    ) {
        Ok(monitor) => monitor,
        Err(error) => {
            return ExtractionAttempt {
                provider_id,
                provider_version,
                result: Err(error),
            };
        }
    };
    let result: Result<crate::ExtractionOutput, ExtractionServiceError> = (|| {
        let metadata_output = ImageMetadataExtractor.extract(
            file,
            ExtractionRequest {
                media_kind,
                expected_source_bytes: source.size_bytes,
                modified_unix_ns: source.modified_unix_ns,
            },
            limits,
            &control.cancellation(),
        )?;
        let metadata = metadata_output
            .image_metadata
            .ok_or(ExtractionError::OcrOutputInvalid)?;
        let request = OcrRequest {
            format,
            expected_source_bytes: source.size_bytes,
            modified_unix_ns: source.modified_unix_ns,
            pixel_width: metadata.pixel_width,
            pixel_height: metadata.pixel_height,
        };
        let encoded_image = read_ocr_source(file, request, limits, &control)?;
        let validated_source_bytes = encoded_image.clone();
        let output = recognize_ocr_image_bytes(provider, encoded_image, request, limits, &control)?;
        let output = build_ocr_extraction_output(provider, request, limits, &control, output)?;
        validate_source_bytes_unchanged(file, &validated_source_bytes, &control)?;
        validate_open_file(file, source)?;
        Ok(output)
    })();
    monitor.stop();
    let result = if monitor.failed() {
        Err(ExtractionServiceError::OcrCancellationMonitorFailed)
    } else {
        result
    };
    ExtractionAttempt {
        provider_id,
        provider_version,
        result,
    }
}

fn read_ocr_source(
    source: &mut File,
    request: OcrRequest,
    limits: ExtractionLimits,
    control: &OcrControl,
) -> Result<Vec<u8>, ExtractionError> {
    let max_source_bytes = limits
        .max_image_source_bytes
        .min(ABSOLUTE_MAX_OCR_SOURCE_BYTES);
    if request.expected_source_bytes == 0 || request.expected_source_bytes > max_source_bytes {
        return Err(ExtractionError::SourceTooLarge);
    }
    source
        .seek(SeekFrom::Start(0))
        .map_err(|_| ExtractionError::SourceSeekFailed)?;
    let capacity = usize::try_from(request.expected_source_bytes)
        .map_err(|_| ExtractionError::SourceTooLarge)?;
    let mut bytes = Vec::with_capacity(capacity);
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        control.check()?;
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
            > max_source_bytes
        {
            return Err(ExtractionError::SourceTooLarge);
        }
        bytes.extend_from_slice(&buffer[..read]);
    }
    control.check()?;
    if u64::try_from(bytes.len()).ok() != Some(request.expected_source_bytes) {
        return Err(ExtractionError::SourceChanged);
    }
    Ok(bytes)
}

fn validate_source_bytes_unchanged(
    source: &mut File,
    expected: &[u8],
    control: &OcrControl,
) -> Result<(), ExtractionError> {
    source
        .seek(SeekFrom::Start(0))
        .map_err(|_| ExtractionError::SourceSeekFailed)?;
    let mut offset = 0_usize;
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        control.check()?;
        let read = source
            .read(&mut buffer)
            .map_err(|_| ExtractionError::SourceReadFailed)?;
        if read == 0 {
            break;
        }
        let end = offset
            .checked_add(read)
            .ok_or(ExtractionError::SourceChanged)?;
        if expected.get(offset..end) != Some(&buffer[..read]) {
            return Err(ExtractionError::SourceChanged);
        }
        offset = end;
    }
    control.check()?;
    if offset != expected.len() {
        return Err(ExtractionError::SourceChanged);
    }
    Ok(())
}

struct DurableOcrCancellationMonitor {
    stop_requested: Arc<AtomicBool>,
    failed: Arc<AtomicBool>,
    cancellation: OcrCancellation,
    handle: Option<JoinHandle<()>>,
}

impl DurableOcrCancellationMonitor {
    fn start(
        database_path: PathBuf,
        job_id: i64,
        cancellation: OcrCancellation,
    ) -> Result<Self, ExtractionServiceError> {
        let stop_requested = Arc::new(AtomicBool::new(false));
        let failed = Arc::new(AtomicBool::new(false));
        let thread_stop = Arc::clone(&stop_requested);
        let thread_failed = Arc::clone(&failed);
        let thread_cancellation = cancellation.clone();
        let handle = thread::Builder::new()
            .name("deskgraph-ocr-cancel".to_string())
            .spawn(move || {
                let database = match ManifestDatabase::open(&database_path) {
                    Ok(database) => database,
                    Err(_) => {
                        thread_failed.store(true, Ordering::Release);
                        thread_cancellation.cancel();
                        return;
                    }
                };
                while !thread_stop.load(Ordering::Acquire) {
                    match database.extraction_cancel_requested(job_id) {
                        Ok(true) => {
                            thread_cancellation.cancel();
                            return;
                        }
                        Ok(false) => {}
                        Err(_) => {
                            thread_failed.store(true, Ordering::Release);
                            thread_cancellation.cancel();
                            return;
                        }
                    }
                    thread::park_timeout(Duration::from_millis(25));
                }
            })
            .map_err(|_| ExtractionServiceError::OcrCancellationMonitorFailed)?;
        Ok(Self {
            stop_requested,
            failed,
            cancellation,
            handle: Some(handle),
        })
    }

    fn stop(&mut self) {
        self.stop_requested.store(true, Ordering::Release);
        if let Some(handle) = self.handle.take() {
            handle.thread().unpark();
            if handle.join().is_err() {
                self.failed.store(true, Ordering::Release);
                self.cancellation.cancel();
            }
        }
    }

    fn failed(&self) -> bool {
        self.failed.load(Ordering::Acquire)
    }
}

impl Drop for DurableOcrCancellationMonitor {
    fn drop(&mut self) {
        self.stop();
    }
}

struct ExtractionAttempt {
    provider_id: &'static str,
    provider_version: &'static str,
    result: Result<crate::ExtractionOutput, ExtractionServiceError>,
}

impl ExtractionAttempt {
    fn router_error(error: ExtractionServiceError) -> Self {
        Self {
            provider_id: ROUTER_PROVIDER_ID,
            provider_version: ROUTER_PROVIDER_VERSION,
            result: Err(error),
        }
    }
}

fn validate_source(
    database: &ManifestDatabase,
    source: &ExtractableFile,
) -> Result<(File, MediaKind), ExtractionServiceError> {
    let scope = database.scope_record(source.scope_id)?;
    let stored_root = path_from_raw(&scope.path_raw)
        .map_err(|_| ExtractionServiceError::ScopePathDecodeFailed)?;
    let canonical_root = fs::canonicalize(stored_root)
        .map_err(|_| ExtractionServiceError::ScopeCanonicalizationFailed)?;
    if comparison_key(&canonical_root) != scope.path_key {
        return Err(ExtractionServiceError::ScopeChanged);
    }
    let stored_path = path_from_raw(&source.path_raw)
        .map_err(|_| ExtractionServiceError::SourcePathDecodeFailed)?;
    if comparison_key(&stored_path) != source.path_key {
        return Err(ExtractionServiceError::SourceMetadataChanged);
    }
    let link_metadata = fs::symlink_metadata(&stored_path)
        .map_err(|_| ExtractionServiceError::SourceMetadataUnavailable)?;
    if is_symlink_or_reparse_point(&link_metadata) || has_hidden_or_system_attribute(&link_metadata)
    {
        return Err(ExtractionServiceError::SourceExcluded);
    }
    if !link_metadata.is_file() {
        return Err(ExtractionServiceError::SourceNotFile);
    }
    let canonical_source = fs::canonicalize(&stored_path)
        .map_err(|_| ExtractionServiceError::SourceCanonicalizationFailed)?;
    if !canonical_source.starts_with(&canonical_root) {
        return Err(ExtractionServiceError::SourceScopeEscape);
    }
    if comparison_key(&canonical_source) != source.path_key {
        return Err(ExtractionServiceError::SourceMetadataChanged);
    }
    let extension = canonical_source
        .extension()
        .and_then(|value| value.to_str())
        .ok_or(ExtractionServiceError::UnsupportedMediaKind)?;
    let media_kind =
        media_kind_for_extension(extension).ok_or(ExtractionServiceError::UnsupportedMediaKind)?;
    let file =
        File::open(&canonical_source).map_err(|_| ExtractionServiceError::SourceOpenFailed)?;
    validate_open_file(&file, source)?;
    Ok((file, media_kind))
}

fn validate_open_file(file: &File, source: &ExtractableFile) -> Result<(), ExtractionServiceError> {
    let metadata = file
        .metadata()
        .map_err(|_| ExtractionServiceError::SourceMetadataUnavailable)?;
    if !metadata.is_file()
        || metadata.len() != source.size_bytes
        || modified_unix_ns(&metadata) != source.modified_unix_ns
    {
        return Err(ExtractionServiceError::SourceMetadataChanged);
    }
    let path = path_from_raw(&source.path_raw)
        .map_err(|_| ExtractionServiceError::SourcePathDecodeFailed)?;
    let identity = platform_identity_for_open_file(file, &path, &metadata, IdentityNodeKind::File)
        .unwrap_or_else(|_| fallback_identity(&source.path_key, IdentityNodeKind::File));
    if identity.kind != source.identity_kind || identity.key != source.identity_key {
        return Err(ExtractionServiceError::SourceIdentityChanged);
    }
    Ok(())
}

fn modified_unix_ns(metadata: &Metadata) -> Option<i64> {
    let duration = metadata.modified().ok()?.duration_since(UNIX_EPOCH).ok()?;
    i64::try_from(duration.as_nanos()).ok()
}

fn runner_token() -> Result<String, ExtractionServiceError> {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| ExtractionServiceError::InvalidSystemTime)?
        .as_nanos();
    Ok(format!("{}:{nanos}", std::process::id()))
}

struct DatabaseCancellation {
    database: ManifestDatabase,
    job_id: i64,
}

impl DatabaseCancellation {
    fn open(database_path: &Path, job_id: i64) -> Result<Self, ExtractionServiceError> {
        Ok(Self {
            database: ManifestDatabase::open(database_path)?,
            job_id,
        })
    }
}

impl CancellationSignal for DatabaseCancellation {
    fn is_cancelled(&self) -> bool {
        self.database
            .extraction_cancel_requested(self.job_id)
            .unwrap_or(true)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use deskgraph_database::{
        LexicalCandidateSource, LexicalSearchFilters, LexicalSearchSource, ScopeExclusionWrite,
    };
    use deskgraph_domain::{ExtractionOperation, ExtractionStatus};
    use deskgraph_scanner::{
        ScopeExclusionSelection, authorize_scope_with_access_grant, comparison_key,
        prepare_scope_exclusion_batch, scan_scope,
    };
    use lopdf::content::{Content, Operation};
    use lopdf::{Document, Object, Stream, dictionary};
    use std::io::{Cursor, Write};
    use std::path::PathBuf;
    use zip::write::SimpleFileOptions;
    use zip::{CompressionMethod, ZipWriter};

    struct Fixture {
        _directory: tempfile::TempDir,
        database_path: PathBuf,
        scope_id: i64,
        node_id: i64,
        file_path: PathBuf,
    }

    #[derive(Clone, Copy, Debug)]
    struct MixedLanguageOcrProvider;

    impl OcrProvider for MixedLanguageOcrProvider {
        fn provider_id(&self) -> &'static str {
            "deskgraph.fake-ocr"
        }

        fn provider_version(&self) -> &'static str {
            "1"
        }

        fn recognize(
            &self,
            encoded_image: Vec<u8>,
            request: OcrRequest,
            _limits: crate::OcrProviderLimits,
            control: &OcrControl,
        ) -> Result<crate::OcrOutput, ExtractionError> {
            control.check()?;
            assert_eq!(encoded_image.len() as u64, request.expected_source_bytes);
            assert_eq!((request.pixel_width, request.pixel_height), (640, 480));
            Ok(crate::OcrOutput {
                observations: vec![
                    crate::OcrObservation {
                        text: "DeskGraph OCR".to_string(),
                        bounding_box: crate::OcrBoundingBox {
                            x_ppm: 50_000,
                            y_ppm: 200_000,
                            width_ppm: 400_000,
                            height_ppm: 150_000,
                        },
                        confidence_basis_points: Some(10_000),
                    },
                    crate::OcrObservation {
                        text: "桌面圖譜 安全整理".to_string(),
                        bounding_box: crate::OcrBoundingBox {
                            x_ppm: 50_000,
                            y_ppm: 400_000,
                            width_ppm: 500_000,
                            height_ppm: 150_000,
                        },
                        confidence_basis_points: Some(8_500),
                    },
                ],
            })
        }
    }

    #[derive(Clone, Copy, Debug)]
    struct NoConfidenceOcrProvider;

    impl OcrProvider for NoConfidenceOcrProvider {
        fn provider_id(&self) -> &'static str {
            "deskgraph.no-confidence-fake-ocr"
        }

        fn provider_version(&self) -> &'static str {
            "1"
        }

        fn recognize(
            &self,
            _encoded_image: Vec<u8>,
            _request: OcrRequest,
            _limits: crate::OcrProviderLimits,
            control: &OcrControl,
        ) -> Result<crate::OcrOutput, ExtractionError> {
            control.check()?;
            Ok(crate::OcrOutput {
                observations: vec![crate::OcrObservation {
                    text: "Windows OCR 無分數".to_string(),
                    bounding_box: crate::OcrBoundingBox {
                        x_ppm: 100_000,
                        y_ppm: 200_000,
                        width_ppm: 500_000,
                        height_ppm: 100_000,
                    },
                    confidence_basis_points: None,
                }],
            })
        }
    }

    #[derive(Clone, Debug)]
    struct BlockingOcrProvider {
        started: Arc<AtomicBool>,
    }

    impl OcrProvider for BlockingOcrProvider {
        fn provider_id(&self) -> &'static str {
            "deskgraph.blocking-fake-ocr"
        }

        fn provider_version(&self) -> &'static str {
            "1"
        }

        fn recognize(
            &self,
            _encoded_image: Vec<u8>,
            _request: OcrRequest,
            _limits: crate::OcrProviderLimits,
            control: &OcrControl,
        ) -> Result<crate::OcrOutput, ExtractionError> {
            self.started.store(true, Ordering::Release);
            loop {
                control.check()?;
                thread::sleep(Duration::from_millis(5));
            }
        }
    }

    #[derive(Clone, Debug)]
    struct FailureAfterCancelOcrProvider {
        started: Arc<AtomicBool>,
        release_failure: Arc<AtomicBool>,
    }

    impl OcrProvider for FailureAfterCancelOcrProvider {
        fn provider_id(&self) -> &'static str {
            "deskgraph.cancel-race-fake-ocr"
        }

        fn provider_version(&self) -> &'static str {
            "1"
        }

        fn recognize(
            &self,
            _encoded_image: Vec<u8>,
            _request: OcrRequest,
            _limits: crate::OcrProviderLimits,
            _control: &OcrControl,
        ) -> Result<crate::OcrOutput, ExtractionError> {
            self.started.store(true, Ordering::Release);
            while !self.release_failure.load(Ordering::Acquire) {
                thread::sleep(Duration::from_millis(2));
            }
            Err(ExtractionError::OcrProviderFailed)
        }
    }

    #[derive(Clone, Copy, Debug)]
    struct CapacityBusyOcrProvider;

    impl OcrProvider for CapacityBusyOcrProvider {
        fn provider_id(&self) -> &'static str {
            "deskgraph.capacity-busy-fake-ocr"
        }

        fn provider_version(&self) -> &'static str {
            "1"
        }

        fn recognize(
            &self,
            _encoded_image: Vec<u8>,
            _request: OcrRequest,
            _limits: crate::OcrProviderLimits,
            _control: &OcrControl,
        ) -> Result<crate::OcrOutput, ExtractionError> {
            Err(ExtractionError::OcrCapacityBusy)
        }
    }

    #[derive(Clone, Copy, Debug)]
    struct NoTextOcrProvider;

    impl OcrProvider for NoTextOcrProvider {
        fn provider_id(&self) -> &'static str {
            "deskgraph.no-text-fake-ocr"
        }

        fn provider_version(&self) -> &'static str {
            "1"
        }

        fn recognize(
            &self,
            _encoded_image: Vec<u8>,
            _request: OcrRequest,
            _limits: crate::OcrProviderLimits,
            control: &OcrControl,
        ) -> Result<crate::OcrOutput, ExtractionError> {
            control.check()?;
            Ok(crate::OcrOutput {
                observations: Vec::new(),
            })
        }
    }

    #[derive(Clone, Debug)]
    struct MutatingOcrProvider {
        file_path: PathBuf,
        original_modified: SystemTime,
    }

    impl OcrProvider for MutatingOcrProvider {
        fn provider_id(&self) -> &'static str {
            "deskgraph.mutating-fake-ocr"
        }

        fn provider_version(&self) -> &'static str {
            "1"
        }

        fn recognize(
            &self,
            encoded_image: Vec<u8>,
            _request: OcrRequest,
            _limits: crate::OcrProviderLimits,
            control: &OcrControl,
        ) -> Result<crate::OcrOutput, ExtractionError> {
            control.check()?;
            let mut changed = encoded_image;
            let last = changed
                .last_mut()
                .expect("test image should contain at least one byte");
            *last ^= 1;
            fs::write(&self.file_path, changed).expect("test provider should mutate fixture");
            File::open(&self.file_path)
                .and_then(|file| {
                    file.set_times(fs::FileTimes::new().set_modified(self.original_modified))
                })
                .expect("test provider should restore source metadata");
            Ok(crate::OcrOutput {
                observations: vec![crate::OcrObservation {
                    text: "stale OCR must not publish".to_string(),
                    bounding_box: crate::OcrBoundingBox {
                        x_ppm: 0,
                        y_ppm: 0,
                        width_ppm: 100_000,
                        height_ppm: 100_000,
                    },
                    confidence_basis_points: Some(9_000),
                }],
            })
        }
    }

    fn fixture(file_name: &str, contents: &[u8]) -> Fixture {
        let directory = tempfile::tempdir().expect("fixture root should exist");
        let scope_path = directory.path().join("authorized");
        let database_path = directory.path().join("app-data/manifest.sqlite3");
        fs::create_dir(&scope_path).expect("scope should create");
        let file_path = scope_path.join(file_name);
        fs::write(&file_path, contents).expect("fixture should write");
        let mut database = ManifestDatabase::open(&database_path).expect("database should open");
        let scope = authorize_scope_with_access_grant(
            &mut database,
            &scope_path,
            std::env::consts::OS,
            b"test-access-grant",
        )
        .expect("scope should authorize with an active test grant");
        scan_scope(&mut database, scope.id).expect("scope should scan");
        let canonical_file = fs::canonicalize(&file_path).expect("file should canonicalize");
        let node_id = database
            .node_id_for_path_key(scope.id, &comparison_key(&canonical_file))
            .expect("node query should pass")
            .expect("file node should exist");
        drop(database);
        Fixture {
            _directory: directory,
            database_path,
            scope_id: scope.id,
            node_id,
            file_path,
        }
    }

    fn pdf_bytes(text: &str) -> Vec<u8> {
        let mut document = Document::with_version("1.7");
        let pages_id = document.new_object_id();
        let font_id = document.add_object(dictionary! {
            "Type" => "Font",
            "Subtype" => "Type1",
            "BaseFont" => "Courier",
        });
        let resources_id = document.add_object(dictionary! {
            "Font" => dictionary! { "F1" => font_id },
        });
        let content = Content {
            operations: vec![
                Operation::new("BT", vec![]),
                Operation::new("Tf", vec!["F1".into(), 12.into()]),
                Operation::new("Tj", vec![Object::string_literal(text)]),
                Operation::new("ET", vec![]),
            ],
        };
        let content_id = document.add_object(Stream::new(
            dictionary! {},
            content.encode().expect("PDF content should encode"),
        ));
        let page_id = document.add_object(dictionary! {
            "Type" => "Page",
            "Parent" => pages_id,
            "Resources" => resources_id,
            "Contents" => content_id,
            "MediaBox" => vec![0.into(), 0.into(), 612.into(), 792.into()],
        });
        document.objects.insert(
            pages_id,
            Object::Dictionary(dictionary! {
                "Type" => "Pages",
                "Kids" => vec![Object::Reference(page_id)],
                "Count" => 1,
            }),
        );
        let catalog_id = document.add_object(dictionary! {
            "Type" => "Catalog",
            "Pages" => pages_id,
        });
        document.trailer.set("Root", catalog_id);
        let mut bytes = Vec::new();
        document
            .save_to(&mut bytes)
            .expect("PDF fixture should save");
        bytes
    }

    fn ooxml_bytes(parts: &[(&str, &[u8])]) -> Vec<u8> {
        let cursor = Cursor::new(Vec::new());
        let mut writer = ZipWriter::new(cursor);
        let options = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);
        for (name, contents) in parts {
            writer
                .start_file(*name, options)
                .expect("OOXML fixture part should start");
            writer
                .write_all(contents)
                .expect("OOXML fixture part should write");
        }
        writer
            .finish()
            .expect("OOXML fixture should finish")
            .into_inner()
    }

    fn png_bytes(width: u32, height: u32) -> Vec<u8> {
        let mut bytes = vec![0_u8; 32];
        bytes[..8].copy_from_slice(b"\x89PNG\r\n\x1a\n");
        bytes[8..12].copy_from_slice(&13_u32.to_be_bytes());
        bytes[12..16].copy_from_slice(b"IHDR");
        bytes[16..20].copy_from_slice(&width.to_be_bytes());
        bytes[20..24].copy_from_slice(&height.to_be_bytes());
        bytes
    }

    fn jpeg_bytes(width: u16, height: u16) -> Vec<u8> {
        let mut bytes = vec![0_u8; 16];
        bytes[..2].copy_from_slice(b"\xff\xd8");
        bytes[2..4].copy_from_slice(b"\xff\xc0");
        bytes[4..6].copy_from_slice(&17_u16.to_be_bytes());
        bytes[6] = 8;
        bytes[7..9].copy_from_slice(&height.to_be_bytes());
        bytes[9..11].copy_from_slice(&width.to_be_bytes());
        bytes
    }

    fn gif_bytes(width: u16, height: u16) -> Vec<u8> {
        let mut bytes = vec![0_u8; 12];
        bytes[..6].copy_from_slice(b"GIF89a");
        bytes[6..8].copy_from_slice(&width.to_le_bytes());
        bytes[8..10].copy_from_slice(&height.to_le_bytes());
        bytes
    }

    #[test]
    fn markdown_file_runs_from_manifest_identity_to_atomic_chunks() {
        let fixture = fixture("notes.md", "# DeskGraph\n本機 context\n".as_bytes());
        let job =
            create_extraction_job_at(&fixture.database_path, fixture.scope_id, fixture.node_id)
                .expect("job should create");

        let completed = run_extraction_job_at(
            &fixture.database_path,
            job.job_id,
            ExtractionLimits::default(),
        )
        .expect("job should run");

        assert_eq!(completed.status, ExtractionStatus::Completed);
        assert_eq!(
            completed.provider_id.as_deref(),
            Some("deskgraph.utf8-text")
        );
        assert!(completed.chunk_count > 0);
        let stats = extraction_stats_at(&fixture.database_path).expect("stats should load");
        assert_eq!(stats.extracted_file_count, 1);
        assert_eq!(stats.active_chunk_count, completed.chunk_count);
    }

    #[cfg(unix)]
    #[test]
    fn durable_exclusion_identity_blocks_surviving_hardlink_rescan_extraction_and_search() {
        let directory = tempfile::tempdir().expect("fixture root should exist");
        let scope_path = directory.path().join("authorized");
        let private_path = scope_path.join("private/secret.txt");
        let public_hardlink = scope_path.join("public-hardlink.txt");
        let database_path = directory.path().join("app-data/manifest.sqlite3");
        fs::create_dir(&scope_path).expect("scope should create");
        fs::write(&public_hardlink, b"durableidentitysecret must disappear")
            .expect("initial public source should create");

        let mut database = ManifestDatabase::open(&database_path).expect("database should open");
        let scope = authorize_scope_with_access_grant(
            &mut database,
            &scope_path,
            std::env::consts::OS,
            b"test-access-grant",
        )
        .expect("scope should authorize");
        scan_scope(&mut database, scope.id).expect("initial scan should publish the public source");
        let public_key = comparison_key(
            &fs::canonicalize(&public_hardlink).expect("public hardlink should canonicalize"),
        );
        let node_id = database
            .node_id_for_path_key(scope.id, &public_key)
            .expect("public lookup should pass")
            .expect("public hardlink should initially publish");
        drop(database);

        let job = create_extraction_job_at(&database_path, scope.id, node_id)
            .expect("initial extraction should queue");
        run_extraction_job_at(&database_path, job.job_id, ExtractionLimits::default())
            .expect("initial extraction should complete");
        let mut database = ManifestDatabase::open(&database_path).expect("database should reopen");
        assert_eq!(
            database
                .lexical_search_candidates(
                    "\"durableidentitysecret\"",
                    LexicalSearchFilters {
                        scope_id: Some(scope.id),
                        source: LexicalSearchSource::ExtractedText,
                        extension: None,
                        modified_since_unix_ns: None,
                        modified_before_unix_ns: None,
                    },
                    10,
                )
                .expect("initial search should run")
                .len(),
            1
        );

        fs::create_dir_all(private_path.parent().expect("private parent"))
            .expect("private directory should create after the manifest snapshot");
        fs::hard_link(&public_hardlink, &private_path)
            .expect("unscanned private hardlink should create");

        let prepared = prepare_scope_exclusion_batch(
            &database,
            scope.id,
            &[ScopeExclusionSelection {
                requested_path: &private_path,
            }],
        )
        .expect("private alias should prepare");
        let exclusion = prepared.exclusions.first().expect("one exclusion");
        let write = ScopeExclusionWrite {
            kind: exclusion.kind,
            path_raw: &exclusion.path_raw,
            path_key: &exclusion.path_key,
            display_path: &exclusion.display_path,
            identity_kind: &exclusion.identity_kind,
            identity_key: &exclusion.identity_key,
        };
        let binding = database
            .bind_scope_policy_revision(scope.id)
            .expect("policy should bind");
        database
            .apply_scope_exclusion_batch(binding, &[write], 1)
            .expect("exclusion and purge should commit");
        assert!(
            public_hardlink.exists(),
            "privacy purge must not mutate source files"
        );

        scan_scope(&mut database, scope.id).expect("rescan should safely withhold both aliases");
        assert_eq!(
            database
                .node_id_for_path_key(scope.id, &public_key)
                .expect("post-rescan lookup should pass"),
            None,
            "surviving hardlink identity must not republish a manifest location"
        );
        assert!(
            database
                .lexical_search_candidates(
                    "\"durableidentitysecret\"",
                    LexicalSearchFilters {
                        scope_id: Some(scope.id),
                        source: LexicalSearchSource::All,
                        extension: None,
                        modified_since_unix_ns: None,
                        modified_before_unix_ns: None,
                    },
                    10,
                )
                .expect("post-exclusion search should run")
                .is_empty(),
            "excluded inode content must not become searchable again"
        );
        drop(database);
        assert!(
            create_extraction_job_at(&database_path, scope.id, node_id).is_err(),
            "excluded inode must not regain an extraction entry point"
        );
    }

    #[test]
    fn pdf_file_routes_from_manifest_identity_to_bounded_provider() {
        let fixture = fixture("reference.pdf", &pdf_bytes("DeskGraph PDF context"));
        let job =
            create_extraction_job_at(&fixture.database_path, fixture.scope_id, fixture.node_id)
                .expect("job should create");

        let completed = run_extraction_job_at(
            &fixture.database_path,
            job.job_id,
            ExtractionLimits::default(),
        )
        .expect("PDF job should run");

        assert_eq!(completed.status, ExtractionStatus::Completed);
        assert_eq!(completed.provider_id.as_deref(), Some("deskgraph.pdf-text"));
        assert!(completed.chunk_count > 0);
        let stats = extraction_stats_at(&fixture.database_path).expect("stats should load");
        assert_eq!(stats.extracted_file_count, 1);
        assert_eq!(stats.active_chunk_count, completed.chunk_count);
    }

    #[test]
    fn office_files_route_from_manifest_to_atomic_fts_content() {
        let docx = ooxml_bytes(&[(
            "word/document.xml",
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:t>DeskGraph document context</w:t></w:p></w:body></w:document>"#.as_bytes(),
        )]);
        let pptx = ooxml_bytes(&[(
            "ppt/slides/slide1.xml",
            r#"<p:sld xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main" xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main"><a:p><a:t>DeskGraph slide context</a:t></a:p></p:sld>"#.as_bytes(),
        )]);
        let xlsx = ooxml_bytes(&[(
            "xl/worksheets/sheet1.xml",
            r#"<worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main"><sheetData><row><c r="A1" t="inlineStr"><is><t>DeskGraph sheet context</t></is></c></row></sheetData></worksheet>"#.as_bytes(),
        )]);

        for (file_name, contents) in [
            ("context.docx", docx),
            ("context.pptx", pptx),
            ("context.xlsx", xlsx),
        ] {
            let fixture = fixture(file_name, &contents);
            let job =
                create_extraction_job_at(&fixture.database_path, fixture.scope_id, fixture.node_id)
                    .expect("Office job should create");
            let completed = run_extraction_job_at(
                &fixture.database_path,
                job.job_id,
                ExtractionLimits::default(),
            )
            .expect("Office job should run");

            assert_eq!(completed.status, ExtractionStatus::Completed);
            assert_eq!(
                completed.provider_id.as_deref(),
                Some("deskgraph.ooxml-text")
            );
            assert!(completed.chunk_count > 0);
            let database =
                ManifestDatabase::open(&fixture.database_path).expect("database should reopen");
            let candidates = database
                .lexical_search_candidates(
                    "\"DeskGraph\"",
                    LexicalSearchFilters {
                        scope_id: Some(fixture.scope_id),
                        source: LexicalSearchSource::ExtractedText,
                        extension: None,
                        modified_since_unix_ns: None,
                        modified_before_unix_ns: None,
                    },
                    10,
                )
                .expect("Office text should be searchable");
            assert_eq!(candidates.len(), 1);
            assert_eq!(candidates[0].source, LexicalCandidateSource::ExtractedText);
            assert_eq!(candidates[0].node_id, fixture.node_id);
        }
    }

    #[test]
    fn image_routes_from_manifest_to_structured_atomic_metadata() {
        let contents = png_bytes(1920, 1080);
        let image_fixture = fixture("Screenshot.png", &contents);
        let job = create_extraction_job_at(
            &image_fixture.database_path,
            image_fixture.scope_id,
            image_fixture.node_id,
        )
        .expect("image job should create");
        let completed = run_extraction_job_at(
            &image_fixture.database_path,
            job.job_id,
            ExtractionLimits::default(),
        )
        .expect("image job should run");

        assert_eq!(completed.status, ExtractionStatus::Completed);
        assert_eq!(
            completed.provider_id.as_deref(),
            Some("deskgraph.image-metadata")
        );
        assert_eq!(completed.chunk_count, 0);
        assert_eq!(completed.output_bytes, 0);
        let metadata = image_metadata_for_job_at(&image_fixture.database_path, job.job_id)
            .expect("structured metadata should load");
        assert_eq!(metadata.format, deskgraph_domain::ImageFormat::Png);
        assert_eq!((metadata.pixel_width, metadata.pixel_height), (1920, 1080));
        assert_eq!(metadata.node_id, image_fixture.node_id);
        let stats = extraction_stats_at(&image_fixture.database_path).expect("stats should load");
        assert_eq!(stats.extracted_file_count, 1);
        assert_eq!(stats.active_chunk_count, 0);

        let mut changed_image = png_bytes(800, 600);
        changed_image.push(0);
        fs::write(&image_fixture.file_path, changed_image).expect("image fixture should change");
        let stale_job = create_extraction_job_at(
            &image_fixture.database_path,
            image_fixture.scope_id,
            image_fixture.node_id,
        )
        .expect("stale image job should create from manifest");
        let stale = run_extraction_job_at(
            &image_fixture.database_path,
            stale_job.job_id,
            ExtractionLimits::default(),
        )
        .expect("stale image should fail safely");
        assert_eq!(stale.status, ExtractionStatus::Failed);
        assert_eq!(
            stale.error_code.as_deref(),
            Some("extraction_source_metadata_changed")
        );
        assert_eq!(
            extraction_stats_at(&image_fixture.database_path)
                .expect("stale metadata should invalidate")
                .extracted_file_count,
            0
        );

        let mismatched = fixture("renamed.jpg", &contents);
        let mismatched_job = create_extraction_job_at(
            &mismatched.database_path,
            mismatched.scope_id,
            mismatched.node_id,
        )
        .expect("mismatched job should create");
        let failed = run_extraction_job_at(
            &mismatched.database_path,
            mismatched_job.job_id,
            ExtractionLimits::default(),
        )
        .expect("signature mismatch should be isolated");
        assert_eq!(failed.status, ExtractionStatus::Failed);
        assert_eq!(
            failed.error_code.as_deref(),
            Some("extraction_image_format_mismatch")
        );
        assert_eq!(
            image_metadata_for_job_at(&mismatched.database_path, mismatched_job.job_id)
                .expect_err("failed job must not publish metadata")
                .code(),
            "image_metadata_not_found"
        );
    }

    #[test]
    fn screenshot_ocr_routes_bounded_bytes_to_atomic_searchable_chunks() {
        let contents = png_bytes(640, 480);
        let fixture = fixture("Screenshot.png", &contents);
        let job =
            create_screenshot_ocr_job_at(&fixture.database_path, fixture.scope_id, fixture.node_id)
                .expect("OCR job should create");
        let completed = run_extraction_job_with_ocr_provider_at(
            &fixture.database_path,
            job.job_id,
            ExtractionLimits::default(),
            &MixedLanguageOcrProvider,
        )
        .expect("fake OCR job should run");

        assert_eq!(completed.operation, ExtractionOperation::ScreenshotOcr);
        assert_eq!(completed.status, ExtractionStatus::Completed);
        assert_eq!(completed.provider_id.as_deref(), Some("deskgraph.fake-ocr"));
        assert_eq!(completed.chunk_count, 2);
        let database =
            ManifestDatabase::open(&fixture.database_path).expect("OCR database should reopen");
        let candidates = database
            .lexical_search_candidates(
                "\"桌面圖譜\"",
                LexicalSearchFilters {
                    scope_id: Some(fixture.scope_id),
                    source: LexicalSearchSource::ExtractedText,
                    extension: None,
                    modified_since_unix_ns: None,
                    modified_before_unix_ns: None,
                },
                10,
            )
            .expect("OCR text should be searchable");
        assert_eq!(candidates.len(), 1);
        assert_eq!(candidates[0].node_id, fixture.node_id);
    }

    #[test]
    fn screenshot_ocr_without_confidence_atomically_replaces_prior_output() {
        let fixture = fixture("Screenshot.png", &png_bytes(640, 480));
        let initial_job =
            create_screenshot_ocr_job_at(&fixture.database_path, fixture.scope_id, fixture.node_id)
                .expect("initial OCR job should create");
        let initial = run_extraction_job_with_ocr_provider_at(
            &fixture.database_path,
            initial_job.job_id,
            ExtractionLimits::default(),
            &MixedLanguageOcrProvider,
        )
        .expect("initial OCR should complete");
        assert_eq!(initial.status, ExtractionStatus::Completed);
        assert_eq!(initial.chunk_count, 2);

        let replacement_job =
            create_screenshot_ocr_job_at(&fixture.database_path, fixture.scope_id, fixture.node_id)
                .expect("replacement OCR job should create");
        let replacement = run_extraction_job_with_ocr_provider_at(
            &fixture.database_path,
            replacement_job.job_id,
            ExtractionLimits::default(),
            &NoConfidenceOcrProvider,
        )
        .expect("provider without confidence should replace prior OCR");

        assert_eq!(replacement.status, ExtractionStatus::Completed);
        assert_eq!(
            replacement.provider_id.as_deref(),
            Some("deskgraph.no-confidence-fake-ocr")
        );
        assert_eq!(replacement.chunk_count, 1);

        let database =
            ManifestDatabase::open(&fixture.database_path).expect("OCR database should reopen");
        let filters = LexicalSearchFilters {
            scope_id: Some(fixture.scope_id),
            source: LexicalSearchSource::ExtractedText,
            extension: None,
            modified_since_unix_ns: None,
            modified_before_unix_ns: None,
        };
        assert!(
            database
                .lexical_search_candidates("\"桌面圖譜\"", filters, 10)
                .expect("prior OCR query should run")
                .is_empty(),
            "the prior complete OCR result must be inactive after replacement"
        );
        let replacement_candidates = database
            .lexical_search_candidates("\"無分數\"", filters, 10)
            .expect("replacement OCR should be searchable");
        assert_eq!(replacement_candidates.len(), 1);
        assert_eq!(replacement_candidates[0].node_id, fixture.node_id);
        assert_eq!(
            extraction_stats_at(&fixture.database_path)
                .expect("replacement stats should load")
                .active_chunk_count,
            1
        );
    }

    #[test]
    fn screenshot_ocr_no_text_is_a_complete_empty_result() {
        let fixture = fixture("blank.png", &png_bytes(640, 480));
        let job =
            create_screenshot_ocr_job_at(&fixture.database_path, fixture.scope_id, fixture.node_id)
                .expect("OCR job should create");
        let completed = run_extraction_job_with_ocr_provider_at(
            &fixture.database_path,
            job.job_id,
            ExtractionLimits::default(),
            &NoTextOcrProvider,
        )
        .expect("no-text OCR should complete");

        assert_eq!(completed.status, ExtractionStatus::Completed);
        assert_eq!(completed.output_bytes, 0);
        assert_eq!(completed.chunk_count, 0);
        assert_eq!(
            extraction_stats_at(&fixture.database_path)
                .expect("no-text stats should load")
                .active_chunk_count,
            0
        );
    }

    #[test]
    fn screenshot_ocr_creation_accepts_only_valid_png_and_jpeg() {
        for (file_name, contents) in [
            ("Screenshot.png", png_bytes(640, 480)),
            ("Screenshot.jpeg", jpeg_bytes(640, 480)),
        ] {
            let fixture = fixture(file_name, &contents);
            let job = create_screenshot_ocr_job_at(
                &fixture.database_path,
                fixture.scope_id,
                fixture.node_id,
            )
            .expect("valid PNG/JPEG should create a durable OCR job");
            assert_eq!(job.status, ExtractionStatus::Queued);
            assert_eq!(job.operation, ExtractionOperation::ScreenshotOcr);
        }
    }

    #[test]
    fn screenshot_ocr_rejects_invalid_sources_without_leaving_a_job() {
        let mut oversized_png = png_bytes(640, 480);
        oversized_png.resize(ABSOLUTE_MAX_OCR_SOURCE_BYTES as usize + 1, 0);
        for (file_name, contents, expected_error) in [
            (
                "notes.md",
                b"# not an image".to_vec(),
                "extraction_media_kind_unsupported",
            ),
            (
                "animated.gif",
                gif_bytes(640, 480),
                "extraction_media_kind_unsupported",
            ),
            (
                "corrupt.png",
                b"not a png".to_vec(),
                "extraction_image_invalid",
            ),
            (
                "mismatched.png",
                gif_bytes(640, 480),
                "extraction_image_format_mismatch",
            ),
            (
                "oversized.png",
                oversized_png,
                "extraction_source_too_large",
            ),
            (
                "too-wide.png",
                png_bytes(crate::ABSOLUTE_MAX_OCR_DIMENSION + 1, 1),
                "extraction_image_dimension_limit_exceeded",
            ),
            (
                "too-many-pixels.png",
                png_bytes(
                    crate::ABSOLUTE_MAX_OCR_DIMENSION,
                    u32::try_from(
                        crate::ABSOLUTE_MAX_OCR_PIXELS
                            / u64::from(crate::ABSOLUTE_MAX_OCR_DIMENSION)
                            + 1,
                    )
                    .expect("pixel-limit test height should fit u32"),
                ),
                "extraction_image_dimension_limit_exceeded",
            ),
        ] {
            let fixture = fixture(file_name, &contents);
            let error = create_screenshot_ocr_job_at(
                &fixture.database_path,
                fixture.scope_id,
                fixture.node_id,
            )
            .expect_err("invalid OCR input must fail before durable insert");
            assert_eq!(error.code(), expected_error);
            assert!(
                recent_extraction_jobs_at(&fixture.database_path)
                    .expect("jobs should remain queryable")
                    .is_empty(),
                "rejected OCR input must not leave a job"
            );
            let content =
                create_extraction_job_at(&fixture.database_path, fixture.scope_id, fixture.node_id)
                    .expect("rejected OCR must not block content extraction");
            assert_eq!(content.operation, ExtractionOperation::Content);
            assert_eq!(content.status, ExtractionStatus::Queued);
            assert_eq!(
                extraction_stats_at(&fixture.database_path)
                    .expect("stats should load")
                    .active_chunk_count,
                0
            );
        }
    }

    #[cfg(unix)]
    #[test]
    fn screenshot_ocr_creation_revalidates_open_handle_identity_before_insert() {
        let contents = png_bytes(640, 480);
        let fixture = fixture("Screenshot.png", &contents);
        let original_modified = fs::metadata(&fixture.file_path)
            .and_then(|metadata| metadata.modified())
            .expect("original modification time should load");
        let moved_original = fixture._directory.path().join("original.png");
        let replacement = fixture._directory.path().join("replacement.png");
        fs::write(&replacement, &contents).expect("replacement should write");
        File::open(&replacement)
            .and_then(|file| file.set_times(fs::FileTimes::new().set_modified(original_modified)))
            .expect("replacement modification time should match");
        fs::rename(&fixture.file_path, moved_original).expect("original should remain preserved");
        fs::rename(replacement, &fixture.file_path).expect("replacement should move into place");

        let error =
            create_screenshot_ocr_job_at(&fixture.database_path, fixture.scope_id, fixture.node_id)
                .expect_err("replacement identity must fail before durable insert");
        assert_eq!(error.code(), "extraction_source_identity_changed");
        assert!(
            recent_extraction_jobs_at(&fixture.database_path)
                .expect("jobs should remain queryable")
                .is_empty()
        );
        let content =
            create_extraction_job_at(&fixture.database_path, fixture.scope_id, fixture.node_id)
                .expect("rejected OCR must not block content extraction");
        assert_eq!(content.operation, ExtractionOperation::Content);
    }

    #[test]
    fn native_ocr_capacity_refusal_requeues_without_automatic_retry() {
        let fixture = fixture("Screenshot.png", &png_bytes(640, 480));
        let job =
            create_screenshot_ocr_job_at(&fixture.database_path, fixture.scope_id, fixture.node_id)
                .expect("OCR job should create");

        let capacity = run_extraction_job_with_ocr_provider_at(
            &fixture.database_path,
            job.job_id,
            ExtractionLimits::default(),
            &CapacityBusyOcrProvider,
        )
        .expect_err("capacity refusal should be explicit");
        assert_eq!(capacity.code(), "extraction_ocr_capacity_busy");
        let still_queued = extraction_job_at(&fixture.database_path, job.job_id)
            .expect("capacity refusal should preserve durable progress");
        assert_eq!(still_queued.status, ExtractionStatus::Queued);
        assert_eq!(still_queued.provider_id, None);
        assert_eq!(still_queued.error_code, None);

        let completed = run_extraction_job_with_ocr_provider_at(
            &fixture.database_path,
            job.job_id,
            ExtractionLimits::default(),
            &NoTextOcrProvider,
        )
        .expect("one explicit retry should run after capacity becomes available");
        assert_eq!(completed.status, ExtractionStatus::Completed);
    }

    #[test]
    fn screenshot_ocr_source_change_invalidates_prior_searchable_chunks() {
        let fixture = fixture("Screenshot.png", &png_bytes(640, 480));
        let original_modified = fs::metadata(&fixture.file_path)
            .and_then(|metadata| metadata.modified())
            .expect("source modified time should load");
        let initial_job =
            create_screenshot_ocr_job_at(&fixture.database_path, fixture.scope_id, fixture.node_id)
                .expect("initial OCR job should create");
        let initial = run_extraction_job_with_ocr_provider_at(
            &fixture.database_path,
            initial_job.job_id,
            ExtractionLimits::default(),
            &MixedLanguageOcrProvider,
        )
        .expect("initial OCR should complete");
        assert_eq!(initial.status, ExtractionStatus::Completed);
        assert_eq!(initial.chunk_count, 2);

        let stale_job =
            create_screenshot_ocr_job_at(&fixture.database_path, fixture.scope_id, fixture.node_id)
                .expect("replacement OCR job should create");
        let stale = run_extraction_job_with_ocr_provider_at(
            &fixture.database_path,
            stale_job.job_id,
            ExtractionLimits::default(),
            &MutatingOcrProvider {
                file_path: fixture.file_path.clone(),
                original_modified,
            },
        )
        .expect("source change should become a terminal job failure");

        assert_eq!(stale.status, ExtractionStatus::Failed);
        assert_eq!(
            stale.error_code.as_deref(),
            Some("extraction_source_changed")
        );
        assert_eq!(
            extraction_stats_at(&fixture.database_path)
                .expect("stale OCR content should invalidate")
                .active_chunk_count,
            0
        );
    }

    #[test]
    fn screenshot_ocr_provider_deadline_fails_without_partial_publication() {
        let fixture = fixture("Screenshot.png", &png_bytes(640, 480));
        let job =
            create_screenshot_ocr_job_at(&fixture.database_path, fixture.scope_id, fixture.node_id)
                .expect("OCR job should create");
        let started = Arc::new(AtomicBool::new(false));
        let limits = ExtractionLimits {
            max_processing_time: Duration::from_millis(250),
            ..ExtractionLimits::default()
        };
        let failed = run_extraction_job_with_ocr_provider_at(
            &fixture.database_path,
            job.job_id,
            limits,
            &BlockingOcrProvider {
                started: Arc::clone(&started),
            },
        )
        .expect("deadline should become a terminal job failure");

        assert!(started.load(Ordering::Acquire));
        assert_eq!(failed.status, ExtractionStatus::Failed);
        assert_eq!(
            failed.error_code.as_deref(),
            Some("extraction_time_limit_exceeded")
        );
        assert_eq!(
            extraction_stats_at(&fixture.database_path)
                .expect("timed out OCR must publish nothing")
                .active_chunk_count,
            0
        );
    }

    #[test]
    fn durable_cancel_reaches_a_running_ocr_provider_and_publishes_nothing() {
        let fixture = fixture("Screenshot.png", &png_bytes(640, 480));
        let job =
            create_screenshot_ocr_job_at(&fixture.database_path, fixture.scope_id, fixture.node_id)
                .expect("OCR job should create");
        let started = Arc::new(AtomicBool::new(false));
        let provider = BlockingOcrProvider {
            started: Arc::clone(&started),
        };
        let database_path = fixture.database_path.clone();
        let job_id = job.job_id;
        let runner = thread::spawn(move || {
            run_extraction_job_with_ocr_provider_at(
                &database_path,
                job_id,
                ExtractionLimits::default(),
                &provider,
            )
        });
        let wait_started = Instant::now();
        while !started.load(Ordering::Acquire) {
            assert!(
                wait_started.elapsed() < Duration::from_secs(2),
                "fake OCR provider should start"
            );
            thread::sleep(Duration::from_millis(5));
        }
        cancel_extraction_job_at(&fixture.database_path, job.job_id)
            .expect("durable cancel should persist");
        let cancelled = runner
            .join()
            .expect("OCR runner should not panic")
            .expect("OCR cancellation should return progress");

        assert_eq!(cancelled.status, ExtractionStatus::Cancelled);
        assert!(cancelled.cancel_requested);
        assert_eq!(
            extraction_stats_at(&fixture.database_path)
                .expect("stats should load")
                .active_chunk_count,
            0
        );
    }

    #[test]
    fn durable_cancel_wins_a_provider_failure_race() {
        let fixture = fixture("Screenshot.png", &png_bytes(640, 480));
        let job =
            create_screenshot_ocr_job_at(&fixture.database_path, fixture.scope_id, fixture.node_id)
                .expect("OCR job should create");
        let started = Arc::new(AtomicBool::new(false));
        let release_failure = Arc::new(AtomicBool::new(false));
        let provider = FailureAfterCancelOcrProvider {
            started: Arc::clone(&started),
            release_failure: Arc::clone(&release_failure),
        };
        let database_path = fixture.database_path.clone();
        let job_id = job.job_id;
        let runner = thread::spawn(move || {
            run_extraction_job_with_ocr_provider_at(
                &database_path,
                job_id,
                ExtractionLimits::default(),
                &provider,
            )
        });
        let wait_started = Instant::now();
        while !started.load(Ordering::Acquire) {
            assert!(
                wait_started.elapsed() < Duration::from_secs(2),
                "fake OCR provider should start"
            );
            thread::sleep(Duration::from_millis(2));
        }
        cancel_extraction_job_at(&fixture.database_path, job_id)
            .expect("durable cancel should persist before provider failure");
        release_failure.store(true, Ordering::Release);

        let terminal = runner
            .join()
            .expect("OCR runner should not panic")
            .expect("cancelled race should return terminal progress");
        assert_eq!(terminal.status, ExtractionStatus::Cancelled);
        assert!(terminal.cancel_requested);
        assert_eq!(terminal.error_code, None);
        assert_eq!(
            extraction_stats_at(&fixture.database_path)
                .expect("cancelled race should publish nothing")
                .active_chunk_count,
            0
        );
    }

    #[test]
    fn invalid_utf8_is_recorded_per_file_without_crashing_the_queue() {
        let fixture = fixture("invalid.txt", &[0x66, 0x80, 0x6f]);
        let job =
            create_extraction_job_at(&fixture.database_path, fixture.scope_id, fixture.node_id)
                .expect("job should create");

        let failed = run_extraction_job_at(
            &fixture.database_path,
            job.job_id,
            ExtractionLimits::default(),
        )
        .expect("per-file failure should return progress");

        assert_eq!(failed.status, ExtractionStatus::Failed);
        assert_eq!(
            failed.error_code.as_deref(),
            Some("extraction_invalid_utf8")
        );
        let stats = extraction_stats_at(&fixture.database_path).expect("stats should load");
        assert_eq!(stats.failed_job_count, 1);
        assert_eq!(stats.active_chunk_count, 0);
    }

    #[test]
    fn provider_failure_and_cancellation_preserve_prior_complete_chunks() {
        let fixture = fixture("notes.txt", b"stable content");
        let first =
            create_extraction_job_at(&fixture.database_path, fixture.scope_id, fixture.node_id)
                .expect("first job should create");
        let completed = run_extraction_job_at(
            &fixture.database_path,
            first.job_id,
            ExtractionLimits::default(),
        )
        .expect("first job should complete");
        assert_eq!(completed.status, ExtractionStatus::Completed);

        let retry =
            create_extraction_job_at(&fixture.database_path, fixture.scope_id, fixture.node_id)
                .expect("retry should create");
        let invalid_limits = ExtractionLimits {
            max_source_bytes: 0,
            ..ExtractionLimits::default()
        };
        let failed = run_extraction_job_at(&fixture.database_path, retry.job_id, invalid_limits)
            .expect("provider failure should be isolated");
        assert_eq!(failed.status, ExtractionStatus::Failed);
        assert_eq!(
            failed.error_code.as_deref(),
            Some("extraction_limits_invalid")
        );

        let cancelled =
            create_extraction_job_at(&fixture.database_path, fixture.scope_id, fixture.node_id)
                .expect("cancelled retry should create");
        cancel_extraction_job_at(&fixture.database_path, cancelled.job_id)
            .expect("queued retry should cancel");
        let stats = extraction_stats_at(&fixture.database_path).expect("stats should load");
        assert_eq!(stats.active_chunk_count, completed.chunk_count);
        assert_eq!(stats.extracted_file_count, 1);
    }

    #[test]
    fn source_change_invalidates_prior_chunks_instead_of_serving_stale_text() {
        let fixture = fixture("notes.txt", b"first");
        let first =
            create_extraction_job_at(&fixture.database_path, fixture.scope_id, fixture.node_id)
                .expect("job should create");
        let completed = run_extraction_job_at(
            &fixture.database_path,
            first.job_id,
            ExtractionLimits::default(),
        )
        .expect("first job should run");
        assert_eq!(completed.status, ExtractionStatus::Completed);
        fs::write(&fixture.file_path, b"second version").expect("fixture should change");
        let changed =
            create_extraction_job_at(&fixture.database_path, fixture.scope_id, fixture.node_id)
                .expect("changed job should create");

        let failed = run_extraction_job_at(
            &fixture.database_path,
            changed.job_id,
            ExtractionLimits::default(),
        )
        .expect("changed job should fail safely");

        assert_eq!(failed.status, ExtractionStatus::Failed);
        assert_eq!(
            failed.error_code.as_deref(),
            Some("extraction_source_metadata_changed")
        );
        assert_eq!(
            extraction_stats_at(&fixture.database_path)
                .expect("stats should load")
                .active_chunk_count,
            0
        );
    }

    #[cfg(unix)]
    #[test]
    fn post_scan_symlink_swap_is_denied_before_content_read() {
        use std::os::unix::fs::symlink;

        let fixture = fixture("notes.txt", b"authorized");
        let outside = fixture._directory.path().join("outside-secret.txt");
        let moved = fixture._directory.path().join("original-authorized.txt");
        fs::write(&outside, b"must never be extracted").expect("outside fixture should write");
        fs::rename(&fixture.file_path, &moved).expect("authorized fixture should move");
        symlink(&outside, &fixture.file_path).expect("symlink swap should create");
        let job =
            create_extraction_job_at(&fixture.database_path, fixture.scope_id, fixture.node_id)
                .expect("job should create from prior manifest");

        let failed = run_extraction_job_at(
            &fixture.database_path,
            job.job_id,
            ExtractionLimits::default(),
        )
        .expect("symlink swap should be isolated");

        assert_eq!(failed.status, ExtractionStatus::Failed);
        assert_eq!(
            failed.error_code.as_deref(),
            Some("extraction_source_excluded")
        );
        assert_eq!(
            extraction_stats_at(&fixture.database_path)
                .expect("stats should load")
                .active_chunk_count,
            0
        );
    }

    #[test]
    fn cancelled_queued_job_never_opens_the_source() {
        let fixture = fixture("notes.txt", b"cancel me");
        let job =
            create_extraction_job_at(&fixture.database_path, fixture.scope_id, fixture.node_id)
                .expect("job should create");
        let cancelled = cancel_extraction_job_at(&fixture.database_path, job.job_id)
            .expect("job should cancel");
        assert_eq!(cancelled.status, ExtractionStatus::Cancelled);

        let terminal = run_extraction_job_at(
            &fixture.database_path,
            job.job_id,
            ExtractionLimits::default(),
        )
        .expect("cancelled job should remain terminal");

        assert_eq!(terminal.status, ExtractionStatus::Cancelled);
        assert_eq!(terminal.chunk_count, 0);
    }
}
