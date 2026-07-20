use std::process::Command;

use deskgraph_database::{ContentChunkProvenanceWrite, ContentChunkWrite, ManifestDatabase};
use deskgraph_extractors::{
    ExtractionLimits, create_extraction_job_at, create_screenshot_ocr_job_at, run_extraction_job_at,
};
use deskgraph_scanner::{authorize_scope, authorize_scope_with_access_grant, scan_scope};

#[test]
fn health_command_emits_privacy_safe_json() {
    let output = Command::new(env!("CARGO_BIN_EXE_deskgraph"))
        .arg("health")
        .output()
        .expect("deskgraph health should start");

    assert!(output.status.success());

    let report: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("stdout should be JSON");
    assert_eq!(report["product"], "DeskGraph");
    assert_eq!(report["database"]["state"], "not_initialized");
    assert_eq!(report["privacy"]["network_required"], false);
    assert_eq!(report["privacy"]["filesystem_locations_included"], false);

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let current_directory = std::env::current_dir().expect("test should have a current directory");
    let current_directory = current_directory.to_string_lossy();

    assert!(!stdout.contains(current_directory.as_ref()));
    assert!(!stderr.contains(current_directory.as_ref()));
    assert!(!stderr.contains("/Users/"));
    assert!(!stderr.contains("C:\\Users\\"));
    assert!(!stderr.contains("HOME"));
    assert!(
        stderr
            .lines()
            .all(|line| serde_json::from_str::<serde_json::Value>(line).is_ok())
    );
}

#[test]
fn incomplete_command_fails_with_usage_without_a_stack_trace() {
    let output = Command::new(env!("CARGO_BIN_EXE_deskgraph"))
        .arg("scan")
        .output()
        .expect("deskgraph should start");

    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("Usage:"));
    assert!(!stderr.contains("panicked"));
}

#[test]
fn scope_add_child_process_creates_a_path_free_active_grant_for_scan_and_search() {
    let directory = tempfile::tempdir().expect("fixture root should exist");
    let database_path = directory.path().join("manifest.sqlite3");
    let scope_path = directory.path().join("explicit-cli-consent");
    std::fs::create_dir(&scope_path).expect("scope should create");
    let source_path = scope_path.join("receipt-e2e-metadata.md");
    let private_contents = "this local content must never enter command diagnostics";
    std::fs::write(&source_path, private_contents).expect("fixture should write");

    let binary = env!("CARGO_BIN_EXE_deskgraph");
    let database_arg = database_path
        .to_str()
        .expect("database path should be UTF-8");
    let scope_arg = scope_path.to_str().expect("scope path should be UTF-8");

    let initialized = Command::new(binary)
        .args(["manifest", "init", "--database", database_arg])
        .output()
        .expect("manifest init should start");
    assert!(initialized.status.success());

    let added = Command::new(binary)
        .args([
            "scope",
            "add",
            "--database",
            database_arg,
            "--path",
            scope_arg,
        ])
        .output()
        .expect("scope add should start");
    assert!(added.status.success());
    let added_scope: serde_json::Value =
        serde_json::from_slice(&added.stdout).expect("scope add should emit JSON");
    let scope_id = added_scope["id"]
        .as_i64()
        .expect("scope add should return an ID");

    let database = ManifestDatabase::open(&database_path).expect("database should reopen");
    let grant = database
        .active_scope_grant(scope_id)
        .expect("CLI scope add should atomically persist an active grant");
    assert_eq!(grant.platform, std::env::consts::OS);
    assert!(
        grant
            .opaque_grant
            .starts_with(b"deskgraph-cli-explicit-consent-v1")
    );
    let canonical_scope = std::fs::canonicalize(&scope_path).expect("scope should canonicalize");
    assert!(
        !grant
            .opaque_grant
            .windows(canonical_scope.as_os_str().as_encoded_bytes().len())
            .any(|window| window == canonical_scope.as_os_str().as_encoded_bytes()),
        "the receipt must not contain path bytes"
    );
    assert!(
        !grant
            .opaque_grant
            .windows(private_contents.len())
            .any(|window| window == private_contents.as_bytes())
    );
    drop(database);

    let scope_id_arg = scope_id.to_string();
    let scanned = Command::new(binary)
        .args([
            "scan",
            "start",
            "--database",
            database_arg,
            "--scope",
            &scope_id_arg,
        ])
        .output()
        .expect("scan start should start");
    assert!(scanned.status.success());

    let searched = Command::new(binary)
        .args([
            "search",
            "--database",
            database_arg,
            "--query",
            "receipt-e2e-metadata",
            "--scope",
            &scope_id_arg,
            "--source",
            "metadata",
        ])
        .output()
        .expect("search should start");
    assert!(searched.status.success());
    let response: serde_json::Value =
        serde_json::from_slice(&searched.stdout).expect("search should emit JSON");
    assert_eq!(response["result_count"], 1);

    for output in [&initialized, &added, &scanned, &searched] {
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(!stderr.contains(private_contents));
        assert!(!stderr.contains(scope_arg));
        assert!(!stderr.contains("deskgraph-cli-explicit-consent-v1"));
        assert!(
            stderr
                .lines()
                .all(|line| serde_json::from_str::<serde_json::Value>(line).is_ok())
        );
    }
    let add_stdout = String::from_utf8_lossy(&added.stdout);
    let scan_stdout = String::from_utf8_lossy(&scanned.stdout);
    let search_stdout = String::from_utf8_lossy(&searched.stdout);
    for output in [&add_stdout, &scan_stdout, &search_stdout] {
        assert!(!output.contains(private_contents));
        assert!(!output.contains("deskgraph-cli-explicit-consent-v1"));
    }
}

