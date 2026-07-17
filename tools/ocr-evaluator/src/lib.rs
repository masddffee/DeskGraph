use std::collections::{BTreeMap, HashMap, HashSet};
use std::fs::File;
use std::io::Read;
use std::path::Path;

use serde::{Deserialize, Serialize, de::DeserializeOwned};
use sha2::{Digest, Sha256};
use unicode_normalization::UnicodeNormalization;

const CORPUS_API_VERSION: &str = "deskgraph.ocr-corpus.v1";
const RUN_API_VERSION: &str = "deskgraph.ocr-run.v1";
const REPORT_API_VERSION: &str = "deskgraph.ocr-evaluation-report.v1";
const NORMALIZATION_VERSION: &str = "nfc_unicode_whitespace_v1";
const MAX_INPUT_BYTES: u64 = 64 * 1024 * 1024;
const MAX_CASES: usize = 1_000;
const MAX_OBSERVATIONS_PER_CASE: usize = 4_096;
const MAX_TOTAL_OBSERVATIONS: usize = 100_000;
const MAX_TEXT_BYTES_PER_CASE: usize = 256 * 1024;
const MAX_TEXT_UNITS_PER_CASE: usize = 16_384;
const MAX_TOTAL_TEXT_BYTES: usize = 32 * 1024 * 1024;
const MAX_EDIT_CELLS: u64 = 100_000_000;
const NORMALIZED_SCALE: u32 = 1_000_000;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Corpus {
    api_version: String,
    corpus_id: String,
    normalization: String,
    cases: Vec<CorpusCase>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct CorpusCase {
    case_id: String,
    image_sha256: String,
    expected_text: String,
    tags: Vec<CaseTag>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
enum CaseTag {
    TraditionalChinese,
    English,
    MixedLanguage,
    NoText,
    SmallText,
    LowContrast,
    DarkMode,
    DenseUi,
}

const ALL_CASE_TAGS: [CaseTag; 8] = [
    CaseTag::TraditionalChinese,
    CaseTag::English,
    CaseTag::MixedLanguage,
    CaseTag::NoText,
    CaseTag::SmallText,
    CaseTag::LowContrast,
    CaseTag::DarkMode,
    CaseTag::DenseUi,
];

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ProviderRun {
    api_version: String,
    run_id: String,
    corpus_id: String,
    corpus_input_sha256: String,
    provider: ProviderEvidence,
    host: HostEvidence,
    rss: RssEvidence,
    cases: Vec<RunCase>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ProviderEvidence {
    provider_id: String,
    provider_version: String,
    runtime: ProviderRuntimeEvidence,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
enum ProviderRuntimeEvidence {
    OsManaged {
        runtime_revision: String,
    },
    Packaged {
        artifact_manifest_sha256: String,
        model_manifest_sha256: String,
    },
    Missing,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct HostEvidence {
    os: HostOs,
    os_version: String,
    arch: HostArch,
    cpu_model: String,
    ram_bytes: u64,
    rust_toolchain: String,
    deskgraph_commit: String,
    harness_id: String,
    harness_version: String,
    command: String,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
enum HostOs {
    Macos,
    Windows,
    Linux,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
enum HostArch {
    Aarch64,
    X86_64,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RssEvidence {
    scope: Option<RssScope>,
    before: Option<RssMeasurement>,
    peak: Option<RssMeasurement>,
    after_caller: Option<RssMeasurement>,
    after_cleanup: Option<RssMeasurement>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
enum RssScope {
    WholeProcess,
    ProviderSidecarProcess,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct RssMeasurement {
    bytes: u64,
    source: RssMeasurementSource,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
enum RssMeasurementSource {
    MacosTimeL,
    WindowsProcessMemoryInfo,
    LinuxProcStatus,
    ExternalHarness,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RunCase {
    case_id: String,
    image_sha256: String,
    status: RunStatus,
    elapsed_us: u64,
    error_code: Option<String>,
    recognized_text: String,
    observations: Vec<RunObservation>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
enum RunStatus {
    Completed,
    Failed,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RunObservation {
    text: String,
    bounding_box: BoundingBox,
    confidence_basis_points: Option<u32>,
}

#[derive(Clone, Copy, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct BoundingBox {
    x_ppm: u32,
    y_ppm: u32,
    width_ppm: u32,
    height_ppm: u32,
}

impl BoundingBox {
    fn is_valid(self) -> bool {
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

#[derive(Debug, Serialize)]
pub struct EvaluationReport {
    api_version: &'static str,
    corpus_id: String,
    corpus_input_sha256: String,
    run_input_sha256: String,
    run_id: String,
    provider_id: String,
    provider_version: String,
    provider_runtime: ProviderRuntimeEvidence,
    host: ReportHost,
    case_count: u64,
    completed_cases: u64,
    failed_cases: u64,
    failure_codes: BTreeMap<String, u64>,
    micro_cer: ErrorMetric,
    micro_whitespace_token_error_rate: ErrorMetric,
    no_text_cases: u64,
    no_text_false_positive_cases: u64,
    case_slices: Vec<CaseSliceReport>,
    observations: ObservationReport,
    attempt_latency: LatencyReport,
    completed_latency: LatencyReport,
    reported_rss: ReportedRss,
    missing_evidence: Vec<&'static str>,
}

#[derive(Debug, Serialize)]
struct ReportHost {
    os: HostOs,
    os_version: String,
    arch: HostArch,
    cpu_model: String,
    ram_bytes: u64,
    rust_toolchain: String,
    deskgraph_commit: String,
    harness_id: String,
    harness_version: String,
    command_sha256: String,
}

#[derive(Debug, Serialize)]
struct ErrorMetric {
    edits: u64,
    reference_units: u64,
    rate_ppm: Option<u64>,
}

#[derive(Debug, Serialize)]
struct CaseSliceReport {
    tag: CaseTag,
    case_count: u64,
    completed_cases: u64,
    failed_cases: u64,
    failure_codes: BTreeMap<String, u64>,
    micro_cer: ErrorMetric,
    micro_whitespace_token_error_rate: ErrorMetric,
    no_text_false_positive_cases: u64,
    attempt_latency: LatencyReport,
    completed_latency: LatencyReport,
}

#[derive(Debug, Default)]
struct CaseSliceMetrics {
    case_count: u64,
    completed_cases: u64,
    failed_cases: u64,
    character_edits: u64,
    character_reference_units: u64,
    word_edits: u64,
    word_reference_units: u64,
    no_text_false_positive_cases: u64,
    failure_codes: BTreeMap<String, u64>,
    attempt_latencies: Vec<u64>,
    completed_latencies: Vec<u64>,
}

#[derive(Clone, Copy, Debug)]
struct CaseMetricSample<'a> {
    status: RunStatus,
    character_edits: u64,
    character_reference_units: u64,
    word_edits: u64,
    word_reference_units: u64,
    no_text_false_positive: bool,
    elapsed_us: u64,
    error_code: Option<&'a str>,
}

#[derive(Debug, Serialize)]
struct ObservationReport {
    total: u64,
    valid_boxes: u64,
    invalid_boxes: u64,
    empty_text: u64,
    invalid_confidence: u64,
}

#[derive(Debug, Serialize)]
struct LatencyReport {
    sample_count: u64,
    p50_us: u64,
    p95_us: u64,
    max_us: u64,
}

#[derive(Debug, Serialize)]
struct ReportedRss {
    scope: Option<RssScope>,
    before: Option<RssMeasurement>,
    peak: Option<RssMeasurement>,
    after_caller: Option<RssMeasurement>,
    after_cleanup: Option<RssMeasurement>,
}

pub fn evaluate_paths(
    corpus_path: &Path,
    run_path: &Path,
) -> Result<EvaluationReport, &'static str> {
    let corpus = read_json_bounded(corpus_path, "ocr_corpus_read_failed")?;
    let run = read_json_bounded(run_path, "ocr_run_read_failed")?;
    evaluate(
        corpus.value,
        run.value,
        corpus.input_sha256,
        run.input_sha256,
    )
}

struct ParsedInput<T> {
    value: T,
    input_sha256: String,
}

fn read_json_bounded<T: DeserializeOwned>(
    path: &Path,
    read_error: &'static str,
) -> Result<ParsedInput<T>, &'static str> {
    let file = File::open(path).map_err(|_| read_error)?;
    let metadata = file.metadata().map_err(|_| read_error)?;
    if !metadata.is_file() || metadata.len() == 0 || metadata.len() > MAX_INPUT_BYTES {
        return Err("ocr_evaluation_input_size_invalid");
    }
    let mut bytes = Vec::with_capacity(
        usize::try_from(metadata.len())
            .map_err(|_| "ocr_evaluation_input_size_invalid")?
            .min(1024 * 1024),
    );
    file.take(MAX_INPUT_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| read_error)?;
    if bytes.is_empty() || u64::try_from(bytes.len()).unwrap_or(u64::MAX) > MAX_INPUT_BYTES {
        return Err("ocr_evaluation_input_size_invalid");
    }
    let value = serde_json::from_slice(&bytes).map_err(|_| "ocr_evaluation_json_invalid")?;
    Ok(ParsedInput {
        value,
        input_sha256: sha256_hex(&bytes),
    })
}

fn evaluate(
    corpus: Corpus,
    run: ProviderRun,
    corpus_input_sha256: String,
    run_input_sha256: String,
) -> Result<EvaluationReport, &'static str> {
    validate_top_level(&corpus, &run, &corpus_input_sha256)?;
    let corpus_cases = validate_corpus(&corpus)?;
    validate_run(&run, &corpus_cases)?;

    let mut character_edits = 0_u64;
    let mut character_reference_units = 0_u64;
    let mut word_edits = 0_u64;
    let mut word_reference_units = 0_u64;
    let mut edit_cells = 0_u64;
    let mut completed_cases = 0_u64;
    let mut failed_cases = 0_u64;
    let mut no_text_cases = 0_u64;
    let mut no_text_false_positive_cases = 0_u64;
    let mut total_observations = 0_u64;
    let mut valid_boxes = 0_u64;
    let mut invalid_boxes = 0_u64;
    let mut empty_text = 0_u64;
    let mut invalid_confidence = 0_u64;
    let mut failure_codes = BTreeMap::new();
    let mut attempt_latencies = Vec::with_capacity(run.cases.len());
    let mut completed_latencies = Vec::with_capacity(run.cases.len());
    let mut case_slice_metrics = HashMap::<CaseTag, CaseSliceMetrics>::new();

    for result in &run.cases {
        let expected = corpus_cases
            .get(result.case_id.as_str())
            .ok_or("ocr_run_case_unknown")?;
        let expected_text = normalize_text(&expected.expected_text);
        let recognized_text = if result.status == RunStatus::Completed {
            normalize_text(&result.recognized_text)
        } else {
            String::new()
        };

        let expected_characters: Vec<char> = expected_text.chars().collect();
        let recognized_characters: Vec<char> = recognized_text.chars().collect();
        validate_unit_count(expected_characters.len(), recognized_characters.len())?;
        add_edit_cells(
            &mut edit_cells,
            expected_characters.len(),
            recognized_characters.len(),
        )?;
        let case_character_edits =
            u64::try_from(levenshtein(&expected_characters, &recognized_characters))
                .map_err(|_| "ocr_evaluation_metric_overflow")?;
        character_edits = character_edits
            .checked_add(case_character_edits)
            .ok_or("ocr_evaluation_metric_overflow")?;
        let case_character_reference_units = u64::try_from(expected_characters.len())
            .map_err(|_| "ocr_evaluation_metric_overflow")?;
        character_reference_units = character_reference_units
            .checked_add(case_character_reference_units)
            .ok_or("ocr_evaluation_metric_overflow")?;

        let expected_words: Vec<&str> = expected_text.split_whitespace().collect();
        let recognized_words: Vec<&str> = recognized_text.split_whitespace().collect();
        add_edit_cells(
            &mut edit_cells,
            expected_words.len(),
            recognized_words.len(),
        )?;
        let case_word_edits = u64::try_from(levenshtein(&expected_words, &recognized_words))
            .map_err(|_| "ocr_evaluation_metric_overflow")?;
        word_edits = word_edits
            .checked_add(case_word_edits)
            .ok_or("ocr_evaluation_metric_overflow")?;
        let case_word_reference_units =
            u64::try_from(expected_words.len()).map_err(|_| "ocr_evaluation_metric_overflow")?;
        word_reference_units = word_reference_units
            .checked_add(case_word_reference_units)
            .ok_or("ocr_evaluation_metric_overflow")?;

        let mut no_text_false_positive = false;
        if expected_text.is_empty() {
            no_text_cases = no_text_cases
                .checked_add(1)
                .ok_or("ocr_evaluation_metric_overflow")?;
            if !recognized_text.is_empty() {
                no_text_false_positive = true;
                no_text_false_positive_cases = no_text_false_positive_cases
                    .checked_add(1)
                    .ok_or("ocr_evaluation_metric_overflow")?;
            }
        }

        match result.status {
            RunStatus::Completed => {
                completed_cases = checked_add(completed_cases, 1)?;
                completed_latencies.push(result.elapsed_us);
            }
            RunStatus::Failed => {
                failed_cases = checked_add(failed_cases, 1)?;
                increment_failure_code(
                    &mut failure_codes,
                    result
                        .error_code
                        .as_deref()
                        .ok_or("ocr_run_case_contract_invalid")?,
                )?;
            }
        }
        attempt_latencies.push(result.elapsed_us);
        for tag in &expected.tags {
            case_slice_metrics
                .entry(*tag)
                .or_default()
                .record(CaseMetricSample {
                    status: result.status,
                    character_edits: case_character_edits,
                    character_reference_units: case_character_reference_units,
                    word_edits: case_word_edits,
                    word_reference_units: case_word_reference_units,
                    no_text_false_positive,
                    elapsed_us: result.elapsed_us,
                    error_code: result.error_code.as_deref(),
                })?;
        }
        for observation in &result.observations {
            total_observations = total_observations
                .checked_add(1)
                .ok_or("ocr_evaluation_metric_overflow")?;
            if observation.bounding_box.is_valid() {
                valid_boxes += 1;
            } else {
                invalid_boxes += 1;
            }
            if observation.text.is_empty() {
                empty_text += 1;
            }
            if observation
                .confidence_basis_points
                .is_some_and(|confidence| confidence > 10_000)
            {
                invalid_confidence += 1;
            }
        }
    }

    let missing_evidence = missing_evidence(&run);
    let mut case_slices = Vec::with_capacity(case_slice_metrics.len());
    for tag in ALL_CASE_TAGS {
        if let Some(metrics) = case_slice_metrics.remove(&tag) {
            case_slices.push(metrics.into_report(tag)?);
        }
    }
    let command_sha256 = sha256_hex(run.host.command.as_bytes());
    Ok(EvaluationReport {
        api_version: REPORT_API_VERSION,
        corpus_id: corpus.corpus_id,
        corpus_input_sha256,
        run_input_sha256,
        run_id: run.run_id,
        provider_id: run.provider.provider_id,
        provider_version: run.provider.provider_version,
        provider_runtime: run.provider.runtime,
        host: ReportHost {
            os: run.host.os,
            os_version: run.host.os_version,
            arch: run.host.arch,
            cpu_model: run.host.cpu_model,
            ram_bytes: run.host.ram_bytes,
            rust_toolchain: run.host.rust_toolchain,
            deskgraph_commit: run.host.deskgraph_commit,
            harness_id: run.host.harness_id,
            harness_version: run.host.harness_version,
            command_sha256,
        },
        case_count: u64::try_from(run.cases.len()).map_err(|_| "ocr_evaluation_metric_overflow")?,
        completed_cases,
        failed_cases,
        failure_codes,
        micro_cer: error_metric(character_edits, character_reference_units),
        micro_whitespace_token_error_rate: error_metric(word_edits, word_reference_units),
        no_text_cases,
        no_text_false_positive_cases,
        case_slices,
        observations: ObservationReport {
            total: total_observations,
            valid_boxes,
            invalid_boxes,
            empty_text,
            invalid_confidence,
        },
        attempt_latency: latency_report(&mut attempt_latencies)?,
        completed_latency: latency_report(&mut completed_latencies)?,
        reported_rss: ReportedRss {
            scope: run.rss.scope,
            before: run.rss.before,
            peak: run.rss.peak,
            after_caller: run.rss.after_caller,
            after_cleanup: run.rss.after_cleanup,
        },
        missing_evidence,
    })
}

impl CaseSliceMetrics {
    fn record(&mut self, sample: CaseMetricSample<'_>) -> Result<(), &'static str> {
        self.case_count = checked_add(self.case_count, 1)?;
        match sample.status {
            RunStatus::Completed => {
                self.completed_cases = checked_add(self.completed_cases, 1)?;
                self.completed_latencies.push(sample.elapsed_us);
            }
            RunStatus::Failed => {
                self.failed_cases = checked_add(self.failed_cases, 1)?;
                increment_failure_code(
                    &mut self.failure_codes,
                    sample.error_code.ok_or("ocr_run_case_contract_invalid")?,
                )?;
            }
        }
        self.character_edits = checked_add(self.character_edits, sample.character_edits)?;
        self.character_reference_units = checked_add(
            self.character_reference_units,
            sample.character_reference_units,
        )?;
        self.word_edits = checked_add(self.word_edits, sample.word_edits)?;
        self.word_reference_units =
            checked_add(self.word_reference_units, sample.word_reference_units)?;
        if sample.no_text_false_positive {
            self.no_text_false_positive_cases = checked_add(self.no_text_false_positive_cases, 1)?;
        }
        self.attempt_latencies.push(sample.elapsed_us);
        Ok(())
    }

    fn into_report(mut self, tag: CaseTag) -> Result<CaseSliceReport, &'static str> {
        let attempt_latency = latency_report(&mut self.attempt_latencies)?;
        let completed_latency = latency_report(&mut self.completed_latencies)?;
        Ok(CaseSliceReport {
            tag,
            case_count: self.case_count,
            completed_cases: self.completed_cases,
            failed_cases: self.failed_cases,
            failure_codes: self.failure_codes,
            micro_cer: error_metric(self.character_edits, self.character_reference_units),
            micro_whitespace_token_error_rate: error_metric(
                self.word_edits,
                self.word_reference_units,
            ),
            no_text_false_positive_cases: self.no_text_false_positive_cases,
            attempt_latency,
            completed_latency,
        })
    }
}

fn checked_add(left: u64, right: u64) -> Result<u64, &'static str> {
    left.checked_add(right)
        .ok_or("ocr_evaluation_metric_overflow")
}

fn increment_failure_code(
    failure_codes: &mut BTreeMap<String, u64>,
    code: &str,
) -> Result<(), &'static str> {
    let count = failure_codes.entry(code.to_owned()).or_default();
    *count = checked_add(*count, 1)?;
    Ok(())
}

fn latency_report(samples: &mut [u64]) -> Result<LatencyReport, &'static str> {
    samples.sort_unstable();
    Ok(LatencyReport {
        sample_count: u64::try_from(samples.len()).map_err(|_| "ocr_evaluation_metric_overflow")?,
        p50_us: percentile(samples, 50),
        p95_us: percentile(samples, 95),
        max_us: samples.last().copied().unwrap_or(0),
    })
}

fn validate_top_level(
    corpus: &Corpus,
    run: &ProviderRun,
    corpus_input_sha256: &str,
) -> Result<(), &'static str> {
    if corpus.api_version != CORPUS_API_VERSION
        || run.api_version != RUN_API_VERSION
        || corpus.normalization != NORMALIZATION_VERSION
    {
        return Err("ocr_evaluation_api_version_unsupported");
    }
    validate_identifier(&corpus.corpus_id)?;
    validate_identifier(&run.run_id)?;
    if corpus.corpus_id != run.corpus_id {
        return Err("ocr_evaluation_corpus_mismatch");
    }
    validate_sha256(corpus_input_sha256)?;
    validate_sha256(&run.corpus_input_sha256)?;
    if corpus_input_sha256 != run.corpus_input_sha256 {
        return Err("ocr_evaluation_corpus_digest_mismatch");
    }
    validate_identifier(&run.provider.provider_id)?;
    validate_identifier(&run.provider.provider_version)?;
    match &run.provider.runtime {
        ProviderRuntimeEvidence::OsManaged { runtime_revision } => {
            if !is_safe_report_text(runtime_revision, 128) {
                return Err("ocr_run_provider_evidence_invalid");
            }
        }
        ProviderRuntimeEvidence::Packaged {
            artifact_manifest_sha256,
            model_manifest_sha256,
        } => {
            validate_sha256(artifact_manifest_sha256)?;
            validate_sha256(model_manifest_sha256)?;
        }
        ProviderRuntimeEvidence::Missing => {}
    }
    if run.host.ram_bytes == 0
        || !is_safe_report_text(&run.host.os_version, 128)
        || !is_safe_report_text(&run.host.cpu_model, 128)
        || !is_safe_report_text(&run.host.rust_toolchain, 128)
        || !is_lower_hex(&run.host.deskgraph_commit, 40)
        || validate_identifier(&run.host.harness_id).is_err()
        || validate_identifier(&run.host.harness_version).is_err()
        || !is_bounded_text(&run.host.command, 4_096)
    {
        return Err("ocr_run_host_evidence_invalid");
    }
    validate_rss(&run.rss, run.host.ram_bytes, run.host.os)
}

fn validate_corpus(corpus: &Corpus) -> Result<HashMap<&str, &CorpusCase>, &'static str> {
    if corpus.cases.is_empty() || corpus.cases.len() > MAX_CASES {
        return Err("ocr_corpus_case_count_invalid");
    }
    let mut total_text_bytes = 0_usize;
    let mut cases = HashMap::with_capacity(corpus.cases.len());
    for case in &corpus.cases {
        validate_identifier(&case.case_id)?;
        validate_sha256(&case.image_sha256)?;
        if case.expected_text.len() > MAX_TEXT_BYTES_PER_CASE {
            return Err("ocr_corpus_text_limit_exceeded");
        }
        total_text_bytes = total_text_bytes
            .checked_add(case.expected_text.len())
            .ok_or("ocr_corpus_text_limit_exceeded")?;
        if total_text_bytes > MAX_TOTAL_TEXT_BYTES {
            return Err("ocr_corpus_text_limit_exceeded");
        }
        if case.tags.is_empty() {
            return Err("ocr_corpus_tags_invalid");
        }
        if case.tags.iter().copied().collect::<HashSet<_>>().len() != case.tags.len() {
            return Err("ocr_corpus_tags_invalid");
        }
        let normalized = normalize_text(&case.expected_text);
        let has_no_text = case.tags.contains(&CaseTag::NoText);
        if normalized.is_empty() != has_no_text {
            return Err("ocr_corpus_no_text_tag_mismatch");
        }
        let language_tags = [
            CaseTag::TraditionalChinese,
            CaseTag::English,
            CaseTag::MixedLanguage,
        ]
        .into_iter()
        .filter(|tag| case.tags.contains(tag))
        .count();
        if (!has_no_text && language_tags != 1) || (has_no_text && language_tags != 0) {
            return Err("ocr_corpus_tags_invalid");
        }
        if cases.insert(case.case_id.as_str(), case).is_some() {
            return Err("ocr_corpus_case_duplicate");
        }
    }
    Ok(cases)
}

fn validate_run(
    run: &ProviderRun,
    corpus_cases: &HashMap<&str, &CorpusCase>,
) -> Result<(), &'static str> {
    if run.cases.len() != corpus_cases.len() || run.cases.len() > MAX_CASES {
        return Err("ocr_run_case_set_mismatch");
    }
    let mut seen = HashSet::with_capacity(run.cases.len());
    let mut total_observations = 0_usize;
    let mut total_text_bytes = 0_usize;
    for case in &run.cases {
        validate_identifier(&case.case_id)?;
        let expected = corpus_cases
            .get(case.case_id.as_str())
            .ok_or("ocr_run_case_unknown")?;
        if !seen.insert(case.case_id.as_str()) {
            return Err("ocr_run_case_duplicate");
        }
        validate_sha256(&case.image_sha256)?;
        if case.image_sha256 != expected.image_sha256 {
            return Err("ocr_run_image_checksum_mismatch");
        }
        if case.elapsed_us == 0 {
            return Err("ocr_run_latency_invalid");
        }
        if case.recognized_text.len() > MAX_TEXT_BYTES_PER_CASE {
            return Err("ocr_run_text_limit_exceeded");
        }
        match case.status {
            RunStatus::Completed
                if case.error_code.is_some()
                    || case.observations.len() > MAX_OBSERVATIONS_PER_CASE =>
            {
                return Err("ocr_run_case_contract_invalid");
            }
            RunStatus::Failed
                if case
                    .error_code
                    .as_deref()
                    .is_none_or(|code| !is_safe_code(code))
                    || !case.recognized_text.is_empty()
                    || !case.observations.is_empty() =>
            {
                return Err("ocr_run_case_contract_invalid");
            }
            RunStatus::Completed | RunStatus::Failed => {}
        }
        total_text_bytes = total_text_bytes
            .checked_add(case.recognized_text.len())
            .ok_or("ocr_run_text_limit_exceeded")?;
        if total_text_bytes > MAX_TOTAL_TEXT_BYTES {
            return Err("ocr_run_text_limit_exceeded");
        }
        total_observations = total_observations
            .checked_add(case.observations.len())
            .ok_or("ocr_run_observation_limit_exceeded")?;
        if total_observations > MAX_TOTAL_OBSERVATIONS {
            return Err("ocr_run_observation_limit_exceeded");
        }
        for observation in &case.observations {
            if observation.text.len() > MAX_TEXT_BYTES_PER_CASE {
                return Err("ocr_run_text_limit_exceeded");
            }
            total_text_bytes = total_text_bytes
                .checked_add(observation.text.len())
                .ok_or("ocr_run_text_limit_exceeded")?;
            if total_text_bytes > MAX_TOTAL_TEXT_BYTES {
                return Err("ocr_run_text_limit_exceeded");
            }
        }
    }
    if seen.len() != corpus_cases.len() {
        return Err("ocr_run_case_set_mismatch");
    }
    Ok(())
}

fn validate_rss(
    rss: &RssEvidence,
    host_ram_bytes: u64,
    host_os: HostOs,
) -> Result<(), &'static str> {
    let measurements = [rss.before, rss.peak, rss.after_caller, rss.after_cleanup];
    let has_measurement = measurements.iter().any(Option::is_some);
    if has_measurement != rss.scope.is_some() {
        return Err("ocr_run_rss_evidence_invalid");
    }
    for measurement in [rss.before, rss.after_caller, rss.after_cleanup]
        .into_iter()
        .flatten()
    {
        validate_rss_measurement(measurement, host_ram_bytes, host_os, false)?;
    }
    if let Some(peak) = rss.peak {
        validate_rss_measurement(peak, host_ram_bytes, host_os, true)?;
    }
    if let Some(peak) = rss.peak
        && [rss.before, rss.after_caller, rss.after_cleanup]
            .into_iter()
            .flatten()
            .any(|measurement| measurement.bytes > peak.bytes)
    {
        return Err("ocr_run_rss_evidence_invalid");
    }
    Ok(())
}

fn validate_rss_measurement(
    measurement: RssMeasurement,
    host_ram_bytes: u64,
    host_os: HostOs,
    is_peak: bool,
) -> Result<(), &'static str> {
    if measurement.bytes == 0 || measurement.bytes > host_ram_bytes {
        return Err("ocr_run_rss_evidence_invalid");
    }
    let source_matches_host = match measurement.source {
        RssMeasurementSource::MacosTimeL => is_peak && matches!(host_os, HostOs::Macos),
        RssMeasurementSource::WindowsProcessMemoryInfo => matches!(host_os, HostOs::Windows),
        RssMeasurementSource::LinuxProcStatus => matches!(host_os, HostOs::Linux),
        RssMeasurementSource::ExternalHarness => true,
    };
    if !source_matches_host {
        return Err("ocr_run_rss_evidence_invalid");
    }
    Ok(())
}

fn validate_identifier(value: &str) -> Result<(), &'static str> {
    if value.is_empty()
        || value.len() > 128
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-'))
    {
        return Err("ocr_evaluation_identifier_invalid");
    }
    Ok(())
}

fn validate_sha256(value: &str) -> Result<(), &'static str> {
    if !is_lower_hex(value, 64) {
        return Err("ocr_evaluation_sha256_invalid");
    }
    Ok(())
}

fn is_lower_hex(value: &str, expected_len: usize) -> bool {
    value.len() == expected_len
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn is_bounded_text(value: &str, max_bytes: usize) -> bool {
    !value.is_empty()
        && value.len() <= max_bytes
        && !value
            .chars()
            .any(|character| character.is_control() && !matches!(character, '\n' | '\r' | '\t'))
}

fn is_safe_report_text(value: &str, max_bytes: usize) -> bool {
    !value.is_empty()
        && value.len() <= max_bytes
        && value.bytes().all(|byte| {
            byte.is_ascii_alphanumeric()
                || matches!(byte, b' ' | b'.' | b'+' | b'-' | b'_' | b'(' | b')')
        })
}

fn is_safe_code(value: &str) -> bool {
    validate_identifier(value).is_ok()
}

fn normalize_text(input: &str) -> String {
    let mut output = String::with_capacity(input.len());
    let mut pending_space = false;
    for character in input.nfc() {
        if character.is_whitespace() {
            pending_space = !output.is_empty();
        } else {
            if pending_space {
                output.push(' ');
                pending_space = false;
            }
            output.push(character);
        }
    }
    output
}

fn validate_unit_count(reference: usize, hypothesis: usize) -> Result<(), &'static str> {
    if reference > MAX_TEXT_UNITS_PER_CASE || hypothesis > MAX_TEXT_UNITS_PER_CASE {
        return Err("ocr_evaluation_text_unit_limit_exceeded");
    }
    Ok(())
}

fn add_edit_cells(
    total: &mut u64,
    reference: usize,
    hypothesis: usize,
) -> Result<(), &'static str> {
    let cells = u64::try_from(reference.saturating_add(1))
        .ok()
        .and_then(|reference| {
            u64::try_from(hypothesis.saturating_add(1))
                .ok()
                .and_then(|hypothesis| reference.checked_mul(hypothesis))
        })
        .ok_or("ocr_evaluation_metric_limit_exceeded")?;
    *total = total
        .checked_add(cells)
        .ok_or("ocr_evaluation_metric_limit_exceeded")?;
    if *total > MAX_EDIT_CELLS {
        return Err("ocr_evaluation_metric_limit_exceeded");
    }
    Ok(())
}

