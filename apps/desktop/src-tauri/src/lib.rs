use std::path::{Path, PathBuf};

use deskgraph_database::ManifestDatabase;
use deskgraph_domain::{
    AuthorizedScope, ExtractionJobProgress, ExtractionStats, HealthReport, ManifestStats,
    ScanJobProgress, SearchFilters, SearchResponse, collect_health_with_manifest,
};
use deskgraph_extractors::{
    extraction_stats_at as read_extraction_stats_at,
    recent_extraction_jobs_at as read_recent_extraction_jobs_at,
};
use deskgraph_retrieval::{SearchRequest, SearchSourceFilter, search_at as run_search_at};
use deskgraph_scanner::{
    authorize_scope, create_scan_job, pause_scan_job, resume_scan_job, run_scan_job_to_terminal,
};
use deskgraph_telemetry::{Service, init_privacy_safe_logging};
use tauri::{Manager, State};
use tracing::info;

struct ManifestState {
    database_path: PathBuf,
}

#[tauri::command]
fn health(state: State<'_, ManifestState>) -> Result<HealthReport, String> {
    let report = health_at(&state.database_path).map_err(str::to_string)?;
    info!(
        event = "health_check_completed",
        status = report.status,
        database_state = ?report.database.state
    );
    Ok(report)
}

#[tauri::command]
fn manifest_status(state: State<'_, ManifestState>) -> Result<ManifestStats, String> {
    manifest_status_at(&state.database_path).map_err(str::to_string)
}

#[tauri::command]
fn authorized_scopes(state: State<'_, ManifestState>) -> Result<Vec<AuthorizedScope>, String> {
    authorized_scopes_at(&state.database_path).map_err(str::to_string)
}

#[tauri::command]
fn authorize_scope_path(
    state: State<'_, ManifestState>,
    path: String,
) -> Result<AuthorizedScope, String> {
    let scope =
        authorize_scope_at(&state.database_path, Path::new(&path)).map_err(str::to_string)?;
    info!(event = "scope_authorized", scope_id = scope.id);
    Ok(scope)
}

#[tauri::command]
fn create_manifest_scan(
    state: State<'_, ManifestState>,
    scope_id: i64,
) -> Result<ScanJobProgress, String> {
    let progress =
        create_manifest_scan_at(&state.database_path, scope_id).map_err(str::to_string)?;
    log_scan_progress("metadata_scan_created", &progress);
    Ok(progress)
}

#[tauri::command]
async fn run_manifest_scan(
    state: State<'_, ManifestState>,
    job_id: i64,
) -> Result<ScanJobProgress, String> {
    let database_path = state.database_path.clone();
    let progress = tauri::async_runtime::spawn_blocking(move || {
        run_manifest_scan_at(&database_path, job_id).map_err(str::to_string)
    })
    .await
    .map_err(|_| "scan_worker_failed".to_string())??;
    log_scan_progress("metadata_scan_runner_stopped", &progress);
    Ok(progress)
}

#[tauri::command]
fn scan_job_status(
    state: State<'_, ManifestState>,
    job_id: i64,
) -> Result<ScanJobProgress, String> {
    scan_job_status_at(&state.database_path, job_id).map_err(str::to_string)
}

#[tauri::command]
fn recent_scan_jobs(state: State<'_, ManifestState>) -> Result<Vec<ScanJobProgress>, String> {
    recent_scan_jobs_at(&state.database_path).map_err(str::to_string)
}

#[tauri::command]
fn content_extraction_stats(state: State<'_, ManifestState>) -> Result<ExtractionStats, String> {
    content_extraction_stats_at(&state.database_path).map_err(str::to_string)
}

#[tauri::command]
fn recent_content_extractions(
    state: State<'_, ManifestState>,
) -> Result<Vec<ExtractionJobProgress>, String> {
    recent_content_extractions_at(&state.database_path).map_err(str::to_string)
}

#[tauri::command]
fn search_local(
    state: State<'_, ManifestState>,
    query: String,
    filters: SearchFilters,
    limit: Option<u32>,
) -> Result<SearchResponse, String> {
    let response =
        search_local_at(&state.database_path, &query, &filters, limit).map_err(str::to_string)?;
    info!(
        event = "local_search_completed",
        scope_id = filters.scope_id,
        result_count = response.result_count,
        elapsed_ms = response.elapsed_ms,
        filters_applied = filters.extension.is_some()
            || filters.modified_since_unix_seconds.is_some()
            || filters.modified_before_unix_seconds.is_some()
            || filters.source != SearchSourceFilter::All,
        mode = "lexical"
    );
    Ok(response)
}

