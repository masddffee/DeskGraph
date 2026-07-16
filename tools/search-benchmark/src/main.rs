use std::fs;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::time::Instant;

use clap::Parser;
use deskgraph_database::ManifestDatabase;
use deskgraph_retrieval::{SearchRequest, SearchSourceFilter, search};
use rusqlite::{Connection, OptionalExtension, params};
use serde::Serialize;

const DEFAULT_DOCUMENTS: u32 = 10_000;
const MAX_DOCUMENTS: u32 = 100_000;
const DEFAULT_ITERATIONS: u32 = 50;
const MAX_ITERATIONS: u32 = 1_000;

#[derive(Debug, Parser)]
#[command(
    name = "deskgraph-search-benchmark",
    about = "Generate and measure a bounded synthetic DeskGraph FTS corpus"
)]
struct Args {
    /// New SQLite path to create. Existing paths are never overwritten.
    #[arg(long)]
    database: PathBuf,
    #[arg(long, default_value_t = DEFAULT_DOCUMENTS)]
    documents: u32,
    #[arg(long, default_value_t = DEFAULT_ITERATIONS)]
    iterations: u32,
}

#[derive(Debug, Serialize)]
struct QueryReport {
    case: &'static str,
    result_count: u64,
    iterations: u32,
    p50_us: u64,
    p95_us: u64,
    max_us: u64,
}

#[derive(Debug, Serialize)]
struct BenchmarkReport {
    api_version: &'static str,
    corpus: &'static str,
    documents: u32,
    content_bytes: u64,
    fixture_elapsed_ms: u64,
    database_bytes: u64,
    fts_index_bytes: Option<u64>,
    query_reports: Vec<QueryReport>,
}

fn main() -> ExitCode {
    match run(Args::parse()) {
        Ok(report) => match serde_json::to_string_pretty(&report) {
            Ok(json) => {
                println!("{json}");
                ExitCode::SUCCESS
            }
            Err(_) => fail("benchmark_serialization_failed"),
        },
        Err(code) => fail(code),
    }
}

fn fail(code: &'static str) -> ExitCode {
    eprintln!("Benchmark failed: {code}");
    ExitCode::FAILURE
}

fn run(args: Args) -> Result<BenchmarkReport, &'static str> {
    validate_counts(args.documents, args.iterations)?;
    if args
        .database
        .try_exists()
        .map_err(|_| "benchmark_database_check_failed")?
    {
        return Err("benchmark_database_already_exists");
    }
    if let Some(parent) = args
        .database
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        fs::create_dir_all(parent).map_err(|_| "benchmark_parent_create_failed")?;
    }

    ManifestDatabase::open(&args.database).map_err(|_| "benchmark_database_initialize_failed")?;
    let fixture_started = Instant::now();
    let content_bytes = populate_fixture(&args.database, args.documents)?;
    let fixture_elapsed_ms = elapsed_ms(fixture_started);

    let database =
        ManifestDatabase::open(&args.database).map_err(|_| "benchmark_database_reopen_failed")?;
    let query_cases = [
        ("traditional_chinese_content", "專案脈絡", true),
        ("english_content", "English context", true),
        ("exact_filename", "專案-context-0000042.md", true),
        ("missing_term", "definitely-absent-term", false),
    ];
    let mut query_reports = Vec::with_capacity(query_cases.len());
    for (case, query, should_match) in query_cases {
        search(
            &database,
            SearchRequest {
                query,
                scope_id: Some(1),
                source: SearchSourceFilter::All,
                extension: None,
                modified_since_unix_seconds: None,
                modified_before_unix_seconds: None,
                limit: Some(20),
            },
        )
        .map_err(|_| "benchmark_warmup_failed")?;
        let mut samples = Vec::with_capacity(
            usize::try_from(args.iterations)
                .map_err(|_| "benchmark_iteration_count_out_of_range")?,
        );
        let mut result_count = 0;
        for _ in 0..args.iterations {
            let started = Instant::now();
            let response = search(
                &database,
                SearchRequest {
                    query,
                    scope_id: Some(1),
                    source: SearchSourceFilter::All,
                    extension: None,
                    modified_since_unix_seconds: None,
                    modified_before_unix_seconds: None,
                    limit: Some(20),
                },
            )
            .map_err(|_| "benchmark_query_failed")?;
            samples.push(elapsed_us(started));
            result_count = response.result_count;
        }
        if should_match != (result_count > 0) {
            return Err("benchmark_result_contract_failed");
        }
        samples.sort_unstable();
        query_reports.push(QueryReport {
            case,
            result_count,
            iterations: args.iterations,
            p50_us: percentile(&samples, 50),
            p95_us: percentile(&samples, 95),
            max_us: samples.last().copied().unwrap_or(0),
        });
    }
    drop(database);

    checkpoint(&args.database)?;
    let database_bytes = file_bytes(&args.database)?;
    let fts_index_bytes = fts_index_bytes(&args.database)?;
    Ok(BenchmarkReport {
        api_version: "deskgraph.search-benchmark.v1",
        corpus: "synthetic_traditional_chinese_english_v1",
        documents: args.documents,
        content_bytes,
        fixture_elapsed_ms,
        database_bytes,
        fts_index_bytes,
        query_reports,
    })
}