fn levenshtein<T: Eq>(reference: &[T], hypothesis: &[T]) -> usize {
    if reference.is_empty() {
        return hypothesis.len();
    }
    if hypothesis.is_empty() {
        return reference.len();
    }
    let (rows, columns) = if reference.len() >= hypothesis.len() {
        (reference, hypothesis)
    } else {
        (hypothesis, reference)
    };
    let mut previous: Vec<usize> = (0..=columns.len()).collect();
    let mut current = vec![0; columns.len() + 1];
    for (row_index, row_value) in rows.iter().enumerate() {
        current[0] = row_index + 1;
        for (column_index, column_value) in columns.iter().enumerate() {
            let substitution = previous[column_index] + usize::from(row_value != column_value);
            let insertion = current[column_index] + 1;
            let deletion = previous[column_index + 1] + 1;
            current[column_index + 1] = substitution.min(insertion).min(deletion);
        }
        std::mem::swap(&mut previous, &mut current);
    }
    previous[columns.len()]
}

fn error_metric(edits: u64, reference_units: u64) -> ErrorMetric {
    ErrorMetric {
        edits,
        reference_units,
        rate_ppm: (reference_units > 0).then(|| {
            u64::try_from(
                u128::from(edits)
                    .saturating_mul(1_000_000)
                    .checked_div(u128::from(reference_units))
                    .unwrap_or(0),
            )
            .unwrap_or(u64::MAX)
        }),
    }
}

