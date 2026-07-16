use deskgraph_domain::{HealthReport, collect_health};
use deskgraph_telemetry::{Service, init_privacy_safe_logging};
use tracing::info;

#[tauri::command]
fn health() -> HealthReport {
    let report = collect_health();
    info!(
        event = "health_check_completed",
        status = report.status,
        database_state = ?report.database.state
    );
    report
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let _logger_installed = init_privacy_safe_logging(Service::Desktop);
    info!(event = "desktop_starting");

    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![health])
        .run(tauri::generate_context!())
        .expect("DeskGraph desktop runtime failed");
}

#[cfg(test)]
mod tests {
    use super::*;
    use deskgraph_domain::{LifecycleState, collect_health};

    #[test]
    fn tauri_health_command_uses_the_shared_domain_contract() {
        let from_command = health();
        let from_domain = collect_health();

        assert_eq!(from_command, from_domain);
        assert_eq!(from_command.database.state, LifecycleState::NotInitialized);
    }

    #[test]
    fn tauri_health_payload_excludes_filesystem_locations() {
        let payload = serde_json::to_string(&health()).expect("health must serialize");

        assert!(!payload.contains("/Users/"));
        assert!(!payload.contains("C:\\Users\\"));
        assert!(payload.contains("\"filesystem_locations_included\":false"));
    }
}
