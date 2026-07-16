use std::process::Command;

use deskgraph_database::ManifestDatabase;
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
