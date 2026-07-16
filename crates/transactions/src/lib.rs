use std::ffi::OsStr;
use std::fmt;
use std::fs::{self, File, Metadata};
use std::io::ErrorKind;
use std::path::Path;
use std::time::UNIX_EPOCH;

use deskgraph_database::{ActionPlanWrite, ActionSourceRecord, DatabaseError, ManifestDatabase};
use deskgraph_domain::{ActionExecutionStrategy, ActionPlanPreview, ActionPlanSummary};
use deskgraph_identity::{
    IdentityNodeKind, comparison_key, is_symlink_or_reparse_point, path_to_raw, platform_identity,
    platform_identity_for_open_file,
};
use deskgraph_scanner::{ScannerError, validated_scope_root};

const MAX_PORTABLE_NAME_BYTES: usize = 255;

#[derive(Debug)]
pub enum TransactionError {
    Database(DatabaseError),
    Scanner(ScannerError),
    SourcePathMustBeAbsolute,
    SourceUnavailable,
    SourceSymlinkOrReparseDenied,
    SourceOutsideScope,
    SourceMustBeFile,
    SourceIdentityUnavailable,
    SourceIdentityChanged,
    SourceMetadataChanged,
    SourceOpenFailed,
    TargetNameInvalid,
    RenameNoOp,
    DestinationOutsideScope,
    DestinationConflict,
    DestinationUnavailable,
}

impl TransactionError {
    #[must_use]
    pub fn code(&self) -> &'static str {
        match self {
            Self::Database(error) => error.code(),
            Self::Scanner(error) => error.code(),
            Self::SourcePathMustBeAbsolute => "action_source_path_must_be_absolute",
            Self::SourceUnavailable => "action_source_unavailable",
            Self::SourceSymlinkOrReparseDenied => "action_source_symlink_or_reparse_denied",
            Self::SourceOutsideScope => "action_source_outside_scope",
            Self::SourceMustBeFile => "action_source_must_be_file",
            Self::SourceIdentityUnavailable => "action_source_identity_unavailable",
            Self::SourceIdentityChanged => "action_source_identity_changed",
            Self::SourceMetadataChanged => "action_source_metadata_changed",
            Self::SourceOpenFailed => "action_source_open_failed",
            Self::TargetNameInvalid => "action_target_name_invalid",
            Self::RenameNoOp => "action_rename_no_op",
            Self::DestinationOutsideScope => "action_destination_outside_scope",
            Self::DestinationConflict => "action_destination_conflict",
            Self::DestinationUnavailable => "action_destination_unavailable",
        }
    }
}

impl fmt::Display for TransactionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.code())
    }
}

impl std::error::Error for TransactionError {}

impl From<DatabaseError> for TransactionError {
    fn from(error: DatabaseError) -> Self {
        Self::Database(error)
    }
}

impl From<ScannerError> for TransactionError {
    fn from(error: ScannerError) -> Self {
        Self::Scanner(error)
    }
}

pub fn create_rename_preview_at(
    database_path: &Path,
    scope_id: i64,
    source_path: &Path,
    new_name: &str,
) -> Result<ActionPlanPreview, TransactionError> {
    let mut database = ManifestDatabase::open(database_path)?;
    create_rename_preview(&mut database, scope_id, source_path, new_name)
}

