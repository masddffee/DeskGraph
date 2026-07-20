use std::io::Write;
use std::path::{Path, PathBuf};

use deskgraph_database::CleanupActionSelection;
use deskgraph_domain::{
    CleanupActionOperation, CleanupActionPlanPreview, CleanupActionPlanState,
    ExtractionJobProgress, ExtractionStatus, FileRelationCandidate, FileRelationKind,
    FileVersionCandidate, ProjectCandidate, ProjectCandidateState, ScanReport, ScanStatus,
    SearchMatchedField, SearchResponse, SmartCleanupInbox, SmartCleanupSourceKind,
};
use deskgraph_extractors::{ExtractionLimits, create_extraction_job_at, run_extraction_job_at};
use deskgraph_projects::{
    check_exact_duplicate_at, propose_project_at, refresh_smart_cleanup_inbox_at,
    suggest_file_version_at,
};
use deskgraph_retrieval::{SearchRequest, SearchSourceFilter, search_at};
use deskgraph_scanner::scan_scope;
use deskgraph_transactions::create_cleanup_preview_at;
use serde::Serialize;

use super::{
    authorize_scope_with_cli_consent, open_database, resolve_extraction_node, resolve_manifest_node,
};

#[derive(Debug, Serialize)]
pub(super) struct DemoFixtureReport {
    api_version: &'static str,
    verified: bool,
    workspace_path: PathBuf,
    authorized_scope_path: PathBuf,
    database_path: PathBuf,
    scope_id: i64,
    scan: ScanReport,
    extractions: Vec<ExtractionJobProgress>,
    traditional_chinese_search: SearchResponse,
    english_search: SearchResponse,
    project: ProjectCandidate,
    exact_duplicate: FileRelationCandidate,
    version_relation: FileVersionCandidate,
    cleanup_inbox: SmartCleanupInbox,
    cleanup_preview: CleanupActionPlanPreview,
    safety: DemoFixtureSafety,
}

#[derive(Debug, Serialize)]
struct DemoFixtureSafety {
    source_files_unchanged: bool,
    organization_actions_performed: bool,
    cleanup_action_authorized: bool,
    optional_ocr_used: bool,
}

struct DemoFixtureFiles {
    workspace_path: PathBuf,
    scope_path: PathBuf,
    database_path: PathBuf,
    project_path: PathBuf,
    extraction_paths: [PathBuf; 2],
    duplicate_paths: [PathBuf; 2],
    version_paths: [PathBuf; 2],
    expected_sources: Vec<(PathBuf, &'static [u8])>,
}

const DEMO_PROJECT_README: &str = "# DeskGraph Launch Lab\n\n\
DeskGraph 建立完全本機的電腦情境圖譜。\n\
Bounded context stays local and explainable for every result.\n";
const DEMO_CODE: &str = "fn main() {\n    println!(\"Graphify your computer, locally.\");\n}\n";
const DEMO_CARGO_MANIFEST: &str =
    "[package]\nname = \"deskgraph-launch-lab\"\nversion = \"0.1.0\"\nedition = \"2024\"\n";
const DEMO_DUPLICATE: &str = "DeskGraph exact duplicate evidence.\n這兩份檔案的位元完全相同。\n";
const DEMO_ROADMAP_V1: &str = "# Roadmap v1\nMetadata scan and bounded extraction.\n";
const DEMO_ROADMAP_V2: &str =
    "# Roadmap v2\nMetadata scan, bounded extraction, and explainable cleanup review.\n";

pub(super) fn run_demo_fixture(path: &Path) -> Result<DemoFixtureReport, &'static str> {
    let files = create_demo_fixture(path)?;
    let mut database = open_database(&files.database_path)?;
    let scope = authorize_scope_with_cli_consent(&mut database, &files.scope_path)
        .map_err(|error| error.code())?;
    let scan = scan_scope(&mut database, scope.id).map_err(|error| error.code())?;
    if scan.status != ScanStatus::Completed || scan.discovered_files < 7 {
        return Err("demo_fixture_scan_verification_failed");
    }
    drop(database);

    let mut extractions = Vec::with_capacity(files.extraction_paths.len());
    for extraction_path in &files.extraction_paths {
        let node_id =
            resolve_extraction_node(&files.database_path, scope.id, None, Some(extraction_path))?;
        let created = create_extraction_job_at(&files.database_path, scope.id, node_id)
            .map_err(|error| error.code())?;
        let progress = run_extraction_job_at(
            &files.database_path,
            created.job_id,
            ExtractionLimits::default(),
        )
        .map_err(|error| error.code())?;
        if progress.status != ExtractionStatus::Completed || progress.chunk_count == 0 {
            return Err("demo_fixture_extraction_verification_failed");
        }
        extractions.push(progress);
    }

