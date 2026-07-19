use std::ffi::OsStr;
use std::fmt;
use std::fs::{self, Metadata};
use std::path::Path;
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use deskgraph_database::{
    CoverageRootAccessGrantWrite, DatabaseError, ManifestDatabase, NodeKind, Observation,
    QueueEntry, QueuedPath, ScanIssue, ScopeAccessGrantState, ScopeAccessGrantWrite,
};
use deskgraph_domain::{AuthorizedScope, ScanJobProgress, ScanReport, ScanStatus};
pub use deskgraph_identity::comparison_key;
use deskgraph_identity::{
    IdentityNodeKind, fallback_identity, has_hidden_or_system_attribute,
    is_symlink_or_reparse_point, path_from_raw, path_to_raw, platform_identity,
};

#[derive(Debug)]
pub enum ScannerError {
    Database(DatabaseError),
    CanonicalizationFailed,
    ScopeIsNotDirectory,
    ProtectedSystemScope,
    ScopeChanged,
    ScopePathDecodeFailed,
    CoverageSetEmpty,
    CoverageSetTooLarge,
    CoverageRootOverlap,
    ScanFailed,
    InvalidBatchSize,
    ScanNotCompleted,
}

impl ScannerError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::Database(error) => error.code(),
            Self::CanonicalizationFailed => "scope_canonicalization_failed",
            Self::ScopeIsNotDirectory => "scope_is_not_directory",
            Self::ProtectedSystemScope => "protected_system_scope_denied",
            Self::ScopeChanged => "authorized_scope_identity_changed",
            Self::ScopePathDecodeFailed => "scope_path_decode_failed",
            Self::CoverageSetEmpty => "coverage_set_empty",
            Self::CoverageSetTooLarge => "coverage_set_too_large",
            Self::CoverageRootOverlap => "coverage_root_overlap",
            Self::ScanFailed => "metadata_scan_failed",
            Self::InvalidBatchSize => "scan_batch_size_out_of_range",
            Self::ScanNotCompleted => "scan_job_not_completed",
        }
    }
}

const DEFAULT_BATCH_SIZE: usize = 256;
const MAX_BATCH_SIZE: usize = 10_000;
pub const MAX_COVERAGE_ROOTS_PER_SELECTION: usize = 32;
const RUNNER_LEASE_MS: i64 = 30_000;

/// One native-picker result in a user-confirmed coverage-set transaction.
/// The opaque grant remains backend-only and is never returned by this API.
#[derive(Clone, Copy)]
pub struct CoverageRootAuthorizationRequest<'a> {
    pub requested_path: &'a Path,
    pub grant_platform: &'a str,
    pub opaque_grant: &'a [u8],
}

impl fmt::Display for ScannerError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.code())
    }
}

impl std::error::Error for ScannerError {}

impl From<DatabaseError> for ScannerError {
    fn from(error: DatabaseError) -> Self {
        Self::Database(error)
    }
}

pub fn authorize_scope(
    database: &ManifestDatabase,
    requested_path: &Path,
) -> Result<AuthorizedScope, ScannerError> {
    let canonical = validated_requested_scope(requested_path)?;
    let path_key = comparison_key(&canonical);
    database
        .add_scope(
            &path_to_raw(&canonical),
            &path_key,
            &canonical.to_string_lossy(),
            std::env::consts::OS,
        )
        .map_err(Into::into)
}

/// Persists a native user selection and its platform capability atomically.
/// The opaque bytes remain backend-only and are never part of
/// [`AuthorizedScope`].
pub fn authorize_scope_with_access_grant(
    database: &mut ManifestDatabase,
    requested_path: &Path,
    grant_platform: &str,
    opaque_grant: &[u8],
) -> Result<AuthorizedScope, ScannerError> {
    let canonical = validated_requested_scope(requested_path)?;
    let path_key = comparison_key(&canonical);
    database
        .add_scope_with_access_grant(
            &path_to_raw(&canonical),
            &path_key,
            &canonical.to_string_lossy(),
            ScopeAccessGrantWrite {
                scope_platform: std::env::consts::OS,
                grant_platform,
                opaque_grant,
                state: ScopeAccessGrantState::Active,
            },
        )
        .map_err(Into::into)
}