fn percentile(sorted_samples: &[u64], percentile: usize) -> u64 {
    if sorted_samples.is_empty() {
        return 0;
    }
    let rank = sorted_samples
        .len()
        .saturating_mul(percentile)
        .saturating_add(99)
        / 100;
    sorted_samples[rank.saturating_sub(1).min(sorted_samples.len() - 1)]
}

fn sha256_hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let digest = Sha256::digest(bytes);
    let mut output = String::with_capacity(64);
    for byte in digest {
        output.push(HEX[usize::from(byte >> 4)] as char);
        output.push(HEX[usize::from(byte & 0x0f)] as char);
    }
    output
}

fn missing_evidence(run: &ProviderRun) -> Vec<&'static str> {
    let mut missing = Vec::new();
    if matches!(&run.provider.runtime, ProviderRuntimeEvidence::Missing) {
        missing.push("provider_runtime_manifest");
    }
    if run.rss.before.is_none() {
        missing.push("rss_before");
    }
    if run.rss.peak.is_none() {
        missing.push("rss_peak");
    }
    if run.rss.after_caller.is_none() {
        missing.push("rss_after_caller");
    }
    if run.rss.after_cleanup.is_none() {
        missing.push("rss_after_cleanup");
    }
    missing.extend([
        "cancellation_e2e",
        "packaging_targets",
        "spatial_accuracy_ground_truth",
    ]);
    missing
}

