use std::collections::HashMap;
use std::fmt;
use std::path::Path;
use std::time::Instant;

use deskgraph_database::{
    DatabaseError, LexicalCandidateSource, LexicalSearchCandidate, LexicalSearchFilters,
    LexicalSearchSource, ManifestDatabase,
};
pub use deskgraph_domain::SearchSourceFilter;
use deskgraph_domain::{
    SearchFilters, SearchMatchedField, SearchMode, SearchResponse, SearchResult,
};

const MIN_QUERY_CHARS: usize = 3;
const MAX_QUERY_CHARS: usize = 256;
const DEFAULT_RESULT_LIMIT: u32 = 20;
const MAX_RESULT_LIMIT: u32 = 50;
const MAX_CANDIDATES_PER_SOURCE: u32 = 100;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SearchRequest<'a> {
    pub query: &'a str,
    pub scope_id: Option<i64>,
    pub source: SearchSourceFilter,
    pub extension: Option<&'a str>,
    pub modified_since_unix_seconds: Option<i64>,
    pub modified_before_unix_seconds: Option<i64>,
    pub limit: Option<u32>,
}

#[derive(Debug)]
pub enum SearchError {
    QueryEmpty,
    QueryTooShort,
    QueryTooLong,
    QueryInvalid,
    ScopeInvalid,
    ExtensionInvalid,
    ModifiedRangeInvalid,
    LimitOutOfRange,
    Database(DatabaseError),
}

impl SearchError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::QueryEmpty => "search_query_empty",
            Self::QueryTooShort => "search_query_too_short",
            Self::QueryTooLong => "search_query_too_long",
            Self::QueryInvalid => "search_query_invalid",
            Self::ScopeInvalid => "search_scope_invalid",
            Self::ExtensionInvalid => "search_extension_invalid",
            Self::ModifiedRangeInvalid => "search_modified_range_invalid",
            Self::LimitOutOfRange => "search_limit_out_of_range",
            Self::Database(error) => error.code(),
        }
    }
}

impl fmt::Display for SearchError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.code())
    }
}

impl std::error::Error for SearchError {}

impl From<DatabaseError> for SearchError {
    fn from(error: DatabaseError) -> Self {
        Self::Database(error)
    }
}

#[derive(Debug)]
struct CombinedCandidate {
    scope_id: i64,
    node_id: i64,
    location_id: i64,
    display_path: String,
    snippet: Option<String>,
    metadata_rank: Option<usize>,
    content_rank: Option<usize>,
    exact_filename: bool,
}

#[derive(Debug)]
struct NormalizedFilters {
    scope_id: Option<i64>,
    source: SearchSourceFilter,
    extension: Option<String>,
    modified_since_unix_seconds: Option<i64>,
    modified_before_unix_seconds: Option<i64>,
    modified_since_unix_ns: Option<i64>,
    modified_before_unix_ns: Option<i64>,
}

impl NormalizedFilters {
    fn database_filters(&self) -> LexicalSearchFilters<'_> {
        LexicalSearchFilters {
            scope_id: self.scope_id,
            source: match self.source {
                SearchSourceFilter::All => LexicalSearchSource::All,
                SearchSourceFilter::MetadataPath => LexicalSearchSource::MetadataPath,
                SearchSourceFilter::ExtractedText => LexicalSearchSource::ExtractedText,
            },
            extension: self.extension.as_deref(),
            modified_since_unix_ns: self.modified_since_unix_ns,
            modified_before_unix_ns: self.modified_before_unix_ns,
        }
    }

    fn into_applied(self) -> SearchFilters {
        SearchFilters {
            scope_id: self.scope_id,
            source: self.source,
            extension: self.extension,
            modified_since_unix_seconds: self.modified_since_unix_seconds,
            modified_before_unix_seconds: self.modified_before_unix_seconds,
        }
    }
}

pub fn search_at(path: &Path, request: SearchRequest<'_>) -> Result<SearchResponse, SearchError> {
    let database = ManifestDatabase::open(path)?;
    search(&database, request)
}

pub fn search(
    database: &ManifestDatabase,
    request: SearchRequest<'_>,
) -> Result<SearchResponse, SearchError> {
    let started = Instant::now();
    let normalized_query = normalize_query(request.query)?;
    let filters = normalize_filters(&request)?;
    let limit = request.limit.unwrap_or(DEFAULT_RESULT_LIMIT);
    if limit == 0 || limit > MAX_RESULT_LIMIT {
        return Err(SearchError::LimitOutOfRange);
    }
    let per_source_candidate_limit = limit.saturating_mul(2).min(MAX_CANDIDATES_PER_SOURCE);
    let match_query = quote_fts_phrase(&normalized_query);
    let candidates = database.lexical_search_candidates(
        &match_query,
        filters.database_filters(),
        per_source_candidate_limit,
    )?;
    let results = rank_candidates(&normalized_query, candidates, limit);
    let result_count = u64::try_from(results.len()).map_err(|_| SearchError::LimitOutOfRange)?;
    let elapsed_ms = u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX);

    Ok(SearchResponse {
        api_version: SearchResponse::API_VERSION,
        mode: SearchMode::Lexical,
        embeddings_enabled: false,
        query: normalized_query,
        filters: filters.into_applied(),
        result_count,
        results,
        elapsed_ms,
    })
}

