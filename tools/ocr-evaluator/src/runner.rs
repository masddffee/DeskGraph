use std::collections::{HashMap, HashSet};
use std::ffi::CString;
use std::fs::{self, File, Metadata};
use std::io::{Cursor, Read, Write};
use std::mem::MaybeUninit;
use std::os::fd::{AsRawFd, FromRawFd};
use std::os::unix::ffi::OsStrExt;
use std::path::{Component, Path, PathBuf};
use std::time::{Duration, Instant, UNIX_EPOCH};

use deskgraph_domain::ImageFormat;
#[cfg(target_os = "macos")]
use deskgraph_extractors::NativeOcrProvider;
use deskgraph_extractors::{
    ABSOLUTE_MAX_OCR_SOURCE_BYTES, ExtractionLimits, ExtractionRequest, ExtractorProvider,
    ImageMetadataExtractor, MediaKind, OcrControl, OcrProvider, OcrRequest,
    recognize_ocr_image_bytes,
};
use deskgraph_identity::{
    FileIdentity, IdentityNodeKind, has_hidden_or_system_attribute, is_symlink_or_reparse_point,
    platform_identity, platform_identity_for_open_file,
};
use serde::{Deserialize, Serialize};

use crate::protocol::{
    BoundingBox, CORPUS_API_VERSION, Corpus, HostArch, HostEvidence, HostOs, NORMALIZATION_VERSION,
    ProviderEvidence, ProviderRun, ProviderRuntimeEvidence, RUN_API_VERSION, RssEvidence, RunCase,
    RunObservation, RunStatus, TextReconstruction,
};
use crate::{
    MAX_INPUT_BYTES, ParsedInput, is_lower_hex, is_safe_report_text, read_json_bounded, sha256_hex,
    validate_corpus, validate_identifier, validate_run, validate_top_level,
};

const ASSET_MANIFEST_API_VERSION: &str = "deskgraph.ocr-assets.v1";
const MAX_ASSET_MANIFEST_BYTES: u64 = 4 * 1024 * 1024;
const MAX_RELATIVE_PATH_BYTES: usize = 1_024;
const MAX_TOTAL_IMAGE_BYTES: u64 = 512 * 1024 * 1024;
const MAX_CASE_TIMEOUT_MS: u64 = 60_000;
const MAX_TOTAL_RUN_TIME: Duration = Duration::from_secs(60 * 60);
const READ_BUFFER_BYTES: usize = 64 * 1024;
const HARNESS_ID: &str = "deskgraph-macos-vision-runner";
const HARNESS_VERSION: &str = "1";
const RECONSTRUCTION_COMMAND: &str =
    "deskgraph-macos-vision-runner controlled-corpus observation-order-newline-v1";

