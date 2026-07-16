use std::fmt;
use std::path::Path;

use deskgraph_database::{DatabaseError, FolderProfileFacts, ManifestDatabase};
use deskgraph_domain::{
    FolderProfile, ProjectSignal, ProjectSignalKind, ProjectSuggestion, ProjectSuggestionCreator,
};

const DEFAULT_ENTRY_LIMIT: u64 = 100_000;

#[derive(Debug)]
pub enum ProjectError {
    Database(DatabaseError),
}

impl ProjectError {
    #[must_use]
    pub fn code(&self) -> &'static str {
        match self {
            Self::Database(error) => error.code(),
        }
    }
}

impl fmt::Display for ProjectError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.code())
    }
}

impl std::error::Error for ProjectError {}

impl From<DatabaseError> for ProjectError {
    fn from(error: DatabaseError) -> Self {
        Self::Database(error)
    }
}

pub fn folder_profile_at(
    database_path: &Path,
    scope_id: i64,
    folder_node_id: i64,
) -> Result<FolderProfile, ProjectError> {
    let database = ManifestDatabase::open(database_path)?;
    folder_profile(&database, scope_id, folder_node_id)
}

pub fn folder_profile(
    database: &ManifestDatabase,
    scope_id: i64,
    folder_node_id: i64,
) -> Result<FolderProfile, ProjectError> {
    folder_profile_with_limit(database, scope_id, folder_node_id, DEFAULT_ENTRY_LIMIT)
}

fn folder_profile_with_limit(
    database: &ManifestDatabase,
    scope_id: i64,
    folder_node_id: i64,
    entry_limit: u64,
) -> Result<FolderProfile, ProjectError> {
    let facts = database.folder_profile_facts(scope_id, folder_node_id, entry_limit)?;
    Ok(profile_from_facts(facts))
}

fn profile_from_facts(facts: FolderProfileFacts) -> FolderProfile {
    let provenance = facts
        .project_markers
        .iter()
        .copied()
        .map(project_signal)
        .collect::<Vec<_>>();
    let strong_weights = provenance
        .iter()
        .filter(|signal| signal.kind != ProjectSignalKind::Readme)
        .map(|signal| signal.weight_basis_points)
        .collect::<Vec<_>>();
    let project_suggestion = strong_weights.iter().copied().max().map(|maximum| {
        let additional = u16::try_from(strong_weights.len().saturating_sub(1))
            .unwrap_or(u16::MAX)
            .saturating_mul(500);
        ProjectSuggestion {
            confidence_basis_points: maximum.saturating_add(additional).min(9_500),
            provenance,
            observed_at_unix_ms: facts.observed_at_unix_ms,
            created_by: ProjectSuggestionCreator::SystemRule,
            provider_id: ProjectSuggestion::PROVIDER_ID,
            provider_version: ProjectSuggestion::PROVIDER_VERSION,
            model_version: None,
        }
    });
    FolderProfile {
        api_version: FolderProfile::API_VERSION,
        scope_id: facts.scope_id,
        folder_node_id: facts.folder_node_id,
        folder_location_id: facts.folder_location_id,
        display_path: facts.display_path,
        direct_file_count: facts.direct_file_count,
        direct_folder_count: facts.direct_folder_count,
        descendant_file_count: facts.descendant_file_count,
        descendant_folder_count: facts.descendant_folder_count,
        total_file_bytes: facts.total_file_bytes,
        latest_modified_unix_ns: facts.latest_modified_unix_ns,
        file_categories: facts.file_categories,
        project_suggestion,
        observed_at_unix_ms: facts.observed_at_unix_ms,
        bounded_entry_limit: facts.bounded_entry_limit,
    }
}

