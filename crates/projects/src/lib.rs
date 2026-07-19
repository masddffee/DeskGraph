use std::fmt;
use std::fs::{self, File, Metadata};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant, UNIX_EPOCH};

use deskgraph_database::{ActionSourceRecord, DatabaseError, FolderProfileFacts, ManifestDatabase};
use deskgraph_domain::{
    CleanupSourceDetail, CleanupSourceDetailMember, CleanupSourceMemberRole,
    CleanupSourceSelectionRule, FileRelationCandidate, FileRelationCandidateState,
    FileRelationCandidateSummary, FileRelationDecisionKind, FileVersionCandidate, FolderProfile,
    ProjectCandidate, ProjectCandidateSummary, ProjectDecisionKind, ProjectSignal,
    ProjectSignalKind, ProjectSuggestion, ProjectSuggestionCreator, ScreenshotGroupCandidate,
    ScreenshotGroupCandidateSummary, ScreenshotGroupDiscovery, SmartCleanupInbox,
    SmartCleanupInboxItem, SmartCleanupSourceKind, parse_explicit_file_version_name,
};
use deskgraph_identity::{
    IdentityNodeKind, comparison_key, is_symlink_or_reparse_point, path_from_raw,
    platform_identity, platform_identity_for_open_file,
};
use deskgraph_scanner::{ScannerError, validated_scope_root};

const DEFAULT_ENTRY_LIMIT: u64 = 100_000;
const MAX_EXACT_DUPLICATE_BYTES: u64 = 64 * 1024 * 1024;
const DUPLICATE_BUFFER_BYTES: usize = 64 * 1024;
const DUPLICATE_COMPARE_DEADLINE: Duration = Duration::from_secs(5);
const MAX_SCREENSHOT_GROUP_IMAGES: u32 = 2_000;
const MAX_SCREENSHOT_GROUPS: u32 = 20;
const MAX_SCREENSHOT_GROUP_MEMBERS: u32 = 20;
const MAX_SMART_CLEANUP_SOURCES: u32 = 20;

#[derive(Debug)]
pub enum ProjectError {
    Database(DatabaseError),
    Scanner(ScannerError),
    SuggestionUnavailable,
    RelationPathMustBeAbsolute,
    RelationSourceUnavailable,
    RelationSourceSymlinkOrReparseDenied,
    RelationSourceOutsideScope,
    RelationSourceMustBeFile,
    RelationSourceIdentityUnavailable,
    RelationSourceIdentityChanged,
    RelationSourceMetadataChanged,
    RelationSourceOpenFailed,
    RelationSourceReadFailed,
    RelationSourceEmpty,
    RelationSourceTooLarge,
    RelationSameFileIdentity,
    RelationContentDiffers,
    RelationComparisonTimedOut,
    RelationPathDecodeFailed,
    VersionNameUnsupported,
    VersionBaseMismatch,
    VersionExtensionMismatch,
    VersionNumberEqual,
    CleanupSourceMemberLimitExceeded,
}