#[cfg(test)]
mod tests {
    use super::*;

    const SHA_A: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
    const SHA_B: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
    const CORPUS_SHA: &str = "cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc";
    const RUN_SHA: &str = "dddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddd";
    const COMMIT_SHA: &str = "eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee";

    fn corpus() -> Corpus {
        Corpus {
            api_version: CORPUS_API_VERSION.to_owned(),
            corpus_id: "mixed-v1".to_owned(),
            normalization: NORMALIZATION_VERSION.to_owned(),
            cases: vec![
                CorpusCase {
                    case_id: "mixed".to_owned(),
                    image_sha256: SHA_A.to_owned(),
                    expected_text: "桌面圖譜\nCafe\u{301}".to_owned(),
                    tags: vec![CaseTag::MixedLanguage],
                },
                CorpusCase {
                    case_id: "empty".to_owned(),
                    image_sha256: SHA_B.to_owned(),
                    expected_text: " \n".to_owned(),
                    tags: vec![CaseTag::NoText],
                },
            ],
        }
    }

    fn run() -> ProviderRun {
        ProviderRun {
            api_version: RUN_API_VERSION.to_owned(),
            run_id: "native-macos-1".to_owned(),
            corpus_id: "mixed-v1".to_owned(),
            corpus_input_sha256: CORPUS_SHA.to_owned(),
            provider: ProviderEvidence {
                provider_id: "deskgraph.apple-vision".to_owned(),
                provider_version: "1".to_owned(),
                runtime: ProviderRuntimeEvidence::OsManaged {
                    runtime_revision: "macOS Vision 15.5".to_owned(),
                },
            },
            host: HostEvidence {
                os: HostOs::Macos,
                os_version: "macOS 15.5".to_owned(),
                arch: HostArch::Aarch64,
                cpu_model: "Apple M4".to_owned(),
                ram_bytes: 8 * 1024 * 1024 * 1024,
                rust_toolchain: "rustc 1.97.0".to_owned(),
                deskgraph_commit: COMMIT_SHA.to_owned(),
                harness_id: "deskgraph-ocr-harness".to_owned(),
                harness_version: "1".to_owned(),
                command: "platform-harness controlled-corpus".to_owned(),
            },
            rss: RssEvidence {
                scope: None,
                before: None,
                peak: None,
                after_caller: None,
                after_cleanup: None,
            },
            cases: vec![
                RunCase {
                    case_id: "mixed".to_owned(),
                    image_sha256: SHA_A.to_owned(),
                    status: RunStatus::Completed,
                    elapsed_us: 1_000,
                    error_code: None,
                    recognized_text: "桌面圖譜 Café".to_owned(),
                    observations: vec![
                        observation("桌面圖譜", valid_box()),
                        observation("Café", valid_box()),
                    ],
                },
                RunCase {
                    case_id: "empty".to_owned(),
                    image_sha256: SHA_B.to_owned(),
                    status: RunStatus::Completed,
                    elapsed_us: 2_000,
                    error_code: None,
                    recognized_text: String::new(),
                    observations: Vec::new(),
                },
            ],
        }
    }

