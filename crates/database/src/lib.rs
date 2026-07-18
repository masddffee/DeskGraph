use std::fmt;
use std::fs;
use std::path::{MAIN_SEPARATOR, Path};
use std::time::{SystemTime, UNIX_EPOCH};

use deskgraph_domain::{
    ActionExecutionStrategy, ActionOperation, ActionPlanPreview, ActionPlanState,
    ActionPlanSummary, ActionPolicyReport, AuthorizedScope, ExplicitFileVersionName,
    ExtractionJobProgress, ExtractionOperation, ExtractionStats, ExtractionStatus,
    FileRelationCandidate, FileRelationCandidateState, FileRelationCandidateSummary,
    FileRelationComparisonKind, FileRelationCreator, FileRelationDecision,
    FileRelationDecisionCreator, FileRelationDecisionKind, FileRelationEndpoint,
    FileRelationEvidence, FileRelationKind, FileVersionCandidate, FileVersionDecision,
    FileVersionEvidence, FileVersionSignalKind, FolderCategoryCount, FolderFileCategory,
    ImageFormat, ImageMetadata, ManifestStats, ProjectCandidate, ProjectCandidateState,
    ProjectCandidateSummary, ProjectDecision, ProjectDecisionCreator, ProjectDecisionKind,
    ProjectSignal, ProjectSignalKind, ProjectSuggestion, ProjectSuggestionCreator, ScanJobProgress,
    ScanReport, ScanStatus, WatchEventProgress, WatchEventReason, WatchEventStatus,
    is_valid_image_dimensions, is_valid_xlsx_cell_reference, parse_explicit_file_version_name,
};
use rusqlite::{Connection, OptionalExtension, Transaction, params};

const MIGRATIONS: &[Migration] = &[
    Migration {
        version: 1,
        name: "manifest",
        sql: include_str!("../../../migrations/0001_manifest.sql"),
    },
    Migration {
        version: 2,
        name: "resumable_scan_jobs",
        sql: include_str!("../../../migrations/0002_resumable_scan_jobs.sql"),
    },
    Migration {
        version: 3,
        name: "content_extraction",
        sql: include_str!("../../../migrations/0003_content_extraction.sql"),
    },
    Migration {
        version: 4,
        name: "content_chunk_provenance",
        sql: include_str!("../../../migrations/0004_content_chunk_provenance.sql"),
    },
    Migration {
        version: 5,
        name: "lexical_search",
        sql: include_str!("../../../migrations/0005_lexical_search.sql"),
    },
    Migration {
        version: 6,
        name: "watch_reconciliation",
        sql: include_str!("../../../migrations/0006_watch_reconciliation.sql"),
    },
    Migration {
        version: 7,
        name: "action_plan_preview",
        sql: include_str!("../../../migrations/0007_action_plan_preview.sql"),
    },
    Migration {
        version: 8,
        name: "project_candidates",
        sql: include_str!("../../../migrations/0008_project_candidates.sql"),
    },
    Migration {
        version: 9,
        name: "exact_duplicate_candidates",
        sql: include_str!("../../../migrations/0009_exact_duplicate_candidates.sql"),
    },
    Migration {
        version: 10,
        name: "file_relation_feedback",
        sql: include_str!("../../../migrations/0010_file_relation_feedback.sql"),
    },
    Migration {
        version: 11,
        name: "file_version_candidates",
        sql: include_str!("../../../migrations/0011_file_version_candidates.sql"),
    },
    Migration {
        version: 12,
        name: "file_version_feedback",
        sql: include_str!("../../../migrations/0012_file_version_feedback.sql"),
    },
    Migration {
        version: 13,
        name: "ooxml_chunk_provenance",
        sql: include_str!("../../../migrations/0013_ooxml_chunk_provenance.sql"),
    },
    Migration {
        version: 14,
        name: "image_metadata",
        sql: include_str!("../../../migrations/0014_image_metadata.sql"),
    },
    Migration {
        version: 15,
        name: "ocr_jobs_and_provenance",
        sql: include_str!("../../../migrations/0015_ocr_jobs_and_provenance.sql"),
    },
    Migration {
        version: 16,
        name: "nullable_ocr_confidence",
        sql: include_str!("../../../migrations/0016_nullable_ocr_confidence.sql"),
    },
];
const MAX_EXTRACTION_SOURCE_BYTES: u64 = 64 * 1024 * 1024;
const MAX_EXTRACTION_OUTPUT_BYTES: u64 = 64 * 1024 * 1024;
const MAX_EXTRACTION_CHUNKS: usize = 65_536;
const MAX_EXTRACTION_CHUNK_BYTES: usize = 64 * 1024;
const MAX_SEARCH_MATCH_BYTES: usize = 1024;
const MAX_SEARCH_CANDIDATES_PER_SOURCE: u32 = 100;
const MAX_WATCH_PATH_BYTES: usize = 64 * 1024;
const MAX_ACTION_PATH_BYTES: usize = 64 * 1024;
const MAX_FOLDER_PROFILE_ENTRIES: u64 = 100_000;
const MAX_FILE_RELATION_SOURCE_BYTES: u64 = 64 * 1024 * 1024;

struct Migration {
    version: i64,
    name: &'static str,
    sql: &'static str,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NodeKind {
    File,
    Folder,
}

