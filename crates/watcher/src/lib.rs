use std::ffi::OsStr;
use std::fmt;
use std::fs::{self, File, Metadata};
use std::path::{Component, Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use deskgraph_database::{
    DatabaseError, ManifestDatabase, QueuedPath, WatchObservationWrite, WatchSnapshot,
    WatchSnapshotKind,
};
use deskgraph_domain::{ScanStatus, WatchEventProgress, WatchEventReason, WatchEventStatus};
use deskgraph_identity::{
    IdentityNodeKind, comparison_key, has_hidden_or_system_attribute, is_symlink_or_reparse_point,
    path_from_raw, path_to_raw, platform_identity, platform_identity_for_open_file,
};
use deskgraph_scanner::{
    ScannerError, resume_scan_job, run_scan_job_to_terminal, validated_scope_root,
};

const DEFAULT_STABILITY_WINDOW_MS: i64 = 1_000;
const MIN_STABILITY_WINDOW_MS: i64 = 250;
const MAX_STABILITY_WINDOW_MS: i64 = 60_000;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct WatchPolicy {
    stability_window_ms: i64,
}

impl WatchPolicy {
    pub fn new(stability_window_ms: i64) -> Result<Self, WatcherError> {
        if !(MIN_STABILITY_WINDOW_MS..=MAX_STABILITY_WINDOW_MS).contains(&stability_window_ms) {
            return Err(WatcherError::InvalidPolicy);
        }
        Ok(Self {
            stability_window_ms,
        })
    }
}

impl Default for WatchPolicy {
    fn default() -> Self {
        Self {
            stability_window_ms: DEFAULT_STABILITY_WINDOW_MS,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WatchHint {
    pub scope_id: i64,
    pub path: PathBuf,
}

pub trait WatchEventSource {
    fn next_hint(&mut self) -> Result<Option<WatchHint>, WatcherError>;
}

#[derive(Debug)]
pub enum WatcherError {
    Database(DatabaseError),
    Scanner(ScannerError),
    InvalidPolicy,
    InvalidTimestamp,
    ObservedPathMustBeAbsolute,
    ObservedPathOutsideScope,
    ObservedPathDecodeFailed,
    SymlinkOrReparsePointDenied,
    SourceUnavailable,
    SourceIdentityChanged,
    EventSourceFailed,
}

impl WatcherError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::Database(error) => error.code(),
            Self::Scanner(error) => error.code(),
            Self::InvalidPolicy => "watch_policy_invalid",
            Self::InvalidTimestamp => "watch_timestamp_invalid",
            Self::ObservedPathMustBeAbsolute => "watch_path_must_be_absolute",
            Self::ObservedPathOutsideScope => "watch_path_outside_scope",
            Self::ObservedPathDecodeFailed => "watch_path_decode_failed",
            Self::SymlinkOrReparsePointDenied => "watch_symlink_or_reparse_denied",
            Self::SourceUnavailable => "watch_source_unavailable",
            Self::SourceIdentityChanged => "watch_source_identity_changed",
            Self::EventSourceFailed => "watch_event_source_failed",
        }
    }
}

impl fmt::Display for WatcherError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.code())
    }
}

impl std::error::Error for WatcherError {}

impl From<DatabaseError> for WatcherError {
    fn from(error: DatabaseError) -> Self {
        Self::Database(error)
    }
}

impl From<ScannerError> for WatcherError {
    fn from(error: ScannerError) -> Self {
        Self::Scanner(error)
    }
}

#[derive(Debug)]
struct ValidatedHint {
    path: PathBuf,
    path_raw: Vec<u8>,
    path_key: String,
    snapshot: WatchSnapshot,
}

enum EvaluatedHint {
    Track(ValidatedHint),
    Ignore(ValidatedHint, WatchEventReason),
}