    fn evaluate_fixture(
        corpus: Corpus,
        run: ProviderRun,
    ) -> Result<EvaluationReport, &'static str> {
        evaluate(corpus, run, CORPUS_SHA.to_owned(), RUN_SHA.to_owned())
    }

    fn observation(text: &str, bounding_box: BoundingBox) -> RunObservation {
        RunObservation {
            text: text.to_owned(),
            bounding_box,
            confidence_basis_points: Some(9_000),
        }
    }

    fn valid_box() -> BoundingBox {
        BoundingBox {
            x_ppm: 1,
            y_ppm: 2,
            width_ppm: 100,
            height_ppm: 200,
        }
    }

    #[test]
    fn normalization_is_nfc_and_collapses_unicode_whitespace() {
        assert_eq!(
            normalize_text("  桌面\t圖譜\nCafe\u{301}  "),
            "桌面 圖譜 Café"
        );
    }

    #[test]
    fn levenshtein_handles_unicode_words_and_empty_inputs() {
        let reference: Vec<char> = "桌面圖譜".chars().collect();
        let hypothesis: Vec<char> = "桌面知識圖譜".chars().collect();
        assert_eq!(levenshtein(&reference, &hypothesis), 2);
        assert_eq!(levenshtein::<char>(&[], &hypothesis), hypothesis.len());
        assert_eq!(levenshtein(&reference, &[]), reference.len());
        assert_eq!(levenshtein(&["local", "graph"], &["private", "graph"]), 1);
    }

    #[test]
    fn sha256_is_stable_and_lowercase() {
        assert_eq!(
            sha256_hex(b"hello world"),
            "b94d27b9934d3e08a52e52d7da7dabfac484efe37a5380ee9088f7ace2efcde9"
        );
    }

    #[test]
    fn perfect_mixed_run_produces_integer_metrics_without_text() {
        let report = evaluate_fixture(corpus(), run()).expect("valid evaluation");
        assert_eq!(report.micro_cer.edits, 0);
        assert_eq!(report.micro_cer.rate_ppm, Some(0));
        assert_eq!(report.micro_whitespace_token_error_rate.edits, 0);
        assert_eq!(report.no_text_cases, 1);
        assert_eq!(report.no_text_false_positive_cases, 0);
        assert_eq!(report.observations.valid_boxes, 2);
        assert_eq!(report.attempt_latency.p50_us, 1_000);
        assert_eq!(report.attempt_latency.p95_us, 2_000);
        assert_eq!(report.completed_latency.sample_count, 2);
        assert_eq!(report.corpus_input_sha256, CORPUS_SHA);
        assert_eq!(report.run_input_sha256, RUN_SHA);
        let mixed_slice = report
            .case_slices
            .iter()
            .find(|slice| slice.tag == CaseTag::MixedLanguage)
            .expect("mixed-language slice");
        assert_eq!(mixed_slice.case_count, 1);
        assert_eq!(mixed_slice.micro_cer.rate_ppm, Some(0));
        let no_text_slice = report
            .case_slices
            .iter()
            .find(|slice| slice.tag == CaseTag::NoText)
            .expect("no-text slice");
        assert_eq!(no_text_slice.micro_cer.rate_ppm, None);

        let json = serde_json::to_string(&report).expect("serialize report");
        assert!(!json.contains("桌面圖譜"));
        assert!(!json.contains("Café"));
        assert!(!json.contains("platform-harness controlled-corpus"));
        assert!(!json.contains("/Users/"));
    }

    #[test]
    fn failed_case_is_counted_and_scored_as_empty_hypothesis() {
        let mut provider_run = run();
        provider_run.cases[0].status = RunStatus::Failed;
        provider_run.cases[0].error_code = Some("provider_failed".to_owned());
        provider_run.cases[0].recognized_text.clear();
        provider_run.cases[0].observations.clear();
        let report = evaluate_fixture(corpus(), provider_run).expect("failed run remains evidence");
        assert_eq!(report.failed_cases, 1);
        assert_eq!(report.completed_cases, 1);
        assert_eq!(report.micro_cer.edits, report.micro_cer.reference_units);
        assert_eq!(report.failure_codes.get("provider_failed"), Some(&1));
        assert_eq!(report.attempt_latency.sample_count, 2);
        assert_eq!(report.completed_latency.sample_count, 1);
    }

    #[test]
    fn canonical_text_is_independent_of_observation_order() {
        let mut provider_run = run();
        provider_run.cases[0].observations.reverse();
        let report = evaluate_fixture(corpus(), provider_run).expect("valid run");
        assert_eq!(report.micro_cer.edits, 0);
        assert_eq!(report.micro_whitespace_token_error_rate.edits, 0);
    }

    #[test]
    fn no_text_false_positive_is_explicit_even_with_zero_reference_units() {
        let mut provider_run = run();
        provider_run.cases[1].recognized_text = "noise".to_owned();
        provider_run.cases[1]
            .observations
            .push(observation("noise", valid_box()));
        let report = evaluate_fixture(corpus(), provider_run).expect("valid run");
        assert_eq!(report.no_text_false_positive_cases, 1);
        assert!(report.micro_cer.edits > 0);
    }

    #[test]
    fn spatial_and_confidence_contract_failures_are_reported_not_hidden() {
        let mut provider_run = run();
        provider_run.cases[0].observations.push(RunObservation {
            text: String::new(),
            bounding_box: BoundingBox {
                x_ppm: u32::MAX,
                y_ppm: 0,
                width_ppm: 2,
                height_ppm: 1,
            },
            confidence_basis_points: Some(10_001),
        });
        let report = evaluate_fixture(corpus(), provider_run).expect("invalid output is measured");
        assert_eq!(report.observations.invalid_boxes, 1);
        assert_eq!(report.observations.empty_text, 1);
        assert_eq!(report.observations.invalid_confidence, 1);
    }

    #[test]
    fn duplicate_missing_and_extra_cases_are_rejected() {
        let mut duplicate = run();
        duplicate.cases[1].case_id = "mixed".to_owned();
        duplicate.cases[1].image_sha256 = SHA_A.to_owned();
        assert_eq!(
            evaluate_fixture(corpus(), duplicate).unwrap_err(),
            "ocr_run_case_duplicate"
        );

        let mut missing = run();
        missing.cases.pop();
        assert_eq!(
            evaluate_fixture(corpus(), missing).unwrap_err(),
            "ocr_run_case_set_mismatch"
        );

        let mut extra = run();
        extra.cases[1].case_id = "unknown".to_owned();
        assert_eq!(
            evaluate_fixture(corpus(), extra).unwrap_err(),
            "ocr_run_case_unknown"
        );
    }

    #[test]
    fn api_corpus_checksum_and_no_text_mismatches_are_rejected() {
        let mut unsupported = corpus();
        unsupported.api_version = "future".to_owned();
        assert_eq!(
            evaluate_fixture(unsupported, run()).unwrap_err(),
            "ocr_evaluation_api_version_unsupported"
        );

        let mut wrong_corpus = run();
        wrong_corpus.corpus_id = "different".to_owned();
        assert_eq!(
            evaluate_fixture(corpus(), wrong_corpus).unwrap_err(),
            "ocr_evaluation_corpus_mismatch"
        );

        let mut checksum = run();
        checksum.cases[0].image_sha256 = SHA_B.to_owned();
        assert_eq!(
            evaluate_fixture(corpus(), checksum).unwrap_err(),
            "ocr_run_image_checksum_mismatch"
        );

        let mut bad_tags = corpus();
        bad_tags.cases[1].expected_text = "not empty".to_owned();
        assert_eq!(
            evaluate_fixture(bad_tags, run()).unwrap_err(),
            "ocr_corpus_no_text_tag_mismatch"
        );

        let mut duplicate_tags = corpus();
        duplicate_tags.cases[0].tags.push(CaseTag::MixedLanguage);
        assert_eq!(
            evaluate_fixture(duplicate_tags, run()).unwrap_err(),
            "ocr_corpus_tags_invalid"
        );

        let mut language_tagged_no_text = corpus();
        language_tagged_no_text.cases[1].tags.push(CaseTag::English);
        assert_eq!(
            evaluate_fixture(language_tagged_no_text, run()).unwrap_err(),
            "ocr_corpus_tags_invalid"
        );

        let mut wrong_digest = run();
        wrong_digest.corpus_input_sha256 = SHA_A.to_owned();
        assert_eq!(
            evaluate_fixture(corpus(), wrong_digest).unwrap_err(),
            "ocr_evaluation_corpus_digest_mismatch"
        );
    }

    #[test]
    fn case_status_contract_rejects_ambiguous_partial_output() {
        let mut failed_with_text = run();
        failed_with_text.cases[0].status = RunStatus::Failed;
        failed_with_text.cases[0].error_code = Some("provider_failed".to_owned());
        assert_eq!(
            evaluate_fixture(corpus(), failed_with_text).unwrap_err(),
            "ocr_run_case_contract_invalid"
        );

        let mut completed_with_error = run();
        completed_with_error.cases[0].error_code = Some("provider_failed".to_owned());
        assert_eq!(
            evaluate_fixture(corpus(), completed_with_error).unwrap_err(),
            "ocr_run_case_contract_invalid"
        );
    }

    #[test]
    fn text_unit_and_observation_limits_fail_before_unbounded_work() {
        let mut large_corpus = corpus();
        large_corpus.cases[0].expected_text = "a".repeat(MAX_TEXT_UNITS_PER_CASE + 1);
        assert_eq!(
            evaluate_fixture(large_corpus, run()).unwrap_err(),
            "ocr_evaluation_text_unit_limit_exceeded"
        );

        let mut too_many_observations = run();
        too_many_observations.cases[0].observations =
            vec![observation("x", valid_box()); MAX_OBSERVATIONS_PER_CASE + 1];
        assert_eq!(
            evaluate_fixture(corpus(), too_many_observations).unwrap_err(),
            "ocr_run_case_contract_invalid"
        );
    }

    #[test]
    fn rss_is_external_evidence_and_missing_values_stay_missing() {
        let report = evaluate_fixture(corpus(), run()).expect("valid evaluation");
        assert!(report.reported_rss.peak.is_none());
        assert!(report.missing_evidence.contains(&"rss_peak"));
        assert!(report.missing_evidence.contains(&"cancellation_e2e"));

        let mut impossible = run();
        impossible.rss.scope = Some(RssScope::WholeProcess);
        impossible.rss.peak = Some(RssMeasurement {
            bytes: impossible.host.ram_bytes + 1,
            source: RssMeasurementSource::MacosTimeL,
        });
        assert_eq!(
            evaluate_fixture(corpus(), impossible).unwrap_err(),
            "ocr_run_rss_evidence_invalid"
        );

        let mut path_bearing_toolchain = run();
        path_bearing_toolchain.host.rust_toolchain = "/Users/example/rustc".to_owned();
        assert_eq!(
            evaluate_fixture(corpus(), path_bearing_toolchain).unwrap_err(),
            "ocr_run_host_evidence_invalid"
        );

        let mut before_from_time_l = run();
        before_from_time_l.rss.scope = Some(RssScope::WholeProcess);
        before_from_time_l.rss.before = Some(RssMeasurement {
            bytes: 1,
            source: RssMeasurementSource::MacosTimeL,
        });
        assert_eq!(
            evaluate_fixture(corpus(), before_from_time_l).unwrap_err(),
            "ocr_run_rss_evidence_invalid"
        );

        let mut wrong_os_source = run();
        wrong_os_source.rss.scope = Some(RssScope::WholeProcess);
        wrong_os_source.rss.peak = Some(RssMeasurement {
            bytes: 1,
            source: RssMeasurementSource::WindowsProcessMemoryInfo,
        });
        assert_eq!(
            evaluate_fixture(corpus(), wrong_os_source).unwrap_err(),
            "ocr_run_rss_evidence_invalid"
        );
    }

    #[test]
    fn runtime_evidence_distinguishes_os_managed_packaged_and_missing() {
        let native = evaluate_fixture(corpus(), run()).expect("native evidence");
        assert!(
            !native
                .missing_evidence
                .contains(&"provider_runtime_manifest")
        );

        let mut packaged = run();
        packaged.provider.runtime = ProviderRuntimeEvidence::Packaged {
            artifact_manifest_sha256: SHA_A.to_owned(),
            model_manifest_sha256: SHA_B.to_owned(),
        };
        let packaged_report = evaluate_fixture(corpus(), packaged).expect("packaged evidence");
        assert!(
            !packaged_report
                .missing_evidence
                .contains(&"provider_runtime_manifest")
        );

        let mut missing = run();
        missing.provider.runtime = ProviderRuntimeEvidence::Missing;
        let missing_report = evaluate_fixture(corpus(), missing).expect("missing evidence");
        assert!(
            missing_report
                .missing_evidence
                .contains(&"provider_runtime_manifest")
        );
    }

    #[test]
    fn percentile_uses_nearest_rank() {
        assert_eq!(percentile(&[], 95), 0);
        assert_eq!(percentile(&[1, 2, 3, 4, 5], 50), 3);
        assert_eq!(percentile(&[1, 2, 3, 4, 5], 95), 5);
    }

    #[test]
    fn unknown_json_fields_are_rejected() {
        let corpus_json = format!(
            r#"{{
                "api_version":"{CORPUS_API_VERSION}",
                "corpus_id":"mixed-v1",
                "normalization":"{NORMALIZATION_VERSION}",
                "cases":[],
                "unexpected":true
            }}"#
        );
        assert!(serde_json::from_str::<Corpus>(&corpus_json).is_err());
    }

    #[test]
    fn public_examples_match_the_strict_contract_and_digest_binding() {
        let corpus_bytes = include_bytes!("../../../benchmarks/ocr/corpus-v1.example.json");
        let run_bytes = include_bytes!("../../../benchmarks/ocr/run-v1.example.json");
        let corpus: Corpus = serde_json::from_slice(corpus_bytes).expect("example corpus");
        let run: ProviderRun = serde_json::from_slice(run_bytes).expect("example run");
        let corpus_digest = sha256_hex(corpus_bytes);
        assert_eq!(run.corpus_input_sha256, corpus_digest);

        let report =
            evaluate(corpus, run, corpus_digest, sha256_hex(run_bytes)).expect("examples evaluate");
        assert_eq!(report.micro_cer.rate_ppm, Some(0));
        assert_eq!(report.completed_cases, 1);
    }
}