/// Validates every selected root before committing any scope or grant. Exact
/// duplicates and ancestor/descendant roots are rejected as one set so the
/// manifest cannot index the same subtree through overlapping coverage roots.
pub fn authorize_coverage_roots_with_access_grants(
    database: &mut ManifestDatabase,
    requests: &[CoverageRootAuthorizationRequest<'_>],
) -> Result<Vec<AuthorizedScope>, ScannerError> {
    if requests.is_empty() {
        return Err(ScannerError::CoverageSetEmpty);
    }
    if requests.len() > MAX_COVERAGE_ROOTS_PER_SELECTION {
        return Err(ScannerError::CoverageSetTooLarge);
    }

    let mut canonical_roots = Vec::with_capacity(requests.len());
    for request in requests {
        canonical_roots.push(validated_requested_scope(request.requested_path)?);
    }
    for (index, root) in canonical_roots.iter().enumerate() {
        if canonical_roots
            .iter()
            .skip(index + 1)
            .any(|other| coverage_roots_overlap(root, other))
        {
            return Err(ScannerError::CoverageRootOverlap);
        }
    }

    for existing in database.list_active_scope_records()? {
        if existing.platform != std::env::consts::OS {
            continue;
        }
        let existing_root =
            path_from_raw(&existing.path_raw).map_err(|_| ScannerError::ScopePathDecodeFailed)?;
        for selected_root in &canonical_roots {
            if comparison_key(selected_root) != comparison_key(&existing_root)
                && coverage_roots_overlap(selected_root, &existing_root)
            {
                return Err(ScannerError::CoverageRootOverlap);
            }
        }
    }

    let path_raw = canonical_roots
        .iter()
        .map(|root| path_to_raw(root))
        .collect::<Vec<_>>();
    let path_keys = canonical_roots
        .iter()
        .map(|root| comparison_key(root))
        .collect::<Vec<_>>();
    let display_paths = canonical_roots
        .iter()
        .map(|root| root.to_string_lossy().into_owned())
        .collect::<Vec<_>>();
    let writes = requests
        .iter()
        .enumerate()
        .map(|(index, request)| CoverageRootAccessGrantWrite {
            path_raw: &path_raw[index],
            path_key: &path_keys[index],
            display_path: &display_paths[index],
            grant: ScopeAccessGrantWrite {
                scope_platform: std::env::consts::OS,
                grant_platform: request.grant_platform,
                opaque_grant: request.opaque_grant,
                state: ScopeAccessGrantState::Active,
            },
        })
        .collect::<Vec<_>>();
    database
        .add_coverage_roots_with_access_grants(&writes)
        .map_err(Into::into)
}

fn coverage_roots_overlap(left: &Path, right: &Path) -> bool {
    let left_key = comparison_key(left);
    let right_key = comparison_key(right);
    left.ancestors()
        .any(|ancestor| comparison_key(ancestor) == right_key)
        || right
            .ancestors()
            .any(|ancestor| comparison_key(ancestor) == left_key)
}

fn validated_requested_scope(requested_path: &Path) -> Result<std::path::PathBuf, ScannerError> {
    let canonical =
        fs::canonicalize(requested_path).map_err(|_| ScannerError::CanonicalizationFailed)?;
    let metadata =
        fs::symlink_metadata(&canonical).map_err(|_| ScannerError::CanonicalizationFailed)?;
    if !metadata.is_dir() {
        return Err(ScannerError::ScopeIsNotDirectory);
    }
    if is_protected_system_scope(&canonical) {
        return Err(ScannerError::ProtectedSystemScope);
    }
    Ok(canonical)
}

pub fn scan_scope(
    database: &mut ManifestDatabase,
    scope_id: i64,
) -> Result<ScanReport, ScannerError> {
    let job = create_scan_job(database, scope_id)?;
    let completed = run_scan_job_to_terminal(database, job.job_id)?;
    ScanReport::try_from(completed).map_err(|_| ScannerError::ScanNotCompleted)
}

pub fn create_scan_job(
    database: &mut ManifestDatabase,
    scope_id: i64,
) -> Result<ScanJobProgress, ScannerError> {
    let canonical_root = validated_scope_root(database, scope_id)?;
    database
        .create_resumable_scan_job(
            scope_id,
            &QueuedPath {
                path_raw: path_to_raw(&canonical_root),
                path_key: comparison_key(&canonical_root),
                parent_identity_key: None,
                is_root: true,
            },
        )
        .map_err(Into::into)
}

pub fn run_scan_job_batch(
    database: &mut ManifestDatabase,
    job_id: i64,
    batch_size: usize,
) -> Result<ScanJobProgress, ScannerError> {
    if batch_size == 0 || batch_size > MAX_BATCH_SIZE {
        return Err(ScannerError::InvalidBatchSize);
    }
    let current = database.scan_job(job_id)?;
    if current.is_terminal()
        || matches!(current.status, ScanStatus::Paused | ScanStatus::Interrupted)
    {
        return Ok(current);
    }
    let canonical_root = validated_scope_root(database, current.scope_id)?;
    let runner_token = runner_token()?;
    database.claim_scan_job(job_id, &runner_token, RUNNER_LEASE_MS)?;
    let batch_started = Instant::now();

    for _ in 0..batch_size {
        let progress = database.scan_job(job_id)?;
        if progress.pause_requested {
            persist_batch_elapsed(database, job_id, &runner_token, batch_started)?;
            return database
                .release_scan_job(job_id, &runner_token)
                .map_err(Into::into);
        }
        let Some(entry) = database.next_scan_queue_entry(job_id, &runner_token, RUNNER_LEASE_MS)?
        else {
            persist_batch_elapsed(database, job_id, &runner_token, batch_started)?;
            return database
                .finalize_resumable_scan_job(job_id, &runner_token)
                .map_err(Into::into);
        };
        let processed = match process_queue_entry(&canonical_root, &entry) {
            Ok(processed) => processed,
            Err(error) => {
                persist_batch_elapsed(database, job_id, &runner_token, batch_started)?;
                database.fail_resumable_scan_job(job_id, &runner_token)?;
                return Err(error);
            }
        };
        database.stage_scan_queue_entry(
            job_id,
            &runner_token,
            entry.id,
            processed.observation.as_ref(),
            &processed.children,
            &processed.issues,
            processed.skipped_entries,
            0,
            RUNNER_LEASE_MS,
        )?;
    }

    persist_batch_elapsed(database, job_id, &runner_token, batch_started)?;
    database
        .release_scan_job(job_id, &runner_token)
        .map_err(Into::into)
}

pub fn run_scan_job_to_terminal(
    database: &mut ManifestDatabase,
    job_id: i64,
) -> Result<ScanJobProgress, ScannerError> {
    loop {
        let progress = run_scan_job_batch(database, job_id, DEFAULT_BATCH_SIZE)?;
        if progress.is_terminal()
            || matches!(
                progress.status,
                ScanStatus::Paused | ScanStatus::Interrupted
            )
        {
            return Ok(progress);
        }
    }
}

pub fn pause_scan_job(
    database: &mut ManifestDatabase,
    job_id: i64,
) -> Result<ScanJobProgress, ScannerError> {
    database.request_scan_pause(job_id).map_err(Into::into)
}

pub fn resume_scan_job(
    database: &mut ManifestDatabase,
    job_id: i64,
) -> Result<ScanJobProgress, ScannerError> {
    let progress = database.scan_job(job_id)?;
    validated_scope_root(database, progress.scope_id)?;
    database.resume_scan_job(job_id).map_err(Into::into)
}

pub fn validated_scope_root(
    database: &ManifestDatabase,
    scope_id: i64,
) -> Result<std::path::PathBuf, ScannerError> {
    let scope = database.scope_record(scope_id)?;
    let stored_root =
        path_from_raw(&scope.path_raw).map_err(|_| ScannerError::ScopePathDecodeFailed)?;
    let canonical_root =
        fs::canonicalize(&stored_root).map_err(|_| ScannerError::CanonicalizationFailed)?;
    if comparison_key(&canonical_root) != scope.path_key {
        return Err(ScannerError::ScopeChanged);
    }
    if is_protected_system_scope(&canonical_root) {
        return Err(ScannerError::ProtectedSystemScope);
    }
    Ok(canonical_root)
}

struct ProcessedQueueEntry {
    observation: Option<Observation>,
    children: Vec<QueuedPath>,
    issues: Vec<ScanIssue>,
    skipped_entries: u64,
}

fn process_queue_entry(
    root: &Path,
    entry: &QueueEntry,
) -> Result<ProcessedQueueEntry, ScannerError> {
    let path = path_from_raw(&entry.path_raw).map_err(|_| ScannerError::ScopePathDecodeFailed)?;
    if comparison_key(&path) != entry.path_key {
        return Err(ScannerError::ScopeChanged);
    }
    if !entry.is_root && is_temporary_download_path(&path) {
        return Ok(ProcessedQueueEntry {
            observation: None,
            children: Vec::new(),
            issues: vec![issue("temporary_download_excluded", &path, None)],
            skipped_entries: 1,
        });
    }
    let mut issues = Vec::new();
    let mut skipped_entries = 0_u64;
    let metadata = match fs::symlink_metadata(&path) {
        Ok(metadata) => metadata,
        Err(error) => {
            if entry.is_root {
                return Err(ScannerError::ScanFailed);
            }
            issues.push(issue("metadata_unavailable", &path, io_error_code(&error)));
            return Ok(ProcessedQueueEntry {
                observation: None,
                children: Vec::new(),
                issues,
                skipped_entries: 1,
            });
        }
    };

    if !entry.is_root && (is_hidden(path.file_name()) || has_hidden_or_system_attribute(&metadata))
    {
        let code = if is_hidden(path.file_name()) {
            "hidden_entry_excluded"
        } else {
            "platform_hidden_or_system_excluded"
        };
        issues.push(issue(code, &path, None));
        return Ok(ProcessedQueueEntry {
            observation: None,
            children: Vec::new(),
            issues,
            skipped_entries: 1,
        });
    }
    if is_symlink_or_reparse_point(&metadata) {
        issues.push(issue("symlink_not_followed", &path, None));
        return Ok(ProcessedQueueEntry {
            observation: None,
            children: Vec::new(),
            issues,
            skipped_entries: 1,
        });
    }

    let canonical = match fs::canonicalize(&path) {
        Ok(canonical) => canonical,
        Err(error) => {
            issues.push(issue(
                "canonicalization_failed",
                &path,
                io_error_code(&error),
            ));
            return Ok(ProcessedQueueEntry {
                observation: None,
                children: Vec::new(),
                issues,
                skipped_entries: 1,
            });
        }
    };
    if !canonical.starts_with(root) {
        issues.push(issue("scope_escape_denied", &path, None));
        return Ok(ProcessedQueueEntry {
            observation: None,
            children: Vec::new(),
            issues,
            skipped_entries: 1,
        });
    }

    let kind = if metadata.is_dir() {
        NodeKind::Folder
    } else if metadata.is_file() {
        NodeKind::File
    } else {
        issues.push(issue("unsupported_entry_type", &path, None));
        return Ok(ProcessedQueueEntry {
            observation: None,
            children: Vec::new(),
            issues,
            skipped_entries: 1,
        });
    };
    let path_key = comparison_key(&canonical);
    let identity_kind = match kind {
        NodeKind::File => IdentityNodeKind::File,
        NodeKind::Folder => IdentityNodeKind::Folder,
    };
    let identity = platform_identity(&canonical, &metadata, identity_kind)
        .unwrap_or_else(|_| fallback_identity(&path_key, identity_kind));
    let identity_key = identity.key;

    let observation = Observation {
        kind,
        identity_kind: identity.kind.to_string(),
        identity_key: identity_key.clone(),
        parent_identity_key: entry.parent_identity_key.clone(),
        path_raw: path_to_raw(&canonical),
        path_key,
        display_path: canonical.to_string_lossy().into_owned(),
        size_bytes: if kind == NodeKind::File {
            metadata.len()
        } else {
            0
        },
        modified_unix_ns: modified_unix_ns(&metadata),
        link_count: identity.link_count,
    };

    let mut children = Vec::new();
    if kind == NodeKind::Folder {
        match fs::read_dir(&canonical) {
            Ok(entries) => {
                for child in entries {
                    match child {
                        Ok(child) => {
                            let child_path = child.path();
                            children.push(QueuedPath {
                                path_raw: path_to_raw(&child_path),
                                path_key: comparison_key(&child_path),
                                parent_identity_key: Some(identity_key.clone()),
                                is_root: false,
                            });
                        }
                        Err(error) => {
                            issues.push(ScanIssue {
                                code: "directory_entry_unavailable",
                                path_key: Some(comparison_key(&canonical)),
                                detail_code: io_error_code(&error),
                            });
                            skipped_entries = skipped_entries.saturating_add(1);
                        }
                    }
                }
                children.sort_by(|left, right| left.path_key.cmp(&right.path_key));
            }
            Err(error) => {
                issues.push(issue(
                    "directory_read_denied",
                    &canonical,
                    io_error_code(&error),
                ));
            }
        }
    }

    Ok(ProcessedQueueEntry {
        observation: Some(observation),
        children,
        issues,
        skipped_entries,
    })
}

fn runner_token() -> Result<String, ScannerError> {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| ScannerError::ScanFailed)?
        .as_nanos();
    Ok(format!("{}:{nanos}", std::process::id()))
}