pub fn observe_watch_path_at(
    database_path: &Path,
    scope_id: i64,
    observed_path: &Path,
    policy: WatchPolicy,
) -> Result<WatchEventProgress, WatcherError> {
    let mut database = ManifestDatabase::open(database_path)?;
    observe_watch_path(&mut database, scope_id, observed_path, policy)
}

pub fn observe_watch_path(
    database: &mut ManifestDatabase,
    scope_id: i64,
    observed_path: &Path,
    policy: WatchPolicy,
) -> Result<WatchEventProgress, WatcherError> {
    observe_watch_path_at_time(database, scope_id, observed_path, policy, unix_ms()?)
}

pub fn ingest_next_source_hint_at_time(
    database: &mut ManifestDatabase,
    source: &mut impl WatchEventSource,
    policy: WatchPolicy,
    now_unix_ms: i64,
) -> Result<Option<WatchEventProgress>, WatcherError> {
    let Some(hint) = source.next_hint()? else {
        return Ok(None);
    };
    observe_watch_path_at_time(database, hint.scope_id, &hint.path, policy, now_unix_ms).map(Some)
}

pub fn observe_watch_path_at_time(
    database: &mut ManifestDatabase,
    scope_id: i64,
    observed_path: &Path,
    policy: WatchPolicy,
    now_unix_ms: i64,
) -> Result<WatchEventProgress, WatcherError> {
    let stable_after = stable_after(now_unix_ms, policy)?;
    match evaluate_hint(database, scope_id, observed_path)? {
        EvaluatedHint::Track(hint) => database
            .record_watch_observation_at(WatchObservationWrite {
                scope_id,
                path_raw: &hint.path_raw,
                path_key: &hint.path_key,
                snapshot: &hint.snapshot,
                stable_after_unix_ms: stable_after,
                ignored_reason: None,
                observed_at_unix_ms: now_unix_ms,
            })
            .map(|event| event.progress)
            .map_err(Into::into),
        EvaluatedHint::Ignore(hint, reason) => database
            .record_watch_observation_at(WatchObservationWrite {
                scope_id,
                path_raw: &hint.path_raw,
                path_key: &hint.path_key,
                snapshot: &hint.snapshot,
                stable_after_unix_ms: now_unix_ms,
                ignored_reason: Some(reason),
                observed_at_unix_ms: now_unix_ms,
            })
            .map(|event| event.progress)
            .map_err(Into::into),
    }
}

pub fn advance_watch_event_at(
    database_path: &Path,
    event_id: i64,
    policy: WatchPolicy,
) -> Result<WatchEventProgress, WatcherError> {
    let mut database = ManifestDatabase::open(database_path)?;
    advance_watch_event(&mut database, event_id, policy)
}

pub fn advance_watch_event(
    database: &mut ManifestDatabase,
    event_id: i64,
    policy: WatchPolicy,
) -> Result<WatchEventProgress, WatcherError> {
    advance_watch_event_at_time(database, event_id, policy, unix_ms()?)
}

