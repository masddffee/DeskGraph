use std::fmt;

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SearchMode {
    Lexical,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SearchMatchedField {
    MetadataPath,
    ExtractedText,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SearchSourceFilter {
    All,
    MetadataPath,
    ExtractedText,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct SearchFilters {
    pub scope_id: Option<i64>,
    pub folder_node_id: Option<i64>,
    pub source: SearchSourceFilter,
    pub extension: Option<String>,
    pub modified_since_unix_seconds: Option<i64>,
    pub modified_before_unix_seconds: Option<i64>,
}

#[derive(Clone, Eq, PartialEq, Serialize, Deserialize)]
pub struct SearchFolderOption {
    pub scope_id: i64,
    pub folder_node_id: i64,
    pub display_path: String,
}

impl fmt::Debug for SearchFolderOption {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("SearchFolderOption")
            .field("scope_id", &self.scope_id)
            .field("folder_node_id", &self.folder_node_id)
            .field("display_path", &"<redacted>")
            .finish()
    }
}

#[derive(Clone, Eq, PartialEq, Serialize, Deserialize)]
pub struct SearchFolderListResponse {
    pub api_version: &'static str,
    pub scope_id: i64,
    pub folder_count: u64,
    pub folders: Vec<SearchFolderOption>,
    pub truncated: bool,
}

impl SearchFolderListResponse {
    pub const API_VERSION: &str = "deskgraph.search-folders.v1";
}

impl fmt::Debug for SearchFolderListResponse {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("SearchFolderListResponse")
            .field("api_version", &self.api_version)
            .field("scope_id", &self.scope_id)
            .field("folder_count", &self.folder_count)
            .field("folders", &self.folders)
            .field("truncated", &self.truncated)
            .finish()
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct SearchResult {
    pub scope_id: i64,
    pub node_id: i64,
    pub location_id: i64,
    pub display_path: String,
    pub snippet: Option<String>,
    pub matched_fields: Vec<SearchMatchedField>,
    pub explanation: String,
    pub lexical_rank: u32,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct SearchResponse {
    pub api_version: &'static str,
    pub mode: SearchMode,
    pub embeddings_enabled: bool,
    pub query: String,
    pub filters: SearchFilters,
    pub result_count: u64,
    pub results: Vec<SearchResult>,
    pub elapsed_ms: u64,
}

impl SearchResponse {
    pub const API_VERSION: &str = "deskgraph.search.v1";
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn search_contract_serializes_fixed_diagnostics() {
        let response = SearchResponse {
            api_version: SearchResponse::API_VERSION,
            mode: SearchMode::Lexical,
            embeddings_enabled: false,
            query: "專案 context".to_string(),
            filters: SearchFilters {
                scope_id: Some(1),
                folder_node_id: Some(4),
                source: SearchSourceFilter::All,
                extension: Some("md".to_string()),
                modified_since_unix_seconds: None,
                modified_before_unix_seconds: None,
            },
            result_count: 1,
            results: vec![SearchResult {
                scope_id: 1,
                node_id: 2,
                location_id: 3,
                display_path: "/authorized/專案-context.md".to_string(),
                snippet: Some("專案 [context]".to_string()),
                matched_fields: vec![
                    SearchMatchedField::MetadataPath,
                    SearchMatchedField::ExtractedText,
                ],
                explanation: "path_and_extracted_text_substring".to_string(),
                lexical_rank: 1,
            }],
            elapsed_ms: 2,
        };

        let value = serde_json::to_value(response).expect("search response should serialize");
        assert_eq!(value["api_version"], "deskgraph.search.v1");
        assert_eq!(value["mode"], "lexical");
        assert_eq!(value["embeddings_enabled"], false);
        assert_eq!(value["filters"]["source"], "all");
        assert_eq!(value["filters"]["folder_node_id"], 4);
        assert_eq!(value["filters"]["extension"], "md");
        assert_eq!(value["results"][0]["matched_fields"][1], "extracted_text");
    }

    #[test]
    fn folder_list_serializes_paths_but_redacts_them_from_debug_output() {
        let response = SearchFolderListResponse {
            api_version: SearchFolderListResponse::API_VERSION,
            scope_id: 7,
            folder_count: 1,
            folders: vec![SearchFolderOption {
                scope_id: 7,
                folder_node_id: 11,
                display_path: "/authorized/private-project".to_string(),
            }],
            truncated: false,
        };

        let value = serde_json::to_value(&response).expect("folder list should serialize");
        assert_eq!(value["api_version"], "deskgraph.search-folders.v1");
        assert_eq!(value["scope_id"], 7);
        assert_eq!(value["folder_count"], 1);
        assert_eq!(value["folders"][0]["folder_node_id"], 11);
        assert_eq!(
            value["folders"][0]["display_path"],
            "/authorized/private-project"
        );

        let debug = format!("{response:?}");
        assert!(!debug.contains("private-project"));
        assert!(debug.contains("<redacted>"));
    }
}
