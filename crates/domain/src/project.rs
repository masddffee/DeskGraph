use std::path::Path;

use serde::{Deserialize, Serialize};
use unicode_normalization::UnicodeNormalization;

use crate::extraction::ImageFormat;

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FolderFileCategory {
    Document,
    Code,
    Image,
    Data,
    Archive,
    Media,
    Other,
}

impl FolderFileCategory {
    pub const ALL: [Self; 7] = [
        Self::Document,
        Self::Code,
        Self::Image,
        Self::Data,
        Self::Archive,
        Self::Media,
        Self::Other,
    ];
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct FolderCategoryCount {
    pub category: FolderFileCategory,
    pub file_count: u64,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProjectSignalKind {
    CargoManifest,
    JavaScriptPackage,
    PythonProject,
    GoModule,
    SwiftPackage,
    XcodeProject,
    VisualStudioSolution,
    Readme,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ProjectSignal {
    pub kind: ProjectSignalKind,
    pub marker_name: String,
    pub weight_basis_points: u16,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProjectSuggestionCreator {
    SystemRule,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ProjectSuggestion {
    pub confidence_basis_points: u16,
    pub provenance: Vec<ProjectSignal>,
    pub observed_at_unix_ms: i64,
    pub created_by: ProjectSuggestionCreator,
    pub provider_id: &'static str,
    pub provider_version: &'static str,
    pub model_version: Option<String>,
}

impl ProjectSuggestion {
    pub const PROVIDER_ID: &str = "deskgraph.folder-marker-rules";
    pub const PROVIDER_VERSION: &str = "1";
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProjectCandidateState {
    Suggested,
    Accepted,
    Rejected,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProjectDecisionKind {
    Accepted,
    Rejected,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProjectDecisionCreator {
    User,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ProjectDecision {
    pub sequence: u64,
    pub kind: ProjectDecisionKind,
    pub created_by: ProjectDecisionCreator,
    pub decided_at_unix_ms: i64,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ProjectCandidate {
    pub api_version: &'static str,
    pub project_id: i64,
    pub scope_id: i64,
    pub root_folder_node_id: i64,
    pub root_folder_location_id: i64,
    pub display_path: String,
    pub state: ProjectCandidateState,
    pub suggestion: ProjectSuggestion,
    pub latest_decision: Option<ProjectDecision>,
}

impl ProjectCandidate {
    pub const API_VERSION: &str = "deskgraph.project-candidate.v1";
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ProjectCandidateSummary {
    pub api_version: &'static str,
    pub project_id: i64,
    pub scope_id: i64,
    pub root_folder_node_id: i64,
    pub state: ProjectCandidateState,
    pub confidence_basis_points: u16,
    pub observed_at_unix_ms: i64,
    pub latest_decision_at_unix_ms: Option<i64>,
}

impl ProjectCandidateSummary {
    pub const API_VERSION: &str = "deskgraph.project-candidate-summary.v1";
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FileRelationKind {
    ExactDuplicate,
    Version,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FileRelationCandidateState {
    Suggested,
    Accepted,
    Rejected,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FileRelationComparisonKind {
    ByteForByte,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FileRelationCreator {
    SystemRule,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FileRelationDecisionKind {
    Accepted,
    Rejected,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FileRelationDecisionCreator {
    User,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct FileRelationDecision {
    pub sequence: u64,
    pub kind: FileRelationDecisionKind,
    pub created_by: FileRelationDecisionCreator,
    pub decided_at_unix_ms: i64,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct FileVersionDecision {
    pub sequence: u64,
    pub evidence_observation_id: i64,
    pub kind: FileRelationDecisionKind,
    pub created_by: FileRelationDecisionCreator,
    pub decided_at_unix_ms: i64,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct FileRelationEndpoint {
    pub scope_id: i64,
    pub node_id: i64,
    pub location_id: i64,
    pub display_path: String,
    pub size_bytes: u64,
    pub modified_unix_ns: Option<i64>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct FileRelationEvidence {
    pub comparison_kind: FileRelationComparisonKind,
    pub compared_bytes: u64,
    pub confidence_basis_points: u16,
    pub observed_at_unix_ms: i64,
    pub created_by: FileRelationCreator,
    pub provider_id: &'static str,
    pub provider_version: &'static str,
    pub model_version: Option<String>,
    pub bounded_max_bytes: u64,
}

impl FileRelationEvidence {
    pub const PROVIDER_ID: &str = "deskgraph.byte-equality";
    pub const PROVIDER_VERSION: &str = "1";
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FileVersionSignalKind {
    ExplicitNumericSuffix,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct FileVersionEvidence {
    pub signal_kind: FileVersionSignalKind,
    pub base_key: String,
    pub extension_key: String,
    pub older_version: u32,
    pub newer_version: u32,
    pub confidence_basis_points: u16,
    pub observed_at_unix_ms: i64,
    pub created_by: FileRelationCreator,
    pub provider_id: &'static str,
    pub provider_version: &'static str,
    pub model_version: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExplicitFileVersionName {
    pub base_key: String,
    pub extension_key: String,
    pub version: u32,
}

#[must_use]
pub fn parse_explicit_file_version_name(file_name: &str) -> Option<ExplicitFileVersionName> {
    let path = Path::new(file_name);
    if path.file_name()?.to_str()? != file_name {
        return None;
    }
    let stem = path.file_stem()?.to_str()?;
    let (base, version) = split_explicit_version_stem(stem)?;
    let base = base.trim();
    if base.is_empty() || split_explicit_version_stem(base).is_some() {
        return None;
    }
    let extension = path
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or("");
    let base_key = normalized_version_name_component(base);
    let extension_key = normalized_version_name_component(extension);
    if base_key.is_empty() || base_key.len() > 1_024 || extension_key.len() > 64 {
        return None;
    }
    Some(ExplicitFileVersionName {
        base_key,
        extension_key,
        version,
    })
}

fn split_explicit_version_stem(stem: &str) -> Option<(&str, u32)> {
    const MARKERS: [&str; 8] = ["-v", "-V", "_v", "_V", " v", " V", ".v", ".V"];
    let (marker_index, marker) = MARKERS
        .iter()
        .filter_map(|marker| stem.rfind(marker).map(|index| (index, *marker)))
        .max_by_key(|(index, _)| *index)?;
    let digits = &stem[marker_index + marker.len()..];
    if digits.is_empty()
        || digits.len() > 6
        || !digits.bytes().all(|byte| byte.is_ascii_digit())
        || (digits.len() > 1 && digits.starts_with('0'))
    {
        return None;
    }
    let version = digits.parse::<u32>().ok()?;
    if !(1..=999_999).contains(&version) {
        return None;
    }
    Some((&stem[..marker_index], version))
}

fn normalized_version_name_component(value: &str) -> String {
    let nfc = value.nfc().collect::<String>();
    nfc.to_lowercase().nfc().collect()
}

impl FileVersionEvidence {
    pub const PROVIDER_ID: &str = "deskgraph.filename-version";
    pub const PROVIDER_VERSION: &str = "1";
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct FileRelationCandidate {
    pub api_version: &'static str,
    pub relation_id: i64,
    pub kind: FileRelationKind,
    pub state: FileRelationCandidateState,
    pub left: FileRelationEndpoint,
    pub right: FileRelationEndpoint,
    pub evidence: FileRelationEvidence,
    pub latest_decision: Option<FileRelationDecision>,
}

impl FileRelationCandidate {
    pub const API_VERSION: &str = "deskgraph.file-relation-candidate.v1";
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct FileVersionCandidate {
    pub api_version: &'static str,
    pub relation_id: i64,
    pub kind: FileRelationKind,
    pub state: FileRelationCandidateState,
    pub older: FileRelationEndpoint,
    pub newer: FileRelationEndpoint,
    pub evidence: FileVersionEvidence,
    pub latest_decision: Option<FileVersionDecision>,
}

impl FileVersionCandidate {
    pub const API_VERSION: &str = "deskgraph.file-version-candidate.v2";
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct FileRelationCandidateSummary {
    pub api_version: &'static str,
    pub relation_id: i64,
    pub kind: FileRelationKind,
    pub state: FileRelationCandidateState,
    pub scope_id: i64,
    pub left_node_id: i64,
    pub right_node_id: i64,
    pub confidence_basis_points: u16,
    pub last_observed_at_unix_ms: i64,
    pub latest_decision_at_unix_ms: Option<i64>,
    pub verification_required: bool,
}

impl FileRelationCandidateSummary {
    pub const API_VERSION: &str = "deskgraph.file-relation-candidate-summary.v1";
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ScreenshotGroupCandidateState {
    Suggested,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ScreenshotGroupRuleKind {
    SameDimensionsTimeWindowWithOcr,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ScreenshotGroupCreator {
    SystemRule,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ScreenshotGroupMember {
    pub node_id: i64,
    pub location_id: i64,
    pub display_path: String,
    pub image_metadata_id: i64,
    pub ocr_extraction_job_id: i64,
    pub size_bytes: u64,
    pub modified_unix_ns: i64,
    pub format: ImageFormat,
    pub pixel_width: u32,
    pub pixel_height: u32,
    pub ocr_chunk_count: u32,
    pub ocr_provider_id: String,
    pub ocr_provider_version: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ScreenshotGroupEvidence {
    pub observation_id: i64,
    pub rule_kind: ScreenshotGroupRuleKind,
    pub confidence_basis_points: u16,
    pub observed_at_unix_ms: i64,
    pub created_by: ScreenshotGroupCreator,
    pub provider_id: &'static str,
    pub provider_version: &'static str,
    pub model_version: Option<String>,
    pub time_window_seconds: u32,
    pub review_assistance_only: bool,
    pub content_similarity_claimed: bool,
    pub cleanup_authorized: bool,
}

impl ScreenshotGroupEvidence {
    pub const PROVIDER_ID: &str = "deskgraph.screenshot-group-rules";
    pub const PROVIDER_VERSION: &str = "1";
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ScreenshotGroupCandidate {
    pub api_version: &'static str,
    pub group_id: i64,
    pub scope_id: i64,
    pub state: ScreenshotGroupCandidateState,
    pub members: Vec<ScreenshotGroupMember>,
    pub total_size_bytes: u64,
    pub members_independently_selectable: bool,
    pub evidence: ScreenshotGroupEvidence,
}

impl ScreenshotGroupCandidate {
    pub const API_VERSION: &str = "deskgraph.screenshot-group-candidate.v1";
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ScreenshotGroupCandidateSummary {
    pub api_version: &'static str,
    pub group_id: i64,
    pub scope_id: i64,
    pub state: ScreenshotGroupCandidateState,
    pub current_evidence: bool,
    pub member_count: u32,
    pub total_size_bytes: u64,
    pub confidence_basis_points: u16,
    pub last_observed_at_unix_ms: i64,
    pub verification_required: bool,
    pub cleanup_authorized: bool,
}

impl ScreenshotGroupCandidateSummary {
    pub const API_VERSION: &str = "deskgraph.screenshot-group-candidate-summary.v1";
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ScreenshotGroupDiscovery {
    pub api_version: &'static str,
    pub scope_id: i64,
    pub evaluated_image_count: u32,
    pub groups: Vec<ScreenshotGroupCandidate>,
    pub bounded_image_limit: u32,
    pub bounded_group_limit: u32,
    pub bounded_members_per_group: u32,
}

impl ScreenshotGroupDiscovery {
    pub const API_VERSION: &str = "deskgraph.screenshot-group-discovery.v1";
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SmartCleanupSourceKind {
    ExactDuplicate,
    Version,
    ScreenshotReviewGroup,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SmartCleanupCandidateState {
    Suggested,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct SmartCleanupInboxItem {
    pub source_kind: SmartCleanupSourceKind,
    pub source_id: i64,
    pub source_observation_id: i64,
    pub scope_id: i64,
    pub state: SmartCleanupCandidateState,
    pub member_count: u32,
    pub confidence_basis_points: u16,
    pub observed_at_unix_ms: i64,
    pub current_evidence: bool,
    pub verification_required: bool,
    pub review_assistance_only: bool,
    pub cleanup_authorized: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct SmartCleanupInbox {
    pub api_version: &'static str,
    pub scope_id: i64,
    pub items: Vec<SmartCleanupInboxItem>,
    pub evaluated_source_count: u32,
    pub not_current_source_count: u32,
    pub bounded_source_limit: u32,
    pub evaluation_complete: bool,
    pub action_authorized: bool,
}

impl SmartCleanupInbox {
    pub const API_VERSION: &str = "deskgraph.smart-cleanup-inbox.v1";
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct FolderProfile {
    pub api_version: &'static str,
    pub scope_id: i64,
    pub folder_node_id: i64,
    pub folder_location_id: i64,
    pub display_path: String,
    pub direct_file_count: u64,
    pub direct_folder_count: u64,
    pub descendant_file_count: u64,
    pub descendant_folder_count: u64,
    pub total_file_bytes: u64,
    pub latest_modified_unix_ns: Option<i64>,
    pub file_categories: Vec<FolderCategoryCount>,
    pub project_suggestion: Option<ProjectSuggestion>,
    pub observed_at_unix_ms: i64,
    pub bounded_entry_limit: u64,
}

impl FolderProfile {
    pub const API_VERSION: &str = "deskgraph.folder-profile.v1";
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn project_suggestion_is_explainable_and_model_free() {
        let suggestion = ProjectSuggestion {
            confidence_basis_points: 8_500,
            provenance: vec![ProjectSignal {
                kind: ProjectSignalKind::CargoManifest,
                marker_name: "Cargo.toml".to_string(),
                weight_basis_points: 8_500,
            }],
            observed_at_unix_ms: 1,
            created_by: ProjectSuggestionCreator::SystemRule,
            provider_id: ProjectSuggestion::PROVIDER_ID,
            provider_version: ProjectSuggestion::PROVIDER_VERSION,
            model_version: None,
        };
        let value = serde_json::to_value(&suggestion).expect("suggestion should serialize");
        assert_eq!(value["created_by"], "system_rule");
        assert_eq!(value["model_version"], serde_json::Value::Null);
        assert_eq!(value["provenance"][0]["kind"], "cargo_manifest");
    }

    #[test]
    fn project_candidate_summary_is_path_free_and_feedback_is_explicit() {
        let summary = ProjectCandidateSummary {
            api_version: ProjectCandidateSummary::API_VERSION,
            project_id: 1,
            scope_id: 2,
            root_folder_node_id: 3,
            state: ProjectCandidateState::Rejected,
            confidence_basis_points: 8_500,
            observed_at_unix_ms: 4,
            latest_decision_at_unix_ms: Some(5),
        };
        let value = serde_json::to_value(&summary).expect("summary should serialize");
        assert_eq!(value["state"], "rejected");
        assert!(value.get("display_path").is_none());
        assert!(value.get("suggestion").is_none());
    }

    #[test]
    fn exact_duplicate_candidate_is_explicit_explainable_and_model_free() {
        let endpoint = |node_id: i64, path: &str| FileRelationEndpoint {
            scope_id: 1,
            node_id,
            location_id: node_id + 10,
            display_path: path.to_string(),
            size_bytes: 4,
            modified_unix_ns: Some(5),
        };
        let candidate = FileRelationCandidate {
            api_version: FileRelationCandidate::API_VERSION,
            relation_id: 1,
            kind: FileRelationKind::ExactDuplicate,
            state: FileRelationCandidateState::Suggested,
            left: endpoint(2, "/scope/private-left.txt"),
            right: endpoint(3, "/scope/private-right.txt"),
            evidence: FileRelationEvidence {
                comparison_kind: FileRelationComparisonKind::ByteForByte,
                compared_bytes: 4,
                confidence_basis_points: 10_000,
                observed_at_unix_ms: 6,
                created_by: FileRelationCreator::SystemRule,
                provider_id: FileRelationEvidence::PROVIDER_ID,
                provider_version: FileRelationEvidence::PROVIDER_VERSION,
                model_version: None,
                bounded_max_bytes: 64 * 1024 * 1024,
            },
            latest_decision: None,
        };
        let value = serde_json::to_value(candidate).expect("candidate should serialize");
        assert_eq!(value["kind"], "exact_duplicate");
        assert_eq!(value["state"], "suggested");
        assert_eq!(value["evidence"]["comparison_kind"], "byte_for_byte");
        assert_eq!(value["evidence"]["confidence_basis_points"], 10_000);
        assert_eq!(value["evidence"]["created_by"], "system_rule");
        assert_eq!(value["evidence"]["model_version"], serde_json::Value::Null);
        assert_eq!(value["latest_decision"], serde_json::Value::Null);
    }

    #[test]
    fn file_relation_history_summary_is_path_free_and_requires_verification() {
        let summary = FileRelationCandidateSummary {
            api_version: FileRelationCandidateSummary::API_VERSION,
            relation_id: 1,
            kind: FileRelationKind::ExactDuplicate,
            state: FileRelationCandidateState::Rejected,
            scope_id: 2,
            left_node_id: 3,
            right_node_id: 4,
            confidence_basis_points: 10_000,
            last_observed_at_unix_ms: 5,
            latest_decision_at_unix_ms: Some(6),
            verification_required: true,
        };
        let value = serde_json::to_value(summary).expect("summary should serialize");
        assert_eq!(value["state"], "rejected");
        assert_eq!(value["verification_required"], true);
        assert!(value.get("display_path").is_none());
        assert!(value.get("left").is_none());
        assert!(value.get("right").is_none());
    }

    #[test]
    fn file_version_candidate_is_directional_explainable_and_model_free() {
        let endpoint = |node_id: i64, path: &str| FileRelationEndpoint {
            scope_id: 1,
            node_id,
            location_id: node_id + 10,
            display_path: path.to_string(),
            size_bytes: 4,
            modified_unix_ns: Some(5),
        };
        let candidate = FileVersionCandidate {
            api_version: FileVersionCandidate::API_VERSION,
            relation_id: 1,
            kind: FileRelationKind::Version,
            state: FileRelationCandidateState::Suggested,
            older: endpoint(2, "/scope/企劃-v1.md"),
            newer: endpoint(3, "/scope/企劃-v2.md"),
            evidence: FileVersionEvidence {
                signal_kind: FileVersionSignalKind::ExplicitNumericSuffix,
                base_key: "企劃".to_string(),
                extension_key: "md".to_string(),
                older_version: 1,
                newer_version: 2,
                confidence_basis_points: 9_000,
                observed_at_unix_ms: 6,
                created_by: FileRelationCreator::SystemRule,
                provider_id: FileVersionEvidence::PROVIDER_ID,
                provider_version: FileVersionEvidence::PROVIDER_VERSION,
                model_version: None,
            },
            latest_decision: None,
        };
        let value = serde_json::to_value(candidate).expect("candidate should serialize");
        assert_eq!(value["kind"], "version");
        assert_eq!(value["state"], "suggested");
        assert_eq!(value["evidence"]["signal_kind"], "explicit_numeric_suffix");
        assert_eq!(value["evidence"]["confidence_basis_points"], 9_000);
        assert_eq!(value["evidence"]["created_by"], "system_rule");
        assert_eq!(value["evidence"]["model_version"], serde_json::Value::Null);
        assert_eq!(value["latest_decision"], serde_json::Value::Null);
    }

    #[test]
    fn explicit_file_version_names_are_conservative_and_unicode_normalized() {
        let first =
            parse_explicit_file_version_name("企劃-v1.MD").expect("explicit version should parse");
        let second =
            parse_explicit_file_version_name("企劃_V2.md").expect("uppercase marker should parse");
        assert_eq!(first.base_key, second.base_key);
        assert_eq!(first.extension_key, "md");
        assert_eq!(first.version, 1);
        assert_eq!(second.version, 2);
        assert!(parse_explicit_file_version_name("企劃-final.md").is_none());
        assert!(parse_explicit_file_version_name("企劃-v01.md").is_none());
        assert!(parse_explicit_file_version_name("企劃-v1-v2.md").is_none());
        assert!(parse_explicit_file_version_name("folder/企劃-v2.md").is_none());
    }

    #[test]
    fn screenshot_group_contract_is_review_only_and_summary_is_path_free() {
        let candidate = ScreenshotGroupCandidate {
            api_version: ScreenshotGroupCandidate::API_VERSION,
            group_id: 1,
            scope_id: 2,
            state: ScreenshotGroupCandidateState::Suggested,
            members: vec![ScreenshotGroupMember {
                node_id: 3,
                location_id: 4,
                display_path: "/private/screenshot.png".to_string(),
                image_metadata_id: 5,
                ocr_extraction_job_id: 6,
                size_bytes: 7,
                modified_unix_ns: 8,
                format: ImageFormat::Png,
                pixel_width: 1440,
                pixel_height: 900,
                ocr_chunk_count: 1,
                ocr_provider_id: "local-ocr".to_string(),
                ocr_provider_version: "1".to_string(),
            }],
            total_size_bytes: 7,
            members_independently_selectable: true,
            evidence: ScreenshotGroupEvidence {
                observation_id: 9,
                rule_kind: ScreenshotGroupRuleKind::SameDimensionsTimeWindowWithOcr,
                confidence_basis_points: 6_000,
                observed_at_unix_ms: 10,
                created_by: ScreenshotGroupCreator::SystemRule,
                provider_id: ScreenshotGroupEvidence::PROVIDER_ID,
                provider_version: ScreenshotGroupEvidence::PROVIDER_VERSION,
                model_version: None,
                time_window_seconds: 600,
                review_assistance_only: true,
                content_similarity_claimed: false,
                cleanup_authorized: false,
            },
        };
        let value = serde_json::to_value(candidate).expect("candidate should serialize");
        assert_eq!(value["evidence"]["review_assistance_only"], true);
        assert_eq!(value["evidence"]["content_similarity_claimed"], false);
        assert_eq!(value["evidence"]["cleanup_authorized"], false);

        let summary = ScreenshotGroupCandidateSummary {
            api_version: ScreenshotGroupCandidateSummary::API_VERSION,
            group_id: 1,
            scope_id: 2,
            state: ScreenshotGroupCandidateState::Suggested,
            current_evidence: false,
            member_count: 2,
            total_size_bytes: 14,
            confidence_basis_points: 6_000,
            last_observed_at_unix_ms: 10,
            verification_required: true,
            cleanup_authorized: false,
        };
        let value = serde_json::to_value(summary).expect("summary should serialize");
        assert!(value.get("display_path").is_none());
        assert!(value.get("members").is_none());
        assert_eq!(value["verification_required"], true);
        assert_eq!(value["cleanup_authorized"], false);
    }

    #[test]
    fn smart_cleanup_inbox_contract_is_path_free_and_cannot_authorize_actions() {
        let inbox = SmartCleanupInbox {
            api_version: SmartCleanupInbox::API_VERSION,
            scope_id: 2,
            items: vec![SmartCleanupInboxItem {
                source_kind: SmartCleanupSourceKind::ExactDuplicate,
                source_id: 3,
                source_observation_id: 4,
                scope_id: 2,
                state: SmartCleanupCandidateState::Suggested,
                member_count: 2,
                confidence_basis_points: 10_000,
                observed_at_unix_ms: 5,
                current_evidence: true,
                verification_required: true,
                review_assistance_only: true,
                cleanup_authorized: false,
            }],
            evaluated_source_count: 1,
            not_current_source_count: 0,
            bounded_source_limit: 20,
            evaluation_complete: true,
            action_authorized: false,
        };
        let value = serde_json::to_value(inbox).expect("Inbox should serialize");
        assert_eq!(value["api_version"], SmartCleanupInbox::API_VERSION);
        assert_eq!(value["items"][0]["source_kind"], "exact_duplicate");
        assert_eq!(value["items"][0]["cleanup_authorized"], false);
        assert_eq!(value["action_authorized"], false);
        for forbidden in [
            "display_path",
            "path_raw",
            "path_key",
            "file_name",
            "ocr_text",
            "base_key",
            "extension_key",
            "evidence_key",
            "reclaimable_bytes",
            "keeper",
            "selection",
        ] {
            assert!(!value.to_string().contains(forbidden));
        }
    }
}
