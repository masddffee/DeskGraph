use serde::{Deserialize, Serialize};

pub const CORPUS_API_VERSION: &str = "deskgraph.ocr-corpus.v1";
pub const RUN_API_VERSION: &str = "deskgraph.ocr-run.v1";
pub const NORMALIZATION_VERSION: &str = "nfc_unicode_whitespace_v1";

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Corpus {
    pub api_version: String,
    pub corpus_id: String,
    pub normalization: String,
    pub cases: Vec<CorpusCase>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CorpusCase {
    pub case_id: String,
    pub image_sha256: String,
    pub expected_text: String,
    pub tags: Vec<CaseTag>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CaseTag {
    TraditionalChinese,
    English,
    MixedLanguage,
    NoText,
    SmallText,
    LowContrast,
    DarkMode,
    DenseUi,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ProviderRun {
    pub api_version: String,
    pub run_id: String,
    pub corpus_id: String,
    pub corpus_input_sha256: String,
    pub asset_manifest_input_sha256: String,
    pub text_reconstruction: TextReconstruction,
    pub provider: ProviderEvidence,
    pub host: HostEvidence,
    pub rss: RssEvidence,
    pub cases: Vec<RunCase>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TextReconstruction {
    ProviderObservationOrderNewlineJoinV1,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ProviderEvidence {
    pub provider_id: String,
    pub provider_version: String,
    pub runtime: ProviderRuntimeEvidence,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum ProviderRuntimeEvidence {
    OsManaged {
        runtime_revision: String,
    },
    Packaged {
        artifact_manifest_sha256: String,
        model_manifest_sha256: String,
    },
    Missing,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct HostEvidence {
    pub os: HostOs,
    pub os_version: String,
    pub arch: HostArch,
    pub cpu_model: String,
    pub ram_bytes: u64,
    pub rust_toolchain: String,
    pub deskgraph_commit: String,
    pub harness_id: String,
    pub harness_version: String,
    pub command: String,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum HostOs {
    Macos,
    Windows,
    Linux,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum HostArch {
    Aarch64,
    X86_64,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RssEvidence {
    pub scope: Option<RssScope>,
    pub before: Option<RssMeasurement>,
    pub peak: Option<RssMeasurement>,
    pub after_caller: Option<RssMeasurement>,
    pub after_cleanup: Option<RssMeasurement>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RssScope {
    WholeProcess,
    ProviderSidecarProcess,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RssMeasurement {
    pub bytes: u64,
    pub source: RssMeasurementSource,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RssMeasurementSource {
    MacosTimeL,
    WindowsProcessMemoryInfo,
    LinuxProcStatus,
    ExternalHarness,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RunCase {
    pub case_id: String,
    pub image_sha256: String,
    pub status: RunStatus,
    pub elapsed_us: u64,
    pub error_code: Option<String>,
    pub recognized_text: String,
    pub observations: Vec<RunObservation>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RunStatus {
    Completed,
    Failed,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RunObservation {
    pub text: String,
    pub bounding_box: BoundingBox,
    pub confidence_basis_points: Option<u32>,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct BoundingBox {
    pub x_ppm: u32,
    pub y_ppm: u32,
    pub width_ppm: u32,
    pub height_ppm: u32,
}

impl BoundingBox {
    pub(crate) fn is_valid(self) -> bool {
        const NORMALIZED_SCALE: u32 = 1_000_000;
        self.width_ppm > 0
            && self.height_ppm > 0
            && self
                .x_ppm
                .checked_add(self.width_ppm)
                .is_some_and(|right| right <= NORMALIZED_SCALE)
            && self
                .y_ppm
                .checked_add(self.height_ppm)
                .is_some_and(|bottom| bottom <= NORMALIZED_SCALE)
    }
}