#[tauri::command]
fn pause_manifest_scan(
    state: State<'_, ManifestState>,
    job_id: i64,
) -> Result<ScanJobProgress, String> {
    let progress = pause_manifest_scan_at(&state.database_path, job_id).map_err(str::to_string)?;
    log_scan_progress("metadata_scan_pause_requested", &progress);
    Ok(progress)
}

#[tauri::command]
fn resume_manifest_scan(
    state: State<'_, ManifestState>,
    job_id: i64,
) -> Result<ScanJobProgress, String> {
    let progress = resume_manifest_scan_at(&state.database_path, job_id).map_err(str::to_string)?;
    log_scan_progress("metadata_scan_resumed", &progress);
    Ok(progress)
}

fn initialize_manifest(path: &Path) -> Result<(), &'static str> {
    ManifestDatabase::open(path)
        .map(|_| ())
        .map_err(|error| error.code())
}

fn health_at(path: &Path) -> Result<HealthReport, &'static str> {
    let stats = manifest_status_at(path)?;
    let scope_count = u32::try_from(stats.authorized_scope_count)
        .map_err(|_| "authorized_scope_count_out_of_range")?;
    Ok(collect_health_with_manifest(scope_count))
}

fn manifest_status_at(path: &Path) -> Result<ManifestStats, &'static str> {
    ManifestDatabase::open(path)
        .and_then(|database| database.stats())
        .map_err(|error| error.code())
}

fn authorized_scopes_at(path: &Path) -> Result<Vec<AuthorizedScope>, &'static str> {
    ManifestDatabase::open(path)
        .and_then(|database| database.list_scopes())
        .map_err(|error| error.code())
}

fn authorize_scope_at(path: &Path, requested_path: &Path) -> Result<AuthorizedScope, &'static str> {
    let database = ManifestDatabase::open(path).map_err(|error| error.code())?;
    authorize_scope(&database, requested_path).map_err(|error| error.code())
}

fn create_manifest_scan_at(path: &Path, scope_id: i64) -> Result<ScanJobProgress, &'static str> {
    let mut database = ManifestDatabase::open(path).map_err(|error| error.code())?;
    create_scan_job(&mut database, scope_id).map_err(|error| error.code())
}

fn run_manifest_scan_at(path: &Path, job_id: i64) -> Result<ScanJobProgress, &'static str> {
    let mut database = ManifestDatabase::open(path).map_err(|error| error.code())?;
    run_scan_job_to_terminal(&mut database, job_id).map_err(|error| error.code())
}

fn scan_job_status_at(path: &Path, job_id: i64) -> Result<ScanJobProgress, &'static str> {
    ManifestDatabase::open(path)
        .and_then(|database| database.scan_job(job_id))
        .map_err(|error| error.code())
}

fn recent_scan_jobs_at(path: &Path) -> Result<Vec<ScanJobProgress>, &'static str> {
    ManifestDatabase::open(path)
        .and_then(|database| database.recent_scan_jobs())
        .map_err(|error| error.code())
}

fn content_extraction_stats_at(path: &Path) -> Result<ExtractionStats, &'static str> {
    read_extraction_stats_at(path).map_err(|error| error.code())
}

fn recent_content_extractions_at(path: &Path) -> Result<Vec<ExtractionJobProgress>, &'static str> {
    read_recent_extraction_jobs_at(path).map_err(|error| error.code())
}

fn search_local_at(
    path: &Path,
    query: &str,
    filters: &SearchFilters,
    limit: Option<u32>,
) -> Result<SearchResponse, &'static str> {
    run_search_at(
        path,
        SearchRequest {
            query,
            scope_id: filters.scope_id,
            source: filters.source,
            extension: filters.extension.as_deref(),
            modified_since_unix_seconds: filters.modified_since_unix_seconds,
            modified_before_unix_seconds: filters.modified_before_unix_seconds,
            limit,
        },
    )
    .map_err(|error| error.code())
}

fn pause_manifest_scan_at(path: &Path, job_id: i64) -> Result<ScanJobProgress, &'static str> {
    let mut database = ManifestDatabase::open(path).map_err(|error| error.code())?;
    pause_scan_job(&mut database, job_id).map_err(|error| error.code())
}

