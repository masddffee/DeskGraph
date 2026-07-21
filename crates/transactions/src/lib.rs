mod platform_rename;

use platform_rename::direct_rename_supported;
pub use platform_rename::{PlatformRenameError, rename_same_parent_no_replace, sync_action_parent};

use std::ffi::OsStr;
use std::fmt;
use std::fs::{self, File, Metadata};
use std::io::{ErrorKind, Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

#[cfg(unix)]
use deskgraph_database::ActionCommandWrite;
use deskgraph_database::{
    ActionExecutionPlan, ActionExecutionSourceRecord, ActionJournalAppend, ActionPlanWrite,
    ActionSourceRecord, CleanupActionPlanWrite, CleanupActionSelection, CleanupKeeperBindingWrite,
    DatabaseError, ManifestDatabase, ScopeExclusionMatcher, ScopePolicyBinding,
};
use deskgraph_domain::{
    ActionCommandKind, ActionExecutionRecord, ActionExecutionStrategy, ActionJournalEventKind,
    ActionPlanPreview, ActionPlanState, ActionPlanSummary, CleanupActionPlanPreview,
    SmartCleanupSourceKind,
};
#[cfg(not(unix))]
use deskgraph_identity::platform_identity_for_open_file;
use deskgraph_identity::{
    ActionBindingError, IdentityNodeKind, comparison_key, is_symlink_or_reparse_point,
    path_from_raw, path_to_raw, platform_identity,
};
#[cfg(unix)]
use deskgraph_identity::{
    ActionEntryObservation, ActionFileBinding, IdentityExpectation, bind_action_file,
};
use deskgraph_scanner::{ScannerError, validated_scope_root};
use serde::Serialize;
use sha2::{Digest, Sha256};

const MAX_PORTABLE_NAME_BYTES: usize = 255;
const MAX_ACTION_HASH_BYTES: u64 = 8 * 1024 * 1024 * 1024;
const MAX_ACTION_HASH_DURATION: Duration = Duration::from_secs(90);
const ACTION_EXECUTOR_LEASE_MS: i64 = 120_000;
const ACTION_RECOVERY_LIMIT: u32 = 100;
const HASH_BUFFER_BYTES: usize = 64 * 1024;
const CLEANUP_COMPARE_DURATION: Duration = Duration::from_secs(5);
static EXECUTOR_TOKEN_COUNTER: AtomicU64 = AtomicU64::new(1);

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
    SourceHashTooLarge,
    SourceHashReadFailed,
    SourceHashTimedOut,
    SourceHashChanged,
    CleanupEvidenceChanged,
    ExecutionStrategyUnsupported,
    ExecutionPathInvalid,
    ActionNeedsAttention,
    ScopePolicyChanged,
    Binding(ActionBindingError),
    Platform(PlatformRenameError),
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
            Self::SourceHashTooLarge => "action_source_hash_too_large",
            Self::SourceHashReadFailed => "action_source_hash_read_failed",
            Self::SourceHashTimedOut => "action_source_hash_timed_out",
            Self::SourceHashChanged => "action_source_hash_changed",
            Self::CleanupEvidenceChanged => "cleanup_source_evidence_changed",
            Self::ExecutionStrategyUnsupported => "action_execution_strategy_unsupported",
            Self::ExecutionPathInvalid => "action_execution_path_invalid",
            Self::ActionNeedsAttention => "action_needs_attention",
            Self::ScopePolicyChanged => "scope_policy_changed",
            Self::Binding(error) => error.code(),
            Self::Platform(error) => error.code(),
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

impl From<ActionBindingError> for TransactionError {
    fn from(error: ActionBindingError) -> Self {
        Self::Binding(error)
    }
}

impl From<PlatformRenameError> for TransactionError {
    fn from(error: PlatformRenameError) -> Self {
        Self::Platform(error)
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ActionCommandResult {
    pub api_version: &'static str,
    pub plan_id: i64,
    pub command: ActionCommandKind,
    pub state: ActionPlanState,
    pub journal_sequence: u64,
    pub idempotent: bool,
}

impl ActionCommandResult {
    pub const API_VERSION: &str = "deskgraph.action-command-result.v1";
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ActionRecoveryReport {
    pub api_version: &'static str,
    pub inspected: u32,
    pub returned_to_stable: u32,
    pub completed: u32,
    pub not_applied: u32,
    pub needs_attention: u32,
    pub deferred: u32,
}

impl ActionRecoveryReport {
    pub const API_VERSION: &str = "deskgraph.action-recovery-report.v1";
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
    let read_fence = database.acquire_scope_filesystem_read_fence(scope_id)?;
    let (policy_binding, exclusion_matcher) = bind_transaction_scope_policy(database, scope_id)?;
    if read_fence.binding() != policy_binding {
        return Err(TransactionError::ScopePolicyChanged);
    }
    validate_portable_name(new_name)?;
    if !source_path.is_absolute() {
        return Err(TransactionError::SourcePathMustBeAbsolute);
    }
    ensure_transaction_path_not_excluded(&exclusion_matcher, source_path)?;
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
    ensure_transaction_path_not_excluded(&exclusion_matcher, &canonical_source)?;
    if canonical_source.file_name() == Some(OsStr::new(new_name)) {
        return Err(TransactionError::RenameNoOp);
    }
    let execution_source = database
        .action_execution_source_for_path_key(scope_id, &comparison_key(&canonical_source))
        .map_err(|error| match error {
            DatabaseError::ActionSourceNotFound
            | DatabaseError::ActionExecutionBindingUnavailable => {
                TransactionError::SourceUnavailable
            }
            other => TransactionError::Database(other),
        })?;
    let source = &execution_source.source;
    validate_source_snapshot(&canonical_source, source, &source_link_metadata)?;

    let parent = canonical_source
        .parent()
        .ok_or(TransactionError::DestinationOutsideScope)?;
    let canonical_parent =
        fs::canonicalize(parent).map_err(|_| TransactionError::DestinationUnavailable)?;
    if canonical_parent != parent || !canonical_parent.starts_with(&canonical_root) {
        return Err(TransactionError::DestinationOutsideScope);
    }
    ensure_transaction_path_not_excluded(&exclusion_matcher, &canonical_parent)?;
    let destination = canonical_parent.join(new_name);
    if destination.parent() != Some(canonical_parent.as_path())
        || !destination.starts_with(&canonical_root)
    {
        return Err(TransactionError::DestinationOutsideScope);
    }
    ensure_transaction_path_not_excluded(&exclusion_matcher, &destination)?;
    let execution_strategy = destination_strategy(&canonical_source, &destination, source)?;
    let live = create_preview_live_binding(
        &canonical_root,
        &canonical_source,
        OsStr::new(new_name),
        &execution_source,
        execution_strategy,
    )?;

    let preview = database
        .create_rename_action_plan_with_policy(
            policy_binding,
            ActionPlanWrite {
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
                source_sha256: &live.source_sha256,
                source_hash_bytes: live.source_hash_bytes,
                scope_root_identity_kind: &live.scope_root_identity_kind,
                scope_root_identity_key: &live.scope_root_identity_key,
                parent_identity_kind: &live.parent_identity_kind,
                parent_identity_key: &live.parent_identity_key,
                execution_strategy,
            },
        )
        .map_err(TransactionError::from)?;
    assert_transaction_scope_policy_current(database, policy_binding)?;
    Ok(preview)
}

pub fn create_cleanup_preview_at(
    database_path: &Path,
    selection: CleanupActionSelection,
) -> Result<CleanupActionPlanPreview, TransactionError> {
    let mut database = ManifestDatabase::open(database_path)?;
    create_cleanup_preview(&mut database, selection)
}

/// Creates one immutable, path-free preview for one explicitly selected Smart
/// Cleanup target. This performs no filesystem mutation and deliberately has
/// no confirm, execute, Trash, recovery, or Undo companion.
pub fn create_cleanup_preview(
    database: &mut ManifestDatabase,
    selection: CleanupActionSelection,
) -> Result<CleanupActionPlanPreview, TransactionError> {
    let read_fence = database.acquire_scope_filesystem_read_fence(selection.scope_id)?;
    let (policy_binding, exclusion_matcher) =
        bind_transaction_scope_policy(database, selection.scope_id)?;
    if read_fence.binding() != policy_binding {
        return Err(TransactionError::ScopePolicyChanged);
    }
    let (execution_source, keeper_execution_source) = database.cleanup_action_sources(selection)?;
    let source = &execution_source.source;
    let canonical_root = validated_scope_root(database, selection.scope_id)?;
    let source_path =
        path_from_raw(&source.path_raw).map_err(|_| TransactionError::ExecutionPathInvalid)?;
    if !source_path.is_absolute() {
        return Err(TransactionError::SourcePathMustBeAbsolute);
    }
    ensure_transaction_path_not_excluded(&exclusion_matcher, &source_path)?;
    let source_link_metadata = fs::symlink_metadata(&source_path).map_err(map_source_error)?;
    if is_symlink_or_reparse_point(&source_link_metadata) {
        return Err(TransactionError::SourceSymlinkOrReparseDenied);
    }
    if !source_link_metadata.is_file() {
        return Err(TransactionError::SourceMustBeFile);
    }
    let canonical_source = fs::canonicalize(&source_path).map_err(map_source_error)?;
    if canonical_source == canonical_root || !canonical_source.starts_with(&canonical_root) {
        return Err(TransactionError::SourceOutsideScope);
    }
    ensure_transaction_path_not_excluded(&exclusion_matcher, &canonical_source)?;
    if comparison_key(&canonical_source) != source.path_key {
        return Err(TransactionError::SourceSymlinkOrReparseDenied);
    }
    validate_source_snapshot(&canonical_source, source, &source_link_metadata)?;
    let keeper_path = keeper_execution_source
        .as_ref()
        .map(|keeper| cleanup_peer_path(&canonical_root, &exclusion_matcher, &keeper.source))
        .transpose()?;
    let binding = create_cleanup_live_binding(
        &canonical_root,
        &canonical_source,
        &execution_source,
        keeper_path.as_deref(),
        keeper_execution_source.as_ref(),
        selection.source_kind,
    )?;
    let preview = database
        .create_cleanup_action_plan_with_policy(
            policy_binding,
            CleanupActionPlanWrite {
                selection,
                target_location_id: source.location_id,
                target_identity_kind: &source.identity_kind,
                target_identity_key: &source.identity_key,
                target_size_bytes: source.size_bytes,
                target_modified_unix_ns: source.modified_unix_ns,
                target_sha256: &binding.target.source_sha256,
                target_hash_bytes: binding.target.source_hash_bytes,
                keeper: keeper_execution_source
                    .as_ref()
                    .zip(binding.keeper.as_ref())
                    .map(|(keeper, binding)| CleanupKeeperBindingWrite {
                        location_id: keeper.source.location_id,
                        identity_kind: &keeper.source.identity_kind,
                        identity_key: &keeper.source.identity_key,
                        size_bytes: keeper.source.size_bytes,
                        modified_unix_ns: keeper.source.modified_unix_ns,
                        sha256: &binding.source_sha256,
                        hash_bytes: binding.source_hash_bytes,
                        scope_root_node_id: keeper.scope_root_node_id,
                        scope_root_identity_kind: &binding.scope_root_identity_kind,
                        scope_root_identity_key: &binding.scope_root_identity_key,
                        parent_node_id: keeper.parent_node_id,
                        parent_identity_kind: &binding.parent_identity_kind,
                        parent_identity_key: &binding.parent_identity_key,
                    }),
                scope_root_node_id: execution_source.scope_root_node_id,
                scope_root_identity_kind: &binding.target.scope_root_identity_kind,
                scope_root_identity_key: &binding.target.scope_root_identity_key,
                parent_node_id: execution_source.parent_node_id,
                parent_identity_kind: &binding.target.parent_identity_kind,
                parent_identity_key: &binding.target.parent_identity_key,
            },
        )
        .map_err(TransactionError::from)?;
    assert_transaction_scope_policy_current(database, policy_binding)?;
    Ok(preview)
}

fn bind_transaction_scope_policy(
    database: &ManifestDatabase,
    scope_id: i64,
) -> Result<(ScopePolicyBinding, ScopeExclusionMatcher), TransactionError> {
    let binding = database.bind_scope_policy_revision(scope_id)?;
    let matcher = database.scope_exclusion_matcher(scope_id)?;
    if matcher.revision == binding.revision && database.is_scope_policy_binding_current(binding)? {
        Ok((binding, matcher))
    } else {
        Err(TransactionError::ScopePolicyChanged)
    }
}

fn ensure_transaction_path_not_excluded(
    matcher: &ScopeExclusionMatcher,
    path: &Path,
) -> Result<(), TransactionError> {
    if matcher.is_excluded_path_key(&comparison_key(path)) {
        Err(TransactionError::ScopePolicyChanged)
    } else {
        Ok(())
    }
}

fn assert_transaction_scope_policy_current(
    database: &ManifestDatabase,
    binding: ScopePolicyBinding,
) -> Result<(), TransactionError> {
    if database.is_scope_policy_binding_current(binding)? {
        Ok(())
    } else {
        Err(TransactionError::ScopePolicyChanged)
    }
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

pub fn execute_rename_at(
    database_path: &Path,
    plan_id: i64,
    request_id: &str,
) -> Result<ActionCommandResult, TransactionError> {
    if !direct_rename_supported() {
        return Err(TransactionError::Platform(PlatformRenameError::Unsupported));
    }
    let mut database = ManifestDatabase::open(database_path)?;
    run_rename_command(
        &mut database,
        plan_id,
        request_id,
        ActionCommandKind::Execute,
    )
}

pub fn undo_rename_at(
    database_path: &Path,
    plan_id: i64,
    request_id: &str,
) -> Result<ActionCommandResult, TransactionError> {
    if !direct_rename_supported() {
        return Err(TransactionError::Platform(PlatformRenameError::Unsupported));
    }
    let mut database = ManifestDatabase::open(database_path)?;
    run_rename_command(&mut database, plan_id, request_id, ActionCommandKind::Undo)
}

pub fn recover_rename_actions_at(
    database_path: &Path,
) -> Result<ActionRecoveryReport, TransactionError> {
    if !direct_rename_supported() {
        return Err(TransactionError::Platform(PlatformRenameError::Unsupported));
    }
    let mut database = ManifestDatabase::open(database_path)?;
    recover_rename_actions(&mut database)
}

#[cfg(unix)]
fn run_rename_command(
    database: &mut ManifestDatabase,
    plan_id: i64,
    request_id: &str,
    command: ActionCommandKind,
) -> Result<ActionCommandResult, TransactionError> {
    let plan = database.action_execution_plan(plan_id)?;
    let _read_fence = database.acquire_scope_filesystem_read_fence(plan.scope_id)?;
    validate_execution_plan(&plan)?;
    let before = database.action_execution_record(plan_id)?;
    let start = database.start_action_command(ActionCommandWrite {
        plan_id,
        request_id,
        kind: command,
        expected_sequence: before.journal_sequence,
    })?;
    if !matches!(
        start.state,
        ActionPlanState::ExecuteRequested
            | ActionPlanState::DirectRenameIntent
            | ActionPlanState::UndoRequested
            | ActionPlanState::UndoRenameIntent
    ) {
        return Ok(command_result(
            command,
            start.plan_id,
            start.state,
            start.journal_sequence,
            true,
        ));
    }

    let owner_token = executor_owner_token();
    database.acquire_action_executor_lease(plan_id, &owner_token, ACTION_EXECUTOR_LEASE_MS)?;
    let result = if matches!(
        start.state,
        ActionPlanState::DirectRenameIntent | ActionPlanState::UndoRenameIntent
    ) {
        resolve_durable_intent(
            database,
            &plan,
            start.command_request_id,
            start.state,
            start.journal_sequence,
            &owner_token,
        )
    } else {
        execute_requested_command(
            database,
            &plan,
            command,
            start.command_request_id,
            start.state,
            start.journal_sequence,
            &owner_token,
        )
    };
    let release = database.release_action_executor_lease(plan_id, &owner_token);
    match (result, release) {
        (Ok(record), Ok(())) => Ok(command_result(
            command,
            plan_id,
            record.state,
            record.journal_sequence,
            start.idempotent,
        )),
        (Err(error), _) => Err(error),
        (Ok(_), Err(error)) => Err(error.into()),
    }
}

#[cfg(not(unix))]
fn run_rename_command(
    database: &mut ManifestDatabase,
    plan_id: i64,
    _request_id: &str,
    _command: ActionCommandKind,
) -> Result<ActionCommandResult, TransactionError> {
    let plan = database.action_execution_plan(plan_id)?;
    validate_execution_plan(&plan)?;
    Err(TransactionError::Platform(PlatformRenameError::Unsupported))
}

#[cfg(unix)]
fn execute_requested_command(
    database: &mut ManifestDatabase,
    plan: &ActionExecutionPlan,
    command: ActionCommandKind,
    command_request_id: i64,
    requested_state: ActionPlanState,
    requested_sequence: u64,
    owner_token: &str,
) -> Result<ActionExecutionRecord, TransactionError> {
    let (current_path, target_path, intent_kind, not_started_kind) = match command {
        ActionCommandKind::Execute => (
            execution_source_path(plan)?,
            execution_destination_path(plan)?,
            ActionJournalEventKind::DirectRenameIntent,
            ActionJournalEventKind::ExecuteRequestNotStarted,
        ),
        ActionCommandKind::Undo => (
            execution_destination_path(plan)?,
            execution_source_path(plan)?,
            ActionJournalEventKind::UndoRenameIntent,
            ActionJournalEventKind::UndoRequestNotStarted,
        ),
    };
    let mut binding = match bind_and_verify_file(database, plan, &current_path) {
        Ok(binding) => binding,
        Err(error) => {
            let _ = append_event(
                database,
                plan.plan_id,
                command_request_id,
                requested_sequence,
                requested_state,
                not_started_kind,
                owner_token,
            );
            return Err(error);
        }
    };
    let target_name = target_path
        .file_name()
        .ok_or(TransactionError::ExecutionPathInvalid)?;
    if let Err(error) = binding.prepare_absent_destination(target_name) {
        let _ = append_event(
            database,
            plan.plan_id,
            command_request_id,
            requested_sequence,
            requested_state,
            not_started_kind,
            owner_token,
        );
        return Err(error.into());
    }

    let intent = append_event(
        database,
        plan.plan_id,
        command_request_id,
        requested_sequence,
        requested_state,
        intent_kind,
        owner_token,
    )?;
    let rename_result = rename_same_parent_no_replace(&mut binding, target_name);
    if rename_result.is_ok() {
        database.renew_action_executor_lease(
            plan.plan_id,
            owner_token,
            ACTION_EXECUTOR_LEASE_MS,
        )?;
        if binding.revalidate_bound_source().is_ok()
            && hash_matches_binding(&binding, &plan.binding.source_sha256).is_ok()
        {
            let completed_kind = match command {
                ActionCommandKind::Execute => ActionJournalEventKind::ExecutionCompleted,
                ActionCommandKind::Undo => ActionJournalEventKind::UndoCompleted,
            };
            return append_event(
                database,
                plan.plan_id,
                command_request_id,
                intent.journal_sequence,
                intent.state,
                completed_kind,
                owner_token,
            );
        }
    }

    database.renew_action_executor_lease(plan.plan_id, owner_token, ACTION_EXECUTOR_LEASE_MS)?;
    resolve_durable_intent(
        database,
        plan,
        command_request_id,
        intent.state,
        intent.journal_sequence,
        owner_token,
    )
}

#[cfg(unix)]
fn resolve_durable_intent(
    database: &mut ManifestDatabase,
    plan: &ActionExecutionPlan,
    command_request_id: i64,
    intent_state: ActionPlanState,
    intent_sequence: u64,
    owner_token: &str,
) -> Result<ActionExecutionRecord, TransactionError> {
    let observation = observe_intent(database, plan, intent_state);
    let kind = match (intent_state, observation) {
        (ActionPlanState::DirectRenameIntent, IntentObservation::Applied) => {
            ActionJournalEventKind::ExecutionCompleted
        }
        (ActionPlanState::DirectRenameIntent, IntentObservation::NotApplied) => {
            ActionJournalEventKind::ExecutionNotApplied
        }
        (ActionPlanState::DirectRenameIntent, IntentObservation::Ambiguous) => {
            ActionJournalEventKind::ExecutionNeedsAttention
        }
        (ActionPlanState::UndoRenameIntent, IntentObservation::Applied) => {
            ActionJournalEventKind::UndoCompleted
        }
        (ActionPlanState::UndoRenameIntent, IntentObservation::NotApplied) => {
            ActionJournalEventKind::UndoNotApplied
        }
        (ActionPlanState::UndoRenameIntent, IntentObservation::Ambiguous) => {
            ActionJournalEventKind::UndoNeedsAttention
        }
        _ => return Err(TransactionError::ActionNeedsAttention),
    };
    append_event(
        database,
        plan.plan_id,
        command_request_id,
        intent_sequence,
        intent_state,
        kind,
        owner_token,
    )
}

fn append_event(
    database: &mut ManifestDatabase,
    plan_id: i64,
    command_request_id: i64,
    expected_sequence: u64,
    expected_state: ActionPlanState,
    kind: ActionJournalEventKind,
    owner_token: &str,
) -> Result<ActionExecutionRecord, TransactionError> {
    database
        .append_action_journal_event(ActionJournalAppend {
            plan_id,
            command_request_id,
            expected_sequence,
            expected_state,
            kind,
            executor_lease_owner_token: owner_token,
        })
        .map_err(Into::into)
}

fn recover_rename_actions(
    database: &mut ManifestDatabase,
) -> Result<ActionRecoveryReport, TransactionError> {
    let work = database.incomplete_action_recovery(ACTION_RECOVERY_LIMIT)?;
    let mut report = ActionRecoveryReport {
        api_version: ActionRecoveryReport::API_VERSION,
        inspected: 0,
        returned_to_stable: 0,
        completed: 0,
        not_applied: 0,
        needs_attention: 0,
        deferred: 0,
    };
    for item in work {
        report.inspected += 1;
        if item.state == ActionPlanState::NeedsAttention {
            report.needs_attention += 1;
            continue;
        }
        let owner_token = executor_owner_token();
        match database.acquire_action_executor_lease(
            item.plan_id,
            &owner_token,
            ACTION_EXECUTOR_LEASE_MS,
        ) {
            Ok(_) => {}
            Err(DatabaseError::ActionExecutorLeaseUnavailable) => {
                report.deferred += 1;
                continue;
            }
            Err(error) => return Err(error.into()),
        }
        let recovered = recover_one_action(database, &item, &owner_token);
        let release = database.release_action_executor_lease(item.plan_id, &owner_token);
        let record = recovered?;
        release?;
        match (item.state, record.state) {
            (ActionPlanState::ExecuteRequested, ActionPlanState::Previewed)
            | (ActionPlanState::UndoRequested, ActionPlanState::Executed) => {
                report.returned_to_stable += 1;
            }
            (ActionPlanState::DirectRenameIntent, ActionPlanState::Executed)
            | (ActionPlanState::UndoRenameIntent, ActionPlanState::Undone) => {
                report.completed += 1;
            }
            (ActionPlanState::DirectRenameIntent, ActionPlanState::Previewed)
            | (ActionPlanState::UndoRenameIntent, ActionPlanState::Executed) => {
                report.not_applied += 1;
            }
            (_, ActionPlanState::NeedsAttention) => report.needs_attention += 1,
            _ => return Err(TransactionError::ActionNeedsAttention),
        }
    }
    Ok(report)
}

fn recover_one_action(
    database: &mut ManifestDatabase,
    item: &deskgraph_database::IncompleteActionRecovery,
    owner_token: &str,
) -> Result<ActionExecutionRecord, TransactionError> {
    match item.state {
        ActionPlanState::ExecuteRequested => append_event(
            database,
            item.plan_id,
            item.command_request_id,
            item.journal_sequence,
            item.state,
            ActionJournalEventKind::ExecuteRequestNotStarted,
            owner_token,
        ),
        ActionPlanState::UndoRequested => append_event(
            database,
            item.plan_id,
            item.command_request_id,
            item.journal_sequence,
            item.state,
            ActionJournalEventKind::UndoRequestNotStarted,
            owner_token,
        ),
        ActionPlanState::DirectRenameIntent | ActionPlanState::UndoRenameIntent => {
            #[cfg(unix)]
            {
                let plan = database.action_execution_plan(item.plan_id)?;
                let _read_fence = database.acquire_scope_filesystem_read_fence(plan.scope_id)?;
                validate_execution_plan(&plan)?;
                database.renew_action_executor_lease(
                    item.plan_id,
                    owner_token,
                    ACTION_EXECUTOR_LEASE_MS,
                )?;
                resolve_durable_intent(
                    database,
                    &plan,
                    item.command_request_id,
                    item.state,
                    item.journal_sequence,
                    owner_token,
                )
            }
            #[cfg(not(unix))]
            {
                Err(TransactionError::Platform(PlatformRenameError::Unsupported))
            }
        }
        _ => Err(TransactionError::ActionNeedsAttention),
    }
}

#[cfg(unix)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum IntentObservation {
    Applied,
    NotApplied,
    Ambiguous,
}

#[cfg(unix)]
fn observe_intent(
    database: &ManifestDatabase,
    plan: &ActionExecutionPlan,
    state: ActionPlanState,
) -> IntentObservation {
    let Ok(source) = execution_source_path(plan) else {
        return IntentObservation::Ambiguous;
    };
    let Ok(destination) = execution_destination_path(plan) else {
        return IntentObservation::Ambiguous;
    };
    let (before, after) = match state {
        ActionPlanState::DirectRenameIntent => (&source, &destination),
        ActionPlanState::UndoRenameIntent => (&destination, &source),
        _ => return IntentObservation::Ambiguous,
    };
    if let Ok(binding) = bind_and_verify_file(database, plan, before) {
        let Some(after_name) = after.file_name() else {
            return IntentObservation::Ambiguous;
        };
        return match binding.observe_current_and(after_name) {
            Ok(observation)
                if observation.current == ActionEntryObservation::ExpectedIdentity
                    && observation.alternate == ActionEntryObservation::Missing =>
            {
                IntentObservation::NotApplied
            }
            _ => IntentObservation::Ambiguous,
        };
    }
    if let Ok(binding) = bind_and_verify_file(database, plan, after) {
        let Some(before_name) = before.file_name() else {
            return IntentObservation::Ambiguous;
        };
        return match binding.observe_current_and(before_name) {
            Ok(observation)
                if observation.current == ActionEntryObservation::ExpectedIdentity
                    && observation.alternate == ActionEntryObservation::Missing =>
            {
                if sync_action_parent(&binding).is_ok() {
                    IntentObservation::Applied
                } else {
                    IntentObservation::Ambiguous
                }
            }
            _ => IntentObservation::Ambiguous,
        };
    }
    IntentObservation::Ambiguous
}

#[cfg(unix)]
fn bind_and_verify_file(
    database: &ManifestDatabase,
    plan: &ActionExecutionPlan,
    current_path: &Path,
) -> Result<ActionFileBinding, TransactionError> {
    let canonical_root = validated_scope_root(database, plan.scope_id)?;
    let binding = bind_action_file(
        &canonical_root,
        current_path,
        IdentityExpectation {
            kind: &plan.binding.scope_root_identity_kind,
            key: &plan.binding.scope_root_identity_key,
        },
        IdentityExpectation {
            kind: &plan.binding.parent_identity_kind,
            key: &plan.binding.parent_identity_key,
        },
        IdentityExpectation {
            kind: &plan.source_identity_kind,
            key: &plan.source_identity_key,
        },
    )?;
    if binding.source_size_bytes() != plan.source_size_bytes
        || binding.source_modified_unix_ns() != plan.source_modified_unix_ns
        || binding.source_size_bytes() != plan.binding.source_hash_bytes
    {
        return Err(TransactionError::SourceMetadataChanged);
    }
    hash_matches_binding(&binding, &plan.binding.source_sha256)?;
    binding.revalidate_bound_source()?;
    Ok(binding)
}

#[cfg(unix)]
fn hash_matches_binding(
    binding: &ActionFileBinding,
    expected_sha256: &[u8],
) -> Result<(), TransactionError> {
    let digest = hash_open_file(binding.source_file(), binding.source_size_bytes())?;
    if digest != expected_sha256 {
        return Err(TransactionError::SourceHashChanged);
    }
    Ok(())
}

#[cfg(unix)]
fn command_result(
    command: ActionCommandKind,
    plan_id: i64,
    state: ActionPlanState,
    journal_sequence: u64,
    idempotent: bool,
) -> ActionCommandResult {
    ActionCommandResult {
        api_version: ActionCommandResult::API_VERSION,
        plan_id,
        command,
        state,
        journal_sequence,
        idempotent,
    }
}

fn validate_execution_plan(plan: &ActionExecutionPlan) -> Result<(), TransactionError> {
    if plan.execution_strategy != ActionExecutionStrategy::Direct {
        return Err(TransactionError::ExecutionStrategyUnsupported);
    }
    let source = execution_source_path(plan)?;
    let destination = execution_destination_path(plan)?;
    if !source.is_absolute()
        || !destination.is_absolute()
        || source.parent().is_none()
        || source.parent() != destination.parent()
        || source.file_name().is_none()
        || destination.file_name().is_none()
        || source.file_name() == destination.file_name()
    {
        return Err(TransactionError::ExecutionPathInvalid);
    }
    Ok(())
}

fn execution_source_path(plan: &ActionExecutionPlan) -> Result<PathBuf, TransactionError> {
    path_from_raw(&plan.source_path_raw).map_err(|_| TransactionError::ExecutionPathInvalid)
}

fn execution_destination_path(plan: &ActionExecutionPlan) -> Result<PathBuf, TransactionError> {
    path_from_raw(&plan.destination_path_raw).map_err(|_| TransactionError::ExecutionPathInvalid)
}

fn executor_owner_token() -> String {
    let count = EXECUTOR_TOKEN_COUNTER.fetch_add(1, Ordering::Relaxed);
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    format!("deskgraph-{:x}-{now:x}-{count:x}", std::process::id())
}

#[derive(Debug)]
struct PreviewLiveBinding {
    source_sha256: Vec<u8>,
    source_hash_bytes: u64,
    scope_root_identity_kind: String,
    scope_root_identity_key: Vec<u8>,
    parent_identity_kind: String,
    parent_identity_key: Vec<u8>,
}

#[derive(Debug)]
struct CleanupLiveBinding {
    target: PreviewLiveBinding,
    keeper: Option<PreviewLiveBinding>,
}

#[cfg(unix)]
fn create_preview_live_binding(
    canonical_root: &Path,
    canonical_source: &Path,
    destination_name: &OsStr,
    execution_source: &ActionExecutionSourceRecord,
    strategy: ActionExecutionStrategy,
) -> Result<PreviewLiveBinding, TransactionError> {
    let source = &execution_source.source;
    let binding = bind_action_file(
        canonical_root,
        canonical_source,
        IdentityExpectation {
            kind: &execution_source.scope_root_identity_kind,
            key: &execution_source.scope_root_identity_key,
        },
        IdentityExpectation {
            kind: &execution_source.parent_identity_kind,
            key: &execution_source.parent_identity_key,
        },
        IdentityExpectation {
            kind: &source.identity_kind,
            key: &source.identity_key,
        },
    )?;
    if binding.source_size_bytes() != source.size_bytes
        || binding.source_modified_unix_ns() != source.modified_unix_ns
    {
        return Err(TransactionError::SourceMetadataChanged);
    }
    if strategy == ActionExecutionStrategy::Direct {
        binding.prepare_absent_destination(destination_name)?;
    }
    let source_sha256 = hash_open_file(binding.source_file(), binding.source_size_bytes())?;
    binding.revalidate_bound_source()?;
    Ok(PreviewLiveBinding {
        source_sha256,
        source_hash_bytes: binding.source_size_bytes(),
        scope_root_identity_kind: binding.root_identity().kind.to_owned(),
        scope_root_identity_key: binding.root_identity().key.clone(),
        parent_identity_kind: binding.parent_identity().kind.to_owned(),
        parent_identity_key: binding.parent_identity().key.clone(),
    })
}

#[cfg(not(unix))]
fn create_preview_live_binding(
    canonical_root: &Path,
    canonical_source: &Path,
    _destination_name: &OsStr,
    execution_source: &ActionExecutionSourceRecord,
    _strategy: ActionExecutionStrategy,
) -> Result<PreviewLiveBinding, TransactionError> {
    let parent = canonical_source
        .parent()
        .ok_or(TransactionError::ExecutionPathInvalid)?;
    let root = File::open(canonical_root).map_err(|_| TransactionError::SourceOpenFailed)?;
    let parent_file = File::open(parent).map_err(|_| TransactionError::SourceOpenFailed)?;
    let source_file =
        File::open(canonical_source).map_err(|_| TransactionError::SourceOpenFailed)?;
    let root_metadata = root
        .metadata()
        .map_err(|_| TransactionError::SourceOpenFailed)?;
    let parent_metadata = parent_file
        .metadata()
        .map_err(|_| TransactionError::SourceOpenFailed)?;
    let source_metadata = source_file
        .metadata()
        .map_err(|_| TransactionError::SourceOpenFailed)?;
    let root_identity = platform_identity_for_open_file(
        &root,
        canonical_root,
        &root_metadata,
        IdentityNodeKind::Folder,
    )
    .map_err(|_| TransactionError::SourceIdentityUnavailable)?;
    let parent_identity = platform_identity_for_open_file(
        &parent_file,
        parent,
        &parent_metadata,
        IdentityNodeKind::Folder,
    )
    .map_err(|_| TransactionError::SourceIdentityUnavailable)?;
    validate_open_source(
        &source_file,
        canonical_source,
        &source_metadata,
        &execution_source.source,
    )?;
    if root_identity.kind != execution_source.scope_root_identity_kind
        || root_identity.key != execution_source.scope_root_identity_key
        || parent_identity.kind != execution_source.parent_identity_kind
        || parent_identity.key != execution_source.parent_identity_key
    {
        return Err(TransactionError::SourceIdentityChanged);
    }
    let source_sha256 = hash_open_file(&source_file, source_metadata.len())?;
    let final_metadata = source_file
        .metadata()
        .map_err(|_| TransactionError::SourceOpenFailed)?;
    validate_open_source(
        &source_file,
        canonical_source,
        &final_metadata,
        &execution_source.source,
    )?;
    Ok(PreviewLiveBinding {
        source_sha256,
        source_hash_bytes: source_metadata.len(),
        scope_root_identity_kind: root_identity.kind.to_owned(),
        scope_root_identity_key: root_identity.key,
        parent_identity_kind: parent_identity.kind.to_owned(),
        parent_identity_key: parent_identity.key,
    })
}

fn cleanup_peer_path(
    canonical_root: &Path,
    exclusion_matcher: &ScopeExclusionMatcher,
    source: &ActionSourceRecord,
) -> Result<PathBuf, TransactionError> {
    let path =
        path_from_raw(&source.path_raw).map_err(|_| TransactionError::ExecutionPathInvalid)?;
    if !path.is_absolute() {
        return Err(TransactionError::SourcePathMustBeAbsolute);
    }
    ensure_transaction_path_not_excluded(exclusion_matcher, &path)?;
    let link_metadata = fs::symlink_metadata(&path).map_err(map_source_error)?;
    if is_symlink_or_reparse_point(&link_metadata) {
        return Err(TransactionError::SourceSymlinkOrReparseDenied);
    }
    if !link_metadata.is_file() {
        return Err(TransactionError::SourceMustBeFile);
    }
    let canonical = fs::canonicalize(&path).map_err(map_source_error)?;
    if canonical == canonical_root || !canonical.starts_with(canonical_root) {
        return Err(TransactionError::SourceOutsideScope);
    }
    ensure_transaction_path_not_excluded(exclusion_matcher, &canonical)?;
    if comparison_key(&canonical) != source.path_key {
        return Err(TransactionError::SourceSymlinkOrReparseDenied);
    }
    validate_source_snapshot(&canonical, source, &link_metadata)?;
    Ok(canonical)
}

#[cfg(unix)]
fn create_cleanup_live_binding(
    canonical_root: &Path,
    canonical_target: &Path,
    target: &ActionExecutionSourceRecord,
    canonical_keeper: Option<&Path>,
    keeper: Option<&ActionExecutionSourceRecord>,
    source_kind: SmartCleanupSourceKind,
) -> Result<CleanupLiveBinding, TransactionError> {
    if canonical_keeper.is_some() != keeper.is_some() {
        return Err(TransactionError::CleanupEvidenceChanged);
    }
    let target_binding = bind_action_file(
        canonical_root,
        canonical_target,
        IdentityExpectation {
            kind: &target.scope_root_identity_kind,
            key: &target.scope_root_identity_key,
        },
        IdentityExpectation {
            kind: &target.parent_identity_kind,
            key: &target.parent_identity_key,
        },
        IdentityExpectation {
            kind: &target.source.identity_kind,
            key: &target.source.identity_key,
        },
    )?;
    if target_binding.source_size_bytes() != target.source.size_bytes
        || target_binding.source_modified_unix_ns() != target.source.modified_unix_ns
    {
        return Err(TransactionError::SourceMetadataChanged);
    }
    let keeper_binding = match (canonical_keeper, keeper) {
        (Some(path), Some(source)) => {
            let binding = bind_action_file(
                canonical_root,
                path,
                IdentityExpectation {
                    kind: &source.scope_root_identity_kind,
                    key: &source.scope_root_identity_key,
                },
                IdentityExpectation {
                    kind: &source.parent_identity_kind,
                    key: &source.parent_identity_key,
                },
                IdentityExpectation {
                    kind: &source.source.identity_kind,
                    key: &source.source.identity_key,
                },
            )?;
            if binding.source_size_bytes() != source.source.size_bytes
                || binding.source_modified_unix_ns() != source.source.modified_unix_ns
            {
                return Err(TransactionError::SourceMetadataChanged);
            }
            Some(binding)
        }
        (None, None) => None,
        _ => return Err(TransactionError::CleanupEvidenceChanged),
    };
    if source_kind == SmartCleanupSourceKind::ExactDuplicate {
        let keeper_binding = keeper_binding
            .as_ref()
            .ok_or(TransactionError::CleanupEvidenceChanged)?;
        compare_open_files_exact(
            target_binding.source_file(),
            keeper_binding.source_file(),
            target_binding.source_size_bytes(),
        )?;
    }
    let source_sha256 = hash_open_file(
        target_binding.source_file(),
        target_binding.source_size_bytes(),
    )?;
    let keeper_sha256 = keeper_binding
        .as_ref()
        .map(|binding| hash_open_file(binding.source_file(), binding.source_size_bytes()))
        .transpose()?;
    target_binding.revalidate_bound_source()?;
    if let Some(binding) = &keeper_binding {
        binding.revalidate_bound_source()?;
    }
    validate_cleanup_final_hashes(
        source_kind,
        &source_sha256,
        target_binding.source_size_bytes(),
        keeper_sha256.as_deref(),
        keeper_binding
            .as_ref()
            .map(ActionFileBinding::source_size_bytes),
    )?;
    let keeper = keeper_binding
        .zip(keeper_sha256)
        .map(|(binding, sha256)| PreviewLiveBinding {
            source_sha256: sha256,
            source_hash_bytes: binding.source_size_bytes(),
            scope_root_identity_kind: binding.root_identity().kind.to_owned(),
            scope_root_identity_key: binding.root_identity().key.clone(),
            parent_identity_kind: binding.parent_identity().kind.to_owned(),
            parent_identity_key: binding.parent_identity().key.clone(),
        });
    Ok(CleanupLiveBinding {
        target: PreviewLiveBinding {
            source_sha256,
            source_hash_bytes: target_binding.source_size_bytes(),
            scope_root_identity_kind: target_binding.root_identity().kind.to_owned(),
            scope_root_identity_key: target_binding.root_identity().key.clone(),
            parent_identity_kind: target_binding.parent_identity().kind.to_owned(),
            parent_identity_key: target_binding.parent_identity().key.clone(),
        },
        keeper,
    })
}

#[cfg(not(unix))]
fn create_cleanup_live_binding(
    canonical_root: &Path,
    canonical_target: &Path,
    target: &ActionExecutionSourceRecord,
    canonical_keeper: Option<&Path>,
    keeper: Option<&ActionExecutionSourceRecord>,
    source_kind: SmartCleanupSourceKind,
) -> Result<CleanupLiveBinding, TransactionError> {
    if canonical_keeper.is_some() != keeper.is_some() {
        return Err(TransactionError::CleanupEvidenceChanged);
    }
    let target_parent = canonical_target
        .parent()
        .ok_or(TransactionError::ExecutionPathInvalid)?;
    let root_file = File::open(canonical_root).map_err(|_| TransactionError::SourceOpenFailed)?;
    let target_parent_file =
        File::open(target_parent).map_err(|_| TransactionError::SourceOpenFailed)?;
    let target_file =
        File::open(canonical_target).map_err(|_| TransactionError::SourceOpenFailed)?;
    let root_metadata = root_file
        .metadata()
        .map_err(|_| TransactionError::SourceOpenFailed)?;
    let target_parent_metadata = target_parent_file
        .metadata()
        .map_err(|_| TransactionError::SourceOpenFailed)?;
    let target_metadata = target_file
        .metadata()
        .map_err(|_| TransactionError::SourceOpenFailed)?;
    let root_identity = platform_identity_for_open_file(
        &root_file,
        canonical_root,
        &root_metadata,
        IdentityNodeKind::Folder,
    )
    .map_err(|_| TransactionError::SourceIdentityUnavailable)?;
    let target_parent_identity = platform_identity_for_open_file(
        &target_parent_file,
        target_parent,
        &target_parent_metadata,
        IdentityNodeKind::Folder,
    )
    .map_err(|_| TransactionError::SourceIdentityUnavailable)?;
    validate_open_source(
        &target_file,
        canonical_target,
        &target_metadata,
        &target.source,
    )?;
    if root_identity.kind != target.scope_root_identity_kind
        || root_identity.key != target.scope_root_identity_key
        || target_parent_identity.kind != target.parent_identity_kind
        || target_parent_identity.key != target.parent_identity_key
    {
        return Err(TransactionError::SourceIdentityChanged);
    }
    let keeper_file = match (canonical_keeper, keeper) {
        (Some(path), Some(source)) => {
            let parent = path
                .parent()
                .ok_or(TransactionError::ExecutionPathInvalid)?;
            let parent_file = File::open(parent).map_err(|_| TransactionError::SourceOpenFailed)?;
            let file = File::open(path).map_err(|_| TransactionError::SourceOpenFailed)?;
            let parent_metadata = parent_file
                .metadata()
                .map_err(|_| TransactionError::SourceOpenFailed)?;
            let metadata = file
                .metadata()
                .map_err(|_| TransactionError::SourceOpenFailed)?;
            let parent_identity = platform_identity_for_open_file(
                &parent_file,
                parent,
                &parent_metadata,
                IdentityNodeKind::Folder,
            )
            .map_err(|_| TransactionError::SourceIdentityUnavailable)?;
            validate_open_source(&file, path, &metadata, &source.source)?;
            if root_identity.kind != source.scope_root_identity_kind
                || root_identity.key != source.scope_root_identity_key
                || parent_identity.kind != source.parent_identity_kind
                || parent_identity.key != source.parent_identity_key
            {
                return Err(TransactionError::SourceIdentityChanged);
            }
            Some((file, path, source))
        }
        (None, None) => None,
        _ => return Err(TransactionError::CleanupEvidenceChanged),
    };
    if source_kind == SmartCleanupSourceKind::ExactDuplicate {
        let (file, _, _) = keeper_file
            .as_ref()
            .ok_or(TransactionError::CleanupEvidenceChanged)?;
        compare_open_files_exact(&target_file, file, target_metadata.len())?;
    }
    let source_sha256 = hash_open_file(&target_file, target_metadata.len())?;
    let keeper_sha256 = keeper_file
        .as_ref()
        .map(|(file, _, source)| hash_open_file(file, source.source.size_bytes))
        .transpose()?;
    validate_open_source(
        &target_file,
        canonical_target,
        &target_file
            .metadata()
            .map_err(|_| TransactionError::SourceOpenFailed)?,
        &target.source,
    )?;
    if let Some((file, path, source)) = &keeper_file {
        validate_open_source(
            file,
            path,
            &file
                .metadata()
                .map_err(|_| TransactionError::SourceOpenFailed)?,
            &source.source,
        )?;
    }
    validate_cleanup_final_hashes(
        source_kind,
        &source_sha256,
        target_metadata.len(),
        keeper_sha256.as_deref(),
        keeper_file
            .as_ref()
            .map(|(_, _, source)| source.source.size_bytes),
    )?;
    let keeper = keeper_file
        .zip(keeper_sha256)
        .map(|((_, _, source), sha256)| PreviewLiveBinding {
            source_sha256: sha256,
            source_hash_bytes: source.source.size_bytes,
            scope_root_identity_kind: source.scope_root_identity_kind.clone(),
            scope_root_identity_key: source.scope_root_identity_key.clone(),
            parent_identity_kind: source.parent_identity_kind.clone(),
            parent_identity_key: source.parent_identity_key.clone(),
        });
    Ok(CleanupLiveBinding {
        target: PreviewLiveBinding {
            source_sha256,
            source_hash_bytes: target_metadata.len(),
            scope_root_identity_kind: root_identity.kind.to_owned(),
            scope_root_identity_key: root_identity.key,
            parent_identity_kind: target_parent_identity.kind.to_owned(),
            parent_identity_key: target_parent_identity.key,
        },
        keeper,
    })
}

fn compare_open_files_exact(
    left: &File,
    right: &File,
    expected_size: u64,
) -> Result<(), TransactionError> {
    let started = Instant::now();
    let mut left = left
        .try_clone()
        .map_err(|_| TransactionError::SourceHashReadFailed)?;
    let mut right = right
        .try_clone()
        .map_err(|_| TransactionError::SourceHashReadFailed)?;
    left.seek(SeekFrom::Start(0))
        .map_err(|_| TransactionError::SourceHashReadFailed)?;
    right
        .seek(SeekFrom::Start(0))
        .map_err(|_| TransactionError::SourceHashReadFailed)?;
    let mut left_buffer = [0_u8; HASH_BUFFER_BYTES];
    let mut right_buffer = [0_u8; HASH_BUFFER_BYTES];
    let mut total = 0_u64;
    loop {
        if started.elapsed() > CLEANUP_COMPARE_DURATION {
            return Err(TransactionError::SourceHashTimedOut);
        }
        let left_read = left
            .read(&mut left_buffer)
            .map_err(|_| TransactionError::SourceHashReadFailed)?;
        let right_read = right
            .read(&mut right_buffer)
            .map_err(|_| TransactionError::SourceHashReadFailed)?;
        if left_read != right_read || left_buffer[..left_read] != right_buffer[..right_read] {
            return Err(TransactionError::CleanupEvidenceChanged);
        }
        if left_read == 0 {
            break;
        }
        total = total
            .checked_add(
                u64::try_from(left_read).map_err(|_| TransactionError::SourceHashReadFailed)?,
            )
            .ok_or(TransactionError::SourceHashTooLarge)?;
        if total > expected_size {
            return Err(TransactionError::SourceMetadataChanged);
        }
    }
    if total != expected_size {
        return Err(TransactionError::SourceMetadataChanged);
    }
    Ok(())
}

fn validate_cleanup_final_hashes(
    source_kind: SmartCleanupSourceKind,
    target_sha256: &[u8],
    target_hash_bytes: u64,
    keeper_sha256: Option<&[u8]>,
    keeper_hash_bytes: Option<u64>,
) -> Result<(), TransactionError> {
    if source_kind == SmartCleanupSourceKind::ExactDuplicate
        && (keeper_sha256 != Some(target_sha256) || keeper_hash_bytes != Some(target_hash_bytes))
    {
        return Err(TransactionError::CleanupEvidenceChanged);
    }
    Ok(())
}

fn hash_open_file(file: &File, expected_size: u64) -> Result<Vec<u8>, TransactionError> {
    if expected_size > MAX_ACTION_HASH_BYTES {
        return Err(TransactionError::SourceHashTooLarge);
    }
    let started = Instant::now();
    let mut reader = file
        .try_clone()
        .map_err(|_| TransactionError::SourceHashReadFailed)?;
    reader
        .seek(SeekFrom::Start(0))
        .map_err(|_| TransactionError::SourceHashReadFailed)?;
    let mut hasher = Sha256::new();
    let mut buffer = [0_u8; HASH_BUFFER_BYTES];
    let mut total = 0_u64;
    loop {
        if started.elapsed() > MAX_ACTION_HASH_DURATION {
            return Err(TransactionError::SourceHashTimedOut);
        }
        let read = reader
            .read(&mut buffer)
            .map_err(|_| TransactionError::SourceHashReadFailed)?;
        if read == 0 {
            break;
        }
        total = total
            .checked_add(u64::try_from(read).map_err(|_| TransactionError::SourceHashReadFailed)?)
            .ok_or(TransactionError::SourceHashTooLarge)?;
        if total > expected_size || total > MAX_ACTION_HASH_BYTES {
            return Err(TransactionError::SourceMetadataChanged);
        }
        hasher.update(&buffer[..read]);
    }
    if total != expected_size {
        return Err(TransactionError::SourceMetadataChanged);
    }
    Ok(hasher.finalize().to_vec())
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

#[cfg(not(unix))]
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
    _source: &ActionSourceRecord,
) -> Result<ActionExecutionStrategy, TransactionError> {
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
    if is_ascii_case_only {
        return Ok(ActionExecutionStrategy::CaseOnlyStaged);
    }
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
    Err(TransactionError::DestinationConflict)
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
    use deskgraph_database::ScopeExclusionWrite;
    use deskgraph_domain::{
        ActionJournalEventKind, ActionOperation, ActionPlanState, ActionPolicyDecision,
        CleanupActionOperation, CleanupActionPlanState, SmartCleanupSourceKind,
    };
    use deskgraph_scanner::{
        ScopeExclusionSelection, authorize_scope, prepare_scope_exclusion_batch, scan_scope,
    };
    use std::fs::OpenOptions;
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
            database
                .upsert_scope_access_grant(scope.id, std::env::consts::OS, b"test-active-grant")
                .expect("test grant should activate");
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

    fn exclude_path(database: &mut ManifestDatabase, scope_id: i64, path: &Path) {
        let prepared = prepare_scope_exclusion_batch(
            database,
            scope_id,
            &[ScopeExclusionSelection {
                requested_path: path,
            }],
        )
        .expect("strict fixture exclusion should prepare");
        let writes = prepared
            .exclusions
            .iter()
            .map(|exclusion| ScopeExclusionWrite {
                kind: exclusion.kind,
                path_raw: &exclusion.path_raw,
                path_key: &exclusion.path_key,
                display_path: &exclusion.display_path,
                identity_kind: &exclusion.identity_kind,
                identity_key: &exclusion.identity_key,
            })
            .collect::<Vec<_>>();
        let binding = database
            .bind_scope_policy_revision(scope_id)
            .expect("active fixture scope should bind");
        database
            .apply_scope_exclusion_batch(binding, &writes, 1)
            .expect("fixture exclusion should commit");
    }

    #[test]
    fn rename_preview_rejects_an_excluded_destination_without_persisting_a_plan() {
        let fixture = Fixture::new();
        let destination = fixture.scope_path.join("Renamed.txt");
        fs::write(&destination, "excluded destination fixture")
            .expect("destination fixture should write");
        let mut database =
            ManifestDatabase::open(&fixture.database_path).expect("database should reopen");
        scan_scope(&mut database, fixture.scope_id).expect("rescan should include destination");
        exclude_path(&mut database, fixture.scope_id, &destination);
        drop(database);

        let error = create_rename_preview_at(
            &fixture.database_path,
            fixture.scope_id,
            &fixture.source_path,
            "Renamed.txt",
        )
        .expect_err("excluded destination must fail closed");
        assert_eq!(error.code(), "scope_policy_changed");
        let database =
            ManifestDatabase::open(&fixture.database_path).expect("database should reopen");
        assert!(
            database
                .recent_action_plans()
                .expect("plans should load")
                .is_empty(),
            "a denied destination must not create an action plan"
        );
    }

    struct CleanupFixture {
        _directory: tempfile::TempDir,
        database_path: PathBuf,
        keeper_path: PathBuf,
        target_path: PathBuf,
        selection: CleanupActionSelection,
    }

    impl CleanupFixture {
        fn new() -> Self {
            let directory = tempfile::tempdir().expect("fixture root should exist");
            let database_path = directory.path().join("manifest.sqlite3");
            let scope_path = directory.path().join("authorized");
            fs::create_dir(&scope_path).expect("scope should create");
            let keeper_path = scope_path.join("Keeper.txt");
            let target_path = scope_path.join("Target.txt");
            fs::write(&keeper_path, "same cleanup bytes").expect("keeper should write");
            fs::write(&target_path, "same cleanup bytes").expect("target should write");
            let mut database =
                ManifestDatabase::open(&database_path).expect("database should initialize");
            let scope = authorize_scope(&database, &scope_path).expect("scope should authorize");
            database
                .upsert_scope_access_grant(scope.id, std::env::consts::OS, b"test-active-grant")
                .expect("test grant should activate");
            scan_scope(&mut database, scope.id).expect("scope should scan");
            let canonical_keeper =
                fs::canonicalize(&keeper_path).expect("keeper should canonicalize");
            let canonical_target =
                fs::canonicalize(&target_path).expect("target should canonicalize");
            let keeper = database
                .action_source_for_path_key(scope.id, &comparison_key(&canonical_keeper))
                .expect("keeper should load");
            let target = database
                .action_source_for_path_key(scope.id, &comparison_key(&canonical_target))
                .expect("target should load");
            let binding = database
                .bind_scope_policy_revision(scope.id)
                .expect("active fixture scope should bind");
            let candidate = database
                .record_exact_duplicate_candidate_with_policy(binding, &keeper, &target)
                .expect("fresh evidence should persist");
            let inbox_item = database
                .smart_cleanup_relation_item(
                    candidate.relation_id,
                    candidate.evidence.observed_at_unix_ms,
                )
                .expect("fresh evidence should become a path-free item");
            let selection = CleanupActionSelection {
                scope_id: scope.id,
                source_kind: SmartCleanupSourceKind::ExactDuplicate,
                source_id: candidate.relation_id,
                source_observation_id: inbox_item.source_observation_id,
                keeper_node_id: Some(keeper.node_id),
                target_node_id: target.node_id,
            };
            drop(database);
            Self {
                _directory: directory,
                database_path,
                keeper_path,
                target_path,
                selection,
            }
        }
    }

    #[test]
    fn cleanup_preview_is_durable_path_free_and_never_mutates_the_target() {
        let fixture = CleanupFixture::new();
        let preview = create_cleanup_preview_at(&fixture.database_path, fixture.selection)
            .expect("current explicit cleanup selection should preview");
        assert_eq!(
            preview.operation,
            CleanupActionOperation::SystemTrashPreview
        );
        assert_eq!(preview.state, CleanupActionPlanState::Previewed);
        assert_eq!(preview.journal_sequence, 1);
        assert!(preview.policy.confirmation_required);
        assert!(!preview.policy.action_authorized);
        assert!(!preview.policy.execution_available);
        assert!(preview.keeper_hash_bound);
        assert!(fixture.keeper_path.exists());
        assert!(fixture.target_path.exists());
        assert_eq!(
            fs::read_to_string(&fixture.target_path).expect("target should remain readable"),
            "same cleanup bytes"
        );

        let reopened = ManifestDatabase::open(&fixture.database_path)
            .expect("database should reopen")
            .cleanup_action_plan(preview.plan_id)
            .expect("preview should survive reopen");
        assert_eq!(reopened, preview);
        let serialized = serde_json::to_string(&preview).expect("preview should serialize");
        assert!(!serialized.contains("Keeper.txt"));
        assert!(!serialized.contains("Target.txt"));
        assert!(!serialized.contains("sha256"));
    }

    #[test]
    fn cleanup_preview_rejects_an_excluded_keeper_without_persisting_a_plan() {
        let fixture = CleanupFixture::new();
        let mut database =
            ManifestDatabase::open(&fixture.database_path).expect("database should reopen");
        exclude_path(
            &mut database,
            fixture.selection.scope_id,
            &fixture.keeper_path,
        );
        drop(database);

        let error = create_cleanup_preview_at(&fixture.database_path, fixture.selection)
            .expect_err("excluded keeper must fail closed");
        assert!(
            matches!(
                error.code(),
                "scope_policy_changed" | "cleanup_action_source_not_current"
            ),
            "privacy purge or the runtime guard must deny an excluded keeper; code={}",
            error.code()
        );
        assert!(fixture.target_path.exists());
        assert!(fixture.keeper_path.exists());
    }

    #[test]
    fn cleanup_preview_rejects_same_size_and_mtime_keeper_content_change() {
        let fixture = CleanupFixture::new();
        let original_metadata =
            fs::metadata(&fixture.keeper_path).expect("keeper metadata should exist");
        let original_modified = original_metadata
            .modified()
            .expect("keeper mtime should exist");
        let original_modified_unix_ns = modified_unix_ns(&original_metadata);
        let changed =
            vec![b'X'; usize::try_from(original_metadata.len()).expect("size should fit")];
        fs::write(&fixture.keeper_path, changed).expect("same-size keeper change should write");
        let keeper = OpenOptions::new()
            .write(true)
            .open(&fixture.keeper_path)
            .expect("keeper should reopen");
        keeper
            .set_times(std::fs::FileTimes::new().set_modified(original_modified))
            .expect("keeper mtime should restore");
        drop(keeper);
        let restored_metadata =
            fs::metadata(&fixture.keeper_path).expect("restored metadata should exist");
        assert_eq!(restored_metadata.len(), original_metadata.len());
        assert_eq!(
            modified_unix_ns(&restored_metadata),
            original_modified_unix_ns
        );

        let error = create_cleanup_preview_at(&fixture.database_path, fixture.selection)
            .expect_err("keeper content drift must invalidate exact-duplicate evidence");
        assert_eq!(error.code(), "cleanup_source_evidence_changed");
        assert!(fixture.keeper_path.exists());
        assert!(fixture.target_path.exists());
    }

    #[test]
    fn cleanup_preview_rejects_refreshed_observation_and_changed_target() {
        let fixture = CleanupFixture::new();
        let mut database =
            ManifestDatabase::open(&fixture.database_path).expect("database should reopen");
        let (left, right) = database
            .exact_duplicate_sources(fixture.selection.source_id)
            .expect("relation sources should load");
        let binding = database
            .bind_scope_policy_revision(left.scope_id)
            .expect("active fixture scope should bind");
        let refreshed = database
            .record_exact_duplicate_candidate_with_policy(binding, &left, &right)
            .expect("refresh should append a new observation");
        let refreshed_item = database
            .smart_cleanup_relation_item(
                refreshed.relation_id,
                refreshed.evidence.observed_at_unix_ms,
            )
            .expect("refreshed observation should become current");
        drop(database);
        let stale = create_cleanup_preview_at(&fixture.database_path, fixture.selection)
            .expect_err("old observation must not be silently replaced");
        assert_eq!(stale.code(), "cleanup_action_source_not_current");
        fs::write(&fixture.target_path, "changed after manifest").expect("target should change");
        let mut latest_selection = fixture.selection;
        latest_selection.source_observation_id = refreshed_item.source_observation_id;
        let changed = create_cleanup_preview_at(&fixture.database_path, latest_selection)
            .expect_err("changed target must fail before preview persistence");
        assert!(matches!(
            changed,
            TransactionError::SourceIdentityChanged | TransactionError::SourceMetadataChanged
        ));
    }

    #[test]
    fn cleanup_exact_pair_comparison_rejects_equal_length_different_bytes() {
        let directory = tempfile::tempdir().expect("fixture root should exist");
        let left_path = directory.path().join("left.bin");
        let right_path = directory.path().join("right.bin");
        fs::write(&left_path, b"same-size-left!").expect("left should write");
        fs::write(&right_path, b"same-size-right").expect("right should write");
        let left = File::open(&left_path).expect("left should open read-only");
        let right = File::open(&right_path).expect("right should open read-only");
        let error = compare_open_files_exact(&left, &right, 15)
            .expect_err("equal-length different content must invalidate exact evidence");
        assert_eq!(error.code(), "cleanup_source_evidence_changed");
    }

    #[test]
    fn cleanup_exact_duplicate_requires_matching_final_hashes_and_byte_counts() {
        let target_sha256 = [7_u8; 32];
        let keeper_sha256 = [8_u8; 32];
        assert!(matches!(
            validate_cleanup_final_hashes(
                SmartCleanupSourceKind::ExactDuplicate,
                &target_sha256,
                42,
                Some(&keeper_sha256),
                Some(42),
            ),
            Err(TransactionError::CleanupEvidenceChanged)
        ));
        assert!(matches!(
            validate_cleanup_final_hashes(
                SmartCleanupSourceKind::ExactDuplicate,
                &target_sha256,
                42,
                Some(&target_sha256),
                Some(41),
            ),
            Err(TransactionError::CleanupEvidenceChanged)
        ));
        assert!(
            validate_cleanup_final_hashes(
                SmartCleanupSourceKind::ExactDuplicate,
                &target_sha256,
                42,
                Some(&target_sha256),
                Some(42),
            )
            .is_ok()
        );
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
        let preview = create_rename_preview_at(
            &fixture.database_path,
            fixture.scope_id,
            &fixture.source_path,
            "draft.txt",
        )
        .expect("case-only preview should be safe on either filesystem behavior");
        assert_eq!(
            preview.execution_strategy,
            ActionExecutionStrategy::CaseOnlyStaged
        );
        assert!(fixture.source_path.exists());
    }

    #[cfg(unix)]
    #[test]
    fn direct_execute_and_undo_are_durable_and_idempotent() {
        let fixture = Fixture::new();
        let destination = fixture.scope_path.join("Final.txt");
        let preview = create_rename_preview_at(
            &fixture.database_path,
            fixture.scope_id,
            &fixture.source_path,
            "Final.txt",
        )
        .expect("preview should create");

        let executed = execute_rename_at(
            &fixture.database_path,
            preview.plan_id,
            "execute-request-0001",
        )
        .expect("execute should succeed");
        assert_eq!(executed.state, ActionPlanState::Executed);
        assert!(!executed.idempotent);
        assert!(!fixture.source_path.exists());
        assert_eq!(
            fs::read_to_string(&destination).expect("renamed file should remain readable"),
            "bounded preview fixture"
        );

        let replay = execute_rename_at(
            &fixture.database_path,
            preview.plan_id,
            "execute-request-0001",
        )
        .expect("same request should replay");
        assert_eq!(replay.state, ActionPlanState::Executed);
        assert!(replay.idempotent);

        let undone = undo_rename_at(&fixture.database_path, preview.plan_id, "undo-request-0001")
            .expect("undo should succeed");
        assert_eq!(undone.state, ActionPlanState::Undone);
        assert!(fixture.source_path.exists());
        assert!(!destination.exists());

        let undo_replay =
            undo_rename_at(&fixture.database_path, preview.plan_id, "undo-request-0001")
                .expect("same undo request should replay");
        assert_eq!(undo_replay.state, ActionPlanState::Undone);
        assert!(undo_replay.idempotent);
    }

    #[cfg(unix)]
    #[test]
    fn destination_race_never_overwrites_and_returns_preview_to_stable() {
        let fixture = Fixture::new();
        let destination = fixture.scope_path.join("Final.txt");
        let preview = create_rename_preview_at(
            &fixture.database_path,
            fixture.scope_id,
            &fixture.source_path,
            "Final.txt",
        )
        .expect("preview should create");
        fs::write(&destination, "unrelated race winner").expect("race file should create");

        let error = execute_rename_at(&fixture.database_path, preview.plan_id, "execute-race-0001")
            .expect_err("occupied destination must fail");
        assert_eq!(error.code(), "action_binding_destination_conflict");
        assert_eq!(
            fs::read_to_string(&destination).expect("race winner must remain"),
            "unrelated race winner"
        );
        assert!(fixture.source_path.exists());
        assert_eq!(
            action_plan_at(&fixture.database_path, preview.plan_id)
                .expect("plan should reload")
                .state,
            ActionPlanState::Previewed
        );
    }

    #[cfg(unix)]
    #[test]
    fn competing_request_ids_issue_only_one_filesystem_action() {
        let fixture = Fixture::new();
        let preview = create_rename_preview_at(
            &fixture.database_path,
            fixture.scope_id,
            &fixture.source_path,
            "Final.txt",
        )
        .expect("preview should create");
        let first_database = fixture.database_path.clone();
        let second_database = fixture.database_path.clone();
        let plan_id = preview.plan_id;
        let first = std::thread::spawn(move || {
            execute_rename_at(&first_database, plan_id, "competing-request-0001")
        });
        let second = std::thread::spawn(move || {
            execute_rename_at(&second_database, plan_id, "competing-request-0002")
        });
        let outcomes = [
            first.join().expect("first thread should join"),
            second.join().expect("second thread should join"),
        ];
        assert_eq!(outcomes.iter().filter(|result| result.is_ok()).count(), 1);
        assert_eq!(
            action_plan_at(&fixture.database_path, preview.plan_id)
                .expect("plan should reload")
                .state,
            ActionPlanState::Executed
        );
        assert!(!fixture.source_path.exists());
        assert_eq!(
            fs::read_to_string(fixture.scope_path.join("Final.txt"))
                .expect("destination should remain readable"),
            "bounded preview fixture"
        );
    }

    #[cfg(unix)]
    #[test]
    fn same_size_and_mtime_content_change_is_caught_by_bound_hash() {
        let fixture = Fixture::new();
        let preview = create_rename_preview_at(
            &fixture.database_path,
            fixture.scope_id,
            &fixture.source_path,
            "Final.txt",
        )
        .expect("preview should create");
        let modified = fs::metadata(&fixture.source_path)
            .and_then(|metadata| metadata.modified())
            .expect("mtime should exist");
        fs::write(&fixture.source_path, "changed preview fixture").expect("same size write");
        let file = OpenOptions::new()
            .write(true)
            .open(&fixture.source_path)
            .expect("source should reopen");
        file.set_times(std::fs::FileTimes::new().set_modified(modified))
            .expect("mtime should restore");

        let error = execute_rename_at(&fixture.database_path, preview.plan_id, "execute-hash-0001")
            .expect_err("content hash mismatch must fail");
        assert_eq!(error.code(), "action_source_hash_changed");
        assert!(fixture.source_path.exists());
        assert!(!fixture.scope_path.join("Final.txt").exists());
    }

    #[cfg(unix)]
    #[test]
    fn hard_link_added_after_preview_blocks_execution() {
        let fixture = Fixture::new();
        let preview = create_rename_preview_at(
            &fixture.database_path,
            fixture.scope_id,
            &fixture.source_path,
            "Final.txt",
        )
        .expect("preview should create");
        fs::hard_link(&fixture.source_path, fixture.scope_path.join("alias.txt"))
            .expect("hard link should create");

        let error = execute_rename_at(&fixture.database_path, preview.plan_id, "execute-link-0001")
            .expect_err("hard link must fail closed");
        assert_eq!(error.code(), "action_binding_source_hard_link_denied");
        assert!(fixture.source_path.exists());
    }

    #[cfg(unix)]
    #[test]
    fn case_only_plan_remains_preview_only_in_this_slice() {
        let fixture = Fixture::new();
        let preview = create_rename_preview_at(
            &fixture.database_path,
            fixture.scope_id,
            &fixture.source_path,
            "draft.txt",
        )
        .expect("case-only preview should create");
        let error = execute_rename_at(&fixture.database_path, preview.plan_id, "execute-case-0001")
            .expect_err("case-only execution is not part of this slice");
        assert_eq!(error.code(), "action_execution_strategy_unsupported");
        assert!(fixture.source_path.exists());
    }

    #[cfg(unix)]
    #[test]
    fn recovery_closes_requested_command_without_mutation() {
        let fixture = Fixture::new();
        let preview = create_rename_preview_at(
            &fixture.database_path,
            fixture.scope_id,
            &fixture.source_path,
            "Final.txt",
        )
        .expect("preview should create");
        let mut database =
            ManifestDatabase::open(&fixture.database_path).expect("database should open");
        database
            .start_action_command(ActionCommandWrite {
                plan_id: preview.plan_id,
                request_id: "crash-before-intent-0001",
                kind: ActionCommandKind::Execute,
                expected_sequence: 1,
            })
            .expect("requested event should persist");
        drop(database);

        let report = recover_rename_actions_at(&fixture.database_path)
            .expect("recovery should close request");
        assert_eq!(report.returned_to_stable, 1);
        assert!(fixture.source_path.exists());
        assert_eq!(
            action_plan_at(&fixture.database_path, preview.plan_id)
                .expect("plan should reload")
                .state,
            ActionPlanState::Previewed
        );
    }

    #[cfg(unix)]
    #[test]
    fn recovery_observes_both_sides_of_durable_intent() {
        let not_applied = Fixture::new();
        let not_applied_preview = persist_execute_intent(&not_applied, false);
        let report = recover_rename_actions_at(&not_applied.database_path)
            .expect("unapplied intent should recover");
        assert_eq!(report.not_applied, 1);
        assert_eq!(
            action_plan_at(&not_applied.database_path, not_applied_preview.plan_id)
                .expect("plan should reload")
                .state,
            ActionPlanState::Previewed
        );

        let applied = Fixture::new();
        let applied_preview = persist_execute_intent(&applied, true);
        let report = recover_rename_actions_at(&applied.database_path)
            .expect("applied intent should recover");
        assert_eq!(report.completed, 1);
        assert_eq!(
            action_plan_at(&applied.database_path, applied_preview.plan_id)
                .expect("plan should reload")
                .state,
            ActionPlanState::Executed
        );
        assert!(!applied.source_path.exists());
        assert!(applied.scope_path.join("Final.txt").exists());
    }

    #[cfg(unix)]
    #[test]
    fn recovery_observes_both_sides_of_durable_undo_intent() {
        let not_applied = Fixture::new();
        let not_applied_preview = persist_undo_intent(&not_applied, false);
        let report = recover_rename_actions_at(&not_applied.database_path)
            .expect("unapplied undo intent should recover");
        assert_eq!(report.not_applied, 1);
        assert_eq!(
            action_plan_at(&not_applied.database_path, not_applied_preview.plan_id)
                .expect("plan should reload")
                .state,
            ActionPlanState::Executed
        );

        let applied = Fixture::new();
        let applied_preview = persist_undo_intent(&applied, true);
        let report = recover_rename_actions_at(&applied.database_path)
            .expect("applied undo intent should recover");
        assert_eq!(report.completed, 1);
        assert_eq!(
            action_plan_at(&applied.database_path, applied_preview.plan_id)
                .expect("plan should reload")
                .state,
            ActionPlanState::Undone
        );
        assert!(applied.source_path.exists());
        assert!(!applied.scope_path.join("Final.txt").exists());
    }

    #[cfg(unix)]
    #[test]
    fn ambiguous_intent_fails_closed_to_needs_attention() {
        let fixture = Fixture::new();
        let preview = persist_execute_intent(&fixture, false);
        fs::write(fixture.scope_path.join("Final.txt"), "unrelated file")
            .expect("ambiguous destination should create");

        let report = recover_rename_actions_at(&fixture.database_path)
            .expect("recovery should journal ambiguity");
        assert_eq!(report.needs_attention, 1);
        assert_eq!(
            action_plan_at(&fixture.database_path, preview.plan_id)
                .expect("plan should reload")
                .state,
            ActionPlanState::NeedsAttention
        );
        assert!(fixture.source_path.exists());
    }

    #[cfg(unix)]
    fn persist_execute_intent(fixture: &Fixture, apply_rename: bool) -> ActionPlanPreview {
        let preview = create_rename_preview_at(
            &fixture.database_path,
            fixture.scope_id,
            &fixture.source_path,
            "Final.txt",
        )
        .expect("preview should create");
        let mut database =
            ManifestDatabase::open(&fixture.database_path).expect("database should open");
        let start = database
            .start_action_command(ActionCommandWrite {
                plan_id: preview.plan_id,
                request_id: "crash-after-intent-0001",
                kind: ActionCommandKind::Execute,
                expected_sequence: 1,
            })
            .expect("command should start");
        let owner = "test-executor-owner-0001";
        database
            .acquire_action_executor_lease(preview.plan_id, owner, ACTION_EXECUTOR_LEASE_MS)
            .expect("lease should acquire");
        let intent = database
            .append_action_journal_event(ActionJournalAppend {
                plan_id: preview.plan_id,
                command_request_id: start.command_request_id,
                expected_sequence: start.journal_sequence,
                expected_state: start.state,
                kind: ActionJournalEventKind::DirectRenameIntent,
                executor_lease_owner_token: owner,
            })
            .expect("intent should persist");
        assert_eq!(intent.state, ActionPlanState::DirectRenameIntent);
        if apply_rename {
            let plan = database
                .action_execution_plan(preview.plan_id)
                .expect("execution plan should load");
            let source_path = execution_source_path(&plan).expect("source path should decode");
            let mut binding =
                bind_and_verify_file(&database, &plan, &source_path).expect("source should bind");
            rename_same_parent_no_replace(&mut binding, OsStr::new("Final.txt"))
                .expect("filesystem rename should apply");
        }
        database
            .release_action_executor_lease(preview.plan_id, owner)
            .expect("lease should release");
        preview
    }

    #[cfg(unix)]
    fn persist_undo_intent(fixture: &Fixture, apply_rename: bool) -> ActionPlanPreview {
        let preview = create_rename_preview_at(
            &fixture.database_path,
            fixture.scope_id,
            &fixture.source_path,
            "Final.txt",
        )
        .expect("preview should create");
        execute_rename_at(
            &fixture.database_path,
            preview.plan_id,
            "execute-before-undo-0001",
        )
        .expect("execute should complete");
        let mut database =
            ManifestDatabase::open(&fixture.database_path).expect("database should open");
        let current = database
            .action_execution_record(preview.plan_id)
            .expect("record should load");
        let start = database
            .start_action_command(ActionCommandWrite {
                plan_id: preview.plan_id,
                request_id: "crash-after-undo-intent-0001",
                kind: ActionCommandKind::Undo,
                expected_sequence: current.journal_sequence,
            })
            .expect("undo command should start");
        let owner = "test-undo-owner-0001";
        database
            .acquire_action_executor_lease(preview.plan_id, owner, ACTION_EXECUTOR_LEASE_MS)
            .expect("lease should acquire");
        let intent = database
            .append_action_journal_event(ActionJournalAppend {
                plan_id: preview.plan_id,
                command_request_id: start.command_request_id,
                expected_sequence: start.journal_sequence,
                expected_state: start.state,
                kind: ActionJournalEventKind::UndoRenameIntent,
                executor_lease_owner_token: owner,
            })
            .expect("undo intent should persist");
        assert_eq!(intent.state, ActionPlanState::UndoRenameIntent);
        if apply_rename {
            let plan = database
                .action_execution_plan(preview.plan_id)
                .expect("execution plan should load");
            let destination_path =
                execution_destination_path(&plan).expect("destination should decode");
            let mut binding = bind_and_verify_file(&database, &plan, &destination_path)
                .expect("destination should bind");
            rename_same_parent_no_replace(&mut binding, OsStr::new("Draft.txt"))
                .expect("filesystem undo should apply");
        }
        database
            .release_action_executor_lease(preview.plan_id, owner)
            .expect("lease should release");
        preview
    }
}