    let traditional_chinese_search = search_at(
        &files.database_path,
        SearchRequest {
            query: "電腦情境圖譜",
            scope_id: Some(scope.id),
            source: SearchSourceFilter::ExtractedText,
            extension: Some("md"),
            modified_since_unix_seconds: None,
            modified_before_unix_seconds: None,
            limit: Some(10),
        },
    )
    .map_err(|error| error.code())?;
    let english_search = search_at(
        &files.database_path,
        SearchRequest {
            query: "bounded context",
            scope_id: Some(scope.id),
            source: SearchSourceFilter::ExtractedText,
            extension: Some("md"),
            modified_since_unix_seconds: None,
            modified_before_unix_seconds: None,
            limit: Some(10),
        },
    )
    .map_err(|error| error.code())?;
    if !has_extracted_text_result(&traditional_chinese_search)
        || !has_extracted_text_result(&english_search)
    {
        return Err("demo_fixture_search_verification_failed");
    }

    let project_node_id = resolve_manifest_node(
        &files.database_path,
        scope.id,
        None,
        Some(&files.project_path),
        "demo_fixture_project_not_found",
        "demo_fixture_project_not_found",
        "demo_fixture_project_selection_invalid",
    )?;
    let project = propose_project_at(&files.database_path, scope.id, project_node_id)
        .map_err(|error| error.code())?;
    if project.state != ProjectCandidateState::Suggested || project.suggestion.provenance.is_empty()
    {
        return Err("demo_fixture_project_verification_failed");
    }

    let exact_duplicate = check_exact_duplicate_at(
        &files.database_path,
        scope.id,
        &files.duplicate_paths[0],
        &files.duplicate_paths[1],
    )
    .map_err(|error| error.code())?;
    if exact_duplicate.kind != FileRelationKind::ExactDuplicate
        || exact_duplicate.evidence.compared_bytes == 0
    {
        return Err("demo_fixture_duplicate_verification_failed");
    }

    let version_relation = suggest_file_version_at(
        &files.database_path,
        scope.id,
        &files.version_paths[0],
        &files.version_paths[1],
    )
    .map_err(|error| error.code())?;
    if version_relation.kind != FileRelationKind::Version
        || version_relation.evidence.older_version != 1
        || version_relation.evidence.newer_version != 2
    {
        return Err("demo_fixture_version_verification_failed");
    }

    let cleanup_inbox = refresh_smart_cleanup_inbox_at(&files.database_path, scope.id)
        .map_err(|error| error.code())?;
    let has_duplicate = cleanup_inbox
        .items
        .iter()
        .any(|item| item.source_kind == SmartCleanupSourceKind::ExactDuplicate);
    let has_version = cleanup_inbox
        .items
        .iter()
        .any(|item| item.source_kind == SmartCleanupSourceKind::Version);
    if !cleanup_inbox.evaluation_complete
        || cleanup_inbox.action_authorized
        || cleanup_inbox
            .items
            .iter()
            .any(|item| item.cleanup_authorized)
        || !has_duplicate
        || !has_version
    {
        return Err("demo_fixture_cleanup_verification_failed");
    }
    let duplicate_inbox_item = cleanup_inbox
        .items
        .iter()
        .find(|item| item.source_kind == SmartCleanupSourceKind::ExactDuplicate)
        .ok_or("demo_fixture_cleanup_preview_source_missing")?;
    let cleanup_preview = create_cleanup_preview_at(
        &files.database_path,
        CleanupActionSelection {
            scope_id: scope.id,
            source_kind: duplicate_inbox_item.source_kind,
            source_id: duplicate_inbox_item.source_id,
            source_observation_id: duplicate_inbox_item.source_observation_id,
            keeper_node_id: Some(exact_duplicate.left.node_id),
            target_node_id: exact_duplicate.right.node_id,
        },
    )
    .map_err(|error| error.code())?;
    if cleanup_preview.operation != CleanupActionOperation::SystemTrashPreview
        || cleanup_preview.state != CleanupActionPlanState::Previewed
        || !cleanup_preview.policy.confirmation_required
        || cleanup_preview.policy.action_authorized
        || cleanup_preview.policy.execution_available
    {
        return Err("demo_fixture_cleanup_preview_verification_failed");
    }

    let source_files_unchanged = files.expected_sources.iter().all(|(path, expected)| {
        std::fs::read(path)
            .map(|actual| actual == *expected)
            .unwrap_or(false)
    });
    if !source_files_unchanged {
        return Err("demo_fixture_source_changed");
    }
    let cleanup_action_authorized = cleanup_inbox.action_authorized;