pub fn create_rename_preview(
    database: &mut ManifestDatabase,
    scope_id: i64,
    source_path: &Path,
    new_name: &str,
) -> Result<ActionPlanPreview, TransactionError> {
    validate_portable_name(new_name)?;
    if !source_path.is_absolute() {
        return Err(TransactionError::SourcePathMustBeAbsolute);
    }
    let canonical_root = validated_scope_root(database, scope_id)?;
    let source_link_metadata = fs::symlink_metadata(source_path).map_err(map_source_error)?;
    if is_symlink_or_reparse_point(&source_link_metadata) {
        return Err(TransactionError::SourceSymlinkOrReparseDenied);
    }
    if !source_link_metadata.is_file() {
        return Err(TransactionError::SourceMustBeFile);
    }
    let canonical_source = fs::canonicalize(source_path).map_err(map_source_error)?;
    if canonical_source == canonical_root || !canonical_source.starts_with(&canonical_root) {
        return Err(TransactionError::SourceOutsideScope);
    }
    if canonical_source.file_name() == Some(OsStr::new(new_name)) {
        return Err(TransactionError::RenameNoOp);
    }
    let source = database
        .action_source_for_path_key(scope_id, &comparison_key(&canonical_source))
        .map_err(|error| match error {
            DatabaseError::ActionSourceNotFound => TransactionError::SourceUnavailable,
            other => TransactionError::Database(other),
        })?;
    validate_source_snapshot(&canonical_source, &source, &source_link_metadata)?;

    let parent = canonical_source
        .parent()
        .ok_or(TransactionError::DestinationOutsideScope)?;
    let canonical_parent =
        fs::canonicalize(parent).map_err(|_| TransactionError::DestinationUnavailable)?;
    if canonical_parent != parent || !canonical_parent.starts_with(&canonical_root) {
        return Err(TransactionError::DestinationOutsideScope);
    }
    let destination = canonical_parent.join(new_name);
    if destination.parent() != Some(canonical_parent.as_path())
        || !destination.starts_with(&canonical_root)
    {
        return Err(TransactionError::DestinationOutsideScope);
    }
    let execution_strategy = destination_strategy(&canonical_source, &destination, &source)?;

    let open_source =
        File::open(&canonical_source).map_err(|_| TransactionError::SourceOpenFailed)?;
    let open_metadata = open_source
        .metadata()
        .map_err(|_| TransactionError::SourceOpenFailed)?;
    validate_open_source(&open_source, &canonical_source, &open_metadata, &source)?;

    database
        .create_rename_action_plan(ActionPlanWrite {
            scope_id,
            node_id: source.node_id,
            source_location_id: source.location_id,
            source_path_raw: &path_to_raw(&canonical_source),
            source_path_key: &comparison_key(&canonical_source),
            source_display_path: &canonical_source.to_string_lossy(),
            destination_path_raw: &path_to_raw(&destination),
            destination_path_key: &comparison_key(&destination),
            destination_display_path: &destination.to_string_lossy(),
            source_identity_kind: &source.identity_kind,
            source_identity_key: &source.identity_key,
            source_size_bytes: source.size_bytes,
            source_modified_unix_ns: source.modified_unix_ns,
            execution_strategy,
        })
        .map_err(Into::into)
}

pub fn action_plan_at(
    database_path: &Path,
    plan_id: i64,
) -> Result<ActionPlanPreview, TransactionError> {
    ManifestDatabase::open(database_path)?
        .action_plan(plan_id)
        .map_err(Into::into)
}

pub fn recent_action_plans_at(
    database_path: &Path,
) -> Result<Vec<ActionPlanSummary>, TransactionError> {
    ManifestDatabase::open(database_path)?
        .recent_action_plans()
        .map_err(Into::into)
}

fn validate_source_snapshot(
    canonical_source: &Path,
    source: &ActionSourceRecord,
    metadata: &Metadata,
) -> Result<(), TransactionError> {
    if source.identity_kind == "path_fallback" {
        return Err(TransactionError::SourceIdentityUnavailable);
    }
    let identity = platform_identity(canonical_source, metadata, IdentityNodeKind::File)
        .map_err(|_| TransactionError::SourceIdentityUnavailable)?;
    if identity.kind != source.identity_kind || identity.key != source.identity_key {
        return Err(TransactionError::SourceIdentityChanged);
    }
    if metadata.len() != source.size_bytes || modified_unix_ns(metadata) != source.modified_unix_ns
    {
        return Err(TransactionError::SourceMetadataChanged);
    }
    Ok(())
}

fn validate_open_source(
    file: &File,
    canonical_source: &Path,
    metadata: &Metadata,
    source: &ActionSourceRecord,
) -> Result<(), TransactionError> {
    let identity =
        platform_identity_for_open_file(file, canonical_source, metadata, IdentityNodeKind::File)
            .map_err(|_| TransactionError::SourceIdentityUnavailable)?;
    if identity.kind != source.identity_kind || identity.key != source.identity_key {
        return Err(TransactionError::SourceIdentityChanged);
    }
    if metadata.len() != source.size_bytes || modified_unix_ns(metadata) != source.modified_unix_ns
    {
        return Err(TransactionError::SourceMetadataChanged);
    }
    Ok(())
}