fn populate_fixture(path: &Path, documents: u32) -> Result<u64, &'static str> {
    let mut connection = Connection::open(path).map_err(|_| "benchmark_fixture_open_failed")?;
    connection
        .execute_batch("PRAGMA foreign_keys = ON; PRAGMA synchronous = NORMAL;")
        .map_err(|_| "benchmark_fixture_pragma_failed")?;
    let transaction = connection
        .transaction()
        .map_err(|_| "benchmark_fixture_transaction_failed")?;
    transaction
        .execute(
            "INSERT INTO authorized_scopes(id, path_raw, path_key, display_path, platform, created_at_unix_ms) \
             VALUES (1, X'2F62656E63686D61726B', '/benchmark', '/benchmark', 'synthetic', 0)",
            [],
        )
        .map_err(|_| "benchmark_fixture_scope_failed")?;
    transaction
        .execute(
            "INSERT INTO scan_jobs(id, scope_id, status, discovered_files, started_at_unix_ms, finished_at_unix_ms) \
             VALUES (1, 1, 'completed', ?1, 0, 0)",
            [i64::from(documents)],
        )
        .map_err(|_| "benchmark_fixture_scan_failed")?;

    let mut content_bytes = 0_u64;
    for index in 0..documents {
        let id = i64::from(index) + 1;
        let display_path = format!(
            "/benchmark/group-{:03}/專案-context-{index:07}.md",
            index % 100
        );
        let text = format!(
            "DeskGraph synthetic document {index:07}. Traditional Chinese 專案脈絡 and English context remain local. Bucket {:03} roadmap notes.",
            index % 100
        );
        let text_bytes =
            u64::try_from(text.len()).map_err(|_| "benchmark_content_size_out_of_range")?;
        content_bytes = content_bytes
            .checked_add(text_bytes)
            .ok_or("benchmark_content_size_out_of_range")?;
        let identity_key = index.to_le_bytes();
        transaction
            .execute(
                "INSERT INTO nodes(id, kind, identity_kind, identity_key, created_at_unix_ms, updated_at_unix_ms) \
                 VALUES (?1, 'file', 'synthetic', ?2, 0, 0)",
                params![id, identity_key.as_slice()],
            )
            .map_err(|_| "benchmark_fixture_node_failed")?;
        transaction
            .execute(
                "INSERT INTO files(node_id, size_bytes, modified_unix_ns, link_count) \
                 VALUES (?1, ?2, ?1, 1)",
                params![
                    id,
                    i64::try_from(text_bytes).map_err(|_| "benchmark_content_size_out_of_range")?
                ],
            )
            .map_err(|_| "benchmark_fixture_file_failed")?;
        transaction
            .execute(
                "INSERT INTO locations(id, scope_id, node_id, path_raw, path_key, display_path, present, last_seen_scan_id) \
                 VALUES (?1, 1, ?1, ?2, ?3, ?3, 1, 1)",
                params![id, display_path.as_bytes(), &display_path],
            )
            .map_err(|_| "benchmark_fixture_location_failed")?;
        transaction
            .execute(
                "INSERT INTO extraction_jobs( \
                    id, scope_id, node_id, location_id, status, provider_id, provider_version, \
                    source_size_bytes, source_modified_unix_ns, output_bytes, chunk_count, \
                    created_at_unix_ms, finished_at_unix_ms, updated_at_unix_ms \
                 ) VALUES (?1, 1, ?1, ?1, 'completed', 'deskgraph.synthetic-benchmark', '1', \
                    ?2, ?1, ?2, 1, 0, 0, 0)",
                params![
                    id,
                    i64::try_from(text_bytes).map_err(|_| "benchmark_content_size_out_of_range")?
                ],
            )
            .map_err(|_| "benchmark_fixture_job_failed")?;
        transaction
            .execute(
                "INSERT INTO content_chunks( \
                    id, scope_id, node_id, location_id, extraction_job_id, ordinal, text, \
                    provenance_kind, source_byte_start, source_byte_end, source_page_number, \
                    source_fragment_index, source_size_bytes, source_modified_unix_ns, trust_class, \
                    provider_id, provider_version, active, created_at_unix_ms \
                 ) VALUES (?1, 1, ?1, ?1, ?1, 0, ?2, 'byte_range', 0, ?3, NULL, NULL, ?3, ?1, \
                    'untrusted_extracted_text', 'deskgraph.synthetic-benchmark', '1', 1, 0)",
                params![
                    id,
                    text,
                    i64::try_from(text_bytes)
                        .map_err(|_| "benchmark_content_size_out_of_range")?
                ],
            )
            .map_err(|_| "benchmark_fixture_chunk_failed")?;
    }
    transaction
        .commit()
        .map_err(|_| "benchmark_fixture_commit_failed")?;
    Ok(content_bytes)
}

