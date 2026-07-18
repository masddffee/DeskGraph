use std::path::{Path, PathBuf};

use deskgraph_database::ManifestDatabase;
use deskgraph_domain::{
    ActionPlanPreview, ActionPlanSummary, AuthorizedScope, ExtractionJobProgress,
    ExtractionOperation, ExtractionStats, HealthReport, ManifestStats, ScanJobProgress,
    SearchFilters, SearchResponse, WatchEventProgress, collect_health_with_manifest,
};
use deskgraph_extractors::{
    ExtractionLimits, cancel_extraction_job_at, create_screenshot_ocr_job_at, extraction_job_at,
    extraction_stats_at as read_extraction_stats_at,
    recent_extraction_jobs_at as read_recent_extraction_jobs_at, resume_extraction_job_at,
    run_extraction_job_at,
};
use deskgraph_retrieval::{SearchRequest, SearchSourceFilter, search_at as run_search_at};
use deskgraph_scanner::{
    authorize_scope, create_scan_job, pause_scan_job, resume_scan_job, run_scan_job_to_terminal,
};
use deskgraph_telemetry::{Service, init_privacy_safe_logging};
use deskgraph_transactions::{create_rename_preview_at, recent_action_plans_at};
use deskgraph_watcher::recent_watch_events_at as read_recent_watch_events_at;
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

/// Queues OCR for an already-scanned image. The core service revalidates the
/// authorized scope and node identity; callers never provide a filesystem path.
#[tauri::command]
fn create_screenshot_ocr_job(
    state: State<'_, ManifestState>,
    scope_id: i64,
    node_id: i64,
) -> Result<ExtractionJobProgress, String> {
    let progress = create_screenshot_ocr_job_for_database(&state.database_path, scope_id, node_id)
        .map_err(str::to_string)?;
    log_screenshot_ocr_progress("screenshot_ocr_created", &progress);
    Ok(progress)
}

/// Runs only a previously-created screenshot OCR job. It deliberately accepts
/// no path or content and is moved off Tauri's async runtime while native OCR runs.
#[tauri::command]
async fn run_screenshot_ocr_job(
    state: State<'_, ManifestState>,
    job_id: i64,
) -> Result<ExtractionJobProgress, String> {
    let database_path = state.database_path.clone();
    let progress = tauri::async_runtime::spawn_blocking(move || {
        run_screenshot_ocr_job_at(&database_path, job_id).map_err(str::to_string)
    })
    .await
    .map_err(|_| "screenshot_ocr_worker_failed".to_string())??;
    log_screenshot_ocr_progress("screenshot_ocr_runner_stopped", &progress);
    Ok(progress)
}

#[tauri::command]
fn screenshot_ocr_job_status(
    state: State<'_, ManifestState>,
    job_id: i64,
) -> Result<ExtractionJobProgress, String> {
    screenshot_ocr_job_status_at(&state.database_path, job_id).map_err(str::to_string)
}

/// Looks up only the most actionable screenshot OCR job for one already-scanned
/// node. This avoids exposing a filesystem path or document text while letting
/// the desktop recover interrupted work that has fallen outside the recent-jobs
/// dashboard window.
#[tauri::command]
fn screenshot_ocr_job_for_node(
    state: State<'_, ManifestState>,
    scope_id: i64,
    node_id: i64,
) -> Result<Option<ExtractionJobProgress>, String> {
    screenshot_ocr_job_for_node_at(&state.database_path, scope_id, node_id).map_err(str::to_string)
}

#[tauri::command]
fn cancel_screenshot_ocr_job(
    state: State<'_, ManifestState>,
    job_id: i64,
) -> Result<ExtractionJobProgress, String> {
    let progress =
        cancel_screenshot_ocr_job_at(&state.database_path, job_id).map_err(str::to_string)?;
    log_screenshot_ocr_progress("screenshot_ocr_cancel_requested", &progress);
    Ok(progress)
}

