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
    Paused,
    Completed,
    Failed,
    Interrupted,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ScanJobProgress {
    pub api_version: &'static str,
    pub job_id: i64,
    pub scope_id: i64,
    pub status: ScanStatus,
    pub queued_entries: u64,
    pub processed_entries: u64,
    pub discovered_files: u64,
    pub discovered_folders: u64,
    pub skipped_entries: u64,
    pub issue_count: u64,
    pub elapsed_ms: u64,
    pub pause_requested: bool,
}

impl ScanJobProgress {
    pub const API_VERSION: &str = "deskgraph.scan-job.v1";

    #[must_use]
    pub fn is_terminal(&self) -> bool {
        matches!(self.status, ScanStatus::Completed | ScanStatus::Failed)
    }
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

impl TryFrom<ScanJobProgress> for ScanReport {
    type Error = &'static str;

    fn try_from(job: ScanJobProgress) -> Result<Self, Self::Error> {
        if job.status != ScanStatus::Completed {
            return Err("scan_job_not_completed");
        }
        Ok(Self {
            api_version: Self::API_VERSION,
            job_id: job.job_id,
            scope_id: job.scope_id,
            status: job.status,
            discovered_files: job.discovered_files,
            discovered_folders: job.discovered_folders,
            skipped_entries: job.skipped_entries,
            issue_count: job.issue_count,
            elapsed_ms: job.elapsed_ms,
        })
    }
}