fn resume_manifest_scan_at(path: &Path, job_id: i64) -> Result<ScanJobProgress, &'static str> {
    let mut database = ManifestDatabase::open(path).map_err(|error| error.code())?;
    resume_scan_job(&mut database, job_id).map_err(|error| error.code())
}

fn log_scan_progress(event: &'static str, progress: &ScanJobProgress) {
    info!(
        event = event,
        scope_id = progress.scope_id,
        job_id = progress.job_id,
        status = ?progress.status,
        queued_entries = progress.queued_entries,
        processed_entries = progress.processed_entries,
        discovered_files = progress.discovered_files,
        discovered_folders = progress.discovered_folders,
        skipped_entries = progress.skipped_entries,
        issue_count = progress.issue_count,
        elapsed_ms = progress.elapsed_ms
    );
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let _logger_installed = init_privacy_safe_logging(Service::Desktop);
    info!(event = "desktop_starting");

    tauri::Builder::default()
        .setup(|app| {
            let app_data = app
                .path()
                .app_data_dir()
                .map_err(|_| "app_data_path_unavailable")?;
            let database_path = app_data.join("manifest.sqlite3");
            initialize_manifest(&database_path)?;
            app.manage(ManifestState { database_path });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            health,
            manifest_status,
            authorized_scopes,
            authorize_scope_path,
            create_manifest_scan,
            run_manifest_scan,
            scan_job_status,
            recent_scan_jobs,
            content_extraction_stats,
            recent_content_extractions,
            search_local,
            pause_manifest_scan,
            resume_manifest_scan
        ])
        .run(tauri::generate_context!())
        .expect("DeskGraph desktop runtime failed");
}

#[cfg(test)]
mod tests {
    use super::*;
    use deskgraph_domain::LifecycleState;
    use deskgraph_extractors::{ExtractionLimits, create_extraction_job_at, run_extraction_job_at};

    #[test]
    fn initialized_health_uses_the_shared_domain_contract() {
        let directory = tempfile::tempdir().expect("fixture root should exist");
        let database_path = directory.path().join("manifest.sqlite3");
        initialize_manifest(&database_path).expect("manifest should initialize");

        let report = health_at(&database_path).expect("health should load");
        assert_eq!(report.database.state, LifecycleState::Ready);
        assert_eq!(report.privacy.authorized_scope_count, 0);
    }

    #[test]
    fn tauri_health_payload_excludes_filesystem_locations() {
        let directory = tempfile::tempdir().expect("fixture root should exist");
        let database_path = directory.path().join("manifest.sqlite3");
        initialize_manifest(&database_path).expect("manifest should initialize");
        let payload =
            serde_json::to_string(&health_at(&database_path).expect("health should load"))
                .expect("health must serialize");

        assert!(!payload.contains("/Users/"));
        assert!(!payload.contains("C:\\Users\\"));
        assert!(payload.contains("\"filesystem_locations_included\":false"));
    }

    #[test]
    fn manifest_helpers_cover_authorize_scan_and_status() {
        let directory = tempfile::tempdir().expect("fixture root should exist");
        let database_path = directory.path().join("app-data/manifest.sqlite3");
        let scope_path = directory.path().join("authorized");
        std::fs::create_dir(&scope_path).expect("scope should create");
        std::fs::write(scope_path.join("note.md"), "metadata fixture").expect("file should create");

        initialize_manifest(&database_path).expect("manifest should initialize");
        let scope =
            authorize_scope_at(&database_path, &scope_path).expect("scope should authorize");
        let job = create_manifest_scan_at(&database_path, scope.id).expect("job should create");
        let paused = pause_manifest_scan_at(&database_path, job.job_id).expect("job should pause");
        assert_eq!(paused.status, deskgraph_domain::ScanStatus::Paused);
        resume_manifest_scan_at(&database_path, job.job_id).expect("job should resume");
        let report =
            run_manifest_scan_at(&database_path, job.job_id).expect("metadata scan should pass");
        let status = manifest_status_at(&database_path).expect("status should load");

        assert_eq!(report.discovered_files, 1);
        assert_eq!(
            scan_job_status_at(&database_path, job.job_id)
                .expect("job status should load")
                .status,
            deskgraph_domain::ScanStatus::Completed
        );
        assert_eq!(
            recent_scan_jobs_at(&database_path)
                .expect("recent jobs should load")
                .first()
                .map(|recent| recent.job_id),
            Some(job.job_id)
        );
        assert_eq!(status.authorized_scope_count, 1);
        assert_eq!(status.file_count, 1);
        assert_eq!(
            authorized_scopes_at(&database_path).expect("scopes should load"),
            vec![scope]
        );
    }

