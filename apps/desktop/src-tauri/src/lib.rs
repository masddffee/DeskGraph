use std::path::{Path, PathBuf};

use deskgraph_database::ManifestDatabase;
use deskgraph_domain::{
    AuthorizedScope, HealthReport, ManifestStats, ScanJobProgress, collect_health_with_manifest,
};
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
}