pub fn advance_watch_event_at_time(
    database: &mut ManifestDatabase,
    event_id: i64,
    policy: WatchPolicy,
    now_unix_ms: i64,
) -> Result<WatchEventProgress, WatcherError> {
    if now_unix_ms < 0 {
        return Err(WatcherError::InvalidTimestamp);
    }
    let event = database.watch_event(event_id)?;
    if event.progress.is_terminal() {
        return Ok(event.progress);
    }
    if event.progress.status == WatchEventStatus::Reconciling {
        return finish_reconciliation(database, event_id, now_unix_ms);
    }
    if event.progress.status != WatchEventStatus::Stabilizing {
        return Err(WatcherError::Database(
            DatabaseError::InvalidWatchEventState,
        ));
    }
    if now_unix_ms < event.progress.stable_after_unix_ms {
        return Ok(event.progress);
    }

    let observed_path =
        path_from_raw(&event.path_raw).map_err(|_| WatcherError::ObservedPathDecodeFailed)?;
    let evaluated = match evaluate_hint(database, event.progress.scope_id, &observed_path) {
        Ok(evaluated) => evaluated,
        Err(WatcherError::SourceUnavailable) => {
            return database
                .fail_watch_event_at(event_id, WatchEventReason::SourceUnavailable, now_unix_ms)
                .map_err(Into::into);
        }
        Err(error) => return Err(error),
    };
    let hint = match evaluated {
        EvaluatedHint::Ignore(_, reason) => {
            return database
                .mark_watch_event_ignored_at(event_id, reason, now_unix_ms)
                .map_err(Into::into);
        }
        EvaluatedHint::Track(hint) => hint,
    };
    if hint.path_key != event.path_key || hint.snapshot != event.snapshot {
        return record_changed_snapshot(
            database,
            event.progress.scope_id,
            &hint,
            policy,
            now_unix_ms,
        );
    }
    if hint.snapshot.kind == WatchSnapshotKind::File && !open_file_matches_snapshot(&hint)? {
        let refreshed = match evaluate_hint(database, event.progress.scope_id, &hint.path)? {
            EvaluatedHint::Track(hint) => hint,
            EvaluatedHint::Ignore(_, reason) => {
                return database
                    .mark_watch_event_ignored_at(event_id, reason, now_unix_ms)
                    .map_err(Into::into);
            }
        };
        return record_changed_snapshot(
            database,
            event.progress.scope_id,
            &refreshed,
            policy,
            now_unix_ms,
        );
    }

    let canonical_root = validated_scope_root(database, event.progress.scope_id)?;
    let root = QueuedPath {
        path_raw: path_to_raw(&canonical_root),
        path_key: comparison_key(&canonical_root),
        parent_identity_key: None,
        is_root: true,
    };
    database.begin_watch_reconciliation_at(event_id, &root, now_unix_ms)?;
    finish_reconciliation(database, event_id, now_unix_ms)
}

pub fn watch_event_at(
    database_path: &Path,
    event_id: i64,
) -> Result<WatchEventProgress, WatcherError> {
    ManifestDatabase::open(database_path)?
        .watch_event(event_id)
        .map(|event| event.progress)
        .map_err(Into::into)
}

pub fn recent_watch_events_at(
    database_path: &Path,
) -> Result<Vec<WatchEventProgress>, WatcherError> {
    ManifestDatabase::open(database_path)?
        .recent_watch_events()
        .map_err(Into::into)
}

fn finish_reconciliation(
    database: &mut ManifestDatabase,
    event_id: i64,
    now_unix_ms: i64,
) -> Result<WatchEventProgress, WatcherError> {
    let event = database.watch_event(event_id)?;
    let scan_job_id = event
        .progress
        .scan_job_id
        .ok_or(DatabaseError::InvalidWatchEventState)?;
    let scan = database.scan_job(scan_job_id)?;
    let scan = match scan.status {
        ScanStatus::Interrupted => {
            resume_scan_job(database, scan_job_id)?;
            run_scan_job_to_terminal(database, scan_job_id)
        }
        ScanStatus::Running => run_scan_job_to_terminal(database, scan_job_id),
        ScanStatus::Paused => return Ok(event.progress),
        ScanStatus::Completed => Ok(scan),
        ScanStatus::Failed => {
            return database
                .fail_watch_event_at(event_id, WatchEventReason::ReconcileFailed, now_unix_ms)
                .map_err(Into::into);
        }
    };
    match scan {
        Ok(scan) if scan.status == ScanStatus::Completed => database
            .complete_watch_reconciliation_at(event_id, now_unix_ms)
            .map_err(Into::into),
        Ok(_) => Ok(database.watch_event(event_id)?.progress),
        Err(error) => {
            database.fail_watch_event_at(
                event_id,
                WatchEventReason::ReconcileFailed,
                now_unix_ms,
            )?;
            Err(error.into())
        }
    }
}

