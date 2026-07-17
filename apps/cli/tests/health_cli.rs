use std::process::Command;

use deskgraph_database::ManifestDatabase;
use deskgraph_extractors::{ExtractionLimits, create_extraction_job_at, run_extraction_job_at};
use deskgraph_scanner::{authorize_scope, scan_scope};

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
    let scope = authorize_scope(&database, &scope_path).expect("scope should authorize");
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
    let scope = authorize_scope(&database, &scope_path).expect("scope should authorize");
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
    let scope = authorize_scope(&database, &scope_path).expect("scope should authorize");
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
    let scope = authorize_scope(&database, &scope_path).expect("scope should authorize");
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
    let scope = authorize_scope(&database, &scope_path).expect("scope should authorize");
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
    assert_eq!(preview["api_version"], "deskgraph.action-plan.v1");
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
