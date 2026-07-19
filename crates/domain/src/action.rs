use serde::{Deserialize, Serialize};

use crate::project::SmartCleanupSourceKind;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ActionOperation {
    Rename,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ActionPlanState {
    Previewed,
    ExecuteRequested,
    DirectRenameIntent,
    Executed,
    UndoRequested,
    UndoRenameIntent,
    Undone,
    NeedsAttention,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ActionExecutionStrategy {
    Direct,
    CaseOnlyStaged,
}

/// A closed journal vocabulary. Unknown events are deliberately not reduced
/// into a plausible state by callers; database decoding rejects them.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ActionJournalEventKind {
    PreviewCreated,
    ExecuteRequested,
    ExecuteRequestNotStarted,
    DirectRenameIntent,
    ExecutionCompleted,
    ExecutionNotApplied,
    ExecutionNeedsAttention,
    UndoRequested,
    UndoRequestNotStarted,
    UndoRenameIntent,
    UndoCompleted,
    UndoNotApplied,
    UndoNeedsAttention,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ActionCommandKind {
    Execute,
    Undo,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ActionJournalEvent {
    pub api_version: &'static str,
    pub event_id: i64,
    pub plan_id: i64,
    pub sequence: u64,
    pub kind: ActionJournalEventKind,
    pub command_request_id: Option<i64>,
    pub created_at_unix_ms: i64,
}

impl ActionJournalEvent {
    pub const API_VERSION: &str = "deskgraph.action-journal.v1";
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ActionExecutionBinding {
    pub api_version: &'static str,
    pub source_hash_bytes: u64,
    pub source_sha256: Vec<u8>,
    pub scope_root_node_id: i64,
    pub scope_root_identity_kind: String,
    pub scope_root_identity_key: Vec<u8>,
    pub parent_node_id: i64,
    pub parent_identity_kind: String,
    pub parent_identity_key: Vec<u8>,
    pub created_at_unix_ms: i64,
}

impl ActionExecutionBinding {
    pub const API_VERSION: &str = "deskgraph.action-execution-binding.v1";
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ActionExecutionRecord {
    pub api_version: &'static str,
    pub plan_id: i64,
    pub operation: ActionOperation,
    pub execution_strategy: ActionExecutionStrategy,
    pub state: ActionPlanState,
    pub journal_sequence: u64,
    pub binding: ActionExecutionBinding,
}

impl ActionExecutionRecord {
    pub const API_VERSION: &str = "deskgraph.action-execution-record.v1";
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ActionCommandStart {
    pub api_version: &'static str,
    pub command_request_id: i64,
    pub plan_id: i64,
    pub kind: ActionCommandKind,
    pub state: ActionPlanState,
    pub journal_sequence: u64,
    pub idempotent: bool,
}

impl ActionCommandStart {
    pub const API_VERSION: &str = "deskgraph.action-command-start.v1";
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ActionJournalReductionError {
    Empty,
    NonMonotonicSequence,
    InvalidInitialEvent,
    InvalidTransition,
}

/// Derives the only executable state from immutable events. `not_applied`
/// events deliberately return to the prior stable state, while ambiguous
/// events fail closed into `NeedsAttention`.
pub fn reduce_action_journal(
    events: &[ActionJournalEvent],
) -> Result<ActionPlanState, ActionJournalReductionError> {
    let Some(first) = events.first() else {
        return Err(ActionJournalReductionError::Empty);
    };
    if first.sequence != 1 || first.kind != ActionJournalEventKind::PreviewCreated {
        return Err(ActionJournalReductionError::InvalidInitialEvent);
    }
    let mut previous_sequence = first.sequence;
    let mut state = ActionPlanState::Previewed;
    for event in &events[1..] {
        if event.sequence != previous_sequence.saturating_add(1) {
            return Err(ActionJournalReductionError::NonMonotonicSequence);
        }
        state = match (state, event.kind) {
            (ActionPlanState::Previewed, ActionJournalEventKind::ExecuteRequested) => {
                ActionPlanState::ExecuteRequested
            }
            (
                ActionPlanState::ExecuteRequested,
                ActionJournalEventKind::ExecuteRequestNotStarted,
            ) => ActionPlanState::Previewed,
            (ActionPlanState::ExecuteRequested, ActionJournalEventKind::DirectRenameIntent) => {
                ActionPlanState::DirectRenameIntent
            }
            (ActionPlanState::DirectRenameIntent, ActionJournalEventKind::ExecutionCompleted) => {
                ActionPlanState::Executed
            }
            (ActionPlanState::DirectRenameIntent, ActionJournalEventKind::ExecutionNotApplied) => {
                ActionPlanState::Previewed
            }
            (
                ActionPlanState::DirectRenameIntent,
                ActionJournalEventKind::ExecutionNeedsAttention,
            ) => ActionPlanState::NeedsAttention,
            (ActionPlanState::Executed, ActionJournalEventKind::UndoRequested) => {
                ActionPlanState::UndoRequested
            }
            (ActionPlanState::UndoRequested, ActionJournalEventKind::UndoRequestNotStarted) => {
                ActionPlanState::Executed
            }
            (ActionPlanState::UndoRequested, ActionJournalEventKind::UndoRenameIntent) => {
                ActionPlanState::UndoRenameIntent
            }
            (ActionPlanState::UndoRenameIntent, ActionJournalEventKind::UndoCompleted) => {
                ActionPlanState::Undone
            }
            (ActionPlanState::UndoRenameIntent, ActionJournalEventKind::UndoNotApplied) => {
                ActionPlanState::Executed
            }
            (ActionPlanState::UndoRenameIntent, ActionJournalEventKind::UndoNeedsAttention) => {
                ActionPlanState::NeedsAttention
            }
            _ => return Err(ActionJournalReductionError::InvalidTransition),
        };
        previous_sequence = event.sequence;
    }
    Ok(state)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ActionPolicyDecision {
    Allowed,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ActionPolicyCheck {
    ExplicitAuthorizedScope,
    PresentManifestFile,
    CanonicalSourceContained,
    SourceIdentityMatches,
    ReadOnlyHandleIdentityMatches,
    PortableSingleComponentName,
    SameCanonicalParent,
    DestinationContained,
    DestinationAvailable,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ActionPolicyReport {
    pub api_version: &'static str,
    pub decision: ActionPolicyDecision,
    pub checks: Vec<ActionPolicyCheck>,
}

impl ActionPolicyReport {
    pub const API_VERSION: &str = "deskgraph.action-policy.v1";

    #[must_use]
    pub fn rename_allowed() -> Self {
        Self {
            api_version: Self::API_VERSION,
            decision: ActionPolicyDecision::Allowed,
            checks: vec![
                ActionPolicyCheck::ExplicitAuthorizedScope,
                ActionPolicyCheck::PresentManifestFile,
                ActionPolicyCheck::CanonicalSourceContained,
                ActionPolicyCheck::SourceIdentityMatches,
                ActionPolicyCheck::ReadOnlyHandleIdentityMatches,
                ActionPolicyCheck::PortableSingleComponentName,
                ActionPolicyCheck::SameCanonicalParent,
                ActionPolicyCheck::DestinationContained,
                ActionPolicyCheck::DestinationAvailable,
            ],
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ActionPlanPreview {
    pub api_version: &'static str,
    pub plan_id: i64,
    pub operation: ActionOperation,
    pub state: ActionPlanState,
    pub scope_id: i64,
    pub node_id: i64,
    pub source_path: String,
    pub destination_path: String,
    pub execution_strategy: ActionExecutionStrategy,
    pub policy: ActionPolicyReport,
    pub journal_sequence: u64,
    pub created_at_unix_ms: i64,
}

impl ActionPlanPreview {
    pub const API_VERSION: &str = "deskgraph.action-plan.v2";
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ActionPlanSummary {
    pub api_version: &'static str,
    pub plan_id: i64,
    pub operation: ActionOperation,
    pub state: ActionPlanState,
    pub scope_id: i64,
    pub node_id: i64,
    pub execution_strategy: ActionExecutionStrategy,
    pub journal_sequence: u64,
    pub created_at_unix_ms: i64,
}

impl ActionPlanSummary {
    pub const API_VERSION: &str = "deskgraph.action-plan-summary.v2";
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CleanupActionOperation {
    SystemTrashPreview,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CleanupActionPlanState {
    Previewed,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CleanupActionPolicyCheck {
    ExplicitAuthorizedScope,
    ActiveScopeGrant,
    SuggestedSource,
    ExactSourceObservation,
    SelectedMember,
    KeeperDistinctWhenPresent,
    PresentManifestFile,
    StrongTargetIdentity,
    ReadOnlyHandleIdentityMatches,
    TargetHashBound,
    KeeperSnapshotAndHashBoundWhenPresent,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct CleanupActionPolicyReport {
    pub api_version: &'static str,
    pub checks: Vec<CleanupActionPolicyCheck>,
    pub confirmation_required: bool,
    pub action_authorized: bool,
    pub execution_available: bool,
}

impl CleanupActionPolicyReport {
    pub const API_VERSION: &str = "deskgraph.cleanup-action-policy.v1";

    #[must_use]
    pub fn preview_only() -> Self {
        Self {
            api_version: Self::API_VERSION,
            checks: vec![
                CleanupActionPolicyCheck::ExplicitAuthorizedScope,
                CleanupActionPolicyCheck::ActiveScopeGrant,
                CleanupActionPolicyCheck::SuggestedSource,
                CleanupActionPolicyCheck::ExactSourceObservation,
                CleanupActionPolicyCheck::SelectedMember,
                CleanupActionPolicyCheck::KeeperDistinctWhenPresent,
                CleanupActionPolicyCheck::PresentManifestFile,
                CleanupActionPolicyCheck::StrongTargetIdentity,
                CleanupActionPolicyCheck::ReadOnlyHandleIdentityMatches,
                CleanupActionPolicyCheck::TargetHashBound,
                CleanupActionPolicyCheck::KeeperSnapshotAndHashBoundWhenPresent,
            ],
            confirmation_required: true,
            action_authorized: false,
            execution_available: false,
        }
    }
}

/// A path-free, immutable preview of one explicitly selected cleanup target.
/// It is deliberately not a command, confirmation, transaction receipt, or
/// authorization to invoke a platform Trash API.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct CleanupActionPlanPreview {
    pub api_version: &'static str,
    pub plan_id: i64,
    pub operation: CleanupActionOperation,
    pub state: CleanupActionPlanState,
    pub scope_id: i64,
    pub source_kind: SmartCleanupSourceKind,
    pub source_id: i64,
    pub source_observation_id: i64,
    pub keeper_node_id: Option<i64>,
    pub target_node_id: i64,
    pub expected_bytes: u64,
    pub keeper_hash_bound: bool,
    pub policy: CleanupActionPolicyReport,
    pub journal_sequence: u64,
    pub created_at_unix_ms: i64,
}

impl CleanupActionPlanPreview {
    pub const API_VERSION: &str = "deskgraph.cleanup-action-plan-preview.v1";
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CleanupSourceMemberRole {
    DuplicateCandidate,
    OlderVersion,
    NewerVersion,
    ScreenshotCandidate,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CleanupSourceSelectionRule {
    EitherMemberIsTarget,
    OlderTargetNewerKeeper,
    SingleTargetNoKeeper,
}

/// A transient, explicitly requested local detail response for one current
/// Smart Cleanup source. Paths are intentionally allowed here so the user can
/// identify local files, but this value is never persisted as a plan or
/// journal event and cannot authorize an action.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct CleanupSourceDetailMember {
    pub node_id: i64,
    pub display_path: String,
    pub size_bytes: u64,
    pub role: CleanupSourceMemberRole,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct CleanupSourceDetail {
    pub api_version: &'static str,
    pub scope_id: i64,
    pub source_kind: SmartCleanupSourceKind,
    pub source_id: i64,
    pub source_observation_id: i64,
    pub members: Vec<CleanupSourceDetailMember>,
    pub selection_rule: CleanupSourceSelectionRule,
    pub current_evidence: bool,
    pub user_requested_paths: bool,
    pub action_authorized: bool,
    pub execution_available: bool,
}

impl CleanupSourceDetail {
    pub const API_VERSION: &str = "deskgraph.cleanup-source-detail.v1";
    pub const MAX_MEMBERS: usize = 20;
}

#[cfg(test)]
mod tests {
    use super::*;

    fn event(sequence: u64, kind: ActionJournalEventKind) -> ActionJournalEvent {
        ActionJournalEvent {
            api_version: ActionJournalEvent::API_VERSION,
            event_id: i64::try_from(sequence).expect("test sequence should fit"),
            plan_id: 1,
            sequence,
            kind,
            command_request_id: (sequence > 1).then_some(1),
            created_at_unix_ms: 1,
        }
    }

    #[test]
    fn preview_contract_is_versioned_and_explainable() {
        let report = ActionPolicyReport::rename_allowed();
        assert_eq!(report.api_version, "deskgraph.action-policy.v1");
        assert_eq!(report.decision, ActionPolicyDecision::Allowed);
        assert_eq!(report.checks.len(), 9);
        assert!(
            report
                .checks
                .contains(&ActionPolicyCheck::ReadOnlyHandleIdentityMatches)
        );
        assert!(
            report
                .checks
                .contains(&ActionPolicyCheck::DestinationAvailable)
        );
    }

    #[test]
    fn cleanup_preview_contract_is_path_free_and_cannot_authorize_an_action() {
        let preview = CleanupActionPlanPreview {
            api_version: CleanupActionPlanPreview::API_VERSION,
            plan_id: 1,
            operation: CleanupActionOperation::SystemTrashPreview,
            state: CleanupActionPlanState::Previewed,
            scope_id: 2,
            source_kind: SmartCleanupSourceKind::ExactDuplicate,
            source_id: 3,
            source_observation_id: 4,
            keeper_node_id: Some(5),
            target_node_id: 6,
            expected_bytes: 7,
            keeper_hash_bound: true,
            policy: CleanupActionPolicyReport::preview_only(),
            journal_sequence: 1,
            created_at_unix_ms: 8,
        };
        let value = serde_json::to_value(preview).expect("cleanup preview should serialize");
        assert_eq!(value["operation"], "system_trash_preview");
        assert_eq!(value["policy"]["confirmation_required"], true);
        assert_eq!(value["policy"]["action_authorized"], false);
        assert_eq!(value["policy"]["execution_available"], false);
        assert_eq!(value["keeper_hash_bound"], true);
        assert!(value.get("source_path").is_none());
        assert!(value.get("target_path").is_none());
        assert!(value.get("target_sha256").is_none());
    }

    #[test]
    fn cleanup_source_detail_paths_are_transient_and_non_executable() {
        let detail = CleanupSourceDetail {
            api_version: CleanupSourceDetail::API_VERSION,
            scope_id: 1,
            source_kind: SmartCleanupSourceKind::ExactDuplicate,
            source_id: 2,
            source_observation_id: 3,
            members: vec![CleanupSourceDetailMember {
                node_id: 4,
                display_path: "/authorized/private.txt".to_string(),
                size_bytes: 5,
                role: CleanupSourceMemberRole::DuplicateCandidate,
            }],
            selection_rule: CleanupSourceSelectionRule::EitherMemberIsTarget,
            current_evidence: true,
            user_requested_paths: true,
            action_authorized: false,
            execution_available: false,
        };
        let value = serde_json::to_value(detail).expect("cleanup detail should serialize");
        assert_eq!(value["api_version"], CleanupSourceDetail::API_VERSION);
        assert_eq!(
            value["members"][0]["display_path"],
            "/authorized/private.txt"
        );
        assert_eq!(value["members"][0]["role"], "duplicate_candidate");
        assert_eq!(value["selection_rule"], "either_member_is_target");
        assert_eq!(value["current_evidence"], true);
        assert_eq!(value["user_requested_paths"], true);
        assert_eq!(value["action_authorized"], false);
        assert_eq!(value["execution_available"], false);
        assert!(value["members"][0].get("modified_unix_ns").is_none());
    }

    #[test]
    fn journal_reducer_returns_to_stable_state_when_a_command_never_starts() {
        assert_eq!(
            reduce_action_journal(&[
                event(1, ActionJournalEventKind::PreviewCreated),
                event(2, ActionJournalEventKind::ExecuteRequested),
                event(3, ActionJournalEventKind::ExecuteRequestNotStarted),
            ]),
            Ok(ActionPlanState::Previewed)
        );
        assert_eq!(
            reduce_action_journal(&[
                event(1, ActionJournalEventKind::PreviewCreated),
                event(2, ActionJournalEventKind::ExecuteRequested),
                event(3, ActionJournalEventKind::DirectRenameIntent),
                event(4, ActionJournalEventKind::ExecutionCompleted),
                event(5, ActionJournalEventKind::UndoRequested),
                event(6, ActionJournalEventKind::UndoRequestNotStarted),
            ]),
            Ok(ActionPlanState::Executed)
        );
    }

    #[test]
    fn journal_reducer_rejects_arbitrary_or_non_monotonic_events() {
        assert_eq!(
            reduce_action_journal(&[
                event(1, ActionJournalEventKind::PreviewCreated),
                event(2, ActionJournalEventKind::ExecutionCompleted),
            ]),
            Err(ActionJournalReductionError::InvalidTransition)
        );
        assert_eq!(
            reduce_action_journal(&[
                event(1, ActionJournalEventKind::PreviewCreated),
                event(3, ActionJournalEventKind::ExecuteRequested),
            ]),
            Err(ActionJournalReductionError::NonMonotonicSequence)
        );
    }
}