#[derive(Clone, Debug)]
pub struct NativeRunnerConfig {
    pub corpus_path: PathBuf,
    pub asset_manifest_path: PathBuf,
    pub images_root: PathBuf,
    pub output_path: PathBuf,
    pub run_id: String,
    pub os_version: String,
    pub cpu_model: String,
    pub ram_bytes: u64,
    pub rust_toolchain: String,
    pub deskgraph_commit: String,
    pub runtime_revision: String,
    pub case_timeout_ms: u64,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct AssetManifest {
    api_version: String,
    corpus_id: String,
    corpus_input_sha256: String,
    assets: Vec<AssetEntry>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct AssetEntry {
    case_id: String,
    relative_path: String,
    image_sha256: String,
    format: AssetFormat,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
enum AssetFormat {
    Png,
    Jpeg,
}

impl AssetFormat {
    fn image_format(self) -> ImageFormat {
        match self {
            Self::Png => ImageFormat::Png,
            Self::Jpeg => ImageFormat::Jpeg,
        }
    }

    fn extension_matches(self, path: &Path) -> bool {
        let Some(extension) = path.extension().and_then(|value| value.to_str()) else {
            return false;
        };
        match self {
            Self::Png => extension == "png",
            Self::Jpeg => matches!(extension, "jpg" | "jpeg"),
        }
    }
}

struct ValidatedRoot {
    directory: File,
}

struct ValidatedOutput {
    parent: ValidatedDirectory,
    file_name: CString,
}

struct ValidatedDirectory {
    file: File,
    identity: FileIdentity,
    original_path: PathBuf,
}

#[derive(Clone)]
struct FileSnapshot {
    identity: FileIdentity,
    len: u64,
    modified_unix_ns: Option<i64>,
}

pub fn run_macos_vision(config: NativeRunnerConfig) -> Result<(), &'static str> {
    #[cfg(target_os = "macos")]
    {
        let arch = host_arch()?;
        run_with_provider(
            config,
            &NativeOcrProvider,
            HostOs::Macos,
            arch,
            |runtime_revision| ProviderRuntimeEvidence::OsManaged { runtime_revision },
        )
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = config;
        Err("ocr_native_runner_platform_unsupported")
    }
}

fn run_with_provider(
    config: NativeRunnerConfig,
    provider: &dyn OcrProvider,
    host_os: HostOs,
    host_arch: HostArch,
    runtime_evidence: impl FnOnce(String) -> ProviderRuntimeEvidence,
) -> Result<(), &'static str> {
    validate_config(&config)?;
    let output_destination = validate_output_destination(&config.output_path)?;
    let corpus: ParsedInput<Corpus> =
        read_json_bounded(&config.corpus_path, "ocr_native_runner_corpus_read_failed")?;
    validate_corpus_for_run(&corpus.value)?;
    let corpus_cases = validate_corpus(&corpus.value)?;

    let asset_manifest: ParsedInput<AssetManifest> = read_asset_manifest(
        &config.asset_manifest_path,
        "ocr_native_runner_asset_manifest_read_failed",
    )?;
    let assets =
        validate_asset_manifest(&asset_manifest.value, &corpus.value, &corpus.input_sha256)?;
    let images_root = validate_images_root(&config.images_root)?;

    let mut run_cases = Vec::with_capacity(corpus.value.cases.len());
    let mut total_image_bytes = 0_u64;
    let run_started = Instant::now();
    for corpus_case in &corpus.value.cases {
        if run_started.elapsed() > MAX_TOTAL_RUN_TIME {
            return Err("ocr_native_runner_total_time_limit_exceeded");
        }
        let asset = assets
            .get(corpus_case.case_id.as_str())
            .ok_or("ocr_native_runner_asset_set_mismatch")?;
        let started = Instant::now();
        let control = OcrControl::new(Duration::from_millis(config.case_timeout_ms));
        let remaining_image_bytes = MAX_TOTAL_IMAGE_BYTES
            .checked_sub(total_image_bytes)
            .ok_or("ocr_native_runner_total_source_limit_exceeded")?;
        let (encoded_image, modified_unix_ns) = read_validated_asset(
            &images_root,
            asset,
            remaining_image_bytes.min(ABSOLUTE_MAX_OCR_SOURCE_BYTES),
            &control,
        )?;
        total_image_bytes = total_image_bytes
            .checked_add(
                u64::try_from(encoded_image.len())
                    .map_err(|_| "ocr_native_runner_total_source_limit_exceeded")?,
            )
            .ok_or("ocr_native_runner_total_source_limit_exceeded")?;
        if total_image_bytes > MAX_TOTAL_IMAGE_BYTES {
            return Err("ocr_native_runner_total_source_limit_exceeded");
        }

        let format = asset.format.image_format();
        let limits = runner_limits(config.case_timeout_ms);
        let mut cursor = Cursor::new(encoded_image.as_slice());
        let cancellation = control.cancellation();
        let metadata = ImageMetadataExtractor
            .extract(
                &mut cursor,
                ExtractionRequest {
                    media_kind: MediaKind::Image(format),
                    expected_source_bytes: u64::try_from(encoded_image.len())
                        .map_err(|_| "ocr_native_runner_source_limit_exceeded")?,
                    modified_unix_ns,
                },
                limits,
                &cancellation,
            )
            .map_err(|error| runner_input_error(error.code()))?
            .image_metadata
            .ok_or("ocr_native_runner_image_metadata_missing")?;
        let request = OcrRequest {
            format,
            expected_source_bytes: u64::try_from(encoded_image.len())
                .map_err(|_| "ocr_native_runner_source_limit_exceeded")?,
            modified_unix_ns,
            pixel_width: metadata.pixel_width,
            pixel_height: metadata.pixel_height,
        };
        let result = recognize_ocr_image_bytes(provider, encoded_image, request, limits, &control);
        let elapsed_us = u64::try_from(started.elapsed().as_micros())
            .unwrap_or(u64::MAX)
            .max(1);
        let run_case = match result {
            Ok(output) => {
                let recognized_text = output
                    .observations
                    .iter()
                    .map(|observation| observation.text.as_str())
                    .collect::<Vec<_>>()
                    .join("\n");
                let observations = output
                    .observations
                    .into_iter()
                    .map(|observation| RunObservation {
                        text: observation.text,
                        bounding_box: BoundingBox {
                            x_ppm: observation.bounding_box.x_ppm,
                            y_ppm: observation.bounding_box.y_ppm,
                            width_ppm: observation.bounding_box.width_ppm,
                            height_ppm: observation.bounding_box.height_ppm,
                        },
                        confidence_basis_points: observation.confidence_basis_points.map(u32::from),
                    })
                    .collect();
                RunCase {
                    case_id: corpus_case.case_id.clone(),
                    image_sha256: corpus_case.image_sha256.clone(),
                    status: RunStatus::Completed,
                    elapsed_us,
                    error_code: None,
                    recognized_text,
                    observations,
                }
            }
            Err(error) => RunCase {
                case_id: corpus_case.case_id.clone(),
                image_sha256: corpus_case.image_sha256.clone(),
                status: RunStatus::Failed,
                elapsed_us,
                error_code: Some(error.code().to_owned()),
                recognized_text: String::new(),
                observations: Vec::new(),
            },
        };
        run_cases.push(run_case);
    }
    if run_started.elapsed() > MAX_TOTAL_RUN_TIME {
        return Err("ocr_native_runner_total_time_limit_exceeded");
    }

    let run = ProviderRun {
        api_version: RUN_API_VERSION.to_owned(),
        run_id: config.run_id,
        corpus_id: corpus.value.corpus_id.clone(),
        corpus_input_sha256: corpus.input_sha256.clone(),
        asset_manifest_input_sha256: asset_manifest.input_sha256,
        text_reconstruction: TextReconstruction::ProviderObservationOrderNewlineJoinV1,
        provider: ProviderEvidence {
            provider_id: provider.provider_id().to_owned(),
            provider_version: provider.provider_version().to_owned(),
            runtime: runtime_evidence(config.runtime_revision),
        },
        host: HostEvidence {
            os: host_os,
            os_version: config.os_version,
            arch: host_arch,
            cpu_model: config.cpu_model,
            ram_bytes: config.ram_bytes,
            rust_toolchain: config.rust_toolchain,
            deskgraph_commit: config.deskgraph_commit,
            harness_id: HARNESS_ID.to_owned(),
            harness_version: HARNESS_VERSION.to_owned(),
            command: RECONSTRUCTION_COMMAND.to_owned(),
        },
        rss: RssEvidence {
            scope: None,
            before: None,
            peak: None,
            after_caller: None,
            after_cleanup: None,
        },
        cases: run_cases,
    };
    validate_top_level(&corpus.value, &run, &corpus.input_sha256)?;
    validate_run(&run, &corpus_cases)?;
    let json =
        serde_json::to_vec_pretty(&run).map_err(|_| "ocr_native_runner_serialization_failed")?;
    if json.is_empty() || u64::try_from(json.len()).unwrap_or(u64::MAX) > MAX_INPUT_BYTES {
        return Err("ocr_native_runner_output_limit_exceeded");
    }
    publish_sensitive_json(&output_destination, &run.run_id, &json)
}

fn validate_config(config: &NativeRunnerConfig) -> Result<(), &'static str> {
    validate_identifier(&config.run_id).map_err(|_| "ocr_native_runner_configuration_invalid")?;
    if config.ram_bytes == 0
        || config.case_timeout_ms == 0
        || config.case_timeout_ms > MAX_CASE_TIMEOUT_MS
        || !is_safe_report_text(&config.os_version, 128)
        || !is_safe_report_text(&config.cpu_model, 128)
        || !is_safe_report_text(&config.rust_toolchain, 128)
        || !is_lower_hex(&config.deskgraph_commit, 40)
        || !is_safe_report_text(&config.runtime_revision, 128)
    {
        return Err("ocr_native_runner_configuration_invalid");
    }
    Ok(())
}