fn checkpoint(path: &Path) -> Result<(), &'static str> {
    Connection::open(path)
        .and_then(|connection| connection.execute_batch("PRAGMA wal_checkpoint(TRUNCATE);"))
        .map_err(|_| "benchmark_checkpoint_failed")
}

fn fts_index_bytes(path: &Path) -> Result<Option<u64>, &'static str> {
    let connection = Connection::open(path).map_err(|_| "benchmark_size_open_failed")?;
    let bytes = connection
        .query_row(
            "SELECT SUM(pgsize) FROM dbstat \
             WHERE name GLOB 'location_search_fts_*' OR name GLOB 'content_search_fts_*'",
            [],
            |row| row.get::<_, Option<i64>>(0),
        )
        .optional();
    match bytes {
        Ok(Some(Some(bytes))) => u64::try_from(bytes)
            .map(Some)
            .map_err(|_| "benchmark_index_size_out_of_range"),
        Ok(Some(None) | None) | Err(_) => Ok(None),
    }
}

fn file_bytes(path: &Path) -> Result<u64, &'static str> {
    fs::metadata(path)
        .map(|metadata| metadata.len())
        .map_err(|_| "benchmark_database_size_failed")
}

fn elapsed_us(started: Instant) -> u64 {
    u64::try_from(started.elapsed().as_micros()).unwrap_or(u64::MAX)
}

fn elapsed_ms(started: Instant) -> u64 {
    u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX)
}

fn validate_counts(documents: u32, iterations: u32) -> Result<(), &'static str> {
    if documents == 0 || documents > MAX_DOCUMENTS {
        return Err("benchmark_document_count_out_of_range");
    }
    if iterations == 0 || iterations > MAX_ITERATIONS {
        return Err("benchmark_iteration_count_out_of_range");
    }
    Ok(())
}

fn percentile(sorted_samples: &[u64], percentile: usize) -> u64 {
    if sorted_samples.is_empty() {
        return 0;
    }
    let rank = sorted_samples
        .len()
        .saturating_mul(percentile)
        .saturating_add(99)
        / 100;
    sorted_samples[rank.saturating_sub(1).min(sorted_samples.len() - 1)]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn percentile_uses_nearest_rank_without_panicking_on_empty_input() {
        assert_eq!(percentile(&[], 95), 0);
        assert_eq!(percentile(&[1, 2, 3, 4, 5], 50), 3);
        assert_eq!(percentile(&[1, 2, 3, 4, 5], 95), 5);
    }

    #[test]
    fn benchmark_limits_are_finite() {
        assert_eq!(
            validate_counts(DEFAULT_DOCUMENTS, DEFAULT_ITERATIONS),
            Ok(())
        );
        assert_eq!(
            validate_counts(0, DEFAULT_ITERATIONS),
            Err("benchmark_document_count_out_of_range")
        );
        assert_eq!(
            validate_counts(DEFAULT_DOCUMENTS, MAX_ITERATIONS + 1),
            Err("benchmark_iteration_count_out_of_range")
        );
    }
}
