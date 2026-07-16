mod action;
mod extraction;
mod health;
mod manifest;
mod project;
mod search;
mod watch;

pub use action::{
    ActionExecutionStrategy, ActionOperation, ActionPlanPreview, ActionPlanState,
    ActionPlanSummary, ActionPolicyCheck, ActionPolicyDecision, ActionPolicyReport,
};
pub use extraction::{ExtractionJobProgress, ExtractionStats, ExtractionStatus};
pub use health::{
    ComponentHealth, HealthReport, LifecycleState, PlatformHealth, PrivacyHealth, ProviderHealth,
    collect_health, collect_health_with_manifest,
};
pub use manifest::{AuthorizedScope, ManifestStats, ScanJobProgress, ScanReport, ScanStatus};
pub use project::{
    FileRelationCandidate, FileRelationCandidateState, FileRelationComparisonKind,
    FileRelationCreator, FileRelationEndpoint, FileRelationEvidence, FileRelationKind,
    FolderCategoryCount, FolderFileCategory, FolderProfile, ProjectCandidate,
    ProjectCandidateState, ProjectCandidateSummary, ProjectDecision, ProjectDecisionCreator,
    ProjectDecisionKind, ProjectSignal, ProjectSignalKind, ProjectSuggestion,
    ProjectSuggestionCreator,
};
pub use search::{
    SearchFilters, SearchMatchedField, SearchMode, SearchResponse, SearchResult, SearchSourceFilter,
};
pub use watch::{WatchEventProgress, WatchEventReason, WatchEventStatus};
