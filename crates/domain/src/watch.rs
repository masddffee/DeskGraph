use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WatchEventStatus {
    Stabilizing,
    Reconciling,
    Completed,
    Ignored,
    Failed,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WatchEventReason {
    TemporaryDownload,
    HiddenEntry,
    UnsupportedEntry,
    SourceUnavailable,
    ReconcileFailed,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct WatchEventProgress {
    pub api_version: &'static str,
    pub event_id: i64,
    pub scope_id: i64,
    pub status: WatchEventStatus,
    pub observation_count: u64,
    pub stable_after_unix_ms: i64,
    pub scan_job_id: Option<i64>,
    pub reason: Option<WatchEventReason>,
}

impl WatchEventProgress {
    pub const API_VERSION: &str = "deskgraph.watch-event.v1";

    pub fn is_terminal(&self) -> bool {
        matches!(
            self.status,
            WatchEventStatus::Completed | WatchEventStatus::Ignored | WatchEventStatus::Failed
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn watch_progress_is_path_free_and_uses_closed_states() {
        let progress = WatchEventProgress {
            api_version: WatchEventProgress::API_VERSION,
            event_id: 1,
            scope_id: 2,
            status: WatchEventStatus::Ignored,
            observation_count: 1,
            stable_after_unix_ms: 3,
            scan_job_id: None,
            reason: Some(WatchEventReason::TemporaryDownload),
        };
        let value = serde_json::to_value(progress).expect("watch progress should serialize");
        assert_eq!(value["api_version"], "deskgraph.watch-event.v1");
        assert_eq!(value["status"], "ignored");
        assert_eq!(value["reason"], "temporary_download");
        assert!(value.get("path").is_none());
    }
}