fn validate_corpus_for_run(corpus: &Corpus) -> Result<(), &'static str> {
    if corpus.api_version != CORPUS_API_VERSION || corpus.normalization != NORMALIZATION_VERSION {
        return Err("ocr_native_runner_corpus_version_unsupported");
    }
    validate_identifier(&corpus.corpus_id)
        .map_err(|_| "ocr_native_runner_corpus_contract_invalid")?;
    validate_corpus(corpus)
        .map(|_| ())
        .map_err(|_| "ocr_native_runner_corpus_contract_invalid")
}

fn read_asset_manifest<T: serde::de::DeserializeOwned>(
    path: &Path,
    read_error: &'static str,
) -> Result<ParsedInput<T>, &'static str> {
    let metadata = fs::metadata(path).map_err(|_| read_error)?;
    if !metadata.is_file() || metadata.len() == 0 || metadata.len() > MAX_ASSET_MANIFEST_BYTES {
        return Err("ocr_native_runner_asset_manifest_size_invalid");
    }
    read_json_bounded(path, read_error)
}

fn validate_asset_manifest<'a>(
    manifest: &'a AssetManifest,
    corpus: &Corpus,
    corpus_input_sha256: &str,
) -> Result<HashMap<&'a str, &'a AssetEntry>, &'static str> {
    if manifest.api_version != ASSET_MANIFEST_API_VERSION {
        return Err("ocr_native_runner_asset_manifest_version_unsupported");
    }
    if manifest.corpus_id != corpus.corpus_id || manifest.corpus_input_sha256 != corpus_input_sha256
    {
        return Err("ocr_native_runner_asset_manifest_corpus_mismatch");
    }
    if manifest.assets.len() != corpus.cases.len() {
        return Err("ocr_native_runner_asset_set_mismatch");
    }
    let corpus_by_id = corpus
        .cases
        .iter()
        .map(|case| (case.case_id.as_str(), case))
        .collect::<HashMap<_, _>>();
    let mut seen_paths = HashSet::with_capacity(manifest.assets.len());
    let mut assets = HashMap::with_capacity(manifest.assets.len());
    for asset in &manifest.assets {
        validate_identifier(&asset.case_id)
            .map_err(|_| "ocr_native_runner_asset_contract_invalid")?;
        let corpus_case = corpus_by_id
            .get(asset.case_id.as_str())
            .ok_or("ocr_native_runner_asset_set_mismatch")?;
        if asset.image_sha256 != corpus_case.image_sha256 {
            return Err("ocr_native_runner_asset_checksum_mismatch");
        }
        let relative_path = validate_relative_asset_path(&asset.relative_path)?;
        if !asset.format.extension_matches(relative_path) {
            return Err("ocr_native_runner_asset_format_mismatch");
        }
        if !seen_paths.insert(relative_path.as_os_str().as_bytes()) {
            return Err("ocr_native_runner_asset_path_duplicate");
        }
        if assets.insert(asset.case_id.as_str(), asset).is_some() {
            return Err("ocr_native_runner_asset_case_duplicate");
        }
    }
    Ok(assets)
}