fn persist_batch_elapsed(
    database: &mut ManifestDatabase,
    job_id: i64,
    runner_token: &str,
    started: Instant,
) -> Result<(), ScannerError> {
    let elapsed_ms = u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX);
    database.record_scan_runner_elapsed(job_id, runner_token, elapsed_ms)?;
    Ok(())
}

fn is_hidden(name: Option<&OsStr>) -> bool {
    name.is_some_and(|name| name.to_string_lossy().starts_with('.'))
}

pub fn is_temporary_download_path(path: &Path) -> bool {
    let filename = path
        .file_name()
        .unwrap_or_else(|| OsStr::new(""))
        .to_string_lossy()
        .to_ascii_lowercase();
    [".part", ".crdownload", ".download"]
        .iter()
        .any(|suffix| filename.ends_with(suffix))
}

#[cfg(unix)]
fn is_protected_system_scope(path: &Path) -> bool {
    const PROTECTED_TREES: &[&str] = &[
        "/System",
        "/Library",
        "/usr",
        "/bin",
        "/sbin",
        "/etc",
        "/var",
        "/dev",
        "/proc",
        "/sys",
        "/run",
        "/boot",
        "/private/etc",
        "/private/var/db",
        "/private/var/root",
        "/private/var/vm",
        "/private/var/protected",
    ];
    const PROTECTED_CONTAINER_ROOTS: &[&str] = &[
        "/Users", "/home", "/Volumes", "/mnt", "/media", "/Network", "/private", "/tmp",
    ];

    path == Path::new("/")
        || PROTECTED_TREES
            .iter()
            .any(|root| path.starts_with(Path::new(root)))
        || PROTECTED_CONTAINER_ROOTS
            .iter()
            .any(|root| path == Path::new(root))
}