#[test]
fn demo_fixture_command_runs_the_real_bilingual_cleanup_vertical_slice_without_mutation() {
    let directory = tempfile::tempdir().expect("fixture parent should exist");
    let workspace_path = directory.path().join("judge-demo-workspace");
    let workspace_arg = workspace_path
        .to_str()
        .expect("demo workspace path should be UTF-8");

    let output = Command::new(env!("CARGO_BIN_EXE_deskgraph"))
        .args(["fixture", "demo", "--path", workspace_arg])
        .output()
        .expect("deskgraph fixture demo should start");
    assert!(output.status.success());

    let report: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("demo report should be JSON");
    assert_eq!(report["api_version"], "deskgraph.demo-fixture.v1");
    assert_eq!(report["verified"], true);
    assert_eq!(report["scan"]["status"], "completed");
    assert_eq!(report["scan"]["discovered_files"], 7);
    assert_eq!(report["extractions"].as_array().map(Vec::len), Some(2));
    assert!(
        report["extractions"]
            .as_array()
            .expect("extraction results should be an array")
            .iter()
            .all(|job| job["status"] == "completed" && job["chunk_count"].as_u64() > Some(0))
    );
    assert_eq!(
        report["traditional_chinese_search"]["query"],
        "電腦情境圖譜"
    );
    assert_eq!(report["english_search"]["query"], "bounded context");
    for search in [
        &report["traditional_chinese_search"],
        &report["english_search"],
    ] {
        assert_eq!(search["mode"], "lexical");
        assert_eq!(search["embeddings_enabled"], false);
        assert!(search["result_count"].as_u64() > Some(0));
        assert_eq!(search["results"][0]["matched_fields"][0], "extracted_text");
    }
    assert_eq!(report["project"]["state"], "suggested");
    assert_eq!(
        report["project"]["suggestion"]["provenance"][0]["kind"],
        "cargo_manifest"
    );
    assert_eq!(report["exact_duplicate"]["kind"], "exact_duplicate");
    assert_eq!(
        report["exact_duplicate"]["evidence"]["comparison_kind"],
        "byte_for_byte"
    );
    assert_eq!(report["version_relation"]["kind"], "version");
    assert_eq!(report["version_relation"]["evidence"]["older_version"], 1);
    assert_eq!(report["version_relation"]["evidence"]["newer_version"], 2);
    assert_eq!(report["cleanup_inbox"]["evaluation_complete"], true);
    assert_eq!(report["cleanup_inbox"]["action_authorized"], false);
    let cleanup_kinds = report["cleanup_inbox"]["items"]
        .as_array()
        .expect("cleanup items should be an array")
        .iter()
        .map(|item| item["source_kind"].as_str().unwrap_or_default())
        .collect::<Vec<_>>();
    assert!(cleanup_kinds.contains(&"exact_duplicate"));
    assert!(cleanup_kinds.contains(&"version"));
    assert_eq!(
        report["cleanup_preview"]["operation"],
        "system_trash_preview"
    );
    assert_eq!(report["cleanup_preview"]["state"], "previewed");
    assert_eq!(
        report["cleanup_preview"]["policy"]["confirmation_required"],
        true
    );
    assert_eq!(
        report["cleanup_preview"]["policy"]["action_authorized"],
        false
    );
    assert_eq!(
        report["cleanup_preview"]["policy"]["execution_available"],
        false
    );
    assert_eq!(report["safety"]["source_files_unchanged"], true);
    assert_eq!(report["safety"]["organization_actions_performed"], false);
    assert_eq!(report["safety"]["cleanup_action_authorized"], false);
    assert_eq!(report["safety"]["optional_ocr_used"], false);

    let readme_path = workspace_path.join("authorized-files/DeskGraph-Launch-Lab/README.md");
    let duplicate_left = workspace_path.join("authorized-files/Smart-Inbox/launch-brief-copy-a.md");
    let duplicate_right =
        workspace_path.join("authorized-files/Smart-Inbox/launch-brief-copy-b.md");
    let readme_before = std::fs::read(&readme_path).expect("demo README should exist");
    let left_before = std::fs::read(&duplicate_left).expect("left duplicate should exist");
    let right_before = std::fs::read(&duplicate_right).expect("right duplicate should exist");
    assert_eq!(left_before, right_before);

    let database_path = workspace_path.join("deskgraph-demo.sqlite3");
    let database = ManifestDatabase::open(&database_path).expect("demo database should reopen");
    let stats = database.stats().expect("demo manifest stats should load");
    assert_eq!(stats.completed_scan_count, 1);
    assert_eq!(stats.file_count, 7);
    assert_eq!(
        database
            .extraction_stats()
            .expect("demo extraction stats should load")
            .extracted_file_count,
        2
    );
    drop(database);

    let second = Command::new(env!("CARGO_BIN_EXE_deskgraph"))
        .args(["fixture", "demo", "--path", workspace_arg])
        .output()
        .expect("second fixture command should start");
    assert!(!second.status.success());
    assert!(String::from_utf8_lossy(&second.stderr).contains("demo_fixture_path_already_exists"));
    assert_eq!(
        std::fs::read(&readme_path).expect("README should remain readable"),
        readme_before
    );
    assert_eq!(
        std::fs::read(&duplicate_left).expect("left duplicate should remain readable"),
        left_before
    );
    assert_eq!(
        std::fs::read(&duplicate_right).expect("right duplicate should remain readable"),
        right_before
    );

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(!stderr.contains(workspace_arg));
    assert!(
        stderr
            .lines()
            .all(|line| serde_json::from_str::<serde_json::Value>(line).is_ok())
    );
}

#[test]
fn extraction_command_emits_counts_without_paths_or_content() {
    let directory = tempfile::tempdir().expect("fixture root should exist");
    let database_path = directory.path().join("manifest.sqlite3");
    let scope_path = directory.path().join("authorized-private");
    std::fs::create_dir(&scope_path).expect("scope should create");
    let source_path = scope_path.join("private-notes.md");
    let private_text = "不可出現在 CLI 輸出的私人內容";
    std::fs::write(&source_path, private_text).expect("fixture should write");
    let mut database = ManifestDatabase::open(&database_path).expect("database should initialize");
    let scope = authorize_scope(&database, &scope_path).expect("scope should authorize");
    scan_scope(&mut database, scope.id).expect("scope should scan");
    drop(database);

    let output = Command::new(env!("CARGO_BIN_EXE_deskgraph"))
        .args([
            "extract",
            "start",
            "--database",
            database_path
                .to_str()
                .expect("database path should be UTF-8"),
            "--scope",
            &scope.id.to_string(),
            "--path",
            source_path.to_str().expect("source path should be UTF-8"),
        ])
        .output()
        .expect("deskgraph extract should start");

    assert!(output.status.success());
    let progress: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("stdout should be JSON");
    assert_eq!(progress["status"], "completed");
    assert_eq!(progress["provider_id"], "deskgraph.utf8-text");
    assert!(progress["chunk_count"].as_u64().unwrap_or_default() > 0);

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    for secret in [
        private_text,
        "private-notes.md",
        scope_path.to_str().expect("scope path should be UTF-8"),
        source_path.to_str().expect("source path should be UTF-8"),
    ] {
        assert!(!stdout.contains(secret));
        assert!(!stderr.contains(secret));
    }
    assert!(
        stderr
            .lines()
            .all(|line| serde_json::from_str::<serde_json::Value>(line).is_ok())
    );
}

#[test]
fn ocr_create_command_emits_path_free_durable_job_status() {
    let directory = tempfile::tempdir().expect("fixture root should exist");
    let database_path = directory.path().join("private-manifest.sqlite3");
    let scope_path = directory.path().join("private-screenshots");
    std::fs::create_dir(&scope_path).expect("scope should create");
    let source_path = scope_path.join("private-OCR-Screenshot.png");
    let mut png = vec![0_u8; 32];
    png[..8].copy_from_slice(b"\x89PNG\r\n\x1a\n");
    png[8..12].copy_from_slice(&13_u32.to_be_bytes());
    png[12..16].copy_from_slice(b"IHDR");
    png[16..20].copy_from_slice(&640_u32.to_be_bytes());
    png[20..24].copy_from_slice(&480_u32.to_be_bytes());
    std::fs::write(&source_path, png).expect("fixture should write");
    let mut database = ManifestDatabase::open(&database_path).expect("database should initialize");
    let scope = authorize_scope(&database, &scope_path).expect("scope should authorize");
    scan_scope(&mut database, scope.id).expect("scope should scan");
    drop(database);

    let output = Command::new(env!("CARGO_BIN_EXE_deskgraph"))
        .args([
            "extract",
            "ocr-create",
            "--database",
            database_path
                .to_str()
                .expect("database path should be UTF-8"),
            "--scope",
            &scope.id.to_string(),
            "--path",
            source_path.to_str().expect("source path should be UTF-8"),
        ])
        .output()
        .expect("deskgraph OCR create should start");

    assert!(output.status.success());
    let progress: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("stdout should be JSON");
    assert_eq!(progress["operation"], "screenshot_ocr");
    assert_eq!(progress["status"], "queued");
    assert_eq!(progress["provider_id"], serde_json::Value::Null);
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    for secret in [
        "private-OCR-Screenshot.png",
        scope_path.to_str().expect("scope path should be UTF-8"),
        source_path.to_str().expect("source path should be UTF-8"),
        database_path
            .to_str()
            .expect("database path should be UTF-8"),
    ] {
        assert!(!stdout.contains(secret));
        assert!(!stderr.contains(secret));
    }
    assert!(
        stderr
            .lines()
            .all(|line| serde_json::from_str::<serde_json::Value>(line).is_ok())
    );
}