fn record_changed_snapshot(
    database: &mut ManifestDatabase,
    scope_id: i64,
    hint: &ValidatedHint,
    policy: WatchPolicy,
    now_unix_ms: i64,
) -> Result<WatchEventProgress, WatcherError> {
    database
        .record_watch_observation_at(WatchObservationWrite {
            scope_id,
            path_raw: &hint.path_raw,
            path_key: &hint.path_key,
            snapshot: &hint.snapshot,
            stable_after_unix_ms: stable_after(now_unix_ms, policy)?,
            ignored_reason: None,
            observed_at_unix_ms: now_unix_ms,
        })
        .map(|event| event.progress)
        .map_err(Into::into)
}

fn evaluate_hint(
    database: &ManifestDatabase,
    scope_id: i64,
    observed_path: &Path,
) -> Result<EvaluatedHint, WatcherError> {
    if !observed_path.is_absolute() {
        return Err(WatcherError::ObservedPathMustBeAbsolute);
    }
    let canonical_root = validated_scope_root(database, scope_id)?;
    let metadata = match fs::symlink_metadata(observed_path) {
        Ok(metadata) => Some(metadata),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
        Err(_) => return Err(WatcherError::SourceUnavailable),
    };
    let (path, snapshot, platform_hidden) = if let Some(metadata) = metadata {
        if is_symlink_or_reparse_point(&metadata) {
            return Err(WatcherError::SymlinkOrReparsePointDenied);
        }
        let canonical =
            fs::canonicalize(observed_path).map_err(|_| WatcherError::SourceUnavailable)?;
        if !canonical.starts_with(&canonical_root) {
            return Err(WatcherError::ObservedPathOutsideScope);
        }
        let (kind, identity_kind, size_bytes) = if metadata.is_file() {
            (
                WatchSnapshotKind::File,
                IdentityNodeKind::File,
                Some(metadata.len()),
            )
        } else if metadata.is_dir() {
            (WatchSnapshotKind::Folder, IdentityNodeKind::Folder, None)
        } else {
            let hint = missing_hint(canonical);
            return Ok(EvaluatedHint::Ignore(
                hint,
                WatchEventReason::UnsupportedEntry,
            ));
        };
        let identity = platform_identity(&canonical, &metadata, identity_kind)
            .map_err(|_| WatcherError::SourceIdentityChanged)?;
        (
            canonical,
            WatchSnapshot {
                kind,
                size_bytes,
                modified_unix_ns: modified_unix_ns(&metadata),
                identity_key: Some(identity.key),
            },
            has_hidden_or_system_attribute(&metadata),
        )
    } else {
        (
            resolve_missing_path(&canonical_root, observed_path)?,
            WatchSnapshot {
                kind: WatchSnapshotKind::Missing,
                size_bytes: None,
                modified_unix_ns: None,
                identity_key: None,
            },
            false,
        )
    };
    if !path.starts_with(&canonical_root) {
        return Err(WatcherError::ObservedPathOutsideScope);
    }
    let hint = ValidatedHint {
        path_raw: path_to_raw(&path),
        path_key: comparison_key(&path),
        path,
        snapshot,
    };
    if is_temporary_download(&hint.path) {
        return Ok(EvaluatedHint::Ignore(
            hint,
            WatchEventReason::TemporaryDownload,
        ));
    }
    if platform_hidden || has_hidden_component(&canonical_root, &hint.path) {
        return Ok(EvaluatedHint::Ignore(hint, WatchEventReason::HiddenEntry));
    }
    Ok(EvaluatedHint::Track(hint))
}

fn missing_hint(path: PathBuf) -> ValidatedHint {
    ValidatedHint {
        path_raw: path_to_raw(&path),
        path_key: comparison_key(&path),
        path,
        snapshot: WatchSnapshot {
            kind: WatchSnapshotKind::Missing,
            size_bytes: None,
            modified_unix_ns: None,
            identity_key: None,
        },
    }
}