#[cfg(windows)]
fn is_protected_system_scope(path: &Path) -> bool {
    use std::path::Component;

    if path.parent().is_none() {
        return true;
    }
    let components = path
        .components()
        .filter_map(|component| match component {
            Component::Normal(value) => Some(value.to_string_lossy().to_lowercase()),
            _ => None,
        })
        .collect::<Vec<_>>();
    let Some(first) = components.first().map(String::as_str) else {
        return true;
    };
    matches!(
        first,
        "windows"
            | "program files"
            | "program files (x86)"
            | "programdata"
            | "$recycle.bin"
            | "system volume information"
            | "recovery"
            | "boot"
            | "perflogs"
    ) || (matches!(first, "users" | "documents and settings") && components.len() == 1)
}

#[cfg(not(any(unix, windows)))]
fn is_protected_system_scope(path: &Path) -> bool {
    path.parent().is_none()
}

fn issue(code: &'static str, path: &Path, detail_code: Option<&'static str>) -> ScanIssue {
    ScanIssue {
        code,
        path_key: Some(comparison_key(path)),
        detail_code,
    }
}

fn io_error_code(error: &std::io::Error) -> Option<&'static str> {
    use std::io::ErrorKind;
    match error.kind() {
        ErrorKind::NotFound => Some("not_found"),
        ErrorKind::PermissionDenied => Some("permission_denied"),
        ErrorKind::InvalidData => Some("invalid_data"),
        ErrorKind::TimedOut => Some("timed_out"),
        _ => Some("io_error"),
    }
}