#[test]
fn screenshot_cleanup_groups_are_review_only_current_and_path_free_in_history() {
    let directory = tempfile::tempdir().expect("fixture root should exist");
    let database_path = directory.path().join("private-manifest.sqlite3");
    let scope_path = directory.path().join("private-screenshots");
    std::fs::create_dir(&scope_path).expect("scope should create");
    let private_ocr_text = "不得出現在候選輸出的 OCR 私密文字";
    let mut png = vec![0_u8; 32];
    png[..8].copy_from_slice(b"\x89PNG\r\n\x1a\n");
    png[8..12].copy_from_slice(&13_u32.to_be_bytes());
    png[12..16].copy_from_slice(b"IHDR");
    png[16..20].copy_from_slice(&1440_u32.to_be_bytes());
    png[20..24].copy_from_slice(&900_u32.to_be_bytes());
    let paths = [
        scope_path.join("private-shot-a.png"),
        scope_path.join("private-shot-b.png"),
    ];
    for path in &paths {
        std::fs::write(path, &png).expect("image fixture should write");
    }

    let mut database = ManifestDatabase::open(&database_path).expect("database should initialize");
    let scope = authorize_scope(&database, &scope_path).expect("scope should authorize");
    database
        .upsert_scope_access_grant(scope.id, std::env::consts::OS, b"cli-test-grant")
        .expect("active native grant should persist");
    scan_scope(&mut database, scope.id).expect("scope should scan");
    let mut node_ids = Vec::new();
    for path in &paths {
        let node_id = database
            .node_id_for_path_key(
                scope.id,
                &deskgraph_scanner::comparison_key(
                    &std::fs::canonicalize(path).expect("image should canonicalize"),
                ),
            )
            .expect("node lookup should pass")
            .expect("image should be scanned");
        node_ids.push(node_id);
    }
    drop(database);

    for node_id in &node_ids {
        let image_job = create_extraction_job_at(&database_path, scope.id, *node_id)
            .expect("image metadata job should create");
        run_extraction_job_at(
            &database_path,
            image_job.job_id,
            ExtractionLimits::default(),
        )
        .expect("image metadata should complete");
        let ocr_job = create_screenshot_ocr_job_at(&database_path, scope.id, *node_id)
            .expect("OCR job should create");
        let mut database = ManifestDatabase::open(&database_path).expect("database should reopen");
        let source = database
            .extractable_file(scope.id, *node_id)
            .expect("current image source should load");
        database
            .claim_extraction_job(ocr_job.job_id, "cli-test-ocr", 60_000)
            .expect("OCR job should claim");
        database
            .complete_extraction_job(
                ocr_job.job_id,
                "cli-test-ocr",
                "local-test-ocr",
                "1",
                source.size_bytes,
                source.modified_unix_ns,
                u64::try_from(private_ocr_text.len()).expect("text size should fit"),
                1,
                &[ContentChunkWrite {
                    ordinal: 0,
                    text: private_ocr_text.to_string(),
                    provenance: ContentChunkProvenanceWrite::OcrObservation {
                        observation_number: 1,
                        fragment_index: 0,
                        bbox_x_ppm: 0,
                        bbox_y_ppm: 0,
                        bbox_width_ppm: 1_000_000,
                        bbox_height_ppm: 1_000_000,
                        confidence_basis_points: None,
                    },
                    trust_class: "untrusted_extracted_text",
                }],
            )
            .expect("OCR provenance should complete");
    }

    let database_arg = database_path
        .to_str()
        .expect("database path should be UTF-8");
    let suggest = Command::new(env!("CARGO_BIN_EXE_deskgraph"))
        .args([
            "cleanup",
            "screenshot-groups",
            "--database",
            database_arg,
            "--scope",
            &scope.id.to_string(),
        ])
        .output()
        .expect("screenshot group discovery should start");
    assert!(suggest.status.success());
    let discovery: serde_json::Value =
        serde_json::from_slice(&suggest.stdout).expect("discovery should be JSON");
    assert_eq!(discovery["groups"].as_array().map(Vec::len), Some(1));
    assert_eq!(
        discovery["groups"][0]["members"].as_array().map(Vec::len),
        Some(2)
    );
    assert_eq!(
        discovery["groups"][0]["evidence"]["review_assistance_only"],
        true
    );
    assert_eq!(
        discovery["groups"][0]["evidence"]["cleanup_authorized"],
        false
    );
    let suggest_stdout = String::from_utf8_lossy(&suggest.stdout);
    let suggest_stderr = String::from_utf8_lossy(&suggest.stderr);
    assert!(!suggest_stdout.contains(private_ocr_text));
    assert!(!suggest_stderr.contains(private_ocr_text));
    for path in &paths {
        let path = path.to_str().expect("path should be UTF-8");
        assert!(suggest_stdout.contains(path));
        assert!(!suggest_stderr.contains(path));
    }
    let group_arg = discovery["groups"][0]["group_id"]
        .as_i64()
        .expect("group id should exist")
        .to_string();
    let status = Command::new(env!("CARGO_BIN_EXE_deskgraph"))
        .args([
            "cleanup",
            "screenshot-group-status",
            "--database",
            database_arg,
            "--group",
            &group_arg,
        ])
        .output()
        .expect("screenshot group status should start");
    assert!(status.status.success());
    let status_stdout = String::from_utf8_lossy(&status.stdout);
    let status_stderr = String::from_utf8_lossy(&status.stderr);
    assert!(!status_stdout.contains(private_ocr_text));
    assert!(!status_stderr.contains(private_ocr_text));
    for path in &paths {
        let path = path.to_str().expect("path should be UTF-8");
        assert!(status_stdout.contains(path));
        assert!(!status_stderr.contains(path));
    }

    let list = Command::new(env!("CARGO_BIN_EXE_deskgraph"))
        .args([
            "cleanup",
            "screenshot-group-list",
            "--database",
            database_arg,
        ])
        .output()
        .expect("screenshot group history should start");
    assert!(list.status.success());
    let summaries: serde_json::Value =
        serde_json::from_slice(&list.stdout).expect("history should be JSON");
    assert_eq!(summaries[0]["current_evidence"], true);
    assert_eq!(summaries[0]["verification_required"], true);
    assert_eq!(summaries[0]["cleanup_authorized"], false);
    let list_stdout = String::from_utf8_lossy(&list.stdout);
    let list_stderr = String::from_utf8_lossy(&list.stderr);
    for private_value in [
        private_ocr_text,
        "private-shot-a.png",
        "private-shot-b.png",
        scope_path.to_str().expect("scope should be UTF-8"),
    ] {
        assert!(!list_stdout.contains(private_value));
        assert!(!list_stderr.contains(private_value));
    }

    ManifestDatabase::open(&database_path)
        .expect("database should reopen")
        .mark_scope_access_grant_needs_reauthorization(scope.id)
        .expect("native grant should become inactive");
    let denied = Command::new(env!("CARGO_BIN_EXE_deskgraph"))
        .args([
            "cleanup",
            "screenshot-group-status",
            "--database",
            database_arg,
            "--group",
            &group_arg,
        ])
        .output()
        .expect("inactive screenshot group status should start");
    assert!(!denied.status.success());
    assert!(denied.stdout.is_empty());
    let denied_stderr = String::from_utf8_lossy(&denied.stderr);
    assert!(denied_stderr.contains("scope_access_grant_not_active"));
    for private_value in [
        private_ocr_text,
        "private-shot-a.png",
        "private-shot-b.png",
        scope_path.to_str().expect("scope should be UTF-8"),
        database_arg,
    ] {
        assert!(!denied_stderr.contains(private_value));
    }

    let inactive_list = Command::new(env!("CARGO_BIN_EXE_deskgraph"))
        .args([
            "cleanup",
            "screenshot-group-list",
            "--database",
            database_arg,
        ])
        .output()
        .expect("inactive screenshot group history should start");
    assert!(inactive_list.status.success());
    let inactive_summaries: serde_json::Value =
        serde_json::from_slice(&inactive_list.stdout).expect("inactive history should be JSON");
    assert_eq!(inactive_summaries[0]["current_evidence"], false);
    let inactive_stdout = String::from_utf8_lossy(&inactive_list.stdout);
    let inactive_stderr = String::from_utf8_lossy(&inactive_list.stderr);
    for private_value in [
        private_ocr_text,
        "private-shot-a.png",
        "private-shot-b.png",
        scope_path.to_str().expect("scope should be UTF-8"),
        database_arg,
    ] {
        assert!(!inactive_stdout.contains(private_value));
        assert!(!inactive_stderr.contains(private_value));
    }
}

#[test]
fn cleanup_help_exposes_review_only_screenshot_commands() {
    let output = Command::new(env!("CARGO_BIN_EXE_deskgraph"))
        .args(["cleanup", "--help"])
        .output()
        .expect("cleanup help should start");
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout).to_lowercase();
    assert!(stdout.contains("inbox"));
    assert!(stdout.contains("screenshot-groups"));
    assert!(stdout.contains("screenshot-group-status"));
    assert!(stdout.contains("screenshot-group-list"));
    for forbidden in ["trash", "delete", "execute", "move", "undo", "auto-clean"] {
        assert!(
            !stdout
                .lines()
                .any(|line| line.trim_start().starts_with(forbidden)),
            "cleanup help exposed forbidden action command {forbidden}"
        );
    }
}