fn project_signal(kind: ProjectSignalKind) -> ProjectSignal {
    let (marker_name, weight_basis_points) = match kind {
        ProjectSignalKind::CargoManifest => ("Cargo.toml", 8_500),
        ProjectSignalKind::JavaScriptPackage => ("package.json", 7_500),
        ProjectSignalKind::PythonProject => ("pyproject.toml", 8_000),
        ProjectSignalKind::GoModule => ("go.mod", 8_500),
        ProjectSignalKind::SwiftPackage => ("Package.swift", 8_500),
        ProjectSignalKind::XcodeProject => ("*.xcodeproj", 9_000),
        ProjectSignalKind::VisualStudioSolution => ("*.sln", 8_500),
        ProjectSignalKind::Readme => ("README", 1_500),
    };
    ProjectSignal {
        kind,
        marker_name: marker_name.to_string(),
        weight_basis_points,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use deskgraph_domain::FolderFileCategory;
    use deskgraph_scanner::{authorize_scope, comparison_key, scan_scope};

    struct Fixture {
        _directory: tempfile::TempDir,
        database: ManifestDatabase,
        scope_id: i64,
        root_node_id: i64,
        source_node_id: i64,
    }

    impl Fixture {
        fn new() -> Self {
            let directory = tempfile::tempdir().expect("fixture root should exist");
            let scope_path = directory.path().join("sample-project");
            let source_folder = scope_path.join("src");
            let asset_folder = scope_path.join("assets");
            std::fs::create_dir_all(&source_folder).expect("source folder should create");
            std::fs::create_dir(&asset_folder).expect("asset folder should create");
            std::fs::write(scope_path.join("Cargo.toml"), "[package]")
                .expect("Cargo marker should write");
            std::fs::write(scope_path.join("README.md"), "project docs")
                .expect("README should write");
            std::fs::write(scope_path.join("records.csv"), "a,b").expect("data should write");
            std::fs::write(source_folder.join("lib.rs"), "pub fn graph() {}")
                .expect("source should write");
            std::fs::write(asset_folder.join("logo.png"), b"png").expect("asset should write");
            let mut database = ManifestDatabase::open_in_memory().expect("database should open");
            let scope = authorize_scope(&database, &scope_path).expect("scope should authorize");
            scan_scope(&mut database, scope.id).expect("scope should scan");
            let canonical_root =
                std::fs::canonicalize(&scope_path).expect("root should canonicalize");
            let canonical_source =
                std::fs::canonicalize(&source_folder).expect("source should canonicalize");
            let root_node_id = database
                .node_id_for_path_key(scope.id, &comparison_key(&canonical_root))
                .expect("root lookup should pass")
                .expect("root should exist");
            let source_node_id = database
                .node_id_for_path_key(scope.id, &comparison_key(&canonical_source))
                .expect("source lookup should pass")
                .expect("source should exist");
            Self {
                _directory: directory,
                database,
                scope_id: scope.id,
                root_node_id,
                source_node_id,
            }
        }
    }

    #[test]
    fn profile_uses_bounded_manifest_facts_and_explains_project_suggestion() {
        let fixture = Fixture::new();
        let profile = folder_profile(&fixture.database, fixture.scope_id, fixture.root_node_id)
            .expect("profile should build");

        assert_eq!(profile.direct_file_count, 3);
        assert_eq!(profile.direct_folder_count, 2);
        assert_eq!(profile.descendant_file_count, 5);
        assert_eq!(profile.descendant_folder_count, 2);
        assert_eq!(profile.bounded_entry_limit, 100_000);
        let category = |expected| {
            profile
                .file_categories
                .iter()
                .find(|count| count.category == expected)
                .map(|count| count.file_count)
        };
        assert_eq!(category(FolderFileCategory::Code), Some(2));
        assert_eq!(category(FolderFileCategory::Document), Some(1));
        assert_eq!(category(FolderFileCategory::Data), Some(1));
        assert_eq!(category(FolderFileCategory::Image), Some(1));
        let suggestion = profile
            .project_suggestion
            .expect("Cargo marker should create a suggestion");
        assert_eq!(suggestion.confidence_basis_points, 8_500);
        assert_eq!(suggestion.created_by, ProjectSuggestionCreator::SystemRule);
        assert_eq!(suggestion.model_version, None);
        assert_eq!(suggestion.provenance.len(), 2);
        assert_eq!(
            suggestion.provenance[0].kind,
            ProjectSignalKind::CargoManifest
        );
        assert_eq!(suggestion.provenance[1].kind, ProjectSignalKind::Readme);
    }

    #[test]
    fn nested_profile_excludes_sibling_locations() {
        let fixture = Fixture::new();
        let profile = folder_profile(&fixture.database, fixture.scope_id, fixture.source_node_id)
            .expect("nested profile should build");
        assert_eq!(profile.direct_file_count, 1);
        assert_eq!(profile.descendant_file_count, 1);
        assert_eq!(profile.descendant_folder_count, 0);
        assert!(profile.project_suggestion.is_none());
    }

    #[test]
    fn profile_entry_limit_fails_closed() {
        let fixture = Fixture::new();
        let error =
            folder_profile_with_limit(&fixture.database, fixture.scope_id, fixture.root_node_id, 1)
                .expect_err("oversized profile should fail closed");
        assert_eq!(error.code(), "folder_profile_entry_limit_exceeded");
    }
}