fn validate_relative_asset_path(value: &str) -> Result<&Path, &'static str> {
    if value.is_empty()
        || value.len() > MAX_RELATIVE_PATH_BYTES
        || value.starts_with('/')
        || value.ends_with('/')
        || value.contains("//")
        || value.contains('\\')
        || value
            .chars()
            .any(|character| character.is_control() || "<>:\"|?*".contains(character))
    {
        return Err("ocr_native_runner_asset_path_invalid");
    }
    let path = Path::new(value);
    if path.is_absolute()
        || !path
            .components()
            .all(|component| matches!(component, Component::Normal(_)))
    {
        return Err("ocr_native_runner_asset_path_invalid");
    }
    Ok(path)
}

fn validate_images_root(path: &Path) -> Result<ValidatedRoot, &'static str> {
    let directory = open_validated_directory(
        path,
        "ocr_native_runner_images_root_unavailable",
        "ocr_native_runner_images_root_invalid",
    )?;
    Ok(ValidatedRoot {
        directory: directory.file,
    })
}

fn read_validated_asset(
    root: &ValidatedRoot,
    asset: &AssetEntry,
    max_source_bytes: u64,
    control: &OcrControl,
) -> Result<(Vec<u8>, Option<i64>), &'static str> {
    control
        .check()
        .map_err(|error| runner_input_error(error.code()))?;
    let relative_path = validate_relative_asset_path(&asset.relative_path)?;
    let relative_path_c =
        cstring_for_path(relative_path).map_err(|_| "ocr_native_runner_asset_path_invalid")?;
    let flags = libc::O_RDONLY | libc::O_CLOEXEC | libc::O_NOFOLLOW_ANY | libc::O_NONBLOCK;
    // SAFETY: the directory fd is owned by `root`, the C string is valid for the call, and
    // no mode argument is required because O_CREAT is not set.
    let raw_fd =
        unsafe { libc::openat(root.directory.as_raw_fd(), relative_path_c.as_ptr(), flags) };
    if raw_fd < 0 {
        return Err("ocr_native_runner_asset_file_invalid");
    }
    // SAFETY: `openat` returned a fresh owned descriptor and this is its sole owner.
    let mut file = unsafe { File::from_raw_fd(raw_fd) };
    let metadata = file
        .metadata()
        .map_err(|_| "ocr_native_runner_asset_metadata_unavailable")?;
    if has_hidden_or_system_attribute(&metadata) || !metadata.is_file() {
        return Err("ocr_native_runner_asset_file_invalid");
    }
    if metadata.len() == 0 || metadata.len() > max_source_bytes {
        return Err("ocr_native_runner_source_limit_exceeded");
    }
    let pre_snapshot = snapshot_for_open_file(&file, relative_path, &metadata)?;
    validate_open_file(&file, relative_path, &pre_snapshot)?;

    let capacity =
        usize::try_from(pre_snapshot.len).map_err(|_| "ocr_native_runner_source_limit_exceeded")?;
    let mut bytes = Vec::with_capacity(capacity);
    let mut buffer = [0_u8; READ_BUFFER_BYTES];
    loop {
        control
            .check()
            .map_err(|error| runner_input_error(error.code()))?;
        let read = file
            .read(&mut buffer)
            .map_err(|_| "ocr_native_runner_asset_read_failed")?;
        if read == 0 {
            break;
        }
        let next_len = bytes
            .len()
            .checked_add(read)
            .ok_or("ocr_native_runner_source_limit_exceeded")?;
        if u64::try_from(next_len).unwrap_or(u64::MAX) > max_source_bytes {
            return Err("ocr_native_runner_source_limit_exceeded");
        }
        bytes.extend_from_slice(&buffer[..read]);
    }
    if u64::try_from(bytes.len()).ok() != Some(pre_snapshot.len) {
        return Err("ocr_native_runner_asset_changed");
    }
    validate_open_file(&file, relative_path, &pre_snapshot)?;
    if sha256_hex(&bytes) != asset.image_sha256 {
        return Err("ocr_native_runner_asset_checksum_mismatch");
    }
    control
        .check()
        .map_err(|error| runner_input_error(error.code()))?;
    Ok((bytes, pre_snapshot.modified_unix_ns))
}

fn snapshot_for_open_file(
    file: &File,
    path: &Path,
    metadata: &Metadata,
) -> Result<FileSnapshot, &'static str> {
    let identity = platform_identity_for_open_file(file, path, metadata, IdentityNodeKind::File)
        .map_err(|_| "ocr_native_runner_asset_identity_unavailable")?;
    Ok(FileSnapshot {
        identity,
        len: metadata.len(),
        modified_unix_ns: modified_unix_ns(metadata),
    })
}