#[test]
fn cleanup_inbox_revalidates_sources_without_paths_or_file_actions() {
    let directory = tempfile::tempdir().expect("fixture root should exist");
    let database_path = directory.path().join("private-manifest.sqlite3");
    let scope_path = directory.path().join("private-cleanup-inbox");
    std::fs::create_dir(&scope_path).expect("scope should create");
    let left_path = scope_path.join("secret-duplicate-left.bin");
    let right_path = scope_path.join("secret-duplicate-right.bin");
    let older_path = scope_path.join("secret-plan-v1.md");
    let newer_path = scope_path.join("secret-plan-v2.md");
    std::fs::write(&left_path, b"secret duplicate bytes").expect("left should write");
    std::fs::write(&right_path, b"secret duplicate bytes").expect("right should write");
    std::fs::write(&older_path, b"secret old plan").expect("older should write");
    std::fs::write(&newer_path, b"secret new plan").expect("newer should write");
    let mut database = ManifestDatabase::open(&database_path).expect("database should initialize");
    let scope = authorize_scope(&database, &scope_path).expect("scope should authorize");
    scan_scope(&mut database, scope.id).expect("scope should scan");
    database
        .upsert_scope_access_grant(scope.id, std::env::consts::OS, b"test-grant")
        .expect("active grant should persist");
    drop(database);

    let database_arg = database_path
        .to_str()
        .expect("database path should be UTF-8");
    let scope_arg = scope.id.to_string();
    let canonical_left = std::fs::canonicalize(&left_path).expect("left should canonicalize");
    let canonical_right = std::fs::canonicalize(&right_path).expect("right should canonicalize");
    let canonical_older = std::fs::canonicalize(&older_path).expect("older should canonicalize");
    let canonical_newer = std::fs::canonicalize(&newer_path).expect("newer should canonicalize");
    let duplicate = Command::new(env!("CARGO_BIN_EXE_deskgraph"))
        .args([
            "relation",
            "duplicate",
            "--database",
            database_arg,
            "--scope",
            &scope_arg,
            "--left",
            canonical_left.to_str().expect("left path should be UTF-8"),
            "--right",
            canonical_right
                .to_str()
                .expect("right path should be UTF-8"),
        ])
        .output()
        .expect("duplicate command should start");
    assert!(duplicate.status.success());
    let version = Command::new(env!("CARGO_BIN_EXE_deskgraph"))
        .args([
            "relation",
            "version",
            "--database",
            database_arg,
            "--scope",
            &scope_arg,
            "--first",
            canonical_older
                .to_str()
                .expect("older path should be UTF-8"),
            "--second",
            canonical_newer
                .to_str()
                .expect("newer path should be UTF-8"),
        ])
        .output()
        .expect("version command should start");
    assert!(version.status.success());

    let inbox = Command::new(env!("CARGO_BIN_EXE_deskgraph"))
        .args([
            "cleanup",
            "inbox",
            "--database",
            database_arg,
            "--scope",
            &scope_arg,
        ])
        .output()
        .expect("cleanup Inbox should start");
    assert!(inbox.status.success());
    let response: serde_json::Value =
        serde_json::from_slice(&inbox.stdout).expect("Inbox should be JSON");
    assert_eq!(response["api_version"], "deskgraph.smart-cleanup-inbox.v1");
    assert_eq!(response["items"].as_array().map(Vec::len), Some(2));
    assert_eq!(response["items"][0]["source_kind"], "exact_duplicate");
    assert_eq!(response["items"][1]["source_kind"], "version");
    assert_eq!(response["action_authorized"], false);
    let stdout = String::from_utf8_lossy(&inbox.stdout);
    let stderr = String::from_utf8_lossy(&inbox.stderr);
    for private in [
        "secret-duplicate-left",
        "secret-duplicate-right",
        "secret-plan-v1",
        "secret-plan-v2",
        "secret duplicate bytes",
        scope_path.to_str().expect("scope path should be UTF-8"),
        database_arg,
        "display_path",
        "base_key",
        "extension_key",
        "reclaimable",
    ] {
        assert!(!stdout.contains(private));
        assert!(!stderr.contains(private));
    }
    assert!(
        stderr
            .lines()
            .all(|line| serde_json::from_str::<serde_json::Value>(line).is_ok())
    );
    assert_eq!(
        std::fs::read(&left_path).expect("left should remain"),
        b"secret duplicate bytes"
    );
    assert_eq!(
        std::fs::read(&older_path).expect("older should remain"),
        b"secret old plan"
    );
}

#[test]
fn image_metadata_command_returns_only_bounded_structured_fields() {
    let directory = tempfile::tempdir().expect("fixture root should exist");
    let database_path = directory.path().join("manifest.sqlite3");
    let scope_path = directory.path().join("private-screenshots");
    std::fs::create_dir(&scope_path).expect("scope should create");
    let source_path = scope_path.join("private-Screenshot.png");
    let mut png = vec![0_u8; 32];
    png[..8].copy_from_slice(b"\x89PNG\r\n\x1a\n");
    png[8..12].copy_from_slice(&13_u32.to_be_bytes());
    png[12..16].copy_from_slice(b"IHDR");
    png[16..20].copy_from_slice(&2560_u32.to_be_bytes());
    png[20..24].copy_from_slice(&1440_u32.to_be_bytes());
    std::fs::write(&source_path, png).expect("fixture should write");
    let mut database = ManifestDatabase::open(&database_path).expect("database should initialize");
    let scope = authorize_scope(&database, &scope_path).expect("scope should authorize");
    scan_scope(&mut database, scope.id).expect("scope should scan");
    let node_id = database
        .node_id_for_path_key(
            scope.id,
            &deskgraph_scanner::comparison_key(
                &std::fs::canonicalize(&source_path).expect("source should canonicalize"),
            ),
        )
        .expect("node lookup should pass")
        .expect("image node should exist");
    drop(database);
    let job = create_extraction_job_at(&database_path, scope.id, node_id)
        .expect("image extraction should create");
    run_extraction_job_at(&database_path, job.job_id, ExtractionLimits::default())
        .expect("image extraction should complete");

    let output = Command::new(env!("CARGO_BIN_EXE_deskgraph"))
        .args([
            "extract",
            "image-metadata",
            "--database",
            database_path
                .to_str()
                .expect("database path should be UTF-8"),
            "--job",
            &job.job_id.to_string(),
        ])
        .output()
        .expect("deskgraph image metadata should start");

    assert!(output.status.success());
    let metadata: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("stdout should be JSON");
    assert_eq!(metadata["api_version"], "deskgraph.image-metadata.v1");
    assert_eq!(metadata["format"], "png");
    assert_eq!(metadata["pixel_width"], 2560);
    assert_eq!(metadata["pixel_height"], 1440);
    assert_eq!(metadata["provider_id"], "deskgraph.image-metadata");
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    for secret in [
        "private-Screenshot.png",
        scope_path.to_str().expect("scope path should be UTF-8"),
        source_path.to_str().expect("source path should be UTF-8"),
        database_path
            .to_str()
            .expect("database path should be UTF-8"),
    ] {
        assert!(!stdout.contains(secret));
        assert!(!stderr.contains(secret));
    }
    assert!(
        stderr
            .lines()
            .all(|line| serde_json::from_str::<serde_json::Value>(line).is_ok())
    );
}