fn modified_unix_ns(metadata: &Metadata) -> Option<i64> {
    let duration = metadata.modified().ok()?.duration_since(UNIX_EPOCH).ok()?;
    i64::try_from(duration.as_nanos()).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn setup() -> (tempfile::TempDir, ManifestDatabase, i64) {
        let directory = tempfile::tempdir().expect("fixture root should exist");
        let database = ManifestDatabase::open_in_memory().expect("database should initialize");
        let scope = authorize_scope(&database, directory.path()).expect("scope should authorize");
        (directory, database, scope.id)
    }

    #[test]
    fn authorization_requires_an_existing_directory() {
        let directory = tempfile::tempdir().expect("fixture root should exist");
        let database = ManifestDatabase::open_in_memory().expect("database should initialize");
        let file = directory.path().join("file.txt");
        fs::write(&file, "metadata only fixture").expect("fixture should write");

        let error = authorize_scope(&database, &file).expect_err("file scope must fail");
        assert!(matches!(error, ScannerError::ScopeIsNotDirectory));
    }

    #[test]
    fn multiple_coverage_roots_and_grants_commit_together() {
        let directory = tempfile::tempdir().expect("fixture root should exist");
        let desktop = directory.path().join("Desktop");
        let documents = directory.path().join("Documents");
        fs::create_dir(&desktop).expect("desktop root should create");
        fs::create_dir(&documents).expect("documents root should create");
        let mut database = ManifestDatabase::open_in_memory().expect("database should initialize");
        let requests = [
            CoverageRootAuthorizationRequest {
                requested_path: &desktop,
                grant_platform: std::env::consts::OS,
                opaque_grant: b"desktop-grant",
            },
            CoverageRootAuthorizationRequest {
                requested_path: &documents,
                grant_platform: std::env::consts::OS,
                opaque_grant: b"documents-grant",
            },
        ];

        let scopes = authorize_coverage_roots_with_access_grants(&mut database, &requests)
            .expect("coverage set should authorize");

        assert_eq!(scopes.len(), 2);
        assert_eq!(
            database
                .list_active_scope_grants()
                .expect("active grants should load")
                .len(),
            2
        );
        assert_eq!(
            database
                .list_scopes()
                .expect("authorized roots should load"),
            scopes
        );
    }

    #[test]
    fn invalid_or_overlapping_coverage_set_commits_nothing() {
        let directory = tempfile::tempdir().expect("fixture root should exist");
        let parent = directory.path().join("Home");
        let child = parent.join("Desktop");
        fs::create_dir_all(&child).expect("nested roots should create");
        let mut database = ManifestDatabase::open_in_memory().expect("database should initialize");
        let overlapping = [
            CoverageRootAuthorizationRequest {
                requested_path: &parent,
                grant_platform: std::env::consts::OS,
                opaque_grant: b"home-grant",
            },
            CoverageRootAuthorizationRequest {
                requested_path: &child,
                grant_platform: std::env::consts::OS,
                opaque_grant: b"desktop-grant",
            },
        ];

        assert!(matches!(
            authorize_coverage_roots_with_access_grants(&mut database, &overlapping),
            Err(ScannerError::CoverageRootOverlap)
        ));
        assert!(
            database
                .list_scopes()
                .expect("failed set must leave no scopes")
                .is_empty()
        );

        let missing = directory.path().join("missing");
        let one_missing = [
            CoverageRootAuthorizationRequest {
                requested_path: &parent,
                grant_platform: std::env::consts::OS,
                opaque_grant: b"home-grant",
            },
            CoverageRootAuthorizationRequest {
                requested_path: &missing,
                grant_platform: std::env::consts::OS,
                opaque_grant: b"missing-grant",
            },
        ];
        assert!(matches!(
            authorize_coverage_roots_with_access_grants(&mut database, &one_missing),
            Err(ScannerError::CanonicalizationFailed)
        ));
        assert!(
            database
                .list_active_scope_grants()
                .expect("failed set must leave no grants")
                .is_empty()
        );
    }

    #[test]
    fn coverage_set_bounds_fail_before_any_write() {
        let directory = tempfile::tempdir().expect("fixture root should exist");
        let mut database = ManifestDatabase::open_in_memory().expect("database should initialize");
        assert!(matches!(
            authorize_coverage_roots_with_access_grants(&mut database, &[]),
            Err(ScannerError::CoverageSetEmpty)
        ));

        let requests = (0..=MAX_COVERAGE_ROOTS_PER_SELECTION)
            .map(|_| CoverageRootAuthorizationRequest {
                requested_path: directory.path(),
                grant_platform: std::env::consts::OS,
                opaque_grant: b"bounded-grant",
            })
            .collect::<Vec<_>>();
        assert!(matches!(
            authorize_coverage_roots_with_access_grants(&mut database, &requests),
            Err(ScannerError::CoverageSetTooLarge)
        ));
        assert!(
            database
                .list_scopes()
                .expect("bounded failures must leave no scopes")
                .is_empty()
        );
    }

    #[test]
    fn exact_root_reauthorization_preserves_scope_identity_and_replaces_grant() {
        let directory = tempfile::tempdir().expect("fixture root should exist");
        let mut database = ManifestDatabase::open_in_memory().expect("database should initialize");
        let first = authorize_coverage_roots_with_access_grants(
            &mut database,
            &[CoverageRootAuthorizationRequest {
                requested_path: directory.path(),
                grant_platform: std::env::consts::OS,
                opaque_grant: b"first-grant",
            }],
        )
        .expect("first authorization should persist")
        .remove(0);
        let second = authorize_coverage_roots_with_access_grants(
            &mut database,
            &[CoverageRootAuthorizationRequest {
                requested_path: directory.path(),
                grant_platform: std::env::consts::OS,
                opaque_grant: b"replacement-grant",
            }],
        )
        .expect("exact reauthorization should persist")
        .remove(0);

        assert_eq!(second.id, first.id);
        assert_eq!(second.created_at_unix_ms, first.created_at_unix_ms);
        assert_eq!(
            database
                .active_scope_grant(first.id)
                .expect("replacement grant should be active")
                .opaque_grant,
            b"replacement-grant"
        );
    }

    #[cfg(unix)]
    #[test]
    fn protected_system_descendants_are_denied_but_user_containers_require_a_child() {
        assert!(is_protected_system_scope(Path::new("/System/Library")));
        assert!(is_protected_system_scope(Path::new("/usr/local/bin")));
        assert!(is_protected_system_scope(Path::new("/private/var/db")));
        assert!(is_protected_system_scope(Path::new("/Users")));
        assert!(!is_protected_system_scope(Path::new(
            "/Users/person/Documents"
        )));
        assert!(!is_protected_system_scope(Path::new(
            "/private/var/folders/person/test"
        )));
    }

    #[cfg(unix)]
    #[test]
    fn authorization_rejects_an_existing_protected_system_descendant() {
        let database = ManifestDatabase::open_in_memory().expect("database should initialize");

        let error = authorize_scope(&database, Path::new("/usr"))
            .expect_err("protected system tree must not authorize");

        assert!(matches!(error, ScannerError::ProtectedSystemScope));
        assert!(
            database
                .list_scopes()
                .expect("scopes should load")
                .is_empty()
        );
    }

    #[test]
    fn rescans_are_idempotent_and_hidden_entries_are_excluded() {
        let (directory, mut database, scope_id) = setup();
        fs::create_dir(directory.path().join("project")).expect("folder should create");
        fs::write(directory.path().join("project/readme.md"), "hello")
            .expect("fixture should write");
        fs::write(directory.path().join(".secret"), "not indexed")
            .expect("hidden fixture should write");

        let first = scan_scope(&mut database, scope_id).expect("first scan should pass");
        let first_stats = database.stats().expect("stats should load");
        let second = scan_scope(&mut database, scope_id).expect("second scan should pass");
        let second_stats = database.stats().expect("stats should load");

        assert_eq!(first.discovered_files, 1);
        assert_eq!(first.discovered_folders, 2);
        assert_eq!(first.skipped_entries, 1);
        assert_eq!(second.discovered_files, first.discovered_files);
        assert_eq!(second_stats.node_count, first_stats.node_count);
        assert_eq!(
            second_stats.active_location_count,
            first_stats.active_location_count
        );
        assert_eq!(second_stats.completed_scan_count, 2);
    }

    #[test]
    fn temporary_downloads_are_excluded_until_renamed_to_a_final_name() {
        let (directory, mut database, scope_id) = setup();
        let partial = directory.path().join("report.pdf.part");
        let completed = directory.path().join("report.pdf");
        fs::write(&partial, "partial").expect("partial fixture should write");
        fs::write(directory.path().join("archive.crdownload"), "partial")
            .expect("browser fixture should write");
        fs::write(directory.path().join("capture.download"), "partial")
            .expect("download fixture should write");
        fs::write(directory.path().join("ready.md"), "ready").expect("ready fixture should write");

        let initial = scan_scope(&mut database, scope_id).expect("initial scan should pass");
        assert_eq!(initial.discovered_files, 1);
        assert_eq!(initial.skipped_entries, 3);
        assert_eq!(initial.issue_count, 3);

        fs::rename(&partial, &completed).expect("partial fixture should finalize");
        let rescanned = scan_scope(&mut database, scope_id).expect("rescan should pass");
        let completed_key = comparison_key(
            &fs::canonicalize(&completed).expect("completed fixture should canonicalize"),
        );

        assert_eq!(rescanned.discovered_files, 2);
        assert_eq!(rescanned.skipped_entries, 2);
        assert_eq!(rescanned.issue_count, 2);
        assert!(
            database
                .node_id_for_path_key(scope_id, &completed_key)
                .expect("completed fixture lookup should pass")
                .is_some()
        );
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn finder_hidden_flag_is_excluded_and_recorded() {
        let (directory, mut database, scope_id) = setup();
        let visible = directory.path().join("visible.txt");
        let hidden = directory.path().join("finder-hidden.txt");
        fs::write(&visible, "visible fixture").expect("fixture should write");
        fs::write(&hidden, "hidden fixture").expect("fixture should write");
        let status = std::process::Command::new("/usr/bin/chflags")
            .arg("hidden")
            .arg(&hidden)
            .status()
            .expect("chflags should execute");
        assert!(status.success());

        let report = scan_scope(&mut database, scope_id).expect("scan should pass");

        assert_eq!(report.discovered_files, 1);
        assert_eq!(report.skipped_entries, 1);
        assert_eq!(report.issue_count, 1);
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn canonical_scope_uses_filesystem_case_behavior() {
        let directory = tempfile::tempdir().expect("fixture root should exist");
        let actual = directory.path().join("MixedCaseScope");
        let alternate_case = directory.path().join("mixedcasescope");
        fs::create_dir(&actual).expect("scope should create");
        fs::write(actual.join("note.md"), "metadata fixture").expect("fixture should write");
        let Ok(canonical_alias) = fs::canonicalize(&alternate_case) else {
            assert!(fs::canonicalize(&actual).is_ok());
            return;
        };
        let canonical_actual = fs::canonicalize(&actual).expect("scope should canonicalize");
        assert_eq!(
            comparison_key(&canonical_alias),
            comparison_key(&canonical_actual)
        );
        let mut database = ManifestDatabase::open_in_memory().expect("database should initialize");

        let scope = authorize_scope(&database, &alternate_case).expect("alias should authorize");
        let report = scan_scope(&mut database, scope.id).expect("canonical scope should scan");

        assert_eq!(report.discovered_files, 1);
        assert_eq!(report.discovered_folders, 1);
    }

    #[test]
    fn bounded_batches_pause_without_publishing_and_resume_to_completion() {
        let (directory, mut database, scope_id) = setup();
        for index in 0..3 {
            fs::write(
                directory.path().join(format!("file-{index}.txt")),
                "metadata only fixture",
            )
            .expect("fixture should write");
        }

        let job = create_scan_job(&mut database, scope_id).expect("job should create");
        let partial =
            run_scan_job_batch(&mut database, job.job_id, 1).expect("first batch should run");

        assert_eq!(partial.status, ScanStatus::Running);
        assert_eq!(partial.processed_entries, 1);
        assert_eq!(partial.queued_entries, 4);
        assert_eq!(database.stats().expect("stats should load").node_count, 0);

        let paused = pause_scan_job(&mut database, job.job_id).expect("job should pause");
        assert_eq!(paused.status, ScanStatus::Paused);
        assert_eq!(database.stats().expect("stats should load").node_count, 0);

        let resumed = resume_scan_job(&mut database, job.job_id).expect("job should resume");
        assert_eq!(resumed.status, ScanStatus::Running);
        let completed = run_scan_job_to_terminal(&mut database, job.job_id)
            .expect("resumed job should complete");

        assert_eq!(completed.status, ScanStatus::Completed);
        assert_eq!(completed.discovered_files, 3);
        assert_eq!(completed.discovered_folders, 1);
        assert_eq!(database.stats().expect("stats should load").node_count, 4);
    }

    #[test]
    fn resume_revalidates_the_authorized_root_before_mutating_job_state() {
        let (directory, mut database, scope_id) = setup();
        let job = create_scan_job(&mut database, scope_id).expect("job should create");
        pause_scan_job(&mut database, job.job_id).expect("job should pause");
        let moved_root = directory.path().with_extension("moved");
        fs::rename(directory.path(), &moved_root).expect("fixture root should move");

        let error = resume_scan_job(&mut database, job.job_id)
            .expect_err("changed scope must prevent resume");

        assert!(matches!(error, ScannerError::CanonicalizationFailed));
        assert_eq!(
            database
                .scan_job(job.job_id)
                .expect("job should remain readable")
                .status,
            ScanStatus::Paused
        );
        fs::rename(&moved_root, directory.path()).expect("fixture root should restore");
    }

    #[cfg(unix)]
    #[test]
    fn unreadable_directories_record_permission_denial_without_failing_the_job() {
        use std::os::unix::fs::PermissionsExt;

        let directory = tempfile::tempdir().expect("fixture root should exist");
        let denied = directory.path().join("denied");
        fs::create_dir(&denied).expect("denied fixture should create");
        fs::write(denied.join("private.txt"), "not readable").expect("fixture should write");
        let canonical_root =
            fs::canonicalize(directory.path()).expect("fixture root should canonicalize");
        let canonical_denied = fs::canonicalize(&denied).expect("fixture should canonicalize");
        let original_permissions = fs::metadata(&denied)
            .expect("permissions should load")
            .permissions();
        fs::set_permissions(&denied, fs::Permissions::from_mode(0o000))
            .expect("permissions should restrict");
        let entry = QueueEntry {
            id: 1,
            path_raw: path_to_raw(&canonical_denied),
            path_key: comparison_key(&canonical_denied),
            parent_identity_key: None,
            is_root: false,
        };

        let processed = process_queue_entry(&canonical_root, &entry);
        fs::set_permissions(&denied, original_permissions).expect("permissions should restore");
        let processed = processed.expect("permission issue should be recoverable");

        assert!(processed.children.is_empty());
        assert_eq!(processed.issues.len(), 1);
        assert!(matches!(
            processed.issues[0].code,
            "canonicalization_failed" | "directory_read_denied"
        ));
        assert_eq!(processed.issues[0].detail_code, Some("permission_denied"));
    }

    #[cfg(unix)]
    #[test]
    fn rename_and_hard_links_preserve_stable_identity() {
        let (directory, mut database, scope_id) = setup();
        let original = directory.path().join("original.txt");
        let hard_link = directory.path().join("linked.txt");
        let renamed = directory.path().join("renamed.txt");
        fs::write(&original, "same inode").expect("fixture should write");
        fs::hard_link(&original, &hard_link).expect("hard link should create");
        let original_key =
            comparison_key(&fs::canonicalize(&original).expect("original should canonicalize"));
        let linked_key =
            comparison_key(&fs::canonicalize(&hard_link).expect("link should canonicalize"));

        scan_scope(&mut database, scope_id).expect("first scan should pass");
        let original_node = database
            .node_id_for_path_key(scope_id, &original_key)
            .expect("query should pass")
            .expect("original should exist");
        let linked_node = database
            .node_id_for_path_key(scope_id, &linked_key)
            .expect("query should pass")
            .expect("link should exist");
        assert_eq!(original_node, linked_node);

        fs::rename(&original, &renamed).expect("rename should pass");
        scan_scope(&mut database, scope_id).expect("rescan should pass");
        let renamed_key =
            comparison_key(&fs::canonicalize(&renamed).expect("renamed path should canonicalize"));
        let renamed_node = database
            .node_id_for_path_key(scope_id, &renamed_key)
            .expect("query should pass")
            .expect("renamed path should exist");
        assert_eq!(original_node, renamed_node);
        assert_eq!(database.stats().expect("stats should load").file_count, 1);
    }

    #[cfg(unix)]
    #[test]
    fn symlink_loops_are_recorded_and_never_followed() {
        use std::os::unix::fs::symlink;

        let (directory, mut database, scope_id) = setup();
        let child = directory.path().join("child");
        fs::create_dir(&child).expect("folder should create");
        symlink(directory.path(), child.join("loop")).expect("symlink should create");

        let report = scan_scope(&mut database, scope_id).expect("scan should terminate");
        assert_eq!(report.discovered_folders, 2);
        assert_eq!(report.skipped_entries, 1);
        assert_eq!(report.issue_count, 1);
    }

    #[test]
    fn unicode_paths_are_normalized_for_comparison() {
        let directory = tempfile::tempdir().expect("fixture root should exist");
        let composed = directory.path().join("caf\u{e9}.txt");
        let decomposed = directory.path().join("cafe\u{301}.txt");

        assert_eq!(comparison_key(&composed), comparison_key(&decomposed));
    }
}
