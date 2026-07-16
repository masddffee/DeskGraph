use serde::{Deserialize, Serialize};

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
}