#[test]
fn search_command_returns_requested_local_context_without_logging_it() {
    let directory = tempfile::tempdir().expect("fixture root should exist");
    let database_path = directory.path().join("manifest.sqlite3");
    let scope_path = directory.path().join("authorized-private");
    std::fs::create_dir(&scope_path).expect("scope should create");
    let source_path = scope_path.join("private-search-notes.md");
    let private_text = "confidentially searchable context stays local";
    std::fs::write(&source_path, private_text).expect("fixture should write");
    let mut database = ManifestDatabase::open(&database_path).expect("database should initialize");
    let scope = authorize_scope_with_access_grant(
        &mut database,
        &scope_path,
        std::env::consts::OS,
        b"cli-search-test-grant",
    )
    .expect("scope should authorize with an active test grant");
    scan_scope(&mut database, scope.id).expect("scope should scan");
    let node_id = database
        .node_id_for_path_key(
            scope.id,
            &deskgraph_scanner::comparison_key(
                &std::fs::canonicalize(&source_path).expect("source should canonicalize"),
            ),
        )
        .expect("node query should pass")
        .expect("source node should exist");
    drop(database);
    let job = create_extraction_job_at(&database_path, scope.id, node_id)
        .expect("extraction should create");
    run_extraction_job_at(&database_path, job.job_id, ExtractionLimits::default())
        .expect("extraction should complete");

    let output = Command::new(env!("CARGO_BIN_EXE_deskgraph"))
        .args([
            "search",
            "--database",
            database_path
                .to_str()
                .expect("database path should be UTF-8"),
            "--query",
            "searchable context",
            "--scope",
            &scope.id.to_string(),
            "--source",
            "content",
            "--extension",
            ".MD",
        ])
        .output()
        .expect("deskgraph search should start");

    assert!(output.status.success());
    let response: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("stdout should be JSON");
    assert_eq!(response["api_version"], "deskgraph.search.v1");
    assert_eq!(response["mode"], "lexical");
    assert_eq!(response["embeddings_enabled"], false);
    assert_eq!(response["filters"]["scope_id"], scope.id);
    assert_eq!(response["filters"]["source"], "extracted_text");
    assert_eq!(response["filters"]["extension"], "md");
    assert_eq!(response["result_count"], 1);
    assert_eq!(response["results"][0]["node_id"], node_id);
    let snippet = response["results"][0]["snippet"]
        .as_str()
        .unwrap_or_default();
    assert!(
        snippet.contains("searchable"),
        "unexpected snippet: {snippet:?}"
    );
    assert_eq!(
        response["results"][0]["explanation"],
        "extracted_text_substring"
    );

    let stderr = String::from_utf8_lossy(&output.stderr);
    for secret in [
        "searchable context",
        private_text,
        "private-search-notes.md",
        scope_path.to_str().expect("scope path should be UTF-8"),
    ] {
        assert!(!stderr.contains(secret));
    }
    assert!(
        stderr
            .lines()
            .all(|line| serde_json::from_str::<serde_json::Value>(line).is_ok())
    );
}

#[test]
fn watch_observe_persists_path_free_progress_without_logging_the_hint() {
    let directory = tempfile::tempdir().expect("fixture root should exist");
    let database_path = directory.path().join("manifest.sqlite3");
    let scope_path = directory.path().join("authorized-watch");
    std::fs::create_dir(&scope_path).expect("scope should create");
    let watched_path = scope_path.join("private-watch-notes.md");
    std::fs::write(&watched_path, "private local context").expect("fixture should write");
    let mut database = ManifestDatabase::open(&database_path).expect("database should initialize");
    let scope = authorize_scope(&database, &scope_path).expect("scope should authorize");
    scan_scope(&mut database, scope.id).expect("scope should scan");
    drop(database);

    let output = Command::new(env!("CARGO_BIN_EXE_deskgraph"))
        .args([
            "watch",
            "observe",
            "--database",
            database_path
                .to_str()
                .expect("database path should be UTF-8"),
            "--scope",
            &scope.id.to_string(),
            "--path",
            watched_path.to_str().expect("watch path should be UTF-8"),
        ])
        .output()
        .expect("deskgraph watch observe should start");

    assert!(output.status.success());
    let progress: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("stdout should be JSON");
    assert_eq!(progress["api_version"], "deskgraph.watch-event.v1");
    assert_eq!(progress["status"], "stabilizing");
    assert_eq!(progress["scope_id"], scope.id);
    assert!(progress.get("path").is_none());

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    for secret in [
        "private-watch-notes.md",
        watched_path.to_str().expect("watch path should be UTF-8"),
        scope_path.to_str().expect("scope path should be UTF-8"),
        "private local context",
    ] {
        assert!(!stdout.contains(secret));
        assert!(!stderr.contains(secret));
    }
    assert!(
        stderr
            .lines()
            .all(|line| serde_json::from_str::<serde_json::Value>(line).is_ok())
    );
}

#[test]
fn folder_profile_returns_explainable_local_facts_without_logging_paths() {
    let directory = tempfile::tempdir().expect("fixture root should exist");
    let database_path = directory.path().join("manifest.sqlite3");
    let scope_path = directory.path().join("private-project");
    let source_folder = scope_path.join("src");
    std::fs::create_dir_all(&source_folder).expect("scope should create");
    let marker_path = scope_path.join("Cargo.toml");
    let private_source = source_folder.join("private_graph.rs");
    std::fs::write(&marker_path, "[package]").expect("marker should write");
    std::fs::write(&private_source, "pub fn private_graph() {}").expect("source should write");
    let mut database = ManifestDatabase::open(&database_path).expect("database should initialize");
    let scope = authorize_scope_with_access_grant(
        &mut database,
        &scope_path,
        std::env::consts::OS,
        b"cli-folder-profile-test-grant",
    )
    .expect("scope should authorize with an active test grant");
    scan_scope(&mut database, scope.id).expect("scope should scan");
    drop(database);

    let output = Command::new(env!("CARGO_BIN_EXE_deskgraph"))
        .args([
            "folder",
            "profile",
            "--database",
            database_path
                .to_str()
                .expect("database path should be UTF-8"),
            "--scope",
            &scope.id.to_string(),
            "--path",
            scope_path.to_str().expect("scope path should be UTF-8"),
        ])
        .output()
        .expect("deskgraph folder profile should start");

    assert!(output.status.success());
    let profile: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("stdout should be JSON");
    assert_eq!(profile["api_version"], "deskgraph.folder-profile.v1");
    assert_eq!(profile["scope_id"], scope.id);
    assert_eq!(
        profile["display_path"],
        std::fs::canonicalize(&scope_path)
            .expect("scope should canonicalize")
            .to_string_lossy()
            .as_ref()
    );
    assert_eq!(profile["descendant_file_count"], 2);
    assert_eq!(profile["descendant_folder_count"], 1);
    assert_eq!(profile["project_suggestion"]["created_by"], "system_rule");
    assert_eq!(
        profile["project_suggestion"]["provenance"][0]["kind"],
        "cargo_manifest"
    );
    assert_eq!(
        profile["project_suggestion"]["confidence_basis_points"],
        8_500
    );
    assert_eq!(
        profile["project_suggestion"]["model_version"],
        serde_json::Value::Null
    );
    assert!(marker_path.exists());
    assert!(private_source.exists());

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(!stdout.contains("private_graph.rs"));
    for secret in [
        "private-project",
        "private_graph.rs",
        scope_path.to_str().expect("scope path should be UTF-8"),
        private_source
            .to_str()
            .expect("source path should be UTF-8"),
    ] {
        assert!(!stderr.contains(secret));
    }
    assert!(
        stderr
            .lines()
            .all(|line| serde_json::from_str::<serde_json::Value>(line).is_ok())
    );
}

