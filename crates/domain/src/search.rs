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
        assert_eq!(value["results"][0]["matched_fields"][1], "extracted_text");
    }
}