impl ProjectError {
    #[must_use]
    pub fn code(&self) -> &'static str {
        match self {
            Self::Database(error) => error.code(),
            Self::Scanner(error) => error.code(),
            Self::SuggestionUnavailable => "project_suggestion_unavailable",
            Self::RelationPathMustBeAbsolute => "file_relation_path_must_be_absolute",
            Self::RelationSourceUnavailable => "file_relation_source_unavailable",
            Self::RelationSourceSymlinkOrReparseDenied => {
                "file_relation_source_symlink_or_reparse_denied"
            }
            Self::RelationSourceOutsideScope => "file_relation_source_outside_scope",
            Self::RelationSourceMustBeFile => "file_relation_source_must_be_file",
            Self::RelationSourceIdentityUnavailable => "file_relation_source_identity_unavailable",
            Self::RelationSourceIdentityChanged => "file_relation_source_identity_changed",
            Self::RelationSourceMetadataChanged => "file_relation_source_metadata_changed",
            Self::RelationSourceOpenFailed => "file_relation_source_open_failed",
            Self::RelationSourceReadFailed => "file_relation_source_read_failed",
            Self::RelationSourceEmpty => "file_relation_source_empty",
            Self::RelationSourceTooLarge => "file_relation_source_too_large",
            Self::RelationSameFileIdentity => "file_relation_same_file_identity",
            Self::RelationContentDiffers => "file_relation_content_differs",
            Self::RelationComparisonTimedOut => "file_relation_comparison_timed_out",
            Self::RelationPathDecodeFailed => "file_relation_path_decode_failed",
            Self::VersionNameUnsupported => "file_version_name_unsupported",
            Self::VersionBaseMismatch => "file_version_base_mismatch",
            Self::VersionExtensionMismatch => "file_version_extension_mismatch",
            Self::VersionNumberEqual => "file_version_number_equal",
            Self::CleanupSourceMemberLimitExceeded => "cleanup_source_member_limit_exceeded",
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

impl From<ScannerError> for ProjectError {
    fn from(error: ScannerError) -> Self {
        Self::Scanner(error)
    }
}

struct OpenRelationSource {
    path: PathBuf,
    snapshot: ActionSourceRecord,
    file: File,
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

pub fn propose_project_at(
    database_path: &Path,
    scope_id: i64,
    root_folder_node_id: i64,
) -> Result<ProjectCandidate, ProjectError> {
    let mut database = ManifestDatabase::open(database_path)?;
    propose_project(&mut database, scope_id, root_folder_node_id)
}

pub fn propose_project(
    database: &mut ManifestDatabase,
    scope_id: i64,
    root_folder_node_id: i64,
) -> Result<ProjectCandidate, ProjectError> {
    let profile = folder_profile(database, scope_id, root_folder_node_id)?;
    let suggestion = profile
        .project_suggestion
        .ok_or(ProjectError::SuggestionUnavailable)?;
    database
        .record_project_candidate(scope_id, root_folder_node_id, &suggestion)
        .map_err(Into::into)
}

pub fn project_candidate_at(
    database_path: &Path,
    project_id: i64,
) -> Result<ProjectCandidate, ProjectError> {
    ManifestDatabase::open(database_path)?
        .project_candidate(project_id)
        .map_err(Into::into)
}

pub fn decide_project_candidate_at(
    database_path: &Path,
    project_id: i64,
    decision: ProjectDecisionKind,
) -> Result<ProjectCandidate, ProjectError> {
    let mut database = ManifestDatabase::open(database_path)?;
    database
        .decide_project_candidate(project_id, decision)
        .map_err(Into::into)
}

pub fn recent_project_candidates_at(
    database_path: &Path,
) -> Result<Vec<ProjectCandidateSummary>, ProjectError> {
    ManifestDatabase::open(database_path)?
        .recent_project_candidates()
        .map_err(Into::into)
}

pub fn check_exact_duplicate_at(
    database_path: &Path,
    scope_id: i64,
    left_path: &Path,
    right_path: &Path,
) -> Result<FileRelationCandidate, ProjectError> {
    let mut database = ManifestDatabase::open(database_path)?;
    check_exact_duplicate(&mut database, scope_id, left_path, right_path)
}

pub fn check_exact_duplicate(
    database: &mut ManifestDatabase,
    scope_id: i64,
    left_path: &Path,
    right_path: &Path,
) -> Result<FileRelationCandidate, ProjectError> {
    let canonical_root = validated_scope_root(database, scope_id)?;
    let left = open_relation_source(database, scope_id, &canonical_root, left_path, None)?;
    let right = open_relation_source(database, scope_id, &canonical_root, right_path, None)?;
    compare_and_record(database, left, right)
}

pub fn verify_exact_duplicate_at(
    database_path: &Path,
    relation_id: i64,
) -> Result<FileRelationCandidate, ProjectError> {
    let mut database = ManifestDatabase::open(database_path)?;
    verify_exact_duplicate(&mut database, relation_id)
}

pub fn verify_exact_duplicate(
    database: &mut ManifestDatabase,
    relation_id: i64,
) -> Result<FileRelationCandidate, ProjectError> {
    let (left_snapshot, right_snapshot) = database.exact_duplicate_sources(relation_id)?;
    let canonical_root = validated_scope_root(database, left_snapshot.scope_id)?;
    let left_path = path_from_raw(&left_snapshot.path_raw)
        .map_err(|_| ProjectError::RelationPathDecodeFailed)?;
    let right_path = path_from_raw(&right_snapshot.path_raw)
        .map_err(|_| ProjectError::RelationPathDecodeFailed)?;
    let left = open_relation_source(
        database,
        left_snapshot.scope_id,
        &canonical_root,
        &left_path,
        Some(left_snapshot.node_id),
    )?;
    let right = open_relation_source(
        database,
        right_snapshot.scope_id,
        &canonical_root,
        &right_path,
        Some(right_snapshot.node_id),
    )?;
    compare_and_record(database, left, right)
}

pub fn decide_exact_duplicate_at(
    database_path: &Path,
    relation_id: i64,
    decision: FileRelationDecisionKind,
) -> Result<FileRelationCandidate, ProjectError> {
    let mut database = ManifestDatabase::open(database_path)?;
    decide_exact_duplicate(&mut database, relation_id, decision)
}

pub fn decide_exact_duplicate(
    database: &mut ManifestDatabase,
    relation_id: i64,
    decision: FileRelationDecisionKind,
) -> Result<FileRelationCandidate, ProjectError> {
    verify_exact_duplicate(database, relation_id)?;
    database
        .decide_file_relation_candidate(relation_id, decision)
        .map_err(Into::into)
}

pub fn recent_file_relation_candidates_at(
    database_path: &Path,
) -> Result<Vec<FileRelationCandidateSummary>, ProjectError> {
    ManifestDatabase::open(database_path)?
        .recent_file_relation_candidates()
        .map_err(Into::into)
}

pub fn suggest_file_version_at(
    database_path: &Path,
    scope_id: i64,
    first_path: &Path,
    second_path: &Path,
) -> Result<FileVersionCandidate, ProjectError> {
    let mut database = ManifestDatabase::open(database_path)?;
    suggest_file_version(&mut database, scope_id, first_path, second_path)
}

pub fn suggest_file_version(
    database: &mut ManifestDatabase,
    scope_id: i64,
    first_path: &Path,
    second_path: &Path,
) -> Result<FileVersionCandidate, ProjectError> {
    let canonical_root = validated_scope_root(database, scope_id)?;
    let first = open_relation_source(database, scope_id, &canonical_root, first_path, None)?;
    let second = open_relation_source(database, scope_id, &canonical_root, second_path, None)?;
    analyze_and_record_file_version(database, first, second)
}

pub fn verify_file_version_at(
    database_path: &Path,
    relation_id: i64,
) -> Result<FileVersionCandidate, ProjectError> {
    let mut database = ManifestDatabase::open(database_path)?;
    verify_file_version(&mut database, relation_id)
}

pub fn verify_file_version(
    database: &mut ManifestDatabase,
    relation_id: i64,
) -> Result<FileVersionCandidate, ProjectError> {
    let (first_snapshot, second_snapshot) = database.file_version_sources(relation_id)?;
    let canonical_root = validated_scope_root(database, first_snapshot.scope_id)?;
    let first_path = path_from_raw(&first_snapshot.path_raw)
        .map_err(|_| ProjectError::RelationPathDecodeFailed)?;
    let second_path = path_from_raw(&second_snapshot.path_raw)
        .map_err(|_| ProjectError::RelationPathDecodeFailed)?;
    let first = open_relation_source(
        database,
        first_snapshot.scope_id,
        &canonical_root,
        &first_path,
        Some(first_snapshot.node_id),
    )?;
    let second = open_relation_source(
        database,
        second_snapshot.scope_id,
        &canonical_root,
        &second_path,
        Some(second_snapshot.node_id),
    )?;
    analyze_and_record_file_version(database, first, second)
}

pub fn decide_file_version_at(
    database_path: &Path,
    relation_id: i64,
    decision: FileRelationDecisionKind,
) -> Result<FileVersionCandidate, ProjectError> {
    let mut database = ManifestDatabase::open(database_path)?;
    decide_file_version(&mut database, relation_id, decision)
}

pub fn decide_file_version(
    database: &mut ManifestDatabase,
    relation_id: i64,
    decision: FileRelationDecisionKind,
) -> Result<FileVersionCandidate, ProjectError> {
    verify_file_version(database, relation_id)?;
    database
        .decide_file_version_candidate(relation_id, decision)
        .map_err(Into::into)
}

pub fn suggest_screenshot_groups_at(
    database_path: &Path,
    scope_id: i64,
) -> Result<ScreenshotGroupDiscovery, ProjectError> {
    let mut database = ManifestDatabase::open(database_path)?;
    suggest_screenshot_groups(&mut database, scope_id)
}

pub fn suggest_screenshot_groups(
    database: &mut ManifestDatabase,
    scope_id: i64,
) -> Result<ScreenshotGroupDiscovery, ProjectError> {
    let (evaluated_image_count, groups) =
        database.discover_screenshot_group_candidates(scope_id)?;
    Ok(ScreenshotGroupDiscovery {
        api_version: ScreenshotGroupDiscovery::API_VERSION,
        scope_id,
        evaluated_image_count,
        groups,
        bounded_image_limit: MAX_SCREENSHOT_GROUP_IMAGES,
        bounded_group_limit: MAX_SCREENSHOT_GROUPS,
        bounded_members_per_group: MAX_SCREENSHOT_GROUP_MEMBERS,
    })
}

pub fn screenshot_group_at(
    database_path: &Path,
    group_id: i64,
) -> Result<ScreenshotGroupCandidate, ProjectError> {
    ManifestDatabase::open(database_path)?
        .screenshot_group_candidate(group_id)
        .map_err(Into::into)
}

pub fn recent_screenshot_groups_at(
    database_path: &Path,
) -> Result<Vec<ScreenshotGroupCandidateSummary>, ProjectError> {
    ManifestDatabase::open(database_path)?
        .recent_screenshot_group_candidates()
        .map_err(Into::into)
}

pub fn refresh_smart_cleanup_inbox_at(
    database_path: &Path,
    scope_id: i64,
) -> Result<SmartCleanupInbox, ProjectError> {
    let mut database = ManifestDatabase::open(database_path)?;
    refresh_smart_cleanup_inbox(&mut database, scope_id)
}

pub fn refresh_smart_cleanup_inbox(
    database: &mut ManifestDatabase,
    scope_id: i64,
) -> Result<SmartCleanupInbox, ProjectError> {
    if !database.scope_has_active_access_grant(scope_id)? {
        return Err(DatabaseError::ScopeAccessGrantNotActive.into());
    }
    let (references, mut evaluation_complete) =
        database.smart_cleanup_source_references(scope_id, MAX_SMART_CLEANUP_SOURCES)?;
    let evaluated_source_count =
        u32::try_from(references.len()).map_err(|_| DatabaseError::InvalidCount)?;
    let mut not_current_source_count = 0_u32;
    let mut items = Vec::with_capacity(references.len());

    for reference in references {
        if reference.state != FileRelationCandidateState::Suggested {
            continue;
        }
        if !database.scope_has_active_access_grant(scope_id)? {
            return Err(DatabaseError::ScopeAccessGrantNotActive.into());
        }
        let result = match reference.kind {
            SmartCleanupSourceKind::ExactDuplicate => {
                let candidate = verify_exact_duplicate(database, reference.source_id);
                candidate.and_then(|candidate| {
                    database
                        .smart_cleanup_relation_item(
                            candidate.relation_id,
                            candidate.evidence.observed_at_unix_ms,
                        )
                        .map_err(Into::into)
                })
            }
            SmartCleanupSourceKind::Version => {
                let candidate = verify_file_version(database, reference.source_id);
                candidate.and_then(|candidate| {
                    database
                        .smart_cleanup_relation_item(
                            candidate.relation_id,
                            candidate.evidence.observed_at_unix_ms,
                        )
                        .map_err(Into::into)
                })
            }
            SmartCleanupSourceKind::ScreenshotReviewGroup => database
                .smart_cleanup_screenshot_item(reference.source_id)
                .map_err(Into::into),
        };
        match result {
            Ok(item) => items.push(item),
            Err(error) if cleanup_source_is_not_current(&error) => {
                not_current_source_count = not_current_source_count
                    .checked_add(1)
                    .ok_or(DatabaseError::InvalidCount)?;
            }
            Err(error) if cleanup_source_evaluation_is_incomplete(&error) => {
                evaluation_complete = false;
            }
            Err(error) => return Err(error),
        }
    }

    items.sort_by(|left, right| {
        left.source_kind
            .cmp(&right.source_kind)
            .then_with(|| right.observed_at_unix_ms.cmp(&left.observed_at_unix_ms))
            .then_with(|| left.source_id.cmp(&right.source_id))
    });
    Ok(SmartCleanupInbox {
        api_version: SmartCleanupInbox::API_VERSION,
        scope_id,
        items,
        evaluated_source_count,
        not_current_source_count,
        bounded_source_limit: MAX_SMART_CLEANUP_SOURCES,
        evaluation_complete,
        action_authorized: false,
    })
}

pub fn cleanup_source_detail_at(
    database_path: &Path,
    scope_id: i64,
    source_kind: SmartCleanupSourceKind,
    source_id: i64,
    source_observation_id: i64,
) -> Result<CleanupSourceDetail, ProjectError> {
    let mut database = ManifestDatabase::open(database_path)?;
    cleanup_source_detail(
        &mut database,
        scope_id,
        source_kind,
        source_id,
        source_observation_id,
    )
}

pub fn cleanup_source_detail(
    database: &mut ManifestDatabase,
    scope_id: i64,
    source_kind: SmartCleanupSourceKind,
    source_id: i64,
    source_observation_id: i64,
) -> Result<CleanupSourceDetail, ProjectError> {
    database.validate_cleanup_source_observation(
        scope_id,
        source_kind,
        source_id,
        source_observation_id,
    )?;

    let (item, members, selection_rule) = match source_kind {
        SmartCleanupSourceKind::ExactDuplicate => {
            let candidate = verify_exact_duplicate(database, source_id)?;
            let item = database.smart_cleanup_relation_item(
                candidate.relation_id,
                candidate.evidence.observed_at_unix_ms,
            )?;
            let members = [candidate.left, candidate.right]
                .into_iter()
                .map(|member| CleanupSourceDetailMember {
                    node_id: member.node_id,
                    display_path: member.display_path,
                    size_bytes: member.size_bytes,
                    role: CleanupSourceMemberRole::DuplicateCandidate,
                })
                .collect();
            (
                item,
                members,
                CleanupSourceSelectionRule::EitherMemberIsTarget,
            )
        }
        SmartCleanupSourceKind::Version => {
            let candidate = verify_file_version(database, source_id)?;
            let item = database.smart_cleanup_relation_item(
                candidate.relation_id,
                candidate.evidence.observed_at_unix_ms,
            )?;
            let members = vec![
                CleanupSourceDetailMember {
                    node_id: candidate.older.node_id,
                    display_path: candidate.older.display_path,
                    size_bytes: candidate.older.size_bytes,
                    role: CleanupSourceMemberRole::OlderVersion,
                },
                CleanupSourceDetailMember {
                    node_id: candidate.newer.node_id,
                    display_path: candidate.newer.display_path,
                    size_bytes: candidate.newer.size_bytes,
                    role: CleanupSourceMemberRole::NewerVersion,
                },
            ];
            (
                item,
                members,
                CleanupSourceSelectionRule::OlderTargetNewerKeeper,
            )
        }
        SmartCleanupSourceKind::ScreenshotReviewGroup => {
            let candidate = database.screenshot_group_candidate(source_id)?;
            let members = candidate
                .members
                .into_iter()
                .map(|member| CleanupSourceDetailMember {
                    node_id: member.node_id,
                    display_path: member.display_path,
                    size_bytes: member.size_bytes,
                    role: CleanupSourceMemberRole::ScreenshotCandidate,
                })
                .collect();
            let item = database.validate_cleanup_source_observation(
                scope_id,
                source_kind,
                source_id,
                source_observation_id,
            )?;
            (
                item,
                members,
                CleanupSourceSelectionRule::SingleTargetNoKeeper,
            )
        }
    };
    validate_cleanup_detail_item(&item, scope_id, source_kind, source_id)?;
    let item = database.validate_cleanup_source_observation(
        scope_id,
        source_kind,
        source_id,
        item.source_observation_id,
    )?;
    if members.is_empty() || members.len() > CleanupSourceDetail::MAX_MEMBERS {
        return Err(ProjectError::CleanupSourceMemberLimitExceeded);
    }
    Ok(CleanupSourceDetail {
        api_version: CleanupSourceDetail::API_VERSION,
        scope_id,
        source_kind,
        source_id,
        source_observation_id: item.source_observation_id,
        members,
        selection_rule,
        current_evidence: true,
        user_requested_paths: true,
        action_authorized: false,
        execution_available: false,
    })
}

fn validate_cleanup_detail_item(
    item: &SmartCleanupInboxItem,
    scope_id: i64,
    source_kind: SmartCleanupSourceKind,
    source_id: i64,
) -> Result<(), ProjectError> {
    if item.scope_id != scope_id
        || item.source_kind != source_kind
        || item.source_id != source_id
        || !item.current_evidence
        || item.cleanup_authorized
    {
        return Err(DatabaseError::CleanupActionSourceNotCurrent.into());
    }
    Ok(())
}

fn cleanup_source_is_not_current(error: &ProjectError) -> bool {
    matches!(
        error,
        ProjectError::RelationSourceUnavailable
            | ProjectError::RelationSourceSymlinkOrReparseDenied
            | ProjectError::RelationSourceOutsideScope
            | ProjectError::RelationSourceMustBeFile
            | ProjectError::RelationSourceIdentityUnavailable
            | ProjectError::RelationSourceIdentityChanged
            | ProjectError::RelationSourceMetadataChanged
            | ProjectError::RelationSourceEmpty
            | ProjectError::RelationSourceTooLarge
            | ProjectError::RelationSameFileIdentity
            | ProjectError::RelationContentDiffers
            | ProjectError::VersionNameUnsupported
            | ProjectError::VersionBaseMismatch
            | ProjectError::VersionExtensionMismatch
            | ProjectError::VersionNumberEqual
            | ProjectError::Database(DatabaseError::FileRelationCandidateNotCurrent)
            | ProjectError::Database(DatabaseError::ScreenshotGroupCandidateNotCurrent)
    )
}

fn cleanup_source_evaluation_is_incomplete(error: &ProjectError) -> bool {
    matches!(
        error,
        ProjectError::RelationSourceOpenFailed
            | ProjectError::RelationSourceReadFailed
            | ProjectError::RelationComparisonTimedOut
    )
}

fn open_relation_source(
    database: &ManifestDatabase,
    scope_id: i64,
    canonical_root: &Path,
    requested_path: &Path,
    expected_node_id: Option<i64>,
) -> Result<OpenRelationSource, ProjectError> {
    if !requested_path.is_absolute() {
        return Err(ProjectError::RelationPathMustBeAbsolute);
    }
    let link_metadata = fs::symlink_metadata(requested_path)
        .map_err(|_| ProjectError::RelationSourceUnavailable)?;
    if is_symlink_or_reparse_point(&link_metadata) {
        return Err(ProjectError::RelationSourceSymlinkOrReparseDenied);
    }
    if !link_metadata.is_file() {
        return Err(ProjectError::RelationSourceMustBeFile);
    }
    let canonical_path =
        fs::canonicalize(requested_path).map_err(|_| ProjectError::RelationSourceUnavailable)?;
    if canonical_path == canonical_root || !canonical_path.starts_with(canonical_root) {
        return Err(ProjectError::RelationSourceOutsideScope);
    }
    if comparison_key(requested_path) != comparison_key(&canonical_path) {
        return Err(ProjectError::RelationSourceSymlinkOrReparseDenied);
    }
    validate_canonical_path_state(canonical_root, &canonical_path)?;
    let snapshot = database
        .action_source_for_path_key(scope_id, &comparison_key(&canonical_path))
        .map_err(|error| match error {
            DatabaseError::ActionSourceNotFound => ProjectError::RelationSourceUnavailable,
            other => ProjectError::Database(other),
        })?;
    if expected_node_id.is_some_and(|expected| expected != snapshot.node_id) {
        return Err(ProjectError::RelationSourceIdentityChanged);
    }
    validate_path_snapshot(&canonical_path, &snapshot, &link_metadata)?;
    let file = File::open(&canonical_path).map_err(|_| ProjectError::RelationSourceOpenFailed)?;
    let open_metadata = file
        .metadata()
        .map_err(|_| ProjectError::RelationSourceOpenFailed)?;
    validate_open_snapshot(&file, &canonical_path, &snapshot, &open_metadata)?;
    Ok(OpenRelationSource {
        path: canonical_path,
        snapshot,
        file,
    })
}

fn compare_and_record(
    database: &mut ManifestDatabase,
    mut left: OpenRelationSource,
    mut right: OpenRelationSource,
) -> Result<FileRelationCandidate, ProjectError> {
    if left.snapshot.node_id == right.snapshot.node_id
        || (left.snapshot.identity_kind == right.snapshot.identity_kind
            && left.snapshot.identity_key == right.snapshot.identity_key)
    {
        return Err(ProjectError::RelationSameFileIdentity);
    }
    if left.snapshot.size_bytes == 0 || right.snapshot.size_bytes == 0 {
        return Err(ProjectError::RelationSourceEmpty);
    }
    if left.snapshot.size_bytes > MAX_EXACT_DUPLICATE_BYTES
        || right.snapshot.size_bytes > MAX_EXACT_DUPLICATE_BYTES
    {
        return Err(ProjectError::RelationSourceTooLarge);
    }
    if left.snapshot.size_bytes != right.snapshot.size_bytes {
        return Err(ProjectError::RelationContentDiffers);
    }
    compare_exact_bytes(&mut left.file, &mut right.file, left.snapshot.size_bytes)?;
    revalidate_open_relation_sources(database, &left, &right)?;

    let (left_snapshot, right_snapshot) = if left.snapshot.node_id < right.snapshot.node_id {
        (&left.snapshot, &right.snapshot)
    } else {
        (&right.snapshot, &left.snapshot)
    };
    database
        .record_exact_duplicate_candidate(left_snapshot, right_snapshot)
        .map_err(Into::into)
}

fn analyze_and_record_file_version(
    database: &mut ManifestDatabase,
    first: OpenRelationSource,
    second: OpenRelationSource,
) -> Result<FileVersionCandidate, ProjectError> {
    if first.snapshot.node_id == second.snapshot.node_id
        || (first.snapshot.identity_kind == second.snapshot.identity_kind
            && first.snapshot.identity_key == second.snapshot.identity_key)
    {
        return Err(ProjectError::RelationSameFileIdentity);
    }
    let first_name = first
        .path
        .file_name()
        .and_then(|value| value.to_str())
        .and_then(parse_explicit_file_version_name)
        .ok_or(ProjectError::VersionNameUnsupported)?;
    let second_name = second
        .path
        .file_name()
        .and_then(|value| value.to_str())
        .and_then(parse_explicit_file_version_name)
        .ok_or(ProjectError::VersionNameUnsupported)?;
    if first_name.base_key != second_name.base_key {
        return Err(ProjectError::VersionBaseMismatch);
    }
    if first_name.extension_key != second_name.extension_key {
        return Err(ProjectError::VersionExtensionMismatch);
    }
    if first_name.version == second_name.version {
        return Err(ProjectError::VersionNumberEqual);
    }
    revalidate_open_relation_sources(database, &first, &second)?;
    database
        .record_file_version_candidate(&first.snapshot, &second.snapshot)
        .map_err(Into::into)
}

fn revalidate_open_relation_sources(
    database: &ManifestDatabase,
    first: &OpenRelationSource,
    second: &OpenRelationSource,
) -> Result<(), ProjectError> {
    let canonical_root = validated_scope_root(database, first.snapshot.scope_id)?;
    validate_canonical_path_state(&canonical_root, &first.path)?;
    validate_canonical_path_state(&canonical_root, &second.path)?;
    for source in [first, second] {
        let metadata = source
            .file
            .metadata()
            .map_err(|_| ProjectError::RelationSourceReadFailed)?;
        validate_open_snapshot(&source.file, &source.path, &source.snapshot, &metadata)?;
    }
    Ok(())
}

fn compare_exact_bytes(
    left: &mut File,
    right: &mut File,
    expected_bytes: u64,
) -> Result<(), ProjectError> {
    let started = Instant::now();
    let mut remaining = expected_bytes;
    let mut left_buffer = vec![0_u8; DUPLICATE_BUFFER_BYTES];
    let mut right_buffer = vec![0_u8; DUPLICATE_BUFFER_BYTES];
    while remaining > 0 {
        if started.elapsed() > DUPLICATE_COMPARE_DEADLINE {
            return Err(ProjectError::RelationComparisonTimedOut);
        }
        let chunk = usize::try_from(remaining.min(DUPLICATE_BUFFER_BYTES as u64))
            .map_err(|_| ProjectError::RelationSourceReadFailed)?;
        left.read_exact(&mut left_buffer[..chunk])
            .map_err(|_| ProjectError::RelationSourceReadFailed)?;
        right
            .read_exact(&mut right_buffer[..chunk])
            .map_err(|_| ProjectError::RelationSourceReadFailed)?;
        if left_buffer[..chunk] != right_buffer[..chunk] {
            return Err(ProjectError::RelationContentDiffers);
        }
        remaining -= u64::try_from(chunk).map_err(|_| ProjectError::RelationSourceReadFailed)?;
    }
    let mut left_extra = [0_u8; 1];
    let mut right_extra = [0_u8; 1];
    let left_extra = left
        .read(&mut left_extra)
        .map_err(|_| ProjectError::RelationSourceReadFailed)?;
    let right_extra = right
        .read(&mut right_extra)
        .map_err(|_| ProjectError::RelationSourceReadFailed)?;
    if left_extra != 0 || right_extra != 0 {
        return Err(ProjectError::RelationSourceMetadataChanged);
    }
    if started.elapsed() > DUPLICATE_COMPARE_DEADLINE {
        return Err(ProjectError::RelationComparisonTimedOut);
    }
    Ok(())
}

fn validate_path_snapshot(
    path: &Path,
    snapshot: &ActionSourceRecord,
    metadata: &Metadata,
) -> Result<(), ProjectError> {
    if snapshot.identity_kind == "path_fallback" {
        return Err(ProjectError::RelationSourceIdentityUnavailable);
    }
    let identity = platform_identity(path, metadata, IdentityNodeKind::File)
        .map_err(|_| ProjectError::RelationSourceIdentityUnavailable)?;
    if identity.kind != snapshot.identity_kind || identity.key != snapshot.identity_key {
        return Err(ProjectError::RelationSourceIdentityChanged);
    }
    validate_metadata(snapshot, metadata)
}

fn validate_canonical_path_state(canonical_root: &Path, path: &Path) -> Result<(), ProjectError> {
    let link_metadata =
        fs::symlink_metadata(path).map_err(|_| ProjectError::RelationSourceUnavailable)?;
    if is_symlink_or_reparse_point(&link_metadata) {
        return Err(ProjectError::RelationSourceSymlinkOrReparseDenied);
    }
    let current = fs::canonicalize(path).map_err(|_| ProjectError::RelationSourceUnavailable)?;
    if current == canonical_root || !current.starts_with(canonical_root) {
        return Err(ProjectError::RelationSourceOutsideScope);
    }
    if comparison_key(&current) != comparison_key(path) {
        return Err(ProjectError::RelationSourceSymlinkOrReparseDenied);
    }
    Ok(())
}

fn validate_open_snapshot(
    file: &File,
    path: &Path,
    snapshot: &ActionSourceRecord,
    metadata: &Metadata,
) -> Result<(), ProjectError> {
    let identity = platform_identity_for_open_file(file, path, metadata, IdentityNodeKind::File)
        .map_err(|_| ProjectError::RelationSourceIdentityUnavailable)?;
    if identity.kind != snapshot.identity_kind || identity.key != snapshot.identity_key {
        return Err(ProjectError::RelationSourceIdentityChanged);
    }
    validate_metadata(snapshot, metadata)
}

fn validate_metadata(
    snapshot: &ActionSourceRecord,
    metadata: &Metadata,
) -> Result<(), ProjectError> {
    if metadata.len() != snapshot.size_bytes
        || modified_unix_ns(metadata) != snapshot.modified_unix_ns
    {
        return Err(ProjectError::RelationSourceMetadataChanged);
    }
    Ok(())
}

fn modified_unix_ns(metadata: &Metadata) -> Option<i64> {
    let duration = metadata.modified().ok()?.duration_since(UNIX_EPOCH).ok()?;
    i64::try_from(duration.as_nanos()).ok()
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
    use deskgraph_domain::{
        FileRelationCandidateState, FileRelationComparisonKind, FileRelationCreator,
        FileRelationDecisionKind, FileRelationKind, FileVersionSignalKind, FolderFileCategory,
        ProjectCandidateState,
    };
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
    fn readme_without_a_strong_marker_remains_profile_evidence_only() {
        let directory = tempfile::tempdir().expect("fixture root should exist");
        let scope_path = directory.path().join("notes");
        std::fs::create_dir(&scope_path).expect("scope should create");
        std::fs::write(scope_path.join("README.md"), "folder notes").expect("README should write");
        let mut database = ManifestDatabase::open_in_memory().expect("database should open");
        let scope = authorize_scope(&database, &scope_path).expect("scope should authorize");
        scan_scope(&mut database, scope.id).expect("scope should scan");
        let canonical_root = std::fs::canonicalize(&scope_path).expect("scope should canonicalize");
        let root_node_id = database
            .node_id_for_path_key(scope.id, &comparison_key(&canonical_root))
            .expect("root lookup should pass")
            .expect("root should exist");

        let profile = folder_profile(&database, scope.id, root_node_id)
            .expect("README-only profile should build");

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

    #[test]
    fn explicit_feedback_changes_future_candidate_state_and_is_idempotent() {
        let mut fixture = Fixture::new();
        let proposed = propose_project(
            &mut fixture.database,
            fixture.scope_id,
            fixture.root_node_id,
        )
        .expect("candidate should persist");
        assert_eq!(proposed.state, ProjectCandidateState::Suggested);
        assert!(proposed.latest_decision.is_none());

        let rejected = fixture
            .database
            .decide_project_candidate(proposed.project_id, ProjectDecisionKind::Rejected)
            .expect("candidate should reject");
        assert_eq!(rejected.state, ProjectCandidateState::Rejected);
        assert_eq!(
            rejected
                .latest_decision
                .as_ref()
                .map(|event| event.sequence),
            Some(1)
        );

        let proposed_again = propose_project(
            &mut fixture.database,
            fixture.scope_id,
            fixture.root_node_id,
        )
        .expect("same evidence should resolve the existing candidate");
        assert_eq!(proposed_again.project_id, proposed.project_id);
        assert_eq!(proposed_again.state, ProjectCandidateState::Rejected);

        let accepted = fixture
            .database
            .decide_project_candidate(proposed.project_id, ProjectDecisionKind::Accepted)
            .expect("user should be able to correct the rejection");
        assert_eq!(accepted.state, ProjectCandidateState::Accepted);
        assert_eq!(
            accepted
                .latest_decision
                .as_ref()
                .map(|event| event.sequence),
            Some(2)
        );
        let idempotent = fixture
            .database
            .decide_project_candidate(proposed.project_id, ProjectDecisionKind::Accepted)
            .expect("repeated decision should be idempotent");
        assert_eq!(idempotent.latest_decision, accepted.latest_decision);

        let summaries = fixture
            .database
            .recent_project_candidates()
            .expect("summaries should load");
        assert_eq!(summaries.len(), 1);
        assert_eq!(summaries[0].state, ProjectCandidateState::Accepted);
        let summary = serde_json::to_value(&summaries[0]).expect("summary should serialize");
        assert!(summary.get("display_path").is_none());
        assert!(summary.get("suggestion").is_none());
    }

    #[test]
    fn folder_without_strong_evidence_cannot_be_persisted_as_a_project() {
        let mut fixture = Fixture::new();
        let error = propose_project(
            &mut fixture.database,
            fixture.scope_id,
            fixture.source_node_id,
        )
        .expect_err("source folder has no project marker");
        assert_eq!(error.code(), "project_suggestion_unavailable");
        assert!(
            fixture
                .database
                .recent_project_candidates()
                .expect("summaries should load")
                .is_empty()
        );
    }

    #[test]
    fn exact_duplicate_candidate_is_bounded_durable_and_order_independent() {
        let directory = tempfile::tempdir().expect("fixture root should exist");
        let scope_path = directory.path().join("duplicates");
        std::fs::create_dir(&scope_path).expect("scope should create");
        let left_path = scope_path.join("private-left.txt");
        let right_path = scope_path.join("private-right.txt");
        let private_bytes = b"private exact duplicate bytes";
        std::fs::write(&left_path, private_bytes).expect("left should write");
        std::fs::write(&right_path, private_bytes).expect("right should write");
        let mut database = ManifestDatabase::open_in_memory().expect("database should open");
        let scope = authorize_scope(&database, &scope_path).expect("scope should authorize");
        scan_scope(&mut database, scope.id).expect("scope should scan");
        let canonical_left = std::fs::canonicalize(&left_path).expect("left should canonicalize");
        let canonical_right =
            std::fs::canonicalize(&right_path).expect("right should canonicalize");

        let candidate =
            check_exact_duplicate(&mut database, scope.id, &canonical_left, &canonical_right)
                .expect("identical files should create a candidate");
        assert_eq!(candidate.state, FileRelationCandidateState::Suggested);
        assert!(candidate.left.node_id < candidate.right.node_id);
        assert_eq!(
            candidate.evidence.comparison_kind,
            FileRelationComparisonKind::ByteForByte
        );
        assert_eq!(
            candidate.evidence.compared_bytes,
            u64::try_from(private_bytes.len()).expect("fixture size should fit")
        );
        assert_eq!(candidate.evidence.confidence_basis_points, 10_000);
        assert_eq!(
            candidate.evidence.created_by,
            FileRelationCreator::SystemRule
        );
        assert_eq!(candidate.evidence.model_version, None);
        assert_eq!(candidate.evidence.bounded_max_bytes, 64 * 1024 * 1024);

        let swapped =
            check_exact_duplicate(&mut database, scope.id, &canonical_right, &canonical_left)
                .expect("reversed input should reuse the relation");
        assert_eq!(swapped.relation_id, candidate.relation_id);
        let verified = verify_exact_duplicate(&mut database, candidate.relation_id)
            .expect("current relation should verify");
        assert_eq!(verified.relation_id, candidate.relation_id);
        let rejected = decide_exact_duplicate(
            &mut database,
            candidate.relation_id,
            FileRelationDecisionKind::Rejected,
        )
        .expect("a verified relation should reject");
        assert_eq!(rejected.state, FileRelationCandidateState::Rejected);
        assert_eq!(
            rejected
                .latest_decision
                .as_ref()
                .map(|decision| decision.sequence),
            Some(1)
        );
        let checked_again =
            check_exact_duplicate(&mut database, scope.id, &canonical_left, &canonical_right)
                .expect("later evidence should retain pair feedback");
        assert_eq!(checked_again.state, FileRelationCandidateState::Rejected);
        let accepted = decide_exact_duplicate(
            &mut database,
            candidate.relation_id,
            FileRelationDecisionKind::Accepted,
        )
        .expect("a verified relation should accept after correction");
        assert_eq!(accepted.state, FileRelationCandidateState::Accepted);
        assert_eq!(
            accepted
                .latest_decision
                .as_ref()
                .map(|decision| decision.sequence),
            Some(2)
        );
        let repeated_acceptance = decide_exact_duplicate(
            &mut database,
            candidate.relation_id,
            FileRelationDecisionKind::Accepted,
        )
        .expect("repeating acceptance should remain idempotent");
        assert_eq!(
            repeated_acceptance.latest_decision,
            accepted.latest_decision
        );
        let summaries = database
            .recent_file_relation_candidates()
            .expect("relation summaries should load");
        assert_eq!(summaries.len(), 1);
        assert_eq!(summaries[0].state, FileRelationCandidateState::Accepted);
        assert!(summaries[0].verification_required);
        assert_eq!(
            std::fs::read(&left_path).expect("left should remain"),
            private_bytes
        );
        assert_eq!(
            std::fs::read(&right_path).expect("right should remain"),
            private_bytes
        );
    }

    #[test]
    fn duplicate_check_rejects_different_same_identity_and_stale_sources() {
        let directory = tempfile::tempdir().expect("fixture root should exist");
        let scope_path = directory.path().join("duplicate-errors");
        std::fs::create_dir(&scope_path).expect("scope should create");
        let left_path = scope_path.join("left.bin");
        let right_path = scope_path.join("right.bin");
        let different_path = scope_path.join("different.bin");
        let outside_path = directory.path().join("outside.bin");
        std::fs::write(&left_path, b"same-length!").expect("left should write");
        std::fs::write(&right_path, b"same-length!").expect("right should write");
        std::fs::write(&different_path, b"diff-length?").expect("different should write");
        std::fs::write(&outside_path, b"same-length!").expect("outside should write");
        #[cfg(unix)]
        let hard_link_path = {
            let path = scope_path.join("left-hard-link.bin");
            std::fs::hard_link(&left_path, &path).expect("hard link should create");
            path
        };
        #[cfg(unix)]
        let symlink_alias = {
            let path = scope_path.join("scope-alias");
            std::os::unix::fs::symlink(&scope_path, &path).expect("scope alias should create");
            path
        };
        let mut database = ManifestDatabase::open_in_memory().expect("database should open");
        let scope = authorize_scope(&database, &scope_path).expect("scope should authorize");
        scan_scope(&mut database, scope.id).expect("scope should scan");
        let canonical_left = std::fs::canonicalize(&left_path).expect("left should canonicalize");
        let canonical_right =
            std::fs::canonicalize(&right_path).expect("right should canonicalize");
        let canonical_different =
            std::fs::canonicalize(&different_path).expect("different should canonicalize");
        let canonical_outside =
            std::fs::canonicalize(&outside_path).expect("outside should canonicalize");
        #[cfg(unix)]
        let canonical_hard_link =
            std::fs::canonicalize(&hard_link_path).expect("hard link should canonicalize");

        let different = check_exact_duplicate(
            &mut database,
            scope.id,
            &canonical_left,
            &canonical_different,
        )
        .expect_err("different bytes should not create a relation");
        assert_eq!(different.code(), "file_relation_content_differs");
        let same_identity =
            check_exact_duplicate(&mut database, scope.id, &canonical_left, &canonical_left)
                .expect_err("the same stable file is not a duplicate");
        assert_eq!(same_identity.code(), "file_relation_same_file_identity");
        #[cfg(unix)]
        {
            let hard_link = check_exact_duplicate(
                &mut database,
                scope.id,
                &canonical_left,
                &canonical_hard_link,
            )
            .expect_err("hard-link aliases are one stable file");
            assert_eq!(hard_link.code(), "file_relation_same_file_identity");
            let canonical_scope =
                std::fs::canonicalize(&scope_path).expect("scope should canonicalize");
            let aliased_right = canonical_scope
                .join(
                    symlink_alias
                        .file_name()
                        .expect("symlink alias should have a name"),
                )
                .join("right.bin");
            let symlink =
                check_exact_duplicate(&mut database, scope.id, &canonical_left, &aliased_right)
                    .expect_err("symlinked parent traversal must fail closed");
            assert_eq!(
                symlink.code(),
                "file_relation_source_symlink_or_reparse_denied"
            );
        }
        let outside =
            check_exact_duplicate(&mut database, scope.id, &canonical_left, &canonical_outside)
                .expect_err("an outside path must fail before manifest lookup");
        assert_eq!(outside.code(), "file_relation_source_outside_scope");
        let relative = check_exact_duplicate(
            &mut database,
            scope.id,
            Path::new("left.bin"),
            &canonical_right,
        )
        .expect_err("relative paths are ambiguous and must fail");
        assert_eq!(relative.code(), "file_relation_path_must_be_absolute");

        let candidate =
            check_exact_duplicate(&mut database, scope.id, &canonical_left, &canonical_right)
                .expect("initial identical files should create a candidate");
        std::fs::write(&right_path, b"changed").expect("right should change");
        let stale = verify_exact_duplicate(&mut database, candidate.relation_id)
            .expect_err("changed source should invalidate verification");
        assert_eq!(stale.code(), "file_relation_source_metadata_changed");
    }

    #[test]
    fn file_version_candidate_is_explicit_directional_and_revalidated() {
        let directory = tempfile::tempdir().expect("fixture root should exist");
        let scope_path = directory.path().join("versions");
        std::fs::create_dir(&scope_path).expect("scope should create");
        let first_path = scope_path.join("企劃-v1.MD");
        let second_path = scope_path.join("企劃_V2.md");
        let same_version_path = scope_path.join("企劃.v1.md");
        let other_base_path = scope_path.join("其他-v3.md");
        let other_extension_path = scope_path.join("企劃-v3.txt");
        let unsupported_path = scope_path.join("企劃-final.md");
        std::fs::write(&first_path, b"old revision").expect("first should write");
        std::fs::write(&second_path, b"new revision with different bytes")
            .expect("second should write");
        std::fs::write(&same_version_path, b"same ordinal").expect("same should write");
        std::fs::write(&other_base_path, b"other base").expect("other base should write");
        std::fs::write(&other_extension_path, b"other extension")
            .expect("other extension should write");
        std::fs::write(&unsupported_path, b"unsupported").expect("unsupported should write");
        let mut database = ManifestDatabase::open_in_memory().expect("database should open");
        let scope = authorize_scope(&database, &scope_path).expect("scope should authorize");
        scan_scope(&mut database, scope.id).expect("scope should scan");
        let canonical_first =
            std::fs::canonicalize(&first_path).expect("first should canonicalize");
        let canonical_second =
            std::fs::canonicalize(&second_path).expect("second should canonicalize");

        let candidate =
            suggest_file_version(&mut database, scope.id, &canonical_second, &canonical_first)
                .expect("explicit numeric versions should create a candidate");
        assert_eq!(candidate.kind, FileRelationKind::Version);
        assert_eq!(candidate.state, FileRelationCandidateState::Suggested);
        assert_eq!(
            candidate.evidence.signal_kind,
            FileVersionSignalKind::ExplicitNumericSuffix
        );
        assert_eq!(candidate.evidence.base_key, "企劃");
        assert_eq!(candidate.evidence.extension_key, "md");
        assert_eq!(candidate.evidence.older_version, 1);
        assert_eq!(candidate.evidence.newer_version, 2);
        assert_eq!(candidate.evidence.confidence_basis_points, 9_000);
        assert_eq!(candidate.evidence.model_version, None);
        assert_eq!(candidate.latest_decision, None);
        assert_eq!(
            candidate.older.display_path,
            canonical_first.to_string_lossy()
        );
        assert_eq!(
            candidate.newer.display_path,
            canonical_second.to_string_lossy()
        );

        let verified = verify_file_version(&mut database, candidate.relation_id)
            .expect("current names and identities should verify");
        assert_eq!(verified.relation_id, candidate.relation_id);
        let rejected = decide_file_version(
            &mut database,
            candidate.relation_id,
            FileRelationDecisionKind::Rejected,
        )
        .expect("a decision should reverify current filename evidence");
        assert_eq!(rejected.state, FileRelationCandidateState::Rejected);
        assert_eq!(
            rejected
                .latest_decision
                .as_ref()
                .expect("decision should exist")
                .sequence,
            1
        );
        let repeated = decide_file_version(
            &mut database,
            candidate.relation_id,
            FileRelationDecisionKind::Rejected,
        )
        .expect("equivalent repeated decision should be idempotent");
        assert_eq!(
            repeated
                .latest_decision
                .as_ref()
                .expect("decision should exist")
                .sequence,
            1
        );
        let summaries = database
            .recent_file_relation_candidates()
            .expect("history should load");
        assert_eq!(summaries.len(), 1);
        assert_eq!(summaries[0].kind, FileRelationKind::Version);
        assert_eq!(summaries[0].state, FileRelationCandidateState::Rejected);
        assert!(summaries[0].verification_required);

        for (path, expected_code) in [
            (&same_version_path, "file_version_number_equal"),
            (&other_base_path, "file_version_base_mismatch"),
            (&other_extension_path, "file_version_extension_mismatch"),
            (&unsupported_path, "file_version_name_unsupported"),
        ] {
            let canonical = std::fs::canonicalize(path).expect("fixture should canonicalize");
            let error = suggest_file_version(&mut database, scope.id, &canonical_first, &canonical)
                .expect_err("ambiguous evidence must not create a version relation");
            assert_eq!(error.code(), expected_code);
        }

        std::fs::write(&second_path, b"changed after scan").expect("second should change");
        let stale = decide_file_version(
            &mut database,
            candidate.relation_id,
            FileRelationDecisionKind::Accepted,
        )
        .expect_err("changed metadata must prevent a new decision");
        assert_eq!(stale.code(), "file_relation_source_metadata_changed");
        assert_eq!(
            std::fs::read(&first_path).expect("first should remain"),
            b"old revision"
        );
    }

    #[test]
    fn duplicate_check_rejects_empty_and_oversized_files_before_reading() {
        let directory = tempfile::tempdir().expect("fixture root should exist");
        let scope_path = directory.path().join("duplicate-limits");
        std::fs::create_dir(&scope_path).expect("scope should create");
        let empty_left = scope_path.join("empty-left.bin");
        let empty_right = scope_path.join("empty-right.bin");
        std::fs::write(&empty_left, []).expect("empty left should write");
        std::fs::write(&empty_right, []).expect("empty right should write");
        let large_left = scope_path.join("large-left.bin");
        let large_right = scope_path.join("large-right.bin");
        File::create(&large_left)
            .expect("large left should create")
            .set_len(MAX_EXACT_DUPLICATE_BYTES + 1)
            .expect("large left should resize");
        File::create(&large_right)
            .expect("large right should create")
            .set_len(MAX_EXACT_DUPLICATE_BYTES + 1)
            .expect("large right should resize");
        let mut database = ManifestDatabase::open_in_memory().expect("database should open");
        let scope = authorize_scope(&database, &scope_path).expect("scope should authorize");
        scan_scope(&mut database, scope.id).expect("scope should scan");
        let canonical_empty_left =
            std::fs::canonicalize(&empty_left).expect("empty left should canonicalize");
        let canonical_empty_right =
            std::fs::canonicalize(&empty_right).expect("empty right should canonicalize");
        let canonical_large_left =
            std::fs::canonicalize(&large_left).expect("large left should canonicalize");
        let canonical_large_right =
            std::fs::canonicalize(&large_right).expect("large right should canonicalize");

        let empty = check_exact_duplicate(
            &mut database,
            scope.id,
            &canonical_empty_left,
            &canonical_empty_right,
        )
        .expect_err("empty files should be excluded");
        assert_eq!(empty.code(), "file_relation_source_empty");
        let large = check_exact_duplicate(
            &mut database,
            scope.id,
            &canonical_large_left,
            &canonical_large_right,
        )
        .expect_err("oversized files should be excluded before comparison");
        assert_eq!(large.code(), "file_relation_source_too_large");
    }

    #[test]
    fn smart_cleanup_inbox_reverifies_suggested_relations_and_stays_path_free() {
        let directory = tempfile::tempdir().expect("fixture root should exist");
        let scope_path = directory.path().join("cleanup-inbox");
        std::fs::create_dir(&scope_path).expect("scope should create");
        let duplicate_left = scope_path.join("private-duplicate-left.bin");
        let duplicate_right = scope_path.join("private-duplicate-right.bin");
        let version_old = scope_path.join("private-plan-v1.md");
        let version_new = scope_path.join("private-plan-v2.md");
        std::fs::write(&duplicate_left, b"private duplicate bytes")
            .expect("left duplicate should write");
        std::fs::write(&duplicate_right, b"private duplicate bytes")
            .expect("right duplicate should write");
        std::fs::write(&version_old, b"old private version").expect("old version should write");
        std::fs::write(&version_new, b"new private version").expect("new version should write");

        let mut database = ManifestDatabase::open_in_memory().expect("database should open");
        let scope = authorize_scope(&database, &scope_path).expect("scope should authorize");
        scan_scope(&mut database, scope.id).expect("scope should scan");
        database
            .upsert_scope_access_grant(scope.id, std::env::consts::OS, b"test-grant")
            .expect("active grant should persist");
        let canonical_duplicate_left =
            std::fs::canonicalize(&duplicate_left).expect("left should canonicalize");
        let canonical_duplicate_right =
            std::fs::canonicalize(&duplicate_right).expect("right should canonicalize");
        let canonical_version_old =
            std::fs::canonicalize(&version_old).expect("old version should canonicalize");
        let canonical_version_new =
            std::fs::canonicalize(&version_new).expect("new version should canonicalize");
        let duplicate = check_exact_duplicate(
            &mut database,
            scope.id,
            &canonical_duplicate_left,
            &canonical_duplicate_right,
        )
        .expect("duplicate source should exist");
        let version = suggest_file_version(
            &mut database,
            scope.id,
            &canonical_version_old,
            &canonical_version_new,
        )
        .expect("version source should exist");

        let inbox = refresh_smart_cleanup_inbox(&mut database, scope.id)
            .expect("current sources should refresh");
        assert_eq!(inbox.items.len(), 2);
        assert_eq!(
            inbox.items[0].source_kind,
            SmartCleanupSourceKind::ExactDuplicate
        );
        assert_eq!(inbox.items[1].source_kind, SmartCleanupSourceKind::Version);
        assert!(inbox.items.iter().all(|item| item.current_evidence));
        assert!(inbox.items.iter().all(|item| item.verification_required));
        assert!(inbox.items.iter().all(|item| item.review_assistance_only));
        assert!(inbox.items.iter().all(|item| !item.cleanup_authorized));
        assert!(inbox.evaluation_complete);
        assert!(!inbox.action_authorized);
        let json = serde_json::to_string(&inbox).expect("Inbox should serialize");
        for private in [
            "private-duplicate-left",
            "private-duplicate-right",
            "private-plan-v1",
            "private-plan-v2",
            "display_path",
            "base_key",
            "extension_key",
            "reclaimable",
        ] {
            assert!(!json.contains(private));
        }
        assert_eq!(
            std::fs::read(&duplicate_left).expect("duplicate should remain"),
            b"private duplicate bytes"
        );

        decide_exact_duplicate(
            &mut database,
            duplicate.relation_id,
            FileRelationDecisionKind::Accepted,
        )
        .expect("duplicate graph feedback should persist");
        decide_file_version(
            &mut database,
            version.relation_id,
            FileRelationDecisionKind::Rejected,
        )
        .expect("version graph feedback should persist");
        let filtered = refresh_smart_cleanup_inbox(&mut database, scope.id)
            .expect("decided relations should be safely filtered");
        assert!(filtered.items.is_empty());
        assert_eq!(filtered.evaluated_source_count, 2);
        assert!(!filtered.action_authorized);
    }

    #[test]
    fn cleanup_duplicate_detail_reverifies_paths_and_rejects_stale_or_inactive_requests() {
        let directory = tempfile::tempdir().expect("fixture root should exist");
        let scope_path = directory.path().join("cleanup-detail-duplicate");
        std::fs::create_dir(&scope_path).expect("scope should create");
        let left = scope_path.join("private-left.bin");
        let right = scope_path.join("private-right.bin");
        std::fs::write(&left, b"same private detail bytes").expect("left should write");
        std::fs::write(&right, b"same private detail bytes").expect("right should write");
        let mut database = ManifestDatabase::open_in_memory().expect("database should open");
        let scope = authorize_scope(&database, &scope_path).expect("scope should authorize");
        scan_scope(&mut database, scope.id).expect("scope should scan");
        database
            .upsert_scope_access_grant(scope.id, std::env::consts::OS, b"test-grant")
            .expect("active grant should persist");
        let canonical_left = std::fs::canonicalize(&left).expect("left should canonicalize");
        let canonical_right = std::fs::canonicalize(&right).expect("right should canonicalize");
        let candidate =
            check_exact_duplicate(&mut database, scope.id, &canonical_left, &canonical_right)
                .expect("duplicate should persist");
        let item = database
            .smart_cleanup_relation_item(
                candidate.relation_id,
                candidate.evidence.observed_at_unix_ms,
            )
            .expect("current observation should map");

        let detail = cleanup_source_detail(
            &mut database,
            scope.id,
            SmartCleanupSourceKind::ExactDuplicate,
            candidate.relation_id,
            item.source_observation_id,
        )
        .expect("explicit current detail should live-verify");
        assert_eq!(detail.api_version, CleanupSourceDetail::API_VERSION);
        assert_eq!(detail.members.len(), 2);
        assert_eq!(
            detail.selection_rule,
            CleanupSourceSelectionRule::EitherMemberIsTarget
        );
        let detail_paths = detail
            .members
            .iter()
            .map(|member| member.display_path.as_str())
            .collect::<Vec<_>>();
        assert!(
            detail
                .members
                .iter()
                .all(|member| member.role == CleanupSourceMemberRole::DuplicateCandidate)
        );
        assert!(detail_paths.contains(&canonical_left.to_string_lossy().as_ref()));
        assert!(detail_paths.contains(&canonical_right.to_string_lossy().as_ref()));
        assert_ne!(detail.source_observation_id, item.source_observation_id);
        assert!(detail.current_evidence);
        assert!(detail.user_requested_paths);
        assert!(!detail.action_authorized);
        assert!(!detail.execution_available);

        let stale = cleanup_source_detail(
            &mut database,
            scope.id,
            SmartCleanupSourceKind::ExactDuplicate,
            candidate.relation_id,
            item.source_observation_id,
        )
        .expect_err("an older Inbox observation must fail closed");
        assert_eq!(stale.code(), "cleanup_action_source_not_current");

        database
            .mark_scope_access_grant_revoked(scope.id)
            .expect("grant should revoke");
        let denied = cleanup_source_detail(
            &mut database,
            scope.id,
            SmartCleanupSourceKind::ExactDuplicate,
            candidate.relation_id,
            detail.source_observation_id,
        )
        .expect_err("an inactive grant must fail before another live read");
        assert_eq!(denied.code(), "scope_access_grant_not_active");
    }

    #[test]
    fn cleanup_version_detail_preserves_older_target_newer_keeper_direction() {
        let directory = tempfile::tempdir().expect("fixture root should exist");
        let scope_path = directory.path().join("cleanup-detail-version");
        std::fs::create_dir(&scope_path).expect("scope should create");
        let older = scope_path.join("private-plan-v1.md");
        let newer = scope_path.join("private-plan-v2.md");
        std::fs::write(&older, b"old revision").expect("older should write");
        std::fs::write(&newer, b"new revision with different bytes").expect("newer should write");
        let mut database = ManifestDatabase::open_in_memory().expect("database should open");
        let scope = authorize_scope(&database, &scope_path).expect("scope should authorize");
        scan_scope(&mut database, scope.id).expect("scope should scan");
        database
            .upsert_scope_access_grant(scope.id, std::env::consts::OS, b"test-grant")
            .expect("active grant should persist");
        let canonical_newer = std::fs::canonicalize(&newer).expect("newer should canonicalize");
        let canonical_older = std::fs::canonicalize(&older).expect("older should canonicalize");
        let candidate =
            suggest_file_version(&mut database, scope.id, &canonical_newer, &canonical_older)
                .expect("version should persist");
        let item = database
            .smart_cleanup_relation_item(
                candidate.relation_id,
                candidate.evidence.observed_at_unix_ms,
            )
            .expect("current observation should map");

        let detail = cleanup_source_detail(
            &mut database,
            scope.id,
            SmartCleanupSourceKind::Version,
            candidate.relation_id,
            item.source_observation_id,
        )
        .expect("explicit version detail should live-verify");
        assert_eq!(
            detail.selection_rule,
            CleanupSourceSelectionRule::OlderTargetNewerKeeper
        );
        assert_eq!(detail.members.len(), 2);
        assert_eq!(
            detail.members[0].role,
            CleanupSourceMemberRole::OlderVersion
        );
        assert_eq!(
            detail.members[0].display_path,
            canonical_older.to_string_lossy()
        );
        assert_eq!(
            detail.members[1].role,
            CleanupSourceMemberRole::NewerVersion
        );
        assert_eq!(
            detail.members[1].display_path,
            canonical_newer.to_string_lossy()
        );
        assert_ne!(detail.source_observation_id, item.source_observation_id);
        assert!(!detail.action_authorized);
        assert!(!detail.execution_available);
    }

    #[test]
    fn smart_cleanup_inbox_omits_stale_sources_and_denies_inactive_grants() {
        let directory = tempfile::tempdir().expect("fixture root should exist");
        let scope_path = directory.path().join("cleanup-stale");
        std::fs::create_dir(&scope_path).expect("scope should create");
        let left = scope_path.join("private-left.bin");
        let right = scope_path.join("private-right.bin");
        std::fs::write(&left, b"same private bytes").expect("left should write");
        std::fs::write(&right, b"same private bytes").expect("right should write");
        let mut database = ManifestDatabase::open_in_memory().expect("database should open");
        let scope = authorize_scope(&database, &scope_path).expect("scope should authorize");
        scan_scope(&mut database, scope.id).expect("scope should scan");
        database
            .upsert_scope_access_grant(scope.id, std::env::consts::OS, b"test-grant")
            .expect("active grant should persist");
        check_exact_duplicate(
            &mut database,
            scope.id,
            &std::fs::canonicalize(&left).expect("left should canonicalize"),
            &std::fs::canonicalize(&right).expect("right should canonicalize"),
        )
        .expect("duplicate source should exist");

        std::fs::write(&right, b"changed private bytes").expect("right should change");
        let stale = refresh_smart_cleanup_inbox(&mut database, scope.id)
            .expect("stale evidence should be omitted without a path leak");
        assert!(stale.items.is_empty());
        assert_eq!(stale.not_current_source_count, 1);
        assert!(stale.evaluation_complete);

        database
            .mark_scope_access_grant_revoked(scope.id)
            .expect("grant should revoke");
        let denied = refresh_smart_cleanup_inbox(&mut database, scope.id)
            .expect_err("revoked grant must fail before another file open");
        assert_eq!(denied.code(), "scope_access_grant_not_active");
    }

    #[test]
    fn smart_cleanup_stale_classifier_covers_safe_source_shape_changes() {
        for error in [
            ProjectError::RelationSourceUnavailable,
            ProjectError::RelationSourceSymlinkOrReparseDenied,
            ProjectError::RelationSourceOutsideScope,
            ProjectError::RelationSourceMustBeFile,
            ProjectError::RelationSourceIdentityUnavailable,
            ProjectError::RelationSourceIdentityChanged,
            ProjectError::RelationSourceMetadataChanged,
            ProjectError::RelationSourceEmpty,
            ProjectError::RelationSourceTooLarge,
            ProjectError::RelationSameFileIdentity,
            ProjectError::RelationContentDiffers,
            ProjectError::VersionNameUnsupported,
            ProjectError::VersionBaseMismatch,
            ProjectError::VersionExtensionMismatch,
            ProjectError::VersionNumberEqual,
            ProjectError::Database(DatabaseError::FileRelationCandidateNotCurrent),
            ProjectError::Database(DatabaseError::ScreenshotGroupCandidateNotCurrent),
        ] {
            assert!(cleanup_source_is_not_current(&error), "{}", error.code());
        }
        for error in [
            ProjectError::RelationSourceOpenFailed,
            ProjectError::RelationSourceReadFailed,
            ProjectError::RelationComparisonTimedOut,
        ] {
            assert!(cleanup_source_evaluation_is_incomplete(&error));
            assert!(!cleanup_source_is_not_current(&error));
        }
        for error in [
            ProjectError::RelationPathMustBeAbsolute,
            ProjectError::RelationPathDecodeFailed,
            ProjectError::Database(DatabaseError::InvalidStoredValue),
        ] {
            assert!(!cleanup_source_is_not_current(&error));
            assert!(!cleanup_source_evaluation_is_incomplete(&error));
        }
    }
}