fn normalize_filters(request: &SearchRequest<'_>) -> Result<NormalizedFilters, SearchError> {
    if request.scope_id.is_some_and(|scope_id| scope_id <= 0) {
        return Err(SearchError::ScopeInvalid);
    }
    let extension = request
        .extension
        .map(str::trim)
        .map(|extension| extension.strip_prefix('.').unwrap_or(extension))
        .map(str::to_ascii_lowercase)
        .map(|extension| {
            if extension.is_empty()
                || extension.len() > 16
                || !extension
                    .bytes()
                    .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit())
            {
                Err(SearchError::ExtensionInvalid)
            } else {
                Ok(extension)
            }
        })
        .transpose()?;
    let modified_since_unix_ns = seconds_to_nanoseconds(request.modified_since_unix_seconds)?;
    let modified_before_unix_ns = seconds_to_nanoseconds(request.modified_before_unix_seconds)?;
    if matches!(
        (
            request.modified_since_unix_seconds,
            request.modified_before_unix_seconds
        ),
        (Some(since), Some(before)) if since >= before
    ) {
        return Err(SearchError::ModifiedRangeInvalid);
    }
    Ok(NormalizedFilters {
        scope_id: request.scope_id,
        source: request.source,
        extension,
        modified_since_unix_seconds: request.modified_since_unix_seconds,
        modified_before_unix_seconds: request.modified_before_unix_seconds,
        modified_since_unix_ns,
        modified_before_unix_ns,
    })
}

fn seconds_to_nanoseconds(seconds: Option<i64>) -> Result<Option<i64>, SearchError> {
    seconds
        .map(|seconds| {
            if seconds < 0 {
                return Err(SearchError::ModifiedRangeInvalid);
            }
            seconds
                .checked_mul(1_000_000_000)
                .ok_or(SearchError::ModifiedRangeInvalid)
        })
        .transpose()
}

fn normalize_query(query: &str) -> Result<String, SearchError> {
    let normalized = query.split_whitespace().collect::<Vec<_>>().join(" ");
    let character_count = normalized.chars().count();
    if character_count == 0 {
        return Err(SearchError::QueryEmpty);
    }
    if character_count < MIN_QUERY_CHARS {
        return Err(SearchError::QueryTooShort);
    }
    if character_count > MAX_QUERY_CHARS {
        return Err(SearchError::QueryTooLong);
    }
    if normalized.chars().any(char::is_control) {
        return Err(SearchError::QueryInvalid);
    }
    Ok(normalized)
}

fn quote_fts_phrase(query: &str) -> String {
    format!("\"{}\"", query.replace('"', "\"\""))
}

fn rank_candidates(
    query: &str,
    candidates: Vec<LexicalSearchCandidate>,
    limit: u32,
) -> Vec<SearchResult> {
    let mut combined: HashMap<(i64, i64), CombinedCandidate> = HashMap::new();
    let mut metadata_rank = 0_usize;
    let mut content_rank = 0_usize;
    for candidate in candidates {
        let source_rank = match candidate.source {
            LexicalCandidateSource::MetadataPath => {
                metadata_rank = metadata_rank.saturating_add(1);
                metadata_rank
            }
            LexicalCandidateSource::ExtractedText => {
                content_rank = content_rank.saturating_add(1);
                content_rank
            }
        };
        let key = (candidate.node_id, candidate.location_id);
        let entry = combined.entry(key).or_insert_with(|| CombinedCandidate {
            scope_id: candidate.scope_id,
            node_id: candidate.node_id,
            location_id: candidate.location_id,
            exact_filename: filename(&candidate.display_path).to_lowercase()
                == query.to_lowercase(),
            display_path: candidate.display_path.clone(),
            snippet: None,
            metadata_rank: None,
            content_rank: None,
        });
        match candidate.source {
            LexicalCandidateSource::MetadataPath => {
                entry.metadata_rank = Some(
                    entry
                        .metadata_rank
                        .map_or(source_rank, |rank| rank.min(source_rank)),
                );
            }
            LexicalCandidateSource::ExtractedText => {
                entry.content_rank = Some(
                    entry
                        .content_rank
                        .map_or(source_rank, |rank| rank.min(source_rank)),
                );
                if entry.snippet.is_none() {
                    entry.snippet = candidate.snippet;
                }
            }
        }
    }

    let mut combined = combined.into_values().collect::<Vec<_>>();
    combined.sort_by_key(|candidate| {
        (
            !candidate.exact_filename,
            !(candidate.metadata_rank.is_some() && candidate.content_rank.is_some()),
            candidate.metadata_rank.unwrap_or(usize::MAX),
            candidate.content_rank.unwrap_or(usize::MAX),
            candidate.node_id,
            candidate.location_id,
        )
    });
    combined
        .into_iter()
        .take(usize::try_from(limit).unwrap_or(usize::MAX))
        .enumerate()
        .map(|(index, candidate)| {
            let matched_fields = match (candidate.metadata_rank, candidate.content_rank) {
                (Some(_), Some(_)) => vec![
                    SearchMatchedField::MetadataPath,
                    SearchMatchedField::ExtractedText,
                ],
                (Some(_), None) => vec![SearchMatchedField::MetadataPath],
                (None, Some(_)) => vec![SearchMatchedField::ExtractedText],
                (None, None) => Vec::new(),
            };
            let explanation = match (
                candidate.exact_filename,
                candidate.metadata_rank.is_some(),
                candidate.content_rank.is_some(),
            ) {
                (true, _, true) => "exact_filename_and_extracted_text",
                (true, _, false) => "exact_filename",
                (false, true, true) => "path_and_extracted_text_substring",
                (false, true, false) => "path_substring",
                (false, false, true) => "extracted_text_substring",
                (false, false, false) => "no_match",
            };
            SearchResult {
                scope_id: candidate.scope_id,
                node_id: candidate.node_id,
                location_id: candidate.location_id,
                display_path: candidate.display_path,
                snippet: candidate.snippet,
                matched_fields,
                explanation: explanation.to_string(),
                lexical_rank: u32::try_from(index.saturating_add(1)).unwrap_or(u32::MAX),
            }
        })
        .collect()
}

