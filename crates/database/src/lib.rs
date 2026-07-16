use std::fmt;
use std::fs;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use deskgraph_domain::{AuthorizedScope, ManifestStats, ScanReport, ScanStatus};
use rusqlite::{Connection, OptionalExtension, Transaction, params};

const MIGRATIONS: &[Migration] = &[Migration {
    version: 1,
    name: "manifest",
    sql: include_str!("../../../migrations/0001_manifest.sql"),
}];

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

#[derive(Debug)]
pub enum DatabaseError {
    Io(std::io::Error),
    Sqlite(rusqlite::Error),
    MigrationChanged { version: i64 },
    ScopeNotFound,
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
}
