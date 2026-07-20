use deskgraph_database::ManifestDatabase;
use deskgraph_domain::ActionPlanState;
use deskgraph_scanner::{authorize_scope, scan_scope};
use deskgraph_transactions::{
    action_plan_at, create_rename_preview_at, execute_rename_at, recover_rename_actions_at,
    undo_rename_at,
};
use std::ffi::OsString;
use std::path::Path;

fn directory_entries(path: &Path) -> Vec<OsString> {
    let mut entries = std::fs::read_dir(path)
        .expect("fixture directory should remain readable")
        .map(|entry| {
            entry
                .expect("fixture entry should remain readable")
                .file_name()
        })
        .collect::<Vec<_>>();
    entries.sort();
    entries
}

#[test]
fn production_action_apis_fail_closed_before_database_or_filesystem_side_effects() {
    let directory = tempfile::tempdir().expect("fixture root should exist");
    let database_path = directory.path().join("manifest.sqlite3");
    let scope_path = directory.path().join("authorized");
    std::fs::create_dir(&scope_path).expect("scope should create");
    let source_path = scope_path.join("Draft.txt");
    let destination_path = scope_path.join("Final.txt");
    std::fs::write(&source_path, "production gate fixture").expect("source should write");

    let mut database = ManifestDatabase::open(&database_path).expect("database should initialize");
    let scope = authorize_scope(&database, &scope_path).expect("scope should authorize");
    scan_scope(&mut database, scope.id).expect("scope should scan");
    database
        .upsert_scope_access_grant(
            scope.id,
            std::env::consts::OS,
            b"production-gate-active-grant",
        )
        .expect("scope should retain an active runtime grant");
    drop(database);

    let preview = create_rename_preview_at(&database_path, scope.id, &source_path, "Final.txt")
        .expect("preview should remain available");
    let database_before = std::fs::read(&database_path).expect("database should be readable");
    let entries_before = directory_entries(directory.path());

    let execute_error =
        execute_rename_at(&database_path, preview.plan_id, "integration-execute-01")
            .expect_err("unaccepted production execute must fail closed");
    let undo_error = undo_rename_at(&database_path, preview.plan_id, "integration-undo-01")
        .expect_err("unaccepted production undo must fail closed");
    let recovery_error = recover_rename_actions_at(&database_path)
        .expect_err("unaccepted production recovery must fail closed");

    assert_eq!(execute_error.code(), "action_platform_rename_unsupported");
    assert_eq!(undo_error.code(), "action_platform_rename_unsupported");
    assert_eq!(recovery_error.code(), "action_platform_rename_unsupported");
    assert!(source_path.exists());
    assert!(!destination_path.exists());
    assert_eq!(
        std::fs::read(&database_path).expect("database should remain readable"),
        database_before,
        "unsupported action APIs must not change the SQLite main file"
    );
    assert_eq!(
        directory_entries(directory.path()),
        entries_before,
        "unsupported action APIs must not create WAL, SHM, lock, or other sidecars"
    );
    let persisted = action_plan_at(&database_path, preview.plan_id)
        .expect("preview journal should remain readable");
    assert_eq!(persisted.state, ActionPlanState::Previewed);
    assert_eq!(persisted.journal_sequence, 1);
}