fn destination_strategy(
    source_path: &Path,
    destination: &Path,
    source: &ActionSourceRecord,
) -> Result<ActionExecutionStrategy, TransactionError> {
    let destination_metadata = match fs::symlink_metadata(destination) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == ErrorKind::NotFound => {
            return Ok(ActionExecutionStrategy::Direct);
        }
        Err(_) => return Err(TransactionError::DestinationUnavailable),
    };
    if is_symlink_or_reparse_point(&destination_metadata) || !destination_metadata.is_file() {
        return Err(TransactionError::DestinationConflict);
    }
    let destination_identity =
        platform_identity(destination, &destination_metadata, IdentityNodeKind::File)
            .map_err(|_| TransactionError::DestinationUnavailable)?;
    let source_name = source_path
        .file_name()
        .and_then(OsStr::to_str)
        .ok_or(TransactionError::DestinationConflict)?;
    let destination_name = destination
        .file_name()
        .and_then(OsStr::to_str)
        .ok_or(TransactionError::DestinationConflict)?;
    let is_ascii_case_only =
        source_name != destination_name && source_name.eq_ignore_ascii_case(destination_name);
    if is_ascii_case_only
        && destination_identity.kind == source.identity_kind
        && destination_identity.key == source.identity_key
    {
        Ok(ActionExecutionStrategy::CaseOnlyStaged)
    } else {
        Err(TransactionError::DestinationConflict)
    }
}

fn validate_portable_name(name: &str) -> Result<(), TransactionError> {
    if name.is_empty()
        || name == "."
        || name == ".."
        || name.len() > MAX_PORTABLE_NAME_BYTES
        || name.ends_with([' ', '.'])
        || name
            .chars()
            .any(|character| character.is_control() || "<>:\"/\\|?*".contains(character))
        || is_windows_reserved_name(name)
    {
        return Err(TransactionError::TargetNameInvalid);
    }
    Ok(())
}

fn is_windows_reserved_name(name: &str) -> bool {
    let stem = name
        .split('.')
        .next()
        .unwrap_or_default()
        .to_ascii_uppercase();
    matches!(stem.as_str(), "CON" | "PRN" | "AUX" | "NUL")
        || stem.strip_prefix("COM").is_some_and(|suffix| {
            matches!(suffix, "1" | "2" | "3" | "4" | "5" | "6" | "7" | "8" | "9")
        })
        || stem.strip_prefix("LPT").is_some_and(|suffix| {
            matches!(suffix, "1" | "2" | "3" | "4" | "5" | "6" | "7" | "8" | "9")
        })
}

fn modified_unix_ns(metadata: &Metadata) -> Option<i64> {
    let duration = metadata.modified().ok()?.duration_since(UNIX_EPOCH).ok()?;
    i64::try_from(duration.as_nanos()).ok()
}

fn map_source_error(_error: std::io::Error) -> TransactionError {
    TransactionError::SourceUnavailable
}

#[cfg(test)]
mod tests {
    use super::*;
    use deskgraph_domain::{ActionOperation, ActionPlanState, ActionPolicyDecision};
    use deskgraph_scanner::{authorize_scope, scan_scope};
    use std::path::PathBuf;

    struct Fixture {
        _directory: tempfile::TempDir,
        database_path: PathBuf,
        scope_path: PathBuf,
        source_path: PathBuf,
        scope_id: i64,
    }

    impl Fixture {
        fn new() -> Self {
            let directory = tempfile::tempdir().expect("fixture root should exist");
            let database_path = directory.path().join("manifest.sqlite3");
            let scope_path = directory.path().join("authorized");
            fs::create_dir(&scope_path).expect("scope should create");
            let source_path = scope_path.join("Draft.txt");
            fs::write(&source_path, "bounded preview fixture").expect("source should write");
            let mut database =
                ManifestDatabase::open(&database_path).expect("database should initialize");
            let scope = authorize_scope(&database, &scope_path).expect("scope should authorize");
            scan_scope(&mut database, scope.id).expect("scope should scan");
            drop(database);
            Self {
                _directory: directory,
                database_path,
                scope_path,
                source_path,
                scope_id: scope.id,
            }
        }
    }

