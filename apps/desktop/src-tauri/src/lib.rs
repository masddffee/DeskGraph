use std::path::{Path, PathBuf};

use deskgraph_database::ManifestDatabase;
use deskgraph_domain::{
    AuthorizedScope, HealthReport, ManifestStats, ScanReport, collect_health_with_manifest,
};
use deskgraph_scanner::{authorize_scope, scan_scope};
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
async fn run_manifest_scan(
    state: State<'_, ManifestState>,
    scope_id: i64,
) -> Result<ScanReport, String> {
    let database_path = state.database_path.clone();
    let report = tauri::async_runtime::spawn_blocking(move || {
        run_manifest_scan_at(&database_path, scope_id).map_err(str::to_string)
    })
    .await
    .map_err(|_| "scan_worker_failed".to_string())??;
    info!(
        event = "metadata_scan_completed",
        scope_id = report.scope_id,
        job_id = report.job_id,
        discovered_files = report.discovered_files,
        discovered_folders = report.discovered_folders,
        skipped_entries = report.skipped_entries,
        issue_count = report.issue_count,
        elapsed_ms = report.elapsed_ms
    );
    Ok(report)
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

fn run_manifest_scan_at(path: &Path, scope_id: i64) -> Result<ScanReport, &'static str> {
    let mut database = ManifestDatabase::open(path).map_err(|error| error.code())?;
    scan_scope(&mut database, scope_id).map_err(|error| error.code())
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
            run_manifest_scan
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
        let report =
            run_manifest_scan_at(&database_path, scope.id).expect("metadata scan should pass");
        let status = manifest_status_at(&database_path).expect("status should load");

        assert_eq!(report.discovered_files, 1);
        assert_eq!(status.authorized_scope_count, 1);
        assert_eq!(status.file_count, 1);
        assert_eq!(
            authorized_scopes_at(&database_path).expect("scopes should load"),
            vec![scope]
        );
    }
}