fn resolve_missing_path(root: &Path, observed_path: &Path) -> Result<PathBuf, WatcherError> {
    let mut ancestor = observed_path.to_path_buf();
    let mut suffix = Vec::new();
    loop {
        match fs::symlink_metadata(&ancestor) {
            Ok(metadata) => {
                if is_symlink_or_reparse_point(&metadata) {
                    return Err(WatcherError::SymlinkOrReparsePointDenied);
                }
                let canonical =
                    fs::canonicalize(&ancestor).map_err(|_| WatcherError::SourceUnavailable)?;
                if !canonical.starts_with(root) {
                    return Err(WatcherError::ObservedPathOutsideScope);
                }
                let mut resolved = canonical;
                for component in suffix.iter().rev() {
                    resolved.push(component);
                }
                return Ok(resolved);
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                let name = ancestor
                    .file_name()
                    .ok_or(WatcherError::ObservedPathOutsideScope)?;
                if !matches!(
                    Path::new(name).components().next(),
                    Some(Component::Normal(_))
                ) {
                    return Err(WatcherError::ObservedPathOutsideScope);
                }
                suffix.push(name.to_os_string());
                ancestor = ancestor
                    .parent()
                    .ok_or(WatcherError::ObservedPathOutsideScope)?
                    .to_path_buf();
            }
            Err(_) => return Err(WatcherError::SourceUnavailable),
        }
    }
}

fn open_file_matches_snapshot(hint: &ValidatedHint) -> Result<bool, WatcherError> {
    let file = match File::open(&hint.path) {
        Ok(file) => file,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(false),
        Err(_) => return Err(WatcherError::SourceUnavailable),
    };
    let metadata = file
        .metadata()
        .map_err(|_| WatcherError::SourceUnavailable)?;
    let identity =
        platform_identity_for_open_file(&file, &hint.path, &metadata, IdentityNodeKind::File)
            .map_err(|_| WatcherError::SourceIdentityChanged)?;
    Ok(
        hint.snapshot.identity_key.as_deref() == Some(identity.key.as_slice())
            && hint.snapshot.size_bytes == Some(metadata.len())
            && hint.snapshot.modified_unix_ns == modified_unix_ns(&metadata),
    )
}

fn stable_after(now_unix_ms: i64, policy: WatchPolicy) -> Result<i64, WatcherError> {
    if now_unix_ms < 0 {
        return Err(WatcherError::InvalidTimestamp);
    }
    now_unix_ms
        .checked_add(policy.stability_window_ms)
        .ok_or(WatcherError::InvalidTimestamp)
}

fn is_temporary_download(path: &Path) -> bool {
    let filename = path
        .file_name()
        .unwrap_or_else(|| OsStr::new(""))
        .to_string_lossy()
        .to_ascii_lowercase();
    [".part", ".crdownload", ".download"]
        .iter()
        .any(|suffix| filename.ends_with(suffix))
}

fn has_hidden_component(root: &Path, path: &Path) -> bool {
    path.strip_prefix(root).is_ok_and(|relative| {
        relative.components().any(|component| {
            matches!(component, Component::Normal(name) if name.to_string_lossy().starts_with('.'))
        })
    })
}

fn modified_unix_ns(metadata: &Metadata) -> Option<i64> {
    let duration = metadata.modified().ok()?.duration_since(UNIX_EPOCH).ok()?;
    i64::try_from(duration.as_nanos()).ok()
}

fn unix_ms() -> Result<i64, WatcherError> {
    let duration = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| WatcherError::InvalidTimestamp)?;
    i64::try_from(duration.as_millis()).map_err(|_| WatcherError::InvalidTimestamp)
}

#[cfg(test)]
mod tests {
    use super::*;
    use deskgraph_scanner::{authorize_scope, scan_scope};