fn validate_open_file(
    file: &File,
    path: &Path,
    expected: &FileSnapshot,
) -> Result<(), &'static str> {
    let metadata = file
        .metadata()
        .map_err(|_| "ocr_native_runner_asset_metadata_unavailable")?;
    if !metadata.is_file()
        || metadata.len() != expected.len
        || modified_unix_ns(&metadata) != expected.modified_unix_ns
    {
        return Err("ocr_native_runner_asset_changed");
    }
    let identity = platform_identity_for_open_file(file, path, &metadata, IdentityNodeKind::File)
        .map_err(|_| "ocr_native_runner_asset_identity_unavailable")?;
    if identity.kind != expected.identity.kind || identity.key != expected.identity.key {
        return Err("ocr_native_runner_asset_changed");
    }
    Ok(())
}

fn runner_limits(case_timeout_ms: u64) -> ExtractionLimits {
    ExtractionLimits {
        max_image_source_bytes: ABSOLUTE_MAX_OCR_SOURCE_BYTES,
        max_processing_time: Duration::from_millis(case_timeout_ms),
        ..ExtractionLimits::default()
    }
}

fn runner_input_error(extraction_code: &'static str) -> &'static str {
    match extraction_code {
        "extraction_cancelled" => "ocr_native_runner_cancelled",
        "extraction_time_limit_exceeded" => "ocr_native_runner_time_limit_exceeded",
        "extraction_source_too_large" => "ocr_native_runner_source_limit_exceeded",
        "extraction_image_metadata_limit_exceeded" => {
            "ocr_native_runner_image_metadata_limit_exceeded"
        }
        "extraction_image_dimension_limit_exceeded" => {
            "ocr_native_runner_image_dimension_limit_exceeded"
        }
        "extraction_image_format_mismatch" => "ocr_native_runner_asset_format_mismatch",
        _ => "ocr_native_runner_image_invalid",
    }
}

fn modified_unix_ns(metadata: &Metadata) -> Option<i64> {
    let duration = metadata.modified().ok()?.duration_since(UNIX_EPOCH).ok()?;
    i64::try_from(duration.as_nanos()).ok()
}

fn publish_sensitive_json(
    destination: &ValidatedOutput,
    run_id: &str,
    bytes: &[u8],
) -> Result<(), &'static str> {
    validate_directory_path_still_bound(&destination.parent)?;
    if entry_exists_at(&destination.parent.file, &destination.file_name)? {
        return Err("ocr_native_runner_output_exists");
    }

    let temporary_name = CString::new(format!(".{run_id}.{}.ocr-partial", std::process::id()))
        .map_err(|_| "ocr_native_runner_temporary_output_create_failed")?;
    let parent_fd = destination.parent.file.as_raw_fd();
    let flags =
        libc::O_WRONLY | libc::O_CREAT | libc::O_EXCL | libc::O_CLOEXEC | libc::O_NOFOLLOW_ANY;
    // SAFETY: `parent_fd` remains open, `temporary_name` is a valid C string, and the mode is
    // supplied because O_CREAT is set. O_EXCL reserves a name owned solely by this runner.
    let raw_fd = unsafe {
        libc::openat(
            parent_fd,
            temporary_name.as_ptr(),
            flags,
            0o600 as libc::c_uint,
        )
    };
    if raw_fd < 0 {
        return Err("ocr_native_runner_temporary_output_create_failed");
    }
    // SAFETY: `openat` returned a fresh owned descriptor and this is its sole owner.
    let mut temporary = unsafe { File::from_raw_fd(raw_fd) };
    if temporary
        .write_all(bytes)
        .and_then(|_| temporary.sync_all())
        .is_err()
    {
        drop(temporary);
        return Err("ocr_native_runner_output_write_failed");
    }
    drop(temporary);

    // SAFETY: both names are valid C strings, both directory descriptors remain open, and
    // RENAME_EXCL guarantees the final destination is never replaced.
    let published = unsafe {
        libc::renameatx_np(
            parent_fd,
            temporary_name.as_ptr(),
            parent_fd,
            destination.file_name.as_ptr(),
            libc::RENAME_EXCL,
        )
    };
    if published != 0 {
        return Err("ocr_native_runner_output_publish_failed");
    }
    sync_directory_fd(&destination.parent.file)?;
    validate_directory_path_still_bound(&destination.parent)
}

fn validate_output_destination(output_path: &Path) -> Result<ValidatedOutput, &'static str> {
    if !output_path.is_absolute() {
        return Err("ocr_native_runner_output_path_invalid");
    }
    let parent = output_path
        .parent()
        .ok_or("ocr_native_runner_output_path_invalid")?;
    let file_name = output_path
        .file_name()
        .and_then(|value| value.to_str())
        .ok_or("ocr_native_runner_output_path_invalid")?;
    if file_name.is_empty()
        || file_name.len() > 128
        || file_name
            .chars()
            .any(|character| character.is_control() || matches!(character, '/' | '\\'))
    {
        return Err("ocr_native_runner_output_path_invalid");
    }
    let file_name =
        CString::new(file_name.as_bytes()).map_err(|_| "ocr_native_runner_output_path_invalid")?;
    let parent = open_validated_directory(
        parent,
        "ocr_native_runner_output_parent_unavailable",
        "ocr_native_runner_output_parent_invalid",
    )?;
    if entry_exists_at(&parent.file, &file_name)? {
        return Err("ocr_native_runner_output_exists");
    }
    Ok(ValidatedOutput { parent, file_name })
}

