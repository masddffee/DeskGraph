use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExtractionStatus {
    Queued,
    Running,
    Completed,
    Failed,
    Cancelled,
    Interrupted,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ExtractionJobProgress {
    pub api_version: &'static str,
    pub job_id: i64,
    pub scope_id: i64,
    pub node_id: i64,
    pub status: ExtractionStatus,
    pub provider_id: Option<String>,
    pub provider_version: Option<String>,
    pub error_code: Option<String>,
    pub source_bytes: u64,
    pub output_bytes: u64,
    pub chunk_count: u64,
    pub elapsed_ms: u64,
    pub cancel_requested: bool,
}

impl ExtractionJobProgress {
    pub const API_VERSION: &str = "deskgraph.extraction-job.v1";

    #[must_use]
    pub fn is_terminal(&self) -> bool {
        matches!(
            self.status,
            ExtractionStatus::Completed | ExtractionStatus::Failed | ExtractionStatus::Cancelled
        )
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ExtractionStats {
    pub api_version: &'static str,
    pub active_chunk_count: u64,
    pub extracted_file_count: u64,
    pub completed_job_count: u64,
    pub failed_job_count: u64,
    pub cancelled_job_count: u64,
}

impl ExtractionStats {
    pub const API_VERSION: &str = "deskgraph.extraction-stats.v1";
}