#[test]
fn project_feedback_is_durable_correctable_and_path_free_in_list_and_logs() {
    let directory = tempfile::tempdir().expect("fixture root should exist");
    let database_path = directory.path().join("manifest.sqlite3");
    let scope_path = directory.path().join("private-correctable-project");
    let source_folder = scope_path.join("src");
    std::fs::create_dir_all(&source_folder).expect("scope should create");
    let marker_path = scope_path.join("Cargo.toml");
    let private_source = source_folder.join("secret_context.rs");
    std::fs::write(&marker_path, "[package]").expect("marker should write");
    std::fs::write(&private_source, "pub fn secret_context() {}").expect("source should write");
    let mut database = ManifestDatabase::open(&database_path).expect("database should initialize");
    let scope = authorize_scope(&database, &scope_path).expect("scope should authorize");
    scan_scope(&mut database, scope.id).expect("scope should scan");
    database
        .upsert_scope_access_grant(scope.id, std::env::consts::OS, b"project-cli-test-grant")
        .expect("test grant should activate");
    drop(database);

    let database_arg = database_path
        .to_str()
        .expect("database path should be UTF-8");
    let scope_arg = scope.id.to_string();
    let path_arg = scope_path.to_str().expect("scope path should be UTF-8");
    let run = |arguments: &[&str]| {
        Command::new(env!("CARGO_BIN_EXE_deskgraph"))
            .args(arguments)
            .output()
            .expect("deskgraph project command should start")
    };

    let proposed = run(&[
        "project",
        "propose",
        "--database",
        database_arg,
        "--scope",
        &scope_arg,
        "--path",
        path_arg,
    ]);
    assert!(proposed.status.success());
    let proposed_json: serde_json::Value =
        serde_json::from_slice(&proposed.stdout).expect("proposal should be JSON");
    assert_eq!(
        proposed_json["api_version"],
        "deskgraph.project-candidate.v1"
    );
    assert_eq!(proposed_json["state"], "suggested");
    assert_eq!(proposed_json["latest_decision"], serde_json::Value::Null);
    let project_id = proposed_json["project_id"]
        .as_i64()
        .expect("project id should exist");
    let project_arg = project_id.to_string();

    let rejected = run(&[
        "project",
        "decide",
        "--database",
        database_arg,
        "--scope",
        &scope_arg,
        "--project",
        &project_arg,
        "--decision",
        "reject",
    ]);
    assert!(rejected.status.success());
    let rejected_json: serde_json::Value =
        serde_json::from_slice(&rejected.stdout).expect("rejection should be JSON");
    assert_eq!(rejected_json["state"], "rejected");
    assert_eq!(rejected_json["latest_decision"]["sequence"], 1);
    assert_eq!(rejected_json["latest_decision"]["created_by"], "user");

    let proposed_again = run(&[
        "project",
        "propose",
        "--database",
        database_arg,
        "--scope",
        &scope_arg,
        "--path",
        path_arg,
    ]);
    assert!(proposed_again.status.success());
    let proposed_again_json: serde_json::Value =
        serde_json::from_slice(&proposed_again.stdout).expect("proposal should be JSON");
    assert_eq!(proposed_again_json["project_id"], project_id);
    assert_eq!(proposed_again_json["state"], "rejected");

    let accepted = run(&[
        "project",
        "decide",
        "--database",
        database_arg,
        "--scope",
        &scope_arg,
        "--project",
        &project_arg,
        "--decision",
        "accept",
    ]);
    assert!(accepted.status.success());
    let accepted_json: serde_json::Value =
        serde_json::from_slice(&accepted.stdout).expect("acceptance should be JSON");
    assert_eq!(accepted_json["state"], "accepted");
    assert_eq!(accepted_json["latest_decision"]["sequence"], 2);

    let accepted_again = run(&[
        "project",
        "decide",
        "--database",
        database_arg,
        "--scope",
        &scope_arg,
        "--project",
        &project_arg,
        "--decision",
        "accept",
    ]);
    assert!(accepted_again.status.success());
    let accepted_again_json: serde_json::Value =
        serde_json::from_slice(&accepted_again.stdout).expect("acceptance should be JSON");
    assert_eq!(accepted_again_json["latest_decision"]["sequence"], 2);

    let list = run(&["project", "list", "--database", database_arg]);
    assert!(list.status.success());
    let summaries: serde_json::Value =
        serde_json::from_slice(&list.stdout).expect("candidate list should be JSON");
    assert_eq!(summaries[0]["project_id"], project_id);
    assert_eq!(summaries[0]["state"], "accepted");
    assert!(summaries[0].get("display_path").is_none());
    assert!(summaries[0].get("suggestion").is_none());

    assert!(marker_path.exists());
    assert!(private_source.exists());
    let list_stdout = String::from_utf8_lossy(&list.stdout);
    for output in [
        &proposed,
        &rejected,
        &proposed_again,
        &accepted,
        &accepted_again,
        &list,
    ] {
        let stderr = String::from_utf8_lossy(&output.stderr);
        for secret in [
            "private-correctable-project",
            "secret_context.rs",
            path_arg,
            private_source
                .to_str()
                .expect("source path should be UTF-8"),
        ] {
            assert!(!stderr.contains(secret));
            assert!(!list_stdout.contains(secret));
        }
        assert!(
            stderr
                .lines()
                .all(|line| serde_json::from_str::<serde_json::Value>(line).is_ok())
        );
    }

    ManifestDatabase::open(&database_path)
        .expect("database should reopen")
        .mark_scope_access_grant_revoked(scope.id)
        .expect("test grant should revoke");
    let revoked_propose = run(&[
        "project",
        "propose",
        "--database",
        database_arg,
        "--scope",
        &scope_arg,
        "--path",
        path_arg,
    ]);
    let revoked_status = run(&[
        "project",
        "status",
        "--database",
        database_arg,
        "--scope",
        &scope_arg,
        "--project",
        &project_arg,
    ]);
    let revoked_decide = run(&[
        "project",
        "decide",
        "--database",
        database_arg,
        "--scope",
        &scope_arg,
        "--project",
        &project_arg,
        "--decision",
        "reject",
    ]);
    for denied in [&revoked_propose, &revoked_status, &revoked_decide] {
        assert!(!denied.status.success());
        assert!(denied.stdout.is_empty());
        let stderr = String::from_utf8_lossy(&denied.stderr);
        assert!(stderr.contains("scope_access_grant_not_active"));
        assert!(!stderr.contains(path_arg));
        assert!(!stderr.contains("secret_context.rs"));
    }
    let candidate = ManifestDatabase::open(&database_path)
        .expect("database should reopen")
        .project_candidate(project_id)
        .expect("candidate should remain readable to the test");
    assert_eq!(
        candidate.state,
        deskgraph_domain::ProjectCandidateState::Accepted
    );
}