impl NodeKind {
    fn as_str(self) -> &'static str {
        match self {
            Self::File => "file",
            Self::Folder => "folder",
        }
    }

    fn from_db(value: &str) -> Result<Self, DatabaseError> {
        match value {
            "file" => Ok(Self::File),
            "folder" => Ok(Self::Folder),
            _ => Err(DatabaseError::InvalidStoredValue),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Observation {
    pub kind: NodeKind,
    pub identity_kind: String,
    pub identity_key: Vec<u8>,
    pub parent_identity_key: Option<Vec<u8>>,
    pub path_raw: Vec<u8>,
    pub path_key: String,
    pub display_path: String,
    pub size_bytes: u64,
    pub modified_unix_ns: Option<i64>,
    pub link_count: Option<u64>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ScanIssue {
    pub code: &'static str,
    pub path_key: Option<String>,
    pub detail_code: Option<&'static str>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ScopeRecord {
    pub id: i64,
    pub path_raw: Vec<u8>,
    pub path_key: String,
    pub display_path: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct QueuedPath {
    pub path_raw: Vec<u8>,
    pub path_key: String,
    pub parent_identity_key: Option<Vec<u8>>,
    pub is_root: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct QueueEntry {
    pub id: i64,
    pub path_raw: Vec<u8>,
    pub path_key: String,
    pub parent_identity_key: Option<Vec<u8>>,
    pub is_root: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WatchSnapshotKind {
    Missing,
    File,
    Folder,
}

impl WatchSnapshotKind {
    fn as_str(self) -> &'static str {
        match self {
            Self::Missing => "missing",
            Self::File => "file",
            Self::Folder => "folder",
        }
    }

    fn from_db(value: &str) -> Result<Self, DatabaseError> {
        match value {
            "missing" => Ok(Self::Missing),
            "file" => Ok(Self::File),
            "folder" => Ok(Self::Folder),
            _ => Err(DatabaseError::InvalidStoredValue),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WatchSnapshot {
    pub kind: WatchSnapshotKind,
    pub size_bytes: Option<u64>,
    pub modified_unix_ns: Option<i64>,
    pub identity_key: Option<Vec<u8>>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WatchEventRecord {
    pub progress: WatchEventProgress,
    pub path_raw: Vec<u8>,
    pub path_key: String,
    pub snapshot: WatchSnapshot,
}

#[derive(Clone, Copy, Debug)]
pub struct WatchObservationWrite<'a> {
    pub scope_id: i64,
    pub path_raw: &'a [u8],
    pub path_key: &'a str,
    pub snapshot: &'a WatchSnapshot,
    pub stable_after_unix_ms: i64,
    pub ignored_reason: Option<WatchEventReason>,
    pub observed_at_unix_ms: i64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExtractableFile {
    pub scope_id: i64,
    pub node_id: i64,
    pub location_id: i64,
    pub path_raw: Vec<u8>,
    pub path_key: String,
    pub identity_kind: String,
    pub identity_key: Vec<u8>,
    pub size_bytes: u64,
    pub modified_unix_ns: Option<i64>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ActionSourceRecord {
    pub scope_id: i64,
    pub node_id: i64,
    pub location_id: i64,
    pub path_raw: Vec<u8>,
    pub path_key: String,
    pub display_path: String,
    pub identity_kind: String,
    pub identity_key: Vec<u8>,
    pub size_bytes: u64,
    pub modified_unix_ns: Option<i64>,
}

#[derive(Clone, Copy, Debug)]
pub struct ActionPlanWrite<'a> {
    pub scope_id: i64,
    pub node_id: i64,
    pub source_location_id: i64,
    pub source_path_raw: &'a [u8],
    pub source_path_key: &'a str,
    pub source_display_path: &'a str,
    pub destination_path_raw: &'a [u8],
    pub destination_path_key: &'a str,
    pub destination_display_path: &'a str,
    pub source_identity_kind: &'a str,
    pub source_identity_key: &'a [u8],
    pub source_size_bytes: u64,
    pub source_modified_unix_ns: Option<i64>,
    pub execution_strategy: ActionExecutionStrategy,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FolderProfileFacts {
    pub scope_id: i64,
    pub folder_node_id: i64,
    pub folder_location_id: i64,
    pub display_path: String,
    pub direct_file_count: u64,
    pub direct_folder_count: u64,
    pub descendant_file_count: u64,
    pub descendant_folder_count: u64,
    pub total_file_bytes: u64,
    pub latest_modified_unix_ns: Option<i64>,
    pub file_categories: Vec<FolderCategoryCount>,
    pub project_markers: Vec<ProjectSignalKind>,
    pub observed_at_unix_ms: i64,
    pub bounded_entry_limit: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ContentChunkProvenanceWrite {
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
        bbox_x_ppm: u32,
        bbox_y_ppm: u32,
        bbox_width_ppm: u32,
        bbox_height_ppm: u32,
        confidence_basis_points: Option<u16>,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ContentChunkWrite {
    pub ordinal: u32,
    pub text: String,
    pub provenance: ContentChunkProvenanceWrite,
    pub trust_class: &'static str,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ImageMetadataWrite {
    pub format: ImageFormat,
    pub pixel_width: u32,
    pub pixel_height: u32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LexicalCandidateSource {
    MetadataPath,
    ExtractedText,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LexicalSearchSource {
    All,
    MetadataPath,
    ExtractedText,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LexicalSearchFilters<'a> {
    pub scope_id: Option<i64>,
    pub source: LexicalSearchSource,
    pub extension: Option<&'a str>,
    pub modified_since_unix_ns: Option<i64>,
    pub modified_before_unix_ns: Option<i64>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct LexicalSearchCandidate {
    pub source: LexicalCandidateSource,
    pub scope_id: i64,
    pub node_id: i64,
    pub location_id: i64,
    pub display_path: String,
    pub snippet: Option<String>,
}

#[derive(Debug)]
pub enum DatabaseError {
    Io(std::io::Error),
    Sqlite(rusqlite::Error),
    MigrationChanged { version: i64 },
    ScopeNotFound,
    ScanJobNotFound,
    ScanJobAlreadyActive,
    ScanJobBusy,
    InvalidScanJobState,
    ScanJobIncomplete,
    RunnerLeaseLost,
    ExtractableFileNotFound,
    ExtractionJobNotFound,
    ExtractionJobAlreadyActive,
    ExtractionJobBusy,
    InvalidExtractionJobState,
    ExtractionRunnerLeaseLost,
    ExtractionOutputInvalid,
    ImageMetadataNotFound,
    SearchInputInvalid,
    WatchEventNotFound,
    InvalidWatchEventState,
    WatchInputInvalid,
    ActionSourceNotFound,
    ActionPlanNotFound,
    ActionPlanInputInvalid,
    ActionSourceSnapshotChanged,
    FolderNotFound,
    FolderProfileInputInvalid,
    FolderProfileTooLarge,
    ProjectCandidateNotFound,
    ProjectCandidateInputInvalid,
    ProjectCandidateRootNotCurrent,
    FileRelationCandidateNotFound,
    FileRelationCandidateInputInvalid,
    FileRelationCandidateNotCurrent,
    InvalidStoredValue,
    InvalidCount,
    InvalidTimestamp,
}

impl DatabaseError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::Io(_) => "database_io_failed",
            Self::Sqlite(_) => "database_operation_failed",
            Self::MigrationChanged { .. } => "database_migration_changed",
            Self::ScopeNotFound => "authorized_scope_not_found",
            Self::ScanJobNotFound => "scan_job_not_found",
            Self::ScanJobAlreadyActive => "scan_job_already_active",
            Self::ScanJobBusy => "scan_job_busy",
            Self::InvalidScanJobState => "invalid_scan_job_state",
            Self::ScanJobIncomplete => "scan_job_incomplete",
            Self::RunnerLeaseLost => "scan_runner_lease_lost",
            Self::ExtractableFileNotFound => "extractable_file_not_found",
            Self::ExtractionJobNotFound => "extraction_job_not_found",
            Self::ExtractionJobAlreadyActive => "extraction_job_already_active",
            Self::ExtractionJobBusy => "extraction_job_busy",
            Self::InvalidExtractionJobState => "invalid_extraction_job_state",
            Self::ExtractionRunnerLeaseLost => "extraction_runner_lease_lost",
            Self::ExtractionOutputInvalid => "extraction_output_invalid",
            Self::ImageMetadataNotFound => "image_metadata_not_found",
            Self::SearchInputInvalid => "search_input_invalid",
            Self::WatchEventNotFound => "watch_event_not_found",
            Self::InvalidWatchEventState => "invalid_watch_event_state",
            Self::WatchInputInvalid => "watch_input_invalid",
            Self::ActionSourceNotFound => "action_source_not_found",
            Self::ActionPlanNotFound => "action_plan_not_found",
            Self::ActionPlanInputInvalid => "action_plan_input_invalid",
            Self::ActionSourceSnapshotChanged => "action_source_snapshot_changed",
            Self::FolderNotFound => "folder_not_found",
            Self::FolderProfileInputInvalid => "folder_profile_input_invalid",
            Self::FolderProfileTooLarge => "folder_profile_entry_limit_exceeded",
            Self::ProjectCandidateNotFound => "project_candidate_not_found",
            Self::ProjectCandidateInputInvalid => "project_candidate_input_invalid",
            Self::ProjectCandidateRootNotCurrent => "project_candidate_root_not_current",
            Self::FileRelationCandidateNotFound => "file_relation_candidate_not_found",
            Self::FileRelationCandidateInputInvalid => "file_relation_candidate_input_invalid",
            Self::FileRelationCandidateNotCurrent => "file_relation_candidate_not_current",
            Self::InvalidStoredValue => "database_invalid_stored_value",
            Self::InvalidCount => "database_count_out_of_range",
            Self::InvalidTimestamp => "system_time_invalid",
        }
    }
}

impl fmt::Display for DatabaseError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.code())
    }
}

impl std::error::Error for DatabaseError {}

impl From<std::io::Error> for DatabaseError {
    fn from(error: std::io::Error) -> Self {
        Self::Io(error)
    }
}

impl From<rusqlite::Error> for DatabaseError {
    fn from(error: rusqlite::Error) -> Self {
        Self::Sqlite(error)
    }
}

pub struct ManifestDatabase {
    connection: Connection,
}

impl ManifestDatabase {
    pub fn open(path: &Path) -> Result<Self, DatabaseError> {
        if let Some(parent) = path
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
        {
            fs::create_dir_all(parent)?;
        }

        let connection = Connection::open(path)?;
        Self::from_connection(connection)
    }

    pub fn open_in_memory() -> Result<Self, DatabaseError> {
        Self::from_connection(Connection::open_in_memory()?)
    }

    fn from_connection(connection: Connection) -> Result<Self, DatabaseError> {
        connection.execute_batch(
            "PRAGMA foreign_keys = ON;\
             PRAGMA busy_timeout = 5000;\
             PRAGMA synchronous = FULL;",
        )?;
        if !connection.is_autocommit() {
            return Err(DatabaseError::Sqlite(
                rusqlite::Error::ExecuteReturnedResults,
            ));
        }
        if connection.path().is_some() {
            connection.pragma_update(None, "journal_mode", "WAL")?;
        }

        let mut database = Self { connection };
        database.apply_migrations()?;
        database.recover_expired_scan_jobs_at(unix_ms()?)?;
        database.recover_expired_extraction_jobs_at(unix_ms()?)?;
        Ok(database)
    }

    fn apply_migrations(&mut self) -> Result<(), DatabaseError> {
        self.connection.execute_batch(
            "CREATE TABLE IF NOT EXISTS schema_migrations (\
                version INTEGER PRIMARY KEY,\
                name TEXT NOT NULL,\
                checksum TEXT NOT NULL,\
                applied_at_unix_ms INTEGER NOT NULL\
             );",
        )?;

        for migration in MIGRATIONS {
            let checksum = migration_checksum(migration.sql);
            let existing = self
                .connection
                .query_row(
                    "SELECT checksum FROM schema_migrations WHERE version = ?1",
                    [migration.version],
                    |row| row.get::<_, String>(0),
                )
                .optional()?;

            if let Some(existing) = existing {
                if existing != checksum {
                    return Err(DatabaseError::MigrationChanged {
                        version: migration.version,
                    });
                }
                continue;
            }

            let applied_at = unix_ms()?;
            let transaction = self.connection.transaction()?;
            transaction.execute_batch(migration.sql)?;
            transaction.execute(
                "INSERT INTO schema_migrations(version, name, checksum, applied_at_unix_ms)\
                 VALUES (?1, ?2, ?3, ?4)",
                params![migration.version, migration.name, checksum, applied_at],
            )?;
            transaction.commit()?;
        }

        Ok(())
    }

    pub fn add_scope(
        &self,
        path_raw: &[u8],
        path_key: &str,
        display_path: &str,
        platform: &str,
    ) -> Result<AuthorizedScope, DatabaseError> {
        let created_at = unix_ms()?;
        self.connection.execute(
            "INSERT INTO authorized_scopes(path_raw, path_key, display_path, platform, created_at_unix_ms)\
             VALUES (?1, ?2, ?3, ?4, ?5)\
             ON CONFLICT(path_key) DO UPDATE SET path_raw = excluded.path_raw, display_path = excluded.display_path\
             ",
            params![path_raw, path_key, display_path, platform, created_at],
        )?;

        self.connection.query_row(
            "SELECT id, display_path, created_at_unix_ms FROM authorized_scopes WHERE path_key = ?1",
            [path_key],
            |row| {
                Ok(AuthorizedScope {
                    id: row.get(0)?,
                    display_path: row.get(1)?,
                    created_at_unix_ms: row.get(2)?,
                })
            },
        ).map_err(Into::into)
    }

    pub fn list_scopes(&self) -> Result<Vec<AuthorizedScope>, DatabaseError> {
        let mut statement = self.connection.prepare(
            "SELECT id, display_path, created_at_unix_ms FROM authorized_scopes ORDER BY id",
        )?;
        let rows = statement.query_map([], |row| {
            Ok(AuthorizedScope {
                id: row.get(0)?,
                display_path: row.get(1)?,
                created_at_unix_ms: row.get(2)?,
            })
        })?;
        rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    }

    pub fn scope_record(&self, scope_id: i64) -> Result<ScopeRecord, DatabaseError> {
        self.connection
            .query_row(
                "SELECT id, path_raw, path_key, display_path FROM authorized_scopes WHERE id = ?1",
                [scope_id],
                |row| {
                    Ok(ScopeRecord {
                        id: row.get(0)?,
                        path_raw: row.get(1)?,
                        path_key: row.get(2)?,
                        display_path: row.get(3)?,
                    })
                },
            )
            .optional()?
            .ok_or(DatabaseError::ScopeNotFound)
    }

    pub fn create_scan_job(&self, scope_id: i64) -> Result<i64, DatabaseError> {
        self.scope_record(scope_id)?;
        self.connection.execute(
            "INSERT INTO scan_jobs(scope_id, status, started_at_unix_ms) VALUES (?1, 'running', ?2)",
            params![scope_id, unix_ms()?],
        )?;
        Ok(self.connection.last_insert_rowid())
    }

    pub fn fail_scan(&self, job_id: i64, issue_count: u64) -> Result<(), DatabaseError> {
        self.connection.execute(
            "UPDATE scan_jobs SET status = 'failed', issue_count = ?2, finished_at_unix_ms = ?3\
             WHERE id = ?1 AND status = 'running'",
            params![job_id, to_i64(issue_count)?, unix_ms()?],
        )?;
        Ok(())
    }

    pub fn complete_scan(
        &mut self,
        job_id: i64,
        scope_id: i64,
        observations: &[Observation],
        issues: &[ScanIssue],
        skipped_entries: u64,
        elapsed_ms: u64,
    ) -> Result<ScanReport, DatabaseError> {
        let discovered_files = observations
            .iter()
            .filter(|entry| entry.kind == NodeKind::File)
            .count() as u64;
        let discovered_folders = observations
            .iter()
            .filter(|entry| entry.kind == NodeKind::Folder)
            .count() as u64;
        let finished_at = unix_ms()?;
        let transaction = self.connection.transaction()?;

        for observation in observations {
            upsert_observation(&transaction, scope_id, job_id, observation, finished_at)?;
        }

        transaction.execute(
            "UPDATE locations SET present = 0 WHERE scope_id = ?1 AND last_seen_scan_id <> ?2",
            params![scope_id, job_id],
        )?;
        transaction.execute(
            "UPDATE edges SET active = 0 WHERE scope_id = ?1 AND last_seen_scan_id <> ?2",
            params![scope_id, job_id],
        )?;
        invalidate_stale_extraction_outputs(&transaction, scope_id)?;

        for issue in issues {
            transaction.execute(
                "INSERT INTO scan_issues(scan_id, code, path_key, detail_code) VALUES (?1, ?2, ?3, ?4)",
                params![job_id, issue.code, issue.path_key, issue.detail_code],
            )?;
        }

        transaction.execute(
            "UPDATE scan_jobs SET status = 'completed', discovered_files = ?2, discovered_folders = ?3,\
                skipped_entries = ?4, issue_count = ?5, finished_at_unix_ms = ?6\
             WHERE id = ?1 AND scope_id = ?7 AND status = 'running'",
            params![
                job_id,
                to_i64(discovered_files)?,
                to_i64(discovered_folders)?,
                to_i64(skipped_entries)?,
                to_i64(issues.len() as u64)?,
                finished_at,
                scope_id,
            ],
        )?;
        transaction.commit()?;

        Ok(ScanReport {
            api_version: ScanReport::API_VERSION,
            job_id,
            scope_id,
            status: ScanStatus::Completed,
            discovered_files,
            discovered_folders,
            skipped_entries,
            issue_count: issues.len() as u64,
            elapsed_ms,
        })
    }

    pub fn create_resumable_scan_job(
        &mut self,
        scope_id: i64,
        root: &QueuedPath,
    ) -> Result<ScanJobProgress, DatabaseError> {
        let now = unix_ms()?;
        let transaction = self.connection.transaction()?;
        let job_id = insert_resumable_scan_job(&transaction, scope_id, root, now)?;
        transaction.commit()?;
        self.scan_job(job_id)
    }

    pub fn scan_job(&self, job_id: i64) -> Result<ScanJobProgress, DatabaseError> {
        self.connection
            .query_row(
                "SELECT id, scope_id, status, control_state, queued_entries, processed_entries, \
                    discovered_files, discovered_folders, skipped_entries, issue_count, elapsed_ms, pause_requested \
                 FROM scan_jobs WHERE id = ?1",
                [job_id],
                scan_job_from_row,
            )
            .optional()?
            .ok_or(DatabaseError::ScanJobNotFound)
    }

    pub fn recent_scan_jobs(&self) -> Result<Vec<ScanJobProgress>, DatabaseError> {
        let mut statement = self.connection.prepare(
            "SELECT id, scope_id, status, control_state, queued_entries, processed_entries, \
                discovered_files, discovered_folders, skipped_entries, issue_count, elapsed_ms, pause_requested \
             FROM scan_jobs ORDER BY id DESC LIMIT 20",
        )?;
        let jobs = statement.query_map([], scan_job_from_row)?;
        jobs.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    }

    pub fn record_watch_observation_at(
        &mut self,
        observation: WatchObservationWrite<'_>,
    ) -> Result<WatchEventRecord, DatabaseError> {
        validate_watch_observation(&observation)?;
        let transaction = self.connection.transaction()?;
        let scope_exists: i64 = transaction.query_row(
            "SELECT COUNT(*) FROM authorized_scopes WHERE id = ?1",
            [observation.scope_id],
            |row| row.get(0),
        )?;
        if scope_exists != 1 {
            return Err(DatabaseError::ScopeNotFound);
        }
        let (status, reason) = if let Some(reason) = observation.ignored_reason {
            ("ignored", Some(watch_reason_as_str(reason)))
        } else {
            ("stabilizing", None)
        };
        let size_bytes = observation.snapshot.size_bytes.map(to_i64).transpose()?;
        let event_id = if observation.ignored_reason.is_none() {
            let existing = transaction
                .query_row(
                    "SELECT id FROM watch_events WHERE scope_id = ?1 AND status = 'stabilizing'",
                    [observation.scope_id],
                    |row| row.get::<_, i64>(0),
                )
                .optional()?;
            if let Some(event_id) = existing {
                transaction.execute(
                    "UPDATE watch_events SET path_raw = ?2, path_key = ?3, observed_kind = ?4, \
                        observed_size_bytes = ?5, observed_modified_unix_ns = ?6, \
                        observed_identity_key = ?7, observation_count = observation_count + 1, \
                        stable_after_unix_ms = ?8, updated_at_unix_ms = ?9 \
                     WHERE id = ?1 AND status = 'stabilizing'",
                    params![
                        event_id,
                        observation.path_raw,
                        observation.path_key,
                        observation.snapshot.kind.as_str(),
                        size_bytes,
                        observation.snapshot.modified_unix_ns,
                        observation.snapshot.identity_key,
                        observation.stable_after_unix_ms,
                        observation.observed_at_unix_ms,
                    ],
                )?;
                event_id
            } else {
                insert_watch_event(&transaction, observation, status, reason, size_bytes)?
            }
        } else {
            insert_watch_event(&transaction, observation, status, reason, size_bytes)?
        };
        transaction.commit()?;
        self.watch_event(event_id)
    }

    pub fn watch_event(&self, event_id: i64) -> Result<WatchEventRecord, DatabaseError> {
        self.connection
            .query_row(
                "SELECT id, scope_id, status, observation_count, stable_after_unix_ms, \
                    scan_job_id, reason, path_raw, path_key, observed_kind, observed_size_bytes, \
                    observed_modified_unix_ns, observed_identity_key \
                 FROM watch_events WHERE id = ?1",
                [event_id],
                watch_event_from_row,
            )
            .optional()?
            .ok_or(DatabaseError::WatchEventNotFound)
    }

    pub fn recent_watch_events(&self) -> Result<Vec<WatchEventProgress>, DatabaseError> {
        let mut statement = self.connection.prepare(
            "SELECT id, scope_id, status, observation_count, stable_after_unix_ms, \
                scan_job_id, reason, path_raw, path_key, observed_kind, observed_size_bytes, \
                observed_modified_unix_ns, observed_identity_key \
             FROM watch_events ORDER BY id DESC LIMIT 20",
        )?;
        let events = statement.query_map([], watch_event_from_row)?;
        events
            .map(|event| event.map(|event| event.progress))
            .collect::<Result<Vec<_>, _>>()
            .map_err(Into::into)
    }

    pub fn mark_watch_event_ignored_at(
        &self,
        event_id: i64,
        reason: WatchEventReason,
        now_unix_ms: i64,
    ) -> Result<WatchEventProgress, DatabaseError> {
        if now_unix_ms < 0 {
            return Err(DatabaseError::WatchInputInvalid);
        }
        let changed = self.connection.execute(
            "UPDATE watch_events SET status = 'ignored', reason = ?2, updated_at_unix_ms = ?3 \
             WHERE id = ?1 AND status = 'stabilizing'",
            params![event_id, watch_reason_as_str(reason), now_unix_ms],
        )?;
        if changed != 1 {
            return Err(DatabaseError::InvalidWatchEventState);
        }
        Ok(self.watch_event(event_id)?.progress)
    }

    pub fn begin_watch_reconciliation_at(
        &mut self,
        event_id: i64,
        root: &QueuedPath,
        now_unix_ms: i64,
    ) -> Result<WatchEventProgress, DatabaseError> {
        if now_unix_ms < 0 {
            return Err(DatabaseError::WatchInputInvalid);
        }
        let transaction = self.connection.transaction()?;
        let (scope_id, status, stable_after): (i64, String, i64) = transaction
            .query_row(
                "SELECT scope_id, status, stable_after_unix_ms FROM watch_events WHERE id = ?1",
                [event_id],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .optional()?
            .ok_or(DatabaseError::WatchEventNotFound)?;
        if status != "stabilizing" || stable_after > now_unix_ms {
            return Err(DatabaseError::InvalidWatchEventState);
        }
        let scan_job_id = insert_resumable_scan_job(&transaction, scope_id, root, now_unix_ms)?;
        let changed = transaction.execute(
            "UPDATE watch_events SET status = 'reconciling', scan_job_id = ?2, reason = NULL, \
                updated_at_unix_ms = ?3 WHERE id = ?1 AND status = 'stabilizing'",
            params![event_id, scan_job_id, now_unix_ms],
        )?;
        if changed != 1 {
            return Err(DatabaseError::InvalidWatchEventState);
        }
        transaction.commit()?;
        Ok(self.watch_event(event_id)?.progress)
    }

    pub fn complete_watch_reconciliation_at(
        &self,
        event_id: i64,
        now_unix_ms: i64,
    ) -> Result<WatchEventProgress, DatabaseError> {
        if now_unix_ms < 0 {
            return Err(DatabaseError::WatchInputInvalid);
        }
        let changed = self.connection.execute(
            "UPDATE watch_events SET status = 'completed', reason = NULL, updated_at_unix_ms = ?2 \
             WHERE id = ?1 AND status = 'reconciling' AND EXISTS ( \
                SELECT 1 FROM scan_jobs WHERE scan_jobs.id = watch_events.scan_job_id \
                    AND scan_jobs.status = 'completed' \
             )",
            params![event_id, now_unix_ms],
        )?;
        if changed != 1 {
            return Err(DatabaseError::InvalidWatchEventState);
        }
        Ok(self.watch_event(event_id)?.progress)
    }

    pub fn fail_watch_event_at(
        &self,
        event_id: i64,
        reason: WatchEventReason,
        now_unix_ms: i64,
    ) -> Result<WatchEventProgress, DatabaseError> {
        if now_unix_ms < 0 {
            return Err(DatabaseError::WatchInputInvalid);
        }
        let changed = self.connection.execute(
            "UPDATE watch_events SET status = 'failed', reason = ?2, \
                updated_at_unix_ms = ?3 WHERE id = ?1 AND status IN ('stabilizing', 'reconciling')",
            params![event_id, watch_reason_as_str(reason), now_unix_ms],
        )?;
        if changed != 1 {
            return Err(DatabaseError::InvalidWatchEventState);
        }
        Ok(self.watch_event(event_id)?.progress)
    }

    pub fn request_scan_pause(&mut self, job_id: i64) -> Result<ScanJobProgress, DatabaseError> {
        let now = unix_ms()?;
        let transaction = self.connection.transaction()?;
        let (status, control): (String, String) = transaction
            .query_row(
                "SELECT status, control_state FROM scan_jobs WHERE id = ?1",
                [job_id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()?
            .ok_or(DatabaseError::ScanJobNotFound)?;
        if status != "running" {
            return Err(DatabaseError::InvalidScanJobState);
        }
        match control.as_str() {
            "ready" => {
                transaction.execute(
                    "UPDATE scan_jobs SET control_state = 'paused', pause_requested = 1, updated_at_unix_ms = ?2 \
                     WHERE id = ?1",
                    params![job_id, now],
                )?;
            }
            "active" => {
                transaction.execute(
                    "UPDATE scan_jobs SET control_state = 'pause_requested', pause_requested = 1, updated_at_unix_ms = ?2 \
                     WHERE id = ?1",
                    params![job_id, now],
                )?;
            }
            "pause_requested" | "paused" => {}
            _ => return Err(DatabaseError::InvalidStoredValue),
        }
        transaction.commit()?;
        self.scan_job(job_id)
    }

    pub fn resume_scan_job(&mut self, job_id: i64) -> Result<ScanJobProgress, DatabaseError> {
        let now = unix_ms()?;
        let transaction = self.connection.transaction()?;
        let (status, control): (String, String) = transaction
            .query_row(
                "SELECT status, control_state FROM scan_jobs WHERE id = ?1",
                [job_id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()?
            .ok_or(DatabaseError::ScanJobNotFound)?;
        if matches!(status.as_str(), "completed" | "failed") || control == "active" {
            return Err(DatabaseError::InvalidScanJobState);
        }
        if !matches!(status.as_str(), "running" | "interrupted") {
            return Err(DatabaseError::InvalidStoredValue);
        }
        transaction.execute(
            "UPDATE scan_queue SET state = 'pending' WHERE scan_id = ?1 AND state = 'processing'",
            [job_id],
        )?;
        transaction.execute(
            "UPDATE scan_jobs SET status = 'running', control_state = 'ready', pause_requested = 0, \
                runner_token = NULL, lease_expires_at_unix_ms = NULL, updated_at_unix_ms = ?2 \
             WHERE id = ?1",
            params![job_id, now],
        )?;
        transaction.commit()?;
        self.scan_job(job_id)
    }

    pub fn claim_scan_job(
        &mut self,
        job_id: i64,
        runner_token: &str,
        lease_ms: i64,
    ) -> Result<ScanJobProgress, DatabaseError> {
        let now = unix_ms()?;
        self.recover_expired_scan_jobs_at(now)?;
        let lease_expires = now
            .checked_add(lease_ms)
            .ok_or(DatabaseError::InvalidTimestamp)?;
        let transaction = self.connection.transaction()?;
        let (status, control, existing_runner): (String, String, Option<String>) = transaction
            .query_row(
                "SELECT status, control_state, runner_token FROM scan_jobs WHERE id = ?1",
                [job_id],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .optional()?
            .ok_or(DatabaseError::ScanJobNotFound)?;
        if status != "running" {
            return Err(DatabaseError::InvalidScanJobState);
        }
        match control.as_str() {
            "ready" => {}
            "active" if existing_runner.as_deref() == Some(runner_token) => {}
            "active" => return Err(DatabaseError::ScanJobBusy),
            "pause_requested" | "paused" => return Err(DatabaseError::InvalidScanJobState),
            _ => return Err(DatabaseError::InvalidStoredValue),
        }
        transaction.execute(
            "UPDATE scan_jobs SET control_state = 'active', runner_token = ?2, \
                lease_expires_at_unix_ms = ?3, updated_at_unix_ms = ?4 \
             WHERE id = ?1",
            params![job_id, runner_token, lease_expires, now],
        )?;
        transaction.commit()?;
        self.scan_job(job_id)
    }

    pub fn next_scan_queue_entry(
        &mut self,
        job_id: i64,
        runner_token: &str,
        lease_ms: i64,
    ) -> Result<Option<QueueEntry>, DatabaseError> {
        let now = unix_ms()?;
        let lease_expires = now
            .checked_add(lease_ms)
            .ok_or(DatabaseError::InvalidTimestamp)?;
        let transaction = self.connection.transaction()?;
        ensure_active_runner(&transaction, job_id, runner_token, now)?;
        let entry = transaction
            .query_row(
                "SELECT id, path_raw, path_key, parent_identity_key, is_root \
                 FROM scan_queue WHERE scan_id = ?1 AND state = 'pending' ORDER BY id LIMIT 1",
                [job_id],
                |row| {
                    Ok(QueueEntry {
                        id: row.get(0)?,
                        path_raw: row.get(1)?,
                        path_key: row.get(2)?,
                        parent_identity_key: row.get(3)?,
                        is_root: row.get::<_, i64>(4)? != 0,
                    })
                },
            )
            .optional()?;
        if let Some(entry) = &entry {
            let changed = transaction.execute(
                "UPDATE scan_queue SET state = 'processing' WHERE id = ?1 AND scan_id = ?2 AND state = 'pending'",
                params![entry.id, job_id],
            )?;
            if changed != 1 {
                return Err(DatabaseError::ScanJobBusy);
            }
            transaction.execute(
                "UPDATE scan_jobs SET lease_expires_at_unix_ms = ?2, updated_at_unix_ms = ?3 WHERE id = ?1",
                params![job_id, lease_expires, now],
            )?;
        }
        transaction.commit()?;
        Ok(entry)
    }

    #[allow(clippy::too_many_arguments)]
    pub fn stage_scan_queue_entry(
        &mut self,
        job_id: i64,
        runner_token: &str,
        queue_entry_id: i64,
        observation: Option<&Observation>,
        children: &[QueuedPath],
        issues: &[ScanIssue],
        skipped_entries: u64,
        elapsed_ms: u64,
        lease_ms: i64,
    ) -> Result<ScanJobProgress, DatabaseError> {
        let now = unix_ms()?;
        let lease_expires = now
            .checked_add(lease_ms)
            .ok_or(DatabaseError::InvalidTimestamp)?;
        let transaction = self.connection.transaction()?;
        ensure_owned_runner(&transaction, job_id, runner_token, now)?;

        if let Some(observation) = observation {
            transaction.execute(
                "INSERT INTO scan_staged_observations( \
                    scan_id, kind, identity_kind, identity_key, parent_identity_key, path_raw, \
                    path_key, display_path, size_bytes, modified_unix_ns, link_count \
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11) \
                 ON CONFLICT(scan_id, path_key) DO UPDATE SET \
                    kind = excluded.kind, identity_kind = excluded.identity_kind, identity_key = excluded.identity_key, \
                    parent_identity_key = excluded.parent_identity_key, path_raw = excluded.path_raw, \
                    display_path = excluded.display_path, size_bytes = excluded.size_bytes, \
                    modified_unix_ns = excluded.modified_unix_ns, link_count = excluded.link_count",
                params![
                    job_id,
                    observation.kind.as_str(),
                    observation.identity_kind,
                    observation.identity_key,
                    observation.parent_identity_key,
                    observation.path_raw,
                    observation.path_key,
                    observation.display_path,
                    to_i64(observation.size_bytes)?,
                    observation.modified_unix_ns,
                    observation.link_count.map(to_i64).transpose()?,
                ],
            )?;
        }

        for child in children {
            transaction.execute(
                "INSERT INTO scan_queue( \
                    scan_id, path_raw, path_key, parent_identity_key, is_root, state \
                 ) VALUES (?1, ?2, ?3, ?4, ?5, 'pending') \
                 ON CONFLICT(scan_id, path_key) DO NOTHING",
                params![
                    job_id,
                    child.path_raw,
                    child.path_key,
                    child.parent_identity_key,
                    i64::from(child.is_root),
                ],
            )?;
        }
        for issue in issues {
            transaction.execute(
                "INSERT INTO scan_staged_issues(scan_id, code, path_key, detail_code) \
                 VALUES (?1, ?2, ?3, ?4)",
                params![job_id, issue.code, issue.path_key, issue.detail_code],
            )?;
        }
        let changed = transaction.execute(
            "UPDATE scan_queue SET state = 'done' WHERE id = ?1 AND scan_id = ?2 AND state = 'processing'",
            params![queue_entry_id, job_id],
        )?;
        if changed != 1 {
            return Err(DatabaseError::RunnerLeaseLost);
        }

        let queued_entries: i64 = transaction.query_row(
            "SELECT COUNT(*) FROM scan_queue WHERE scan_id = ?1",
            [job_id],
            |row| row.get(0),
        )?;
        let processed_entries: i64 = transaction.query_row(
            "SELECT COUNT(*) FROM scan_queue WHERE scan_id = ?1 AND state = 'done'",
            [job_id],
            |row| row.get(0),
        )?;
        let file_increment =
            i64::from(observation.is_some_and(|value| value.kind == NodeKind::File));
        let folder_increment =
            i64::from(observation.is_some_and(|value| value.kind == NodeKind::Folder));
        transaction.execute(
            "UPDATE scan_jobs SET queued_entries = ?2, processed_entries = ?3, \
                discovered_files = discovered_files + ?4, discovered_folders = discovered_folders + ?5, \
                skipped_entries = skipped_entries + ?6, issue_count = issue_count + ?7, \
                elapsed_ms = elapsed_ms + ?8, lease_expires_at_unix_ms = ?9, updated_at_unix_ms = ?10 \
             WHERE id = ?1",
            params![
                job_id,
                queued_entries,
                processed_entries,
                file_increment,
                folder_increment,
                to_i64(skipped_entries)?,
                to_i64(issues.len() as u64)?,
                to_i64(elapsed_ms)?,
                lease_expires,
                now,
            ],
        )?;
        transaction.commit()?;
        self.scan_job(job_id)
    }

    pub fn release_scan_job(
        &mut self,
        job_id: i64,
        runner_token: &str,
    ) -> Result<ScanJobProgress, DatabaseError> {
        let now = unix_ms()?;
        let transaction = self.connection.transaction()?;
        ensure_owned_runner(&transaction, job_id, runner_token, now)?;
        transaction.execute(
            "UPDATE scan_jobs SET \
                control_state = CASE WHEN pause_requested = 1 THEN 'paused' ELSE 'ready' END, \
                runner_token = NULL, lease_expires_at_unix_ms = NULL, updated_at_unix_ms = ?3 \
             WHERE id = ?1 AND runner_token = ?2",
            params![job_id, runner_token, now],
        )?;
        transaction.commit()?;
        self.scan_job(job_id)
    }

    pub fn record_scan_runner_elapsed(
        &mut self,
        job_id: i64,
        runner_token: &str,
        elapsed_ms: u64,
    ) -> Result<ScanJobProgress, DatabaseError> {
        let now = unix_ms()?;
        let transaction = self.connection.transaction()?;
        ensure_owned_runner(&transaction, job_id, runner_token, now)?;
        transaction.execute(
            "UPDATE scan_jobs SET elapsed_ms = elapsed_ms + ?3, updated_at_unix_ms = ?4 \
             WHERE id = ?1 AND runner_token = ?2",
            params![job_id, runner_token, to_i64(elapsed_ms)?, now],
        )?;
        transaction.commit()?;
        self.scan_job(job_id)
    }

    pub fn finalize_resumable_scan_job(
        &mut self,
        job_id: i64,
        runner_token: &str,
    ) -> Result<ScanJobProgress, DatabaseError> {
        let now = unix_ms()?;
        let transaction = self.connection.transaction()?;
        ensure_owned_runner(&transaction, job_id, runner_token, now)?;
        let remaining: i64 = transaction.query_row(
            "SELECT COUNT(*) FROM scan_queue WHERE scan_id = ?1 AND state <> 'done'",
            [job_id],
            |row| row.get(0),
        )?;
        let pause_requested: i64 = transaction.query_row(
            "SELECT pause_requested FROM scan_jobs WHERE id = ?1",
            [job_id],
            |row| row.get(0),
        )?;
        if remaining != 0 || pause_requested != 0 {
            return Err(DatabaseError::ScanJobIncomplete);
        }
        let scope_id: i64 = transaction.query_row(
            "SELECT scope_id FROM scan_jobs WHERE id = ?1",
            [job_id],
            |row| row.get(0),
        )?;

        let observations = {
            let mut statement = transaction.prepare(
                "SELECT kind, identity_kind, identity_key, parent_identity_key, path_raw, path_key, \
                    display_path, size_bytes, modified_unix_ns, link_count \
                 FROM scan_staged_observations WHERE scan_id = ?1 ORDER BY id",
            )?;
            let rows = statement.query_map([job_id], |row| {
                let kind_text: String = row.get(0)?;
                let size: i64 = row.get(7)?;
                let link_count: Option<i64> = row.get(9)?;
                Ok((
                    kind_text,
                    row.get::<_, String>(1)?,
                    row.get::<_, Vec<u8>>(2)?,
                    row.get::<_, Option<Vec<u8>>>(3)?,
                    row.get::<_, Vec<u8>>(4)?,
                    row.get::<_, String>(5)?,
                    row.get::<_, String>(6)?,
                    size,
                    row.get::<_, Option<i64>>(8)?,
                    link_count,
                ))
            })?;
            let mut observations = Vec::new();
            for row in rows {
                let (
                    kind,
                    identity_kind,
                    identity_key,
                    parent_identity_key,
                    path_raw,
                    path_key,
                    display_path,
                    size_bytes,
                    modified_unix_ns,
                    link_count,
                ) = row?;
                observations.push(Observation {
                    kind: NodeKind::from_db(&kind)?,
                    identity_kind,
                    identity_key,
                    parent_identity_key,
                    path_raw,
                    path_key,
                    display_path,
                    size_bytes: u64::try_from(size_bytes)
                        .map_err(|_| DatabaseError::InvalidStoredValue)?,
                    modified_unix_ns,
                    link_count: link_count
                        .map(u64::try_from)
                        .transpose()
                        .map_err(|_| DatabaseError::InvalidStoredValue)?,
                });
            }
            observations
        };
        for observation in &observations {
            upsert_observation(&transaction, scope_id, job_id, observation, now)?;
        }
        transaction.execute(
            "UPDATE locations SET present = 0 WHERE scope_id = ?1 AND last_seen_scan_id <> ?2",
            params![scope_id, job_id],
        )?;
        transaction.execute(
            "UPDATE edges SET active = 0 WHERE scope_id = ?1 AND last_seen_scan_id <> ?2",
            params![scope_id, job_id],
        )?;
        invalidate_stale_extraction_outputs(&transaction, scope_id)?;
        transaction.execute(
            "INSERT INTO scan_issues(scan_id, code, path_key, detail_code) \
             SELECT scan_id, code, path_key, detail_code FROM scan_staged_issues WHERE scan_id = ?1",
            [job_id],
        )?;
        transaction.execute(
            "UPDATE scan_jobs SET status = 'completed', control_state = 'ready', pause_requested = 0, \
                runner_token = NULL, lease_expires_at_unix_ms = NULL, finished_at_unix_ms = ?2, \
                updated_at_unix_ms = ?2 WHERE id = ?1",
            params![job_id, now],
        )?;
        transaction.execute("DELETE FROM scan_queue WHERE scan_id = ?1", [job_id])?;
        transaction.execute(
            "DELETE FROM scan_staged_observations WHERE scan_id = ?1",
            [job_id],
        )?;
        transaction.execute(
            "DELETE FROM scan_staged_issues WHERE scan_id = ?1",
            [job_id],
        )?;
        transaction.commit()?;
        self.scan_job(job_id)
    }

    pub fn fail_resumable_scan_job(
        &mut self,
        job_id: i64,
        runner_token: &str,
    ) -> Result<ScanJobProgress, DatabaseError> {
        let now = unix_ms()?;
        let changed = self.connection.execute(
            "UPDATE scan_jobs SET status = 'failed', control_state = 'ready', runner_token = NULL, \
                lease_expires_at_unix_ms = NULL, finished_at_unix_ms = ?3, updated_at_unix_ms = ?3 \
             WHERE id = ?1 AND runner_token = ?2 AND status = 'running' \
                AND lease_expires_at_unix_ms IS NOT NULL AND lease_expires_at_unix_ms > ?3",
            params![job_id, runner_token, now],
        )?;
        if changed != 1 {
            return Err(DatabaseError::RunnerLeaseLost);
        }
        self.scan_job(job_id)
    }

    pub fn recover_expired_scan_jobs_at(&mut self, now: i64) -> Result<u64, DatabaseError> {
        let transaction = self.connection.transaction()?;
        let recovered = transaction.execute(
            "UPDATE scan_jobs SET status = 'interrupted', control_state = 'ready', runner_token = NULL, \
                lease_expires_at_unix_ms = NULL, updated_at_unix_ms = ?1 \
             WHERE status = 'running' AND control_state IN ('active', 'pause_requested') \
                AND lease_expires_at_unix_ms IS NOT NULL AND lease_expires_at_unix_ms <= ?1",
            [now],
        )?;
        transaction.execute(
            "UPDATE scan_queue SET state = 'pending' WHERE state = 'processing' AND scan_id IN ( \
                SELECT id FROM scan_jobs WHERE status = 'interrupted' \
             )",
            [],
        )?;
        transaction.commit()?;
        u64::try_from(recovered).map_err(|_| DatabaseError::InvalidCount)
    }

    pub fn extractable_file(
        &self,
        scope_id: i64,
        node_id: i64,
    ) -> Result<ExtractableFile, DatabaseError> {
        self.connection
            .query_row(
                "SELECT l.scope_id, l.node_id, l.id, l.path_raw, l.path_key, \
                    n.identity_kind, n.identity_key, f.size_bytes, f.modified_unix_ns \
                 FROM locations l \
                 JOIN nodes n ON n.id = l.node_id \
                 JOIN files f ON f.node_id = l.node_id \
                 WHERE l.scope_id = ?1 AND l.node_id = ?2 AND l.present = 1 AND n.kind = 'file' \
                 ORDER BY l.id LIMIT 1",
                params![scope_id, node_id],
                |row| {
                    let size_bytes: i64 = row.get(7)?;
                    Ok(ExtractableFile {
                        scope_id: row.get(0)?,
                        node_id: row.get(1)?,
                        location_id: row.get(2)?,
                        path_raw: row.get(3)?,
                        path_key: row.get(4)?,
                        identity_kind: row.get(5)?,
                        identity_key: row.get(6)?,
                        size_bytes: u64::try_from(size_bytes)
                            .map_err(|_| rusqlite::Error::IntegralValueOutOfRange(7, size_bytes))?,
                        modified_unix_ns: row.get(8)?,
                    })
                },
            )
            .optional()?
            .ok_or(DatabaseError::ExtractableFileNotFound)
    }

    pub fn extractable_file_for_job(&self, job_id: i64) -> Result<ExtractableFile, DatabaseError> {
        self.connection
            .query_row(
                "SELECT l.scope_id, l.node_id, l.id, l.path_raw, l.path_key, \
                    n.identity_kind, n.identity_key, j.source_size_bytes, j.source_modified_unix_ns \
                 FROM extraction_jobs j \
                 JOIN locations l ON l.id = j.location_id AND l.node_id = j.node_id \
                 JOIN nodes n ON n.id = l.node_id \
                 JOIN files f ON f.node_id = l.node_id \
                 WHERE j.id = ?1 AND l.present = 1 AND n.kind = 'file'",
                [job_id],
                |row| {
                    let size_bytes: i64 = row.get(7)?;
                    Ok(ExtractableFile {
                        scope_id: row.get(0)?,
                        node_id: row.get(1)?,
                        location_id: row.get(2)?,
                        path_raw: row.get(3)?,
                        path_key: row.get(4)?,
                        identity_kind: row.get(5)?,
                        identity_key: row.get(6)?,
                        size_bytes: u64::try_from(size_bytes)
                            .map_err(|_| rusqlite::Error::IntegralValueOutOfRange(7, size_bytes))?,
                        modified_unix_ns: row.get(8)?,
                    })
                },
            )
            .optional()?
            .ok_or(DatabaseError::ExtractableFileNotFound)
    }

    pub fn create_extraction_job(
        &mut self,
        scope_id: i64,
        node_id: i64,
    ) -> Result<ExtractionJobProgress, DatabaseError> {
        self.create_extraction_job_for_operation(scope_id, node_id, ExtractionOperation::Content)
    }

    #[cfg(test)]
    fn create_screenshot_ocr_job(
        &mut self,
        scope_id: i64,
        node_id: i64,
    ) -> Result<ExtractionJobProgress, DatabaseError> {
        self.create_extraction_job_for_operation(
            scope_id,
            node_id,
            ExtractionOperation::ScreenshotOcr,
        )
    }

    /// Low-level storage compare-and-insert used only after the extraction core
    /// has validated the authorized scope, open handle, and encoded image.
    ///
    /// This method rechecks only the manifest metadata snapshot inside the
    /// insertion transaction. It does not prove that file bytes are unchanged,
    /// and it is not a filesystem or image-validation entry point. Workspace
    /// callers must use `deskgraph_extractors::create_screenshot_ocr_job_at`.
    #[doc(hidden)]
    pub fn low_level_insert_screenshot_ocr_job_after_core_validation(
        &mut self,
        source: &ExtractableFile,
    ) -> Result<ExtractionJobProgress, DatabaseError> {
        self.create_extraction_job_for_current_source(source, ExtractionOperation::ScreenshotOcr)
    }

    fn create_extraction_job_for_operation(
        &mut self,
        scope_id: i64,
        node_id: i64,
        operation: ExtractionOperation,
    ) -> Result<ExtractionJobProgress, DatabaseError> {
        let source = self.extractable_file(scope_id, node_id)?;
        self.create_extraction_job_for_current_source(&source, operation)
    }

    fn create_extraction_job_for_current_source(
        &mut self,
        source: &ExtractableFile,
        operation: ExtractionOperation,
    ) -> Result<ExtractionJobProgress, DatabaseError> {
        let now = unix_ms()?;
        let transaction = self.connection.transaction()?;
        let current: i64 = transaction.query_row(
            "SELECT COUNT(*) \
             FROM locations l \
             JOIN nodes n ON n.id = l.node_id \
             JOIN files f ON f.node_id = l.node_id \
             WHERE l.scope_id = ?1 AND l.node_id = ?2 AND l.id = ?3 \
                AND l.path_raw = ?4 AND l.path_key = ?5 AND l.present = 1 \
                AND n.kind = 'file' AND n.identity_kind = ?6 AND n.identity_key = ?7 \
                AND f.size_bytes = ?8 AND f.modified_unix_ns IS ?9",
            params![
                source.scope_id,
                source.node_id,
                source.location_id,
                source.path_raw,
                source.path_key,
                source.identity_kind,
                source.identity_key,
                to_i64(source.size_bytes)?,
                source.modified_unix_ns,
            ],
            |row| row.get(0),
        )?;
        if current != 1 {
            return Err(DatabaseError::ExtractableFileNotFound);
        }
        let active: i64 = transaction.query_row(
            "SELECT COUNT(*) FROM extraction_jobs \
             WHERE scope_id = ?1 AND node_id = ?2 AND status IN ('queued', 'running', 'interrupted')",
            params![source.scope_id, source.node_id],
            |row| row.get(0),
        )?;
        if active != 0 {
            return Err(DatabaseError::ExtractionJobAlreadyActive);
        }
        transaction.execute(
            "INSERT INTO extraction_jobs( \
                scope_id, node_id, location_id, status, source_size_bytes, source_modified_unix_ns, \
                created_at_unix_ms, updated_at_unix_ms, operation \
             ) VALUES (?1, ?2, ?3, 'queued', ?4, ?5, ?6, ?6, ?7)",
            params![
                source.scope_id,
                source.node_id,
                source.location_id,
                to_i64(source.size_bytes)?,
                source.modified_unix_ns,
                now,
                operation.as_str(),
            ],
        )?;
        let job_id = transaction.last_insert_rowid();
        transaction.commit()?;
        self.extraction_job(job_id)
    }

    pub fn extraction_job(&self, job_id: i64) -> Result<ExtractionJobProgress, DatabaseError> {
        self.connection
            .query_row(
                "SELECT id, scope_id, node_id, operation, status, provider_id, provider_version, error_code, \
                    source_size_bytes, output_bytes, chunk_count, elapsed_ms, cancel_requested \
                 FROM extraction_jobs WHERE id = ?1",
                [job_id],
                extraction_job_from_row,
            )
            .optional()?
            .ok_or(DatabaseError::ExtractionJobNotFound)
    }

    pub fn recent_extraction_jobs(&self) -> Result<Vec<ExtractionJobProgress>, DatabaseError> {
        let mut statement = self.connection.prepare(
            "SELECT id, scope_id, node_id, operation, status, provider_id, provider_version, error_code, \
                source_size_bytes, output_bytes, chunk_count, elapsed_ms, cancel_requested \
             FROM extraction_jobs ORDER BY id DESC LIMIT 20",
        )?;
        let jobs = statement.query_map([], extraction_job_from_row)?;
        jobs.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    }

    /// Returns the most actionable screenshot OCR job for one manifest node.
    ///
    /// Active or interrupted work wins over terminal history, so an interrupted
    /// job remains discoverable for recovery even after later terminal rows are
    /// present. The query deliberately returns only the path-free progress
    /// projection used by desktop IPC.
    pub fn screenshot_ocr_job_for_node(
        &self,
        scope_id: i64,
        node_id: i64,
    ) -> Result<Option<ExtractionJobProgress>, DatabaseError> {
        self.connection
            .query_row(
                "SELECT id, scope_id, node_id, operation, status, provider_id, provider_version, error_code, \
                    source_size_bytes, output_bytes, chunk_count, elapsed_ms, cancel_requested \
                 FROM extraction_jobs \
                 WHERE scope_id = ?1 AND node_id = ?2 AND operation = 'screenshot_ocr' \
                 ORDER BY CASE status \
                    WHEN 'running' THEN 0 \
                    WHEN 'queued' THEN 1 \
                    WHEN 'interrupted' THEN 2 \
                    ELSE 3 \
                 END, id DESC \
                 LIMIT 1",
                params![scope_id, node_id],
                extraction_job_from_row,
            )
            .optional()
            .map_err(Into::into)
    }

    pub fn image_metadata_for_job(&self, job_id: i64) -> Result<ImageMetadata, DatabaseError> {
        let stored: (i64, i64, String, i64, i64, i64, String, String) = self
            .connection
            .query_row(
                "SELECT scope_id, node_id, format, pixel_width, pixel_height, source_size_bytes, \
                    provider_id, provider_version \
                 FROM image_metadata WHERE extraction_job_id = ?1",
                [job_id],
                |row| {
                    Ok((
                        row.get(0)?,
                        row.get(1)?,
                        row.get(2)?,
                        row.get(3)?,
                        row.get(4)?,
                        row.get(5)?,
                        row.get(6)?,
                        row.get(7)?,
                    ))
                },
            )
            .optional()?
            .ok_or(DatabaseError::ImageMetadataNotFound)?;
        let format =
            ImageFormat::from_storage(&stored.2).ok_or(DatabaseError::InvalidStoredValue)?;
        let pixel_width = u32::try_from(stored.3).map_err(|_| DatabaseError::InvalidStoredValue)?;
        let pixel_height =
            u32::try_from(stored.4).map_err(|_| DatabaseError::InvalidStoredValue)?;
        if !is_valid_image_dimensions(pixel_width, pixel_height) {
            return Err(DatabaseError::InvalidStoredValue);
        }
        Ok(ImageMetadata {
            api_version: ImageMetadata::API_VERSION,
            extraction_job_id: job_id,
            scope_id: stored.0,
            node_id: stored.1,
            format,
            pixel_width,
            pixel_height,
            source_bytes: u64::try_from(stored.5).map_err(|_| DatabaseError::InvalidStoredValue)?,
            provider_id: stored.6,
            provider_version: stored.7,
        })
    }

    pub fn request_extraction_cancel(
        &mut self,
        job_id: i64,
    ) -> Result<ExtractionJobProgress, DatabaseError> {
        let now = unix_ms()?;
        let transaction = self.connection.transaction()?;
        let status: String = transaction
            .query_row(
                "SELECT status FROM extraction_jobs WHERE id = ?1",
                [job_id],
                |row| row.get(0),
            )
            .optional()?
            .ok_or(DatabaseError::ExtractionJobNotFound)?;
        match status.as_str() {
            "queued" | "interrupted" => {
                transaction.execute(
                    "UPDATE extraction_jobs SET status = 'cancelled', cancel_requested = 1, \
                        finished_at_unix_ms = ?2, updated_at_unix_ms = ?2 \
                     WHERE id = ?1",
                    params![job_id, now],
                )?;
            }
            "running" => {
                transaction.execute(
                    "UPDATE extraction_jobs SET cancel_requested = 1, updated_at_unix_ms = ?2 \
                     WHERE id = ?1",
                    params![job_id, now],
                )?;
            }
            "cancelled" => {}
            "completed" | "failed" => return Err(DatabaseError::InvalidExtractionJobState),
            _ => return Err(DatabaseError::InvalidStoredValue),
        }
        transaction.commit()?;
        self.extraction_job(job_id)
    }

    pub fn resume_extraction_job(
        &mut self,
        job_id: i64,
    ) -> Result<ExtractionJobProgress, DatabaseError> {
        let now = unix_ms()?;
        let changed = self.connection.execute(
            "UPDATE extraction_jobs SET status = 'queued', cancel_requested = 0, error_code = NULL, \
                runner_token = NULL, lease_expires_at_unix_ms = NULL, updated_at_unix_ms = ?2 \
             WHERE id = ?1 AND status = 'interrupted'",
            params![job_id, now],
        )?;
        if changed != 1 {
            self.extraction_job(job_id)?;
            return Err(DatabaseError::InvalidExtractionJobState);
        }
        self.extraction_job(job_id)
    }

    pub fn claim_extraction_job(
        &mut self,
        job_id: i64,
        runner_token: &str,
        lease_ms: i64,
    ) -> Result<ExtractionJobProgress, DatabaseError> {
        let now = unix_ms()?;
        self.recover_expired_extraction_jobs_at(now)?;
        let lease_expires = now
            .checked_add(lease_ms)
            .ok_or(DatabaseError::InvalidTimestamp)?;
        let transaction = self.connection.transaction()?;
        let (status, existing_runner): (String, Option<String>) = transaction
            .query_row(
                "SELECT status, runner_token FROM extraction_jobs WHERE id = ?1",
                [job_id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()?
            .ok_or(DatabaseError::ExtractionJobNotFound)?;
        match status.as_str() {
            "queued" => {}
            "running" if existing_runner.as_deref() == Some(runner_token) => {}
            "running" => return Err(DatabaseError::ExtractionJobBusy),
            _ => return Err(DatabaseError::InvalidExtractionJobState),
        }
        transaction.execute(
            "UPDATE extraction_jobs SET status = 'running', runner_token = ?2, \
                lease_expires_at_unix_ms = ?3, started_at_unix_ms = COALESCE(started_at_unix_ms, ?4), \
                updated_at_unix_ms = ?4 WHERE id = ?1",
            params![job_id, runner_token, lease_expires, now],
        )?;
        transaction.commit()?;
        self.extraction_job(job_id)
    }

    pub fn extraction_cancel_requested(&self, job_id: i64) -> Result<bool, DatabaseError> {
        let (status, requested): (String, i64) = self
            .connection
            .query_row(
                "SELECT status, cancel_requested FROM extraction_jobs WHERE id = ?1",
                [job_id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()?
            .ok_or(DatabaseError::ExtractionJobNotFound)?;
        Ok(status != "running" || requested != 0)
    }

    pub fn requeue_extraction_job_after_capacity_refusal(
        &mut self,
        job_id: i64,
        runner_token: &str,
    ) -> Result<ExtractionJobProgress, DatabaseError> {
        let now = unix_ms()?;
        let changed = self.connection.execute(
            "UPDATE extraction_jobs SET \
                status = CASE WHEN cancel_requested != 0 THEN 'cancelled' ELSE 'queued' END, \
                runner_token = NULL, lease_expires_at_unix_ms = NULL, \
                finished_at_unix_ms = CASE WHEN cancel_requested != 0 THEN ?4 ELSE NULL END, \
                updated_at_unix_ms = ?4 \
             WHERE id = ?1 AND runner_token = ?2 AND status = 'running' \
                AND lease_expires_at_unix_ms IS NOT NULL AND lease_expires_at_unix_ms > ?3",
            params![job_id, runner_token, now, now],
        )?;
        if changed != 1 {
            return Err(DatabaseError::ExtractionRunnerLeaseLost);
        }
        self.extraction_job(job_id)
    }

    #[allow(clippy::too_many_arguments)]
    pub fn complete_extraction_job(
        &mut self,
        job_id: i64,
        runner_token: &str,
        provider_id: &str,
        provider_version: &str,
        source_size_bytes: u64,
        source_modified_unix_ns: Option<i64>,
        output_bytes: u64,
        elapsed_ms: u64,
        chunks: &[ContentChunkWrite],
    ) -> Result<ExtractionJobProgress, DatabaseError> {
        self.complete_extraction_job_with_image_metadata(
            job_id,
            runner_token,
            provider_id,
            provider_version,
            source_size_bytes,
            source_modified_unix_ns,
            output_bytes,
            elapsed_ms,
            chunks,
            None,
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub fn complete_extraction_job_with_image_metadata(
        &mut self,
        job_id: i64,
        runner_token: &str,
        provider_id: &str,
        provider_version: &str,
        source_size_bytes: u64,
        source_modified_unix_ns: Option<i64>,
        output_bytes: u64,
        elapsed_ms: u64,
        chunks: &[ContentChunkWrite],
        image_metadata: Option<&ImageMetadataWrite>,
    ) -> Result<ExtractionJobProgress, DatabaseError> {
        if provider_id.is_empty()
            || provider_version.is_empty()
            || source_size_bytes > MAX_EXTRACTION_SOURCE_BYTES
            || chunks.len() > MAX_EXTRACTION_CHUNKS
            || chunks
                .iter()
                .any(|chunk| chunk.text.len() > MAX_EXTRACTION_CHUNK_BYTES)
        {
            return Err(DatabaseError::ExtractionOutputInvalid);
        }
        let mut computed_output_bytes = 0_u64;
        for chunk in chunks {
            let chunk_bytes = u64::try_from(chunk.text.len())
                .map_err(|_| DatabaseError::ExtractionOutputInvalid)?;
            computed_output_bytes = computed_output_bytes
                .checked_add(chunk_bytes)
                .ok_or(DatabaseError::ExtractionOutputInvalid)?;
        }
        if output_bytes > MAX_EXTRACTION_OUTPUT_BYTES || computed_output_bytes != output_bytes {
            return Err(DatabaseError::ExtractionOutputInvalid);
        }
        let now = unix_ms()?;
        let transaction = self.connection.transaction()?;
        ensure_extraction_runner(&transaction, job_id, runner_token, now)?;
        let (
            scope_id,
            node_id,
            location_id,
            stored_size,
            stored_modified,
            cancel_requested,
            stored_operation,
        ): (i64, i64, i64, i64, Option<i64>, i64, String) = transaction.query_row(
            "SELECT scope_id, node_id, location_id, source_size_bytes, source_modified_unix_ns, \
                cancel_requested, operation FROM extraction_jobs WHERE id = ?1",
            [job_id],
            |row| {
                Ok((
                    row.get(0)?,
                    row.get(1)?,
                    row.get(2)?,
                    row.get(3)?,
                    row.get(4)?,
                    row.get(5)?,
                    row.get(6)?,
                ))
            },
        )?;
        let operation = ExtractionOperation::from_storage(&stored_operation)
            .ok_or(DatabaseError::InvalidStoredValue)?;
        let is_image_metadata_provider = provider_id == "deskgraph.image-metadata";
        let contains_ocr = chunks.iter().any(|chunk| {
            matches!(
                &chunk.provenance,
                ContentChunkProvenanceWrite::OcrObservation { .. }
            )
        });
        let invalid_operation_output = match operation {
            ExtractionOperation::Content => {
                contains_ocr
                    || is_image_metadata_provider != image_metadata.is_some()
                    || image_metadata.is_some_and(|metadata| {
                        !chunks.is_empty()
                            || output_bytes != 0
                            || !is_valid_image_dimensions(
                                metadata.pixel_width,
                                metadata.pixel_height,
                            )
                    })
            }
            ExtractionOperation::ScreenshotOcr => {
                image_metadata.is_some()
                    || is_image_metadata_provider
                    || chunks.iter().any(|chunk| {
                        !matches!(
                            &chunk.provenance,
                            ContentChunkProvenanceWrite::OcrObservation { .. }
                        )
                    })
            }
        };
        if invalid_operation_output {
            return Err(DatabaseError::ExtractionOutputInvalid);
        }
        if cancel_requested != 0
            || u64::try_from(stored_size).map_err(|_| DatabaseError::InvalidStoredValue)?
                != source_size_bytes
            || stored_modified != source_modified_unix_ns
        {
            return Err(DatabaseError::ExtractionOutputInvalid);
        }
        let current_source: Option<(i64, Option<i64>)> = transaction
            .query_row(
                "SELECT f.size_bytes, f.modified_unix_ns \
                 FROM locations l JOIN files f ON f.node_id = l.node_id \
                 WHERE l.id = ?1 AND l.node_id = ?2 AND l.present = 1",
                params![location_id, node_id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()?;
        let Some((current_size, current_modified)) = current_source else {
            return Err(DatabaseError::ExtractionOutputInvalid);
        };
        if u64::try_from(current_size).map_err(|_| DatabaseError::InvalidStoredValue)?
            != source_size_bytes
            || current_modified != source_modified_unix_ns
        {
            return Err(DatabaseError::ExtractionOutputInvalid);
        }
        for (index, chunk) in chunks.iter().enumerate() {
            let invalid_provenance = match &chunk.provenance {
                ContentChunkProvenanceWrite::ByteRange { start, end } => {
                    start > end || *end > source_size_bytes
                }
                ContentChunkProvenanceWrite::PdfPage { page_number, .. } => *page_number == 0,
                ContentChunkProvenanceWrite::DocxParagraph {
                    paragraph_number, ..
                } => *paragraph_number == 0,
                ContentChunkProvenanceWrite::PptxSlide { slide_number, .. } => *slide_number == 0,
                ContentChunkProvenanceWrite::XlsxCell {
                    sheet_number,
                    cell_reference,
                    ..
                } => *sheet_number == 0 || !is_valid_xlsx_cell_reference(cell_reference),
                ContentChunkProvenanceWrite::OcrObservation {
                    observation_number,
                    bbox_x_ppm,
                    bbox_y_ppm,
                    bbox_width_ppm,
                    bbox_height_ppm,
                    confidence_basis_points,
                    ..
                } => {
                    *observation_number == 0
                        || *bbox_width_ppm == 0
                        || *bbox_height_ppm == 0
                        || bbox_x_ppm
                            .checked_add(*bbox_width_ppm)
                            .is_none_or(|right| right > 1_000_000)
                        || bbox_y_ppm
                            .checked_add(*bbox_height_ppm)
                            .is_none_or(|top| top > 1_000_000)
                        || confidence_basis_points.is_some_and(|value| value > 10_000)
                }
            };
            if usize::try_from(chunk.ordinal).map_err(|_| DatabaseError::ExtractionOutputInvalid)?
                != index
                || chunk.trust_class != "untrusted_extracted_text"
                || invalid_provenance
            {
                return Err(DatabaseError::ExtractionOutputInvalid);
            }
        }

        match operation {
            ExtractionOperation::Content => {
                transaction.execute(
                    "UPDATE content_chunks SET active = 0 \
                     WHERE scope_id = ?1 AND node_id = ?2 AND active = 1 \
                        AND provenance_kind <> 'ocr_observation'",
                    params![scope_id, node_id],
                )?;
                transaction.execute(
                    "UPDATE image_metadata SET active = 0 \
                     WHERE scope_id = ?1 AND node_id = ?2 AND active = 1",
                    params![scope_id, node_id],
                )?;
            }
            ExtractionOperation::ScreenshotOcr => {
                transaction.execute(
                    "UPDATE content_chunks SET active = 0 \
                     WHERE scope_id = ?1 AND node_id = ?2 AND active = 1 \
                        AND provenance_kind = 'ocr_observation'",
                    params![scope_id, node_id],
                )?;
            }
        }
        for chunk in chunks {
            let (
                provenance_kind,
                source_byte_start,
                source_byte_end,
                source_page_number,
                source_unit_number,
                source_cell_reference,
                source_fragment_index,
                source_bbox_x_ppm,
                source_bbox_y_ppm,
                source_bbox_width_ppm,
                source_bbox_height_ppm,
                source_confidence_basis_points,
            ) = match &chunk.provenance {
                ContentChunkProvenanceWrite::ByteRange { start, end } => (
                    "byte_range",
                    Some(to_i64(*start)?),
                    Some(to_i64(*end)?),
                    None,
                    None,
                    None,
                    None,
                    None,
                    None,
                    None,
                    None,
                    None,
                ),
                ContentChunkProvenanceWrite::PdfPage {
                    page_number,
                    fragment_index,
                } => (
                    "pdf_page",
                    None,
                    None,
                    Some(i64::from(*page_number)),
                    None,
                    None,
                    Some(i64::from(*fragment_index)),
                    None,
                    None,
                    None,
                    None,
                    None,
                ),
                ContentChunkProvenanceWrite::DocxParagraph {
                    paragraph_number,
                    fragment_index,
                } => (
                    "docx_paragraph",
                    None,
                    None,
                    None,
                    Some(i64::from(*paragraph_number)),
                    None,
                    Some(i64::from(*fragment_index)),
                    None,
                    None,
                    None,
                    None,
                    None,
                ),
                ContentChunkProvenanceWrite::PptxSlide {
                    slide_number,
                    fragment_index,
                } => (
                    "pptx_slide",
                    None,
                    None,
                    None,
                    Some(i64::from(*slide_number)),
                    None,
                    Some(i64::from(*fragment_index)),
                    None,
                    None,
                    None,
                    None,
                    None,
                ),
                ContentChunkProvenanceWrite::XlsxCell {
                    sheet_number,
                    cell_reference,
                    fragment_index,
                } => (
                    "xlsx_cell",
                    None,
                    None,
                    None,
                    Some(i64::from(*sheet_number)),
                    Some(cell_reference.as_str()),
                    Some(i64::from(*fragment_index)),
                    None,
                    None,
                    None,
                    None,
                    None,
                ),
                ContentChunkProvenanceWrite::OcrObservation {
                    observation_number,
                    fragment_index,
                    bbox_x_ppm,
                    bbox_y_ppm,
                    bbox_width_ppm,
                    bbox_height_ppm,
                    confidence_basis_points,
                } => (
                    "ocr_observation",
                    None,
                    None,
                    None,
                    Some(i64::from(*observation_number)),
                    None,
                    Some(i64::from(*fragment_index)),
                    Some(i64::from(*bbox_x_ppm)),
                    Some(i64::from(*bbox_y_ppm)),
                    Some(i64::from(*bbox_width_ppm)),
                    Some(i64::from(*bbox_height_ppm)),
                    confidence_basis_points.map(i64::from),
                ),
            };
            transaction.execute(
                "INSERT INTO content_chunks( \
                    scope_id, node_id, location_id, extraction_job_id, ordinal, text, \
                    provenance_kind, source_byte_start, source_byte_end, source_page_number, \
                    source_unit_number, source_cell_reference, source_fragment_index, \
                    source_bbox_x_ppm, source_bbox_y_ppm, source_bbox_width_ppm, \
                    source_bbox_height_ppm, source_confidence_basis_points, source_size_bytes, \
                    source_modified_unix_ns, trust_class, provider_id, provider_version, active, \
                    created_at_unix_ms \
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20, ?21, ?22, ?23, 1, ?24)",
                params![
                    scope_id,
                    node_id,
                    location_id,
                    job_id,
                    i64::from(chunk.ordinal),
                    chunk.text,
                    provenance_kind,
                    source_byte_start,
                    source_byte_end,
                    source_page_number,
                    source_unit_number,
                    source_cell_reference,
                    source_fragment_index,
                    source_bbox_x_ppm,
                    source_bbox_y_ppm,
                    source_bbox_width_ppm,
                    source_bbox_height_ppm,
                    source_confidence_basis_points,
                    to_i64(source_size_bytes)?,
                    source_modified_unix_ns,
                    chunk.trust_class,
                    provider_id,
                    provider_version,
                    now,
                ],
            )?;
        }
        if let Some(metadata) = image_metadata {
            transaction.execute(
                "INSERT INTO image_metadata( \
                    scope_id, node_id, location_id, extraction_job_id, format, pixel_width, \
                    pixel_height, source_size_bytes, source_modified_unix_ns, provider_id, \
                    provider_version, active, created_at_unix_ms \
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, 1, ?12)",
                params![
                    scope_id,
                    node_id,
                    location_id,
                    job_id,
                    metadata.format.as_str(),
                    i64::from(metadata.pixel_width),
                    i64::from(metadata.pixel_height),
                    to_i64(source_size_bytes)?,
                    source_modified_unix_ns,
                    provider_id,
                    provider_version,
                    now,
                ],
            )?;
        }
        let completed = transaction.execute(
            "UPDATE extraction_jobs SET status = 'completed', provider_id = ?3, provider_version = ?4, \
                error_code = NULL, output_bytes = ?5, chunk_count = ?6, elapsed_ms = ?7, \
                runner_token = NULL, lease_expires_at_unix_ms = NULL, finished_at_unix_ms = ?8, \
                updated_at_unix_ms = ?8 WHERE id = ?1 AND runner_token = ?2 \
                AND status = 'running' AND cancel_requested = 0",
            params![
                job_id,
                runner_token,
                provider_id,
                provider_version,
                to_i64(output_bytes)?,
                to_i64(chunks.len() as u64)?,
                to_i64(elapsed_ms)?,
                now,
            ],
        )?;
        if completed != 1 {
            return Err(DatabaseError::ExtractionRunnerLeaseLost);
        }
        transaction.commit()?;
        self.extraction_job(job_id)
    }

    pub fn fail_extraction_job(
        &mut self,
        job_id: i64,
        runner_token: &str,
        provider_id: &str,
        provider_version: &str,
        error_code: &str,
        elapsed_ms: u64,
    ) -> Result<ExtractionJobProgress, DatabaseError> {
        let now = unix_ms()?;
        let changed = self.connection.execute(
            "UPDATE extraction_jobs SET \
                status = CASE WHEN cancel_requested != 0 THEN 'cancelled' ELSE 'failed' END, \
                provider_id = ?3, provider_version = ?4, \
                error_code = CASE WHEN cancel_requested != 0 THEN NULL ELSE ?5 END, \
                elapsed_ms = ?6, runner_token = NULL, lease_expires_at_unix_ms = NULL, \
                finished_at_unix_ms = ?7, updated_at_unix_ms = ?7 \
             WHERE id = ?1 AND runner_token = ?2 AND status = 'running' \
                AND lease_expires_at_unix_ms IS NOT NULL AND lease_expires_at_unix_ms > ?7",
            params![
                job_id,
                runner_token,
                provider_id,
                provider_version,
                error_code,
                to_i64(elapsed_ms)?,
                now,
            ],
        )?;
        if changed != 1 {
            return Err(DatabaseError::ExtractionRunnerLeaseLost);
        }
        self.extraction_job(job_id)
    }

    pub fn cancel_extraction_job_from_runner(
        &mut self,
        job_id: i64,
        runner_token: &str,
        provider_id: &str,
        provider_version: &str,
        elapsed_ms: u64,
    ) -> Result<ExtractionJobProgress, DatabaseError> {
        let now = unix_ms()?;
        let changed = self.connection.execute(
            "UPDATE extraction_jobs SET status = 'cancelled', cancel_requested = 1, \
                provider_id = ?3, provider_version = ?4, error_code = NULL, elapsed_ms = ?5, \
                runner_token = NULL, lease_expires_at_unix_ms = NULL, finished_at_unix_ms = ?6, \
                updated_at_unix_ms = ?6 \
             WHERE id = ?1 AND runner_token = ?2 AND status = 'running' \
                AND lease_expires_at_unix_ms IS NOT NULL AND lease_expires_at_unix_ms > ?6",
            params![
                job_id,
                runner_token,
                provider_id,
                provider_version,
                to_i64(elapsed_ms)?,
                now,
            ],
        )?;
        if changed != 1 {
            return Err(DatabaseError::ExtractionRunnerLeaseLost);
        }
        self.extraction_job(job_id)
    }

    pub fn recover_expired_extraction_jobs_at(&mut self, now: i64) -> Result<u64, DatabaseError> {
        let recovered = self.connection.execute(
            "UPDATE extraction_jobs SET status = 'interrupted', runner_token = NULL, \
                lease_expires_at_unix_ms = NULL, updated_at_unix_ms = ?1 \
             WHERE status = 'running' AND lease_expires_at_unix_ms IS NOT NULL \
                AND lease_expires_at_unix_ms <= ?1",
            [now],
        )?;
        u64::try_from(recovered).map_err(|_| DatabaseError::InvalidCount)
    }

    pub fn extraction_stats(&self) -> Result<ExtractionStats, DatabaseError> {
        Ok(ExtractionStats {
            api_version: ExtractionStats::API_VERSION,
            active_chunk_count: count(
                &self.connection,
                "SELECT COUNT(*) FROM content_chunks WHERE active = 1",
            )?,
            extracted_file_count: count(
                &self.connection,
                "SELECT COUNT(*) FROM ( \
                    SELECT node_id FROM content_chunks WHERE active = 1 \
                    UNION SELECT node_id FROM image_metadata WHERE active = 1 \
                 )",
            )?,
            completed_job_count: count(
                &self.connection,
                "SELECT COUNT(*) FROM extraction_jobs WHERE status = 'completed'",
            )?,
            failed_job_count: count(
                &self.connection,
                "SELECT COUNT(*) FROM extraction_jobs WHERE status = 'failed'",
            )?,
            cancelled_job_count: count(
                &self.connection,
                "SELECT COUNT(*) FROM extraction_jobs WHERE status = 'cancelled'",
            )?,
        })
    }

    pub fn lexical_search_candidates(
        &self,
        match_query: &str,
        filters: LexicalSearchFilters<'_>,
        per_source_candidate_limit: u32,
    ) -> Result<Vec<LexicalSearchCandidate>, DatabaseError> {
        if match_query.is_empty()
            || match_query.len() > MAX_SEARCH_MATCH_BYTES
            || per_source_candidate_limit == 0
            || per_source_candidate_limit > MAX_SEARCH_CANDIDATES_PER_SOURCE
            || filters.scope_id.is_some_and(|scope_id| scope_id <= 0)
            || filters.extension.is_some_and(|extension| {
                extension.is_empty()
                    || extension.len() > 16
                    || !extension
                        .bytes()
                        .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit())
            })
            || filters
                .modified_since_unix_ns
                .is_some_and(|timestamp| timestamp < 0)
            || filters
                .modified_before_unix_ns
                .is_some_and(|timestamp| timestamp < 0)
            || matches!(
                (
                    filters.modified_since_unix_ns,
                    filters.modified_before_unix_ns
                ),
                (Some(since), Some(before)) if since >= before
            )
        {
            return Err(DatabaseError::SearchInputInvalid);
        }
        let limit = i64::from(per_source_candidate_limit);
        let maximum_sources = if filters.source == LexicalSearchSource::All {
            2
        } else {
            1
        };
        let mut candidates = Vec::with_capacity(
            usize::try_from(per_source_candidate_limit)
                .map_err(|_| DatabaseError::SearchInputInvalid)?
                .saturating_mul(maximum_sources),
        );

        if filters.source != LexicalSearchSource::ExtractedText {
            let mut metadata_statement = self.connection.prepare(
                "SELECT l.scope_id, l.node_id, l.id, l.display_path \
                 FROM location_search_fts \
                 JOIN locations l ON l.id = location_search_fts.rowid \
                 LEFT JOIN files f ON f.node_id = l.node_id \
                 WHERE location_search_fts MATCH ?1 AND l.present = 1 \
                   AND (?2 IS NULL OR l.scope_id = ?2) \
                   AND (?3 IS NULL OR (f.node_id IS NOT NULL AND substr(lower(l.display_path), -(length(?3) + 1)) = '.' || ?3)) \
                   AND (?4 IS NULL OR f.modified_unix_ns >= ?4) \
                   AND (?5 IS NULL OR f.modified_unix_ns < ?5) \
                 ORDER BY location_search_fts.rank, l.id \
                 LIMIT ?6",
            )?;
            let metadata_rows = metadata_statement.query_map(
                params![
                    match_query,
                    filters.scope_id,
                    filters.extension,
                    filters.modified_since_unix_ns,
                    filters.modified_before_unix_ns,
                    limit
                ],
                |row| {
                    Ok(LexicalSearchCandidate {
                        source: LexicalCandidateSource::MetadataPath,
                        scope_id: row.get(0)?,
                        node_id: row.get(1)?,
                        location_id: row.get(2)?,
                        display_path: row.get(3)?,
                        snippet: None,
                    })
                },
            )?;
            for row in metadata_rows {
                candidates.push(row?);
            }
        }

        if filters.source != LexicalSearchSource::MetadataPath {
            let mut content_statement = self.connection.prepare(
                "SELECT c.scope_id, c.node_id, c.location_id, l.display_path, \
                        snippet(content_search_fts, 0, '[', ']', '…', 24) \
                 FROM content_search_fts \
                 JOIN content_chunks c ON c.id = content_search_fts.rowid \
                 JOIN locations l ON l.id = c.location_id AND l.node_id = c.node_id \
                 JOIN files f ON f.node_id = c.node_id \
                 WHERE content_search_fts MATCH ?1 AND c.active = 1 AND l.present = 1 \
                   AND (?2 IS NULL OR c.scope_id = ?2) \
                   AND (?3 IS NULL OR substr(lower(l.display_path), -(length(?3) + 1)) = '.' || ?3) \
                   AND (?4 IS NULL OR f.modified_unix_ns >= ?4) \
                   AND (?5 IS NULL OR f.modified_unix_ns < ?5) \
                 ORDER BY content_search_fts.rank, c.node_id, c.ordinal \
                 LIMIT ?6",
            )?;
            let content_rows = content_statement.query_map(
                params![
                    match_query,
                    filters.scope_id,
                    filters.extension,
                    filters.modified_since_unix_ns,
                    filters.modified_before_unix_ns,
                    limit
                ],
                |row| {
                    Ok(LexicalSearchCandidate {
                        source: LexicalCandidateSource::ExtractedText,
                        scope_id: row.get(0)?,
                        node_id: row.get(1)?,
                        location_id: row.get(2)?,
                        display_path: row.get(3)?,
                        snippet: Some(row.get(4)?),
                    })
                },
            )?;
            for row in content_rows {
                candidates.push(row?);
            }
        }

        Ok(candidates)
    }

    pub fn invalidate_content_for_node(
        &self,
        scope_id: i64,
        node_id: i64,
    ) -> Result<u64, DatabaseError> {
        let chunks = self.connection.execute(
            "UPDATE content_chunks SET active = 0 \
             WHERE scope_id = ?1 AND node_id = ?2 AND active = 1",
            params![scope_id, node_id],
        )?;
        let metadata = self.connection.execute(
            "UPDATE image_metadata SET active = 0 \
             WHERE scope_id = ?1 AND node_id = ?2 AND active = 1",
            params![scope_id, node_id],
        )?;
        u64::try_from(chunks)
            .ok()
            .and_then(|chunks| {
                u64::try_from(metadata)
                    .ok()
                    .and_then(|metadata| chunks.checked_add(metadata))
            })
            .ok_or(DatabaseError::InvalidCount)
    }

    pub fn stats(&self) -> Result<ManifestStats, DatabaseError> {
        Ok(ManifestStats {
            api_version: ManifestStats::API_VERSION,
            database_ready: true,
            authorized_scope_count: count(
                &self.connection,
                "SELECT COUNT(*) FROM authorized_scopes",
            )?,
            node_count: count(
                &self.connection,
                "SELECT COUNT(DISTINCT node_id) FROM locations WHERE present = 1",
            )?,
            file_count: count(
                &self.connection,
                "SELECT COUNT(DISTINCT files.node_id) FROM files JOIN locations ON locations.node_id = files.node_id WHERE locations.present = 1",
            )?,
            folder_count: count(
                &self.connection,
                "SELECT COUNT(DISTINCT folders.node_id) FROM folders JOIN locations ON locations.node_id = folders.node_id WHERE locations.present = 1",
            )?,
            active_location_count: count(
                &self.connection,
                "SELECT COUNT(*) FROM locations WHERE present = 1",
            )?,
            issue_count: count(
                &self.connection,
                "SELECT COALESCE(issue_count, 0) FROM scan_jobs WHERE status = 'completed' ORDER BY id DESC LIMIT 1",
            )?,
            completed_scan_count: count(
                &self.connection,
                "SELECT COUNT(*) FROM scan_jobs WHERE status = 'completed'",
            )?,
        })
    }

    pub fn folder_profile_facts(
        &self,
        scope_id: i64,
        folder_node_id: i64,
        entry_limit: u64,
    ) -> Result<FolderProfileFacts, DatabaseError> {
        if scope_id <= 0
            || folder_node_id <= 0
            || entry_limit == 0
            || entry_limit > MAX_FOLDER_PROFILE_ENTRIES
        {
            return Err(DatabaseError::FolderProfileInputInvalid);
        }
        let folder = self
            .connection
            .query_row(
                "SELECT l.id, l.path_key, l.display_path \
                 FROM locations l \
                 JOIN nodes n ON n.id = l.node_id AND n.kind = 'folder' \
                 WHERE l.scope_id = ?1 AND l.node_id = ?2 AND l.present = 1 \
                 ORDER BY l.id LIMIT 1",
                params![scope_id, folder_node_id],
                |row| {
                    Ok((
                        row.get::<_, i64>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, String>(2)?,
                    ))
                },
            )
            .optional()?
            .ok_or(DatabaseError::FolderNotFound)?;
        let prefix_chars = i64::try_from(folder.1.chars().count())
            .map_err(|_| DatabaseError::FolderProfileInputInvalid)?;
        let remainder_start = prefix_chars
            .checked_add(2)
            .ok_or(DatabaseError::FolderProfileInputInvalid)?;
        let query_limit = to_i64(
            entry_limit
                .checked_add(1)
                .ok_or(DatabaseError::FolderProfileInputInvalid)?,
        )?;
        let separator = MAIN_SEPARATOR.to_string();
        let mut statement = self.connection.prepare(
            "SELECT n.kind, COALESCE(f.size_bytes, 0), f.modified_unix_ns, l.display_path, \
                    CASE WHEN instr(substr(l.path_key, ?4), ?5) = 0 THEN 1 ELSE 0 END \
             FROM locations l \
             JOIN nodes n ON n.id = l.node_id \
             LEFT JOIN files f ON f.node_id = l.node_id \
             WHERE l.scope_id = ?1 AND l.present = 1 \
               AND length(l.path_key) > ?2 \
               AND substr(l.path_key, 1, ?2) = ?3 \
               AND substr(l.path_key, ?2 + 1, 1) = ?5 \
             ORDER BY l.id \
             LIMIT ?6",
        )?;
        let mut rows = statement.query(params![
            scope_id,
            prefix_chars,
            folder.1,
            remainder_start,
            separator,
            query_limit,
        ])?;
        let mut processed = 0_u64;
        let mut direct_file_count = 0_u64;
        let mut direct_folder_count = 0_u64;
        let mut descendant_file_count = 0_u64;
        let mut descendant_folder_count = 0_u64;
        let mut total_file_bytes = 0_u64;
        let mut latest_modified_unix_ns = None;
        let mut category_counts = [0_u64; 7];
        let mut project_markers = std::collections::BTreeSet::new();
        while let Some(row) = rows.next()? {
            processed = processed
                .checked_add(1)
                .ok_or(DatabaseError::InvalidCount)?;
            if processed > entry_limit {
                return Err(DatabaseError::FolderProfileTooLarge);
            }
            let kind = NodeKind::from_db(&row.get::<_, String>(0)?)?;
            let size_bytes = row_u64(row, 1)?;
            let modified_unix_ns: Option<i64> = row.get(2)?;
            let display_path: String = row.get(3)?;
            let is_direct = row.get::<_, i64>(4)? != 0;
            match kind {
                NodeKind::File => {
                    descendant_file_count = descendant_file_count
                        .checked_add(1)
                        .ok_or(DatabaseError::InvalidCount)?;
                    if is_direct {
                        direct_file_count = direct_file_count
                            .checked_add(1)
                            .ok_or(DatabaseError::InvalidCount)?;
                    }
                    total_file_bytes = total_file_bytes
                        .checked_add(size_bytes)
                        .ok_or(DatabaseError::InvalidCount)?;
                    if let Some(modified) = modified_unix_ns {
                        latest_modified_unix_ns = Some(
                            latest_modified_unix_ns
                                .map_or(modified, |current: i64| current.max(modified)),
                        );
                    }
                    let category = file_category(Path::new(&display_path));
                    let category_index = folder_category_index(category);
                    category_counts[category_index] = category_counts[category_index]
                        .checked_add(1)
                        .ok_or(DatabaseError::InvalidCount)?;
                    if is_direct
                        && let Some(marker) = project_marker(Path::new(&display_path), kind)
                    {
                        project_markers.insert(marker);
                    }
                }
                NodeKind::Folder => {
                    descendant_folder_count = descendant_folder_count
                        .checked_add(1)
                        .ok_or(DatabaseError::InvalidCount)?;
                    if is_direct {
                        direct_folder_count = direct_folder_count
                            .checked_add(1)
                            .ok_or(DatabaseError::InvalidCount)?;
                        if let Some(marker) = project_marker(Path::new(&display_path), kind) {
                            project_markers.insert(marker);
                        }
                    }
                }
            }
        }
        let observed_at_unix_ms: i64 = self.connection.query_row(
            "SELECT COALESCE(MAX(COALESCE(finished_at_unix_ms, started_at_unix_ms)), 0) \
             FROM scan_jobs WHERE scope_id = ?1 AND status = 'completed'",
            [scope_id],
            |row| row.get(0),
        )?;
        let file_categories = FolderFileCategory::ALL
            .into_iter()
            .enumerate()
            .filter_map(|(index, category)| {
                let file_count = category_counts[index];
                (file_count > 0).then_some(FolderCategoryCount {
                    category,
                    file_count,
                })
            })
            .collect();
        Ok(FolderProfileFacts {
            scope_id,
            folder_node_id,
            folder_location_id: folder.0,
            display_path: folder.2,
            direct_file_count,
            direct_folder_count,
            descendant_file_count,
            descendant_folder_count,
            total_file_bytes,
            latest_modified_unix_ns,
            file_categories,
            project_markers: project_markers.into_iter().collect(),
            observed_at_unix_ms,
            bounded_entry_limit: entry_limit,
        })
    }

    pub fn record_project_candidate(
        &mut self,
        scope_id: i64,
        root_folder_node_id: i64,
        suggestion: &ProjectSuggestion,
    ) -> Result<ProjectCandidate, DatabaseError> {
        validate_project_suggestion(scope_id, root_folder_node_id, suggestion)?;
        let current_facts =
            self.folder_profile_facts(scope_id, root_folder_node_id, MAX_FOLDER_PROFILE_ENTRIES)?;
        let suggested_kinds = suggestion
            .provenance
            .iter()
            .map(|signal| signal.kind)
            .collect::<Vec<_>>();
        if current_facts.observed_at_unix_ms != suggestion.observed_at_unix_ms
            || current_facts.project_markers != suggested_kinds
        {
            return Err(DatabaseError::ProjectCandidateInputInvalid);
        }
        let now = unix_ms()?;
        let transaction = self.connection.transaction()?;
        let root_is_current = transaction.query_row(
            "SELECT EXISTS( \
                 SELECT 1 FROM locations l \
                 JOIN nodes n ON n.id = l.node_id AND n.kind = 'folder' \
                 WHERE l.scope_id = ?1 AND l.node_id = ?2 AND l.present = 1 \
             )",
            params![scope_id, root_folder_node_id],
            |row| row.get::<_, i64>(0),
        )? != 0;
        if !root_is_current {
            return Err(DatabaseError::ProjectCandidateRootNotCurrent);
        }
        transaction.execute(
            "INSERT OR IGNORE INTO projects( \
                 api_version, scope_id, root_folder_node_id, created_at_unix_ms \
             ) VALUES (?1, ?2, ?3, ?4)",
            params![
                ProjectCandidate::API_VERSION,
                scope_id,
                root_folder_node_id,
                now,
            ],
        )?;
        let project_id: i64 = transaction.query_row(
            "SELECT id FROM projects WHERE scope_id = ?1 AND root_folder_node_id = ?2",
            params![scope_id, root_folder_node_id],
            |row| row.get(0),
        )?;
        let suggestion_inserted = transaction.execute(
            "INSERT OR IGNORE INTO project_suggestions( \
                 project_id, confidence_basis_points, observed_at_unix_ms, provider_id, \
                 provider_version, model_version, created_at_unix_ms \
             ) VALUES (?1, ?2, ?3, ?4, ?5, NULL, ?6)",
            params![
                project_id,
                i64::from(suggestion.confidence_basis_points),
                suggestion.observed_at_unix_ms,
                suggestion.provider_id,
                suggestion.provider_version,
                now,
            ],
        )?;
        let suggestion_id: i64 = transaction.query_row(
            "SELECT id FROM project_suggestions \
             WHERE project_id = ?1 AND observed_at_unix_ms = ?2 \
               AND provider_id = ?3 AND provider_version = ?4",
            params![
                project_id,
                suggestion.observed_at_unix_ms,
                suggestion.provider_id,
                suggestion.provider_version,
            ],
            |row| row.get(0),
        )?;
        if suggestion_inserted == 1 {
            for (index, signal) in suggestion.provenance.iter().enumerate() {
                transaction.execute(
                    "INSERT INTO project_suggestion_signals( \
                         suggestion_id, ordinal, signal_kind, marker_name, weight_basis_points \
                     ) VALUES (?1, ?2, ?3, ?4, ?5)",
                    params![
                        suggestion_id,
                        to_i64(u64::try_from(index + 1).map_err(|_| DatabaseError::InvalidCount)?)?,
                        project_signal_kind_str(signal.kind),
                        signal.marker_name.as_str(),
                        i64::from(signal.weight_basis_points),
                    ],
                )?;
            }
        }
        transaction.commit()?;
        self.project_candidate(project_id)
    }

    pub fn project_candidate(&self, project_id: i64) -> Result<ProjectCandidate, DatabaseError> {
        if project_id <= 0 {
            return Err(DatabaseError::ProjectCandidateInputInvalid);
        }
        let row = self
            .connection
            .query_row(
                "SELECT p.id, p.scope_id, p.root_folder_node_id, l.id, l.display_path, \
                        s.id, s.confidence_basis_points, s.observed_at_unix_ms, \
                        s.provider_id, s.provider_version, s.model_version, \
                        f.sequence, f.decision, f.created_by, f.created_at_unix_ms \
                 FROM projects p \
                 JOIN nodes n ON n.id = p.root_folder_node_id AND n.kind = 'folder' \
                 JOIN locations l ON l.scope_id = p.scope_id \
                    AND l.node_id = p.root_folder_node_id AND l.present = 1 \
                 JOIN project_suggestions s ON s.id = ( \
                    SELECT latest.id FROM project_suggestions latest \
                    WHERE latest.project_id = p.id \
                    ORDER BY latest.observed_at_unix_ms DESC, latest.id DESC LIMIT 1 \
                 ) \
                 LEFT JOIN project_feedback_events f ON f.id = ( \
                    SELECT latest_feedback.id FROM project_feedback_events latest_feedback \
                    WHERE latest_feedback.project_id = p.id \
                    ORDER BY latest_feedback.sequence DESC LIMIT 1 \
                 ) \
                 WHERE p.id = ?1 \
                 ORDER BY l.id LIMIT 1",
                [project_id],
                |row| {
                    Ok((
                        row.get::<_, i64>(0)?,
                        row.get::<_, i64>(1)?,
                        row.get::<_, i64>(2)?,
                        row.get::<_, i64>(3)?,
                        row.get::<_, String>(4)?,
                        row.get::<_, i64>(5)?,
                        row.get::<_, i64>(6)?,
                        row.get::<_, i64>(7)?,
                        row.get::<_, String>(8)?,
                        row.get::<_, String>(9)?,
                        row.get::<_, Option<String>>(10)?,
                        row.get::<_, Option<i64>>(11)?,
                        row.get::<_, Option<String>>(12)?,
                        row.get::<_, Option<String>>(13)?,
                        row.get::<_, Option<i64>>(14)?,
                    ))
                },
            )
            .optional()?;
        let Some((
            project_id,
            scope_id,
            root_folder_node_id,
            root_folder_location_id,
            display_path,
            suggestion_id,
            confidence_basis_points,
            observed_at_unix_ms,
            provider_id,
            provider_version,
            model_version,
            decision_sequence,
            decision_kind,
            decision_creator,
            decided_at_unix_ms,
        )) = row
        else {
            let project_exists = self.connection.query_row(
                "SELECT EXISTS(SELECT 1 FROM projects WHERE id = ?1)",
                [project_id],
                |row| row.get::<_, i64>(0),
            )? != 0;
            return Err(if project_exists {
                DatabaseError::ProjectCandidateRootNotCurrent
            } else {
                DatabaseError::ProjectCandidateNotFound
            });
        };
        if provider_id != ProjectSuggestion::PROVIDER_ID
            || provider_version != ProjectSuggestion::PROVIDER_VERSION
            || model_version.is_some()
        {
            return Err(DatabaseError::InvalidStoredValue);
        }
        let mut statement = self.connection.prepare(
            "SELECT signal_kind, marker_name, weight_basis_points \
             FROM project_suggestion_signals \
             WHERE suggestion_id = ?1 ORDER BY ordinal",
        )?;
        let rows = statement.query_map([suggestion_id], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, i64>(2)?,
            ))
        })?;
        let mut provenance = Vec::new();
        for row in rows {
            let (kind, marker_name, weight_basis_points) = row?;
            provenance.push(ProjectSignal {
                kind: project_signal_kind_from_str(&kind)?,
                marker_name,
                weight_basis_points: u16::try_from(weight_basis_points)
                    .map_err(|_| DatabaseError::InvalidStoredValue)?,
            });
        }
        let suggestion = ProjectSuggestion {
            confidence_basis_points: u16::try_from(confidence_basis_points)
                .map_err(|_| DatabaseError::InvalidStoredValue)?,
            provenance,
            observed_at_unix_ms,
            created_by: ProjectSuggestionCreator::SystemRule,
            provider_id: ProjectSuggestion::PROVIDER_ID,
            provider_version: ProjectSuggestion::PROVIDER_VERSION,
            model_version: None,
        };
        let latest_decision = match (
            decision_sequence,
            decision_kind,
            decision_creator,
            decided_at_unix_ms,
        ) {
            (None, None, None, None) => None,
            (Some(sequence), Some(kind), Some(creator), Some(decided_at_unix_ms)) => {
                if creator != "user" {
                    return Err(DatabaseError::InvalidStoredValue);
                }
                Some(ProjectDecision {
                    sequence: u64::try_from(sequence)
                        .map_err(|_| DatabaseError::InvalidStoredValue)?,
                    kind: project_decision_kind_from_str(&kind)?,
                    created_by: ProjectDecisionCreator::User,
                    decided_at_unix_ms,
                })
            }
            _ => return Err(DatabaseError::InvalidStoredValue),
        };
        let state = match latest_decision.as_ref().map(|decision| decision.kind) {
            None => ProjectCandidateState::Suggested,
            Some(ProjectDecisionKind::Accepted) => ProjectCandidateState::Accepted,
            Some(ProjectDecisionKind::Rejected) => ProjectCandidateState::Rejected,
        };
        Ok(ProjectCandidate {
            api_version: ProjectCandidate::API_VERSION,
            project_id,
            scope_id,
            root_folder_node_id,
            root_folder_location_id,
            display_path,
            state,
            suggestion,
            latest_decision,
        })
    }

    pub fn decide_project_candidate(
        &mut self,
        project_id: i64,
        decision: ProjectDecisionKind,
    ) -> Result<ProjectCandidate, DatabaseError> {
        if project_id <= 0 {
            return Err(DatabaseError::ProjectCandidateInputInvalid);
        }
        let now = unix_ms()?;
        let transaction = self.connection.transaction()?;
        let project_exists = transaction.query_row(
            "SELECT EXISTS(SELECT 1 FROM projects WHERE id = ?1)",
            [project_id],
            |row| row.get::<_, i64>(0),
        )? != 0;
        if !project_exists {
            return Err(DatabaseError::ProjectCandidateNotFound);
        }
        let root_is_current = transaction.query_row(
            "SELECT EXISTS( \
                 SELECT 1 FROM projects p \
                 JOIN nodes n ON n.id = p.root_folder_node_id AND n.kind = 'folder' \
                 JOIN locations l ON l.scope_id = p.scope_id \
                    AND l.node_id = p.root_folder_node_id AND l.present = 1 \
                 WHERE p.id = ?1 \
             )",
            [project_id],
            |row| row.get::<_, i64>(0),
        )? != 0;
        if !root_is_current {
            return Err(DatabaseError::ProjectCandidateRootNotCurrent);
        }
        let latest = transaction
            .query_row(
                "SELECT sequence, decision FROM project_feedback_events \
                 WHERE project_id = ?1 ORDER BY sequence DESC LIMIT 1",
                [project_id],
                |row| Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?)),
            )
            .optional()?;
        if latest
            .as_ref()
            .is_some_and(|(_, stored)| stored == project_decision_kind_str(decision))
        {
            transaction.commit()?;
            return self.project_candidate(project_id);
        }
        let sequence = latest.map_or(Ok(1_i64), |(sequence, _)| {
            sequence.checked_add(1).ok_or(DatabaseError::InvalidCount)
        })?;
        transaction.execute(
            "INSERT INTO project_feedback_events( \
                 project_id, sequence, decision, created_by, created_at_unix_ms \
             ) VALUES (?1, ?2, ?3, 'user', ?4)",
            params![
                project_id,
                sequence,
                project_decision_kind_str(decision),
                now
            ],
        )?;
        transaction.commit()?;
        self.project_candidate(project_id)
    }

    pub fn recent_project_candidates(&self) -> Result<Vec<ProjectCandidateSummary>, DatabaseError> {
        let project_ids = {
            let mut statement = self.connection.prepare(
                "SELECT p.id FROM projects p \
                 WHERE EXISTS( \
                    SELECT 1 FROM locations l \
                    JOIN nodes n ON n.id = l.node_id AND n.kind = 'folder' \
                    WHERE l.scope_id = p.scope_id \
                      AND l.node_id = p.root_folder_node_id AND l.present = 1 \
                 ) \
                 ORDER BY p.id DESC LIMIT 20",
            )?;
            let rows = statement.query_map([], |row| row.get::<_, i64>(0))?;
            rows.collect::<Result<Vec<_>, _>>()?
        };
        project_ids
            .into_iter()
            .map(|project_id| {
                let candidate = self.project_candidate(project_id)?;
                Ok(ProjectCandidateSummary {
                    api_version: ProjectCandidateSummary::API_VERSION,
                    project_id: candidate.project_id,
                    scope_id: candidate.scope_id,
                    root_folder_node_id: candidate.root_folder_node_id,
                    state: candidate.state,
                    confidence_basis_points: candidate.suggestion.confidence_basis_points,
                    observed_at_unix_ms: candidate.suggestion.observed_at_unix_ms,
                    latest_decision_at_unix_ms: candidate
                        .latest_decision
                        .map(|decision| decision.decided_at_unix_ms),
                })
            })
            .collect()
    }

    pub fn record_exact_duplicate_candidate(
        &mut self,
        left: &ActionSourceRecord,
        right: &ActionSourceRecord,
    ) -> Result<FileRelationCandidate, DatabaseError> {
        validate_exact_duplicate_sources(left, right)?;
        let observed_at = unix_ms()?;
        let transaction = self.connection.transaction()?;
        if !relation_snapshot_matches(&transaction, left)?
            || !relation_snapshot_matches(&transaction, right)?
        {
            return Err(DatabaseError::FileRelationCandidateNotCurrent);
        }
        transaction.execute(
            "INSERT OR IGNORE INTO file_relation_candidates( \
                 api_version, relation_kind, scope_id, left_node_id, right_node_id, \
                 created_at_unix_ms \
             ) VALUES (?1, 'exact_duplicate', ?2, ?3, ?4, ?5)",
            params![
                FileRelationCandidate::API_VERSION,
                left.scope_id,
                left.node_id,
                right.node_id,
                observed_at,
            ],
        )?;
        let relation_id: i64 = transaction.query_row(
            "SELECT id FROM file_relation_candidates \
             WHERE relation_kind = 'exact_duplicate' AND scope_id = ?1 \
               AND left_node_id = ?2 AND right_node_id = ?3",
            params![left.scope_id, left.node_id, right.node_id],
            |row| row.get(0),
        )?;
        transaction.execute(
            "INSERT INTO file_relation_observations( \
                 relation_id, left_location_id, right_location_id, source_size_bytes, \
                 left_modified_unix_ns, right_modified_unix_ns, compared_bytes, \
                 confidence_basis_points, comparison_kind, created_by, provider_id, \
                 provider_version, model_version, observed_at_unix_ms \
             ) VALUES ( \
                 ?1, ?2, ?3, ?4, ?5, ?6, ?4, 10000, 'byte_for_byte', 'system_rule', \
                 'deskgraph.byte-equality', '1', NULL, ?7 \
             )",
            params![
                relation_id,
                left.location_id,
                right.location_id,
                to_i64(left.size_bytes)?,
                left.modified_unix_ns,
                right.modified_unix_ns,
                observed_at,
            ],
        )?;
        transaction.commit()?;
        self.file_relation_candidate(relation_id)
    }

    pub fn record_file_version_candidate(
        &mut self,
        first: &ActionSourceRecord,
        second: &ActionSourceRecord,
    ) -> Result<FileVersionCandidate, DatabaseError> {
        validate_file_relation_sources(first, second)?;
        let first_name = explicit_version_name_from_source(first)?;
        let second_name = explicit_version_name_from_source(second)?;
        if first_name.base_key != second_name.base_key
            || first_name.extension_key != second_name.extension_key
            || first_name.version == second_name.version
        {
            return Err(DatabaseError::FileRelationCandidateInputInvalid);
        }
        let (older, older_name, newer, newer_name) = if first_name.version < second_name.version {
            (first, first_name, second, second_name)
        } else {
            (second, second_name, first, first_name)
        };
        let (left, right) = if first.node_id < second.node_id {
            (first, second)
        } else {
            (second, first)
        };
        let observed_at = unix_ms()?;
        let transaction = self.connection.transaction()?;
        if !relation_snapshot_matches(&transaction, first)?
            || !relation_snapshot_matches(&transaction, second)?
        {
            return Err(DatabaseError::FileRelationCandidateNotCurrent);
        }
        transaction.execute(
            "INSERT OR IGNORE INTO file_relation_candidates( \
                 api_version, relation_kind, scope_id, left_node_id, right_node_id, \
                 created_at_unix_ms \
             ) VALUES (?1, 'version', ?2, ?3, ?4, ?5)",
            params![
                FileRelationCandidate::API_VERSION,
                left.scope_id,
                left.node_id,
                right.node_id,
                observed_at,
            ],
        )?;
        let relation_id: i64 = transaction.query_row(
            "SELECT id FROM file_relation_candidates \
             WHERE relation_kind = 'version' AND scope_id = ?1 \
               AND left_node_id = ?2 AND right_node_id = ?3",
            params![left.scope_id, left.node_id, right.node_id],
            |row| row.get(0),
        )?;
        transaction.execute(
            "INSERT INTO file_version_observations( \
                 relation_id, older_location_id, newer_location_id, older_size_bytes, \
                 newer_size_bytes, older_modified_unix_ns, newer_modified_unix_ns, \
                 base_key, extension_key, older_version, newer_version, \
                 confidence_basis_points, signal_kind, created_by, provider_id, \
                 provider_version, model_version, observed_at_unix_ms \
             ) VALUES ( \
                 ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, 9000, \
                 'explicit_numeric_suffix', 'system_rule', \
                 'deskgraph.filename-version', '1', NULL, ?12 \
             )",
            params![
                relation_id,
                older.location_id,
                newer.location_id,
                to_i64(older.size_bytes)?,
                to_i64(newer.size_bytes)?,
                older.modified_unix_ns,
                newer.modified_unix_ns,
                older_name.base_key,
                older_name.extension_key,
                i64::from(older_name.version),
                i64::from(newer_name.version),
                observed_at,
            ],
        )?;
        transaction.commit()?;
        self.file_version_candidate(relation_id)
    }

    fn file_version_candidate(
        &self,
        relation_id: i64,
    ) -> Result<FileVersionCandidate, DatabaseError> {
        Self::file_version_candidate_from_connection(&self.connection, relation_id)
    }

    fn file_version_candidate_from_connection(
        connection: &Connection,
        relation_id: i64,
    ) -> Result<FileVersionCandidate, DatabaseError> {
        if relation_id <= 0 {
            return Err(DatabaseError::FileRelationCandidateInputInvalid);
        }
        let row = connection
            .query_row(
                "SELECT r.id, r.relation_kind, r.scope_id, r.left_node_id, r.right_node_id, \
                        older_location.node_id, older_location.id, \
                        older_location.display_path, older_file.size_bytes, \
                        older_file.modified_unix_ns, newer_location.node_id, \
                        newer_location.id, newer_location.display_path, newer_file.size_bytes, \
                        newer_file.modified_unix_ns, observation.base_key, \
                        observation.extension_key, observation.older_version, \
                        observation.newer_version, observation.confidence_basis_points, \
                        observation.signal_kind, observation.created_by, \
                        observation.provider_id, observation.provider_version, \
                        observation.model_version, observation.observed_at_unix_ms, \
                        observation.id, feedback.id \
                 FROM file_relation_candidates r \
                 JOIN file_version_observations observation ON observation.id = ( \
                    SELECT latest.id FROM file_version_observations latest \
                    WHERE latest.relation_id = r.id \
                    ORDER BY latest.observed_at_unix_ms DESC, latest.id DESC LIMIT 1 \
                 ) \
                 JOIN locations older_location ON older_location.id = observation.older_location_id \
                    AND older_location.scope_id = r.scope_id AND older_location.present = 1 \
                 JOIN nodes older_node ON older_node.id = older_location.node_id \
                    AND older_node.kind = 'file' \
                 JOIN files older_file ON older_file.node_id = older_location.node_id \
                    AND older_file.size_bytes = observation.older_size_bytes \
                    AND older_file.modified_unix_ns IS observation.older_modified_unix_ns \
                 JOIN locations newer_location ON newer_location.id = observation.newer_location_id \
                    AND newer_location.scope_id = r.scope_id AND newer_location.present = 1 \
                 JOIN nodes newer_node ON newer_node.id = newer_location.node_id \
                    AND newer_node.kind = 'file' \
                 JOIN files newer_file ON newer_file.node_id = newer_location.node_id \
                    AND newer_file.size_bytes = observation.newer_size_bytes \
                    AND newer_file.modified_unix_ns IS observation.newer_modified_unix_ns \
                 LEFT JOIN file_relation_feedback_events feedback \
                    ON feedback.relation_id = r.id \
                 WHERE r.id = ?1 LIMIT 1",
                [relation_id],
                |row| {
                    Ok((
                        row.get::<_, i64>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, i64>(2)?,
                        row.get::<_, i64>(3)?,
                        row.get::<_, i64>(4)?,
                        row.get::<_, i64>(5)?,
                        row.get::<_, i64>(6)?,
                        row.get::<_, String>(7)?,
                        row.get::<_, i64>(8)?,
                        row.get::<_, Option<i64>>(9)?,
                        row.get::<_, i64>(10)?,
                        row.get::<_, i64>(11)?,
                        row.get::<_, String>(12)?,
                        row.get::<_, i64>(13)?,
                        row.get::<_, Option<i64>>(14)?,
                        row.get::<_, String>(15)?,
                        row.get::<_, String>(16)?,
                        row.get::<_, i64>(17)?,
                        row.get::<_, i64>(18)?,
                        row.get::<_, i64>(19)?,
                        row.get::<_, String>(20)?,
                        row.get::<_, String>(21)?,
                        row.get::<_, String>(22)?,
                        row.get::<_, String>(23)?,
                        row.get::<_, Option<String>>(24)?,
                        row.get::<_, i64>(25)?,
                        row.get::<_, i64>(26)?,
                        row.get::<_, Option<i64>>(27)?,
                    ))
                },
            )
            .optional()?;
        let Some((
            relation_id,
            relation_kind,
            scope_id,
            left_node_id,
            right_node_id,
            older_node_id,
            older_location_id,
            older_display_path,
            older_size_bytes,
            older_modified_unix_ns,
            newer_node_id,
            newer_location_id,
            newer_display_path,
            newer_size_bytes,
            newer_modified_unix_ns,
            base_key,
            extension_key,
            older_version,
            newer_version,
            confidence_basis_points,
            signal_kind,
            created_by,
            provider_id,
            provider_version,
            model_version,
            observed_at_unix_ms,
            observation_id,
            feedback_id,
        )) = row
        else {
            let relation_exists = connection.query_row(
                "SELECT EXISTS(SELECT 1 FROM file_relation_candidates WHERE id = ?1)",
                [relation_id],
                |row| row.get::<_, i64>(0),
            )? != 0;
            return Err(if relation_exists {
                DatabaseError::FileRelationCandidateNotCurrent
            } else {
                DatabaseError::FileRelationCandidateNotFound
            });
        };
        let current_older = explicit_version_name_from_display_path(&older_display_path)?;
        let current_newer = explicit_version_name_from_display_path(&newer_display_path)?;
        let stable_nodes_match = left_node_id == older_node_id.min(newer_node_id)
            && right_node_id == older_node_id.max(newer_node_id);
        if relation_kind != "version"
            || !stable_nodes_match
            || older_node_id == newer_node_id
            || base_key != current_older.base_key
            || base_key != current_newer.base_key
            || extension_key != current_older.extension_key
            || extension_key != current_newer.extension_key
            || older_version != i64::from(current_older.version)
            || newer_version != i64::from(current_newer.version)
            || older_version >= newer_version
            || confidence_basis_points != 9_000
            || signal_kind != "explicit_numeric_suffix"
            || created_by != "system_rule"
            || provider_id != FileVersionEvidence::PROVIDER_ID
            || provider_version != FileVersionEvidence::PROVIDER_VERSION
            || model_version.is_some()
            || observation_id <= 0
            || feedback_id.is_some()
        {
            return Err(DatabaseError::InvalidStoredValue);
        }
        let latest_decision =
            latest_equivalent_file_version_decision(connection, relation_id, observation_id)?;
        let state = match latest_decision.as_ref().map(|decision| decision.kind) {
            None => FileRelationCandidateState::Suggested,
            Some(FileRelationDecisionKind::Accepted) => FileRelationCandidateState::Accepted,
            Some(FileRelationDecisionKind::Rejected) => FileRelationCandidateState::Rejected,
        };
        Ok(FileVersionCandidate {
            api_version: FileVersionCandidate::API_VERSION,
            relation_id,
            kind: FileRelationKind::Version,
            state,
            older: FileRelationEndpoint {
                scope_id,
                node_id: older_node_id,
                location_id: older_location_id,
                display_path: older_display_path,
                size_bytes: row_u64_value(older_size_bytes)?,
                modified_unix_ns: older_modified_unix_ns,
            },
            newer: FileRelationEndpoint {
                scope_id,
                node_id: newer_node_id,
                location_id: newer_location_id,
                display_path: newer_display_path,
                size_bytes: row_u64_value(newer_size_bytes)?,
                modified_unix_ns: newer_modified_unix_ns,
            },
            evidence: FileVersionEvidence {
                signal_kind: FileVersionSignalKind::ExplicitNumericSuffix,
                base_key,
                extension_key,
                older_version: u32::try_from(older_version)
                    .map_err(|_| DatabaseError::InvalidStoredValue)?,
                newer_version: u32::try_from(newer_version)
                    .map_err(|_| DatabaseError::InvalidStoredValue)?,
                confidence_basis_points: u16::try_from(confidence_basis_points)
                    .map_err(|_| DatabaseError::InvalidStoredValue)?,
                observed_at_unix_ms,
                created_by: FileRelationCreator::SystemRule,
                provider_id: FileVersionEvidence::PROVIDER_ID,
                provider_version: FileVersionEvidence::PROVIDER_VERSION,
                model_version: None,
            },
            latest_decision,
        })
    }

    fn file_relation_candidate(
        &self,
        relation_id: i64,
    ) -> Result<FileRelationCandidate, DatabaseError> {
        if relation_id <= 0 {
            return Err(DatabaseError::FileRelationCandidateInputInvalid);
        }
        let row = self
            .connection
            .query_row(
                "SELECT r.id, r.relation_kind, r.scope_id, \
                        r.left_node_id, left_location.id, left_location.display_path, \
                        left_file.size_bytes, left_file.modified_unix_ns, \
                        r.right_node_id, right_location.id, right_location.display_path, \
                        right_file.size_bytes, right_file.modified_unix_ns, \
                        observation.compared_bytes, observation.confidence_basis_points, \
                        observation.comparison_kind, observation.created_by, \
                        observation.provider_id, observation.provider_version, \
                        observation.model_version, observation.observed_at_unix_ms, \
                        feedback.sequence, feedback.decision, feedback.created_by, \
                        feedback.created_at_unix_ms \
                 FROM file_relation_candidates r \
                 JOIN file_relation_observations observation ON observation.id = ( \
                    SELECT latest.id FROM file_relation_observations latest \
                    WHERE latest.relation_id = r.id \
                    ORDER BY latest.observed_at_unix_ms DESC, latest.id DESC LIMIT 1 \
                 ) \
                 JOIN nodes left_node ON left_node.id = r.left_node_id \
                    AND left_node.kind = 'file' \
                 JOIN locations left_location ON left_location.id = observation.left_location_id \
                    AND left_location.scope_id = r.scope_id \
                    AND left_location.node_id = r.left_node_id \
                    AND left_location.present = 1 \
                 JOIN files left_file ON left_file.node_id = r.left_node_id \
                    AND left_file.size_bytes = observation.source_size_bytes \
                    AND left_file.modified_unix_ns IS observation.left_modified_unix_ns \
                 JOIN nodes right_node ON right_node.id = r.right_node_id \
                    AND right_node.kind = 'file' \
                 JOIN locations right_location ON right_location.id = observation.right_location_id \
                    AND right_location.scope_id = r.scope_id \
                    AND right_location.node_id = r.right_node_id \
                    AND right_location.present = 1 \
                 JOIN files right_file ON right_file.node_id = r.right_node_id \
                    AND right_file.size_bytes = observation.source_size_bytes \
                    AND right_file.modified_unix_ns IS observation.right_modified_unix_ns \
                 LEFT JOIN file_relation_feedback_events feedback ON feedback.id = ( \
                    SELECT latest_feedback.id FROM file_relation_feedback_events latest_feedback \
                    WHERE latest_feedback.relation_id = r.id \
                    ORDER BY latest_feedback.sequence DESC LIMIT 1 \
                 ) \
                 WHERE r.id = ?1",
                [relation_id],
                |row| {
                    Ok((
                        row.get::<_, i64>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, i64>(2)?,
                        row.get::<_, i64>(3)?,
                        row.get::<_, i64>(4)?,
                        row.get::<_, String>(5)?,
                        row.get::<_, i64>(6)?,
                        row.get::<_, Option<i64>>(7)?,
                        row.get::<_, i64>(8)?,
                        row.get::<_, i64>(9)?,
                        row.get::<_, String>(10)?,
                        row.get::<_, i64>(11)?,
                        row.get::<_, Option<i64>>(12)?,
                        row.get::<_, i64>(13)?,
                        row.get::<_, i64>(14)?,
                        row.get::<_, String>(15)?,
                        row.get::<_, String>(16)?,
                        row.get::<_, String>(17)?,
                        row.get::<_, String>(18)?,
                        row.get::<_, Option<String>>(19)?,
                        row.get::<_, i64>(20)?,
                        row.get::<_, Option<i64>>(21)?,
                        row.get::<_, Option<String>>(22)?,
                        row.get::<_, Option<String>>(23)?,
                        row.get::<_, Option<i64>>(24)?,
                    ))
                },
            )
            .optional()?;
        let Some((
            relation_id,
            relation_kind,
            scope_id,
            left_node_id,
            left_location_id,
            left_display_path,
            left_size_bytes,
            left_modified_unix_ns,
            right_node_id,
            right_location_id,
            right_display_path,
            right_size_bytes,
            right_modified_unix_ns,
            compared_bytes,
            confidence_basis_points,
            comparison_kind,
            created_by,
            provider_id,
            provider_version,
            model_version,
            observed_at_unix_ms,
            decision_sequence,
            decision_kind,
            decision_creator,
            decided_at_unix_ms,
        )) = row
        else {
            let relation_exists = self.connection.query_row(
                "SELECT EXISTS(SELECT 1 FROM file_relation_candidates WHERE id = ?1)",
                [relation_id],
                |row| row.get::<_, i64>(0),
            )? != 0;
            return Err(if relation_exists {
                DatabaseError::FileRelationCandidateNotCurrent
            } else {
                DatabaseError::FileRelationCandidateNotFound
            });
        };
        if relation_kind != "exact_duplicate"
            || comparison_kind != "byte_for_byte"
            || created_by != "system_rule"
            || provider_id != FileRelationEvidence::PROVIDER_ID
            || provider_version != FileRelationEvidence::PROVIDER_VERSION
            || model_version.is_some()
            || left_size_bytes != right_size_bytes
            || compared_bytes != left_size_bytes
            || confidence_basis_points != 10_000
        {
            return Err(DatabaseError::InvalidStoredValue);
        }
        let source_size_bytes = row_u64_value(left_size_bytes)?;
        if source_size_bytes == 0 || source_size_bytes > MAX_FILE_RELATION_SOURCE_BYTES {
            return Err(DatabaseError::InvalidStoredValue);
        }
        let latest_decision = match (
            decision_sequence,
            decision_kind,
            decision_creator,
            decided_at_unix_ms,
        ) {
            (None, None, None, None) => None,
            (Some(sequence), Some(kind), Some(creator), Some(decided_at_unix_ms)) => {
                if creator != "user" {
                    return Err(DatabaseError::InvalidStoredValue);
                }
                Some(FileRelationDecision {
                    sequence: u64::try_from(sequence)
                        .map_err(|_| DatabaseError::InvalidStoredValue)?,
                    kind: file_relation_decision_kind_from_str(&kind)?,
                    created_by: FileRelationDecisionCreator::User,
                    decided_at_unix_ms,
                })
            }
            _ => return Err(DatabaseError::InvalidStoredValue),
        };
        let state = match latest_decision.as_ref().map(|decision| decision.kind) {
            None => FileRelationCandidateState::Suggested,
            Some(FileRelationDecisionKind::Accepted) => FileRelationCandidateState::Accepted,
            Some(FileRelationDecisionKind::Rejected) => FileRelationCandidateState::Rejected,
        };
        Ok(FileRelationCandidate {
            api_version: FileRelationCandidate::API_VERSION,
            relation_id,
            kind: FileRelationKind::ExactDuplicate,
            state,
            left: FileRelationEndpoint {
                scope_id,
                node_id: left_node_id,
                location_id: left_location_id,
                display_path: left_display_path,
                size_bytes: source_size_bytes,
                modified_unix_ns: left_modified_unix_ns,
            },
            right: FileRelationEndpoint {
                scope_id,
                node_id: right_node_id,
                location_id: right_location_id,
                display_path: right_display_path,
                size_bytes: source_size_bytes,
                modified_unix_ns: right_modified_unix_ns,
            },
            evidence: FileRelationEvidence {
                comparison_kind: FileRelationComparisonKind::ByteForByte,
                compared_bytes: row_u64_value(compared_bytes)?,
                confidence_basis_points: u16::try_from(confidence_basis_points)
                    .map_err(|_| DatabaseError::InvalidStoredValue)?,
                observed_at_unix_ms,
                created_by: FileRelationCreator::SystemRule,
                provider_id: FileRelationEvidence::PROVIDER_ID,
                provider_version: FileRelationEvidence::PROVIDER_VERSION,
                model_version: None,
                bounded_max_bytes: MAX_FILE_RELATION_SOURCE_BYTES,
            },
            latest_decision,
        })
    }

    pub fn decide_file_relation_candidate(
        &mut self,
        relation_id: i64,
        decision: FileRelationDecisionKind,
    ) -> Result<FileRelationCandidate, DatabaseError> {
        if relation_id <= 0 {
            return Err(DatabaseError::FileRelationCandidateInputInvalid);
        }
        let now = unix_ms()?;
        let transaction = self.connection.transaction()?;
        let relation_exists = transaction.query_row(
            "SELECT EXISTS(SELECT 1 FROM file_relation_candidates WHERE id = ?1)",
            [relation_id],
            |row| row.get::<_, i64>(0),
        )? != 0;
        if !relation_exists {
            return Err(DatabaseError::FileRelationCandidateNotFound);
        }
        let relation_is_current = transaction.query_row(
            "SELECT EXISTS( \
                 SELECT 1 FROM file_relation_candidates r \
                 JOIN file_relation_observations observation ON observation.id = ( \
                    SELECT latest.id FROM file_relation_observations latest \
                    WHERE latest.relation_id = r.id \
                    ORDER BY latest.observed_at_unix_ms DESC, latest.id DESC LIMIT 1 \
                 ) \
                 JOIN locations left_location ON left_location.id = observation.left_location_id \
                    AND left_location.scope_id = r.scope_id \
                    AND left_location.node_id = r.left_node_id AND left_location.present = 1 \
                 JOIN files left_file ON left_file.node_id = r.left_node_id \
                    AND left_file.size_bytes = observation.source_size_bytes \
                    AND left_file.modified_unix_ns IS observation.left_modified_unix_ns \
                 JOIN locations right_location ON right_location.id = observation.right_location_id \
                    AND right_location.scope_id = r.scope_id \
                    AND right_location.node_id = r.right_node_id AND right_location.present = 1 \
                 JOIN files right_file ON right_file.node_id = r.right_node_id \
                    AND right_file.size_bytes = observation.source_size_bytes \
                    AND right_file.modified_unix_ns IS observation.right_modified_unix_ns \
                 WHERE r.id = ?1 AND r.relation_kind = 'exact_duplicate' \
             )",
            [relation_id],
            |row| row.get::<_, i64>(0),
        )? != 0;
        if !relation_is_current {
            return Err(DatabaseError::FileRelationCandidateNotCurrent);
        }
        let latest = transaction
            .query_row(
                "SELECT sequence, decision FROM file_relation_feedback_events \
                 WHERE relation_id = ?1 ORDER BY sequence DESC LIMIT 1",
                [relation_id],
                |row| Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?)),
            )
            .optional()?;
        if latest
            .as_ref()
            .is_some_and(|(_, stored)| stored == file_relation_decision_kind_str(decision))
        {
            transaction.commit()?;
            return self.file_relation_candidate(relation_id);
        }
        let sequence = latest.map_or(Ok(1_i64), |(sequence, _)| {
            sequence.checked_add(1).ok_or(DatabaseError::InvalidCount)
        })?;
        transaction.execute(
            "INSERT INTO file_relation_feedback_events( \
                 relation_id, sequence, decision, created_by, created_at_unix_ms \
             ) VALUES (?1, ?2, ?3, 'user', ?4)",
            params![
                relation_id,
                sequence,
                file_relation_decision_kind_str(decision),
                now
            ],
        )?;
        transaction.commit()?;
        self.file_relation_candidate(relation_id)
    }

    pub fn decide_file_version_candidate(
        &mut self,
        relation_id: i64,
        decision: FileRelationDecisionKind,
    ) -> Result<FileVersionCandidate, DatabaseError> {
        if relation_id <= 0 {
            return Err(DatabaseError::FileRelationCandidateInputInvalid);
        }
        let now = unix_ms()?;
        let transaction = self.connection.transaction()?;
        Self::file_version_candidate_from_connection(&transaction, relation_id)?;
        let current_observation_id = transaction.query_row(
            "SELECT id FROM file_version_observations WHERE relation_id = ?1 \
             ORDER BY observed_at_unix_ms DESC, id DESC LIMIT 1",
            [relation_id],
            |row| row.get::<_, i64>(0),
        )?;
        let latest_equivalent = latest_equivalent_file_version_decision(
            &transaction,
            relation_id,
            current_observation_id,
        )?;
        if latest_equivalent
            .as_ref()
            .is_some_and(|stored| stored.kind == decision)
        {
            transaction.commit()?;
            return self.file_version_candidate(relation_id);
        }
        let latest_sequence = transaction
            .query_row(
                "SELECT sequence FROM file_version_feedback_events \
                 WHERE relation_id = ?1 ORDER BY sequence DESC LIMIT 1",
                [relation_id],
                |row| row.get::<_, i64>(0),
            )
            .optional()?;
        let sequence = latest_sequence.map_or(Ok(1_i64), |sequence| {
            sequence.checked_add(1).ok_or(DatabaseError::InvalidCount)
        })?;
        transaction.execute(
            "INSERT INTO file_version_feedback_events( \
                 relation_id, evidence_observation_id, sequence, decision, created_by, \
                 created_at_unix_ms \
             ) VALUES (?1, ?2, ?3, ?4, 'user', ?5)",
            params![
                relation_id,
                current_observation_id,
                sequence,
                file_relation_decision_kind_str(decision),
                now,
            ],
        )?;
        transaction.commit()?;
        self.file_version_candidate(relation_id)
    }

    pub fn recent_file_relation_candidates(
        &self,
    ) -> Result<Vec<FileRelationCandidateSummary>, DatabaseError> {
        let relations = {
            let mut statement = self.connection.prepare(
                "SELECT id, relation_kind, scope_id, left_node_id, right_node_id \
                 FROM file_relation_candidates ORDER BY id DESC LIMIT 20",
            )?;
            let rows = statement.query_map([], |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, i64>(2)?,
                    row.get::<_, i64>(3)?,
                    row.get::<_, i64>(4)?,
                ))
            })?;
            rows.collect::<Result<Vec<_>, _>>()?
        };
        relations
            .into_iter()
            .map(
                |(relation_id, relation_kind, scope_id, left_node_id, right_node_id)| {
                    self.file_relation_candidate_summary(
                        relation_id,
                        &relation_kind,
                        scope_id,
                        left_node_id,
                        right_node_id,
                    )
                },
            )
            .collect()
    }

    fn file_relation_candidate_summary(
        &self,
        relation_id: i64,
        relation_kind: &str,
        scope_id: i64,
        left_node_id: i64,
        right_node_id: i64,
    ) -> Result<FileRelationCandidateSummary, DatabaseError> {
        let feedback = self
            .connection
            .query_row(
                "SELECT sequence, decision, created_by, created_at_unix_ms \
                 FROM file_relation_feedback_events WHERE relation_id = ?1 \
                 ORDER BY sequence DESC LIMIT 1",
                [relation_id],
                |row| {
                    Ok((
                        row.get::<_, i64>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, String>(2)?,
                        row.get::<_, i64>(3)?,
                    ))
                },
            )
            .optional()?;
        let mut latest_decision = match feedback {
            None => None,
            Some((sequence, kind, creator, decided_at_unix_ms)) => {
                if creator != "user" {
                    return Err(DatabaseError::InvalidStoredValue);
                }
                let _ = u64::try_from(sequence).map_err(|_| DatabaseError::InvalidStoredValue)?;
                Some((
                    file_relation_decision_kind_from_str(&kind)?,
                    decided_at_unix_ms,
                ))
            }
        };
        let (kind, confidence_basis_points, observed_at_unix_ms) = match relation_kind {
            "exact_duplicate" => {
                let evidence = self.connection.query_row(
                    "SELECT source_size_bytes, compared_bytes, confidence_basis_points, \
                            comparison_kind, created_by, provider_id, provider_version, \
                            model_version, observed_at_unix_ms \
                     FROM file_relation_observations WHERE relation_id = ?1 \
                     ORDER BY observed_at_unix_ms DESC, id DESC LIMIT 1",
                    [relation_id],
                    |row| {
                        Ok((
                            row.get::<_, i64>(0)?,
                            row.get::<_, i64>(1)?,
                            row.get::<_, i64>(2)?,
                            row.get::<_, String>(3)?,
                            row.get::<_, String>(4)?,
                            row.get::<_, String>(5)?,
                            row.get::<_, String>(6)?,
                            row.get::<_, Option<String>>(7)?,
                            row.get::<_, i64>(8)?,
                        ))
                    },
                )?;
                if evidence.0 <= 0
                    || evidence.0 != evidence.1
                    || evidence.2 != 10_000
                    || evidence.3 != "byte_for_byte"
                    || evidence.4 != "system_rule"
                    || evidence.5 != FileRelationEvidence::PROVIDER_ID
                    || evidence.6 != FileRelationEvidence::PROVIDER_VERSION
                    || evidence.7.is_some()
                {
                    return Err(DatabaseError::InvalidStoredValue);
                }
                (FileRelationKind::ExactDuplicate, evidence.2, evidence.8)
            }
            "version" => {
                if latest_decision.is_some() {
                    return Err(DatabaseError::InvalidStoredValue);
                }
                let evidence = self.connection.query_row(
                    "SELECT id, base_key, extension_key, older_version, newer_version, \
                            confidence_basis_points, signal_kind, created_by, provider_id, \
                            provider_version, model_version, observed_at_unix_ms \
                     FROM file_version_observations WHERE relation_id = ?1 \
                     ORDER BY observed_at_unix_ms DESC, id DESC LIMIT 1",
                    [relation_id],
                    |row| {
                        Ok((
                            row.get::<_, i64>(0)?,
                            row.get::<_, String>(1)?,
                            row.get::<_, String>(2)?,
                            row.get::<_, i64>(3)?,
                            row.get::<_, i64>(4)?,
                            row.get::<_, i64>(5)?,
                            row.get::<_, String>(6)?,
                            row.get::<_, String>(7)?,
                            row.get::<_, String>(8)?,
                            row.get::<_, String>(9)?,
                            row.get::<_, Option<String>>(10)?,
                            row.get::<_, i64>(11)?,
                        ))
                    },
                )?;
                if evidence.0 <= 0
                    || evidence.1.is_empty()
                    || evidence.1.len() > 1_024
                    || evidence.2.len() > 64
                    || evidence.3 < 1
                    || evidence.3 >= evidence.4
                    || evidence.4 > 999_999
                    || evidence.5 != 9_000
                    || evidence.6 != "explicit_numeric_suffix"
                    || evidence.7 != "system_rule"
                    || evidence.8 != FileVersionEvidence::PROVIDER_ID
                    || evidence.9 != FileVersionEvidence::PROVIDER_VERSION
                    || evidence.10.is_some()
                {
                    return Err(DatabaseError::InvalidStoredValue);
                }
                latest_decision = latest_equivalent_file_version_decision(
                    &self.connection,
                    relation_id,
                    evidence.0,
                )?
                .map(|decision| (decision.kind, decision.decided_at_unix_ms));
                (FileRelationKind::Version, evidence.5, evidence.11)
            }
            _ => return Err(DatabaseError::InvalidStoredValue),
        };
        let state = match latest_decision.map(|decision| decision.0) {
            None => FileRelationCandidateState::Suggested,
            Some(FileRelationDecisionKind::Accepted) => FileRelationCandidateState::Accepted,
            Some(FileRelationDecisionKind::Rejected) => FileRelationCandidateState::Rejected,
        };
        Ok(FileRelationCandidateSummary {
            api_version: FileRelationCandidateSummary::API_VERSION,
            relation_id,
            kind,
            state,
            scope_id,
            left_node_id,
            right_node_id,
            confidence_basis_points: u16::try_from(confidence_basis_points)
                .map_err(|_| DatabaseError::InvalidStoredValue)?,
            last_observed_at_unix_ms: observed_at_unix_ms,
            latest_decision_at_unix_ms: latest_decision.map(|decision| decision.1),
            verification_required: true,
        })
    }

    pub fn exact_duplicate_sources(
        &self,
        relation_id: i64,
    ) -> Result<(ActionSourceRecord, ActionSourceRecord), DatabaseError> {
        if relation_id <= 0 {
            return Err(DatabaseError::FileRelationCandidateInputInvalid);
        }
        let relation = self
            .connection
            .query_row(
                "SELECT scope_id, left_node_id, right_node_id \
                 FROM file_relation_candidates \
                 WHERE id = ?1 AND relation_kind = 'exact_duplicate'",
                [relation_id],
                |row| {
                    Ok((
                        row.get::<_, i64>(0)?,
                        row.get::<_, i64>(1)?,
                        row.get::<_, i64>(2)?,
                    ))
                },
            )
            .optional()?
            .ok_or(DatabaseError::FileRelationCandidateNotFound)?;
        let left = self.current_file_source(relation.0, relation.1)?;
        let right = self.current_file_source(relation.0, relation.2)?;
        Ok((left, right))
    }

    pub fn file_version_sources(
        &self,
        relation_id: i64,
    ) -> Result<(ActionSourceRecord, ActionSourceRecord), DatabaseError> {
        if relation_id <= 0 {
            return Err(DatabaseError::FileRelationCandidateInputInvalid);
        }
        let relation = self
            .connection
            .query_row(
                "SELECT scope_id, left_node_id, right_node_id \
                 FROM file_relation_candidates \
                 WHERE id = ?1 AND relation_kind = 'version'",
                [relation_id],
                |row| {
                    Ok((
                        row.get::<_, i64>(0)?,
                        row.get::<_, i64>(1)?,
                        row.get::<_, i64>(2)?,
                    ))
                },
            )
            .optional()?
            .ok_or(DatabaseError::FileRelationCandidateNotFound)?;
        let left = self.current_file_source(relation.0, relation.1)?;
        let right = self.current_file_source(relation.0, relation.2)?;
        Ok((left, right))
    }

    fn current_file_source(
        &self,
        scope_id: i64,
        node_id: i64,
    ) -> Result<ActionSourceRecord, DatabaseError> {
        self.connection
            .query_row(
                "SELECT l.scope_id, l.node_id, l.id, l.path_raw, l.path_key, l.display_path, \
                        n.identity_kind, n.identity_key, f.size_bytes, f.modified_unix_ns \
                 FROM locations l \
                 JOIN nodes n ON n.id = l.node_id AND n.kind = 'file' \
                 JOIN files f ON f.node_id = l.node_id \
                 WHERE l.scope_id = ?1 AND l.node_id = ?2 AND l.present = 1 \
                 ORDER BY l.id LIMIT 1",
                params![scope_id, node_id],
                |row| {
                    Ok(ActionSourceRecord {
                        scope_id: row.get(0)?,
                        node_id: row.get(1)?,
                        location_id: row.get(2)?,
                        path_raw: row.get(3)?,
                        path_key: row.get(4)?,
                        display_path: row.get(5)?,
                        identity_kind: row.get(6)?,
                        identity_key: row.get(7)?,
                        size_bytes: row_u64(row, 8)?,
                        modified_unix_ns: row.get(9)?,
                    })
                },
            )
            .optional()?
            .ok_or(DatabaseError::FileRelationCandidateNotCurrent)
    }

    pub fn action_source_for_path_key(
        &self,
        scope_id: i64,
        path_key: &str,
    ) -> Result<ActionSourceRecord, DatabaseError> {
        self.connection
            .query_row(
                "SELECT l.scope_id, l.node_id, l.id, l.path_raw, l.path_key, l.display_path, \
                        n.identity_kind, n.identity_key, f.size_bytes, f.modified_unix_ns \
                 FROM locations l \
                 JOIN nodes n ON n.id = l.node_id AND n.kind = 'file' \
                 JOIN files f ON f.node_id = l.node_id \
                 WHERE l.scope_id = ?1 AND l.path_key = ?2 AND l.present = 1",
                params![scope_id, path_key],
                |row| {
                    Ok(ActionSourceRecord {
                        scope_id: row.get(0)?,
                        node_id: row.get(1)?,
                        location_id: row.get(2)?,
                        path_raw: row.get(3)?,
                        path_key: row.get(4)?,
                        display_path: row.get(5)?,
                        identity_kind: row.get(6)?,
                        identity_key: row.get(7)?,
                        size_bytes: row_u64(row, 8)?,
                        modified_unix_ns: row.get(9)?,
                    })
                },
            )
            .optional()?
            .ok_or(DatabaseError::ActionSourceNotFound)
    }

    pub fn create_rename_action_plan(
        &mut self,
        plan: ActionPlanWrite<'_>,
    ) -> Result<ActionPlanPreview, DatabaseError> {
        validate_action_plan_write(&plan)?;
        let source_size = to_i64(plan.source_size_bytes)?;
        let created_at = unix_ms()?;
        let execution_strategy = action_execution_strategy_str(plan.execution_strategy);
        let transaction = self.connection.transaction()?;
        let snapshot_matches: i64 = transaction.query_row(
            "SELECT COUNT(*) \
             FROM locations l \
             JOIN nodes n ON n.id = l.node_id AND n.kind = 'file' \
             JOIN files f ON f.node_id = l.node_id \
             WHERE l.id = ?1 AND l.scope_id = ?2 AND l.node_id = ?3 AND l.present = 1 \
               AND l.path_raw = ?4 AND l.path_key = ?5 AND l.display_path = ?6 \
               AND n.identity_kind = ?7 AND n.identity_key = ?8 \
               AND f.size_bytes = ?9 AND f.modified_unix_ns IS ?10",
            params![
                plan.source_location_id,
                plan.scope_id,
                plan.node_id,
                plan.source_path_raw,
                plan.source_path_key,
                plan.source_display_path,
                plan.source_identity_kind,
                plan.source_identity_key,
                source_size,
                plan.source_modified_unix_ns,
            ],
            |row| row.get(0),
        )?;
        if snapshot_matches != 1 {
            return Err(DatabaseError::ActionSourceSnapshotChanged);
        }
        transaction.execute(
            "INSERT INTO action_plans( \
                api_version, policy_version, operation, execution_strategy, scope_id, node_id, \
                source_location_id, source_path_raw, source_path_key, source_display_path, \
                destination_path_raw, destination_path_key, destination_display_path, \
                source_identity_kind, source_identity_key, source_size_bytes, \
                source_modified_unix_ns, created_at_unix_ms \
             ) VALUES ( \
                'deskgraph.action-plan.v1', 'deskgraph.action-policy.v1', 'rename', ?1, ?2, ?3, \
                ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15 \
             )",
            params![
                execution_strategy,
                plan.scope_id,
                plan.node_id,
                plan.source_location_id,
                plan.source_path_raw,
                plan.source_path_key,
                plan.source_display_path,
                plan.destination_path_raw,
                plan.destination_path_key,
                plan.destination_display_path,
                plan.source_identity_kind,
                plan.source_identity_key,
                source_size,
                plan.source_modified_unix_ns,
                created_at,
            ],
        )?;
        let plan_id = transaction.last_insert_rowid();
        transaction.execute(
            "INSERT INTO action_plan_events(plan_id, sequence, event_kind, created_at_unix_ms) \
             VALUES (?1, 1, 'preview_created', ?2)",
            params![plan_id, created_at],
        )?;
        transaction.commit()?;
        self.action_plan(plan_id)
    }

    pub fn action_plan(&self, plan_id: i64) -> Result<ActionPlanPreview, DatabaseError> {
        self.connection
            .query_row(
                "SELECT p.id, p.operation, p.scope_id, p.node_id, p.source_display_path, \
                        p.destination_display_path, p.execution_strategy, p.created_at_unix_ms, \
                        MAX(e.sequence) \
                 FROM action_plans p \
                 JOIN action_plan_events e ON e.plan_id = p.id \
                 WHERE p.id = ?1 \
                 GROUP BY p.id",
                [plan_id],
                action_plan_from_row,
            )
            .optional()?
            .ok_or(DatabaseError::ActionPlanNotFound)
    }

    pub fn recent_action_plans(&self) -> Result<Vec<ActionPlanSummary>, DatabaseError> {
        let mut statement = self.connection.prepare(
            "SELECT p.id, p.operation, p.scope_id, p.node_id, p.execution_strategy, \
                    p.created_at_unix_ms, MAX(e.sequence) \
             FROM action_plans p \
             JOIN action_plan_events e ON e.plan_id = p.id \
             GROUP BY p.id \
             ORDER BY p.id DESC \
             LIMIT 20",
        )?;
        let rows = statement.query_map([], action_plan_summary_from_row)?;
        rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    }

    pub fn node_id_for_path_key(
        &self,
        scope_id: i64,
        path_key: &str,
    ) -> Result<Option<i64>, DatabaseError> {
        self.connection
            .query_row(
                "SELECT node_id FROM locations WHERE scope_id = ?1 AND path_key = ?2 AND present = 1",
                params![scope_id, path_key],
                |row| row.get(0),
            )
            .optional()
            .map_err(Into::into)
    }
}

fn validate_exact_duplicate_sources(
    left: &ActionSourceRecord,
    right: &ActionSourceRecord,
) -> Result<(), DatabaseError> {
    if left.scope_id <= 0
        || left.scope_id != right.scope_id
        || left.node_id <= 0
        || left.node_id >= right.node_id
        || left.location_id <= 0
        || right.location_id <= 0
        || left.location_id == right.location_id
        || left.path_raw.is_empty()
        || right.path_raw.is_empty()
        || left.path_key.is_empty()
        || right.path_key.is_empty()
        || left.display_path.is_empty()
        || right.display_path.is_empty()
        || left.identity_kind == "path_fallback"
        || right.identity_kind == "path_fallback"
        || left.identity_kind.is_empty()
        || right.identity_kind.is_empty()
        || left.identity_key.is_empty()
        || right.identity_key.is_empty()
        || (left.identity_kind == right.identity_kind && left.identity_key == right.identity_key)
        || left.size_bytes == 0
        || left.size_bytes != right.size_bytes
        || left.size_bytes > MAX_FILE_RELATION_SOURCE_BYTES
    {
        return Err(DatabaseError::FileRelationCandidateInputInvalid);
    }
    Ok(())
}

fn validate_file_relation_sources(
    first: &ActionSourceRecord,
    second: &ActionSourceRecord,
) -> Result<(), DatabaseError> {
    if first.scope_id <= 0
        || first.scope_id != second.scope_id
        || first.node_id <= 0
        || second.node_id <= 0
        || first.node_id == second.node_id
        || first.location_id <= 0
        || second.location_id <= 0
        || first.location_id == second.location_id
        || first.path_raw.is_empty()
        || second.path_raw.is_empty()
        || first.path_key.is_empty()
        || second.path_key.is_empty()
        || first.display_path.is_empty()
        || second.display_path.is_empty()
        || first.identity_kind == "path_fallback"
        || second.identity_kind == "path_fallback"
        || first.identity_kind.is_empty()
        || second.identity_kind.is_empty()
        || first.identity_key.is_empty()
        || second.identity_key.is_empty()
        || (first.identity_kind == second.identity_kind
            && first.identity_key == second.identity_key)
    {
        return Err(DatabaseError::FileRelationCandidateInputInvalid);
    }
    Ok(())
}

fn explicit_version_name_from_source(
    source: &ActionSourceRecord,
) -> Result<ExplicitFileVersionName, DatabaseError> {
    explicit_version_name_from_display_path(&source.display_path)
}

fn explicit_version_name_from_display_path(
    display_path: &str,
) -> Result<ExplicitFileVersionName, DatabaseError> {
    let file_name = Path::new(display_path)
        .file_name()
        .and_then(|value| value.to_str())
        .ok_or(DatabaseError::FileRelationCandidateInputInvalid)?;
    parse_explicit_file_version_name(file_name)
        .ok_or(DatabaseError::FileRelationCandidateInputInvalid)
}

fn latest_equivalent_file_version_decision(
    connection: &Connection,
    relation_id: i64,
    current_observation_id: i64,
) -> Result<Option<FileVersionDecision>, DatabaseError> {
    let feedback = connection
        .query_row(
            "SELECT feedback.evidence_observation_id, feedback.sequence, feedback.decision, \
                    feedback.created_by, feedback.created_at_unix_ms \
             FROM file_version_feedback_events feedback \
             JOIN file_version_observations bound \
                ON bound.id = feedback.evidence_observation_id \
                AND bound.relation_id = feedback.relation_id \
             JOIN file_version_observations current \
                ON current.id = ?2 AND current.relation_id = feedback.relation_id \
             JOIN locations bound_older ON bound_older.id = bound.older_location_id \
             JOIN locations bound_newer ON bound_newer.id = bound.newer_location_id \
             JOIN locations current_older ON current_older.id = current.older_location_id \
             JOIN locations current_newer ON current_newer.id = current.newer_location_id \
             WHERE feedback.relation_id = ?1 \
               AND bound_older.node_id = current_older.node_id \
               AND bound_newer.node_id = current_newer.node_id \
               AND bound.base_key = current.base_key \
               AND bound.extension_key = current.extension_key \
               AND bound.older_version = current.older_version \
               AND bound.newer_version = current.newer_version \
               AND bound.confidence_basis_points = current.confidence_basis_points \
               AND bound.signal_kind = current.signal_kind \
               AND bound.created_by = current.created_by \
               AND bound.provider_id = current.provider_id \
               AND bound.provider_version = current.provider_version \
               AND bound.model_version IS current.model_version \
             ORDER BY feedback.sequence DESC LIMIT 1",
            params![relation_id, current_observation_id],
            |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, i64>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, i64>(4)?,
                ))
            },
        )
        .optional()?;
    feedback
        .map(
            |(evidence_observation_id, sequence, kind, creator, decided_at_unix_ms)| {
                if evidence_observation_id <= 0 || creator != "user" || decided_at_unix_ms < 0 {
                    return Err(DatabaseError::InvalidStoredValue);
                }
                Ok(FileVersionDecision {
                    sequence: u64::try_from(sequence)
                        .map_err(|_| DatabaseError::InvalidStoredValue)?,
                    evidence_observation_id,
                    kind: file_relation_decision_kind_from_str(&kind)?,
                    created_by: FileRelationDecisionCreator::User,
                    decided_at_unix_ms,
                })
            },
        )
        .transpose()
}

fn relation_snapshot_matches(
    transaction: &Transaction<'_>,
    source: &ActionSourceRecord,
) -> Result<bool, DatabaseError> {
    let matches: i64 = transaction.query_row(
        "SELECT COUNT(*) \
         FROM locations l \
         JOIN nodes n ON n.id = l.node_id AND n.kind = 'file' \
         JOIN files f ON f.node_id = l.node_id \
         WHERE l.id = ?1 AND l.scope_id = ?2 AND l.node_id = ?3 AND l.present = 1 \
           AND l.path_raw = ?4 AND l.path_key = ?5 AND l.display_path = ?6 \
           AND n.identity_kind = ?7 AND n.identity_key = ?8 \
           AND f.size_bytes = ?9 AND f.modified_unix_ns IS ?10",
        params![
            source.location_id,
            source.scope_id,
            source.node_id,
            source.path_raw.as_slice(),
            source.path_key.as_str(),
            source.display_path.as_str(),
            source.identity_kind.as_str(),
            source.identity_key.as_slice(),
            to_i64(source.size_bytes)?,
            source.modified_unix_ns,
        ],
        |row| row.get(0),
    )?;
    Ok(matches == 1)
}

fn validate_action_plan_write(plan: &ActionPlanWrite<'_>) -> Result<(), DatabaseError> {
    let paths_valid = !plan.source_path_raw.is_empty()
        && plan.source_path_raw.len() <= MAX_ACTION_PATH_BYTES
        && !plan.source_path_key.is_empty()
        && plan.source_path_key.len() <= MAX_ACTION_PATH_BYTES
        && !plan.source_display_path.is_empty()
        && plan.source_display_path.len() <= MAX_ACTION_PATH_BYTES
        && !plan.destination_path_raw.is_empty()
        && plan.destination_path_raw.len() <= MAX_ACTION_PATH_BYTES
        && !plan.destination_path_key.is_empty()
        && plan.destination_path_key.len() <= MAX_ACTION_PATH_BYTES
        && !plan.destination_display_path.is_empty()
        && plan.destination_display_path.len() <= MAX_ACTION_PATH_BYTES;
    if plan.scope_id <= 0
        || plan.node_id <= 0
        || plan.source_location_id <= 0
        || !paths_valid
        || plan.source_path_raw == plan.destination_path_raw
        || plan.source_display_path == plan.destination_display_path
        || plan.source_identity_kind.is_empty()
        || plan.source_identity_kind.len() > 128
        || plan.source_identity_key.is_empty()
        || plan.source_identity_key.len() > 4096
    {
        return Err(DatabaseError::ActionPlanInputInvalid);
    }
    Ok(())
}

fn validate_project_suggestion(
    scope_id: i64,
    root_folder_node_id: i64,
    suggestion: &ProjectSuggestion,
) -> Result<(), DatabaseError> {
    if scope_id <= 0
        || root_folder_node_id <= 0
        || suggestion.observed_at_unix_ms < 0
        || suggestion.created_by != ProjectSuggestionCreator::SystemRule
        || suggestion.provider_id != ProjectSuggestion::PROVIDER_ID
        || suggestion.provider_version != ProjectSuggestion::PROVIDER_VERSION
        || suggestion.model_version.is_some()
        || suggestion.provenance.is_empty()
        || suggestion.provenance.len() > 8
    {
        return Err(DatabaseError::ProjectCandidateInputInvalid);
    }
    let mut previous_kind = None;
    let mut strong_weights = Vec::new();
    for signal in &suggestion.provenance {
        let (expected_marker, expected_weight) = expected_project_signal(signal.kind);
        if signal.marker_name != expected_marker
            || signal.weight_basis_points != expected_weight
            || previous_kind.is_some_and(|previous| previous >= signal.kind)
        {
            return Err(DatabaseError::ProjectCandidateInputInvalid);
        }
        previous_kind = Some(signal.kind);
        if signal.kind != ProjectSignalKind::Readme {
            strong_weights.push(signal.weight_basis_points);
        }
    }
    let Some(maximum) = strong_weights.iter().copied().max() else {
        return Err(DatabaseError::ProjectCandidateInputInvalid);
    };
    let additional = u16::try_from(strong_weights.len().saturating_sub(1))
        .map_err(|_| DatabaseError::ProjectCandidateInputInvalid)?
        .saturating_mul(500);
    let expected_confidence = maximum.saturating_add(additional).min(9_500);
    if suggestion.confidence_basis_points != expected_confidence {
        return Err(DatabaseError::ProjectCandidateInputInvalid);
    }
    Ok(())
}

fn expected_project_signal(kind: ProjectSignalKind) -> (&'static str, u16) {
    match kind {
        ProjectSignalKind::CargoManifest => ("Cargo.toml", 8_500),
        ProjectSignalKind::JavaScriptPackage => ("package.json", 7_500),
        ProjectSignalKind::PythonProject => ("pyproject.toml", 8_000),
        ProjectSignalKind::GoModule => ("go.mod", 8_500),
        ProjectSignalKind::SwiftPackage => ("Package.swift", 8_500),
        ProjectSignalKind::XcodeProject => ("*.xcodeproj", 9_000),
        ProjectSignalKind::VisualStudioSolution => ("*.sln", 8_500),
        ProjectSignalKind::Readme => ("README", 1_500),
    }
}

fn project_signal_kind_str(kind: ProjectSignalKind) -> &'static str {
    match kind {
        ProjectSignalKind::CargoManifest => "cargo_manifest",
        ProjectSignalKind::JavaScriptPackage => "javascript_package",
        ProjectSignalKind::PythonProject => "python_project",
        ProjectSignalKind::GoModule => "go_module",
        ProjectSignalKind::SwiftPackage => "swift_package",
        ProjectSignalKind::XcodeProject => "xcode_project",
        ProjectSignalKind::VisualStudioSolution => "visual_studio_solution",
        ProjectSignalKind::Readme => "readme",
    }
}

fn project_signal_kind_from_str(stored: &str) -> Result<ProjectSignalKind, DatabaseError> {
    match stored {
        "cargo_manifest" => Ok(ProjectSignalKind::CargoManifest),
        "javascript_package" => Ok(ProjectSignalKind::JavaScriptPackage),
        "python_project" => Ok(ProjectSignalKind::PythonProject),
        "go_module" => Ok(ProjectSignalKind::GoModule),
        "swift_package" => Ok(ProjectSignalKind::SwiftPackage),
        "xcode_project" => Ok(ProjectSignalKind::XcodeProject),
        "visual_studio_solution" => Ok(ProjectSignalKind::VisualStudioSolution),
        "readme" => Ok(ProjectSignalKind::Readme),
        _ => Err(DatabaseError::InvalidStoredValue),
    }
}

fn project_decision_kind_str(decision: ProjectDecisionKind) -> &'static str {
    match decision {
        ProjectDecisionKind::Accepted => "accepted",
        ProjectDecisionKind::Rejected => "rejected",
    }
}

fn project_decision_kind_from_str(stored: &str) -> Result<ProjectDecisionKind, DatabaseError> {
    match stored {
        "accepted" => Ok(ProjectDecisionKind::Accepted),
        "rejected" => Ok(ProjectDecisionKind::Rejected),
        _ => Err(DatabaseError::InvalidStoredValue),
    }
}

fn file_relation_decision_kind_str(decision: FileRelationDecisionKind) -> &'static str {
    match decision {
        FileRelationDecisionKind::Accepted => "accepted",
        FileRelationDecisionKind::Rejected => "rejected",
    }
}

fn file_relation_decision_kind_from_str(
    stored: &str,
) -> Result<FileRelationDecisionKind, DatabaseError> {
    match stored {
        "accepted" => Ok(FileRelationDecisionKind::Accepted),
        "rejected" => Ok(FileRelationDecisionKind::Rejected),
        _ => Err(DatabaseError::InvalidStoredValue),
    }
}

fn file_category(path: &Path) -> FolderFileCategory {
    let extension = path
        .extension()
        .and_then(|extension| extension.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    match extension.as_str() {
        "txt" | "md" | "markdown" | "pdf" | "docx" | "pptx" | "xlsx" | "rtf" => {
            FolderFileCategory::Document
        }
        "rs" | "ts" | "tsx" | "js" | "jsx" | "py" | "go" | "java" | "kt" | "kts" | "swift"
        | "c" | "cc" | "cpp" | "h" | "hpp" | "cs" | "rb" | "php" | "sh" | "zsh" | "fish"
        | "toml" | "yaml" | "yml" | "json" | "xml" | "html" | "css" | "sql" => {
            FolderFileCategory::Code
        }
        "png" | "jpg" | "jpeg" | "gif" | "webp" | "heic" | "tiff" | "bmp" | "svg" => {
            FolderFileCategory::Image
        }
        "csv" | "tsv" | "parquet" | "sqlite" | "db" => FolderFileCategory::Data,
        "zip" | "tar" | "gz" | "tgz" | "7z" | "rar" => FolderFileCategory::Archive,
        "mp3" | "wav" | "m4a" | "flac" | "mp4" | "mov" | "mkv" | "avi" => FolderFileCategory::Media,
        _ => FolderFileCategory::Other,
    }
}

fn folder_category_index(category: FolderFileCategory) -> usize {
    match category {
        FolderFileCategory::Document => 0,
        FolderFileCategory::Code => 1,
        FolderFileCategory::Image => 2,
        FolderFileCategory::Data => 3,
        FolderFileCategory::Archive => 4,
        FolderFileCategory::Media => 5,
        FolderFileCategory::Other => 6,
    }
}

fn project_marker(path: &Path, kind: NodeKind) -> Option<ProjectSignalKind> {
    let name = path.file_name()?.to_string_lossy().to_ascii_lowercase();
    match (kind, name.as_str()) {
        (NodeKind::File, "cargo.toml") => Some(ProjectSignalKind::CargoManifest),
        (NodeKind::File, "package.json") => Some(ProjectSignalKind::JavaScriptPackage),
        (NodeKind::File, "pyproject.toml") => Some(ProjectSignalKind::PythonProject),
        (NodeKind::File, "go.mod") => Some(ProjectSignalKind::GoModule),
        (NodeKind::File, "package.swift") => Some(ProjectSignalKind::SwiftPackage),
        (NodeKind::File, name) if name.ends_with(".sln") => {
            Some(ProjectSignalKind::VisualStudioSolution)
        }
        (NodeKind::File, "readme" | "readme.md" | "readme.txt" | "readme.rst") => {
            Some(ProjectSignalKind::Readme)
        }
        (NodeKind::Folder, name) if name.ends_with(".xcodeproj") => {
            Some(ProjectSignalKind::XcodeProject)
        }
        _ => None,
    }
}

fn action_execution_strategy_str(strategy: ActionExecutionStrategy) -> &'static str {
    match strategy {
        ActionExecutionStrategy::Direct => "direct",
        ActionExecutionStrategy::CaseOnlyStaged => "case_only_staged",
    }
}

fn action_execution_strategy_from_str(
    stored: &str,
) -> Result<ActionExecutionStrategy, rusqlite::Error> {
    match stored {
        "direct" => Ok(ActionExecutionStrategy::Direct),
        "case_only_staged" => Ok(ActionExecutionStrategy::CaseOnlyStaged),
        _ => Err(rusqlite::Error::InvalidQuery),
    }
}

fn action_operation_from_str(stored: &str) -> Result<ActionOperation, rusqlite::Error> {
    match stored {
        "rename" => Ok(ActionOperation::Rename),
        _ => Err(rusqlite::Error::InvalidQuery),
    }
}

fn action_plan_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<ActionPlanPreview> {
    let operation = action_operation_from_str(&row.get::<_, String>(1)?)?;
    let execution_strategy = action_execution_strategy_from_str(&row.get::<_, String>(6)?)?;
    let journal_sequence = row_u64(row, 8)?;
    if journal_sequence != 1 {
        return Err(rusqlite::Error::InvalidQuery);
    }
    Ok(ActionPlanPreview {
        api_version: ActionPlanPreview::API_VERSION,
        plan_id: row.get(0)?,
        operation,
        state: ActionPlanState::Previewed,
        scope_id: row.get(2)?,
        node_id: row.get(3)?,
        source_path: row.get(4)?,
        destination_path: row.get(5)?,
        execution_strategy,
        policy: ActionPolicyReport::rename_allowed(),
        journal_sequence,
        created_at_unix_ms: row.get(7)?,
    })
}

fn action_plan_summary_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<ActionPlanSummary> {
    let operation = action_operation_from_str(&row.get::<_, String>(1)?)?;
    let execution_strategy = action_execution_strategy_from_str(&row.get::<_, String>(4)?)?;
    let journal_sequence = row_u64(row, 6)?;
    if journal_sequence != 1 {
        return Err(rusqlite::Error::InvalidQuery);
    }
    Ok(ActionPlanSummary {
        api_version: ActionPlanSummary::API_VERSION,
        plan_id: row.get(0)?,
        operation,
        state: ActionPlanState::Previewed,
        scope_id: row.get(2)?,
        node_id: row.get(3)?,
        execution_strategy,
        journal_sequence,
        created_at_unix_ms: row.get(5)?,
    })
}

fn watch_event_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<WatchEventRecord> {
    let stored_status: String = row.get(2)?;
    let status = match stored_status.as_str() {
        "stabilizing" => WatchEventStatus::Stabilizing,
        "reconciling" => WatchEventStatus::Reconciling,
        "completed" => WatchEventStatus::Completed,
        "ignored" => WatchEventStatus::Ignored,
        "failed" => WatchEventStatus::Failed,
        _ => return Err(rusqlite::Error::InvalidQuery),
    };
    let stored_reason: Option<String> = row.get(6)?;
    let reason = stored_reason
        .as_deref()
        .map(watch_reason_from_str)
        .transpose()
        .map_err(|_| rusqlite::Error::InvalidQuery)?;
    let stored_kind: String = row.get(9)?;
    let kind =
        WatchSnapshotKind::from_db(&stored_kind).map_err(|_| rusqlite::Error::InvalidQuery)?;
    let stored_size: Option<i64> = row.get(10)?;
    let size_bytes = stored_size
        .map(|value| {
            u64::try_from(value).map_err(|_| rusqlite::Error::IntegralValueOutOfRange(10, value))
        })
        .transpose()?;
    Ok(WatchEventRecord {
        progress: WatchEventProgress {
            api_version: WatchEventProgress::API_VERSION,
            event_id: row.get(0)?,
            scope_id: row.get(1)?,
            status,
            observation_count: row_u64(row, 3)?,
            stable_after_unix_ms: row.get(4)?,
            scan_job_id: row.get(5)?,
            reason,
        },
        path_raw: row.get(7)?,
        path_key: row.get(8)?,
        snapshot: WatchSnapshot {
            kind,
            size_bytes,
            modified_unix_ns: row.get(11)?,
            identity_key: row.get(12)?,
        },
    })
}

fn scan_job_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<ScanJobProgress> {
    let stored_status: String = row.get(2)?;
    let control_state: String = row.get(3)?;
    let status = match (stored_status.as_str(), control_state.as_str()) {
        ("running", "paused") => ScanStatus::Paused,
        ("running", "ready" | "active" | "pause_requested") => ScanStatus::Running,
        ("completed", _) => ScanStatus::Completed,
        ("failed", _) => ScanStatus::Failed,
        ("interrupted", _) => ScanStatus::Interrupted,
        _ => return Err(rusqlite::Error::InvalidQuery),
    };
    Ok(ScanJobProgress {
        api_version: ScanJobProgress::API_VERSION,
        job_id: row.get(0)?,
        scope_id: row.get(1)?,
        status,
        queued_entries: row_u64(row, 4)?,
        processed_entries: row_u64(row, 5)?,
        discovered_files: row_u64(row, 6)?,
        discovered_folders: row_u64(row, 7)?,
        skipped_entries: row_u64(row, 8)?,
        issue_count: row_u64(row, 9)?,
        elapsed_ms: row_u64(row, 10)?,
        pause_requested: row.get::<_, i64>(11)? != 0,
    })
}

fn validate_watch_observation(
    observation: &WatchObservationWrite<'_>,
) -> Result<(), DatabaseError> {
    if observation.scope_id <= 0
        || observation.path_raw.is_empty()
        || observation.path_raw.len() > MAX_WATCH_PATH_BYTES
        || observation.path_key.is_empty()
        || observation.path_key.len() > MAX_WATCH_PATH_BYTES
        || observation.observed_at_unix_ms < 0
        || observation.stable_after_unix_ms < observation.observed_at_unix_ms
        || observation
            .snapshot
            .identity_key
            .as_ref()
            .is_some_and(|identity| identity.is_empty() || identity.len() > 4096)
    {
        return Err(DatabaseError::WatchInputInvalid);
    }
    let snapshot_valid = match observation.snapshot.kind {
        WatchSnapshotKind::Missing => {
            observation.snapshot.size_bytes.is_none()
                && observation.snapshot.modified_unix_ns.is_none()
                && observation.snapshot.identity_key.is_none()
        }
        WatchSnapshotKind::File => {
            observation.snapshot.size_bytes.is_some() && observation.snapshot.identity_key.is_some()
        }
        WatchSnapshotKind::Folder => {
            observation.snapshot.size_bytes.is_none() && observation.snapshot.identity_key.is_some()
        }
    };
    if snapshot_valid {
        Ok(())
    } else {
        Err(DatabaseError::WatchInputInvalid)
    }
}

fn insert_watch_event(
    transaction: &Transaction<'_>,
    observation: WatchObservationWrite<'_>,
    status: &str,
    reason: Option<&str>,
    size_bytes: Option<i64>,
) -> Result<i64, DatabaseError> {
    transaction.execute(
        "INSERT INTO watch_events( \
            scope_id, status, path_raw, path_key, observed_kind, observed_size_bytes, \
            observed_modified_unix_ns, observed_identity_key, observation_count, \
            stable_after_unix_ms, reason, created_at_unix_ms, updated_at_unix_ms \
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, 1, ?9, ?10, ?11, ?11)",
        params![
            observation.scope_id,
            status,
            observation.path_raw,
            observation.path_key,
            observation.snapshot.kind.as_str(),
            size_bytes,
            observation.snapshot.modified_unix_ns,
            observation.snapshot.identity_key,
            observation.stable_after_unix_ms,
            reason,
            observation.observed_at_unix_ms,
        ],
    )?;
    Ok(transaction.last_insert_rowid())
}

fn insert_resumable_scan_job(
    transaction: &Transaction<'_>,
    scope_id: i64,
    root: &QueuedPath,
    now: i64,
) -> Result<i64, DatabaseError> {
    let scope_exists: i64 = transaction.query_row(
        "SELECT COUNT(*) FROM authorized_scopes WHERE id = ?1",
        [scope_id],
        |row| row.get(0),
    )?;
    if scope_exists != 1 {
        return Err(DatabaseError::ScopeNotFound);
    }
    let active_jobs: i64 = transaction.query_row(
        "SELECT COUNT(*) FROM scan_jobs WHERE scope_id = ?1 AND status IN ('running', 'interrupted')",
        [scope_id],
        |row| row.get(0),
    )?;
    if active_jobs != 0 {
        return Err(DatabaseError::ScanJobAlreadyActive);
    }
    transaction.execute(
        "INSERT INTO scan_jobs( \
            scope_id, status, control_state, queued_entries, processed_entries, \
            started_at_unix_ms, updated_at_unix_ms \
         ) VALUES (?1, 'running', 'ready', 1, 0, ?2, ?2)",
        params![scope_id, now],
    )?;
    let job_id = transaction.last_insert_rowid();
    transaction.execute(
        "INSERT INTO scan_queue( \
            scan_id, path_raw, path_key, parent_identity_key, is_root, state \
         ) VALUES (?1, ?2, ?3, ?4, ?5, 'pending')",
        params![
            job_id,
            root.path_raw,
            root.path_key,
            root.parent_identity_key,
            i64::from(root.is_root),
        ],
    )?;
    Ok(job_id)
}

fn watch_reason_as_str(reason: WatchEventReason) -> &'static str {
    match reason {
        WatchEventReason::TemporaryDownload => "temporary_download",
        WatchEventReason::HiddenEntry => "hidden_entry",
        WatchEventReason::UnsupportedEntry => "unsupported_entry",
        WatchEventReason::SourceUnavailable => "source_unavailable",
        WatchEventReason::ReconcileFailed => "reconcile_failed",
    }
}

fn watch_reason_from_str(value: &str) -> Result<WatchEventReason, DatabaseError> {
    match value {
        "temporary_download" => Ok(WatchEventReason::TemporaryDownload),
        "hidden_entry" => Ok(WatchEventReason::HiddenEntry),
        "unsupported_entry" => Ok(WatchEventReason::UnsupportedEntry),
        "source_unavailable" => Ok(WatchEventReason::SourceUnavailable),
        "reconcile_failed" => Ok(WatchEventReason::ReconcileFailed),
        _ => Err(DatabaseError::InvalidStoredValue),
    }
}

fn extraction_job_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<ExtractionJobProgress> {
    let stored_operation: String = row.get(3)?;
    let operation = ExtractionOperation::from_storage(&stored_operation)
        .ok_or(rusqlite::Error::InvalidQuery)?;
    let stored_status: String = row.get(4)?;
    let status = match stored_status.as_str() {
        "queued" => ExtractionStatus::Queued,
        "running" => ExtractionStatus::Running,
        "completed" => ExtractionStatus::Completed,
        "failed" => ExtractionStatus::Failed,
        "cancelled" => ExtractionStatus::Cancelled,
        "interrupted" => ExtractionStatus::Interrupted,
        _ => return Err(rusqlite::Error::InvalidQuery),
    };
    Ok(ExtractionJobProgress {
        api_version: ExtractionJobProgress::API_VERSION,
        job_id: row.get(0)?,
        scope_id: row.get(1)?,
        node_id: row.get(2)?,
        operation,
        status,
        provider_id: row.get(5)?,
        provider_version: row.get(6)?,
        error_code: row.get(7)?,
        source_bytes: row_u64(row, 8)?,
        output_bytes: row_u64(row, 9)?,
        chunk_count: row_u64(row, 10)?,
        elapsed_ms: row_u64(row, 11)?,
        cancel_requested: row.get::<_, i64>(12)? != 0,
    })
}

fn row_u64(row: &rusqlite::Row<'_>, index: usize) -> rusqlite::Result<u64> {
    let value: i64 = row.get(index)?;
    u64::try_from(value).map_err(|_| rusqlite::Error::IntegralValueOutOfRange(index, value))
}

fn row_u64_value(value: i64) -> Result<u64, DatabaseError> {
    u64::try_from(value).map_err(|_| DatabaseError::InvalidStoredValue)
}

fn ensure_active_runner(
    transaction: &Transaction<'_>,
    job_id: i64,
    runner_token: &str,
    now: i64,
) -> Result<(), DatabaseError> {
    let owned: i64 = transaction.query_row(
        "SELECT COUNT(*) FROM scan_jobs WHERE id = ?1 AND status = 'running' \
            AND control_state = 'active' AND runner_token = ?2 \
            AND lease_expires_at_unix_ms IS NOT NULL AND lease_expires_at_unix_ms > ?3",
        params![job_id, runner_token, now],
        |row| row.get(0),
    )?;
    if owned == 1 {
        Ok(())
    } else {
        Err(DatabaseError::RunnerLeaseLost)
    }
}

fn ensure_owned_runner(
    transaction: &Transaction<'_>,
    job_id: i64,
    runner_token: &str,
    now: i64,
) -> Result<(), DatabaseError> {
    let owned: i64 = transaction.query_row(
        "SELECT COUNT(*) FROM scan_jobs WHERE id = ?1 AND status = 'running' \
            AND control_state IN ('active', 'pause_requested') AND runner_token = ?2 \
            AND lease_expires_at_unix_ms IS NOT NULL AND lease_expires_at_unix_ms > ?3",
        params![job_id, runner_token, now],
        |row| row.get(0),
    )?;
    if owned == 1 {
        Ok(())
    } else {
        Err(DatabaseError::RunnerLeaseLost)
    }
}

fn ensure_extraction_runner(
    transaction: &Transaction<'_>,
    job_id: i64,
    runner_token: &str,
    now: i64,
) -> Result<(), DatabaseError> {
    let owned: i64 = transaction.query_row(
        "SELECT COUNT(*) FROM extraction_jobs WHERE id = ?1 AND status = 'running' \
            AND runner_token = ?2 AND lease_expires_at_unix_ms IS NOT NULL \
            AND lease_expires_at_unix_ms > ?3",
        params![job_id, runner_token, now],
        |row| row.get(0),
    )?;
    if owned == 1 {
        Ok(())
    } else {
        Err(DatabaseError::ExtractionRunnerLeaseLost)
    }
}

fn invalidate_stale_extraction_outputs(
    transaction: &Transaction<'_>,
    scope_id: i64,
) -> Result<(), DatabaseError> {
    transaction.execute(
        "UPDATE content_chunks SET active = 0 \
         WHERE active = 1 AND scope_id = ?1 AND ( \
            NOT EXISTS ( \
                SELECT 1 FROM locations l \
                WHERE l.id = content_chunks.location_id AND l.present = 1 \
            ) OR NOT EXISTS ( \
                SELECT 1 FROM files f \
                WHERE f.node_id = content_chunks.node_id \
                  AND f.size_bytes = content_chunks.source_size_bytes \
                  AND f.modified_unix_ns IS content_chunks.source_modified_unix_ns \
            ) \
         )",
        [scope_id],
    )?;
    transaction.execute(
        "UPDATE image_metadata SET active = 0 \
         WHERE active = 1 AND scope_id = ?1 AND ( \
            NOT EXISTS ( \
                SELECT 1 FROM locations l \
                WHERE l.id = image_metadata.location_id AND l.present = 1 \
            ) OR NOT EXISTS ( \
                SELECT 1 FROM files f \
                WHERE f.node_id = image_metadata.node_id \
                  AND f.size_bytes = image_metadata.source_size_bytes \
                  AND f.modified_unix_ns IS image_metadata.source_modified_unix_ns \
            ) \
         )",
        [scope_id],
    )?;
    Ok(())
}

fn upsert_observation(
    transaction: &Transaction<'_>,
    scope_id: i64,
    job_id: i64,
    observation: &Observation,
    timestamp: i64,
) -> Result<(), DatabaseError> {
    transaction.execute(
        "INSERT INTO nodes(kind, identity_kind, identity_key, created_at_unix_ms, updated_at_unix_ms)\
         VALUES (?1, ?2, ?3, ?4, ?4)\
         ON CONFLICT(identity_key) DO UPDATE SET kind = excluded.kind, identity_kind = excluded.identity_kind, updated_at_unix_ms = excluded.updated_at_unix_ms",
        params![
            observation.kind.as_str(),
            observation.identity_kind,
            observation.identity_key,
            timestamp,
        ],
    )?;
    let node_id: i64 = transaction.query_row(
        "SELECT id FROM nodes WHERE identity_key = ?1",
        [&observation.identity_key],
        |row| row.get(0),
    )?;

    transaction.execute(
        "INSERT INTO locations(scope_id, node_id, path_raw, path_key, display_path, present, last_seen_scan_id)\
         VALUES (?1, ?2, ?3, ?4, ?5, 1, ?6)\
         ON CONFLICT(scope_id, path_key) DO UPDATE SET node_id = excluded.node_id, path_raw = excluded.path_raw,\
            display_path = excluded.display_path, present = 1, last_seen_scan_id = excluded.last_seen_scan_id",
        params![
            scope_id,
            node_id,
            observation.path_raw,
            observation.path_key,
            observation.display_path,
            job_id,
        ],
    )?;

    match observation.kind {
        NodeKind::File => {
            transaction.execute(
                "INSERT INTO files(node_id, size_bytes, modified_unix_ns, link_count) VALUES (?1, ?2, ?3, ?4)\
                 ON CONFLICT(node_id) DO UPDATE SET size_bytes = excluded.size_bytes, modified_unix_ns = excluded.modified_unix_ns, link_count = excluded.link_count",
                params![
                    node_id,
                    to_i64(observation.size_bytes)?,
                    observation.modified_unix_ns,
                    observation.link_count.map(to_i64).transpose()?,
                ],
            )?;
        }
        NodeKind::Folder => {
            transaction.execute(
                "INSERT INTO folders(node_id) VALUES (?1) ON CONFLICT(node_id) DO NOTHING",
                [node_id],
            )?;
        }
    }

    if let Some(parent_identity_key) = &observation.parent_identity_key {
        let parent_node_id: i64 = transaction.query_row(
            "SELECT id FROM nodes WHERE identity_key = ?1",
            [parent_identity_key],
            |row| row.get(0),
        )?;
        transaction.execute(
            "INSERT INTO edges(scope_id, source_node_id, target_node_id, kind, active, last_seen_scan_id)\
             VALUES (?1, ?2, ?3, 'located_in', 1, ?4)\
             ON CONFLICT(scope_id, source_node_id, target_node_id, kind) DO UPDATE SET active = 1, last_seen_scan_id = excluded.last_seen_scan_id",
            params![scope_id, node_id, parent_node_id, job_id],
        )?;
    }

    Ok(())
}

fn count(connection: &Connection, sql: &str) -> Result<u64, DatabaseError> {
    let result = connection
        .query_row(sql, [], |row| row.get::<_, i64>(0))
        .optional()?;
    u64::try_from(result.unwrap_or(0)).map_err(|_| DatabaseError::InvalidCount)
}

fn to_i64(value: u64) -> Result<i64, DatabaseError> {
    i64::try_from(value).map_err(|_| DatabaseError::InvalidCount)
}

fn unix_ms() -> Result<i64, DatabaseError> {
    let duration = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| DatabaseError::InvalidTimestamp)?;
    i64::try_from(duration.as_millis()).map_err(|_| DatabaseError::InvalidTimestamp)
}

fn migration_checksum(sql: &str) -> String {
    let mut hash = 0xcbf29ce484222325_u64;
    for byte in sql.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    format!("fnv1a64:{hash:016x}")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lexical_filters(scope_id: Option<i64>) -> LexicalSearchFilters<'static> {
        LexicalSearchFilters {
            scope_id,
            source: LexicalSearchSource::All,
            extension: None,
            modified_since_unix_ns: None,
            modified_before_unix_ns: None,
        }
    }

    fn resumable_setup() -> (ManifestDatabase, i64, QueuedPath) {
        let database = ManifestDatabase::open_in_memory().expect("database should initialize");
        let scope = database
            .add_scope(b"/scope", "/scope", "/scope", "test")
            .expect("scope should persist");
        let root = QueuedPath {
            path_raw: b"/scope".to_vec(),
            path_key: "/scope".to_string(),
            parent_identity_key: None,
            is_root: true,
        };
        (database, scope.id, root)
    }

    fn observation(path: &str, kind: NodeKind, parent: Option<Vec<u8>>) -> Observation {
        Observation {
            kind,
            identity_kind: "test_identity".to_string(),
            identity_key: format!("identity:{path}").into_bytes(),
            parent_identity_key: parent,
            path_raw: path.as_bytes().to_vec(),
            path_key: path.to_string(),
            display_path: path.to_string(),
            size_bytes: if kind == NodeKind::File { 4 } else { 0 },
            modified_unix_ns: Some(1),
            link_count: Some(1),
        }
    }

    fn publish_manifest_file(
        database: &mut ManifestDatabase,
        scope_id: i64,
        root: &QueuedPath,
        file_size: u64,
    ) -> i64 {
        let job = database
            .create_resumable_scan_job(scope_id, root)
            .expect("scan job should create");
        database
            .claim_scan_job(job.job_id, "scan-runner", 60_000)
            .expect("scan should claim");
        let root_entry = database
            .next_scan_queue_entry(job.job_id, "scan-runner", 60_000)
            .expect("queue should load")
            .expect("root should exist");
        let root_observation = observation("/scope", NodeKind::Folder, None);
        let child = QueuedPath {
            path_raw: b"/scope/file.txt".to_vec(),
            path_key: "/scope/file.txt".to_string(),
            parent_identity_key: Some(root_observation.identity_key.clone()),
            is_root: false,
        };
        database
            .stage_scan_queue_entry(
                job.job_id,
                "scan-runner",
                root_entry.id,
                Some(&root_observation),
                std::slice::from_ref(&child),
                &[],
                0,
                1,
                60_000,
            )
            .expect("root should stage");
        let child_entry = database
            .next_scan_queue_entry(job.job_id, "scan-runner", 60_000)
            .expect("queue should load")
            .expect("child should exist");
        let mut child_observation = observation(
            "/scope/file.txt",
            NodeKind::File,
            Some(root_observation.identity_key),
        );
        child_observation.size_bytes = file_size;
        database
            .stage_scan_queue_entry(
                job.job_id,
                "scan-runner",
                child_entry.id,
                Some(&child_observation),
                &[],
                &[],
                0,
                1,
                60_000,
            )
            .expect("child should stage");
        database
            .finalize_resumable_scan_job(job.job_id, "scan-runner")
            .expect("scan should publish");
        database
            .node_id_for_path_key(scope_id, "/scope/file.txt")
            .expect("node query should pass")
            .expect("file node should exist")
    }

    fn extraction_setup() -> (ManifestDatabase, i64, i64, QueuedPath) {
        let (mut database, scope_id, root) = resumable_setup();
        let node_id = publish_manifest_file(&mut database, scope_id, &root, 4);
        (database, scope_id, node_id, root)
    }

    fn project_setup() -> (ManifestDatabase, i64, i64) {
        let (mut database, scope_id, root) = resumable_setup();
        let job = database
            .create_resumable_scan_job(scope_id, &root)
            .expect("scan job should create");
        database
            .claim_scan_job(job.job_id, "project-scan", 60_000)
            .expect("scan should claim");
        let root_entry = database
            .next_scan_queue_entry(job.job_id, "project-scan", 60_000)
            .expect("queue should load")
            .expect("root should exist");
        let root_observation = observation("/scope", NodeKind::Folder, None);
        let child = QueuedPath {
            path_raw: b"/scope/Cargo.toml".to_vec(),
            path_key: "/scope/Cargo.toml".to_string(),
            parent_identity_key: Some(root_observation.identity_key.clone()),
            is_root: false,
        };
        database
            .stage_scan_queue_entry(
                job.job_id,
                "project-scan",
                root_entry.id,
                Some(&root_observation),
                std::slice::from_ref(&child),
                &[],
                0,
                1,
                60_000,
            )
            .expect("root should stage");
        let child_entry = database
            .next_scan_queue_entry(job.job_id, "project-scan", 60_000)
            .expect("queue should load")
            .expect("child should exist");
        let cargo_observation = observation(
            "/scope/Cargo.toml",
            NodeKind::File,
            Some(root_observation.identity_key),
        );
        database
            .stage_scan_queue_entry(
                job.job_id,
                "project-scan",
                child_entry.id,
                Some(&cargo_observation),
                &[],
                &[],
                0,
                1,
                60_000,
            )
            .expect("marker should stage");
        database
            .finalize_resumable_scan_job(job.job_id, "project-scan")
            .expect("scan should publish");
        let root_node_id = database
            .node_id_for_path_key(scope_id, "/scope")
            .expect("node query should pass")
            .expect("root node should exist");
        (database, scope_id, root_node_id)
    }

    fn exact_duplicate_setup() -> (ManifestDatabase, ActionSourceRecord, ActionSourceRecord) {
        let database = ManifestDatabase::open_in_memory().expect("database should initialize");
        let scope = database
            .add_scope(b"/scope", "/scope", "/scope", "test")
            .expect("scope should persist");
        database
            .connection
            .execute(
                "INSERT INTO scan_jobs( \
                     scope_id, status, discovered_files, discovered_folders, started_at_unix_ms, \
                     finished_at_unix_ms \
                 ) VALUES (?1, 'completed', 2, 0, 1, 1)",
                [scope.id],
            )
            .expect("scan should persist");
        let scan_id = database.connection.last_insert_rowid();
        for (path, identity) in [
            ("/scope/left.bin", b"identity:left".as_slice()),
            ("/scope/right.bin", b"identity:right".as_slice()),
        ] {
            database
                .connection
                .execute(
                    "INSERT INTO nodes( \
                         kind, identity_kind, identity_key, created_at_unix_ms, updated_at_unix_ms \
                     ) VALUES ('file', 'test_identity', ?1, 1, 1)",
                    [identity],
                )
                .expect("node should persist");
            let node_id = database.connection.last_insert_rowid();
            database
                .connection
                .execute(
                    "INSERT INTO files(node_id, size_bytes, modified_unix_ns, link_count) \
                     VALUES (?1, 4, 2, 1)",
                    [node_id],
                )
                .expect("file should persist");
            database
                .connection
                .execute(
                    "INSERT INTO locations( \
                         scope_id, node_id, path_raw, path_key, display_path, present, \
                         last_seen_scan_id \
                     ) VALUES (?1, ?2, ?3, ?4, ?4, 1, ?5)",
                    params![scope.id, node_id, path.as_bytes(), path, scan_id],
                )
                .expect("location should persist");
        }
        let left = database
            .action_source_for_path_key(scope.id, "/scope/left.bin")
            .expect("left should load");
        let right = database
            .action_source_for_path_key(scope.id, "/scope/right.bin")
            .expect("right should load");
        (database, left, right)
    }

    fn file_version_setup() -> (ManifestDatabase, ActionSourceRecord, ActionSourceRecord) {
        let database = ManifestDatabase::open_in_memory().expect("database should initialize");
        let scope = database
            .add_scope(b"/scope", "/scope", "/scope", "test")
            .expect("scope should persist");
        database
            .connection
            .execute(
                "INSERT INTO scan_jobs( \
                     scope_id, status, discovered_files, discovered_folders, started_at_unix_ms, \
                     finished_at_unix_ms \
                 ) VALUES (?1, 'completed', 2, 0, 1, 1)",
                [scope.id],
            )
            .expect("scan should persist");
        let scan_id = database.connection.last_insert_rowid();
        for (path, identity, size) in [
            ("/scope/企劃-v1.md", b"identity:v1".as_slice(), 4_i64),
            ("/scope/企劃_V2.MD", b"identity:v2".as_slice(), 6_i64),
        ] {
            database
                .connection
                .execute(
                    "INSERT INTO nodes( \
                         kind, identity_kind, identity_key, created_at_unix_ms, updated_at_unix_ms \
                     ) VALUES ('file', 'test_identity', ?1, 1, 1)",
                    [identity],
                )
                .expect("node should persist");
            let node_id = database.connection.last_insert_rowid();
            database
                .connection
                .execute(
                    "INSERT INTO files(node_id, size_bytes, modified_unix_ns, link_count) \
                     VALUES (?1, ?2, 2, 1)",
                    params![node_id, size],
                )
                .expect("file should persist");
            database
                .connection
                .execute(
                    "INSERT INTO locations( \
                         scope_id, node_id, path_raw, path_key, display_path, present, \
                         last_seen_scan_id \
                     ) VALUES (?1, ?2, ?3, ?4, ?4, 1, ?5)",
                    params![scope.id, node_id, path.as_bytes(), path, scan_id],
                )
                .expect("location should persist");
        }
        let first = database
            .action_source_for_path_key(scope.id, "/scope/企劃-v1.md")
            .expect("first version should load");
        let second = database
            .action_source_for_path_key(scope.id, "/scope/企劃_V2.MD")
            .expect("second version should load");
        (database, first, second)
    }

    #[test]
    fn migrations_initialize_manifest_schema() {
        let database = ManifestDatabase::open_in_memory().expect("database should initialize");
        let stats = database.stats().expect("stats should be readable");

        assert!(stats.database_ready);
        assert_eq!(stats.authorized_scope_count, 0);
        assert_eq!(stats.node_count, 0);
        assert_eq!(
            database
                .connection
                .query_row("SELECT COUNT(*) FROM schema_migrations", [], |row| {
                    row.get::<_, i64>(0)
                })
                .expect("migration count should load"),
            i64::try_from(MIGRATIONS.len()).expect("migration count should fit")
        );
        assert_eq!(
            database
                .connection
                .query_row(
                    "SELECT COUNT(*) FROM sqlite_schema \
                     WHERE (type = 'table' AND name = 'file_version_feedback_events') \
                        OR (type = 'trigger' AND name IN ( \
                            'file_version_feedback_events_immutable_update', \
                            'file_version_feedback_events_immutable_delete' \
                        ))",
                    [],
                    |row| row.get::<_, i64>(0),
                )
                .expect("version feedback schema should load"),
            3
        );
    }

    #[test]
    fn changed_applied_migration_is_rejected() {
        let connection = Connection::open_in_memory().expect("connection should open");
        connection
            .execute_batch(
                "CREATE TABLE schema_migrations (version INTEGER PRIMARY KEY, name TEXT NOT NULL, checksum TEXT NOT NULL, applied_at_unix_ms INTEGER NOT NULL);\
                 INSERT INTO schema_migrations VALUES (1, 'manifest', 'wrong', 0);",
            )
            .expect("fixture should initialize");
        let error = ManifestDatabase::from_connection(connection)
            .err()
            .expect("changed migration must fail");

        assert!(matches!(
            error,
            DatabaseError::MigrationChanged { version: 1 }
        ));
    }

    #[test]
    fn file_backed_database_reopens_without_duplicate_migrations() {
        let directory = tempfile::tempdir().expect("tempdir should exist");
        let path = directory.path().join("manifest.sqlite3");
        ManifestDatabase::open(&path).expect("first open should initialize");
        let database = ManifestDatabase::open(&path).expect("second open should be idempotent");

        assert_eq!(
            database
                .stats()
                .expect("stats should load")
                .completed_scan_count,
            0
        );
    }

    #[test]
    fn image_metadata_migration_preserves_existing_chunks_and_fts() {
        let directory = tempfile::tempdir().expect("tempdir should exist");
        let path = directory.path().join("pre-image-metadata.sqlite3");
        let connection = Connection::open(&path).expect("legacy database should open");
        connection
            .execute_batch(
                "CREATE TABLE schema_migrations (version INTEGER PRIMARY KEY, name TEXT NOT NULL, checksum TEXT NOT NULL, applied_at_unix_ms INTEGER NOT NULL);",
            )
            .expect("migration registry should initialize");
        for migration in &MIGRATIONS[..13] {
            connection
                .execute_batch(migration.sql)
                .expect("historical migration should apply");
            connection
                .execute(
                    "INSERT INTO schema_migrations(version, name, checksum, applied_at_unix_ms) VALUES (?1, ?2, ?3, 0)",
                    params![
                        migration.version,
                        migration.name,
                        migration_checksum(migration.sql)
                    ],
                )
                .expect("historical migration should register");
        }
        connection
            .execute_batch(
                "INSERT INTO authorized_scopes VALUES (1, X'2F73636F7065', '/scope', '/scope', 'test', 0); \
                 INSERT INTO scan_jobs(id, scope_id, status, discovered_files, discovered_folders, started_at_unix_ms, finished_at_unix_ms) VALUES (1, 1, 'completed', 1, 0, 0, 0); \
                 INSERT INTO nodes VALUES (1, 'file', 'test', X'01', 0, 0); \
                 INSERT INTO files VALUES (1, 4, 1, 1); \
                 INSERT INTO locations VALUES (1, 1, 1, X'2F73636F70652F66696C652E747874', '/scope/file.txt', '/scope/file.txt', 1, 1); \
                 INSERT INTO extraction_jobs(id, scope_id, node_id, location_id, status, source_size_bytes, source_modified_unix_ns, output_bytes, chunk_count, elapsed_ms, created_at_unix_ms, updated_at_unix_ms) VALUES (1, 1, 1, 1, 'completed', 4, 1, 4, 1, 1, 0, 0); \
                 INSERT INTO content_chunks(id, scope_id, node_id, location_id, extraction_job_id, ordinal, text, provenance_kind, source_byte_start, source_byte_end, source_size_bytes, source_modified_unix_ns, trust_class, provider_id, provider_version, active, created_at_unix_ms) VALUES (1, 1, 1, 1, 1, 0, '保留text', 'byte_range', 0, 4, 4, 1, 'untrusted_extracted_text', 'deskgraph.utf8-text', '1', 1, 0);",
            )
            .expect("legacy content should persist");
        drop(connection);

        let upgraded =
            ManifestDatabase::open(&path).expect("image metadata migration should apply");
        assert_eq!(
            upgraded
                .connection
                .query_row("SELECT text FROM content_chunks WHERE id = 1", [], |row| {
                    row.get::<_, String>(0)
                })
                .expect("legacy chunk should load"),
            "保留text"
        );
        assert_eq!(
            upgraded
                .lexical_search_candidates("保留t", lexical_filters(Some(1)), 10)
                .expect("legacy FTS row should remain searchable")
                .len(),
            1
        );
        assert_eq!(
            upgraded
                .connection
                .query_row("SELECT COUNT(*) FROM image_metadata", [], |row| row
                    .get::<_, i64>(0))
                .expect("new metadata table should exist"),
            0
        );
    }

    #[test]
    fn ocr_migration_preserves_content_metadata_and_search() {
        let directory = tempfile::tempdir().expect("tempdir should exist");
        let path = directory.path().join("pre-ocr.sqlite3");
        let connection = Connection::open(&path).expect("legacy database should open");
        connection
            .execute_batch(
                "CREATE TABLE schema_migrations (version INTEGER PRIMARY KEY, name TEXT NOT NULL, checksum TEXT NOT NULL, applied_at_unix_ms INTEGER NOT NULL);",
            )
            .expect("migration registry should initialize");
        for migration in &MIGRATIONS[..14] {
            connection
                .execute_batch(migration.sql)
                .expect("historical migration should apply");
            connection
                .execute(
                    "INSERT INTO schema_migrations(version, name, checksum, applied_at_unix_ms) VALUES (?1, ?2, ?3, 0)",
                    params![
                        migration.version,
                        migration.name,
                        migration_checksum(migration.sql)
                    ],
                )
                .expect("historical migration should register");
        }
        connection
            .execute_batch(
                "INSERT INTO authorized_scopes VALUES (1, X'2F73636F7065', '/scope', '/scope', 'test', 0); \
                 INSERT INTO scan_jobs(id, scope_id, status, discovered_files, discovered_folders, started_at_unix_ms, finished_at_unix_ms) VALUES (1, 1, 'completed', 1, 0, 0, 0); \
                 INSERT INTO nodes VALUES (1, 'file', 'test', X'01', 0, 0); \
                 INSERT INTO files VALUES (1, 4, 1, 1); \
                 INSERT INTO locations VALUES (1, 1, 1, X'2F73636F70652F66696C652E706E67', '/scope/file.png', '/scope/file.png', 1, 1); \
                 INSERT INTO extraction_jobs(id, scope_id, node_id, location_id, status, source_size_bytes, source_modified_unix_ns, output_bytes, chunk_count, elapsed_ms, created_at_unix_ms, updated_at_unix_ms) VALUES (1, 1, 1, 1, 'completed', 4, 1, 12, 1, 1, 0, 0); \
                 INSERT INTO content_chunks(id, scope_id, node_id, location_id, extraction_job_id, ordinal, text, provenance_kind, source_byte_start, source_byte_end, source_size_bytes, source_modified_unix_ns, trust_class, provider_id, provider_version, active, created_at_unix_ms) VALUES (1, 1, 1, 1, 1, 0, '保留legacy', 'byte_range', 0, 4, 4, 1, 'untrusted_extracted_text', 'deskgraph.utf8-text', '1', 1, 0); \
                 INSERT INTO image_metadata(id, scope_id, node_id, location_id, extraction_job_id, format, pixel_width, pixel_height, source_size_bytes, source_modified_unix_ns, provider_id, provider_version, active, created_at_unix_ms) VALUES (1, 1, 1, 1, 1, 'png', 2, 2, 4, 1, 'deskgraph.image-metadata', '1', 1, 0);",
            )
            .expect("legacy OCR-boundary fixture should persist");
        drop(connection);

        let upgraded = ManifestDatabase::open(&path).expect("OCR migration should apply");
        let job = upgraded.extraction_job(1).expect("legacy job should load");
        assert_eq!(job.api_version, "deskgraph.extraction-job.v2");
        assert_eq!(job.operation, ExtractionOperation::Content);
        let stored: (String, Option<i64>, Option<i64>) = upgraded
            .connection
            .query_row(
                "SELECT text, source_bbox_x_ppm, source_confidence_basis_points \
                 FROM content_chunks WHERE id = 1",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .expect("legacy chunk should load");
        assert_eq!(stored, ("保留legacy".to_string(), None, None));
        assert_eq!(
            upgraded
                .lexical_search_candidates("保留l", lexical_filters(Some(1)), 10)
                .expect("legacy FTS row should remain searchable")
                .len(),
            1
        );
        assert_eq!(
            upgraded
                .connection
                .query_row(
                    "SELECT COUNT(*) FROM image_metadata WHERE active = 1",
                    [],
                    |row| { row.get::<_, i64>(0) }
                )
                .expect("legacy metadata should remain active"),
            1
        );
    }

    #[test]
    fn version_schema_upgrade_preserves_exact_relations_observations_and_feedback() {
        let directory = tempfile::tempdir().expect("tempdir should exist");
        let path = directory.path().join("pre-version.sqlite3");
        let mut connection = Connection::open(&path).expect("old database should open");
        connection
            .execute_batch(
                "PRAGMA foreign_keys = ON; \
                 CREATE TABLE schema_migrations ( \
                    version INTEGER PRIMARY KEY, name TEXT NOT NULL, checksum TEXT NOT NULL, \
                    applied_at_unix_ms INTEGER NOT NULL \
                 );",
            )
            .expect("migration table should initialize");
        for migration in MIGRATIONS.iter().take(10) {
            let transaction = connection
                .transaction()
                .expect("historical migration should start");
            transaction
                .execute_batch(migration.sql)
                .expect("historical migration should apply");
            transaction
                .execute(
                    "INSERT INTO schema_migrations(version, name, checksum, applied_at_unix_ms) \
                     VALUES (?1, ?2, ?3, 1)",
                    params![
                        migration.version,
                        migration.name,
                        migration_checksum(migration.sql)
                    ],
                )
                .expect("historical migration should record");
            transaction
                .commit()
                .expect("historical migration should commit");
        }
        let database = ManifestDatabase { connection };
        let scope = database
            .add_scope(b"/scope", "/scope", "/scope", "test")
            .expect("scope should persist");
        database
            .connection
            .execute(
                "INSERT INTO scan_jobs( \
                    scope_id, status, discovered_files, discovered_folders, started_at_unix_ms, \
                    finished_at_unix_ms \
                 ) VALUES (?1, 'completed', 2, 0, 1, 1)",
                [scope.id],
            )
            .expect("scan should persist");
        let scan_id = database.connection.last_insert_rowid();
        let mut node_ids = Vec::new();
        let mut location_ids = Vec::new();
        for (path, identity) in [
            ("/scope/left.bin", b"identity:left".as_slice()),
            ("/scope/right.bin", b"identity:right".as_slice()),
        ] {
            database
                .connection
                .execute(
                    "INSERT INTO nodes( \
                        kind, identity_kind, identity_key, created_at_unix_ms, updated_at_unix_ms \
                     ) VALUES ('file', 'test_identity', ?1, 1, 1)",
                    [identity],
                )
                .expect("node should persist");
            let node_id = database.connection.last_insert_rowid();
            node_ids.push(node_id);
            database
                .connection
                .execute(
                    "INSERT INTO files(node_id, size_bytes, modified_unix_ns, link_count) \
                     VALUES (?1, 4, 2, 1)",
                    [node_id],
                )
                .expect("file should persist");
            database
                .connection
                .execute(
                    "INSERT INTO locations( \
                        scope_id, node_id, path_raw, path_key, display_path, present, \
                        last_seen_scan_id \
                     ) VALUES (?1, ?2, ?3, ?4, ?4, 1, ?5)",
                    params![scope.id, node_id, path.as_bytes(), path, scan_id],
                )
                .expect("location should persist");
            location_ids.push(database.connection.last_insert_rowid());
        }
        database
            .connection
            .execute(
                "INSERT INTO file_relation_candidates( \
                    api_version, relation_kind, scope_id, left_node_id, right_node_id, \
                    created_at_unix_ms \
                 ) VALUES (?1, 'exact_duplicate', ?2, ?3, ?4, 1)",
                params![
                    FileRelationCandidate::API_VERSION,
                    scope.id,
                    node_ids[0],
                    node_ids[1]
                ],
            )
            .expect("relation should persist");
        let relation_id = database.connection.last_insert_rowid();
        database
            .connection
            .execute(
                "INSERT INTO file_relation_observations( \
                    relation_id, left_location_id, right_location_id, source_size_bytes, \
                    left_modified_unix_ns, right_modified_unix_ns, compared_bytes, \
                    confidence_basis_points, comparison_kind, created_by, provider_id, \
                    provider_version, model_version, observed_at_unix_ms \
                 ) VALUES (?1, ?2, ?3, 4, 2, 2, 4, 10000, 'byte_for_byte', \
                    'system_rule', 'deskgraph.byte-equality', '1', NULL, 1)",
                params![relation_id, location_ids[0], location_ids[1]],
            )
            .expect("observation should persist");
        database
            .connection
            .execute(
                "INSERT INTO file_relation_feedback_events( \
                    relation_id, sequence, decision, created_by, created_at_unix_ms \
                 ) VALUES (?1, 1, 'rejected', 'user', 2)",
                [relation_id],
            )
            .expect("feedback should persist");
        let ManifestDatabase { connection } = database;
        drop(connection);

        let upgraded = ManifestDatabase::open(&path).expect("version migration should apply");
        let candidate = upgraded
            .file_relation_candidate(relation_id)
            .expect("exact candidate should survive upgrade");
        assert_eq!(candidate.state, FileRelationCandidateState::Rejected);
        assert_eq!(
            candidate
                .latest_decision
                .as_ref()
                .map(|decision| decision.sequence),
            Some(1)
        );
        assert_eq!(
            upgraded
                .connection
                .query_row(
                    "SELECT COUNT(*) FROM file_relation_observations",
                    [],
                    |row| { row.get::<_, i64>(0) }
                )
                .expect("observations should count"),
            1
        );
        assert_eq!(
            upgraded
                .connection
                .query_row(
                    "SELECT COUNT(*) FROM file_relation_feedback_events",
                    [],
                    |row| { row.get::<_, i64>(0) }
                )
                .expect("feedback should count"),
            1
        );
        let foreign_key_issues = upgraded
            .connection
            .prepare("PRAGMA foreign_key_check")
            .and_then(|mut statement| {
                let rows = statement.query_map([], |_| Ok(()))?;
                rows.collect::<Result<Vec<_>, _>>()
            })
            .expect("foreign key check should run");
        assert!(foreign_key_issues.is_empty());
    }

    #[test]
    fn action_preview_and_first_journal_event_commit_atomically_and_are_immutable() {
        let (mut database, scope_id, node_id, _) = extraction_setup();
        let source = database
            .action_source_for_path_key(scope_id, "/scope/file.txt")
            .expect("action source should load");
        assert_eq!(source.node_id, node_id);
        let preview = database
            .create_rename_action_plan(ActionPlanWrite {
                scope_id,
                node_id,
                source_location_id: source.location_id,
                source_path_raw: &source.path_raw,
                source_path_key: &source.path_key,
                source_display_path: &source.display_path,
                destination_path_raw: b"/scope/renamed.txt",
                destination_path_key: "/scope/renamed.txt",
                destination_display_path: "/scope/renamed.txt",
                source_identity_kind: &source.identity_kind,
                source_identity_key: &source.identity_key,
                source_size_bytes: source.size_bytes,
                source_modified_unix_ns: source.modified_unix_ns,
                execution_strategy: ActionExecutionStrategy::Direct,
            })
            .expect("preview and journal should persist");

        assert_eq!(preview.state, ActionPlanState::Previewed);
        assert_eq!(preview.journal_sequence, 1);
        assert_eq!(
            database
                .connection
                .query_row(
                    "SELECT COUNT(*) FROM action_plan_events WHERE plan_id = ?1",
                    [preview.plan_id],
                    |row| row.get::<_, i64>(0),
                )
                .expect("journal should count"),
            1
        );
        assert!(
            database
                .connection
                .execute(
                    "UPDATE action_plans SET destination_display_path = '/scope/other.txt' WHERE id = ?1",
                    [preview.plan_id],
                )
                .is_err(),
            "immutable plan update must fail"
        );
        assert!(
            database
                .connection
                .execute(
                    "DELETE FROM action_plan_events WHERE plan_id = ?1",
                    [preview.plan_id],
                )
                .is_err(),
            "append-only journal delete must fail"
        );
    }

    #[test]
    fn project_candidate_feedback_is_append_only_validated_and_idempotent() {
        let (mut database, scope_id, root_node_id) = project_setup();
        let facts = database
            .folder_profile_facts(scope_id, root_node_id, MAX_FOLDER_PROFILE_ENTRIES)
            .expect("profile facts should load");
        let suggestion = ProjectSuggestion {
            confidence_basis_points: 8_500,
            provenance: vec![ProjectSignal {
                kind: ProjectSignalKind::CargoManifest,
                marker_name: "Cargo.toml".to_string(),
                weight_basis_points: 8_500,
            }],
            observed_at_unix_ms: facts.observed_at_unix_ms,
            created_by: ProjectSuggestionCreator::SystemRule,
            provider_id: ProjectSuggestion::PROVIDER_ID,
            provider_version: ProjectSuggestion::PROVIDER_VERSION,
            model_version: None,
        };
        let candidate = database
            .record_project_candidate(scope_id, root_node_id, &suggestion)
            .expect("candidate should persist");
        let repeated = database
            .record_project_candidate(scope_id, root_node_id, &suggestion)
            .expect("same observation should be idempotent");
        assert_eq!(candidate.project_id, repeated.project_id);
        assert_eq!(candidate.state, ProjectCandidateState::Suggested);
        assert_eq!(
            database
                .connection
                .query_row("SELECT COUNT(*) FROM project_suggestions", [], |row| {
                    row.get::<_, i64>(0)
                })
                .expect("suggestions should count"),
            1
        );

        let rejected = database
            .decide_project_candidate(candidate.project_id, ProjectDecisionKind::Rejected)
            .expect("candidate should reject");
        let repeated_rejection = database
            .decide_project_candidate(candidate.project_id, ProjectDecisionKind::Rejected)
            .expect("same decision should be idempotent");
        assert_eq!(rejected.latest_decision, repeated_rejection.latest_decision);
        let accepted = database
            .decide_project_candidate(candidate.project_id, ProjectDecisionKind::Accepted)
            .expect("candidate should accept after correction");
        assert_eq!(accepted.state, ProjectCandidateState::Accepted);
        assert_eq!(
            accepted
                .latest_decision
                .as_ref()
                .map(|event| event.sequence),
            Some(2)
        );
        assert_eq!(
            database
                .connection
                .query_row("SELECT COUNT(*) FROM project_feedback_events", [], |row| {
                    row.get::<_, i64>(0)
                })
                .expect("feedback should count"),
            2
        );

        let mut invalid = suggestion.clone();
        invalid.confidence_basis_points = 8_501;
        assert!(matches!(
            database.record_project_candidate(scope_id, root_node_id, &invalid),
            Err(DatabaseError::ProjectCandidateInputInvalid)
        ));
        assert!(
            database
                .connection
                .execute(
                    "UPDATE projects SET created_at_unix_ms = created_at_unix_ms + 1 WHERE id = ?1",
                    [candidate.project_id],
                )
                .is_err()
        );
        assert!(
            database
                .connection
                .execute(
                    "DELETE FROM project_suggestion_signals WHERE suggestion_id = ( \
                    SELECT id FROM project_suggestions WHERE project_id = ?1 LIMIT 1 \
                 )",
                    [candidate.project_id],
                )
                .is_err()
        );
        assert!(
            database
                .connection
                .execute(
                    "DELETE FROM project_feedback_events WHERE project_id = ?1",
                    [candidate.project_id],
                )
                .is_err()
        );
    }

    #[test]
    fn exact_duplicate_candidate_is_append_only_and_snapshot_validated() {
        let (mut database, left, right) = exact_duplicate_setup();
        let candidate = database
            .record_exact_duplicate_candidate(&left, &right)
            .expect("candidate should persist");
        assert_eq!(candidate.kind, FileRelationKind::ExactDuplicate);
        assert_eq!(candidate.state, FileRelationCandidateState::Suggested);
        assert_eq!(candidate.left.node_id, left.node_id);
        assert_eq!(candidate.right.node_id, right.node_id);
        assert_eq!(candidate.evidence.compared_bytes, 4);
        assert_eq!(candidate.evidence.confidence_basis_points, 10_000);
        assert_eq!(candidate.evidence.model_version, None);

        let repeated = database
            .record_exact_duplicate_candidate(&left, &right)
            .expect("a new immutable observation should append");
        assert_eq!(repeated.relation_id, candidate.relation_id);
        let observation_count: i64 = database
            .connection
            .query_row(
                "SELECT COUNT(*) FROM file_relation_observations WHERE relation_id = ?1",
                [candidate.relation_id],
                |row| row.get(0),
            )
            .expect("observation count should load");
        assert_eq!(observation_count, 2);

        let rejected = database
            .decide_file_relation_candidate(
                candidate.relation_id,
                FileRelationDecisionKind::Rejected,
            )
            .expect("candidate should reject");
        assert_eq!(rejected.state, FileRelationCandidateState::Rejected);
        assert_eq!(
            rejected
                .latest_decision
                .as_ref()
                .map(|decision| decision.sequence),
            Some(1)
        );
        let repeated_rejection = database
            .decide_file_relation_candidate(
                candidate.relation_id,
                FileRelationDecisionKind::Rejected,
            )
            .expect("same decision should be idempotent");
        assert_eq!(repeated_rejection.latest_decision, rejected.latest_decision);
        let observed_after_rejection = database
            .record_exact_duplicate_candidate(&left, &right)
            .expect("new evidence should retain pair-specific feedback");
        assert_eq!(
            observed_after_rejection.state,
            FileRelationCandidateState::Rejected
        );
        let accepted = database
            .decide_file_relation_candidate(
                candidate.relation_id,
                FileRelationDecisionKind::Accepted,
            )
            .expect("candidate should accept after correction");
        assert_eq!(accepted.state, FileRelationCandidateState::Accepted);
        assert_eq!(
            accepted
                .latest_decision
                .as_ref()
                .map(|decision| decision.sequence),
            Some(2)
        );
        let repeated_acceptance = database
            .decide_file_relation_candidate(
                candidate.relation_id,
                FileRelationDecisionKind::Accepted,
            )
            .expect("same acceptance should be idempotent");
        assert_eq!(
            repeated_acceptance.latest_decision,
            accepted.latest_decision
        );
        assert_eq!(
            database
                .connection
                .query_row(
                    "SELECT COUNT(*) FROM file_relation_feedback_events WHERE relation_id = ?1",
                    [candidate.relation_id],
                    |row| row.get::<_, i64>(0),
                )
                .expect("feedback count should load"),
            2
        );
        let summaries = database
            .recent_file_relation_candidates()
            .expect("relation summaries should load");
        assert_eq!(summaries.len(), 1);
        assert_eq!(summaries[0].state, FileRelationCandidateState::Accepted);
        assert!(summaries[0].verification_required);
        assert_eq!(summaries[0].left_node_id, left.node_id);
        assert_eq!(summaries[0].right_node_id, right.node_id);

        let mut stale_left = left.clone();
        stale_left.modified_unix_ns = Some(3);
        assert!(matches!(
            database.record_exact_duplicate_candidate(&stale_left, &right),
            Err(DatabaseError::FileRelationCandidateNotCurrent)
        ));
        assert!(matches!(
            database.record_exact_duplicate_candidate(&right, &left),
            Err(DatabaseError::FileRelationCandidateInputInvalid)
        ));

        assert!(
            database
                .connection
                .execute(
                    "UPDATE file_relation_candidates SET scope_id = scope_id WHERE id = ?1",
                    [candidate.relation_id],
                )
                .is_err()
        );
        assert!(
            database
                .connection
                .execute(
                    "UPDATE file_relation_feedback_events SET decision = decision \
                     WHERE relation_id = ?1",
                    [candidate.relation_id],
                )
                .is_err()
        );
        assert!(
            database
                .connection
                .execute(
                    "DELETE FROM file_relation_feedback_events WHERE relation_id = ?1",
                    [candidate.relation_id],
                )
                .is_err()
        );
        assert!(
            database
                .connection
                .execute(
                    "DELETE FROM file_relation_candidates WHERE id = ?1",
                    [candidate.relation_id],
                )
                .is_err()
        );
        assert!(
            database
                .connection
                .execute(
                    "UPDATE file_relation_observations SET compared_bytes = compared_bytes \
                     WHERE relation_id = ?1",
                    [candidate.relation_id],
                )
                .is_err()
        );
        assert!(
            database
                .connection
                .execute(
                    "DELETE FROM file_relation_observations WHERE relation_id = ?1",
                    [candidate.relation_id],
                )
                .is_err()
        );

        database
            .connection
            .execute(
                "UPDATE locations SET present = 0 WHERE id = ?1",
                [left.location_id],
            )
            .expect("location should become absent");
        assert!(matches!(
            database.file_relation_candidate(candidate.relation_id),
            Err(DatabaseError::FileRelationCandidateNotCurrent)
        ));
    }

    #[test]
    fn file_version_candidate_is_directional_append_only_and_path_free_in_history() {
        let (mut database, first, second) = file_version_setup();
        let candidate = database
            .record_file_version_candidate(&first, &second)
            .expect("explicit versions should persist");
        assert_eq!(candidate.kind, FileRelationKind::Version);
        assert_eq!(candidate.state, FileRelationCandidateState::Suggested);
        assert_eq!(candidate.older.node_id, first.node_id);
        assert_eq!(candidate.newer.node_id, second.node_id);
        assert_eq!(candidate.evidence.base_key, "企劃");
        assert_eq!(candidate.evidence.extension_key, "md");
        assert_eq!(candidate.evidence.older_version, 1);
        assert_eq!(candidate.evidence.newer_version, 2);
        assert_eq!(candidate.evidence.confidence_basis_points, 9_000);
        assert_eq!(candidate.evidence.model_version, None);
        assert_eq!(candidate.latest_decision, None);

        let reversed = database
            .record_file_version_candidate(&second, &first)
            .expect("reversed inputs should reuse stable relation identity");
        assert_eq!(reversed.relation_id, candidate.relation_id);
        assert_eq!(
            database
                .connection
                .query_row(
                    "SELECT COUNT(*) FROM file_version_observations WHERE relation_id = ?1",
                    [candidate.relation_id],
                    |row| row.get::<_, i64>(0),
                )
                .expect("version observations should count"),
            2
        );
        let summaries = database
            .recent_file_relation_candidates()
            .expect("relation history should load");
        assert_eq!(summaries.len(), 1);
        assert_eq!(summaries[0].kind, FileRelationKind::Version);
        assert_eq!(summaries[0].state, FileRelationCandidateState::Suggested);
        assert_eq!(summaries[0].confidence_basis_points, 9_000);
        assert!(summaries[0].verification_required);
        assert_eq!(summaries[0].latest_decision_at_unix_ms, None);
        let mut mismatched = second.clone();
        mismatched.display_path = "/scope/其他-v2.md".to_string();
        assert!(matches!(
            database.record_file_version_candidate(&first, &mismatched),
            Err(DatabaseError::FileRelationCandidateInputInvalid)
        ));
        let mut same_version = second.clone();
        same_version.display_path = "/scope/企劃-v1.md".to_string();
        assert!(matches!(
            database.record_file_version_candidate(&first, &same_version),
            Err(DatabaseError::FileRelationCandidateInputInvalid)
        ));
        assert!(
            database
                .connection
                .execute(
                    "UPDATE file_version_observations SET newer_version = newer_version \
                     WHERE relation_id = ?1",
                    [candidate.relation_id],
                )
                .is_err()
        );
        assert!(
            database
                .connection
                .execute(
                    "DELETE FROM file_version_observations WHERE relation_id = ?1",
                    [candidate.relation_id],
                )
                .is_err()
        );
        database
            .connection
            .execute(
                "UPDATE locations SET present = 0 WHERE id = ?1",
                [first.location_id],
            )
            .expect("location should become absent");
        assert!(matches!(
            database.file_version_candidate(candidate.relation_id),
            Err(DatabaseError::FileRelationCandidateNotCurrent)
        ));
    }

    #[test]
    fn file_version_feedback_is_append_only_idempotent_and_direction_bound() {
        let (mut database, first, second) = file_version_setup();
        let candidate = database
            .record_file_version_candidate(&first, &second)
            .expect("version candidate should persist");
        let rejected = database
            .decide_file_version_candidate(
                candidate.relation_id,
                FileRelationDecisionKind::Rejected,
            )
            .expect("current evidence should be rejectable");
        assert_eq!(rejected.state, FileRelationCandidateState::Rejected);
        assert_eq!(
            rejected
                .latest_decision
                .as_ref()
                .expect("decision should exist")
                .sequence,
            1
        );

        let reverified = database
            .record_file_version_candidate(&first, &second)
            .expect("equivalent evidence should append an observation");
        assert_eq!(reverified.state, FileRelationCandidateState::Rejected);
        let idempotent = database
            .decide_file_version_candidate(
                candidate.relation_id,
                FileRelationDecisionKind::Rejected,
            )
            .expect("repeated equivalent decision should succeed");
        assert_eq!(
            idempotent
                .latest_decision
                .as_ref()
                .expect("decision should exist")
                .sequence,
            1
        );
        let accepted = database
            .decide_file_version_candidate(
                candidate.relation_id,
                FileRelationDecisionKind::Accepted,
            )
            .expect("opposite decision should append a correction");
        assert_eq!(accepted.state, FileRelationCandidateState::Accepted);
        assert_eq!(
            accepted
                .latest_decision
                .as_ref()
                .expect("decision should exist")
                .sequence,
            2
        );

        let changed_path = "/scope/企劃-v3.md";
        database
            .connection
            .execute(
                "UPDATE locations SET path_raw = ?1, path_key = ?2, display_path = ?2 \
                 WHERE id = ?3",
                params![changed_path.as_bytes(), changed_path, first.location_id,],
            )
            .expect("fixture rename should update current location");
        database
            .connection
            .execute(
                "INSERT INTO file_version_observations( \
                     relation_id, older_location_id, newer_location_id, older_size_bytes, \
                     newer_size_bytes, older_modified_unix_ns, newer_modified_unix_ns, \
                     base_key, extension_key, older_version, newer_version, \
                     confidence_basis_points, signal_kind, created_by, provider_id, \
                     provider_version, model_version, observed_at_unix_ms \
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, '企劃', 'md', 2, 3, 9000, \
                     'explicit_numeric_suffix', 'system_rule', 'deskgraph.filename-version', \
                     '1', NULL, ?8)",
                params![
                    candidate.relation_id,
                    second.location_id,
                    first.location_id,
                    to_i64(second.size_bytes).expect("size should fit"),
                    to_i64(first.size_bytes).expect("size should fit"),
                    second.modified_unix_ns,
                    first.modified_unix_ns,
                    accepted.evidence.observed_at_unix_ms,
                ],
            )
            .expect("new direction should append");
        let changed = database
            .file_version_candidate(candidate.relation_id)
            .expect("changed direction should load");
        assert_eq!(changed.older.node_id, second.node_id);
        assert_eq!(changed.newer.node_id, first.node_id);
        assert_eq!(changed.state, FileRelationCandidateState::Suggested);
        assert_eq!(changed.latest_decision, None);
        let changed_rejected = database
            .decide_file_version_candidate(
                candidate.relation_id,
                FileRelationDecisionKind::Rejected,
            )
            .expect("new direction should accept its own decision");
        assert_eq!(changed_rejected.state, FileRelationCandidateState::Rejected);
        assert_eq!(
            changed_rejected
                .latest_decision
                .as_ref()
                .expect("changed decision should exist")
                .sequence,
            3
        );

        let original_path = "/scope/企劃-v1.md";
        database
            .connection
            .execute(
                "UPDATE locations SET path_raw = ?1, path_key = ?2, display_path = ?2 \
                 WHERE id = ?3",
                params![original_path.as_bytes(), original_path, first.location_id],
            )
            .expect("fixture rename should restore current location");
        let restored_first = database
            .action_source_for_path_key(first.scope_id, original_path)
            .expect("restored source should load");
        let restored = database
            .record_file_version_candidate(&restored_first, &second)
            .expect("restored equivalent evidence should append");
        assert_eq!(restored.state, FileRelationCandidateState::Accepted);
        assert_eq!(
            restored
                .latest_decision
                .as_ref()
                .expect("restored decision should exist")
                .sequence,
            2
        );
        database
            .decide_file_version_candidate(
                candidate.relation_id,
                FileRelationDecisionKind::Accepted,
            )
            .expect("restored repeated decision should remain idempotent");
        assert_eq!(
            database
                .connection
                .query_row(
                    "SELECT COUNT(*) FROM file_version_feedback_events WHERE relation_id = ?1",
                    [candidate.relation_id],
                    |row| row.get::<_, i64>(0),
                )
                .expect("feedback events should count"),
            3
        );
        assert!(
            database
                .connection
                .execute(
                    "UPDATE file_version_feedback_events SET decision = decision \
                     WHERE relation_id = ?1",
                    [candidate.relation_id],
                )
                .is_err()
        );
        assert!(
            database
                .connection
                .execute(
                    "DELETE FROM file_version_feedback_events WHERE relation_id = ?1",
                    [candidate.relation_id],
                )
                .is_err()
        );
    }

    #[test]
    fn database_rejects_action_plan_when_manifest_snapshot_does_not_match() {
        let (mut database, scope_id, node_id, _) = extraction_setup();
        let source = database
            .action_source_for_path_key(scope_id, "/scope/file.txt")
            .expect("action source should load");
        let error = database
            .create_rename_action_plan(ActionPlanWrite {
                scope_id,
                node_id,
                source_location_id: source.location_id,
                source_path_raw: &source.path_raw,
                source_path_key: &source.path_key,
                source_display_path: &source.display_path,
                destination_path_raw: b"/scope/renamed.txt",
                destination_path_key: "/scope/renamed.txt",
                destination_display_path: "/scope/renamed.txt",
                source_identity_kind: &source.identity_kind,
                source_identity_key: &source.identity_key,
                source_size_bytes: source.size_bytes + 1,
                source_modified_unix_ns: source.modified_unix_ns,
                execution_strategy: ActionExecutionStrategy::Direct,
            })
            .expect_err("database boundary must reject stale snapshot");
        assert!(matches!(error, DatabaseError::ActionSourceSnapshotChanged));
        assert!(
            database
                .recent_action_plans()
                .expect("summaries should load")
                .is_empty()
        );
    }

    #[test]
    fn v3_content_migration_preserves_exact_byte_provenance() {
        let connection = Connection::open_in_memory().expect("connection should open");
        connection
            .execute_batch(
                "CREATE TABLE schema_migrations (version INTEGER PRIMARY KEY, name TEXT NOT NULL, checksum TEXT NOT NULL, applied_at_unix_ms INTEGER NOT NULL);",
            )
            .expect("migration registry should initialize");
        for migration in &MIGRATIONS[..3] {
            connection
                .execute_batch(migration.sql)
                .expect("legacy migration should apply");
            connection
                .execute(
                    "INSERT INTO schema_migrations(version, name, checksum, applied_at_unix_ms) VALUES (?1, ?2, ?3, 0)",
                    params![
                        migration.version,
                        migration.name,
                        migration_checksum(migration.sql)
                    ],
                )
                .expect("legacy migration should register");
        }
        connection
            .execute_batch(
                "INSERT INTO authorized_scopes VALUES (1, X'2F73636F7065', '/scope', '/scope', 'test', 0);\
                 INSERT INTO scan_jobs(id, scope_id, status, started_at_unix_ms) VALUES (1, 1, 'completed', 0);\
                 INSERT INTO nodes VALUES (1, 'file', 'test', X'01', 0, 0);\
                 INSERT INTO files VALUES (1, 4, 1, 1);\
                 INSERT INTO locations VALUES (1, 1, 1, X'2F73636F70652F66696C652E747874', '/scope/file.txt', '/scope/file.txt', 1, 1);\
                 INSERT INTO extraction_jobs(id, scope_id, node_id, location_id, status, source_size_bytes, source_modified_unix_ns, output_bytes, chunk_count, created_at_unix_ms, updated_at_unix_ms) VALUES (1, 1, 1, 1, 'completed', 4, 1, 6, 1, 0, 0);\
                 INSERT INTO content_chunks(id, scope_id, node_id, location_id, extraction_job_id, ordinal, text, source_byte_start, source_byte_end, source_size_bytes, source_modified_unix_ns, trust_class, provider_id, provider_version, active, created_at_unix_ms) VALUES (1, 1, 1, 1, 1, 0, 'legacy', 1, 3, 4, 1, 'untrusted_extracted_text', 'deskgraph.utf8-text', '1', 1, 0);",
            )
            .expect("legacy content fixture should initialize");

        let database = ManifestDatabase::from_connection(connection)
            .expect("new provenance migration should apply");
        let stored = database
            .connection
            .query_row(
                "SELECT provenance_kind, source_byte_start, source_byte_end, source_page_number, \
                    source_fragment_index, source_unit_number, source_cell_reference \
                 FROM content_chunks WHERE id = 1",
                [],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, Option<i64>>(1)?,
                        row.get::<_, Option<i64>>(2)?,
                        row.get::<_, Option<i64>>(3)?,
                        row.get::<_, Option<i64>>(4)?,
                        row.get::<_, Option<i64>>(5)?,
                        row.get::<_, Option<String>>(6)?,
                    ))
                },
            )
            .expect("migrated provenance should load");

        assert_eq!(
            stored,
            (
                "byte_range".to_string(),
                Some(1),
                Some(3),
                None,
                None,
                None,
                None,
            )
        );
        let candidates = database
            .lexical_search_candidates("\"legacy\"", lexical_filters(None), 10)
            .expect("search migration should backfill existing content");
        assert_eq!(candidates.len(), 1);
        assert_eq!(candidates[0].source, LexicalCandidateSource::ExtractedText);
    }

    #[test]
    fn trigram_search_indexes_multilingual_metadata_and_only_active_content() {
        let (mut database, scope_id, node_id, root) = extraction_setup();
        database
            .connection
            .execute(
                "UPDATE locations SET display_path = '/scope/專案-context.md' WHERE node_id = ?1",
                [node_id],
            )
            .expect("display path should update through the FTS trigger");
        let text = "Traditional Chinese 專案脈絡 and English context stay local";
        let job = database
            .create_extraction_job(scope_id, node_id)
            .expect("job should create");
        database
            .claim_extraction_job(job.job_id, "search-runner", 60_000)
            .expect("job should claim");
        database
            .complete_extraction_job(
                job.job_id,
                "search-runner",
                "deskgraph.utf8-text",
                "1",
                4,
                Some(1),
                u64::try_from(text.len()).expect("fixture length should fit"),
                1,
                &[ContentChunkWrite {
                    ordinal: 0,
                    text: text.to_string(),
                    provenance: ContentChunkProvenanceWrite::ByteRange { start: 0, end: 4 },
                    trust_class: "untrusted_extracted_text",
                }],
            )
            .expect("content should publish");

        let metadata = database
            .lexical_search_candidates("\"專案-context\"", lexical_filters(Some(scope_id)), 10)
            .expect("metadata search should pass");
        assert!(
            metadata
                .iter()
                .any(|candidate| candidate.source == LexicalCandidateSource::MetadataPath)
        );
        let filtered_metadata = database
            .lexical_search_candidates(
                "\"專案-context\"",
                LexicalSearchFilters {
                    scope_id: Some(scope_id),
                    source: LexicalSearchSource::MetadataPath,
                    extension: Some("md"),
                    modified_since_unix_ns: Some(1),
                    modified_before_unix_ns: Some(2),
                },
                10,
            )
            .expect("bounded metadata filters should pass");
        assert_eq!(filtered_metadata.len(), 1);
        assert_eq!(
            filtered_metadata[0].source,
            LexicalCandidateSource::MetadataPath
        );
        let content = database
            .lexical_search_candidates("\"專案脈絡\"", lexical_filters(None), 10)
            .expect("content search should pass");
        let snippet = content
            .iter()
            .find(|candidate| candidate.source == LexicalCandidateSource::ExtractedText)
            .and_then(|candidate| candidate.snippet.as_deref())
            .expect("content result should include a bounded snippet");
        assert!(snippet.contains("專案脈絡"));

        let filtered_content = database
            .lexical_search_candidates(
                "\"專案脈絡\"",
                LexicalSearchFilters {
                    scope_id: Some(scope_id),
                    source: LexicalSearchSource::ExtractedText,
                    extension: Some("md"),
                    modified_since_unix_ns: Some(1),
                    modified_before_unix_ns: Some(2),
                },
                10,
            )
            .expect("bounded content filters should pass");
        assert_eq!(filtered_content.len(), 1);
        assert_eq!(
            filtered_content[0].source,
            LexicalCandidateSource::ExtractedText
        );
        for filters in [
            LexicalSearchFilters {
                source: LexicalSearchSource::MetadataPath,
                ..lexical_filters(Some(scope_id))
            },
            LexicalSearchFilters {
                extension: Some("txt"),
                ..lexical_filters(Some(scope_id))
            },
            LexicalSearchFilters {
                modified_since_unix_ns: Some(2),
                ..lexical_filters(Some(scope_id))
            },
        ] {
            assert!(
                database
                    .lexical_search_candidates("\"專案脈絡\"", filters, 10)
                    .expect("bounded non-matching filter should pass")
                    .is_empty()
            );
        }

        publish_manifest_file(&mut database, scope_id, &root, 5);
        let stale = database
            .lexical_search_candidates("\"專案脈絡\"", lexical_filters(None), 10)
            .expect("stale search should pass");
        assert!(
            stale
                .iter()
                .all(|candidate| candidate.source != LexicalCandidateSource::ExtractedText)
        );
    }

    #[test]
    fn search_database_boundary_rejects_unbounded_requests() {
        let database = ManifestDatabase::open_in_memory().expect("database should initialize");
        assert!(matches!(
            database.lexical_search_candidates("", lexical_filters(None), 10),
            Err(DatabaseError::SearchInputInvalid)
        ));
        assert!(matches!(
            database.lexical_search_candidates("\"bounded\"", lexical_filters(None), 101),
            Err(DatabaseError::SearchInputInvalid)
        ));
        for filters in [
            LexicalSearchFilters {
                scope_id: Some(0),
                ..lexical_filters(None)
            },
            LexicalSearchFilters {
                extension: Some("m_d"),
                ..lexical_filters(None)
            },
            LexicalSearchFilters {
                modified_since_unix_ns: Some(2),
                modified_before_unix_ns: Some(2),
                ..lexical_filters(None)
            },
        ] {
            assert!(matches!(
                database.lexical_search_candidates("\"bounded\"", filters, 10),
                Err(DatabaseError::SearchInputInvalid)
            ));
        }
    }

    #[test]
    fn ready_job_can_pause_and_resume_without_processing() {
        let (mut database, scope_id, root) = resumable_setup();
        let job = database
            .create_resumable_scan_job(scope_id, &root)
            .expect("job should create");

        let paused = database
            .request_scan_pause(job.job_id)
            .expect("ready job should pause");
        assert_eq!(paused.status, ScanStatus::Paused);
        assert_eq!(paused.processed_entries, 0);

        let resumed = database
            .resume_scan_job(job.job_id)
            .expect("paused job should resume");
        assert_eq!(resumed.status, ScanStatus::Running);
        assert!(!resumed.pause_requested);
    }

    #[test]
    fn active_runner_acknowledges_a_durable_pause_request_on_release() {
        let (mut database, scope_id, root) = resumable_setup();
        let job = database
            .create_resumable_scan_job(scope_id, &root)
            .expect("job should create");
        database
            .claim_scan_job(job.job_id, "runner", 60_000)
            .expect("job should claim");

        let requested = database
            .request_scan_pause(job.job_id)
            .expect("pause should persist");
        assert_eq!(requested.status, ScanStatus::Running);
        assert!(requested.pause_requested);
        let timed = database
            .record_scan_runner_elapsed(job.job_id, "runner", 17)
            .expect("active time should persist");
        assert_eq!(timed.elapsed_ms, 17);

        let paused = database
            .release_scan_job(job.job_id, "runner")
            .expect("runner should acknowledge pause");
        assert_eq!(paused.status, ScanStatus::Paused);
        assert!(paused.pause_requested);
    }

    #[test]
    fn expired_runner_is_interrupted_and_queue_item_is_replayable() {
        let (mut database, scope_id, root) = resumable_setup();
        let job = database
            .create_resumable_scan_job(scope_id, &root)
            .expect("job should create");
        database
            .claim_scan_job(job.job_id, "runner-a", 60_000)
            .expect("job should claim");
        let first = database
            .next_scan_queue_entry(job.job_id, "runner-a", 60_000)
            .expect("queue should read")
            .expect("root should be pending");

        assert_eq!(
            database
                .recover_expired_scan_jobs_at(i64::MAX)
                .expect("expired job should recover"),
            1
        );
        assert_eq!(
            database
                .scan_job(job.job_id)
                .expect("job should load")
                .status,
            ScanStatus::Interrupted
        );
        database
            .resume_scan_job(job.job_id)
            .expect("interrupted job should resume");
        database
            .claim_scan_job(job.job_id, "runner-b", 60_000)
            .expect("resumed job should claim");
        let replay = database
            .next_scan_queue_entry(job.job_id, "runner-b", 60_000)
            .expect("queue should read")
            .expect("root should replay");
        assert_eq!(replay.id, first.id);
    }

    #[test]
    fn reopening_a_file_database_recovers_an_expired_processing_entry() {
        let directory = tempfile::tempdir().expect("tempdir should exist");
        let path = directory.path().join("manifest.sqlite3");
        let mut database = ManifestDatabase::open(&path).expect("database should initialize");
        let scope = database
            .add_scope(b"/scope", "/scope", "/scope", "test")
            .expect("scope should persist");
        let root = QueuedPath {
            path_raw: b"/scope".to_vec(),
            path_key: "/scope".to_string(),
            parent_identity_key: None,
            is_root: true,
        };
        let job = database
            .create_resumable_scan_job(scope.id, &root)
            .expect("job should create");
        database
            .claim_scan_job(job.job_id, "crashed-runner", 60_000)
            .expect("job should claim");
        let processing = database
            .next_scan_queue_entry(job.job_id, "crashed-runner", 60_000)
            .expect("queue should read")
            .expect("root should be processing");
        database
            .connection
            .execute(
                "UPDATE scan_jobs SET lease_expires_at_unix_ms = 0 WHERE id = ?1",
                [job.job_id],
            )
            .expect("fixture lease should expire");
        drop(database);

        let mut recovered = ManifestDatabase::open(&path).expect("database should recover on open");
        assert_eq!(
            recovered
                .scan_job(job.job_id)
                .expect("job should load")
                .status,
            ScanStatus::Interrupted
        );
        recovered
            .resume_scan_job(job.job_id)
            .expect("interrupted job should resume");
        recovered
            .claim_scan_job(job.job_id, "new-runner", 60_000)
            .expect("resumed job should claim");
        let replay = recovered
            .next_scan_queue_entry(job.job_id, "new-runner", 60_000)
            .expect("queue should read")
            .expect("processing entry should replay");
        assert_eq!(replay.id, processing.id);
    }

    #[test]
    fn staged_batches_are_invisible_until_atomic_publish() {
        let (mut database, scope_id, root) = resumable_setup();
        let job = database
            .create_resumable_scan_job(scope_id, &root)
            .expect("job should create");
        database
            .claim_scan_job(job.job_id, "runner", 60_000)
            .expect("job should claim");
        let root_entry = database
            .next_scan_queue_entry(job.job_id, "runner", 60_000)
            .expect("queue should read")
            .expect("root should exist");
        let root_observation = observation("/scope", NodeKind::Folder, None);
        let child = QueuedPath {
            path_raw: b"/scope/file.txt".to_vec(),
            path_key: "/scope/file.txt".to_string(),
            parent_identity_key: Some(root_observation.identity_key.clone()),
            is_root: false,
        };
        database
            .stage_scan_queue_entry(
                job.job_id,
                "runner",
                root_entry.id,
                Some(&root_observation),
                std::slice::from_ref(&child),
                &[],
                0,
                1,
                60_000,
            )
            .expect("root should stage");
        let child_entry = database
            .next_scan_queue_entry(job.job_id, "runner", 60_000)
            .expect("queue should read")
            .expect("child should exist");
        let child_observation = observation(
            "/scope/file.txt",
            NodeKind::File,
            Some(root_observation.identity_key.clone()),
        );
        let staged = database
            .stage_scan_queue_entry(
                job.job_id,
                "runner",
                child_entry.id,
                Some(&child_observation),
                &[],
                &[],
                0,
                1,
                60_000,
            )
            .expect("child should stage");

        assert_eq!(staged.processed_entries, 2);
        assert_eq!(database.stats().expect("stats should load").node_count, 0);
        let completed = database
            .finalize_resumable_scan_job(job.job_id, "runner")
            .expect("job should publish");
        assert_eq!(completed.status, ScanStatus::Completed);
        assert_eq!(database.stats().expect("stats should load").node_count, 2);
    }

    #[test]
    fn queued_and_running_extractions_can_be_cancelled_durably() {
        let (mut database, scope_id, node_id, _) = extraction_setup();
        let queued = database
            .create_extraction_job(scope_id, node_id)
            .expect("job should create");

        let cancelled = database
            .request_extraction_cancel(queued.job_id)
            .expect("queued job should cancel");
        assert_eq!(cancelled.status, ExtractionStatus::Cancelled);

        let running = database
            .create_extraction_job(scope_id, node_id)
            .expect("replacement job should create");
        database
            .claim_extraction_job(running.job_id, "extract-runner", 60_000)
            .expect("job should claim");
        let requested = database
            .request_extraction_cancel(running.job_id)
            .expect("running cancellation should persist");
        assert_eq!(requested.status, ExtractionStatus::Running);
        assert!(requested.cancel_requested);
        let acknowledged = database
            .cancel_extraction_job_from_runner(
                running.job_id,
                "extract-runner",
                "deskgraph.utf8-text",
                "1",
                2,
            )
            .expect("runner should acknowledge cancellation");
        assert_eq!(acknowledged.status, ExtractionStatus::Cancelled);
        assert_eq!(
            database
                .extraction_stats()
                .expect("stats should load")
                .cancelled_job_count,
            2
        );
    }

    #[test]
    fn durable_cancel_wins_when_provider_failure_is_recorded_afterward() {
        let (mut database, scope_id, node_id, _) = extraction_setup();
        let job = database
            .create_extraction_job(scope_id, node_id)
            .expect("job should create");
        database
            .claim_extraction_job(job.job_id, "failure-race-runner", 60_000)
            .expect("job should claim");
        let requested = database
            .request_extraction_cancel(job.job_id)
            .expect("running cancellation should persist");
        assert_eq!(requested.status, ExtractionStatus::Running);
        assert!(requested.cancel_requested);

        let terminal = database
            .fail_extraction_job(
                job.job_id,
                "failure-race-runner",
                "deskgraph.racing-provider",
                "1",
                "extraction_ocr_provider_failed",
                7,
            )
            .expect("durable cancellation must win the terminal-state race");
        assert_eq!(terminal.status, ExtractionStatus::Cancelled);
        assert!(terminal.cancel_requested);
        assert_eq!(terminal.error_code, None);
        assert_eq!(
            terminal.provider_id.as_deref(),
            Some("deskgraph.racing-provider")
        );
    }

    #[test]
    fn durable_cancel_prevents_atomic_publication_before_runner_acknowledgement() {
        let (mut database, scope_id, node_id, _) = extraction_setup();
        let job = database
            .create_extraction_job(scope_id, node_id)
            .expect("job should create");
        database
            .claim_extraction_job(job.job_id, "completion-race-runner", 60_000)
            .expect("job should claim");
        database
            .request_extraction_cancel(job.job_id)
            .expect("running cancellation should persist");

        let error = database
            .complete_extraction_job(
                job.job_id,
                "completion-race-runner",
                "deskgraph.utf8-text",
                "1",
                4,
                Some(1),
                4,
                1,
                &[ContentChunkWrite {
                    ordinal: 0,
                    text: "text".to_string(),
                    provenance: ContentChunkProvenanceWrite::ByteRange { start: 0, end: 4 },
                    trust_class: "untrusted_extracted_text",
                }],
            )
            .expect_err("durable cancellation must reject publication");
        assert!(matches!(error, DatabaseError::ExtractionOutputInvalid));
        assert_eq!(
            database
                .connection
                .query_row(
                    "SELECT COUNT(*) FROM content_chunks WHERE extraction_job_id = ?1",
                    [job.job_id],
                    |row| row.get::<_, i64>(0),
                )
                .expect("published chunks should count"),
            0
        );
        let terminal = database
            .cancel_extraction_job_from_runner(
                job.job_id,
                "completion-race-runner",
                "deskgraph.utf8-text",
                "1",
                1,
            )
            .expect("runner should acknowledge cancellation");
        assert_eq!(terminal.status, ExtractionStatus::Cancelled);
    }

    #[test]
    fn validated_ocr_source_must_still_match_current_manifest_at_insert() {
        let (mut database, scope_id, node_id, _) = extraction_setup();
        let validated = database
            .extractable_file(scope_id, node_id)
            .expect("validated source should load");
        database
            .connection
            .execute(
                "UPDATE files SET size_bytes = size_bytes + 1 WHERE node_id = ?1",
                [node_id],
            )
            .expect("fixture manifest should change");

        assert!(matches!(
            database.low_level_insert_screenshot_ocr_job_after_core_validation(&validated),
            Err(DatabaseError::ExtractableFileNotFound)
        ));
        assert!(
            database
                .recent_extraction_jobs()
                .expect("jobs should remain queryable")
                .is_empty()
        );
        let content = database
            .create_extraction_job(scope_id, node_id)
            .expect("stale OCR validation must not block a current content job");
        assert_eq!(content.operation, ExtractionOperation::Content);
        assert_eq!(content.status, ExtractionStatus::Queued);
    }

    #[test]
    fn low_level_ocr_insert_has_one_production_workspace_callsite() {
        fn collect_calls(
            root: &Path,
            workspace: &Path,
            calls: &mut Vec<String>,
        ) -> std::io::Result<()> {
            for entry in fs::read_dir(root)? {
                let entry = entry?;
                let file_type = entry.file_type()?;
                if file_type.is_symlink() {
                    continue;
                }
                let path = entry.path();
                if file_type.is_dir() {
                    collect_calls(&path, workspace, calls)?;
                } else if path.extension().and_then(|value| value.to_str()) == Some("rs")
                    && !path.ends_with("crates/database/src/lib.rs")
                {
                    let source = fs::read_to_string(&path)?;
                    let matches = source
                        .matches("low_level_insert_screenshot_ocr_job_after_core_validation")
                        .count();
                    for _ in 0..matches {
                        calls.push(
                            path.strip_prefix(workspace)
                                .unwrap_or(&path)
                                .to_string_lossy()
                                .into_owned(),
                        );
                    }
                }
            }
            Ok(())
        }

        let workspace = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../..")
            .canonicalize()
            .expect("workspace should canonicalize");
        let mut calls = Vec::new();
        for root in ["apps", "crates", "tools"] {
            collect_calls(&workspace.join(root), &workspace, &mut calls)
                .expect("workspace Rust sources should remain readable");
        }
        assert_eq!(calls, vec!["crates/extractors/src/service.rs"]);
    }

    #[test]
    fn screenshot_ocr_lookup_prefers_interrupted_work_over_newer_terminal_history() {
        let (mut database, scope_id, node_id, _) = extraction_setup();
        let interrupted = database
            .create_screenshot_ocr_job(scope_id, node_id)
            .expect("OCR job should create");
        database
            .claim_extraction_job(interrupted.job_id, "expired-ocr-runner", 60_000)
            .expect("OCR job should claim");
        assert_eq!(
            database
                .recover_expired_extraction_jobs_at(i64::MAX)
                .expect("expired OCR lease should recover"),
            1
        );

        let location_id: i64 = database
            .connection
            .query_row(
                "SELECT location_id FROM extraction_jobs WHERE id = ?1",
                [interrupted.job_id],
                |row| row.get(0),
            )
            .expect("OCR location should load");
        for ordinal in 0..21_i64 {
            database
                .connection
                .execute(
                    "INSERT INTO extraction_jobs( \
                        scope_id, node_id, location_id, status, source_size_bytes, \
                        created_at_unix_ms, updated_at_unix_ms, operation \
                     ) VALUES (?1, ?2, ?3, 'completed', 4, ?4, ?4, 'content')",
                    params![scope_id, node_id, location_id, ordinal],
                )
                .expect("newer generic terminal history should insert");
        }
        database
            .connection
            .execute(
                "INSERT INTO extraction_jobs( \
                    scope_id, node_id, location_id, status, source_size_bytes, \
                    created_at_unix_ms, updated_at_unix_ms, operation \
                 ) VALUES (?1, ?2, ?3, 'completed', 4, 99, 99, 'screenshot_ocr')",
                params![scope_id, node_id, location_id],
            )
            .expect("newer terminal OCR history should insert");

        let found = database
            .screenshot_ocr_job_for_node(scope_id, node_id)
            .expect("OCR lookup should query")
            .expect("interrupted OCR should remain discoverable");
        assert_eq!(found.job_id, interrupted.job_id);
        assert_eq!(found.operation, ExtractionOperation::ScreenshotOcr);
        assert_eq!(found.status, ExtractionStatus::Interrupted);

        let (mut generic_database, generic_scope_id, generic_node_id, _) = extraction_setup();
        generic_database
            .create_extraction_job(generic_scope_id, generic_node_id)
            .expect("generic job should create");
        assert_eq!(
            generic_database
                .screenshot_ocr_job_for_node(generic_scope_id, generic_node_id)
                .expect("generic lookup should query"),
            None
        );
    }

    #[test]
    fn complete_extraction_atomically_replaces_only_valid_chunks() {
        let (mut database, scope_id, node_id, _) = extraction_setup();
        let first = database
            .create_extraction_job(scope_id, node_id)
            .expect("job should create");
        database
            .claim_extraction_job(first.job_id, "runner-1", 60_000)
            .expect("job should claim");
        let first_chunks = vec![
            ContentChunkWrite {
                ordinal: 0,
                text: "ab".to_string(),
                provenance: ContentChunkProvenanceWrite::ByteRange { start: 0, end: 2 },
                trust_class: "untrusted_extracted_text",
            },
            ContentChunkWrite {
                ordinal: 1,
                text: "cd".to_string(),
                provenance: ContentChunkProvenanceWrite::ByteRange { start: 2, end: 4 },
                trust_class: "untrusted_extracted_text",
            },
        ];
        let completed = database
            .complete_extraction_job(
                first.job_id,
                "runner-1",
                "deskgraph.utf8-text",
                "1",
                4,
                Some(1),
                4,
                3,
                &first_chunks,
            )
            .expect("valid chunks should publish");
        assert_eq!(completed.status, ExtractionStatus::Completed);
        assert_eq!(completed.chunk_count, 2);
        assert_eq!(
            database
                .extraction_stats()
                .expect("stats should load")
                .active_chunk_count,
            2
        );

        let invalid = database
            .create_extraction_job(scope_id, node_id)
            .expect("retry should create");
        database
            .claim_extraction_job(invalid.job_id, "runner-2", 60_000)
            .expect("retry should claim");
        let bounded_chunk = ContentChunkWrite {
            ordinal: 0,
            text: "abcd".to_string(),
            provenance: ContentChunkProvenanceWrite::ByteRange { start: 0, end: 4 },
            trust_class: "untrusted_extracted_text",
        };
        let error = database
            .complete_extraction_job(
                invalid.job_id,
                "runner-2",
                "deskgraph.utf8-text",
                "1",
                4,
                Some(1),
                MAX_EXTRACTION_OUTPUT_BYTES + 1,
                1,
                std::slice::from_ref(&bounded_chunk),
            )
            .expect_err("database must enforce the absolute output cap");
        assert!(matches!(error, DatabaseError::ExtractionOutputInvalid));
        let error = database
            .complete_extraction_job(
                invalid.job_id,
                "runner-2",
                "deskgraph.utf8-text",
                "1",
                4,
                Some(1),
                1,
                1,
                &[bounded_chunk],
            )
            .expect_err("declared output bytes must match staged chunk bytes");
        assert!(matches!(error, DatabaseError::ExtractionOutputInvalid));
        assert_eq!(
            database
                .extraction_stats()
                .expect("stats should load")
                .active_chunk_count,
            2
        );
        let invalid_chunk = ContentChunkWrite {
            ordinal: 0,
            text: "bad".to_string(),
            provenance: ContentChunkProvenanceWrite::ByteRange { start: 0, end: 3 },
            trust_class: "trusted",
        };
        let error = database
            .complete_extraction_job(
                invalid.job_id,
                "runner-2",
                "deskgraph.utf8-text",
                "1",
                4,
                Some(1),
                3,
                1,
                &[invalid_chunk],
            )
            .expect_err("invalid trust class must not publish");
        assert!(matches!(error, DatabaseError::ExtractionOutputInvalid));
        assert_eq!(
            database
                .extraction_stats()
                .expect("stats should load")
                .active_chunk_count,
            2
        );
        database
            .fail_extraction_job(
                invalid.job_id,
                "runner-2",
                "deskgraph.utf8-text",
                "1",
                "extraction_output_invalid",
                1,
            )
            .expect("invalid job should fail");

        let replacement = database
            .create_extraction_job(scope_id, node_id)
            .expect("replacement should create");
        database
            .claim_extraction_job(replacement.job_id, "runner-3", 60_000)
            .expect("replacement should claim");
        let replacement_chunk = ContentChunkWrite {
            ordinal: 0,
            text: "abcd".to_string(),
            provenance: ContentChunkProvenanceWrite::ByteRange { start: 0, end: 4 },
            trust_class: "untrusted_extracted_text",
        };
        database
            .complete_extraction_job(
                replacement.job_id,
                "runner-3",
                "deskgraph.utf8-text",
                "1",
                4,
                Some(1),
                4,
                1,
                &[replacement_chunk],
            )
            .expect("replacement should publish");
        let stats = database.extraction_stats().expect("stats should load");
        assert_eq!(stats.active_chunk_count, 1);
        assert_eq!(stats.extracted_file_count, 1);
        let inactive: i64 = database
            .connection
            .query_row(
                "SELECT COUNT(*) FROM content_chunks WHERE active = 0",
                [],
                |row| row.get(0),
            )
            .expect("inactive chunks should count");
        assert_eq!(inactive, 2);
    }

    #[test]
    fn image_metadata_is_structured_validated_and_atomically_replaced() {
        let (mut database, scope_id, node_id, _) = extraction_setup();
        let image_job = database
            .create_extraction_job(scope_id, node_id)
            .expect("image job should create");
        database
            .claim_extraction_job(image_job.job_id, "image-runner", 60_000)
            .expect("image job should claim");
        database
            .complete_extraction_job_with_image_metadata(
                image_job.job_id,
                "image-runner",
                "deskgraph.image-metadata",
                "1",
                4,
                Some(1),
                0,
                1,
                &[],
                Some(&ImageMetadataWrite {
                    format: ImageFormat::Png,
                    pixel_width: 2,
                    pixel_height: 2,
                }),
            )
            .expect("valid image metadata should publish");
        let stored = database
            .image_metadata_for_job(image_job.job_id)
            .expect("image metadata should load");
        assert_eq!(stored.format, ImageFormat::Png);
        assert_eq!((stored.pixel_width, stored.pixel_height), (2, 2));
        let stats = database.extraction_stats().expect("stats should load");
        assert_eq!(stats.active_chunk_count, 0);
        assert_eq!(stats.extracted_file_count, 1);

        let invalid_job = database
            .create_extraction_job(scope_id, node_id)
            .expect("invalid retry should create");
        database
            .claim_extraction_job(invalid_job.job_id, "invalid-image", 60_000)
            .expect("invalid retry should claim");
        let error = database
            .complete_extraction_job_with_image_metadata(
                invalid_job.job_id,
                "invalid-image",
                "deskgraph.image-metadata",
                "1",
                4,
                Some(1),
                0,
                1,
                &[],
                Some(&ImageMetadataWrite {
                    format: ImageFormat::Png,
                    pixel_width: 25_000,
                    pixel_height: 25_000,
                }),
            )
            .expect_err("dimension bomb must not publish");
        assert!(matches!(error, DatabaseError::ExtractionOutputInvalid));
        assert_eq!(
            database
                .connection
                .query_row(
                    "SELECT COUNT(*) FROM image_metadata WHERE active = 1",
                    [],
                    |row| row.get::<_, i64>(0),
                )
                .expect("active metadata should count"),
            1
        );
        database
            .fail_extraction_job(
                invalid_job.job_id,
                "invalid-image",
                "deskgraph.image-metadata",
                "1",
                "extraction_output_invalid",
                1,
            )
            .expect("invalid retry should fail");

        let text_job = database
            .create_extraction_job(scope_id, node_id)
            .expect("replacement should create");
        database
            .claim_extraction_job(text_job.job_id, "text-runner", 60_000)
            .expect("replacement should claim");
        database
            .complete_extraction_job(
                text_job.job_id,
                "text-runner",
                "deskgraph.utf8-text",
                "1",
                4,
                Some(1),
                4,
                1,
                &[ContentChunkWrite {
                    ordinal: 0,
                    text: "text".to_string(),
                    provenance: ContentChunkProvenanceWrite::ByteRange { start: 0, end: 4 },
                    trust_class: "untrusted_extracted_text",
                }],
            )
            .expect("text replacement should publish atomically");
        assert_eq!(
            database
                .connection
                .query_row(
                    "SELECT COUNT(*) FROM image_metadata WHERE active = 1",
                    [],
                    |row| row.get::<_, i64>(0),
                )
                .expect("active metadata should count"),
            0
        );
        assert_eq!(
            database
                .image_metadata_for_job(image_job.job_id)
                .expect("historical metadata should remain queryable")
                .pixel_width,
            2
        );
    }

    #[test]
    fn ocr_jobs_preserve_image_metadata_and_replace_only_ocr_chunks() {
        let (mut database, scope_id, node_id, _) = extraction_setup();
        let image_job = database
            .create_extraction_job(scope_id, node_id)
            .expect("image job should create");
        assert_eq!(image_job.operation, ExtractionOperation::Content);
        database
            .claim_extraction_job(image_job.job_id, "image-runner", 60_000)
            .expect("image job should claim");
        database
            .complete_extraction_job_with_image_metadata(
                image_job.job_id,
                "image-runner",
                "deskgraph.image-metadata",
                "1",
                4,
                Some(1),
                0,
                1,
                &[],
                Some(&ImageMetadataWrite {
                    format: ImageFormat::Png,
                    pixel_width: 2,
                    pixel_height: 2,
                }),
            )
            .expect("metadata should publish");

        let first_ocr = database
            .create_screenshot_ocr_job(scope_id, node_id)
            .expect("OCR job should create");
        assert_eq!(first_ocr.operation, ExtractionOperation::ScreenshotOcr);
        database
            .claim_extraction_job(first_ocr.job_id, "ocr-runner-1", 60_000)
            .expect("OCR job should claim");
        let first_text = "DeskGraph 桌面圖譜";
        database
            .complete_extraction_job(
                first_ocr.job_id,
                "ocr-runner-1",
                "deskgraph.macos-vision-ocr",
                "1",
                4,
                Some(1),
                u64::try_from(first_text.len()).expect("text length should fit"),
                2,
                &[ContentChunkWrite {
                    ordinal: 0,
                    text: first_text.to_string(),
                    provenance: ContentChunkProvenanceWrite::OcrObservation {
                        observation_number: 1,
                        fragment_index: 0,
                        bbox_x_ppm: 50_000,
                        bbox_y_ppm: 100_000,
                        bbox_width_ppm: 400_000,
                        bbox_height_ppm: 200_000,
                        confidence_basis_points: Some(9_876),
                    },
                    trust_class: "untrusted_extracted_text",
                }],
            )
            .expect("OCR chunks should publish");
        assert_eq!(
            database
                .connection
                .query_row(
                    "SELECT COUNT(*) FROM image_metadata WHERE active = 1",
                    [],
                    |row| { row.get::<_, i64>(0) }
                )
                .expect("metadata should count"),
            1
        );
        let stored: (String, i64, i64, i64, i64, i64, i64) = database
            .connection
            .query_row(
                "SELECT provenance_kind, source_unit_number, source_bbox_x_ppm, \
                    source_bbox_y_ppm, source_bbox_width_ppm, source_bbox_height_ppm, \
                    source_confidence_basis_points \
                 FROM content_chunks WHERE active = 1 AND provenance_kind = 'ocr_observation'",
                [],
                |row| {
                    Ok((
                        row.get(0)?,
                        row.get(1)?,
                        row.get(2)?,
                        row.get(3)?,
                        row.get(4)?,
                        row.get(5)?,
                        row.get(6)?,
                    ))
                },
            )
            .expect("OCR provenance should load");
        assert_eq!(
            stored,
            (
                "ocr_observation".to_string(),
                1,
                50_000,
                100_000,
                400_000,
                200_000,
                9_876,
            )
        );

        let metadata_refresh = database
            .create_extraction_job(scope_id, node_id)
            .expect("metadata refresh should create");
        database
            .claim_extraction_job(metadata_refresh.job_id, "image-runner-2", 60_000)
            .expect("metadata refresh should claim");
        database
            .complete_extraction_job_with_image_metadata(
                metadata_refresh.job_id,
                "image-runner-2",
                "deskgraph.image-metadata",
                "1",
                4,
                Some(1),
                0,
                1,
                &[],
                Some(&ImageMetadataWrite {
                    format: ImageFormat::Png,
                    pixel_width: 2,
                    pixel_height: 2,
                }),
            )
            .expect("metadata refresh should preserve OCR");
        assert_eq!(
            database
                .connection
                .query_row(
                    "SELECT COUNT(*) FROM content_chunks \
                     WHERE active = 1 AND provenance_kind = 'ocr_observation'",
                    [],
                    |row| row.get::<_, i64>(0),
                )
                .expect("active OCR should count"),
            1
        );

        let second_ocr = database
            .create_screenshot_ocr_job(scope_id, node_id)
            .expect("replacement OCR should create");
        database
            .claim_extraction_job(second_ocr.job_id, "ocr-runner-2", 60_000)
            .expect("replacement OCR should claim");
        database
            .complete_extraction_job(
                second_ocr.job_id,
                "ocr-runner-2",
                "deskgraph.macos-vision-ocr",
                "1",
                4,
                Some(1),
                0,
                1,
                &[],
            )
            .expect("no-text OCR should atomically replace prior OCR");
        assert_eq!(
            database
                .connection
                .query_row(
                    "SELECT COUNT(*) FROM content_chunks \
                     WHERE active = 1 AND provenance_kind = 'ocr_observation'",
                    [],
                    |row| row.get::<_, i64>(0),
                )
                .expect("active OCR should count"),
            0
        );
        assert_eq!(
            database
                .connection
                .query_row(
                    "SELECT COUNT(*) FROM image_metadata WHERE active = 1",
                    [],
                    |row| { row.get::<_, i64>(0) }
                )
                .expect("metadata should count"),
            1
        );

        let invalid = database
            .create_screenshot_ocr_job(scope_id, node_id)
            .expect("invalid OCR job should create");
        database
            .claim_extraction_job(invalid.job_id, "ocr-invalid", 60_000)
            .expect("invalid OCR job should claim");
        let error = database
            .complete_extraction_job(
                invalid.job_id,
                "ocr-invalid",
                "deskgraph.macos-vision-ocr",
                "1",
                4,
                Some(1),
                4,
                1,
                &[ContentChunkWrite {
                    ordinal: 0,
                    text: "text".to_string(),
                    provenance: ContentChunkProvenanceWrite::ByteRange { start: 0, end: 4 },
                    trust_class: "untrusted_extracted_text",
                }],
            )
            .expect_err("OCR jobs must reject non-OCR provenance");
        assert!(matches!(error, DatabaseError::ExtractionOutputInvalid));
        let error = database
            .complete_extraction_job(
                invalid.job_id,
                "ocr-invalid",
                "deskgraph.macos-vision-ocr",
                "1",
                4,
                Some(1),
                4,
                1,
                &[ContentChunkWrite {
                    ordinal: 0,
                    text: "text".to_string(),
                    provenance: ContentChunkProvenanceWrite::OcrObservation {
                        observation_number: 1,
                        fragment_index: 0,
                        bbox_x_ppm: 900_000,
                        bbox_y_ppm: 0,
                        bbox_width_ppm: 200_000,
                        bbox_height_ppm: 1,
                        confidence_basis_points: Some(10_000),
                    },
                    trust_class: "untrusted_extracted_text",
                }],
            )
            .expect_err("out-of-bounds OCR provenance must not publish");
        assert!(matches!(error, DatabaseError::ExtractionOutputInvalid));
        database
            .fail_extraction_job(
                invalid.job_id,
                "ocr-invalid",
                "deskgraph.macos-vision-ocr",
                "1",
                "extraction_output_invalid",
                1,
            )
            .expect("invalid OCR job should close");

        let invalid_content = database
            .create_extraction_job(scope_id, node_id)
            .expect("content job should create");
        database
            .claim_extraction_job(invalid_content.job_id, "content-invalid", 60_000)
            .expect("content job should claim");
        let error = database
            .complete_extraction_job(
                invalid_content.job_id,
                "content-invalid",
                "deskgraph.macos-vision-ocr",
                "1",
                4,
                Some(1),
                4,
                1,
                &[ContentChunkWrite {
                    ordinal: 0,
                    text: "text".to_string(),
                    provenance: ContentChunkProvenanceWrite::OcrObservation {
                        observation_number: 1,
                        fragment_index: 0,
                        bbox_x_ppm: 0,
                        bbox_y_ppm: 0,
                        bbox_width_ppm: 1,
                        bbox_height_ppm: 1,
                        confidence_basis_points: Some(10_000),
                    },
                    trust_class: "untrusted_extracted_text",
                }],
            )
            .expect_err("content jobs must reject OCR provenance");
        assert!(matches!(error, DatabaseError::ExtractionOutputInvalid));
    }

    #[test]
    fn ocr_confidence_absence_is_preserved_and_bbox_remains_required() {
        let (mut database, scope_id, node_id, _) = extraction_setup();
        let job = database
            .create_screenshot_ocr_job(scope_id, node_id)
            .expect("OCR job should create");
        database
            .claim_extraction_job(job.job_id, "windows-ocr-runner", 60_000)
            .expect("OCR job should claim");
        database
            .complete_extraction_job(
                job.job_id,
                "windows-ocr-runner",
                "deskgraph.windows-media-ocr",
                "1",
                4,
                Some(1),
                4,
                1,
                &[ContentChunkWrite {
                    ordinal: 0,
                    text: "text".to_string(),
                    provenance: ContentChunkProvenanceWrite::OcrObservation {
                        observation_number: 1,
                        fragment_index: 0,
                        bbox_x_ppm: 10_000,
                        bbox_y_ppm: 20_000,
                        bbox_width_ppm: 300_000,
                        bbox_height_ppm: 100_000,
                        confidence_basis_points: None,
                    },
                    trust_class: "untrusted_extracted_text",
                }],
            )
            .expect("provider without confidence should publish honest provenance");

        let stored: (Option<i64>, i64) = database
            .connection
            .query_row(
                "SELECT source_confidence_basis_points, source_bbox_x_ppm \
                 FROM content_chunks WHERE extraction_job_id = ?1",
                [job.job_id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .expect("OCR provenance should load");
        assert_eq!(stored, (None, 10_000));

        database
            .connection
            .execute(
                "UPDATE content_chunks SET source_bbox_x_ppm = NULL \
                 WHERE extraction_job_id = ?1",
                [job.job_id],
            )
            .expect_err("database guard must reject missing OCR bounds");
        assert_eq!(
            database
                .connection
                .query_row(
                    "SELECT source_bbox_x_ppm FROM content_chunks \
                     WHERE extraction_job_id = ?1",
                    [job.job_id],
                    |row| row.get::<_, i64>(0),
                )
                .expect("valid OCR bounds should remain"),
            10_000
        );
    }

    #[test]
    fn complete_extraction_stores_page_provenance_without_fake_byte_offsets() {
        let (mut database, scope_id, node_id, _) = extraction_setup();
        let job = database
            .create_extraction_job(scope_id, node_id)
            .expect("job should create");
        database
            .claim_extraction_job(job.job_id, "pdf-runner", 60_000)
            .expect("job should claim");
        database
            .complete_extraction_job(
                job.job_id,
                "pdf-runner",
                "deskgraph.pdf-text",
                "1",
                4,
                Some(1),
                4,
                1,
                &[ContentChunkWrite {
                    ordinal: 0,
                    text: "page".to_string(),
                    provenance: ContentChunkProvenanceWrite::PdfPage {
                        page_number: 2,
                        fragment_index: 0,
                    },
                    trust_class: "untrusted_extracted_text",
                }],
            )
            .expect("page provenance should publish");
        let stored: (String, Option<i64>, Option<i64>, Option<i64>, Option<i64>) = database
            .connection
            .query_row(
                "SELECT provenance_kind, source_byte_start, source_byte_end, source_page_number, source_fragment_index FROM content_chunks WHERE active = 1",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?)),
            )
            .expect("page provenance should load");

        assert_eq!(
            stored,
            ("pdf_page".to_string(), None, None, Some(2), Some(0))
        );
    }

    #[test]
    fn complete_extraction_stores_and_validates_office_provenance() {
        let (mut database, scope_id, node_id, _) = extraction_setup();
        let job = database
            .create_extraction_job(scope_id, node_id)
            .expect("job should create");
        database
            .claim_extraction_job(job.job_id, "office-runner", 60_000)
            .expect("job should claim");
        let chunks = [
            ContentChunkWrite {
                ordinal: 0,
                text: "docx".to_string(),
                provenance: ContentChunkProvenanceWrite::DocxParagraph {
                    paragraph_number: 2,
                    fragment_index: 0,
                },
                trust_class: "untrusted_extracted_text",
            },
            ContentChunkWrite {
                ordinal: 1,
                text: "pptx".to_string(),
                provenance: ContentChunkProvenanceWrite::PptxSlide {
                    slide_number: 3,
                    fragment_index: 1,
                },
                trust_class: "untrusted_extracted_text",
            },
            ContentChunkWrite {
                ordinal: 2,
                text: "cell".to_string(),
                provenance: ContentChunkProvenanceWrite::XlsxCell {
                    sheet_number: 4,
                    cell_reference: "XFD1048576".to_string(),
                    fragment_index: 2,
                },
                trust_class: "untrusted_extracted_text",
            },
        ];
        database
            .complete_extraction_job(
                job.job_id,
                "office-runner",
                "deskgraph.ooxml-text",
                "1",
                4,
                Some(1),
                12,
                1,
                &chunks,
            )
            .expect("Office provenance should publish");
        let mut statement = database
            .connection
            .prepare(
                "SELECT provenance_kind, source_unit_number, source_cell_reference, source_fragment_index \
                 FROM content_chunks WHERE active = 1 ORDER BY ordinal",
            )
            .expect("provenance query should prepare");
        let stored = statement
            .query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, Option<i64>>(1)?,
                    row.get::<_, Option<String>>(2)?,
                    row.get::<_, Option<i64>>(3)?,
                ))
            })
            .expect("provenance should query")
            .collect::<Result<Vec<_>, _>>()
            .expect("provenance rows should load");
        assert_eq!(
            stored,
            vec![
                ("docx_paragraph".to_string(), Some(2), None, Some(0)),
                ("pptx_slide".to_string(), Some(3), None, Some(1)),
                (
                    "xlsx_cell".to_string(),
                    Some(4),
                    Some("XFD1048576".to_string()),
                    Some(2),
                ),
            ]
        );
        drop(statement);

        let invalid = database
            .create_extraction_job(scope_id, node_id)
            .expect("invalid job should create");
        database
            .claim_extraction_job(invalid.job_id, "invalid-office-runner", 60_000)
            .expect("invalid job should claim");
        let error = database
            .complete_extraction_job(
                invalid.job_id,
                "invalid-office-runner",
                "deskgraph.ooxml-text",
                "1",
                4,
                Some(1),
                1,
                1,
                &[ContentChunkWrite {
                    ordinal: 0,
                    text: "x".to_string(),
                    provenance: ContentChunkProvenanceWrite::XlsxCell {
                        sheet_number: 1,
                        cell_reference: "XFE1".to_string(),
                        fragment_index: 0,
                    },
                    trust_class: "untrusted_extracted_text",
                }],
            )
            .expect_err("out-of-range cell reference must not publish");
        assert!(matches!(error, DatabaseError::ExtractionOutputInvalid));
        assert_eq!(
            database
                .extraction_stats()
                .expect("stats should remain available")
                .active_chunk_count,
            3
        );
    }

    #[test]
    fn manifest_change_invalidates_prior_content_chunks() {
        let (mut database, scope_id, node_id, root) = extraction_setup();
        let job = database
            .create_extraction_job(scope_id, node_id)
            .expect("job should create");
        database
            .claim_extraction_job(job.job_id, "runner", 60_000)
            .expect("job should claim");
        database
            .complete_extraction_job(
                job.job_id,
                "runner",
                "deskgraph.utf8-text",
                "1",
                4,
                Some(1),
                4,
                1,
                &[ContentChunkWrite {
                    ordinal: 0,
                    text: "abcd".to_string(),
                    provenance: ContentChunkProvenanceWrite::ByteRange { start: 0, end: 4 },
                    trust_class: "untrusted_extracted_text",
                }],
            )
            .expect("chunks should publish");

        let rescanned_node = publish_manifest_file(&mut database, scope_id, &root, 5);

        assert_eq!(rescanned_node, node_id);
        assert_eq!(
            database
                .extraction_stats()
                .expect("stats should load")
                .active_chunk_count,
            0
        );
    }

    #[test]
    fn expired_extraction_runner_requires_explicit_resume() {
        let (mut database, scope_id, node_id, _) = extraction_setup();
        let job = database
            .create_extraction_job(scope_id, node_id)
            .expect("job should create");
        database
            .claim_extraction_job(job.job_id, "old-runner", 60_000)
            .expect("job should claim");

        assert_eq!(
            database
                .recover_expired_extraction_jobs_at(i64::MAX)
                .expect("job should recover"),
            1
        );
        assert_eq!(
            database
                .extraction_job(job.job_id)
                .expect("job should load")
                .status,
            ExtractionStatus::Interrupted
        );
        let resumed = database
            .resume_extraction_job(job.job_id)
            .expect("job should resume");
        assert_eq!(resumed.status, ExtractionStatus::Queued);
        database
            .claim_extraction_job(job.job_id, "new-runner", 60_000)
            .expect("resumed job should claim");
    }
}