fn open_validated_directory(
    path: &Path,
    unavailable_error: &'static str,
    invalid_error: &'static str,
) -> Result<ValidatedDirectory, &'static str> {
    if !path.is_absolute() {
        return Err(invalid_error);
    }
    let pre_metadata = fs::symlink_metadata(path).map_err(|_| unavailable_error)?;
    if is_symlink_or_reparse_point(&pre_metadata)
        || has_hidden_or_system_attribute(&pre_metadata)
        || !pre_metadata.is_dir()
    {
        return Err(invalid_error);
    }
    let pre_identity = platform_identity(path, &pre_metadata, IdentityNodeKind::Folder)
        .map_err(|_| unavailable_error)?;
    let path_c = cstring_for_path(path).map_err(|_| invalid_error)?;
    let flags = libc::O_RDONLY | libc::O_DIRECTORY | libc::O_CLOEXEC | libc::O_NOFOLLOW_ANY;
    // SAFETY: the absolute path C string is valid for the call, and no mode argument is
    // required because O_CREAT is not set.
    let raw_fd = unsafe { libc::open(path_c.as_ptr(), flags) };
    if raw_fd < 0 {
        return Err(unavailable_error);
    }
    // SAFETY: `open` returned a fresh owned descriptor and this is its sole owner.
    let file = unsafe { File::from_raw_fd(raw_fd) };
    let metadata = file.metadata().map_err(|_| unavailable_error)?;
    if has_hidden_or_system_attribute(&metadata) || !metadata.is_dir() {
        return Err(invalid_error);
    }
    let identity =
        platform_identity_for_open_file(&file, path, &metadata, IdentityNodeKind::Folder)
            .map_err(|_| unavailable_error)?;
    if identity.kind != pre_identity.kind || identity.key != pre_identity.key {
        return Err(invalid_error);
    }
    Ok(ValidatedDirectory {
        file,
        identity,
        original_path: path.to_owned(),
    })
}

fn validate_directory_path_still_bound(expected: &ValidatedDirectory) -> Result<(), &'static str> {
    let current = open_validated_directory(
        &expected.original_path,
        "ocr_native_runner_output_parent_changed",
        "ocr_native_runner_output_parent_changed",
    )?;
    if current.identity.kind != expected.identity.kind
        || current.identity.key != expected.identity.key
    {
        return Err("ocr_native_runner_output_parent_changed");
    }
    Ok(())
}

fn cstring_for_path(path: &Path) -> Result<CString, std::ffi::NulError> {
    CString::new(path.as_os_str().as_bytes())
}

fn entry_exists_at(directory: &File, name: &CString) -> Result<bool, &'static str> {
    let mut stat = MaybeUninit::<libc::stat>::uninit();
    // SAFETY: `stat` points to writable storage, `name` is a valid C string, and the directory
    // descriptor remains open for the duration of the call.
    let result = unsafe {
        libc::fstatat(
            directory.as_raw_fd(),
            name.as_ptr(),
            stat.as_mut_ptr(),
            libc::AT_SYMLINK_NOFOLLOW,
        )
    };
    if result == 0 {
        return Ok(true);
    }
    if std::io::Error::last_os_error().kind() == std::io::ErrorKind::NotFound {
        return Ok(false);
    }
    Err("ocr_native_runner_output_unavailable")
}

fn sync_directory_fd(directory: &File) -> Result<(), &'static str> {
    // SAFETY: `directory` owns a live descriptor for a validated directory.
    if unsafe { libc::fsync(directory.as_raw_fd()) } != 0 {
        return Err("ocr_native_runner_output_sync_failed");
    }
    Ok(())
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
fn host_arch() -> Result<HostArch, &'static str> {
    Ok(HostArch::Aarch64)
}

#[cfg(all(target_os = "macos", target_arch = "x86_64"))]
fn host_arch() -> Result<HostArch, &'static str> {
    Ok(HostArch::X86_64)
}