/// Re-queues only an interrupted screenshot OCR job. The client supplies no
/// filesystem path or extracted content; the core service revalidates source
/// identity before returning it to the queue.
#[tauri::command]
fn resume_screenshot_ocr_job(
    state: State<'_, ManifestState>,
    job_id: i64,
) -> Result<ExtractionJobProgress, String> {
    let progress =
        resume_screenshot_ocr_job_at(&state.database_path, job_id).map_err(str::to_string)?;
    log_screenshot_ocr_progress("screenshot_ocr_resume_requested", &progress);
    Ok(progress)
}

#[tauri::command]
fn recent_watch_events(state: State<'_, ManifestState>) -> Result<Vec<WatchEventProgress>, String> {
    recent_watch_events_for_database(&state.database_path).map_err(str::to_string)
}

#[tauri::command]
fn create_rename_preview(
    state: State<'_, ManifestState>,
    scope_id: i64,
    source_path: String,
    new_name: String,
) -> Result<ActionPlanPreview, String> {
    let preview = create_rename_preview_for_database(
        &state.database_path,
        scope_id,
        Path::new(&source_path),
        &new_name,
    )
    .map_err(str::to_string)?;
    info!(
        event = "rename_preview_created",
        plan_id = preview.plan_id,
        scope_id = preview.scope_id,
        node_id = preview.node_id,
        execution_strategy = ?preview.execution_strategy
    );
    Ok(preview)
}

#[tauri::command]
fn recent_action_plans(state: State<'_, ManifestState>) -> Result<Vec<ActionPlanSummary>, String> {
    recent_action_plans_for_database(&state.database_path).map_err(str::to_string)
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

fn create_screenshot_ocr_job_for_database(
    path: &Path,
    scope_id: i64,
    node_id: i64,
) -> Result<ExtractionJobProgress, &'static str> {
    create_screenshot_ocr_job_at(path, scope_id, node_id).map_err(|error| error.code())
}

fn run_screenshot_ocr_job_at(
    path: &Path,
    job_id: i64,
) -> Result<ExtractionJobProgress, &'static str> {
    require_screenshot_ocr_job(path, job_id)?;
    run_extraction_job_at(path, job_id, ExtractionLimits::default()).map_err(|error| error.code())
}

fn screenshot_ocr_job_status_at(
    path: &Path,
    job_id: i64,
) -> Result<ExtractionJobProgress, &'static str> {
    require_screenshot_ocr_job(path, job_id)
}

fn screenshot_ocr_job_for_node_at(
    path: &Path,
    scope_id: i64,
    node_id: i64,
) -> Result<Option<ExtractionJobProgress>, &'static str> {
    ManifestDatabase::open(path)
        .and_then(|database| database.screenshot_ocr_job_for_node(scope_id, node_id))
        .map_err(|error| error.code())
}

fn cancel_screenshot_ocr_job_at(
    path: &Path,
    job_id: i64,
) -> Result<ExtractionJobProgress, &'static str> {
    require_screenshot_ocr_job(path, job_id)?;
    cancel_extraction_job_at(path, job_id).map_err(|error| error.code())
}

fn resume_screenshot_ocr_job_at(
    path: &Path,
    job_id: i64,
) -> Result<ExtractionJobProgress, &'static str> {
    require_screenshot_ocr_job(path, job_id)?;
    resume_extraction_job_at(path, job_id).map_err(|error| error.code())
}

fn require_screenshot_ocr_job(
    path: &Path,
    job_id: i64,
) -> Result<ExtractionJobProgress, &'static str> {
    let progress = extraction_job_at(path, job_id).map_err(|error| error.code())?;
    if progress.operation != ExtractionOperation::ScreenshotOcr {
        return Err("screenshot_ocr_job_required");
    }
    Ok(progress)
}

