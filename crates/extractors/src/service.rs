use std::fmt;
use std::fs::{self, File, Metadata};
use std::path::Path;
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use deskgraph_database::{
    ContentChunkProvenanceWrite, ContentChunkWrite, DatabaseError, ExtractableFile,
    ManifestDatabase,
};
use deskgraph_domain::{ExtractionJobProgress, ExtractionStats, ExtractionStatus};
use deskgraph_identity::{
    IdentityNodeKind, comparison_key, fallback_identity, has_hidden_or_system_attribute,
    is_symlink_or_reparse_point, path_from_raw, platform_identity_for_open_file,
};

use crate::{
    CancellationSignal, ChunkProvenance, ExtractionError, ExtractionLimits, ExtractionRequest,
    ExtractorProvider, MediaKind, Utf8TextExtractor, media_kind_for_extension,
};

// The provider's absolute processing cap is 60 seconds. Keep enough lease headroom for
// post-read identity validation and one atomic SQLite publish without permitting stale runners.
const RUNNER_LEASE_MS: i64 = 120_000;

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
    InvalidSystemTime,
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
            Self::InvalidSystemTime => "system_time_invalid",
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
    database
        .create_extraction_job(scope_id, node_id)
        .map_err(Into::into)
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
    validate_source(&database, &source)?;
    database.resume_extraction_job(job_id).map_err(Into::into)
}

pub fn run_extraction_job_at(
    database_path: &Path,
    job_id: i64,
    limits: ExtractionLimits,
) -> Result<ExtractionJobProgress, ExtractionServiceError> {
    let mut database = ManifestDatabase::open(database_path)?;
    let current = database.extraction_job(job_id)?;
    if current.is_terminal() || current.status == ExtractionStatus::Interrupted {
        return Ok(current);
    }
    let runner_token = runner_token()?;
    database.claim_extraction_job(job_id, &runner_token, RUNNER_LEASE_MS)?;
    let started = Instant::now();
    let provider = Utf8TextExtractor;
    let provider_id = provider.provider_id();
    let provider_version = provider.provider_version();

    let result = extract_claimed_job(&database, database_path, job_id, limits, &provider);
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
                    },
                    trust_class: chunk.trust_class,
                })
                .collect::<Vec<_>>();
            match database.complete_extraction_job(
                job_id,
                &runner_token,
                output.provider_id,
                output.provider_version,
                output.source_bytes,
                output.modified_unix_ns,
                output.output_bytes,
                elapsed_ms,
                &chunks,
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
        Err(error) => {
            if matches!(
                error,
                ExtractionServiceError::Extraction(ExtractionError::Cancelled)
            ) {
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

fn extract_claimed_job(
    database: &ManifestDatabase,
    database_path: &Path,
    job_id: i64,
    limits: ExtractionLimits,
    provider: &Utf8TextExtractor,
) -> Result<crate::ExtractionOutput, ExtractionServiceError> {
    let source = database.extractable_file_for_job(job_id)?;
    let (mut file, media_kind) = validate_source(database, &source)?;
    let cancellation = DatabaseCancellation::open(database_path, job_id)?;
    let output = provider.extract(
        &mut file,
        ExtractionRequest {
            media_kind,
            expected_source_bytes: source.size_bytes,
            modified_unix_ns: source.modified_unix_ns,
        },
        limits,
        &cancellation,
    )?;
    validate_open_file(&file, &source)?;
    Ok(output)
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
    use deskgraph_domain::ExtractionStatus;
    use deskgraph_scanner::{authorize_scope, comparison_key, scan_scope};
    use std::path::PathBuf;

    struct Fixture {
        _directory: tempfile::TempDir,
        database_path: PathBuf,
        scope_id: i64,
        node_id: i64,
        file_path: PathBuf,
    }

    fn fixture(file_name: &str, contents: &[u8]) -> Fixture {
        let directory = tempfile::tempdir().expect("fixture root should exist");
        let scope_path = directory.path().join("authorized");
        let database_path = directory.path().join("app-data/manifest.sqlite3");
        fs::create_dir(&scope_path).expect("scope should create");
        let file_path = scope_path.join(file_name);
        fs::write(&file_path, contents).expect("fixture should write");
        let mut database = ManifestDatabase::open(&database_path).expect("database should open");
        let scope = authorize_scope(&database, &scope_path).expect("scope should authorize");
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