#[cfg(all(
    target_os = "macos",
    not(any(target_arch = "aarch64", target_arch = "x86_64"))
))]
fn host_arch() -> Result<HostArch, &'static str> {
    Err("ocr_native_runner_arch_unsupported")
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicUsize, Ordering};

    use deskgraph_extractors::{OcrBoundingBox, OcrObservation, OcrOutput, OcrProviderLimits};
    use tempfile::TempDir;

    use super::*;
    use crate::evaluate_paths;

    const COMMIT_SHA: &str = "eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee";

    struct FakeProvider {
        calls: AtomicUsize,
        delay: Duration,
        result: Result<OcrOutput, deskgraph_extractors::ExtractionError>,
    }

    impl FakeProvider {
        fn success() -> Self {
            Self {
                calls: AtomicUsize::new(0),
                delay: Duration::ZERO,
                result: Ok(OcrOutput {
                    observations: vec![OcrObservation {
                        text: "桌面圖譜 DeskGraph".to_owned(),
                        bounding_box: OcrBoundingBox {
                            x_ppm: 100_000,
                            y_ppm: 100_000,
                            width_ppm: 800_000,
                            height_ppm: 100_000,
                        },
                        confidence_basis_points: Some(9_000),
                    }],
                }),
            }
        }
    }

    impl OcrProvider for FakeProvider {
        fn provider_id(&self) -> &'static str {
            "deskgraph.fake-ocr"
        }

        fn provider_version(&self) -> &'static str {
            "1"
        }

        fn recognize(
            &self,
            _encoded_image: Vec<u8>,
            _request: OcrRequest,
            _limits: OcrProviderLimits,
            _control: &OcrControl,
        ) -> Result<OcrOutput, deskgraph_extractors::ExtractionError> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            std::thread::sleep(self.delay);
            self.result.clone()
        }
    }

    struct Fixture {
        _temp: TempDir,
        config: NativeRunnerConfig,
        corpus_path: PathBuf,
        output_path: PathBuf,
        image_path: PathBuf,
    }

    fn fixture() -> Fixture {
        let temp = TempDir::new().expect("temp");
        let root = temp.path().join("images");
        let output_dir = temp.path().join("output");
        fs::create_dir(&root).expect("images root");
        fs::create_dir(&output_dir).expect("output root");
        let root = fs::canonicalize(root).expect("canonical images root");
        let output_dir = fs::canonicalize(output_dir).expect("canonical output root");
        let image_path = root.join("mixed.png");
        let image = png(640, 480);
        fs::write(&image_path, &image).expect("image");
        let image_sha256 = sha256_hex(&image);

        let corpus_path = temp.path().join("corpus.json");
        let corpus = Corpus {
            api_version: CORPUS_API_VERSION.to_owned(),
            corpus_id: "controlled-v1".to_owned(),
            normalization: NORMALIZATION_VERSION.to_owned(),
            cases: vec![crate::protocol::CorpusCase {
                case_id: "mixed".to_owned(),
                image_sha256: image_sha256.clone(),
                expected_text: "桌面圖譜 DeskGraph".to_owned(),
                tags: vec![crate::protocol::CaseTag::MixedLanguage],
            }],
        };
        let corpus_bytes = serde_json::to_vec_pretty(&corpus).expect("corpus JSON");
        fs::write(&corpus_path, &corpus_bytes).expect("corpus");

        let asset_manifest_path = temp.path().join("assets.json");
        let manifest = AssetManifest {
            api_version: ASSET_MANIFEST_API_VERSION.to_owned(),
            corpus_id: corpus.corpus_id.clone(),
            corpus_input_sha256: sha256_hex(&corpus_bytes),
            assets: vec![AssetEntry {
                case_id: "mixed".to_owned(),
                relative_path: "mixed.png".to_owned(),
                image_sha256,
                format: AssetFormat::Png,
            }],
        };
        fs::write(
            &asset_manifest_path,
            serde_json::to_vec_pretty(&manifest).expect("manifest JSON"),
        )
        .expect("manifest");

        let output_path = output_dir.join("run.json");
        let config = NativeRunnerConfig {
            corpus_path: corpus_path.clone(),
            asset_manifest_path,
            images_root: root,
            output_path: output_path.clone(),
            run_id: "fake-macos-1".to_owned(),
            os_version: "macOS 15.5".to_owned(),
            cpu_model: "Apple M4".to_owned(),
            ram_bytes: 8 * 1024 * 1024 * 1024,
            rust_toolchain: "rustc 1.97.0".to_owned(),
            deskgraph_commit: COMMIT_SHA.to_owned(),
            runtime_revision: "test runtime 1".to_owned(),
            case_timeout_ms: 1_000,
        };
        Fixture {
            _temp: temp,
            config,
            corpus_path,
            output_path,
            image_path,
        }
    }

    fn run_fake(config: NativeRunnerConfig, provider: &FakeProvider) -> Result<(), &'static str> {
        run_with_provider(
            config,
            provider,
            HostOs::Macos,
            HostArch::Aarch64,
            |runtime_revision| ProviderRuntimeEvidence::OsManaged { runtime_revision },
        )
    }

    fn png(width: u32, height: u32) -> Vec<u8> {
        let mut bytes = vec![0_u8; 32];
        bytes[..8].copy_from_slice(b"\x89PNG\r\n\x1a\n");
        bytes[8..12].copy_from_slice(&13_u32.to_be_bytes());
        bytes[12..16].copy_from_slice(b"IHDR");
        bytes[16..20].copy_from_slice(&width.to_be_bytes());
        bytes[20..24].copy_from_slice(&height.to_be_bytes());
        bytes
    }

    #[test]
    fn fake_provider_run_feeds_the_real_evaluator_without_leaking_text_to_report() {
        let fixture = fixture();
        let provider = FakeProvider::success();
        run_fake(fixture.config, &provider).expect("runner succeeds");
        assert_eq!(provider.calls.load(Ordering::SeqCst), 1);

        let report =
            evaluate_paths(&fixture.corpus_path, &fixture.output_path).expect("evaluation");
        let report_json = serde_json::to_string(&report).expect("report JSON");
        assert!(!report_json.contains("桌面圖譜"));
        assert!(!report_json.contains("DeskGraph"));
        assert!(!report_json.contains(fixture.image_path.to_string_lossy().as_ref()));

        let run: ProviderRun =
            serde_json::from_slice(&fs::read(&fixture.output_path).expect("run bytes"))
                .expect("run JSON");
        assert_eq!(
            run.text_reconstruction,
            TextReconstruction::ProviderObservationOrderNewlineJoinV1
        );
        assert_eq!(run.cases[0].recognized_text, "桌面圖譜 DeskGraph");
    }

    #[test]
    fn checksum_mismatch_aborts_before_provider_and_publishes_nothing() {
        let fixture = fixture();
        let mut image = fs::read(&fixture.image_path).expect("image");
        image.push(0);
        fs::write(&fixture.image_path, image).expect("changed image");
        let provider = FakeProvider::success();
        assert_eq!(
            run_fake(fixture.config, &provider),
            Err("ocr_native_runner_asset_checksum_mismatch")
        );
        assert_eq!(provider.calls.load(Ordering::SeqCst), 0);
        assert!(!fixture.output_path.exists());
    }

    #[cfg(unix)]
    #[test]
    fn symlink_escape_is_rejected_before_provider() {
        use std::os::unix::fs::symlink;

        let fixture = fixture();
        let outside = fixture._temp.path().join("outside.png");
        fs::write(&outside, png(640, 480)).expect("outside");
        fs::remove_file(&fixture.image_path).expect("remove fixture image");
        symlink(&outside, &fixture.image_path).expect("symlink");
        let provider = FakeProvider::success();
        assert_eq!(
            run_fake(fixture.config, &provider),
            Err("ocr_native_runner_asset_file_invalid")
        );
        assert_eq!(provider.calls.load(Ordering::SeqCst), 0);
        assert!(!fixture.output_path.exists());
    }

    #[test]
    fn existing_output_is_never_overwritten() {
        let fixture = fixture();
        fs::write(&fixture.output_path, b"keep").expect("existing output");
        let provider = FakeProvider::success();
        assert_eq!(
            run_fake(fixture.config, &provider),
            Err("ocr_native_runner_output_exists")
        );
        assert_eq!(
            fs::read(&fixture.output_path).expect("existing bytes"),
            b"keep"
        );
    }

    #[test]
    fn output_parent_swap_aborts_without_publishing_to_either_directory() {
        let temp = TempDir::new().expect("temp");
        let temp_root = fs::canonicalize(temp.path()).expect("canonical temp root");
        let output_dir = temp_root.join("output");
        let moved_dir = temp_root.join("moved-output");
        fs::create_dir(&output_dir).expect("output root");
        let output_path = output_dir.join("run.json");
        let destination = validate_output_destination(&output_path).expect("validated output");

        fs::rename(&output_dir, &moved_dir).expect("move validated directory");
        fs::create_dir(&output_dir).expect("replacement directory");

        assert_eq!(
            publish_sensitive_json(&destination, "swapped-output", b"{}"),
            Err("ocr_native_runner_output_parent_changed")
        );
        assert!(!output_path.exists());
        assert!(!moved_dir.join("run.json").exists());
        assert_eq!(
            fs::read_dir(&moved_dir).expect("moved directory").count(),
            0
        );
    }

    #[test]
    fn manifest_format_mismatch_aborts_before_provider() {
        let fixture = fixture();
        let manifest_path = fixture.config.asset_manifest_path.clone();
        let mut manifest: AssetManifest =
            serde_json::from_slice(&fs::read(&manifest_path).expect("manifest bytes"))
                .expect("manifest JSON");
        manifest.assets[0].format = AssetFormat::Jpeg;
        fs::write(
            &manifest_path,
            serde_json::to_vec_pretty(&manifest).expect("manifest JSON"),
        )
        .expect("changed manifest");

        let provider = FakeProvider::success();
        assert_eq!(
            run_fake(fixture.config, &provider),
            Err("ocr_native_runner_asset_format_mismatch")
        );
        assert_eq!(provider.calls.load(Ordering::SeqCst), 0);
        assert!(!fixture.output_path.exists());
    }

    #[test]
    fn provider_deadline_failure_is_recorded_in_a_complete_run() {
        let mut fixture = fixture();
        fixture.config.case_timeout_ms = 1;
        let mut provider = FakeProvider::success();
        provider.delay = Duration::from_millis(5);

        run_fake(fixture.config, &provider).expect("runner completes failed case");
        assert_eq!(provider.calls.load(Ordering::SeqCst), 1);
        let run: ProviderRun =
            serde_json::from_slice(&fs::read(&fixture.output_path).expect("run bytes"))
                .expect("run JSON");
        assert_eq!(run.cases[0].status, RunStatus::Failed);
        assert_eq!(
            run.cases[0].error_code.as_deref(),
            Some("extraction_time_limit_exceeded")
        );
        assert!(run.cases[0].recognized_text.is_empty());
        assert!(run.cases[0].observations.is_empty());
    }
}