#[test]
fn exact_duplicate_relation_is_explicit_revalidated_and_path_free_in_logs() {
    let directory = tempfile::tempdir().expect("fixture root should exist");
    let database_path = directory.path().join("manifest.sqlite3");
    let scope_path = directory.path().join("private-duplicates");
    std::fs::create_dir(&scope_path).expect("scope should create");
    let left_path = scope_path.join("private-left.bin");
    let right_path = scope_path.join("private-right.bin");
    let private_content = b"private duplicate local context";
    std::fs::write(&left_path, private_content).expect("left should write");
    std::fs::write(&right_path, private_content).expect("right should write");
    let mut database = ManifestDatabase::open(&database_path).expect("database should initialize");
    let scope = authorize_scope_with_access_grant(
        &mut database,
        &scope_path,
        std::env::consts::OS,
        b"cli-duplicate-test-grant",
    )
    .expect("scope should authorize with an active test grant");
    scan_scope(&mut database, scope.id).expect("scope should scan");
    drop(database);

    let database_arg = database_path
        .to_str()
        .expect("database path should be UTF-8");
    let scope_arg = scope.id.to_string();
    let canonical_left = std::fs::canonicalize(&left_path).expect("left should canonicalize");
    let canonical_right = std::fs::canonicalize(&right_path).expect("right should canonicalize");
    let left_arg = canonical_left.to_str().expect("left path should be UTF-8");
    let right_arg = canonical_right
        .to_str()
        .expect("right path should be UTF-8");
    let duplicate = Command::new(env!("CARGO_BIN_EXE_deskgraph"))
        .args([
            "relation",
            "duplicate",
            "--database",
            database_arg,
            "--scope",
            &scope_arg,
            "--left",
            left_arg,
            "--right",
            right_arg,
        ])
        .output()
        .expect("deskgraph relation duplicate should start");
    assert!(duplicate.status.success());
    let candidate: serde_json::Value =
        serde_json::from_slice(&duplicate.stdout).expect("candidate should be JSON");
    assert_eq!(
        candidate["api_version"],
        "deskgraph.file-relation-candidate.v1"
    );
    assert_eq!(candidate["kind"], "exact_duplicate");
    assert_eq!(candidate["state"], "suggested");
    assert_eq!(candidate["evidence"]["comparison_kind"], "byte_for_byte");
    assert_eq!(candidate["evidence"]["confidence_basis_points"], 10_000);
    assert_eq!(
        candidate["evidence"]["compared_bytes"],
        u64::try_from(private_content.len()).expect("fixture size should fit")
    );
    assert_eq!(
        candidate["evidence"]["model_version"],
        serde_json::Value::Null
    );
    let returned_paths = [
        candidate["left"]["display_path"]
            .as_str()
            .expect("left path should exist"),
        candidate["right"]["display_path"]
            .as_str()
            .expect("right path should exist"),
    ];
    assert!(returned_paths.contains(&canonical_left.to_string_lossy().as_ref()));
    assert!(returned_paths.contains(&canonical_right.to_string_lossy().as_ref()));

    let relation_arg = candidate["relation_id"]
        .as_i64()
        .expect("relation id should exist")
        .to_string();
    let rejected = Command::new(env!("CARGO_BIN_EXE_deskgraph"))
        .args([
            "relation",
            "decide",
            "--database",
            database_arg,
            "--relation",
            &relation_arg,
            "--decision",
            "reject",
        ])
        .output()
        .expect("deskgraph relation reject should start");
    assert!(rejected.status.success());
    let rejected_json: serde_json::Value =
        serde_json::from_slice(&rejected.stdout).expect("rejection should be JSON");
    assert_eq!(rejected_json["state"], "rejected");
    assert_eq!(rejected_json["latest_decision"]["sequence"], 1);

    let duplicate_again = Command::new(env!("CARGO_BIN_EXE_deskgraph"))
        .args([
            "relation",
            "duplicate",
            "--database",
            database_arg,
            "--scope",
            &scope_arg,
            "--left",
            left_arg,
            "--right",
            right_arg,
        ])
        .output()
        .expect("deskgraph relation duplicate should restart");
    assert!(duplicate_again.status.success());
    let duplicate_again_json: serde_json::Value = serde_json::from_slice(&duplicate_again.stdout)
        .expect("rechecked candidate should be JSON");
    assert_eq!(duplicate_again_json["state"], "rejected");
    assert_eq!(
        duplicate_again_json["relation_id"],
        candidate["relation_id"]
    );

    let accepted = Command::new(env!("CARGO_BIN_EXE_deskgraph"))
        .args([
            "relation",
            "decide",
            "--database",
            database_arg,
            "--relation",
            &relation_arg,
            "--decision",
            "accept",
        ])
        .output()
        .expect("deskgraph relation accept should start");
    assert!(accepted.status.success());
    let accepted_json: serde_json::Value =
        serde_json::from_slice(&accepted.stdout).expect("acceptance should be JSON");
    assert_eq!(accepted_json["state"], "accepted");
    assert_eq!(accepted_json["latest_decision"]["sequence"], 2);

    let accepted_again = Command::new(env!("CARGO_BIN_EXE_deskgraph"))
        .args([
            "relation",
            "decide",
            "--database",
            database_arg,
            "--relation",
            &relation_arg,
            "--decision",
            "accept",
        ])
        .output()
        .expect("deskgraph repeated relation accept should start");
    assert!(accepted_again.status.success());
    let accepted_again_json: serde_json::Value =
        serde_json::from_slice(&accepted_again.stdout).expect("repeated acceptance should be JSON");
    assert_eq!(accepted_again_json["state"], "accepted");
    assert_eq!(accepted_again_json["latest_decision"]["sequence"], 2);

    let verified = Command::new(env!("CARGO_BIN_EXE_deskgraph"))
        .args([
            "relation",
            "verify",
            "--database",
            database_arg,
            "--relation",
            &relation_arg,
        ])
        .output()
        .expect("deskgraph relation verify should start");
    assert!(verified.status.success());
    let verified_json: serde_json::Value =
        serde_json::from_slice(&verified.stdout).expect("verification should be JSON");
    assert_eq!(verified_json["relation_id"], candidate["relation_id"]);
    assert_eq!(verified_json["state"], "accepted");

    let list = Command::new(env!("CARGO_BIN_EXE_deskgraph"))
        .args(["relation", "list", "--database", database_arg])
        .output()
        .expect("deskgraph relation list should start");
    assert!(list.status.success());
    let summaries: serde_json::Value =
        serde_json::from_slice(&list.stdout).expect("relation list should be JSON");
    assert_eq!(summaries[0]["relation_id"], candidate["relation_id"]);
    assert_eq!(summaries[0]["state"], "accepted");
    assert_eq!(summaries[0]["verification_required"], true);
    assert!(summaries[0].get("left").is_none());
    assert!(summaries[0].get("right").is_none());
    assert!(summaries[0].get("display_path").is_none());

    assert_eq!(
        std::fs::read(&left_path).expect("left should remain"),
        private_content
    );
    assert_eq!(
        std::fs::read(&right_path).expect("right should remain"),
        private_content
    );
    let list_stdout = String::from_utf8_lossy(&list.stdout);
    for output in [
        &duplicate,
        &rejected,
        &duplicate_again,
        &accepted,
        &accepted_again,
        &verified,
        &list,
    ] {
        let stderr = String::from_utf8_lossy(&output.stderr);
        for secret in [
            "private-duplicates",
            "private-left.bin",
            "private-right.bin",
            left_arg,
            right_arg,
            database_arg,
            "private duplicate local context",
        ] {
            assert!(!stderr.contains(secret));
            assert!(!list_stdout.contains(secret));
        }
        assert!(
            stderr
                .lines()
                .all(|line| serde_json::from_str::<serde_json::Value>(line).is_ok())
        );
    }
}