    fn setup() -> (tempfile::TempDir, ManifestDatabase, i64) {
        let directory = tempfile::tempdir().expect("fixture root should exist");
        let mut database = ManifestDatabase::open_in_memory().expect("database should initialize");
        let scope = authorize_scope(&database, directory.path()).expect("scope should authorize");
        scan_scope(&mut database, scope.id).expect("initial scan should complete");
        (directory, database, scope.id)
    }

    #[test]
    fn temporary_download_is_ignored_without_a_scan() {
        let (directory, mut database, scope_id) = setup();
        let download = directory.path().join("archive.crdownload");
        fs::write(&download, "partial").expect("temporary file should write");

        let event = observe_watch_path_at_time(
            &mut database,
            scope_id,
            &download,
            WatchPolicy::default(),
            1_000,
        )
        .expect("temporary observation should be recorded safely");

        assert_eq!(event.status, WatchEventStatus::Ignored);
        assert_eq!(event.reason, Some(WatchEventReason::TemporaryDownload));
        assert_eq!(
            database
                .stats()
                .expect("stats should load")
                .completed_scan_count,
            1
        );
    }

    #[test]
    fn changing_snapshot_restarts_the_stability_window() {
        let (directory, mut database, scope_id) = setup();
        let file = directory.path().join("notes.md");
        fs::write(&file, "one").expect("file should write");
        let event = observe_watch_path_at_time(
            &mut database,
            scope_id,
            &file,
            WatchPolicy::default(),
            1_000,
        )
        .expect("observation should persist");
        fs::write(&file, "a longer second version").expect("file should change");

        let changed = advance_watch_event_at_time(
            &mut database,
            event.event_id,
            WatchPolicy::default(),
            2_000,
        )
        .expect("changed snapshot should remain stabilizing");
        assert_eq!(changed.status, WatchEventStatus::Stabilizing);
        assert_eq!(changed.observation_count, 2);
        assert_eq!(changed.stable_after_unix_ms, 3_000);

        let completed = advance_watch_event_at_time(
            &mut database,
            event.event_id,
            WatchPolicy::default(),
            3_000,
        )
        .expect("stable snapshot should reconcile");
        assert_eq!(completed.status, WatchEventStatus::Completed);
        assert_eq!(
            database
                .stats()
                .expect("stats should load")
                .completed_scan_count,
            2
        );
    }

    #[test]
    fn rename_storm_coalesces_and_preserves_identity_after_restart() {
        let directory = tempfile::tempdir().expect("fixture root should exist");
        let database_path = directory.path().join("manifest.sqlite3");
        let scope_path = directory.path().join("scope");
        fs::create_dir(&scope_path).expect("scope should create");
        let old_path = scope_path.join("old-name.md");
        let new_path = scope_path.join("new-name.md");
        fs::write(&old_path, "local context").expect("fixture should write");
        let mut database = ManifestDatabase::open(&database_path).expect("database should open");
        let scope = authorize_scope(&database, &scope_path).expect("scope should authorize");
        scan_scope(&mut database, scope.id).expect("initial scan should complete");
        let old_key = comparison_key(&fs::canonicalize(&old_path).expect("path should exist"));
        let original_node = database
            .node_id_for_path_key(scope.id, &old_key)
            .expect("node lookup should pass")
            .expect("node should exist");

        fs::rename(&old_path, &new_path).expect("fixture rename should pass");
        let event = observe_watch_path_at_time(
            &mut database,
            scope.id,
            &old_path,
            WatchPolicy::default(),
            1_000,
        )
        .expect("missing old path should be observed");
        let coalesced = observe_watch_path_at_time(
            &mut database,
            scope.id,
            &new_path,
            WatchPolicy::default(),
            1_100,
        )
        .expect("new path should coalesce");
        assert_eq!(coalesced.event_id, event.event_id);
        assert_eq!(coalesced.observation_count, 2);
        drop(database);

        let mut reopened = ManifestDatabase::open(&database_path).expect("database should reopen");
        let completed = advance_watch_event_at_time(
            &mut reopened,
            event.event_id,
            WatchPolicy::default(),
            2_100,
        )
        .expect("persisted event should resume");
        assert_eq!(completed.status, WatchEventStatus::Completed);
        let new_key = comparison_key(&fs::canonicalize(&new_path).expect("new path should exist"));
        assert_eq!(
            reopened
                .node_id_for_path_key(scope.id, &new_key)
                .expect("new node lookup should pass"),
            Some(original_node)
        );
        assert_eq!(
            reopened
                .node_id_for_path_key(scope.id, &old_key)
                .expect("old node lookup should pass"),
            None
        );
    }

