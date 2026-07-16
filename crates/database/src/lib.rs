use std::fmt;
use std::fs;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use deskgraph_domain::{AuthorizedScope, ManifestStats, ScanJobProgress, ScanReport, ScanStatus};
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
];

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
        self.scope_record(scope_id)?;
        let now = unix_ms()?;
        let transaction = self.connection.transaction()?;
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
             WHERE id = ?1 AND runner_token = ?2 AND status = 'running'",
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

fn row_u64(row: &rusqlite::Row<'_>, index: usize) -> rusqlite::Result<u64> {
    let value: i64 = row.get(index)?;
    u64::try_from(value).map_err(|_| rusqlite::Error::IntegralValueOutOfRange(index, value))
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

    #[test]
    fn migrations_initialize_manifest_schema() {
        let database = ManifestDatabase::open_in_memory().expect("database should initialize");
        let stats = database.stats().expect("stats should be readable");

        assert!(stats.database_ready);
        assert_eq!(stats.authorized_scope_count, 0);
        assert_eq!(stats.node_count, 0);
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
}