fn recent_watch_events_for_database(path: &Path) -> Result<Vec<WatchEventProgress>, &'static str> {
    read_recent_watch_events_at(path).map_err(|error| error.code())
}

fn create_rename_preview_for_database(
    database_path: &Path,
    scope_id: i64,
    source_path: &Path,
    new_name: &str,
) -> Result<ActionPlanPreview, &'static str> {
    create_rename_preview_at(database_path, scope_id, source_path, new_name)
        .map_err(|error| error.code())
}

fn recent_action_plans_for_database(path: &Path) -> Result<Vec<ActionPlanSummary>, &'static str> {
    recent_action_plans_at(path).map_err(|error| error.code())
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

fn log_screenshot_ocr_progress(event: &'static str, progress: &ExtractionJobProgress) {
    info!(
        event = event,
        scope_id = progress.scope_id,
        job_id = progress.job_id,
        node_id = progress.node_id,
        status = ?progress.status,
        operation = ?progress.operation,
        output_bytes = progress.output_bytes,
        chunk_count = progress.chunk_count,
        elapsed_ms = progress.elapsed_ms,
        cancel_requested = progress.cancel_requested
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
            create_screenshot_ocr_job,
            run_screenshot_ocr_job,
            screenshot_ocr_job_status,
            screenshot_ocr_job_for_node,
            cancel_screenshot_ocr_job,
            resume_screenshot_ocr_job,
            recent_watch_events,
            create_rename_preview,
            recent_action_plans,
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
    use deskgraph_domain::{ExtractionOperation, ExtractionStatus, LifecycleState};
    use deskgraph_extractors::{ExtractionLimits, create_extraction_job_at, run_extraction_job_at};
    use deskgraph_watcher::{WatchPolicy, observe_watch_path_at};

    fn scanned_file_fixture(
        file_name: &str,
        contents: impl AsRef<[u8]>,
    ) -> (tempfile::TempDir, PathBuf, AuthorizedScope, i64) {
        let directory = tempfile::tempdir().expect("fixture root should exist");
        let database_path = directory.path().join("app-data/manifest.sqlite3");
        let scope_path = directory.path().join("authorized-private");
        let source_path = scope_path.join(file_name);
        std::fs::create_dir(&scope_path).expect("scope should create");
        std::fs::write(&source_path, contents).expect("file should create");

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
        (directory, database_path, scope, node_id)
    }

    fn png_bytes(width: u32, height: u32, private_marker: &[u8]) -> Vec<u8> {
        let mut bytes = vec![0_u8; 32];
        bytes[..8].copy_from_slice(b"\x89PNG\r\n\x1a\n");
        bytes[8..12].copy_from_slice(&13_u32.to_be_bytes());
        bytes[12..16].copy_from_slice(b"IHDR");
        bytes[16..20].copy_from_slice(&width.to_be_bytes());
        bytes[20..24].copy_from_slice(&height.to_be_bytes());
        bytes.extend_from_slice(private_marker);
        bytes
    }

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
    fn screenshot_ocr_helpers_queue_cancel_and_expose_path_free_progress() {
        let private_text = "OCR private text must not enter job progress";
        let image = png_bytes(640, 480, private_text.as_bytes());
        let (_directory, database_path, scope, node_id) =
            scanned_file_fixture("private-screenshot.png", image);

        let queued = create_screenshot_ocr_job_for_database(&database_path, scope.id, node_id)
            .expect("screenshot OCR should queue");
        assert_eq!(queued.operation, ExtractionOperation::ScreenshotOcr);
        assert_eq!(queued.status, ExtractionStatus::Queued);
        assert_eq!(
            screenshot_ocr_job_status_at(&database_path, queued.job_id)
                .expect("OCR status should load"),
            queued
        );

        let cancelled = cancel_screenshot_ocr_job_at(&database_path, queued.job_id)
            .expect("queued OCR should cancel durably");
        assert_eq!(cancelled.status, ExtractionStatus::Cancelled);
        assert!(cancelled.cancel_requested);
        assert_eq!(cancelled.chunk_count, 0);
        assert_eq!(cancelled.output_bytes, 0);
        assert_eq!(
            screenshot_ocr_job_status_at(&database_path, queued.job_id)
                .expect("cancelled OCR status should load"),
            cancelled
        );

        let payload = serde_json::to_string(&cancelled).expect("progress should serialize");
        assert!(payload.contains("deskgraph.extraction-job.v2"));
        assert!(!payload.contains("private-screenshot.png"));
        assert!(!payload.contains(private_text));
        assert!(!payload.contains("authorized-private"));
        assert!(!payload.contains("\"path\""));
        assert!(!payload.contains("\"text\""));
    }

    #[test]
    fn screenshot_ocr_helpers_reject_unknown_and_non_ocr_jobs() {
        let (_directory, database_path, scope, node_id) =
            scanned_file_fixture("private-note.md", "private generic content");

        assert_eq!(
            create_screenshot_ocr_job_for_database(&database_path, scope.id, node_id + 9_999)
                .expect_err("unknown nodes must be rejected"),
            "extractable_file_not_found"
        );

        let generic = create_extraction_job_at(&database_path, scope.id, node_id)
            .expect("generic extraction should queue");
        assert_eq!(generic.operation, ExtractionOperation::Content);
        assert_eq!(
            screenshot_ocr_job_status_at(&database_path, generic.job_id)
                .expect_err("OCR status must not expose generic jobs"),
            "screenshot_ocr_job_required"
        );
        assert_eq!(
            run_screenshot_ocr_job_at(&database_path, generic.job_id)
                .expect_err("OCR runner must not execute generic jobs"),
            "screenshot_ocr_job_required"
        );
        assert_eq!(
            cancel_screenshot_ocr_job_at(&database_path, generic.job_id)
                .expect_err("OCR cancellation must not affect generic jobs"),
            "screenshot_ocr_job_required"
        );
        assert_eq!(
            extraction_job_at(&database_path, generic.job_id)
                .expect("generic job should remain readable to core")
                .status,
            ExtractionStatus::Queued
        );
    }

    #[test]
    fn screenshot_ocr_creation_rejects_non_images_without_side_effects() {
        let private_text = "this text must never be OCR job payload";
        let (_directory, database_path, scope, node_id) =
            scanned_file_fixture("private-format.md", private_text);
        let before = recent_content_extractions_at(&database_path)
            .expect("recent jobs should load before failed creation");

        assert_eq!(
            create_screenshot_ocr_job_for_database(&database_path, scope.id, node_id)
                .expect_err("non-image OCR must be rejected before a job is inserted"),
            "extraction_media_kind_unsupported"
        );
        assert_eq!(
            recent_content_extractions_at(&database_path)
                .expect("failed creation must not insert a job"),
            before
        );

        let generic = create_extraction_job_at(&database_path, scope.id, node_id)
            .expect("a rejected OCR request must not block ordinary content extraction");
        assert_eq!(generic.operation, ExtractionOperation::Content);
        let payload = serde_json::to_string(&generic).expect("progress should serialize");
        assert!(!payload.contains("private-format.md"));
        assert!(!payload.contains(private_text));
        assert!(!payload.contains("authorized-private"));
        assert!(!payload.contains("\"path\""));
        assert!(!payload.contains("\"text\""));
    }

    #[test]
    fn screenshot_ocr_resume_requires_an_interrupted_ocr_job_and_keeps_payload_private() {
        let private_text = "interrupted OCR private text must not be exposed";
        let image = png_bytes(640, 480, private_text.as_bytes());
        let (_directory, database_path, scope, node_id) =
            scanned_file_fixture("private-interrupted.png", image);
        let queued = create_screenshot_ocr_job_for_database(&database_path, scope.id, node_id)
            .expect("valid screenshot OCR should queue");

        assert_eq!(
            resume_screenshot_ocr_job_at(&database_path, queued.job_id)
                .expect_err("a non-interrupted OCR job must not resume"),
            "invalid_extraction_job_state"
        );
        assert_eq!(
            extraction_job_at(&database_path, queued.job_id)
                .expect("non-interrupted OCR state should remain readable")
                .status,
            ExtractionStatus::Queued
        );

        let mut database = ManifestDatabase::open(&database_path).expect("database should open");
        database
            .claim_extraction_job(queued.job_id, "expired-ocr-runner", 60_000)
            .expect("OCR job should claim for recovery fixture");
        assert_eq!(
            database
                .recover_expired_extraction_jobs_at(i64::MAX)
                .expect("expired OCR lease should recover"),
            1
        );
        assert_eq!(
            database
                .extraction_job(queued.job_id)
                .expect("interrupted OCR should load")
                .status,
            ExtractionStatus::Interrupted
        );
        drop(database);

        let resumed = resume_screenshot_ocr_job_at(&database_path, queued.job_id)
            .expect("interrupted screenshot OCR should requeue");
        assert_eq!(resumed.operation, ExtractionOperation::ScreenshotOcr);
        assert_eq!(resumed.status, ExtractionStatus::Queued);
        assert!(!resumed.cancel_requested);

        let (_generic_directory, generic_database_path, generic_scope, generic_node_id) =
            scanned_file_fixture("private-generic.md", "private generic content");
        let generic =
            create_extraction_job_at(&generic_database_path, generic_scope.id, generic_node_id)
                .expect("generic extraction should queue");
        assert_eq!(
            resume_screenshot_ocr_job_at(&generic_database_path, generic.job_id)
                .expect_err("generic jobs must not use screenshot OCR resume"),
            "screenshot_ocr_job_required"
        );
        assert_eq!(
            extraction_job_at(&generic_database_path, generic.job_id)
                .expect("generic state should remain readable")
                .status,
            ExtractionStatus::Queued
        );

        let payload = serde_json::to_string(&resumed).expect("progress should serialize");
        assert!(payload.contains("deskgraph.extraction-job.v2"));
        assert!(!payload.contains("private-interrupted.png"));
        assert!(!payload.contains(private_text));
        assert!(!payload.contains("authorized-private"));
        assert!(!payload.contains("\"path\""));
        assert!(!payload.contains("\"text\""));
    }

    #[test]
    fn screenshot_ocr_node_lookup_is_operation_scoped_and_path_free() {
        let private_text = "node lookup OCR text must stay private";
        let image = png_bytes(640, 480, private_text.as_bytes());
        let (_directory, database_path, scope, node_id) =
            scanned_file_fixture("private-node-lookup.png", image);
        let queued = create_screenshot_ocr_job_for_database(&database_path, scope.id, node_id)
            .expect("valid screenshot OCR should queue");
        let mut database = ManifestDatabase::open(&database_path).expect("database should open");
        database
            .claim_extraction_job(queued.job_id, "expired-node-lookup-runner", 60_000)
            .expect("OCR job should claim for recovery fixture");
        database
            .recover_expired_extraction_jobs_at(i64::MAX)
            .expect("expired OCR lease should recover");
        drop(database);

        let found = screenshot_ocr_job_for_node_at(&database_path, scope.id, node_id)
            .expect("node lookup should query")
            .expect("interrupted OCR should be found");
        assert_eq!(found.job_id, queued.job_id);
        assert_eq!(found.operation, ExtractionOperation::ScreenshotOcr);
        assert_eq!(found.status, ExtractionStatus::Interrupted);
        let payload = serde_json::to_string(&found).expect("progress should serialize");
        assert!(!payload.contains("private-node-lookup.png"));
        assert!(!payload.contains(private_text));
        assert!(!payload.contains("authorized-private"));
        assert!(!payload.contains("\"path\""));
        assert!(!payload.contains("\"text\""));

        let (_generic_directory, generic_database_path, generic_scope, generic_node_id) =
            scanned_file_fixture("private-node-lookup.md", "generic private text");
        create_extraction_job_at(&generic_database_path, generic_scope.id, generic_node_id)
            .expect("generic job should queue");
        assert_eq!(
            screenshot_ocr_job_for_node_at(
                &generic_database_path,
                generic_scope.id,
                generic_node_id,
            )
            .expect("generic node lookup should query"),
            None
        );
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

    #[test]
    fn rename_preview_helper_changes_no_file_and_history_is_path_free() {
        let directory = tempfile::tempdir().expect("fixture root should exist");
        let database_path = directory.path().join("app-data/manifest.sqlite3");
        let scope_path = directory.path().join("authorized-actions-private");
        let source_path = scope_path.join("private-draft.md");
        let destination_path = scope_path.join("private-final.md");
        std::fs::create_dir(&scope_path).expect("scope should create");
        std::fs::write(&source_path, "private action text").expect("file should create");
        initialize_manifest(&database_path).expect("manifest should initialize");
        let scope =
            authorize_scope_at(&database_path, &scope_path).expect("scope should authorize");
        let scan = create_manifest_scan_at(&database_path, scope.id).expect("scan should create");
        run_manifest_scan_at(&database_path, scan.job_id).expect("scan should complete");

        let preview = create_rename_preview_for_database(
            &database_path,
            scope.id,
            &source_path,
            "private-final.md",
        )
        .expect("preview should create");
        assert_eq!(preview.state, deskgraph_domain::ActionPlanState::Previewed);
        assert!(source_path.exists());
        assert!(!destination_path.exists());
        let explicit_payload =
            serde_json::to_string(&preview).expect("explicit preview should serialize");
        assert!(explicit_payload.contains("private-draft.md"));
        assert!(explicit_payload.contains("private-final.md"));

        let summary_payload = serde_json::to_string(
            &recent_action_plans_for_database(&database_path)
                .expect("path-free plan history should load"),
        )
        .expect("plan summaries should serialize");
        assert!(summary_payload.contains("deskgraph.action-plan-summary.v1"));
        assert!(!summary_payload.contains("private-draft.md"));
        assert!(!summary_payload.contains("private-final.md"));
        assert!(!summary_payload.contains(scope_path.to_string_lossy().as_ref()));
    }

    #[test]
    fn watch_helper_exposes_only_path_free_durable_states() {
        let directory = tempfile::tempdir().expect("fixture root should exist");
        let database_path = directory.path().join("app-data/manifest.sqlite3");
        let scope_path = directory.path().join("authorized-watch-private");
        let source_path = scope_path.join("private-watch.md");
        std::fs::create_dir(&scope_path).expect("scope should create");
        std::fs::write(&source_path, "private watch text").expect("file should create");
        initialize_manifest(&database_path).expect("manifest should initialize");
        let scope =
            authorize_scope_at(&database_path, &scope_path).expect("scope should authorize");
        let scan = create_manifest_scan_at(&database_path, scope.id).expect("scan should create");
        run_manifest_scan_at(&database_path, scan.job_id).expect("scan should complete");
        observe_watch_path_at(
            &database_path,
            scope.id,
            &source_path,
            WatchPolicy::default(),
        )
        .expect("watch hint should persist");

        let payload = serde_json::to_string(
            &recent_watch_events_for_database(&database_path).expect("watch states should load"),
        )
        .expect("watch states should serialize");
        assert!(payload.contains("deskgraph.watch-event.v1"));
        assert!(payload.contains("stabilizing"));
        assert!(!payload.contains("private-watch.md"));
        assert!(!payload.contains("private watch text"));
        assert!(!payload.contains(scope_path.to_string_lossy().as_ref()));
    }
}