#[test]
fn file_version_relation_is_directional_revalidated_and_path_free_in_history() {
    let directory = tempfile::tempdir().expect("fixture root should exist");
    let database_path = directory.path().join("manifest.sqlite3");
    let scope_path = directory.path().join("private-versions");
    std::fs::create_dir(&scope_path).expect("scope should create");
    let older_path = scope_path.join("private-roadmap-v1.MD");
    let newer_path = scope_path.join("private-roadmap_V2.md");
    let older_content = b"private older roadmap";
    let newer_content = b"private newer roadmap with different bytes";
    std::fs::write(&older_path, older_content).expect("older should write");
    std::fs::write(&newer_path, newer_content).expect("newer should write");
    let mut database = ManifestDatabase::open(&database_path).expect("database should initialize");
    let scope = authorize_scope_with_access_grant(
        &mut database,
        &scope_path,
        std::env::consts::OS,
        b"cli-version-test-grant",
    )
    .expect("scope should authorize with an active test grant");
    scan_scope(&mut database, scope.id).expect("scope should scan");
    drop(database);

    let database_arg = database_path
        .to_str()
        .expect("database path should be UTF-8");
    let scope_arg = scope.id.to_string();
    let canonical_older = std::fs::canonicalize(&older_path).expect("older should canonicalize");
    let canonical_newer = std::fs::canonicalize(&newer_path).expect("newer should canonicalize");
    let older_arg = canonical_older
        .to_str()
        .expect("older path should be UTF-8");
    let newer_arg = canonical_newer
        .to_str()
        .expect("newer path should be UTF-8");
    let suggested = Command::new(env!("CARGO_BIN_EXE_deskgraph"))
        .args([
            "relation",
            "version",
            "--database",
            database_arg,
            "--scope",
            &scope_arg,
            "--first",
            newer_arg,
            "--second",
            older_arg,
        ])
        .output()
        .expect("deskgraph relation version should start");
    assert!(suggested.status.success());
    let candidate: serde_json::Value =
        serde_json::from_slice(&suggested.stdout).expect("candidate should be JSON");
    assert_eq!(
        candidate["api_version"],
        "deskgraph.file-version-candidate.v2"
    );
    assert_eq!(candidate["kind"], "version");
    assert_eq!(candidate["state"], "suggested");
    assert_eq!(
        candidate["evidence"]["signal_kind"],
        "explicit_numeric_suffix"
    );
    assert_eq!(candidate["evidence"]["base_key"], "private-roadmap");
    assert_eq!(candidate["evidence"]["extension_key"], "md");
    assert_eq!(candidate["evidence"]["older_version"], 1);
    assert_eq!(candidate["evidence"]["newer_version"], 2);
    assert_eq!(candidate["evidence"]["confidence_basis_points"], 9_000);
    assert_eq!(candidate["older"]["display_path"], older_arg);
    assert_eq!(candidate["newer"]["display_path"], newer_arg);
    assert_eq!(candidate["latest_decision"], serde_json::Value::Null);

    let relation_arg = candidate["relation_id"]
        .as_i64()
        .expect("relation id should exist")
        .to_string();
    let verified = Command::new(env!("CARGO_BIN_EXE_deskgraph"))
        .args([
            "relation",
            "version-verify",
            "--database",
            database_arg,
            "--relation",
            &relation_arg,
        ])
        .output()
        .expect("deskgraph relation version verify should start");
    assert!(verified.status.success());
    let verified_json: serde_json::Value =
        serde_json::from_slice(&verified.stdout).expect("verification should be JSON");
    assert_eq!(verified_json["relation_id"], candidate["relation_id"]);
    assert_eq!(verified_json["state"], "suggested");

    let rejected = Command::new(env!("CARGO_BIN_EXE_deskgraph"))
        .args([
            "relation",
            "version-decide",
            "--database",
            database_arg,
            "--relation",
            &relation_arg,
            "--decision",
            "reject",
        ])
        .output()
        .expect("deskgraph relation version decide should start");
    assert!(rejected.status.success());
    let rejected_json: serde_json::Value =
        serde_json::from_slice(&rejected.stdout).expect("decision should be JSON");
    assert_eq!(rejected_json["state"], "rejected");
    assert_eq!(rejected_json["latest_decision"]["sequence"], 1);
    assert!(
        rejected_json["latest_decision"]["evidence_observation_id"]
            .as_i64()
            .is_some_and(|value| value > 0)
    );

    let repeated = Command::new(env!("CARGO_BIN_EXE_deskgraph"))
        .args([
            "relation",
            "version-decide",
            "--database",
            database_arg,
            "--relation",
            &relation_arg,
            "--decision",
            "reject",
        ])
        .output()
        .expect("repeated version decision should start");
    assert!(repeated.status.success());
    let repeated_json: serde_json::Value =
        serde_json::from_slice(&repeated.stdout).expect("repeated decision should be JSON");
    assert_eq!(repeated_json["latest_decision"]["sequence"], 1);

    let accepted = Command::new(env!("CARGO_BIN_EXE_deskgraph"))
        .args([
            "relation",
            "version-decide",
            "--database",
            database_arg,
            "--relation",
            &relation_arg,
            "--decision",
            "accept",
        ])
        .output()
        .expect("corrected version decision should start");
    assert!(accepted.status.success());
    let accepted_json: serde_json::Value =
        serde_json::from_slice(&accepted.stdout).expect("correction should be JSON");
    assert_eq!(accepted_json["state"], "accepted");
    assert_eq!(accepted_json["latest_decision"]["sequence"], 2);

    let list = Command::new(env!("CARGO_BIN_EXE_deskgraph"))
        .args(["relation", "list", "--database", database_arg])
        .output()
        .expect("deskgraph relation list should start");
    assert!(list.status.success());
    let summaries: serde_json::Value =
        serde_json::from_slice(&list.stdout).expect("relation list should be JSON");
    assert_eq!(summaries[0]["relation_id"], candidate["relation_id"]);
    assert_eq!(summaries[0]["kind"], "version");
    assert_eq!(summaries[0]["state"], "accepted");
    assert!(summaries[0]["latest_decision_at_unix_ms"].is_i64());
    assert_eq!(summaries[0]["verification_required"], true);
    assert!(summaries[0].get("older").is_none());
    assert!(summaries[0].get("newer").is_none());
    assert!(summaries[0].get("evidence").is_none());

    assert_eq!(
        std::fs::read(&older_path).expect("older should remain"),
        older_content
    );
    assert_eq!(
        std::fs::read(&newer_path).expect("newer should remain"),
        newer_content
    );
    let list_stdout = String::from_utf8_lossy(&list.stdout);
    for output in [
        &suggested, &verified, &rejected, &repeated, &accepted, &list,
    ] {
        let stderr = String::from_utf8_lossy(&output.stderr);
        for secret in [
            "private-versions",
            "private-roadmap",
            "private-roadmap-v1.MD",
            "private-roadmap_V2.md",
            older_arg,
            newer_arg,
            database_arg,
            "private older roadmap",
            "private newer roadmap with different bytes",
        ] {
            assert!(!stderr.contains(secret));
            assert!(!list_stdout.contains(secret));
        }
        assert!(
            stderr
                .lines()
                .all(|line| serde_json::from_str::<serde_json::Value>(line).is_ok())
        );
    }
}

#[test]
fn rename_preview_returns_explicit_paths_without_logging_or_changing_files() {
    let directory = tempfile::tempdir().expect("fixture root should exist");
    let database_path = directory.path().join("manifest.sqlite3");
    let scope_path = directory.path().join("authorized-actions");
    std::fs::create_dir(&scope_path).expect("scope should create");
    let source_path = scope_path.join("private-draft.txt");
    let destination_path = scope_path.join("private-final.txt");
    std::fs::write(&source_path, "private local action fixture").expect("fixture should write");
    let mut database = ManifestDatabase::open(&database_path).expect("database should initialize");
    let scope = authorize_scope_with_access_grant(
        &mut database,
        &scope_path,
        std::env::consts::OS,
        b"cli-rename-preview-test-grant",
    )
    .expect("scope should authorize with an active test grant");
    scan_scope(&mut database, scope.id).expect("scope should scan");
    drop(database);

    let output = Command::new(env!("CARGO_BIN_EXE_deskgraph"))
        .args([
            "organize",
            "rename-preview",
            "--database",
            database_path
                .to_str()
                .expect("database path should be UTF-8"),
            "--scope",
            &scope.id.to_string(),
            "--source",
            source_path.to_str().expect("source path should be UTF-8"),
            "--new-name",
            "private-final.txt",
        ])
        .output()
        .expect("deskgraph rename preview should start");

    assert!(output.status.success());
    let preview: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("stdout should be JSON");
    assert_eq!(preview["api_version"], "deskgraph.action-plan.v2");
    assert_eq!(preview["operation"], "rename");
    assert_eq!(preview["state"], "previewed");
    let canonical_source = std::fs::canonicalize(&source_path).expect("source should canonicalize");
    let canonical_destination = canonical_source
        .parent()
        .expect("source should have parent")
        .join("private-final.txt");
    assert_eq!(
        preview["source_path"],
        canonical_source.to_string_lossy().as_ref()
    );
    assert_eq!(
        preview["destination_path"],
        canonical_destination.to_string_lossy().as_ref()
    );
    assert_eq!(preview["policy"]["decision"], "allowed");
    assert_eq!(preview["journal_sequence"], 1);
    assert!(source_path.exists());
    assert!(!destination_path.exists());

    let stderr = String::from_utf8_lossy(&output.stderr);
    for secret in [
        "private-draft.txt",
        "private-final.txt",
        scope_path.to_str().expect("scope path should be UTF-8"),
        source_path.to_str().expect("source path should be UTF-8"),
    ] {
        assert!(!stderr.contains(secret));
    }
    assert!(
        stderr
            .lines()
            .all(|line| serde_json::from_str::<serde_json::Value>(line).is_ok())
    );

    let list = Command::new(env!("CARGO_BIN_EXE_deskgraph"))
        .args([
            "organize",
            "list",
            "--database",
            database_path
                .to_str()
                .expect("database path should be UTF-8"),
        ])
        .output()
        .expect("deskgraph action list should start");
    assert!(list.status.success());
    let summaries: serde_json::Value =
        serde_json::from_slice(&list.stdout).expect("summary stdout should be JSON");
    assert_eq!(summaries[0]["plan_id"], preview["plan_id"]);
    assert!(summaries[0].get("source_path").is_none());
    assert!(summaries[0].get("destination_path").is_none());
    let list_stdout = String::from_utf8_lossy(&list.stdout);
    let list_stderr = String::from_utf8_lossy(&list.stderr);
    for secret in [
        "private-draft.txt",
        "private-final.txt",
        canonical_source.to_str().expect("source should be UTF-8"),
    ] {
        assert!(!list_stdout.contains(secret));
        assert!(!list_stderr.contains(secret));
    }
}

#[test]
fn organizer_help_exposes_preview_and_history_but_no_execution_commands() {
    let output = Command::new(env!("CARGO_BIN_EXE_deskgraph"))
        .args(["organize", "--help"])
        .output()
        .expect("deskgraph organizer help should start");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("rename-preview"));
    assert!(stdout.contains("status"));
    assert!(stdout.contains("list"));
    assert!(!stdout.contains("rename-execute"));
    assert!(!stdout.contains("rename-undo"));
    assert!(!stdout.contains("recover"));
}
