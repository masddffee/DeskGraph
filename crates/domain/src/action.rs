use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ActionOperation {
    Rename,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ActionPlanState {
    Previewed,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ActionExecutionStrategy {
    Direct,
    CaseOnlyStaged,
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
    pub const API_VERSION: &str = "deskgraph.action-plan.v1";
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
    pub const API_VERSION: &str = "deskgraph.action-plan-summary.v1";
}

#[cfg(test)]
mod tests {
    use super::*;

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
}
