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