fn filename(path: &str) -> &str {
    path.rsplit(['/', '\\']).next().unwrap_or(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn candidate(
        source: LexicalCandidateSource,
        node_id: i64,
        path: &str,
        snippet: Option<&str>,
    ) -> LexicalSearchCandidate {
        LexicalSearchCandidate {
            source,
            scope_id: 1,
            node_id,
            location_id: node_id,
            display_path: path.to_string(),
            snippet: snippet.map(str::to_string),
        }
    }

    #[test]
    fn query_validation_is_bounded_and_fts_syntax_is_quoted() {
        assert!(matches!(
            normalize_query("  "),
            Err(SearchError::QueryEmpty)
        ));
        assert!(matches!(
            normalize_query("AI"),
            Err(SearchError::QueryTooShort)
        ));
        assert!(matches!(
            normalize_query(&"a".repeat(257)),
            Err(SearchError::QueryTooLong)
        ));
        assert!(matches!(
            normalize_query("abc\0def"),
            Err(SearchError::QueryInvalid)
        ));
        assert_eq!(
            normalize_query("  專案   context ").unwrap(),
            "專案 context"
        );
        assert_eq!(quote_fts_phrase("a\" OR b"), "\"a\"\" OR b\"");
    }

    #[test]
    fn filters_normalize_extension_and_reject_invalid_ranges() {
        let filters = normalize_filters(&SearchRequest {
            query: "context",
            scope_id: Some(1),
            source: SearchSourceFilter::ExtractedText,
            extension: Some(" .MD "),
            modified_since_unix_seconds: Some(1),
            modified_before_unix_seconds: Some(2),
            limit: None,
        })
        .expect("bounded filters should normalize");
        assert_eq!(filters.extension.as_deref(), Some("md"));
        assert_eq!(filters.modified_since_unix_ns, Some(1_000_000_000));
        assert_eq!(filters.modified_before_unix_ns, Some(2_000_000_000));

        for request in [
            SearchRequest {
                query: "context",
                scope_id: Some(0),
                source: SearchSourceFilter::All,
                extension: None,
                modified_since_unix_seconds: None,
                modified_before_unix_seconds: None,
                limit: None,
            },
            SearchRequest {
                query: "context",
                scope_id: None,
                source: SearchSourceFilter::All,
                extension: Some("m_d"),
                modified_since_unix_seconds: None,
                modified_before_unix_seconds: None,
                limit: None,
            },
            SearchRequest {
                query: "context",
                scope_id: None,
                source: SearchSourceFilter::All,
                extension: None,
                modified_since_unix_seconds: Some(2),
                modified_before_unix_seconds: Some(2),
                limit: None,
            },
        ] {
            assert!(normalize_filters(&request).is_err());
        }
    }

    #[test]
    fn ranking_fuses_sources_and_explains_exact_filename() {
        let results = rank_candidates(
            "project.md",
            vec![
                candidate(
                    LexicalCandidateSource::MetadataPath,
                    2,
                    "/scope/project-notes.md",
                    None,
                ),
                candidate(
                    LexicalCandidateSource::MetadataPath,
                    1,
                    "/scope/project.md",
                    None,
                ),
                candidate(
                    LexicalCandidateSource::ExtractedText,
                    1,
                    "/scope/project.md",
                    Some("[project.md] context"),
                ),
            ],
            20,
        );

        assert_eq!(results[0].node_id, 1);
        assert_eq!(results[0].explanation, "exact_filename_and_extracted_text");
        assert_eq!(
            results[0].matched_fields,
            vec![
                SearchMatchedField::MetadataPath,
                SearchMatchedField::ExtractedText
            ]
        );
        assert_eq!(results[0].lexical_rank, 1);
    }
}
