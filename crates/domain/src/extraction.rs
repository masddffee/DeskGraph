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

#[must_use]
pub fn is_valid_xlsx_cell_reference(value: &str) -> bool {
    let bytes = value.as_bytes();
    let column_end = bytes
        .iter()
        .position(|byte| !byte.is_ascii_uppercase())
        .unwrap_or(bytes.len());
    if !(1..=3).contains(&column_end) || column_end == bytes.len() {
        return false;
    }
    let mut column = 0_u32;
    for byte in &bytes[..column_end] {
        column = column
            .saturating_mul(26)
            .saturating_add(u32::from(*byte - b'A' + 1));
    }
    if column > 16_384 {
        return false;
    }
    let row_bytes = &bytes[column_end..];
    if row_bytes.first() == Some(&b'0') || !row_bytes.iter().all(u8::is_ascii_digit) {
        return false;
    }
    std::str::from_utf8(row_bytes)
        .ok()
        .and_then(|row| row.parse::<u32>().ok())
        .is_some_and(|row| (1..=1_048_576).contains(&row))
}

#[cfg(test)]
mod tests {
    use super::is_valid_xlsx_cell_reference;

    #[test]
    fn xlsx_cell_references_are_bounded_to_excel_grid() {
        for valid in ["A1", "Z9", "AA10", "XFD1048576"] {
            assert!(is_valid_xlsx_cell_reference(valid), "{valid}");
        }
        for invalid in [
            "", "A0", "A01", "a1", "$A$1", "XFE1", "A1048577", "AAAA1", "A1:B2",
        ] {
            assert!(!is_valid_xlsx_cell_reference(invalid), "{invalid}");
        }
    }
}