    Ok(DemoFixtureReport {
        api_version: "deskgraph.demo-fixture.v1",
        verified: true,
        workspace_path: files.workspace_path,
        authorized_scope_path: files.scope_path,
        database_path: files.database_path,
        scope_id: scope.id,
        scan,
        extractions,
        traditional_chinese_search,
        english_search,
        project,
        exact_duplicate,
        version_relation,
        cleanup_inbox,
        cleanup_preview,
        safety: DemoFixtureSafety {
            source_files_unchanged,
            organization_actions_performed: false,
            cleanup_action_authorized,
            optional_ocr_used: false,
        },
    })
}

fn has_extracted_text_result(response: &SearchResponse) -> bool {
    response.result_count > 0
        && response.results.iter().any(|result| {
            result
                .matched_fields
                .contains(&SearchMatchedField::ExtractedText)
        })
}

fn create_demo_fixture(path: &Path) -> Result<DemoFixtureFiles, &'static str> {
    let parent = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    std::fs::create_dir_all(parent).map_err(|_| "demo_fixture_parent_create_failed")?;
    if let Err(error) = std::fs::create_dir(path) {
        return if error.kind() == std::io::ErrorKind::AlreadyExists {
            Err("demo_fixture_path_already_exists")
        } else {
            Err("demo_fixture_root_create_failed")
        };
    }

    let scope_path = path.join("authorized-files");
    let project_path = scope_path.join("DeskGraph-Launch-Lab");
    let source_path = project_path.join("src");
    let docs_path = project_path.join("docs");
    let inbox_path = scope_path.join("Smart-Inbox");
    for directory in [&source_path, &docs_path, &inbox_path] {
        std::fs::create_dir_all(directory).map_err(|_| "demo_fixture_directory_create_failed")?;
    }

    let project_readme = project_path.join("README.md");
    let code = source_path.join("main.rs");
    let cargo_manifest = project_path.join("Cargo.toml");
    let duplicate_left = inbox_path.join("launch-brief-copy-a.md");
    let duplicate_right = inbox_path.join("launch-brief-copy-b.md");
    let version_v1 = docs_path.join("launch-roadmap-v1.md");
    let version_v2 = docs_path.join("launch-roadmap-v2.md");
    let sources = [
        (&project_readme, DEMO_PROJECT_README),
        (&code, DEMO_CODE),
        (&cargo_manifest, DEMO_CARGO_MANIFEST),
        (&duplicate_left, DEMO_DUPLICATE),
        (&duplicate_right, DEMO_DUPLICATE),
        (&version_v1, DEMO_ROADMAP_V1),
        (&version_v2, DEMO_ROADMAP_V2),
    ];
    for (source, contents) in sources {
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(source)
            .map_err(|_| "demo_fixture_file_create_failed")?;
        file.write_all(contents.as_bytes())
            .map_err(|_| "demo_fixture_file_write_failed")?;
    }

    let workspace_path = std::fs::canonicalize(path)
        .map_err(|_| "demo_fixture_workspace_canonicalization_failed")?;
    let scope_path = std::fs::canonicalize(scope_path)
        .map_err(|_| "demo_fixture_scope_canonicalization_failed")?;
    let project_path = std::fs::canonicalize(project_path)
        .map_err(|_| "demo_fixture_project_canonicalization_failed")?;
    let project_readme = project_path.join("README.md");
    let code = project_path.join("src/main.rs");
    let cargo_manifest = project_path.join("Cargo.toml");
    let duplicate_left = scope_path.join("Smart-Inbox/launch-brief-copy-a.md");
    let duplicate_right = scope_path.join("Smart-Inbox/launch-brief-copy-b.md");
    let version_v1 = project_path.join("docs/launch-roadmap-v1.md");
    let version_v2 = project_path.join("docs/launch-roadmap-v2.md");

    Ok(DemoFixtureFiles {
        database_path: workspace_path.join("deskgraph-demo.sqlite3"),
        workspace_path,
        scope_path,
        project_path,
        extraction_paths: [project_readme.clone(), code.clone()],
        duplicate_paths: [duplicate_left.clone(), duplicate_right.clone()],
        version_paths: [version_v1.clone(), version_v2.clone()],
        expected_sources: vec![
            (project_readme, DEMO_PROJECT_README.as_bytes()),
            (code, DEMO_CODE.as_bytes()),
            (cargo_manifest, DEMO_CARGO_MANIFEST.as_bytes()),
            (duplicate_left, DEMO_DUPLICATE.as_bytes()),
            (duplicate_right, DEMO_DUPLICATE.as_bytes()),
            (version_v1, DEMO_ROADMAP_V1.as_bytes()),
            (version_v2, DEMO_ROADMAP_V2.as_bytes()),
        ],
    })
}