    #[test]
    fn valid_preview_is_durable_explainable_and_does_not_rename() {
        let fixture = Fixture::new();
        let destination = fixture.scope_path.join("Final.txt");
        let preview = create_rename_preview_at(
            &fixture.database_path,
            fixture.scope_id,
            &fixture.source_path,
            "Final.txt",
        )
        .expect("preview should create");

        assert_eq!(preview.operation, ActionOperation::Rename);
        assert_eq!(preview.state, ActionPlanState::Previewed);
        assert_eq!(preview.policy.decision, ActionPolicyDecision::Allowed);
        assert_eq!(preview.journal_sequence, 1);
        assert_eq!(preview.execution_strategy, ActionExecutionStrategy::Direct);
        assert!(fixture.source_path.exists());
        assert!(!destination.exists());

        let reopened = action_plan_at(&fixture.database_path, preview.plan_id)
            .expect("journal should survive reopen");
        assert_eq!(reopened, preview);
        let summaries = recent_action_plans_at(&fixture.database_path)
            .expect("path-free summaries should load");
        assert_eq!(summaries.len(), 1);
        let serialized = serde_json::to_string(&summaries).expect("summary should serialize");
        assert!(!serialized.contains("Draft.txt"));
        assert!(!serialized.contains("Final.txt"));
    }

    #[test]
    fn portable_name_policy_rejects_traversal_reserved_and_no_op_names() {
        let fixture = Fixture::new();
        for name in [
            "../escape.txt",
            "nested/file.txt",
            "CON.txt",
            "bad?.txt",
            "trail. ",
        ] {
            let error = create_rename_preview_at(
                &fixture.database_path,
                fixture.scope_id,
                &fixture.source_path,
                name,
            )
            .expect_err("unsafe name should fail closed");
            assert_eq!(error.code(), "action_target_name_invalid");
        }
        let error = create_rename_preview_at(
            &fixture.database_path,
            fixture.scope_id,
            &fixture.source_path,
            "Draft.txt",
        )
        .expect_err("same name should be rejected");
        assert_eq!(error.code(), "action_rename_no_op");
    }

    #[test]
    fn destination_conflict_and_stale_manifest_fail_before_journaling() {
        let fixture = Fixture::new();
        fs::write(fixture.scope_path.join("Occupied.txt"), "other file")
            .expect("conflict should write");
        let conflict = create_rename_preview_at(
            &fixture.database_path,
            fixture.scope_id,
            &fixture.source_path,
            "Occupied.txt",
        )
        .expect_err("occupied destination should fail");
        assert_eq!(conflict.code(), "action_destination_conflict");

        fs::write(&fixture.source_path, "source changed since manifest scan")
            .expect("source should change");
        let stale = create_rename_preview_at(
            &fixture.database_path,
            fixture.scope_id,
            &fixture.source_path,
            "Fresh.txt",
        )
        .expect_err("stale source should fail");
        assert_eq!(stale.code(), "action_source_metadata_changed");
        assert!(
            recent_action_plans_at(&fixture.database_path)
                .expect("summaries should load")
                .is_empty()
        );
    }

    #[test]
    fn outside_scope_source_is_denied() {
        let fixture = Fixture::new();
        let outside = fixture
            .scope_path
            .parent()
            .expect("scope should have parent")
            .join("outside.txt");
        fs::write(&outside, "outside").expect("outside fixture should write");
        let error = create_rename_preview_at(
            &fixture.database_path,
            fixture.scope_id,
            &outside,
            "renamed.txt",
        )
        .expect_err("outside source should fail");
        assert_eq!(error.code(), "action_source_outside_scope");
    }

    #[cfg(unix)]
    #[test]
    fn symlink_source_is_denied() {
        use std::os::unix::fs::symlink;

        let fixture = Fixture::new();
        let link = fixture.scope_path.join("source-link.txt");
        symlink(&fixture.source_path, &link).expect("symlink should create");
        let error = create_rename_preview_at(
            &fixture.database_path,
            fixture.scope_id,
            &link,
            "renamed.txt",
        )
        .expect_err("symlink should fail closed");
        assert_eq!(error.code(), "action_source_symlink_or_reparse_denied");
    }

    #[test]
    fn case_only_preview_records_filesystem_strategy() {
        let fixture = Fixture::new();
        let case_alias_exists = fixture.scope_path.join("draft.txt").exists();
        let preview = create_rename_preview_at(
            &fixture.database_path,
            fixture.scope_id,
            &fixture.source_path,
            "draft.txt",
        )
        .expect("case-only preview should be safe on either filesystem behavior");
        assert_eq!(
            preview.execution_strategy,
            if case_alias_exists {
                ActionExecutionStrategy::CaseOnlyStaged
            } else {
                ActionExecutionStrategy::Direct
            }
        );
        assert!(fixture.source_path.exists());
    }
}
