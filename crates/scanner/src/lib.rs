use std::ffi::OsStr;
use std::fmt;
use std::fs::{self, Metadata};
use std::path::Path;
use std::time::{Instant, UNIX_EPOCH};

use deskgraph_database::{DatabaseError, ManifestDatabase, NodeKind, Observation, ScanIssue};
use deskgraph_domain::{AuthorizedScope, ScanReport};
pub use deskgraph_identity::comparison_key;
use deskgraph_identity::{
    IdentityNodeKind, fallback_identity, is_symlink_or_reparse_point, path_from_raw, path_to_raw,
    platform_identity,
};

#[derive(Debug)]
pub enum ScannerError {
    Database(DatabaseError),
    CanonicalizationFailed,
    ScopeIsNotDirectory,
    ProtectedSystemScope,
    ScopeChanged,
    ScopePathDecodeFailed,
    ScanFailed,
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
            Self::ScanFailed => "metadata_scan_failed",
        }
    }
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

pub fn scan_scope(
    database: &mut ManifestDatabase,
    scope_id: i64,
) -> Result<ScanReport, ScannerError> {
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

    let job_id = database.create_scan_job(scope_id)?;
    let started = Instant::now();
    let discovery = discover(&canonical_root);
    let (observations, issues, skipped_entries) = match discovery {
        Ok(result) => result,
        Err(error) => {
            database.fail_scan(job_id, 1)?;
            return Err(error);
        }
    };
    let elapsed_ms = u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX);

    database
        .complete_scan(
            job_id,
            scope_id,
            &observations,
            &issues,
            skipped_entries,
            elapsed_ms,
        )
        .map_err(Into::into)
}

fn discover(root: &Path) -> Result<(Vec<Observation>, Vec<ScanIssue>, u64), ScannerError> {
    let mut observations = Vec::new();
    let mut issues = Vec::new();
    let mut skipped_entries = 0_u64;
    let mut pending = vec![(root.to_path_buf(), None, true)];

    while let Some((path, parent_identity_key, is_root)) = pending.pop() {
        let metadata = match fs::symlink_metadata(&path) {
            Ok(metadata) => metadata,
            Err(error) => {
                if is_root {
                    return Err(ScannerError::ScanFailed);
                }
                issues.push(issue("metadata_unavailable", &path, io_error_code(&error)));
                skipped_entries = skipped_entries.saturating_add(1);
                continue;
            }
        };

        if !is_root && is_hidden(path.file_name()) {
            issues.push(issue("hidden_entry_excluded", &path, None));
            skipped_entries = skipped_entries.saturating_add(1);
            continue;
        }
        if is_symlink_or_reparse_point(&metadata) {
            issues.push(issue("symlink_not_followed", &path, None));
            skipped_entries = skipped_entries.saturating_add(1);
            continue;
        }

        let canonical = match fs::canonicalize(&path) {
            Ok(canonical) => canonical,
            Err(error) => {
                issues.push(issue(
                    "canonicalization_failed",
                    &path,
                    io_error_code(&error),
                ));
                skipped_entries = skipped_entries.saturating_add(1);
                continue;
            }
        };
        if !canonical.starts_with(root) {
            issues.push(issue("scope_escape_denied", &path, None));
            skipped_entries = skipped_entries.saturating_add(1);
            continue;
        }

        let kind = if metadata.is_dir() {
            NodeKind::Folder
        } else if metadata.is_file() {
            NodeKind::File
        } else {
            issues.push(issue("unsupported_entry_type", &path, None));
            skipped_entries = skipped_entries.saturating_add(1);
            continue;
        };
        let path_key = comparison_key(&canonical);
        let identity_kind = match kind {
            NodeKind::File => IdentityNodeKind::File,
            NodeKind::Folder => IdentityNodeKind::Folder,
        };
        let identity = platform_identity(&canonical, &metadata, identity_kind)
            .unwrap_or_else(|_| fallback_identity(&path_key, identity_kind));
        let identity_key = identity.key;

        observations.push(Observation {
            kind,
            identity_kind: identity.kind.to_string(),
            identity_key: identity_key.clone(),
            parent_identity_key,
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
        });

        if kind == NodeKind::Folder {
            let read_directory = match fs::read_dir(&canonical) {
                Ok(entries) => entries,
                Err(error) => {
                    issues.push(issue(
                        "directory_read_denied",
                        &canonical,
                        io_error_code(&error),
                    ));
                    continue;
                }
            };
            let mut children = Vec::new();
            for entry in read_directory {
                match entry {
                    Ok(entry) => children.push(entry.path()),
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
            children.sort_by_key(|child| comparison_key(child));
            for child in children.into_iter().rev() {
                pending.push((child, Some(identity_key.clone()), false));
            }
        }
    }

    Ok((observations, issues, skipped_entries))
}

fn is_hidden(name: Option<&OsStr>) -> bool {
    name.is_some_and(|name| name.to_string_lossy().starts_with('.'))
}

#[cfg(unix)]
fn is_protected_system_scope(path: &Path) -> bool {
    const ROOTS: &[&str] = &[
        "/", "/System", "/Library", "/private", "/usr", "/bin", "/sbin", "/etc", "/var", "/dev",
        "/proc", "/sys", "/run", "/boot",
    ];
    ROOTS.iter().any(|root| path == Path::new(root))
}

#[cfg(windows)]
fn is_protected_system_scope(path: &Path) -> bool {
    if path.parent().is_none() {
        return true;
    }
    let key = comparison_key(path);
    ["\\windows", "\\program files", "\\programdata"]
        .iter()
        .any(|suffix| key.ends_with(suffix))
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