    #[test]
    fn extraction_helpers_expose_counts_without_paths_or_text() {
        let directory = tempfile::tempdir().expect("fixture root should exist");
        let database_path = directory.path().join("app-data/manifest.sqlite3");
        let scope_path = directory.path().join("authorized-private");
        let source_path = scope_path.join("private-notes.md");
        let private_text = "私人內容不得出現在狀態 payload";
        std::fs::create_dir(&scope_path).expect("scope should create");
        std::fs::write(&source_path, private_text).expect("file should create");

        initialize_manifest(&database_path).expect("manifest should initialize");
        let scope =
            authorize_scope_at(&database_path, &scope_path).expect("scope should authorize");
        let scan = create_manifest_scan_at(&database_path, scope.id).expect("scan should create");
        run_manifest_scan_at(&database_path, scan.job_id).expect("scan should complete");
        let database = ManifestDatabase::open(&database_path).expect("database should open");
        let node_id = database
            .node_id_for_path_key(
                scope.id,
                &deskgraph_scanner::comparison_key(
                    &std::fs::canonicalize(&source_path).expect("source should canonicalize"),
                ),
            )
            .expect("node lookup should pass")
            .expect("source node should exist");
        drop(database);
        let job = create_extraction_job_at(&database_path, scope.id, node_id)
            .expect("extraction should create");
        run_extraction_job_at(&database_path, job.job_id, ExtractionLimits::default())
            .expect("extraction should complete");

        let payload = serde_json::to_string(&(
            content_extraction_stats_at(&database_path).expect("stats should load"),
            recent_content_extractions_at(&database_path).expect("jobs should load"),
        ))
        .expect("payload should serialize");
        assert!(!payload.contains(private_text));
        assert!(!payload.contains("private-notes.md"));
        assert!(!payload.contains(scope_path.to_string_lossy().as_ref()));
        assert!(payload.contains("deskgraph.extraction-stats.v1"));
        assert!(payload.contains("\"status\":\"completed\""));
    }

    #[test]
    fn search_helper_returns_bounded_user_requested_context() {
        let directory = tempfile::tempdir().expect("fixture root should exist");
        let database_path = directory.path().join("app-data/manifest.sqlite3");
        let scope_path = directory.path().join("authorized-search");
        let source_path = scope_path.join("專案-context.md");
        let private_text = "Traditional Chinese 專案脈絡 and English context";
        std::fs::create_dir(&scope_path).expect("scope should create");
        std::fs::write(&source_path, private_text).expect("file should create");

        initialize_manifest(&database_path).expect("manifest should initialize");
        let scope =
            authorize_scope_at(&database_path, &scope_path).expect("scope should authorize");
        let scan = create_manifest_scan_at(&database_path, scope.id).expect("scan should create");
        run_manifest_scan_at(&database_path, scan.job_id).expect("scan should complete");
        let database = ManifestDatabase::open(&database_path).expect("database should open");
        let node_id = database
            .node_id_for_path_key(
                scope.id,
                &deskgraph_scanner::comparison_key(
                    &std::fs::canonicalize(&source_path).expect("source should canonicalize"),
                ),
            )
            .expect("node lookup should pass")
            .expect("source node should exist");
        drop(database);
        let job = create_extraction_job_at(&database_path, scope.id, node_id)
            .expect("extraction should create");
        run_extraction_job_at(&database_path, job.job_id, ExtractionLimits::default())
            .expect("extraction should complete");

        let response = search_local_at(
            &database_path,
            "專案脈絡",
            &SearchFilters {
                scope_id: Some(scope.id),
                source: SearchSourceFilter::ExtractedText,
                extension: Some("MD".to_string()),
                modified_since_unix_seconds: None,
                modified_before_unix_seconds: None,
            },
            Some(10),
        )
        .expect("search should pass");
        assert_eq!(response.result_count, 1);
        assert_eq!(response.filters.source, SearchSourceFilter::ExtractedText);
        assert_eq!(response.filters.extension.as_deref(), Some("md"));
        assert_eq!(response.results[0].node_id, node_id);
        assert_eq!(response.results[0].explanation, "extracted_text_substring");
        assert!(
            response.results[0]
                .snippet
                .as_deref()
                .unwrap_or_default()
                .contains("專案脈絡")
        );
    }
}