    #[test]
    fn reconciling_event_resumes_its_atomically_linked_scan_after_restart() {
        let directory = tempfile::tempdir().expect("fixture root should exist");
        let database_path = directory.path().join("manifest.sqlite3");
        let scope_path = directory.path().join("scope");
        fs::create_dir(&scope_path).expect("scope should create");
        let watched_file = scope_path.join("restart.md");
        fs::write(&watched_file, "before restart").expect("fixture should write");
        let mut database = ManifestDatabase::open(&database_path).expect("database should open");
        let scope = authorize_scope(&database, &scope_path).expect("scope should authorize");
        scan_scope(&mut database, scope.id).expect("initial scan should complete");
        fs::write(&watched_file, "after restart").expect("fixture should change");
        let event = observe_watch_path_at_time(
            &mut database,
            scope.id,
            &watched_file,
            WatchPolicy::default(),
            1_000,
        )
        .expect("event should persist");
        let canonical_root =
            validated_scope_root(&database, scope.id).expect("root should validate");
        let root = QueuedPath {
            path_raw: path_to_raw(&canonical_root),
            path_key: comparison_key(&canonical_root),
            parent_identity_key: None,
            is_root: true,
        };
        let reconciling = database
            .begin_watch_reconciliation_at(event.event_id, &root, 2_000)
            .expect("event and scan should link atomically");
        assert_eq!(reconciling.status, WatchEventStatus::Reconciling);
        drop(database);

        let mut reopened = ManifestDatabase::open(&database_path).expect("database should reopen");
        let completed = advance_watch_event_at_time(
            &mut reopened,
            event.event_id,
            WatchPolicy::default(),
            2_100,
        )
        .expect("linked ready scan should resume");
        assert_eq!(completed.status, WatchEventStatus::Completed);
        assert!(completed.scan_job_id.is_some());
    }

    #[cfg(unix)]
    #[test]
    fn scope_escape_and_symlink_hints_are_denied() {
        use std::os::unix::fs::symlink;

        let (directory, mut database, scope_id) = setup();
        let outside = tempfile::tempdir().expect("outside root should exist");
        let outside_file = outside.path().join("outside.md");
        fs::write(&outside_file, "outside").expect("outside fixture should write");
        let link = directory.path().join("escape-link");
        symlink(&outside_file, &link).expect("symlink should create");

        assert!(matches!(
            observe_watch_path_at_time(
                &mut database,
                scope_id,
                &outside_file,
                WatchPolicy::default(),
                1_000
            ),
            Err(WatcherError::ObservedPathOutsideScope)
        ));
        assert!(matches!(
            observe_watch_path_at_time(
                &mut database,
                scope_id,
                &link,
                WatchPolicy::default(),
                1_000
            ),
            Err(WatcherError::SymlinkOrReparsePointDenied)
        ));
        let missing_escape = directory
            .path()
            .join("missing")
            .join("..")
            .join("..")
            .join("not-there.md");
        assert!(matches!(
            observe_watch_path_at_time(
                &mut database,
                scope_id,
                &missing_escape,
                WatchPolicy::default(),
                1_000
            ),
            Err(WatcherError::ObservedPathOutsideScope)
        ));
    }
}
