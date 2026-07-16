use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct AuthorizedScope {
    pub id: i64,
    pub display_path: String,
    pub created_at_unix_ms: i64,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ManifestStats {
    pub api_version: &'static str,
    pub database_ready: bool,
    pub authorized_scope_count: u64,
    pub node_count: u64,
    pub file_count: u64,
    pub folder_count: u64,
    pub active_location_count: u64,
    pub issue_count: u64,
    pub completed_scan_count: u64,
}

impl ManifestStats {
    pub const API_VERSION: &str = "deskgraph.manifest.v1";
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ScanStatus {
    Running,
    Completed,
    Failed,
    Interrupted,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ScanReport {
    pub api_version: &'static str,
    pub job_id: i64,
    pub scope_id: i64,
    pub status: ScanStatus,
    pub discovered_files: u64,
    pub discovered_folders: u64,
    pub skipped_entries: u64,
    pub issue_count: u64,
    pub elapsed_ms: u64,
}

impl ScanReport {
    pub const API_VERSION: &str = "deskgraph.scan.v1";
}
