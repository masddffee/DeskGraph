use std::collections::HashSet;
#[cfg(unix)]
use std::ffi::CString;
use std::fmt;
#[cfg(not(any(unix, windows)))]
use std::fs::DirBuilder;
#[cfg(any(not(any(unix, windows)), test))]
use std::fs::OpenOptions;
use std::fs::{self, File};
#[cfg(unix)]
use std::os::fd::{AsRawFd, FromRawFd};
#[cfg(unix)]
use std::os::unix::ffi::OsStrExt;
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
#[cfg(any(target_os = "macos", target_os = "linux"))]
use std::path::Component;
#[cfg(not(test))]
use std::path::{MAIN_SEPARATOR, Path};
#[cfg(test)]
use std::path::{MAIN_SEPARATOR, Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use deskgraph_domain::{
    ActionCommandKind, ActionCommandStart, ActionExecutionBinding, ActionExecutionRecord,
    ActionExecutionStrategy, ActionJournalEvent, ActionJournalEventKind, ActionOperation,
    ActionPlanPreview, ActionPlanState, ActionPlanSummary, ActionPolicyReport, AuthorizedScope,
    CleanupActionOperation, CleanupActionPlanPreview, CleanupActionPlanState,
    CleanupActionPolicyReport, ExplicitFileVersionName, ExtractionJobProgress, ExtractionOperation,
    ExtractionStats, ExtractionStatus, FileRelationCandidate, FileRelationCandidateState,
    FileRelationCandidateSummary, FileRelationComparisonKind, FileRelationCreator,
    FileRelationDecision, FileRelationDecisionCreator, FileRelationDecisionKind,
    FileRelationEndpoint, FileRelationEvidence, FileRelationKind, FileVersionCandidate,
    FileVersionDecision, FileVersionEvidence, FileVersionSignalKind, FolderCategoryCount,
    FolderFileCategory, ImageFormat, ImageMetadata, ManifestStats, ProjectCandidate,
    ProjectCandidateState, ProjectCandidateSummary, ProjectDecision, ProjectDecisionCreator,
    ProjectDecisionKind, ProjectSignal, ProjectSignalKind, ProjectSuggestion,
    ProjectSuggestionCreator, ScanJobProgress, ScanReport, ScanStatus, ScreenshotGroupCandidate,
    ScreenshotGroupCandidateState, ScreenshotGroupCandidateSummary, ScreenshotGroupCreator,
    ScreenshotGroupEvidence, ScreenshotGroupMember, ScreenshotGroupRuleKind,
    SearchFolderListResponse, SearchFolderOption, SmartCleanupCandidateState,
    SmartCleanupInboxItem, SmartCleanupSourceKind, WatchEventProgress, WatchEventReason,
    WatchEventStatus, is_valid_image_dimensions, is_valid_xlsx_cell_reference,
    parse_explicit_file_version_name, reduce_action_journal,
};
use deskgraph_identity::{
    IdentityNodeKind, comparison_key, is_symlink_or_reparse_point, path_from_raw,
    platform_identity, platform_identity_for_open_file,
};
#[cfg(any(target_os = "macos", target_os = "linux"))]
use rusqlite::OpenFlags;
use rusqlite::{Connection, OptionalExtension, Transaction, TransactionBehavior, params};

#[cfg(windows)]
mod fence_windows;

const MIGRATIONS: &[Migration] = &[
    Migration {
        version: 1,
        name: "manifest",
        sql: include_str!("../../../migrations/0001_manifest.sql"),
    },
    Migration {
        version: 2,
        name: "resumable_scan_jobs",
        sql: include_str!("../../../migrations/0002_resumable_scan_jobs.sql"),
    },
    Migration {
        version: 3,
        name: "content_extraction",
        sql: include_str!("../../../migrations/0003_content_extraction.sql"),
    },
    Migration {
        version: 4,
        name: "content_chunk_provenance",
        sql: include_str!("../../../migrations/0004_content_chunk_provenance.sql"),
    },
    Migration {
        version: 5,
        name: "lexical_search",
        sql: include_str!("../../../migrations/0005_lexical_search.sql"),
    },
    Migration {
        version: 6,
        name: "watch_reconciliation",
        sql: include_str!("../../../migrations/0006_watch_reconciliation.sql"),
    },
    Migration {
        version: 7,
        name: "action_plan_preview",
        sql: include_str!("../../../migrations/0007_action_plan_preview.sql"),
    },
    Migration {
        version: 8,
        name: "project_candidates",
        sql: include_str!("../../../migrations/0008_project_candidates.sql"),
    },
    Migration {
        version: 9,
        name: "exact_duplicate_candidates",
        sql: include_str!("../../../migrations/0009_exact_duplicate_candidates.sql"),
    },
    Migration {
        version: 10,
        name: "file_relation_feedback",
        sql: include_str!("../../../migrations/0010_file_relation_feedback.sql"),
    },
    Migration {
        version: 11,
        name: "file_version_candidates",
        sql: include_str!("../../../migrations/0011_file_version_candidates.sql"),
    },
    Migration {
        version: 12,
        name: "file_version_feedback",
        sql: include_str!("../../../migrations/0012_file_version_feedback.sql"),
    },
    Migration {
        version: 13,
        name: "ooxml_chunk_provenance",
        sql: include_str!("../../../migrations/0013_ooxml_chunk_provenance.sql"),
    },
    Migration {
        version: 14,
        name: "image_metadata",
        sql: include_str!("../../../migrations/0014_image_metadata.sql"),
    },
    Migration {
        version: 15,
        name: "ocr_jobs_and_provenance",
        sql: include_str!("../../../migrations/0015_ocr_jobs_and_provenance.sql"),
    },
    Migration {
        version: 16,
        name: "nullable_ocr_confidence",
        sql: include_str!("../../../migrations/0016_nullable_ocr_confidence.sql"),
    },
    Migration {
        version: 17,
        name: "watch_active_deadline_index",
        sql: include_str!("../../../migrations/0017_watch_active_deadline_index.sql"),
    },
    Migration {
        version: 18,
        name: "watch_reconciliation_kind",
        sql: include_str!("../../../migrations/0018_watch_reconciliation_kind.sql"),
    },
    Migration {
        version: 19,
        name: "action_transaction_journal",
        sql: include_str!("../../../migrations/0019_action_transaction_journal.sql"),
    },
    Migration {
        version: 20,
        name: "scope_access_grants",
        sql: include_str!("../../../migrations/0020_scope_access_grants.sql"),
    },
    Migration {
        version: 21,
        name: "screenshot_group_candidates",
        sql: include_str!("../../../migrations/0021_screenshot_group_candidates.sql"),
    },
    Migration {
        version: 22,
        name: "cleanup_action_plan_preview",
        sql: include_str!("../../../migrations/0022_cleanup_action_plan_preview.sql"),
    },
    Migration {
        version: 23,
        name: "coverage_root_overlap_guard",
        sql: include_str!("../../../migrations/0023_coverage_root_overlap_guard.sql"),
    },
    Migration {
        version: 24,
        name: "scope_exclusions_and_privacy_purge",
        sql: include_str!("../../../migrations/0024_scope_exclusions_and_privacy_purge.sql"),
    },
    Migration {
        version: 25,
        name: "scope_root_revocation",
        sql: include_str!("../../../migrations/0025_scope_root_revocation.sql"),
    },
    Migration {
        version: 26,
        name: "scope_root_revocation_hardening",
        sql: include_str!("../../../migrations/0026_scope_root_revocation_hardening.sql"),
    },
    Migration {
        version: 27,
        name: "folder_search_descendant_index",
        sql: include_str!("../../../migrations/0027_folder_search_descendant_index.sql"),
    },
];

#[cfg(any(target_os = "macos", target_os = "linux"))]
const READ_ONLY_BUSY_TIMEOUT: Duration = Duration::from_secs(1);
const READ_ONLY_QUERY_TIMEOUT: Duration = Duration::from_secs(2);
const READ_ONLY_PROGRESS_OPS: i32 = 100;
const MAX_EXTRACTION_SOURCE_BYTES: u64 = 64 * 1024 * 1024;
const MAX_EXTRACTION_OUTPUT_BYTES: u64 = 64 * 1024 * 1024;
const MIN_ACTION_EXECUTOR_LEASE_MS: i64 = 1_000;
const MAX_ACTION_EXECUTOR_LEASE_MS: i64 = 120_000;
const MAX_EXTRACTION_CHUNKS: usize = 65_536;
const MAX_EXTRACTION_CHUNK_BYTES: usize = 64 * 1024;
const MAX_SEARCH_MATCH_BYTES: usize = 1024;
const MAX_SEARCH_CANDIDATES_PER_SOURCE: u32 = 100;
pub const DEFAULT_SEARCH_FOLDER_LIST_LIMIT: u32 = 200;
pub const MAX_SEARCH_FOLDER_LIST_LIMIT: u32 = 500;
const MAX_WATCH_PATH_BYTES: usize = 64 * 1024;
const MAX_SCOPE_EXCLUSION_PATH_BYTES: usize = 64 * 1024;
const MAX_SCOPE_EXCLUSION_BATCH: usize = 128;
const MAX_SCOPE_EXCLUSION_IDENTITY_KIND_BYTES: usize = 128;
const MAX_SCOPE_EXCLUSION_IDENTITY_KEY_BYTES: usize = 1024;
const MAX_ACTION_PATH_BYTES: usize = 64 * 1024;
const MAX_FOLDER_PROFILE_ENTRIES: u64 = 100_000;
const MAX_FILE_RELATION_SOURCE_BYTES: u64 = 64 * 1024 * 1024;
const MAX_SCOPE_ACCESS_GRANT_BYTES: usize = 1024 * 1024;
const SCOPE_FILESYSTEM_FENCE_WAIT_TIMEOUT: Duration = Duration::from_secs(2);
const SCOPE_FILESYSTEM_FENCE_RETRY_INTERVAL: Duration = Duration::from_millis(10);
const MAX_SCREENSHOT_GROUP_IMAGES: u32 = 2_000;
const MAX_SCREENSHOT_GROUPS: usize = 20;
const MAX_SCREENSHOT_GROUP_MEMBERS: usize = 20;
const SCREENSHOT_GROUP_TIME_WINDOW_NS: i64 = 600 * 1_000_000_000;
static IN_MEMORY_FENCE_DOMAIN_SEQUENCE: AtomicU64 = AtomicU64::new(1);

struct Migration {
    version: i64,
    name: &'static str,
    sql: &'static str,
}

#[derive(Clone, Debug)]
struct ScopeExclusionValidationContext {
    scope_id: i64,
    root_path_key: String,
    revision: i64,
    separator: char,
    platform: String,
}

fn stable_scope_exclusion_identity_is_valid(
    platform: &str,
    kind: ScopeExclusionKind,
    identity_kind: &str,
    identity_key: &[u8],
) -> bool {
    let expected_key_len = match platform {
        "macos" | "linux" if identity_kind == "unix_device_inode" => 17,
        "windows" if identity_kind == "windows_volume_file_index" => 13,
        _ => return false,
    };
    let expected_kind_byte = match kind {
        ScopeExclusionKind::File => b'f',
        ScopeExclusionKind::Folder => b'd',
    };
    identity_kind.len() <= MAX_SCOPE_EXCLUSION_IDENTITY_KIND_BYTES
        && identity_key.len() == expected_key_len
        && identity_key.len() <= MAX_SCOPE_EXCLUSION_IDENTITY_KEY_BYTES
        && identity_key.first() == Some(&expected_kind_byte)
}

fn platform_separator(platform: &str) -> Result<char, DatabaseError> {
    match platform {
        "windows" => Ok('\\'),
        "macos" | "linux" => Ok('/'),
        _ => Err(DatabaseError::InvalidStoredValue),
    }
}

fn canonical_descendant_of(path_key: &str, ancestor_key: &str, separator: char) -> bool {
    path_key.len() > ancestor_key.len()
        && path_key.starts_with(ancestor_key)
        && (ancestor_key.ends_with(separator)
            || path_key.as_bytes().get(ancestor_key.len()) == Some(&(separator as u8)))
}

fn scope_path_key_is_excluded(
    connection: &Connection,
    scope_id: i64,
    path_key: &str,
) -> Result<bool, DatabaseError> {
    let excluded: i64 = connection.query_row(
        "SELECT EXISTS( \
             SELECT 1 FROM scope_exclusions x \
             JOIN authorized_scopes s ON s.id=x.scope_id \
             WHERE x.scope_id=?1 AND ( \
                 x.path_key=?2 OR (x.kind='folder' \
                   AND length(?2)>length(x.path_key) \
                   AND substr(?2,1,length(x.path_key))=x.path_key \
                   AND (substr(x.path_key,-1,1)=CASE WHEN s.platform='windows' THEN char(92) ELSE '/' END \
                        OR substr(?2,length(x.path_key)+1,1)=CASE WHEN s.platform='windows' THEN char(92) ELSE '/' END)) \
                 OR EXISTS(SELECT 1 FROM locations l JOIN nodes n ON n.id=l.node_id \
                    WHERE l.scope_id=?1 AND l.path_key=?2 AND l.present=1 \
                      AND n.identity_kind=x.identity_kind AND n.identity_key=x.identity_key)))",
        params![scope_id, path_key],
        |row| row.get(0),
    )?;
    Ok(excluded != 0)
}

fn scope_identity_is_excluded(
    connection: &Connection,
    scope_id: i64,
    identity_kind: &str,
    identity_key: &[u8],
) -> Result<bool, DatabaseError> {
    if identity_kind.is_empty() || identity_key.is_empty() {
        return Err(DatabaseError::InvalidStoredValue);
    }
    Ok(connection.query_row(
        "SELECT EXISTS(SELECT 1 FROM scope_exclusions \
         WHERE scope_id=?1 AND identity_kind=?2 AND identity_key=?3)",
        params![scope_id, identity_kind, identity_key],
        |row| row.get::<_, i64>(0),
    )? != 0)
}

fn assert_scope_identity_allowed(
    connection: &Connection,
    scope_id: i64,
    identity_kind: &str,
    identity_key: &[u8],
) -> Result<(), DatabaseError> {
    if scope_identity_is_excluded(connection, scope_id, identity_kind, identity_key)? {
        Err(DatabaseError::ScopePolicyRevisionStale)
    } else {
        Ok(())
    }
}

fn assert_scope_path_key_allowed(
    connection: &Connection,
    scope_id: i64,
    path_key: &str,
) -> Result<(), DatabaseError> {
    if scope_path_key_is_excluded(connection, scope_id, path_key)? {
        Err(DatabaseError::ScopePolicyRevisionStale)
    } else {
        Ok(())
    }
}

fn current_scope_policy_revision_from_connection(
    connection: &Connection,
    scope_id: i64,
) -> Result<i64, DatabaseError> {
    connection
        .query_row(
            "SELECT policy_revision FROM authorized_scopes WHERE id = ?1",
            [scope_id],
            |row| row.get(0),
        )
        .optional()?
        .ok_or(DatabaseError::ScopeNotFound)
}

fn scope_policy_binding_is_current(
    connection: &Connection,
    binding: ScopePolicyBinding,
) -> Result<bool, DatabaseError> {
    if binding.scope_id <= 0 || binding.revision <= 0 {
        return Ok(false);
    }
    let current = connection.query_row(
        "SELECT EXISTS(SELECT 1 FROM authorized_scopes s \
         JOIN scope_access_grants g ON g.scope_id = s.id AND g.state = 'active' \
         WHERE s.id = ?1 AND s.policy_revision = ?2 \
           AND s.platform = ?3 AND g.platform = ?3)",
        params![binding.scope_id, binding.revision, std::env::consts::OS],
        |row| row.get::<_, i64>(0),
    )?;
    Ok(current != 0)
}

fn core_scope_policy_binding_is_current(
    connection: &Connection,
    binding: ScopeRevisionBinding,
) -> Result<bool, DatabaseError> {
    if binding.scope_id <= 0 || binding.revision <= 0 {
        return Ok(false);
    }
    Ok(connection.query_row(
        "SELECT EXISTS(SELECT 1 FROM authorized_scopes WHERE id=?1 AND policy_revision=?2)",
        params![binding.scope_id, binding.revision],
        |row| row.get::<_, i64>(0),
    )? != 0)
}

fn assert_scope_revision_binding_in_transaction(
    transaction: &Transaction<'_>,
    binding: ScopeRevisionBinding,
) -> Result<(), DatabaseError> {
    if binding.scope_id <= 0 || binding.revision <= 0 {
        return Err(DatabaseError::ScopePolicyRevisionStale);
    }
    let revision = transaction
        .query_row(
            "SELECT policy_revision FROM authorized_scopes WHERE id = ?1",
            [binding.scope_id],
            |row| row.get::<_, i64>(0),
        )
        .optional()?
        .ok_or(DatabaseError::ScopeNotFound)?;
    if revision != binding.revision {
        return Err(DatabaseError::ScopePolicyRevisionStale);
    }
    Ok(())
}

fn scope_exclusion_matcher_from_connection(
    connection: &Connection,
    scope_id: i64,
) -> Result<ScopeExclusionMatcher, DatabaseError> {
    let (revision, platform): (i64, String) = connection
        .query_row(
            "SELECT policy_revision, platform FROM authorized_scopes WHERE id = ?1",
            [scope_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()?
        .ok_or(DatabaseError::ScopeNotFound)?;
    let mut statement = connection.prepare(
        "SELECT path_key, kind, identity_kind, identity_key \
         FROM scope_exclusions WHERE scope_id = ?1 ORDER BY id",
    )?;
    let rows = statement.query_map([scope_id], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, String>(2)?,
            row.get::<_, Vec<u8>>(3)?,
        ))
    })?;
    let mut exclusions = Vec::new();
    let mut excluded_identities = Vec::new();
    for row in rows {
        let (path_key, kind, identity_kind, identity_key) = row?;
        let kind = ScopeExclusionKind::from_db(&kind)?;
        if !stable_scope_exclusion_identity_is_valid(&platform, kind, &identity_kind, &identity_key)
        {
            return Err(DatabaseError::InvalidStoredValue);
        }
        exclusions.push((path_key, kind));
        excluded_identities.push((identity_kind, identity_key));
    }
    Ok(ScopeExclusionMatcher {
        scope_id,
        revision,
        exclusions,
        excluded_identities,
        separator: platform_separator(&platform)?,
    })
}

fn assert_scope_policy_binding_in_transaction(
    transaction: &Transaction<'_>,
    binding: ScopePolicyBinding,
) -> Result<(), DatabaseError> {
    if binding.scope_id <= 0 || binding.revision <= 0 {
        return Err(DatabaseError::ScopePolicyRevisionStale);
    }
    let state = transaction
        .query_row(
            "SELECT s.policy_revision, g.state \
             FROM authorized_scopes s \
             LEFT JOIN scope_access_grants g \
               ON g.scope_id = s.id AND g.platform = s.platform \
              AND s.platform = ?2 AND g.platform = ?2 \
             WHERE s.id = ?1",
            params![binding.scope_id, std::env::consts::OS],
            |row| Ok((row.get::<_, i64>(0)?, row.get::<_, Option<String>>(1)?)),
        )
        .optional()?
        .ok_or(DatabaseError::ScopeNotFound)?;
    if state.0 != binding.revision {
        return Err(DatabaseError::ScopePolicyRevisionStale);
    }
    if state.1.as_deref() != Some("active") {
        return Err(DatabaseError::ScopeAccessGrantNotActive);
    }
    Ok(())
}

fn scan_job_revision_binding(
    transaction: &Transaction<'_>,
    job_id: i64,
) -> Result<ScopeRevisionBinding, DatabaseError> {
    transaction
        .query_row(
            "SELECT scope_id, policy_revision FROM scan_jobs WHERE id=?1",
            [job_id],
            |row| {
                Ok(ScopeRevisionBinding {
                    scope_id: row.get(0)?,
                    revision: row.get(1)?,
                })
            },
        )
        .optional()?
        .ok_or(DatabaseError::ScanJobNotFound)
}

fn extraction_job_revision_binding(
    transaction: &Transaction<'_>,
    job_id: i64,
) -> Result<ScopeRevisionBinding, DatabaseError> {
    transaction
        .query_row(
            "SELECT scope_id, policy_revision FROM extraction_jobs WHERE id=?1",
            [job_id],
            |row| {
                Ok(ScopeRevisionBinding {
                    scope_id: row.get(0)?,
                    revision: row.get(1)?,
                })
            },
        )
        .optional()?
        .ok_or(DatabaseError::ExtractionJobNotFound)
}

fn scope_exclusion_validation_context(
    connection: &Connection,
    scope_id: i64,
) -> Result<ScopeExclusionValidationContext, DatabaseError> {
    let (root_path_key, platform, revision) = connection
        .query_row(
            "SELECT path_key, platform, policy_revision FROM authorized_scopes WHERE id = ?1",
            [scope_id],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, i64>(2)?,
                ))
            },
        )
        .optional()?
        .ok_or(DatabaseError::ScopeNotFound)?;
    Ok(ScopeExclusionValidationContext {
        scope_id,
        root_path_key,
        separator: platform_separator(&platform)?,
        platform,
        revision,
    })
}

fn validate_scope_exclusion_batch(
    connection: &Connection,
    scope: &ScopeExclusionValidationContext,
    writes: &[ScopeExclusionWrite<'_>],
) -> Result<(), DatabaseError> {
    if writes.is_empty() || writes.len() > MAX_SCOPE_EXCLUSION_BATCH {
        return Err(DatabaseError::ScopeExclusionInputInvalid);
    }
    let existing = {
        let mut statement = connection.prepare(
            "SELECT path_key, kind, identity_kind, identity_key \
             FROM scope_exclusions WHERE scope_id = ?1 ORDER BY id",
        )?;
        statement
            .query_map([scope.scope_id], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, Vec<u8>>(3)?,
                ))
            })?
            .collect::<Result<Vec<_>, _>>()?
    };
    let mut seen = HashSet::with_capacity(writes.len());
    for write in writes {
        if write.path_raw.is_empty()
            || write.path_raw.len() > MAX_SCOPE_EXCLUSION_PATH_BYTES
            || write.path_key.is_empty()
            || write.path_key.len() > MAX_SCOPE_EXCLUSION_PATH_BYTES
            || write.display_path.is_empty()
            || write.display_path.len() > MAX_SCOPE_EXCLUSION_PATH_BYTES
            || !seen.insert(write.path_key)
            || !canonical_descendant_of(write.path_key, &scope.root_path_key, scope.separator)
            || !stable_scope_exclusion_identity_is_valid(
                &scope.platform,
                write.kind,
                write.identity_kind,
                write.identity_key,
            )
        {
            return Err(DatabaseError::ScopeExclusionInputInvalid);
        }
        let decoded =
            path_from_raw(write.path_raw).map_err(|_| DatabaseError::ScopeExclusionInputInvalid)?;
        if comparison_key(&decoded) != write.path_key {
            return Err(DatabaseError::ScopeExclusionInputInvalid);
        }
        for (existing_key, existing_kind, existing_identity_kind, existing_identity_key) in
            &existing
        {
            let existing_kind = ScopeExclusionKind::from_db(existing_kind)?;
            if !stable_scope_exclusion_identity_is_valid(
                &scope.platform,
                existing_kind,
                existing_identity_kind,
                existing_identity_key,
            ) {
                return Err(DatabaseError::InvalidStoredValue);
            }
            if write.path_key == existing_key
                || (write.identity_kind == existing_identity_kind
                    && write.identity_key == existing_identity_key)
                || (existing_kind == ScopeExclusionKind::Folder
                    && canonical_descendant_of(write.path_key, existing_key, scope.separator))
                || (write.kind == ScopeExclusionKind::Folder
                    && canonical_descendant_of(existing_key, write.path_key, scope.separator))
            {
                return Err(DatabaseError::ScopeExclusionInputInvalid);
            }
        }
    }
    for (index, left) in writes.iter().enumerate() {
        for right in writes.iter().skip(index + 1) {
            if (left.identity_kind == right.identity_kind
                && left.identity_key == right.identity_key)
                || (left.kind == ScopeExclusionKind::Folder
                    && canonical_descendant_of(right.path_key, left.path_key, scope.separator))
                || (right.kind == ScopeExclusionKind::Folder
                    && canonical_descendant_of(left.path_key, right.path_key, scope.separator))
            {
                return Err(DatabaseError::ScopeExclusionInputInvalid);
            }
        }
    }
    Ok(())
}

fn scope_exclusions_from_connection(
    connection: &Connection,
    scope_id: i64,
) -> Result<Vec<ScopeExclusionRecord>, DatabaseError> {
    let mut statement = connection.prepare(
        "SELECT id, scope_id, kind, path_raw, path_key, display_path, \
                policy_revision, created_at_unix_ms \
         FROM scope_exclusions WHERE scope_id = ?1 ORDER BY id",
    )?;
    let rows = statement.query_map([scope_id], |row| {
        Ok((
            row.get::<_, i64>(0)?,
            row.get::<_, i64>(1)?,
            row.get::<_, String>(2)?,
            row.get::<_, Vec<u8>>(3)?,
            row.get::<_, String>(4)?,
            row.get::<_, String>(5)?,
            row.get::<_, i64>(6)?,
            row.get::<_, i64>(7)?,
        ))
    })?;
    rows.map(|row| {
        let row = row?;
        Ok(ScopeExclusionRecord {
            id: row.0,
            scope_id: row.1,
            kind: ScopeExclusionKind::from_db(&row.2)?,
            path_raw: row.3,
            path_key: row.4,
            display_path: row.5,
            policy_revision: row.6,
            created_at_unix_ms: row.7,
        })
    })
    .collect()
}

fn begin_privacy_purge_capability(
    transaction: &Transaction<'_>,
    scope_id: i64,
    from_revision: i64,
    to_revision: i64,
    now_unix_ms: i64,
) -> Result<Vec<u8>, DatabaseError> {
    transaction.execute(
        "INSERT INTO privacy_purge_capabilities( \
             nonce, scope_id, from_revision, to_revision, created_at_unix_ms \
         ) VALUES (randomblob(32), ?1, ?2, ?3, ?4)",
        params![scope_id, from_revision, to_revision, now_unix_ms],
    )?;
    transaction
        .query_row(
            "SELECT nonce FROM privacy_purge_capabilities WHERE scope_id = ?1",
            [scope_id],
            |row| row.get(0),
        )
        .map_err(Into::into)
}

fn insert_privacy_targets_for_writes(
    transaction: &Transaction<'_>,
    nonce: &[u8],
    scope: &ScopeExclusionValidationContext,
    writes: &[ScopeExclusionWrite<'_>],
) -> Result<(), DatabaseError> {
    for write in writes {
        transaction.execute(
            "INSERT OR IGNORE INTO privacy_purge_location_targets(nonce, location_id, node_id, direct_match) \
             SELECT ?1, id, node_id, 1 FROM locations \
             WHERE scope_id = ?2 AND (path_key = ?3 OR ( \
                 ?4 = 'folder' AND length(path_key) > length(?3) \
                 AND substr(path_key, 1, length(?3)) = ?3 \
                 AND (substr(?3, -1, 1) = ?5 \
                      OR substr(path_key, length(?3) + 1, 1) = ?5)))",
            params![
                nonce,
                scope.scope_id,
                write.path_key,
                write.kind.as_str(),
                scope.separator.to_string(),
            ],
        )?;
        transaction.execute(
            "INSERT OR IGNORE INTO privacy_purge_location_targets(nonce, location_id, node_id, direct_match) \
             SELECT ?1, l.id, l.node_id, 0 FROM locations l \
             JOIN nodes n ON n.id=l.node_id \
             WHERE l.scope_id=?2 AND n.identity_kind=?3 AND n.identity_key=?4",
            params![nonce, scope.scope_id, write.identity_kind, write.identity_key],
        )?;
    }
    close_privacy_targets_over_same_scope_hardlinks(transaction, nonce, scope.scope_id)?;
    for write in writes {
        transaction.execute(
            "INSERT OR IGNORE INTO privacy_purge_action_plan_targets(nonce, plan_id) \
             SELECT ?1, id FROM action_plans WHERE scope_id=?2 AND (source_path_key=?3 OR destination_path_key=?3 OR (?4='folder' AND ((length(source_path_key)>length(?3) AND substr(source_path_key,1,length(?3))=?3 AND (substr(?3,-1,1)=?5 OR substr(source_path_key,length(?3)+1,1)=?5)) OR (length(destination_path_key)>length(?3) AND substr(destination_path_key,1,length(?3))=?3 AND (substr(?3,-1,1)=?5 OR substr(destination_path_key,length(?3)+1,1)=?5)))))",
            params![nonce,scope.scope_id,write.path_key,write.kind.as_str(),scope.separator.to_string()],
        )?;
    }
    Ok(())
}

fn insert_all_privacy_targets(
    transaction: &Transaction<'_>,
    nonce: &[u8],
    scope: &ScopeExclusionValidationContext,
) -> Result<(), DatabaseError> {
    let separator = scope.separator.to_string();
    transaction.execute(
        "INSERT OR IGNORE INTO privacy_purge_location_targets(nonce, location_id, node_id, direct_match) \
         SELECT ?1, l.id, l.node_id, CASE WHEN EXISTS( \
             SELECT 1 FROM scope_exclusions e WHERE e.scope_id=l.scope_id AND (l.path_key=e.path_key OR (e.kind='folder' \
                 AND length(l.path_key)>length(e.path_key) AND substr(l.path_key,1,length(e.path_key))=e.path_key \
                 AND (substr(e.path_key,-1,1)=?3 OR substr(l.path_key,length(e.path_key)+1,1)=?3)))) THEN 1 ELSE 0 END \
         FROM locations l JOIN nodes n ON n.id=l.node_id \
         WHERE l.scope_id = ?2 AND EXISTS( \
             SELECT 1 FROM scope_exclusions e WHERE e.scope_id = l.scope_id \
               AND (l.path_key = e.path_key OR (e.kind = 'folder' \
                    AND length(l.path_key) > length(e.path_key) \
                    AND substr(l.path_key, 1, length(e.path_key)) = e.path_key \
                    AND (substr(e.path_key, -1, 1) = ?3 \
                         OR substr(l.path_key, length(e.path_key) + 1, 1) = ?3)) \
                   OR (n.identity_kind=e.identity_kind AND n.identity_key=e.identity_key)))",
        params![nonce, scope.scope_id, separator],
    )?;
    close_privacy_targets_over_same_scope_hardlinks(transaction, nonce, scope.scope_id)?;
    transaction.execute(
        "INSERT OR IGNORE INTO privacy_purge_action_plan_targets(nonce, plan_id) \
         SELECT ?1, p.id FROM action_plans p WHERE p.scope_id=?2 AND EXISTS(SELECT 1 FROM scope_exclusions e WHERE e.scope_id=p.scope_id AND (p.source_path_key=e.path_key OR p.destination_path_key=e.path_key OR (e.kind='folder' AND ((length(p.source_path_key)>length(e.path_key) AND substr(p.source_path_key,1,length(e.path_key))=e.path_key AND (substr(e.path_key,-1,1)=?3 OR substr(p.source_path_key,length(e.path_key)+1,1)=?3)) OR (length(p.destination_path_key)>length(e.path_key) AND substr(p.destination_path_key,1,length(e.path_key))=e.path_key AND (substr(e.path_key,-1,1)=?3 OR substr(p.destination_path_key,length(e.path_key)+1,1)=?3))))))",
        params![nonce,scope.scope_id,separator],
    )?;
    Ok(())
}

/// Builds the conservative target closure for withdrawing an entire coverage
/// root. Unlike an exclusion purge, this deliberately selects every
/// scope-owned derived record and never depends on a path prefix.
fn insert_full_scope_privacy_targets(
    transaction: &Transaction<'_>,
    nonce: &[u8],
    scope_id: i64,
) -> Result<(), DatabaseError> {
    transaction.execute(
        "INSERT OR IGNORE INTO privacy_purge_location_targets(\
             nonce, location_id, node_id, direct_match\
         ) SELECT ?1, id, node_id, 1 FROM locations WHERE scope_id=?2",
        params![nonce, scope_id],
    )?;
    close_privacy_targets_over_same_scope_hardlinks(transaction, nonce, scope_id)?;
    for (table, id_column, target_table, target_column) in [
        (
            "projects",
            "id",
            "privacy_purge_project_targets",
            "project_id",
        ),
        (
            "action_plans",
            "id",
            "privacy_purge_action_plan_targets",
            "plan_id",
        ),
        (
            "file_relation_candidates",
            "id",
            "privacy_purge_relation_targets",
            "relation_id",
        ),
        (
            "screenshot_group_candidates",
            "id",
            "privacy_purge_screenshot_group_targets",
            "group_id",
        ),
        (
            "cleanup_action_plans",
            "id",
            "privacy_purge_cleanup_action_plan_targets",
            "plan_id",
        ),
    ] {
        let sql = format!(
            "INSERT OR IGNORE INTO {target_table}(nonce, {target_column}) \
             SELECT ?1, {id_column} FROM {table} WHERE scope_id=?2"
        );
        transaction.execute(&sql, params![nonce, scope_id])?;
    }
    Ok(())
}

fn close_privacy_targets_over_same_scope_hardlinks(
    transaction: &Transaction<'_>,
    nonce: &[u8],
    scope_id: i64,
) -> Result<(), DatabaseError> {
    transaction.execute(
        "INSERT OR IGNORE INTO privacy_purge_node_targets(nonce, node_id) \
         SELECT ?1, node_id FROM privacy_purge_location_targets WHERE nonce = ?1",
        [nonce],
    )?;
    transaction.execute(
        "INSERT OR IGNORE INTO privacy_purge_location_targets(nonce, location_id, node_id, direct_match) \
         SELECT ?1, l.id, l.node_id, 0 FROM locations l \
         JOIN privacy_purge_node_targets n ON n.nonce = ?1 AND n.node_id = l.node_id \
         WHERE l.scope_id = ?2",
        params![nonce, scope_id],
    )?;
    transaction.execute(
        "INSERT OR IGNORE INTO privacy_purge_project_targets(nonce, project_id) \
         SELECT ?1, p.id FROM projects p WHERE p.scope_id=?2 AND ( \
             EXISTS(SELECT 1 FROM privacy_purge_node_targets n WHERE n.nonce=?1 AND n.node_id=p.root_folder_node_id) \
             OR EXISTS(SELECT 1 FROM edges e JOIN privacy_purge_node_targets n ON n.nonce=?1 AND n.node_id=e.source_node_id WHERE e.scope_id=p.scope_id AND e.kind='located_in' AND e.active=1 AND e.target_node_id=p.root_folder_node_id))",
        params![nonce,scope_id],
    )?;
    transaction.execute(
        "INSERT OR IGNORE INTO privacy_purge_action_plan_targets(nonce, plan_id) \
         SELECT ?1, p.id FROM action_plans p JOIN privacy_purge_node_targets n ON n.nonce=?1 AND n.node_id=p.node_id WHERE p.scope_id=?2",
        params![nonce,scope_id],
    )?;
    transaction.execute(
        "INSERT OR IGNORE INTO privacy_purge_relation_targets(nonce, relation_id) \
         SELECT ?1, r.id FROM file_relation_candidates r WHERE r.scope_id=?2 AND ( \
             EXISTS(SELECT 1 FROM privacy_purge_node_targets n WHERE n.nonce=?1 AND n.node_id=r.left_node_id) \
             OR EXISTS(SELECT 1 FROM privacy_purge_node_targets n WHERE n.nonce=?1 AND n.node_id=r.right_node_id))",
        params![nonce, scope_id],
    )?;
    transaction.execute(
        "INSERT OR IGNORE INTO privacy_purge_screenshot_group_targets(nonce, group_id) \
         SELECT DISTINCT ?1, g.id FROM screenshot_group_candidates g \
         JOIN screenshot_group_observations o ON o.group_id=g.id \
         JOIN screenshot_group_members m ON m.observation_id=o.id \
         JOIN privacy_purge_node_targets n ON n.nonce=?1 AND n.node_id=m.node_id \
         WHERE g.scope_id=?2",
        params![nonce, scope_id],
    )?;
    transaction.execute(
        "INSERT OR IGNORE INTO privacy_purge_cleanup_action_plan_targets(nonce, plan_id) \
         SELECT ?1, p.id FROM cleanup_action_plans p WHERE p.scope_id=?2 AND ( \
             EXISTS(SELECT 1 FROM privacy_purge_node_targets n WHERE n.nonce=?1 AND n.node_id=p.target_node_id) \
             OR EXISTS(SELECT 1 FROM privacy_purge_node_targets n WHERE n.nonce=?1 AND n.node_id=p.keeper_node_id) \
             OR (p.source_kind='screenshot_review_group' AND EXISTS( \
                 SELECT 1 FROM privacy_purge_screenshot_group_targets g \
                 WHERE g.nonce=?1 AND g.group_id=p.source_id)))",
        params![nonce, scope_id],
    )?;
    Ok(())
}

fn count_query(
    transaction: &Transaction<'_>,
    sql: &str,
    nonce: &[u8],
    scope_id: i64,
) -> Result<u64, DatabaseError> {
    let count = transaction.query_row(sql, params![nonce, scope_id], |row| row.get::<_, i64>(0))?;
    u64::try_from(count).map_err(|_| DatabaseError::InvalidCount)
}

fn privacy_purge_impact(
    transaction: &Transaction<'_>,
    nonce: &[u8],
    scope_id: i64,
) -> Result<ScopeExclusionImpactPreview, DatabaseError> {
    let direct_locations = count_query(
        transaction,
        "SELECT COUNT(*) FROM privacy_purge_location_targets WHERE nonce = ?1 AND direct_match = 1 AND ?2=?2",
        nonce,
        scope_id,
    )?;
    let locations = count_query(
        transaction,
        "SELECT COUNT(*) FROM privacy_purge_location_targets WHERE nonce = ?1 AND ?2=?2",
        nonce,
        scope_id,
    )?;
    let nodes = count_query(
        transaction,
        "SELECT COUNT(*) FROM privacy_purge_node_targets WHERE nonce = ?1 AND ?2=?2",
        nonce,
        scope_id,
    )?;
    let extraction_jobs = count_query(
        transaction,
        "SELECT COUNT(*) FROM extraction_jobs j JOIN privacy_purge_node_targets n ON n.nonce = ?1 AND n.node_id = j.node_id WHERE j.scope_id = ?2",
        nonce,
        scope_id,
    )?;
    let content_chunks = count_query(
        transaction,
        "SELECT COUNT(*) FROM content_chunks c JOIN privacy_purge_node_targets n ON n.nonce = ?1 AND n.node_id = c.node_id WHERE c.scope_id = ?2",
        nonce,
        scope_id,
    )?;
    let image_metadata = count_query(
        transaction,
        "SELECT COUNT(*) FROM image_metadata i JOIN privacy_purge_node_targets n ON n.nonce = ?1 AND n.node_id = i.node_id WHERE i.scope_id = ?2",
        nonce,
        scope_id,
    )?;
    let edges = count_query(
        transaction,
        "SELECT COUNT(*) FROM edges e WHERE e.scope_id = ?2 AND (EXISTS(SELECT 1 FROM privacy_purge_node_targets n WHERE n.nonce = ?1 AND n.node_id = e.source_node_id) OR EXISTS(SELECT 1 FROM privacy_purge_node_targets n WHERE n.nonce = ?1 AND n.node_id = e.target_node_id))",
        nonce,
        scope_id,
    )?;
    let projects = count_query(
        transaction,
        "SELECT COUNT(*) FROM privacy_purge_project_targets WHERE nonce=?1 AND ?2=?2",
        nonce,
        scope_id,
    )?;
    let relations = count_query(
        transaction,
        "SELECT COUNT(*) FROM privacy_purge_relation_targets WHERE nonce=?1 AND ?2=?2",
        nonce,
        scope_id,
    )?;
    let groups = count_query(
        transaction,
        "SELECT COUNT(*) FROM privacy_purge_screenshot_group_targets WHERE nonce=?1 AND ?2=?2",
        nonce,
        scope_id,
    )?;
    let actions = count_query(
        transaction,
        "SELECT COUNT(*) FROM privacy_purge_action_plan_targets WHERE nonce=?1 AND ?2=?2",
        nonce,
        scope_id,
    )?;
    let cleanup = count_query(
        transaction,
        "SELECT COUNT(*) FROM privacy_purge_cleanup_action_plan_targets WHERE nonce=?1 AND ?2=?2",
        nonce,
        scope_id,
    )?;
    let watches = count_query(
        transaction,
        "SELECT COUNT(*) FROM watch_events w WHERE w.scope_id = ?2 AND (w.status IN ('stabilizing','reconciling') OR EXISTS(SELECT 1 FROM scope_exclusions e WHERE e.scope_id = w.scope_id AND (w.path_key = e.path_key OR (e.kind = 'folder' AND length(w.path_key) > length(e.path_key) AND substr(w.path_key,1,length(e.path_key)) = e.path_key AND (substr(e.path_key,-1,1) = CASE WHEN (SELECT platform FROM authorized_scopes WHERE id=?2)='windows' THEN char(92) ELSE '/' END OR substr(w.path_key,length(e.path_key)+1,1) = CASE WHEN (SELECT platform FROM authorized_scopes WHERE id=?2)='windows' THEN char(92) ELSE '/' END)))))",
        nonce,
        scope_id,
    )?;
    let pending = count_query(
        transaction,
        "SELECT (SELECT COUNT(*) FROM scan_jobs WHERE scope_id = ?2 AND status IN ('running','interrupted')) + (SELECT COUNT(*) FROM extraction_jobs WHERE scope_id = ?2 AND status IN ('queued','running','interrupted')) + (SELECT COUNT(*) FROM watch_events WHERE scope_id = ?2 AND status IN ('stabilizing','reconciling'))",
        nonce,
        scope_id,
    )?;
    let scan_jobs = count_query(
        transaction,
        "SELECT COUNT(*) FROM scan_jobs WHERE scope_id = ?2 AND status IN ('running','interrupted')",
        nonce,
        scope_id,
    )?;
    let blocking_actions = privacy_purge_blocking_action_count(transaction, nonce, scope_id)?;
    Ok(ScopeExclusionImpactPreview {
        direct_location_count: direct_locations,
        conservative_location_count: locations,
        conservative_node_count: nodes,
        scan_job_count: scan_jobs,
        extraction_job_count: extraction_jobs,
        watch_event_count: watches,
        content_chunk_count: content_chunks,
        image_metadata_count: image_metadata,
        edge_count: edges,
        project_count: projects,
        relation_count: relations,
        screenshot_group_count: groups,
        action_plan_count: actions,
        cleanup_action_plan_count: cleanup,
        blocking_action_count: blocking_actions,
        pending_job_count: pending,
    })
}

fn scope_root_revocation_impact(
    transaction: &Transaction<'_>,
    nonce: &[u8],
    scope_id: i64,
) -> Result<ScopeExclusionImpactPreview, DatabaseError> {
    let mut impact = privacy_purge_impact(transaction, nonce, scope_id)?;
    impact.watch_event_count = count_query(
        transaction,
        "SELECT COUNT(*) FROM watch_events WHERE scope_id=?2 AND ?1=?1",
        nonce,
        scope_id,
    )?;
    impact.scan_job_count = count_query(
        transaction,
        "SELECT COUNT(*) FROM scan_jobs WHERE scope_id=?2 AND ?1=?1",
        nonce,
        scope_id,
    )?;
    Ok(impact)
}

fn ensure_privacy_purge_actions_are_safe(
    transaction: &Transaction<'_>,
    nonce: &[u8],
    scope_id: i64,
) -> Result<(), DatabaseError> {
    let blocked = privacy_purge_blocking_action_count(transaction, nonce, scope_id)?;
    if blocked != 0 {
        return Err(DatabaseError::ScopePrivacyPurgeBlocked);
    }
    Ok(())
}

/// Only a pristine sequence-one preview may be removed by ADR-033's bounded
/// privacy exception. Any later command/event, including a transition that
/// reduces back to Previewed, is an executable-action safety receipt and blocks
/// the purge. Missing or corrupt journal history also fails closed.
fn privacy_purge_blocking_action_count(
    transaction: &Transaction<'_>,
    nonce: &[u8],
    scope_id: i64,
) -> Result<u64, DatabaseError> {
    count_query(
        transaction,
        "SELECT COUNT(*) FROM privacy_purge_action_plan_targets t \
         WHERE t.nonce=?1 AND ?2=?2 AND NOT EXISTS( \
             SELECT 1 FROM action_journal_events e \
             WHERE e.plan_id=t.plan_id \
               AND e.sequence=(SELECT MAX(latest.sequence) FROM action_journal_events latest WHERE latest.plan_id=t.plan_id) \
               AND e.sequence=1 AND e.event_kind='preview_created')",
        nonce,
        scope_id,
    )
}

fn execute_counted(
    transaction: &Transaction<'_>,
    sql: &str,
    parameters: impl rusqlite::Params,
    total: &mut u64,
) -> Result<(), DatabaseError> {
    let changed = transaction.execute(sql, parameters)?;
    *total = total
        .checked_add(u64::try_from(changed).map_err(|_| DatabaseError::InvalidCount)?)
        .ok_or(DatabaseError::InvalidCount)?;
    Ok(())
}

fn execute_privacy_purge(
    transaction: &Transaction<'_>,
    nonce: &[u8],
    scope_id: i64,
    new_revision: i64,
) -> Result<u64, DatabaseError> {
    let mut total = 0_u64;

    // Only a pristine sequence-one preview reaches this point. Any later
    // executable-action safety record was rejected before the purge begins.
    execute_counted(
        transaction,
        "DELETE FROM action_executor_leases WHERE plan_id IN (SELECT plan_id FROM privacy_purge_action_plan_targets WHERE nonce=?1 AND ?2=?2)",
        params![nonce, scope_id],
        &mut total,
    )?;
    execute_counted(
        transaction,
        "DELETE FROM action_execution_bindings WHERE plan_id IN (SELECT plan_id FROM privacy_purge_action_plan_targets WHERE nonce=?1 AND ?2=?2)",
        params![nonce, scope_id],
        &mut total,
    )?;
    execute_counted(
        transaction,
        "DELETE FROM action_journal_events WHERE plan_id IN (SELECT plan_id FROM privacy_purge_action_plan_targets WHERE nonce=?1 AND ?2=?2)",
        params![nonce, scope_id],
        &mut total,
    )?;
    execute_counted(
        transaction,
        "DELETE FROM action_command_requests WHERE plan_id IN (SELECT plan_id FROM privacy_purge_action_plan_targets WHERE nonce=?1 AND ?2=?2)",
        params![nonce, scope_id],
        &mut total,
    )?;
    execute_counted(
        transaction,
        "DELETE FROM action_plan_events WHERE plan_id IN (SELECT plan_id FROM privacy_purge_action_plan_targets WHERE nonce=?1 AND ?2=?2)",
        params![nonce, scope_id],
        &mut total,
    )?;
    execute_counted(
        transaction,
        "DELETE FROM action_plans WHERE id IN (SELECT plan_id FROM privacy_purge_action_plan_targets WHERE nonce=?1 AND ?2=?2)",
        params![nonce, scope_id],
        &mut total,
    )?;

    execute_counted(
        transaction,
        "DELETE FROM cleanup_action_journal_events WHERE plan_id IN (SELECT plan_id FROM privacy_purge_cleanup_action_plan_targets WHERE nonce=?1 AND ?2=?2)",
        params![nonce, scope_id],
        &mut total,
    )?;
    execute_counted(
        transaction,
        "DELETE FROM cleanup_action_plans WHERE id IN (SELECT plan_id FROM privacy_purge_cleanup_action_plan_targets WHERE nonce=?1 AND ?2=?2)",
        params![nonce, scope_id],
        &mut total,
    )?;

    execute_counted(
        transaction,
        "DELETE FROM screenshot_group_members WHERE observation_id IN (SELECT o.id FROM screenshot_group_observations o JOIN privacy_purge_screenshot_group_targets t ON t.nonce=?1 AND t.group_id=o.group_id WHERE ?2=?2)",
        params![nonce, scope_id],
        &mut total,
    )?;
    execute_counted(
        transaction,
        "DELETE FROM screenshot_group_observations WHERE group_id IN (SELECT group_id FROM privacy_purge_screenshot_group_targets WHERE nonce=?1 AND ?2=?2)",
        params![nonce, scope_id],
        &mut total,
    )?;
    execute_counted(
        transaction,
        "DELETE FROM screenshot_group_candidates WHERE id IN (SELECT group_id FROM privacy_purge_screenshot_group_targets WHERE nonce=?1 AND ?2=?2)",
        params![nonce, scope_id],
        &mut total,
    )?;

    execute_counted(
        transaction,
        "DELETE FROM project_suggestion_signals WHERE suggestion_id IN (SELECT s.id FROM project_suggestions s JOIN privacy_purge_project_targets t ON t.nonce=?1 AND t.project_id=s.project_id WHERE ?2=?2)",
        params![nonce, scope_id],
        &mut total,
    )?;
    execute_counted(
        transaction,
        "DELETE FROM project_feedback_events WHERE project_id IN (SELECT project_id FROM privacy_purge_project_targets WHERE nonce=?1 AND ?2=?2)",
        params![nonce, scope_id],
        &mut total,
    )?;
    execute_counted(
        transaction,
        "DELETE FROM project_suggestions WHERE project_id IN (SELECT project_id FROM privacy_purge_project_targets WHERE nonce=?1 AND ?2=?2)",
        params![nonce, scope_id],
        &mut total,
    )?;
    execute_counted(
        transaction,
        "DELETE FROM projects WHERE id IN (SELECT project_id FROM privacy_purge_project_targets WHERE nonce=?1 AND ?2=?2)",
        params![nonce, scope_id],
        &mut total,
    )?;

    execute_counted(
        transaction,
        "DELETE FROM file_version_feedback_events WHERE relation_id IN (SELECT relation_id FROM privacy_purge_relation_targets WHERE nonce=?1 AND ?2=?2)",
        params![nonce, scope_id],
        &mut total,
    )?;
    execute_counted(
        transaction,
        "DELETE FROM file_relation_feedback_events WHERE relation_id IN (SELECT relation_id FROM privacy_purge_relation_targets WHERE nonce=?1 AND ?2=?2)",
        params![nonce, scope_id],
        &mut total,
    )?;
    execute_counted(
        transaction,
        "DELETE FROM file_version_observations WHERE relation_id IN (SELECT relation_id FROM privacy_purge_relation_targets WHERE nonce=?1 AND ?2=?2)",
        params![nonce, scope_id],
        &mut total,
    )?;
    execute_counted(
        transaction,
        "DELETE FROM file_relation_observations WHERE relation_id IN (SELECT relation_id FROM privacy_purge_relation_targets WHERE nonce=?1 AND ?2=?2)",
        params![nonce, scope_id],
        &mut total,
    )?;
    execute_counted(
        transaction,
        "DELETE FROM file_relation_candidates WHERE id IN (SELECT relation_id FROM privacy_purge_relation_targets WHERE nonce=?1 AND ?2=?2)",
        params![nonce, scope_id],
        &mut total,
    )?;

    execute_counted(
        transaction,
        "UPDATE extraction_jobs SET status='cancelled', cancel_requested=1, runner_token=NULL, lease_expires_at_unix_ms=NULL, finished_at_unix_ms=COALESCE(finished_at_unix_ms,updated_at_unix_ms) WHERE scope_id=?2 AND policy_revision < ?3 AND status IN ('queued','running','interrupted') AND NOT EXISTS(SELECT 1 FROM privacy_purge_node_targets n WHERE n.nonce=?1 AND n.node_id=extraction_jobs.node_id)",
        params![nonce, scope_id, new_revision],
        &mut total,
    )?;
    execute_counted(
        transaction,
        "DELETE FROM content_chunks WHERE scope_id=?2 AND EXISTS(SELECT 1 FROM privacy_purge_node_targets n WHERE n.nonce=?1 AND n.node_id=content_chunks.node_id)",
        params![nonce, scope_id],
        &mut total,
    )?;
    execute_counted(
        transaction,
        "DELETE FROM image_metadata WHERE scope_id=?2 AND EXISTS(SELECT 1 FROM privacy_purge_node_targets n WHERE n.nonce=?1 AND n.node_id=image_metadata.node_id)",
        params![nonce, scope_id],
        &mut total,
    )?;
    execute_counted(
        transaction,
        "DELETE FROM extraction_jobs WHERE scope_id=?2 AND EXISTS(SELECT 1 FROM privacy_purge_node_targets n WHERE n.nonce=?1 AND n.node_id=extraction_jobs.node_id)",
        params![nonce, scope_id],
        &mut total,
    )?;

    // Every running unit was bound to the previous revision. It cannot publish.
    execute_counted(
        transaction,
        "UPDATE scan_jobs SET status='failed', control_state='ready', pause_requested=0, runner_token=NULL, lease_expires_at_unix_ms=NULL, finished_at_unix_ms=COALESCE(finished_at_unix_ms,updated_at_unix_ms) WHERE scope_id=?1 AND policy_revision < ?2 AND status IN ('running','interrupted')",
        params![scope_id, new_revision],
        &mut total,
    )?;
    execute_counted(
        transaction,
        "DELETE FROM scan_queue WHERE scan_id IN (SELECT id FROM scan_jobs WHERE scope_id=?1 AND policy_revision < ?2)",
        params![scope_id, new_revision],
        &mut total,
    )?;
    execute_counted(
        transaction,
        "DELETE FROM scan_staged_observations WHERE scan_id IN (SELECT id FROM scan_jobs WHERE scope_id=?1 AND policy_revision < ?2)",
        params![scope_id, new_revision],
        &mut total,
    )?;
    execute_counted(
        transaction,
        "DELETE FROM scan_staged_issues WHERE scan_id IN (SELECT id FROM scan_jobs WHERE scope_id=?1 AND policy_revision < ?2)",
        params![scope_id, new_revision],
        &mut total,
    )?;
    execute_counted(
        transaction,
        "DELETE FROM scan_issues WHERE scan_id IN (SELECT id FROM scan_jobs WHERE scope_id=?1) AND path_key IS NOT NULL AND EXISTS(SELECT 1 FROM scope_exclusions e WHERE e.scope_id=?1 AND (scan_issues.path_key=e.path_key OR (e.kind='folder' AND length(scan_issues.path_key)>length(e.path_key) AND substr(scan_issues.path_key,1,length(e.path_key))=e.path_key AND (substr(e.path_key,-1,1)=CASE WHEN (SELECT platform FROM authorized_scopes WHERE id=?1)='windows' THEN char(92) ELSE '/' END OR substr(scan_issues.path_key,length(e.path_key)+1,1)=CASE WHEN (SELECT platform FROM authorized_scopes WHERE id=?1)='windows' THEN char(92) ELSE '/' END))))",
        [scope_id],
        &mut total,
    )?;

    execute_counted(
        transaction,
        "DELETE FROM watch_events WHERE scope_id=?2 AND (status IN ('stabilizing','reconciling') OR EXISTS(SELECT 1 FROM scope_exclusions e WHERE e.scope_id=?2 AND (watch_events.path_key=e.path_key OR (e.kind='folder' AND length(watch_events.path_key)>length(e.path_key) AND substr(watch_events.path_key,1,length(e.path_key))=e.path_key AND (substr(e.path_key,-1,1)=CASE WHEN (SELECT platform FROM authorized_scopes WHERE id=?2)='windows' THEN char(92) ELSE '/' END OR substr(watch_events.path_key,length(e.path_key)+1,1)=CASE WHEN (SELECT platform FROM authorized_scopes WHERE id=?2)='windows' THEN char(92) ELSE '/' END)))))",
        params![nonce, scope_id],
        &mut total,
    )?;
    execute_counted(
        transaction,
        "DELETE FROM edges WHERE scope_id=?2 AND (EXISTS(SELECT 1 FROM privacy_purge_node_targets n WHERE n.nonce=?1 AND n.node_id=edges.source_node_id) OR EXISTS(SELECT 1 FROM privacy_purge_node_targets n WHERE n.nonce=?1 AND n.node_id=edges.target_node_id))",
        params![nonce, scope_id],
        &mut total,
    )?;
    execute_counted(
        transaction,
        "DELETE FROM locations WHERE scope_id=?2 AND EXISTS(SELECT 1 FROM privacy_purge_location_targets t WHERE t.nonce=?1 AND t.location_id=locations.id)",
        params![nonce, scope_id],
        &mut total,
    )?;

    // Node identity is global. Cross-scope hardlinks retain their source node;
    // only identities with no remaining location or derived reference are freed.
    execute_counted(
        transaction,
        "DELETE FROM files WHERE EXISTS(SELECT 1 FROM privacy_purge_node_targets n WHERE n.nonce=?1 AND n.node_id=files.node_id) AND NOT EXISTS(SELECT 1 FROM locations l WHERE l.node_id=files.node_id)",
        [nonce],
        &mut total,
    )?;
    execute_counted(
        transaction,
        "DELETE FROM folders WHERE EXISTS(SELECT 1 FROM privacy_purge_node_targets n WHERE n.nonce=?1 AND n.node_id=folders.node_id) AND NOT EXISTS(SELECT 1 FROM locations l WHERE l.node_id=folders.node_id)",
        [nonce],
        &mut total,
    )?;
    execute_counted(
        transaction,
        "DELETE FROM nodes WHERE EXISTS(SELECT 1 FROM privacy_purge_node_targets n WHERE n.nonce=?1 AND n.node_id=nodes.id) AND NOT EXISTS(SELECT 1 FROM locations l WHERE l.node_id=nodes.id) AND NOT EXISTS(SELECT 1 FROM edges e WHERE e.source_node_id=nodes.id OR e.target_node_id=nodes.id)",
        [nonce],
        &mut total,
    )?;
    Ok(total)
}

fn execute_scope_root_revocation_purge(
    transaction: &Transaction<'_>,
    nonce: &[u8],
    scope_id: i64,
    new_revision: i64,
) -> Result<u64, DatabaseError> {
    let mut total = execute_privacy_purge(transaction, nonce, scope_id, new_revision)?;
    // Watch rows and scan issues may carry paths even when they were already
    // terminal or did not match a user exclusion. Root withdrawal removes all
    // of them. Completed scan rows retain only path-free operational counters
    // but are permanently failed below as the durable authorization-epoch
    // boundary: reauthorization can never reuse an old completed scan, while
    // an add-only exclusion may keep the atomically pruned manifest usable
    // without starting an automatic scan.
    execute_counted(
        transaction,
        "DELETE FROM watch_events WHERE scope_id=?1",
        [scope_id],
        &mut total,
    )?;
    execute_counted(
        transaction,
        "DELETE FROM scan_issues WHERE scan_id IN (SELECT id FROM scan_jobs WHERE scope_id=?1)",
        [scope_id],
        &mut total,
    )?;
    execute_counted(
        transaction,
        "DELETE FROM scan_queue WHERE scan_id IN (SELECT id FROM scan_jobs WHERE scope_id=?1)",
        [scope_id],
        &mut total,
    )?;
    execute_counted(
        transaction,
        "DELETE FROM scan_staged_observations WHERE scan_id IN (SELECT id FROM scan_jobs WHERE scope_id=?1)",
        [scope_id],
        &mut total,
    )?;
    execute_counted(
        transaction,
        "DELETE FROM scan_staged_issues WHERE scan_id IN (SELECT id FROM scan_jobs WHERE scope_id=?1)",
        [scope_id],
        &mut total,
    )?;
    execute_counted(
        transaction,
        "UPDATE scan_jobs SET status='failed', control_state='ready', pause_requested=0, \
            runner_token=NULL, lease_expires_at_unix_ms=NULL \
         WHERE scope_id=?1 AND status='completed'",
        [scope_id],
        &mut total,
    )?;
    Ok(total)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NodeKind {
    File,
    Folder,
}

impl NodeKind {
    fn as_str(self) -> &'static str {
        match self {
            Self::File => "file",
            Self::Folder => "folder",
        }
    }

    fn from_db(value: &str) -> Result<Self, DatabaseError> {
        match value {
            "file" => Ok(Self::File),
            "folder" => Ok(Self::Folder),
            _ => Err(DatabaseError::InvalidStoredValue),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Observation {
    pub kind: NodeKind,
    pub identity_kind: String,
    pub identity_key: Vec<u8>,
    pub parent_identity_key: Option<Vec<u8>>,
    pub path_raw: Vec<u8>,
    pub path_key: String,
    pub display_path: String,
    pub size_bytes: u64,
    pub modified_unix_ns: Option<i64>,
    pub link_count: Option<u64>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ScanIssue {
    pub code: &'static str,
    pub path_key: Option<String>,
    pub detail_code: Option<&'static str>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ScopeRecord {
    pub id: i64,
    pub path_raw: Vec<u8>,
    pub path_key: String,
    pub display_path: String,
    pub platform: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ScopePolicyRevision {
    pub scope_id: i64,
    pub revision: i64,
}

/// A revision-only snapshot for explicitly authorized core scan/extraction work.
/// This never proves that a durable OS access grant exists or is active.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ScopeRevisionBinding {
    pub scope_id: i64,
    pub revision: i64,
}

/// An active-grant policy snapshot. Path-bearing packaged/query/action work must
/// present this binding; the database rechecks both revision and durable active
/// grant in the same transaction that commits the mutation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ScopePolicyBinding {
    pub scope_id: i64,
    pub revision: i64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ScopeExclusionKind {
    File,
    Folder,
}

impl ScopeExclusionKind {
    fn as_str(self) -> &'static str {
        match self {
            Self::File => "file",
            Self::Folder => "folder",
        }
    }

    fn from_db(value: &str) -> Result<Self, DatabaseError> {
        match value {
            "file" => Ok(Self::File),
            "folder" => Ok(Self::Folder),
            _ => Err(DatabaseError::InvalidStoredValue),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ScopeExclusionRecord {
    pub id: i64,
    pub scope_id: i64,
    pub kind: ScopeExclusionKind,
    pub path_raw: Vec<u8>,
    pub path_key: String,
    pub display_path: String,
    pub policy_revision: i64,
    pub created_at_unix_ms: i64,
}

#[derive(Clone, Copy, Eq, PartialEq)]
pub struct ScopeExclusionWrite<'a> {
    pub kind: ScopeExclusionKind,
    pub path_raw: &'a [u8],
    /// Canonical `deskgraph_identity::comparison_key`, never a display path.
    pub path_key: &'a str,
    pub display_path: &'a str,
    /// Canonical stable filesystem identity. `path_fallback` is never accepted.
    pub identity_kind: &'a str,
    pub identity_key: &'a [u8],
}

impl fmt::Debug for ScopeExclusionWrite<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ScopeExclusionWrite")
            .field("kind", &self.kind)
            .field("path_raw_len", &self.path_raw.len())
            .field("path_key_len", &self.path_key.len())
            .field("display_path_len", &self.display_path.len())
            .field("identity_kind_len", &self.identity_kind.len())
            .field("identity_key", &"<redacted>")
            .finish()
    }
}

/// Canonical component-boundary matcher for one immutable policy snapshot.
/// It never performs a raw string-prefix authorization decision.
#[derive(Clone, Eq, PartialEq)]
pub struct ScopeExclusionMatcher {
    pub scope_id: i64,
    pub revision: i64,
    exclusions: Vec<(String, ScopeExclusionKind)>,
    excluded_identities: Vec<(String, Vec<u8>)>,
    separator: char,
}

impl fmt::Debug for ScopeExclusionMatcher {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ScopeExclusionMatcher")
            .field("scope_id", &self.scope_id)
            .field("revision", &self.revision)
            .field("path_exclusion_count", &self.exclusions.len())
            .field("identity_exclusion_count", &self.excluded_identities.len())
            .finish()
    }
}

impl ScopeExclusionMatcher {
    pub fn is_excluded_path_key(&self, canonical_path_key: &str) -> bool {
        self.exclusions.iter().any(|(excluded, kind)| {
            canonical_path_key == excluded
                || (*kind == ScopeExclusionKind::Folder
                    && canonical_descendant_of(canonical_path_key, excluded, self.separator))
        })
    }

    pub fn is_excluded_identity(&self, identity_kind: &str, identity_key: &[u8]) -> bool {
        if identity_kind.is_empty() || identity_key.is_empty() {
            return !self.excluded_identities.is_empty();
        }
        self.excluded_identities
            .iter()
            .any(|(excluded_kind, excluded_key)| {
                identity_kind == excluded_kind && identity_key == excluded_key
            })
    }

    pub fn requires_stable_identity(&self) -> bool {
        !self.excluded_identities.is_empty()
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct ScopeExclusionImpactPreview {
    pub direct_location_count: u64,
    pub conservative_location_count: u64,
    pub conservative_node_count: u64,
    pub scan_job_count: u64,
    pub extraction_job_count: u64,
    pub watch_event_count: u64,
    pub content_chunk_count: u64,
    pub image_metadata_count: u64,
    pub edge_count: u64,
    pub project_count: u64,
    pub relation_count: u64,
    pub screenshot_group_count: u64,
    pub action_plan_count: u64,
    pub cleanup_action_plan_count: u64,
    pub blocking_action_count: u64,
    pub pending_job_count: u64,
}

impl ScopeExclusionImpactPreview {
    pub fn total_purge_rows(self) -> Result<u64, DatabaseError> {
        [
            self.conservative_location_count,
            self.scan_job_count,
            self.extraction_job_count,
            self.watch_event_count,
            self.content_chunk_count,
            self.image_metadata_count,
            self.edge_count,
            self.project_count,
            self.relation_count,
            self.screenshot_group_count,
            self.action_plan_count,
            self.cleanup_action_plan_count,
        ]
        .into_iter()
        .try_fold(0_u64, |sum, value| sum.checked_add(value))
        .ok_or(DatabaseError::InvalidCount)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PrivacyPurgeReceipt {
    pub id: i64,
    pub scope_id: i64,
    pub from_revision: i64,
    pub to_revision: i64,
    pub exclusions_added: u64,
    pub affected_location_count: u64,
    pub affected_node_count: u64,
    pub purged_row_count: u64,
    pub created_at_unix_ms: i64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ScopeExclusionApplyResult {
    pub policy: ScopePolicyRevision,
    pub receipt: PrivacyPurgeReceipt,
    pub purged: ScopeExclusionImpactPreview,
    pub exclusions: Vec<ScopeExclusionRecord>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ScopeRootRevocationPreview {
    pub scope_id: i64,
    pub base_policy_revision: i64,
    pub impact: ScopeExclusionImpactPreview,
    pub exclusion_count: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ScopeRootRevocationReceipt {
    pub id: i64,
    pub scope_id: i64,
    pub from_revision: i64,
    pub to_revision: i64,
    pub affected_location_count: u64,
    pub affected_node_count: u64,
    pub exclusions_removed: u64,
    pub purged_row_count: u64,
    pub created_at_unix_ms: i64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ScopeRootRevocationApplyResult {
    pub policy: ScopePolicyRevision,
    pub receipt: ScopeRootRevocationReceipt,
    pub purged: ScopeExclusionImpactPreview,
}

/// The durable lifecycle state of a platform-owned scope access grant.
///
/// A scope with no row in `scope_access_grants` is intentionally interpreted
/// as [`Self::NeedsReauthorization`], never as active.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ScopeAccessGrantState {
    Active,
    NeedsReauthorization,
    Revoked,
}

impl ScopeAccessGrantState {
    fn as_str(self) -> &'static str {
        match self {
            Self::Active => "active",
            Self::NeedsReauthorization => "needs_reauthorization",
            Self::Revoked => "revoked",
        }
    }

    fn from_db(value: &str) -> Result<Self, DatabaseError> {
        match value {
            "active" => Ok(Self::Active),
            "needs_reauthorization" => Ok(Self::NeedsReauthorization),
            "revoked" => Ok(Self::Revoked),
            _ => Err(DatabaseError::InvalidStoredValue),
        }
    }
}

/// Backend-only record for restoring an OS-granted scope capability.
///
/// `opaque_grant` must not be mapped into `AuthorizedScope`, domain objects,
/// IPC responses, diagnostics, or logs. Only platform adapters may interpret
/// the bytes.
#[derive(Clone, Eq, PartialEq)]
pub struct ScopeAccessGrant {
    pub scope_id: i64,
    pub platform: String,
    pub opaque_grant: Vec<u8>,
    pub state: ScopeAccessGrantState,
    pub updated_at_unix_ms: i64,
}

/// Input for atomically storing a scope and its platform-owned access grant.
/// The state is explicit so a caller cannot accidentally relabel a restored or
/// revoked capability as active.
#[derive(Clone, Copy, Eq, PartialEq)]
pub struct ScopeAccessGrantWrite<'a> {
    pub scope_platform: &'a str,
    pub grant_platform: &'a str,
    pub opaque_grant: &'a [u8],
    pub state: ScopeAccessGrantState,
}

/// One root in an atomic coverage-set authorization transaction.
///
/// Paths and opaque grants remain Rust-backend inputs. Callers must validate
/// canonical scope policy before constructing this write.
#[derive(Clone, Copy, Eq, PartialEq)]
pub struct CoverageRootAccessGrantWrite<'a> {
    pub path_raw: &'a [u8],
    pub path_key: &'a str,
    pub display_path: &'a str,
    pub grant: ScopeAccessGrantWrite<'a>,
}

impl fmt::Debug for CoverageRootAccessGrantWrite<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CoverageRootAccessGrantWrite")
            .field("path_raw_len", &self.path_raw.len())
            .field("path_key_len", &self.path_key.len())
            .field("display_path_len", &self.display_path.len())
            .field("grant", &self.grant)
            .finish_non_exhaustive()
    }
}

impl fmt::Debug for ScopeAccessGrantWrite<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ScopeAccessGrantWrite")
            .field("scope_platform", &self.scope_platform)
            .field("grant_platform", &self.grant_platform)
            .field("opaque_grant_len", &self.opaque_grant.len())
            .field("state", &self.state)
            .finish()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct QueuedPath {
    pub path_raw: Vec<u8>,
    pub path_key: String,
    pub parent_identity_key: Option<Vec<u8>>,
    pub is_root: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct QueueEntry {
    pub id: i64,
    pub path_raw: Vec<u8>,
    pub path_key: String,
    pub parent_identity_key: Option<Vec<u8>>,
    pub is_root: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WatchSnapshotKind {
    Missing,
    File,
    Folder,
}

/// Controls whether a durable watch event may use the narrow file-metadata
/// publish path or must remain on the existing full-scope reconciliation path.
///
/// This is deliberately database-internal state. Ordinary status payloads do
/// not expose paths or reconciliation strategy.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WatchReconciliationKind {
    FileDelta,
    FullScope,
}

impl WatchReconciliationKind {
    fn as_str(self) -> &'static str {
        match self {
            Self::FileDelta => "file_delta",
            Self::FullScope => "full_scope",
        }
    }

    fn from_db(value: &str) -> Result<Self, DatabaseError> {
        match value {
            "file_delta" => Ok(Self::FileDelta),
            "full_scope" => Ok(Self::FullScope),
            _ => Err(DatabaseError::InvalidStoredValue),
        }
    }
}

impl WatchSnapshotKind {
    fn as_str(self) -> &'static str {
        match self {
            Self::Missing => "missing",
            Self::File => "file",
            Self::Folder => "folder",
        }
    }

    fn from_db(value: &str) -> Result<Self, DatabaseError> {
        match value {
            "missing" => Ok(Self::Missing),
            "file" => Ok(Self::File),
            "folder" => Ok(Self::Folder),
            _ => Err(DatabaseError::InvalidStoredValue),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WatchSnapshot {
    pub kind: WatchSnapshotKind,
    pub size_bytes: Option<u64>,
    pub modified_unix_ns: Option<i64>,
    pub identity_key: Option<Vec<u8>>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WatchEventRecord {
    pub progress: WatchEventProgress,
    pub reconciliation_kind: WatchReconciliationKind,
    pub path_raw: Vec<u8>,
    pub path_key: String,
    pub snapshot: WatchSnapshot,
}

#[derive(Clone, Copy, Debug)]
pub struct WatchObservationWrite<'a> {
    pub scope_id: i64,
    pub path_raw: &'a [u8],
    pub path_key: &'a str,
    pub snapshot: &'a WatchSnapshot,
    pub stable_after_unix_ms: i64,
    pub ignored_reason: Option<WatchEventReason>,
    pub reconciliation_kind: WatchReconciliationKind,
    pub observed_at_unix_ms: i64,
}

/// Immutable binding to the exact current manifest row that a narrow
/// file-delta publish may update. It is intentionally unavailable for files
/// with multiple present locations, hard links, missing parent topology, or a
/// non-file watch snapshot.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WatchFileDeltaBinding {
    pub event_id: i64,
    pub scope_id: i64,
    pub path_raw: Vec<u8>,
    pub path_key: String,
    pub stable_after_unix_ms: i64,
    pub snapshot: WatchSnapshot,
    pub node_id: i64,
    pub location_id: i64,
    pub root_location_id: i64,
    pub root_node_id: i64,
    pub root_identity_key: Vec<u8>,
    pub parent_location_id: i64,
    pub parent_node_id: i64,
    pub parent_path_raw: Vec<u8>,
    pub parent_path_key: String,
    pub parent_identity_key: Vec<u8>,
    pub identity_kind: String,
    pub identity_key: Vec<u8>,
    pub old_size_bytes: u64,
    pub old_modified_unix_ns: Option<i64>,
}

/// Candidate metadata for a bound file-delta publish. This type cannot carry
/// a location change, a rename, a folder, or a second hard-link location.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WatchFileDeltaWrite {
    pub snapshot: WatchSnapshot,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExtractableFile {
    pub scope_id: i64,
    pub node_id: i64,
    pub location_id: i64,
    pub path_raw: Vec<u8>,
    pub path_key: String,
    pub identity_kind: String,
    pub identity_key: Vec<u8>,
    pub size_bytes: u64,
    pub modified_unix_ns: Option<i64>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ActionSourceRecord {
    pub scope_id: i64,
    pub node_id: i64,
    pub location_id: i64,
    pub path_raw: Vec<u8>,
    pub path_key: String,
    pub display_path: String,
    pub identity_kind: String,
    pub identity_key: Vec<u8>,
    pub size_bytes: u64,
    pub modified_unix_ns: Option<i64>,
}

/// The only database-derived topology snapshot that can be used to create an
/// executable rename preview. It intentionally requires a completed scan, a
/// single present source location, and a single active parent edge, so the
/// transaction crate never infers a folder identity from a path.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ActionExecutionSourceRecord {
    pub source: ActionSourceRecord,
    pub scope_root_node_id: i64,
    pub scope_root_identity_kind: String,
    pub scope_root_identity_key: Vec<u8>,
    pub parent_node_id: i64,
    pub parent_identity_kind: String,
    pub parent_identity_key: Vec<u8>,
}

/// Trusted internal execution detail for the transaction engine. This is not
/// a UI/read-model payload: it carries canonical raw paths only after an
/// immutable binding has been found and the closed journal has decoded.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ActionExecutionPlan {
    pub plan_id: i64,
    pub scope_id: i64,
    pub node_id: i64,
    pub source_location_id: i64,
    pub source_path_raw: Vec<u8>,
    pub source_path_key: String,
    pub destination_path_raw: Vec<u8>,
    pub destination_path_key: String,
    pub source_identity_kind: String,
    pub source_identity_key: Vec<u8>,
    pub source_size_bytes: u64,
    pub source_modified_unix_ns: Option<i64>,
    pub execution_strategy: ActionExecutionStrategy,
    pub binding: ActionExecutionBinding,
}

#[derive(Clone, Copy, Debug)]
pub struct ActionPlanWrite<'a> {
    pub scope_id: i64,
    pub node_id: i64,
    pub source_location_id: i64,
    pub source_path_raw: &'a [u8],
    pub source_path_key: &'a str,
    pub source_display_path: &'a str,
    pub destination_path_raw: &'a [u8],
    pub destination_path_key: &'a str,
    pub destination_display_path: &'a str,
    pub source_identity_kind: &'a str,
    pub source_identity_key: &'a [u8],
    pub source_size_bytes: u64,
    pub source_modified_unix_ns: Option<i64>,
    /// Full SHA-256 of exactly the source bytes read through the open, verified
    /// source handle by the transaction crate. The database never opens files.
    pub source_sha256: &'a [u8],
    pub source_hash_bytes: u64,
    /// Strong root and parent identities observed by the transaction crate at
    /// preview time. The database compares them to the current manifest in the
    /// same transaction before sealing the immutable binding.
    pub scope_root_identity_kind: &'a str,
    pub scope_root_identity_key: &'a [u8],
    pub parent_identity_kind: &'a str,
    pub parent_identity_key: &'a [u8],
    pub execution_strategy: ActionExecutionStrategy,
}

/// Exact user selection from one already-refreshed Smart Cleanup source.
/// This is not confirmation and cannot authorize a filesystem action.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CleanupActionSelection {
    pub scope_id: i64,
    pub source_kind: SmartCleanupSourceKind,
    pub source_id: i64,
    pub source_observation_id: i64,
    pub keeper_node_id: Option<i64>,
    pub target_node_id: i64,
}

/// Trusted internal write assembled after the transaction core hashes an open,
/// identity-verified target. Paths are intentionally not duplicated into the
/// cleanup preview family.
#[derive(Clone, Copy, Debug)]
pub struct CleanupActionPlanWrite<'a> {
    pub selection: CleanupActionSelection,
    pub keeper: Option<CleanupKeeperBindingWrite<'a>>,
    pub target_location_id: i64,
    pub target_identity_kind: &'a str,
    pub target_identity_key: &'a [u8],
    pub target_size_bytes: u64,
    pub target_modified_unix_ns: Option<i64>,
    pub target_sha256: &'a [u8],
    pub target_hash_bytes: u64,
    pub scope_root_node_id: i64,
    pub scope_root_identity_kind: &'a str,
    pub scope_root_identity_key: &'a [u8],
    pub parent_node_id: i64,
    pub parent_identity_kind: &'a str,
    pub parent_identity_key: &'a [u8],
}

#[derive(Clone, Copy, Debug)]
pub struct CleanupKeeperBindingWrite<'a> {
    pub location_id: i64,
    pub identity_kind: &'a str,
    pub identity_key: &'a [u8],
    pub size_bytes: u64,
    pub modified_unix_ns: Option<i64>,
    pub sha256: &'a [u8],
    pub hash_bytes: u64,
    pub scope_root_node_id: i64,
    pub scope_root_identity_kind: &'a str,
    pub scope_root_identity_key: &'a [u8],
    pub parent_node_id: i64,
    pub parent_identity_kind: &'a str,
    pub parent_identity_key: &'a [u8],
}

/// A caller-supplied, bounded idempotency key for an explicit user command.
/// It is never a path and is persisted separately from immutable plan data.
#[derive(Clone, Copy, Debug)]
pub struct ActionCommandWrite<'a> {
    pub plan_id: i64,
    pub request_id: &'a str,
    pub kind: ActionCommandKind,
    pub expected_sequence: u64,
}

/// An internal journal transition belongs to the immutable user command that
/// first obtained the plan. It cannot create a filesystem action on its own.
#[derive(Clone, Debug)]
pub struct ActionJournalAppend<'a> {
    pub plan_id: i64,
    pub command_request_id: i64,
    pub expected_sequence: u64,
    pub expected_state: ActionPlanState,
    pub kind: ActionJournalEventKind,
    pub executor_lease_owner_token: &'a str,
}

/// A short-lived operational lease for exactly one executor/recovery process.
/// It is not audit history and expires so a crashed executor cannot block
/// recovery indefinitely.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ActionExecutorLease {
    pub plan_id: i64,
    pub owner_token: String,
    pub expires_at_unix_ms: i64,
}

/// Internal-only recovery work. It intentionally has no source or destination
/// pathname; the transaction engine must reload the immutable plan and perform
/// scope/open-handle validation before it can inspect the filesystem.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct IncompleteActionRecovery {
    pub plan_id: i64,
    pub command_request_id: i64,
    pub state: ActionPlanState,
    pub journal_sequence: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FolderProfileFacts {
    pub scope_id: i64,
    pub folder_node_id: i64,
    pub folder_location_id: i64,
    pub display_path: String,
    pub direct_file_count: u64,
    pub direct_folder_count: u64,
    pub descendant_file_count: u64,
    pub descendant_folder_count: u64,
    pub total_file_bytes: u64,
    pub latest_modified_unix_ns: Option<i64>,
    pub file_categories: Vec<FolderCategoryCount>,
    pub project_markers: Vec<ProjectSignalKind>,
    pub observed_at_unix_ms: i64,
    pub bounded_entry_limit: u64,
}

/// Current manifest roots that have at least one direct strong project marker.
/// The IDs are path-free; callers must explicitly request one candidate before
/// resolving its current display path.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProjectDiscoveryRoots {
    pub root_folder_node_ids: Vec<i64>,
    pub evaluation_complete: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SmartCleanupSourceReference {
    pub kind: SmartCleanupSourceKind,
    pub source_id: i64,
    pub state: FileRelationCandidateState,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct ScreenshotGroupSourceRecord {
    pub scope_id: i64,
    pub node_id: i64,
    pub location_id: i64,
    pub image_metadata_id: i64,
    pub ocr_extraction_job_id: i64,
    pub size_bytes: u64,
    pub modified_unix_ns: i64,
    pub format: ImageFormat,
    pub pixel_width: u32,
    pub pixel_height: u32,
    pub ocr_chunk_count: u32,
    pub ocr_provider_id: String,
    pub ocr_provider_version: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct ScreenshotGroupObservationRecord {
    id: i64,
    evidence_key: String,
    member_count: i64,
    confidence_basis_points: i64,
    observed_at_unix_ms: i64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ContentChunkProvenanceWrite {
    ByteRange {
        start: u64,
        end: u64,
    },
    PdfPage {
        page_number: u32,
        fragment_index: u32,
    },
    DocxParagraph {
        paragraph_number: u32,
        fragment_index: u32,
    },
    PptxSlide {
        slide_number: u32,
        fragment_index: u32,
    },
    XlsxCell {
        sheet_number: u32,
        cell_reference: String,
        fragment_index: u32,
    },
    OcrObservation {
        observation_number: u32,
        fragment_index: u32,
        bbox_x_ppm: u32,
        bbox_y_ppm: u32,
        bbox_width_ppm: u32,
        bbox_height_ppm: u32,
        confidence_basis_points: Option<u16>,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ContentChunkWrite {
    pub ordinal: u32,
    pub text: String,
    pub provenance: ContentChunkProvenanceWrite,
    pub trust_class: &'static str,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ImageMetadataWrite {
    pub format: ImageFormat,
    pub pixel_width: u32,
    pub pixel_height: u32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LexicalCandidateSource {
    MetadataPath,
    ExtractedText,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LexicalSearchSource {
    All,
    MetadataPath,
    ExtractedText,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LexicalSearchFilters<'a> {
    pub scope_id: Option<i64>,
    pub folder_node_id: Option<i64>,
    pub source: LexicalSearchSource,
    pub extension: Option<&'a str>,
    pub modified_since_unix_ns: Option<i64>,
    pub modified_before_unix_ns: Option<i64>,
}

#[derive(Clone, PartialEq)]
pub struct LexicalSearchCandidate {
    pub source: LexicalCandidateSource,
    pub scope_id: i64,
    pub policy_revision: i64,
    pub node_id: i64,
    pub location_id: i64,
    pub path_key: String,
    pub display_path: String,
    pub identity_kind: String,
    pub identity_key: Vec<u8>,
    pub snippet: Option<String>,
}

impl fmt::Debug for LexicalSearchCandidate {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("LexicalSearchCandidate")
            .field("source", &self.source)
            .field("scope_id", &self.scope_id)
            .field("policy_revision", &self.policy_revision)
            .field("node_id", &self.node_id)
            .field("location_id", &self.location_id)
            .field("path_key", &"<redacted>")
            .field("display_path", &"<redacted>")
            .field("identity_kind_len", &self.identity_kind.len())
            .field("identity_key", &"<redacted>")
            .field("snippet", &self.snippet.as_ref().map(|_| "<redacted>"))
            .finish()
    }
}

#[derive(Debug)]
pub enum DatabaseError {
    Io(std::io::Error),
    Sqlite(rusqlite::Error),
    MigrationChanged { version: i64 },
    ReadOnlyPathInvalid,
    ReadOnlySchemaInvalid,
    ReadOnlyModeUnavailable,
    ReadOnlyQueryTimeout,
    ScopeNotFound,
    ScopeAccessGrantNotFound,
    ScopeAccessGrantNotActive,
    ScopeAccessGrantInputInvalid,
    ScopeExclusionInputInvalid,
    ScopeFilesystemFenceInvalid,
    ScopeFilesystemFenceBusy,
    ScopePolicyRevisionStale,
    ScopeRootRevocationPreviewStale,
    ScopePrivacyPurgeBlocked,
    ScanJobNotFound,
    ScanJobAlreadyActive,
    ScanJobBusy,
    InvalidScanJobState,
    ScanJobIncomplete,
    RunnerLeaseLost,
    ExtractableFileNotFound,
    ExtractionJobNotFound,
    ExtractionJobAlreadyActive,
    ExtractionJobBusy,
    InvalidExtractionJobState,
    ExtractionRunnerLeaseLost,
    ExtractionOutputInvalid,
    ImageMetadataNotFound,
    SearchInputInvalid,
    SearchFolderInvalid,
    WatchEventNotFound,
    InvalidWatchEventState,
    WatchScopeInitialScanRequired,
    WatchInputInvalid,
    WatchFileDeltaNotEligible,
    WatchFileDeltaSnapshotChanged,
    ActionSourceNotFound,
    ActionPlanNotFound,
    ActionPlanInputInvalid,
    ActionSourceSnapshotChanged,
    ActionExecutionBindingUnavailable,
    ActionExecutionRecordNotFound,
    ActionJournalInputInvalid,
    ActionJournalInvalidTransition,
    ActionJournalCompareAndSwapFailed,
    ActionJournalIdempotencyConflict,
    ActionJournalCommandNotFound,
    ActionExecutorLeaseUnavailable,
    CleanupActionPlanNotFound,
    CleanupActionPlanInputInvalid,
    CleanupActionSourceNotCurrent,
    SmartCleanupSourceInputInvalid,
    FolderNotFound,
    FolderProfileInputInvalid,
    FolderProfileTooLarge,
    ProjectCandidateNotFound,
    ProjectCandidateInputInvalid,
    ProjectCandidateRootNotCurrent,
    FileRelationCandidateNotFound,
    FileRelationCandidateInputInvalid,
    FileRelationCandidateNotCurrent,
    ScreenshotGroupCandidateNotFound,
    ScreenshotGroupCandidateInputInvalid,
    ScreenshotGroupCandidateNotCurrent,
    ScreenshotGroupImageLimitExceeded,
    ScreenshotGroupLimitExceeded,
    ScreenshotGroupMemberLimitExceeded,
    InvalidStoredValue,
    InvalidCount,
    InvalidTimestamp,
}

impl DatabaseError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::Io(_) => "database_io_failed",
            Self::Sqlite(_) => "database_operation_failed",
            Self::MigrationChanged { .. } => "database_migration_changed",
            Self::ReadOnlyPathInvalid => "database_read_only_path_invalid",
            Self::ReadOnlySchemaInvalid => "database_read_only_schema_invalid",
            Self::ReadOnlyModeUnavailable => "database_read_only_mode_unavailable",
            Self::ReadOnlyQueryTimeout => "database_read_only_query_timeout",
            Self::ScopeNotFound => "authorized_scope_not_found",
            Self::ScopeAccessGrantNotFound => "scope_access_grant_not_found",
            Self::ScopeAccessGrantNotActive => "scope_access_grant_not_active",
            Self::ScopeAccessGrantInputInvalid => "scope_access_grant_input_invalid",
            Self::ScopeExclusionInputInvalid => "scope_exclusion_input_invalid",
            Self::ScopeFilesystemFenceInvalid => "scope_filesystem_fence_invalid",
            Self::ScopeFilesystemFenceBusy => "scope_filesystem_fence_busy",
            Self::ScopePolicyRevisionStale => "scope_policy_revision_stale",
            Self::ScopeRootRevocationPreviewStale => "scope_root_revocation_preview_stale",
            Self::ScopePrivacyPurgeBlocked => "scope_privacy_purge_blocked",
            Self::ScanJobNotFound => "scan_job_not_found",
            Self::ScanJobAlreadyActive => "scan_job_already_active",
            Self::ScanJobBusy => "scan_job_busy",
            Self::InvalidScanJobState => "invalid_scan_job_state",
            Self::ScanJobIncomplete => "scan_job_incomplete",
            Self::RunnerLeaseLost => "scan_runner_lease_lost",
            Self::ExtractableFileNotFound => "extractable_file_not_found",
            Self::ExtractionJobNotFound => "extraction_job_not_found",
            Self::ExtractionJobAlreadyActive => "extraction_job_already_active",
            Self::ExtractionJobBusy => "extraction_job_busy",
            Self::InvalidExtractionJobState => "invalid_extraction_job_state",
            Self::ExtractionRunnerLeaseLost => "extraction_runner_lease_lost",
            Self::ExtractionOutputInvalid => "extraction_output_invalid",
            Self::ImageMetadataNotFound => "image_metadata_not_found",
            Self::SearchInputInvalid => "search_input_invalid",
            Self::SearchFolderInvalid => "search_folder_invalid",
            Self::WatchEventNotFound => "watch_event_not_found",
            Self::InvalidWatchEventState => "invalid_watch_event_state",
            Self::WatchScopeInitialScanRequired => "watch_scope_initial_scan_required",
            Self::WatchInputInvalid => "watch_input_invalid",
            Self::WatchFileDeltaNotEligible => "watch_file_delta_not_eligible",
            Self::WatchFileDeltaSnapshotChanged => "watch_file_delta_snapshot_changed",
            Self::ActionSourceNotFound => "action_source_not_found",
            Self::ActionPlanNotFound => "action_plan_not_found",
            Self::ActionPlanInputInvalid => "action_plan_input_invalid",
            Self::ActionSourceSnapshotChanged => "action_source_snapshot_changed",
            Self::ActionExecutionBindingUnavailable => "action_execution_binding_unavailable",
            Self::ActionExecutionRecordNotFound => "action_execution_record_not_found",
            Self::ActionJournalInputInvalid => "action_journal_input_invalid",
            Self::ActionJournalInvalidTransition => "action_journal_invalid_transition",
            Self::ActionJournalCompareAndSwapFailed => "action_journal_compare_and_swap_failed",
            Self::ActionJournalIdempotencyConflict => "action_journal_idempotency_conflict",
            Self::ActionJournalCommandNotFound => "action_journal_command_not_found",
            Self::ActionExecutorLeaseUnavailable => "action_executor_lease_unavailable",
            Self::CleanupActionPlanNotFound => "cleanup_action_plan_not_found",
            Self::CleanupActionPlanInputInvalid => "cleanup_action_plan_input_invalid",
            Self::CleanupActionSourceNotCurrent => "cleanup_action_source_not_current",
            Self::SmartCleanupSourceInputInvalid => "smart_cleanup_source_input_invalid",
            Self::FolderNotFound => "folder_not_found",
            Self::FolderProfileInputInvalid => "folder_profile_input_invalid",
            Self::FolderProfileTooLarge => "folder_profile_entry_limit_exceeded",
            Self::ProjectCandidateNotFound => "project_candidate_not_found",
            Self::ProjectCandidateInputInvalid => "project_candidate_input_invalid",
            Self::ProjectCandidateRootNotCurrent => "project_candidate_root_not_current",
            Self::FileRelationCandidateNotFound => "file_relation_candidate_not_found",
            Self::FileRelationCandidateInputInvalid => "file_relation_candidate_input_invalid",
            Self::FileRelationCandidateNotCurrent => "file_relation_candidate_not_current",
            Self::ScreenshotGroupCandidateNotFound => "screenshot_group_candidate_not_found",
            Self::ScreenshotGroupCandidateInputInvalid => {
                "screenshot_group_candidate_input_invalid"
            }
            Self::ScreenshotGroupCandidateNotCurrent => "screenshot_group_candidate_not_current",
            Self::ScreenshotGroupImageLimitExceeded => "screenshot_group_image_limit_exceeded",
            Self::ScreenshotGroupLimitExceeded => "screenshot_group_limit_exceeded",
            Self::ScreenshotGroupMemberLimitExceeded => "screenshot_group_member_limit_exceeded",
            Self::InvalidStoredValue => "database_invalid_stored_value",
            Self::InvalidCount => "database_count_out_of_range",
            Self::InvalidTimestamp => "system_time_invalid",
        }
    }
}

impl fmt::Display for DatabaseError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.code())
    }
}

impl std::error::Error for DatabaseError {}

impl From<std::io::Error> for DatabaseError {
    fn from(error: std::io::Error) -> Self {
        Self::Io(error)
    }
}

impl From<rusqlite::Error> for DatabaseError {
    fn from(error: rusqlite::Error) -> Self {
        Self::Sqlite(error)
    }
}

pub struct ManifestDatabase {
    connection: Connection,
    fence_domain: ScopeFilesystemFenceDomain,
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum ScopeFilesystemFenceDomain {
    File {
        identity_kind: &'static str,
        identity_key: Vec<u8>,
    },
    ProcessLocal(u64),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ScopeFilesystemFenceRole {
    Root,
    Gate,
    Data,
}

impl ScopeFilesystemFenceRole {
    const fn as_str(self) -> &'static str {
        match self {
            Self::Root => "root",
            Self::Gate => "gate",
            Self::Data => "data",
        }
    }
}

/// A cooperative, per-scope, cross-process read admission guard.
///
/// DeskGraph-owned readers hold a shared OS lock from their final durable
/// authorization check through every source-path syscall. Root revocation
/// holds the matching exclusive lock through its atomic SQLite commit. The
/// lock file lives only in DeskGraph-managed app data and never inside a user
/// scope. SQLite policy revision checks remain the durable authorization
/// source; this guard closes only the live check-to-read race.
pub struct ScopeFilesystemReadFence {
    data_file: Option<File>,
    binding: ScopePolicyBinding,
    domain: ScopeFilesystemFenceDomain,
}

impl ScopeFilesystemReadFence {
    #[must_use]
    pub fn binding(&self) -> ScopePolicyBinding {
        self.binding
    }
}

impl fmt::Debug for ScopeFilesystemReadFence {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ScopeFilesystemReadFence")
            .field("data_file", &self.data_file.is_some())
            .field("binding", &self.binding)
            .field("domain", &"<redacted>")
            .finish()
    }
}

impl Drop for ScopeFilesystemReadFence {
    fn drop(&mut self) {
        if let Some(file) = self.data_file.as_ref() {
            let _ = file.unlock();
        }
    }
}

/// Scope-matched exclusive capability used while committing root withdrawal
/// or a hard exclusion. Its private binding prevents callers from forging a
/// fence for another scope or revision.
pub struct ScopeFilesystemRevocationFence {
    gate_file: Option<File>,
    data_file: Option<File>,
    binding: ScopePolicyBinding,
    domain: ScopeFilesystemFenceDomain,
}

impl ScopeFilesystemRevocationFence {
    #[must_use]
    pub fn binding(&self) -> ScopePolicyBinding {
        self.binding
    }
}

impl fmt::Debug for ScopeFilesystemRevocationFence {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ScopeFilesystemRevocationFence")
            .field("gate_file", &self.gate_file.is_some())
            .field("data_file", &self.data_file.is_some())
            .field("binding", &self.binding)
            .field("domain", &"<redacted>")
            .finish()
    }
}

impl Drop for ScopeFilesystemRevocationFence {
    fn drop(&mut self) {
        if let Some(file) = self.data_file.as_ref() {
            let _ = file.unlock();
        }
        if let Some(file) = self.gate_file.as_ref() {
            let _ = file.unlock();
        }
    }
}

fn lock_scope_filesystem_fence_exclusive(file: &File) -> Result<(), DatabaseError> {
    let deadline = Instant::now() + SCOPE_FILESYSTEM_FENCE_WAIT_TIMEOUT;
    loop {
        match file.try_lock() {
            Ok(()) => return Ok(()),
            Err(std::fs::TryLockError::WouldBlock) if Instant::now() < deadline => {
                std::thread::sleep(SCOPE_FILESYSTEM_FENCE_RETRY_INTERVAL);
            }
            Err(std::fs::TryLockError::WouldBlock) => {
                return Err(DatabaseError::ScopeFilesystemFenceBusy);
            }
            Err(std::fs::TryLockError::Error(error)) => return Err(error.into()),
        }
    }
}

/// A capability-limited connection to an existing, fully migrated manifest.
///
/// This type deliberately exposes no migration, recovery, scan, extraction,
/// transaction-journal, or filesystem-action methods. Opening it never creates
/// a parent directory or database and never falls back to read-write access.
pub struct ManifestReadDatabase {
    connection: Connection,
}

impl ManifestReadDatabase {
    pub fn open_existing_read_only(path: &Path) -> Result<Self, DatabaseError> {
        #[cfg(not(any(target_os = "macos", target_os = "linux")))]
        {
            let _ = path;
            Err(DatabaseError::ReadOnlyModeUnavailable)
        }
        #[cfg(any(target_os = "macos", target_os = "linux"))]
        {
            let identity_before = validate_existing_database_path(path)?;
            validate_existing_sqlite_sidecars(path)?;
            let connection = Connection::open_with_flags(
                path,
                OpenFlags::SQLITE_OPEN_READ_ONLY
                    | OpenFlags::SQLITE_OPEN_NO_MUTEX
                    | OpenFlags::SQLITE_OPEN_NOFOLLOW,
            )?;
            let identity_after = validate_existing_database_path(path)?;
            validate_existing_sqlite_sidecars(path)?;
            if identity_before != identity_after {
                return Err(DatabaseError::ReadOnlyPathInvalid);
            }
            connection.execute_batch(
                "PRAGMA query_only = ON;\
                 PRAGMA foreign_keys = ON;\
                 PRAGMA temp_store = MEMORY;",
            )?;
            connection.busy_timeout(READ_ONLY_BUSY_TIMEOUT)?;
            if !connection.is_readonly("main")?
                || !connection.pragma_query_value(None, "query_only", |row| {
                    row.get::<_, i64>(0).map(|value| value == 1)
                })?
            {
                return Err(DatabaseError::ReadOnlyModeUnavailable);
            }
            validate_schema_migrations_exact(&connection)?;
            Ok(Self { connection })
        }
    }

    pub fn ensure_scope_queryable(&self, scope_id: i64) -> Result<(), DatabaseError> {
        ensure_scope_queryable(&self.connection, scope_id)
    }

    pub fn bind_scope_policy_revision(
        &self,
        scope_id: i64,
    ) -> Result<ScopePolicyBinding, DatabaseError> {
        let revision = current_scope_policy_revision_from_connection(&self.connection, scope_id)?;
        ensure_scope_access_permitted(&self.connection, scope_id)?;
        Ok(ScopePolicyBinding { scope_id, revision })
    }

    /// Revision-only binding for the explicitly authorized core CLI scan and
    /// extraction pipeline. Desktop, MCP, Search and privacy changes must use
    /// `bind_scope_policy_revision`, which additionally requires an active OS grant.
    pub fn bind_core_scope_policy_revision(
        &self,
        scope_id: i64,
    ) -> Result<ScopeRevisionBinding, DatabaseError> {
        let revision = current_scope_policy_revision_from_connection(&self.connection, scope_id)?;
        Ok(ScopeRevisionBinding { scope_id, revision })
    }

    pub fn is_core_scope_policy_binding_current(
        &self,
        binding: ScopeRevisionBinding,
    ) -> Result<bool, DatabaseError> {
        core_scope_policy_binding_is_current(&self.connection, binding)
    }

    pub fn is_scope_policy_binding_current(
        &self,
        binding: ScopePolicyBinding,
    ) -> Result<bool, DatabaseError> {
        scope_policy_binding_is_current(&self.connection, binding)
    }

    pub fn scope_exclusion_matcher(
        &self,
        scope_id: i64,
    ) -> Result<ScopeExclusionMatcher, DatabaseError> {
        scope_exclusion_matcher_from_connection(&self.connection, scope_id)
    }

    /// Returns current, explicitly requested folder paths for one queryable
    /// scope. Callers must keep this path-bearing response inside the active
    /// user-invoked folder-selection surface and must not log it.
    pub fn list_search_folders(
        &self,
        scope_id: i64,
        limit: Option<u32>,
    ) -> Result<SearchFolderListResponse, DatabaseError> {
        let deadline = Instant::now() + READ_ONLY_QUERY_TIMEOUT;
        self.connection.progress_handler(
            READ_ONLY_PROGRESS_OPS,
            Some(move || Instant::now() >= deadline),
        )?;
        let result = (|| {
            let transaction = self.connection.unchecked_transaction()?;
            let response = search_folder_list_from_connection(&transaction, scope_id, limit)?;
            transaction.commit()?;
            Ok(response)
        })();
        let clear_result = self
            .connection
            .progress_handler(0, None::<fn() -> bool>)
            .map_err(DatabaseError::from);
        let result = result.map_err(normalize_read_only_query_error);
        clear_result?;
        result
    }

    pub fn lexical_search_candidates(
        &self,
        match_query: &str,
        filters: LexicalSearchFilters<'_>,
        per_source_candidate_limit: u32,
    ) -> Result<Vec<LexicalSearchCandidate>, DatabaseError> {
        self.lexical_search_candidates_until(
            match_query,
            filters,
            per_source_candidate_limit,
            Instant::now() + READ_ONLY_QUERY_TIMEOUT,
        )
    }

    fn lexical_search_candidates_until(
        &self,
        match_query: &str,
        filters: LexicalSearchFilters<'_>,
        per_source_candidate_limit: u32,
        deadline: Instant,
    ) -> Result<Vec<LexicalSearchCandidate>, DatabaseError> {
        let scope_id = filters.scope_id.ok_or(DatabaseError::SearchInputInvalid)?;
        self.connection.progress_handler(
            READ_ONLY_PROGRESS_OPS,
            Some(move || Instant::now() >= deadline),
        )?;
        let result = (|| {
            let transaction = self.connection.unchecked_transaction()?;
            ensure_scope_queryable(&transaction, scope_id)?;
            let candidates = lexical_search_candidates_from_connection(
                &transaction,
                match_query,
                filters,
                per_source_candidate_limit,
            )?;
            transaction.commit()?;
            Ok(candidates)
        })();
        let clear_result = self
            .connection
            .progress_handler(0, None::<fn() -> bool>)
            .map_err(DatabaseError::from);
        let result = result.map_err(normalize_read_only_query_error);
        clear_result?;
        result
    }
}

fn normalize_read_only_query_error(error: DatabaseError) -> DatabaseError {
    match &error {
        DatabaseError::Sqlite(sqlite)
            if matches!(
                sqlite.sqlite_error_code(),
                Some(
                    rusqlite::ffi::ErrorCode::OperationInterrupted
                        | rusqlite::ffi::ErrorCode::DatabaseBusy
                        | rusqlite::ffi::ErrorCode::DatabaseLocked
                )
            ) =>
        {
            DatabaseError::ReadOnlyQueryTimeout
        }
        _ => error,
    }
}

fn scope_filesystem_fence_domain(
    connection: &Connection,
) -> Result<ScopeFilesystemFenceDomain, DatabaseError> {
    let Some(database_path) = connection.path().filter(|path| !path.is_empty()) else {
        let nonce = IN_MEMORY_FENCE_DOMAIN_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        if nonce == 0 {
            return Err(DatabaseError::ScopeFilesystemFenceInvalid);
        }
        return Ok(ScopeFilesystemFenceDomain::ProcessLocal(nonce));
    };
    let canonical = fs::canonicalize(Path::new(database_path))
        .map_err(|_| DatabaseError::ScopeFilesystemFenceInvalid)?;
    let metadata =
        fs::symlink_metadata(&canonical).map_err(|_| DatabaseError::ScopeFilesystemFenceInvalid)?;
    if !metadata.is_file() || is_symlink_or_reparse_point(&metadata) {
        return Err(DatabaseError::ScopeFilesystemFenceInvalid);
    }
    let identity = platform_identity(&canonical, &metadata, IdentityNodeKind::File)
        .map_err(|_| DatabaseError::ScopeFilesystemFenceInvalid)?;
    if identity.link_count.is_some_and(|links| links != 1) {
        return Err(DatabaseError::ScopeFilesystemFenceInvalid);
    }
    Ok(ScopeFilesystemFenceDomain::File {
        identity_kind: identity.kind,
        identity_key: identity.key,
    })
}

impl ManifestDatabase {
    pub fn open(path: &Path) -> Result<Self, DatabaseError> {
        if let Some(parent) = path
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
        {
            fs::create_dir_all(parent)?;
        }

        let connection = Connection::open(path)?;
        Self::from_connection(connection)
    }

    pub fn open_in_memory() -> Result<Self, DatabaseError> {
        Self::from_connection(Connection::open_in_memory()?)
    }

    fn from_connection(connection: Connection) -> Result<Self, DatabaseError> {
        connection.execute_batch(
            "PRAGMA foreign_keys = ON;\
             PRAGMA busy_timeout = 5000;\
             PRAGMA synchronous = FULL;",
        )?;
        if !connection.is_autocommit() {
            return Err(DatabaseError::Sqlite(
                rusqlite::Error::ExecuteReturnedResults,
            ));
        }
        if connection.path().is_some() {
            connection.pragma_update(None, "journal_mode", "WAL")?;
        }

        let fence_domain = scope_filesystem_fence_domain(&connection)?;
        let mut database = Self {
            connection,
            fence_domain,
        };
        database.apply_migrations()?;
        database.recover_expired_scan_jobs_at(unix_ms()?)?;
        database.recover_expired_extraction_jobs_at(unix_ms()?)?;
        Ok(database)
    }

    fn apply_migrations(&mut self) -> Result<(), DatabaseError> {
        self.connection.execute_batch(
            "CREATE TABLE IF NOT EXISTS schema_migrations (\
                version INTEGER PRIMARY KEY,\
                name TEXT NOT NULL,\
                checksum TEXT NOT NULL,\
                applied_at_unix_ms INTEGER NOT NULL\
             );",
        )?;

        for migration in MIGRATIONS {
            let checksum = migration_checksum(migration.sql);
            let existing = self
                .connection
                .query_row(
                    "SELECT checksum FROM schema_migrations WHERE version = ?1",
                    [migration.version],
                    |row| row.get::<_, String>(0),
                )
                .optional()?;

            if let Some(existing) = existing {
                if existing != checksum {
                    return Err(DatabaseError::MigrationChanged {
                        version: migration.version,
                    });
                }
                continue;
            }

            let applied_at = unix_ms()?;
            let transaction = self.connection.transaction()?;
            transaction.execute_batch(migration.sql)?;
            transaction.execute(
                "INSERT INTO schema_migrations(version, name, checksum, applied_at_unix_ms)\
                 VALUES (?1, ?2, ?3, ?4)",
                params![migration.version, migration.name, checksum, applied_at],
            )?;
            transaction.commit()?;
        }

        Ok(())
    }

    pub fn add_scope(
        &self,
        path_raw: &[u8],
        path_key: &str,
        display_path: &str,
        platform: &str,
    ) -> Result<AuthorizedScope, DatabaseError> {
        let created_at = unix_ms()?;
        self.connection.execute(
            "INSERT INTO authorized_scopes(path_raw, path_key, display_path, platform, created_at_unix_ms)\
             VALUES (?1, ?2, ?3, ?4, ?5)\
             ON CONFLICT(path_key) DO UPDATE SET path_raw = excluded.path_raw, display_path = excluded.display_path\
             ",
            params![path_raw, path_key, display_path, platform, created_at],
        )?;

        self.connection.query_row(
            "SELECT id, display_path, created_at_unix_ms FROM authorized_scopes WHERE path_key = ?1",
            [path_key],
            |row| {
                Ok(AuthorizedScope {
                    id: row.get(0)?,
                    display_path: row.get(1)?,
                    created_at_unix_ms: row.get(2)?,
                })
            },
        ).map_err(Into::into)
    }

    /// Atomically authorizes a scope and stores its platform-owned access
    /// grant. This is the selection path for native platform adapters; callers
    /// cannot observe a committed scope without its requested grant state.
    pub fn add_scope_with_access_grant(
        &mut self,
        path_raw: &[u8],
        path_key: &str,
        display_path: &str,
        grant: ScopeAccessGrantWrite<'_>,
    ) -> Result<AuthorizedScope, DatabaseError> {
        self.add_coverage_roots_with_access_grants(&[CoverageRootAccessGrantWrite {
            path_raw,
            path_key,
            display_path,
            grant,
        }])?
        .into_iter()
        .next()
        .ok_or(DatabaseError::ScopeAccessGrantInputInvalid)
    }

    /// Atomically authorizes every root in one user-confirmed coverage set.
    /// A validation or SQLite failure leaves both scope and grant tables
    /// unchanged for the entire request.
    pub fn add_coverage_roots_with_access_grants(
        &mut self,
        roots: &[CoverageRootAccessGrantWrite<'_>],
    ) -> Result<Vec<AuthorizedScope>, DatabaseError> {
        if roots.is_empty() {
            return Err(DatabaseError::ScopeAccessGrantInputInvalid);
        }
        let mut unique_path_keys = HashSet::with_capacity(roots.len());
        for root in roots {
            if root.path_raw.is_empty()
                || root.path_key.is_empty()
                || root.display_path.is_empty()
                || !unique_path_keys.insert(root.path_key)
            {
                return Err(DatabaseError::ScopeAccessGrantInputInvalid);
            }
            validate_scope_access_grant_platform(root.grant.scope_platform)?;
            validate_scope_access_grant_platform(root.grant.grant_platform)?;
            validate_scope_access_grant_bytes(root.grant.opaque_grant)?;
            if root.grant.scope_platform != root.grant.grant_platform {
                return Err(DatabaseError::ScopeAccessGrantInputInvalid);
            }
        }

        let created_at = unix_ms()?;
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let selected_paths = roots
            .iter()
            .map(|root| {
                path_from_raw(root.path_raw)
                    .map_err(|_| DatabaseError::ScopeAccessGrantInputInvalid)
            })
            .collect::<Result<Vec<_>, _>>()?;
        for (index, selected) in selected_paths.iter().enumerate() {
            if selected_paths
                .iter()
                .skip(index + 1)
                .any(|other| coverage_paths_overlap(selected, other))
            {
                return Err(DatabaseError::ScopeAccessGrantInputInvalid);
            }
        }
        let existing_roots = {
            let mut statement = transaction.prepare(
                "SELECT scope.path_raw, scope.path_key, scope.platform \
                 FROM authorized_scopes scope \
                 JOIN scope_access_grants grant \
                   ON grant.scope_id = scope.id AND grant.platform = scope.platform \
                  AND grant.state = 'active' \
                 WHERE scope.platform = ?1 AND grant.platform = ?1 \
                 ORDER BY scope.id",
            )?;
            let rows = statement.query_map([std::env::consts::OS], |row| {
                Ok((
                    row.get::<_, Vec<u8>>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                ))
            })?;
            rows.collect::<Result<Vec<_>, _>>()?
        };
        for (existing_raw, existing_key, existing_platform) in existing_roots {
            let existing_path =
                path_from_raw(&existing_raw).map_err(|_| DatabaseError::InvalidStoredValue)?;
            for (index, root) in roots.iter().enumerate() {
                if root.grant.scope_platform == existing_platform
                    && root.path_key != existing_key
                    && coverage_paths_overlap(&selected_paths[index], &existing_path)
                {
                    return Err(DatabaseError::ScopeAccessGrantInputInvalid);
                }
            }
        }
        let mut scopes = Vec::with_capacity(roots.len());
        for root in roots {
            transaction.execute(
                "INSERT INTO authorized_scopes(path_raw, path_key, display_path, platform, created_at_unix_ms) \
                 VALUES (?1, ?2, ?3, ?4, ?5) \
                 ON CONFLICT(path_key) DO UPDATE SET \
                     path_raw = excluded.path_raw, display_path = excluded.display_path",
                params![
                    root.path_raw,
                    root.path_key,
                    root.display_path,
                    root.grant.scope_platform,
                    created_at
                ],
            )?;
            let (scope, persisted_platform): (AuthorizedScope, String) = transaction.query_row(
                "SELECT id, display_path, created_at_unix_ms, platform \
                 FROM authorized_scopes WHERE path_key = ?1",
                [root.path_key],
                |row| {
                    Ok((
                        AuthorizedScope {
                            id: row.get(0)?,
                            display_path: row.get(1)?,
                            created_at_unix_ms: row.get(2)?,
                        },
                        row.get(3)?,
                    ))
                },
            )?;
            if persisted_platform != root.grant.grant_platform {
                return Err(DatabaseError::ScopeAccessGrantInputInvalid);
            }
            let persisted_opaque_grant: &[u8] =
                if root.grant.state == ScopeAccessGrantState::Revoked {
                    &[0]
                } else {
                    root.grant.opaque_grant
                };
            transaction.execute(
                "INSERT INTO scope_access_grants( \
                     scope_id, platform, opaque_grant, state, updated_at_unix_ms \
                 ) VALUES (?1, ?2, ?3, ?4, ?5) \
                 ON CONFLICT(scope_id) DO UPDATE SET \
                     platform = excluded.platform, \
                     opaque_grant = excluded.opaque_grant, \
                     state = excluded.state, \
                     updated_at_unix_ms = excluded.updated_at_unix_ms",
                params![
                    scope.id,
                    root.grant.grant_platform,
                    persisted_opaque_grant,
                    root.grant.state.as_str(),
                    created_at
                ],
            )?;
            scopes.push(scope);
        }
        transaction.commit()?;
        Ok(scopes)
    }

    pub fn list_scopes(&self) -> Result<Vec<AuthorizedScope>, DatabaseError> {
        let mut statement = self.connection.prepare(
            "SELECT id, display_path, created_at_unix_ms FROM authorized_scopes ORDER BY id",
        )?;
        let rows = statement.query_map([], |row| {
            Ok(AuthorizedScope {
                id: row.get(0)?,
                display_path: row.get(1)?,
                created_at_unix_ms: row.get(2)?,
            })
        })?;
        rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    }

    /// Backend-only canonical records for coverage-policy overlap validation.
    /// Do not serialize this shape through ordinary IPC or logs.
    pub fn list_scope_records(&self) -> Result<Vec<ScopeRecord>, DatabaseError> {
        let mut statement = self.connection.prepare(
            "SELECT id, path_raw, path_key, display_path, platform FROM authorized_scopes ORDER BY id",
        )?;
        let rows = statement.query_map([], |row| {
            Ok(ScopeRecord {
                id: row.get(0)?,
                path_raw: row.get(1)?,
                path_key: row.get(2)?,
                display_path: row.get(3)?,
                platform: row.get(4)?,
            })
        })?;
        rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    }

    /// Backend-only canonical records for roots with a currently usable OS
    /// capability. Historical or quarantined scopes cannot block a new
    /// coverage confirmation and are never treated as active roots.
    pub fn list_active_scope_records(&self) -> Result<Vec<ScopeRecord>, DatabaseError> {
        let mut statement = self.connection.prepare(
            "SELECT scope.id, scope.path_raw, scope.path_key, scope.display_path, scope.platform \
             FROM authorized_scopes scope \
             JOIN scope_access_grants grant \
               ON grant.scope_id = scope.id AND grant.platform = scope.platform \
              AND grant.state = 'active' \
             WHERE scope.platform = ?1 AND grant.platform = ?1 \
             ORDER BY scope.id",
        )?;
        let rows = statement.query_map([std::env::consts::OS], |row| {
            Ok(ScopeRecord {
                id: row.get(0)?,
                path_raw: row.get(1)?,
                path_key: row.get(2)?,
                display_path: row.get(3)?,
                platform: row.get(4)?,
            })
        })?;
        rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    }

    pub fn scope_record(&self, scope_id: i64) -> Result<ScopeRecord, DatabaseError> {
        self.connection
            .query_row(
                "SELECT id, path_raw, path_key, display_path, platform \
                 FROM authorized_scopes WHERE id = ?1",
                [scope_id],
                |row| {
                    Ok(ScopeRecord {
                        id: row.get(0)?,
                        path_raw: row.get(1)?,
                        path_key: row.get(2)?,
                        display_path: row.get(3)?,
                        platform: row.get(4)?,
                    })
                },
            )
            .optional()?
            .ok_or(DatabaseError::ScopeNotFound)
    }

    pub fn current_scope_policy_revision(
        &self,
        scope_id: i64,
    ) -> Result<ScopePolicyRevision, DatabaseError> {
        let revision = self
            .connection
            .query_row(
                "SELECT policy_revision FROM authorized_scopes WHERE id = ?1",
                [scope_id],
                |row| row.get::<_, i64>(0),
            )
            .optional()?
            .ok_or(DatabaseError::ScopeNotFound)?;
        Ok(ScopePolicyRevision { scope_id, revision })
    }

    pub fn bind_scope_policy_revision(
        &self,
        scope_id: i64,
    ) -> Result<ScopePolicyBinding, DatabaseError> {
        let revision = current_scope_policy_revision_from_connection(&self.connection, scope_id)?;
        ensure_scope_access_permitted(&self.connection, scope_id)?;
        Ok(ScopePolicyBinding { scope_id, revision })
    }

    /// Acquires the shared live-read guard and then repeats the durable access
    /// check while the guard is held. A revoker that committed between the
    /// initial check and lock acquisition therefore prevents the caller from
    /// reaching any user-scope filesystem operation.
    pub fn acquire_scope_filesystem_read_fence(
        &self,
        scope_id: i64,
    ) -> Result<ScopeFilesystemReadFence, DatabaseError> {
        let before = self.bind_scope_policy_revision(scope_id)?;
        let Some((gate_file, data_file)) = self.open_scope_filesystem_fence_files(scope_id)? else {
            return Ok(ScopeFilesystemReadFence {
                data_file: None,
                binding: before,
                domain: self.fence_domain.clone(),
            });
        };
        lock_scope_filesystem_fence_exclusive(&gate_file)?;
        data_file.lock_shared()?;
        let after = match self.bind_scope_policy_revision(scope_id) {
            Ok(binding) if binding == before => binding,
            Ok(_) => return Err(DatabaseError::ScopePolicyRevisionStale),
            Err(error) => return Err(error),
        };
        gate_file.unlock()?;
        Ok(ScopeFilesystemReadFence {
            data_file: Some(data_file),
            binding: after,
            domain: self.fence_domain.clone(),
        })
    }

    /// Acquires an exclusive admission turnstile before draining readers. Once
    /// the turnstile is held, no new cooperating reader can enter; both waits
    /// are bounded so a stuck reader becomes an explicit retryable failure
    /// rather than an indefinitely hung privacy action.
    pub fn acquire_scope_filesystem_revocation_fence(
        &self,
        scope_id: i64,
    ) -> Result<ScopeFilesystemRevocationFence, DatabaseError> {
        let before = self.bind_scope_policy_revision(scope_id)?;
        let Some((gate_file, data_file)) = self.open_scope_filesystem_fence_files(scope_id)? else {
            return Ok(ScopeFilesystemRevocationFence {
                gate_file: None,
                data_file: None,
                binding: before,
                domain: self.fence_domain.clone(),
            });
        };
        lock_scope_filesystem_fence_exclusive(&gate_file)?;
        if let Err(error) = lock_scope_filesystem_fence_exclusive(&data_file) {
            let _ = gate_file.unlock();
            return Err(error);
        }
        let after = match self.bind_scope_policy_revision(scope_id) {
            Ok(binding) if binding == before => binding,
            Ok(_) => return Err(DatabaseError::ScopePolicyRevisionStale),
            Err(error) => return Err(error),
        };
        Ok(ScopeFilesystemRevocationFence {
            gate_file: Some(gate_file),
            data_file: Some(data_file),
            binding: after,
            domain: self.fence_domain.clone(),
        })
    }

    pub fn validate_scope_filesystem_revocation_fence(
        &self,
        fence: &ScopeFilesystemRevocationFence,
        binding: ScopePolicyBinding,
    ) -> Result<(), DatabaseError> {
        if fence.domain != self.fence_domain || fence.binding.scope_id != binding.scope_id {
            return Err(DatabaseError::ScopeFilesystemFenceInvalid);
        }
        if fence.binding.revision != binding.revision {
            return Err(DatabaseError::ScopePolicyRevisionStale);
        }
        Ok(())
    }

    pub fn validate_scope_filesystem_read_fence(
        &self,
        fence: &ScopeFilesystemReadFence,
        binding: ScopePolicyBinding,
    ) -> Result<(), DatabaseError> {
        if fence.domain != self.fence_domain || fence.binding.scope_id != binding.scope_id {
            return Err(DatabaseError::ScopeFilesystemFenceInvalid);
        }
        if fence.binding.revision != binding.revision {
            return Err(DatabaseError::ScopePolicyRevisionStale);
        }
        Ok(())
    }

    fn open_scope_filesystem_fence_files(
        &self,
        scope_id: i64,
    ) -> Result<Option<(File, File)>, DatabaseError> {
        if scope_id <= 0 {
            return Err(DatabaseError::ScopeFilesystemFenceInvalid);
        }
        let Some(database_path) = self.connection.path().filter(|path| !path.is_empty()) else {
            // An in-memory SQLite database cannot be shared with another
            // process, so the repeated policy check above is the complete
            // admission boundary for that test-only/runtime-local case.
            return Ok(None);
        };
        // Resolve aliases after SQLite has created/opened the database so two
        // cooperating processes that address the same manifest through
        // different relative or symlinked paths still converge on one fence
        // directory. Failure is closed because a split fence would re-open
        // the live check-to-read race this guard exists to prevent.
        let database_path = fs::canonicalize(Path::new(database_path))
            .map_err(|_| DatabaseError::ScopeFilesystemFenceInvalid)?;
        let database_parent = database_path
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
            .unwrap_or_else(|| Path::new("."));
        #[cfg(unix)]
        {
            self.open_scope_filesystem_fence_files_unix(scope_id, &database_path, database_parent)
                .map(Some)
        }
        #[cfg(windows)]
        {
            fence_windows::open_scope_filesystem_fence_files(
                self,
                scope_id,
                &database_path,
                database_parent,
            )
            .map(Some)
        }
        #[cfg(not(any(unix, windows)))]
        {
            let fence_root = database_parent.join("scope-read-fences-v1");
            self.prepare_scope_filesystem_fence_root(scope_id, &fence_root)?;
            let gate_path = fence_root.join(format!("scope-{scope_id}.gate"));
            let data_path = fence_root.join(format!("scope-{scope_id}.lock"));
            let gate_file = self.open_scope_filesystem_fence_file(
                scope_id,
                ScopeFilesystemFenceRole::Gate,
                &gate_path,
            )?;
            let data_file = self.open_scope_filesystem_fence_file(
                scope_id,
                ScopeFilesystemFenceRole::Data,
                &data_path,
            )?;
            self.validate_and_bind_scope_filesystem_fence_root(scope_id, &fence_root)?;
            Ok(Some((gate_file, data_file)))
        }
    }

    #[cfg(unix)]
    fn open_scope_filesystem_fence_files_unix(
        &self,
        scope_id: i64,
        database_path: &Path,
        database_parent: &Path,
    ) -> Result<(File, File), DatabaseError> {
        let fence_root = database_parent.join("scope-read-fences-v1");
        let parent_name = CString::new(database_parent.as_os_str().as_bytes())
            .map_err(|_| DatabaseError::ScopeFilesystemFenceInvalid)?;
        // SAFETY: `parent_name` is NUL-terminated and the returned descriptor
        // becomes owned by `parent`.
        let parent_fd = unsafe {
            libc::open(
                parent_name.as_ptr(),
                libc::O_RDONLY
                    | libc::O_CLOEXEC
                    | libc::O_DIRECTORY
                    | libc::O_NOFOLLOW
                    | libc::O_NOCTTY,
            )
        };
        if parent_fd < 0 {
            return Err(DatabaseError::ScopeFilesystemFenceInvalid);
        }
        // SAFETY: `parent_fd` is a newly owned descriptor from `open`.
        let parent = unsafe { File::from_raw_fd(parent_fd) };

        // Before any mkdir/open-with-create, prove that this pinned directory
        // still contains the exact database inode from which this manifest's
        // private fence domain was derived.
        let database_name = database_path
            .file_name()
            .ok_or(DatabaseError::ScopeFilesystemFenceInvalid)?;
        let database_name = CString::new(database_name.as_bytes())
            .map_err(|_| DatabaseError::ScopeFilesystemFenceInvalid)?;
        // SAFETY: `parent` is a live directory descriptor and `database_name`
        // is one NUL-terminated child name. O_NOFOLLOW rejects a link leaf.
        let database_fd = unsafe {
            libc::openat(
                parent.as_raw_fd(),
                database_name.as_ptr(),
                libc::O_RDONLY | libc::O_CLOEXEC | libc::O_NOFOLLOW | libc::O_NONBLOCK,
            )
        };
        if database_fd < 0 {
            return Err(DatabaseError::ScopeFilesystemFenceInvalid);
        }
        // SAFETY: `database_fd` is a newly owned descriptor from `openat`.
        let database_file = unsafe { File::from_raw_fd(database_fd) };
        let database_metadata = database_file.metadata()?;
        let database_identity = platform_identity_for_open_file(
            &database_file,
            database_path,
            &database_metadata,
            IdentityNodeKind::File,
        )
        .map_err(|_| DatabaseError::ScopeFilesystemFenceInvalid)?;
        if !database_metadata.is_file()
            || database_identity.link_count.is_some_and(|links| links != 1)
            || !matches!(
                &self.fence_domain,
                ScopeFilesystemFenceDomain::File {
                    identity_kind,
                    identity_key,
                } if *identity_kind == database_identity.kind
                    && identity_key == &database_identity.key
            )
        {
            return Err(DatabaseError::ScopeFilesystemFenceInvalid);
        }

        let root_name = CString::new("scope-read-fences-v1")
            .map_err(|_| DatabaseError::ScopeFilesystemFenceInvalid)?;
        // SAFETY: `parent` is pinned to the verified database directory and
        // `root_name` is a single NUL-terminated child name.
        let created = unsafe { libc::mkdirat(parent.as_raw_fd(), root_name.as_ptr(), 0o700) };
        if created < 0 && std::io::Error::last_os_error().raw_os_error() != Some(libc::EEXIST) {
            return Err(DatabaseError::ScopeFilesystemFenceInvalid);
        }
        // SAFETY: the verified parent descriptor remains live, and O_NOFOLLOW
        // prevents a pre-existing symlink from becoming the fence root.
        let root_fd = unsafe {
            libc::openat(
                parent.as_raw_fd(),
                root_name.as_ptr(),
                libc::O_RDONLY
                    | libc::O_CLOEXEC
                    | libc::O_DIRECTORY
                    | libc::O_NOFOLLOW
                    | libc::O_NOCTTY,
            )
        };
        if root_fd < 0 {
            return Err(DatabaseError::ScopeFilesystemFenceInvalid);
        }
        // SAFETY: `root_fd` is a newly owned descriptor from `openat`.
        let root = unsafe { File::from_raw_fd(root_fd) };
        let root_metadata = root.metadata()?;
        if !root_metadata.is_dir() || root_metadata.permissions().mode() & 0o077 != 0 {
            return Err(DatabaseError::ScopeFilesystemFenceInvalid);
        }
        let root_identity = platform_identity_for_open_file(
            &root,
            &fence_root,
            &root_metadata,
            IdentityNodeKind::Folder,
        )
        .map_err(|_| DatabaseError::ScopeFilesystemFenceInvalid)?;
        self.bind_scope_filesystem_fence_identity(
            scope_id,
            ScopeFilesystemFenceRole::Root,
            root_identity.kind,
            &root_identity.key,
        )?;

        let gate_file = self.open_scope_filesystem_fence_file_at_unix(
            &root,
            scope_id,
            ScopeFilesystemFenceRole::Gate,
            &format!("scope-{scope_id}.gate"),
            &fence_root,
        )?;
        let data_file = self.open_scope_filesystem_fence_file_at_unix(
            &root,
            scope_id,
            ScopeFilesystemFenceRole::Data,
            &format!("scope-{scope_id}.lock"),
            &fence_root,
        )?;
        Ok((gate_file, data_file))
    }

    #[cfg(unix)]
    fn open_scope_filesystem_fence_file_at_unix(
        &self,
        root: &File,
        scope_id: i64,
        role: ScopeFilesystemFenceRole,
        entry_name: &str,
        fence_root: &Path,
    ) -> Result<File, DatabaseError> {
        let entry_name =
            CString::new(entry_name).map_err(|_| DatabaseError::ScopeFilesystemFenceInvalid)?;
        // SAFETY: `root` is a pinned, verified directory descriptor and
        // `entry_name` is one NUL-terminated child. O_NOFOLLOW prevents a link
        // leaf; mode applies atomically only if O_CREAT creates a new inode.
        let file_fd = unsafe {
            libc::openat(
                root.as_raw_fd(),
                entry_name.as_ptr(),
                libc::O_RDWR
                    | libc::O_CREAT
                    | libc::O_CLOEXEC
                    | libc::O_NOFOLLOW
                    | libc::O_NONBLOCK
                    | libc::O_NOCTTY,
                0o600,
            )
        };
        if file_fd < 0 {
            return Err(DatabaseError::ScopeFilesystemFenceInvalid);
        }
        // SAFETY: `file_fd` is a newly owned descriptor from `openat`.
        let file = unsafe { File::from_raw_fd(file_fd) };
        let metadata = file.metadata()?;
        if !metadata.is_file() || metadata.permissions().mode() & 0o077 != 0 {
            return Err(DatabaseError::ScopeFilesystemFenceInvalid);
        }
        let diagnostic_path = fence_root.join(format!("scope-{scope_id}.lock"));
        let identity = platform_identity_for_open_file(
            &file,
            &diagnostic_path,
            &metadata,
            IdentityNodeKind::File,
        )
        .map_err(|_| DatabaseError::ScopeFilesystemFenceInvalid)?;
        if identity.link_count.is_some_and(|links| links != 1) {
            return Err(DatabaseError::ScopeFilesystemFenceInvalid);
        }
        self.bind_scope_filesystem_fence_identity(scope_id, role, identity.kind, &identity.key)?;
        Ok(file)
    }

    #[cfg(not(any(unix, windows)))]
    fn prepare_scope_filesystem_fence_root(
        &self,
        scope_id: i64,
        fence_root: &Path,
    ) -> Result<(), DatabaseError> {
        match fs::symlink_metadata(fence_root) {
            Ok(_) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                let builder = DirBuilder::new();
                if let Err(error) = builder.create(fence_root)
                    && error.kind() != std::io::ErrorKind::AlreadyExists
                {
                    return Err(error.into());
                }
            }
            Err(error) => return Err(error.into()),
        }
        self.validate_and_bind_scope_filesystem_fence_root(scope_id, fence_root)
    }

    #[cfg(not(any(unix, windows)))]
    fn validate_and_bind_scope_filesystem_fence_root(
        &self,
        scope_id: i64,
        fence_root: &Path,
    ) -> Result<(), DatabaseError> {
        let metadata = fs::symlink_metadata(fence_root)
            .map_err(|_| DatabaseError::ScopeFilesystemFenceInvalid)?;
        if !metadata.is_dir() || is_symlink_or_reparse_point(&metadata) {
            return Err(DatabaseError::ScopeFilesystemFenceInvalid);
        }
        let identity = platform_identity(fence_root, &metadata, IdentityNodeKind::Folder)
            .map_err(|_| DatabaseError::ScopeFilesystemFenceInvalid)?;
        self.bind_scope_filesystem_fence_identity(
            scope_id,
            ScopeFilesystemFenceRole::Root,
            identity.kind,
            &identity.key,
        )
    }

    #[cfg(not(any(unix, windows)))]
    fn open_scope_filesystem_fence_file(
        &self,
        scope_id: i64,
        role: ScopeFilesystemFenceRole,
        fence_path: &Path,
    ) -> Result<File, DatabaseError> {
        let mut options = OpenOptions::new();
        options.read(true).write(true).create(true);
        let file = options
            .open(fence_path)
            .map_err(|_| DatabaseError::ScopeFilesystemFenceInvalid)?;
        let path_metadata = fs::symlink_metadata(fence_path)?;
        let open_metadata = file.metadata()?;
        if !path_metadata.is_file()
            || !open_metadata.is_file()
            || is_symlink_or_reparse_point(&path_metadata)
        {
            return Err(DatabaseError::ScopeFilesystemFenceInvalid);
        }
        let path_identity = platform_identity(fence_path, &path_metadata, IdentityNodeKind::File)
            .map_err(|_| DatabaseError::ScopeFilesystemFenceInvalid)?;
        let open_identity = platform_identity_for_open_file(
            &file,
            fence_path,
            &open_metadata,
            IdentityNodeKind::File,
        )
        .map_err(|_| DatabaseError::ScopeFilesystemFenceInvalid)?;
        if path_identity.kind != open_identity.kind
            || path_identity.key != open_identity.key
            || path_identity.link_count.is_some_and(|links| links != 1)
            || open_identity.link_count.is_some_and(|links| links != 1)
        {
            return Err(DatabaseError::ScopeFilesystemFenceInvalid);
        }
        self.bind_scope_filesystem_fence_identity(
            scope_id,
            role,
            open_identity.kind,
            &open_identity.key,
        )?;
        Ok(file)
    }

    fn bind_scope_filesystem_fence_identity(
        &self,
        scope_id: i64,
        role: ScopeFilesystemFenceRole,
        identity_kind: &str,
        identity_key: &[u8],
    ) -> Result<(), DatabaseError> {
        if scope_id <= 0 || identity_kind.is_empty() || identity_key.is_empty() {
            return Err(DatabaseError::ScopeFilesystemFenceInvalid);
        }
        let existing = self
            .connection
            .query_row(
                "SELECT identity_kind, identity_key \
                 FROM scope_filesystem_fence_identities \
                 WHERE scope_id=?1 AND role=?2",
                params![scope_id, role.as_str()],
                |row| Ok((row.get::<_, String>(0)?, row.get::<_, Vec<u8>>(1)?)),
            )
            .optional()?;
        match existing {
            Some((stored_kind, stored_key))
                if stored_kind == identity_kind && stored_key == identity_key =>
            {
                return Ok(());
            }
            Some(_) => return Err(DatabaseError::ScopeFilesystemFenceInvalid),
            None => {}
        }
        let transaction =
            Transaction::new_unchecked(&self.connection, TransactionBehavior::Immediate)?;
        let existing = transaction
            .query_row(
                "SELECT identity_kind, identity_key \
                 FROM scope_filesystem_fence_identities \
                 WHERE scope_id=?1 AND role=?2",
                params![scope_id, role.as_str()],
                |row| Ok((row.get::<_, String>(0)?, row.get::<_, Vec<u8>>(1)?)),
            )
            .optional()?;
        match existing {
            Some((stored_kind, stored_key))
                if stored_kind == identity_kind && stored_key == identity_key => {}
            Some(_) => return Err(DatabaseError::ScopeFilesystemFenceInvalid),
            None => {
                transaction.execute(
                    "INSERT INTO scope_filesystem_fence_identities( \
                         scope_id, role, identity_kind, identity_key \
                     ) VALUES (?1, ?2, ?3, ?4)",
                    params![scope_id, role.as_str(), identity_kind, identity_key],
                )?;
            }
        }
        transaction.commit()?;
        Ok(())
    }

    /// Revision-only binding for core CLI scan/extraction work. Packaged and
    /// query surfaces must use the active-grant `bind_scope_policy_revision`.
    pub fn bind_core_scope_policy_revision(
        &self,
        scope_id: i64,
    ) -> Result<ScopeRevisionBinding, DatabaseError> {
        let revision = current_scope_policy_revision_from_connection(&self.connection, scope_id)?;
        Ok(ScopeRevisionBinding { scope_id, revision })
    }

    pub fn is_core_scope_policy_binding_current(
        &self,
        binding: ScopeRevisionBinding,
    ) -> Result<bool, DatabaseError> {
        core_scope_policy_binding_is_current(&self.connection, binding)
    }

    pub fn is_scope_policy_binding_current(
        &self,
        binding: ScopePolicyBinding,
    ) -> Result<bool, DatabaseError> {
        scope_policy_binding_is_current(&self.connection, binding)
    }

    pub fn scope_exclusion_matcher(
        &self,
        scope_id: i64,
    ) -> Result<ScopeExclusionMatcher, DatabaseError> {
        scope_exclusion_matcher_from_connection(&self.connection, scope_id)
    }

    pub fn scope_exclusions(
        &self,
        scope_id: i64,
    ) -> Result<Vec<ScopeExclusionRecord>, DatabaseError> {
        self.current_scope_policy_revision(scope_id)?;
        scope_exclusions_from_connection(&self.connection, scope_id)
    }

    pub fn preview_scope_exclusion_batch(
        &mut self,
        binding: ScopePolicyBinding,
        writes: &[ScopeExclusionWrite<'_>],
    ) -> Result<ScopeExclusionImpactPreview, DatabaseError> {
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Deferred)?;
        assert_scope_policy_binding_in_transaction(&transaction, binding)?;
        let scope = scope_exclusion_validation_context(&transaction, binding.scope_id)?;
        validate_scope_exclusion_batch(&transaction, &scope, writes)?;
        let next_revision = scope
            .revision
            .checked_add(1)
            .ok_or(DatabaseError::InvalidCount)?;
        let nonce = begin_privacy_purge_capability(
            &transaction,
            binding.scope_id,
            scope.revision,
            next_revision,
            0,
        )?;
        insert_privacy_targets_for_writes(&transaction, &nonce, &scope, writes)?;
        let impact = privacy_purge_impact(&transaction, &nonce, binding.scope_id)?;
        transaction.rollback()?;
        Ok(impact)
    }

    pub fn apply_scope_exclusion_batch(
        &mut self,
        binding: ScopePolicyBinding,
        writes: &[ScopeExclusionWrite<'_>],
        now_unix_ms: i64,
    ) -> Result<ScopeExclusionApplyResult, DatabaseError> {
        let fence = self.acquire_scope_filesystem_revocation_fence(binding.scope_id)?;
        self.apply_scope_exclusion_batch_with_fence(&fence, binding, writes, now_unix_ms)
    }

    pub fn apply_scope_exclusion_batch_with_fence(
        &mut self,
        fence: &ScopeFilesystemRevocationFence,
        binding: ScopePolicyBinding,
        writes: &[ScopeExclusionWrite<'_>],
        now_unix_ms: i64,
    ) -> Result<ScopeExclusionApplyResult, DatabaseError> {
        self.validate_scope_filesystem_revocation_fence(fence, binding)?;
        self.apply_scope_exclusion_batch_internal(binding, writes, now_unix_ms)
    }

    fn apply_scope_exclusion_batch_internal(
        &mut self,
        binding: ScopePolicyBinding,
        writes: &[ScopeExclusionWrite<'_>],
        now_unix_ms: i64,
    ) -> Result<ScopeExclusionApplyResult, DatabaseError> {
        if now_unix_ms < 0 {
            return Err(DatabaseError::InvalidTimestamp);
        }
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        assert_scope_policy_binding_in_transaction(&transaction, binding)?;
        let scope = scope_exclusion_validation_context(&transaction, binding.scope_id)?;
        validate_scope_exclusion_batch(&transaction, &scope, writes)?;
        let next_revision = binding
            .revision
            .checked_add(1)
            .ok_or(DatabaseError::InvalidCount)?;
        let nonce = begin_privacy_purge_capability(
            &transaction,
            binding.scope_id,
            binding.revision,
            next_revision,
            now_unix_ms,
        )?;
        let changed = transaction.execute(
            "UPDATE authorized_scopes SET policy_revision = ?3 \
             WHERE id = ?1 AND policy_revision = ?2",
            params![binding.scope_id, binding.revision, next_revision],
        )?;
        if changed != 1 {
            return Err(DatabaseError::ScopePolicyRevisionStale);
        }
        for write in writes {
            transaction.execute(
                "INSERT INTO scope_exclusions( \
                     scope_id, kind, path_raw, path_key, display_path, identity_kind, identity_key, \
                     policy_revision, created_at_unix_ms \
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
                params![
                    binding.scope_id,
                    write.kind.as_str(),
                    write.path_raw,
                    write.path_key,
                    write.display_path,
                    write.identity_kind,
                    write.identity_key,
                    next_revision,
                    now_unix_ms,
                ],
            )?;
        }
        insert_all_privacy_targets(&transaction, &nonce, &scope)?;
        let impact = privacy_purge_impact(&transaction, &nonce, binding.scope_id)?;
        ensure_privacy_purge_actions_are_safe(&transaction, &nonce, binding.scope_id)?;
        let purged_row_count =
            execute_privacy_purge(&transaction, &nonce, binding.scope_id, next_revision)?;
        transaction.execute(
            "INSERT INTO privacy_purge_receipts( \
                 scope_id, from_revision, to_revision, exclusions_added, \
                 affected_location_count, affected_node_count, purged_row_count, created_at_unix_ms \
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                binding.scope_id,
                binding.revision,
                next_revision,
                to_i64(u64::try_from(writes.len()).map_err(|_| DatabaseError::InvalidCount)?)?,
                to_i64(impact.conservative_location_count)?,
                to_i64(impact.conservative_node_count)?,
                to_i64(purged_row_count)?,
                now_unix_ms,
            ],
        )?;
        let receipt_id = transaction.last_insert_rowid();
        let exclusions = scope_exclusions_from_connection(&transaction, binding.scope_id)?;
        let consumed = transaction.execute(
            "DELETE FROM privacy_purge_capabilities WHERE nonce = ?1 AND scope_id = ?2",
            params![nonce, binding.scope_id],
        )?;
        if consumed != 1 {
            return Err(DatabaseError::InvalidStoredValue);
        }
        transaction.commit()?;
        let receipt = PrivacyPurgeReceipt {
            id: receipt_id,
            scope_id: binding.scope_id,
            from_revision: binding.revision,
            to_revision: next_revision,
            exclusions_added: u64::try_from(writes.len())
                .map_err(|_| DatabaseError::InvalidCount)?,
            affected_location_count: impact.conservative_location_count,
            affected_node_count: impact.conservative_node_count,
            purged_row_count,
            created_at_unix_ms: now_unix_ms,
        };
        Ok(ScopeExclusionApplyResult {
            policy: ScopePolicyRevision {
                scope_id: binding.scope_id,
                revision: next_revision,
            },
            receipt,
            purged: impact,
            exclusions,
        })
    }

    pub fn preview_scope_root_revocation(
        &self,
        binding: ScopePolicyBinding,
    ) -> Result<ScopeRootRevocationPreview, DatabaseError> {
        let transaction =
            Transaction::new_unchecked(&self.connection, TransactionBehavior::Deferred)?;
        assert_scope_policy_binding_in_transaction(&transaction, binding)?;
        let next_revision = binding
            .revision
            .checked_add(1)
            .ok_or(DatabaseError::InvalidCount)?;
        let nonce = begin_privacy_purge_capability(
            &transaction,
            binding.scope_id,
            binding.revision,
            next_revision,
            0,
        )?;
        insert_full_scope_privacy_targets(&transaction, &nonce, binding.scope_id)?;
        let impact = scope_root_revocation_impact(&transaction, &nonce, binding.scope_id)?;
        let exclusion_count = transaction.query_row(
            "SELECT COUNT(*) FROM scope_exclusions WHERE scope_id=?1",
            [binding.scope_id],
            |row| row.get::<_, i64>(0),
        )?;
        let exclusion_count =
            u64::try_from(exclusion_count).map_err(|_| DatabaseError::InvalidCount)?;
        transaction.rollback()?;
        Ok(ScopeRootRevocationPreview {
            scope_id: binding.scope_id,
            base_policy_revision: binding.revision,
            impact,
            exclusion_count,
        })
    }

    /// Atomically withdraws one active coverage root. The source filesystem is
    /// never opened or mutated; only the local grant and derived SQLite state
    /// are changed. The fixed one-byte grant tombstone cannot be restored as a
    /// platform capability and avoids retaining the previous bookmark/token.
    pub fn apply_scope_root_revocation(
        &self,
        binding: ScopePolicyBinding,
        now_unix_ms: i64,
    ) -> Result<ScopeRootRevocationApplyResult, DatabaseError> {
        let fence = self.acquire_scope_filesystem_revocation_fence(binding.scope_id)?;
        self.apply_scope_root_revocation_with_fence(&fence, binding, now_unix_ms)
    }

    pub fn apply_scope_root_revocation_with_fence(
        &self,
        fence: &ScopeFilesystemRevocationFence,
        binding: ScopePolicyBinding,
        now_unix_ms: i64,
    ) -> Result<ScopeRootRevocationApplyResult, DatabaseError> {
        self.validate_scope_filesystem_revocation_fence(fence, binding)?;
        self.apply_scope_root_revocation_internal(binding, None, now_unix_ms)
    }

    /// Applies exactly the impact that was shown by
    /// [`Self::preview_scope_root_revocation`]. Derived state can change
    /// without advancing the policy revision, so the impact and exclusion
    /// counts are re-read inside the same immediate transaction and must
    /// still match before any durable mutation is allowed.
    pub fn apply_scope_root_revocation_from_preview(
        &self,
        preview: ScopeRootRevocationPreview,
        now_unix_ms: i64,
    ) -> Result<ScopeRootRevocationApplyResult, DatabaseError> {
        let fence = self.acquire_scope_filesystem_revocation_fence(preview.scope_id)?;
        self.apply_scope_root_revocation_from_preview_with_fence(&fence, preview, now_unix_ms)
    }

    pub fn apply_scope_root_revocation_from_preview_with_fence(
        &self,
        fence: &ScopeFilesystemRevocationFence,
        preview: ScopeRootRevocationPreview,
        now_unix_ms: i64,
    ) -> Result<ScopeRootRevocationApplyResult, DatabaseError> {
        let binding = ScopePolicyBinding {
            scope_id: preview.scope_id,
            revision: preview.base_policy_revision,
        };
        self.validate_scope_filesystem_revocation_fence(fence, binding)?;
        self.apply_scope_root_revocation_internal(binding, Some(preview), now_unix_ms)
    }

    fn apply_scope_root_revocation_internal(
        &self,
        binding: ScopePolicyBinding,
        expected_preview: Option<ScopeRootRevocationPreview>,
        now_unix_ms: i64,
    ) -> Result<ScopeRootRevocationApplyResult, DatabaseError> {
        if now_unix_ms < 0 {
            return Err(DatabaseError::InvalidTimestamp);
        }
        let transaction =
            Transaction::new_unchecked(&self.connection, TransactionBehavior::Immediate)?;
        assert_scope_policy_binding_in_transaction(&transaction, binding)?;
        let next_revision = binding
            .revision
            .checked_add(1)
            .ok_or(DatabaseError::InvalidCount)?;
        let nonce = begin_privacy_purge_capability(
            &transaction,
            binding.scope_id,
            binding.revision,
            next_revision,
            now_unix_ms,
        )?;
        insert_full_scope_privacy_targets(&transaction, &nonce, binding.scope_id)?;
        let impact = scope_root_revocation_impact(&transaction, &nonce, binding.scope_id)?;
        let exclusion_count = transaction.query_row(
            "SELECT COUNT(*) FROM scope_exclusions WHERE scope_id=?1",
            [binding.scope_id],
            |row| row.get::<_, i64>(0),
        )?;
        let exclusion_count =
            u64::try_from(exclusion_count).map_err(|_| DatabaseError::InvalidCount)?;
        if expected_preview.is_some_and(|expected| {
            expected.scope_id != binding.scope_id
                || expected.base_policy_revision != binding.revision
                || expected.impact != impact
                || expected.exclusion_count != exclusion_count
        }) {
            return Err(DatabaseError::ScopeRootRevocationPreviewStale);
        }
        ensure_privacy_purge_actions_are_safe(&transaction, &nonce, binding.scope_id)?;

        let changed = transaction.execute(
            "UPDATE authorized_scopes SET policy_revision=?3 \
             WHERE id=?1 AND policy_revision=?2",
            params![binding.scope_id, binding.revision, next_revision],
        )?;
        if changed != 1 {
            return Err(DatabaseError::ScopePolicyRevisionStale);
        }
        let revoked = transaction.execute(
            "UPDATE scope_access_grants \
             SET opaque_grant=X'00', state='revoked', updated_at_unix_ms=?2 \
             WHERE scope_id=?1 AND state='active' \
               AND platform=(SELECT platform FROM authorized_scopes WHERE id=?1)",
            params![binding.scope_id, now_unix_ms],
        )?;
        if revoked != 1 {
            return Err(DatabaseError::ScopeAccessGrantNotActive);
        }

        let mut purged_row_count = execute_scope_root_revocation_purge(
            &transaction,
            &nonce,
            binding.scope_id,
            next_revision,
        )?;
        let exclusions_removed = transaction.execute(
            "DELETE FROM scope_exclusions WHERE scope_id=?1",
            [binding.scope_id],
        )?;
        let exclusions_removed =
            u64::try_from(exclusions_removed).map_err(|_| DatabaseError::InvalidCount)?;
        purged_row_count = purged_row_count
            .checked_add(exclusions_removed)
            .ok_or(DatabaseError::InvalidCount)?;
        transaction.execute(
            "INSERT INTO scope_root_revocation_receipts( \
                 scope_id, from_revision, to_revision, affected_location_count, \
                 affected_node_count, exclusions_removed, purged_row_count, created_at_unix_ms \
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                binding.scope_id,
                binding.revision,
                next_revision,
                to_i64(impact.conservative_location_count)?,
                to_i64(impact.conservative_node_count)?,
                to_i64(exclusions_removed)?,
                to_i64(purged_row_count)?,
                now_unix_ms,
            ],
        )?;
        let receipt_id = transaction.last_insert_rowid();
        let consumed = transaction.execute(
            "DELETE FROM privacy_purge_capabilities WHERE nonce=?1 AND scope_id=?2",
            params![nonce, binding.scope_id],
        )?;
        if consumed != 1 {
            return Err(DatabaseError::InvalidStoredValue);
        }
        transaction.commit()?;
        Ok(ScopeRootRevocationApplyResult {
            policy: ScopePolicyRevision {
                scope_id: binding.scope_id,
                revision: next_revision,
            },
            receipt: ScopeRootRevocationReceipt {
                id: receipt_id,
                scope_id: binding.scope_id,
                from_revision: binding.revision,
                to_revision: next_revision,
                affected_location_count: impact.conservative_location_count,
                affected_node_count: impact.conservative_node_count,
                exclusions_removed,
                purged_row_count,
                created_at_unix_ms: now_unix_ms,
            },
            purged: impact,
        })
    }

    /// Stores or replaces a platform-owned grant for an existing scope.
    ///
    /// This is one SQLite upsert, so a failed write cannot leave a partially
    /// updated grant. A successful upsert is the only database operation that
    /// can transition a stored grant to `active`.
    pub fn upsert_scope_access_grant(
        &self,
        scope_id: i64,
        platform: &str,
        opaque_grant: &[u8],
    ) -> Result<ScopeAccessGrant, DatabaseError> {
        self.scope_record(scope_id)?;
        validate_scope_access_grant_platform(platform)?;
        validate_scope_access_grant_bytes(opaque_grant)?;
        let scope_platform = self.connection.query_row(
            "SELECT platform FROM authorized_scopes WHERE id = ?1",
            [scope_id],
            |row| row.get::<_, String>(0),
        )?;
        if scope_platform != platform {
            return Err(DatabaseError::ScopeAccessGrantInputInvalid);
        }
        let updated_at_unix_ms = unix_ms()?;

        self.connection.execute(
            "INSERT INTO scope_access_grants( \
                 scope_id, platform, opaque_grant, state, updated_at_unix_ms \
             ) VALUES (?1, ?2, ?3, 'active', ?4) \
             ON CONFLICT(scope_id) DO UPDATE SET \
                 platform = excluded.platform, \
                 opaque_grant = excluded.opaque_grant, \
                 state = 'active', \
                 updated_at_unix_ms = excluded.updated_at_unix_ms",
            params![scope_id, platform, opaque_grant, updated_at_unix_ms],
        )?;

        self.scope_access_grant(scope_id)?
            .ok_or(DatabaseError::ScopeAccessGrantNotFound)
    }

    /// Returns the opaque record only to Rust backend code. General scope list
    /// APIs deliberately do not expose this capability material.
    pub fn scope_access_grant(
        &self,
        scope_id: i64,
    ) -> Result<Option<ScopeAccessGrant>, DatabaseError> {
        self.scope_record(scope_id)?;
        self.connection
            .query_row(
                "SELECT grant.scope_id, grant.platform, grant.opaque_grant, grant.state, \
                        grant.updated_at_unix_ms \
                 FROM scope_access_grants grant \
                 JOIN authorized_scopes scope \
                   ON scope.id = grant.scope_id AND scope.platform = grant.platform \
                 WHERE grant.scope_id = ?1",
                [scope_id],
                scope_access_grant_from_row,
            )
            .optional()
            .map_err(Into::into)
    }

    /// A missing grant deliberately reports `needs_reauthorization`; migration
    /// never upgrades legacy scopes to active access.
    pub fn scope_access_grant_state(
        &self,
        scope_id: i64,
    ) -> Result<ScopeAccessGrantState, DatabaseError> {
        self.scope_record(scope_id)?;
        let state = self
            .connection
            .query_row(
                "SELECT grant.state FROM scope_access_grants grant \
                 JOIN authorized_scopes scope \
                   ON scope.id = grant.scope_id AND scope.platform = grant.platform \
                 WHERE grant.scope_id = ?1",
                [scope_id],
                |row| row.get::<_, String>(0),
            )
            .optional()?;
        match state {
            Some(state) => ScopeAccessGrantState::from_db(&state),
            None => Ok(ScopeAccessGrantState::NeedsReauthorization),
        }
    }

    /// Returns only active scope identifiers for backend restoration. No path,
    /// grant bytes, platform string, or state leaks through this helper.
    pub fn active_scope_access_grant_ids(&self) -> Result<Vec<i64>, DatabaseError> {
        let mut statement = self.connection.prepare(
            "SELECT grant.scope_id FROM scope_access_grants grant \
             JOIN authorized_scopes scope \
               ON scope.id = grant.scope_id AND scope.platform = grant.platform \
             WHERE grant.state = 'active' \
               AND scope.platform = ?1 AND grant.platform = ?1 \
             ORDER BY grant.scope_id ASC",
        )?;
        let scope_ids = statement.query_map([std::env::consts::OS], |row| row.get(0))?;
        scope_ids.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    }

    /// Restores the active grant for one already-authorized scope. A missing,
    /// revoked, or reauthorization-required record is never returned as a
    /// usable capability.
    pub fn active_scope_grant(&self, scope_id: i64) -> Result<ScopeAccessGrant, DatabaseError> {
        let scope = self.scope_record(scope_id)?;
        let grant = self
            .scope_access_grant(scope_id)?
            .ok_or(DatabaseError::ScopeAccessGrantNotFound)?;
        if grant.state != ScopeAccessGrantState::Active
            || scope.platform != std::env::consts::OS
            || grant.platform != std::env::consts::OS
        {
            return Err(DatabaseError::ScopeAccessGrantNotActive);
        }
        Ok(grant)
    }

    /// Backend-only restoration inventory. General scope listings deliberately
    /// remain opaque-grant-free.
    pub fn list_active_scope_grants(&self) -> Result<Vec<ScopeAccessGrant>, DatabaseError> {
        let mut statement = self.connection.prepare(
            "SELECT grant.scope_id, grant.platform, grant.opaque_grant, grant.state, \
                    grant.updated_at_unix_ms \
             FROM scope_access_grants grant \
             JOIN authorized_scopes scope \
               ON scope.id = grant.scope_id AND scope.platform = grant.platform \
             WHERE grant.state = 'active' \
               AND scope.platform = ?1 AND grant.platform = ?1 \
             ORDER BY grant.scope_id ASC",
        )?;
        let grants = statement.query_map([std::env::consts::OS], scope_access_grant_from_row)?;
        grants.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    }

    pub fn scope_has_active_access_grant(&self, scope_id: i64) -> Result<bool, DatabaseError> {
        self.scope_record(scope_id)?;
        self.connection
            .query_row(
                "SELECT EXISTS( \
                     SELECT 1 FROM scope_access_grants grant \
                     JOIN authorized_scopes scope \
                       ON scope.id = grant.scope_id AND scope.platform = grant.platform \
                     WHERE grant.scope_id = ?1 AND grant.state = 'active' \
                       AND scope.platform = ?2 AND grant.platform = ?2 \
                 )",
                params![scope_id, std::env::consts::OS],
                |row| row.get::<_, i64>(0).map(|value| value == 1),
            )
            .map_err(Into::into)
    }

    pub fn mark_scope_access_grant_needs_reauthorization(
        &self,
        scope_id: i64,
    ) -> Result<(), DatabaseError> {
        self.mark_scope_access_grant_state(scope_id, ScopeAccessGrantState::NeedsReauthorization)
    }

    pub fn mark_scope_access_grant_revoked(&self, scope_id: i64) -> Result<(), DatabaseError> {
        let binding = self.bind_scope_policy_revision(scope_id)?;
        self.apply_scope_root_revocation(binding, unix_ms()?)?;
        Ok(())
    }

    fn mark_scope_access_grant_state(
        &self,
        scope_id: i64,
        state: ScopeAccessGrantState,
    ) -> Result<(), DatabaseError> {
        self.scope_record(scope_id)?;
        let updated = self.connection.execute(
            "UPDATE scope_access_grants SET state = ?2, updated_at_unix_ms = ?3 \
             WHERE scope_id = ?1",
            params![scope_id, state.as_str(), unix_ms()?],
        )?;
        if updated == 0 {
            return Err(DatabaseError::ScopeAccessGrantNotFound);
        }
        Ok(())
    }

    pub fn create_scan_job_with_policy(
        &self,
        binding: ScopeRevisionBinding,
    ) -> Result<i64, DatabaseError> {
        let transaction = self.connection.unchecked_transaction()?;
        assert_scope_revision_binding_in_transaction(&transaction, binding)?;
        transaction.execute(
            "INSERT INTO scan_jobs(scope_id, status, started_at_unix_ms, policy_revision) \
             VALUES (?1, 'running', ?2, ?3)",
            params![binding.scope_id, unix_ms()?, binding.revision],
        )?;
        transaction.commit()?;
        Ok(self.connection.last_insert_rowid())
    }

    #[cfg(test)]
    pub fn create_scan_job(&self, scope_id: i64) -> Result<i64, DatabaseError> {
        self.create_scan_job_with_policy(test_revision_binding(self, scope_id)?)
    }

    pub fn fail_scan(&self, job_id: i64, issue_count: u64) -> Result<(), DatabaseError> {
        self.connection.execute(
            "UPDATE scan_jobs SET status = 'failed', issue_count = ?2, finished_at_unix_ms = ?3\
             WHERE id = ?1 AND status = 'running'",
            params![job_id, to_i64(issue_count)?, unix_ms()?],
        )?;
        Ok(())
    }

    pub fn complete_scan(
        &mut self,
        job_id: i64,
        scope_id: i64,
        observations: &[Observation],
        issues: &[ScanIssue],
        skipped_entries: u64,
        elapsed_ms: u64,
    ) -> Result<ScanReport, DatabaseError> {
        let discovered_files = observations
            .iter()
            .filter(|entry| entry.kind == NodeKind::File)
            .count() as u64;
        let discovered_folders = observations
            .iter()
            .filter(|entry| entry.kind == NodeKind::Folder)
            .count() as u64;
        let finished_at = unix_ms()?;
        let transaction = self.connection.transaction()?;

        for observation in observations {
            upsert_observation(&transaction, scope_id, job_id, observation, finished_at)?;
        }

        transaction.execute(
            "UPDATE locations SET present = 0 WHERE scope_id = ?1 AND last_seen_scan_id <> ?2",
            params![scope_id, job_id],
        )?;
        transaction.execute(
            "UPDATE edges SET active = 0 WHERE scope_id = ?1 AND last_seen_scan_id <> ?2",
            params![scope_id, job_id],
        )?;
        invalidate_stale_extraction_outputs(&transaction, scope_id)?;

        for issue in issues {
            transaction.execute(
                "INSERT INTO scan_issues(scan_id, code, path_key, detail_code) VALUES (?1, ?2, ?3, ?4)",
                params![job_id, issue.code, issue.path_key, issue.detail_code],
            )?;
        }

        transaction.execute(
            "UPDATE scan_jobs SET status = 'completed', discovered_files = ?2, discovered_folders = ?3,\
                skipped_entries = ?4, issue_count = ?5, finished_at_unix_ms = ?6\
             WHERE id = ?1 AND scope_id = ?7 AND status = 'running'",
            params![
                job_id,
                to_i64(discovered_files)?,
                to_i64(discovered_folders)?,
                to_i64(skipped_entries)?,
                to_i64(issues.len() as u64)?,
                finished_at,
                scope_id,
            ],
        )?;
        transaction.commit()?;

        Ok(ScanReport {
            api_version: ScanReport::API_VERSION,
            job_id,
            scope_id,
            status: ScanStatus::Completed,
            discovered_files,
            discovered_folders,
            skipped_entries,
            issue_count: issues.len() as u64,
            elapsed_ms,
        })
    }

    pub fn create_resumable_scan_job_with_policy(
        &mut self,
        binding: ScopeRevisionBinding,
        root: &QueuedPath,
    ) -> Result<ScanJobProgress, DatabaseError> {
        let now = unix_ms()?;
        let transaction = self.connection.transaction()?;
        assert_scope_revision_binding_in_transaction(&transaction, binding)?;
        assert_scope_path_key_allowed(&transaction, binding.scope_id, &root.path_key)?;
        let job_id = insert_resumable_scan_job(&transaction, binding, root, now)?;
        transaction.commit()?;
        self.scan_job(job_id)
    }

    #[cfg(test)]
    pub fn create_resumable_scan_job(
        &mut self,
        scope_id: i64,
        root: &QueuedPath,
    ) -> Result<ScanJobProgress, DatabaseError> {
        let binding = test_revision_binding(self, scope_id)?;
        self.create_resumable_scan_job_with_policy(binding, root)
    }

    pub fn scan_job(&self, job_id: i64) -> Result<ScanJobProgress, DatabaseError> {
        self.connection
            .query_row(
                "SELECT id, scope_id, status, control_state, queued_entries, processed_entries, \
                    discovered_files, discovered_folders, skipped_entries, issue_count, elapsed_ms, pause_requested \
                 FROM scan_jobs WHERE id = ?1",
                [job_id],
                scan_job_from_row,
            )
            .optional()?
            .ok_or(DatabaseError::ScanJobNotFound)
    }

    pub fn recent_scan_jobs(&self) -> Result<Vec<ScanJobProgress>, DatabaseError> {
        let mut statement = self.connection.prepare(
            "SELECT id, scope_id, status, control_state, queued_entries, processed_entries, \
                discovered_files, discovered_folders, skipped_entries, issue_count, elapsed_ms, pause_requested \
             FROM scan_jobs ORDER BY id DESC LIMIT 20",
        )?;
        let jobs = statement.query_map([], scan_job_from_row)?;
        jobs.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    }

    pub fn record_watch_observation_with_policy_at(
        &mut self,
        binding: ScopeRevisionBinding,
        observation: WatchObservationWrite<'_>,
    ) -> Result<WatchEventRecord, DatabaseError> {
        validate_watch_observation(&observation)?;
        if binding.scope_id != observation.scope_id {
            return Err(DatabaseError::ScopePolicyRevisionStale);
        }
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        assert_scope_revision_binding_in_transaction(&transaction, binding)?;
        assert_scope_path_key_allowed(&transaction, observation.scope_id, observation.path_key)?;
        let completed_scan_exists: i64 = transaction.query_row(
            "SELECT EXISTS( \
                SELECT 1 FROM scan_jobs \
                WHERE scope_id = ?1 AND status = 'completed' \
             )",
            [observation.scope_id],
            |row| row.get(0),
        )?;
        if completed_scan_exists != 1 {
            return Err(DatabaseError::WatchScopeInitialScanRequired);
        }
        let (status, reason) = if let Some(reason) = observation.ignored_reason {
            ("ignored", Some(watch_reason_as_str(reason)))
        } else {
            ("stabilizing", None)
        };
        let size_bytes = observation.snapshot.size_bytes.map(to_i64).transpose()?;
        let event_id = if let Some(reason) = observation.ignored_reason {
            let reason = watch_reason_as_str(reason);
            let existing_ignored = transaction
                .query_row(
                    "SELECT id FROM watch_events \
                     WHERE scope_id = ?1 AND status = 'ignored' AND reason = ?2 \
                     ORDER BY id DESC LIMIT 1",
                    params![observation.scope_id, reason],
                    |row| row.get::<_, i64>(0),
                )
                .optional()?;
            match existing_ignored {
                Some(ignored_id) => {
                    transaction.execute(
                        "UPDATE watch_events SET path_raw = ?2, path_key = ?3, \
                            observed_kind = ?4, observed_size_bytes = ?5, \
                            observed_modified_unix_ns = ?6, observed_identity_key = ?7, \
                            observation_count = observation_count + 1, \
                            stable_after_unix_ms = ?8, updated_at_unix_ms = ?9 \
                         WHERE id = ?1 AND status = 'ignored' AND reason = ?10",
                        params![
                            ignored_id,
                            observation.path_raw,
                            observation.path_key,
                            observation.snapshot.kind.as_str(),
                            size_bytes,
                            observation.snapshot.modified_unix_ns,
                            observation.snapshot.identity_key,
                            observation.stable_after_unix_ms,
                            observation.observed_at_unix_ms,
                            reason,
                        ],
                    )?;
                    ignored_id
                }
                None => insert_watch_event(
                    &transaction,
                    observation,
                    status,
                    Some(reason),
                    size_bytes,
                    WatchReconciliationKind::FullScope,
                )?,
            }
        } else {
            let existing = transaction
                .query_row(
                    "SELECT id, path_raw, path_key, reconciliation_kind, observed_kind, \
                        observed_identity_key \
                     FROM watch_events WHERE scope_id = ?1 AND status = 'stabilizing'",
                    [observation.scope_id],
                    |row| {
                        Ok((
                            row.get::<_, i64>(0)?,
                            row.get::<_, Vec<u8>>(1)?,
                            row.get::<_, String>(2)?,
                            row.get::<_, String>(3)?,
                            row.get::<_, String>(4)?,
                            row.get::<_, Option<Vec<u8>>>(5)?,
                        ))
                    },
                )
                .optional()?;
            if let Some((
                event_id,
                existing_path_raw,
                existing_path_key,
                existing_kind,
                existing_observed_kind,
                existing_identity_key,
            )) = existing
            {
                let existing_kind = WatchReconciliationKind::from_db(&existing_kind)?;
                let reconciliation_kind = if existing_kind == WatchReconciliationKind::FullScope
                    || observation.reconciliation_kind == WatchReconciliationKind::FullScope
                    || existing_path_raw != observation.path_raw
                    || existing_path_key != observation.path_key
                    || existing_observed_kind != WatchSnapshotKind::File.as_str()
                    || observation.snapshot.kind != WatchSnapshotKind::File
                    || existing_identity_key.as_deref()
                        != observation.snapshot.identity_key.as_deref()
                {
                    WatchReconciliationKind::FullScope
                } else {
                    WatchReconciliationKind::FileDelta
                };
                transaction.execute(
                    "UPDATE watch_events SET path_raw = ?2, path_key = ?3, observed_kind = ?4, \
                        observed_size_bytes = ?5, observed_modified_unix_ns = ?6, \
                        observed_identity_key = ?7, observation_count = observation_count + 1, \
                        stable_after_unix_ms = ?8, reconciliation_kind = ?9, updated_at_unix_ms = ?10 \
                     WHERE id = ?1 AND status = 'stabilizing'",
                    params![
                        event_id,
                        observation.path_raw,
                        observation.path_key,
                        observation.snapshot.kind.as_str(),
                        size_bytes,
                        observation.snapshot.modified_unix_ns,
                        observation.snapshot.identity_key,
                        observation.stable_after_unix_ms,
                        reconciliation_kind.as_str(),
                        observation.observed_at_unix_ms,
                    ],
                )?;
                event_id
            } else {
                insert_watch_event(
                    &transaction,
                    observation,
                    status,
                    reason,
                    size_bytes,
                    observation.reconciliation_kind,
                )?
            }
        };
        transaction.commit()?;
        self.watch_event(event_id)
    }

    #[cfg(test)]
    pub fn record_watch_observation_at(
        &mut self,
        observation: WatchObservationWrite<'_>,
    ) -> Result<WatchEventRecord, DatabaseError> {
        let binding = test_revision_binding(self, observation.scope_id)?;
        self.record_watch_observation_with_policy_at(binding, observation)
    }

    pub fn watch_event(&self, event_id: i64) -> Result<WatchEventRecord, DatabaseError> {
        self.connection
            .query_row(
                "SELECT id, scope_id, status, observation_count, stable_after_unix_ms, \
                    scan_job_id, reason, path_raw, path_key, observed_kind, observed_size_bytes, \
                    observed_modified_unix_ns, observed_identity_key, reconciliation_kind \
                 FROM watch_events WHERE id = ?1",
                [event_id],
                watch_event_from_row,
            )
            .optional()?
            .ok_or(DatabaseError::WatchEventNotFound)
    }

    pub fn watch_event_created_at(&self, event_id: i64) -> Result<i64, DatabaseError> {
        let created_at = self
            .connection
            .query_row(
                "SELECT created_at_unix_ms FROM watch_events WHERE id = ?1",
                [event_id],
                |row| row.get::<_, i64>(0),
            )
            .optional()?
            .ok_or(DatabaseError::WatchEventNotFound)?;
        if created_at < 0 {
            return Err(DatabaseError::InvalidStoredValue);
        }
        Ok(created_at)
    }

    pub fn recent_watch_events(&self) -> Result<Vec<WatchEventProgress>, DatabaseError> {
        let mut statement = self.connection.prepare(
            "SELECT id, scope_id, status, observation_count, stable_after_unix_ms, \
                scan_job_id, reason, path_raw, path_key, observed_kind, observed_size_bytes, \
                observed_modified_unix_ns, observed_identity_key, reconciliation_kind \
             FROM watch_events ORDER BY updated_at_unix_ms DESC, id DESC LIMIT 20",
        )?;
        let events = statement.query_map([], watch_event_from_row)?;
        events
            .map(|event| event.map(|event| event.progress))
            .collect::<Result<Vec<_>, _>>()
            .map_err(Into::into)
    }

    pub fn active_watch_events(&self) -> Result<Vec<WatchEventProgress>, DatabaseError> {
        let mut statement = self.connection.prepare(
            "SELECT id, scope_id, status, observation_count, stable_after_unix_ms, \
                scan_job_id, reason \
             FROM watch_events \
             WHERE status IN ('stabilizing', 'reconciling') \
             ORDER BY CASE status WHEN 'reconciling' THEN 0 ELSE 1 END, \
                 stable_after_unix_ms ASC, id ASC",
        )?;
        let events = statement.query_map([], watch_event_progress_from_row)?;
        events.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    }

    pub fn watchable_scope_ids(&self) -> Result<Vec<i64>, DatabaseError> {
        let mut statement = self.connection.prepare(
            "SELECT authorized_scopes.id \
             FROM authorized_scopes \
             WHERE EXISTS ( \
                SELECT 1 FROM scan_jobs \
                WHERE scan_jobs.scope_id = authorized_scopes.id \
                    AND scan_jobs.status = 'completed' \
             ) \
             ORDER BY authorized_scopes.id ASC",
        )?;
        let scope_ids = statement.query_map([], |row| row.get(0))?;
        scope_ids.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    }

    /// Desktop-only eligibility boundary: a completed initial scan is not
    /// sufficient when the current process has not restored a platform-owned
    /// access grant. CLI and deterministic core fixtures continue to use
    /// `watchable_scope_ids`; the packaged Desktop uses this stricter query.
    pub fn watchable_scope_ids_with_active_access_grants(&self) -> Result<Vec<i64>, DatabaseError> {
        let mut statement = self.connection.prepare(
            "SELECT authorized_scopes.id \
             FROM authorized_scopes \
             JOIN scope_access_grants grant \
               ON grant.scope_id = authorized_scopes.id \
              AND grant.platform = authorized_scopes.platform \
              AND grant.state = 'active' \
             WHERE authorized_scopes.platform = ?1 AND grant.platform = ?1 \
               AND EXISTS ( \
                SELECT 1 FROM scan_jobs \
                WHERE scan_jobs.scope_id = authorized_scopes.id \
                    AND scan_jobs.status = 'completed' \
             ) \
             ORDER BY authorized_scopes.id ASC",
        )?;
        let scope_ids = statement.query_map([std::env::consts::OS], |row| row.get(0))?;
        scope_ids.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    }

    pub fn scope_has_completed_scan(&self, scope_id: i64) -> Result<bool, DatabaseError> {
        self.scope_record(scope_id)?;
        self.connection
            .query_row(
                "SELECT EXISTS( \
                    SELECT 1 FROM scan_jobs job \
                    WHERE job.scope_id = ?1 AND job.status = 'completed' \
                 )",
                [scope_id],
                |row| row.get::<_, i64>(0).map(|value| value == 1),
            )
            .map_err(Into::into)
    }

    /// Durably requests the existing full-scope metadata reconciliation path.
    ///
    /// The request is monotonic: an existing `full_scope` event can never be
    /// downgraded by a later file-delta hint, and an existing stabilizing event
    /// is made due immediately without replacing its local path snapshot.
    pub fn request_scope_full_reconciliation_at(
        &mut self,
        scope_id: i64,
        now_unix_ms: i64,
    ) -> Result<WatchEventProgress, DatabaseError> {
        if now_unix_ms < 0 {
            return Err(DatabaseError::WatchInputInvalid);
        }
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let event_id =
            request_scope_full_reconciliation_in_transaction(&transaction, scope_id, now_unix_ms)?;
        transaction.commit()?;
        Ok(self.watch_event(event_id)?.progress)
    }

    /// Applies a durable immediate full-scope request to every currently
    /// watchable scope. A single immediate transaction guarantees that a
    /// process crash or source failure cannot leave only some scopes upgraded.
    pub fn request_all_scope_full_reconciliation_at(
        &mut self,
        now_unix_ms: i64,
    ) -> Result<Vec<WatchEventProgress>, DatabaseError> {
        if now_unix_ms < 0 {
            return Err(DatabaseError::WatchInputInvalid);
        }
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let scope_ids = watchable_scope_ids_in_transaction(&transaction)?;
        let mut event_ids = Vec::with_capacity(scope_ids.len());
        for scope_id in scope_ids {
            event_ids.push(request_scope_full_reconciliation_in_transaction(
                &transaction,
                scope_id,
                now_unix_ms,
            )?);
        }
        transaction.commit()?;
        event_ids
            .into_iter()
            .map(|event_id| self.watch_event(event_id).map(|event| event.progress))
            .collect()
    }

    /// Atomically requests reconciliation for every completed scope carrying
    /// a durable active platform grant. This core/storage helper does not prove
    /// that an OS capability is live in the current process; packaged Desktop
    /// callers must use
    /// `request_live_active_granted_scope_full_reconciliation_at` instead.
    pub fn request_all_active_granted_scope_full_reconciliation_at(
        &mut self,
        now_unix_ms: i64,
    ) -> Result<Vec<WatchEventProgress>, DatabaseError> {
        if now_unix_ms < 0 {
            return Err(DatabaseError::WatchInputInvalid);
        }
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let scope_ids = active_granted_watchable_scope_ids_in_transaction(&transaction)?;
        let mut event_ids = Vec::with_capacity(scope_ids.len());
        for scope_id in scope_ids {
            event_ids.push(request_scope_full_reconciliation_in_transaction(
                &transaction,
                scope_id,
                now_unix_ms,
            )?);
        }
        transaction.commit()?;
        event_ids
            .into_iter()
            .map(|event_id| self.watch_event(event_id).map(|event| event.progress))
            .collect()
    }

    /// Atomically requests reconciliation for the caller's exact live-runtime
    /// scope set, while revalidating completed-scan and active-grant state in
    /// the same transaction. The packaged Desktop passes only scopes whose OS
    /// capability guard is currently held; durable `active` state alone is
    /// never enough to create an event through this entry point.
    pub fn request_live_active_granted_scope_full_reconciliation_at(
        &mut self,
        live_scope_ids: &[i64],
        now_unix_ms: i64,
    ) -> Result<Vec<WatchEventProgress>, DatabaseError> {
        if now_unix_ms < 0 || live_scope_ids.iter().any(|scope_id| *scope_id <= 0) {
            return Err(DatabaseError::WatchInputInvalid);
        }
        let live_scope_ids = live_scope_ids
            .iter()
            .copied()
            .collect::<std::collections::BTreeSet<_>>();
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let eligible_scope_ids = active_granted_watchable_scope_ids_in_transaction(&transaction)?;
        let mut event_ids = Vec::with_capacity(eligible_scope_ids.len().min(live_scope_ids.len()));
        for scope_id in eligible_scope_ids {
            if live_scope_ids.contains(&scope_id) {
                event_ids.push(request_scope_full_reconciliation_in_transaction(
                    &transaction,
                    scope_id,
                    now_unix_ms,
                )?);
            }
        }
        transaction.commit()?;
        event_ids
            .into_iter()
            .map(|event_id| self.watch_event(event_id).map(|event| event.progress))
            .collect()
    }

    /// Returns a narrow, immutable manifest binding for a stable file-delta
    /// event. `None` is a safe fallback signal: callers must use full-scope
    /// reconciliation rather than widening this fast path.
    pub fn watch_file_delta_binding_at(
        &self,
        event_id: i64,
        parent_path_raw: &[u8],
        parent_path_key: &str,
        now_unix_ms: i64,
    ) -> Result<Option<WatchFileDeltaBinding>, DatabaseError> {
        if now_unix_ms < 0 {
            return Err(DatabaseError::WatchInputInvalid);
        }
        let event = self.watch_event(event_id)?;
        if event.progress.status != WatchEventStatus::Stabilizing
            || event.reconciliation_kind != WatchReconciliationKind::FileDelta
            || event.progress.scan_job_id.is_some()
            || event.progress.stable_after_unix_ms > now_unix_ms
            || event.snapshot.kind != WatchSnapshotKind::File
        {
            return Ok(None);
        }
        let Some(expected_identity_key) = event.snapshot.identity_key.as_deref() else {
            return Ok(None);
        };
        if parent_path_raw.is_empty()
            || parent_path_raw.len() > MAX_WATCH_PATH_BYTES
            || parent_path_key.is_empty()
            || parent_path_key.len() > MAX_WATCH_PATH_BYTES
        {
            return Ok(None);
        }
        let binding = self
            .connection
            .query_row(
                "SELECT locations.id, locations.node_id, nodes.identity_kind, nodes.identity_key, \
                    files.size_bytes, files.modified_unix_ns, files.link_count, \
                    root_locations.id, root_locations.node_id, root_nodes.identity_key, \
                    parent_locations.id, parent_locations.node_id, parent_locations.path_raw, \
                    parent_locations.path_key, parent_nodes.identity_key \
                 FROM locations \
                 JOIN authorized_scopes ON authorized_scopes.id = locations.scope_id \
                 JOIN nodes ON nodes.id = locations.node_id AND nodes.kind = 'file' \
                    AND nodes.identity_kind = 'unix_device_inode' \
                 JOIN files ON files.node_id = nodes.id \
                 JOIN locations root_locations ON root_locations.scope_id = locations.scope_id \
                    AND root_locations.path_raw = authorized_scopes.path_raw \
                    AND root_locations.path_key = authorized_scopes.path_key \
                    AND root_locations.present = 1 \
                 JOIN nodes root_nodes ON root_nodes.id = root_locations.node_id \
                    AND root_nodes.kind = 'folder' AND root_nodes.identity_kind = 'unix_device_inode' \
                 JOIN locations parent_locations ON parent_locations.scope_id = locations.scope_id \
                    AND parent_locations.path_raw = ?1 AND parent_locations.path_key = ?2 \
                    AND parent_locations.present = 1 \
                 JOIN nodes parent_nodes ON parent_nodes.id = parent_locations.node_id \
                    AND parent_nodes.kind = 'folder' \
                    AND parent_nodes.identity_kind = 'unix_device_inode' \
                 JOIN edges ON edges.scope_id = locations.scope_id \
                    AND edges.source_node_id = locations.node_id \
                    AND edges.target_node_id = parent_locations.node_id \
                    AND edges.kind = 'located_in' AND edges.active = 1 \
                 WHERE locations.scope_id = ?3 AND locations.path_raw = ?4 \
                    AND locations.path_key = ?5 AND locations.present = 1 \
                    AND NOT EXISTS ( \
                        SELECT 1 FROM scan_jobs \
                        WHERE scan_jobs.scope_id = locations.scope_id \
                            AND scan_jobs.status IN ('running', 'interrupted') \
                    )",
                params![
                    parent_path_raw,
                    parent_path_key,
                    event.progress.scope_id,
                    event.path_raw,
                    event.path_key
                ],
                |row| {
                    Ok((
                        row.get::<_, i64>(0)?,
                        row.get::<_, i64>(1)?,
                        row.get::<_, String>(2)?,
                        row.get::<_, Vec<u8>>(3)?,
                        row.get::<_, i64>(4)?,
                        row.get::<_, Option<i64>>(5)?,
                        row.get::<_, Option<i64>>(6)?,
                        row.get::<_, i64>(7)?,
                        row.get::<_, i64>(8)?,
                        row.get::<_, Vec<u8>>(9)?,
                        row.get::<_, i64>(10)?,
                        row.get::<_, i64>(11)?,
                        row.get::<_, Vec<u8>>(12)?,
                        row.get::<_, String>(13)?,
                        row.get::<_, Vec<u8>>(14)?,
                    ))
                },
            )
            .optional()?;
        let Some((
            location_id,
            node_id,
            identity_kind,
            identity_key,
            old_size_bytes,
            old_modified_unix_ns,
            link_count,
            root_location_id,
            root_node_id,
            root_identity_key,
            parent_location_id,
            parent_node_id,
            bound_parent_path_raw,
            bound_parent_path_key,
            parent_identity_key,
        )) = binding
        else {
            return Ok(None);
        };
        if link_count != Some(1) || identity_key.as_slice() != expected_identity_key {
            return Ok(None);
        }
        let present_location_count: i64 = self.connection.query_row(
            "SELECT COUNT(*) FROM locations WHERE node_id = ?1 AND present = 1",
            [node_id],
            |row| row.get(0),
        )?;
        if present_location_count != 1 {
            return Ok(None);
        }
        let old_size_bytes =
            u64::try_from(old_size_bytes).map_err(|_| DatabaseError::InvalidStoredValue)?;
        Ok(Some(WatchFileDeltaBinding {
            event_id,
            scope_id: event.progress.scope_id,
            path_raw: event.path_raw,
            path_key: event.path_key,
            stable_after_unix_ms: event.progress.stable_after_unix_ms,
            snapshot: event.snapshot,
            node_id,
            location_id,
            root_location_id,
            root_node_id,
            root_identity_key,
            parent_location_id,
            parent_node_id,
            parent_path_raw: bound_parent_path_raw,
            parent_path_key: bound_parent_path_key,
            parent_identity_key,
            identity_kind,
            identity_key,
            old_size_bytes,
            old_modified_unix_ns,
        }))
    }

    /// Atomically publishes a bound same-identity regular-file metadata delta.
    /// Any failed compare-and-swap rolls the whole transaction back, including
    /// content invalidation and the terminal event transition.
    pub fn publish_watch_file_delta_at(
        &mut self,
        binding: &WatchFileDeltaBinding,
        write: &WatchFileDeltaWrite,
        published_at_unix_ms: i64,
    ) -> Result<WatchEventProgress, DatabaseError> {
        validate_watch_file_delta_write(binding, write, published_at_unix_ms)?;
        let size_bytes = write
            .snapshot
            .size_bytes
            .map(to_i64)
            .transpose()?
            .ok_or(DatabaseError::WatchFileDeltaNotEligible)?;
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let policy_revision = transaction
            .query_row(
                "SELECT policy_revision FROM watch_events WHERE id=?1 AND scope_id=?2",
                params![binding.event_id, binding.scope_id],
                |row| row.get::<_, i64>(0),
            )
            .optional()?
            .ok_or(DatabaseError::WatchEventNotFound)?;
        assert_scope_revision_binding_in_transaction(
            &transaction,
            ScopeRevisionBinding {
                scope_id: binding.scope_id,
                revision: policy_revision,
            },
        )?;
        assert_scope_path_key_allowed(&transaction, binding.scope_id, &binding.path_key)?;
        let event_changed = transaction.execute(
            "UPDATE watch_events SET status = 'completed', scan_job_id = NULL, reason = NULL, \
                updated_at_unix_ms = ?2 \
             WHERE id = ?1 AND scope_id = ?3 AND status = 'stabilizing' \
                AND reconciliation_kind = 'file_delta' AND scan_job_id IS NULL \
                AND stable_after_unix_ms = ?4 AND path_raw = ?5 AND path_key = ?6 \
                AND observed_kind = 'file' AND observed_size_bytes = ?7 \
                AND observed_modified_unix_ns IS ?8 AND observed_identity_key = ?9",
            params![
                binding.event_id,
                published_at_unix_ms,
                binding.scope_id,
                binding.stable_after_unix_ms,
                binding.path_raw,
                binding.path_key,
                size_bytes,
                write.snapshot.modified_unix_ns,
                write.snapshot.identity_key,
            ],
        )?;
        if event_changed != 1 {
            return Err(DatabaseError::WatchFileDeltaSnapshotChanged);
        }
        let binding_current: i64 = transaction.query_row(
            "SELECT EXISTS( \
                SELECT 1 FROM locations \
                JOIN nodes ON nodes.id = locations.node_id AND nodes.kind = 'file' \
                JOIN files ON files.node_id = nodes.id \
                JOIN authorized_scopes ON authorized_scopes.id = locations.scope_id \
                JOIN locations root_locations ON root_locations.id = ?1 \
                    AND root_locations.scope_id = locations.scope_id \
                    AND root_locations.path_raw = authorized_scopes.path_raw \
                    AND root_locations.path_key = authorized_scopes.path_key \
                    AND root_locations.present = 1 AND root_locations.node_id = ?3 \
                JOIN nodes root_nodes ON root_nodes.id = root_locations.node_id \
                    AND root_nodes.kind = 'folder' AND root_nodes.identity_kind = 'unix_device_inode' \
                    AND root_nodes.identity_key = ?2 \
                JOIN locations parent_locations ON parent_locations.id = ?4 \
                    AND parent_locations.scope_id = locations.scope_id \
                    AND parent_locations.present = 1 \
                    AND parent_locations.node_id = ?5 \
                    AND parent_locations.path_raw = ?6 AND parent_locations.path_key = ?7 \
                JOIN nodes parent_nodes ON parent_nodes.id = parent_locations.node_id \
                    AND parent_nodes.kind = 'folder' \
                    AND parent_nodes.identity_kind = 'unix_device_inode' \
                    AND parent_nodes.identity_key = ?8 \
                JOIN edges ON edges.scope_id = locations.scope_id \
                    AND edges.source_node_id = locations.node_id \
                    AND edges.target_node_id = parent_locations.node_id \
                    AND edges.kind = 'located_in' AND edges.active = 1 \
                WHERE locations.id = ?9 AND locations.scope_id = ?10 AND locations.node_id = ?11 \
                    AND locations.path_raw = ?12 AND locations.path_key = ?13 AND locations.present = 1 \
                    AND nodes.identity_kind = ?14 AND nodes.identity_key = ?15 \
                    AND files.size_bytes = ?16 AND files.modified_unix_ns IS ?17 \
                    AND files.link_count = 1 \
                    AND (SELECT COUNT(*) FROM locations all_locations \
                         WHERE all_locations.node_id = locations.node_id AND all_locations.present = 1) = 1 \
                    AND NOT EXISTS ( \
                        SELECT 1 FROM scan_jobs \
                        WHERE scan_jobs.scope_id = locations.scope_id \
                            AND scan_jobs.status IN ('running', 'interrupted') \
                    ) \
             )",
            params![
                binding.root_location_id,
                binding.root_identity_key,
                binding.root_node_id,
                binding.parent_location_id,
                binding.parent_node_id,
                binding.parent_path_raw,
                binding.parent_path_key,
                binding.parent_identity_key,
                binding.location_id,
                binding.scope_id,
                binding.node_id,
                binding.path_raw,
                binding.path_key,
                binding.identity_kind,
                binding.identity_key,
                to_i64(binding.old_size_bytes)?,
                binding.old_modified_unix_ns,
            ],
            |row| row.get(0),
        )?;
        if binding_current != 1 {
            return Err(DatabaseError::WatchFileDeltaSnapshotChanged);
        }
        let node_changed = transaction.execute(
            "UPDATE nodes SET updated_at_unix_ms = ?2 \
             WHERE id = ?1 AND kind = 'file' AND identity_kind = ?3 AND identity_key = ?4",
            params![
                binding.node_id,
                published_at_unix_ms,
                binding.identity_kind,
                binding.identity_key,
            ],
        )?;
        if node_changed != 1 {
            return Err(DatabaseError::WatchFileDeltaSnapshotChanged);
        }
        let file_changed = transaction.execute(
            "UPDATE files SET size_bytes = ?2, modified_unix_ns = ?3, link_count = 1 \
             WHERE node_id = ?1 AND size_bytes = ?4 AND modified_unix_ns IS ?5 AND link_count = 1",
            params![
                binding.node_id,
                size_bytes,
                write.snapshot.modified_unix_ns,
                to_i64(binding.old_size_bytes)?,
                binding.old_modified_unix_ns,
            ],
        )?;
        if file_changed != 1 {
            return Err(DatabaseError::WatchFileDeltaSnapshotChanged);
        }
        transaction.execute(
            "UPDATE content_chunks SET active = 0 \
             WHERE scope_id = ?1 AND node_id = ?2 AND active = 1",
            params![binding.scope_id, binding.node_id],
        )?;
        transaction.execute(
            "UPDATE image_metadata SET active = 0 \
             WHERE scope_id = ?1 AND node_id = ?2 AND active = 1",
            params![binding.scope_id, binding.node_id],
        )?;
        transaction.commit()?;
        Ok(self.watch_event(binding.event_id)?.progress)
    }

    pub fn mark_watch_event_ignored_at(
        &mut self,
        event_id: i64,
        reason: WatchEventReason,
        now_unix_ms: i64,
    ) -> Result<WatchEventProgress, DatabaseError> {
        if now_unix_ms < 0 {
            return Err(DatabaseError::WatchInputInvalid);
        }
        let reason = watch_reason_as_str(reason);
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let current = transaction
            .query_row(
                "SELECT scope_id, status, path_raw, path_key, observed_kind, \
                    observed_size_bytes, observed_modified_unix_ns, observed_identity_key, \
                    observation_count \
                 FROM watch_events WHERE id = ?1",
                [event_id],
                |row| {
                    Ok((
                        row.get::<_, i64>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, Vec<u8>>(2)?,
                        row.get::<_, String>(3)?,
                        row.get::<_, String>(4)?,
                        row.get::<_, Option<i64>>(5)?,
                        row.get::<_, Option<i64>>(6)?,
                        row.get::<_, Option<Vec<u8>>>(7)?,
                        row.get::<_, i64>(8)?,
                    ))
                },
            )
            .optional()?
            .ok_or(DatabaseError::WatchEventNotFound)?;
        if current.1 != "stabilizing" {
            return Err(DatabaseError::InvalidWatchEventState);
        }
        let existing_ignored = transaction
            .query_row(
                "SELECT id FROM watch_events \
                 WHERE scope_id = ?1 AND status = 'ignored' AND reason = ?2 AND id != ?3 \
                 ORDER BY id DESC LIMIT 1",
                params![current.0, reason, event_id],
                |row| row.get::<_, i64>(0),
            )
            .optional()?;
        let result_event_id = if let Some(ignored_id) = existing_ignored {
            transaction.execute(
                "UPDATE watch_events SET path_raw = ?2, path_key = ?3, observed_kind = ?4, \
                    observed_size_bytes = ?5, observed_modified_unix_ns = ?6, \
                    observed_identity_key = ?7, observation_count = observation_count + ?8, \
                    stable_after_unix_ms = ?9, updated_at_unix_ms = ?9 \
                 WHERE id = ?1 AND status = 'ignored' AND reason = ?10",
                params![
                    ignored_id,
                    current.2,
                    current.3,
                    current.4,
                    current.5,
                    current.6,
                    current.7,
                    current.8,
                    now_unix_ms,
                    reason,
                ],
            )?;
            let deleted = transaction.execute(
                "DELETE FROM watch_events WHERE id = ?1 AND status = 'stabilizing'",
                [event_id],
            )?;
            if deleted != 1 {
                return Err(DatabaseError::InvalidWatchEventState);
            }
            ignored_id
        } else {
            let changed = transaction.execute(
                "UPDATE watch_events SET status = 'ignored', reason = ?2, \
                    stable_after_unix_ms = ?3, updated_at_unix_ms = ?3 \
                 WHERE id = ?1 AND status = 'stabilizing'",
                params![event_id, reason, now_unix_ms],
            )?;
            if changed != 1 {
                return Err(DatabaseError::InvalidWatchEventState);
            }
            event_id
        };
        transaction.commit()?;
        Ok(self.watch_event(result_event_id)?.progress)
    }

    pub fn begin_watch_reconciliation_at(
        &mut self,
        event_id: i64,
        root: &QueuedPath,
        now_unix_ms: i64,
    ) -> Result<WatchEventProgress, DatabaseError> {
        self.begin_watch_reconciliation_with_policy_at(event_id, root, now_unix_ms, false)
    }

    /// Starts the same durable root-scan transaction as normal watch
    /// reconciliation without waiting for the path hint's debounce deadline.
    ///
    /// This is intentionally limited to the exact stored root of an authorized
    /// scope that already has a completed initial scan. Those bindings are
    /// checked again inside the same write transaction. It exists only for
    /// bounded metadata recovery after a native overflow/source change or a
    /// maximum coalescing age; it does not authorize content extraction or any
    /// filesystem mutation.
    pub fn begin_forced_watch_metadata_reconciliation_at(
        &mut self,
        event_id: i64,
        root: &QueuedPath,
        now_unix_ms: i64,
    ) -> Result<WatchEventProgress, DatabaseError> {
        if !root.is_root || root.parent_identity_key.is_some() {
            return Err(DatabaseError::WatchInputInvalid);
        }
        self.begin_watch_reconciliation_with_policy_at(event_id, root, now_unix_ms, true)
    }

    fn begin_watch_reconciliation_with_policy_at(
        &mut self,
        event_id: i64,
        root: &QueuedPath,
        now_unix_ms: i64,
        allow_before_stable_deadline: bool,
    ) -> Result<WatchEventProgress, DatabaseError> {
        if now_unix_ms < 0 || !root.is_root || root.parent_identity_key.is_some() {
            return Err(DatabaseError::WatchInputInvalid);
        }
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let (scope_id, policy_revision, status, stable_after): (i64, i64, String, i64) =
            transaction
                .query_row(
                    "SELECT scope_id, policy_revision, status, stable_after_unix_ms \
                 FROM watch_events WHERE id = ?1",
                    [event_id],
                    |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
                )
                .optional()?
                .ok_or(DatabaseError::WatchEventNotFound)?;
        let authorized_root = transaction
            .query_row(
                "SELECT path_raw, path_key FROM authorized_scopes WHERE id = ?1",
                [scope_id],
                |row| Ok((row.get::<_, Vec<u8>>(0)?, row.get::<_, String>(1)?)),
            )
            .optional()?
            .ok_or(DatabaseError::ScopeNotFound)?;
        if root.path_raw != authorized_root.0 || root.path_key != authorized_root.1 {
            return Err(DatabaseError::WatchInputInvalid);
        }
        let completed_scan_exists: i64 = transaction.query_row(
            "SELECT EXISTS( \
                SELECT 1 FROM scan_jobs \
                WHERE scope_id = ?1 AND status = 'completed' \
                  AND policy_revision = ?2 \
             )",
            params![scope_id, policy_revision],
            |row| row.get(0),
        )?;
        if completed_scan_exists != 1 {
            return Err(DatabaseError::WatchScopeInitialScanRequired);
        }
        if status != "stabilizing" || (!allow_before_stable_deadline && stable_after > now_unix_ms)
        {
            return Err(DatabaseError::InvalidWatchEventState);
        }
        assert_scope_revision_binding_in_transaction(
            &transaction,
            ScopeRevisionBinding {
                scope_id,
                revision: policy_revision,
            },
        )?;
        let scan_job_id = insert_resumable_scan_job(
            &transaction,
            ScopeRevisionBinding {
                scope_id,
                revision: policy_revision,
            },
            root,
            now_unix_ms,
        )?;
        let changed = transaction.execute(
            "UPDATE watch_events SET status = 'reconciling', scan_job_id = ?2, reason = NULL, \
                updated_at_unix_ms = ?3 WHERE id = ?1 AND status = 'stabilizing'",
            params![event_id, scan_job_id, now_unix_ms],
        )?;
        if changed != 1 {
            return Err(DatabaseError::InvalidWatchEventState);
        }
        transaction.commit()?;
        Ok(self.watch_event(event_id)?.progress)
    }

    pub fn complete_watch_reconciliation_at(
        &self,
        event_id: i64,
        now_unix_ms: i64,
    ) -> Result<WatchEventProgress, DatabaseError> {
        if now_unix_ms < 0 {
            return Err(DatabaseError::WatchInputInvalid);
        }
        let changed = self.connection.execute(
            "UPDATE watch_events SET status = 'completed', reason = NULL, updated_at_unix_ms = ?2 \
             WHERE id = ?1 AND status = 'reconciling' AND EXISTS ( \
                SELECT 1 FROM scan_jobs WHERE scan_jobs.id = watch_events.scan_job_id \
                    AND scan_jobs.status = 'completed' \
                    AND scan_jobs.policy_revision = watch_events.policy_revision \
                    AND watch_events.policy_revision = ( \
                        SELECT policy_revision FROM authorized_scopes \
                        WHERE id = watch_events.scope_id \
                    ) \
             )",
            params![event_id, now_unix_ms],
        )?;
        if changed != 1 {
            return Err(DatabaseError::InvalidWatchEventState);
        }
        Ok(self.watch_event(event_id)?.progress)
    }

    pub fn fail_watch_event_at(
        &self,
        event_id: i64,
        reason: WatchEventReason,
        now_unix_ms: i64,
    ) -> Result<WatchEventProgress, DatabaseError> {
        if now_unix_ms < 0 {
            return Err(DatabaseError::WatchInputInvalid);
        }
        let changed = self.connection.execute(
            "UPDATE watch_events SET status = 'failed', reason = ?2, \
                updated_at_unix_ms = ?3 WHERE id = ?1 AND status IN ('stabilizing', 'reconciling')",
            params![event_id, watch_reason_as_str(reason), now_unix_ms],
        )?;
        if changed != 1 {
            return Err(DatabaseError::InvalidWatchEventState);
        }
        Ok(self.watch_event(event_id)?.progress)
    }

    pub fn request_scan_pause(&mut self, job_id: i64) -> Result<ScanJobProgress, DatabaseError> {
        let now = unix_ms()?;
        let transaction = self.connection.transaction()?;
        let (status, control): (String, String) = transaction
            .query_row(
                "SELECT status, control_state FROM scan_jobs WHERE id = ?1",
                [job_id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()?
            .ok_or(DatabaseError::ScanJobNotFound)?;
        if status != "running" {
            return Err(DatabaseError::InvalidScanJobState);
        }
        match control.as_str() {
            "ready" => {
                transaction.execute(
                    "UPDATE scan_jobs SET control_state = 'paused', pause_requested = 1, updated_at_unix_ms = ?2 \
                     WHERE id = ?1",
                    params![job_id, now],
                )?;
            }
            "active" => {
                transaction.execute(
                    "UPDATE scan_jobs SET control_state = 'pause_requested', pause_requested = 1, updated_at_unix_ms = ?2 \
                     WHERE id = ?1",
                    params![job_id, now],
                )?;
            }
            "pause_requested" | "paused" => {}
            _ => return Err(DatabaseError::InvalidStoredValue),
        }
        transaction.commit()?;
        self.scan_job(job_id)
    }

    pub fn resume_scan_job(&mut self, job_id: i64) -> Result<ScanJobProgress, DatabaseError> {
        let now = unix_ms()?;
        let transaction = self.connection.transaction()?;
        let (status, control): (String, String) = transaction
            .query_row(
                "SELECT status, control_state FROM scan_jobs WHERE id = ?1",
                [job_id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()?
            .ok_or(DatabaseError::ScanJobNotFound)?;
        if matches!(status.as_str(), "completed" | "failed") || control == "active" {
            return Err(DatabaseError::InvalidScanJobState);
        }
        if !matches!(status.as_str(), "running" | "interrupted") {
            return Err(DatabaseError::InvalidStoredValue);
        }
        transaction.execute(
            "UPDATE scan_queue SET state = 'pending' WHERE scan_id = ?1 AND state = 'processing'",
            [job_id],
        )?;
        transaction.execute(
            "UPDATE scan_jobs SET status = 'running', control_state = 'ready', pause_requested = 0, \
                runner_token = NULL, lease_expires_at_unix_ms = NULL, updated_at_unix_ms = ?2 \
             WHERE id = ?1",
            params![job_id, now],
        )?;
        transaction.commit()?;
        self.scan_job(job_id)
    }

    pub fn claim_scan_job(
        &mut self,
        job_id: i64,
        runner_token: &str,
        lease_ms: i64,
    ) -> Result<ScanJobProgress, DatabaseError> {
        let now = unix_ms()?;
        self.recover_expired_scan_jobs_at(now)?;
        let lease_expires = now
            .checked_add(lease_ms)
            .ok_or(DatabaseError::InvalidTimestamp)?;
        let transaction = self.connection.transaction()?;
        let (status, control, existing_runner): (String, String, Option<String>) = transaction
            .query_row(
                "SELECT status, control_state, runner_token FROM scan_jobs WHERE id = ?1",
                [job_id],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .optional()?
            .ok_or(DatabaseError::ScanJobNotFound)?;
        if status != "running" {
            return Err(DatabaseError::InvalidScanJobState);
        }
        match control.as_str() {
            "ready" => {}
            "active" if existing_runner.as_deref() == Some(runner_token) => {}
            "active" => return Err(DatabaseError::ScanJobBusy),
            "pause_requested" | "paused" => return Err(DatabaseError::InvalidScanJobState),
            _ => return Err(DatabaseError::InvalidStoredValue),
        }
        transaction.execute(
            "UPDATE scan_jobs SET control_state = 'active', runner_token = ?2, \
                lease_expires_at_unix_ms = ?3, updated_at_unix_ms = ?4 \
             WHERE id = ?1",
            params![job_id, runner_token, lease_expires, now],
        )?;
        transaction.commit()?;
        self.scan_job(job_id)
    }

    pub fn next_scan_queue_entry(
        &mut self,
        job_id: i64,
        runner_token: &str,
        lease_ms: i64,
    ) -> Result<Option<QueueEntry>, DatabaseError> {
        let now = unix_ms()?;
        let lease_expires = now
            .checked_add(lease_ms)
            .ok_or(DatabaseError::InvalidTimestamp)?;
        let transaction = self.connection.transaction()?;
        ensure_active_runner(&transaction, job_id, runner_token, now)?;
        let entry = transaction
            .query_row(
                "SELECT id, path_raw, path_key, parent_identity_key, is_root \
                 FROM scan_queue WHERE scan_id = ?1 AND state = 'pending' ORDER BY id LIMIT 1",
                [job_id],
                |row| {
                    Ok(QueueEntry {
                        id: row.get(0)?,
                        path_raw: row.get(1)?,
                        path_key: row.get(2)?,
                        parent_identity_key: row.get(3)?,
                        is_root: row.get::<_, i64>(4)? != 0,
                    })
                },
            )
            .optional()?;
        if let Some(entry) = &entry {
            let changed = transaction.execute(
                "UPDATE scan_queue SET state = 'processing' WHERE id = ?1 AND scan_id = ?2 AND state = 'pending'",
                params![entry.id, job_id],
            )?;
            if changed != 1 {
                return Err(DatabaseError::ScanJobBusy);
            }
            transaction.execute(
                "UPDATE scan_jobs SET lease_expires_at_unix_ms = ?2, updated_at_unix_ms = ?3 WHERE id = ?1",
                params![job_id, lease_expires, now],
            )?;
        }
        transaction.commit()?;
        Ok(entry)
    }

    #[allow(clippy::too_many_arguments)]
    pub fn stage_scan_queue_entry(
        &mut self,
        job_id: i64,
        runner_token: &str,
        queue_entry_id: i64,
        observation: Option<&Observation>,
        children: &[QueuedPath],
        issues: &[ScanIssue],
        skipped_entries: u64,
        elapsed_ms: u64,
        lease_ms: i64,
    ) -> Result<ScanJobProgress, DatabaseError> {
        let now = unix_ms()?;
        let lease_expires = now
            .checked_add(lease_ms)
            .ok_or(DatabaseError::InvalidTimestamp)?;
        let transaction = self.connection.transaction()?;
        let policy_binding = scan_job_revision_binding(&transaction, job_id)?;
        assert_scope_revision_binding_in_transaction(&transaction, policy_binding)?;
        ensure_owned_runner(&transaction, job_id, runner_token, now)?;

        if let Some(observation) = observation {
            assert_scope_path_key_allowed(
                &transaction,
                policy_binding.scope_id,
                &observation.path_key,
            )?;
            transaction.execute(
                "INSERT INTO scan_staged_observations( \
                    scan_id, kind, identity_kind, identity_key, parent_identity_key, path_raw, \
                    path_key, display_path, size_bytes, modified_unix_ns, link_count \
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11) \
                 ON CONFLICT(scan_id, path_key) DO UPDATE SET \
                    kind = excluded.kind, identity_kind = excluded.identity_kind, identity_key = excluded.identity_key, \
                    parent_identity_key = excluded.parent_identity_key, path_raw = excluded.path_raw, \
                    display_path = excluded.display_path, size_bytes = excluded.size_bytes, \
                    modified_unix_ns = excluded.modified_unix_ns, link_count = excluded.link_count",
                params![
                    job_id,
                    observation.kind.as_str(),
                    observation.identity_kind,
                    observation.identity_key,
                    observation.parent_identity_key,
                    observation.path_raw,
                    observation.path_key,
                    observation.display_path,
                    to_i64(observation.size_bytes)?,
                    observation.modified_unix_ns,
                    observation.link_count.map(to_i64).transpose()?,
                ],
            )?;
        }

        for child in children {
            assert_scope_path_key_allowed(&transaction, policy_binding.scope_id, &child.path_key)?;
            transaction.execute(
                "INSERT INTO scan_queue( \
                    scan_id, path_raw, path_key, parent_identity_key, is_root, state \
                 ) VALUES (?1, ?2, ?3, ?4, ?5, 'pending') \
                 ON CONFLICT(scan_id, path_key) DO NOTHING",
                params![
                    job_id,
                    child.path_raw,
                    child.path_key,
                    child.parent_identity_key,
                    i64::from(child.is_root),
                ],
            )?;
        }
        for issue in issues {
            transaction.execute(
                "INSERT INTO scan_staged_issues(scan_id, code, path_key, detail_code) \
                 VALUES (?1, ?2, ?3, ?4)",
                params![job_id, issue.code, issue.path_key, issue.detail_code],
            )?;
        }
        let changed = transaction.execute(
            "UPDATE scan_queue SET state = 'done' WHERE id = ?1 AND scan_id = ?2 AND state = 'processing'",
            params![queue_entry_id, job_id],
        )?;
        if changed != 1 {
            return Err(DatabaseError::RunnerLeaseLost);
        }

        let queued_entries: i64 = transaction.query_row(
            "SELECT COUNT(*) FROM scan_queue WHERE scan_id = ?1",
            [job_id],
            |row| row.get(0),
        )?;
        let processed_entries: i64 = transaction.query_row(
            "SELECT COUNT(*) FROM scan_queue WHERE scan_id = ?1 AND state = 'done'",
            [job_id],
            |row| row.get(0),
        )?;
        let file_increment =
            i64::from(observation.is_some_and(|value| value.kind == NodeKind::File));
        let folder_increment =
            i64::from(observation.is_some_and(|value| value.kind == NodeKind::Folder));
        transaction.execute(
            "UPDATE scan_jobs SET queued_entries = ?2, processed_entries = ?3, \
                discovered_files = discovered_files + ?4, discovered_folders = discovered_folders + ?5, \
                skipped_entries = skipped_entries + ?6, issue_count = issue_count + ?7, \
                elapsed_ms = elapsed_ms + ?8, lease_expires_at_unix_ms = ?9, updated_at_unix_ms = ?10 \
             WHERE id = ?1",
            params![
                job_id,
                queued_entries,
                processed_entries,
                file_increment,
                folder_increment,
                to_i64(skipped_entries)?,
                to_i64(issues.len() as u64)?,
                to_i64(elapsed_ms)?,
                lease_expires,
                now,
            ],
        )?;
        transaction.commit()?;
        self.scan_job(job_id)
    }

    pub fn release_scan_job(
        &mut self,
        job_id: i64,
        runner_token: &str,
    ) -> Result<ScanJobProgress, DatabaseError> {
        let now = unix_ms()?;
        let transaction = self.connection.transaction()?;
        let policy_binding = scan_job_revision_binding(&transaction, job_id)?;
        assert_scope_revision_binding_in_transaction(&transaction, policy_binding)?;
        ensure_owned_runner(&transaction, job_id, runner_token, now)?;
        transaction.execute(
            "UPDATE scan_jobs SET \
                control_state = CASE WHEN pause_requested = 1 THEN 'paused' ELSE 'ready' END, \
                runner_token = NULL, lease_expires_at_unix_ms = NULL, updated_at_unix_ms = ?3 \
             WHERE id = ?1 AND runner_token = ?2",
            params![job_id, runner_token, now],
        )?;
        transaction.commit()?;
        self.scan_job(job_id)
    }

    pub fn record_scan_runner_elapsed(
        &mut self,
        job_id: i64,
        runner_token: &str,
        elapsed_ms: u64,
    ) -> Result<ScanJobProgress, DatabaseError> {
        let now = unix_ms()?;
        let transaction = self.connection.transaction()?;
        ensure_owned_runner(&transaction, job_id, runner_token, now)?;
        transaction.execute(
            "UPDATE scan_jobs SET elapsed_ms = elapsed_ms + ?3, updated_at_unix_ms = ?4 \
             WHERE id = ?1 AND runner_token = ?2",
            params![job_id, runner_token, to_i64(elapsed_ms)?, now],
        )?;
        transaction.commit()?;
        self.scan_job(job_id)
    }

    pub fn finalize_resumable_scan_job(
        &mut self,
        job_id: i64,
        runner_token: &str,
    ) -> Result<ScanJobProgress, DatabaseError> {
        let now = unix_ms()?;
        let transaction = self.connection.transaction()?;
        ensure_owned_runner(&transaction, job_id, runner_token, now)?;
        let remaining: i64 = transaction.query_row(
            "SELECT COUNT(*) FROM scan_queue WHERE scan_id = ?1 AND state <> 'done'",
            [job_id],
            |row| row.get(0),
        )?;
        let pause_requested: i64 = transaction.query_row(
            "SELECT pause_requested FROM scan_jobs WHERE id = ?1",
            [job_id],
            |row| row.get(0),
        )?;
        if remaining != 0 || pause_requested != 0 {
            return Err(DatabaseError::ScanJobIncomplete);
        }
        let scope_id: i64 = transaction.query_row(
            "SELECT scope_id FROM scan_jobs WHERE id = ?1",
            [job_id],
            |row| row.get(0),
        )?;

        let observations = {
            let mut statement = transaction.prepare(
                "SELECT kind, identity_kind, identity_key, parent_identity_key, path_raw, path_key, \
                    display_path, size_bytes, modified_unix_ns, link_count \
                 FROM scan_staged_observations WHERE scan_id = ?1 ORDER BY id",
            )?;
            let rows = statement.query_map([job_id], |row| {
                let kind_text: String = row.get(0)?;
                let size: i64 = row.get(7)?;
                let link_count: Option<i64> = row.get(9)?;
                Ok((
                    kind_text,
                    row.get::<_, String>(1)?,
                    row.get::<_, Vec<u8>>(2)?,
                    row.get::<_, Option<Vec<u8>>>(3)?,
                    row.get::<_, Vec<u8>>(4)?,
                    row.get::<_, String>(5)?,
                    row.get::<_, String>(6)?,
                    size,
                    row.get::<_, Option<i64>>(8)?,
                    link_count,
                ))
            })?;
            let mut observations = Vec::new();
            for row in rows {
                let (
                    kind,
                    identity_kind,
                    identity_key,
                    parent_identity_key,
                    path_raw,
                    path_key,
                    display_path,
                    size_bytes,
                    modified_unix_ns,
                    link_count,
                ) = row?;
                observations.push(Observation {
                    kind: NodeKind::from_db(&kind)?,
                    identity_kind,
                    identity_key,
                    parent_identity_key,
                    path_raw,
                    path_key,
                    display_path,
                    size_bytes: u64::try_from(size_bytes)
                        .map_err(|_| DatabaseError::InvalidStoredValue)?,
                    modified_unix_ns,
                    link_count: link_count
                        .map(u64::try_from)
                        .transpose()
                        .map_err(|_| DatabaseError::InvalidStoredValue)?,
                });
            }
            observations
        };
        for observation in &observations {
            assert_scope_path_key_allowed(&transaction, scope_id, &observation.path_key)?;
            assert_scope_identity_allowed(
                &transaction,
                scope_id,
                &observation.identity_kind,
                &observation.identity_key,
            )?;
            upsert_observation(&transaction, scope_id, job_id, observation, now)?;
        }
        transaction.execute(
            "UPDATE locations SET present = 0 WHERE scope_id = ?1 AND last_seen_scan_id <> ?2",
            params![scope_id, job_id],
        )?;
        transaction.execute(
            "UPDATE edges SET active = 0 WHERE scope_id = ?1 AND last_seen_scan_id <> ?2",
            params![scope_id, job_id],
        )?;
        invalidate_stale_extraction_outputs(&transaction, scope_id)?;
        transaction.execute(
            "INSERT INTO scan_issues(scan_id, code, path_key, detail_code) \
             SELECT scan_id, code, path_key, detail_code FROM scan_staged_issues WHERE scan_id = ?1",
            [job_id],
        )?;
        transaction.execute(
            "UPDATE scan_jobs SET status = 'completed', control_state = 'ready', pause_requested = 0, \
                runner_token = NULL, lease_expires_at_unix_ms = NULL, finished_at_unix_ms = ?2, \
                updated_at_unix_ms = ?2 WHERE id = ?1",
            params![job_id, now],
        )?;
        transaction.execute("DELETE FROM scan_queue WHERE scan_id = ?1", [job_id])?;
        transaction.execute(
            "DELETE FROM scan_staged_observations WHERE scan_id = ?1",
            [job_id],
        )?;
        transaction.execute(
            "DELETE FROM scan_staged_issues WHERE scan_id = ?1",
            [job_id],
        )?;
        transaction.commit()?;
        self.scan_job(job_id)
    }

    pub fn fail_resumable_scan_job(
        &mut self,
        job_id: i64,
        runner_token: &str,
    ) -> Result<ScanJobProgress, DatabaseError> {
        let now = unix_ms()?;
        let changed = self.connection.execute(
            "UPDATE scan_jobs SET status = 'failed', control_state = 'ready', runner_token = NULL, \
                lease_expires_at_unix_ms = NULL, finished_at_unix_ms = ?3, updated_at_unix_ms = ?3 \
             WHERE id = ?1 AND runner_token = ?2 AND status = 'running' \
                AND lease_expires_at_unix_ms IS NOT NULL AND lease_expires_at_unix_ms > ?3",
            params![job_id, runner_token, now],
        )?;
        if changed != 1 {
            return Err(DatabaseError::RunnerLeaseLost);
        }
        self.scan_job(job_id)
    }

    pub fn recover_expired_scan_jobs_at(&mut self, now: i64) -> Result<u64, DatabaseError> {
        let transaction = self.connection.transaction()?;
        let recovered = transaction.execute(
            "UPDATE scan_jobs SET status = 'interrupted', control_state = 'ready', runner_token = NULL, \
                lease_expires_at_unix_ms = NULL, updated_at_unix_ms = ?1 \
             WHERE status = 'running' AND control_state IN ('active', 'pause_requested') \
                AND lease_expires_at_unix_ms IS NOT NULL AND lease_expires_at_unix_ms <= ?1",
            [now],
        )?;
        transaction.execute(
            "UPDATE scan_queue SET state = 'pending' WHERE state = 'processing' AND scan_id IN ( \
                SELECT id FROM scan_jobs WHERE status = 'interrupted' \
             )",
            [],
        )?;
        transaction.commit()?;
        u64::try_from(recovered).map_err(|_| DatabaseError::InvalidCount)
    }

    pub fn extractable_file(
        &self,
        scope_id: i64,
        node_id: i64,
    ) -> Result<ExtractableFile, DatabaseError> {
        self.connection
            .query_row(
                "SELECT l.scope_id, l.node_id, l.id, l.path_raw, l.path_key, \
                    n.identity_kind, n.identity_key, f.size_bytes, f.modified_unix_ns \
                 FROM locations l \
                 JOIN nodes n ON n.id = l.node_id \
                 JOIN files f ON f.node_id = l.node_id \
                 WHERE l.scope_id = ?1 AND l.node_id = ?2 AND l.present = 1 AND n.kind = 'file' \
                 ORDER BY l.id LIMIT 1",
                params![scope_id, node_id],
                |row| {
                    let size_bytes: i64 = row.get(7)?;
                    Ok(ExtractableFile {
                        scope_id: row.get(0)?,
                        node_id: row.get(1)?,
                        location_id: row.get(2)?,
                        path_raw: row.get(3)?,
                        path_key: row.get(4)?,
                        identity_kind: row.get(5)?,
                        identity_key: row.get(6)?,
                        size_bytes: u64::try_from(size_bytes)
                            .map_err(|_| rusqlite::Error::IntegralValueOutOfRange(7, size_bytes))?,
                        modified_unix_ns: row.get(8)?,
                    })
                },
            )
            .optional()?
            .ok_or(DatabaseError::ExtractableFileNotFound)
    }

    pub fn extractable_file_for_job(&self, job_id: i64) -> Result<ExtractableFile, DatabaseError> {
        self.connection
            .query_row(
                "SELECT l.scope_id, l.node_id, l.id, l.path_raw, l.path_key, \
                    n.identity_kind, n.identity_key, j.source_size_bytes, j.source_modified_unix_ns \
                 FROM extraction_jobs j \
                 JOIN locations l ON l.id = j.location_id AND l.node_id = j.node_id \
                 JOIN nodes n ON n.id = l.node_id \
                 JOIN files f ON f.node_id = l.node_id \
                 WHERE j.id = ?1 AND l.present = 1 AND n.kind = 'file'",
                [job_id],
                |row| {
                    let size_bytes: i64 = row.get(7)?;
                    Ok(ExtractableFile {
                        scope_id: row.get(0)?,
                        node_id: row.get(1)?,
                        location_id: row.get(2)?,
                        path_raw: row.get(3)?,
                        path_key: row.get(4)?,
                        identity_kind: row.get(5)?,
                        identity_key: row.get(6)?,
                        size_bytes: u64::try_from(size_bytes)
                            .map_err(|_| rusqlite::Error::IntegralValueOutOfRange(7, size_bytes))?,
                        modified_unix_ns: row.get(8)?,
                    })
                },
            )
            .optional()?
            .ok_or(DatabaseError::ExtractableFileNotFound)
    }

    pub fn create_extraction_job_with_policy(
        &mut self,
        binding: ScopeRevisionBinding,
        node_id: i64,
    ) -> Result<ExtractionJobProgress, DatabaseError> {
        self.create_extraction_job_for_operation(binding, node_id, ExtractionOperation::Content)
    }

    #[cfg(test)]
    pub fn create_extraction_job(
        &mut self,
        scope_id: i64,
        node_id: i64,
    ) -> Result<ExtractionJobProgress, DatabaseError> {
        let binding = test_revision_binding(self, scope_id)?;
        self.create_extraction_job_with_policy(binding, node_id)
    }

    #[cfg(test)]
    fn create_screenshot_ocr_job_with_policy(
        &mut self,
        binding: ScopeRevisionBinding,
        node_id: i64,
    ) -> Result<ExtractionJobProgress, DatabaseError> {
        self.create_extraction_job_for_operation(
            binding,
            node_id,
            ExtractionOperation::ScreenshotOcr,
        )
    }

    #[cfg(test)]
    fn create_screenshot_ocr_job(
        &mut self,
        scope_id: i64,
        node_id: i64,
    ) -> Result<ExtractionJobProgress, DatabaseError> {
        let binding = test_revision_binding(self, scope_id)?;
        self.create_screenshot_ocr_job_with_policy(binding, node_id)
    }

    /// Low-level storage compare-and-insert used only after the extraction core
    /// has validated the authorized scope, open handle, and encoded image.
    ///
    /// This method rechecks only the manifest metadata snapshot inside the
    /// insertion transaction. It does not prove that file bytes are unchanged,
    /// and it is not a filesystem or image-validation entry point. Workspace
    /// callers must use `deskgraph_extractors::create_screenshot_ocr_job_at`.
    #[doc(hidden)]
    pub fn low_level_insert_screenshot_ocr_job_with_policy_after_core_validation(
        &mut self,
        binding: ScopeRevisionBinding,
        source: &ExtractableFile,
    ) -> Result<ExtractionJobProgress, DatabaseError> {
        self.create_extraction_job_for_current_source(
            binding,
            source,
            ExtractionOperation::ScreenshotOcr,
        )
    }

    #[cfg(test)]
    pub fn low_level_insert_screenshot_ocr_job_after_core_validation(
        &mut self,
        source: &ExtractableFile,
    ) -> Result<ExtractionJobProgress, DatabaseError> {
        let binding = test_revision_binding(self, source.scope_id)?;
        self.low_level_insert_screenshot_ocr_job_with_policy_after_core_validation(binding, source)
    }

    fn create_extraction_job_for_operation(
        &mut self,
        binding: ScopeRevisionBinding,
        node_id: i64,
        operation: ExtractionOperation,
    ) -> Result<ExtractionJobProgress, DatabaseError> {
        let source = self.extractable_file(binding.scope_id, node_id)?;
        self.create_extraction_job_for_current_source(binding, &source, operation)
    }

    fn create_extraction_job_for_current_source(
        &mut self,
        binding: ScopeRevisionBinding,
        source: &ExtractableFile,
        operation: ExtractionOperation,
    ) -> Result<ExtractionJobProgress, DatabaseError> {
        if source.scope_id != binding.scope_id {
            return Err(DatabaseError::ScopePolicyRevisionStale);
        }
        let now = unix_ms()?;
        let transaction = self.connection.transaction()?;
        assert_scope_revision_binding_in_transaction(&transaction, binding)?;
        assert_scope_path_key_allowed(&transaction, source.scope_id, &source.path_key)?;
        assert_scope_identity_allowed(
            &transaction,
            source.scope_id,
            &source.identity_kind,
            &source.identity_key,
        )?;
        let current: i64 = transaction.query_row(
            "SELECT COUNT(*) \
             FROM locations l \
             JOIN nodes n ON n.id = l.node_id \
             JOIN files f ON f.node_id = l.node_id \
             WHERE l.scope_id = ?1 AND l.node_id = ?2 AND l.id = ?3 \
                AND l.path_raw = ?4 AND l.path_key = ?5 AND l.present = 1 \
                AND n.kind = 'file' AND n.identity_kind = ?6 AND n.identity_key = ?7 \
                AND f.size_bytes = ?8 AND f.modified_unix_ns IS ?9",
            params![
                source.scope_id,
                source.node_id,
                source.location_id,
                source.path_raw,
                source.path_key,
                source.identity_kind,
                source.identity_key,
                to_i64(source.size_bytes)?,
                source.modified_unix_ns,
            ],
            |row| row.get(0),
        )?;
        if current != 1 {
            return Err(DatabaseError::ExtractableFileNotFound);
        }
        let active: i64 = transaction.query_row(
            "SELECT COUNT(*) FROM extraction_jobs \
             WHERE scope_id = ?1 AND node_id = ?2 AND status IN ('queued', 'running', 'interrupted')",
            params![source.scope_id, source.node_id],
            |row| row.get(0),
        )?;
        if active != 0 {
            return Err(DatabaseError::ExtractionJobAlreadyActive);
        }
        transaction.execute(
            "INSERT INTO extraction_jobs( \
                scope_id, node_id, location_id, status, source_size_bytes, source_modified_unix_ns, \
                created_at_unix_ms, updated_at_unix_ms, operation, policy_revision \
             ) VALUES (?1, ?2, ?3, 'queued', ?4, ?5, ?6, ?6, ?7, ?8)",
            params![
                source.scope_id,
                source.node_id,
                source.location_id,
                to_i64(source.size_bytes)?,
                source.modified_unix_ns,
                now,
                operation.as_str(),
                binding.revision,
            ],
        )?;
        let job_id = transaction.last_insert_rowid();
        transaction.commit()?;
        self.extraction_job(job_id)
    }

    pub fn extraction_job(&self, job_id: i64) -> Result<ExtractionJobProgress, DatabaseError> {
        self.connection
            .query_row(
                "SELECT id, scope_id, node_id, operation, status, provider_id, provider_version, error_code, \
                    source_size_bytes, output_bytes, chunk_count, elapsed_ms, cancel_requested \
                 FROM extraction_jobs WHERE id = ?1",
                [job_id],
                extraction_job_from_row,
            )
            .optional()?
            .ok_or(DatabaseError::ExtractionJobNotFound)
    }

    pub fn recent_extraction_jobs(&self) -> Result<Vec<ExtractionJobProgress>, DatabaseError> {
        let mut statement = self.connection.prepare(
            "SELECT id, scope_id, node_id, operation, status, provider_id, provider_version, error_code, \
                source_size_bytes, output_bytes, chunk_count, elapsed_ms, cancel_requested \
             FROM extraction_jobs ORDER BY id DESC LIMIT 20",
        )?;
        let jobs = statement.query_map([], extraction_job_from_row)?;
        jobs.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    }

    /// Returns the most actionable screenshot OCR job for one manifest node.
    ///
    /// Active or interrupted work wins over terminal history, so an interrupted
    /// job remains discoverable for recovery even after later terminal rows are
    /// present. The query deliberately returns only the path-free progress
    /// projection used by desktop IPC.
    pub fn screenshot_ocr_job_for_node(
        &self,
        scope_id: i64,
        node_id: i64,
    ) -> Result<Option<ExtractionJobProgress>, DatabaseError> {
        self.connection
            .query_row(
                "SELECT id, scope_id, node_id, operation, status, provider_id, provider_version, error_code, \
                    source_size_bytes, output_bytes, chunk_count, elapsed_ms, cancel_requested \
                 FROM extraction_jobs \
                 WHERE scope_id = ?1 AND node_id = ?2 AND operation = 'screenshot_ocr' \
                 ORDER BY CASE status \
                    WHEN 'running' THEN 0 \
                    WHEN 'queued' THEN 1 \
                    WHEN 'interrupted' THEN 2 \
                    ELSE 3 \
                 END, id DESC \
                 LIMIT 1",
                params![scope_id, node_id],
                extraction_job_from_row,
            )
            .optional()
            .map_err(Into::into)
    }

    pub fn image_metadata_for_job(&self, job_id: i64) -> Result<ImageMetadata, DatabaseError> {
        let stored: (i64, i64, String, i64, i64, i64, String, String) = self
            .connection
            .query_row(
                "SELECT scope_id, node_id, format, pixel_width, pixel_height, source_size_bytes, \
                    provider_id, provider_version \
                 FROM image_metadata WHERE extraction_job_id = ?1",
                [job_id],
                |row| {
                    Ok((
                        row.get(0)?,
                        row.get(1)?,
                        row.get(2)?,
                        row.get(3)?,
                        row.get(4)?,
                        row.get(5)?,
                        row.get(6)?,
                        row.get(7)?,
                    ))
                },
            )
            .optional()?
            .ok_or(DatabaseError::ImageMetadataNotFound)?;
        let format =
            ImageFormat::from_storage(&stored.2).ok_or(DatabaseError::InvalidStoredValue)?;
        let pixel_width = u32::try_from(stored.3).map_err(|_| DatabaseError::InvalidStoredValue)?;
        let pixel_height =
            u32::try_from(stored.4).map_err(|_| DatabaseError::InvalidStoredValue)?;
        if !is_valid_image_dimensions(pixel_width, pixel_height) {
            return Err(DatabaseError::InvalidStoredValue);
        }
        Ok(ImageMetadata {
            api_version: ImageMetadata::API_VERSION,
            extraction_job_id: job_id,
            scope_id: stored.0,
            node_id: stored.1,
            format,
            pixel_width,
            pixel_height,
            source_bytes: u64::try_from(stored.5).map_err(|_| DatabaseError::InvalidStoredValue)?,
            provider_id: stored.6,
            provider_version: stored.7,
        })
    }

    pub fn request_extraction_cancel(
        &mut self,
        job_id: i64,
    ) -> Result<ExtractionJobProgress, DatabaseError> {
        let now = unix_ms()?;
        let transaction = self.connection.transaction()?;
        let status: String = transaction
            .query_row(
                "SELECT status FROM extraction_jobs WHERE id = ?1",
                [job_id],
                |row| row.get(0),
            )
            .optional()?
            .ok_or(DatabaseError::ExtractionJobNotFound)?;
        match status.as_str() {
            "queued" | "interrupted" => {
                transaction.execute(
                    "UPDATE extraction_jobs SET status = 'cancelled', cancel_requested = 1, \
                        finished_at_unix_ms = ?2, updated_at_unix_ms = ?2 \
                     WHERE id = ?1",
                    params![job_id, now],
                )?;
            }
            "running" => {
                transaction.execute(
                    "UPDATE extraction_jobs SET cancel_requested = 1, updated_at_unix_ms = ?2 \
                     WHERE id = ?1",
                    params![job_id, now],
                )?;
            }
            "cancelled" => {}
            "completed" | "failed" => return Err(DatabaseError::InvalidExtractionJobState),
            _ => return Err(DatabaseError::InvalidStoredValue),
        }
        transaction.commit()?;
        self.extraction_job(job_id)
    }

    pub fn resume_extraction_job(
        &mut self,
        job_id: i64,
    ) -> Result<ExtractionJobProgress, DatabaseError> {
        let now = unix_ms()?;
        let changed = self.connection.execute(
            "UPDATE extraction_jobs SET status = 'queued', cancel_requested = 0, error_code = NULL, \
                runner_token = NULL, lease_expires_at_unix_ms = NULL, updated_at_unix_ms = ?2 \
             WHERE id = ?1 AND status = 'interrupted'",
            params![job_id, now],
        )?;
        if changed != 1 {
            self.extraction_job(job_id)?;
            return Err(DatabaseError::InvalidExtractionJobState);
        }
        self.extraction_job(job_id)
    }

    pub fn claim_extraction_job(
        &mut self,
        job_id: i64,
        runner_token: &str,
        lease_ms: i64,
    ) -> Result<ExtractionJobProgress, DatabaseError> {
        let now = unix_ms()?;
        self.recover_expired_extraction_jobs_at(now)?;
        let lease_expires = now
            .checked_add(lease_ms)
            .ok_or(DatabaseError::InvalidTimestamp)?;
        let transaction = self.connection.transaction()?;
        let (status, existing_runner): (String, Option<String>) = transaction
            .query_row(
                "SELECT status, runner_token FROM extraction_jobs WHERE id = ?1",
                [job_id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()?
            .ok_or(DatabaseError::ExtractionJobNotFound)?;
        match status.as_str() {
            "queued" => {}
            "running" if existing_runner.as_deref() == Some(runner_token) => {}
            "running" => return Err(DatabaseError::ExtractionJobBusy),
            _ => return Err(DatabaseError::InvalidExtractionJobState),
        }
        transaction.execute(
            "UPDATE extraction_jobs SET status = 'running', runner_token = ?2, \
                lease_expires_at_unix_ms = ?3, started_at_unix_ms = COALESCE(started_at_unix_ms, ?4), \
                updated_at_unix_ms = ?4 WHERE id = ?1",
            params![job_id, runner_token, lease_expires, now],
        )?;
        transaction.commit()?;
        self.extraction_job(job_id)
    }

    pub fn extraction_cancel_requested(&self, job_id: i64) -> Result<bool, DatabaseError> {
        let (status, requested): (String, i64) = self
            .connection
            .query_row(
                "SELECT status, cancel_requested FROM extraction_jobs WHERE id = ?1",
                [job_id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()?
            .ok_or(DatabaseError::ExtractionJobNotFound)?;
        Ok(status != "running" || requested != 0)
    }

    pub fn requeue_extraction_job_after_capacity_refusal(
        &mut self,
        job_id: i64,
        runner_token: &str,
    ) -> Result<ExtractionJobProgress, DatabaseError> {
        let now = unix_ms()?;
        let changed = self.connection.execute(
            "UPDATE extraction_jobs SET \
                status = CASE WHEN cancel_requested != 0 THEN 'cancelled' ELSE 'queued' END, \
                runner_token = NULL, lease_expires_at_unix_ms = NULL, \
                finished_at_unix_ms = CASE WHEN cancel_requested != 0 THEN ?4 ELSE NULL END, \
                updated_at_unix_ms = ?4 \
             WHERE id = ?1 AND runner_token = ?2 AND status = 'running' \
                AND lease_expires_at_unix_ms IS NOT NULL AND lease_expires_at_unix_ms > ?3",
            params![job_id, runner_token, now, now],
        )?;
        if changed != 1 {
            return Err(DatabaseError::ExtractionRunnerLeaseLost);
        }
        self.extraction_job(job_id)
    }

    #[allow(clippy::too_many_arguments)]
    pub fn complete_extraction_job(
        &mut self,
        job_id: i64,
        runner_token: &str,
        provider_id: &str,
        provider_version: &str,
        source_size_bytes: u64,
        source_modified_unix_ns: Option<i64>,
        output_bytes: u64,
        elapsed_ms: u64,
        chunks: &[ContentChunkWrite],
    ) -> Result<ExtractionJobProgress, DatabaseError> {
        self.complete_extraction_job_with_image_metadata(
            job_id,
            runner_token,
            provider_id,
            provider_version,
            source_size_bytes,
            source_modified_unix_ns,
            output_bytes,
            elapsed_ms,
            chunks,
            None,
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub fn complete_extraction_job_with_image_metadata(
        &mut self,
        job_id: i64,
        runner_token: &str,
        provider_id: &str,
        provider_version: &str,
        source_size_bytes: u64,
        source_modified_unix_ns: Option<i64>,
        output_bytes: u64,
        elapsed_ms: u64,
        chunks: &[ContentChunkWrite],
        image_metadata: Option<&ImageMetadataWrite>,
    ) -> Result<ExtractionJobProgress, DatabaseError> {
        if provider_id.is_empty()
            || provider_version.is_empty()
            || source_size_bytes > MAX_EXTRACTION_SOURCE_BYTES
            || chunks.len() > MAX_EXTRACTION_CHUNKS
            || chunks
                .iter()
                .any(|chunk| chunk.text.len() > MAX_EXTRACTION_CHUNK_BYTES)
        {
            return Err(DatabaseError::ExtractionOutputInvalid);
        }
        let mut computed_output_bytes = 0_u64;
        for chunk in chunks {
            let chunk_bytes = u64::try_from(chunk.text.len())
                .map_err(|_| DatabaseError::ExtractionOutputInvalid)?;
            computed_output_bytes = computed_output_bytes
                .checked_add(chunk_bytes)
                .ok_or(DatabaseError::ExtractionOutputInvalid)?;
        }
        if output_bytes > MAX_EXTRACTION_OUTPUT_BYTES || computed_output_bytes != output_bytes {
            return Err(DatabaseError::ExtractionOutputInvalid);
        }
        let now = unix_ms()?;
        let transaction = self.connection.transaction()?;
        let policy_binding = extraction_job_revision_binding(&transaction, job_id)?;
        assert_scope_revision_binding_in_transaction(&transaction, policy_binding)?;
        ensure_extraction_runner(&transaction, job_id, runner_token, now)?;
        let (
            scope_id,
            node_id,
            location_id,
            stored_size,
            stored_modified,
            cancel_requested,
            stored_operation,
        ): (i64, i64, i64, i64, Option<i64>, i64, String) = transaction.query_row(
            "SELECT scope_id, node_id, location_id, source_size_bytes, source_modified_unix_ns, \
                cancel_requested, operation FROM extraction_jobs WHERE id = ?1",
            [job_id],
            |row| {
                Ok((
                    row.get(0)?,
                    row.get(1)?,
                    row.get(2)?,
                    row.get(3)?,
                    row.get(4)?,
                    row.get(5)?,
                    row.get(6)?,
                ))
            },
        )?;
        let operation = ExtractionOperation::from_storage(&stored_operation)
            .ok_or(DatabaseError::InvalidStoredValue)?;
        let is_image_metadata_provider = provider_id == "deskgraph.image-metadata";
        let contains_ocr = chunks.iter().any(|chunk| {
            matches!(
                &chunk.provenance,
                ContentChunkProvenanceWrite::OcrObservation { .. }
            )
        });
        let invalid_operation_output = match operation {
            ExtractionOperation::Content => {
                contains_ocr
                    || is_image_metadata_provider != image_metadata.is_some()
                    || image_metadata.is_some_and(|metadata| {
                        !chunks.is_empty()
                            || output_bytes != 0
                            || !is_valid_image_dimensions(
                                metadata.pixel_width,
                                metadata.pixel_height,
                            )
                    })
            }
            ExtractionOperation::ScreenshotOcr => {
                image_metadata.is_some()
                    || is_image_metadata_provider
                    || chunks.iter().any(|chunk| {
                        !matches!(
                            &chunk.provenance,
                            ContentChunkProvenanceWrite::OcrObservation { .. }
                        )
                    })
            }
        };
        if invalid_operation_output {
            return Err(DatabaseError::ExtractionOutputInvalid);
        }
        if cancel_requested != 0
            || u64::try_from(stored_size).map_err(|_| DatabaseError::InvalidStoredValue)?
                != source_size_bytes
            || stored_modified != source_modified_unix_ns
        {
            return Err(DatabaseError::ExtractionOutputInvalid);
        }
        let current_source: Option<(i64, Option<i64>, String)> = transaction
            .query_row(
                "SELECT f.size_bytes, f.modified_unix_ns, l.path_key \
                 FROM locations l JOIN files f ON f.node_id = l.node_id \
                 WHERE l.id = ?1 AND l.node_id = ?2 AND l.present = 1",
                params![location_id, node_id],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .optional()?;
        let Some((current_size, current_modified, current_path_key)) = current_source else {
            return Err(DatabaseError::ExtractionOutputInvalid);
        };
        assert_scope_path_key_allowed(&transaction, scope_id, &current_path_key)?;
        if u64::try_from(current_size).map_err(|_| DatabaseError::InvalidStoredValue)?
            != source_size_bytes
            || current_modified != source_modified_unix_ns
        {
            return Err(DatabaseError::ExtractionOutputInvalid);
        }
        for (index, chunk) in chunks.iter().enumerate() {
            let invalid_provenance = match &chunk.provenance {
                ContentChunkProvenanceWrite::ByteRange { start, end } => {
                    start > end || *end > source_size_bytes
                }
                ContentChunkProvenanceWrite::PdfPage { page_number, .. } => *page_number == 0,
                ContentChunkProvenanceWrite::DocxParagraph {
                    paragraph_number, ..
                } => *paragraph_number == 0,
                ContentChunkProvenanceWrite::PptxSlide { slide_number, .. } => *slide_number == 0,
                ContentChunkProvenanceWrite::XlsxCell {
                    sheet_number,
                    cell_reference,
                    ..
                } => *sheet_number == 0 || !is_valid_xlsx_cell_reference(cell_reference),
                ContentChunkProvenanceWrite::OcrObservation {
                    observation_number,
                    bbox_x_ppm,
                    bbox_y_ppm,
                    bbox_width_ppm,
                    bbox_height_ppm,
                    confidence_basis_points,
                    ..
                } => {
                    *observation_number == 0
                        || *bbox_width_ppm == 0
                        || *bbox_height_ppm == 0
                        || bbox_x_ppm
                            .checked_add(*bbox_width_ppm)
                            .is_none_or(|right| right > 1_000_000)
                        || bbox_y_ppm
                            .checked_add(*bbox_height_ppm)
                            .is_none_or(|top| top > 1_000_000)
                        || confidence_basis_points.is_some_and(|value| value > 10_000)
                }
            };
            if usize::try_from(chunk.ordinal).map_err(|_| DatabaseError::ExtractionOutputInvalid)?
                != index
                || chunk.trust_class != "untrusted_extracted_text"
                || invalid_provenance
            {
                return Err(DatabaseError::ExtractionOutputInvalid);
            }
        }

        match operation {
            ExtractionOperation::Content => {
                transaction.execute(
                    "UPDATE content_chunks SET active = 0 \
                     WHERE scope_id = ?1 AND node_id = ?2 AND active = 1 \
                        AND provenance_kind <> 'ocr_observation'",
                    params![scope_id, node_id],
                )?;
                transaction.execute(
                    "UPDATE image_metadata SET active = 0 \
                     WHERE scope_id = ?1 AND node_id = ?2 AND active = 1",
                    params![scope_id, node_id],
                )?;
            }
            ExtractionOperation::ScreenshotOcr => {
                transaction.execute(
                    "UPDATE content_chunks SET active = 0 \
                     WHERE scope_id = ?1 AND node_id = ?2 AND active = 1 \
                        AND provenance_kind = 'ocr_observation'",
                    params![scope_id, node_id],
                )?;
            }
        }
        for chunk in chunks {
            let (
                provenance_kind,
                source_byte_start,
                source_byte_end,
                source_page_number,
                source_unit_number,
                source_cell_reference,
                source_fragment_index,
                source_bbox_x_ppm,
                source_bbox_y_ppm,
                source_bbox_width_ppm,
                source_bbox_height_ppm,
                source_confidence_basis_points,
            ) = match &chunk.provenance {
                ContentChunkProvenanceWrite::ByteRange { start, end } => (
                    "byte_range",
                    Some(to_i64(*start)?),
                    Some(to_i64(*end)?),
                    None,
                    None,
                    None,
                    None,
                    None,
                    None,
                    None,
                    None,
                    None,
                ),
                ContentChunkProvenanceWrite::PdfPage {
                    page_number,
                    fragment_index,
                } => (
                    "pdf_page",
                    None,
                    None,
                    Some(i64::from(*page_number)),
                    None,
                    None,
                    Some(i64::from(*fragment_index)),
                    None,
                    None,
                    None,
                    None,
                    None,
                ),
                ContentChunkProvenanceWrite::DocxParagraph {
                    paragraph_number,
                    fragment_index,
                } => (
                    "docx_paragraph",
                    None,
                    None,
                    None,
                    Some(i64::from(*paragraph_number)),
                    None,
                    Some(i64::from(*fragment_index)),
                    None,
                    None,
                    None,
                    None,
                    None,
                ),
                ContentChunkProvenanceWrite::PptxSlide {
                    slide_number,
                    fragment_index,
                } => (
                    "pptx_slide",
                    None,
                    None,
                    None,
                    Some(i64::from(*slide_number)),
                    None,
                    Some(i64::from(*fragment_index)),
                    None,
                    None,
                    None,
                    None,
                    None,
                ),
                ContentChunkProvenanceWrite::XlsxCell {
                    sheet_number,
                    cell_reference,
                    fragment_index,
                } => (
                    "xlsx_cell",
                    None,
                    None,
                    None,
                    Some(i64::from(*sheet_number)),
                    Some(cell_reference.as_str()),
                    Some(i64::from(*fragment_index)),
                    None,
                    None,
                    None,
                    None,
                    None,
                ),
                ContentChunkProvenanceWrite::OcrObservation {
                    observation_number,
                    fragment_index,
                    bbox_x_ppm,
                    bbox_y_ppm,
                    bbox_width_ppm,
                    bbox_height_ppm,
                    confidence_basis_points,
                } => (
                    "ocr_observation",
                    None,
                    None,
                    None,
                    Some(i64::from(*observation_number)),
                    None,
                    Some(i64::from(*fragment_index)),
                    Some(i64::from(*bbox_x_ppm)),
                    Some(i64::from(*bbox_y_ppm)),
                    Some(i64::from(*bbox_width_ppm)),
                    Some(i64::from(*bbox_height_ppm)),
                    confidence_basis_points.map(i64::from),
                ),
            };
            transaction.execute(
                "INSERT INTO content_chunks( \
                    scope_id, node_id, location_id, extraction_job_id, ordinal, text, \
                    provenance_kind, source_byte_start, source_byte_end, source_page_number, \
                    source_unit_number, source_cell_reference, source_fragment_index, \
                    source_bbox_x_ppm, source_bbox_y_ppm, source_bbox_width_ppm, \
                    source_bbox_height_ppm, source_confidence_basis_points, source_size_bytes, \
                    source_modified_unix_ns, trust_class, provider_id, provider_version, active, \
                    created_at_unix_ms \
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20, ?21, ?22, ?23, 1, ?24)",
                params![
                    scope_id,
                    node_id,
                    location_id,
                    job_id,
                    i64::from(chunk.ordinal),
                    chunk.text,
                    provenance_kind,
                    source_byte_start,
                    source_byte_end,
                    source_page_number,
                    source_unit_number,
                    source_cell_reference,
                    source_fragment_index,
                    source_bbox_x_ppm,
                    source_bbox_y_ppm,
                    source_bbox_width_ppm,
                    source_bbox_height_ppm,
                    source_confidence_basis_points,
                    to_i64(source_size_bytes)?,
                    source_modified_unix_ns,
                    chunk.trust_class,
                    provider_id,
                    provider_version,
                    now,
                ],
            )?;
        }
        if let Some(metadata) = image_metadata {
            transaction.execute(
                "INSERT INTO image_metadata( \
                    scope_id, node_id, location_id, extraction_job_id, format, pixel_width, \
                    pixel_height, source_size_bytes, source_modified_unix_ns, provider_id, \
                    provider_version, active, created_at_unix_ms \
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, 1, ?12)",
                params![
                    scope_id,
                    node_id,
                    location_id,
                    job_id,
                    metadata.format.as_str(),
                    i64::from(metadata.pixel_width),
                    i64::from(metadata.pixel_height),
                    to_i64(source_size_bytes)?,
                    source_modified_unix_ns,
                    provider_id,
                    provider_version,
                    now,
                ],
            )?;
        }
        let completed = transaction.execute(
            "UPDATE extraction_jobs SET status = 'completed', provider_id = ?3, provider_version = ?4, \
                error_code = NULL, output_bytes = ?5, chunk_count = ?6, elapsed_ms = ?7, \
                runner_token = NULL, lease_expires_at_unix_ms = NULL, finished_at_unix_ms = ?8, \
                updated_at_unix_ms = ?8 WHERE id = ?1 AND runner_token = ?2 \
                AND status = 'running' AND cancel_requested = 0",
            params![
                job_id,
                runner_token,
                provider_id,
                provider_version,
                to_i64(output_bytes)?,
                to_i64(chunks.len() as u64)?,
                to_i64(elapsed_ms)?,
                now,
            ],
        )?;
        if completed != 1 {
            return Err(DatabaseError::ExtractionRunnerLeaseLost);
        }
        transaction.commit()?;
        self.extraction_job(job_id)
    }

    pub fn fail_extraction_job(
        &mut self,
        job_id: i64,
        runner_token: &str,
        provider_id: &str,
        provider_version: &str,
        error_code: &str,
        elapsed_ms: u64,
    ) -> Result<ExtractionJobProgress, DatabaseError> {
        let now = unix_ms()?;
        let changed = self.connection.execute(
            "UPDATE extraction_jobs SET \
                status = CASE WHEN cancel_requested != 0 THEN 'cancelled' ELSE 'failed' END, \
                provider_id = ?3, provider_version = ?4, \
                error_code = CASE WHEN cancel_requested != 0 THEN NULL ELSE ?5 END, \
                elapsed_ms = ?6, runner_token = NULL, lease_expires_at_unix_ms = NULL, \
                finished_at_unix_ms = ?7, updated_at_unix_ms = ?7 \
             WHERE id = ?1 AND runner_token = ?2 AND status = 'running' \
                AND lease_expires_at_unix_ms IS NOT NULL AND lease_expires_at_unix_ms > ?7",
            params![
                job_id,
                runner_token,
                provider_id,
                provider_version,
                error_code,
                to_i64(elapsed_ms)?,
                now,
            ],
        )?;
        if changed != 1 {
            return Err(DatabaseError::ExtractionRunnerLeaseLost);
        }
        self.extraction_job(job_id)
    }

    pub fn cancel_extraction_job_from_runner(
        &mut self,
        job_id: i64,
        runner_token: &str,
        provider_id: &str,
        provider_version: &str,
        elapsed_ms: u64,
    ) -> Result<ExtractionJobProgress, DatabaseError> {
        let now = unix_ms()?;
        let changed = self.connection.execute(
            "UPDATE extraction_jobs SET status = 'cancelled', cancel_requested = 1, \
                provider_id = ?3, provider_version = ?4, error_code = NULL, elapsed_ms = ?5, \
                runner_token = NULL, lease_expires_at_unix_ms = NULL, finished_at_unix_ms = ?6, \
                updated_at_unix_ms = ?6 \
             WHERE id = ?1 AND runner_token = ?2 AND status = 'running' \
                AND lease_expires_at_unix_ms IS NOT NULL AND lease_expires_at_unix_ms > ?6",
            params![
                job_id,
                runner_token,
                provider_id,
                provider_version,
                to_i64(elapsed_ms)?,
                now,
            ],
        )?;
        if changed != 1 {
            return Err(DatabaseError::ExtractionRunnerLeaseLost);
        }
        self.extraction_job(job_id)
    }

    pub fn recover_expired_extraction_jobs_at(&mut self, now: i64) -> Result<u64, DatabaseError> {
        let recovered = self.connection.execute(
            "UPDATE extraction_jobs SET status = 'interrupted', runner_token = NULL, \
                lease_expires_at_unix_ms = NULL, updated_at_unix_ms = ?1 \
             WHERE status = 'running' AND lease_expires_at_unix_ms IS NOT NULL \
                AND lease_expires_at_unix_ms <= ?1",
            [now],
        )?;
        u64::try_from(recovered).map_err(|_| DatabaseError::InvalidCount)
    }

    pub fn extraction_stats(&self) -> Result<ExtractionStats, DatabaseError> {
        Ok(ExtractionStats {
            api_version: ExtractionStats::API_VERSION,
            active_chunk_count: count(
                &self.connection,
                "SELECT COUNT(*) FROM content_chunks WHERE active = 1",
            )?,
            extracted_file_count: count(
                &self.connection,
                "SELECT COUNT(*) FROM ( \
                    SELECT node_id FROM content_chunks WHERE active = 1 \
                    UNION SELECT node_id FROM image_metadata WHERE active = 1 \
                 )",
            )?,
            completed_job_count: count(
                &self.connection,
                "SELECT COUNT(*) FROM extraction_jobs WHERE status = 'completed'",
            )?,
            failed_job_count: count(
                &self.connection,
                "SELECT COUNT(*) FROM extraction_jobs WHERE status = 'failed'",
            )?,
            cancelled_job_count: count(
                &self.connection,
                "SELECT COUNT(*) FROM extraction_jobs WHERE status = 'cancelled'",
            )?,
        })
    }

    /// Returns only aggregate extraction state belonging to scopes whose
    /// platform access grant is currently active. No grant bytes or paths are
    /// selected by these queries.
    pub fn extraction_stats_with_active_access_grants(
        &self,
    ) -> Result<ExtractionStats, DatabaseError> {
        Ok(ExtractionStats {
            api_version: ExtractionStats::API_VERSION,
            active_chunk_count: count_with_host_platform(
                &self.connection,
                "SELECT COUNT(*) FROM content_chunks chunk \
                 JOIN scope_access_grants grant ON grant.scope_id = chunk.scope_id \
                 JOIN authorized_scopes scope ON scope.id = grant.scope_id AND scope.platform = grant.platform \
                 WHERE chunk.active = 1 AND grant.state = 'active' \
                   AND scope.platform = ?1 AND grant.platform = ?1",
            )?,
            extracted_file_count: count_with_host_platform(
                &self.connection,
                "SELECT COUNT(*) FROM ( \
                    SELECT chunk.node_id FROM content_chunks chunk \
                    JOIN scope_access_grants grant ON grant.scope_id = chunk.scope_id \
                    JOIN authorized_scopes scope ON scope.id = grant.scope_id AND scope.platform = grant.platform \
                    WHERE chunk.active = 1 AND grant.state = 'active' AND scope.platform = ?1 AND grant.platform = ?1 \
                    UNION SELECT metadata.node_id FROM image_metadata metadata \
                    JOIN scope_access_grants grant ON grant.scope_id = metadata.scope_id \
                    JOIN authorized_scopes scope ON scope.id = grant.scope_id AND scope.platform = grant.platform \
                    WHERE metadata.active = 1 AND grant.state = 'active' AND scope.platform = ?1 AND grant.platform = ?1 \
                 )",
            )?,
            completed_job_count: count_with_host_platform(
                &self.connection,
                "SELECT COUNT(*) FROM extraction_jobs job \
                 JOIN scope_access_grants grant ON grant.scope_id = job.scope_id \
                 JOIN authorized_scopes scope ON scope.id = grant.scope_id AND scope.platform = grant.platform \
                 WHERE job.status = 'completed' AND grant.state = 'active' \
                   AND scope.platform = ?1 AND grant.platform = ?1",
            )?,
            failed_job_count: count_with_host_platform(
                &self.connection,
                "SELECT COUNT(*) FROM extraction_jobs job \
                 JOIN scope_access_grants grant ON grant.scope_id = job.scope_id \
                 JOIN authorized_scopes scope ON scope.id = grant.scope_id AND scope.platform = grant.platform \
                 WHERE job.status = 'failed' AND grant.state = 'active' \
                   AND scope.platform = ?1 AND grant.platform = ?1",
            )?,
            cancelled_job_count: count_with_host_platform(
                &self.connection,
                "SELECT COUNT(*) FROM extraction_jobs job \
                 JOIN scope_access_grants grant ON grant.scope_id = job.scope_id \
                 JOIN authorized_scopes scope ON scope.id = grant.scope_id AND scope.platform = grant.platform \
                 WHERE job.status = 'cancelled' AND grant.state = 'active' \
                   AND scope.platform = ?1 AND grant.platform = ?1",
            )?,
        })
    }

    pub fn lexical_search_candidates(
        &self,
        match_query: &str,
        filters: LexicalSearchFilters<'_>,
        per_source_candidate_limit: u32,
    ) -> Result<Vec<LexicalSearchCandidate>, DatabaseError> {
        lexical_search_candidates_from_connection(
            &self.connection,
            match_query,
            filters,
            per_source_candidate_limit,
        )
    }

    /// Returns current folder choices only after a direct user request. The
    /// response intentionally contains paths, so ordinary diagnostics must
    /// rely on its redacted `Debug` implementation.
    pub fn list_search_folders(
        &self,
        scope_id: i64,
        limit: Option<u32>,
    ) -> Result<SearchFolderListResponse, DatabaseError> {
        let transaction = self.connection.unchecked_transaction()?;
        let response = search_folder_list_from_connection(&transaction, scope_id, limit)?;
        transaction.commit()?;
        Ok(response)
    }

    pub fn invalidate_content_for_node(
        &self,
        scope_id: i64,
        node_id: i64,
    ) -> Result<u64, DatabaseError> {
        let chunks = self.connection.execute(
            "UPDATE content_chunks SET active = 0 \
             WHERE scope_id = ?1 AND node_id = ?2 AND active = 1",
            params![scope_id, node_id],
        )?;
        let metadata = self.connection.execute(
            "UPDATE image_metadata SET active = 0 \
             WHERE scope_id = ?1 AND node_id = ?2 AND active = 1",
            params![scope_id, node_id],
        )?;
        u64::try_from(chunks)
            .ok()
            .and_then(|chunks| {
                u64::try_from(metadata)
                    .ok()
                    .and_then(|metadata| chunks.checked_add(metadata))
            })
            .ok_or(DatabaseError::InvalidCount)
    }

    pub fn stats(&self) -> Result<ManifestStats, DatabaseError> {
        Ok(ManifestStats {
            api_version: ManifestStats::API_VERSION,
            database_ready: true,
            authorized_scope_count: count(
                &self.connection,
                "SELECT COUNT(*) FROM authorized_scopes",
            )?,
            node_count: count(
                &self.connection,
                "SELECT COUNT(DISTINCT node_id) FROM locations WHERE present = 1",
            )?,
            file_count: count(
                &self.connection,
                "SELECT COUNT(DISTINCT files.node_id) FROM files JOIN locations ON locations.node_id = files.node_id WHERE locations.present = 1",
            )?,
            folder_count: count(
                &self.connection,
                "SELECT COUNT(DISTINCT folders.node_id) FROM folders JOIN locations ON locations.node_id = folders.node_id WHERE locations.present = 1",
            )?,
            active_location_count: count(
                &self.connection,
                "SELECT COUNT(*) FROM locations WHERE present = 1",
            )?,
            issue_count: count(
                &self.connection,
                "SELECT COALESCE(issue_count, 0) FROM scan_jobs WHERE status = 'completed' ORDER BY id DESC LIMIT 1",
            )?,
            completed_scan_count: count(
                &self.connection,
                "SELECT COUNT(*) FROM scan_jobs WHERE status = 'completed'",
            )?,
        })
    }

    /// Returns the packaged Desktop dashboard view. Legacy, revoked, or
    /// restore-failed scopes remain durable for reauthorization but do not
    /// contribute paths, graph counts, issues, or scan totals to this view.
    pub fn stats_with_active_access_grants(&self) -> Result<ManifestStats, DatabaseError> {
        Ok(ManifestStats {
            api_version: ManifestStats::API_VERSION,
            database_ready: true,
            authorized_scope_count: count_with_host_platform(
                &self.connection,
                "SELECT COUNT(*) FROM scope_access_grants grant \
                 JOIN authorized_scopes scope ON scope.id = grant.scope_id AND scope.platform = grant.platform \
                 WHERE grant.state = 'active' AND scope.platform = ?1 AND grant.platform = ?1",
            )?,
            node_count: count_with_host_platform(
                &self.connection,
                "SELECT COUNT(DISTINCT location.node_id) FROM locations location \
                 JOIN scope_access_grants grant ON grant.scope_id = location.scope_id \
                 JOIN authorized_scopes scope ON scope.id = grant.scope_id AND scope.platform = grant.platform \
                 WHERE location.present = 1 AND grant.state = 'active' \
                   AND scope.platform = ?1 AND grant.platform = ?1",
            )?,
            file_count: count_with_host_platform(
                &self.connection,
                "SELECT COUNT(DISTINCT file.node_id) FROM files file \
                 JOIN locations location ON location.node_id = file.node_id \
                 JOIN scope_access_grants grant ON grant.scope_id = location.scope_id \
                 JOIN authorized_scopes scope ON scope.id = grant.scope_id AND scope.platform = grant.platform \
                 WHERE location.present = 1 AND grant.state = 'active' \
                   AND scope.platform = ?1 AND grant.platform = ?1",
            )?,
            folder_count: count_with_host_platform(
                &self.connection,
                "SELECT COUNT(DISTINCT folder.node_id) FROM folders folder \
                 JOIN locations location ON location.node_id = folder.node_id \
                 JOIN scope_access_grants grant ON grant.scope_id = location.scope_id \
                 JOIN authorized_scopes scope ON scope.id = grant.scope_id AND scope.platform = grant.platform \
                 WHERE location.present = 1 AND grant.state = 'active' \
                   AND scope.platform = ?1 AND grant.platform = ?1",
            )?,
            active_location_count: count_with_host_platform(
                &self.connection,
                "SELECT COUNT(*) FROM locations location \
                 JOIN scope_access_grants grant ON grant.scope_id = location.scope_id \
                 JOIN authorized_scopes scope ON scope.id = grant.scope_id AND scope.platform = grant.platform \
                 WHERE location.present = 1 AND grant.state = 'active' \
                   AND scope.platform = ?1 AND grant.platform = ?1",
            )?,
            issue_count: count_with_host_platform(
                &self.connection,
                "SELECT COALESCE(job.issue_count, 0) FROM scan_jobs job \
                 JOIN scope_access_grants grant ON grant.scope_id = job.scope_id \
                 JOIN authorized_scopes scope ON scope.id = grant.scope_id AND scope.platform = grant.platform \
                 WHERE job.status = 'completed' AND grant.state = 'active' \
                   AND scope.platform = ?1 AND grant.platform = ?1 \
                 ORDER BY job.id DESC LIMIT 1",
            )?,
            completed_scan_count: count_with_host_platform(
                &self.connection,
                "SELECT COUNT(*) FROM scan_jobs job \
                 JOIN scope_access_grants grant ON grant.scope_id = job.scope_id \
                 JOIN authorized_scopes scope ON scope.id = grant.scope_id AND scope.platform = grant.platform \
                 WHERE job.status = 'completed' AND grant.state = 'active' \
                   AND scope.platform = ?1 AND grant.platform = ?1",
            )?,
        })
    }

    pub fn folder_profile_facts(
        &self,
        scope_id: i64,
        folder_node_id: i64,
        entry_limit: u64,
    ) -> Result<FolderProfileFacts, DatabaseError> {
        if scope_id <= 0
            || folder_node_id <= 0
            || entry_limit == 0
            || entry_limit > MAX_FOLDER_PROFILE_ENTRIES
        {
            return Err(DatabaseError::FolderProfileInputInvalid);
        }
        let folder = self
            .connection
            .query_row(
                "SELECT l.id, l.path_key, l.display_path \
                 FROM locations l \
                 JOIN nodes n ON n.id = l.node_id AND n.kind = 'folder' \
                 WHERE l.scope_id = ?1 AND l.node_id = ?2 AND l.present = 1 \
                 ORDER BY l.id LIMIT 1",
                params![scope_id, folder_node_id],
                |row| {
                    Ok((
                        row.get::<_, i64>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, String>(2)?,
                    ))
                },
            )
            .optional()?
            .ok_or(DatabaseError::FolderNotFound)?;
        let prefix_chars = i64::try_from(folder.1.chars().count())
            .map_err(|_| DatabaseError::FolderProfileInputInvalid)?;
        let remainder_start = prefix_chars
            .checked_add(2)
            .ok_or(DatabaseError::FolderProfileInputInvalid)?;
        let query_limit = to_i64(
            entry_limit
                .checked_add(1)
                .ok_or(DatabaseError::FolderProfileInputInvalid)?,
        )?;
        let separator = MAIN_SEPARATOR.to_string();
        let mut statement = self.connection.prepare(
            "SELECT n.kind, COALESCE(f.size_bytes, 0), f.modified_unix_ns, l.display_path, \
                    CASE WHEN instr(substr(l.path_key, ?4), ?5) = 0 THEN 1 ELSE 0 END \
             FROM locations l \
             JOIN nodes n ON n.id = l.node_id \
             LEFT JOIN files f ON f.node_id = l.node_id \
             WHERE l.scope_id = ?1 AND l.present = 1 \
               AND length(l.path_key) > ?2 \
               AND substr(l.path_key, 1, ?2) = ?3 \
               AND substr(l.path_key, ?2 + 1, 1) = ?5 \
             ORDER BY l.id \
             LIMIT ?6",
        )?;
        let mut rows = statement.query(params![
            scope_id,
            prefix_chars,
            folder.1,
            remainder_start,
            separator,
            query_limit,
        ])?;
        let mut processed = 0_u64;
        let mut direct_file_count = 0_u64;
        let mut direct_folder_count = 0_u64;
        let mut descendant_file_count = 0_u64;
        let mut descendant_folder_count = 0_u64;
        let mut total_file_bytes = 0_u64;
        let mut latest_modified_unix_ns = None;
        let mut category_counts = [0_u64; 7];
        let mut project_markers = std::collections::BTreeSet::new();
        while let Some(row) = rows.next()? {
            processed = processed
                .checked_add(1)
                .ok_or(DatabaseError::InvalidCount)?;
            if processed > entry_limit {
                return Err(DatabaseError::FolderProfileTooLarge);
            }
            let kind = NodeKind::from_db(&row.get::<_, String>(0)?)?;
            let size_bytes = row_u64(row, 1)?;
            let modified_unix_ns: Option<i64> = row.get(2)?;
            let display_path: String = row.get(3)?;
            let is_direct = row.get::<_, i64>(4)? != 0;
            match kind {
                NodeKind::File => {
                    descendant_file_count = descendant_file_count
                        .checked_add(1)
                        .ok_or(DatabaseError::InvalidCount)?;
                    if is_direct {
                        direct_file_count = direct_file_count
                            .checked_add(1)
                            .ok_or(DatabaseError::InvalidCount)?;
                    }
                    total_file_bytes = total_file_bytes
                        .checked_add(size_bytes)
                        .ok_or(DatabaseError::InvalidCount)?;
                    if let Some(modified) = modified_unix_ns {
                        latest_modified_unix_ns = Some(
                            latest_modified_unix_ns
                                .map_or(modified, |current: i64| current.max(modified)),
                        );
                    }
                    let category = file_category(Path::new(&display_path));
                    let category_index = folder_category_index(category);
                    category_counts[category_index] = category_counts[category_index]
                        .checked_add(1)
                        .ok_or(DatabaseError::InvalidCount)?;
                    if is_direct
                        && let Some(marker) = project_marker(Path::new(&display_path), kind)
                    {
                        project_markers.insert(marker);
                    }
                }
                NodeKind::Folder => {
                    descendant_folder_count = descendant_folder_count
                        .checked_add(1)
                        .ok_or(DatabaseError::InvalidCount)?;
                    if is_direct {
                        direct_folder_count = direct_folder_count
                            .checked_add(1)
                            .ok_or(DatabaseError::InvalidCount)?;
                        if let Some(marker) = project_marker(Path::new(&display_path), kind) {
                            project_markers.insert(marker);
                        }
                    }
                }
            }
        }
        let observed_at_unix_ms: i64 = self.connection.query_row(
            "SELECT COALESCE(MAX(COALESCE(finished_at_unix_ms, started_at_unix_ms)), 0) \
             FROM scan_jobs WHERE scope_id = ?1 AND status = 'completed'",
            [scope_id],
            |row| row.get(0),
        )?;
        let file_categories = FolderFileCategory::ALL
            .into_iter()
            .enumerate()
            .filter_map(|(index, category)| {
                let file_count = category_counts[index];
                (file_count > 0).then_some(FolderCategoryCount {
                    category,
                    file_count,
                })
            })
            .collect();
        Ok(FolderProfileFacts {
            scope_id,
            folder_node_id,
            folder_location_id: folder.0,
            display_path: folder.2,
            direct_file_count,
            direct_folder_count,
            descendant_file_count,
            descendant_folder_count,
            total_file_bytes,
            latest_modified_unix_ns,
            file_categories,
            project_markers: project_markers.into_iter().collect(),
            observed_at_unix_ms,
            bounded_entry_limit: entry_limit,
        })
    }

    /// Finds bounded project roots from direct-child marker entries already in
    /// the current manifest. It never traverses the live filesystem and
    /// requires both a completed scan and a durable active scope grant.
    pub fn project_discovery_roots(
        &self,
        scope_id: i64,
        root_limit: u32,
    ) -> Result<ProjectDiscoveryRoots, DatabaseError> {
        if scope_id <= 0 || root_limit == 0 || root_limit > 100 {
            return Err(DatabaseError::ProjectCandidateInputInvalid);
        }
        if !self.scope_has_active_access_grant(scope_id)? {
            return Err(DatabaseError::ScopeAccessGrantNotActive);
        }
        if !self.scope_has_completed_scan(scope_id)? {
            return Err(DatabaseError::ScanJobIncomplete);
        }

        let row_limit = to_i64(
            MAX_FOLDER_PROFILE_ENTRIES
                .checked_add(1)
                .ok_or(DatabaseError::InvalidCount)?,
        )?;
        let separator = MAIN_SEPARATOR.to_string();
        let mut statement = self.connection.prepare(
            "SELECT parent_edge.target_node_id, marker_node.kind, marker_location.display_path \
             FROM edges parent_edge \
             JOIN nodes marker_node ON marker_node.id = parent_edge.source_node_id \
             JOIN locations marker_location \
               ON marker_location.scope_id = parent_edge.scope_id \
              AND marker_location.node_id = marker_node.id \
              AND marker_location.present = 1 \
             JOIN nodes root_node \
               ON root_node.id = parent_edge.target_node_id AND root_node.kind = 'folder' \
             JOIN locations root_location \
               ON root_location.scope_id = parent_edge.scope_id \
              AND root_location.node_id = root_node.id \
              AND root_location.present = 1 \
             WHERE parent_edge.scope_id = ?1 \
               AND parent_edge.kind = 'located_in' AND parent_edge.active = 1 \
               AND length(marker_location.path_key) > length(root_location.path_key) \
               AND substr(marker_location.path_key, 1, length(root_location.path_key)) \
                   = root_location.path_key \
               AND substr(marker_location.path_key, length(root_location.path_key) + 1, 1) = ?2 \
               AND instr( \
                    substr(marker_location.path_key, length(root_location.path_key) + 2), ?2 \
               ) = 0 \
             ORDER BY parent_edge.target_node_id ASC, marker_location.id ASC \
             LIMIT ?3",
        )?;
        let mut rows = statement.query(params![scope_id, separator, row_limit])?;
        let mut roots = std::collections::BTreeSet::new();
        let mut processed_rows = 0_u64;
        let mut evaluation_complete = true;
        while let Some(row) = rows.next()? {
            processed_rows = processed_rows
                .checked_add(1)
                .ok_or(DatabaseError::InvalidCount)?;
            if processed_rows > MAX_FOLDER_PROFILE_ENTRIES {
                evaluation_complete = false;
                break;
            }
            let root_node_id = row.get::<_, i64>(0)?;
            let marker_kind = NodeKind::from_db(&row.get::<_, String>(1)?)?;
            let display_path = row.get::<_, String>(2)?;
            if project_marker(Path::new(&display_path), marker_kind)
                .is_some_and(|kind| kind != ProjectSignalKind::Readme)
            {
                roots.insert(root_node_id);
                if roots.len()
                    > usize::try_from(root_limit).map_err(|_| DatabaseError::InvalidCount)?
                {
                    roots.pop_last();
                    evaluation_complete = false;
                    break;
                }
            }
        }
        Ok(ProjectDiscoveryRoots {
            root_folder_node_ids: roots.into_iter().collect(),
            evaluation_complete,
        })
    }

    pub fn record_project_candidate_with_policy(
        &mut self,
        policy_binding: ScopePolicyBinding,
        root_folder_node_id: i64,
        suggestion: &ProjectSuggestion,
    ) -> Result<ProjectCandidate, DatabaseError> {
        let scope_id = policy_binding.scope_id;
        validate_project_suggestion(scope_id, root_folder_node_id, suggestion)?;
        let current_facts =
            self.folder_profile_facts(scope_id, root_folder_node_id, MAX_FOLDER_PROFILE_ENTRIES)?;
        let suggested_kinds = suggestion
            .provenance
            .iter()
            .map(|signal| signal.kind)
            .collect::<Vec<_>>();
        if current_facts.observed_at_unix_ms != suggestion.observed_at_unix_ms
            || current_facts.project_markers != suggested_kinds
        {
            return Err(DatabaseError::ProjectCandidateInputInvalid);
        }
        let now = unix_ms()?;
        let transaction = self.connection.transaction()?;
        assert_scope_policy_binding_in_transaction(&transaction, policy_binding)?;
        let root_path_key = transaction
            .query_row(
                "SELECT l.path_key FROM locations l \
                 JOIN nodes n ON n.id = l.node_id AND n.kind = 'folder' \
                 WHERE l.scope_id = ?1 AND l.node_id = ?2 AND l.present = 1 \
                 ORDER BY l.id LIMIT 1",
                params![scope_id, root_folder_node_id],
                |row| row.get::<_, String>(0),
            )
            .optional()?;
        let Some(root_path_key) = root_path_key else {
            return Err(DatabaseError::ProjectCandidateRootNotCurrent);
        };
        assert_scope_path_key_allowed(&transaction, scope_id, &root_path_key)?;
        let current_marker_kinds = {
            let mut statement = transaction.prepare(
                "SELECT n.kind, l.path_key, l.display_path FROM edges e \
                 JOIN nodes n ON n.id=e.source_node_id \
                 JOIN locations l ON l.scope_id=e.scope_id AND l.node_id=n.id AND l.present=1 \
                 WHERE e.scope_id=?1 AND e.target_node_id=?2 \
                   AND e.kind='located_in' AND e.active=1 \
                 ORDER BY l.id",
            )?;
            let rows = statement.query_map(params![scope_id, root_folder_node_id], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                ))
            })?;
            let mut kinds = std::collections::BTreeSet::new();
            for row in rows {
                let (kind, path_key, display_path) = row?;
                let kind = NodeKind::from_db(&kind)?;
                if let Some(marker) = project_marker(Path::new(&display_path), kind) {
                    assert_scope_path_key_allowed(&transaction, scope_id, &path_key)?;
                    kinds.insert(marker);
                }
            }
            kinds.into_iter().collect::<Vec<_>>()
        };
        if current_marker_kinds != suggested_kinds {
            return Err(DatabaseError::ProjectCandidateInputInvalid);
        }
        transaction.execute(
            "INSERT OR IGNORE INTO projects( \
                 api_version, scope_id, root_folder_node_id, created_at_unix_ms \
             ) VALUES (?1, ?2, ?3, ?4)",
            params![
                ProjectCandidate::API_VERSION,
                scope_id,
                root_folder_node_id,
                now,
            ],
        )?;
        let project_id: i64 = transaction.query_row(
            "SELECT id FROM projects WHERE scope_id = ?1 AND root_folder_node_id = ?2",
            params![scope_id, root_folder_node_id],
            |row| row.get(0),
        )?;
        let suggestion_inserted = transaction.execute(
            "INSERT OR IGNORE INTO project_suggestions( \
                 project_id, confidence_basis_points, observed_at_unix_ms, provider_id, \
                 provider_version, model_version, created_at_unix_ms \
             ) VALUES (?1, ?2, ?3, ?4, ?5, NULL, ?6)",
            params![
                project_id,
                i64::from(suggestion.confidence_basis_points),
                suggestion.observed_at_unix_ms,
                suggestion.provider_id,
                suggestion.provider_version,
                now,
            ],
        )?;
        let suggestion_id: i64 = transaction.query_row(
            "SELECT id FROM project_suggestions \
             WHERE project_id = ?1 AND observed_at_unix_ms = ?2 \
               AND provider_id = ?3 AND provider_version = ?4",
            params![
                project_id,
                suggestion.observed_at_unix_ms,
                suggestion.provider_id,
                suggestion.provider_version,
            ],
            |row| row.get(0),
        )?;
        if suggestion_inserted == 1 {
            for (index, signal) in suggestion.provenance.iter().enumerate() {
                transaction.execute(
                    "INSERT INTO project_suggestion_signals( \
                         suggestion_id, ordinal, signal_kind, marker_name, weight_basis_points \
                     ) VALUES (?1, ?2, ?3, ?4, ?5)",
                    params![
                        suggestion_id,
                        to_i64(u64::try_from(index + 1).map_err(|_| DatabaseError::InvalidCount)?)?,
                        project_signal_kind_str(signal.kind),
                        signal.marker_name.as_str(),
                        i64::from(signal.weight_basis_points),
                    ],
                )?;
            }
        }
        transaction.commit()?;
        self.project_candidate(project_id)
    }

    #[cfg(test)]
    pub fn record_project_candidate(
        &mut self,
        scope_id: i64,
        root_folder_node_id: i64,
        suggestion: &ProjectSuggestion,
    ) -> Result<ProjectCandidate, DatabaseError> {
        let binding = test_active_binding(self, scope_id)?;
        self.record_project_candidate_with_policy(binding, root_folder_node_id, suggestion)
    }

    pub fn project_candidate(&self, project_id: i64) -> Result<ProjectCandidate, DatabaseError> {
        if project_id <= 0 {
            return Err(DatabaseError::ProjectCandidateInputInvalid);
        }
        let row = self
            .connection
            .query_row(
                "SELECT p.id, p.scope_id, p.root_folder_node_id, l.id, l.display_path, \
                        s.id, s.confidence_basis_points, s.observed_at_unix_ms, \
                        s.provider_id, s.provider_version, s.model_version, \
                        f.sequence, f.decision, f.created_by, f.created_at_unix_ms \
                 FROM projects p \
                 JOIN nodes n ON n.id = p.root_folder_node_id AND n.kind = 'folder' \
                 JOIN locations l ON l.scope_id = p.scope_id \
                    AND l.node_id = p.root_folder_node_id AND l.present = 1 \
                 JOIN project_suggestions s ON s.id = ( \
                    SELECT latest.id FROM project_suggestions latest \
                    WHERE latest.project_id = p.id \
                    ORDER BY latest.observed_at_unix_ms DESC, latest.id DESC LIMIT 1 \
                 ) \
                 LEFT JOIN project_feedback_events f ON f.id = ( \
                    SELECT latest_feedback.id FROM project_feedback_events latest_feedback \
                    WHERE latest_feedback.project_id = p.id \
                    ORDER BY latest_feedback.sequence DESC LIMIT 1 \
                 ) \
                 WHERE p.id = ?1 \
                 ORDER BY l.id LIMIT 1",
                [project_id],
                |row| {
                    Ok((
                        row.get::<_, i64>(0)?,
                        row.get::<_, i64>(1)?,
                        row.get::<_, i64>(2)?,
                        row.get::<_, i64>(3)?,
                        row.get::<_, String>(4)?,
                        row.get::<_, i64>(5)?,
                        row.get::<_, i64>(6)?,
                        row.get::<_, i64>(7)?,
                        row.get::<_, String>(8)?,
                        row.get::<_, String>(9)?,
                        row.get::<_, Option<String>>(10)?,
                        row.get::<_, Option<i64>>(11)?,
                        row.get::<_, Option<String>>(12)?,
                        row.get::<_, Option<String>>(13)?,
                        row.get::<_, Option<i64>>(14)?,
                    ))
                },
            )
            .optional()?;
        let Some((
            project_id,
            scope_id,
            root_folder_node_id,
            root_folder_location_id,
            display_path,
            suggestion_id,
            confidence_basis_points,
            observed_at_unix_ms,
            provider_id,
            provider_version,
            model_version,
            decision_sequence,
            decision_kind,
            decision_creator,
            decided_at_unix_ms,
        )) = row
        else {
            let project_exists = self.connection.query_row(
                "SELECT EXISTS(SELECT 1 FROM projects WHERE id = ?1)",
                [project_id],
                |row| row.get::<_, i64>(0),
            )? != 0;
            return Err(if project_exists {
                DatabaseError::ProjectCandidateRootNotCurrent
            } else {
                DatabaseError::ProjectCandidateNotFound
            });
        };
        if provider_id != ProjectSuggestion::PROVIDER_ID
            || provider_version != ProjectSuggestion::PROVIDER_VERSION
            || model_version.is_some()
        {
            return Err(DatabaseError::InvalidStoredValue);
        }
        let mut statement = self.connection.prepare(
            "SELECT signal_kind, marker_name, weight_basis_points \
             FROM project_suggestion_signals \
             WHERE suggestion_id = ?1 ORDER BY ordinal",
        )?;
        let rows = statement.query_map([suggestion_id], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, i64>(2)?,
            ))
        })?;
        let mut provenance = Vec::new();
        for row in rows {
            let (kind, marker_name, weight_basis_points) = row?;
            provenance.push(ProjectSignal {
                kind: project_signal_kind_from_str(&kind)?,
                marker_name,
                weight_basis_points: u16::try_from(weight_basis_points)
                    .map_err(|_| DatabaseError::InvalidStoredValue)?,
            });
        }
        let suggestion = ProjectSuggestion {
            confidence_basis_points: u16::try_from(confidence_basis_points)
                .map_err(|_| DatabaseError::InvalidStoredValue)?,
            provenance,
            observed_at_unix_ms,
            created_by: ProjectSuggestionCreator::SystemRule,
            provider_id: ProjectSuggestion::PROVIDER_ID,
            provider_version: ProjectSuggestion::PROVIDER_VERSION,
            model_version: None,
        };
        let latest_decision = match (
            decision_sequence,
            decision_kind,
            decision_creator,
            decided_at_unix_ms,
        ) {
            (None, None, None, None) => None,
            (Some(sequence), Some(kind), Some(creator), Some(decided_at_unix_ms)) => {
                if creator != "user" {
                    return Err(DatabaseError::InvalidStoredValue);
                }
                Some(ProjectDecision {
                    sequence: u64::try_from(sequence)
                        .map_err(|_| DatabaseError::InvalidStoredValue)?,
                    kind: project_decision_kind_from_str(&kind)?,
                    created_by: ProjectDecisionCreator::User,
                    decided_at_unix_ms,
                })
            }
            _ => return Err(DatabaseError::InvalidStoredValue),
        };
        let state = match latest_decision.as_ref().map(|decision| decision.kind) {
            None => ProjectCandidateState::Suggested,
            Some(ProjectDecisionKind::Accepted) => ProjectCandidateState::Accepted,
            Some(ProjectDecisionKind::Rejected) => ProjectCandidateState::Rejected,
        };
        Ok(ProjectCandidate {
            api_version: ProjectCandidate::API_VERSION,
            project_id,
            scope_id,
            root_folder_node_id,
            root_folder_location_id,
            display_path,
            state,
            suggestion,
            latest_decision,
        })
    }

    pub fn decide_project_candidate_with_policy(
        &mut self,
        policy_binding: ScopePolicyBinding,
        project_id: i64,
        decision: ProjectDecisionKind,
    ) -> Result<ProjectCandidate, DatabaseError> {
        if project_id <= 0 {
            return Err(DatabaseError::ProjectCandidateInputInvalid);
        }
        let now = unix_ms()?;
        let transaction = self.connection.transaction()?;
        assert_scope_policy_binding_in_transaction(&transaction, policy_binding)?;
        let project_exists = transaction.query_row(
            "SELECT EXISTS(SELECT 1 FROM projects WHERE id = ?1 AND scope_id = ?2)",
            params![project_id, policy_binding.scope_id],
            |row| row.get::<_, i64>(0),
        )? != 0;
        if !project_exists {
            return Err(DatabaseError::ProjectCandidateNotFound);
        }
        let root_is_current = transaction.query_row(
            "SELECT EXISTS( \
                 SELECT 1 FROM projects p \
                 JOIN nodes n ON n.id = p.root_folder_node_id AND n.kind = 'folder' \
                 JOIN locations l ON l.scope_id = p.scope_id \
                    AND l.node_id = p.root_folder_node_id AND l.present = 1 \
                 WHERE p.id = ?1 \
             )",
            [project_id],
            |row| row.get::<_, i64>(0),
        )? != 0;
        if !root_is_current {
            return Err(DatabaseError::ProjectCandidateRootNotCurrent);
        }
        let latest = transaction
            .query_row(
                "SELECT sequence, decision FROM project_feedback_events \
                 WHERE project_id = ?1 ORDER BY sequence DESC LIMIT 1",
                [project_id],
                |row| Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?)),
            )
            .optional()?;
        if latest
            .as_ref()
            .is_some_and(|(_, stored)| stored == project_decision_kind_str(decision))
        {
            transaction.commit()?;
            return self.project_candidate(project_id);
        }
        let sequence = latest.map_or(Ok(1_i64), |(sequence, _)| {
            sequence.checked_add(1).ok_or(DatabaseError::InvalidCount)
        })?;
        transaction.execute(
            "INSERT INTO project_feedback_events( \
                 project_id, sequence, decision, created_by, created_at_unix_ms \
             ) VALUES (?1, ?2, ?3, 'user', ?4)",
            params![
                project_id,
                sequence,
                project_decision_kind_str(decision),
                now
            ],
        )?;
        transaction.commit()?;
        self.project_candidate(project_id)
    }

    #[cfg(test)]
    pub fn decide_project_candidate(
        &mut self,
        project_id: i64,
        decision: ProjectDecisionKind,
    ) -> Result<ProjectCandidate, DatabaseError> {
        let scope_id: i64 = self.connection.query_row(
            "SELECT scope_id FROM projects WHERE id=?1",
            [project_id],
            |row| row.get(0),
        )?;
        let binding = test_active_binding(self, scope_id)?;
        self.decide_project_candidate_with_policy(binding, project_id, decision)
    }

    pub fn recent_project_candidates(&self) -> Result<Vec<ProjectCandidateSummary>, DatabaseError> {
        let project_ids = {
            let mut statement = self.connection.prepare(
                "SELECT p.id FROM projects p \
                 WHERE EXISTS( \
                    SELECT 1 FROM locations l \
                    JOIN nodes n ON n.id = l.node_id AND n.kind = 'folder' \
                    WHERE l.scope_id = p.scope_id \
                      AND l.node_id = p.root_folder_node_id AND l.present = 1 \
                 ) \
                 ORDER BY p.id DESC LIMIT 20",
            )?;
            let rows = statement.query_map([], |row| row.get::<_, i64>(0))?;
            rows.collect::<Result<Vec<_>, _>>()?
        };
        project_ids
            .into_iter()
            .map(|project_id| {
                let candidate = self.project_candidate(project_id)?;
                Ok(ProjectCandidateSummary {
                    api_version: ProjectCandidateSummary::API_VERSION,
                    project_id: candidate.project_id,
                    scope_id: candidate.scope_id,
                    root_folder_node_id: candidate.root_folder_node_id,
                    state: candidate.state,
                    confidence_basis_points: candidate.suggestion.confidence_basis_points,
                    observed_at_unix_ms: candidate.suggestion.observed_at_unix_ms,
                    latest_decision_at_unix_ms: candidate
                        .latest_decision
                        .map(|decision| decision.decided_at_unix_ms),
                })
            })
            .collect()
    }

    pub fn record_exact_duplicate_candidate_with_policy(
        &mut self,
        policy_binding: ScopePolicyBinding,
        left: &ActionSourceRecord,
        right: &ActionSourceRecord,
    ) -> Result<FileRelationCandidate, DatabaseError> {
        validate_exact_duplicate_sources(left, right)?;
        if policy_binding.scope_id != left.scope_id {
            return Err(DatabaseError::ScopePolicyRevisionStale);
        }
        let observed_at = unix_ms()?;
        let transaction = self.connection.transaction()?;
        assert_scope_policy_binding_in_transaction(&transaction, policy_binding)?;
        assert_scope_path_key_allowed(&transaction, left.scope_id, &left.path_key)?;
        assert_scope_path_key_allowed(&transaction, right.scope_id, &right.path_key)?;
        if !relation_snapshot_matches(&transaction, left)?
            || !relation_snapshot_matches(&transaction, right)?
        {
            return Err(DatabaseError::FileRelationCandidateNotCurrent);
        }
        transaction.execute(
            "INSERT OR IGNORE INTO file_relation_candidates( \
                 api_version, relation_kind, scope_id, left_node_id, right_node_id, \
                 created_at_unix_ms \
             ) VALUES (?1, 'exact_duplicate', ?2, ?3, ?4, ?5)",
            params![
                FileRelationCandidate::API_VERSION,
                left.scope_id,
                left.node_id,
                right.node_id,
                observed_at,
            ],
        )?;
        let relation_id: i64 = transaction.query_row(
            "SELECT id FROM file_relation_candidates \
             WHERE relation_kind = 'exact_duplicate' AND scope_id = ?1 \
               AND left_node_id = ?2 AND right_node_id = ?3",
            params![left.scope_id, left.node_id, right.node_id],
            |row| row.get(0),
        )?;
        transaction.execute(
            "INSERT INTO file_relation_observations( \
                 relation_id, left_location_id, right_location_id, source_size_bytes, \
                 left_modified_unix_ns, right_modified_unix_ns, compared_bytes, \
                 confidence_basis_points, comparison_kind, created_by, provider_id, \
                 provider_version, model_version, observed_at_unix_ms \
             ) VALUES ( \
                 ?1, ?2, ?3, ?4, ?5, ?6, ?4, 10000, 'byte_for_byte', 'system_rule', \
                 'deskgraph.byte-equality', '1', NULL, ?7 \
             )",
            params![
                relation_id,
                left.location_id,
                right.location_id,
                to_i64(left.size_bytes)?,
                left.modified_unix_ns,
                right.modified_unix_ns,
                observed_at,
            ],
        )?;
        transaction.commit()?;
        self.file_relation_candidate(relation_id)
    }

    #[cfg(test)]
    pub fn record_exact_duplicate_candidate(
        &mut self,
        left: &ActionSourceRecord,
        right: &ActionSourceRecord,
    ) -> Result<FileRelationCandidate, DatabaseError> {
        let binding = test_active_binding(self, left.scope_id)?;
        self.record_exact_duplicate_candidate_with_policy(binding, left, right)
    }

    pub fn record_file_version_candidate_with_policy(
        &mut self,
        policy_binding: ScopePolicyBinding,
        first: &ActionSourceRecord,
        second: &ActionSourceRecord,
    ) -> Result<FileVersionCandidate, DatabaseError> {
        validate_file_relation_sources(first, second)?;
        if policy_binding.scope_id != first.scope_id {
            return Err(DatabaseError::ScopePolicyRevisionStale);
        }
        let first_name = explicit_version_name_from_source(first)?;
        let second_name = explicit_version_name_from_source(second)?;
        if first_name.base_key != second_name.base_key
            || first_name.extension_key != second_name.extension_key
            || first_name.version == second_name.version
        {
            return Err(DatabaseError::FileRelationCandidateInputInvalid);
        }
        let (older, older_name, newer, newer_name) = if first_name.version < second_name.version {
            (first, first_name, second, second_name)
        } else {
            (second, second_name, first, first_name)
        };
        let (left, right) = if first.node_id < second.node_id {
            (first, second)
        } else {
            (second, first)
        };
        let observed_at = unix_ms()?;
        let transaction = self.connection.transaction()?;
        assert_scope_policy_binding_in_transaction(&transaction, policy_binding)?;
        assert_scope_path_key_allowed(&transaction, first.scope_id, &first.path_key)?;
        assert_scope_path_key_allowed(&transaction, second.scope_id, &second.path_key)?;
        if !relation_snapshot_matches(&transaction, first)?
            || !relation_snapshot_matches(&transaction, second)?
        {
            return Err(DatabaseError::FileRelationCandidateNotCurrent);
        }
        transaction.execute(
            "INSERT OR IGNORE INTO file_relation_candidates( \
                 api_version, relation_kind, scope_id, left_node_id, right_node_id, \
                 created_at_unix_ms \
             ) VALUES (?1, 'version', ?2, ?3, ?4, ?5)",
            params![
                FileRelationCandidate::API_VERSION,
                left.scope_id,
                left.node_id,
                right.node_id,
                observed_at,
            ],
        )?;
        let relation_id: i64 = transaction.query_row(
            "SELECT id FROM file_relation_candidates \
             WHERE relation_kind = 'version' AND scope_id = ?1 \
               AND left_node_id = ?2 AND right_node_id = ?3",
            params![left.scope_id, left.node_id, right.node_id],
            |row| row.get(0),
        )?;
        transaction.execute(
            "INSERT INTO file_version_observations( \
                 relation_id, older_location_id, newer_location_id, older_size_bytes, \
                 newer_size_bytes, older_modified_unix_ns, newer_modified_unix_ns, \
                 base_key, extension_key, older_version, newer_version, \
                 confidence_basis_points, signal_kind, created_by, provider_id, \
                 provider_version, model_version, observed_at_unix_ms \
             ) VALUES ( \
                 ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, 9000, \
                 'explicit_numeric_suffix', 'system_rule', \
                 'deskgraph.filename-version', '1', NULL, ?12 \
             )",
            params![
                relation_id,
                older.location_id,
                newer.location_id,
                to_i64(older.size_bytes)?,
                to_i64(newer.size_bytes)?,
                older.modified_unix_ns,
                newer.modified_unix_ns,
                older_name.base_key,
                older_name.extension_key,
                i64::from(older_name.version),
                i64::from(newer_name.version),
                observed_at,
            ],
        )?;
        transaction.commit()?;
        self.file_version_candidate(relation_id)
    }

    #[cfg(test)]
    pub fn record_file_version_candidate(
        &mut self,
        first: &ActionSourceRecord,
        second: &ActionSourceRecord,
    ) -> Result<FileVersionCandidate, DatabaseError> {
        let binding = test_active_binding(self, first.scope_id)?;
        self.record_file_version_candidate_with_policy(binding, first, second)
    }

    fn file_version_candidate(
        &self,
        relation_id: i64,
    ) -> Result<FileVersionCandidate, DatabaseError> {
        Self::file_version_candidate_from_connection(&self.connection, relation_id)
    }

    fn file_version_candidate_from_connection(
        connection: &Connection,
        relation_id: i64,
    ) -> Result<FileVersionCandidate, DatabaseError> {
        if relation_id <= 0 {
            return Err(DatabaseError::FileRelationCandidateInputInvalid);
        }
        let row = connection
            .query_row(
                "SELECT r.id, r.relation_kind, r.scope_id, r.left_node_id, r.right_node_id, \
                        older_location.node_id, older_location.id, \
                        older_location.display_path, older_file.size_bytes, \
                        older_file.modified_unix_ns, newer_location.node_id, \
                        newer_location.id, newer_location.display_path, newer_file.size_bytes, \
                        newer_file.modified_unix_ns, observation.base_key, \
                        observation.extension_key, observation.older_version, \
                        observation.newer_version, observation.confidence_basis_points, \
                        observation.signal_kind, observation.created_by, \
                        observation.provider_id, observation.provider_version, \
                        observation.model_version, observation.observed_at_unix_ms, \
                        observation.id, feedback.id \
                 FROM file_relation_candidates r \
                 JOIN file_version_observations observation ON observation.id = ( \
                    SELECT latest.id FROM file_version_observations latest \
                    WHERE latest.relation_id = r.id \
                    ORDER BY latest.observed_at_unix_ms DESC, latest.id DESC LIMIT 1 \
                 ) \
                 JOIN locations older_location ON older_location.id = observation.older_location_id \
                    AND older_location.scope_id = r.scope_id AND older_location.present = 1 \
                 JOIN nodes older_node ON older_node.id = older_location.node_id \
                    AND older_node.kind = 'file' \
                 JOIN files older_file ON older_file.node_id = older_location.node_id \
                    AND older_file.size_bytes = observation.older_size_bytes \
                    AND older_file.modified_unix_ns IS observation.older_modified_unix_ns \
                 JOIN locations newer_location ON newer_location.id = observation.newer_location_id \
                    AND newer_location.scope_id = r.scope_id AND newer_location.present = 1 \
                 JOIN nodes newer_node ON newer_node.id = newer_location.node_id \
                    AND newer_node.kind = 'file' \
                 JOIN files newer_file ON newer_file.node_id = newer_location.node_id \
                    AND newer_file.size_bytes = observation.newer_size_bytes \
                    AND newer_file.modified_unix_ns IS observation.newer_modified_unix_ns \
                 LEFT JOIN file_relation_feedback_events feedback \
                    ON feedback.relation_id = r.id \
                 WHERE r.id = ?1 LIMIT 1",
                [relation_id],
                |row| {
                    Ok((
                        row.get::<_, i64>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, i64>(2)?,
                        row.get::<_, i64>(3)?,
                        row.get::<_, i64>(4)?,
                        row.get::<_, i64>(5)?,
                        row.get::<_, i64>(6)?,
                        row.get::<_, String>(7)?,
                        row.get::<_, i64>(8)?,
                        row.get::<_, Option<i64>>(9)?,
                        row.get::<_, i64>(10)?,
                        row.get::<_, i64>(11)?,
                        row.get::<_, String>(12)?,
                        row.get::<_, i64>(13)?,
                        row.get::<_, Option<i64>>(14)?,
                        row.get::<_, String>(15)?,
                        row.get::<_, String>(16)?,
                        row.get::<_, i64>(17)?,
                        row.get::<_, i64>(18)?,
                        row.get::<_, i64>(19)?,
                        row.get::<_, String>(20)?,
                        row.get::<_, String>(21)?,
                        row.get::<_, String>(22)?,
                        row.get::<_, String>(23)?,
                        row.get::<_, Option<String>>(24)?,
                        row.get::<_, i64>(25)?,
                        row.get::<_, i64>(26)?,
                        row.get::<_, Option<i64>>(27)?,
                    ))
                },
            )
            .optional()?;
        let Some((
            relation_id,
            relation_kind,
            scope_id,
            left_node_id,
            right_node_id,
            older_node_id,
            older_location_id,
            older_display_path,
            older_size_bytes,
            older_modified_unix_ns,
            newer_node_id,
            newer_location_id,
            newer_display_path,
            newer_size_bytes,
            newer_modified_unix_ns,
            base_key,
            extension_key,
            older_version,
            newer_version,
            confidence_basis_points,
            signal_kind,
            created_by,
            provider_id,
            provider_version,
            model_version,
            observed_at_unix_ms,
            observation_id,
            feedback_id,
        )) = row
        else {
            let relation_exists = connection.query_row(
                "SELECT EXISTS(SELECT 1 FROM file_relation_candidates WHERE id = ?1)",
                [relation_id],
                |row| row.get::<_, i64>(0),
            )? != 0;
            return Err(if relation_exists {
                DatabaseError::FileRelationCandidateNotCurrent
            } else {
                DatabaseError::FileRelationCandidateNotFound
            });
        };
        let current_older = explicit_version_name_from_display_path(&older_display_path)?;
        let current_newer = explicit_version_name_from_display_path(&newer_display_path)?;
        let stable_nodes_match = left_node_id == older_node_id.min(newer_node_id)
            && right_node_id == older_node_id.max(newer_node_id);
        if relation_kind != "version"
            || !stable_nodes_match
            || older_node_id == newer_node_id
            || base_key != current_older.base_key
            || base_key != current_newer.base_key
            || extension_key != current_older.extension_key
            || extension_key != current_newer.extension_key
            || older_version != i64::from(current_older.version)
            || newer_version != i64::from(current_newer.version)
            || older_version >= newer_version
            || confidence_basis_points != 9_000
            || signal_kind != "explicit_numeric_suffix"
            || created_by != "system_rule"
            || provider_id != FileVersionEvidence::PROVIDER_ID
            || provider_version != FileVersionEvidence::PROVIDER_VERSION
            || model_version.is_some()
            || observation_id <= 0
            || feedback_id.is_some()
        {
            return Err(DatabaseError::InvalidStoredValue);
        }
        let latest_decision =
            latest_equivalent_file_version_decision(connection, relation_id, observation_id)?;
        let state = match latest_decision.as_ref().map(|decision| decision.kind) {
            None => FileRelationCandidateState::Suggested,
            Some(FileRelationDecisionKind::Accepted) => FileRelationCandidateState::Accepted,
            Some(FileRelationDecisionKind::Rejected) => FileRelationCandidateState::Rejected,
        };
        Ok(FileVersionCandidate {
            api_version: FileVersionCandidate::API_VERSION,
            relation_id,
            kind: FileRelationKind::Version,
            state,
            older: FileRelationEndpoint {
                scope_id,
                node_id: older_node_id,
                location_id: older_location_id,
                display_path: older_display_path,
                size_bytes: row_u64_value(older_size_bytes)?,
                modified_unix_ns: older_modified_unix_ns,
            },
            newer: FileRelationEndpoint {
                scope_id,
                node_id: newer_node_id,
                location_id: newer_location_id,
                display_path: newer_display_path,
                size_bytes: row_u64_value(newer_size_bytes)?,
                modified_unix_ns: newer_modified_unix_ns,
            },
            evidence: FileVersionEvidence {
                signal_kind: FileVersionSignalKind::ExplicitNumericSuffix,
                base_key,
                extension_key,
                older_version: u32::try_from(older_version)
                    .map_err(|_| DatabaseError::InvalidStoredValue)?,
                newer_version: u32::try_from(newer_version)
                    .map_err(|_| DatabaseError::InvalidStoredValue)?,
                confidence_basis_points: u16::try_from(confidence_basis_points)
                    .map_err(|_| DatabaseError::InvalidStoredValue)?,
                observed_at_unix_ms,
                created_by: FileRelationCreator::SystemRule,
                provider_id: FileVersionEvidence::PROVIDER_ID,
                provider_version: FileVersionEvidence::PROVIDER_VERSION,
                model_version: None,
            },
            latest_decision,
        })
    }

    fn file_relation_candidate(
        &self,
        relation_id: i64,
    ) -> Result<FileRelationCandidate, DatabaseError> {
        if relation_id <= 0 {
            return Err(DatabaseError::FileRelationCandidateInputInvalid);
        }
        let row = self
            .connection
            .query_row(
                "SELECT r.id, r.relation_kind, r.scope_id, \
                        r.left_node_id, left_location.id, left_location.display_path, \
                        left_file.size_bytes, left_file.modified_unix_ns, \
                        r.right_node_id, right_location.id, right_location.display_path, \
                        right_file.size_bytes, right_file.modified_unix_ns, \
                        observation.compared_bytes, observation.confidence_basis_points, \
                        observation.comparison_kind, observation.created_by, \
                        observation.provider_id, observation.provider_version, \
                        observation.model_version, observation.observed_at_unix_ms, \
                        feedback.sequence, feedback.decision, feedback.created_by, \
                        feedback.created_at_unix_ms \
                 FROM file_relation_candidates r \
                 JOIN file_relation_observations observation ON observation.id = ( \
                    SELECT latest.id FROM file_relation_observations latest \
                    WHERE latest.relation_id = r.id \
                    ORDER BY latest.observed_at_unix_ms DESC, latest.id DESC LIMIT 1 \
                 ) \
                 JOIN nodes left_node ON left_node.id = r.left_node_id \
                    AND left_node.kind = 'file' \
                 JOIN locations left_location ON left_location.id = observation.left_location_id \
                    AND left_location.scope_id = r.scope_id \
                    AND left_location.node_id = r.left_node_id \
                    AND left_location.present = 1 \
                 JOIN files left_file ON left_file.node_id = r.left_node_id \
                    AND left_file.size_bytes = observation.source_size_bytes \
                    AND left_file.modified_unix_ns IS observation.left_modified_unix_ns \
                 JOIN nodes right_node ON right_node.id = r.right_node_id \
                    AND right_node.kind = 'file' \
                 JOIN locations right_location ON right_location.id = observation.right_location_id \
                    AND right_location.scope_id = r.scope_id \
                    AND right_location.node_id = r.right_node_id \
                    AND right_location.present = 1 \
                 JOIN files right_file ON right_file.node_id = r.right_node_id \
                    AND right_file.size_bytes = observation.source_size_bytes \
                    AND right_file.modified_unix_ns IS observation.right_modified_unix_ns \
                 LEFT JOIN file_relation_feedback_events feedback ON feedback.id = ( \
                    SELECT latest_feedback.id FROM file_relation_feedback_events latest_feedback \
                    WHERE latest_feedback.relation_id = r.id \
                    ORDER BY latest_feedback.sequence DESC LIMIT 1 \
                 ) \
                 WHERE r.id = ?1",
                [relation_id],
                |row| {
                    Ok((
                        row.get::<_, i64>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, i64>(2)?,
                        row.get::<_, i64>(3)?,
                        row.get::<_, i64>(4)?,
                        row.get::<_, String>(5)?,
                        row.get::<_, i64>(6)?,
                        row.get::<_, Option<i64>>(7)?,
                        row.get::<_, i64>(8)?,
                        row.get::<_, i64>(9)?,
                        row.get::<_, String>(10)?,
                        row.get::<_, i64>(11)?,
                        row.get::<_, Option<i64>>(12)?,
                        row.get::<_, i64>(13)?,
                        row.get::<_, i64>(14)?,
                        row.get::<_, String>(15)?,
                        row.get::<_, String>(16)?,
                        row.get::<_, String>(17)?,
                        row.get::<_, String>(18)?,
                        row.get::<_, Option<String>>(19)?,
                        row.get::<_, i64>(20)?,
                        row.get::<_, Option<i64>>(21)?,
                        row.get::<_, Option<String>>(22)?,
                        row.get::<_, Option<String>>(23)?,
                        row.get::<_, Option<i64>>(24)?,
                    ))
                },
            )
            .optional()?;
        let Some((
            relation_id,
            relation_kind,
            scope_id,
            left_node_id,
            left_location_id,
            left_display_path,
            left_size_bytes,
            left_modified_unix_ns,
            right_node_id,
            right_location_id,
            right_display_path,
            right_size_bytes,
            right_modified_unix_ns,
            compared_bytes,
            confidence_basis_points,
            comparison_kind,
            created_by,
            provider_id,
            provider_version,
            model_version,
            observed_at_unix_ms,
            decision_sequence,
            decision_kind,
            decision_creator,
            decided_at_unix_ms,
        )) = row
        else {
            let relation_exists = self.connection.query_row(
                "SELECT EXISTS(SELECT 1 FROM file_relation_candidates WHERE id = ?1)",
                [relation_id],
                |row| row.get::<_, i64>(0),
            )? != 0;
            return Err(if relation_exists {
                DatabaseError::FileRelationCandidateNotCurrent
            } else {
                DatabaseError::FileRelationCandidateNotFound
            });
        };
        if relation_kind != "exact_duplicate"
            || comparison_kind != "byte_for_byte"
            || created_by != "system_rule"
            || provider_id != FileRelationEvidence::PROVIDER_ID
            || provider_version != FileRelationEvidence::PROVIDER_VERSION
            || model_version.is_some()
            || left_size_bytes != right_size_bytes
            || compared_bytes != left_size_bytes
            || confidence_basis_points != 10_000
        {
            return Err(DatabaseError::InvalidStoredValue);
        }
        let source_size_bytes = row_u64_value(left_size_bytes)?;
        if source_size_bytes == 0 || source_size_bytes > MAX_FILE_RELATION_SOURCE_BYTES {
            return Err(DatabaseError::InvalidStoredValue);
        }
        let latest_decision = match (
            decision_sequence,
            decision_kind,
            decision_creator,
            decided_at_unix_ms,
        ) {
            (None, None, None, None) => None,
            (Some(sequence), Some(kind), Some(creator), Some(decided_at_unix_ms)) => {
                if creator != "user" {
                    return Err(DatabaseError::InvalidStoredValue);
                }
                Some(FileRelationDecision {
                    sequence: u64::try_from(sequence)
                        .map_err(|_| DatabaseError::InvalidStoredValue)?,
                    kind: file_relation_decision_kind_from_str(&kind)?,
                    created_by: FileRelationDecisionCreator::User,
                    decided_at_unix_ms,
                })
            }
            _ => return Err(DatabaseError::InvalidStoredValue),
        };
        let state = match latest_decision.as_ref().map(|decision| decision.kind) {
            None => FileRelationCandidateState::Suggested,
            Some(FileRelationDecisionKind::Accepted) => FileRelationCandidateState::Accepted,
            Some(FileRelationDecisionKind::Rejected) => FileRelationCandidateState::Rejected,
        };
        Ok(FileRelationCandidate {
            api_version: FileRelationCandidate::API_VERSION,
            relation_id,
            kind: FileRelationKind::ExactDuplicate,
            state,
            left: FileRelationEndpoint {
                scope_id,
                node_id: left_node_id,
                location_id: left_location_id,
                display_path: left_display_path,
                size_bytes: source_size_bytes,
                modified_unix_ns: left_modified_unix_ns,
            },
            right: FileRelationEndpoint {
                scope_id,
                node_id: right_node_id,
                location_id: right_location_id,
                display_path: right_display_path,
                size_bytes: source_size_bytes,
                modified_unix_ns: right_modified_unix_ns,
            },
            evidence: FileRelationEvidence {
                comparison_kind: FileRelationComparisonKind::ByteForByte,
                compared_bytes: row_u64_value(compared_bytes)?,
                confidence_basis_points: u16::try_from(confidence_basis_points)
                    .map_err(|_| DatabaseError::InvalidStoredValue)?,
                observed_at_unix_ms,
                created_by: FileRelationCreator::SystemRule,
                provider_id: FileRelationEvidence::PROVIDER_ID,
                provider_version: FileRelationEvidence::PROVIDER_VERSION,
                model_version: None,
                bounded_max_bytes: MAX_FILE_RELATION_SOURCE_BYTES,
            },
            latest_decision,
        })
    }

    pub fn decide_file_relation_candidate_with_policy(
        &mut self,
        policy_binding: ScopePolicyBinding,
        relation_id: i64,
        decision: FileRelationDecisionKind,
    ) -> Result<FileRelationCandidate, DatabaseError> {
        if relation_id <= 0 {
            return Err(DatabaseError::FileRelationCandidateInputInvalid);
        }
        let now = unix_ms()?;
        let transaction = self.connection.transaction()?;
        assert_scope_policy_binding_in_transaction(&transaction, policy_binding)?;
        let relation_exists = transaction.query_row(
            "SELECT EXISTS(SELECT 1 FROM file_relation_candidates WHERE id = ?1 AND scope_id = ?2)",
            params![relation_id, policy_binding.scope_id],
            |row| row.get::<_, i64>(0),
        )? != 0;
        if !relation_exists {
            return Err(DatabaseError::FileRelationCandidateNotFound);
        }
        let relation_is_current = transaction.query_row(
            "SELECT EXISTS( \
                 SELECT 1 FROM file_relation_candidates r \
                 JOIN file_relation_observations observation ON observation.id = ( \
                    SELECT latest.id FROM file_relation_observations latest \
                    WHERE latest.relation_id = r.id \
                    ORDER BY latest.observed_at_unix_ms DESC, latest.id DESC LIMIT 1 \
                 ) \
                 JOIN locations left_location ON left_location.id = observation.left_location_id \
                    AND left_location.scope_id = r.scope_id \
                    AND left_location.node_id = r.left_node_id AND left_location.present = 1 \
                 JOIN files left_file ON left_file.node_id = r.left_node_id \
                    AND left_file.size_bytes = observation.source_size_bytes \
                    AND left_file.modified_unix_ns IS observation.left_modified_unix_ns \
                 JOIN locations right_location ON right_location.id = observation.right_location_id \
                    AND right_location.scope_id = r.scope_id \
                    AND right_location.node_id = r.right_node_id AND right_location.present = 1 \
                 JOIN files right_file ON right_file.node_id = r.right_node_id \
                    AND right_file.size_bytes = observation.source_size_bytes \
                    AND right_file.modified_unix_ns IS observation.right_modified_unix_ns \
                 WHERE r.id = ?1 AND r.relation_kind = 'exact_duplicate' \
             )",
            [relation_id],
            |row| row.get::<_, i64>(0),
        )? != 0;
        if !relation_is_current {
            return Err(DatabaseError::FileRelationCandidateNotCurrent);
        }
        let latest = transaction
            .query_row(
                "SELECT sequence, decision FROM file_relation_feedback_events \
                 WHERE relation_id = ?1 ORDER BY sequence DESC LIMIT 1",
                [relation_id],
                |row| Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?)),
            )
            .optional()?;
        if latest
            .as_ref()
            .is_some_and(|(_, stored)| stored == file_relation_decision_kind_str(decision))
        {
            transaction.commit()?;
            return self.file_relation_candidate(relation_id);
        }
        let sequence = latest.map_or(Ok(1_i64), |(sequence, _)| {
            sequence.checked_add(1).ok_or(DatabaseError::InvalidCount)
        })?;
        transaction.execute(
            "INSERT INTO file_relation_feedback_events( \
                 relation_id, sequence, decision, created_by, created_at_unix_ms \
             ) VALUES (?1, ?2, ?3, 'user', ?4)",
            params![
                relation_id,
                sequence,
                file_relation_decision_kind_str(decision),
                now
            ],
        )?;
        transaction.commit()?;
        self.file_relation_candidate(relation_id)
    }

    #[cfg(test)]
    pub fn decide_file_relation_candidate(
        &mut self,
        relation_id: i64,
        decision: FileRelationDecisionKind,
    ) -> Result<FileRelationCandidate, DatabaseError> {
        let scope_id: i64 = self.connection.query_row(
            "SELECT scope_id FROM file_relation_candidates WHERE id=?1",
            [relation_id],
            |row| row.get(0),
        )?;
        let binding = test_active_binding(self, scope_id)?;
        self.decide_file_relation_candidate_with_policy(binding, relation_id, decision)
    }

    pub fn decide_file_version_candidate_with_policy(
        &mut self,
        policy_binding: ScopePolicyBinding,
        relation_id: i64,
        decision: FileRelationDecisionKind,
    ) -> Result<FileVersionCandidate, DatabaseError> {
        if relation_id <= 0 {
            return Err(DatabaseError::FileRelationCandidateInputInvalid);
        }
        let now = unix_ms()?;
        let transaction = self.connection.transaction()?;
        assert_scope_policy_binding_in_transaction(&transaction, policy_binding)?;
        let relation_matches_scope: i64 = transaction.query_row(
            "SELECT EXISTS(SELECT 1 FROM file_relation_candidates WHERE id=?1 AND scope_id=?2)",
            params![relation_id, policy_binding.scope_id],
            |row| row.get(0),
        )?;
        if relation_matches_scope != 1 {
            return Err(DatabaseError::FileRelationCandidateNotFound);
        }
        Self::file_version_candidate_from_connection(&transaction, relation_id)?;
        let current_observation_id = transaction.query_row(
            "SELECT id FROM file_version_observations WHERE relation_id = ?1 \
             ORDER BY observed_at_unix_ms DESC, id DESC LIMIT 1",
            [relation_id],
            |row| row.get::<_, i64>(0),
        )?;
        let latest_equivalent = latest_equivalent_file_version_decision(
            &transaction,
            relation_id,
            current_observation_id,
        )?;
        if latest_equivalent
            .as_ref()
            .is_some_and(|stored| stored.kind == decision)
        {
            transaction.commit()?;
            return self.file_version_candidate(relation_id);
        }
        let latest_sequence = transaction
            .query_row(
                "SELECT sequence FROM file_version_feedback_events \
                 WHERE relation_id = ?1 ORDER BY sequence DESC LIMIT 1",
                [relation_id],
                |row| row.get::<_, i64>(0),
            )
            .optional()?;
        let sequence = latest_sequence.map_or(Ok(1_i64), |sequence| {
            sequence.checked_add(1).ok_or(DatabaseError::InvalidCount)
        })?;
        transaction.execute(
            "INSERT INTO file_version_feedback_events( \
                 relation_id, evidence_observation_id, sequence, decision, created_by, \
                 created_at_unix_ms \
             ) VALUES (?1, ?2, ?3, ?4, 'user', ?5)",
            params![
                relation_id,
                current_observation_id,
                sequence,
                file_relation_decision_kind_str(decision),
                now,
            ],
        )?;
        transaction.commit()?;
        self.file_version_candidate(relation_id)
    }

    #[cfg(test)]
    pub fn decide_file_version_candidate(
        &mut self,
        relation_id: i64,
        decision: FileRelationDecisionKind,
    ) -> Result<FileVersionCandidate, DatabaseError> {
        let scope_id: i64 = self.connection.query_row(
            "SELECT scope_id FROM file_relation_candidates WHERE id=?1",
            [relation_id],
            |row| row.get(0),
        )?;
        let binding = test_active_binding(self, scope_id)?;
        self.decide_file_version_candidate_with_policy(binding, relation_id, decision)
    }

    pub fn recent_file_relation_candidates(
        &self,
    ) -> Result<Vec<FileRelationCandidateSummary>, DatabaseError> {
        let relations = {
            let mut statement = self.connection.prepare(
                "SELECT id, relation_kind, scope_id, left_node_id, right_node_id \
                 FROM file_relation_candidates ORDER BY id DESC LIMIT 20",
            )?;
            let rows = statement.query_map([], |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, i64>(2)?,
                    row.get::<_, i64>(3)?,
                    row.get::<_, i64>(4)?,
                ))
            })?;
            rows.collect::<Result<Vec<_>, _>>()?
        };
        relations
            .into_iter()
            .map(
                |(relation_id, relation_kind, scope_id, left_node_id, right_node_id)| {
                    self.file_relation_candidate_summary(
                        relation_id,
                        &relation_kind,
                        scope_id,
                        left_node_id,
                        right_node_id,
                    )
                },
            )
            .collect()
    }

    /// Returns a bounded, path-free source inventory for one Smart Cleanup
    /// refresh. This is only an inventory: relation entries still require the
    /// Rust service's live byte/name verification before they can become Inbox
    /// items.
    pub fn smart_cleanup_source_references(
        &self,
        scope_id: i64,
        limit: u32,
    ) -> Result<(Vec<SmartCleanupSourceReference>, bool), DatabaseError> {
        if !(1..=20).contains(&limit) {
            return Err(DatabaseError::FileRelationCandidateInputInvalid);
        }
        ensure_scope_queryable(&self.connection, scope_id)?;
        ensure_scope_access_permitted(&self.connection, scope_id)?;
        let query_limit = i64::from(limit) + 1;
        let sources = {
            let mut statement = self.connection.prepare(
                "SELECT source_kind, source_id FROM ( \
                     SELECT relation.relation_kind AS source_kind, relation.id AS source_id, \
                            CASE relation.relation_kind \
                              WHEN 'exact_duplicate' THEN ( \
                                SELECT observation.observed_at_unix_ms \
                                FROM file_relation_observations observation \
                                WHERE observation.relation_id = relation.id \
                                ORDER BY observation.observed_at_unix_ms DESC, \
                                         observation.id DESC LIMIT 1 \
                              ) \
                              WHEN 'version' THEN ( \
                                SELECT observation.observed_at_unix_ms \
                                FROM file_version_observations observation \
                                WHERE observation.relation_id = relation.id \
                                ORDER BY observation.observed_at_unix_ms DESC, \
                                         observation.id DESC LIMIT 1 \
                              ) \
                            END AS observed_at_unix_ms \
                     FROM file_relation_candidates relation \
                     WHERE relation.scope_id = ?1 \
                     UNION ALL \
                     SELECT 'screenshot_review_group' AS source_kind, groups.id AS source_id, \
                            (SELECT observation.observed_at_unix_ms \
                             FROM screenshot_group_observations observation \
                             WHERE observation.group_id = groups.id \
                             ORDER BY observation.observed_at_unix_ms DESC, \
                                      observation.id DESC LIMIT 1) AS observed_at_unix_ms \
                     FROM screenshot_group_candidates groups WHERE groups.scope_id = ?1 \
                 ) sources \
                 WHERE observed_at_unix_ms IS NOT NULL \
                 ORDER BY observed_at_unix_ms DESC, \
                          CASE source_kind WHEN 'exact_duplicate' THEN 0 \
                                           WHEN 'version' THEN 1 ELSE 2 END, \
                          source_id ASC LIMIT ?2",
            )?;
            let rows = statement.query_map(params![scope_id, query_limit], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?))
            })?;
            rows.collect::<Result<Vec<_>, _>>()?
        };
        let evaluation_complete =
            sources.len() <= usize::try_from(limit).map_err(|_| DatabaseError::InvalidCount)?;
        let mut references = Vec::with_capacity(sources.len().min(limit as usize));
        for (source_kind, source_id) in sources.into_iter().take(limit as usize) {
            let reference = match source_kind.as_str() {
                "exact_duplicate" | "version" => {
                    let (relation_kind, scope_id, left_node_id, right_node_id) =
                        self.connection.query_row(
                            "SELECT relation_kind, scope_id, left_node_id, right_node_id \
                             FROM file_relation_candidates WHERE id = ?1",
                            [source_id],
                            |row| {
                                Ok((
                                    row.get::<_, String>(0)?,
                                    row.get::<_, i64>(1)?,
                                    row.get::<_, i64>(2)?,
                                    row.get::<_, i64>(3)?,
                                ))
                            },
                        )?;
                    let summary = self.file_relation_candidate_summary(
                        source_id,
                        &relation_kind,
                        scope_id,
                        left_node_id,
                        right_node_id,
                    )?;
                    SmartCleanupSourceReference {
                        kind: match summary.kind {
                            FileRelationKind::ExactDuplicate => {
                                SmartCleanupSourceKind::ExactDuplicate
                            }
                            FileRelationKind::Version => SmartCleanupSourceKind::Version,
                        },
                        source_id,
                        state: summary.state,
                    }
                }
                "screenshot_review_group" => SmartCleanupSourceReference {
                    kind: SmartCleanupSourceKind::ScreenshotReviewGroup,
                    source_id,
                    state: FileRelationCandidateState::Suggested,
                },
                _ => return Err(DatabaseError::InvalidStoredValue),
            };
            references.push(reference);
        }
        Ok((references, evaluation_complete))
    }

    /// Converts only the latest observation produced by a caller-provided live
    /// verification into a path-free Inbox item. The observation timestamp is
    /// an explicit binding so a history row cannot be mistaken for the result
    /// of the current refresh.
    pub fn smart_cleanup_relation_item(
        &self,
        relation_id: i64,
        expected_observed_at_unix_ms: i64,
    ) -> Result<SmartCleanupInboxItem, DatabaseError> {
        let (relation_kind, scope_id, left_node_id, right_node_id) = self
            .connection
            .query_row(
                "SELECT relation_kind, scope_id, left_node_id, right_node_id \
                 FROM file_relation_candidates WHERE id = ?1",
                [relation_id],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, i64>(1)?,
                        row.get::<_, i64>(2)?,
                        row.get::<_, i64>(3)?,
                    ))
                },
            )
            .optional()?
            .ok_or(DatabaseError::FileRelationCandidateNotFound)?;
        ensure_scope_queryable(&self.connection, scope_id)?;
        ensure_scope_access_permitted(&self.connection, scope_id)?;
        let summary = self.file_relation_candidate_summary(
            relation_id,
            &relation_kind,
            scope_id,
            left_node_id,
            right_node_id,
        )?;
        if summary.state != FileRelationCandidateState::Suggested {
            return Err(DatabaseError::FileRelationCandidateNotCurrent);
        }
        let (source_kind, observation_id, confidence_basis_points, observed_at_unix_ms) =
            match summary.kind {
                FileRelationKind::ExactDuplicate => {
                    let observation = self.connection.query_row(
                        "SELECT id, confidence_basis_points, observed_at_unix_ms \
                         FROM file_relation_observations WHERE relation_id = ?1 \
                         ORDER BY observed_at_unix_ms DESC, id DESC LIMIT 1",
                        [relation_id],
                        |row| {
                            Ok((
                                row.get::<_, i64>(0)?,
                                row.get::<_, i64>(1)?,
                                row.get::<_, i64>(2)?,
                            ))
                        },
                    )?;
                    (
                        SmartCleanupSourceKind::ExactDuplicate,
                        observation.0,
                        observation.1,
                        observation.2,
                    )
                }
                FileRelationKind::Version => {
                    let observation = self.connection.query_row(
                        "SELECT id, confidence_basis_points, observed_at_unix_ms \
                         FROM file_version_observations WHERE relation_id = ?1 \
                         ORDER BY observed_at_unix_ms DESC, id DESC LIMIT 1",
                        [relation_id],
                        |row| {
                            Ok((
                                row.get::<_, i64>(0)?,
                                row.get::<_, i64>(1)?,
                                row.get::<_, i64>(2)?,
                            ))
                        },
                    )?;
                    (
                        SmartCleanupSourceKind::Version,
                        observation.0,
                        observation.1,
                        observation.2,
                    )
                }
            };
        if observation_id <= 0 || observed_at_unix_ms != expected_observed_at_unix_ms {
            return Err(DatabaseError::FileRelationCandidateNotCurrent);
        }
        Ok(SmartCleanupInboxItem {
            source_kind,
            source_id: relation_id,
            source_observation_id: observation_id,
            scope_id,
            state: SmartCleanupCandidateState::Suggested,
            member_count: 2,
            confidence_basis_points: u16::try_from(confidence_basis_points)
                .map_err(|_| DatabaseError::InvalidStoredValue)?,
            observed_at_unix_ms,
            current_evidence: true,
            verification_required: true,
            review_assistance_only: true,
            cleanup_authorized: false,
        })
    }

    fn file_relation_candidate_summary(
        &self,
        relation_id: i64,
        relation_kind: &str,
        scope_id: i64,
        left_node_id: i64,
        right_node_id: i64,
    ) -> Result<FileRelationCandidateSummary, DatabaseError> {
        let feedback = self
            .connection
            .query_row(
                "SELECT sequence, decision, created_by, created_at_unix_ms \
                 FROM file_relation_feedback_events WHERE relation_id = ?1 \
                 ORDER BY sequence DESC LIMIT 1",
                [relation_id],
                |row| {
                    Ok((
                        row.get::<_, i64>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, String>(2)?,
                        row.get::<_, i64>(3)?,
                    ))
                },
            )
            .optional()?;
        let mut latest_decision = match feedback {
            None => None,
            Some((sequence, kind, creator, decided_at_unix_ms)) => {
                if creator != "user" {
                    return Err(DatabaseError::InvalidStoredValue);
                }
                let _ = u64::try_from(sequence).map_err(|_| DatabaseError::InvalidStoredValue)?;
                Some((
                    file_relation_decision_kind_from_str(&kind)?,
                    decided_at_unix_ms,
                ))
            }
        };
        let (kind, confidence_basis_points, observed_at_unix_ms) = match relation_kind {
            "exact_duplicate" => {
                let evidence = self.connection.query_row(
                    "SELECT source_size_bytes, compared_bytes, confidence_basis_points, \
                            comparison_kind, created_by, provider_id, provider_version, \
                            model_version, observed_at_unix_ms \
                     FROM file_relation_observations WHERE relation_id = ?1 \
                     ORDER BY observed_at_unix_ms DESC, id DESC LIMIT 1",
                    [relation_id],
                    |row| {
                        Ok((
                            row.get::<_, i64>(0)?,
                            row.get::<_, i64>(1)?,
                            row.get::<_, i64>(2)?,
                            row.get::<_, String>(3)?,
                            row.get::<_, String>(4)?,
                            row.get::<_, String>(5)?,
                            row.get::<_, String>(6)?,
                            row.get::<_, Option<String>>(7)?,
                            row.get::<_, i64>(8)?,
                        ))
                    },
                )?;
                if evidence.0 <= 0
                    || evidence.0 != evidence.1
                    || evidence.2 != 10_000
                    || evidence.3 != "byte_for_byte"
                    || evidence.4 != "system_rule"
                    || evidence.5 != FileRelationEvidence::PROVIDER_ID
                    || evidence.6 != FileRelationEvidence::PROVIDER_VERSION
                    || evidence.7.is_some()
                {
                    return Err(DatabaseError::InvalidStoredValue);
                }
                (FileRelationKind::ExactDuplicate, evidence.2, evidence.8)
            }
            "version" => {
                if latest_decision.is_some() {
                    return Err(DatabaseError::InvalidStoredValue);
                }
                let evidence = self.connection.query_row(
                    "SELECT id, base_key, extension_key, older_version, newer_version, \
                            confidence_basis_points, signal_kind, created_by, provider_id, \
                            provider_version, model_version, observed_at_unix_ms \
                     FROM file_version_observations WHERE relation_id = ?1 \
                     ORDER BY observed_at_unix_ms DESC, id DESC LIMIT 1",
                    [relation_id],
                    |row| {
                        Ok((
                            row.get::<_, i64>(0)?,
                            row.get::<_, String>(1)?,
                            row.get::<_, String>(2)?,
                            row.get::<_, i64>(3)?,
                            row.get::<_, i64>(4)?,
                            row.get::<_, i64>(5)?,
                            row.get::<_, String>(6)?,
                            row.get::<_, String>(7)?,
                            row.get::<_, String>(8)?,
                            row.get::<_, String>(9)?,
                            row.get::<_, Option<String>>(10)?,
                            row.get::<_, i64>(11)?,
                        ))
                    },
                )?;
                if evidence.0 <= 0
                    || evidence.1.is_empty()
                    || evidence.1.len() > 1_024
                    || evidence.2.len() > 64
                    || evidence.3 < 1
                    || evidence.3 >= evidence.4
                    || evidence.4 > 999_999
                    || evidence.5 != 9_000
                    || evidence.6 != "explicit_numeric_suffix"
                    || evidence.7 != "system_rule"
                    || evidence.8 != FileVersionEvidence::PROVIDER_ID
                    || evidence.9 != FileVersionEvidence::PROVIDER_VERSION
                    || evidence.10.is_some()
                {
                    return Err(DatabaseError::InvalidStoredValue);
                }
                latest_decision = latest_equivalent_file_version_decision(
                    &self.connection,
                    relation_id,
                    evidence.0,
                )?
                .map(|decision| (decision.kind, decision.decided_at_unix_ms));
                (FileRelationKind::Version, evidence.5, evidence.11)
            }
            _ => return Err(DatabaseError::InvalidStoredValue),
        };
        let state = match latest_decision.map(|decision| decision.0) {
            None => FileRelationCandidateState::Suggested,
            Some(FileRelationDecisionKind::Accepted) => FileRelationCandidateState::Accepted,
            Some(FileRelationDecisionKind::Rejected) => FileRelationCandidateState::Rejected,
        };
        Ok(FileRelationCandidateSummary {
            api_version: FileRelationCandidateSummary::API_VERSION,
            relation_id,
            kind,
            state,
            scope_id,
            left_node_id,
            right_node_id,
            confidence_basis_points: u16::try_from(confidence_basis_points)
                .map_err(|_| DatabaseError::InvalidStoredValue)?,
            last_observed_at_unix_ms: observed_at_unix_ms,
            latest_decision_at_unix_ms: latest_decision.map(|decision| decision.1),
            verification_required: true,
        })
    }

    #[cfg(test)]
    fn screenshot_group_sources(
        &self,
        scope_id: i64,
    ) -> Result<Vec<ScreenshotGroupSourceRecord>, DatabaseError> {
        ensure_scope_queryable(&self.connection, scope_id)?;
        ensure_scope_access_permitted(&self.connection, scope_id)?;
        screenshot_group_sources_from_connection(&self.connection, scope_id)
    }

    pub fn discover_screenshot_group_candidates_with_policy(
        &mut self,
        policy_binding: ScopePolicyBinding,
    ) -> Result<(u32, Vec<ScreenshotGroupCandidate>), DatabaseError> {
        self.discover_screenshot_group_candidates_with_policy_and_hook(policy_binding, || Ok(()))
    }

    fn discover_screenshot_group_candidates_with_policy_and_hook<F>(
        &mut self,
        policy_binding: ScopePolicyBinding,
        after_source_snapshot: F,
    ) -> Result<(u32, Vec<ScreenshotGroupCandidate>), DatabaseError>
    where
        F: FnOnce() -> Result<(), DatabaseError>,
    {
        let scope_id = policy_binding.scope_id;
        if scope_id <= 0 {
            return Err(DatabaseError::ScreenshotGroupCandidateInputInvalid);
        }
        let observed_at = unix_ms()?;
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        assert_scope_policy_binding_in_transaction(&transaction, policy_binding)?;
        ensure_scope_queryable(&transaction, scope_id)?;
        ensure_scope_access_permitted(&transaction, scope_id)?;
        let sources = screenshot_group_sources_from_connection(&transaction, scope_id)?;
        let evaluated_image_count =
            u32::try_from(sources.len()).map_err(|_| DatabaseError::InvalidCount)?;
        let groups = group_screenshot_sources(sources)?;
        after_source_snapshot()?;

        let mut all_nodes = std::collections::BTreeSet::new();
        for group in &groups {
            validate_screenshot_group_input(scope_id, group)?;
            for source in group {
                if !all_nodes.insert(source.node_id) {
                    return Err(DatabaseError::ScreenshotGroupCandidateInputInvalid);
                }
                if !screenshot_group_source_matches(&transaction, source)? {
                    return Err(DatabaseError::ScreenshotGroupCandidateNotCurrent);
                }
            }
        }

        let mut group_ids = Vec::with_capacity(groups.len());
        for group in &groups {
            let membership_key = screenshot_group_membership_key(group)?;
            let evidence_key = screenshot_group_evidence_key(group)?;
            transaction.execute(
                "INSERT OR IGNORE INTO screenshot_group_candidates( \
                    api_version, scope_id, membership_key, created_at_unix_ms \
                 ) VALUES (?1, ?2, ?3, ?4)",
                params![
                    ScreenshotGroupCandidate::API_VERSION,
                    scope_id,
                    membership_key,
                    observed_at
                ],
            )?;
            let group_id: i64 = transaction.query_row(
                "SELECT id FROM screenshot_group_candidates \
                 WHERE scope_id = ?1 AND membership_key = ?2",
                params![scope_id, membership_key],
                |row| row.get(0),
            )?;
            let inserted = transaction.execute(
                "INSERT INTO screenshot_group_observations( \
                    group_id, evidence_key, member_count, confidence_basis_points, rule_kind, created_by, \
                    provider_id, provider_version, model_version, observed_at_unix_ms \
                 ) VALUES (?1, ?2, ?3, 6000, 'same_dimensions_time_window_with_ocr', \
                    'system_rule', 'deskgraph.screenshot-group-rules', '1', NULL, ?4) \
                 ON CONFLICT(group_id, evidence_key) DO NOTHING",
                params![
                    group_id,
                    evidence_key,
                    i64::try_from(group.len()).map_err(|_| DatabaseError::InvalidCount)?,
                    observed_at
                ],
            )?;
            let observation_id = if inserted == 1 {
                let observation_id = transaction.last_insert_rowid();
                for (index, source) in group.iter().enumerate() {
                    transaction.execute(
                        "INSERT INTO screenshot_group_members( \
                            observation_id, ordinal, node_id, location_id, image_metadata_id, \
                            ocr_extraction_job_id, source_size_bytes, source_modified_unix_ns, \
                            format, pixel_width, pixel_height, ocr_chunk_count, ocr_provider_id, \
                            ocr_provider_version \
                         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)",
                        params![
                            observation_id,
                            i64::try_from(index + 1).map_err(|_| DatabaseError::InvalidCount)?,
                            source.node_id,
                            source.location_id,
                            source.image_metadata_id,
                            source.ocr_extraction_job_id,
                            i64::try_from(source.size_bytes)
                                .map_err(|_| DatabaseError::InvalidCount)?,
                            source.modified_unix_ns,
                            source.format.as_str(),
                            i64::from(source.pixel_width),
                            i64::from(source.pixel_height),
                            i64::from(source.ocr_chunk_count),
                            source.ocr_provider_id,
                            source.ocr_provider_version,
                        ],
                    )?;
                }
                observation_id
            } else {
                let observation_id: i64 = transaction.query_row(
                    "SELECT id FROM screenshot_group_observations \
                     WHERE group_id = ?1 AND evidence_key = ?2",
                    params![group_id, evidence_key],
                    |row| row.get(0),
                )?;
                let stored = screenshot_group_observation_sources_from_connection(
                    &transaction,
                    scope_id,
                    observation_id,
                )?;
                if stored.as_slice() != group.as_slice() {
                    return Err(DatabaseError::InvalidStoredValue);
                }
                observation_id
            };
            if observation_id <= 0 {
                return Err(DatabaseError::InvalidStoredValue);
            }
            group_ids.push(group_id);
        }
        transaction.commit()?;
        let candidates = group_ids
            .into_iter()
            .map(|group_id| self.screenshot_group_candidate(group_id))
            .collect::<Result<Vec<_>, _>>()?;
        Ok((evaluated_image_count, candidates))
    }

    #[cfg(test)]
    pub fn discover_screenshot_group_candidates(
        &mut self,
        scope_id: i64,
    ) -> Result<(u32, Vec<ScreenshotGroupCandidate>), DatabaseError> {
        let binding = self.bind_scope_policy_revision(scope_id)?;
        self.discover_screenshot_group_candidates_with_policy(binding)
    }

    #[cfg(test)]
    fn discover_screenshot_group_candidates_with_hook<F>(
        &mut self,
        scope_id: i64,
        hook: F,
    ) -> Result<(u32, Vec<ScreenshotGroupCandidate>), DatabaseError>
    where
        F: FnOnce() -> Result<(), DatabaseError>,
    {
        let binding = self.bind_scope_policy_revision(scope_id)?;
        self.discover_screenshot_group_candidates_with_policy_and_hook(binding, hook)
    }

    pub fn screenshot_group_candidate(
        &self,
        group_id: i64,
    ) -> Result<ScreenshotGroupCandidate, DatabaseError> {
        let transaction = self.connection.unchecked_transaction()?;
        let (scope_id, membership_key) =
            screenshot_group_identity_from_connection(&transaction, group_id)?;
        ensure_scope_queryable(&transaction, scope_id)?;
        ensure_scope_access_permitted(&transaction, scope_id)?;
        let sources =
            current_screenshot_group_for_membership(&transaction, scope_id, &membership_key)?
                .ok_or(DatabaseError::ScreenshotGroupCandidateNotCurrent)?;
        let evidence_key = screenshot_group_evidence_key(&sources)?;
        let observation =
            screenshot_group_observation_for_evidence(&transaction, group_id, &evidence_key)?
                .ok_or(DatabaseError::ScreenshotGroupCandidateNotCurrent)?;
        validate_screenshot_group_observation(
            &transaction,
            scope_id,
            &membership_key,
            &observation,
        )?;
        let candidate = screenshot_group_candidate_from_sources(
            &transaction,
            group_id,
            scope_id,
            observation.id,
            observation.confidence_basis_points,
            observation.observed_at_unix_ms,
            sources,
        )?;
        transaction.commit()?;
        Ok(candidate)
    }

    pub fn recent_screenshot_group_candidates(
        &self,
    ) -> Result<Vec<ScreenshotGroupCandidateSummary>, DatabaseError> {
        let group_ids = {
            let mut statement = self
                .connection
                .prepare("SELECT id FROM screenshot_group_candidates ORDER BY id DESC LIMIT 20")?;
            let rows = statement.query_map([], |row| row.get::<_, i64>(0))?;
            rows.collect::<Result<Vec<_>, _>>()?
        };
        group_ids
            .into_iter()
            .map(|group_id| self.screenshot_group_candidate_summary(group_id))
            .collect()
    }

    fn screenshot_group_candidate_summary(
        &self,
        group_id: i64,
    ) -> Result<ScreenshotGroupCandidateSummary, DatabaseError> {
        let transaction = self.connection.unchecked_transaction()?;
        let (scope_id, membership_key) =
            screenshot_group_identity_from_connection(&transaction, group_id)?;
        let observation =
            latest_screenshot_group_observation_from_connection(&transaction, group_id)?;
        let historical_sources = validate_screenshot_group_observation(
            &transaction,
            scope_id,
            &membership_key,
            &observation,
        )?;
        let mut total_size_bytes = 0_u64;
        for source in &historical_sources {
            total_size_bytes = total_size_bytes
                .checked_add(source.size_bytes)
                .ok_or(DatabaseError::InvalidCount)?;
        }

        let current_evidence = match ensure_scope_access_permitted(&transaction, scope_id) {
            Ok(()) => {
                current_screenshot_group_for_membership(&transaction, scope_id, &membership_key)?
                    .map(|sources| {
                        let evidence_key = screenshot_group_evidence_key(&sources)?;
                        Ok::<_, DatabaseError>(
                            screenshot_group_observation_for_evidence(
                                &transaction,
                                group_id,
                                &evidence_key,
                            )?
                            .is_some(),
                        )
                    })
                    .transpose()?
                    .unwrap_or(false)
            }
            Err(DatabaseError::ScopeAccessGrantNotActive) => false,
            Err(error) => return Err(error),
        };
        let summary = ScreenshotGroupCandidateSummary {
            api_version: ScreenshotGroupCandidateSummary::API_VERSION,
            group_id,
            scope_id,
            state: ScreenshotGroupCandidateState::Suggested,
            current_evidence,
            member_count: u32::try_from(observation.member_count)
                .map_err(|_| DatabaseError::InvalidStoredValue)?,
            total_size_bytes,
            confidence_basis_points: u16::try_from(observation.confidence_basis_points)
                .map_err(|_| DatabaseError::InvalidStoredValue)?,
            last_observed_at_unix_ms: observation.observed_at_unix_ms,
            verification_required: true,
            cleanup_authorized: false,
        };
        transaction.commit()?;
        Ok(summary)
    }

    /// Resolves the current screenshot membership directly to its immutable
    /// evidence observation without materializing member paths or OCR
    /// provenance in the response.
    pub fn smart_cleanup_screenshot_item(
        &self,
        group_id: i64,
    ) -> Result<SmartCleanupInboxItem, DatabaseError> {
        let transaction = self.connection.unchecked_transaction()?;
        let (scope_id, membership_key) =
            screenshot_group_identity_from_connection(&transaction, group_id)?;
        ensure_scope_queryable(&transaction, scope_id)?;
        ensure_scope_access_permitted(&transaction, scope_id)?;
        let sources =
            current_screenshot_group_for_membership(&transaction, scope_id, &membership_key)?
                .ok_or(DatabaseError::ScreenshotGroupCandidateNotCurrent)?;
        let evidence_key = screenshot_group_evidence_key(&sources)?;
        let observation =
            screenshot_group_observation_for_evidence(&transaction, group_id, &evidence_key)?
                .ok_or(DatabaseError::ScreenshotGroupCandidateNotCurrent)?;
        validate_screenshot_group_observation(
            &transaction,
            scope_id,
            &membership_key,
            &observation,
        )?;
        let item = SmartCleanupInboxItem {
            source_kind: SmartCleanupSourceKind::ScreenshotReviewGroup,
            source_id: group_id,
            source_observation_id: observation.id,
            scope_id,
            state: SmartCleanupCandidateState::Suggested,
            member_count: u32::try_from(observation.member_count)
                .map_err(|_| DatabaseError::InvalidStoredValue)?,
            confidence_basis_points: u16::try_from(observation.confidence_basis_points)
                .map_err(|_| DatabaseError::InvalidStoredValue)?,
            observed_at_unix_ms: observation.observed_at_unix_ms,
            current_evidence: true,
            verification_required: true,
            review_assistance_only: true,
            cleanup_authorized: false,
        };
        transaction.commit()?;
        Ok(item)
    }

    /// Validates an explicit path-free Inbox reference before a caller asks
    /// the projects layer to reveal transient local member paths. This method
    /// does not return or persist paths and cannot authorize an action.
    pub fn validate_cleanup_source_observation(
        &self,
        scope_id: i64,
        source_kind: SmartCleanupSourceKind,
        source_id: i64,
        source_observation_id: i64,
    ) -> Result<SmartCleanupInboxItem, DatabaseError> {
        if scope_id <= 0 || source_id <= 0 || source_observation_id <= 0 {
            return Err(DatabaseError::SmartCleanupSourceInputInvalid);
        }
        ensure_scope_access_permitted(&self.connection, scope_id)?;
        ensure_scope_queryable(&self.connection, scope_id)?;
        let item = match source_kind {
            SmartCleanupSourceKind::ExactDuplicate | SmartCleanupSourceKind::Version => {
                let query = match source_kind {
                    SmartCleanupSourceKind::ExactDuplicate => {
                        "SELECT observed_at_unix_ms FROM file_relation_observations \
                         WHERE id = ?1 AND relation_id = ?2"
                    }
                    SmartCleanupSourceKind::Version => {
                        "SELECT observed_at_unix_ms FROM file_version_observations \
                         WHERE id = ?1 AND relation_id = ?2"
                    }
                    SmartCleanupSourceKind::ScreenshotReviewGroup => unreachable!(),
                };
                let observed_at_unix_ms = self
                    .connection
                    .query_row(query, params![source_observation_id, source_id], |row| {
                        row.get::<_, i64>(0)
                    })
                    .optional()?
                    .ok_or(DatabaseError::CleanupActionSourceNotCurrent)?;
                self.smart_cleanup_relation_item(source_id, observed_at_unix_ms)
                    .map_err(normalize_cleanup_source_validation_error)?
            }
            SmartCleanupSourceKind::ScreenshotReviewGroup => self
                .smart_cleanup_screenshot_item(source_id)
                .map_err(normalize_cleanup_source_validation_error)?,
        };
        if item.scope_id != scope_id
            || item.source_kind != source_kind
            || item.source_id != source_id
            || item.source_observation_id != source_observation_id
            || item.state != SmartCleanupCandidateState::Suggested
            || !item.current_evidence
            || item.cleanup_authorized
        {
            return Err(DatabaseError::CleanupActionSourceNotCurrent);
        }
        Ok(item)
    }

    /// Resolves one explicit, already-refreshed cleanup selection to the same
    /// strong root/parent/file snapshot used by the transaction core. The
    /// returned path-bearing record is internal only; cleanup preview payloads
    /// and history remain path-free.
    pub fn cleanup_action_source(
        &self,
        selection: CleanupActionSelection,
    ) -> Result<ActionExecutionSourceRecord, DatabaseError> {
        self.cleanup_action_sources(selection)
            .map(|sources| sources.0)
    }

    /// Returns the selected target and, when present, its explicitly retained
    /// keeper from the same exact immutable observation.
    pub fn cleanup_action_sources(
        &self,
        selection: CleanupActionSelection,
    ) -> Result<
        (
            ActionExecutionSourceRecord,
            Option<ActionExecutionSourceRecord>,
        ),
        DatabaseError,
    > {
        validate_cleanup_selection_input(&selection)?;
        let transaction = self.connection.unchecked_transaction()?;
        let expected = cleanup_selection_snapshot(&transaction, &selection)?;
        let target = cleanup_execution_source_from_connection(
            &transaction,
            selection.scope_id,
            selection.target_node_id,
            expected.location_id,
            expected.size_bytes,
            expected.modified_unix_ns,
        )?;
        let keeper = match selection.keeper_node_id {
            None => None,
            Some(keeper_node_id) => {
                let keeper_expected =
                    cleanup_keeper_snapshot(&transaction, &selection, keeper_node_id)?;
                Some(cleanup_execution_source_from_connection(
                    &transaction,
                    selection.scope_id,
                    keeper_node_id,
                    keeper_expected.location_id,
                    keeper_expected.size_bytes,
                    keeper_expected.modified_unix_ns,
                )?)
            }
        };
        transaction.commit()?;
        Ok((target, keeper))
    }

    pub fn exact_duplicate_sources(
        &self,
        relation_id: i64,
    ) -> Result<(ActionSourceRecord, ActionSourceRecord), DatabaseError> {
        if relation_id <= 0 {
            return Err(DatabaseError::FileRelationCandidateInputInvalid);
        }
        let relation = self
            .connection
            .query_row(
                "SELECT scope_id, left_node_id, right_node_id \
                 FROM file_relation_candidates \
                 WHERE id = ?1 AND relation_kind = 'exact_duplicate'",
                [relation_id],
                |row| {
                    Ok((
                        row.get::<_, i64>(0)?,
                        row.get::<_, i64>(1)?,
                        row.get::<_, i64>(2)?,
                    ))
                },
            )
            .optional()?
            .ok_or(DatabaseError::FileRelationCandidateNotFound)?;
        let left = self.current_file_source(relation.0, relation.1)?;
        let right = self.current_file_source(relation.0, relation.2)?;
        Ok((left, right))
    }

    pub fn file_version_sources(
        &self,
        relation_id: i64,
    ) -> Result<(ActionSourceRecord, ActionSourceRecord), DatabaseError> {
        if relation_id <= 0 {
            return Err(DatabaseError::FileRelationCandidateInputInvalid);
        }
        let relation = self
            .connection
            .query_row(
                "SELECT scope_id, left_node_id, right_node_id \
                 FROM file_relation_candidates \
                 WHERE id = ?1 AND relation_kind = 'version'",
                [relation_id],
                |row| {
                    Ok((
                        row.get::<_, i64>(0)?,
                        row.get::<_, i64>(1)?,
                        row.get::<_, i64>(2)?,
                    ))
                },
            )
            .optional()?
            .ok_or(DatabaseError::FileRelationCandidateNotFound)?;
        let left = self.current_file_source(relation.0, relation.1)?;
        let right = self.current_file_source(relation.0, relation.2)?;
        Ok((left, right))
    }

    fn current_file_source(
        &self,
        scope_id: i64,
        node_id: i64,
    ) -> Result<ActionSourceRecord, DatabaseError> {
        self.connection
            .query_row(
                "SELECT l.scope_id, l.node_id, l.id, l.path_raw, l.path_key, l.display_path, \
                        n.identity_kind, n.identity_key, f.size_bytes, f.modified_unix_ns \
                 FROM locations l \
                 JOIN nodes n ON n.id = l.node_id AND n.kind = 'file' \
                 JOIN files f ON f.node_id = l.node_id \
                 WHERE l.scope_id = ?1 AND l.node_id = ?2 AND l.present = 1 \
                 ORDER BY l.id LIMIT 1",
                params![scope_id, node_id],
                |row| {
                    Ok(ActionSourceRecord {
                        scope_id: row.get(0)?,
                        node_id: row.get(1)?,
                        location_id: row.get(2)?,
                        path_raw: row.get(3)?,
                        path_key: row.get(4)?,
                        display_path: row.get(5)?,
                        identity_kind: row.get(6)?,
                        identity_key: row.get(7)?,
                        size_bytes: row_u64(row, 8)?,
                        modified_unix_ns: row.get(9)?,
                    })
                },
            )
            .optional()?
            .ok_or(DatabaseError::FileRelationCandidateNotCurrent)
    }

    pub fn action_source_for_path_key(
        &self,
        scope_id: i64,
        path_key: &str,
    ) -> Result<ActionSourceRecord, DatabaseError> {
        self.connection
            .query_row(
                "SELECT l.scope_id, l.node_id, l.id, l.path_raw, l.path_key, l.display_path, \
                        n.identity_kind, n.identity_key, f.size_bytes, f.modified_unix_ns \
                 FROM locations l \
                 JOIN nodes n ON n.id = l.node_id AND n.kind = 'file' \
                 JOIN files f ON f.node_id = l.node_id \
                 WHERE l.scope_id = ?1 AND l.path_key = ?2 AND l.present = 1",
                params![scope_id, path_key],
                |row| {
                    Ok(ActionSourceRecord {
                        scope_id: row.get(0)?,
                        node_id: row.get(1)?,
                        location_id: row.get(2)?,
                        path_raw: row.get(3)?,
                        path_key: row.get(4)?,
                        display_path: row.get(5)?,
                        identity_kind: row.get(6)?,
                        identity_key: row.get(7)?,
                        size_bytes: row_u64(row, 8)?,
                        modified_unix_ns: row.get(9)?,
                    })
                },
            )
            .optional()?
            .ok_or(DatabaseError::ActionSourceNotFound)
    }

    /// Reads the source and the exact root/parent topology from one completed
    /// manifest snapshot. Callers must use these returned identities when
    /// creating an executable preview; deriving them from filesystem paths is
    /// intentionally unsupported.
    pub fn action_execution_source_for_path_key(
        &self,
        scope_id: i64,
        path_key: &str,
    ) -> Result<ActionExecutionSourceRecord, DatabaseError> {
        if scope_id <= 0 || path_key.is_empty() || path_key.len() > MAX_ACTION_PATH_BYTES {
            return Err(DatabaseError::ActionSourceNotFound);
        }
        self.connection
            .query_row(
                "SELECT source.scope_id, source.node_id, source.id, source.path_raw, \
                        source.path_key, source.display_path, source_node.identity_kind, \
                        source_node.identity_key, source_file.size_bytes, source_file.modified_unix_ns, \
                        root_node.id, root_node.identity_kind, root_node.identity_key, \
                        parent_node.id, parent_node.identity_kind, parent_node.identity_key \
                 FROM authorized_scopes scope \
                 JOIN locations source ON source.scope_id = scope.id AND source.path_key = ?2 \
                    AND source.present = 1 \
                 JOIN nodes source_node ON source_node.id = source.node_id AND source_node.kind = 'file' \
                 JOIN files source_file ON source_file.node_id = source.node_id \
                 JOIN scan_jobs source_scan ON source_scan.id = source.last_seen_scan_id \
                    AND source_scan.scope_id = scope.id AND source_scan.status = 'completed' \
                 JOIN locations root ON root.scope_id = scope.id AND root.path_key = scope.path_key \
                    AND root.present = 1 \
                 JOIN nodes root_node ON root_node.id = root.node_id AND root_node.kind = 'folder' \
                 JOIN edges parent_edge ON parent_edge.scope_id = scope.id \
                    AND parent_edge.source_node_id = source.node_id \
                    AND parent_edge.kind = 'located_in' AND parent_edge.active = 1 \
                 JOIN locations parent ON parent.scope_id = scope.id \
                    AND parent.node_id = parent_edge.target_node_id AND parent.present = 1 \
                 JOIN nodes parent_node ON parent_node.id = parent.node_id AND parent_node.kind = 'folder' \
                 WHERE scope.id = ?1 \
                   AND root_node.identity_kind <> 'path_fallback' \
                   AND parent_node.identity_kind <> 'path_fallback' \
                   AND (SELECT COUNT(*) FROM locations only_source \
                        WHERE only_source.scope_id = scope.id \
                          AND only_source.node_id = source.node_id AND only_source.present = 1) = 1 \
                   AND (SELECT COUNT(*) FROM locations only_root \
                        WHERE only_root.scope_id = scope.id AND only_root.path_key = scope.path_key \
                          AND only_root.present = 1) = 1 \
                   AND (SELECT COUNT(*) FROM edges only_parent \
                        WHERE only_parent.scope_id = scope.id \
                          AND only_parent.source_node_id = source.node_id \
                          AND only_parent.kind = 'located_in' AND only_parent.active = 1) = 1 \
                   AND (SELECT COUNT(*) FROM locations only_parent_location \
                        WHERE only_parent_location.scope_id = scope.id \
                          AND only_parent_location.node_id = parent_node.id \
                          AND only_parent_location.present = 1) = 1",
                params![scope_id, path_key],
                |row| {
                    Ok(ActionExecutionSourceRecord {
                        source: ActionSourceRecord {
                            scope_id: row.get(0)?,
                            node_id: row.get(1)?,
                            location_id: row.get(2)?,
                            path_raw: row.get(3)?,
                            path_key: row.get(4)?,
                            display_path: row.get(5)?,
                            identity_kind: row.get(6)?,
                            identity_key: row.get(7)?,
                            size_bytes: row_u64(row, 8)?,
                            modified_unix_ns: row.get(9)?,
                        },
                        scope_root_node_id: row.get(10)?,
                        scope_root_identity_kind: row.get(11)?,
                        scope_root_identity_key: row.get(12)?,
                        parent_node_id: row.get(13)?,
                        parent_identity_kind: row.get(14)?,
                        parent_identity_key: row.get(15)?,
                    })
                },
            )
            .optional()?
            .ok_or(DatabaseError::ActionExecutionBindingUnavailable)
    }

    fn action_plan_base(&self, plan_id: i64) -> Result<StoredActionPlan, DatabaseError> {
        action_plan_base_from_connection(&self.connection, plan_id)
    }

    pub fn create_rename_action_plan_with_policy(
        &mut self,
        policy_binding: ScopePolicyBinding,
        plan: ActionPlanWrite<'_>,
    ) -> Result<ActionPlanPreview, DatabaseError> {
        validate_action_plan_write(&plan)?;
        if policy_binding.scope_id != plan.scope_id {
            return Err(DatabaseError::ScopePolicyRevisionStale);
        }
        let source_size = to_i64(plan.source_size_bytes)?;
        let created_at = unix_ms()?;
        let execution_strategy = action_execution_strategy_str(plan.execution_strategy);
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        assert_scope_policy_binding_in_transaction(&transaction, policy_binding)?;
        assert_scope_path_key_allowed(&transaction, plan.scope_id, plan.source_path_key)?;
        assert_scope_path_key_allowed(&transaction, plan.scope_id, plan.destination_path_key)?;
        let snapshot_matches: i64 = transaction.query_row(
            "SELECT COUNT(*) \
             FROM locations l \
             JOIN nodes n ON n.id = l.node_id AND n.kind = 'file' \
             JOIN files f ON f.node_id = l.node_id \
             WHERE l.id = ?1 AND l.scope_id = ?2 AND l.node_id = ?3 AND l.present = 1 \
               AND l.path_raw = ?4 AND l.path_key = ?5 AND l.display_path = ?6 \
               AND n.identity_kind = ?7 AND n.identity_key = ?8 \
               AND f.size_bytes = ?9 AND f.modified_unix_ns IS ?10",
            params![
                plan.source_location_id,
                plan.scope_id,
                plan.node_id,
                plan.source_path_raw,
                plan.source_path_key,
                plan.source_display_path,
                plan.source_identity_kind,
                plan.source_identity_key,
                source_size,
                plan.source_modified_unix_ns,
            ],
            |row| row.get(0),
        )?;
        if snapshot_matches != 1 {
            return Err(DatabaseError::ActionSourceSnapshotChanged);
        }
        let execution_binding = preview_execution_binding(&transaction, &plan)?;
        transaction.execute(
            "INSERT INTO action_plans( \
                api_version, policy_version, operation, execution_strategy, scope_id, node_id, \
                source_location_id, source_path_raw, source_path_key, source_display_path, \
                destination_path_raw, destination_path_key, destination_display_path, \
                source_identity_kind, source_identity_key, source_size_bytes, \
                source_modified_unix_ns, created_at_unix_ms, policy_revision \
             ) VALUES ( \
                'deskgraph.action-plan.v1', 'deskgraph.action-policy.v1', 'rename', ?1, ?2, ?3, \
                ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16 \
             )",
            params![
                execution_strategy,
                plan.scope_id,
                plan.node_id,
                plan.source_location_id,
                plan.source_path_raw,
                plan.source_path_key,
                plan.source_display_path,
                plan.destination_path_raw,
                plan.destination_path_key,
                plan.destination_display_path,
                plan.source_identity_kind,
                plan.source_identity_key,
                source_size,
                plan.source_modified_unix_ns,
                created_at,
                policy_binding.revision,
            ],
        )?;
        let plan_id = transaction.last_insert_rowid();
        transaction.execute(
            "INSERT INTO action_journal_events( \
                 api_version, plan_id, sequence, event_kind, command_request_id, created_at_unix_ms \
             ) VALUES ('deskgraph.action-journal.v1', ?1, 1, 'preview_created', NULL, ?2)",
            params![plan_id, created_at],
        )?;
        transaction.execute(
            "INSERT INTO action_execution_bindings( \
                 plan_id, api_version, source_hash_bytes, source_sha256, scope_root_node_id, \
                 scope_root_identity_kind, scope_root_identity_key, parent_node_id, \
                 parent_identity_kind, parent_identity_key, created_at_unix_ms \
             ) VALUES ( \
                 ?1, 'deskgraph.action-execution-binding.v1', ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10 \
             )",
            params![
                plan_id,
                to_i64(plan.source_hash_bytes)?,
                plan.source_sha256,
                execution_binding.scope_root_node_id,
                execution_binding.scope_root_identity_kind,
                execution_binding.scope_root_identity_key,
                execution_binding.parent_node_id,
                execution_binding.parent_identity_kind,
                execution_binding.parent_identity_key,
                created_at,
            ],
        )?;
        transaction.commit()?;
        self.action_plan(plan_id)
    }

    #[cfg(test)]
    pub fn create_rename_action_plan(
        &mut self,
        plan: ActionPlanWrite<'_>,
    ) -> Result<ActionPlanPreview, DatabaseError> {
        let binding = test_active_binding(self, plan.scope_id)?;
        self.create_rename_action_plan_with_policy(binding, plan)
    }

    /// Persists one immutable, path-free System Trash preview. This method has
    /// no confirmation, command, execute, recovery, Trash, or Undo companion.
    pub fn create_cleanup_action_plan_with_policy(
        &mut self,
        policy_binding: ScopePolicyBinding,
        plan: CleanupActionPlanWrite<'_>,
    ) -> Result<CleanupActionPlanPreview, DatabaseError> {
        validate_cleanup_action_plan_write(&plan)?;
        if policy_binding.scope_id != plan.selection.scope_id {
            return Err(DatabaseError::ScopePolicyRevisionStale);
        }
        let target_size = to_i64(plan.target_size_bytes)?;
        let keeper_size = plan
            .keeper
            .map(|keeper| to_i64(keeper.size_bytes))
            .transpose()?;
        let keeper_hash_bytes = plan
            .keeper
            .map(|keeper| to_i64(keeper.hash_bytes))
            .transpose()?;
        let created_at = unix_ms()?;
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        assert_scope_policy_binding_in_transaction(&transaction, policy_binding)?;
        let expected = cleanup_selection_snapshot(&transaction, &plan.selection)?;
        if expected.location_id != plan.target_location_id
            || expected.size_bytes != plan.target_size_bytes
            || expected.modified_unix_ns != plan.target_modified_unix_ns
        {
            return Err(DatabaseError::CleanupActionSourceNotCurrent);
        }
        let current = cleanup_execution_source_from_connection(
            &transaction,
            plan.selection.scope_id,
            plan.selection.target_node_id,
            plan.target_location_id,
            plan.target_size_bytes,
            plan.target_modified_unix_ns,
        )?;
        assert_scope_path_key_allowed(
            &transaction,
            plan.selection.scope_id,
            &current.source.path_key,
        )?;
        if current.source.identity_kind != plan.target_identity_kind
            || current.source.identity_key != plan.target_identity_key
            || current.scope_root_node_id != plan.scope_root_node_id
            || current.scope_root_identity_kind != plan.scope_root_identity_kind
            || current.scope_root_identity_key != plan.scope_root_identity_key
            || current.parent_node_id != plan.parent_node_id
            || current.parent_identity_kind != plan.parent_identity_kind
            || current.parent_identity_key != plan.parent_identity_key
        {
            return Err(DatabaseError::CleanupActionSourceNotCurrent);
        }
        match (plan.selection.keeper_node_id, plan.keeper) {
            (None, None) => {}
            (Some(keeper_node_id), Some(keeper)) => {
                let expected_keeper =
                    cleanup_keeper_snapshot(&transaction, &plan.selection, keeper_node_id)?;
                if expected_keeper.location_id != keeper.location_id
                    || expected_keeper.size_bytes != keeper.size_bytes
                    || expected_keeper.modified_unix_ns != keeper.modified_unix_ns
                {
                    return Err(DatabaseError::CleanupActionSourceNotCurrent);
                }
                let current_keeper = cleanup_execution_source_from_connection(
                    &transaction,
                    plan.selection.scope_id,
                    keeper_node_id,
                    keeper.location_id,
                    keeper.size_bytes,
                    keeper.modified_unix_ns,
                )?;
                assert_scope_path_key_allowed(
                    &transaction,
                    plan.selection.scope_id,
                    &current_keeper.source.path_key,
                )?;
                if current_keeper.source.identity_kind != keeper.identity_kind
                    || current_keeper.source.identity_key != keeper.identity_key
                    || current_keeper.scope_root_node_id != keeper.scope_root_node_id
                    || current_keeper.scope_root_identity_kind != keeper.scope_root_identity_kind
                    || current_keeper.scope_root_identity_key != keeper.scope_root_identity_key
                    || current_keeper.parent_node_id != keeper.parent_node_id
                    || current_keeper.parent_identity_kind != keeper.parent_identity_kind
                    || current_keeper.parent_identity_key != keeper.parent_identity_key
                {
                    return Err(DatabaseError::CleanupActionSourceNotCurrent);
                }
            }
            _ => return Err(DatabaseError::CleanupActionPlanInputInvalid),
        }
        transaction.execute(
            "INSERT INTO cleanup_action_plans( \
                 api_version, policy_version, operation, state, scope_id, source_kind, \
                 source_id, source_observation_id, keeper_node_id, keeper_location_id, \
                 keeper_identity_kind, keeper_identity_key, keeper_size_bytes, \
                 keeper_modified_unix_ns, keeper_sha256, keeper_hash_bytes, \
                 keeper_scope_root_node_id, keeper_scope_root_identity_kind, \
                 keeper_scope_root_identity_key, keeper_parent_node_id, \
                 keeper_parent_identity_kind, keeper_parent_identity_key, target_node_id, \
                 target_location_id, target_identity_kind, target_identity_key, \
                 target_size_bytes, target_modified_unix_ns, target_sha256, target_hash_bytes, \
                 scope_root_node_id, scope_root_identity_kind, scope_root_identity_key, \
                 parent_node_id, parent_identity_kind, parent_identity_key, \
                 confirmation_required, action_authorized, execution_available, created_at_unix_ms, \
                 policy_revision \
             ) VALUES ( \
                 'deskgraph.cleanup-action-plan.v1', 'deskgraph.cleanup-action-policy.v1', \
                 'system_trash_preview', 'previewed', ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, \
                 ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20, ?21, ?22, \
                 ?23, ?24, ?25, ?26, ?27, ?28, ?29, ?30, ?31, ?32, 1, 0, 0, ?33, ?34 \
             )",
            params![
                plan.selection.scope_id,
                smart_cleanup_source_kind_str(plan.selection.source_kind),
                plan.selection.source_id,
                plan.selection.source_observation_id,
                plan.selection.keeper_node_id,
                plan.keeper.map(|keeper| keeper.location_id),
                plan.keeper.map(|keeper| keeper.identity_kind),
                plan.keeper.map(|keeper| keeper.identity_key),
                keeper_size,
                plan.keeper.and_then(|keeper| keeper.modified_unix_ns),
                plan.keeper.map(|keeper| keeper.sha256),
                keeper_hash_bytes,
                plan.keeper.map(|keeper| keeper.scope_root_node_id),
                plan.keeper.map(|keeper| keeper.scope_root_identity_kind),
                plan.keeper.map(|keeper| keeper.scope_root_identity_key),
                plan.keeper.map(|keeper| keeper.parent_node_id),
                plan.keeper.map(|keeper| keeper.parent_identity_kind),
                plan.keeper.map(|keeper| keeper.parent_identity_key),
                plan.selection.target_node_id,
                plan.target_location_id,
                plan.target_identity_kind,
                plan.target_identity_key,
                target_size,
                plan.target_modified_unix_ns,
                plan.target_sha256,
                to_i64(plan.target_hash_bytes)?,
                plan.scope_root_node_id,
                plan.scope_root_identity_kind,
                plan.scope_root_identity_key,
                plan.parent_node_id,
                plan.parent_identity_kind,
                plan.parent_identity_key,
                created_at,
                policy_binding.revision,
            ],
        )?;
        let plan_id = transaction.last_insert_rowid();
        transaction.execute(
            "INSERT INTO cleanup_action_journal_events( \
                 api_version, plan_id, sequence, event_kind, created_at_unix_ms \
             ) VALUES ('deskgraph.cleanup-action-journal.v1', ?1, 1, 'preview_created', ?2)",
            params![plan_id, created_at],
        )?;
        transaction.commit()?;
        self.cleanup_action_plan(plan_id)
    }

    #[cfg(test)]
    pub fn create_cleanup_action_plan(
        &mut self,
        plan: CleanupActionPlanWrite<'_>,
    ) -> Result<CleanupActionPlanPreview, DatabaseError> {
        let binding = test_active_binding(self, plan.selection.scope_id)?;
        self.create_cleanup_action_plan_with_policy(binding, plan)
    }

    pub fn cleanup_action_plan(
        &self,
        plan_id: i64,
    ) -> Result<CleanupActionPlanPreview, DatabaseError> {
        if plan_id <= 0 {
            return Err(DatabaseError::CleanupActionPlanNotFound);
        }
        self.connection
            .query_row(
                "SELECT scope_id, source_kind, source_id, source_observation_id, \
                        keeper_node_id, target_node_id, target_size_bytes, created_at_unix_ms, \
                        keeper_sha256 IS NOT NULL, \
                        (SELECT COUNT(*) FROM cleanup_action_journal_events event \
                         WHERE event.plan_id = cleanup_action_plans.id \
                           AND event.sequence = 1 AND event.event_kind = 'preview_created') \
                 FROM cleanup_action_plans WHERE id = ?1 \
                   AND api_version = 'deskgraph.cleanup-action-plan.v1' \
                   AND policy_version = 'deskgraph.cleanup-action-policy.v1' \
                   AND operation = 'system_trash_preview' AND state = 'previewed' \
                   AND confirmation_required = 1 AND action_authorized = 0 \
                   AND execution_available = 0",
                [plan_id],
                |row| {
                    let event_count = row.get::<_, i64>(9)?;
                    if event_count != 1 {
                        return Err(rusqlite::Error::InvalidQuery);
                    }
                    Ok(CleanupActionPlanPreview {
                        api_version: CleanupActionPlanPreview::API_VERSION,
                        plan_id,
                        operation: CleanupActionOperation::SystemTrashPreview,
                        state: CleanupActionPlanState::Previewed,
                        scope_id: row.get(0)?,
                        source_kind: smart_cleanup_source_kind_from_str(&row.get::<_, String>(1)?)
                            .map_err(|_| rusqlite::Error::InvalidQuery)?,
                        source_id: row.get(2)?,
                        source_observation_id: row.get(3)?,
                        keeper_node_id: row.get(4)?,
                        target_node_id: row.get(5)?,
                        expected_bytes: row_u64(row, 6)?,
                        keeper_hash_bound: row.get(8)?,
                        policy: CleanupActionPolicyReport::preview_only(),
                        journal_sequence: 1,
                        created_at_unix_ms: row.get(7)?,
                    })
                },
            )
            .optional()?
            .ok_or(DatabaseError::CleanupActionPlanNotFound)
    }

    pub fn action_plan(&self, plan_id: i64) -> Result<ActionPlanPreview, DatabaseError> {
        let plan = self.action_plan_base(plan_id)?;
        let events = action_journal_events(&self.connection, plan_id)?;
        let state = reduce_journal_or_stored_value(&events)?;
        Ok(ActionPlanPreview {
            api_version: ActionPlanPreview::API_VERSION,
            plan_id: plan.id,
            operation: plan.operation,
            state,
            scope_id: plan.scope_id,
            node_id: plan.node_id,
            source_path: plan.source_display_path,
            destination_path: plan.destination_display_path,
            execution_strategy: plan.execution_strategy,
            policy: ActionPolicyReport::rename_allowed(),
            journal_sequence: last_journal_sequence(&events)?,
            created_at_unix_ms: plan.created_at_unix_ms,
        })
    }

    pub fn recent_action_plans(&self) -> Result<Vec<ActionPlanSummary>, DatabaseError> {
        let ids = {
            let mut statement = self.connection.prepare(
                "SELECT p.id FROM action_plans p \
                 JOIN action_journal_events e ON e.plan_id = p.id \
                 GROUP BY p.id ORDER BY p.id DESC LIMIT 20",
            )?;
            statement
                .query_map([], |row| row.get::<_, i64>(0))?
                .collect::<Result<Vec<_>, _>>()?
        };
        ids.into_iter()
            .map(|plan_id| {
                let plan = self.action_plan_base(plan_id)?;
                let events = action_journal_events(&self.connection, plan_id)?;
                Ok(ActionPlanSummary {
                    api_version: ActionPlanSummary::API_VERSION,
                    plan_id: plan.id,
                    operation: plan.operation,
                    state: reduce_journal_or_stored_value(&events)?,
                    scope_id: plan.scope_id,
                    node_id: plan.node_id,
                    execution_strategy: plan.execution_strategy,
                    journal_sequence: last_journal_sequence(&events)?,
                    created_at_unix_ms: plan.created_at_unix_ms,
                })
            })
            .collect()
    }

    /// Starts an explicit user command. The request row and first command
    /// event are committed together under an SQLite writer lock, making a
    /// duplicate click/restart idempotent without granting a second intent.
    pub fn start_action_command(
        &mut self,
        command: ActionCommandWrite<'_>,
    ) -> Result<ActionCommandStart, DatabaseError> {
        validate_action_command_write(&command)?;
        let now = unix_ms()?;
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let existing = transaction
            .query_row(
                "SELECT id, command_kind, requested_sequence FROM action_command_requests \
                 WHERE plan_id = ?1 AND request_id = ?2",
                params![command.plan_id, command.request_id],
                |row| {
                    Ok((
                        row.get::<_, i64>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, i64>(2)?,
                    ))
                },
            )
            .optional()?;
        if let Some((command_request_id, stored_kind, requested_sequence)) = existing {
            let stored_kind = action_command_kind_from_str(&stored_kind)?;
            if stored_kind != command.kind {
                return Err(DatabaseError::ActionJournalIdempotencyConflict);
            }
            let events = action_journal_events(&transaction, command.plan_id)?;
            let (state, sequence) = action_command_outcome_from_journal(
                &events,
                command_request_id,
                stored_kind,
                u64::try_from(requested_sequence).map_err(|_| DatabaseError::InvalidStoredValue)?,
            )?;
            transaction.commit()?;
            return Ok(ActionCommandStart {
                api_version: ActionCommandStart::API_VERSION,
                command_request_id,
                plan_id: command.plan_id,
                kind: command.kind,
                state,
                journal_sequence: sequence,
                idempotent: true,
            });
        }
        let record = action_execution_record_from_connection(&transaction, command.plan_id)?;
        if record.journal_sequence != command.expected_sequence {
            return Err(DatabaseError::ActionJournalCompareAndSwapFailed);
        }
        let requested_event = match (record.state, command.kind) {
            (ActionPlanState::Previewed, ActionCommandKind::Execute) => {
                ActionJournalEventKind::ExecuteRequested
            }
            (ActionPlanState::Executed, ActionCommandKind::Undo) => {
                ActionJournalEventKind::UndoRequested
            }
            _ => return Err(DatabaseError::ActionJournalInvalidTransition),
        };
        let requested_sequence = record
            .journal_sequence
            .checked_add(1)
            .ok_or(DatabaseError::InvalidCount)?;
        transaction.execute(
            "INSERT INTO action_command_requests( \
                 plan_id, request_id, command_kind, requested_sequence, created_at_unix_ms \
             ) VALUES (?1, ?2, ?3, ?4, ?5)",
            params![
                command.plan_id,
                command.request_id,
                action_command_kind_str(command.kind),
                to_i64(requested_sequence)?,
                now,
            ],
        )?;
        let command_request_id = transaction.last_insert_rowid();
        transaction.execute(
            "INSERT INTO action_journal_events( \
                 api_version, plan_id, sequence, event_kind, command_request_id, created_at_unix_ms \
             ) VALUES ('deskgraph.action-journal.v1', ?1, ?2, ?3, ?4, ?5)",
            params![
                command.plan_id,
                to_i64(requested_sequence)?,
                action_journal_event_kind_str(requested_event),
                command_request_id,
                now,
            ],
        )?;
        transaction.commit()?;
        Ok(ActionCommandStart {
            api_version: ActionCommandStart::API_VERSION,
            command_request_id,
            plan_id: command.plan_id,
            kind: command.kind,
            state: match command.kind {
                ActionCommandKind::Execute => ActionPlanState::ExecuteRequested,
                ActionCommandKind::Undo => ActionPlanState::UndoRequested,
            },
            journal_sequence: requested_sequence,
            idempotent: false,
        })
    }

    /// Claims an unfinished command for one executor. This writer transaction
    /// ends before any filesystem operation; callers must renew the lease if
    /// their bounded filesystem work can outlive the chosen duration.
    pub fn acquire_action_executor_lease(
        &mut self,
        plan_id: i64,
        owner_token: &str,
        lease_duration_ms: i64,
    ) -> Result<ActionExecutorLease, DatabaseError> {
        validate_action_executor_lease_input(plan_id, owner_token, lease_duration_ms)?;
        let now = unix_ms()?;
        let expires_at = now
            .checked_add(lease_duration_ms)
            .ok_or(DatabaseError::InvalidTimestamp)?;
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let record = action_execution_record_from_connection(&transaction, plan_id)?;
        if !matches!(
            record.state,
            ActionPlanState::ExecuteRequested
                | ActionPlanState::DirectRenameIntent
                | ActionPlanState::UndoRequested
                | ActionPlanState::UndoRenameIntent
        ) {
            return Err(DatabaseError::ActionJournalInvalidTransition);
        }
        transaction.execute(
            "INSERT INTO action_executor_leases( \
                 plan_id, owner_token, expires_at_unix_ms, heartbeat_at_unix_ms \
             ) VALUES (?1, ?2, ?3, ?4) \
             ON CONFLICT(plan_id) DO UPDATE SET \
                 owner_token = excluded.owner_token, \
                 expires_at_unix_ms = excluded.expires_at_unix_ms, \
                 heartbeat_at_unix_ms = excluded.heartbeat_at_unix_ms \
             WHERE action_executor_leases.owner_token = excluded.owner_token \
                OR action_executor_leases.expires_at_unix_ms <= ?4",
            params![plan_id, owner_token, expires_at, now],
        )?;
        let lease = action_executor_lease_from_connection(&transaction, plan_id)?;
        if lease.owner_token != owner_token || lease.expires_at_unix_ms != expires_at {
            return Err(DatabaseError::ActionExecutorLeaseUnavailable);
        }
        transaction.commit()?;
        Ok(lease)
    }

    /// Extends only the caller's still-valid lease. A process that lost its
    /// lease must reacquire and revalidate before performing another syscall.
    pub fn renew_action_executor_lease(
        &mut self,
        plan_id: i64,
        owner_token: &str,
        lease_duration_ms: i64,
    ) -> Result<ActionExecutorLease, DatabaseError> {
        validate_action_executor_lease_input(plan_id, owner_token, lease_duration_ms)?;
        let now = unix_ms()?;
        let expires_at = now
            .checked_add(lease_duration_ms)
            .ok_or(DatabaseError::InvalidTimestamp)?;
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let changed = transaction.execute(
            "UPDATE action_executor_leases \
             SET expires_at_unix_ms = ?3, heartbeat_at_unix_ms = ?4 \
             WHERE plan_id = ?1 AND owner_token = ?2 AND expires_at_unix_ms > ?4",
            params![plan_id, owner_token, expires_at, now],
        )?;
        if changed != 1 {
            return Err(DatabaseError::ActionExecutorLeaseUnavailable);
        }
        let lease = action_executor_lease_from_connection(&transaction, plan_id)?;
        transaction.commit()?;
        Ok(lease)
    }

    /// Releases only the current owner's lease. Releasing stale ownership is
    /// rejected rather than silently touching a new recovery process's lease.
    pub fn release_action_executor_lease(
        &mut self,
        plan_id: i64,
        owner_token: &str,
    ) -> Result<(), DatabaseError> {
        validate_action_executor_owner(plan_id, owner_token)?;
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let changed = transaction.execute(
            "DELETE FROM action_executor_leases WHERE plan_id = ?1 AND owner_token = ?2",
            params![plan_id, owner_token],
        )?;
        if changed != 1 {
            return Err(DatabaseError::ActionExecutorLeaseUnavailable);
        }
        transaction.commit()?;
        Ok(())
    }

    /// Appends a non-user event after the transaction engine has performed a
    /// bounded validation step. This method never touches the filesystem.
    pub fn append_action_journal_event(
        &mut self,
        append: ActionJournalAppend<'_>,
    ) -> Result<ActionExecutionRecord, DatabaseError> {
        validate_action_journal_append(&append)?;
        let now = unix_ms()?;
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let lease = action_executor_lease_from_connection(&transaction, append.plan_id)?;
        if lease.owner_token != append.executor_lease_owner_token || lease.expires_at_unix_ms <= now
        {
            return Err(DatabaseError::ActionExecutorLeaseUnavailable);
        }
        let record = action_execution_record_from_connection(&transaction, append.plan_id)?;
        if record.journal_sequence != append.expected_sequence
            || record.state != append.expected_state
        {
            return Err(DatabaseError::ActionJournalCompareAndSwapFailed);
        }
        let previous_command_request_id: Option<i64> = transaction.query_row(
            "SELECT command_request_id FROM action_journal_events \
             WHERE plan_id = ?1 AND sequence = ?2",
            params![append.plan_id, to_i64(append.expected_sequence)?],
            |row| row.get(0),
        )?;
        if previous_command_request_id != Some(append.command_request_id) {
            return Err(DatabaseError::ActionJournalInvalidTransition);
        }
        let command_kind: String = transaction
            .query_row(
                "SELECT command_kind FROM action_command_requests \
                 WHERE id = ?1 AND plan_id = ?2",
                params![append.command_request_id, append.plan_id],
                |row| row.get(0),
            )
            .optional()?
            .ok_or(DatabaseError::ActionJournalCommandNotFound)?;
        let command_kind = action_command_kind_from_str(&command_kind)?;
        if !internal_event_matches_command(append.kind, command_kind)
            || !transition_is_valid(append.expected_state, append.kind)
        {
            return Err(DatabaseError::ActionJournalInvalidTransition);
        }
        let next_sequence = append
            .expected_sequence
            .checked_add(1)
            .ok_or(DatabaseError::InvalidCount)?;
        transaction.execute(
            "INSERT INTO action_journal_events( \
                 api_version, plan_id, sequence, event_kind, command_request_id, created_at_unix_ms \
             ) VALUES ('deskgraph.action-journal.v1', ?1, ?2, ?3, ?4, ?5)",
            params![
                append.plan_id,
                to_i64(next_sequence)?,
                action_journal_event_kind_str(append.kind),
                append.command_request_id,
                now,
            ],
        )?;
        transaction.commit()?;
        self.action_execution_record(append.plan_id)
    }

    pub fn action_execution_record(
        &self,
        plan_id: i64,
    ) -> Result<ActionExecutionRecord, DatabaseError> {
        action_execution_record_from_connection(&self.connection, plan_id)
    }

    /// Loads the raw, canonical values needed by the filesystem transaction
    /// engine. UI callers must use `action_plan` instead. Missing bindings on
    /// legacy previews intentionally make this fail closed.
    pub fn action_execution_plan(
        &self,
        plan_id: i64,
    ) -> Result<ActionExecutionPlan, DatabaseError> {
        action_execution_plan_from_connection(&self.connection, plan_id)
    }

    /// Returns only bounded, path-free records for unfinished journal states.
    /// Startup recovery must reload and revalidate the plan before any action.
    pub fn incomplete_action_recovery(
        &self,
        limit: u32,
    ) -> Result<Vec<IncompleteActionRecovery>, DatabaseError> {
        if limit == 0 || limit > 100 {
            return Err(DatabaseError::ActionJournalInputInvalid);
        }
        let now = unix_ms()?;
        let candidates = {
            let mut statement = self.connection.prepare(
                "SELECT latest.plan_id, latest.command_request_id, latest.sequence \
                 FROM action_journal_events latest \
                 JOIN ( \
                    SELECT plan_id, MAX(sequence) AS sequence \
                    FROM action_journal_events GROUP BY plan_id \
                 ) tail ON tail.plan_id = latest.plan_id AND tail.sequence = latest.sequence \
                 JOIN action_execution_bindings binding ON binding.plan_id = latest.plan_id \
                 LEFT JOIN action_executor_leases lease ON lease.plan_id = latest.plan_id \
                 WHERE latest.event_kind IN ( \
                    'execute_requested', 'direct_rename_intent', \
                    'undo_requested', 'undo_rename_intent' \
                 ) \
                   AND (lease.plan_id IS NULL OR lease.expires_at_unix_ms <= ?1) \
                 ORDER BY latest.sequence ASC, latest.plan_id ASC LIMIT ?2",
            )?;
            statement
                .query_map(params![now, i64::from(limit)], |row| {
                    let sequence: i64 = row.get(2)?;
                    Ok((
                        row.get::<_, i64>(0)?,
                        row.get::<_, i64>(1)?,
                        u64::try_from(sequence)
                            .map_err(|_| rusqlite::Error::IntegralValueOutOfRange(2, sequence))?,
                    ))
                })?
                .collect::<Result<Vec<_>, _>>()?
        };
        let mut recovery = Vec::new();
        for (plan_id, command_request_id, journal_sequence) in candidates {
            let record = self.action_execution_record(plan_id)?;
            if !matches!(
                record.state,
                ActionPlanState::ExecuteRequested
                    | ActionPlanState::DirectRenameIntent
                    | ActionPlanState::UndoRequested
                    | ActionPlanState::UndoRenameIntent
            ) || record.journal_sequence != journal_sequence
            {
                return Err(DatabaseError::InvalidStoredValue);
            }
            recovery.push(IncompleteActionRecovery {
                plan_id,
                command_request_id,
                state: record.state,
                journal_sequence,
            });
        }
        Ok(recovery)
    }

    pub fn node_id_for_path_key(
        &self,
        scope_id: i64,
        path_key: &str,
    ) -> Result<Option<i64>, DatabaseError> {
        self.connection
            .query_row(
                "SELECT node_id FROM locations WHERE scope_id = ?1 AND path_key = ?2 AND present = 1",
                params![scope_id, path_key],
                |row| row.get(0),
            )
            .optional()
            .map_err(Into::into)
    }
}

fn validate_screenshot_group_input(
    scope_id: i64,
    group: &[ScreenshotGroupSourceRecord],
) -> Result<(), DatabaseError> {
    if !(2..=MAX_SCREENSHOT_GROUP_MEMBERS).contains(&group.len()) {
        return Err(DatabaseError::ScreenshotGroupMemberLimitExceeded);
    }
    let first = group
        .first()
        .ok_or(DatabaseError::ScreenshotGroupCandidateInputInvalid)?;
    let last = group
        .last()
        .ok_or(DatabaseError::ScreenshotGroupCandidateInputInvalid)?;
    if first.scope_id != scope_id
        || last
            .modified_unix_ns
            .checked_sub(first.modified_unix_ns)
            .is_none_or(|delta| !(0..=SCREENSHOT_GROUP_TIME_WINDOW_NS).contains(&delta))
    {
        return Err(DatabaseError::ScreenshotGroupCandidateInputInvalid);
    }
    let mut previous = None;
    let mut node_ids = std::collections::BTreeSet::new();
    for source in group {
        if source.scope_id != scope_id
            || source.node_id <= 0
            || source.location_id <= 0
            || source.image_metadata_id <= 0
            || source.ocr_extraction_job_id <= 0
            || source.size_bytes == 0
            || source.size_bytes > MAX_EXTRACTION_SOURCE_BYTES
            || source.modified_unix_ns < 0
            || source.pixel_width != first.pixel_width
            || source.pixel_height != first.pixel_height
            || !matches!(
                source.format,
                ImageFormat::Png | ImageFormat::Jpeg | ImageFormat::Webp
            )
            || source.ocr_chunk_count == 0
            || usize::try_from(source.ocr_chunk_count)
                .map_or(true, |count| count > MAX_EXTRACTION_CHUNKS)
            || source.ocr_provider_id.is_empty()
            || source.ocr_provider_id.len() > 128
            || source.ocr_provider_version.is_empty()
            || source.ocr_provider_version.len() > 128
            || !is_valid_image_dimensions(source.pixel_width, source.pixel_height)
            || !node_ids.insert(source.node_id)
        {
            return Err(DatabaseError::ScreenshotGroupCandidateInputInvalid);
        }
        let order = (source.modified_unix_ns, source.node_id);
        if previous.is_some_and(|previous| previous >= order) {
            return Err(DatabaseError::ScreenshotGroupCandidateInputInvalid);
        }
        previous = Some(order);
    }
    Ok(())
}

fn screenshot_group_membership_key(
    group: &[ScreenshotGroupSourceRecord],
) -> Result<String, DatabaseError> {
    if !(2..=MAX_SCREENSHOT_GROUP_MEMBERS).contains(&group.len()) {
        return Err(DatabaseError::ScreenshotGroupMemberLimitExceeded);
    }
    let mut node_ids = group
        .iter()
        .map(|source| source.node_id)
        .collect::<Vec<_>>();
    node_ids.sort_unstable();
    node_ids.dedup();
    if node_ids.len() != group.len() || node_ids.first().is_none_or(|node_id| *node_id <= 0) {
        return Err(DatabaseError::ScreenshotGroupCandidateInputInvalid);
    }
    let membership_key = node_ids
        .into_iter()
        .map(|node_id| node_id.to_string())
        .collect::<Vec<_>>()
        .join(",");
    if !(3..=511).contains(&membership_key.len()) {
        return Err(DatabaseError::ScreenshotGroupCandidateInputInvalid);
    }
    Ok(membership_key)
}

fn screenshot_group_evidence_key(
    group: &[ScreenshotGroupSourceRecord],
) -> Result<String, DatabaseError> {
    validate_screenshot_group_input(
        group
            .first()
            .ok_or(DatabaseError::ScreenshotGroupCandidateInputInvalid)?
            .scope_id,
        group,
    )?;
    let mut key = String::from("deskgraph.screenshot-group-evidence.v1|");
    for source in group {
        for field in [
            source.scope_id.to_string(),
            source.node_id.to_string(),
            source.location_id.to_string(),
            source.image_metadata_id.to_string(),
            source.ocr_extraction_job_id.to_string(),
            source.size_bytes.to_string(),
            source.modified_unix_ns.to_string(),
            source.format.as_str().to_string(),
            source.pixel_width.to_string(),
            source.pixel_height.to_string(),
            source.ocr_chunk_count.to_string(),
            source.ocr_provider_id.clone(),
            source.ocr_provider_version.clone(),
        ] {
            key.push_str(&field.len().to_string());
            key.push(':');
            key.push_str(&field);
            key.push('|');
        }
    }
    if !(16..=16_384).contains(&key.len()) {
        return Err(DatabaseError::ScreenshotGroupCandidateInputInvalid);
    }
    Ok(key)
}

fn screenshot_group_source_matches(
    connection: &Connection,
    source: &ScreenshotGroupSourceRecord,
) -> Result<bool, DatabaseError> {
    let matches: i64 = connection.query_row(
        "SELECT COUNT(*) \
         FROM image_metadata im \
         JOIN extraction_jobs image_job ON image_job.id = im.extraction_job_id \
            AND image_job.status = 'completed' \
         JOIN locations l ON l.id = im.location_id AND l.scope_id = im.scope_id \
            AND l.node_id = im.node_id AND l.present = 1 \
         JOIN nodes n ON n.id = im.node_id AND n.kind = 'file' \
         JOIN files f ON f.node_id = im.node_id \
         JOIN extraction_jobs ocr_job ON ocr_job.id = ?5 \
            AND ocr_job.scope_id = im.scope_id AND ocr_job.node_id = im.node_id \
            AND ocr_job.location_id = im.location_id \
            AND ocr_job.operation = 'screenshot_ocr' AND ocr_job.status = 'completed' \
            AND ocr_job.source_size_bytes = f.size_bytes \
            AND ocr_job.source_modified_unix_ns IS f.modified_unix_ns \
         WHERE im.id = ?4 AND im.scope_id = ?1 AND im.node_id = ?2 \
            AND im.location_id = ?3 AND im.active = 1 \
            AND im.source_size_bytes = ?6 AND im.source_modified_unix_ns IS ?7 \
            AND im.format = ?8 AND im.pixel_width = ?9 AND im.pixel_height = ?10 \
            AND f.size_bytes = ?6 AND f.modified_unix_ns IS ?7 \
            AND (SELECT COUNT(*) FROM content_chunks c \
                 WHERE c.extraction_job_id = ?5 AND c.active = 1) = ?11 \
            AND (SELECT COUNT(*) FROM content_chunks c \
                 WHERE c.extraction_job_id = ?5 AND c.active = 1 \
                   AND c.scope_id = ?1 AND c.node_id = ?2 AND c.location_id = ?3 \
                   AND c.provenance_kind = 'ocr_observation' \
                   AND c.source_size_bytes = ?6 AND c.source_modified_unix_ns IS ?7 \
                   AND c.provider_id = ?12 AND c.provider_version = ?13) = ?11",
        params![
            source.scope_id,
            source.node_id,
            source.location_id,
            source.image_metadata_id,
            source.ocr_extraction_job_id,
            to_i64(source.size_bytes)?,
            source.modified_unix_ns,
            source.format.as_str(),
            i64::from(source.pixel_width),
            i64::from(source.pixel_height),
            i64::from(source.ocr_chunk_count),
            source.ocr_provider_id,
            source.ocr_provider_version,
        ],
        |row| row.get(0),
    )?;
    Ok(matches == 1)
}

fn screenshot_group_candidate_from_sources(
    connection: &Connection,
    group_id: i64,
    scope_id: i64,
    observation_id: i64,
    confidence_basis_points: i64,
    observed_at_unix_ms: i64,
    sources: Vec<ScreenshotGroupSourceRecord>,
) -> Result<ScreenshotGroupCandidate, DatabaseError> {
    let mut total_size_bytes = 0_u64;
    let mut members = Vec::with_capacity(sources.len());
    for source in sources {
        let display_path = connection
            .query_row(
                "SELECT display_path FROM locations \
                 WHERE id = ?1 AND scope_id = ?2 AND node_id = ?3 AND present = 1",
                params![source.location_id, source.scope_id, source.node_id],
                |row| row.get::<_, String>(0),
            )
            .optional()?
            .filter(|path| !path.is_empty())
            .ok_or(DatabaseError::ScreenshotGroupCandidateNotCurrent)?;
        total_size_bytes = total_size_bytes
            .checked_add(source.size_bytes)
            .ok_or(DatabaseError::InvalidCount)?;
        members.push(ScreenshotGroupMember {
            node_id: source.node_id,
            location_id: source.location_id,
            display_path,
            image_metadata_id: source.image_metadata_id,
            ocr_extraction_job_id: source.ocr_extraction_job_id,
            size_bytes: source.size_bytes,
            modified_unix_ns: source.modified_unix_ns,
            format: source.format,
            pixel_width: source.pixel_width,
            pixel_height: source.pixel_height,
            ocr_chunk_count: source.ocr_chunk_count,
            ocr_provider_id: source.ocr_provider_id,
            ocr_provider_version: source.ocr_provider_version,
        });
    }
    Ok(ScreenshotGroupCandidate {
        api_version: ScreenshotGroupCandidate::API_VERSION,
        group_id,
        scope_id,
        state: ScreenshotGroupCandidateState::Suggested,
        members,
        total_size_bytes,
        members_independently_selectable: true,
        evidence: ScreenshotGroupEvidence {
            observation_id,
            rule_kind: ScreenshotGroupRuleKind::SameDimensionsTimeWindowWithOcr,
            confidence_basis_points: u16::try_from(confidence_basis_points)
                .map_err(|_| DatabaseError::InvalidStoredValue)?,
            observed_at_unix_ms,
            created_by: ScreenshotGroupCreator::SystemRule,
            provider_id: ScreenshotGroupEvidence::PROVIDER_ID,
            provider_version: ScreenshotGroupEvidence::PROVIDER_VERSION,
            model_version: None,
            time_window_seconds: 600,
            review_assistance_only: true,
            content_similarity_claimed: false,
            cleanup_authorized: false,
        },
    })
}

fn validate_exact_duplicate_sources(
    left: &ActionSourceRecord,
    right: &ActionSourceRecord,
) -> Result<(), DatabaseError> {
    if left.scope_id <= 0
        || left.scope_id != right.scope_id
        || left.node_id <= 0
        || left.node_id >= right.node_id
        || left.location_id <= 0
        || right.location_id <= 0
        || left.location_id == right.location_id
        || left.path_raw.is_empty()
        || right.path_raw.is_empty()
        || left.path_key.is_empty()
        || right.path_key.is_empty()
        || left.display_path.is_empty()
        || right.display_path.is_empty()
        || left.identity_kind == "path_fallback"
        || right.identity_kind == "path_fallback"
        || left.identity_kind.is_empty()
        || right.identity_kind.is_empty()
        || left.identity_key.is_empty()
        || right.identity_key.is_empty()
        || (left.identity_kind == right.identity_kind && left.identity_key == right.identity_key)
        || left.size_bytes == 0
        || left.size_bytes != right.size_bytes
        || left.size_bytes > MAX_FILE_RELATION_SOURCE_BYTES
    {
        return Err(DatabaseError::FileRelationCandidateInputInvalid);
    }
    Ok(())
}

fn validate_file_relation_sources(
    first: &ActionSourceRecord,
    second: &ActionSourceRecord,
) -> Result<(), DatabaseError> {
    if first.scope_id <= 0
        || first.scope_id != second.scope_id
        || first.node_id <= 0
        || second.node_id <= 0
        || first.node_id == second.node_id
        || first.location_id <= 0
        || second.location_id <= 0
        || first.location_id == second.location_id
        || first.path_raw.is_empty()
        || second.path_raw.is_empty()
        || first.path_key.is_empty()
        || second.path_key.is_empty()
        || first.display_path.is_empty()
        || second.display_path.is_empty()
        || first.identity_kind == "path_fallback"
        || second.identity_kind == "path_fallback"
        || first.identity_kind.is_empty()
        || second.identity_kind.is_empty()
        || first.identity_key.is_empty()
        || second.identity_key.is_empty()
        || (first.identity_kind == second.identity_kind
            && first.identity_key == second.identity_key)
    {
        return Err(DatabaseError::FileRelationCandidateInputInvalid);
    }
    Ok(())
}

fn explicit_version_name_from_source(
    source: &ActionSourceRecord,
) -> Result<ExplicitFileVersionName, DatabaseError> {
    explicit_version_name_from_display_path(&source.display_path)
}

fn explicit_version_name_from_display_path(
    display_path: &str,
) -> Result<ExplicitFileVersionName, DatabaseError> {
    let file_name = Path::new(display_path)
        .file_name()
        .and_then(|value| value.to_str())
        .ok_or(DatabaseError::FileRelationCandidateInputInvalid)?;
    parse_explicit_file_version_name(file_name)
        .ok_or(DatabaseError::FileRelationCandidateInputInvalid)
}

fn latest_equivalent_file_version_decision(
    connection: &Connection,
    relation_id: i64,
    current_observation_id: i64,
) -> Result<Option<FileVersionDecision>, DatabaseError> {
    let feedback = connection
        .query_row(
            "SELECT feedback.evidence_observation_id, feedback.sequence, feedback.decision, \
                    feedback.created_by, feedback.created_at_unix_ms \
             FROM file_version_feedback_events feedback \
             JOIN file_version_observations bound \
                ON bound.id = feedback.evidence_observation_id \
                AND bound.relation_id = feedback.relation_id \
             JOIN file_version_observations current \
                ON current.id = ?2 AND current.relation_id = feedback.relation_id \
             JOIN locations bound_older ON bound_older.id = bound.older_location_id \
             JOIN locations bound_newer ON bound_newer.id = bound.newer_location_id \
             JOIN locations current_older ON current_older.id = current.older_location_id \
             JOIN locations current_newer ON current_newer.id = current.newer_location_id \
             WHERE feedback.relation_id = ?1 \
               AND bound_older.node_id = current_older.node_id \
               AND bound_newer.node_id = current_newer.node_id \
               AND bound.base_key = current.base_key \
               AND bound.extension_key = current.extension_key \
               AND bound.older_version = current.older_version \
               AND bound.newer_version = current.newer_version \
               AND bound.confidence_basis_points = current.confidence_basis_points \
               AND bound.signal_kind = current.signal_kind \
               AND bound.created_by = current.created_by \
               AND bound.provider_id = current.provider_id \
               AND bound.provider_version = current.provider_version \
               AND bound.model_version IS current.model_version \
             ORDER BY feedback.sequence DESC LIMIT 1",
            params![relation_id, current_observation_id],
            |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, i64>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, i64>(4)?,
                ))
            },
        )
        .optional()?;
    feedback
        .map(
            |(evidence_observation_id, sequence, kind, creator, decided_at_unix_ms)| {
                if evidence_observation_id <= 0 || creator != "user" || decided_at_unix_ms < 0 {
                    return Err(DatabaseError::InvalidStoredValue);
                }
                Ok(FileVersionDecision {
                    sequence: u64::try_from(sequence)
                        .map_err(|_| DatabaseError::InvalidStoredValue)?,
                    evidence_observation_id,
                    kind: file_relation_decision_kind_from_str(&kind)?,
                    created_by: FileRelationDecisionCreator::User,
                    decided_at_unix_ms,
                })
            },
        )
        .transpose()
}

fn relation_snapshot_matches(
    transaction: &Transaction<'_>,
    source: &ActionSourceRecord,
) -> Result<bool, DatabaseError> {
    let matches: i64 = transaction.query_row(
        "SELECT COUNT(*) \
         FROM locations l \
         JOIN nodes n ON n.id = l.node_id AND n.kind = 'file' \
         JOIN files f ON f.node_id = l.node_id \
         WHERE l.id = ?1 AND l.scope_id = ?2 AND l.node_id = ?3 AND l.present = 1 \
           AND l.path_raw = ?4 AND l.path_key = ?5 AND l.display_path = ?6 \
           AND n.identity_kind = ?7 AND n.identity_key = ?8 \
           AND f.size_bytes = ?9 AND f.modified_unix_ns IS ?10",
        params![
            source.location_id,
            source.scope_id,
            source.node_id,
            source.path_raw.as_slice(),
            source.path_key.as_str(),
            source.display_path.as_str(),
            source.identity_kind.as_str(),
            source.identity_key.as_slice(),
            to_i64(source.size_bytes)?,
            source.modified_unix_ns,
        ],
        |row| row.get(0),
    )?;
    Ok(matches == 1)
}

#[derive(Clone, Debug)]
struct StoredActionPlan {
    id: i64,
    operation: ActionOperation,
    scope_id: i64,
    node_id: i64,
    source_display_path: String,
    destination_display_path: String,
    execution_strategy: ActionExecutionStrategy,
    created_at_unix_ms: i64,
}

#[derive(Clone, Debug)]
struct PreviewExecutionBinding {
    scope_root_node_id: i64,
    scope_root_identity_kind: String,
    scope_root_identity_key: Vec<u8>,
    parent_node_id: i64,
    parent_identity_kind: String,
    parent_identity_key: Vec<u8>,
}

fn action_plan_base_from_connection(
    connection: &Connection,
    plan_id: i64,
) -> Result<StoredActionPlan, DatabaseError> {
    if plan_id <= 0 {
        return Err(DatabaseError::ActionPlanNotFound);
    }
    connection
        .query_row(
            "SELECT id, api_version, policy_version, operation, scope_id, node_id, \
                    source_display_path, destination_display_path, execution_strategy, \
                    created_at_unix_ms \
             FROM action_plans WHERE id = ?1",
            [plan_id],
            |row| {
                let api_version: String = row.get(1)?;
                let policy_version: String = row.get(2)?;
                if api_version != "deskgraph.action-plan.v1"
                    || policy_version != ActionPolicyReport::API_VERSION
                {
                    return Err(rusqlite::Error::InvalidQuery);
                }
                Ok(StoredActionPlan {
                    id: row.get(0)?,
                    operation: action_operation_from_str(&row.get::<_, String>(3)?)?,
                    scope_id: row.get(4)?,
                    node_id: row.get(5)?,
                    source_display_path: row.get(6)?,
                    destination_display_path: row.get(7)?,
                    execution_strategy: action_execution_strategy_from_str(
                        &row.get::<_, String>(8)?,
                    )?,
                    created_at_unix_ms: row.get(9)?,
                })
            },
        )
        .optional()?
        .ok_or(DatabaseError::ActionPlanNotFound)
}

fn action_journal_events(
    connection: &Connection,
    plan_id: i64,
) -> Result<Vec<ActionJournalEvent>, DatabaseError> {
    let mut statement = connection.prepare(
        "SELECT id, plan_id, sequence, event_kind, command_request_id, created_at_unix_ms \
         FROM action_journal_events WHERE plan_id = ?1 ORDER BY sequence ASC",
    )?;
    let rows = statement.query_map([plan_id], |row| {
        Ok(ActionJournalEvent {
            api_version: ActionJournalEvent::API_VERSION,
            event_id: row.get(0)?,
            plan_id: row.get(1)?,
            sequence: row_u64(row, 2)?,
            kind: action_journal_event_kind_from_str(&row.get::<_, String>(3)?)?,
            command_request_id: row.get(4)?,
            created_at_unix_ms: row.get(5)?,
        })
    })?;
    rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
}

/// Resolves an idempotency request against only its own immutable journal
/// chain. Later commands may change the plan's global state, but must never
/// rewrite the historical result returned for this request id.
fn action_command_outcome_from_journal(
    events: &[ActionJournalEvent],
    command_request_id: i64,
    command: ActionCommandKind,
    requested_sequence: u64,
) -> Result<(ActionPlanState, u64), DatabaseError> {
    let command_events = events
        .iter()
        .filter(|event| event.command_request_id == Some(command_request_id));
    let mut outcome = None;
    for event in command_events {
        let state = match (command, event.kind) {
            (ActionCommandKind::Execute, ActionJournalEventKind::ExecuteRequested) => {
                ActionPlanState::ExecuteRequested
            }
            (ActionCommandKind::Execute, ActionJournalEventKind::ExecuteRequestNotStarted)
            | (ActionCommandKind::Execute, ActionJournalEventKind::ExecutionNotApplied) => {
                ActionPlanState::Previewed
            }
            (ActionCommandKind::Execute, ActionJournalEventKind::DirectRenameIntent) => {
                ActionPlanState::DirectRenameIntent
            }
            (ActionCommandKind::Execute, ActionJournalEventKind::ExecutionCompleted) => {
                ActionPlanState::Executed
            }
            (ActionCommandKind::Execute, ActionJournalEventKind::ExecutionNeedsAttention) => {
                ActionPlanState::NeedsAttention
            }
            (ActionCommandKind::Undo, ActionJournalEventKind::UndoRequested) => {
                ActionPlanState::UndoRequested
            }
            (ActionCommandKind::Undo, ActionJournalEventKind::UndoRequestNotStarted)
            | (ActionCommandKind::Undo, ActionJournalEventKind::UndoNotApplied) => {
                ActionPlanState::Executed
            }
            (ActionCommandKind::Undo, ActionJournalEventKind::UndoRenameIntent) => {
                ActionPlanState::UndoRenameIntent
            }
            (ActionCommandKind::Undo, ActionJournalEventKind::UndoCompleted) => {
                ActionPlanState::Undone
            }
            (ActionCommandKind::Undo, ActionJournalEventKind::UndoNeedsAttention) => {
                ActionPlanState::NeedsAttention
            }
            _ => return Err(DatabaseError::InvalidStoredValue),
        };
        if outcome.is_none() {
            let expected = match command {
                ActionCommandKind::Execute => ActionJournalEventKind::ExecuteRequested,
                ActionCommandKind::Undo => ActionJournalEventKind::UndoRequested,
            };
            if event.sequence != requested_sequence || event.kind != expected {
                return Err(DatabaseError::InvalidStoredValue);
            }
        }
        outcome = Some((state, event.sequence));
    }
    outcome.ok_or(DatabaseError::InvalidStoredValue)
}

fn reduce_journal_or_stored_value(
    events: &[ActionJournalEvent],
) -> Result<ActionPlanState, DatabaseError> {
    reduce_action_journal(events).map_err(|_| DatabaseError::InvalidStoredValue)
}

fn last_journal_sequence(events: &[ActionJournalEvent]) -> Result<u64, DatabaseError> {
    events
        .last()
        .map(|event| event.sequence)
        .ok_or(DatabaseError::InvalidStoredValue)
}

fn action_execution_record_from_connection(
    connection: &Connection,
    plan_id: i64,
) -> Result<ActionExecutionRecord, DatabaseError> {
    let plan = action_plan_base_from_connection(connection, plan_id)?;
    let binding = connection
        .query_row(
            "SELECT source_hash_bytes, source_sha256, scope_root_node_id, scope_root_identity_kind, \
                    scope_root_identity_key, parent_node_id, parent_identity_kind, \
                    parent_identity_key, created_at_unix_ms \
             FROM action_execution_bindings WHERE plan_id = ?1",
            [plan_id],
            |row| {
                Ok(ActionExecutionBinding {
                    api_version: ActionExecutionBinding::API_VERSION,
                    source_hash_bytes: row_u64(row, 0)?,
                    source_sha256: row.get(1)?,
                    scope_root_node_id: row.get(2)?,
                    scope_root_identity_kind: row.get(3)?,
                    scope_root_identity_key: row.get(4)?,
                    parent_node_id: row.get(5)?,
                    parent_identity_kind: row.get(6)?,
                    parent_identity_key: row.get(7)?,
                    created_at_unix_ms: row.get(8)?,
                })
            },
        )
        .optional()?
        .ok_or(DatabaseError::ActionExecutionBindingUnavailable)?;
    if binding.source_sha256.len() != 32
        || binding.scope_root_identity_kind == "path_fallback"
        || binding.parent_identity_kind == "path_fallback"
    {
        return Err(DatabaseError::InvalidStoredValue);
    }
    let events = action_journal_events(connection, plan_id)?;
    Ok(ActionExecutionRecord {
        api_version: ActionExecutionRecord::API_VERSION,
        plan_id,
        operation: plan.operation,
        execution_strategy: plan.execution_strategy,
        state: reduce_journal_or_stored_value(&events)?,
        journal_sequence: last_journal_sequence(&events)?,
        binding,
    })
}

fn action_execution_plan_from_connection(
    connection: &Connection,
    plan_id: i64,
) -> Result<ActionExecutionPlan, DatabaseError> {
    let record = action_execution_record_from_connection(connection, plan_id)?;
    connection
        .query_row(
            "SELECT scope_id, node_id, source_location_id, source_path_raw, source_path_key, \
                    destination_path_raw, destination_path_key, source_identity_kind, \
                    source_identity_key, source_size_bytes, source_modified_unix_ns, \
                    execution_strategy \
             FROM action_plans WHERE id = ?1",
            [plan_id],
            |row| {
                let source_size_bytes: i64 = row.get(9)?;
                let strategy: String = row.get(11)?;
                Ok(ActionExecutionPlan {
                    plan_id,
                    scope_id: row.get(0)?,
                    node_id: row.get(1)?,
                    source_location_id: row.get(2)?,
                    source_path_raw: row.get(3)?,
                    source_path_key: row.get(4)?,
                    destination_path_raw: row.get(5)?,
                    destination_path_key: row.get(6)?,
                    source_identity_kind: row.get(7)?,
                    source_identity_key: row.get(8)?,
                    source_size_bytes: u64::try_from(source_size_bytes).map_err(|_| {
                        rusqlite::Error::IntegralValueOutOfRange(9, source_size_bytes)
                    })?,
                    source_modified_unix_ns: row.get(10)?,
                    execution_strategy: action_execution_strategy_from_str(&strategy)?,
                    binding: record.binding,
                })
            },
        )
        .optional()?
        .ok_or(DatabaseError::ActionPlanNotFound)
        .and_then(validate_action_execution_plan)
}

fn validate_action_execution_plan(
    plan: ActionExecutionPlan,
) -> Result<ActionExecutionPlan, DatabaseError> {
    if plan.plan_id <= 0
        || plan.scope_id <= 0
        || plan.node_id <= 0
        || plan.source_location_id <= 0
        || plan.source_path_raw.is_empty()
        || plan.source_path_raw.len() > MAX_ACTION_PATH_BYTES
        || plan.source_path_key.is_empty()
        || plan.source_path_key.len() > MAX_ACTION_PATH_BYTES
        || plan.destination_path_raw.is_empty()
        || plan.destination_path_raw.len() > MAX_ACTION_PATH_BYTES
        || plan.destination_path_key.is_empty()
        || plan.destination_path_key.len() > MAX_ACTION_PATH_BYTES
        || plan.source_identity_kind.is_empty()
        || plan.source_identity_kind.len() > 128
        || plan.source_identity_key.is_empty()
        || plan.source_identity_key.len() > 4096
        || plan.binding.source_hash_bytes != plan.source_size_bytes
    {
        return Err(DatabaseError::InvalidStoredValue);
    }
    Ok(plan)
}

fn action_executor_lease_from_connection(
    connection: &Connection,
    plan_id: i64,
) -> Result<ActionExecutorLease, DatabaseError> {
    connection
        .query_row(
            "SELECT plan_id, owner_token, expires_at_unix_ms \
             FROM action_executor_leases WHERE plan_id = ?1",
            [plan_id],
            |row| {
                Ok(ActionExecutorLease {
                    plan_id: row.get(0)?,
                    owner_token: row.get(1)?,
                    expires_at_unix_ms: row.get(2)?,
                })
            },
        )
        .optional()?
        .ok_or(DatabaseError::ActionExecutorLeaseUnavailable)
}

fn preview_execution_binding(
    transaction: &Transaction<'_>,
    plan: &ActionPlanWrite<'_>,
) -> Result<PreviewExecutionBinding, DatabaseError> {
    let binding = transaction
        .query_row(
            "SELECT root_node.id, root_node.identity_kind, root_node.identity_key, \
                    parent_node.id, parent_node.identity_kind, parent_node.identity_key \
             FROM authorized_scopes scope \
             JOIN locations source ON source.id = ?1 AND source.scope_id = scope.id \
                AND source.node_id = ?2 AND source.present = 1 \
             JOIN scan_jobs source_scan ON source_scan.id = source.last_seen_scan_id \
                AND source_scan.scope_id = scope.id AND source_scan.status = 'completed' \
             JOIN locations root ON root.scope_id = scope.id AND root.path_key = scope.path_key \
                AND root.present = 1 \
             JOIN nodes root_node ON root_node.id = root.node_id AND root_node.kind = 'folder' \
             JOIN edges parent_edge ON parent_edge.scope_id = scope.id \
                AND parent_edge.source_node_id = source.node_id \
                AND parent_edge.kind = 'located_in' AND parent_edge.active = 1 \
             JOIN locations parent ON parent.scope_id = scope.id \
                AND parent.node_id = parent_edge.target_node_id AND parent.present = 1 \
             JOIN nodes parent_node ON parent_node.id = parent.node_id AND parent_node.kind = 'folder' \
             WHERE scope.id = ?3 \
               AND root_node.identity_kind <> 'path_fallback' \
               AND parent_node.identity_kind <> 'path_fallback' \
               AND root_node.identity_kind = ?4 AND root_node.identity_key = ?5 \
               AND parent_node.identity_kind = ?6 AND parent_node.identity_key = ?7 \
               AND (SELECT COUNT(*) FROM locations only_source \
                    WHERE only_source.scope_id = scope.id \
                      AND only_source.node_id = source.node_id AND only_source.present = 1) = 1 \
               AND (SELECT COUNT(*) FROM edges only_parent \
                    WHERE only_parent.scope_id = scope.id \
                      AND only_parent.source_node_id = source.node_id \
                      AND only_parent.kind = 'located_in' AND only_parent.active = 1) = 1",
            params![
                plan.source_location_id,
                plan.node_id,
                plan.scope_id,
                plan.scope_root_identity_kind,
                plan.scope_root_identity_key,
                plan.parent_identity_kind,
                plan.parent_identity_key,
            ],
            |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, Vec<u8>>(2)?,
                    row.get::<_, i64>(3)?,
                    row.get::<_, String>(4)?,
                    row.get::<_, Vec<u8>>(5)?,
                ))
            },
        )
        .optional()?
        .ok_or(DatabaseError::ActionExecutionBindingUnavailable)?;
    Ok(PreviewExecutionBinding {
        scope_root_node_id: binding.0,
        scope_root_identity_kind: binding.1,
        scope_root_identity_key: binding.2,
        parent_node_id: binding.3,
        parent_identity_kind: binding.4,
        parent_identity_key: binding.5,
    })
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct CleanupSelectionSnapshot {
    location_id: i64,
    size_bytes: u64,
    modified_unix_ns: Option<i64>,
}

fn smart_cleanup_source_kind_str(kind: SmartCleanupSourceKind) -> &'static str {
    match kind {
        SmartCleanupSourceKind::ExactDuplicate => "exact_duplicate",
        SmartCleanupSourceKind::Version => "version",
        SmartCleanupSourceKind::ScreenshotReviewGroup => "screenshot_review_group",
    }
}

fn smart_cleanup_source_kind_from_str(
    value: &str,
) -> Result<SmartCleanupSourceKind, DatabaseError> {
    match value {
        "exact_duplicate" => Ok(SmartCleanupSourceKind::ExactDuplicate),
        "version" => Ok(SmartCleanupSourceKind::Version),
        "screenshot_review_group" => Ok(SmartCleanupSourceKind::ScreenshotReviewGroup),
        _ => Err(DatabaseError::InvalidStoredValue),
    }
}

fn normalize_cleanup_source_validation_error(error: DatabaseError) -> DatabaseError {
    match error {
        DatabaseError::FileRelationCandidateNotFound
        | DatabaseError::FileRelationCandidateNotCurrent
        | DatabaseError::ScreenshotGroupCandidateNotFound
        | DatabaseError::ScreenshotGroupCandidateNotCurrent => {
            DatabaseError::CleanupActionSourceNotCurrent
        }
        error => error,
    }
}

fn validate_cleanup_selection_input(
    selection: &CleanupActionSelection,
) -> Result<(), DatabaseError> {
    if selection.scope_id <= 0
        || selection.source_id <= 0
        || selection.source_observation_id <= 0
        || selection.target_node_id <= 0
        || selection.keeper_node_id.is_some_and(|node_id| node_id <= 0)
        || selection.keeper_node_id == Some(selection.target_node_id)
        || matches!(
            selection.source_kind,
            SmartCleanupSourceKind::ExactDuplicate | SmartCleanupSourceKind::Version
        ) && selection.keeper_node_id.is_none()
    {
        return Err(DatabaseError::CleanupActionPlanInputInvalid);
    }
    Ok(())
}

fn cleanup_selection_snapshot(
    connection: &Connection,
    selection: &CleanupActionSelection,
) -> Result<CleanupSelectionSnapshot, DatabaseError> {
    validate_cleanup_selection_input(selection)?;
    ensure_scope_access_permitted(connection, selection.scope_id)?;
    ensure_scope_queryable(connection, selection.scope_id)?;
    match selection.source_kind {
        SmartCleanupSourceKind::ExactDuplicate => {
            let row = connection
                .query_row(
                    "SELECT candidate.left_node_id, candidate.right_node_id, \
                            observation.left_location_id, observation.right_location_id, \
                            observation.source_size_bytes, observation.left_modified_unix_ns, \
                            observation.right_modified_unix_ns \
                     FROM file_relation_candidates candidate \
                     JOIN file_relation_observations observation \
                       ON observation.relation_id = candidate.id \
                     JOIN locations left_location ON left_location.id = observation.left_location_id \
                     JOIN locations right_location ON right_location.id = observation.right_location_id \
                     JOIN nodes left_node ON left_node.id = candidate.left_node_id \
                     JOIN nodes right_node ON right_node.id = candidate.right_node_id \
                     JOIN files left_file ON left_file.node_id = candidate.left_node_id \
                     JOIN files right_file ON right_file.node_id = candidate.right_node_id \
                     WHERE candidate.id = ?1 AND candidate.scope_id = ?2 \
                       AND candidate.relation_kind = 'exact_duplicate' AND observation.id = ?3 \
                       AND observation.id = ( \
                           SELECT latest.id FROM file_relation_observations latest \
                           WHERE latest.relation_id = candidate.id \
                           ORDER BY latest.observed_at_unix_ms DESC, latest.id DESC LIMIT 1 \
                       ) \
                       AND NOT EXISTS ( \
                           SELECT 1 FROM file_relation_feedback_events feedback \
                           WHERE feedback.relation_id = candidate.id \
                       ) \
                       AND left_location.scope_id = candidate.scope_id \
                       AND right_location.scope_id = candidate.scope_id \
                       AND left_location.node_id = candidate.left_node_id \
                       AND right_location.node_id = candidate.right_node_id \
                       AND left_location.present = 1 AND right_location.present = 1 \
                       AND left_node.kind = 'file' AND right_node.kind = 'file' \
                       AND left_node.identity_kind <> 'path_fallback' \
                       AND right_node.identity_kind <> 'path_fallback' \
                       AND left_file.size_bytes = observation.source_size_bytes \
                       AND right_file.size_bytes = observation.source_size_bytes \
                       AND left_file.modified_unix_ns IS observation.left_modified_unix_ns \
                       AND right_file.modified_unix_ns IS observation.right_modified_unix_ns",
                    params![
                        selection.source_id,
                        selection.scope_id,
                        selection.source_observation_id
                    ],
                    |row| {
                        Ok((
                            row.get::<_, i64>(0)?,
                            row.get::<_, i64>(1)?,
                            row.get::<_, i64>(2)?,
                            row.get::<_, i64>(3)?,
                            row_u64(row, 4)?,
                            row.get::<_, Option<i64>>(5)?,
                            row.get::<_, Option<i64>>(6)?,
                        ))
                    },
                )
                .optional()?
                .ok_or(DatabaseError::CleanupActionSourceNotCurrent)?;
            let keeper = selection
                .keeper_node_id
                .ok_or(DatabaseError::CleanupActionPlanInputInvalid)?;
            let (location_id, modified_unix_ns) =
                if selection.target_node_id == row.0 && keeper == row.1 {
                    (row.2, row.5)
                } else if selection.target_node_id == row.1 && keeper == row.0 {
                    (row.3, row.6)
                } else {
                    return Err(DatabaseError::CleanupActionPlanInputInvalid);
                };
            Ok(CleanupSelectionSnapshot {
                location_id,
                size_bytes: row.4,
                modified_unix_ns,
            })
        }
        SmartCleanupSourceKind::Version => {
            let row = connection
                .query_row(
                    "SELECT older_location.node_id, newer_location.node_id, \
                            observation.older_location_id, observation.newer_location_id, \
                            observation.older_size_bytes, observation.newer_size_bytes, \
                            observation.older_modified_unix_ns, observation.newer_modified_unix_ns \
                     FROM file_relation_candidates candidate \
                     JOIN file_version_observations observation \
                       ON observation.relation_id = candidate.id \
                     JOIN locations older_location ON older_location.id = observation.older_location_id \
                     JOIN locations newer_location ON newer_location.id = observation.newer_location_id \
                     JOIN nodes older_node ON older_node.id = older_location.node_id \
                     JOIN nodes newer_node ON newer_node.id = newer_location.node_id \
                     JOIN files older_file ON older_file.node_id = older_location.node_id \
                     JOIN files newer_file ON newer_file.node_id = newer_location.node_id \
                     WHERE candidate.id = ?1 AND candidate.scope_id = ?2 \
                       AND candidate.relation_kind = 'version' AND observation.id = ?3 \
                       AND observation.id = ( \
                           SELECT latest.id FROM file_version_observations latest \
                           WHERE latest.relation_id = candidate.id \
                           ORDER BY latest.observed_at_unix_ms DESC, latest.id DESC LIMIT 1 \
                       ) \
                       AND older_location.scope_id = candidate.scope_id \
                       AND newer_location.scope_id = candidate.scope_id \
                       AND older_location.present = 1 AND newer_location.present = 1 \
                       AND older_node.kind = 'file' AND newer_node.kind = 'file' \
                       AND older_node.identity_kind <> 'path_fallback' \
                       AND newer_node.identity_kind <> 'path_fallback' \
                       AND older_file.size_bytes = observation.older_size_bytes \
                       AND newer_file.size_bytes = observation.newer_size_bytes \
                       AND older_file.modified_unix_ns IS observation.older_modified_unix_ns \
                       AND newer_file.modified_unix_ns IS observation.newer_modified_unix_ns",
                    params![
                        selection.source_id,
                        selection.scope_id,
                        selection.source_observation_id
                    ],
                    |row| {
                        Ok((
                            row.get::<_, i64>(0)?,
                            row.get::<_, i64>(1)?,
                            row.get::<_, i64>(2)?,
                            row.get::<_, i64>(3)?,
                            row_u64(row, 4)?,
                            row_u64(row, 5)?,
                            row.get::<_, Option<i64>>(6)?,
                            row.get::<_, Option<i64>>(7)?,
                        ))
                    },
                )
                .optional()?
                .ok_or(DatabaseError::CleanupActionSourceNotCurrent)?;
            if latest_equivalent_file_version_decision(
                connection,
                selection.source_id,
                selection.source_observation_id,
            )?
            .is_some()
            {
                return Err(DatabaseError::CleanupActionSourceNotCurrent);
            }
            let keeper = selection
                .keeper_node_id
                .ok_or(DatabaseError::CleanupActionPlanInputInvalid)?;
            if selection.target_node_id != row.0 || keeper != row.1 {
                return Err(DatabaseError::CleanupActionPlanInputInvalid);
            }
            let (location_id, size_bytes, modified_unix_ns) = (row.2, row.4, row.6);
            Ok(CleanupSelectionSnapshot {
                location_id,
                size_bytes,
                modified_unix_ns,
            })
        }
        SmartCleanupSourceKind::ScreenshotReviewGroup => {
            let (scope_id, membership_key) =
                screenshot_group_identity_from_connection(connection, selection.source_id)?;
            if scope_id != selection.scope_id {
                return Err(DatabaseError::CleanupActionSourceNotCurrent);
            }
            let sources = current_screenshot_group_for_membership(
                connection,
                selection.scope_id,
                &membership_key,
            )?
            .ok_or(DatabaseError::CleanupActionSourceNotCurrent)?;
            let evidence_key = screenshot_group_evidence_key(&sources)?;
            let observation = screenshot_group_observation_for_evidence(
                connection,
                selection.source_id,
                &evidence_key,
            )?
            .ok_or(DatabaseError::CleanupActionSourceNotCurrent)?;
            if observation.id != selection.source_observation_id {
                return Err(DatabaseError::CleanupActionSourceNotCurrent);
            }
            validate_screenshot_group_observation(
                connection,
                selection.scope_id,
                &membership_key,
                &observation,
            )?;
            if selection
                .keeper_node_id
                .is_some_and(|keeper| !sources.iter().any(|source| source.node_id == keeper))
            {
                return Err(DatabaseError::CleanupActionPlanInputInvalid);
            }
            let source = sources
                .iter()
                .find(|source| source.node_id == selection.target_node_id)
                .ok_or(DatabaseError::CleanupActionPlanInputInvalid)?;
            Ok(CleanupSelectionSnapshot {
                location_id: source.location_id,
                size_bytes: source.size_bytes,
                modified_unix_ns: Some(source.modified_unix_ns),
            })
        }
    }
}

fn cleanup_keeper_snapshot(
    connection: &Connection,
    selection: &CleanupActionSelection,
    keeper_node_id: i64,
) -> Result<CleanupSelectionSnapshot, DatabaseError> {
    if selection.keeper_node_id != Some(keeper_node_id) || keeper_node_id <= 0 {
        return Err(DatabaseError::CleanupActionPlanInputInvalid);
    }
    match selection.source_kind {
        SmartCleanupSourceKind::Version => connection
            .query_row(
                "SELECT newer_location.id, observation.newer_size_bytes, \
                        observation.newer_modified_unix_ns \
                 FROM file_relation_candidates candidate \
                 JOIN file_version_observations observation \
                   ON observation.relation_id = candidate.id \
                 JOIN locations newer_location ON newer_location.id = observation.newer_location_id \
                 JOIN files newer_file ON newer_file.node_id = newer_location.node_id \
                 WHERE candidate.id = ?1 AND candidate.scope_id = ?2 \
                   AND candidate.relation_kind = 'version' AND observation.id = ?3 \
                   AND newer_location.node_id = ?4 AND newer_location.present = 1 \
                   AND newer_file.size_bytes = observation.newer_size_bytes \
                   AND newer_file.modified_unix_ns IS observation.newer_modified_unix_ns \
                   AND observation.id = ( \
                       SELECT latest.id FROM file_version_observations latest \
                       WHERE latest.relation_id = candidate.id \
                       ORDER BY latest.observed_at_unix_ms DESC, latest.id DESC LIMIT 1 \
                   )",
                params![
                    selection.source_id,
                    selection.scope_id,
                    selection.source_observation_id,
                    keeper_node_id
                ],
                |row| {
                    Ok(CleanupSelectionSnapshot {
                        location_id: row.get(0)?,
                        size_bytes: row_u64(row, 1)?,
                        modified_unix_ns: row.get(2)?,
                    })
                },
            )
            .optional()?
            .ok_or(DatabaseError::CleanupActionSourceNotCurrent),
        SmartCleanupSourceKind::ExactDuplicate
        | SmartCleanupSourceKind::ScreenshotReviewGroup => {
            let keeper_selection = CleanupActionSelection {
                keeper_node_id: Some(selection.target_node_id),
                target_node_id: keeper_node_id,
                ..*selection
            };
            cleanup_selection_snapshot(connection, &keeper_selection)
        }
    }
}

fn cleanup_execution_source_from_connection(
    connection: &Connection,
    scope_id: i64,
    node_id: i64,
    location_id: i64,
    size_bytes: u64,
    modified_unix_ns: Option<i64>,
) -> Result<ActionExecutionSourceRecord, DatabaseError> {
    connection
        .query_row(
            "SELECT source.scope_id, source.node_id, source.id, source.path_raw, \
                    source.path_key, source.display_path, source_node.identity_kind, \
                    source_node.identity_key, source_file.size_bytes, source_file.modified_unix_ns, \
                    root_node.id, root_node.identity_kind, root_node.identity_key, \
                    parent_node.id, parent_node.identity_kind, parent_node.identity_key \
             FROM authorized_scopes scope \
             JOIN locations source ON source.scope_id = scope.id AND source.id = ?3 \
                AND source.node_id = ?2 AND source.present = 1 \
             JOIN nodes source_node ON source_node.id = source.node_id AND source_node.kind = 'file' \
                AND source_node.identity_kind <> 'path_fallback' \
             JOIN files source_file ON source_file.node_id = source.node_id \
                AND source_file.size_bytes = ?4 AND source_file.modified_unix_ns IS ?5 \
             JOIN scan_jobs source_scan ON source_scan.id = source.last_seen_scan_id \
                AND source_scan.scope_id = scope.id AND source_scan.status = 'completed' \
             JOIN locations root ON root.scope_id = scope.id AND root.path_key = scope.path_key \
                AND root.present = 1 \
             JOIN nodes root_node ON root_node.id = root.node_id AND root_node.kind = 'folder' \
                AND root_node.identity_kind <> 'path_fallback' \
             JOIN edges parent_edge ON parent_edge.scope_id = scope.id \
                AND parent_edge.source_node_id = source.node_id \
                AND parent_edge.kind = 'located_in' AND parent_edge.active = 1 \
             JOIN locations parent ON parent.scope_id = scope.id \
                AND parent.node_id = parent_edge.target_node_id AND parent.present = 1 \
             JOIN nodes parent_node ON parent_node.id = parent.node_id AND parent_node.kind = 'folder' \
                AND parent_node.identity_kind <> 'path_fallback' \
             WHERE scope.id = ?1 \
               AND (SELECT COUNT(*) FROM locations only_source \
                    WHERE only_source.scope_id = scope.id \
                      AND only_source.node_id = source.node_id AND only_source.present = 1) = 1 \
               AND (SELECT COUNT(*) FROM locations only_root \
                    WHERE only_root.scope_id = scope.id AND only_root.path_key = scope.path_key \
                      AND only_root.present = 1) = 1 \
               AND (SELECT COUNT(*) FROM edges only_parent \
                    WHERE only_parent.scope_id = scope.id \
                      AND only_parent.source_node_id = source.node_id \
                      AND only_parent.kind = 'located_in' AND only_parent.active = 1) = 1 \
               AND (SELECT COUNT(*) FROM locations only_parent_location \
                    WHERE only_parent_location.scope_id = scope.id \
                      AND only_parent_location.node_id = parent_node.id \
                      AND only_parent_location.present = 1) = 1",
            params![
                scope_id,
                node_id,
                location_id,
                to_i64(size_bytes)?,
                modified_unix_ns
            ],
            |row| {
                Ok(ActionExecutionSourceRecord {
                    source: ActionSourceRecord {
                        scope_id: row.get(0)?,
                        node_id: row.get(1)?,
                        location_id: row.get(2)?,
                        path_raw: row.get(3)?,
                        path_key: row.get(4)?,
                        display_path: row.get(5)?,
                        identity_kind: row.get(6)?,
                        identity_key: row.get(7)?,
                        size_bytes: row_u64(row, 8)?,
                        modified_unix_ns: row.get(9)?,
                    },
                    scope_root_node_id: row.get(10)?,
                    scope_root_identity_kind: row.get(11)?,
                    scope_root_identity_key: row.get(12)?,
                    parent_node_id: row.get(13)?,
                    parent_identity_kind: row.get(14)?,
                    parent_identity_key: row.get(15)?,
                })
            },
        )
        .optional()?
        .ok_or(DatabaseError::CleanupActionSourceNotCurrent)
}

fn validate_cleanup_action_plan_write(
    plan: &CleanupActionPlanWrite<'_>,
) -> Result<(), DatabaseError> {
    validate_cleanup_selection_input(&plan.selection)?;
    let keeper_valid = match (plan.selection.keeper_node_id, plan.keeper) {
        (None, None) => plan.selection.source_kind == SmartCleanupSourceKind::ScreenshotReviewGroup,
        (Some(_), Some(keeper)) => {
            keeper.location_id > 0
                && !keeper.identity_kind.is_empty()
                && keeper.identity_kind.len() <= 128
                && keeper.identity_kind != "path_fallback"
                && !keeper.identity_key.is_empty()
                && keeper.identity_key.len() <= 4096
                && keeper.sha256.len() == 32
                && keeper.hash_bytes == keeper.size_bytes
                && keeper.scope_root_node_id > 0
                && !keeper.scope_root_identity_kind.is_empty()
                && keeper.scope_root_identity_kind.len() <= 128
                && keeper.scope_root_identity_kind != "path_fallback"
                && !keeper.scope_root_identity_key.is_empty()
                && keeper.scope_root_identity_key.len() <= 4096
                && keeper.parent_node_id > 0
                && !keeper.parent_identity_kind.is_empty()
                && keeper.parent_identity_kind.len() <= 128
                && keeper.parent_identity_kind != "path_fallback"
                && !keeper.parent_identity_key.is_empty()
                && keeper.parent_identity_key.len() <= 4096
                && (plan.selection.source_kind != SmartCleanupSourceKind::ExactDuplicate
                    || (keeper.sha256 == plan.target_sha256
                        && keeper.hash_bytes == plan.target_hash_bytes))
        }
        _ => false,
    };
    if !keeper_valid
        || plan.target_location_id <= 0
        || plan.target_identity_kind.is_empty()
        || plan.target_identity_kind.len() > 128
        || plan.target_identity_kind == "path_fallback"
        || plan.target_identity_key.is_empty()
        || plan.target_identity_key.len() > 4096
        || plan.target_sha256.len() != 32
        || plan.target_hash_bytes != plan.target_size_bytes
        || plan.scope_root_node_id <= 0
        || plan.scope_root_identity_kind.is_empty()
        || plan.scope_root_identity_kind.len() > 128
        || plan.scope_root_identity_kind == "path_fallback"
        || plan.scope_root_identity_key.is_empty()
        || plan.scope_root_identity_key.len() > 4096
        || plan.parent_node_id <= 0
        || plan.parent_identity_kind.is_empty()
        || plan.parent_identity_kind.len() > 128
        || plan.parent_identity_kind == "path_fallback"
        || plan.parent_identity_key.is_empty()
        || plan.parent_identity_key.len() > 4096
    {
        return Err(DatabaseError::CleanupActionPlanInputInvalid);
    }
    Ok(())
}

fn validate_action_plan_write(plan: &ActionPlanWrite<'_>) -> Result<(), DatabaseError> {
    let paths_valid = !plan.source_path_raw.is_empty()
        && plan.source_path_raw.len() <= MAX_ACTION_PATH_BYTES
        && !plan.source_path_key.is_empty()
        && plan.source_path_key.len() <= MAX_ACTION_PATH_BYTES
        && !plan.source_display_path.is_empty()
        && plan.source_display_path.len() <= MAX_ACTION_PATH_BYTES
        && !plan.destination_path_raw.is_empty()
        && plan.destination_path_raw.len() <= MAX_ACTION_PATH_BYTES
        && !plan.destination_path_key.is_empty()
        && plan.destination_path_key.len() <= MAX_ACTION_PATH_BYTES
        && !plan.destination_display_path.is_empty()
        && plan.destination_display_path.len() <= MAX_ACTION_PATH_BYTES;
    if plan.scope_id <= 0
        || plan.node_id <= 0
        || plan.source_location_id <= 0
        || !paths_valid
        || plan.source_path_raw == plan.destination_path_raw
        || plan.source_display_path == plan.destination_display_path
        || plan.source_identity_kind.is_empty()
        || plan.source_identity_kind.len() > 128
        || plan.source_identity_key.is_empty()
        || plan.source_identity_key.len() > 4096
        || plan.source_sha256.len() != 32
        || plan.source_hash_bytes != plan.source_size_bytes
        || plan.scope_root_identity_kind.is_empty()
        || plan.scope_root_identity_kind.len() > 128
        || plan.scope_root_identity_kind == "path_fallback"
        || plan.scope_root_identity_key.is_empty()
        || plan.scope_root_identity_key.len() > 4096
        || plan.parent_identity_kind.is_empty()
        || plan.parent_identity_kind.len() > 128
        || plan.parent_identity_kind == "path_fallback"
        || plan.parent_identity_key.is_empty()
        || plan.parent_identity_key.len() > 4096
    {
        return Err(DatabaseError::ActionPlanInputInvalid);
    }
    Ok(())
}

fn validate_project_suggestion(
    scope_id: i64,
    root_folder_node_id: i64,
    suggestion: &ProjectSuggestion,
) -> Result<(), DatabaseError> {
    if scope_id <= 0
        || root_folder_node_id <= 0
        || suggestion.observed_at_unix_ms < 0
        || suggestion.created_by != ProjectSuggestionCreator::SystemRule
        || suggestion.provider_id != ProjectSuggestion::PROVIDER_ID
        || suggestion.provider_version != ProjectSuggestion::PROVIDER_VERSION
        || suggestion.model_version.is_some()
        || suggestion.provenance.is_empty()
        || suggestion.provenance.len() > 8
    {
        return Err(DatabaseError::ProjectCandidateInputInvalid);
    }
    let mut previous_kind = None;
    let mut strong_weights = Vec::new();
    for signal in &suggestion.provenance {
        let (expected_marker, expected_weight) = expected_project_signal(signal.kind);
        if signal.marker_name != expected_marker
            || signal.weight_basis_points != expected_weight
            || previous_kind.is_some_and(|previous| previous >= signal.kind)
        {
            return Err(DatabaseError::ProjectCandidateInputInvalid);
        }
        previous_kind = Some(signal.kind);
        if signal.kind != ProjectSignalKind::Readme {
            strong_weights.push(signal.weight_basis_points);
        }
    }
    let Some(maximum) = strong_weights.iter().copied().max() else {
        return Err(DatabaseError::ProjectCandidateInputInvalid);
    };
    let additional = u16::try_from(strong_weights.len().saturating_sub(1))
        .map_err(|_| DatabaseError::ProjectCandidateInputInvalid)?
        .saturating_mul(500);
    let expected_confidence = maximum.saturating_add(additional).min(9_500);
    if suggestion.confidence_basis_points != expected_confidence {
        return Err(DatabaseError::ProjectCandidateInputInvalid);
    }
    Ok(())
}

fn expected_project_signal(kind: ProjectSignalKind) -> (&'static str, u16) {
    match kind {
        ProjectSignalKind::CargoManifest => ("Cargo.toml", 8_500),
        ProjectSignalKind::JavaScriptPackage => ("package.json", 7_500),
        ProjectSignalKind::PythonProject => ("pyproject.toml", 8_000),
        ProjectSignalKind::GoModule => ("go.mod", 8_500),
        ProjectSignalKind::SwiftPackage => ("Package.swift", 8_500),
        ProjectSignalKind::XcodeProject => ("*.xcodeproj", 9_000),
        ProjectSignalKind::VisualStudioSolution => ("*.sln", 8_500),
        ProjectSignalKind::Readme => ("README", 1_500),
    }
}

fn project_signal_kind_str(kind: ProjectSignalKind) -> &'static str {
    match kind {
        ProjectSignalKind::CargoManifest => "cargo_manifest",
        ProjectSignalKind::JavaScriptPackage => "javascript_package",
        ProjectSignalKind::PythonProject => "python_project",
        ProjectSignalKind::GoModule => "go_module",
        ProjectSignalKind::SwiftPackage => "swift_package",
        ProjectSignalKind::XcodeProject => "xcode_project",
        ProjectSignalKind::VisualStudioSolution => "visual_studio_solution",
        ProjectSignalKind::Readme => "readme",
    }
}

fn project_signal_kind_from_str(stored: &str) -> Result<ProjectSignalKind, DatabaseError> {
    match stored {
        "cargo_manifest" => Ok(ProjectSignalKind::CargoManifest),
        "javascript_package" => Ok(ProjectSignalKind::JavaScriptPackage),
        "python_project" => Ok(ProjectSignalKind::PythonProject),
        "go_module" => Ok(ProjectSignalKind::GoModule),
        "swift_package" => Ok(ProjectSignalKind::SwiftPackage),
        "xcode_project" => Ok(ProjectSignalKind::XcodeProject),
        "visual_studio_solution" => Ok(ProjectSignalKind::VisualStudioSolution),
        "readme" => Ok(ProjectSignalKind::Readme),
        _ => Err(DatabaseError::InvalidStoredValue),
    }
}

fn project_decision_kind_str(decision: ProjectDecisionKind) -> &'static str {
    match decision {
        ProjectDecisionKind::Accepted => "accepted",
        ProjectDecisionKind::Rejected => "rejected",
    }
}

fn project_decision_kind_from_str(stored: &str) -> Result<ProjectDecisionKind, DatabaseError> {
    match stored {
        "accepted" => Ok(ProjectDecisionKind::Accepted),
        "rejected" => Ok(ProjectDecisionKind::Rejected),
        _ => Err(DatabaseError::InvalidStoredValue),
    }
}

fn file_relation_decision_kind_str(decision: FileRelationDecisionKind) -> &'static str {
    match decision {
        FileRelationDecisionKind::Accepted => "accepted",
        FileRelationDecisionKind::Rejected => "rejected",
    }
}

fn file_relation_decision_kind_from_str(
    stored: &str,
) -> Result<FileRelationDecisionKind, DatabaseError> {
    match stored {
        "accepted" => Ok(FileRelationDecisionKind::Accepted),
        "rejected" => Ok(FileRelationDecisionKind::Rejected),
        _ => Err(DatabaseError::InvalidStoredValue),
    }
}

fn file_category(path: &Path) -> FolderFileCategory {
    let extension = path
        .extension()
        .and_then(|extension| extension.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    match extension.as_str() {
        "txt" | "md" | "markdown" | "pdf" | "docx" | "pptx" | "xlsx" | "rtf" => {
            FolderFileCategory::Document
        }
        "rs" | "ts" | "tsx" | "js" | "jsx" | "py" | "go" | "java" | "kt" | "kts" | "swift"
        | "c" | "cc" | "cpp" | "h" | "hpp" | "cs" | "rb" | "php" | "sh" | "zsh" | "fish"
        | "toml" | "yaml" | "yml" | "json" | "xml" | "html" | "css" | "sql" => {
            FolderFileCategory::Code
        }
        "png" | "jpg" | "jpeg" | "gif" | "webp" | "heic" | "tiff" | "bmp" | "svg" => {
            FolderFileCategory::Image
        }
        "csv" | "tsv" | "parquet" | "sqlite" | "db" => FolderFileCategory::Data,
        "zip" | "tar" | "gz" | "tgz" | "7z" | "rar" => FolderFileCategory::Archive,
        "mp3" | "wav" | "m4a" | "flac" | "mp4" | "mov" | "mkv" | "avi" => FolderFileCategory::Media,
        _ => FolderFileCategory::Other,
    }
}

fn folder_category_index(category: FolderFileCategory) -> usize {
    match category {
        FolderFileCategory::Document => 0,
        FolderFileCategory::Code => 1,
        FolderFileCategory::Image => 2,
        FolderFileCategory::Data => 3,
        FolderFileCategory::Archive => 4,
        FolderFileCategory::Media => 5,
        FolderFileCategory::Other => 6,
    }
}

fn project_marker(path: &Path, kind: NodeKind) -> Option<ProjectSignalKind> {
    let name = path.file_name()?.to_string_lossy().to_ascii_lowercase();
    match (kind, name.as_str()) {
        (NodeKind::File, "cargo.toml") => Some(ProjectSignalKind::CargoManifest),
        (NodeKind::File, "package.json") => Some(ProjectSignalKind::JavaScriptPackage),
        (NodeKind::File, "pyproject.toml") => Some(ProjectSignalKind::PythonProject),
        (NodeKind::File, "go.mod") => Some(ProjectSignalKind::GoModule),
        (NodeKind::File, "package.swift") => Some(ProjectSignalKind::SwiftPackage),
        (NodeKind::File, name) if name.ends_with(".sln") => {
            Some(ProjectSignalKind::VisualStudioSolution)
        }
        (NodeKind::File, "readme" | "readme.md" | "readme.txt" | "readme.rst") => {
            Some(ProjectSignalKind::Readme)
        }
        (NodeKind::Folder, name) if name.ends_with(".xcodeproj") => {
            Some(ProjectSignalKind::XcodeProject)
        }
        _ => None,
    }
}

fn action_execution_strategy_str(strategy: ActionExecutionStrategy) -> &'static str {
    match strategy {
        ActionExecutionStrategy::Direct => "direct",
        ActionExecutionStrategy::CaseOnlyStaged => "case_only_staged",
    }
}

fn action_execution_strategy_from_str(
    stored: &str,
) -> Result<ActionExecutionStrategy, rusqlite::Error> {
    match stored {
        "direct" => Ok(ActionExecutionStrategy::Direct),
        "case_only_staged" => Ok(ActionExecutionStrategy::CaseOnlyStaged),
        _ => Err(rusqlite::Error::InvalidQuery),
    }
}

fn action_operation_from_str(stored: &str) -> Result<ActionOperation, rusqlite::Error> {
    match stored {
        "rename" => Ok(ActionOperation::Rename),
        _ => Err(rusqlite::Error::InvalidQuery),
    }
}

fn action_command_kind_str(kind: ActionCommandKind) -> &'static str {
    match kind {
        ActionCommandKind::Execute => "execute",
        ActionCommandKind::Undo => "undo",
    }
}

fn action_command_kind_from_str(stored: &str) -> Result<ActionCommandKind, DatabaseError> {
    match stored {
        "execute" => Ok(ActionCommandKind::Execute),
        "undo" => Ok(ActionCommandKind::Undo),
        _ => Err(DatabaseError::InvalidStoredValue),
    }
}

fn action_journal_event_kind_str(kind: ActionJournalEventKind) -> &'static str {
    match kind {
        ActionJournalEventKind::PreviewCreated => "preview_created",
        ActionJournalEventKind::ExecuteRequested => "execute_requested",
        ActionJournalEventKind::ExecuteRequestNotStarted => "execute_request_not_started",
        ActionJournalEventKind::DirectRenameIntent => "direct_rename_intent",
        ActionJournalEventKind::ExecutionCompleted => "execution_completed",
        ActionJournalEventKind::ExecutionNotApplied => "execution_not_applied",
        ActionJournalEventKind::ExecutionNeedsAttention => "execution_needs_attention",
        ActionJournalEventKind::UndoRequested => "undo_requested",
        ActionJournalEventKind::UndoRequestNotStarted => "undo_request_not_started",
        ActionJournalEventKind::UndoRenameIntent => "undo_rename_intent",
        ActionJournalEventKind::UndoCompleted => "undo_completed",
        ActionJournalEventKind::UndoNotApplied => "undo_not_applied",
        ActionJournalEventKind::UndoNeedsAttention => "undo_needs_attention",
    }
}

fn action_journal_event_kind_from_str(
    stored: &str,
) -> Result<ActionJournalEventKind, rusqlite::Error> {
    match stored {
        "preview_created" => Ok(ActionJournalEventKind::PreviewCreated),
        "execute_requested" => Ok(ActionJournalEventKind::ExecuteRequested),
        "execute_request_not_started" => Ok(ActionJournalEventKind::ExecuteRequestNotStarted),
        "direct_rename_intent" => Ok(ActionJournalEventKind::DirectRenameIntent),
        "execution_completed" => Ok(ActionJournalEventKind::ExecutionCompleted),
        "execution_not_applied" => Ok(ActionJournalEventKind::ExecutionNotApplied),
        "execution_needs_attention" => Ok(ActionJournalEventKind::ExecutionNeedsAttention),
        "undo_requested" => Ok(ActionJournalEventKind::UndoRequested),
        "undo_request_not_started" => Ok(ActionJournalEventKind::UndoRequestNotStarted),
        "undo_rename_intent" => Ok(ActionJournalEventKind::UndoRenameIntent),
        "undo_completed" => Ok(ActionJournalEventKind::UndoCompleted),
        "undo_not_applied" => Ok(ActionJournalEventKind::UndoNotApplied),
        "undo_needs_attention" => Ok(ActionJournalEventKind::UndoNeedsAttention),
        _ => Err(rusqlite::Error::InvalidQuery),
    }
}

fn internal_event_matches_command(
    event: ActionJournalEventKind,
    command: ActionCommandKind,
) -> bool {
    matches!(
        (event, command),
        (
            ActionJournalEventKind::DirectRenameIntent
                | ActionJournalEventKind::ExecuteRequestNotStarted
                | ActionJournalEventKind::ExecutionCompleted
                | ActionJournalEventKind::ExecutionNotApplied
                | ActionJournalEventKind::ExecutionNeedsAttention,
            ActionCommandKind::Execute
        ) | (
            ActionJournalEventKind::UndoRenameIntent
                | ActionJournalEventKind::UndoRequestNotStarted
                | ActionJournalEventKind::UndoCompleted
                | ActionJournalEventKind::UndoNotApplied
                | ActionJournalEventKind::UndoNeedsAttention,
            ActionCommandKind::Undo
        )
    )
}

fn transition_is_valid(state: ActionPlanState, event: ActionJournalEventKind) -> bool {
    matches!(
        (state, event),
        (
            ActionPlanState::ExecuteRequested,
            ActionJournalEventKind::DirectRenameIntent
        ) | (
            ActionPlanState::ExecuteRequested,
            ActionJournalEventKind::ExecuteRequestNotStarted
        ) | (
            ActionPlanState::DirectRenameIntent,
            ActionJournalEventKind::ExecutionCompleted
        ) | (
            ActionPlanState::DirectRenameIntent,
            ActionJournalEventKind::ExecutionNotApplied
        ) | (
            ActionPlanState::DirectRenameIntent,
            ActionJournalEventKind::ExecutionNeedsAttention
        ) | (
            ActionPlanState::UndoRequested,
            ActionJournalEventKind::UndoRenameIntent
        ) | (
            ActionPlanState::UndoRequested,
            ActionJournalEventKind::UndoRequestNotStarted
        ) | (
            ActionPlanState::UndoRenameIntent,
            ActionJournalEventKind::UndoCompleted
        ) | (
            ActionPlanState::UndoRenameIntent,
            ActionJournalEventKind::UndoNotApplied
        ) | (
            ActionPlanState::UndoRenameIntent,
            ActionJournalEventKind::UndoNeedsAttention
        )
    )
}

fn validate_action_command_write(command: &ActionCommandWrite<'_>) -> Result<(), DatabaseError> {
    if command.plan_id <= 0
        || command.expected_sequence == 0
        || command.request_id.len() < 8
        || command.request_id.len() > 128
        || !command
            .request_id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
    {
        return Err(DatabaseError::ActionJournalInputInvalid);
    }
    Ok(())
}

fn validate_action_journal_append(append: &ActionJournalAppend<'_>) -> Result<(), DatabaseError> {
    if append.plan_id <= 0
        || append.command_request_id <= 0
        || append.expected_sequence == 0
        || matches!(
            append.kind,
            ActionJournalEventKind::PreviewCreated
                | ActionJournalEventKind::ExecuteRequested
                | ActionJournalEventKind::UndoRequested
        )
    {
        return Err(DatabaseError::ActionJournalInputInvalid);
    }
    validate_action_executor_owner(append.plan_id, append.executor_lease_owner_token)
        .map_err(|_| DatabaseError::ActionJournalInputInvalid)
}

fn validate_action_executor_lease_input(
    plan_id: i64,
    owner_token: &str,
    lease_duration_ms: i64,
) -> Result<(), DatabaseError> {
    validate_action_executor_owner(plan_id, owner_token)?;
    if !(MIN_ACTION_EXECUTOR_LEASE_MS..=MAX_ACTION_EXECUTOR_LEASE_MS).contains(&lease_duration_ms) {
        return Err(DatabaseError::ActionJournalInputInvalid);
    }
    Ok(())
}

fn validate_action_executor_owner(plan_id: i64, owner_token: &str) -> Result<(), DatabaseError> {
    if plan_id <= 0
        || owner_token.len() < 16
        || owner_token.len() > 128
        || !owner_token
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
    {
        return Err(DatabaseError::ActionJournalInputInvalid);
    }
    Ok(())
}

fn watch_event_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<WatchEventRecord> {
    let stored_status: String = row.get(2)?;
    let status = watch_status_from_str(&stored_status)?;
    let stored_reason: Option<String> = row.get(6)?;
    let reason = stored_reason
        .as_deref()
        .map(watch_reason_from_str)
        .transpose()
        .map_err(|_| rusqlite::Error::InvalidQuery)?;
    let stored_kind: String = row.get(9)?;
    let kind =
        WatchSnapshotKind::from_db(&stored_kind).map_err(|_| rusqlite::Error::InvalidQuery)?;
    let stored_size: Option<i64> = row.get(10)?;
    let size_bytes = stored_size
        .map(|value| {
            u64::try_from(value).map_err(|_| rusqlite::Error::IntegralValueOutOfRange(10, value))
        })
        .transpose()?;
    let reconciliation_kind = WatchReconciliationKind::from_db(&row.get::<_, String>(13)?)
        .map_err(|_| rusqlite::Error::InvalidQuery)?;
    Ok(WatchEventRecord {
        progress: WatchEventProgress {
            api_version: WatchEventProgress::API_VERSION,
            event_id: row.get(0)?,
            scope_id: row.get(1)?,
            status,
            observation_count: row_u64(row, 3)?,
            stable_after_unix_ms: row.get(4)?,
            scan_job_id: row.get(5)?,
            reason,
        },
        reconciliation_kind,
        path_raw: row.get(7)?,
        path_key: row.get(8)?,
        snapshot: WatchSnapshot {
            kind,
            size_bytes,
            modified_unix_ns: row.get(11)?,
            identity_key: row.get(12)?,
        },
    })
}

fn watch_event_progress_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<WatchEventProgress> {
    let status = row.get::<_, String>(2)?;
    let status = watch_status_from_str(&status)?;
    let reason = row
        .get::<_, Option<String>>(6)?
        .as_deref()
        .map(watch_reason_from_str)
        .transpose()
        .map_err(|_| rusqlite::Error::InvalidQuery)?;
    Ok(WatchEventProgress {
        api_version: WatchEventProgress::API_VERSION,
        event_id: row.get(0)?,
        scope_id: row.get(1)?,
        status,
        observation_count: row_u64(row, 3)?,
        stable_after_unix_ms: row.get(4)?,
        scan_job_id: row.get(5)?,
        reason,
    })
}

fn watch_status_from_str(value: &str) -> rusqlite::Result<WatchEventStatus> {
    match value {
        "stabilizing" => Ok(WatchEventStatus::Stabilizing),
        "reconciling" => Ok(WatchEventStatus::Reconciling),
        "completed" => Ok(WatchEventStatus::Completed),
        "ignored" => Ok(WatchEventStatus::Ignored),
        "failed" => Ok(WatchEventStatus::Failed),
        _ => Err(rusqlite::Error::InvalidQuery),
    }
}

fn scan_job_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<ScanJobProgress> {
    let stored_status: String = row.get(2)?;
    let control_state: String = row.get(3)?;
    let status = match (stored_status.as_str(), control_state.as_str()) {
        ("running", "paused") => ScanStatus::Paused,
        ("running", "ready" | "active" | "pause_requested") => ScanStatus::Running,
        ("completed", _) => ScanStatus::Completed,
        ("failed", _) => ScanStatus::Failed,
        ("interrupted", _) => ScanStatus::Interrupted,
        _ => return Err(rusqlite::Error::InvalidQuery),
    };
    Ok(ScanJobProgress {
        api_version: ScanJobProgress::API_VERSION,
        job_id: row.get(0)?,
        scope_id: row.get(1)?,
        status,
        queued_entries: row_u64(row, 4)?,
        processed_entries: row_u64(row, 5)?,
        discovered_files: row_u64(row, 6)?,
        discovered_folders: row_u64(row, 7)?,
        skipped_entries: row_u64(row, 8)?,
        issue_count: row_u64(row, 9)?,
        elapsed_ms: row_u64(row, 10)?,
        pause_requested: row.get::<_, i64>(11)? != 0,
    })
}

fn validate_watch_observation(
    observation: &WatchObservationWrite<'_>,
) -> Result<(), DatabaseError> {
    if observation.scope_id <= 0
        || observation.path_raw.is_empty()
        || observation.path_raw.len() > MAX_WATCH_PATH_BYTES
        || observation.path_key.is_empty()
        || observation.path_key.len() > MAX_WATCH_PATH_BYTES
        || observation.observed_at_unix_ms < 0
        || observation.stable_after_unix_ms < observation.observed_at_unix_ms
        || observation
            .snapshot
            .identity_key
            .as_ref()
            .is_some_and(|identity| identity.is_empty() || identity.len() > 4096)
    {
        return Err(DatabaseError::WatchInputInvalid);
    }
    let snapshot_valid = match observation.snapshot.kind {
        WatchSnapshotKind::Missing => {
            observation.snapshot.size_bytes.is_none()
                && observation.snapshot.modified_unix_ns.is_none()
                && observation.snapshot.identity_key.is_none()
        }
        WatchSnapshotKind::File => {
            observation.snapshot.size_bytes.is_some() && observation.snapshot.identity_key.is_some()
        }
        WatchSnapshotKind::Folder => {
            observation.snapshot.size_bytes.is_none() && observation.snapshot.identity_key.is_some()
        }
    };
    if snapshot_valid {
        Ok(())
    } else {
        Err(DatabaseError::WatchInputInvalid)
    }
}

fn validate_watch_file_delta_write(
    binding: &WatchFileDeltaBinding,
    write: &WatchFileDeltaWrite,
    published_at_unix_ms: i64,
) -> Result<(), DatabaseError> {
    if published_at_unix_ms < binding.stable_after_unix_ms
        || binding.path_raw.is_empty()
        || binding.path_raw.len() > MAX_WATCH_PATH_BYTES
        || binding.path_key.is_empty()
        || binding.path_key.len() > MAX_WATCH_PATH_BYTES
        || binding.identity_kind != "unix_device_inode"
        || binding.identity_key.is_empty()
        || binding.root_identity_key.is_empty()
        || binding.parent_path_raw.is_empty()
        || binding.parent_path_raw.len() > MAX_WATCH_PATH_BYTES
        || binding.parent_path_key.is_empty()
        || binding.parent_path_key.len() > MAX_WATCH_PATH_BYTES
        || binding.parent_identity_key.is_empty()
        || binding.old_size_bytes > i64::MAX as u64
        || write.snapshot.kind != WatchSnapshotKind::File
        || write.snapshot.size_bytes.is_none()
        || write.snapshot.identity_key.is_none()
        || write.snapshot != binding.snapshot
        || write.snapshot.identity_key.as_deref() != Some(binding.identity_key.as_slice())
    {
        return Err(DatabaseError::WatchFileDeltaNotEligible);
    }
    Ok(())
}

fn insert_watch_event(
    transaction: &Transaction<'_>,
    observation: WatchObservationWrite<'_>,
    status: &str,
    reason: Option<&str>,
    size_bytes: Option<i64>,
    reconciliation_kind: WatchReconciliationKind,
) -> Result<i64, DatabaseError> {
    let policy_revision =
        current_scope_policy_revision_from_connection(transaction, observation.scope_id)?;
    transaction.execute(
        "INSERT INTO watch_events( \
            scope_id, status, path_raw, path_key, observed_kind, observed_size_bytes, \
            observed_modified_unix_ns, observed_identity_key, observation_count, \
            stable_after_unix_ms, reason, reconciliation_kind, created_at_unix_ms, updated_at_unix_ms, \
            policy_revision \
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, 1, ?9, ?10, ?11, ?12, ?12, ?13)",
        params![
            observation.scope_id,
            status,
            observation.path_raw,
            observation.path_key,
            observation.snapshot.kind.as_str(),
            size_bytes,
            observation.snapshot.modified_unix_ns,
            observation.snapshot.identity_key,
            observation.stable_after_unix_ms,
            reason,
            reconciliation_kind.as_str(),
            observation.observed_at_unix_ms,
            policy_revision,
        ],
    )?;
    Ok(transaction.last_insert_rowid())
}

fn watchable_scope_ids_in_transaction(
    transaction: &Transaction<'_>,
) -> Result<Vec<i64>, DatabaseError> {
    let mut statement = transaction.prepare(
        "SELECT authorized_scopes.id \
         FROM authorized_scopes \
         WHERE EXISTS ( \
            SELECT 1 FROM scan_jobs \
            WHERE scan_jobs.scope_id = authorized_scopes.id \
                AND scan_jobs.status = 'completed' \
         ) \
         ORDER BY authorized_scopes.id ASC",
    )?;
    let scope_ids = statement.query_map([], |row| row.get(0))?;
    scope_ids.collect::<Result<Vec<_>, _>>().map_err(Into::into)
}

fn active_granted_watchable_scope_ids_in_transaction(
    transaction: &Transaction<'_>,
) -> Result<Vec<i64>, DatabaseError> {
    let mut statement = transaction.prepare(
        "SELECT authorized_scopes.id \
         FROM authorized_scopes \
         JOIN scope_access_grants grant \
           ON grant.scope_id = authorized_scopes.id \
          AND grant.platform = authorized_scopes.platform \
          AND grant.state = 'active' \
         WHERE authorized_scopes.platform = ?1 AND grant.platform = ?1 \
           AND EXISTS ( \
            SELECT 1 FROM scan_jobs \
            WHERE scan_jobs.scope_id = authorized_scopes.id \
                AND scan_jobs.status = 'completed' \
         ) \
         ORDER BY authorized_scopes.id ASC",
    )?;
    let scope_ids = statement.query_map([std::env::consts::OS], |row| row.get(0))?;
    scope_ids.collect::<Result<Vec<_>, _>>().map_err(Into::into)
}

fn request_scope_full_reconciliation_in_transaction(
    transaction: &Transaction<'_>,
    scope_id: i64,
    now_unix_ms: i64,
) -> Result<i64, DatabaseError> {
    let completed_scan_exists: i64 = transaction.query_row(
        "SELECT EXISTS( \
            SELECT 1 FROM scan_jobs job \
            WHERE job.scope_id = ?1 AND job.status = 'completed' \
         )",
        [scope_id],
        |row| row.get(0),
    )?;
    if completed_scan_exists != 1 {
        return Err(DatabaseError::WatchScopeInitialScanRequired);
    }

    let existing = transaction
        .query_row(
            "SELECT id FROM watch_events \
             WHERE scope_id = ?1 AND status = 'stabilizing'",
            [scope_id],
            |row| row.get::<_, i64>(0),
        )
        .optional()?;
    if let Some(event_id) = existing {
        let changed = transaction.execute(
            "UPDATE watch_events SET reconciliation_kind = 'full_scope', \
                observation_count = observation_count + 1, stable_after_unix_ms = ?2, \
                updated_at_unix_ms = ?2 \
             WHERE id = ?1 AND status = 'stabilizing'",
            params![event_id, now_unix_ms],
        )?;
        if changed != 1 {
            return Err(DatabaseError::InvalidWatchEventState);
        }
        return Ok(event_id);
    }

    let root = transaction
        .query_row(
            "SELECT scopes.path_raw, scopes.path_key, nodes.identity_key \
             FROM authorized_scopes scopes \
             JOIN locations ON locations.scope_id = scopes.id \
                AND locations.path_raw = scopes.path_raw \
                AND locations.path_key = scopes.path_key \
                AND locations.present = 1 \
             JOIN nodes ON nodes.id = locations.node_id AND nodes.kind = 'folder' \
             WHERE scopes.id = ?1",
            [scope_id],
            |row| {
                Ok((
                    row.get::<_, Vec<u8>>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, Vec<u8>>(2)?,
                ))
            },
        )
        .optional()?
        .ok_or(DatabaseError::WatchFileDeltaNotEligible)?;
    let snapshot = WatchSnapshot {
        kind: WatchSnapshotKind::Folder,
        size_bytes: None,
        modified_unix_ns: None,
        identity_key: Some(root.2),
    };
    insert_watch_event(
        transaction,
        WatchObservationWrite {
            scope_id,
            path_raw: &root.0,
            path_key: &root.1,
            snapshot: &snapshot,
            stable_after_unix_ms: now_unix_ms,
            ignored_reason: None,
            reconciliation_kind: WatchReconciliationKind::FullScope,
            observed_at_unix_ms: now_unix_ms,
        },
        "stabilizing",
        None,
        None,
        WatchReconciliationKind::FullScope,
    )
}

fn insert_resumable_scan_job(
    transaction: &Transaction<'_>,
    binding: ScopeRevisionBinding,
    root: &QueuedPath,
    now: i64,
) -> Result<i64, DatabaseError> {
    let active_jobs: i64 = transaction.query_row(
        "SELECT COUNT(*) FROM scan_jobs WHERE scope_id = ?1 AND status IN ('running', 'interrupted')",
        [binding.scope_id],
        |row| row.get(0),
    )?;
    if active_jobs != 0 {
        return Err(DatabaseError::ScanJobAlreadyActive);
    }
    transaction.execute(
        "INSERT INTO scan_jobs( \
            scope_id, status, control_state, queued_entries, processed_entries, \
            started_at_unix_ms, updated_at_unix_ms, policy_revision \
         ) VALUES (?1, 'running', 'ready', 1, 0, ?2, ?2, ?3)",
        params![binding.scope_id, now, binding.revision],
    )?;
    let job_id = transaction.last_insert_rowid();
    transaction.execute(
        "INSERT INTO scan_queue( \
            scan_id, path_raw, path_key, parent_identity_key, is_root, state \
         ) VALUES (?1, ?2, ?3, ?4, ?5, 'pending')",
        params![
            job_id,
            root.path_raw,
            root.path_key,
            root.parent_identity_key,
            i64::from(root.is_root),
        ],
    )?;
    Ok(job_id)
}

fn watch_reason_as_str(reason: WatchEventReason) -> &'static str {
    match reason {
        WatchEventReason::TemporaryDownload => "temporary_download",
        WatchEventReason::HiddenEntry => "hidden_entry",
        WatchEventReason::UnsupportedEntry => "unsupported_entry",
        WatchEventReason::SourceUnavailable => "source_unavailable",
        WatchEventReason::ReconcileFailed => "reconcile_failed",
    }
}

fn watch_reason_from_str(value: &str) -> Result<WatchEventReason, DatabaseError> {
    match value {
        "temporary_download" => Ok(WatchEventReason::TemporaryDownload),
        "hidden_entry" => Ok(WatchEventReason::HiddenEntry),
        "unsupported_entry" => Ok(WatchEventReason::UnsupportedEntry),
        "source_unavailable" => Ok(WatchEventReason::SourceUnavailable),
        "reconcile_failed" => Ok(WatchEventReason::ReconcileFailed),
        _ => Err(DatabaseError::InvalidStoredValue),
    }
}

fn extraction_job_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<ExtractionJobProgress> {
    let stored_operation: String = row.get(3)?;
    let operation = ExtractionOperation::from_storage(&stored_operation)
        .ok_or(rusqlite::Error::InvalidQuery)?;
    let stored_status: String = row.get(4)?;
    let status = match stored_status.as_str() {
        "queued" => ExtractionStatus::Queued,
        "running" => ExtractionStatus::Running,
        "completed" => ExtractionStatus::Completed,
        "failed" => ExtractionStatus::Failed,
        "cancelled" => ExtractionStatus::Cancelled,
        "interrupted" => ExtractionStatus::Interrupted,
        _ => return Err(rusqlite::Error::InvalidQuery),
    };
    Ok(ExtractionJobProgress {
        api_version: ExtractionJobProgress::API_VERSION,
        job_id: row.get(0)?,
        scope_id: row.get(1)?,
        node_id: row.get(2)?,
        operation,
        status,
        provider_id: row.get(5)?,
        provider_version: row.get(6)?,
        error_code: row.get(7)?,
        source_bytes: row_u64(row, 8)?,
        output_bytes: row_u64(row, 9)?,
        chunk_count: row_u64(row, 10)?,
        elapsed_ms: row_u64(row, 11)?,
        cancel_requested: row.get::<_, i64>(12)? != 0,
    })
}

fn row_u64(row: &rusqlite::Row<'_>, index: usize) -> rusqlite::Result<u64> {
    let value: i64 = row.get(index)?;
    u64::try_from(value).map_err(|_| rusqlite::Error::IntegralValueOutOfRange(index, value))
}

fn row_u64_value(value: i64) -> Result<u64, DatabaseError> {
    u64::try_from(value).map_err(|_| DatabaseError::InvalidStoredValue)
}

fn ensure_active_runner(
    transaction: &Transaction<'_>,
    job_id: i64,
    runner_token: &str,
    now: i64,
) -> Result<(), DatabaseError> {
    let owned: i64 = transaction.query_row(
        "SELECT COUNT(*) FROM scan_jobs WHERE id = ?1 AND status = 'running' \
            AND control_state = 'active' AND runner_token = ?2 \
            AND lease_expires_at_unix_ms IS NOT NULL AND lease_expires_at_unix_ms > ?3",
        params![job_id, runner_token, now],
        |row| row.get(0),
    )?;
    if owned == 1 {
        Ok(())
    } else {
        Err(DatabaseError::RunnerLeaseLost)
    }
}

fn ensure_owned_runner(
    transaction: &Transaction<'_>,
    job_id: i64,
    runner_token: &str,
    now: i64,
) -> Result<(), DatabaseError> {
    let owned: i64 = transaction.query_row(
        "SELECT COUNT(*) FROM scan_jobs WHERE id = ?1 AND status = 'running' \
            AND control_state IN ('active', 'pause_requested') AND runner_token = ?2 \
            AND lease_expires_at_unix_ms IS NOT NULL AND lease_expires_at_unix_ms > ?3",
        params![job_id, runner_token, now],
        |row| row.get(0),
    )?;
    if owned == 1 {
        Ok(())
    } else {
        Err(DatabaseError::RunnerLeaseLost)
    }
}

fn ensure_extraction_runner(
    transaction: &Transaction<'_>,
    job_id: i64,
    runner_token: &str,
    now: i64,
) -> Result<(), DatabaseError> {
    let owned: i64 = transaction.query_row(
        "SELECT COUNT(*) FROM extraction_jobs WHERE id = ?1 AND status = 'running' \
            AND runner_token = ?2 AND lease_expires_at_unix_ms IS NOT NULL \
            AND lease_expires_at_unix_ms > ?3",
        params![job_id, runner_token, now],
        |row| row.get(0),
    )?;
    if owned == 1 {
        Ok(())
    } else {
        Err(DatabaseError::ExtractionRunnerLeaseLost)
    }
}

fn invalidate_stale_extraction_outputs(
    transaction: &Transaction<'_>,
    scope_id: i64,
) -> Result<(), DatabaseError> {
    transaction.execute(
        "UPDATE content_chunks SET active = 0 \
         WHERE active = 1 AND scope_id = ?1 AND ( \
            NOT EXISTS ( \
                SELECT 1 FROM locations l \
                WHERE l.id = content_chunks.location_id AND l.node_id = content_chunks.node_id \
                    AND l.present = 1 \
            ) OR NOT EXISTS ( \
                SELECT 1 FROM files f \
                WHERE f.node_id = content_chunks.node_id \
                  AND f.size_bytes = content_chunks.source_size_bytes \
                  AND f.modified_unix_ns IS content_chunks.source_modified_unix_ns \
            ) \
         )",
        [scope_id],
    )?;
    transaction.execute(
        "UPDATE image_metadata SET active = 0 \
         WHERE active = 1 AND scope_id = ?1 AND ( \
            NOT EXISTS ( \
                SELECT 1 FROM locations l \
                WHERE l.id = image_metadata.location_id AND l.node_id = image_metadata.node_id \
                    AND l.present = 1 \
            ) OR NOT EXISTS ( \
                SELECT 1 FROM files f \
                WHERE f.node_id = image_metadata.node_id \
                  AND f.size_bytes = image_metadata.source_size_bytes \
                  AND f.modified_unix_ns IS image_metadata.source_modified_unix_ns \
            ) \
         )",
        [scope_id],
    )?;
    Ok(())
}

#[cfg(any(target_os = "macos", target_os = "linux"))]
fn validate_existing_database_path(path: &Path) -> Result<Vec<u8>, DatabaseError> {
    if !path.is_absolute()
        || path.as_os_str().is_empty()
        || path
            .components()
            .any(|component| matches!(component, Component::CurDir | Component::ParentDir))
    {
        return Err(DatabaseError::ReadOnlyPathInvalid);
    }

    let mut database_metadata = None;
    for (index, candidate) in path.ancestors().enumerate() {
        let metadata =
            fs::symlink_metadata(candidate).map_err(|_| DatabaseError::ReadOnlyPathInvalid)?;
        if is_symlink_or_reparse_point(&metadata)
            || (index == 0 && !metadata.is_file())
            || (index != 0 && !metadata.is_dir())
        {
            return Err(DatabaseError::ReadOnlyPathInvalid);
        }
        if index == 0 {
            database_metadata = Some(metadata);
        }
    }

    let metadata = database_metadata.ok_or(DatabaseError::ReadOnlyPathInvalid)?;
    platform_identity(path, &metadata, IdentityNodeKind::File)
        .map(|identity| identity.key)
        .map_err(|_| DatabaseError::ReadOnlyPathInvalid)
}

#[cfg(any(target_os = "macos", target_os = "linux"))]
fn validate_existing_sqlite_sidecars(path: &Path) -> Result<(), DatabaseError> {
    let file_name = path.file_name().ok_or(DatabaseError::ReadOnlyPathInvalid)?;
    for suffix in ["-wal", "-shm"] {
        let mut sidecar_name = file_name.to_os_string();
        sidecar_name.push(suffix);
        let sidecar = path.with_file_name(sidecar_name);
        match fs::symlink_metadata(&sidecar) {
            Ok(metadata) if is_symlink_or_reparse_point(&metadata) || !metadata.is_file() => {
                return Err(DatabaseError::ReadOnlyPathInvalid);
            }
            Ok(_) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(_) => return Err(DatabaseError::ReadOnlyPathInvalid),
        }
    }
    Ok(())
}

#[cfg(any(target_os = "macos", target_os = "linux"))]
fn validate_schema_migrations_exact(connection: &Connection) -> Result<(), DatabaseError> {
    let migration_table_count = connection.query_row(
        "SELECT COUNT(*) FROM sqlite_schema WHERE type = 'table' AND name = 'schema_migrations'",
        [],
        |row| row.get::<_, i64>(0),
    )?;
    let expected_count =
        i64::try_from(MIGRATIONS.len()).map_err(|_| DatabaseError::ReadOnlySchemaInvalid)?;
    let actual_count = if migration_table_count == 1 {
        connection.query_row("SELECT COUNT(*) FROM schema_migrations", [], |row| {
            row.get::<_, i64>(0)
        })?
    } else {
        return Err(DatabaseError::ReadOnlySchemaInvalid);
    };
    if actual_count != expected_count {
        return Err(DatabaseError::ReadOnlySchemaInvalid);
    }

    for migration in MIGRATIONS {
        let applied = connection
            .query_row(
                "SELECT name, checksum FROM schema_migrations WHERE version = ?1",
                [migration.version],
                |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
            )
            .optional()?;
        let Some((name, checksum)) = applied else {
            return Err(DatabaseError::ReadOnlySchemaInvalid);
        };
        if name != migration.name {
            return Err(DatabaseError::ReadOnlySchemaInvalid);
        }
        if checksum != migration_checksum(migration.sql) {
            return Err(DatabaseError::MigrationChanged {
                version: migration.version,
            });
        }
    }
    Ok(())
}

fn ensure_scope_queryable(connection: &Connection, scope_id: i64) -> Result<(), DatabaseError> {
    if scope_id <= 0 {
        return Err(DatabaseError::ScopeNotFound);
    }
    let exists = connection.query_row(
        "SELECT EXISTS(SELECT 1 FROM authorized_scopes WHERE id = ?1)",
        [scope_id],
        |row| row.get::<_, i64>(0).map(|value| value == 1),
    )?;
    if !exists {
        return Err(DatabaseError::ScopeNotFound);
    }
    let completed = connection.query_row(
        "SELECT EXISTS( \
            SELECT 1 FROM scan_jobs job \
            WHERE job.scope_id = ?1 AND job.status = 'completed' \
         )",
        [scope_id],
        |row| row.get::<_, i64>(0).map(|value| value == 1),
    )?;
    if !completed {
        return Err(DatabaseError::ScanJobIncomplete);
    }
    Ok(())
}

fn ensure_scope_access_permitted(
    connection: &Connection,
    scope_id: i64,
) -> Result<(), DatabaseError> {
    let state = connection
        .query_row(
            "SELECT grant.state \
             FROM authorized_scopes scope \
             LEFT JOIN scope_access_grants grant \
               ON grant.scope_id = scope.id AND grant.platform = scope.platform \
              AND scope.platform = ?2 AND grant.platform = ?2 \
             WHERE scope.id = ?1",
            params![scope_id, std::env::consts::OS],
            |row| row.get::<_, Option<String>>(0),
        )
        .optional()?
        .ok_or(DatabaseError::ScopeNotFound)?;
    match state.as_deref() {
        Some("active") => Ok(()),
        Some("needs_reauthorization" | "revoked") | None => {
            Err(DatabaseError::ScopeAccessGrantNotActive)
        }
        Some(_) => Err(DatabaseError::InvalidStoredValue),
    }
}

fn screenshot_group_sources_from_connection(
    connection: &Connection,
    scope_id: i64,
) -> Result<Vec<ScreenshotGroupSourceRecord>, DatabaseError> {
    let query_limit = i64::from(MAX_SCREENSHOT_GROUP_IMAGES) + 1;
    let mut statement = connection.prepare(
        "SELECT im.scope_id, im.node_id, im.location_id, im.id, ocr_job.id, \
                f.size_bytes, f.modified_unix_ns, im.format, im.pixel_width, im.pixel_height, \
                COUNT(chunk.id), MIN(chunk.provider_id), MIN(chunk.provider_version), \
                MAX(chunk.provider_id), MAX(chunk.provider_version) \
         FROM image_metadata im \
         JOIN authorized_scopes scope ON scope.id = im.scope_id \
         JOIN extraction_jobs image_job \
           ON image_job.id = im.extraction_job_id AND image_job.status = 'completed' \
         JOIN locations location \
           ON location.id = im.location_id AND location.scope_id = im.scope_id \
          AND location.node_id = im.node_id AND location.present = 1 \
         JOIN nodes node ON node.id = im.node_id AND node.kind = 'file' \
         JOIN files f ON f.node_id = im.node_id \
         JOIN extraction_jobs ocr_job \
           ON ocr_job.scope_id = im.scope_id AND ocr_job.node_id = im.node_id \
          AND ocr_job.location_id = im.location_id \
          AND ocr_job.operation = 'screenshot_ocr' AND ocr_job.status = 'completed' \
          AND ocr_job.source_size_bytes = f.size_bytes \
          AND ocr_job.source_modified_unix_ns IS f.modified_unix_ns \
         JOIN content_chunks chunk \
           ON chunk.extraction_job_id = ocr_job.id AND chunk.active = 1 \
          AND chunk.scope_id = im.scope_id AND chunk.node_id = im.node_id \
          AND chunk.location_id = im.location_id \
          AND chunk.provenance_kind = 'ocr_observation' \
          AND chunk.source_size_bytes = f.size_bytes \
          AND chunk.source_modified_unix_ns IS f.modified_unix_ns \
         WHERE im.scope_id = ?1 AND im.active = 1 \
           AND NOT EXISTS ( \
               SELECT 1 FROM scope_exclusions exclusion \
               WHERE exclusion.scope_id = location.scope_id \
                 AND (location.path_key = exclusion.path_key OR ( \
                     exclusion.kind = 'folder' \
                     AND length(location.path_key) > length(exclusion.path_key) \
                     AND substr(location.path_key, 1, length(exclusion.path_key)) = exclusion.path_key \
                     AND (substr(exclusion.path_key, -1, 1) = CASE WHEN scope.platform='windows' THEN char(92) ELSE '/' END \
                          OR substr(location.path_key, length(exclusion.path_key) + 1, 1) = CASE WHEN scope.platform='windows' THEN char(92) ELSE '/' END))) \
           ) \
           AND im.source_size_bytes = f.size_bytes \
           AND im.source_modified_unix_ns IS f.modified_unix_ns \
           AND im.format IN ('png', 'jpeg', 'webp') \
           AND ocr_job.id = ( \
               SELECT MAX(candidate.id) FROM extraction_jobs candidate \
               WHERE candidate.scope_id = im.scope_id \
                 AND candidate.node_id = im.node_id \
                 AND candidate.location_id = im.location_id \
                 AND candidate.operation = 'screenshot_ocr' \
                 AND candidate.status = 'completed' \
                 AND candidate.source_size_bytes = f.size_bytes \
                 AND candidate.source_modified_unix_ns IS f.modified_unix_ns \
                 AND EXISTS ( \
                     SELECT 1 FROM content_chunks active_chunk \
                     WHERE active_chunk.extraction_job_id = candidate.id \
                       AND active_chunk.active = 1 \
                 ) \
           ) \
           AND NOT EXISTS ( \
               SELECT 1 FROM content_chunks invalid_chunk \
               WHERE invalid_chunk.extraction_job_id = ocr_job.id \
                 AND invalid_chunk.active = 1 \
                 AND (invalid_chunk.scope_id != im.scope_id \
                   OR invalid_chunk.node_id != im.node_id \
                   OR invalid_chunk.location_id != im.location_id \
                   OR invalid_chunk.provenance_kind != 'ocr_observation' \
                   OR invalid_chunk.source_size_bytes != f.size_bytes \
                   OR invalid_chunk.source_modified_unix_ns IS NOT f.modified_unix_ns) \
           ) \
         GROUP BY im.scope_id, im.node_id, im.location_id, im.id, ocr_job.id, \
                  ocr_job.provider_id, ocr_job.provider_version, \
                  f.size_bytes, f.modified_unix_ns, im.format, im.pixel_width, im.pixel_height \
         HAVING COUNT(chunk.id) BETWEEN 1 AND ?2 \
            AND MIN(chunk.provider_id) = MAX(chunk.provider_id) \
            AND MIN(chunk.provider_version) = MAX(chunk.provider_version) \
            AND MIN(chunk.provider_id) = ocr_job.provider_id \
            AND MIN(chunk.provider_version) = ocr_job.provider_version \
         ORDER BY im.pixel_width, im.pixel_height, f.modified_unix_ns, im.node_id \
         LIMIT ?3",
    )?;
    let rows = statement.query_map(
        params![
            scope_id,
            i64::try_from(MAX_EXTRACTION_CHUNKS).map_err(|_| DatabaseError::InvalidCount)?,
            query_limit
        ],
        |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, i64>(1)?,
                row.get::<_, i64>(2)?,
                row.get::<_, i64>(3)?,
                row.get::<_, i64>(4)?,
                row.get::<_, i64>(5)?,
                row.get::<_, i64>(6)?,
                row.get::<_, String>(7)?,
                row.get::<_, i64>(8)?,
                row.get::<_, i64>(9)?,
                row.get::<_, i64>(10)?,
                row.get::<_, String>(11)?,
                row.get::<_, String>(12)?,
                row.get::<_, String>(13)?,
                row.get::<_, String>(14)?,
            ))
        },
    )?;
    let mut sources = Vec::new();
    for row in rows {
        let row = row?;
        if row.11 != row.13 || row.12 != row.14 {
            return Err(DatabaseError::InvalidStoredValue);
        }
        sources.push(ScreenshotGroupSourceRecord {
            scope_id: row.0,
            node_id: row.1,
            location_id: row.2,
            image_metadata_id: row.3,
            ocr_extraction_job_id: row.4,
            size_bytes: u64::try_from(row.5).map_err(|_| DatabaseError::InvalidStoredValue)?,
            modified_unix_ns: row.6,
            format: ImageFormat::from_storage(&row.7).ok_or(DatabaseError::InvalidStoredValue)?,
            pixel_width: u32::try_from(row.8).map_err(|_| DatabaseError::InvalidStoredValue)?,
            pixel_height: u32::try_from(row.9).map_err(|_| DatabaseError::InvalidStoredValue)?,
            ocr_chunk_count: u32::try_from(row.10)
                .map_err(|_| DatabaseError::InvalidStoredValue)?,
            ocr_provider_id: row.11,
            ocr_provider_version: row.12,
        });
    }
    if sources.len() > usize::try_from(MAX_SCREENSHOT_GROUP_IMAGES).unwrap_or(usize::MAX) {
        return Err(DatabaseError::ScreenshotGroupImageLimitExceeded);
    }
    Ok(sources)
}

fn group_screenshot_sources(
    mut sources: Vec<ScreenshotGroupSourceRecord>,
) -> Result<Vec<Vec<ScreenshotGroupSourceRecord>>, DatabaseError> {
    sources.sort_by_key(|source| {
        (
            source.pixel_width,
            source.pixel_height,
            source.modified_unix_ns,
            source.node_id,
        )
    });
    let mut node_ids = std::collections::BTreeSet::new();
    if sources
        .iter()
        .any(|source| !node_ids.insert(source.node_id))
    {
        return Err(DatabaseError::ScreenshotGroupCandidateInputInvalid);
    }
    let mut groups = Vec::new();
    let mut current = Vec::new();
    for source in sources {
        let belongs = current
            .first()
            .is_none_or(|anchor: &ScreenshotGroupSourceRecord| {
                source.pixel_width == anchor.pixel_width
                    && source.pixel_height == anchor.pixel_height
                    && source
                        .modified_unix_ns
                        .checked_sub(anchor.modified_unix_ns)
                        .is_some_and(|delta| (0..=SCREENSHOT_GROUP_TIME_WINDOW_NS).contains(&delta))
            });
        if !belongs {
            push_screenshot_group(&mut groups, std::mem::take(&mut current))?;
        }
        current.push(source);
    }
    push_screenshot_group(&mut groups, current)?;
    if groups.len() > MAX_SCREENSHOT_GROUPS {
        return Err(DatabaseError::ScreenshotGroupLimitExceeded);
    }
    Ok(groups)
}

fn push_screenshot_group(
    groups: &mut Vec<Vec<ScreenshotGroupSourceRecord>>,
    group: Vec<ScreenshotGroupSourceRecord>,
) -> Result<(), DatabaseError> {
    if group.len() > MAX_SCREENSHOT_GROUP_MEMBERS {
        return Err(DatabaseError::ScreenshotGroupMemberLimitExceeded);
    }
    if group.len() >= 2 {
        groups.push(group);
    }
    Ok(())
}

fn current_screenshot_group_for_membership(
    connection: &Connection,
    scope_id: i64,
    membership_key: &str,
) -> Result<Option<Vec<ScreenshotGroupSourceRecord>>, DatabaseError> {
    for group in group_screenshot_sources(screenshot_group_sources_from_connection(
        connection, scope_id,
    )?)? {
        if screenshot_group_membership_key(&group)? == membership_key {
            return Ok(Some(group));
        }
    }
    Ok(None)
}

fn screenshot_group_identity_from_connection(
    connection: &Connection,
    group_id: i64,
) -> Result<(i64, String), DatabaseError> {
    if group_id <= 0 {
        return Err(DatabaseError::ScreenshotGroupCandidateInputInvalid);
    }
    connection
        .query_row(
            "SELECT scope_id, membership_key FROM screenshot_group_candidates \
             WHERE id = ?1 AND api_version = ?2",
            params![group_id, ScreenshotGroupCandidate::API_VERSION],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()?
        .ok_or(DatabaseError::ScreenshotGroupCandidateNotFound)
}

fn parse_screenshot_group_observation(
    stored: (
        i64,
        String,
        i64,
        i64,
        i64,
        String,
        String,
        String,
        String,
        Option<String>,
    ),
) -> Result<ScreenshotGroupObservationRecord, DatabaseError> {
    if stored.0 <= 0
        || !(16..=16_384).contains(&stored.1.len())
        || !(2..=20).contains(&stored.2)
        || stored.3 != 6_000
        || stored.4 < 0
        || stored.5 != "same_dimensions_time_window_with_ocr"
        || stored.6 != "system_rule"
        || stored.7 != ScreenshotGroupEvidence::PROVIDER_ID
        || stored.8 != ScreenshotGroupEvidence::PROVIDER_VERSION
        || stored.9.is_some()
    {
        return Err(DatabaseError::InvalidStoredValue);
    }
    Ok(ScreenshotGroupObservationRecord {
        id: stored.0,
        evidence_key: stored.1,
        member_count: stored.2,
        confidence_basis_points: stored.3,
        observed_at_unix_ms: stored.4,
    })
}

fn latest_screenshot_group_observation_from_connection(
    connection: &Connection,
    group_id: i64,
) -> Result<ScreenshotGroupObservationRecord, DatabaseError> {
    let stored = connection.query_row(
        "SELECT id, evidence_key, member_count, confidence_basis_points, observed_at_unix_ms, \
                rule_kind, created_by, provider_id, provider_version, model_version \
         FROM screenshot_group_observations WHERE group_id = ?1 \
         ORDER BY observed_at_unix_ms DESC, id DESC LIMIT 1",
        [group_id],
        |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, i64>(2)?,
                row.get::<_, i64>(3)?,
                row.get::<_, i64>(4)?,
                row.get::<_, String>(5)?,
                row.get::<_, String>(6)?,
                row.get::<_, String>(7)?,
                row.get::<_, String>(8)?,
                row.get::<_, Option<String>>(9)?,
            ))
        },
    )?;
    parse_screenshot_group_observation(stored)
}

fn screenshot_group_observation_for_evidence(
    connection: &Connection,
    group_id: i64,
    evidence_key: &str,
) -> Result<Option<ScreenshotGroupObservationRecord>, DatabaseError> {
    let stored = connection
        .query_row(
            "SELECT id, evidence_key, member_count, confidence_basis_points, observed_at_unix_ms, \
                    rule_kind, created_by, provider_id, provider_version, model_version \
             FROM screenshot_group_observations \
             WHERE group_id = ?1 AND evidence_key = ?2",
            params![group_id, evidence_key],
            |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, i64>(2)?,
                    row.get::<_, i64>(3)?,
                    row.get::<_, i64>(4)?,
                    row.get::<_, String>(5)?,
                    row.get::<_, String>(6)?,
                    row.get::<_, String>(7)?,
                    row.get::<_, String>(8)?,
                    row.get::<_, Option<String>>(9)?,
                ))
            },
        )
        .optional()?;
    stored.map(parse_screenshot_group_observation).transpose()
}

fn screenshot_group_observation_sources_from_connection(
    connection: &Connection,
    scope_id: i64,
    observation_id: i64,
) -> Result<Vec<ScreenshotGroupSourceRecord>, DatabaseError> {
    if scope_id <= 0 || observation_id <= 0 {
        return Err(DatabaseError::InvalidStoredValue);
    }
    let mut statement = connection.prepare(
        "SELECT member.node_id, member.location_id, member.image_metadata_id, \
                member.ocr_extraction_job_id, member.source_size_bytes, \
                member.source_modified_unix_ns, member.format, member.pixel_width, \
                member.pixel_height, member.ocr_chunk_count, member.ocr_provider_id, \
                member.ocr_provider_version \
         FROM screenshot_group_members member \
         JOIN screenshot_group_observations observation \
           ON observation.id = member.observation_id \
         JOIN screenshot_group_candidates candidate \
           ON candidate.id = observation.group_id AND candidate.scope_id = ?1 \
         WHERE member.observation_id = ?2 ORDER BY member.ordinal",
    )?;
    let rows = statement.query_map(params![scope_id, observation_id], |row| {
        Ok((
            row.get::<_, i64>(0)?,
            row.get::<_, i64>(1)?,
            row.get::<_, i64>(2)?,
            row.get::<_, i64>(3)?,
            row.get::<_, i64>(4)?,
            row.get::<_, i64>(5)?,
            row.get::<_, String>(6)?,
            row.get::<_, i64>(7)?,
            row.get::<_, i64>(8)?,
            row.get::<_, i64>(9)?,
            row.get::<_, String>(10)?,
            row.get::<_, String>(11)?,
        ))
    })?;
    rows.map(|row| {
        let row = row?;
        Ok(ScreenshotGroupSourceRecord {
            scope_id,
            node_id: row.0,
            location_id: row.1,
            image_metadata_id: row.2,
            ocr_extraction_job_id: row.3,
            size_bytes: u64::try_from(row.4).map_err(|_| DatabaseError::InvalidStoredValue)?,
            modified_unix_ns: row.5,
            format: ImageFormat::from_storage(&row.6).ok_or(DatabaseError::InvalidStoredValue)?,
            pixel_width: u32::try_from(row.7).map_err(|_| DatabaseError::InvalidStoredValue)?,
            pixel_height: u32::try_from(row.8).map_err(|_| DatabaseError::InvalidStoredValue)?,
            ocr_chunk_count: u32::try_from(row.9).map_err(|_| DatabaseError::InvalidStoredValue)?,
            ocr_provider_id: row.10,
            ocr_provider_version: row.11,
        })
    })
    .collect()
}

fn validate_screenshot_group_observation(
    connection: &Connection,
    scope_id: i64,
    membership_key: &str,
    observation: &ScreenshotGroupObservationRecord,
) -> Result<Vec<ScreenshotGroupSourceRecord>, DatabaseError> {
    let sources =
        screenshot_group_observation_sources_from_connection(connection, scope_id, observation.id)?;
    if usize::try_from(observation.member_count).map_err(|_| DatabaseError::InvalidStoredValue)?
        != sources.len()
        || screenshot_group_membership_key(&sources)? != membership_key
        || screenshot_group_evidence_key(&sources)? != observation.evidence_key
    {
        return Err(DatabaseError::InvalidStoredValue);
    }
    Ok(sources)
}

fn search_folder_list_from_connection(
    connection: &Connection,
    scope_id: i64,
    limit: Option<u32>,
) -> Result<SearchFolderListResponse, DatabaseError> {
    let limit = limit.unwrap_or(DEFAULT_SEARCH_FOLDER_LIST_LIMIT);
    if scope_id <= 0 || limit == 0 || limit > MAX_SEARCH_FOLDER_LIST_LIMIT {
        return Err(DatabaseError::SearchInputInvalid);
    }
    ensure_scope_queryable(connection, scope_id)?;
    ensure_scope_access_permitted(connection, scope_id)?;

    let query_limit = i64::from(limit)
        .checked_add(1)
        .ok_or(DatabaseError::SearchInputInvalid)?;
    let mut statement = connection.prepare(
        "SELECT location.node_id, MIN(location.display_path) \
         FROM locations location \
         JOIN nodes node ON node.id = location.node_id AND node.kind = 'folder' \
         JOIN folders folder ON folder.node_id = node.id \
         JOIN authorized_scopes scope ON scope.id = location.scope_id \
         JOIN scope_access_grants grant \
           ON grant.scope_id = scope.id AND grant.platform = scope.platform \
          AND grant.state = 'active' \
         WHERE location.scope_id = ?1 AND location.present = 1 \
           AND scope.platform = ?3 AND grant.platform = ?3 \
           AND NOT EXISTS ( \
               SELECT 1 FROM scope_exclusions exclusion \
               WHERE exclusion.scope_id = location.scope_id \
                 AND (location.path_key = exclusion.path_key OR ( \
                     exclusion.kind = 'folder' \
                     AND length(location.path_key) > length(exclusion.path_key) \
                     AND substr(location.path_key, 1, length(exclusion.path_key)) = exclusion.path_key \
                     AND (substr(exclusion.path_key, -1, 1) = CASE WHEN scope.platform='windows' THEN char(92) ELSE '/' END \
                          OR substr(location.path_key, length(exclusion.path_key) + 1, 1) = CASE WHEN scope.platform='windows' THEN char(92) ELSE '/' END)) \
                     OR (node.identity_kind = exclusion.identity_kind \
                         AND node.identity_key = exclusion.identity_key)) \
           ) \
         GROUP BY location.node_id \
         ORDER BY lower(MIN(location.display_path)), location.node_id \
         LIMIT ?2",
    )?;
    let rows = statement.query_map(
        params![scope_id, query_limit, std::env::consts::OS],
        |row| {
            Ok(SearchFolderOption {
                scope_id,
                folder_node_id: row.get(0)?,
                display_path: row.get(1)?,
            })
        },
    )?;
    let mut folders = rows.collect::<Result<Vec<_>, _>>()?;
    let truncated =
        folders.len() > usize::try_from(limit).map_err(|_| DatabaseError::SearchInputInvalid)?;
    if truncated {
        folders.pop();
    }
    let folder_count = u64::try_from(folders.len()).map_err(|_| DatabaseError::InvalidCount)?;
    Ok(SearchFolderListResponse {
        api_version: SearchFolderListResponse::API_VERSION,
        scope_id,
        folder_count,
        folders,
        truncated,
    })
}

fn validate_lexical_search_folder_filter(
    connection: &Connection,
    scope_id: i64,
    folder_node_id: i64,
) -> Result<String, DatabaseError> {
    let mut statement = connection.prepare(
        "SELECT location.path_key \
             FROM nodes node \
             JOIN folders folder ON folder.node_id = node.id \
             JOIN locations location \
               ON location.node_id = node.id AND location.scope_id = ?1 \
              AND location.present = 1 \
             JOIN authorized_scopes scope ON scope.id = location.scope_id \
             JOIN scope_access_grants grant \
               ON grant.scope_id = scope.id AND grant.platform = scope.platform \
              AND grant.state = 'active' \
             WHERE node.id = ?2 AND node.kind = 'folder' \
               AND scope.platform = ?3 AND grant.platform = ?3 \
               AND NOT EXISTS ( \
                   SELECT 1 FROM scope_exclusions exclusion \
                   WHERE exclusion.scope_id = location.scope_id \
                     AND (location.path_key = exclusion.path_key OR ( \
                         exclusion.kind = 'folder' \
                         AND length(location.path_key) > length(exclusion.path_key) \
                         AND substr(location.path_key, 1, length(exclusion.path_key)) = exclusion.path_key \
                         AND (substr(exclusion.path_key, -1, 1) = CASE WHEN scope.platform='windows' THEN char(92) ELSE '/' END \
                              OR substr(location.path_key, length(exclusion.path_key) + 1, 1) = CASE WHEN scope.platform='windows' THEN char(92) ELSE '/' END)) \
                         OR (node.identity_kind = exclusion.identity_kind \
                             AND node.identity_key = exclusion.identity_key)) \
               ) \
             ORDER BY location.id \
             LIMIT 2",
    )?;
    let path_keys = statement
        .query_map(
            params![scope_id, folder_node_id, std::env::consts::OS],
            |row| row.get::<_, String>(0),
        )?
        .collect::<Result<Vec<_>, _>>()?;
    match path_keys.as_slice() {
        [path_key] => Ok(path_key.clone()),
        [] | [_, _, ..] => Err(DatabaseError::SearchFolderInvalid),
    }
}

fn lexical_search_candidates_from_connection(
    connection: &Connection,
    match_query: &str,
    filters: LexicalSearchFilters<'_>,
    per_source_candidate_limit: u32,
) -> Result<Vec<LexicalSearchCandidate>, DatabaseError> {
    if match_query.is_empty()
        || match_query.len() > MAX_SEARCH_MATCH_BYTES
        || per_source_candidate_limit == 0
        || per_source_candidate_limit > MAX_SEARCH_CANDIDATES_PER_SOURCE
        || filters.scope_id.is_some_and(|scope_id| scope_id <= 0)
        || filters
            .folder_node_id
            .is_some_and(|folder_node_id| folder_node_id <= 0)
        || (filters.folder_node_id.is_some() && filters.scope_id.is_none())
        || filters.extension.is_some_and(|extension| {
            extension.is_empty()
                || extension.len() > 16
                || !extension
                    .bytes()
                    .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit())
        })
        || filters
            .modified_since_unix_ns
            .is_some_and(|timestamp| timestamp < 0)
        || filters
            .modified_before_unix_ns
            .is_some_and(|timestamp| timestamp < 0)
        || matches!(
            (
                filters.modified_since_unix_ns,
                filters.modified_before_unix_ns
            ),
            (Some(since), Some(before)) if since >= before
        )
    {
        return Err(DatabaseError::SearchInputInvalid);
    }
    let selected_folder_path_key = if let (Some(scope_id), Some(folder_node_id)) =
        (filters.scope_id, filters.folder_node_id)
    {
        Some(validate_lexical_search_folder_filter(
            connection,
            scope_id,
            folder_node_id,
        )?)
    } else {
        None
    };
    let limit = i64::from(per_source_candidate_limit);
    let maximum_sources = if filters.source == LexicalSearchSource::All {
        2
    } else {
        1
    };
    let mut candidates = Vec::with_capacity(
        usize::try_from(per_source_candidate_limit)
            .map_err(|_| DatabaseError::SearchInputInvalid)?
            .saturating_mul(maximum_sources),
    );

    if filters.source != LexicalSearchSource::ExtractedText {
        // Keep the original FTS query completely free of recursive work when
        // there is no folder filter. SQLite may otherwise plan the CTE even
        // behind a NULL short-circuit and turn ordinary lexical search into a
        // graph traversal.
        let metadata_sql = if filters.folder_node_id.is_some() {
            "WITH RECURSIVE folder_tree(node_id) AS ( \
                 SELECT ?8 \
                 UNION \
                 SELECT edge.source_node_id \
                 FROM folder_tree parent \
                 CROSS JOIN edges edge ON edge.target_node_id = parent.node_id \
                 WHERE edge.scope_id = ?2 AND edge.kind = 'located_in' AND edge.active = 1 \
             ) \
             SELECT l.scope_id, s.policy_revision, l.node_id, l.id, l.path_key, l.display_path, \
                    n.identity_kind, n.identity_key \
             FROM folder_tree \
             JOIN locations l ON l.node_id = folder_tree.node_id \
             JOIN location_search_fts ON location_search_fts.rowid = l.id \
             JOIN nodes n ON n.id = l.node_id \
             JOIN authorized_scopes s ON s.id = l.scope_id \
             JOIN scope_access_grants g ON g.scope_id = s.id AND g.platform = s.platform AND g.state = 'active' \
             LEFT JOIN files f ON f.node_id = l.node_id \
             WHERE location_search_fts MATCH ?1 AND l.present = 1 \
               AND (?2 IS NULL OR l.scope_id = ?2) \
               AND (?3 IS NULL OR (f.node_id IS NOT NULL AND substr(lower(l.display_path), -(length(?3) + 1)) = '.' || ?3)) \
               AND (?4 IS NULL OR f.modified_unix_ns >= ?4) \
               AND (?5 IS NULL OR f.modified_unix_ns < ?5) \
               AND s.platform = ?7 AND g.platform = ?7 \
               AND (l.path_key = ?9 OR ( \
                   length(l.path_key) > length(?9) \
                   AND substr(l.path_key, 1, length(?9)) = ?9 \
                   AND (substr(?9, -1, 1) = CASE WHEN s.platform='windows' THEN char(92) ELSE '/' END \
                        OR substr(l.path_key, length(?9) + 1, 1) = CASE WHEN s.platform='windows' THEN char(92) ELSE '/' END))) \
               AND NOT EXISTS (SELECT 1 FROM scope_exclusions x WHERE x.scope_id=l.scope_id AND (l.path_key=x.path_key OR (x.kind='folder' AND length(l.path_key)>length(x.path_key) AND substr(l.path_key,1,length(x.path_key))=x.path_key AND (substr(x.path_key,-1,1)=CASE WHEN s.platform='windows' THEN char(92) ELSE '/' END OR substr(l.path_key,length(x.path_key)+1,1)=CASE WHEN s.platform='windows' THEN char(92) ELSE '/' END)) OR (n.identity_kind=x.identity_kind AND n.identity_key=x.identity_key))) \
             ORDER BY location_search_fts.rank, l.id \
             LIMIT ?6"
        } else {
            "SELECT l.scope_id, s.policy_revision, l.node_id, l.id, l.path_key, l.display_path, \
                    n.identity_kind, n.identity_key \
             FROM location_search_fts \
             JOIN locations l ON l.id = location_search_fts.rowid \
             JOIN nodes n ON n.id = l.node_id \
             JOIN authorized_scopes s ON s.id = l.scope_id \
             JOIN scope_access_grants g ON g.scope_id = s.id AND g.platform = s.platform AND g.state = 'active' \
             LEFT JOIN files f ON f.node_id = l.node_id \
             WHERE location_search_fts MATCH ?1 AND l.present = 1 \
               AND (?2 IS NULL OR l.scope_id = ?2) \
               AND (?3 IS NULL OR (f.node_id IS NOT NULL AND substr(lower(l.display_path), -(length(?3) + 1)) = '.' || ?3)) \
               AND (?4 IS NULL OR f.modified_unix_ns >= ?4) \
               AND (?5 IS NULL OR f.modified_unix_ns < ?5) \
               AND s.platform = ?7 AND g.platform = ?7 \
               AND NOT EXISTS (SELECT 1 FROM scope_exclusions x WHERE x.scope_id=l.scope_id AND (l.path_key=x.path_key OR (x.kind='folder' AND length(l.path_key)>length(x.path_key) AND substr(l.path_key,1,length(x.path_key))=x.path_key AND (substr(x.path_key,-1,1)=CASE WHEN s.platform='windows' THEN char(92) ELSE '/' END OR substr(l.path_key,length(x.path_key)+1,1)=CASE WHEN s.platform='windows' THEN char(92) ELSE '/' END)) OR (n.identity_kind=x.identity_kind AND n.identity_key=x.identity_key))) \
             ORDER BY location_search_fts.rank, l.id \
             LIMIT ?6"
        };
        let mut metadata_statement = connection.prepare(metadata_sql)?;
        let metadata_rows = if let Some(folder_node_id) = filters.folder_node_id {
            metadata_statement.query_map(
                params![
                    match_query,
                    filters.scope_id,
                    filters.extension,
                    filters.modified_since_unix_ns,
                    filters.modified_before_unix_ns,
                    limit,
                    std::env::consts::OS,
                    folder_node_id,
                    selected_folder_path_key
                        .as_deref()
                        .ok_or(DatabaseError::SearchFolderInvalid)?,
                ],
                lexical_metadata_candidate_from_row,
            )?
        } else {
            metadata_statement.query_map(
                params![
                    match_query,
                    filters.scope_id,
                    filters.extension,
                    filters.modified_since_unix_ns,
                    filters.modified_before_unix_ns,
                    limit,
                    std::env::consts::OS,
                ],
                lexical_metadata_candidate_from_row,
            )?
        };
        for row in metadata_rows {
            candidates.push(row?);
        }
    }

    if filters.source != LexicalSearchSource::MetadataPath {
        let content_sql = if filters.folder_node_id.is_some() {
            "WITH RECURSIVE folder_tree(node_id) AS ( \
                 SELECT ?8 \
                 UNION \
                 SELECT edge.source_node_id \
                 FROM folder_tree parent \
                 CROSS JOIN edges edge ON edge.target_node_id = parent.node_id \
                 WHERE edge.scope_id = ?2 AND edge.kind = 'located_in' AND edge.active = 1 \
             ) \
             SELECT c.scope_id, s.policy_revision, c.node_id, c.location_id, l.path_key, l.display_path, \
                    n.identity_kind, n.identity_key, snippet(content_search_fts, 0, '[', ']', '…', 24) \
             FROM content_search_fts \
             CROSS JOIN content_chunks c ON c.id = content_search_fts.rowid \
             JOIN folder_tree ON folder_tree.node_id = c.node_id \
             JOIN locations l ON l.id = c.location_id \
                AND l.node_id = c.node_id AND l.scope_id = c.scope_id \
             JOIN nodes n ON n.id = c.node_id \
             JOIN authorized_scopes s ON s.id = c.scope_id \
             JOIN scope_access_grants g ON g.scope_id = s.id AND g.platform = s.platform AND g.state = 'active' \
             JOIN extraction_jobs e ON e.id = c.extraction_job_id \
                AND e.scope_id = c.scope_id AND e.node_id = c.node_id \
                AND e.location_id = c.location_id AND e.status = 'completed' \
             JOIN files f ON f.node_id = c.node_id \
             WHERE content_search_fts MATCH ?1 AND c.active = 1 AND l.present = 1 \
               AND (?2 IS NULL OR c.scope_id = ?2) \
               AND (?3 IS NULL OR substr(lower(l.display_path), -(length(?3) + 1)) = '.' || ?3) \
               AND (?4 IS NULL OR f.modified_unix_ns >= ?4) \
               AND (?5 IS NULL OR f.modified_unix_ns < ?5) \
               AND s.platform = ?7 AND g.platform = ?7 \
               AND (l.path_key = ?9 OR ( \
                   length(l.path_key) > length(?9) \
                   AND substr(l.path_key, 1, length(?9)) = ?9 \
                   AND (substr(?9, -1, 1) = CASE WHEN s.platform='windows' THEN char(92) ELSE '/' END \
                        OR substr(l.path_key, length(?9) + 1, 1) = CASE WHEN s.platform='windows' THEN char(92) ELSE '/' END))) \
               AND NOT EXISTS (SELECT 1 FROM scope_exclusions x WHERE x.scope_id=l.scope_id AND (l.path_key=x.path_key OR (x.kind='folder' AND length(l.path_key)>length(x.path_key) AND substr(l.path_key,1,length(x.path_key))=x.path_key AND (substr(x.path_key,-1,1)=CASE WHEN s.platform='windows' THEN char(92) ELSE '/' END OR substr(l.path_key,length(x.path_key)+1,1)=CASE WHEN s.platform='windows' THEN char(92) ELSE '/' END)) OR (n.identity_kind=x.identity_kind AND n.identity_key=x.identity_key))) \
             ORDER BY content_search_fts.rank, c.node_id, c.ordinal \
             LIMIT ?6"
        } else {
            "SELECT c.scope_id, s.policy_revision, c.node_id, c.location_id, l.path_key, l.display_path, \
                    n.identity_kind, n.identity_key, snippet(content_search_fts, 0, '[', ']', '…', 24) \
             FROM content_search_fts \
             CROSS JOIN content_chunks c ON c.id = content_search_fts.rowid \
             JOIN locations l ON l.id = c.location_id \
                AND l.node_id = c.node_id AND l.scope_id = c.scope_id \
             JOIN nodes n ON n.id = c.node_id \
             JOIN authorized_scopes s ON s.id = c.scope_id \
             JOIN scope_access_grants g ON g.scope_id = s.id AND g.platform = s.platform AND g.state = 'active' \
             JOIN extraction_jobs e ON e.id = c.extraction_job_id \
                AND e.scope_id = c.scope_id AND e.node_id = c.node_id \
                AND e.location_id = c.location_id AND e.status = 'completed' \
             JOIN files f ON f.node_id = c.node_id \
             WHERE content_search_fts MATCH ?1 AND c.active = 1 AND l.present = 1 \
               AND (?2 IS NULL OR c.scope_id = ?2) \
               AND (?3 IS NULL OR substr(lower(l.display_path), -(length(?3) + 1)) = '.' || ?3) \
               AND (?4 IS NULL OR f.modified_unix_ns >= ?4) \
               AND (?5 IS NULL OR f.modified_unix_ns < ?5) \
               AND s.platform = ?7 AND g.platform = ?7 \
               AND NOT EXISTS (SELECT 1 FROM scope_exclusions x WHERE x.scope_id=l.scope_id AND (l.path_key=x.path_key OR (x.kind='folder' AND length(l.path_key)>length(x.path_key) AND substr(l.path_key,1,length(x.path_key))=x.path_key AND (substr(x.path_key,-1,1)=CASE WHEN s.platform='windows' THEN char(92) ELSE '/' END OR substr(l.path_key,length(x.path_key)+1,1)=CASE WHEN s.platform='windows' THEN char(92) ELSE '/' END)) OR (n.identity_kind=x.identity_kind AND n.identity_key=x.identity_key))) \
             ORDER BY content_search_fts.rank, c.node_id, c.ordinal \
             LIMIT ?6"
        };
        let mut content_statement = connection.prepare(content_sql)?;
        let content_rows = if let Some(folder_node_id) = filters.folder_node_id {
            content_statement.query_map(
                params![
                    match_query,
                    filters.scope_id,
                    filters.extension,
                    filters.modified_since_unix_ns,
                    filters.modified_before_unix_ns,
                    limit,
                    std::env::consts::OS,
                    folder_node_id,
                    selected_folder_path_key
                        .as_deref()
                        .ok_or(DatabaseError::SearchFolderInvalid)?,
                ],
                lexical_content_candidate_from_row,
            )?
        } else {
            content_statement.query_map(
                params![
                    match_query,
                    filters.scope_id,
                    filters.extension,
                    filters.modified_since_unix_ns,
                    filters.modified_before_unix_ns,
                    limit,
                    std::env::consts::OS,
                ],
                lexical_content_candidate_from_row,
            )?
        };
        for row in content_rows {
            candidates.push(row?);
        }
    }

    Ok(candidates)
}

fn lexical_metadata_candidate_from_row(
    row: &rusqlite::Row<'_>,
) -> rusqlite::Result<LexicalSearchCandidate> {
    Ok(LexicalSearchCandidate {
        source: LexicalCandidateSource::MetadataPath,
        scope_id: row.get(0)?,
        policy_revision: row.get(1)?,
        node_id: row.get(2)?,
        location_id: row.get(3)?,
        path_key: row.get(4)?,
        display_path: row.get(5)?,
        identity_kind: row.get(6)?,
        identity_key: row.get(7)?,
        snippet: None,
    })
}

fn lexical_content_candidate_from_row(
    row: &rusqlite::Row<'_>,
) -> rusqlite::Result<LexicalSearchCandidate> {
    Ok(LexicalSearchCandidate {
        source: LexicalCandidateSource::ExtractedText,
        scope_id: row.get(0)?,
        policy_revision: row.get(1)?,
        node_id: row.get(2)?,
        location_id: row.get(3)?,
        path_key: row.get(4)?,
        display_path: row.get(5)?,
        identity_kind: row.get(6)?,
        identity_key: row.get(7)?,
        snippet: Some(row.get(8)?),
    })
}

fn upsert_observation(
    transaction: &Transaction<'_>,
    scope_id: i64,
    job_id: i64,
    observation: &Observation,
    timestamp: i64,
) -> Result<(), DatabaseError> {
    transaction.execute(
        "INSERT INTO nodes(kind, identity_kind, identity_key, created_at_unix_ms, updated_at_unix_ms)\
         VALUES (?1, ?2, ?3, ?4, ?4)\
         ON CONFLICT(identity_key) DO UPDATE SET kind = excluded.kind, identity_kind = excluded.identity_kind, updated_at_unix_ms = excluded.updated_at_unix_ms",
        params![
            observation.kind.as_str(),
            observation.identity_kind,
            observation.identity_key,
            timestamp,
        ],
    )?;
    let node_id: i64 = transaction.query_row(
        "SELECT id FROM nodes WHERE identity_key = ?1",
        [&observation.identity_key],
        |row| row.get(0),
    )?;

    transaction.execute(
        "INSERT INTO locations(scope_id, node_id, path_raw, path_key, display_path, present, last_seen_scan_id)\
         VALUES (?1, ?2, ?3, ?4, ?5, 1, ?6)\
         ON CONFLICT(scope_id, path_key) DO UPDATE SET node_id = excluded.node_id, path_raw = excluded.path_raw,\
            display_path = excluded.display_path, present = 1, last_seen_scan_id = excluded.last_seen_scan_id",
        params![
            scope_id,
            node_id,
            observation.path_raw,
            observation.path_key,
            observation.display_path,
            job_id,
        ],
    )?;

    match observation.kind {
        NodeKind::File => {
            transaction.execute(
                "INSERT INTO files(node_id, size_bytes, modified_unix_ns, link_count) VALUES (?1, ?2, ?3, ?4)\
                 ON CONFLICT(node_id) DO UPDATE SET size_bytes = excluded.size_bytes, modified_unix_ns = excluded.modified_unix_ns, link_count = excluded.link_count",
                params![
                    node_id,
                    to_i64(observation.size_bytes)?,
                    observation.modified_unix_ns,
                    observation.link_count.map(to_i64).transpose()?,
                ],
            )?;
        }
        NodeKind::Folder => {
            transaction.execute(
                "INSERT INTO folders(node_id) VALUES (?1) ON CONFLICT(node_id) DO NOTHING",
                [node_id],
            )?;
        }
    }

    if let Some(parent_identity_key) = &observation.parent_identity_key {
        let parent_node_id: i64 = transaction.query_row(
            "SELECT id FROM nodes WHERE identity_key = ?1",
            [parent_identity_key],
            |row| row.get(0),
        )?;
        transaction.execute(
            "INSERT INTO edges(scope_id, source_node_id, target_node_id, kind, active, last_seen_scan_id)\
             VALUES (?1, ?2, ?3, 'located_in', 1, ?4)\
             ON CONFLICT(scope_id, source_node_id, target_node_id, kind) DO UPDATE SET active = 1, last_seen_scan_id = excluded.last_seen_scan_id",
            params![scope_id, node_id, parent_node_id, job_id],
        )?;
    }

    Ok(())
}

fn count(connection: &Connection, sql: &str) -> Result<u64, DatabaseError> {
    let result = connection
        .query_row(sql, [], |row| row.get::<_, i64>(0))
        .optional()?;
    u64::try_from(result.unwrap_or(0)).map_err(|_| DatabaseError::InvalidCount)
}

fn count_with_host_platform(connection: &Connection, sql: &str) -> Result<u64, DatabaseError> {
    let result = connection
        .query_row(sql, [std::env::consts::OS], |row| row.get::<_, i64>(0))
        .optional()?;
    u64::try_from(result.unwrap_or(0)).map_err(|_| DatabaseError::InvalidCount)
}

fn to_i64(value: u64) -> Result<i64, DatabaseError> {
    i64::try_from(value).map_err(|_| DatabaseError::InvalidCount)
}

fn validate_scope_access_grant_platform(platform: &str) -> Result<(), DatabaseError> {
    match platform {
        "macos" | "windows" | "linux" => Ok(()),
        _ => Err(DatabaseError::ScopeAccessGrantInputInvalid),
    }
}

fn validate_scope_access_grant_bytes(opaque_grant: &[u8]) -> Result<(), DatabaseError> {
    if opaque_grant.is_empty() || opaque_grant.len() > MAX_SCOPE_ACCESS_GRANT_BYTES {
        return Err(DatabaseError::ScopeAccessGrantInputInvalid);
    }
    Ok(())
}

fn coverage_paths_overlap(left: &Path, right: &Path) -> bool {
    let left_key = comparison_key(left);
    let right_key = comparison_key(right);
    left.ancestors()
        .any(|ancestor| comparison_key(ancestor) == right_key)
        || right
            .ancestors()
            .any(|ancestor| comparison_key(ancestor) == left_key)
}

fn scope_access_grant_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<ScopeAccessGrant> {
    let state = row.get::<_, String>(3)?;
    let state = ScopeAccessGrantState::from_db(&state).map_err(|_| {
        rusqlite::Error::FromSqlConversionFailure(
            3,
            rusqlite::types::Type::Text,
            Box::new(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "invalid scope access grant state",
            )),
        )
    })?;
    Ok(ScopeAccessGrant {
        scope_id: row.get(0)?,
        platform: row.get(1)?,
        opaque_grant: row.get(2)?,
        state,
        updated_at_unix_ms: row.get(4)?,
    })
}

fn unix_ms() -> Result<i64, DatabaseError> {
    let duration = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| DatabaseError::InvalidTimestamp)?;
    i64::try_from(duration.as_millis()).map_err(|_| DatabaseError::InvalidTimestamp)
}

fn migration_checksum(sql: &str) -> String {
    let mut hash = 0xcbf29ce484222325_u64;
    for byte in sql.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    format!("fnv1a64:{hash:016x}")
}

#[cfg(test)]
fn test_revision_binding(
    database: &ManifestDatabase,
    scope_id: i64,
) -> Result<ScopeRevisionBinding, DatabaseError> {
    let revision = current_scope_policy_revision_from_connection(&database.connection, scope_id)?;
    Ok(ScopeRevisionBinding { scope_id, revision })
}

#[cfg(test)]
fn test_active_binding(
    database: &ManifestDatabase,
    scope_id: i64,
) -> Result<ScopePolicyBinding, DatabaseError> {
    if !database.scope_has_active_access_grant(scope_id)? {
        let mut platform: String = database.connection.query_row(
            "SELECT platform FROM authorized_scopes WHERE id=?1",
            [scope_id],
            |row| row.get(0),
        )?;
        if !matches!(platform.as_str(), "macos" | "linux" | "windows") {
            platform = "macos".to_string();
            database.connection.execute(
                "UPDATE authorized_scopes SET platform=?2 WHERE id=?1",
                params![scope_id, platform],
            )?;
        }
        database.upsert_scope_access_grant(scope_id, &platform, b"database-unit-test-grant")?;
    }
    database.bind_scope_policy_revision(scope_id)
}

#[cfg(test)]
mod tests {
    use super::*;

    const TEST_EXCLUDED_FILE_IDENTITY: &[u8] = b"f0000000000000000";
    const TEST_EXCLUDED_FILE_IDENTITY_2: &[u8] = b"f0000000000000001";
    const TEST_EXCLUDED_FOLDER_IDENTITY: &[u8] = b"d0000000000000000";
    const TEST_EXCLUDED_IDENTITY_KIND: &str = "unix_device_inode";

    fn lexical_filters(scope_id: Option<i64>) -> LexicalSearchFilters<'static> {
        LexicalSearchFilters {
            scope_id,
            folder_node_id: None,
            source: LexicalSearchSource::All,
            extension: None,
            modified_since_unix_ns: None,
            modified_before_unix_ns: None,
        }
    }

    fn folder_search_setup() -> (ManifestDatabase, i64) {
        let mut database = ManifestDatabase::open_in_memory().expect("database should initialize");
        let scope = database
            .add_scope_with_access_grant(
                b"/scope",
                "/scope",
                "/scope",
                ScopeAccessGrantWrite {
                    scope_platform: std::env::consts::OS,
                    grant_platform: std::env::consts::OS,
                    opaque_grant: b"folder-search-grant",
                    state: ScopeAccessGrantState::Active,
                },
            )
            .expect("folder-search scope should persist");
        let scan_id = database
            .create_scan_job_with_policy(
                database
                    .bind_core_scope_policy_revision(scope.id)
                    .expect("core scope binding should load"),
            )
            .expect("folder-search scan should start");
        let root = observation("/scope", NodeKind::Folder, None);
        let selected = observation(
            "/scope/needle-project",
            NodeKind::Folder,
            Some(root.identity_key.clone()),
        );
        let deep = observation(
            "/scope/needle-project/needle-deep",
            NodeKind::Folder,
            Some(selected.identity_key.clone()),
        );
        let direct = observation(
            "/scope/needle-project/needle-direct.txt",
            NodeKind::File,
            Some(selected.identity_key.clone()),
        );
        let deep_file = observation(
            "/scope/needle-project/needle-deep/needle-deep-file.txt",
            NodeKind::File,
            Some(deep.identity_key.clone()),
        );
        let stale = observation(
            "/scope/needle-project/needle-stale.txt",
            NodeKind::File,
            Some(selected.identity_key.clone()),
        );
        let sibling = observation(
            "/scope/needle-sibling.txt",
            NodeKind::File,
            Some(root.identity_key.clone()),
        );
        database
            .complete_scan(
                scan_id,
                scope.id,
                &[root, selected, deep, direct, deep_file, stale, sibling],
                &[],
                0,
                0,
            )
            .expect("folder-search manifest should publish");
        (database, scope.id)
    }

    fn folder_search_node_id(database: &ManifestDatabase, scope_id: i64, path: &str) -> i64 {
        database
            .node_id_for_path_key(scope_id, path)
            .expect("folder-search node lookup should pass")
            .expect("folder-search node should exist")
    }

    fn insert_folder_search_content(
        database: &ManifestDatabase,
        scope_id: i64,
        path: &str,
        text: &str,
        active: bool,
    ) {
        let (node_id, location_id): (i64, i64) = database
            .connection
            .query_row(
                "SELECT node_id,id FROM locations WHERE scope_id=?1 AND path_key=?2",
                params![scope_id, path],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .expect("folder-search location should load");
        database
            .connection
            .execute(
                "INSERT INTO extraction_jobs( \
                     scope_id,node_id,location_id,status,source_size_bytes,created_at_unix_ms, \
                     finished_at_unix_ms,updated_at_unix_ms,policy_revision \
                 ) VALUES(?1,?2,?3,'completed',4,0,1,1,1)",
                params![scope_id, node_id, location_id],
            )
            .expect("folder-search extraction job should persist");
        let extraction_job_id = database.connection.last_insert_rowid();
        database
            .connection
            .execute(
                "INSERT INTO content_chunks( \
                     scope_id,node_id,location_id,extraction_job_id,ordinal,text,provenance_kind, \
                     source_byte_start,source_byte_end,source_size_bytes,source_modified_unix_ns, \
                     trust_class,provider_id,provider_version,active,created_at_unix_ms \
                 ) VALUES(?1,?2,?3,?4,0,?5,'byte_range',0,4,4,1, \
                          'untrusted_extracted_text','test','1',?6,1)",
                params![
                    scope_id,
                    node_id,
                    location_id,
                    extraction_job_id,
                    text,
                    i64::from(active)
                ],
            )
            .expect("folder-search content chunk should persist");
    }

    fn apply_migration_prefix(connection: &Connection, count: usize) {
        connection
            .execute_batch(
                "CREATE TABLE schema_migrations ( \
                     version INTEGER PRIMARY KEY, \
                     name TEXT NOT NULL, \
                     checksum TEXT NOT NULL, \
                     applied_at_unix_ms INTEGER NOT NULL \
                 );",
            )
            .expect("migration registry should initialize");
        for migration in &MIGRATIONS[..count] {
            connection
                .execute_batch(migration.sql)
                .expect("historical migration should apply");
            connection
                .execute(
                    "INSERT INTO schema_migrations(version, name, checksum, applied_at_unix_ms) \
                     VALUES (?1, ?2, ?3, 0)",
                    params![
                        migration.version,
                        migration.name,
                        migration_checksum(migration.sql)
                    ],
                )
                .expect("historical migration should register");
        }
    }

    fn apply_migrations_before_scope_exclusions(connection: &Connection) {
        apply_migration_prefix(connection, 23);
    }

    fn foreign_platform() -> &'static str {
        match std::env::consts::OS {
            "windows" => "macos",
            _ => "windows",
        }
    }

    fn clone_table_row_without_id(
        connection: &Connection,
        table: &str,
        source_id: i64,
    ) -> rusqlite::Result<usize> {
        let mut statement = connection.prepare(&format!("PRAGMA table_info({table})"))?;
        let columns = statement
            .query_map([], |row| row.get::<_, String>(1))?
            .collect::<rusqlite::Result<Vec<_>>>()?
            .into_iter()
            .filter(|column| column != "id")
            .collect::<Vec<_>>()
            .join(",");
        connection.execute(
            &format!("INSERT INTO {table}({columns}) SELECT {columns} FROM {table} WHERE id=?1"),
            [source_id],
        )
    }

    fn resumable_setup() -> (ManifestDatabase, i64, QueuedPath) {
        resumable_setup_in(ManifestDatabase::open_in_memory().expect("database should initialize"))
    }

    fn resumable_setup_in(database: ManifestDatabase) -> (ManifestDatabase, i64, QueuedPath) {
        let scope = database
            .add_scope(b"/scope", "/scope", "/scope", "test")
            .expect("scope should persist");
        let root = QueuedPath {
            path_raw: b"/scope".to_vec(),
            path_key: "/scope".to_string(),
            parent_identity_key: None,
            is_root: true,
        };
        (database, scope.id, root)
    }

    fn record_file_watch_event(
        database: &mut ManifestDatabase,
        scope_id: i64,
        path: &str,
        stable_after_unix_ms: i64,
        ignored_reason: Option<WatchEventReason>,
    ) -> WatchEventRecord {
        record_file_watch_event_with_kind(
            database,
            scope_id,
            path,
            stable_after_unix_ms,
            ignored_reason,
            WatchReconciliationKind::FullScope,
        )
    }

    fn record_file_watch_event_with_kind(
        database: &mut ManifestDatabase,
        scope_id: i64,
        path: &str,
        stable_after_unix_ms: i64,
        ignored_reason: Option<WatchEventReason>,
        reconciliation_kind: WatchReconciliationKind,
    ) -> WatchEventRecord {
        if !database
            .scope_has_completed_scan(scope_id)
            .expect("scope readiness should load")
        {
            let job_id = database
                .create_scan_job(scope_id)
                .expect("initial scan should start");
            database
                .complete_scan(job_id, scope_id, &[], &[], 0, 0)
                .expect("initial scan should complete");
        }
        let snapshot = WatchSnapshot {
            kind: WatchSnapshotKind::File,
            size_bytes: Some(7),
            modified_unix_ns: Some(11),
            identity_key: Some(format!("identity:{path}").into_bytes()),
        };
        database
            .record_watch_observation_at(WatchObservationWrite {
                scope_id,
                path_raw: path.as_bytes(),
                path_key: path,
                snapshot: &snapshot,
                stable_after_unix_ms,
                ignored_reason,
                reconciliation_kind,
                observed_at_unix_ms: 1,
            })
            .expect("watch event should persist")
    }

    #[test]
    fn forced_watch_metadata_reconciliation_is_root_only_and_keeps_normal_debounce() {
        let (mut database, scope_id, root) = resumable_setup();
        let event = record_file_watch_event(
            &mut database,
            scope_id,
            "/scope/private-active.md",
            5_000,
            None,
        );

        assert!(matches!(
            database.begin_watch_reconciliation_at(event.progress.event_id, &root, 1_000),
            Err(DatabaseError::InvalidWatchEventState)
        ));
        let mut non_root = root.clone();
        non_root.is_root = false;
        assert!(matches!(
            database.begin_forced_watch_metadata_reconciliation_at(
                event.progress.event_id,
                &non_root,
                1_000
            ),
            Err(DatabaseError::WatchInputInvalid)
        ));
        let mut mismatched_raw = root.clone();
        mismatched_raw.path_raw = b"/scope-other-raw".to_vec();
        assert!(matches!(
            database.begin_forced_watch_metadata_reconciliation_at(
                event.progress.event_id,
                &mismatched_raw,
                1_000
            ),
            Err(DatabaseError::WatchInputInvalid)
        ));
        let mut mismatched_key = root.clone();
        mismatched_key.path_key = "/scope-other-key".to_string();
        assert!(matches!(
            database.begin_forced_watch_metadata_reconciliation_at(
                event.progress.event_id,
                &mismatched_key,
                1_000
            ),
            Err(DatabaseError::WatchInputInvalid)
        ));
        let relative_root = QueuedPath {
            path_raw: b"scope".to_vec(),
            path_key: "scope".to_string(),
            parent_identity_key: None,
            is_root: true,
        };
        assert!(matches!(
            database.begin_forced_watch_metadata_reconciliation_at(
                event.progress.event_id,
                &relative_root,
                1_000
            ),
            Err(DatabaseError::WatchInputInvalid)
        ));
        database
            .add_scope(b"/other", "/other", "/other", "test")
            .expect("second scope should persist");
        let another_scope_root = QueuedPath {
            path_raw: b"/other".to_vec(),
            path_key: "/other".to_string(),
            parent_identity_key: None,
            is_root: true,
        };
        assert!(matches!(
            database.begin_forced_watch_metadata_reconciliation_at(
                event.progress.event_id,
                &another_scope_root,
                1_000
            ),
            Err(DatabaseError::WatchInputInvalid)
        ));
        let forced = database
            .begin_forced_watch_metadata_reconciliation_at(event.progress.event_id, &root, 1_000)
            .expect("authorized root metadata recovery should start durably");
        assert_eq!(forced.status, WatchEventStatus::Reconciling);
    }

    #[test]
    fn forced_watch_metadata_reconciliation_rechecks_completed_initial_scan_in_transaction() {
        let (mut database, scope_id, root) = resumable_setup();
        let event = record_file_watch_event(
            &mut database,
            scope_id,
            "/scope/private-active.md",
            5_000,
            None,
        );
        database
            .connection
            .execute(
                "UPDATE scan_jobs SET status = 'failed' WHERE scope_id = ?1",
                [scope_id],
            )
            .expect("fixture should remove completed-scan eligibility");

        assert!(matches!(
            database.begin_forced_watch_metadata_reconciliation_at(
                event.progress.event_id,
                &root,
                1_000
            ),
            Err(DatabaseError::WatchScopeInitialScanRequired)
        ));
    }

    #[test]
    fn ignored_state_transition_merges_and_removes_only_the_transient_row() {
        let (mut database, scope_id, _) = resumable_setup();
        let ignored = record_file_watch_event(
            &mut database,
            scope_id,
            "/scope/archive.crdownload",
            1,
            Some(WatchEventReason::TemporaryDownload),
        );
        let stabilizing =
            record_file_watch_event(&mut database, scope_id, "/scope/archive.md", 2_000, None);
        assert_ne!(stabilizing.progress.event_id, ignored.progress.event_id);

        let merged = database
            .mark_watch_event_ignored_at(
                stabilizing.progress.event_id,
                WatchEventReason::TemporaryDownload,
                1_500,
            )
            .expect("ignored transition should merge transactionally");

        assert_eq!(merged.event_id, ignored.progress.event_id);
        assert_eq!(merged.status, WatchEventStatus::Ignored);
        assert_eq!(merged.observation_count, 2);
        assert!(matches!(
            database.watch_event(stabilizing.progress.event_id),
            Err(DatabaseError::WatchEventNotFound)
        ));
        assert_eq!(
            database
                .recent_watch_events()
                .expect("watch history should load"),
            vec![merged.clone()]
        );

        let second_stabilizing = record_file_watch_event(
            &mut database,
            scope_id,
            "/scope/archive-final.md",
            3_000,
            None,
        );
        let direct_ignored = record_file_watch_event(
            &mut database,
            scope_id,
            "/scope/archive-final.download",
            2_500,
            Some(WatchEventReason::TemporaryDownload),
        );
        assert_eq!(direct_ignored.progress.event_id, merged.event_id);
        assert_eq!(direct_ignored.progress.observation_count, 3);
        assert_eq!(
            database
                .watch_event(second_stabilizing.progress.event_id)
                .expect("unrelated stabilizing work must remain pending")
                .progress,
            second_stabilizing.progress
        );
        let transitioned = database
            .mark_watch_event_ignored_at(
                second_stabilizing.progress.event_id,
                WatchEventReason::TemporaryDownload,
                2_550,
            )
            .expect("only an explicit transition may supersede stabilizing work");
        assert_eq!(transitioned.event_id, merged.event_id);
        assert_eq!(transitioned.observation_count, 4);
        assert!(matches!(
            database.watch_event(second_stabilizing.progress.event_id),
            Err(DatabaseError::WatchEventNotFound)
        ));
        assert_eq!(
            database
                .recent_watch_events()
                .expect("watch history should remain bounded"),
            vec![transitioned]
        );

        let newer_id = record_file_watch_event(
            &mut database,
            scope_id,
            "/scope/reconcile-failed.md",
            4_000,
            None,
        );
        database
            .fail_watch_event_at(
                newer_id.progress.event_id,
                WatchEventReason::ReconcileFailed,
                2_600,
            )
            .expect("separate terminal history should persist");
        let latest_snapshot = WatchSnapshot {
            kind: WatchSnapshotKind::File,
            size_bytes: Some(7),
            modified_unix_ns: Some(11),
            identity_key: Some(b"identity:/scope/archive-latest.part".to_vec()),
        };
        let latest_ignored = database
            .record_watch_observation_at(WatchObservationWrite {
                scope_id,
                path_raw: b"/scope/archive-latest.part",
                path_key: "/scope/archive-latest.part",
                snapshot: &latest_snapshot,
                stable_after_unix_ms: 2_700,
                ignored_reason: Some(WatchEventReason::TemporaryDownload),
                reconciliation_kind: WatchReconciliationKind::FullScope,
                observed_at_unix_ms: 2_700,
            })
            .expect("latest ignored observation should update its terminal aggregate");
        let recent = database
            .recent_watch_events()
            .expect("recent history should order by the latest update");
        assert_eq!(recent[0], latest_ignored.progress);
        assert_eq!(recent[1].event_id, newer_id.progress.event_id);
    }

    #[test]
    fn direct_ignored_observation_never_supersedes_unrelated_stabilizing_work() {
        let (mut database, scope_id, _) = resumable_setup();
        let stabilizing =
            record_file_watch_event(&mut database, scope_id, "/scope/report.md", 2_000, None);
        let ignored = record_file_watch_event(
            &mut database,
            scope_id,
            "/scope/download.crdownload",
            1_100,
            Some(WatchEventReason::TemporaryDownload),
        );

        assert_ne!(ignored.progress.event_id, stabilizing.progress.event_id);
        assert_eq!(ignored.progress.status, WatchEventStatus::Ignored);
        assert_eq!(
            database
                .watch_event(stabilizing.progress.event_id)
                .expect("normal pending work must survive an unrelated ignored path")
                .progress,
            stabilizing.progress
        );
        assert_eq!(
            database
                .active_watch_events()
                .expect("active work should remain queryable"),
            vec![stabilizing.progress]
        );
    }

    #[test]
    fn active_watch_events_are_path_free_ordered_and_exclude_terminal_states() {
        let (mut database, scope_id, root) = resumable_setup();
        let reconciling = record_file_watch_event(
            &mut database,
            scope_id,
            "/scope/private-reconciling.md",
            1,
            None,
        );
        database
            .begin_watch_reconciliation_at(reconciling.progress.event_id, &root, 1)
            .expect("ready watch event should become reconciling");

        let mut stabilizing_ids = Vec::new();
        for index in 1_i64..=21 {
            let scope_path = format!("/scope-{index}");
            let scope = database
                .add_scope(scope_path.as_bytes(), &scope_path, &scope_path, "test")
                .expect("scope should persist");
            let event = record_file_watch_event(
                &mut database,
                scope.id,
                &format!("{scope_path}/private-stabilizing-{index}.md"),
                200 + (22 - index),
                None,
            );
            stabilizing_ids.push(event.progress.event_id);
        }

        let ignored_scope = database
            .add_scope(
                b"/scope-terminal-ignored",
                "/scope-terminal-ignored",
                "/scope-terminal-ignored",
                "test",
            )
            .expect("ignored fixture scope should persist");
        let ignored = record_file_watch_event(
            &mut database,
            ignored_scope.id,
            "/scope-terminal-ignored/private-ignored.md",
            300,
            Some(WatchEventReason::TemporaryDownload),
        );

        let failed_scope = database
            .add_scope(
                b"/scope-terminal-failed",
                "/scope-terminal-failed",
                "/scope-terminal-failed",
                "test",
            )
            .expect("failed fixture scope should persist");
        let failed = record_file_watch_event(
            &mut database,
            failed_scope.id,
            "/scope-terminal-failed/private-failed.md",
            300,
            None,
        );
        database
            .fail_watch_event_at(
                failed.progress.event_id,
                WatchEventReason::ReconcileFailed,
                2,
            )
            .expect("watch event should fail");

        let active = database
            .active_watch_events()
            .expect("active watch events should load");
        assert_eq!(active.len(), 22, "active list must not be limited to 20");
        assert_eq!(active[0].event_id, reconciling.progress.event_id);
        assert_eq!(active[0].status, WatchEventStatus::Reconciling);
        assert_eq!(
            active
                .iter()
                .skip(1)
                .map(|event| event.event_id)
                .collect::<Vec<_>>(),
            stabilizing_ids.into_iter().rev().collect::<Vec<_>>(),
            "stabilizing events should sort by stable deadline, then id"
        );
        assert!(active.iter().all(|event| matches!(
            event.status,
            WatchEventStatus::Stabilizing | WatchEventStatus::Reconciling
        )));
        assert!(
            active
                .iter()
                .all(|event| event.event_id != ignored.progress.event_id
                    && event.event_id != failed.progress.event_id)
        );

        let payload = active
            .iter()
            .map(|event| {
                format!(
                    "{{\"api_version\":\"{}\",\"event_id\":{},\"scope_id\":{},\"status\":\"{:?}\",\"observation_count\":{},\"stable_after_unix_ms\":{},\"scan_job_id\":{:?},\"reason\":{:?}}}",
                    event.api_version,
                    event.event_id,
                    event.scope_id,
                    event.status,
                    event.observation_count,
                    event.stable_after_unix_ms,
                    event.scan_job_id,
                    event.reason,
                )
            })
            .collect::<Vec<_>>()
            .join(",");
        for private_value in [
            "private-reconciling.md",
            "private-stabilizing-1.md",
            "private-ignored.md",
            "private-failed.md",
            "/scope-terminal-ignored",
            "/scope-terminal-failed",
        ] {
            assert!(
                !payload.contains(private_value),
                "path-free serialized payload exposed {private_value:?}"
            );
        }
    }

    #[test]
    fn active_watch_event_query_uses_the_partial_deadline_index() {
        let database = ManifestDatabase::open_in_memory().expect("database should initialize");
        let mut statement = database
            .connection
            .prepare(
                "EXPLAIN QUERY PLAN \
                 SELECT id, scope_id, status, observation_count, stable_after_unix_ms, \
                    scan_job_id, reason \
                 FROM watch_events \
                 WHERE status IN ('stabilizing', 'reconciling') \
                 ORDER BY CASE status WHEN 'reconciling' THEN 0 ELSE 1 END, \
                    stable_after_unix_ms ASC, id ASC",
            )
            .expect("query plan should prepare");
        let plan = statement
            .query_map([], |row| row.get::<_, String>(3))
            .expect("query plan should execute")
            .collect::<Result<Vec<_>, _>>()
            .expect("query plan should collect");

        assert!(
            plan.iter()
                .any(|detail| detail.contains("watch_events_active_deadline_idx")),
            "active watch lookup must use the partial deadline index: {plan:?}"
        );
    }

    #[test]
    fn watchable_scope_ids_require_a_completed_initial_scan() {
        let (mut database, scanned_scope_id, root) = resumable_setup();
        let unscanned_scope = database
            .add_scope(
                b"/scope-not-yet-scanned",
                "/scope-not-yet-scanned",
                "/scope-not-yet-scanned",
                "test",
            )
            .expect("unscanned scope should authorize");

        assert!(
            database
                .watchable_scope_ids()
                .expect("watchable scopes should load")
                .is_empty()
        );

        publish_manifest_file(&mut database, scanned_scope_id, &root, 4);

        assert_eq!(
            database
                .watchable_scope_ids()
                .expect("watchable scopes should load"),
            vec![scanned_scope_id]
        );
        assert_ne!(scanned_scope_id, unscanned_scope.id);
    }

    #[test]
    fn desktop_watchability_requires_a_completed_scan_and_active_access_grant() {
        let mut database = ManifestDatabase::open_in_memory().expect("database should initialize");
        let scope = database
            .add_scope(b"/scope", "/scope", "/scope", "macos")
            .expect("scope should persist");
        let root = QueuedPath {
            path_raw: b"/scope".to_vec(),
            path_key: "/scope".to_string(),
            parent_identity_key: None,
            is_root: true,
        };

        assert!(
            database
                .watchable_scope_ids_with_active_access_grants()
                .expect("strict watchable scopes should load")
                .is_empty()
        );
        publish_manifest_file(&mut database, scope.id, &root, 4);
        assert_eq!(
            database
                .watchable_scope_ids()
                .expect("general watchable scopes should load"),
            vec![scope.id]
        );
        assert!(
            database
                .watchable_scope_ids_with_active_access_grants()
                .expect("strict watchable scopes should load")
                .is_empty(),
            "a completed legacy scope must not become Desktop-watchable"
        );
        assert!(
            database
                .request_all_active_granted_scope_full_reconciliation_at(2_000)
                .expect("strict all-scope request should be a no-op")
                .is_empty()
        );

        database
            .upsert_scope_access_grant(scope.id, "macos", b"opaque-grant")
            .expect("active grant should persist");
        assert_eq!(
            database
                .watchable_scope_ids_with_active_access_grants()
                .expect("strict watchable scopes should load"),
            vec![scope.id]
        );
        let requested = database
            .request_all_active_granted_scope_full_reconciliation_at(2_100)
            .expect("strict all-scope request should be atomic");
        assert_eq!(requested.len(), 1);
        assert_eq!(requested[0].scope_id, scope.id);

        database
            .mark_scope_access_grant_needs_reauthorization(scope.id)
            .expect("grant should become inactive");
        assert!(
            database
                .watchable_scope_ids_with_active_access_grants()
                .expect("strict watchable scopes should load")
                .is_empty()
        );
    }

    #[test]
    fn watch_observation_transaction_rejects_an_unscanned_scope() {
        let (mut database, scope_id, _) = resumable_setup();
        let snapshot = WatchSnapshot {
            kind: WatchSnapshotKind::File,
            size_bytes: Some(7),
            modified_unix_ns: Some(11),
            identity_key: Some(b"identity:/scope/private.md".to_vec()),
        };

        let result = database.record_watch_observation_at(WatchObservationWrite {
            scope_id,
            path_raw: b"/scope/private.md",
            path_key: "/scope/private.md",
            snapshot: &snapshot,
            stable_after_unix_ms: 1_001,
            ignored_reason: None,
            reconciliation_kind: WatchReconciliationKind::FullScope,
            observed_at_unix_ms: 1,
        });

        assert!(matches!(
            result,
            Err(DatabaseError::WatchScopeInitialScanRequired)
        ));
        assert_eq!(
            DatabaseError::WatchScopeInitialScanRequired.code(),
            "watch_scope_initial_scan_required"
        );
    }

    #[test]
    fn file_delta_mode_is_path_bound_monotonic_and_reopens_after_terminal_history() {
        let (mut database, scope_id, root) = resumable_setup();
        publish_manifest_file(&mut database, scope_id, &root, 4);

        let first = record_file_watch_event_with_kind(
            &mut database,
            scope_id,
            "/scope/file.txt",
            1_000,
            None,
            WatchReconciliationKind::FileDelta,
        );
        assert_eq!(
            first.reconciliation_kind,
            WatchReconciliationKind::FileDelta
        );
        let same_path = record_file_watch_event_with_kind(
            &mut database,
            scope_id,
            "/scope/file.txt",
            2_000,
            None,
            WatchReconciliationKind::FileDelta,
        );
        assert_eq!(same_path.progress.event_id, first.progress.event_id);
        assert_eq!(
            same_path.reconciliation_kind,
            WatchReconciliationKind::FileDelta
        );

        let replacement_snapshot = WatchSnapshot {
            kind: WatchSnapshotKind::File,
            size_bytes: Some(7),
            modified_unix_ns: Some(11),
            identity_key: Some(b"identity:/scope/file.txt:replacement".to_vec()),
        };
        let atomic_replace = database
            .record_watch_observation_at(WatchObservationWrite {
                scope_id,
                path_raw: b"/scope/file.txt",
                path_key: "/scope/file.txt",
                snapshot: &replacement_snapshot,
                stable_after_unix_ms: 2_500,
                ignored_reason: None,
                reconciliation_kind: WatchReconciliationKind::FileDelta,
                observed_at_unix_ms: 1,
            })
            .expect("same-path replacement observation should persist");
        assert_eq!(atomic_replace.progress.event_id, first.progress.event_id);
        assert_eq!(
            atomic_replace.reconciliation_kind,
            WatchReconciliationKind::FullScope,
            "same-path inode replacement must not stay on the narrow path"
        );

        let distinct_path = record_file_watch_event_with_kind(
            &mut database,
            scope_id,
            "/scope/other.txt",
            3_000,
            None,
            WatchReconciliationKind::FileDelta,
        );
        assert_eq!(distinct_path.progress.event_id, first.progress.event_id);
        assert_eq!(
            distinct_path.reconciliation_kind,
            WatchReconciliationKind::FullScope
        );
        let upgraded = database
            .request_scope_full_reconciliation_at(scope_id, 10)
            .expect("full request should durably upgrade the existing event");
        assert_eq!(upgraded.event_id, first.progress.event_id);
        assert_eq!(
            database
                .watch_event(upgraded.event_id)
                .expect("upgraded event should load")
                .reconciliation_kind,
            WatchReconciliationKind::FullScope
        );

        database
            .connection
            .execute(
                "UPDATE watch_events SET status = 'completed' WHERE id = ?1",
                [upgraded.event_id],
            )
            .expect("fixture should close the old history");
        let reopened = record_file_watch_event_with_kind(
            &mut database,
            scope_id,
            "/scope/file.txt",
            4_000,
            None,
            WatchReconciliationKind::FileDelta,
        );
        assert_ne!(reopened.progress.event_id, upgraded.event_id);
        assert_eq!(
            reopened.reconciliation_kind,
            WatchReconciliationKind::FileDelta
        );
    }

    #[test]
    fn durable_full_scope_requests_create_and_upgrade_watchable_scopes() {
        let (mut database, scope_id, root) = resumable_setup();
        publish_manifest_file(&mut database, scope_id, &root, 4);

        let created = database
            .request_scope_full_reconciliation_at(scope_id, 2_000)
            .expect("scope request should create a durable event");
        assert_eq!(created.status, WatchEventStatus::Stabilizing);
        assert_eq!(created.stable_after_unix_ms, 2_000);
        let record = database
            .watch_event(created.event_id)
            .expect("durable event should load");
        assert_eq!(
            record.reconciliation_kind,
            WatchReconciliationKind::FullScope
        );
        assert_eq!(record.snapshot.kind, WatchSnapshotKind::Folder);

        let all = database
            .request_all_scope_full_reconciliation_at(2_100)
            .expect("all request should update every watchable scope");
        assert_eq!(all.len(), 1);
        assert_eq!(all[0].event_id, created.event_id);
        assert_eq!(all[0].stable_after_unix_ms, 2_100);
        assert_eq!(all[0].observation_count, 2);
    }

    #[test]
    fn all_scope_full_reconciliation_rolls_back_every_scope_when_one_scope_is_invalid() {
        let (mut database, healthy_scope_id, root) = resumable_setup();
        publish_manifest_file(&mut database, healthy_scope_id, &root, 4);
        let incomplete_scope = database
            .add_scope(
                b"/scope-without-root",
                "/scope-without-root",
                "/scope-without-root",
                "test",
            )
            .expect("second scope should authorize");
        let job_id = database
            .create_scan_job(incomplete_scope.id)
            .expect("second scope scan should start");
        database
            .complete_scan(job_id, incomplete_scope.id, &[], &[], 0, 0)
            .expect("empty second scope scan should complete");

        let error = database
            .request_all_scope_full_reconciliation_at(2_000)
            .expect_err("a missing root manifest row must reject the all-scope request");
        assert!(matches!(error, DatabaseError::WatchFileDeltaNotEligible));
        assert!(
            database
                .active_watch_events()
                .expect("events should load")
                .is_empty(),
            "the earlier healthy scope update must roll back with the failing scope"
        );
    }

    #[test]
    fn file_delta_publish_keeps_siblings_and_atomically_invalidates_content() {
        let (mut database, scope_id, node_id, _) = extraction_setup();
        database
            .connection
            .execute("UPDATE nodes SET identity_kind = 'unix_device_inode'", [])
            .expect("fixture should use an anchored Unix identity");
        let location_id: i64 = database
            .connection
            .query_row(
                "SELECT id FROM locations WHERE scope_id = ?1 AND node_id = ?2 AND present = 1",
                params![scope_id, node_id],
                |row| row.get(0),
            )
            .expect("target location should exist");
        let root_node_id = database
            .node_id_for_path_key(scope_id, "/scope")
            .expect("root should resolve")
            .expect("root should be present");
        let scan_id: i64 = database
            .connection
            .query_row(
                "SELECT MAX(id) FROM scan_jobs WHERE scope_id = ?1 AND status = 'completed'",
                [scope_id],
                |row| row.get(0),
            )
            .expect("completed scan should exist");
        database
            .connection
            .execute(
                "INSERT INTO nodes(kind, identity_kind, identity_key, created_at_unix_ms, updated_at_unix_ms) \
                 VALUES ('file', 'unix_device_inode', 'identity:/scope/sibling.txt', 1, 1)",
                [],
            )
            .expect("sibling node should exist");
        let sibling_node_id = database.connection.last_insert_rowid();
        database
            .connection
            .execute(
                "INSERT INTO files(node_id, size_bytes, modified_unix_ns, link_count) VALUES (?1, 9, 1, 1)",
                [sibling_node_id],
            )
            .expect("sibling file metadata should exist");
        database
            .connection
            .execute(
                "INSERT INTO locations(scope_id, node_id, path_raw, path_key, display_path, present, last_seen_scan_id) \
                 VALUES (?1, ?2, '/scope/sibling.txt', '/scope/sibling.txt', '/scope/sibling.txt', 1, ?3)",
                params![scope_id, sibling_node_id, scan_id],
            )
            .expect("sibling location should exist");
        database
            .connection
            .execute(
                "INSERT INTO edges(scope_id, source_node_id, target_node_id, kind, active, last_seen_scan_id) \
                 VALUES (?1, ?2, ?3, 'located_in', 1, ?4)",
                params![scope_id, sibling_node_id, root_node_id, scan_id],
            )
            .expect("sibling edge should exist");

        let extraction = database
            .create_extraction_job(scope_id, node_id)
            .expect("content extraction should queue");
        database
            .claim_extraction_job(extraction.job_id, "delta-content", 60_000)
            .expect("content extraction should claim");
        database
            .complete_extraction_job(
                extraction.job_id,
                "delta-content",
                "deskgraph.utf8-text",
                "1",
                4,
                Some(1),
                4,
                1,
                &[ContentChunkWrite {
                    ordinal: 0,
                    text: "abcd".to_string(),
                    provenance: ContentChunkProvenanceWrite::ByteRange { start: 0, end: 4 },
                    trust_class: "untrusted_extracted_text",
                }],
            )
            .expect("content should publish");
        database
            .connection
            .execute(
                "INSERT INTO image_metadata( \
                    scope_id, node_id, location_id, extraction_job_id, format, pixel_width, pixel_height, \
                    source_size_bytes, source_modified_unix_ns, provider_id, provider_version, active, created_at_unix_ms \
                 ) VALUES (?1, ?2, ?3, ?4, 'png', 1, 1, 4, 1, 'fixture', '1', 1, 1)",
                params![scope_id, node_id, location_id, extraction.job_id],
            )
            .expect("fixture image metadata should exist");

        let event = record_file_watch_event_with_kind(
            &mut database,
            scope_id,
            "/scope/file.txt",
            1_000,
            None,
            WatchReconciliationKind::FileDelta,
        );
        assert!(
            database
                .watch_file_delta_binding_at(
                    event.progress.event_id,
                    b"/wrong-parent",
                    "/wrong-parent",
                    1_000,
                )
                .expect("wrong parent lookup should not fail")
                .is_none(),
            "the parent raw and normalized key are both mandatory CAS inputs"
        );
        let binding = database
            .watch_file_delta_binding_at(event.progress.event_id, b"/scope", "/scope", 1_000)
            .expect("binding lookup should pass")
            .expect("single-link file should be eligible");
        let completed = database
            .publish_watch_file_delta_at(
                &binding,
                &WatchFileDeltaWrite {
                    snapshot: event.snapshot.clone(),
                },
                1_000,
            )
            .expect("bound delta should publish atomically");
        assert_eq!(completed.status, WatchEventStatus::Completed);
        assert!(completed.scan_job_id.is_none());
        let (new_size, new_modified): (i64, Option<i64>) = database
            .connection
            .query_row(
                "SELECT size_bytes, modified_unix_ns FROM files WHERE node_id = ?1",
                [node_id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .expect("target metadata should load");
        assert_eq!((new_size, new_modified), (7, Some(11)));
        let sibling_present: i64 = database
            .connection
            .query_row(
                "SELECT present FROM locations WHERE node_id = ?1",
                [sibling_node_id],
                |row| row.get(0),
            )
            .expect("sibling should remain present");
        assert_eq!(sibling_present, 1);
        assert_eq!(
            database
                .extraction_stats()
                .expect("stats should load")
                .active_chunk_count,
            0
        );
        let active_image_metadata: i64 = database
            .connection
            .query_row(
                "SELECT COUNT(*) FROM image_metadata WHERE node_id = ?1 AND active = 1",
                [node_id],
                |row| row.get(0),
            )
            .expect("image metadata should query");
        assert_eq!(active_image_metadata, 0);
    }

    #[test]
    fn file_delta_publish_rolls_back_when_manifest_metadata_changed_after_binding() {
        let (mut database, scope_id, node_id, _) = extraction_setup();
        database
            .connection
            .execute("UPDATE nodes SET identity_kind = 'unix_device_inode'", [])
            .expect("fixture should use an anchored Unix identity");
        let event = record_file_watch_event_with_kind(
            &mut database,
            scope_id,
            "/scope/file.txt",
            1_000,
            None,
            WatchReconciliationKind::FileDelta,
        );
        let binding = database
            .watch_file_delta_binding_at(event.progress.event_id, b"/scope", "/scope", 1_000)
            .expect("binding lookup should pass")
            .expect("single-link file should be eligible");
        database
            .connection
            .execute(
                "UPDATE files SET size_bytes = 6 WHERE node_id = ?1",
                [node_id],
            )
            .expect("fixture should invalidate the old metadata CAS");

        let error = database
            .publish_watch_file_delta_at(
                &binding,
                &WatchFileDeltaWrite {
                    snapshot: event.snapshot.clone(),
                },
                1_000,
            )
            .expect_err("changed manifest metadata must reject publication");
        assert!(matches!(
            error,
            DatabaseError::WatchFileDeltaSnapshotChanged
        ));
        assert_eq!(
            database
                .watch_event(event.progress.event_id)
                .expect("event should remain durable after rollback")
                .progress
                .status,
            WatchEventStatus::Stabilizing
        );
        let stored_size: i64 = database
            .connection
            .query_row(
                "SELECT size_bytes FROM files WHERE node_id = ?1",
                [node_id],
                |row| row.get(0),
            )
            .expect("fixture metadata should remain untouched");
        assert_eq!(stored_size, 6);
    }

    #[test]
    fn file_delta_publish_rejects_a_running_or_interrupted_scan_writer() {
        let (mut database, scope_id, node_id, _) = extraction_setup();
        database
            .connection
            .execute("UPDATE nodes SET identity_kind = 'unix_device_inode'", [])
            .expect("fixture should use an anchored Unix identity");
        let event = record_file_watch_event_with_kind(
            &mut database,
            scope_id,
            "/scope/file.txt",
            1_000,
            None,
            WatchReconciliationKind::FileDelta,
        );
        let binding = database
            .watch_file_delta_binding_at(event.progress.event_id, b"/scope", "/scope", 1_000)
            .expect("binding lookup should pass")
            .expect("single-link file should be eligible before a scan starts");
        let running_scan = database
            .create_scan_job(scope_id)
            .expect("concurrent full scan should start");
        assert_eq!(
            database
                .scan_job(running_scan)
                .expect("running scan should load")
                .status,
            ScanStatus::Running
        );
        assert!(
            database
                .watch_file_delta_binding_at(event.progress.event_id, b"/scope", "/scope", 1_000)
                .expect("binding lookup should pass")
                .is_none(),
            "a running scan owns the manifest writer path"
        );
        let error = database
            .publish_watch_file_delta_at(
                &binding,
                &WatchFileDeltaWrite {
                    snapshot: event.snapshot.clone(),
                },
                1_000,
            )
            .expect_err("a scan that starts after binding must reject the delta CAS");
        assert!(matches!(
            error,
            DatabaseError::WatchFileDeltaSnapshotChanged
        ));
        assert_eq!(
            database
                .watch_event(event.progress.event_id)
                .expect("event should survive rollback")
                .progress
                .status,
            WatchEventStatus::Stabilizing
        );
        let size: i64 = database
            .connection
            .query_row(
                "SELECT size_bytes FROM files WHERE node_id = ?1",
                [node_id],
                |row| row.get(0),
            )
            .expect("file metadata should remain available");
        assert_eq!(size, 4);
    }

    fn observation(path: &str, kind: NodeKind, parent: Option<Vec<u8>>) -> Observation {
        Observation {
            kind,
            identity_kind: "test_identity".to_string(),
            identity_key: format!("identity:{path}").into_bytes(),
            parent_identity_key: parent,
            path_raw: path.as_bytes().to_vec(),
            path_key: path.to_string(),
            display_path: path.to_string(),
            size_bytes: if kind == NodeKind::File { 4 } else { 0 },
            modified_unix_ns: Some(1),
            link_count: Some(1),
        }
    }

    fn publish_manifest_file(
        database: &mut ManifestDatabase,
        scope_id: i64,
        root: &QueuedPath,
        file_size: u64,
    ) -> i64 {
        let job = database
            .create_resumable_scan_job(scope_id, root)
            .expect("scan job should create");
        database
            .claim_scan_job(job.job_id, "scan-runner", 60_000)
            .expect("scan should claim");
        let root_entry = database
            .next_scan_queue_entry(job.job_id, "scan-runner", 60_000)
            .expect("queue should load")
            .expect("root should exist");
        let root_observation = observation("/scope", NodeKind::Folder, None);
        let child = QueuedPath {
            path_raw: b"/scope/file.txt".to_vec(),
            path_key: "/scope/file.txt".to_string(),
            parent_identity_key: Some(root_observation.identity_key.clone()),
            is_root: false,
        };
        database
            .stage_scan_queue_entry(
                job.job_id,
                "scan-runner",
                root_entry.id,
                Some(&root_observation),
                std::slice::from_ref(&child),
                &[],
                0,
                1,
                60_000,
            )
            .expect("root should stage");
        let child_entry = database
            .next_scan_queue_entry(job.job_id, "scan-runner", 60_000)
            .expect("queue should load")
            .expect("child should exist");
        let mut child_observation = observation(
            "/scope/file.txt",
            NodeKind::File,
            Some(root_observation.identity_key),
        );
        child_observation.size_bytes = file_size;
        database
            .stage_scan_queue_entry(
                job.job_id,
                "scan-runner",
                child_entry.id,
                Some(&child_observation),
                &[],
                &[],
                0,
                1,
                60_000,
            )
            .expect("child should stage");
        database
            .finalize_resumable_scan_job(job.job_id, "scan-runner")
            .expect("scan should publish");
        database
            .node_id_for_path_key(scope_id, "/scope/file.txt")
            .expect("node query should pass")
            .expect("file node should exist")
    }

    fn extraction_setup() -> (ManifestDatabase, i64, i64, QueuedPath) {
        extraction_setup_in(ManifestDatabase::open_in_memory().expect("database should initialize"))
    }

    fn extraction_setup_in(database: ManifestDatabase) -> (ManifestDatabase, i64, i64, QueuedPath) {
        let (mut database, scope_id, root) = resumable_setup_in(database);
        let node_id = publish_manifest_file(&mut database, scope_id, &root, 4);
        (database, scope_id, node_id, root)
    }

    fn synthetic_screenshot_group_source(
        node_id: i64,
        modified_unix_ns: i64,
        pixel_width: u32,
        pixel_height: u32,
    ) -> ScreenshotGroupSourceRecord {
        ScreenshotGroupSourceRecord {
            scope_id: 1,
            node_id,
            location_id: node_id + 100,
            image_metadata_id: node_id + 200,
            ocr_extraction_job_id: node_id + 300,
            size_bytes: 1_024,
            modified_unix_ns,
            format: ImageFormat::Png,
            pixel_width,
            pixel_height,
            ocr_chunk_count: 1,
            ocr_provider_id: "local-ocr".to_string(),
            ocr_provider_version: "1".to_string(),
        }
    }

    fn insert_screenshot_group_source(
        database: &ManifestDatabase,
        scope_id: i64,
        scan_id: i64,
        index: i64,
        ocr_provider_version: &str,
    ) -> i64 {
        let path = format!("/scope/screenshot-{index}.png");
        let identity = format!("screenshot-identity-{index}");
        database
            .connection
            .execute(
                "INSERT INTO nodes( \
                    kind, identity_kind, identity_key, created_at_unix_ms, updated_at_unix_ms \
                 ) VALUES ('file', 'test', ?1, 1, 1)",
                [identity.as_bytes()],
            )
            .expect("image node should persist");
        let node_id = database.connection.last_insert_rowid();
        let modified_unix_ns = 1_000_000_000 + index * 60_000_000_000;
        database
            .connection
            .execute(
                "INSERT INTO files(node_id, size_bytes, modified_unix_ns, link_count) \
                 VALUES (?1, 1024, ?2, 1)",
                params![node_id, modified_unix_ns],
            )
            .expect("image file facts should persist");
        database
            .connection
            .execute(
                "INSERT INTO locations( \
                    scope_id, node_id, path_raw, path_key, display_path, present, last_seen_scan_id \
                 ) VALUES (?1, ?2, ?3, ?4, ?4, 1, ?5)",
                params![scope_id, node_id, path.as_bytes(), path, scan_id],
            )
            .expect("image location should persist");
        let location_id = database.connection.last_insert_rowid();
        database
            .connection
            .execute(
                "INSERT INTO extraction_jobs( \
                    scope_id, node_id, location_id, status, provider_id, provider_version, \
                    source_size_bytes, source_modified_unix_ns, output_bytes, chunk_count, \
                    created_at_unix_ms, started_at_unix_ms, finished_at_unix_ms, updated_at_unix_ms \
                 ) VALUES (?1, ?2, ?3, 'completed', 'deskgraph.image-metadata', '1', \
                    1024, ?4, 0, 0, 1, 1, 1, 1)",
                params![scope_id, node_id, location_id, modified_unix_ns],
            )
            .expect("image metadata job should persist");
        let image_job_id = database.connection.last_insert_rowid();
        database
            .connection
            .execute(
                "INSERT INTO image_metadata( \
                    scope_id, node_id, location_id, extraction_job_id, format, pixel_width, \
                    pixel_height, source_size_bytes, source_modified_unix_ns, provider_id, \
                    provider_version, active, created_at_unix_ms \
                 ) VALUES (?1, ?2, ?3, ?4, 'png', 1440, 900, 1024, ?5, \
                    'deskgraph.image-metadata', '1', 1, 1)",
                params![
                    scope_id,
                    node_id,
                    location_id,
                    image_job_id,
                    modified_unix_ns
                ],
            )
            .expect("image metadata should persist");
        database
            .connection
            .execute(
                "INSERT INTO extraction_jobs( \
                    scope_id, node_id, location_id, status, provider_id, provider_version, \
                    source_size_bytes, source_modified_unix_ns, output_bytes, chunk_count, \
                    created_at_unix_ms, started_at_unix_ms, finished_at_unix_ms, updated_at_unix_ms, \
                    operation \
                 ) VALUES (?1, ?2, ?3, 'completed', 'local-ocr', ?4, 1024, ?5, \
                    1, 1, 1, 1, 1, 1, 'screenshot_ocr')",
                params![
                    scope_id,
                    node_id,
                    location_id,
                    ocr_provider_version,
                    modified_unix_ns
                ],
            )
            .expect("OCR job should persist");
        let ocr_job_id = database.connection.last_insert_rowid();
        database
            .connection
            .execute(
                "INSERT INTO content_chunks( \
                    scope_id, node_id, location_id, extraction_job_id, ordinal, text, \
                    provenance_kind, source_unit_number, source_fragment_index, \
                    source_bbox_x_ppm, source_bbox_y_ppm, source_bbox_width_ppm, \
                    source_bbox_height_ppm, source_confidence_basis_points, source_size_bytes, \
                    source_modified_unix_ns, trust_class, provider_id, provider_version, active, \
                    created_at_unix_ms \
                 ) VALUES (?1, ?2, ?3, ?4, 0, 'private OCR text', 'ocr_observation', \
                    1, 0, 0, 0, 1000000, 1000000, NULL, 1024, ?5, \
                    'untrusted_extracted_text', 'local-ocr', ?6, 1, 1)",
                params![
                    scope_id,
                    node_id,
                    location_id,
                    ocr_job_id,
                    modified_unix_ns,
                    ocr_provider_version
                ],
            )
            .expect("OCR provenance should persist");
        node_id
    }

    fn screenshot_group_setup() -> (ManifestDatabase, i64, Vec<ScreenshotGroupSourceRecord>) {
        screenshot_group_setup_in(
            ManifestDatabase::open_in_memory().expect("database should initialize"),
        )
    }

    fn screenshot_group_setup_in(
        mut database: ManifestDatabase,
    ) -> (ManifestDatabase, i64, Vec<ScreenshotGroupSourceRecord>) {
        let scope = database
            .add_scope(b"/scope", "/scope", "/scope", std::env::consts::OS)
            .expect("scope should persist");
        database
            .upsert_scope_access_grant(scope.id, std::env::consts::OS, b"test-grant")
            .expect("active test grant should persist");
        let scan_id = database
            .create_scan_job(scope.id)
            .expect("scan should start");
        database
            .complete_scan(scan_id, scope.id, &[], &[], 0, 0)
            .expect("scan should complete");

        for index in 0..2_i64 {
            insert_screenshot_group_source(&database, scope.id, scan_id, index, "1");
        }

        let sources = database
            .screenshot_group_sources(scope.id)
            .expect("current local screenshot sources should load");
        (database, scope.id, sources)
    }

    fn create_bound_rename_preview(
        database: &mut ManifestDatabase,
        scope_id: i64,
        node_id: i64,
    ) -> ActionPlanPreview {
        let execution_source = database
            .action_execution_source_for_path_key(scope_id, "/scope/file.txt")
            .expect("execution source should load from a completed manifest");
        let source = &execution_source.source;
        assert_eq!(source.node_id, node_id);
        let source_sha256 = [0xa5; 32];
        database
            .create_rename_action_plan(ActionPlanWrite {
                scope_id,
                node_id,
                source_location_id: source.location_id,
                source_path_raw: &source.path_raw,
                source_path_key: &source.path_key,
                source_display_path: &source.display_path,
                destination_path_raw: b"/scope/renamed.txt",
                destination_path_key: "/scope/renamed.txt",
                destination_display_path: "/scope/renamed.txt",
                source_identity_kind: &source.identity_kind,
                source_identity_key: &source.identity_key,
                source_size_bytes: source.size_bytes,
                source_modified_unix_ns: source.modified_unix_ns,
                source_sha256: &source_sha256,
                source_hash_bytes: source.size_bytes,
                scope_root_identity_kind: &execution_source.scope_root_identity_kind,
                scope_root_identity_key: &execution_source.scope_root_identity_key,
                parent_identity_kind: &execution_source.parent_identity_kind,
                parent_identity_key: &execution_source.parent_identity_key,
                execution_strategy: ActionExecutionStrategy::Direct,
            })
            .expect("preview and binding should persist")
    }

    fn assert_action_safety_record_blocks_privacy_purge(
        database: &mut ManifestDatabase,
        scope_id: i64,
        plan_id: i64,
    ) {
        let binding = database
            .bind_scope_policy_revision(scope_id)
            .expect("active policy binding should load");
        let write = ScopeExclusionWrite {
            kind: ScopeExclusionKind::File,
            path_raw: b"/scope/renamed.txt",
            path_key: "/scope/renamed.txt",
            display_path: "/scope/renamed.txt",
            identity_kind: TEST_EXCLUDED_IDENTITY_KIND,
            identity_key: TEST_EXCLUDED_FILE_IDENTITY,
        };
        let event_count_before: i64 = database
            .connection
            .query_row(
                "SELECT COUNT(*) FROM action_journal_events WHERE plan_id=?1",
                [plan_id],
                |row| row.get(0),
            )
            .expect("journal count should load");
        let location_count_before: i64 = database
            .connection
            .query_row(
                "SELECT COUNT(*) FROM locations WHERE scope_id=?1",
                [scope_id],
                |row| row.get(0),
            )
            .expect("location count should load");
        let preview = database
            .preview_scope_exclusion_batch(binding, &[write])
            .expect("impact preview should remain available");
        assert_eq!(preview.action_plan_count, 1);
        assert_eq!(preview.blocking_action_count, 1);
        assert!(matches!(
            database.apply_scope_exclusion_batch(binding, &[write], 10),
            Err(DatabaseError::ScopePrivacyPurgeBlocked)
        ));
        assert_eq!(
            database
                .current_scope_policy_revision(scope_id)
                .expect("revision should load")
                .revision,
            1
        );
        assert!(database.scope_exclusions(scope_id).unwrap().is_empty());
        assert_eq!(
            database
                .connection
                .query_row(
                    "SELECT COUNT(*) FROM action_plans WHERE id=?1",
                    [plan_id],
                    |row| row.get::<_, i64>(0)
                )
                .unwrap(),
            1
        );
        assert_eq!(
            database
                .connection
                .query_row(
                    "SELECT COUNT(*) FROM action_journal_events WHERE plan_id=?1",
                    [plan_id],
                    |row| row.get::<_, i64>(0)
                )
                .unwrap(),
            event_count_before
        );
        assert_eq!(
            database
                .connection
                .query_row(
                    "SELECT COUNT(*) FROM locations WHERE scope_id=?1",
                    [scope_id],
                    |row| row.get::<_, i64>(0)
                )
                .unwrap(),
            location_count_before
        );
        for table in ["privacy_purge_receipts", "privacy_purge_capabilities"] {
            let sql = format!("SELECT COUNT(*) FROM {table}");
            assert_eq!(
                database
                    .connection
                    .query_row(&sql, [], |row| row.get::<_, i64>(0))
                    .unwrap(),
                0,
                "{table} must roll back"
            );
        }
    }

    fn acquire_test_executor_lease(database: &mut ManifestDatabase, plan_id: i64) {
        database
            .acquire_action_executor_lease(plan_id, "test_executor_0001", 60_000)
            .expect("executor lease should be claimable");
    }

    fn insert_terminal_action_fixture(
        database: &mut ManifestDatabase,
        source_plan_id: i64,
        request_id: &str,
    ) {
        database
            .connection
            .execute(
                "INSERT INTO action_plans( \
                     api_version, policy_version, operation, execution_strategy, scope_id, node_id, \
                     source_location_id, source_path_raw, source_path_key, source_display_path, \
                     destination_path_raw, destination_path_key, destination_display_path, \
                     source_identity_kind, source_identity_key, source_size_bytes, \
                     source_modified_unix_ns, created_at_unix_ms \
                 ) SELECT \
                     api_version, policy_version, operation, execution_strategy, scope_id, node_id, \
                     source_location_id, source_path_raw, source_path_key, source_display_path, \
                     destination_path_raw, destination_path_key, destination_display_path, \
                     source_identity_kind, source_identity_key, source_size_bytes, \
                     source_modified_unix_ns, created_at_unix_ms \
                 FROM action_plans WHERE id = ?1",
                [source_plan_id],
            )
            .expect("terminal plan fixture should clone immutable plan data");
        let plan_id = database.connection.last_insert_rowid();
        database
            .connection
            .execute(
                "INSERT INTO action_execution_bindings( \
                     plan_id, api_version, source_hash_bytes, source_sha256, scope_root_node_id, \
                     scope_root_identity_kind, scope_root_identity_key, parent_node_id, \
                     parent_identity_kind, parent_identity_key, created_at_unix_ms \
                 ) SELECT \
                     ?1, api_version, source_hash_bytes, source_sha256, scope_root_node_id, \
                     scope_root_identity_kind, scope_root_identity_key, parent_node_id, \
                     parent_identity_kind, parent_identity_key, created_at_unix_ms \
                 FROM action_execution_bindings WHERE plan_id = ?2",
                params![plan_id, source_plan_id],
            )
            .expect("terminal plan fixture should clone binding");
        database
            .connection
            .execute(
                "INSERT INTO action_journal_events( \
                     api_version, plan_id, sequence, event_kind, command_request_id, created_at_unix_ms \
                 ) VALUES ('deskgraph.action-journal.v1', ?1, 1, 'preview_created', NULL, 1)",
                [plan_id],
            )
            .expect("fixture preview should persist");
        database
            .connection
            .execute(
                "INSERT INTO action_command_requests( \
                     plan_id, request_id, command_kind, requested_sequence, created_at_unix_ms \
                 ) VALUES (?1, ?2, 'execute', 2, 1)",
                params![plan_id, request_id],
            )
            .expect("fixture command should persist");
        let command_id = database.connection.last_insert_rowid();
        for (sequence, event_kind) in [
            (2_i64, "execute_requested"),
            (3, "direct_rename_intent"),
            (4, "execution_completed"),
        ] {
            database
                .connection
                .execute(
                    "INSERT INTO action_journal_events( \
                         api_version, plan_id, sequence, event_kind, command_request_id, created_at_unix_ms \
                     ) VALUES ('deskgraph.action-journal.v1', ?1, ?2, ?3, ?4, 1)",
                    params![plan_id, sequence, event_kind, command_id],
                )
                .expect("terminal fixture journal should persist");
        }
    }

    fn insert_legacy_preview_fixture(database: &mut ManifestDatabase, source_plan_id: i64) {
        database
            .connection
            .execute(
                "INSERT INTO action_plans( \
                     api_version, policy_version, operation, execution_strategy, scope_id, node_id, \
                     source_location_id, source_path_raw, source_path_key, source_display_path, \
                     destination_path_raw, destination_path_key, destination_display_path, \
                     source_identity_kind, source_identity_key, source_size_bytes, \
                     source_modified_unix_ns, created_at_unix_ms \
                 ) SELECT \
                     api_version, policy_version, operation, execution_strategy, scope_id, node_id, \
                     source_location_id, source_path_raw, source_path_key, source_display_path, \
                     destination_path_raw, destination_path_key, destination_display_path, \
                     source_identity_kind, source_identity_key, source_size_bytes, \
                     source_modified_unix_ns, created_at_unix_ms \
                 FROM action_plans WHERE id = ?1",
                [source_plan_id],
            )
            .expect("legacy fixture should clone plan without a binding");
        let plan_id = database.connection.last_insert_rowid();
        database
            .connection
            .execute(
                "INSERT INTO action_journal_events( \
                     api_version, plan_id, sequence, event_kind, command_request_id, created_at_unix_ms \
                 ) VALUES ('deskgraph.action-journal.v1', ?1, 1, 'preview_created', NULL, 1)",
                [plan_id],
            )
            .expect("legacy preview event should persist");
    }

    fn project_setup() -> (ManifestDatabase, i64, i64) {
        let (mut database, scope_id, root) = resumable_setup();
        let job = database
            .create_resumable_scan_job(scope_id, &root)
            .expect("scan job should create");
        database
            .claim_scan_job(job.job_id, "project-scan", 60_000)
            .expect("scan should claim");
        let root_entry = database
            .next_scan_queue_entry(job.job_id, "project-scan", 60_000)
            .expect("queue should load")
            .expect("root should exist");
        let root_observation = observation("/scope", NodeKind::Folder, None);
        let child = QueuedPath {
            path_raw: b"/scope/Cargo.toml".to_vec(),
            path_key: "/scope/Cargo.toml".to_string(),
            parent_identity_key: Some(root_observation.identity_key.clone()),
            is_root: false,
        };
        database
            .stage_scan_queue_entry(
                job.job_id,
                "project-scan",
                root_entry.id,
                Some(&root_observation),
                std::slice::from_ref(&child),
                &[],
                0,
                1,
                60_000,
            )
            .expect("root should stage");
        let child_entry = database
            .next_scan_queue_entry(job.job_id, "project-scan", 60_000)
            .expect("queue should load")
            .expect("child should exist");
        let cargo_observation = observation(
            "/scope/Cargo.toml",
            NodeKind::File,
            Some(root_observation.identity_key),
        );
        database
            .stage_scan_queue_entry(
                job.job_id,
                "project-scan",
                child_entry.id,
                Some(&cargo_observation),
                &[],
                &[],
                0,
                1,
                60_000,
            )
            .expect("marker should stage");
        database
            .finalize_resumable_scan_job(job.job_id, "project-scan")
            .expect("scan should publish");
        let root_node_id = database
            .node_id_for_path_key(scope_id, "/scope")
            .expect("node query should pass")
            .expect("root node should exist");
        (database, scope_id, root_node_id)
    }

    fn exact_duplicate_setup() -> (ManifestDatabase, ActionSourceRecord, ActionSourceRecord) {
        let database = ManifestDatabase::open_in_memory().expect("database should initialize");
        let scope = database
            .add_scope(b"/scope", "/scope", "/scope", "macos")
            .expect("scope should persist");
        database
            .connection
            .execute(
                "INSERT INTO scan_jobs( \
                     scope_id, status, discovered_files, discovered_folders, started_at_unix_ms, \
                     finished_at_unix_ms \
                 ) VALUES (?1, 'completed', 2, 0, 1, 1)",
                [scope.id],
            )
            .expect("scan should persist");
        let scan_id = database.connection.last_insert_rowid();
        for (path, identity) in [
            ("/scope/left.bin", b"identity:left".as_slice()),
            ("/scope/right.bin", b"identity:right".as_slice()),
        ] {
            database
                .connection
                .execute(
                    "INSERT INTO nodes( \
                         kind, identity_kind, identity_key, created_at_unix_ms, updated_at_unix_ms \
                     ) VALUES ('file', 'test_identity', ?1, 1, 1)",
                    [identity],
                )
                .expect("node should persist");
            let node_id = database.connection.last_insert_rowid();
            database
                .connection
                .execute(
                    "INSERT INTO files(node_id, size_bytes, modified_unix_ns, link_count) \
                     VALUES (?1, 4, 2, 1)",
                    [node_id],
                )
                .expect("file should persist");
            database
                .connection
                .execute(
                    "INSERT INTO locations( \
                         scope_id, node_id, path_raw, path_key, display_path, present, \
                         last_seen_scan_id \
                     ) VALUES (?1, ?2, ?3, ?4, ?4, 1, ?5)",
                    params![scope.id, node_id, path.as_bytes(), path, scan_id],
                )
                .expect("location should persist");
        }
        let left = database
            .action_source_for_path_key(scope.id, "/scope/left.bin")
            .expect("left should load");
        let right = database
            .action_source_for_path_key(scope.id, "/scope/right.bin")
            .expect("right should load");
        (database, left, right)
    }

    fn cleanup_exact_duplicate_setup() -> (
        ManifestDatabase,
        CleanupActionSelection,
        ActionExecutionSourceRecord,
        ActionExecutionSourceRecord,
    ) {
        let (mut database, left, right) = exact_duplicate_setup();
        database
            .upsert_scope_access_grant(left.scope_id, "macos", b"test-active-grant")
            .expect("scope grant should activate");
        let scan_id: i64 = database
            .connection
            .query_row(
                "SELECT MAX(id) FROM scan_jobs WHERE scope_id = ?1 AND status = 'completed'",
                [left.scope_id],
                |row| row.get(0),
            )
            .expect("completed scan should exist");
        database
            .connection
            .execute(
                "INSERT INTO nodes( \
                     kind, identity_kind, identity_key, created_at_unix_ms, updated_at_unix_ms \
                 ) VALUES ('folder', 'test_identity', X'6964656E746974793A726F6F74', 1, 1)",
                [],
            )
            .expect("root node should persist");
        let root_node_id = database.connection.last_insert_rowid();
        database
            .connection
            .execute("INSERT INTO folders(node_id) VALUES (?1)", [root_node_id])
            .expect("root folder should persist");
        database
            .connection
            .execute(
                "INSERT INTO locations( \
                     scope_id, node_id, path_raw, path_key, display_path, present, last_seen_scan_id \
                 ) VALUES (?1, ?2, '/scope', '/scope', '/scope', 1, ?3)",
                params![left.scope_id, root_node_id, scan_id],
            )
            .expect("root location should persist");
        for node_id in [left.node_id, right.node_id] {
            database
                .connection
                .execute(
                    "INSERT INTO edges( \
                         scope_id, source_node_id, target_node_id, kind, active, last_seen_scan_id \
                     ) VALUES (?1, ?2, ?3, 'located_in', 1, ?4)",
                    params![left.scope_id, node_id, root_node_id, scan_id],
                )
                .expect("parent edge should persist");
        }
        let candidate = database
            .record_exact_duplicate_candidate(&left, &right)
            .expect("duplicate evidence should persist");
        let item = database
            .smart_cleanup_relation_item(
                candidate.relation_id,
                candidate.evidence.observed_at_unix_ms,
            )
            .expect("current observation should map");
        let selection = CleanupActionSelection {
            scope_id: left.scope_id,
            source_kind: SmartCleanupSourceKind::ExactDuplicate,
            source_id: candidate.relation_id,
            source_observation_id: item.source_observation_id,
            keeper_node_id: Some(left.node_id),
            target_node_id: right.node_id,
        };
        let (source, keeper) = database
            .cleanup_action_sources(selection)
            .expect("current selected members should resolve");
        (
            database,
            selection,
            source,
            keeper.expect("exact duplicate should bind a keeper"),
        )
    }

    fn cleanup_exact_duplicate_plan_write<'a>(
        selection: CleanupActionSelection,
        source: &'a ActionExecutionSourceRecord,
        keeper: &'a ActionExecutionSourceRecord,
        target_sha256: &'a [u8],
        keeper_sha256: &'a [u8],
    ) -> CleanupActionPlanWrite<'a> {
        CleanupActionPlanWrite {
            selection,
            keeper: Some(CleanupKeeperBindingWrite {
                location_id: keeper.source.location_id,
                identity_kind: &keeper.source.identity_kind,
                identity_key: &keeper.source.identity_key,
                size_bytes: keeper.source.size_bytes,
                modified_unix_ns: keeper.source.modified_unix_ns,
                sha256: keeper_sha256,
                hash_bytes: keeper.source.size_bytes,
                scope_root_node_id: keeper.scope_root_node_id,
                scope_root_identity_kind: &keeper.scope_root_identity_kind,
                scope_root_identity_key: &keeper.scope_root_identity_key,
                parent_node_id: keeper.parent_node_id,
                parent_identity_kind: &keeper.parent_identity_kind,
                parent_identity_key: &keeper.parent_identity_key,
            }),
            target_location_id: source.source.location_id,
            target_identity_kind: &source.source.identity_kind,
            target_identity_key: &source.source.identity_key,
            target_size_bytes: source.source.size_bytes,
            target_modified_unix_ns: source.source.modified_unix_ns,
            target_sha256,
            target_hash_bytes: source.source.size_bytes,
            scope_root_node_id: source.scope_root_node_id,
            scope_root_identity_kind: &source.scope_root_identity_kind,
            scope_root_identity_key: &source.scope_root_identity_key,
            parent_node_id: source.parent_node_id,
            parent_identity_kind: &source.parent_identity_kind,
            parent_identity_key: &source.parent_identity_key,
        }
    }

    fn file_version_setup() -> (ManifestDatabase, ActionSourceRecord, ActionSourceRecord) {
        let database = ManifestDatabase::open_in_memory().expect("database should initialize");
        let scope = database
            .add_scope(b"/scope", "/scope", "/scope", "macos")
            .expect("scope should persist");
        database
            .connection
            .execute(
                "INSERT INTO scan_jobs( \
                     scope_id, status, discovered_files, discovered_folders, started_at_unix_ms, \
                     finished_at_unix_ms \
                 ) VALUES (?1, 'completed', 2, 0, 1, 1)",
                [scope.id],
            )
            .expect("scan should persist");
        let scan_id = database.connection.last_insert_rowid();
        for (path, identity, size) in [
            ("/scope/企劃-v1.md", b"identity:v1".as_slice(), 4_i64),
            ("/scope/企劃_V2.MD", b"identity:v2".as_slice(), 6_i64),
        ] {
            database
                .connection
                .execute(
                    "INSERT INTO nodes( \
                         kind, identity_kind, identity_key, created_at_unix_ms, updated_at_unix_ms \
                     ) VALUES ('file', 'test_identity', ?1, 1, 1)",
                    [identity],
                )
                .expect("node should persist");
            let node_id = database.connection.last_insert_rowid();
            database
                .connection
                .execute(
                    "INSERT INTO files(node_id, size_bytes, modified_unix_ns, link_count) \
                     VALUES (?1, ?2, 2, 1)",
                    params![node_id, size],
                )
                .expect("file should persist");
            database
                .connection
                .execute(
                    "INSERT INTO locations( \
                         scope_id, node_id, path_raw, path_key, display_path, present, \
                         last_seen_scan_id \
                     ) VALUES (?1, ?2, ?3, ?4, ?4, 1, ?5)",
                    params![scope.id, node_id, path.as_bytes(), path, scan_id],
                )
                .expect("location should persist");
        }
        let first = database
            .action_source_for_path_key(scope.id, "/scope/企劃-v1.md")
            .expect("first version should load");
        let second = database
            .action_source_for_path_key(scope.id, "/scope/企劃_V2.MD")
            .expect("second version should load");
        (database, first, second)
    }

    #[test]
    fn migrations_initialize_manifest_schema() {
        let database = ManifestDatabase::open_in_memory().expect("database should initialize");
        let stats = database.stats().expect("stats should be readable");

        assert!(stats.database_ready);
        assert_eq!(stats.authorized_scope_count, 0);
        assert_eq!(stats.node_count, 0);
        assert_eq!(
            database
                .connection
                .query_row("SELECT COUNT(*) FROM schema_migrations", [], |row| {
                    row.get::<_, i64>(0)
                })
                .expect("migration count should load"),
            i64::try_from(MIGRATIONS.len()).expect("migration count should fit")
        );
        assert_eq!(
            database
                .connection
                .query_row(
                    "SELECT COUNT(*) FROM sqlite_schema \
                     WHERE (type = 'table' AND name = 'file_version_feedback_events') \
                        OR (type = 'trigger' AND name IN ( \
                            'file_version_feedback_events_immutable_update', \
                            'file_version_feedback_events_immutable_delete' \
                        ))",
                    [],
                    |row| row.get::<_, i64>(0),
                )
                .expect("version feedback schema should load"),
            3
        );
    }

    #[test]
    fn folder_search_index_migration_preserves_a_populated_pre_0027_manifest() {
        let directory = tempfile::tempdir().expect("tempdir should exist");
        let path = directory.path().join("populated-pre-folder-index.sqlite3");
        let connection = Connection::open(&path).expect("legacy database should open");
        apply_migration_prefix(&connection, 26);
        connection
            .execute(
                "INSERT INTO authorized_scopes( \
                     id,path_raw,path_key,display_path,platform,created_at_unix_ms \
                 ) VALUES(1,X'2F73636F7065','/scope','/scope',?1,0)",
                [std::env::consts::OS],
            )
            .expect("legacy scope should persist");
        connection
            .execute_batch(
                "INSERT INTO scan_jobs( \
                     id,scope_id,status,discovered_files,discovered_folders, \
                     started_at_unix_ms,finished_at_unix_ms \
                 ) VALUES(1,1,'completed',1,2,0,0); \
                 INSERT INTO nodes(id,kind,identity_kind,identity_key,created_at_unix_ms,updated_at_unix_ms) \
                 VALUES(1,'folder','test',X'01',0,0), \
                       (2,'folder','test',X'02',0,0), \
                       (3,'file','test',X'03',0,0); \
                 INSERT INTO folders(node_id) VALUES(1),(2); \
                 INSERT INTO files(node_id,size_bytes,modified_unix_ns,link_count) VALUES(3,4,1,1); \
                 INSERT INTO locations( \
                     id,scope_id,node_id,path_raw,path_key,display_path,present,last_seen_scan_id \
                 ) VALUES(1,1,1,X'2F73636F7065','/scope','/scope',1,1), \
                         (2,1,2,X'2F73636F70652F666F6C646572','/scope/folder','/scope/folder',1,1), \
                         (3,1,3,X'2F73636F70652F666F6C6465722F66696C652E747874','/scope/folder/file.txt','/scope/folder/file.txt',1,1); \
                 INSERT INTO edges( \
                     id,scope_id,source_node_id,target_node_id,kind,active,last_seen_scan_id \
                 ) VALUES(1,1,2,1,'located_in',1,1), \
                         (2,1,3,2,'located_in',1,1);",
            )
            .expect("populated legacy manifest should persist");
        drop(connection);

        let database = ManifestDatabase::open(&path).expect("0027 migration should apply");
        assert_eq!(
            database
                .connection
                .query_row("SELECT COUNT(*) FROM locations", [], |row| row
                    .get::<_, i64>(0))
                .expect("locations should count"),
            3
        );
        assert_eq!(
            database
                .connection
                .query_row("SELECT COUNT(*) FROM edges", [], |row| row.get::<_, i64>(0))
                .expect("edges should count"),
            2
        );
        assert_eq!(
            database
                .connection
                .query_row(
                    "SELECT COUNT(*) FROM schema_migrations \
                     WHERE version=27 AND name='folder_search_descendant_index'",
                    [],
                    |row| row.get::<_, i64>(0),
                )
                .expect("0027 registry row should load"),
            1
        );
        let mut index_statement = database
            .connection
            .prepare(
                "SELECT name FROM pragma_index_info( \
                     'edges_scope_kind_active_target_source_idx' \
                 ) ORDER BY seqno",
            )
            .expect("folder traversal index should inspect");
        let index_columns = index_statement
            .query_map([], |row| row.get::<_, String>(0))
            .expect("index columns should load")
            .collect::<Result<Vec<_>, _>>()
            .expect("index columns should decode");
        assert_eq!(
            index_columns,
            [
                "scope_id",
                "kind",
                "active",
                "target_node_id",
                "source_node_id"
            ]
        );
    }

    #[test]
    fn scope_exclusion_migration_upgrades_an_empty_pre_0024_database() {
        let directory = tempfile::tempdir().expect("tempdir should exist");
        let path = directory.path().join("pre-scope-exclusions-empty.sqlite3");
        let connection = Connection::open(&path).expect("legacy database should open");
        apply_migrations_before_scope_exclusions(&connection);
        drop(connection);

        let database = ManifestDatabase::open(&path).expect("privacy migrations should apply");
        assert_eq!(
            database
                .connection
                .query_row(
                    "SELECT COUNT(*) FROM schema_migrations WHERE version=24 AND name='scope_exclusions_and_privacy_purge'",
                    [],
                    |row| row.get::<_, i64>(0),
                )
                .expect("0024 registry row should load"),
            1
        );
        assert_eq!(
            database
                .connection
                .query_row(
                    "SELECT COUNT(*) FROM schema_migrations WHERE version=25 AND name='scope_root_revocation'",
                    [],
                    |row| row.get::<_, i64>(0),
                )
                .expect("0025 registry row should load"),
            1
        );
        assert_eq!(
            database
                .connection
                .query_row(
                    "SELECT COUNT(*) FROM schema_migrations WHERE version=26 AND name='scope_root_revocation_hardening'",
                    [],
                    |row| row.get::<_, i64>(0),
                )
                .expect("0026 registry row should load"),
            1
        );
        assert_eq!(
            database
                .connection
                .query_row(
                    "SELECT COUNT(*) FROM schema_migrations WHERE version=27 AND name='folder_search_descendant_index'",
                    [],
                    |row| row.get::<_, i64>(0),
                )
                .expect("0027 registry row should load"),
            1
        );
        assert_eq!(
            database
                .connection
                .query_row(
                    "SELECT COUNT(*) FROM pragma_index_info('edges_scope_kind_active_target_source_idx')",
                    [],
                    |row| row.get::<_, i64>(0),
                )
                .expect("folder traversal index should load"),
            5
        );
        assert_eq!(
            database
                .connection
                .query_row(
                    "SELECT COUNT(*) FROM sqlite_schema WHERE type='table' AND name='scope_root_revocation_receipts'",
                    [],
                    |row| row.get::<_, i64>(0),
                )
                .expect("root revocation receipt table should load"),
            1
        );
        assert_eq!(
            database
                .connection
                .query_row(
                    "SELECT COUNT(*) FROM sqlite_schema WHERE type='trigger' AND name IN ( \
                         'scope_access_grants_revocation_tombstone_update', \
                         'scope_access_grants_revocation_tombstone_insert', \
                         'scope_access_grants_revocation_privacy_capability_update' \
                     )",
                    [],
                    |row| row.get::<_, i64>(0),
                )
                .expect("root revocation grant guards should load"),
            3
        );
        assert_eq!(
            database
                .connection
                .query_row(
                    "SELECT COUNT(*) FROM sqlite_schema WHERE \
                         (type='table' AND name='scope_filesystem_fence_identities') \
                         OR (type='trigger' AND name IN ( \
                             'scope_filesystem_fence_identities_immutable_update', \
                             'scope_filesystem_fence_identities_immutable_delete' \
                         ))",
                    [],
                    |row| row.get::<_, i64>(0),
                )
                .expect("stable fence identity schema should load"),
            3
        );
        assert_eq!(
            database
                .connection
                .query_row(
                    "SELECT (SELECT COUNT(*) FROM pragma_table_info('authorized_scopes') WHERE name='policy_revision') \
                          + (SELECT COUNT(*) FROM pragma_table_info('scan_jobs') WHERE name='policy_revision') \
                          + (SELECT COUNT(*) FROM pragma_table_info('extraction_jobs') WHERE name='policy_revision') \
                          + (SELECT COUNT(*) FROM pragma_table_info('watch_events') WHERE name='policy_revision') \
                          + (SELECT COUNT(*) FROM pragma_table_info('action_plans') WHERE name='policy_revision') \
                          + (SELECT COUNT(*) FROM pragma_table_info('cleanup_action_plans') WHERE name='policy_revision')",
                    [],
                    |row| row.get::<_, i64>(0),
                )
                .expect("revision columns should load"),
            6
        );
        assert_eq!(
            database
                .connection
                .query_row(
                    "SELECT COUNT(*) FROM sqlite_schema WHERE type='table' AND name IN ( \
                         'scope_exclusions','privacy_purge_capabilities','privacy_purge_location_targets', \
                         'privacy_purge_node_targets','privacy_purge_project_targets', \
                         'privacy_purge_action_plan_targets','privacy_purge_relation_targets', \
                         'privacy_purge_screenshot_group_targets', \
                         'privacy_purge_cleanup_action_plan_targets','privacy_purge_receipts')",
                    [],
                    |row| row.get::<_, i64>(0),
                )
                .expect("privacy tables should load"),
            10
        );
        assert_eq!(
            database
                .connection
                .query_row(
                    "SELECT (SELECT COUNT(*) FROM pragma_table_info('scope_exclusions') \
                             WHERE name IN ('identity_kind','identity_key')) \
                          + (SELECT COUNT(*) FROM sqlite_schema \
                             WHERE type='index' AND name='scope_exclusions_scope_identity_idx')",
                    [],
                    |row| row.get::<_, i64>(0),
                )
                .expect("durable exclusion identity schema should load"),
            3
        );
    }

    #[test]
    fn root_revocation_hardening_backfills_legacy_revoked_capability_bytes() {
        let directory = tempfile::tempdir().expect("tempdir should exist");
        let path = directory.path().join("pre-root-hardening.sqlite3");
        let connection = Connection::open(&path).expect("legacy database should open");
        connection
            .execute_batch(
                "CREATE TABLE schema_migrations ( \
                     version INTEGER PRIMARY KEY, \
                     name TEXT NOT NULL, \
                     checksum TEXT NOT NULL, \
                     applied_at_unix_ms INTEGER NOT NULL \
                 );",
            )
            .expect("migration registry should initialize");
        for migration in &MIGRATIONS[..25] {
            connection
                .execute_batch(migration.sql)
                .expect("pre-hardening migration should apply");
            connection
                .execute(
                    "INSERT INTO schema_migrations(version, name, checksum, applied_at_unix_ms) \
                     VALUES (?1, ?2, ?3, 0)",
                    params![
                        migration.version,
                        migration.name,
                        migration_checksum(migration.sql)
                    ],
                )
                .expect("pre-hardening migration should register");
        }
        connection
            .execute(
                "INSERT INTO authorized_scopes( \
                     id, path_raw, path_key, display_path, platform, created_at_unix_ms \
                 ) VALUES (1, X'2F6C6567616379', '/legacy', '/legacy', ?1, 0)",
                [std::env::consts::OS],
            )
            .expect("legacy scope should persist");
        connection
            .execute(
                "INSERT INTO scope_access_grants( \
                     scope_id, platform, opaque_grant, state, updated_at_unix_ms \
                 ) VALUES (1, ?1, X'6C65676163792D626F6F6B6D61726B', 'revoked', 0)",
                [std::env::consts::OS],
            )
            .expect("legacy revoked capability bytes should persist before hardening");
        drop(connection);

        let database = ManifestDatabase::open(&path).expect("hardening migration should apply");
        let grant = database
            .scope_access_grant(1)
            .expect("legacy grant should load")
            .expect("legacy grant row should remain");
        assert_eq!(grant.state, ScopeAccessGrantState::Revoked);
        assert_eq!(grant.opaque_grant, [0]);
    }

    #[test]
    fn scope_exclusion_migration_backfills_every_pre_0024_policy_owner() {
        let directory = tempfile::tempdir().expect("tempdir should exist");
        let path = directory
            .path()
            .join("pre-scope-exclusions-populated.sqlite3");
        let connection = Connection::open(&path).expect("legacy database should open");
        apply_migrations_before_scope_exclusions(&connection);
        connection
            .execute(
                "INSERT INTO authorized_scopes(id,path_raw,path_key,display_path,platform,created_at_unix_ms) \
                 VALUES(1,X'2F73636F7065','/scope','/scope',?1,0)",
                [std::env::consts::OS],
            )
            .expect("legacy scope should persist");
        connection
            .execute(
                "INSERT INTO scope_access_grants(scope_id,platform,opaque_grant,state,updated_at_unix_ms) \
                 VALUES(1,?1,X'6772616E74','active',0)",
                [std::env::consts::OS],
            )
            .expect("legacy active grant should persist");
        connection
            .execute_batch(
                "INSERT INTO scan_jobs(id,scope_id,status,started_at_unix_ms,finished_at_unix_ms) \
                     VALUES(1,1,'completed',0,1); \
                 INSERT INTO nodes(id,kind,identity_kind,identity_key,created_at_unix_ms,updated_at_unix_ms) VALUES \
                     (1,'folder','test',X'01',0,0),(2,'file','test',X'02',0,0); \
                 INSERT INTO folders(node_id) VALUES(1); \
                 INSERT INTO files(node_id,size_bytes,modified_unix_ns,link_count) VALUES(2,4,1,1); \
                 INSERT INTO locations(id,scope_id,node_id,path_raw,path_key,display_path,present,last_seen_scan_id) VALUES \
                     (1,1,1,X'2F73636F7065','/scope','/scope',1,1), \
                     (2,1,2,X'2F73636F70652F66696C652E747874','/scope/file.txt','/scope/file.txt',1,1); \
                 INSERT INTO edges(scope_id,source_node_id,target_node_id,kind,active,last_seen_scan_id) \
                     VALUES(1,2,1,'located_in',1,1); \
                 INSERT INTO extraction_jobs(id,scope_id,node_id,location_id,status,source_size_bytes,created_at_unix_ms,updated_at_unix_ms) \
                     VALUES(1,1,2,2,'completed',4,0,1); \
                 INSERT INTO watch_events(id,scope_id,status,path_raw,path_key,observed_kind,observed_size_bytes, \
                     observed_modified_unix_ns,observed_identity_key,observation_count,stable_after_unix_ms,created_at_unix_ms,updated_at_unix_ms) \
                     VALUES(1,1,'completed',X'2F73636F70652F66696C652E747874','/scope/file.txt','file',4,1,X'02',1,1,0,1); \
                 INSERT INTO action_plans(id,api_version,policy_version,operation,execution_strategy,scope_id,node_id, \
                     source_location_id,source_path_raw,source_path_key,source_display_path,destination_path_raw, \
                     destination_path_key,destination_display_path,source_identity_kind,source_identity_key, \
                     source_size_bytes,source_modified_unix_ns,created_at_unix_ms) \
                     VALUES(1,'deskgraph.action-plan.v1','deskgraph.action-policy.v1','rename','direct',1,2,2, \
                     X'2F73636F70652F66696C652E747874','/scope/file.txt','/scope/file.txt', \
                     X'2F73636F70652F72656E616D65642E747874','/scope/renamed.txt','/scope/renamed.txt','test',X'02',4,1,0); \
                 INSERT INTO action_journal_events(api_version,plan_id,sequence,event_kind,command_request_id,created_at_unix_ms) \
                     VALUES('deskgraph.action-journal.v1',1,1,'preview_created',NULL,0); \
                 INSERT INTO cleanup_action_plans(id,api_version,policy_version,operation,state,scope_id,source_kind, \
                     source_id,source_observation_id,target_node_id,target_location_id,target_identity_kind,target_identity_key, \
                     target_size_bytes,target_modified_unix_ns,target_sha256,target_hash_bytes,scope_root_node_id, \
                     scope_root_identity_kind,scope_root_identity_key,parent_node_id,parent_identity_kind,parent_identity_key, \
                     confirmation_required,action_authorized,execution_available,created_at_unix_ms) \
                     VALUES(1,'deskgraph.cleanup-action-plan.v1','deskgraph.cleanup-action-policy.v1','system_trash_preview', \
                     'previewed',1,'screenshot_review_group',1,1,2,2,'test',X'02',4,1,zeroblob(32),4,1, \
                     'test',X'01',1,'test',X'01',1,0,0,0); \
                 INSERT INTO cleanup_action_journal_events(api_version,plan_id,sequence,event_kind,created_at_unix_ms) \
                     VALUES('deskgraph.cleanup-action-journal.v1',1,1,'preview_created',0);",
            )
            .expect("legacy policy-owned rows should persist");
        drop(connection);

        let database = ManifestDatabase::open(&path).expect("0024 migration should backfill");
        for table in [
            "authorized_scopes",
            "scan_jobs",
            "extraction_jobs",
            "watch_events",
            "action_plans",
            "cleanup_action_plans",
        ] {
            let sql = format!("SELECT policy_revision FROM {table} WHERE id=1");
            assert_eq!(
                database
                    .connection
                    .query_row(&sql, [], |row| row.get::<_, i64>(0))
                    .expect("backfilled revision should load"),
                1,
                "{table} must be bound to the pre-upgrade scope revision"
            );
        }
        assert!(
            database
                .scope_exclusions(1)
                .expect("new exclusion table should load")
                .is_empty()
        );
        assert_eq!(
            database
                .connection
                .query_row("PRAGMA foreign_key_check", [], |_| Ok(1_i64))
                .optional()
                .expect("foreign key check should run"),
            None
        );
    }

    #[test]
    fn scope_access_grant_migration_preserves_legacy_scopes_as_reauthorization_required() {
        let directory = tempfile::tempdir().expect("tempdir should exist");
        let path = directory.path().join("pre-scope-access-grants.sqlite3");
        let connection = Connection::open(&path).expect("legacy database should open");
        connection
            .execute_batch(
                "CREATE TABLE schema_migrations ( \
                     version INTEGER PRIMARY KEY, \
                     name TEXT NOT NULL, \
                     checksum TEXT NOT NULL, \
                     applied_at_unix_ms INTEGER NOT NULL \
                 );",
            )
            .expect("migration registry should initialize");
        for migration in &MIGRATIONS[..19] {
            connection
                .execute_batch(migration.sql)
                .expect("historical migration should apply");
            connection
                .execute(
                    "INSERT INTO schema_migrations(version, name, checksum, applied_at_unix_ms) \
                     VALUES (?1, ?2, ?3, 0)",
                    params![
                        migration.version,
                        migration.name,
                        migration_checksum(migration.sql)
                    ],
                )
                .expect("historical migration should register");
        }
        connection
            .execute(
                "INSERT INTO authorized_scopes( \
                     id, path_raw, path_key, display_path, platform, created_at_unix_ms \
                 ) VALUES (1, X'2F73636F7065', '/scope', '/scope', 'macos', 0)",
                [],
            )
            .expect("legacy scope should persist");
        drop(connection);

        let database = ManifestDatabase::open(&path).expect("new migration should apply");
        assert_eq!(
            database
                .scope_access_grant_state(1)
                .expect("legacy scope state should load"),
            ScopeAccessGrantState::NeedsReauthorization
        );
        assert!(
            database
                .scope_access_grant(1)
                .expect("legacy scope grant should load")
                .is_none(),
            "migration must not fabricate an active grant"
        );
        assert!(
            database
                .active_scope_access_grant_ids()
                .expect("active grant ids should load")
                .is_empty()
        );
    }

    #[test]
    fn coverage_overlap_migration_quarantines_legacy_descendants_and_guards_reactivation() {
        let directory = tempfile::tempdir().expect("tempdir should exist");
        let path = directory.path().join("pre-coverage-overlap-guard.sqlite3");
        let connection = Connection::open(&path).expect("legacy database should open");
        connection
            .execute_batch(
                "CREATE TABLE schema_migrations ( \
                     version INTEGER PRIMARY KEY, \
                     name TEXT NOT NULL, \
                     checksum TEXT NOT NULL, \
                     applied_at_unix_ms INTEGER NOT NULL \
                 );",
            )
            .expect("migration registry should initialize");
        for migration in &MIGRATIONS[..22] {
            connection
                .execute_batch(migration.sql)
                .expect("historical migration should apply");
            connection
                .execute(
                    "INSERT INTO schema_migrations(version, name, checksum, applied_at_unix_ms) \
                     VALUES (?1, ?2, ?3, 0)",
                    params![
                        migration.version,
                        migration.name,
                        migration_checksum(migration.sql)
                    ],
                )
                .expect("historical migration should register");
        }
        connection
            .execute_batch(
                "INSERT INTO authorized_scopes( \
                     id, path_raw, path_key, display_path, platform, created_at_unix_ms \
                 ) VALUES \
                     (1, X'2F686F6D652F706572736F6E', '/home/person', '/home/person', 'macos', 0), \
                     (2, X'2F686F6D652F706572736F6E2F4465736B746F70', '/home/person/Desktop', '/home/person/Desktop', 'macos', 0), \
                     (3, X'2F65787465726E616C', '/external', '/external', 'macos', 0), \
                     (4, X'2F', '/', '/', 'linux', 0), \
                     (5, X'2F726F6F742D6368696C64', '/root-child', '/root-child', 'linux', 0), \
                     (6, X'433A5C', 'c:\\', 'C:\\', 'windows', 0), \
                     (7, X'433A5C726F6F742D6368696C64', 'c:\\root-child', 'C:\\root-child', 'windows', 0); \
                 INSERT INTO scope_access_grants( \
                     scope_id, platform, opaque_grant, state, updated_at_unix_ms \
                 ) VALUES \
                     (1, 'macos', X'706172656E74', 'active', 0), \
                     (2, 'macos', X'6368696C64', 'active', 0), \
                     (3, 'macos', X'7369626C696E67', 'active', 0), \
                     (4, 'linux', X'756E69782D726F6F74', 'active', 0), \
                     (5, 'linux', X'756E69782D6368696C64', 'active', 0), \
                     (6, 'windows', X'77696E646F77732D726F6F74', 'active', 0), \
                     (7, 'windows', X'77696E646F77732D6368696C64', 'active', 0);",
            )
            .expect("legacy overlapping grants should persist");
        drop(connection);

        let mut database = ManifestDatabase::open(&path).expect("overlap migration should apply");
        assert_eq!(
            database
                .scope_access_grant_state(1)
                .expect("broad grant state should load"),
            ScopeAccessGrantState::Active
        );
        assert_eq!(
            database
                .scope_access_grant_state(2)
                .expect("descendant grant state should load"),
            ScopeAccessGrantState::NeedsReauthorization
        );
        for descendant_id in [5, 7] {
            assert_eq!(
                database
                    .scope_access_grant_state(descendant_id)
                    .expect("root descendant grant state should load"),
                ScopeAccessGrantState::NeedsReauthorization
            );
        }
        assert_eq!(
            database
                .list_active_scope_records()
                .expect("active roots should load")
                .into_iter()
                .map(|scope| scope.id)
                .collect::<Vec<_>>(),
            match std::env::consts::OS {
                "macos" => vec![1, 3],
                "linux" => vec![4],
                "windows" => vec![6],
                _ => Vec::new(),
            }
        );
        assert!(
            database
                .upsert_scope_access_grant(2, "macos", b"reactivated-child")
                .is_err(),
            "an inactive descendant must not reactivate beside its active parent"
        );
        database
            .add_scope_with_access_grant(
                b"/home/person",
                "/home/person",
                "/home/person",
                ScopeAccessGrantWrite {
                    scope_platform: "macos",
                    grant_platform: "macos",
                    opaque_grant: b"refreshed-parent",
                    state: ScopeAccessGrantState::Active,
                },
            )
            .expect("the broad root must remain reauthorizable");
    }

    #[test]
    fn scope_access_grants_are_opaque_upserted_and_never_exposed_by_scope_listing() {
        let database = ManifestDatabase::open_in_memory().expect("database should initialize");
        let scope = database
            .add_scope(b"/scope", "/scope", "/scope", "macos")
            .expect("scope should persist");

        assert_eq!(
            database
                .scope_access_grant_state(scope.id)
                .expect("missing grant state should load"),
            ScopeAccessGrantState::NeedsReauthorization
        );
        let first = database
            .upsert_scope_access_grant(scope.id, "macos", b"first-grant")
            .expect("grant should persist");
        assert_eq!(first.scope_id, scope.id);
        assert_eq!(first.platform, "macos");
        assert_eq!(first.opaque_grant, b"first-grant");
        assert_eq!(first.state, ScopeAccessGrantState::Active);
        assert!(first.updated_at_unix_ms >= 0);
        assert_eq!(
            database.list_scopes().expect("ordinary scopes should load"),
            vec![scope.clone()],
            "ordinary scope listings must retain their path-only domain shape"
        );
        assert!(
            database
                .scope_has_active_access_grant(scope.id)
                .expect("active state should load")
        );
        assert_eq!(
            database
                .active_scope_access_grant_ids()
                .expect("active grant ids should load"),
            vec![scope.id]
        );

        let replacement = database
            .upsert_scope_access_grant(scope.id, "macos", b"replacement-grant")
            .expect("replacement grant should upsert");
        assert_eq!(replacement.platform, "macos");
        assert_eq!(replacement.opaque_grant, b"replacement-grant");
        assert_eq!(
            database
                .connection
                .query_row(
                    "SELECT COUNT(*) FROM scope_access_grants WHERE scope_id = ?1",
                    [scope.id],
                    |row| row.get::<_, i64>(0),
                )
                .expect("one-to-one grant count should load"),
            1
        );

        database
            .mark_scope_access_grant_needs_reauthorization(scope.id)
            .expect("grant should become reauthorization-required");
        assert_eq!(
            database
                .scope_access_grant_state(scope.id)
                .expect("grant state should load"),
            ScopeAccessGrantState::NeedsReauthorization
        );
        database
            .upsert_scope_access_grant(scope.id, "macos", b"replacement-grant")
            .expect("explicit reauthorization should restore the grant before revocation");
        database
            .mark_scope_access_grant_revoked(scope.id)
            .expect("grant should become revoked");
        let revoked = database
            .scope_access_grant(scope.id)
            .expect("revoked grant should remain backend-readable")
            .expect("grant should remain durable for platform diagnostics");
        assert_eq!(revoked.state, ScopeAccessGrantState::Revoked);
        assert_eq!(
            revoked.opaque_grant,
            [0],
            "revocation must wipe grant bytes"
        );
        assert!(
            !database
                .scope_has_active_access_grant(scope.id)
                .expect("inactive state should load")
        );
        assert!(
            database
                .active_scope_access_grant_ids()
                .expect("active grant ids should load")
                .is_empty()
        );
    }

    #[test]
    fn scope_access_grant_write_debug_output_redacts_capability_bytes() {
        let write = ScopeAccessGrantWrite {
            scope_platform: "macos",
            grant_platform: "macos",
            opaque_grant: b"private-capability-marker",
            state: ScopeAccessGrantState::Active,
        };
        let output = format!("{write:?}");

        assert!(output.contains("opaque_grant_len: 25"));
        assert!(!output.contains("private-capability-marker"));
        assert!(!output.contains("opaque_grant: ["));
    }

    #[test]
    fn lexical_candidate_debug_output_redacts_local_context() {
        let candidate = LexicalSearchCandidate {
            source: LexicalCandidateSource::ExtractedText,
            scope_id: 7,
            policy_revision: 3,
            node_id: 11,
            location_id: 13,
            path_key: "/private/context.md".to_string(),
            display_path: "/Private/Context.md".to_string(),
            identity_kind: "unix_device_inode".to_string(),
            identity_key: b"private-identity".to_vec(),
            snippet: Some("private extracted text".to_string()),
        };

        let output = format!("{candidate:?}");
        assert!(!output.contains("/private/context.md"));
        assert!(!output.contains("/Private/Context.md"));
        assert!(!output.contains("private-identity"));
        assert!(!output.contains("private extracted text"));
        assert!(output.contains("<redacted>"));
    }

    #[test]
    fn scope_access_grants_reject_invalid_values_enforce_uniqueness_and_cascade_with_scope() {
        let database = ManifestDatabase::open_in_memory().expect("database should initialize");
        let scope = database
            .add_scope(b"/scope", "/scope", "/scope", "macos")
            .expect("scope should persist");

        for (platform, bytes) in [("unknown", b"grant".as_slice()), ("macos", b"".as_slice())] {
            assert!(matches!(
                database.upsert_scope_access_grant(scope.id, platform, bytes),
                Err(DatabaseError::ScopeAccessGrantInputInvalid)
            ));
        }
        assert!(matches!(
            database.upsert_scope_access_grant(
                scope.id,
                "macos",
                &vec![0_u8; MAX_SCOPE_ACCESS_GRANT_BYTES + 1],
            ),
            Err(DatabaseError::ScopeAccessGrantInputInvalid)
        ));
        assert!(matches!(
            database.mark_scope_access_grant_revoked(scope.id),
            Err(DatabaseError::ScopeAccessGrantNotActive)
        ));
        assert!(matches!(
            database.upsert_scope_access_grant(999, "macos", b"grant"),
            Err(DatabaseError::ScopeNotFound)
        ));

        database
            .upsert_scope_access_grant(scope.id, "macos", b"grant")
            .expect("valid grant should persist");
        for (index, (platform, bytes, state)) in [
            ("unknown", b"second".as_slice(), "active"),
            ("macos", b"".as_slice(), "active"),
            ("macos", b"second".as_slice(), "unknown"),
        ]
        .into_iter()
        .enumerate()
        {
            let invalid_scope = database
                .add_scope(
                    format!("/invalid-{index}").as_bytes(),
                    &format!("/invalid-{index}"),
                    &format!("/invalid-{index}"),
                    "macos",
                )
                .expect("invalid SQL fixture scope should persist");
            assert!(
                database
                    .connection
                    .execute(
                        "INSERT INTO scope_access_grants( \
                         scope_id, platform, opaque_grant, state, updated_at_unix_ms \
                     ) VALUES (?1, ?2, ?3, ?4, 0)",
                        params![invalid_scope.id, platform, bytes, state],
                    )
                    .is_err()
            );
        }
        assert!(
            database
                .connection
                .execute(
                    "INSERT INTO scope_access_grants( \
                     scope_id, platform, opaque_grant, state, updated_at_unix_ms \
                 ) VALUES (?1, 'macos', X'01', 'active', 0)",
                    [scope.id],
                )
                .is_err()
        );

        database
            .connection
            .execute("DELETE FROM authorized_scopes WHERE id = ?1", [scope.id])
            .expect("scope deletion should cascade its grant");
        assert_eq!(
            database
                .connection
                .query_row("SELECT COUNT(*) FROM scope_access_grants", [], |row| {
                    row.get::<_, i64>(0)
                })
                .expect("grant count should load"),
            0
        );
    }

    #[test]
    fn scope_and_access_grant_are_committed_or_rolled_back_together() {
        let mut database = ManifestDatabase::open_in_memory().expect("database should initialize");
        let scope = database
            .add_scope_with_access_grant(
                b"/selected",
                "/selected",
                "/selected",
                ScopeAccessGrantWrite {
                    scope_platform: "macos",
                    grant_platform: "macos",
                    opaque_grant: b"picker-grant",
                    state: ScopeAccessGrantState::Active,
                },
            )
            .expect("scope and active grant should persist atomically");
        assert_eq!(
            database
                .active_scope_grant(scope.id)
                .expect("active grant should be backend-readable")
                .opaque_grant,
            b"picker-grant"
        );
        assert_eq!(
            database
                .list_active_scope_grants()
                .expect("active grants should load")
                .len(),
            1
        );

        let failed = database.add_scope_with_access_grant(
            b"/selected-replaced",
            "/selected",
            "/selected-replaced",
            ScopeAccessGrantWrite {
                scope_platform: "windows",
                grant_platform: "windows",
                opaque_grant: b"wrong-platform",
                state: ScopeAccessGrantState::Active,
            },
        );
        assert!(matches!(
            failed,
            Err(DatabaseError::ScopeAccessGrantInputInvalid)
        ));
        let record = database
            .scope_record(scope.id)
            .expect("scope should remain readable");
        assert_eq!(record.path_raw, b"/selected");
        assert_eq!(record.display_path, "/selected");
        assert_eq!(
            database
                .active_scope_grant(scope.id)
                .expect("failed transaction must not replace existing grant")
                .opaque_grant,
            b"picker-grant"
        );

        let inactive_scope = database
            .add_scope_with_access_grant(
                b"/revoked",
                "/revoked",
                "/revoked",
                ScopeAccessGrantWrite {
                    scope_platform: "windows",
                    grant_platform: "windows",
                    opaque_grant: b"revoked-grant",
                    state: ScopeAccessGrantState::Revoked,
                },
            )
            .expect("explicitly revoked grant should persist atomically");
        assert!(matches!(
            database.active_scope_grant(inactive_scope.id),
            Err(DatabaseError::ScopeAccessGrantNotActive)
        ));
        assert_eq!(
            database
                .scope_access_grant(inactive_scope.id)
                .expect("revoked grant should load")
                .expect("revoked tombstone should persist")
                .opaque_grant,
            [0]
        );
    }

    #[test]
    fn coverage_roots_and_grants_commit_as_one_bounded_set() {
        let mut database = ManifestDatabase::open_in_memory().expect("database should initialize");
        let roots = [
            CoverageRootAccessGrantWrite {
                path_raw: b"/desktop",
                path_key: "/desktop",
                display_path: "/desktop",
                grant: ScopeAccessGrantWrite {
                    scope_platform: "macos",
                    grant_platform: "macos",
                    opaque_grant: b"desktop-grant",
                    state: ScopeAccessGrantState::Active,
                },
            },
            CoverageRootAccessGrantWrite {
                path_raw: b"/documents",
                path_key: "/documents",
                display_path: "/documents",
                grant: ScopeAccessGrantWrite {
                    scope_platform: "macos",
                    grant_platform: "macos",
                    opaque_grant: b"documents-grant",
                    state: ScopeAccessGrantState::Active,
                },
            },
        ];

        let scopes = database
            .add_coverage_roots_with_access_grants(&roots)
            .expect("coverage set should commit");
        assert_eq!(scopes.len(), 2);
        assert_eq!(
            database
                .active_scope_grant(scopes[0].id)
                .expect("first grant should persist")
                .opaque_grant,
            b"desktop-grant"
        );
        assert_eq!(
            database
                .active_scope_grant(scopes[1].id)
                .expect("second grant should persist")
                .opaque_grant,
            b"documents-grant"
        );
        assert_eq!(
            database
                .list_scope_records()
                .expect("backend scope records should load")
                .len(),
            2
        );
    }

    #[test]
    fn coverage_root_overlap_trigger_guards_every_writer() {
        let mut database = ManifestDatabase::open_in_memory().expect("database should initialize");
        let parent = database
            .add_scope_with_access_grant(
                b"/home/person",
                "/home/person",
                "/home/person",
                ScopeAccessGrantWrite {
                    scope_platform: "macos",
                    grant_platform: "macos",
                    opaque_grant: b"home-grant",
                    state: ScopeAccessGrantState::Active,
                },
            )
            .expect("parent root should persist");
        assert!(
            database
                .add_scope(
                    b"/home/person/Desktop",
                    "/home/person/Desktop",
                    "/home/person/Desktop",
                    "macos",
                )
                .is_err(),
            "a non-native writer must not bypass ancestor overlap policy"
        );
        let exact = database
            .add_scope(b"/home/person", "/home/person", "/home/person", "macos")
            .expect("exact reauthorization should remain valid");
        assert_eq!(exact.id, parent.id);
        database
            .add_scope(
                b"/home/person-2",
                "/home/person-2",
                "/home/person-2",
                "macos",
            )
            .expect("component sibling must not be treated as a descendant");

        database
            .add_scope_with_access_grant(
                b"C:\\Users\\person",
                "c:\\users\\person",
                "C:\\Users\\person",
                ScopeAccessGrantWrite {
                    scope_platform: "windows",
                    grant_platform: "windows",
                    opaque_grant: b"windows-home-grant",
                    state: ScopeAccessGrantState::Active,
                },
            )
            .expect("Windows parent root should persist");
        assert!(
            database
                .add_scope(
                    b"C:\\Users\\person\\Desktop",
                    "c:\\users\\person\\desktop",
                    "C:\\Users\\person\\Desktop",
                    "windows",
                )
                .is_err(),
            "Windows normalized path keys must enforce component overlap"
        );
    }

    #[test]
    fn coverage_root_separator_keys_cannot_bypass_active_grant_overlap_guards() {
        let mut unix_database =
            ManifestDatabase::open_in_memory().expect("Unix database should initialize");
        let unix_inactive_child = unix_database
            .add_scope(
                b"/inactive-child",
                "/inactive-child",
                "/inactive-child",
                "macos",
            )
            .expect("inactive Unix child fixture should persist");
        unix_database
            .add_scope_with_access_grant(
                b"/",
                "/",
                "/",
                ScopeAccessGrantWrite {
                    scope_platform: "macos",
                    grant_platform: "macos",
                    opaque_grant: b"unix-root-grant",
                    state: ScopeAccessGrantState::Active,
                },
            )
            .expect("low-level Unix root fixture should persist");
        assert!(
            unix_database
                .add_scope(b"/child", "/child", "/child", "macos",)
                .is_err(),
            "the scope trigger must reject a Unix descendant beneath '/'"
        );
        assert!(
            unix_database
                .upsert_scope_access_grant(
                    unix_inactive_child.id,
                    "macos",
                    b"unix-inactive-child-grant",
                )
                .is_err(),
            "the grant trigger must reject activating a Unix descendant beneath '/'"
        );

        let mut windows_database =
            ManifestDatabase::open_in_memory().expect("Windows database should initialize");
        let windows_inactive_child = windows_database
            .add_scope(
                b"C:\\inactive-child",
                "c:\\inactive-child",
                "C:\\inactive-child",
                "windows",
            )
            .expect("inactive Windows child fixture should persist");
        windows_database
            .add_scope_with_access_grant(
                b"C:\\",
                "c:\\",
                "C:\\",
                ScopeAccessGrantWrite {
                    scope_platform: "windows",
                    grant_platform: "windows",
                    opaque_grant: b"windows-root-grant",
                    state: ScopeAccessGrantState::Active,
                },
            )
            .expect("low-level Windows root fixture should persist");
        assert!(
            windows_database
                .add_scope(b"C:\\child", "c:\\child", "C:\\child", "windows",)
                .is_err(),
            "the scope trigger must reject a Windows descendant beneath 'C:\\'"
        );
        assert!(
            windows_database
                .upsert_scope_access_grant(
                    windows_inactive_child.id,
                    "windows",
                    b"windows-inactive-child-grant",
                )
                .is_err(),
            "the grant trigger must reject activating a Windows descendant beneath 'C:\\'"
        );
    }

    #[test]
    fn invalid_coverage_root_rolls_back_the_entire_set() {
        let mut database = ManifestDatabase::open_in_memory().expect("database should initialize");
        let roots = [
            CoverageRootAccessGrantWrite {
                path_raw: b"/desktop",
                path_key: "/desktop",
                display_path: "/desktop",
                grant: ScopeAccessGrantWrite {
                    scope_platform: "macos",
                    grant_platform: "macos",
                    opaque_grant: b"desktop-grant",
                    state: ScopeAccessGrantState::Active,
                },
            },
            CoverageRootAccessGrantWrite {
                path_raw: b"/documents",
                path_key: "/documents",
                display_path: "/documents",
                grant: ScopeAccessGrantWrite {
                    scope_platform: "windows",
                    grant_platform: "macos",
                    opaque_grant: b"invalid-mixed-platform-grant",
                    state: ScopeAccessGrantState::Active,
                },
            },
        ];

        assert!(matches!(
            database.add_coverage_roots_with_access_grants(&roots),
            Err(DatabaseError::ScopeAccessGrantInputInvalid)
        ));
        assert!(
            database
                .list_scope_records()
                .expect("failed set must leave no roots")
                .is_empty()
        );
        assert!(
            database
                .list_active_scope_grants()
                .expect("failed set must leave no grants")
                .is_empty()
        );

        let duplicate_keys = [roots[0], roots[0]];
        assert!(matches!(
            database.add_coverage_roots_with_access_grants(&duplicate_keys),
            Err(DatabaseError::ScopeAccessGrantInputInvalid)
        ));
        assert!(
            database
                .list_scope_records()
                .expect("duplicate set must leave no roots")
                .is_empty()
        );

        let nested_roots = [
            CoverageRootAccessGrantWrite {
                path_raw: b"/home/person",
                path_key: "/home/person",
                display_path: "/home/person",
                grant: ScopeAccessGrantWrite {
                    scope_platform: "macos",
                    grant_platform: "macos",
                    opaque_grant: b"home-grant",
                    state: ScopeAccessGrantState::Active,
                },
            },
            CoverageRootAccessGrantWrite {
                path_raw: b"/home/person/Desktop",
                path_key: "/home/person/Desktop",
                display_path: "/home/person/Desktop",
                grant: ScopeAccessGrantWrite {
                    scope_platform: "macos",
                    grant_platform: "macos",
                    opaque_grant: b"desktop-grant",
                    state: ScopeAccessGrantState::Active,
                },
            },
        ];
        assert!(matches!(
            database.add_coverage_roots_with_access_grants(&nested_roots),
            Err(DatabaseError::ScopeAccessGrantInputInvalid)
        ));
        assert!(
            database
                .list_scope_records()
                .expect("nested set must leave no roots")
                .is_empty()
        );

        let existing = database
            .add_scope_with_access_grant(
                b"C:\\existing",
                "c:\\existing",
                "C:\\existing",
                ScopeAccessGrantWrite {
                    scope_platform: "windows",
                    grant_platform: "windows",
                    opaque_grant: b"existing-windows-grant",
                    state: ScopeAccessGrantState::Active,
                },
            )
            .expect("existing platform-bound root should persist");
        let transaction_failure = [
            CoverageRootAccessGrantWrite {
                path_raw: b"/first-new-root",
                path_key: "/first-new-root",
                display_path: "/first-new-root",
                grant: ScopeAccessGrantWrite {
                    scope_platform: "macos",
                    grant_platform: "macos",
                    opaque_grant: b"first-new-grant",
                    state: ScopeAccessGrantState::Active,
                },
            },
            CoverageRootAccessGrantWrite {
                path_raw: b"C:\\existing",
                path_key: "c:\\existing",
                display_path: "C:\\existing",
                grant: ScopeAccessGrantWrite {
                    scope_platform: "macos",
                    grant_platform: "macos",
                    opaque_grant: b"wrong-platform-replacement",
                    state: ScopeAccessGrantState::Active,
                },
            },
        ];
        assert!(matches!(
            database.add_coverage_roots_with_access_grants(&transaction_failure),
            Err(DatabaseError::ScopeAccessGrantInputInvalid)
        ));
        assert_eq!(
            database
                .list_scope_records()
                .expect("transaction failure should preserve only existing root")
                .len(),
            1
        );
        assert_eq!(
            database
                .scope_access_grant(existing.id)
                .expect("stored grant should remain readable")
                .expect("existing grant should remain unchanged")
                .opaque_grant,
            b"existing-windows-grant"
        );
    }

    #[cfg(any(target_os = "macos", target_os = "linux"))]
    #[test]
    fn read_only_manifest_requires_an_existing_absolute_regular_file() {
        let directory = tempfile::tempdir().expect("tempdir should exist");
        let canonical_directory = directory
            .path()
            .canonicalize()
            .expect("tempdir should canonicalize");
        let missing_parent = canonical_directory.join("missing-parent");
        let missing_database = missing_parent.join("manifest.sqlite3");

        let error = ManifestReadDatabase::open_existing_read_only(&missing_database)
            .err()
            .expect("missing database must fail closed");

        assert_eq!(error.code(), "database_read_only_path_invalid");
        assert!(!missing_parent.exists());
        assert!(matches!(
            ManifestReadDatabase::open_existing_read_only(Path::new("relative.sqlite3")),
            Err(DatabaseError::ReadOnlyPathInvalid)
        ));

        let empty_database = canonical_directory.join("empty.sqlite3");
        fs::write(&empty_database, []).expect("empty fixture should exist");
        assert!(matches!(
            ManifestReadDatabase::open_existing_read_only(&empty_database),
            Err(DatabaseError::ReadOnlySchemaInvalid)
        ));
    }

    #[cfg(any(target_os = "macos", target_os = "linux"))]
    #[test]
    fn read_only_manifest_rejects_symlinked_database_and_parent() {
        use std::os::unix::fs::symlink;

        let directory = tempfile::tempdir().expect("tempdir should exist");
        let canonical_directory = directory
            .path()
            .canonicalize()
            .expect("tempdir should canonicalize");
        let real_parent = canonical_directory.join("real");
        fs::create_dir(&real_parent).expect("real parent should exist");
        let real_database = real_parent.join("manifest.sqlite3");
        ManifestDatabase::open(&real_database).expect("fixture should initialize");

        let database_link = canonical_directory.join("manifest-link.sqlite3");
        symlink(&real_database, &database_link).expect("database symlink should exist");
        assert!(matches!(
            ManifestReadDatabase::open_existing_read_only(&database_link),
            Err(DatabaseError::ReadOnlyPathInvalid)
        ));

        let parent_link = canonical_directory.join("parent-link");
        symlink(&real_parent, &parent_link).expect("parent symlink should exist");
        assert!(matches!(
            ManifestReadDatabase::open_existing_read_only(&parent_link.join("manifest.sqlite3")),
            Err(DatabaseError::ReadOnlyPathInvalid)
        ));

        let victim = canonical_directory.join("sidecar-victim");
        fs::write(&victim, b"must remain unchanged").expect("victim should exist");
        symlink(&victim, real_parent.join("manifest.sqlite3-shm"))
            .expect("sidecar symlink should exist");
        assert!(matches!(
            ManifestReadDatabase::open_existing_read_only(&real_database),
            Err(DatabaseError::ReadOnlyPathInvalid)
        ));
        assert_eq!(
            fs::read(&victim).expect("victim should remain readable"),
            b"must remain unchanged"
        );
    }

    #[cfg(any(target_os = "macos", target_os = "linux"))]
    #[test]
    fn read_only_manifest_seals_schema_and_blocks_writes() {
        let directory = tempfile::tempdir().expect("tempdir should exist");
        let path = directory
            .path()
            .canonicalize()
            .expect("tempdir should canonicalize")
            .join("manifest.sqlite3");
        let writer = ManifestDatabase::open(&path).expect("fixture should initialize");
        let scope = writer
            .add_scope(b"/scope", "/scope", "/scope", "macos")
            .expect("scope should persist");
        let unscanned_scope = writer
            .add_scope(b"/unscanned", "/unscanned", "/unscanned", "macos")
            .expect("unscanned scope should persist");
        writer
            .upsert_scope_access_grant(scope.id, "macos", b"read-only-test-grant")
            .expect("queryable scope grant should persist");
        writer
            .connection
            .execute(
                "INSERT INTO scan_jobs(scope_id, status, started_at_unix_ms, finished_at_unix_ms) \
                 VALUES (?1, 'completed', 1, 1)",
                [scope.id],
            )
            .expect("completed scan should persist");
        let scan_id = writer.connection.last_insert_rowid();
        writer
            .connection
            .execute(
                "INSERT INTO nodes(kind, identity_kind, identity_key, created_at_unix_ms, updated_at_unix_ms) \
                 VALUES ('file', 'test', X'01', 1, 1)",
                [],
            )
            .expect("node should persist");
        let node_id = writer.connection.last_insert_rowid();
        writer
            .connection
            .execute(
                "INSERT INTO files(node_id, size_bytes, modified_unix_ns, link_count) \
                 VALUES (?1, 10, 1, 1)",
                [node_id],
            )
            .expect("file should persist");
        writer
            .connection
            .execute(
                "INSERT INTO locations(scope_id, node_id, path_raw, path_key, display_path, present, last_seen_scan_id) \
                 VALUES (?1, ?2, X'2F73636F70652F6E6F7465732E6D64', '/scope/notes.md', '/scope/notes.md', 1, ?3)",
                params![scope.id, node_id, scan_id],
            )
            .expect("location should persist");
        let migration_count_before = writer
            .connection
            .query_row("SELECT COUNT(*) FROM schema_migrations", [], |row| {
                row.get::<_, i64>(0)
            })
            .expect("migration count should load");

        let reader = ManifestReadDatabase::open_existing_read_only(&path)
            .expect("active WAL database should open read-only");
        reader
            .ensure_scope_queryable(scope.id)
            .expect("completed scope should be queryable");
        assert!(matches!(
            reader.ensure_scope_queryable(unscanned_scope.id),
            Err(DatabaseError::ScanJobIncomplete)
        ));
        let candidates = reader
            .lexical_search_candidates("\"notes\"", lexical_filters(Some(scope.id)), 10)
            .expect("read-only metadata search should pass");
        assert_eq!(candidates.len(), 1);
        assert_eq!(candidates[0].display_path, "/scope/notes.md");
        let timeout = reader
            .lexical_search_candidates_until(
                "\"notes\"",
                lexical_filters(Some(scope.id)),
                10,
                Instant::now() - Duration::from_millis(1),
            )
            .expect_err("expired read-only search must stop");
        assert!(matches!(timeout, DatabaseError::ReadOnlyQueryTimeout));
        let recovered = reader
            .lexical_search_candidates("\"notes\"", lexical_filters(Some(scope.id)), 10)
            .expect("a request after timeout should recover");
        assert_eq!(recovered.len(), 1);
        assert!(reader.connection.is_readonly("main").unwrap_or(false));
        assert_eq!(
            reader
                .connection
                .pragma_query_value(None, "query_only", |row| row.get::<_, i64>(0))
                .expect("query-only state should load"),
            1
        );
        let write_error = reader
            .connection
            .execute(
                "INSERT INTO authorized_scopes(path_raw, path_key, display_path, platform, created_at_unix_ms) \
                 VALUES (X'00', 'denied', 'denied', 'test', 1)",
                [],
            )
            .expect_err("read-only connection must reject writes");
        assert!(matches!(write_error, rusqlite::Error::SqliteFailure(_, _)));
        drop(reader);

        assert_eq!(
            writer
                .connection
                .query_row("SELECT COUNT(*) FROM schema_migrations", [], |row| {
                    row.get::<_, i64>(0)
                })
                .expect("migration count should remain readable"),
            migration_count_before
        );
        assert_eq!(
            writer
                .connection
                .query_row("SELECT COUNT(*) FROM authorized_scopes", [], |row| {
                    row.get::<_, i64>(0)
                })
                .expect("scope count should remain readable"),
            2
        );
    }

    #[cfg(any(target_os = "macos", target_os = "linux"))]
    #[test]
    fn read_only_manifest_rejects_incomplete_or_changed_schema() {
        let directory = tempfile::tempdir().expect("tempdir should exist");
        let canonical_directory = directory
            .path()
            .canonicalize()
            .expect("tempdir should canonicalize");
        let path = canonical_directory.join("manifest.sqlite3");
        let writer = ManifestDatabase::open(&path).expect("fixture should initialize");
        writer
            .connection
            .execute("DELETE FROM schema_migrations WHERE version = 18", [])
            .expect("fixture should remove latest migration row");
        drop(writer);
        assert!(matches!(
            ManifestReadDatabase::open_existing_read_only(&path),
            Err(DatabaseError::ReadOnlySchemaInvalid)
        ));

        let changed_path = canonical_directory.join("changed.sqlite3");
        let changed = ManifestDatabase::open(&changed_path).expect("fixture should initialize");
        changed
            .connection
            .execute(
                "UPDATE schema_migrations SET checksum = 'changed' WHERE version = 18",
                [],
            )
            .expect("fixture checksum should change");
        drop(changed);
        assert!(matches!(
            ManifestReadDatabase::open_existing_read_only(&changed_path),
            Err(DatabaseError::MigrationChanged { version: 18 })
        ));
    }

    #[cfg(not(any(target_os = "macos", target_os = "linux")))]
    #[test]
    fn read_only_manifest_fails_closed_without_nofollow_sidecar_evidence() {
        assert!(matches!(
            ManifestReadDatabase::open_existing_read_only(Path::new("C:\\manifest.sqlite3")),
            Err(DatabaseError::ReadOnlyModeUnavailable)
        ));
    }

    #[test]
    fn changed_applied_migration_is_rejected() {
        let connection = Connection::open_in_memory().expect("connection should open");
        connection
            .execute_batch(
                "CREATE TABLE schema_migrations (version INTEGER PRIMARY KEY, name TEXT NOT NULL, checksum TEXT NOT NULL, applied_at_unix_ms INTEGER NOT NULL);\
                 INSERT INTO schema_migrations VALUES (1, 'manifest', 'wrong', 0);",
            )
            .expect("fixture should initialize");
        let error = ManifestDatabase::from_connection(connection)
            .err()
            .expect("changed migration must fail");

        assert!(matches!(
            error,
            DatabaseError::MigrationChanged { version: 1 }
        ));
    }

    #[test]
    fn file_backed_database_reopens_without_duplicate_migrations() {
        let directory = tempfile::tempdir().expect("tempdir should exist");
        let path = directory.path().join("manifest.sqlite3");
        ManifestDatabase::open(&path).expect("first open should initialize");
        let database = ManifestDatabase::open(&path).expect("second open should be idempotent");

        assert_eq!(
            database
                .stats()
                .expect("stats should load")
                .completed_scan_count,
            0
        );
    }

    #[test]
    fn image_metadata_migration_preserves_existing_chunks_and_fts() {
        let directory = tempfile::tempdir().expect("tempdir should exist");
        let path = directory.path().join("pre-image-metadata.sqlite3");
        let connection = Connection::open(&path).expect("legacy database should open");
        connection
            .execute_batch(
                "CREATE TABLE schema_migrations (version INTEGER PRIMARY KEY, name TEXT NOT NULL, checksum TEXT NOT NULL, applied_at_unix_ms INTEGER NOT NULL);",
            )
            .expect("migration registry should initialize");
        for migration in &MIGRATIONS[..13] {
            connection
                .execute_batch(migration.sql)
                .expect("historical migration should apply");
            connection
                .execute(
                    "INSERT INTO schema_migrations(version, name, checksum, applied_at_unix_ms) VALUES (?1, ?2, ?3, 0)",
                    params![
                        migration.version,
                        migration.name,
                        migration_checksum(migration.sql)
                    ],
                )
                .expect("historical migration should register");
        }
        connection
            .execute_batch(
                "INSERT INTO authorized_scopes VALUES (1, X'2F73636F7065', '/scope', '/scope', 'macos', 0); \
                 INSERT INTO scan_jobs(id, scope_id, status, discovered_files, discovered_folders, started_at_unix_ms, finished_at_unix_ms) VALUES (1, 1, 'completed', 1, 0, 0, 0); \
                 INSERT INTO nodes VALUES (1, 'file', 'test', X'01', 0, 0); \
                 INSERT INTO files VALUES (1, 4, 1, 1); \
                 INSERT INTO locations VALUES (1, 1, 1, X'2F73636F70652F66696C652E747874', '/scope/file.txt', '/scope/file.txt', 1, 1); \
                 INSERT INTO extraction_jobs(id, scope_id, node_id, location_id, status, source_size_bytes, source_modified_unix_ns, output_bytes, chunk_count, elapsed_ms, created_at_unix_ms, updated_at_unix_ms) VALUES (1, 1, 1, 1, 'completed', 4, 1, 4, 1, 1, 0, 0); \
                 INSERT INTO content_chunks(id, scope_id, node_id, location_id, extraction_job_id, ordinal, text, provenance_kind, source_byte_start, source_byte_end, source_size_bytes, source_modified_unix_ns, trust_class, provider_id, provider_version, active, created_at_unix_ms) VALUES (1, 1, 1, 1, 1, 0, '保留text', 'byte_range', 0, 4, 4, 1, 'untrusted_extracted_text', 'deskgraph.utf8-text', '1', 1, 0);",
            )
            .expect("legacy content should persist");
        drop(connection);

        let upgraded =
            ManifestDatabase::open(&path).expect("image metadata migration should apply");
        assert_eq!(
            upgraded
                .connection
                .query_row("SELECT text FROM content_chunks WHERE id = 1", [], |row| {
                    row.get::<_, String>(0)
                })
                .expect("legacy chunk should load"),
            "保留text"
        );
        assert_eq!(
            upgraded
                .lexical_search_candidates("保留t", lexical_filters(Some(1)), 10)
                .expect("legacy FTS query should fail closed before reauthorization")
                .len(),
            0
        );
        upgraded
            .upsert_scope_access_grant(1, "macos", b"migration-test-grant")
            .expect("legacy scope should be explicitly reauthorized");
        assert_eq!(
            upgraded
                .lexical_search_candidates("保留t", lexical_filters(Some(1)), 10)
                .expect("legacy FTS row should remain searchable after reauthorization")
                .len(),
            1
        );
        assert_eq!(
            upgraded
                .connection
                .query_row("SELECT COUNT(*) FROM image_metadata", [], |row| row
                    .get::<_, i64>(0))
                .expect("new metadata table should exist"),
            0
        );
    }

    #[test]
    fn ocr_migration_preserves_content_metadata_and_search() {
        let directory = tempfile::tempdir().expect("tempdir should exist");
        let path = directory.path().join("pre-ocr.sqlite3");
        let connection = Connection::open(&path).expect("legacy database should open");
        connection
            .execute_batch(
                "CREATE TABLE schema_migrations (version INTEGER PRIMARY KEY, name TEXT NOT NULL, checksum TEXT NOT NULL, applied_at_unix_ms INTEGER NOT NULL);",
            )
            .expect("migration registry should initialize");
        for migration in &MIGRATIONS[..14] {
            connection
                .execute_batch(migration.sql)
                .expect("historical migration should apply");
            connection
                .execute(
                    "INSERT INTO schema_migrations(version, name, checksum, applied_at_unix_ms) VALUES (?1, ?2, ?3, 0)",
                    params![
                        migration.version,
                        migration.name,
                        migration_checksum(migration.sql)
                    ],
                )
                .expect("historical migration should register");
        }
        connection
            .execute_batch(
                "INSERT INTO authorized_scopes VALUES (1, X'2F73636F7065', '/scope', '/scope', 'macos', 0); \
                 INSERT INTO scan_jobs(id, scope_id, status, discovered_files, discovered_folders, started_at_unix_ms, finished_at_unix_ms) VALUES (1, 1, 'completed', 1, 0, 0, 0); \
                 INSERT INTO nodes VALUES (1, 'file', 'test', X'01', 0, 0); \
                 INSERT INTO files VALUES (1, 4, 1, 1); \
                 INSERT INTO locations VALUES (1, 1, 1, X'2F73636F70652F66696C652E706E67', '/scope/file.png', '/scope/file.png', 1, 1); \
                 INSERT INTO extraction_jobs(id, scope_id, node_id, location_id, status, source_size_bytes, source_modified_unix_ns, output_bytes, chunk_count, elapsed_ms, created_at_unix_ms, updated_at_unix_ms) VALUES (1, 1, 1, 1, 'completed', 4, 1, 12, 1, 1, 0, 0); \
                 INSERT INTO content_chunks(id, scope_id, node_id, location_id, extraction_job_id, ordinal, text, provenance_kind, source_byte_start, source_byte_end, source_size_bytes, source_modified_unix_ns, trust_class, provider_id, provider_version, active, created_at_unix_ms) VALUES (1, 1, 1, 1, 1, 0, '保留legacy', 'byte_range', 0, 4, 4, 1, 'untrusted_extracted_text', 'deskgraph.utf8-text', '1', 1, 0); \
                 INSERT INTO image_metadata(id, scope_id, node_id, location_id, extraction_job_id, format, pixel_width, pixel_height, source_size_bytes, source_modified_unix_ns, provider_id, provider_version, active, created_at_unix_ms) VALUES (1, 1, 1, 1, 1, 'png', 2, 2, 4, 1, 'deskgraph.image-metadata', '1', 1, 0);",
            )
            .expect("legacy OCR-boundary fixture should persist");
        drop(connection);

        let upgraded = ManifestDatabase::open(&path).expect("OCR migration should apply");
        let job = upgraded.extraction_job(1).expect("legacy job should load");
        assert_eq!(job.api_version, "deskgraph.extraction-job.v2");
        assert_eq!(job.operation, ExtractionOperation::Content);
        let stored: (String, Option<i64>, Option<i64>) = upgraded
            .connection
            .query_row(
                "SELECT text, source_bbox_x_ppm, source_confidence_basis_points \
                 FROM content_chunks WHERE id = 1",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .expect("legacy chunk should load");
        assert_eq!(stored, ("保留legacy".to_string(), None, None));
        assert_eq!(
            upgraded
                .lexical_search_candidates("保留l", lexical_filters(Some(1)), 10)
                .expect("legacy FTS query should fail closed before reauthorization")
                .len(),
            0
        );
        upgraded
            .upsert_scope_access_grant(1, "macos", b"migration-test-grant")
            .expect("legacy scope should be explicitly reauthorized");
        assert_eq!(
            upgraded
                .lexical_search_candidates("保留l", lexical_filters(Some(1)), 10)
                .expect("legacy FTS row should remain searchable after reauthorization")
                .len(),
            1
        );
        assert_eq!(
            upgraded
                .connection
                .query_row(
                    "SELECT COUNT(*) FROM image_metadata WHERE active = 1",
                    [],
                    |row| { row.get::<_, i64>(0) }
                )
                .expect("legacy metadata should remain active"),
            1
        );
    }

    #[test]
    fn version_schema_upgrade_preserves_exact_relations_observations_and_feedback() {
        let directory = tempfile::tempdir().expect("tempdir should exist");
        let path = directory.path().join("pre-version.sqlite3");
        let mut connection = Connection::open(&path).expect("old database should open");
        connection
            .execute_batch(
                "PRAGMA foreign_keys = ON; \
                 CREATE TABLE schema_migrations ( \
                    version INTEGER PRIMARY KEY, name TEXT NOT NULL, checksum TEXT NOT NULL, \
                    applied_at_unix_ms INTEGER NOT NULL \
                 );",
            )
            .expect("migration table should initialize");
        for migration in MIGRATIONS.iter().take(10) {
            let transaction = connection
                .transaction()
                .expect("historical migration should start");
            transaction
                .execute_batch(migration.sql)
                .expect("historical migration should apply");
            transaction
                .execute(
                    "INSERT INTO schema_migrations(version, name, checksum, applied_at_unix_ms) \
                     VALUES (?1, ?2, ?3, 1)",
                    params![
                        migration.version,
                        migration.name,
                        migration_checksum(migration.sql)
                    ],
                )
                .expect("historical migration should record");
            transaction
                .commit()
                .expect("historical migration should commit");
        }
        let fence_domain =
            scope_filesystem_fence_domain(&connection).expect("test manifest domain should bind");
        let database = ManifestDatabase {
            connection,
            fence_domain,
        };
        let scope = database
            .add_scope(b"/scope", "/scope", "/scope", "test")
            .expect("scope should persist");
        database
            .connection
            .execute(
                "INSERT INTO scan_jobs( \
                    scope_id, status, discovered_files, discovered_folders, started_at_unix_ms, \
                    finished_at_unix_ms \
                 ) VALUES (?1, 'completed', 2, 0, 1, 1)",
                [scope.id],
            )
            .expect("scan should persist");
        let scan_id = database.connection.last_insert_rowid();
        let mut node_ids = Vec::new();
        let mut location_ids = Vec::new();
        for (path, identity) in [
            ("/scope/left.bin", b"identity:left".as_slice()),
            ("/scope/right.bin", b"identity:right".as_slice()),
        ] {
            database
                .connection
                .execute(
                    "INSERT INTO nodes( \
                        kind, identity_kind, identity_key, created_at_unix_ms, updated_at_unix_ms \
                     ) VALUES ('file', 'test_identity', ?1, 1, 1)",
                    [identity],
                )
                .expect("node should persist");
            let node_id = database.connection.last_insert_rowid();
            node_ids.push(node_id);
            database
                .connection
                .execute(
                    "INSERT INTO files(node_id, size_bytes, modified_unix_ns, link_count) \
                     VALUES (?1, 4, 2, 1)",
                    [node_id],
                )
                .expect("file should persist");
            database
                .connection
                .execute(
                    "INSERT INTO locations( \
                        scope_id, node_id, path_raw, path_key, display_path, present, \
                        last_seen_scan_id \
                     ) VALUES (?1, ?2, ?3, ?4, ?4, 1, ?5)",
                    params![scope.id, node_id, path.as_bytes(), path, scan_id],
                )
                .expect("location should persist");
            location_ids.push(database.connection.last_insert_rowid());
        }
        database
            .connection
            .execute(
                "INSERT INTO file_relation_candidates( \
                    api_version, relation_kind, scope_id, left_node_id, right_node_id, \
                    created_at_unix_ms \
                 ) VALUES (?1, 'exact_duplicate', ?2, ?3, ?4, 1)",
                params![
                    FileRelationCandidate::API_VERSION,
                    scope.id,
                    node_ids[0],
                    node_ids[1]
                ],
            )
            .expect("relation should persist");
        let relation_id = database.connection.last_insert_rowid();
        database
            .connection
            .execute(
                "INSERT INTO file_relation_observations( \
                    relation_id, left_location_id, right_location_id, source_size_bytes, \
                    left_modified_unix_ns, right_modified_unix_ns, compared_bytes, \
                    confidence_basis_points, comparison_kind, created_by, provider_id, \
                    provider_version, model_version, observed_at_unix_ms \
                 ) VALUES (?1, ?2, ?3, 4, 2, 2, 4, 10000, 'byte_for_byte', \
                    'system_rule', 'deskgraph.byte-equality', '1', NULL, 1)",
                params![relation_id, location_ids[0], location_ids[1]],
            )
            .expect("observation should persist");
        database
            .connection
            .execute(
                "INSERT INTO file_relation_feedback_events( \
                    relation_id, sequence, decision, created_by, created_at_unix_ms \
                 ) VALUES (?1, 1, 'rejected', 'user', 2)",
                [relation_id],
            )
            .expect("feedback should persist");
        let ManifestDatabase { connection, .. } = database;
        drop(connection);

        let upgraded = ManifestDatabase::open(&path).expect("version migration should apply");
        let candidate = upgraded
            .file_relation_candidate(relation_id)
            .expect("exact candidate should survive upgrade");
        assert_eq!(candidate.state, FileRelationCandidateState::Rejected);
        assert_eq!(
            candidate
                .latest_decision
                .as_ref()
                .map(|decision| decision.sequence),
            Some(1)
        );
        assert_eq!(
            upgraded
                .connection
                .query_row(
                    "SELECT COUNT(*) FROM file_relation_observations",
                    [],
                    |row| { row.get::<_, i64>(0) }
                )
                .expect("observations should count"),
            1
        );
        assert_eq!(
            upgraded
                .connection
                .query_row(
                    "SELECT COUNT(*) FROM file_relation_feedback_events",
                    [],
                    |row| { row.get::<_, i64>(0) }
                )
                .expect("feedback should count"),
            1
        );
        let foreign_key_issues = upgraded
            .connection
            .prepare("PRAGMA foreign_key_check")
            .and_then(|mut statement| {
                let rows = statement.query_map([], |_| Ok(()))?;
                rows.collect::<Result<Vec<_>, _>>()
            })
            .expect("foreign key check should run");
        assert!(foreign_key_issues.is_empty());
    }

    #[test]
    fn action_preview_and_first_journal_event_commit_atomically_and_are_immutable() {
        let (mut database, scope_id, node_id, _) = extraction_setup();
        let preview = create_bound_rename_preview(&mut database, scope_id, node_id);

        assert_eq!(preview.state, ActionPlanState::Previewed);
        assert_eq!(preview.journal_sequence, 1);
        assert_eq!(
            database
                .connection
                .query_row(
                    "SELECT COUNT(*) FROM action_journal_events WHERE plan_id = ?1",
                    [preview.plan_id],
                    |row| row.get::<_, i64>(0),
                )
                .expect("journal should count"),
            1
        );
        assert!(
            database
                .connection
                .execute(
                    "UPDATE action_plans SET destination_display_path = '/scope/other.txt' WHERE id = ?1",
                    [preview.plan_id],
                )
                .is_err(),
            "immutable plan update must fail"
        );
        assert!(
            database
                .connection
                .execute(
                    "DELETE FROM action_journal_events WHERE plan_id = ?1",
                    [preview.plan_id],
                )
                .is_err(),
            "append-only journal delete must fail"
        );
        assert!(
            database
                .connection
                .execute(
                    "DELETE FROM action_execution_bindings WHERE plan_id = ?1",
                    [preview.plan_id],
                )
                .is_err(),
            "execution binding delete must fail"
        );
    }

    #[test]
    fn action_journal_uses_closed_cas_state_transitions_and_idempotent_commands() {
        let (mut database, scope_id, node_id, _) = extraction_setup();
        let preview = create_bound_rename_preview(&mut database, scope_id, node_id);
        let binding = database
            .action_execution_record(preview.plan_id)
            .expect("new preview should have an immutable execution binding");
        assert_eq!(binding.state, ActionPlanState::Previewed);
        assert_eq!(binding.binding.source_hash_bytes, 4);
        assert_eq!(binding.binding.source_sha256.len(), 32);
        let execution_plan = database
            .action_execution_plan(preview.plan_id)
            .expect("bound plan should expose trusted raw execution detail");
        assert_eq!(execution_plan.source_path_raw, b"/scope/file.txt");
        assert_eq!(execution_plan.destination_path_raw, b"/scope/renamed.txt");
        assert_eq!(execution_plan.binding, binding.binding);

        let execute = database
            .start_action_command(ActionCommandWrite {
                plan_id: preview.plan_id,
                request_id: "request_execute_0001",
                kind: ActionCommandKind::Execute,
                expected_sequence: 1,
            })
            .expect("execute request should journal");
        assert_eq!(execute.state, ActionPlanState::ExecuteRequested);
        assert_eq!(execute.journal_sequence, 2);
        assert!(!execute.idempotent);
        acquire_test_executor_lease(&mut database, preview.plan_id);
        let replay = database
            .start_action_command(ActionCommandWrite {
                plan_id: preview.plan_id,
                request_id: "request_execute_0001",
                kind: ActionCommandKind::Execute,
                expected_sequence: 1,
            })
            .expect("same request must be idempotent");
        assert!(replay.idempotent);
        assert_eq!(replay.command_request_id, execute.command_request_id);
        assert_eq!(replay.journal_sequence, 2);
        assert!(matches!(
            database.start_action_command(ActionCommandWrite {
                plan_id: preview.plan_id,
                request_id: "request_execute_0001",
                kind: ActionCommandKind::Undo,
                expected_sequence: 2,
            }),
            Err(DatabaseError::ActionJournalIdempotencyConflict)
        ));
        assert!(matches!(
            database.append_action_journal_event(ActionJournalAppend {
                plan_id: preview.plan_id,
                command_request_id: execute.command_request_id,
                expected_sequence: 2,
                expected_state: ActionPlanState::ExecuteRequested,
                kind: ActionJournalEventKind::ExecutionCompleted,
                executor_lease_owner_token: "test_executor_0001",
            }),
            Err(DatabaseError::ActionJournalInvalidTransition)
        ));
        database
            .connection
            .execute(
                "INSERT INTO action_command_requests( \
                     plan_id, request_id, command_kind, requested_sequence, created_at_unix_ms \
                 ) VALUES (?1, 'request_execute_other', 'execute', 99, 1)",
                [preview.plan_id],
            )
            .expect("fixture command should persist");
        let other_command_id = database.connection.last_insert_rowid();
        assert!(
            database
                .connection
                .execute(
                    "INSERT INTO action_journal_events( \
                         api_version, plan_id, sequence, event_kind, command_request_id, \
                         created_at_unix_ms \
                     ) VALUES ('deskgraph.action-journal.v1', ?1, 3, 'direct_rename_intent', ?2, 1)",
                    params![preview.plan_id, other_command_id],
                )
                .is_err(),
            "a second command cannot attach itself to another command's intent"
        );
        let intent = database
            .append_action_journal_event(ActionJournalAppend {
                plan_id: preview.plan_id,
                command_request_id: execute.command_request_id,
                expected_sequence: 2,
                expected_state: ActionPlanState::ExecuteRequested,
                kind: ActionJournalEventKind::DirectRenameIntent,
                executor_lease_owner_token: "test_executor_0001",
            })
            .expect("rename intent should be journaled first");
        assert_eq!(intent.state, ActionPlanState::DirectRenameIntent);
        let restored = database
            .append_action_journal_event(ActionJournalAppend {
                plan_id: preview.plan_id,
                command_request_id: execute.command_request_id,
                expected_sequence: 3,
                expected_state: ActionPlanState::DirectRenameIntent,
                kind: ActionJournalEventKind::ExecutionNotApplied,
                executor_lease_owner_token: "test_executor_0001",
            })
            .expect("not-applied execution should return to previewed");
        assert_eq!(restored.state, ActionPlanState::Previewed);
        assert_eq!(restored.journal_sequence, 4);
        assert!(matches!(
            database.append_action_journal_event(ActionJournalAppend {
                plan_id: preview.plan_id,
                command_request_id: execute.command_request_id,
                expected_sequence: 3,
                expected_state: ActionPlanState::DirectRenameIntent,
                kind: ActionJournalEventKind::ExecutionNotApplied,
                executor_lease_owner_token: "test_executor_0001",
            }),
            Err(DatabaseError::ActionJournalCompareAndSwapFailed)
        ));
        let summary = database
            .recent_action_plans()
            .expect("path-free history should reduce later journal states");
        assert_eq!(summary[0].api_version, "deskgraph.action-plan-summary.v2");
        assert_eq!(summary[0].state, ActionPlanState::Previewed);
        assert_eq!(summary[0].journal_sequence, 4);
    }

    #[test]
    fn action_journal_request_not_started_events_release_only_the_original_stable_state() {
        let (mut database, scope_id, node_id, _) = extraction_setup();
        let preview = create_bound_rename_preview(&mut database, scope_id, node_id);
        let execute = database
            .start_action_command(ActionCommandWrite {
                plan_id: preview.plan_id,
                request_id: "request_execute_0003",
                kind: ActionCommandKind::Execute,
                expected_sequence: 1,
            })
            .expect("execute request should persist");
        acquire_test_executor_lease(&mut database, preview.plan_id);
        let previewed = database
            .append_action_journal_event(ActionJournalAppend {
                plan_id: preview.plan_id,
                command_request_id: execute.command_request_id,
                expected_sequence: execute.journal_sequence,
                expected_state: ActionPlanState::ExecuteRequested,
                kind: ActionJournalEventKind::ExecuteRequestNotStarted,
                executor_lease_owner_token: "test_executor_0001",
            })
            .expect("a command that never starts should release the preview");
        assert_eq!(previewed.state, ActionPlanState::Previewed);

        let execute_again = database
            .start_action_command(ActionCommandWrite {
                plan_id: preview.plan_id,
                request_id: "request_execute_0004",
                kind: ActionCommandKind::Execute,
                expected_sequence: previewed.journal_sequence,
            })
            .expect("a released preview may request execution again");
        let intent = database
            .append_action_journal_event(ActionJournalAppend {
                plan_id: preview.plan_id,
                command_request_id: execute_again.command_request_id,
                expected_sequence: execute_again.journal_sequence,
                expected_state: ActionPlanState::ExecuteRequested,
                kind: ActionJournalEventKind::DirectRenameIntent,
                executor_lease_owner_token: "test_executor_0001",
            })
            .expect("intent should persist");
        let executed = database
            .append_action_journal_event(ActionJournalAppend {
                plan_id: preview.plan_id,
                command_request_id: execute_again.command_request_id,
                expected_sequence: intent.journal_sequence,
                expected_state: ActionPlanState::DirectRenameIntent,
                kind: ActionJournalEventKind::ExecutionCompleted,
                executor_lease_owner_token: "test_executor_0001",
            })
            .expect("completion should persist");
        let undo = database
            .start_action_command(ActionCommandWrite {
                plan_id: preview.plan_id,
                request_id: "request_undo_0000002",
                kind: ActionCommandKind::Undo,
                expected_sequence: executed.journal_sequence,
            })
            .expect("undo request should persist");
        let still_executed = database
            .append_action_journal_event(ActionJournalAppend {
                plan_id: preview.plan_id,
                command_request_id: undo.command_request_id,
                expected_sequence: undo.journal_sequence,
                expected_state: ActionPlanState::UndoRequested,
                kind: ActionJournalEventKind::UndoRequestNotStarted,
                executor_lease_owner_token: "test_executor_0001",
            })
            .expect("an undo that never starts should retain executed state");
        assert_eq!(still_executed.state, ActionPlanState::Executed);
    }

    #[test]
    fn action_executor_lease_blocks_a_second_process_then_allows_expired_recovery() {
        let directory = tempfile::tempdir().expect("tempdir should exist");
        let path = directory
            .path()
            .canonicalize()
            .expect("tempdir should canonicalize")
            .join("manifest.sqlite3");
        let (mut first, scope_id, node_id, _) = extraction_setup_in(
            ManifestDatabase::open(&path).expect("first process should initialize"),
        );
        let preview = create_bound_rename_preview(&mut first, scope_id, node_id);
        let execute = first
            .start_action_command(ActionCommandWrite {
                plan_id: preview.plan_id,
                request_id: "request_execute_lease_01",
                kind: ActionCommandKind::Execute,
                expected_sequence: 1,
            })
            .expect("command should persist");
        first
            .acquire_action_executor_lease(preview.plan_id, "executor_process_one", 60_000)
            .expect("first process should claim lease");

        let mut second = ManifestDatabase::open(&path).expect("second process should open");
        assert!(matches!(
            second.acquire_action_executor_lease(preview.plan_id, "executor_process_two", 60_000,),
            Err(DatabaseError::ActionExecutorLeaseUnavailable)
        ));
        first
            .connection
            .execute(
                "UPDATE action_executor_leases SET expires_at_unix_ms = 0 WHERE plan_id = ?1",
                [preview.plan_id],
            )
            .expect("test crash simulation should expire the operational lease");
        drop(first);

        second
            .acquire_action_executor_lease(preview.plan_id, "executor_process_two", 60_000)
            .expect("expired lease should be recoverable after reopen");
        let intent = second
            .append_action_journal_event(ActionJournalAppend {
                plan_id: preview.plan_id,
                command_request_id: execute.command_request_id,
                expected_sequence: execute.journal_sequence,
                expected_state: ActionPlanState::ExecuteRequested,
                kind: ActionJournalEventKind::DirectRenameIntent,
                executor_lease_owner_token: "executor_process_two",
            })
            .expect("only recovered owner may advance the journal");
        assert_eq!(intent.state, ActionPlanState::DirectRenameIntent);
    }

    #[test]
    fn action_journal_supports_execute_then_idempotent_undo_and_recovery_query() {
        let (mut database, scope_id, node_id, _) = extraction_setup();
        let preview = create_bound_rename_preview(&mut database, scope_id, node_id);
        let execute = database
            .start_action_command(ActionCommandWrite {
                plan_id: preview.plan_id,
                request_id: "request_execute_0002",
                kind: ActionCommandKind::Execute,
                expected_sequence: 1,
            })
            .expect("execute request should persist");
        acquire_test_executor_lease(&mut database, preview.plan_id);
        let intent = database
            .append_action_journal_event(ActionJournalAppend {
                plan_id: preview.plan_id,
                command_request_id: execute.command_request_id,
                expected_sequence: 2,
                expected_state: ActionPlanState::ExecuteRequested,
                kind: ActionJournalEventKind::DirectRenameIntent,
                executor_lease_owner_token: "test_executor_0001",
            })
            .expect("intent should persist");
        let recovery = database
            .incomplete_action_recovery(10)
            .expect("active executor work must not be offered to recovery");
        assert!(recovery.is_empty());
        database
            .release_action_executor_lease(preview.plan_id, "test_executor_0001")
            .expect("test executor should release before recovery");
        let recovery = database
            .incomplete_action_recovery(10)
            .expect("released intent should be listed path-free");
        assert_eq!(recovery.len(), 1);
        assert_eq!(recovery[0].state, ActionPlanState::DirectRenameIntent);
        assert_eq!(recovery[0].command_request_id, execute.command_request_id);
        acquire_test_executor_lease(&mut database, preview.plan_id);
        let executed = database
            .append_action_journal_event(ActionJournalAppend {
                plan_id: preview.plan_id,
                command_request_id: execute.command_request_id,
                expected_sequence: intent.journal_sequence,
                expected_state: ActionPlanState::DirectRenameIntent,
                kind: ActionJournalEventKind::ExecutionCompleted,
                executor_lease_owner_token: "test_executor_0001",
            })
            .expect("completion should persist");
        assert_eq!(executed.state, ActionPlanState::Executed);
        let undo = database
            .start_action_command(ActionCommandWrite {
                plan_id: preview.plan_id,
                request_id: "request_undo_0000001",
                kind: ActionCommandKind::Undo,
                expected_sequence: executed.journal_sequence,
            })
            .expect("undo request should persist");
        let undo_intent = database
            .append_action_journal_event(ActionJournalAppend {
                plan_id: preview.plan_id,
                command_request_id: undo.command_request_id,
                expected_sequence: undo.journal_sequence,
                expected_state: ActionPlanState::UndoRequested,
                kind: ActionJournalEventKind::UndoRenameIntent,
                executor_lease_owner_token: "test_executor_0001",
            })
            .expect("undo intent should persist");
        let undone = database
            .append_action_journal_event(ActionJournalAppend {
                plan_id: preview.plan_id,
                command_request_id: undo.command_request_id,
                expected_sequence: undo_intent.journal_sequence,
                expected_state: ActionPlanState::UndoRenameIntent,
                kind: ActionJournalEventKind::UndoCompleted,
                executor_lease_owner_token: "test_executor_0001",
            })
            .expect("undo completion should persist");
        assert_eq!(undone.state, ActionPlanState::Undone);
        let execute_replay = database
            .start_action_command(ActionCommandWrite {
                plan_id: preview.plan_id,
                request_id: "request_execute_0002",
                kind: ActionCommandKind::Execute,
                expected_sequence: 1,
            })
            .expect("old execute replay should be historical and non-mutating");
        assert!(execute_replay.idempotent);
        assert_eq!(execute_replay.state, ActionPlanState::Executed);
        assert_eq!(execute_replay.journal_sequence, executed.journal_sequence);
        let undo_replay = database
            .start_action_command(ActionCommandWrite {
                plan_id: preview.plan_id,
                request_id: "request_undo_0000001",
                kind: ActionCommandKind::Undo,
                expected_sequence: 1,
            })
            .expect("old undo replay should be historical and non-mutating");
        assert!(undo_replay.idempotent);
        assert_eq!(undo_replay.state, ActionPlanState::Undone);
        assert_eq!(undo_replay.journal_sequence, undone.journal_sequence);
        assert_eq!(
            database
                .action_execution_record(preview.plan_id)
                .expect("global plan should remain unchanged")
                .state,
            ActionPlanState::Undone
        );
        assert!(
            database
                .incomplete_action_recovery(10)
                .expect("terminal undo is not recovery work")
                .is_empty()
        );
    }

    #[test]
    fn recovery_limits_qualifying_bound_work_not_terminal_or_legacy_plans() {
        let (mut database, scope_id, node_id, _) = extraction_setup();
        let terminal_seed = create_bound_rename_preview(&mut database, scope_id, node_id);
        for index in 0..101 {
            insert_terminal_action_fixture(
                &mut database,
                terminal_seed.plan_id,
                &format!("terminal_request_{index:04}"),
            );
        }
        insert_legacy_preview_fixture(&mut database, terminal_seed.plan_id);

        let recoverable = create_bound_rename_preview(&mut database, scope_id, node_id);
        let request = database
            .start_action_command(ActionCommandWrite {
                plan_id: recoverable.plan_id,
                request_id: "request_recovery_0001",
                kind: ActionCommandKind::Execute,
                expected_sequence: 1,
            })
            .expect("recoverable request should persist");
        let recovery = database
            .incomplete_action_recovery(1)
            .expect("terminal and legacy rows must not starve recovery");
        assert_eq!(recovery.len(), 1);
        assert_eq!(recovery[0].plan_id, recoverable.plan_id);
        assert_eq!(recovery[0].command_request_id, request.command_request_id);
        assert_eq!(recovery[0].state, ActionPlanState::ExecuteRequested);
        assert_eq!(recovery[0].journal_sequence, request.journal_sequence);
    }

    #[test]
    fn legacy_previews_without_a_v19_binding_are_non_executable() {
        let (mut database, scope_id, node_id, _) = extraction_setup();
        test_active_binding(&database, scope_id)
            .expect("historical action fixture scope should be active");
        database
            .connection
            .execute(
                "INSERT INTO action_plans( \
                     api_version, policy_version, operation, execution_strategy, scope_id, node_id, \
                     source_location_id, source_path_raw, source_path_key, source_display_path, \
                     destination_path_raw, destination_path_key, destination_display_path, \
                     source_identity_kind, source_identity_key, source_size_bytes, \
                     source_modified_unix_ns, created_at_unix_ms \
                 ) SELECT \
                     'deskgraph.action-plan.v1', 'deskgraph.action-policy.v1', 'rename', 'direct', \
                     l.scope_id, l.node_id, l.id, l.path_raw, l.path_key, l.display_path, \
                     X'2F73636F70652F6C65676163792E747874', '/scope/legacy.txt', '/scope/legacy.txt', \
                     n.identity_kind, n.identity_key, f.size_bytes, f.modified_unix_ns, 1 \
                 FROM locations l JOIN nodes n ON n.id = l.node_id \
                    JOIN files f ON f.node_id = l.node_id \
                 WHERE l.scope_id = ?1 AND l.node_id = ?2",
                params![scope_id, node_id],
            )
            .expect("historical plan fixture should insert");
        let plan_id = database.connection.last_insert_rowid();
        database
            .connection
            .execute(
                "INSERT INTO action_journal_events( \
                     api_version, plan_id, sequence, event_kind, command_request_id, \
                     created_at_unix_ms \
                 ) VALUES ('deskgraph.action-journal.v1', ?1, 1, 'preview_created', NULL, 1)",
                [plan_id],
            )
            .expect("historical event fixture should insert");
        assert!(matches!(
            database.action_execution_record(plan_id),
            Err(DatabaseError::ActionExecutionBindingUnavailable)
        ));
        assert!(matches!(
            database.action_execution_plan(plan_id),
            Err(DatabaseError::ActionExecutionBindingUnavailable)
        ));
        assert!(matches!(
            database.start_action_command(ActionCommandWrite {
                plan_id,
                request_id: "request_legacy_0001",
                kind: ActionCommandKind::Execute,
                expected_sequence: 1,
            }),
            Err(DatabaseError::ActionExecutionBindingUnavailable)
        ));
    }

    #[test]
    fn screenshot_groups_use_current_provenance_and_persist_complete_immutable_observations() {
        let (mut database, scope_id, sources) = screenshot_group_setup();
        assert_eq!(sources.len(), 2);
        assert_eq!(sources[0].pixel_width, 1440);
        assert_eq!(sources[0].pixel_height, 900);
        assert_eq!(sources[0].ocr_chunk_count, 1);

        let (evaluated, candidates) = database
            .discover_screenshot_group_candidates(scope_id)
            .expect("complete screenshot group should persist atomically");
        assert_eq!(evaluated, 2);
        assert_eq!(candidates.len(), 1);
        let candidate = &candidates[0];
        assert_eq!(candidate.members.len(), 2);
        assert_eq!(candidate.evidence.confidence_basis_points, 6_000);
        assert!(candidate.evidence.review_assistance_only);
        assert!(!candidate.evidence.content_similarity_claimed);
        assert!(!candidate.evidence.cleanup_authorized);
        assert_eq!(candidate.total_size_bytes, 2_048);
        assert_eq!(
            database
                .connection
                .query_row("SELECT COUNT(*) FROM screenshot_group_members", [], |row| {
                    row.get::<_, i64>(0)
                })
                .expect("members should count"),
            2
        );

        assert!(
            database
                .connection
                .execute(
                    "UPDATE screenshot_group_observations SET confidence_basis_points = 7000",
                    []
                )
                .is_err()
        );
        assert!(
            database
                .connection
                .execute("DELETE FROM screenshot_group_members", [])
                .is_err()
        );
    }

    #[test]
    fn screenshot_grouping_is_deterministic_dimensioned_and_time_windowed() {
        let minute = 60_i64 * 1_000_000_000;
        let groups = group_screenshot_sources(vec![
            synthetic_screenshot_group_source(4, 2 * minute, 1440, 900),
            synthetic_screenshot_group_source(1, 0, 1440, 900),
            synthetic_screenshot_group_source(5, 20 * minute, 1440, 900),
            synthetic_screenshot_group_source(3, minute, 1920, 1080),
            synthetic_screenshot_group_source(2, minute, 1440, 900),
            synthetic_screenshot_group_source(6, 21 * minute, 1440, 900),
        ])
        .expect("bounded groups should build");
        assert_eq!(groups.len(), 2);
        assert_eq!(
            groups[0]
                .iter()
                .map(|source| source.node_id)
                .collect::<Vec<_>>(),
            vec![1, 2, 4]
        );
        assert_eq!(
            groups[1]
                .iter()
                .map(|source| source.node_id)
                .collect::<Vec<_>>(),
            vec![5, 6]
        );
    }

    #[test]
    fn screenshot_grouping_rejects_ambiguous_or_oversized_membership() {
        let duplicate = synthetic_screenshot_group_source(1, 1, 1440, 900);
        assert!(matches!(
            group_screenshot_sources(vec![duplicate.clone(), duplicate]),
            Err(DatabaseError::ScreenshotGroupCandidateInputInvalid)
        ));
        let oversized = (1..=21)
            .map(|node_id| synthetic_screenshot_group_source(node_id, node_id, 1440, 900))
            .collect();
        assert!(matches!(
            group_screenshot_sources(oversized),
            Err(DatabaseError::ScreenshotGroupMemberLimitExceeded)
        ));
    }

    #[test]
    fn screenshot_group_history_is_path_free_and_currentness_fails_closed() {
        let (mut database, scope_id, sources) = screenshot_group_setup();
        let candidate = database
            .discover_screenshot_group_candidates(scope_id)
            .expect("group should persist")
            .1
            .remove(0);
        let summary = database
            .recent_screenshot_group_candidates()
            .expect("summary should load")
            .remove(0);
        assert!(summary.current_evidence);
        assert!(summary.verification_required);
        assert!(!summary.cleanup_authorized);

        database
            .connection
            .execute(
                "UPDATE files SET modified_unix_ns = modified_unix_ns + 1 WHERE node_id = ?1",
                [sources[0].node_id],
            )
            .expect("source mutation should persist");
        assert!(matches!(
            database.screenshot_group_candidate(candidate.group_id),
            Err(DatabaseError::ScreenshotGroupCandidateNotCurrent)
        ));
        let summary = database
            .recent_screenshot_group_candidates()
            .expect("stale history should remain readable")
            .remove(0);
        assert!(!summary.current_evidence);
        assert!(!summary.cleanup_authorized);
    }

    #[test]
    fn smart_cleanup_screenshot_item_binds_current_path_free_observation() {
        let (mut database, scope_id, _) = screenshot_group_setup();
        let candidate = database
            .discover_screenshot_group_candidates(scope_id)
            .expect("group should persist")
            .1
            .remove(0);
        let (references, complete) = database
            .smart_cleanup_source_references(scope_id, 20)
            .expect("current source inventory should load");
        assert!(complete);
        assert_eq!(references.len(), 1);
        assert_eq!(
            references[0].kind,
            SmartCleanupSourceKind::ScreenshotReviewGroup
        );
        let item = database
            .smart_cleanup_screenshot_item(candidate.group_id)
            .expect("current observation should map without paths");
        assert_eq!(item.source_id, candidate.group_id);
        assert_eq!(
            item.source_observation_id,
            candidate.evidence.observation_id
        );
        assert_eq!(item.member_count, 2);
        assert!(item.current_evidence);
        assert!(item.verification_required);
        assert!(item.review_assistance_only);
        assert!(!item.cleanup_authorized);
        let validated = database
            .validate_cleanup_source_observation(
                scope_id,
                SmartCleanupSourceKind::ScreenshotReviewGroup,
                candidate.group_id,
                candidate.evidence.observation_id,
            )
            .expect("explicit current screenshot observation should validate");
        assert_eq!(validated, item);
        assert!(matches!(
            database.validate_cleanup_source_observation(
                scope_id,
                SmartCleanupSourceKind::ScreenshotReviewGroup,
                candidate.group_id,
                candidate.evidence.observation_id + 1,
            ),
            Err(DatabaseError::CleanupActionSourceNotCurrent)
        ));
        database
            .mark_scope_access_grant_revoked(scope_id)
            .expect("grant should revoke");
        assert!(matches!(
            database.validate_cleanup_source_observation(
                scope_id,
                SmartCleanupSourceKind::ScreenshotReviewGroup,
                candidate.group_id,
                candidate.evidence.observation_id,
            ),
            Err(DatabaseError::ScopeAccessGrantNotActive)
        ));
    }

    #[test]
    fn screenshot_group_discovery_is_evidence_idempotent() {
        let (mut database, scope_id, _) = screenshot_group_setup();
        let first = database
            .discover_screenshot_group_candidates(scope_id)
            .expect("first discovery should persist")
            .1
            .remove(0);
        let second = database
            .discover_screenshot_group_candidates(scope_id)
            .expect("unchanged discovery should be idempotent")
            .1
            .remove(0);
        assert_eq!(first.group_id, second.group_id);
        assert_eq!(
            first.evidence.observation_id,
            second.evidence.observation_id
        );
        assert_eq!(
            database
                .connection
                .query_row(
                    "SELECT COUNT(*) FROM screenshot_group_candidates",
                    [],
                    |row| { row.get::<_, i64>(0) }
                )
                .expect("candidate count should load"),
            1
        );
        assert_eq!(
            database
                .connection
                .query_row(
                    "SELECT COUNT(*) FROM screenshot_group_observations",
                    [],
                    |row| { row.get::<_, i64>(0) }
                )
                .expect("observation count should load"),
            1
        );
        assert_eq!(
            database
                .connection
                .query_row("SELECT COUNT(*) FROM screenshot_group_members", [], |row| {
                    row.get::<_, i64>(0)
                })
                .expect("member count should load"),
            2
        );
    }

    #[test]
    fn screenshot_group_source_query_excludes_missing_or_inactive_ocr_provenance() {
        let (database, scope_id, sources) = screenshot_group_setup();
        database
            .connection
            .execute(
                "UPDATE content_chunks SET active = 0 WHERE node_id = ?1",
                [sources[0].node_id],
            )
            .expect("OCR provenance should deactivate");
        let remaining = database
            .screenshot_group_sources(scope_id)
            .expect("eligible sources should reload");
        assert_eq!(remaining.len(), 1);
        assert_ne!(remaining[0].node_id, sources[0].node_id);
        assert!(!format!("{remaining:?}").contains("/scope"));
    }

    #[test]
    fn screenshot_group_membership_change_invalidates_the_old_candidate() {
        let (mut database, scope_id, _) = screenshot_group_setup();
        let old_candidate = database
            .discover_screenshot_group_candidates(scope_id)
            .expect("initial pair should persist")
            .1
            .remove(0);
        let scan_id = database
            .connection
            .query_row(
                "SELECT MAX(id) FROM scan_jobs WHERE scope_id = ?1 AND status = 'completed'",
                [scope_id],
                |row| row.get::<_, i64>(0),
            )
            .expect("completed scan should exist");
        insert_screenshot_group_source(&database, scope_id, scan_id, 2, "1");

        assert!(matches!(
            database.screenshot_group_candidate(old_candidate.group_id),
            Err(DatabaseError::ScreenshotGroupCandidateNotCurrent)
        ));
        assert!(
            !database
                .recent_screenshot_group_candidates()
                .expect("history should remain readable")
                .into_iter()
                .find(|summary| summary.group_id == old_candidate.group_id)
                .expect("old group should remain in history")
                .current_evidence
        );

        let discovery = database
            .discover_screenshot_group_candidates(scope_id)
            .expect("new complete membership should persist");
        assert_eq!(discovery.0, 3);
        assert_eq!(discovery.1.len(), 1);
        assert_eq!(discovery.1[0].members.len(), 3);
        assert_ne!(discovery.1[0].group_id, old_candidate.group_id);
    }

    #[test]
    fn screenshot_group_ocr_refresh_requires_a_new_complete_observation() {
        let (mut database, scope_id, sources) = screenshot_group_setup();
        let first = database
            .discover_screenshot_group_candidates(scope_id)
            .expect("initial evidence should persist")
            .1
            .remove(0);
        for source in &sources {
            database
                .connection
                .execute(
                    "INSERT INTO extraction_jobs( \
                        scope_id, node_id, location_id, status, provider_id, provider_version, \
                        source_size_bytes, source_modified_unix_ns, output_bytes, chunk_count, \
                        created_at_unix_ms, started_at_unix_ms, finished_at_unix_ms, updated_at_unix_ms, \
                        operation \
                     ) VALUES (?1, ?2, ?3, 'completed', 'local-ocr', '2', ?4, ?5, \
                        1, 1, 2, 2, 2, 2, 'screenshot_ocr')",
                    params![
                        source.scope_id,
                        source.node_id,
                        source.location_id,
                        to_i64(source.size_bytes).expect("size should fit"),
                        source.modified_unix_ns
                    ],
                )
                .expect("refreshed OCR job should persist");
            let job_id = database.connection.last_insert_rowid();
            database
                .connection
                .execute(
                    "INSERT INTO content_chunks( \
                        scope_id, node_id, location_id, extraction_job_id, ordinal, text, \
                        provenance_kind, source_unit_number, source_fragment_index, \
                        source_bbox_x_ppm, source_bbox_y_ppm, source_bbox_width_ppm, \
                        source_bbox_height_ppm, source_confidence_basis_points, source_size_bytes, \
                        source_modified_unix_ns, trust_class, provider_id, provider_version, active, \
                        created_at_unix_ms \
                     ) VALUES (?1, ?2, ?3, ?4, 0, 'refreshed private OCR text', \
                        'ocr_observation', 1, 0, 0, 0, 1000000, 1000000, NULL, ?5, ?6, \
                        'untrusted_extracted_text', 'local-ocr', '2', 1, 2)",
                    params![
                        source.scope_id,
                        source.node_id,
                        source.location_id,
                        job_id,
                        to_i64(source.size_bytes).expect("size should fit"),
                        source.modified_unix_ns
                    ],
                )
                .expect("refreshed OCR provenance should persist");
        }

        assert!(matches!(
            database.screenshot_group_candidate(first.group_id),
            Err(DatabaseError::ScreenshotGroupCandidateNotCurrent)
        ));
        let refreshed = database
            .discover_screenshot_group_candidates(scope_id)
            .expect("refreshed evidence should persist")
            .1
            .remove(0);
        assert_eq!(refreshed.group_id, first.group_id);
        assert_ne!(
            refreshed.evidence.observation_id,
            first.evidence.observation_id
        );
        assert!(
            refreshed
                .members
                .iter()
                .all(|member| member.ocr_provider_version == "2")
        );
        assert_eq!(
            database
                .connection
                .query_row(
                    "SELECT COUNT(*) FROM screenshot_group_observations",
                    [],
                    |row| { row.get::<_, i64>(0) }
                )
                .expect("observation count should load"),
            2
        );
    }

    #[test]
    fn screenshot_group_platform_grant_revocation_fails_closed_without_paths() {
        let (mut database, scope_id, _) = screenshot_group_setup();
        let candidate = database
            .discover_screenshot_group_candidates(scope_id)
            .expect("initial evidence should persist")
            .1
            .remove(0);
        database
            .mark_scope_access_grant_needs_reauthorization(scope_id)
            .expect("test grant should become inactive");

        assert!(matches!(
            database.screenshot_group_sources(scope_id),
            Err(DatabaseError::ScopeAccessGrantNotActive)
        ));
        assert!(matches!(
            database.screenshot_group_candidate(candidate.group_id),
            Err(DatabaseError::ScopeAccessGrantNotActive)
        ));
        let summary = database
            .recent_screenshot_group_candidates()
            .expect("path-free history should remain readable")
            .remove(0);
        assert!(!summary.current_evidence);
        assert!(!summary.cleanup_authorized);
        assert!(!format!("{summary:?}").contains("/scope"));
    }

    #[test]
    fn screenshot_group_missing_and_revoked_grants_fail_closed() {
        let (mut database, scope_id, _) = screenshot_group_setup();
        let candidate = database
            .discover_screenshot_group_candidates(scope_id)
            .expect("initial evidence should persist")
            .1
            .remove(0);

        database
            .connection
            .execute(
                "DELETE FROM scope_access_grants WHERE scope_id = ?1",
                [scope_id],
            )
            .expect("test grant should be removable");
        assert!(matches!(
            database.discover_screenshot_group_candidates(scope_id),
            Err(DatabaseError::ScopeAccessGrantNotActive)
        ));
        assert!(matches!(
            database.screenshot_group_candidate(candidate.group_id),
            Err(DatabaseError::ScopeAccessGrantNotActive)
        ));
        assert!(
            !database
                .recent_screenshot_group_candidates()
                .expect("missing-grant history should remain readable")
                .remove(0)
                .current_evidence
        );

        database
            .upsert_scope_access_grant(scope_id, std::env::consts::OS, b"replacement-grant")
            .expect("test grant should reactivate");
        database
            .mark_scope_access_grant_revoked(scope_id)
            .expect("test grant should revoke");
        assert!(matches!(
            database.discover_screenshot_group_candidates(scope_id),
            Err(DatabaseError::ScopeAccessGrantNotActive)
        ));
        assert!(matches!(
            database.screenshot_group_candidate(candidate.group_id),
            Err(DatabaseError::ScreenshotGroupCandidateNotFound)
        ));
        assert!(
            database
                .recent_screenshot_group_candidates()
                .expect("revocation should leave no derived screenshot history")
                .is_empty()
        );
    }

    #[test]
    fn screenshot_group_member_removal_invalidates_complete_membership() {
        let (mut database, scope_id, sources) = screenshot_group_setup();
        let candidate = database
            .discover_screenshot_group_candidates(scope_id)
            .expect("initial evidence should persist")
            .1
            .remove(0);
        database
            .connection
            .execute(
                "UPDATE locations SET present = 0 WHERE id = ?1",
                [sources[0].location_id],
            )
            .expect("test member should become absent");

        assert!(matches!(
            database.screenshot_group_candidate(candidate.group_id),
            Err(DatabaseError::ScreenshotGroupCandidateNotCurrent)
        ));
        assert!(
            !database
                .recent_screenshot_group_candidates()
                .expect("removed-member history should remain readable")
                .remove(0)
                .current_evidence
        );
        let discovery = database
            .discover_screenshot_group_candidates(scope_id)
            .expect("single remaining image should not form a group");
        assert_eq!(discovery.0, 1);
        assert!(discovery.1.is_empty());
    }

    #[test]
    fn screenshot_group_source_snapshot_and_persistence_share_one_immediate_transaction() {
        let directory = tempfile::tempdir().expect("fixture directory should exist");
        let path = directory.path().join("screenshot-groups.sqlite3");
        let (mut database, scope_id, sources) = screenshot_group_setup_in(
            ManifestDatabase::open(&path).expect("database should initialize"),
        );
        let competing = Connection::open(&path).expect("competing writer should open");
        competing
            .busy_timeout(Duration::ZERO)
            .expect("competing writer should fail immediately");
        let mut writer_was_blocked = false;
        let discovery = database
            .discover_screenshot_group_candidates_with_hook(scope_id, || {
                match competing.execute(
                    "UPDATE files SET size_bytes = size_bytes + 1 WHERE node_id = ?1",
                    [sources[0].node_id],
                ) {
                    Err(error)
                        if matches!(
                            error.sqlite_error_code(),
                            Some(
                                rusqlite::ErrorCode::DatabaseBusy
                                    | rusqlite::ErrorCode::DatabaseLocked
                            )
                        ) =>
                    {
                        writer_was_blocked = true;
                        Ok(())
                    }
                    Err(error) => Err(DatabaseError::Sqlite(error)),
                    Ok(_) => Err(DatabaseError::ScreenshotGroupCandidateNotCurrent),
                }
            })
            .expect("atomic discovery should complete");
        assert!(writer_was_blocked);
        assert_eq!(discovery.1.len(), 1);
        assert_eq!(discovery.1[0].members.len(), 2);
    }

    #[test]
    fn project_candidate_feedback_is_append_only_validated_and_idempotent() {
        let (mut database, scope_id, root_node_id) = project_setup();
        let facts = database
            .folder_profile_facts(scope_id, root_node_id, MAX_FOLDER_PROFILE_ENTRIES)
            .expect("profile facts should load");
        let suggestion = ProjectSuggestion {
            confidence_basis_points: 8_500,
            provenance: vec![ProjectSignal {
                kind: ProjectSignalKind::CargoManifest,
                marker_name: "Cargo.toml".to_string(),
                weight_basis_points: 8_500,
            }],
            observed_at_unix_ms: facts.observed_at_unix_ms,
            created_by: ProjectSuggestionCreator::SystemRule,
            provider_id: ProjectSuggestion::PROVIDER_ID,
            provider_version: ProjectSuggestion::PROVIDER_VERSION,
            model_version: None,
        };
        let candidate = database
            .record_project_candidate(scope_id, root_node_id, &suggestion)
            .expect("candidate should persist");
        let repeated = database
            .record_project_candidate(scope_id, root_node_id, &suggestion)
            .expect("same observation should be idempotent");
        assert_eq!(candidate.project_id, repeated.project_id);
        assert_eq!(candidate.state, ProjectCandidateState::Suggested);
        assert_eq!(
            database
                .connection
                .query_row("SELECT COUNT(*) FROM project_suggestions", [], |row| {
                    row.get::<_, i64>(0)
                })
                .expect("suggestions should count"),
            1
        );

        let rejected = database
            .decide_project_candidate(candidate.project_id, ProjectDecisionKind::Rejected)
            .expect("candidate should reject");
        let repeated_rejection = database
            .decide_project_candidate(candidate.project_id, ProjectDecisionKind::Rejected)
            .expect("same decision should be idempotent");
        assert_eq!(rejected.latest_decision, repeated_rejection.latest_decision);
        let accepted = database
            .decide_project_candidate(candidate.project_id, ProjectDecisionKind::Accepted)
            .expect("candidate should accept after correction");
        assert_eq!(accepted.state, ProjectCandidateState::Accepted);
        assert_eq!(
            accepted
                .latest_decision
                .as_ref()
                .map(|event| event.sequence),
            Some(2)
        );
        assert_eq!(
            database
                .connection
                .query_row("SELECT COUNT(*) FROM project_feedback_events", [], |row| {
                    row.get::<_, i64>(0)
                })
                .expect("feedback should count"),
            2
        );

        let mut invalid = suggestion.clone();
        invalid.confidence_basis_points = 8_501;
        assert!(matches!(
            database.record_project_candidate(scope_id, root_node_id, &invalid),
            Err(DatabaseError::ProjectCandidateInputInvalid)
        ));
        assert!(
            database
                .connection
                .execute(
                    "UPDATE projects SET created_at_unix_ms = created_at_unix_ms + 1 WHERE id = ?1",
                    [candidate.project_id],
                )
                .is_err()
        );
        assert!(
            database
                .connection
                .execute(
                    "DELETE FROM project_suggestion_signals WHERE suggestion_id = ( \
                    SELECT id FROM project_suggestions WHERE project_id = ?1 LIMIT 1 \
                 )",
                    [candidate.project_id],
                )
                .is_err()
        );
        assert!(
            database
                .connection
                .execute(
                    "DELETE FROM project_feedback_events WHERE project_id = ?1",
                    [candidate.project_id],
                )
                .is_err()
        );
    }

    #[test]
    fn exact_duplicate_candidate_is_append_only_and_snapshot_validated() {
        let (mut database, left, right) = exact_duplicate_setup();
        let candidate = database
            .record_exact_duplicate_candidate(&left, &right)
            .expect("candidate should persist");
        assert_eq!(candidate.kind, FileRelationKind::ExactDuplicate);
        assert_eq!(candidate.state, FileRelationCandidateState::Suggested);
        assert_eq!(candidate.left.node_id, left.node_id);
        assert_eq!(candidate.right.node_id, right.node_id);
        assert_eq!(candidate.evidence.compared_bytes, 4);
        assert_eq!(candidate.evidence.confidence_basis_points, 10_000);
        assert_eq!(candidate.evidence.model_version, None);

        let repeated = database
            .record_exact_duplicate_candidate(&left, &right)
            .expect("a new immutable observation should append");
        assert_eq!(repeated.relation_id, candidate.relation_id);
        let observation_count: i64 = database
            .connection
            .query_row(
                "SELECT COUNT(*) FROM file_relation_observations WHERE relation_id = ?1",
                [candidate.relation_id],
                |row| row.get(0),
            )
            .expect("observation count should load");
        assert_eq!(observation_count, 2);

        let rejected = database
            .decide_file_relation_candidate(
                candidate.relation_id,
                FileRelationDecisionKind::Rejected,
            )
            .expect("candidate should reject");
        assert_eq!(rejected.state, FileRelationCandidateState::Rejected);
        assert_eq!(
            rejected
                .latest_decision
                .as_ref()
                .map(|decision| decision.sequence),
            Some(1)
        );
        let repeated_rejection = database
            .decide_file_relation_candidate(
                candidate.relation_id,
                FileRelationDecisionKind::Rejected,
            )
            .expect("same decision should be idempotent");
        assert_eq!(repeated_rejection.latest_decision, rejected.latest_decision);
        let observed_after_rejection = database
            .record_exact_duplicate_candidate(&left, &right)
            .expect("new evidence should retain pair-specific feedback");
        assert_eq!(
            observed_after_rejection.state,
            FileRelationCandidateState::Rejected
        );
        let accepted = database
            .decide_file_relation_candidate(
                candidate.relation_id,
                FileRelationDecisionKind::Accepted,
            )
            .expect("candidate should accept after correction");
        assert_eq!(accepted.state, FileRelationCandidateState::Accepted);
        assert_eq!(
            accepted
                .latest_decision
                .as_ref()
                .map(|decision| decision.sequence),
            Some(2)
        );
        let repeated_acceptance = database
            .decide_file_relation_candidate(
                candidate.relation_id,
                FileRelationDecisionKind::Accepted,
            )
            .expect("same acceptance should be idempotent");
        assert_eq!(
            repeated_acceptance.latest_decision,
            accepted.latest_decision
        );
        assert_eq!(
            database
                .connection
                .query_row(
                    "SELECT COUNT(*) FROM file_relation_feedback_events WHERE relation_id = ?1",
                    [candidate.relation_id],
                    |row| row.get::<_, i64>(0),
                )
                .expect("feedback count should load"),
            2
        );
        let summaries = database
            .recent_file_relation_candidates()
            .expect("relation summaries should load");
        assert_eq!(summaries.len(), 1);
        assert_eq!(summaries[0].state, FileRelationCandidateState::Accepted);
        assert!(summaries[0].verification_required);
        assert_eq!(summaries[0].left_node_id, left.node_id);
        assert_eq!(summaries[0].right_node_id, right.node_id);

        let mut stale_left = left.clone();
        stale_left.modified_unix_ns = Some(3);
        assert!(matches!(
            database.record_exact_duplicate_candidate(&stale_left, &right),
            Err(DatabaseError::FileRelationCandidateNotCurrent)
        ));
        assert!(matches!(
            database.record_exact_duplicate_candidate(&right, &left),
            Err(DatabaseError::FileRelationCandidateInputInvalid)
        ));

        assert!(
            database
                .connection
                .execute(
                    "UPDATE file_relation_candidates SET scope_id = scope_id WHERE id = ?1",
                    [candidate.relation_id],
                )
                .is_err()
        );
        assert!(
            database
                .connection
                .execute(
                    "UPDATE file_relation_feedback_events SET decision = decision \
                     WHERE relation_id = ?1",
                    [candidate.relation_id],
                )
                .is_err()
        );
        assert!(
            database
                .connection
                .execute(
                    "DELETE FROM file_relation_feedback_events WHERE relation_id = ?1",
                    [candidate.relation_id],
                )
                .is_err()
        );
        assert!(
            database
                .connection
                .execute(
                    "DELETE FROM file_relation_candidates WHERE id = ?1",
                    [candidate.relation_id],
                )
                .is_err()
        );
        assert!(
            database
                .connection
                .execute(
                    "UPDATE file_relation_observations SET compared_bytes = compared_bytes \
                     WHERE relation_id = ?1",
                    [candidate.relation_id],
                )
                .is_err()
        );
        assert!(
            database
                .connection
                .execute(
                    "DELETE FROM file_relation_observations WHERE relation_id = ?1",
                    [candidate.relation_id],
                )
                .is_err()
        );

        database
            .connection
            .execute(
                "UPDATE locations SET present = 0 WHERE id = ?1",
                [left.location_id],
            )
            .expect("location should become absent");
        assert!(matches!(
            database.file_relation_candidate(candidate.relation_id),
            Err(DatabaseError::FileRelationCandidateNotCurrent)
        ));
    }

    #[test]
    fn cleanup_action_preview_is_independent_immutable_and_observation_bound() {
        let (mut database, selection, source, keeper) = cleanup_exact_duplicate_setup();
        let sha256 = [7_u8; 32];
        let preview = database
            .create_cleanup_action_plan(cleanup_exact_duplicate_plan_write(
                selection, &source, &keeper, &sha256, &sha256,
            ))
            .expect("bound preview should persist");
        assert_eq!(preview.state, CleanupActionPlanState::Previewed);
        assert_eq!(
            preview.source_observation_id,
            selection.source_observation_id
        );
        assert_eq!(preview.target_node_id, selection.target_node_id);
        assert!(preview.policy.confirmation_required);
        assert!(!preview.policy.action_authorized);
        assert!(!preview.policy.execution_available);
        assert!(preview.keeper_hash_bound);
        assert_eq!(preview.journal_sequence, 1);
        assert_eq!(
            database
                .connection
                .query_row(
                    "SELECT COUNT(*) FROM cleanup_action_plans WHERE id = ?1 \
                       AND keeper_location_id = ?2 \
                       AND keeper_identity_kind = ?3 AND keeper_identity_key = ?4 \
                       AND keeper_size_bytes = ?5 AND keeper_modified_unix_ns IS ?6 \
                       AND keeper_sha256 = ?7 AND keeper_hash_bytes = ?5 \
                       AND keeper_scope_root_node_id = ?8 \
                       AND keeper_scope_root_identity_kind = ?9 \
                       AND keeper_scope_root_identity_key = ?10 \
                       AND keeper_parent_node_id = ?11 \
                       AND keeper_parent_identity_kind = ?12 \
                       AND keeper_parent_identity_key = ?13",
                    params![
                        preview.plan_id,
                        keeper.source.location_id,
                        keeper.source.identity_kind,
                        keeper.source.identity_key,
                        to_i64(keeper.source.size_bytes).expect("keeper size should fit"),
                        keeper.source.modified_unix_ns,
                        sha256,
                        keeper.scope_root_node_id,
                        keeper.scope_root_identity_kind,
                        keeper.scope_root_identity_key,
                        keeper.parent_node_id,
                        keeper.parent_identity_kind,
                        keeper.parent_identity_key,
                    ],
                    |row| row.get::<_, i64>(0),
                )
                .expect("keeper binding should load"),
            1,
            "the full keeper snapshot and hash must be durably bound"
        );
        assert_eq!(
            database
                .connection
                .query_row("SELECT COUNT(*) FROM action_plans", [], |row| {
                    row.get::<_, i64>(0)
                })
                .expect("rename plan count should load"),
            0,
            "cleanup preview must not widen or reuse rename action_plans"
        );
        assert!(
            database
                .connection
                .execute(
                    "UPDATE cleanup_action_plans SET target_size_bytes = target_size_bytes \
                     WHERE id = ?1",
                    [preview.plan_id]
                )
                .is_err()
        );
        assert!(
            database
                .connection
                .execute(
                    "UPDATE cleanup_action_plans SET keeper_sha256 = zeroblob(32) WHERE id = ?1",
                    [preview.plan_id]
                )
                .is_err()
        );
        assert!(
            database
                .connection
                .execute(
                    "DELETE FROM cleanup_action_plans WHERE id = ?1",
                    [preview.plan_id]
                )
                .is_err()
        );
        assert!(
            database
                .connection
                .execute(
                    "DELETE FROM cleanup_action_journal_events WHERE plan_id = ?1",
                    [preview.plan_id]
                )
                .is_err()
        );
        let (left, right) = database
            .exact_duplicate_sources(selection.source_id)
            .expect("relation sources should remain current");
        database
            .record_exact_duplicate_candidate(&left, &right)
            .expect("refresh should append a new observation");
        assert!(matches!(
            database.cleanup_action_source(selection),
            Err(DatabaseError::CleanupActionSourceNotCurrent)
        ));
    }

    #[test]
    fn cleanup_exact_duplicate_plan_rejects_mismatched_final_hashes() {
        let (mut database, selection, source, keeper) = cleanup_exact_duplicate_setup();
        let target_sha256 = [7_u8; 32];
        let keeper_sha256 = [8_u8; 32];
        assert!(matches!(
            database.create_cleanup_action_plan(cleanup_exact_duplicate_plan_write(
                selection,
                &source,
                &keeper,
                &target_sha256,
                &keeper_sha256,
            )),
            Err(DatabaseError::CleanupActionPlanInputInvalid)
        ));
        assert!(
            database
                .connection
                .execute(
                    "INSERT INTO cleanup_action_plans( \
                         api_version, policy_version, operation, state, scope_id, source_kind, \
                         source_id, source_observation_id, keeper_node_id, keeper_location_id, \
                         keeper_identity_kind, keeper_identity_key, keeper_size_bytes, \
                         keeper_modified_unix_ns, keeper_sha256, keeper_hash_bytes, \
                         keeper_scope_root_node_id, keeper_scope_root_identity_kind, \
                         keeper_scope_root_identity_key, keeper_parent_node_id, \
                         keeper_parent_identity_kind, keeper_parent_identity_key, target_node_id, \
                         target_location_id, target_identity_kind, target_identity_key, \
                         target_size_bytes, target_modified_unix_ns, target_sha256, \
                         target_hash_bytes, scope_root_node_id, scope_root_identity_kind, \
                         scope_root_identity_key, parent_node_id, parent_identity_kind, \
                         parent_identity_key, confirmation_required, action_authorized, \
                         execution_available, created_at_unix_ms \
                     ) VALUES ( \
                         'deskgraph.cleanup-action-plan.v1', \
                         'deskgraph.cleanup-action-policy.v1', 'system_trash_preview', \
                         'previewed', ?1, 'exact_duplicate', ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, \
                         ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20, ?21, ?22, \
                         ?23, ?24, ?25, ?26, ?27, ?28, ?29, ?30, ?31, 1, 0, 0, 1 \
                     )",
                    params![
                        selection.scope_id,
                        selection.source_id,
                        selection.source_observation_id,
                        selection.keeper_node_id,
                        keeper.source.location_id,
                        keeper.source.identity_kind,
                        keeper.source.identity_key,
                        to_i64(keeper.source.size_bytes).expect("keeper size should fit"),
                        keeper.source.modified_unix_ns,
                        keeper_sha256,
                        to_i64(keeper.source.size_bytes).expect("keeper hash bytes should fit"),
                        keeper.scope_root_node_id,
                        keeper.scope_root_identity_kind,
                        keeper.scope_root_identity_key,
                        keeper.parent_node_id,
                        keeper.parent_identity_kind,
                        keeper.parent_identity_key,
                        selection.target_node_id,
                        source.source.location_id,
                        source.source.identity_kind,
                        source.source.identity_key,
                        to_i64(source.source.size_bytes).expect("target size should fit"),
                        source.source.modified_unix_ns,
                        target_sha256,
                        to_i64(source.source.size_bytes).expect("target hash bytes should fit"),
                        source.scope_root_node_id,
                        source.scope_root_identity_kind,
                        source.scope_root_identity_key,
                        source.parent_node_id,
                        source.parent_identity_kind,
                        source.parent_identity_key,
                    ],
                )
                .is_err(),
            "the SQLite schema must independently reject mismatched exact-duplicate hashes"
        );
    }

    #[test]
    fn cleanup_action_preview_rejects_invalid_keeper_and_inactive_scope() {
        let (database, selection, _, _) = cleanup_exact_duplicate_setup();
        let invalid_keeper = CleanupActionSelection {
            keeper_node_id: Some(selection.target_node_id),
            ..selection
        };
        assert!(matches!(
            database.cleanup_action_source(invalid_keeper),
            Err(DatabaseError::CleanupActionPlanInputInvalid)
        ));
        database
            .mark_scope_access_grant_revoked(selection.scope_id)
            .expect("scope grant should revoke");
        assert!(matches!(
            database.cleanup_action_source(selection),
            Err(DatabaseError::ScopeAccessGrantNotActive)
        ));
    }

    #[test]
    fn cleanup_keeper_binding_check_allows_all_nulls_only_for_screenshot_groups() {
        let (database, selection, source, keeper) = cleanup_exact_duplicate_setup();
        let insert =
            |source_kind: &str, keeper_node_id: Option<i64>, keeper_location_id: Option<i64>| {
                database.connection.execute(
                    "INSERT INTO cleanup_action_plans( \
                     api_version, policy_version, operation, state, scope_id, source_kind, \
                     source_id, source_observation_id, keeper_node_id, keeper_location_id, \
                     target_node_id, target_location_id, target_identity_kind, \
                     target_identity_key, target_size_bytes, target_modified_unix_ns, \
                     target_sha256, target_hash_bytes, scope_root_node_id, \
                     scope_root_identity_kind, scope_root_identity_key, parent_node_id, \
                     parent_identity_kind, parent_identity_key, confirmation_required, \
                     action_authorized, execution_available, created_at_unix_ms \
                 ) VALUES ( \
                     'deskgraph.cleanup-action-plan.v1', \
                     'deskgraph.cleanup-action-policy.v1', 'system_trash_preview', \
                     'previewed', ?1, ?18, ?2, ?3, ?4, ?5, ?6, ?7, \
                     ?8, ?9, ?10, ?11, zeroblob(32), ?10, ?12, ?13, ?14, ?15, \
                     ?16, ?17, 1, 0, 0, 1 \
                 )",
                    params![
                        selection.scope_id,
                        selection.source_id,
                        selection.source_observation_id,
                        keeper_node_id,
                        keeper_location_id,
                        selection.target_node_id,
                        source.source.location_id,
                        source.source.identity_kind,
                        source.source.identity_key,
                        to_i64(source.source.size_bytes).expect("target size should fit"),
                        source.source.modified_unix_ns,
                        source.scope_root_node_id,
                        source.scope_root_identity_kind,
                        source.scope_root_identity_key,
                        source.parent_node_id,
                        source.parent_identity_kind,
                        source.parent_identity_key,
                        source_kind,
                    ],
                )
            };

        assert!(
            insert(
                "screenshot_review_group",
                selection.keeper_node_id,
                Some(keeper.source.location_id)
            )
            .is_err(),
            "SQLite NULL semantics must not admit a partially bound keeper"
        );
        assert!(
            insert("exact_duplicate", None, None).is_err(),
            "exact duplicates must never omit the full keeper binding"
        );
        assert!(
            insert("version", None, None).is_err(),
            "version previews must never omit the full keeper binding"
        );
        assert_eq!(
            insert("screenshot_review_group", None, None)
                .expect("screenshot review may explicitly omit a keeper"),
            1
        );
    }

    #[test]
    fn cleanup_selection_binds_version_and_screenshot_members_to_exact_observations() {
        let (mut version_database, older, newer) = file_version_setup();
        version_database
            .upsert_scope_access_grant(older.scope_id, "macos", b"test-active-grant")
            .expect("version scope should activate");
        let version = version_database
            .record_file_version_candidate(&older, &newer)
            .expect("version evidence should persist");
        let version_item = version_database
            .smart_cleanup_relation_item(version.relation_id, version.evidence.observed_at_unix_ms)
            .expect("version should map to current cleanup evidence");
        let version_selection = CleanupActionSelection {
            scope_id: older.scope_id,
            source_kind: SmartCleanupSourceKind::Version,
            source_id: version.relation_id,
            source_observation_id: version_item.source_observation_id,
            keeper_node_id: Some(version.newer.node_id),
            target_node_id: version.older.node_id,
        };
        let version_snapshot =
            cleanup_selection_snapshot(&version_database.connection, &version_selection)
                .expect("exact version observation should resolve selected members");
        assert_eq!(version_snapshot.location_id, version.older.location_id);
        assert!(matches!(
            cleanup_selection_snapshot(
                &version_database.connection,
                &CleanupActionSelection {
                    keeper_node_id: Some(version.older.node_id),
                    target_node_id: version.newer.node_id,
                    ..version_selection
                }
            ),
            Err(DatabaseError::CleanupActionPlanInputInvalid)
        ));
        assert!(matches!(
            cleanup_selection_snapshot(
                &version_database.connection,
                &CleanupActionSelection {
                    source_observation_id: version_item.source_observation_id + 1,
                    ..version_selection
                }
            ),
            Err(DatabaseError::CleanupActionSourceNotCurrent)
        ));

        let (mut screenshot_database, scope_id, _) = screenshot_group_setup();
        let group = screenshot_database
            .discover_screenshot_group_candidates(scope_id)
            .expect("screenshot evidence should persist")
            .1
            .remove(0);
        let screenshot_selection = CleanupActionSelection {
            scope_id,
            source_kind: SmartCleanupSourceKind::ScreenshotReviewGroup,
            source_id: group.group_id,
            source_observation_id: group.evidence.observation_id,
            keeper_node_id: Some(group.members[0].node_id),
            target_node_id: group.members[1].node_id,
        };
        let screenshot_snapshot =
            cleanup_selection_snapshot(&screenshot_database.connection, &screenshot_selection)
                .expect("exact screenshot observation should resolve selected members");
        assert_eq!(
            screenshot_snapshot.location_id,
            group.members[1].location_id
        );
        assert!(matches!(
            cleanup_selection_snapshot(
                &screenshot_database.connection,
                &CleanupActionSelection {
                    keeper_node_id: Some(999_999),
                    ..screenshot_selection
                }
            ),
            Err(DatabaseError::CleanupActionPlanInputInvalid)
        ));
    }

    #[test]
    fn file_version_candidate_is_directional_append_only_and_path_free_in_history() {
        let (mut database, first, second) = file_version_setup();
        let candidate = database
            .record_file_version_candidate(&first, &second)
            .expect("explicit versions should persist");
        assert_eq!(candidate.kind, FileRelationKind::Version);
        assert_eq!(candidate.state, FileRelationCandidateState::Suggested);
        assert_eq!(candidate.older.node_id, first.node_id);
        assert_eq!(candidate.newer.node_id, second.node_id);
        assert_eq!(candidate.evidence.base_key, "企劃");
        assert_eq!(candidate.evidence.extension_key, "md");
        assert_eq!(candidate.evidence.older_version, 1);
        assert_eq!(candidate.evidence.newer_version, 2);
        assert_eq!(candidate.evidence.confidence_basis_points, 9_000);
        assert_eq!(candidate.evidence.model_version, None);
        assert_eq!(candidate.latest_decision, None);

        let reversed = database
            .record_file_version_candidate(&second, &first)
            .expect("reversed inputs should reuse stable relation identity");
        assert_eq!(reversed.relation_id, candidate.relation_id);
        assert_eq!(
            database
                .connection
                .query_row(
                    "SELECT COUNT(*) FROM file_version_observations WHERE relation_id = ?1",
                    [candidate.relation_id],
                    |row| row.get::<_, i64>(0),
                )
                .expect("version observations should count"),
            2
        );
        let summaries = database
            .recent_file_relation_candidates()
            .expect("relation history should load");
        assert_eq!(summaries.len(), 1);
        assert_eq!(summaries[0].kind, FileRelationKind::Version);
        assert_eq!(summaries[0].state, FileRelationCandidateState::Suggested);
        assert_eq!(summaries[0].confidence_basis_points, 9_000);
        assert!(summaries[0].verification_required);
        assert_eq!(summaries[0].latest_decision_at_unix_ms, None);
        let mut mismatched = second.clone();
        mismatched.display_path = "/scope/其他-v2.md".to_string();
        assert!(matches!(
            database.record_file_version_candidate(&first, &mismatched),
            Err(DatabaseError::FileRelationCandidateInputInvalid)
        ));
        let mut same_version = second.clone();
        same_version.display_path = "/scope/企劃-v1.md".to_string();
        assert!(matches!(
            database.record_file_version_candidate(&first, &same_version),
            Err(DatabaseError::FileRelationCandidateInputInvalid)
        ));
        assert!(
            database
                .connection
                .execute(
                    "UPDATE file_version_observations SET newer_version = newer_version \
                     WHERE relation_id = ?1",
                    [candidate.relation_id],
                )
                .is_err()
        );
        assert!(
            database
                .connection
                .execute(
                    "DELETE FROM file_version_observations WHERE relation_id = ?1",
                    [candidate.relation_id],
                )
                .is_err()
        );
        database
            .connection
            .execute(
                "UPDATE locations SET present = 0 WHERE id = ?1",
                [first.location_id],
            )
            .expect("location should become absent");
        assert!(matches!(
            database.file_version_candidate(candidate.relation_id),
            Err(DatabaseError::FileRelationCandidateNotCurrent)
        ));
    }

    #[test]
    fn file_version_feedback_is_append_only_idempotent_and_direction_bound() {
        let (mut database, first, second) = file_version_setup();
        let candidate = database
            .record_file_version_candidate(&first, &second)
            .expect("version candidate should persist");
        let rejected = database
            .decide_file_version_candidate(
                candidate.relation_id,
                FileRelationDecisionKind::Rejected,
            )
            .expect("current evidence should be rejectable");
        assert_eq!(rejected.state, FileRelationCandidateState::Rejected);
        assert_eq!(
            rejected
                .latest_decision
                .as_ref()
                .expect("decision should exist")
                .sequence,
            1
        );

        let reverified = database
            .record_file_version_candidate(&first, &second)
            .expect("equivalent evidence should append an observation");
        assert_eq!(reverified.state, FileRelationCandidateState::Rejected);
        let idempotent = database
            .decide_file_version_candidate(
                candidate.relation_id,
                FileRelationDecisionKind::Rejected,
            )
            .expect("repeated equivalent decision should succeed");
        assert_eq!(
            idempotent
                .latest_decision
                .as_ref()
                .expect("decision should exist")
                .sequence,
            1
        );
        let accepted = database
            .decide_file_version_candidate(
                candidate.relation_id,
                FileRelationDecisionKind::Accepted,
            )
            .expect("opposite decision should append a correction");
        assert_eq!(accepted.state, FileRelationCandidateState::Accepted);
        assert_eq!(
            accepted
                .latest_decision
                .as_ref()
                .expect("decision should exist")
                .sequence,
            2
        );

        let changed_path = "/scope/企劃-v3.md";
        database
            .connection
            .execute(
                "UPDATE locations SET path_raw = ?1, path_key = ?2, display_path = ?2 \
                 WHERE id = ?3",
                params![changed_path.as_bytes(), changed_path, first.location_id,],
            )
            .expect("fixture rename should update current location");
        database
            .connection
            .execute(
                "INSERT INTO file_version_observations( \
                     relation_id, older_location_id, newer_location_id, older_size_bytes, \
                     newer_size_bytes, older_modified_unix_ns, newer_modified_unix_ns, \
                     base_key, extension_key, older_version, newer_version, \
                     confidence_basis_points, signal_kind, created_by, provider_id, \
                     provider_version, model_version, observed_at_unix_ms \
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, '企劃', 'md', 2, 3, 9000, \
                     'explicit_numeric_suffix', 'system_rule', 'deskgraph.filename-version', \
                     '1', NULL, ?8)",
                params![
                    candidate.relation_id,
                    second.location_id,
                    first.location_id,
                    to_i64(second.size_bytes).expect("size should fit"),
                    to_i64(first.size_bytes).expect("size should fit"),
                    second.modified_unix_ns,
                    first.modified_unix_ns,
                    accepted.evidence.observed_at_unix_ms,
                ],
            )
            .expect("new direction should append");
        let changed = database
            .file_version_candidate(candidate.relation_id)
            .expect("changed direction should load");
        assert_eq!(changed.older.node_id, second.node_id);
        assert_eq!(changed.newer.node_id, first.node_id);
        assert_eq!(changed.state, FileRelationCandidateState::Suggested);
        assert_eq!(changed.latest_decision, None);
        let changed_rejected = database
            .decide_file_version_candidate(
                candidate.relation_id,
                FileRelationDecisionKind::Rejected,
            )
            .expect("new direction should accept its own decision");
        assert_eq!(changed_rejected.state, FileRelationCandidateState::Rejected);
        assert_eq!(
            changed_rejected
                .latest_decision
                .as_ref()
                .expect("changed decision should exist")
                .sequence,
            3
        );

        let original_path = "/scope/企劃-v1.md";
        database
            .connection
            .execute(
                "UPDATE locations SET path_raw = ?1, path_key = ?2, display_path = ?2 \
                 WHERE id = ?3",
                params![original_path.as_bytes(), original_path, first.location_id],
            )
            .expect("fixture rename should restore current location");
        let restored_first = database
            .action_source_for_path_key(first.scope_id, original_path)
            .expect("restored source should load");
        let restored = database
            .record_file_version_candidate(&restored_first, &second)
            .expect("restored equivalent evidence should append");
        assert_eq!(restored.state, FileRelationCandidateState::Accepted);
        assert_eq!(
            restored
                .latest_decision
                .as_ref()
                .expect("restored decision should exist")
                .sequence,
            2
        );
        database
            .decide_file_version_candidate(
                candidate.relation_id,
                FileRelationDecisionKind::Accepted,
            )
            .expect("restored repeated decision should remain idempotent");
        assert_eq!(
            database
                .connection
                .query_row(
                    "SELECT COUNT(*) FROM file_version_feedback_events WHERE relation_id = ?1",
                    [candidate.relation_id],
                    |row| row.get::<_, i64>(0),
                )
                .expect("feedback events should count"),
            3
        );
        assert!(
            database
                .connection
                .execute(
                    "UPDATE file_version_feedback_events SET decision = decision \
                     WHERE relation_id = ?1",
                    [candidate.relation_id],
                )
                .is_err()
        );
        assert!(
            database
                .connection
                .execute(
                    "DELETE FROM file_version_feedback_events WHERE relation_id = ?1",
                    [candidate.relation_id],
                )
                .is_err()
        );
    }

    #[test]
    fn database_rejects_action_plan_when_manifest_snapshot_does_not_match() {
        let (mut database, scope_id, node_id, _) = extraction_setup();
        let execution_source = database
            .action_execution_source_for_path_key(scope_id, "/scope/file.txt")
            .expect("execution source should load");
        let source = &execution_source.source;
        let source_sha256 = [0xa5; 32];
        let error = database
            .create_rename_action_plan(ActionPlanWrite {
                scope_id,
                node_id,
                source_location_id: source.location_id,
                source_path_raw: &source.path_raw,
                source_path_key: &source.path_key,
                source_display_path: &source.display_path,
                destination_path_raw: b"/scope/renamed.txt",
                destination_path_key: "/scope/renamed.txt",
                destination_display_path: "/scope/renamed.txt",
                source_identity_kind: &source.identity_kind,
                source_identity_key: &source.identity_key,
                source_size_bytes: source.size_bytes + 1,
                source_modified_unix_ns: source.modified_unix_ns,
                source_sha256: &source_sha256,
                source_hash_bytes: source.size_bytes + 1,
                scope_root_identity_kind: &execution_source.scope_root_identity_kind,
                scope_root_identity_key: &execution_source.scope_root_identity_key,
                parent_identity_kind: &execution_source.parent_identity_kind,
                parent_identity_key: &execution_source.parent_identity_key,
                execution_strategy: ActionExecutionStrategy::Direct,
            })
            .expect_err("database boundary must reject stale snapshot");
        assert!(matches!(error, DatabaseError::ActionSourceSnapshotChanged));
        assert!(
            database
                .recent_action_plans()
                .expect("summaries should load")
                .is_empty()
        );
    }

    #[test]
    fn v3_content_migration_preserves_exact_byte_provenance() {
        let connection = Connection::open_in_memory().expect("connection should open");
        connection
            .execute_batch(
                "CREATE TABLE schema_migrations (version INTEGER PRIMARY KEY, name TEXT NOT NULL, checksum TEXT NOT NULL, applied_at_unix_ms INTEGER NOT NULL);",
            )
            .expect("migration registry should initialize");
        for migration in &MIGRATIONS[..3] {
            connection
                .execute_batch(migration.sql)
                .expect("legacy migration should apply");
            connection
                .execute(
                    "INSERT INTO schema_migrations(version, name, checksum, applied_at_unix_ms) VALUES (?1, ?2, ?3, 0)",
                    params![
                        migration.version,
                        migration.name,
                        migration_checksum(migration.sql)
                    ],
                )
                .expect("legacy migration should register");
        }
        connection
            .execute_batch(
                "INSERT INTO authorized_scopes VALUES (1, X'2F73636F7065', '/scope', '/scope', 'macos', 0);\
                 INSERT INTO scan_jobs(id, scope_id, status, started_at_unix_ms) VALUES (1, 1, 'completed', 0);\
                 INSERT INTO nodes VALUES (1, 'file', 'test', X'01', 0, 0);\
                 INSERT INTO files VALUES (1, 4, 1, 1);\
                 INSERT INTO locations VALUES (1, 1, 1, X'2F73636F70652F66696C652E747874', '/scope/file.txt', '/scope/file.txt', 1, 1);\
                 INSERT INTO extraction_jobs(id, scope_id, node_id, location_id, status, source_size_bytes, source_modified_unix_ns, output_bytes, chunk_count, created_at_unix_ms, updated_at_unix_ms) VALUES (1, 1, 1, 1, 'completed', 4, 1, 6, 1, 0, 0);\
                 INSERT INTO content_chunks(id, scope_id, node_id, location_id, extraction_job_id, ordinal, text, source_byte_start, source_byte_end, source_size_bytes, source_modified_unix_ns, trust_class, provider_id, provider_version, active, created_at_unix_ms) VALUES (1, 1, 1, 1, 1, 0, 'legacy', 1, 3, 4, 1, 'untrusted_extracted_text', 'deskgraph.utf8-text', '1', 1, 0);",
            )
            .expect("legacy content fixture should initialize");

        let database = ManifestDatabase::from_connection(connection)
            .expect("new provenance migration should apply");
        let stored = database
            .connection
            .query_row(
                "SELECT provenance_kind, source_byte_start, source_byte_end, source_page_number, \
                    source_fragment_index, source_unit_number, source_cell_reference \
                 FROM content_chunks WHERE id = 1",
                [],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, Option<i64>>(1)?,
                        row.get::<_, Option<i64>>(2)?,
                        row.get::<_, Option<i64>>(3)?,
                        row.get::<_, Option<i64>>(4)?,
                        row.get::<_, Option<i64>>(5)?,
                        row.get::<_, Option<String>>(6)?,
                    ))
                },
            )
            .expect("migrated provenance should load");

        assert_eq!(
            stored,
            (
                "byte_range".to_string(),
                Some(1),
                Some(3),
                None,
                None,
                None,
                None,
            )
        );
        let candidates = database
            .lexical_search_candidates("\"legacy\"", lexical_filters(None), 10)
            .expect("search must fail closed before legacy scope reauthorization");
        assert!(candidates.is_empty());
        database
            .upsert_scope_access_grant(1, "macos", b"migration-test-grant")
            .expect("legacy scope should be explicitly reauthorized");
        let candidates = database
            .lexical_search_candidates("\"legacy\"", lexical_filters(None), 10)
            .expect("search migration should backfill existing content after reauthorization");
        assert_eq!(candidates.len(), 1);
        assert_eq!(candidates[0].source, LexicalCandidateSource::ExtractedText);
    }

    #[test]
    fn trigram_search_indexes_multilingual_metadata_and_only_active_content() {
        let (mut database, scope_id, node_id, root) = extraction_setup();
        test_active_binding(&database, scope_id).expect("search fixture scope should be active");
        database
            .connection
            .execute(
                "UPDATE locations SET display_path = '/scope/專案-context.md' WHERE node_id = ?1",
                [node_id],
            )
            .expect("display path should update through the FTS trigger");
        let text = "Traditional Chinese 專案脈絡 and English context stay local";
        let job = database
            .create_extraction_job(scope_id, node_id)
            .expect("job should create");
        database
            .claim_extraction_job(job.job_id, "search-runner", 60_000)
            .expect("job should claim");
        database
            .complete_extraction_job(
                job.job_id,
                "search-runner",
                "deskgraph.utf8-text",
                "1",
                4,
                Some(1),
                u64::try_from(text.len()).expect("fixture length should fit"),
                1,
                &[ContentChunkWrite {
                    ordinal: 0,
                    text: text.to_string(),
                    provenance: ContentChunkProvenanceWrite::ByteRange { start: 0, end: 4 },
                    trust_class: "untrusted_extracted_text",
                }],
            )
            .expect("content should publish");

        let metadata = database
            .lexical_search_candidates("\"專案-context\"", lexical_filters(Some(scope_id)), 10)
            .expect("metadata search should pass");
        assert!(
            metadata
                .iter()
                .any(|candidate| candidate.source == LexicalCandidateSource::MetadataPath)
        );
        let filtered_metadata = database
            .lexical_search_candidates(
                "\"專案-context\"",
                LexicalSearchFilters {
                    scope_id: Some(scope_id),
                    folder_node_id: None,
                    source: LexicalSearchSource::MetadataPath,
                    extension: Some("md"),
                    modified_since_unix_ns: Some(1),
                    modified_before_unix_ns: Some(2),
                },
                10,
            )
            .expect("bounded metadata filters should pass");
        assert_eq!(filtered_metadata.len(), 1);
        assert_eq!(
            filtered_metadata[0].source,
            LexicalCandidateSource::MetadataPath
        );
        let content = database
            .lexical_search_candidates("\"專案脈絡\"", lexical_filters(None), 10)
            .expect("content search should pass");
        let snippet = content
            .iter()
            .find(|candidate| candidate.source == LexicalCandidateSource::ExtractedText)
            .and_then(|candidate| candidate.snippet.as_deref())
            .expect("content result should include a bounded snippet");
        assert!(snippet.contains("專案脈絡"));

        let filtered_content = database
            .lexical_search_candidates(
                "\"專案脈絡\"",
                LexicalSearchFilters {
                    scope_id: Some(scope_id),
                    folder_node_id: None,
                    source: LexicalSearchSource::ExtractedText,
                    extension: Some("md"),
                    modified_since_unix_ns: Some(1),
                    modified_before_unix_ns: Some(2),
                },
                10,
            )
            .expect("bounded content filters should pass");
        assert_eq!(filtered_content.len(), 1);
        assert_eq!(
            filtered_content[0].source,
            LexicalCandidateSource::ExtractedText
        );
        for filters in [
            LexicalSearchFilters {
                source: LexicalSearchSource::MetadataPath,
                ..lexical_filters(Some(scope_id))
            },
            LexicalSearchFilters {
                extension: Some("txt"),
                ..lexical_filters(Some(scope_id))
            },
            LexicalSearchFilters {
                modified_since_unix_ns: Some(2),
                ..lexical_filters(Some(scope_id))
            },
        ] {
            assert!(
                database
                    .lexical_search_candidates("\"專案脈絡\"", filters, 10)
                    .expect("bounded non-matching filter should pass")
                    .is_empty()
            );
        }

        publish_manifest_file(&mut database, scope_id, &root, 5);
        let stale = database
            .lexical_search_candidates("\"專案脈絡\"", lexical_filters(None), 10)
            .expect("stale search should pass");
        assert!(
            stale
                .iter()
                .all(|candidate| candidate.source != LexicalCandidateSource::ExtractedText)
        );
    }

    #[test]
    fn folder_descendant_lookup_uses_the_target_traversal_index() {
        let (database, scope_id) = folder_search_setup();
        let selected_id = folder_search_node_id(&database, scope_id, "/scope/needle-project");
        let mut statement = database
            .connection
            .prepare(
                "EXPLAIN QUERY PLAN \
                 WITH RECURSIVE folder_tree(node_id) AS ( \
                     SELECT ?2 \
                     UNION \
                     SELECT edge.source_node_id \
                     FROM folder_tree parent \
                     CROSS JOIN edges edge ON edge.target_node_id=parent.node_id \
                     WHERE edge.scope_id=?1 AND edge.kind='located_in' AND edge.active=1 \
                 ) \
                 SELECT node_id FROM folder_tree",
            )
            .expect("folder traversal plan should prepare");
        let details = statement
            .query_map(params![scope_id, selected_id], |row| {
                row.get::<_, String>(3)
            })
            .expect("folder traversal plan should execute")
            .collect::<Result<Vec<_>, _>>()
            .expect("folder traversal plan rows should decode");
        assert!(
            details.iter().any(|detail| detail
                .contains("edges_scope_kind_active_target_source_idx")
                && detail.contains("target_node_id=?")),
            "recursive parent-to-child traversal must use the complete target lookup: {details:?}"
        );
    }

    #[test]
    fn content_search_plan_drives_from_fts_before_scope_filtering_chunks() {
        let (database, scope_id) = folder_search_setup();
        let mut statement = database
            .connection
            .prepare(
                "EXPLAIN QUERY PLAN \
                 SELECT c.id \
                 FROM content_search_fts \
                 CROSS JOIN content_chunks c ON c.id=content_search_fts.rowid \
                 WHERE content_search_fts MATCH ?1 AND c.scope_id=?2 AND c.active=1 \
                 ORDER BY content_search_fts.rank,c.node_id,c.ordinal \
                 LIMIT 20",
            )
            .expect("content search plan should prepare");
        let details = statement
            .query_map(params!["needle", scope_id], |row| row.get::<_, String>(3))
            .expect("content search plan should execute")
            .collect::<Result<Vec<_>, _>>()
            .expect("content search plan rows should decode");
        assert!(
            details
                .iter()
                .any(|detail| detail.contains("content_search_fts VIRTUAL TABLE INDEX 0:M1")),
            "content search must begin with the FTS match: {details:?}"
        );
        assert!(
            details
                .iter()
                .any(|detail| detail.contains("c USING INTEGER PRIMARY KEY")),
            "matched FTS rowids must probe content chunks by primary key: {details:?}"
        );
        assert!(
            details
                .iter()
                .all(|detail| !detail.contains("content_chunks_active_node_idx")),
            "scope filtering must not drive repeated FTS probes: {details:?}"
        );
    }

    #[test]
    fn folder_scoped_search_includes_self_direct_and_deep_descendants_only() {
        let (database, scope_id) = folder_search_setup();
        let selected_id = folder_search_node_id(&database, scope_id, "/scope/needle-project");
        let deep_folder_id =
            folder_search_node_id(&database, scope_id, "/scope/needle-project/needle-deep");
        let direct_id = folder_search_node_id(
            &database,
            scope_id,
            "/scope/needle-project/needle-direct.txt",
        );
        let deep_file_id = folder_search_node_id(
            &database,
            scope_id,
            "/scope/needle-project/needle-deep/needle-deep-file.txt",
        );
        let stale_id = folder_search_node_id(
            &database,
            scope_id,
            "/scope/needle-project/needle-stale.txt",
        );
        let sibling_id = folder_search_node_id(&database, scope_id, "/scope/needle-sibling.txt");

        for (path, text, active) in [
            (
                "/scope/needle-project/needle-direct.txt",
                "needle direct body",
                true,
            ),
            (
                "/scope/needle-project/needle-deep/needle-deep-file.txt",
                "needle deep body",
                true,
            ),
            (
                "/scope/needle-project/needle-stale.txt",
                "needle stale body",
                false,
            ),
            ("/scope/needle-sibling.txt", "needle sibling body", true),
        ] {
            insert_folder_search_content(&database, scope_id, path, text, active);
        }

        // A corrupt cycle must terminate deterministically. UNION deduplicates
        // node IDs while preserving the selected folder's descendant closure.
        let scan_id: i64 = database
            .connection
            .query_row(
                "SELECT MAX(id) FROM scan_jobs WHERE scope_id=?1 AND status='completed'",
                [scope_id],
                |row| row.get(0),
            )
            .expect("completed scan should load");
        database
            .connection
            .execute(
                "INSERT INTO edges( \
                     scope_id,source_node_id,target_node_id,kind,active,last_seen_scan_id \
                 ) VALUES(?1,?2,?3,'located_in',1,?4)",
                params![scope_id, selected_id, deep_folder_id, scan_id],
            )
            .expect("cycle fixture should persist");

        let metadata = database
            .lexical_search_candidates(
                "needle",
                LexicalSearchFilters {
                    scope_id: Some(scope_id),
                    folder_node_id: Some(selected_id),
                    source: LexicalSearchSource::MetadataPath,
                    ..lexical_filters(Some(scope_id))
                },
                20,
            )
            .expect("folder-scoped metadata search should pass");
        let metadata_ids = metadata
            .iter()
            .map(|candidate| candidate.node_id)
            .collect::<HashSet<_>>();
        assert!(
            metadata_ids.contains(&selected_id),
            "folder self must match"
        );
        assert!(metadata_ids.contains(&deep_folder_id));
        assert!(metadata_ids.contains(&direct_id));
        assert!(metadata_ids.contains(&deep_file_id));
        assert!(metadata_ids.contains(&stale_id));
        assert!(!metadata_ids.contains(&sibling_id));

        let content = database
            .lexical_search_candidates(
                "needle",
                LexicalSearchFilters {
                    scope_id: Some(scope_id),
                    folder_node_id: Some(selected_id),
                    source: LexicalSearchSource::ExtractedText,
                    ..lexical_filters(Some(scope_id))
                },
                20,
            )
            .expect("folder-scoped content search should pass");
        let content_ids = content
            .iter()
            .map(|candidate| candidate.node_id)
            .collect::<HashSet<_>>();
        assert_eq!(content_ids, HashSet::from([direct_id, deep_file_id]));
        assert!(!content_ids.contains(&stale_id));
        assert!(!content_ids.contains(&sibling_id));
    }

    #[test]
    fn folder_scoped_search_never_returns_a_sibling_hard_link_location() {
        let (database, scope_id) = folder_search_setup();
        let selected_id = folder_search_node_id(&database, scope_id, "/scope/needle-project");
        let selected_path = "/scope/needle-project/needle-direct.txt";
        let sibling_path = "/scope/needle-hardlink-sibling.txt";
        let shared_node_id = folder_search_node_id(&database, scope_id, selected_path);
        let root_node_id = folder_search_node_id(&database, scope_id, "/scope");
        let scan_id: i64 = database
            .connection
            .query_row(
                "SELECT MAX(id) FROM scan_jobs WHERE scope_id=?1 AND status='completed'",
                [scope_id],
                |row| row.get(0),
            )
            .expect("completed scan should load");
        database
            .connection
            .execute(
                "INSERT INTO locations( \
                     scope_id,node_id,path_raw,path_key,display_path,present,last_seen_scan_id \
                 ) VALUES(?1,?2,?3,?4,?4,1,?5)",
                params![
                    scope_id,
                    shared_node_id,
                    sibling_path.as_bytes(),
                    sibling_path,
                    scan_id
                ],
            )
            .expect("sibling hard-link location should persist");
        database
            .connection
            .execute(
                "INSERT INTO edges( \
                     scope_id,source_node_id,target_node_id,kind,active,last_seen_scan_id \
                 ) VALUES(?1,?2,?3,'located_in',1,?4)",
                params![scope_id, shared_node_id, root_node_id, scan_id],
            )
            .expect("sibling hard-link parent edge should persist");
        database
            .connection
            .execute(
                "UPDATE files SET link_count=2 WHERE node_id=?1",
                [shared_node_id],
            )
            .expect("hard-link count should update");
        insert_folder_search_content(
            &database,
            scope_id,
            selected_path,
            "needle hardlink shared body",
            true,
        );
        insert_folder_search_content(
            &database,
            scope_id,
            sibling_path,
            "needle hardlink shared body",
            true,
        );

        let metadata = database
            .lexical_search_candidates(
                "needle",
                LexicalSearchFilters {
                    scope_id: Some(scope_id),
                    folder_node_id: Some(selected_id),
                    source: LexicalSearchSource::MetadataPath,
                    ..lexical_filters(Some(scope_id))
                },
                20,
            )
            .expect("folder-scoped metadata search should pass");
        assert!(
            metadata
                .iter()
                .any(|candidate| candidate.display_path == selected_path)
        );
        assert!(
            metadata
                .iter()
                .all(|candidate| candidate.display_path != sibling_path),
            "node membership must not authorize a sibling hard-link location"
        );

        let content = database
            .lexical_search_candidates(
                "hardlink",
                LexicalSearchFilters {
                    scope_id: Some(scope_id),
                    folder_node_id: Some(selected_id),
                    source: LexicalSearchSource::ExtractedText,
                    ..lexical_filters(Some(scope_id))
                },
                20,
            )
            .expect("folder-scoped content search should pass");
        assert_eq!(content.len(), 1);
        assert_eq!(content[0].display_path, selected_path);
        assert_eq!(content[0].location_id, {
            database
                .connection
                .query_row(
                    "SELECT id FROM locations WHERE scope_id=?1 AND path_key=?2",
                    params![scope_id, selected_path],
                    |row| row.get::<_, i64>(0),
                )
                .expect("selected hard-link location should load")
        });
    }

    #[test]
    fn folder_search_selector_and_path_list_fail_closed_at_every_boundary() {
        let (mut database, scope_id) = folder_search_setup();
        let selected_path = "/scope/needle-project";
        let selected_id = folder_search_node_id(&database, scope_id, selected_path);
        let file_id = folder_search_node_id(
            &database,
            scope_id,
            "/scope/needle-project/needle-direct.txt",
        );

        let list = database
            .list_search_folders(scope_id, None)
            .expect("explicit folder list should load");
        assert_eq!(list.api_version, SearchFolderListResponse::API_VERSION);
        assert_eq!(list.scope_id, scope_id);
        assert_eq!(list.folder_count, 3);
        assert!(!list.truncated);
        assert!(
            list.folders
                .iter()
                .any(|folder| folder.folder_node_id == selected_id
                    && folder.display_path == selected_path)
        );
        let debug = format!("{list:?}");
        assert!(!debug.contains(selected_path));
        assert!(debug.contains("<redacted>"));

        let bounded = database
            .list_search_folders(scope_id, Some(2))
            .expect("bounded folder list should load");
        assert_eq!(bounded.folder_count, 2);
        assert_eq!(bounded.folders.len(), 2);
        assert!(bounded.truncated);
        for limit in [Some(0), Some(MAX_SEARCH_FOLDER_LIST_LIMIT + 1)] {
            assert!(matches!(
                database.list_search_folders(scope_id, limit),
                Err(DatabaseError::SearchInputInvalid)
            ));
        }

        assert!(matches!(
            database.lexical_search_candidates(
                "needle",
                LexicalSearchFilters {
                    scope_id: None,
                    folder_node_id: Some(selected_id),
                    ..lexical_filters(None)
                },
                10,
            ),
            Err(DatabaseError::SearchInputInvalid)
        ));
        assert!(matches!(
            database.lexical_search_candidates(
                "needle",
                LexicalSearchFilters {
                    scope_id: Some(scope_id),
                    folder_node_id: Some(0),
                    ..lexical_filters(Some(scope_id))
                },
                10,
            ),
            Err(DatabaseError::SearchInputInvalid)
        ));
        for invalid_folder_id in [file_id, i64::MAX] {
            assert!(matches!(
                database.lexical_search_candidates(
                    "needle",
                    LexicalSearchFilters {
                        scope_id: Some(scope_id),
                        folder_node_id: Some(invalid_folder_id),
                        ..lexical_filters(Some(scope_id))
                    },
                    10,
                ),
                Err(DatabaseError::SearchFolderInvalid)
            ));
        }

        let scan_id: i64 = database
            .connection
            .query_row(
                "SELECT MAX(id) FROM scan_jobs WHERE scope_id=?1 AND status='completed'",
                [scope_id],
                |row| row.get(0),
            )
            .expect("completed scan should load");
        let ambiguous_path = "/scope/needle-project-alias";
        database
            .connection
            .execute(
                "INSERT INTO locations( \
                     scope_id,node_id,path_raw,path_key,display_path,present,last_seen_scan_id \
                 ) VALUES(?1,?2,?3,?4,?4,1,?5)",
                params![
                    scope_id,
                    selected_id,
                    ambiguous_path.as_bytes(),
                    ambiguous_path,
                    scan_id
                ],
            )
            .expect("ambiguous folder location should persist");
        assert!(matches!(
            database.lexical_search_candidates(
                "needle",
                LexicalSearchFilters {
                    scope_id: Some(scope_id),
                    folder_node_id: Some(selected_id),
                    ..lexical_filters(Some(scope_id))
                },
                10,
            ),
            Err(DatabaseError::SearchFolderInvalid)
        ));
        database
            .connection
            .execute(
                "UPDATE locations SET present=0 WHERE scope_id=?1 AND path_key=?2",
                params![scope_id, ambiguous_path],
            )
            .expect("ambiguous folder location should become absent");

        let other = database
            .add_scope_with_access_grant(
                b"/other",
                "/other",
                "/other",
                ScopeAccessGrantWrite {
                    scope_platform: std::env::consts::OS,
                    grant_platform: std::env::consts::OS,
                    opaque_grant: b"other-folder-search-grant",
                    state: ScopeAccessGrantState::Active,
                },
            )
            .expect("other scope should persist");
        assert!(matches!(
            database.lexical_search_candidates(
                "needle",
                LexicalSearchFilters {
                    scope_id: Some(other.id),
                    folder_node_id: Some(selected_id),
                    ..lexical_filters(Some(other.id))
                },
                10,
            ),
            Err(DatabaseError::SearchFolderInvalid)
        ));

        database
            .connection
            .execute(
                "UPDATE locations SET present=0 WHERE scope_id=?1 AND node_id=?2",
                params![scope_id, selected_id],
            )
            .expect("folder should become absent");
        assert!(matches!(
            database.lexical_search_candidates(
                "needle",
                LexicalSearchFilters {
                    scope_id: Some(scope_id),
                    folder_node_id: Some(selected_id),
                    ..lexical_filters(Some(scope_id))
                },
                10,
            ),
            Err(DatabaseError::SearchFolderInvalid)
        ));
        database
            .connection
            .execute(
                "UPDATE locations SET present=1 WHERE scope_id=?1 AND node_id=?2",
                params![scope_id, selected_id],
            )
            .expect("folder should become present again");

        database
            .connection
            .execute(
                "UPDATE scope_access_grants SET state='needs_reauthorization' WHERE scope_id=?1",
                [scope_id],
            )
            .expect("grant should become inactive");
        assert!(matches!(
            database.list_search_folders(scope_id, None),
            Err(DatabaseError::ScopeAccessGrantNotActive)
        ));
        assert!(matches!(
            database.lexical_search_candidates(
                "needle",
                LexicalSearchFilters {
                    scope_id: Some(scope_id),
                    folder_node_id: Some(selected_id),
                    ..lexical_filters(Some(scope_id))
                },
                10,
            ),
            Err(DatabaseError::SearchFolderInvalid)
        ));
        database
            .upsert_scope_access_grant(scope_id, std::env::consts::OS, b"restored-search-grant")
            .expect("grant should reactivate");

        database
            .connection
            .execute(
                "UPDATE nodes SET identity_kind=?2,identity_key=?3 WHERE id=?1",
                params![
                    selected_id,
                    TEST_EXCLUDED_IDENTITY_KIND,
                    TEST_EXCLUDED_FOLDER_IDENTITY
                ],
            )
            .expect("selected folder should use a stable exclusion identity");
        let binding = database
            .bind_scope_policy_revision(scope_id)
            .expect("active search scope should bind");
        database
            .apply_scope_exclusion_batch(
                binding,
                &[ScopeExclusionWrite {
                    kind: ScopeExclusionKind::Folder,
                    path_raw: selected_path.as_bytes(),
                    path_key: selected_path,
                    display_path: selected_path,
                    identity_kind: TEST_EXCLUDED_IDENTITY_KIND,
                    identity_key: TEST_EXCLUDED_FOLDER_IDENTITY,
                }],
                2,
            )
            .expect("selected folder exclusion should purge atomically");
        assert!(matches!(
            database.lexical_search_candidates(
                "needle",
                LexicalSearchFilters {
                    scope_id: Some(scope_id),
                    folder_node_id: Some(selected_id),
                    ..lexical_filters(Some(scope_id))
                },
                10,
            ),
            Err(DatabaseError::SearchFolderInvalid)
        ));
        let after_exclusion = database
            .list_search_folders(scope_id, None)
            .expect("remaining folder list should load");
        assert!(
            after_exclusion
                .folders
                .iter()
                .all(|folder| folder.folder_node_id != selected_id)
        );
    }

    #[test]
    fn content_search_rejects_cross_scope_location_inconsistency() {
        let (mut database, granted_scope_id, node_id, _) = extraction_setup();
        let denied = database
            .add_scope(b"/denied", "/denied", "/denied", "test")
            .expect("denied scope should persist");
        database
            .connection
            .execute(
                "INSERT INTO scan_jobs(scope_id, status, started_at_unix_ms, finished_at_unix_ms) \
                 VALUES (?1, 'completed', 1, 1)",
                [denied.id],
            )
            .expect("denied scan should persist");
        let denied_scan_id = database.connection.last_insert_rowid();
        database
            .connection
            .execute(
                "INSERT INTO locations(scope_id, node_id, path_raw, path_key, display_path, present, last_seen_scan_id) \
                 VALUES (?1, ?2, X'2F64656E6965642F707269766174652E6D64', '/denied/private.md', '/denied/private.md', 1, ?3)",
                params![denied.id, node_id, denied_scan_id],
            )
            .expect("cross-scope identity location should persist");
        let denied_location_id = database.connection.last_insert_rowid();

        let text = "forged cross scope private marker";
        let job = database
            .create_extraction_job(granted_scope_id, node_id)
            .expect("granted job should create");
        database
            .claim_extraction_job(job.job_id, "scope-runner", 60_000)
            .expect("job should claim");
        database
            .complete_extraction_job(
                job.job_id,
                "scope-runner",
                "deskgraph.utf8-text",
                "1",
                4,
                Some(1),
                u64::try_from(text.len()).expect("fixture length should fit"),
                1,
                &[ContentChunkWrite {
                    ordinal: 0,
                    text: text.to_string(),
                    provenance: ContentChunkProvenanceWrite::ByteRange { start: 0, end: 4 },
                    trust_class: "untrusted_extracted_text",
                }],
            )
            .expect("content should publish");
        database
            .connection
            .execute(
                "UPDATE content_chunks SET location_id = ?1 WHERE extraction_job_id = ?2",
                params![denied_location_id, job.job_id],
            )
            .expect("fixture should forge an inconsistent cross-scope location");

        let candidates = database
            .lexical_search_candidates(
                "\"cross scope private\"",
                LexicalSearchFilters {
                    scope_id: Some(granted_scope_id),
                    source: LexicalSearchSource::ExtractedText,
                    ..lexical_filters(Some(granted_scope_id))
                },
                10,
            )
            .expect("inconsistent row should fail closed without failing the query");
        assert!(candidates.is_empty());
    }

    #[test]
    fn search_database_boundary_rejects_unbounded_requests() {
        let database = ManifestDatabase::open_in_memory().expect("database should initialize");
        assert!(matches!(
            database.lexical_search_candidates("", lexical_filters(None), 10),
            Err(DatabaseError::SearchInputInvalid)
        ));
        assert!(matches!(
            database.lexical_search_candidates("\"bounded\"", lexical_filters(None), 101),
            Err(DatabaseError::SearchInputInvalid)
        ));
        for filters in [
            LexicalSearchFilters {
                scope_id: Some(0),
                ..lexical_filters(None)
            },
            LexicalSearchFilters {
                extension: Some("m_d"),
                ..lexical_filters(None)
            },
            LexicalSearchFilters {
                modified_since_unix_ns: Some(2),
                modified_before_unix_ns: Some(2),
                ..lexical_filters(None)
            },
        ] {
            assert!(matches!(
                database.lexical_search_candidates("\"bounded\"", filters, 10),
                Err(DatabaseError::SearchInputInvalid)
            ));
        }
    }

    #[test]
    fn ready_job_can_pause_and_resume_without_processing() {
        let (mut database, scope_id, root) = resumable_setup();
        let job = database
            .create_resumable_scan_job(scope_id, &root)
            .expect("job should create");

        let paused = database
            .request_scan_pause(job.job_id)
            .expect("ready job should pause");
        assert_eq!(paused.status, ScanStatus::Paused);
        assert_eq!(paused.processed_entries, 0);

        let resumed = database
            .resume_scan_job(job.job_id)
            .expect("paused job should resume");
        assert_eq!(resumed.status, ScanStatus::Running);
        assert!(!resumed.pause_requested);
    }

    #[test]
    fn active_runner_acknowledges_a_durable_pause_request_on_release() {
        let (mut database, scope_id, root) = resumable_setup();
        let job = database
            .create_resumable_scan_job(scope_id, &root)
            .expect("job should create");
        database
            .claim_scan_job(job.job_id, "runner", 60_000)
            .expect("job should claim");

        let requested = database
            .request_scan_pause(job.job_id)
            .expect("pause should persist");
        assert_eq!(requested.status, ScanStatus::Running);
        assert!(requested.pause_requested);
        let timed = database
            .record_scan_runner_elapsed(job.job_id, "runner", 17)
            .expect("active time should persist");
        assert_eq!(timed.elapsed_ms, 17);

        let paused = database
            .release_scan_job(job.job_id, "runner")
            .expect("runner should acknowledge pause");
        assert_eq!(paused.status, ScanStatus::Paused);
        assert!(paused.pause_requested);
    }

    #[test]
    fn expired_runner_is_interrupted_and_queue_item_is_replayable() {
        let (mut database, scope_id, root) = resumable_setup();
        let job = database
            .create_resumable_scan_job(scope_id, &root)
            .expect("job should create");
        database
            .claim_scan_job(job.job_id, "runner-a", 60_000)
            .expect("job should claim");
        let first = database
            .next_scan_queue_entry(job.job_id, "runner-a", 60_000)
            .expect("queue should read")
            .expect("root should be pending");

        assert_eq!(
            database
                .recover_expired_scan_jobs_at(i64::MAX)
                .expect("expired job should recover"),
            1
        );
        assert_eq!(
            database
                .scan_job(job.job_id)
                .expect("job should load")
                .status,
            ScanStatus::Interrupted
        );
        database
            .resume_scan_job(job.job_id)
            .expect("interrupted job should resume");
        database
            .claim_scan_job(job.job_id, "runner-b", 60_000)
            .expect("resumed job should claim");
        let replay = database
            .next_scan_queue_entry(job.job_id, "runner-b", 60_000)
            .expect("queue should read")
            .expect("root should replay");
        assert_eq!(replay.id, first.id);
    }

    #[test]
    fn reopening_a_file_database_recovers_an_expired_processing_entry() {
        let directory = tempfile::tempdir().expect("tempdir should exist");
        let path = directory.path().join("manifest.sqlite3");
        let mut database = ManifestDatabase::open(&path).expect("database should initialize");
        let scope = database
            .add_scope(b"/scope", "/scope", "/scope", "test")
            .expect("scope should persist");
        let root = QueuedPath {
            path_raw: b"/scope".to_vec(),
            path_key: "/scope".to_string(),
            parent_identity_key: None,
            is_root: true,
        };
        let job = database
            .create_resumable_scan_job(scope.id, &root)
            .expect("job should create");
        database
            .claim_scan_job(job.job_id, "crashed-runner", 60_000)
            .expect("job should claim");
        let processing = database
            .next_scan_queue_entry(job.job_id, "crashed-runner", 60_000)
            .expect("queue should read")
            .expect("root should be processing");
        database
            .connection
            .execute(
                "UPDATE scan_jobs SET lease_expires_at_unix_ms = 0 WHERE id = ?1",
                [job.job_id],
            )
            .expect("fixture lease should expire");
        drop(database);

        let mut recovered = ManifestDatabase::open(&path).expect("database should recover on open");
        assert_eq!(
            recovered
                .scan_job(job.job_id)
                .expect("job should load")
                .status,
            ScanStatus::Interrupted
        );
        recovered
            .resume_scan_job(job.job_id)
            .expect("interrupted job should resume");
        recovered
            .claim_scan_job(job.job_id, "new-runner", 60_000)
            .expect("resumed job should claim");
        let replay = recovered
            .next_scan_queue_entry(job.job_id, "new-runner", 60_000)
            .expect("queue should read")
            .expect("processing entry should replay");
        assert_eq!(replay.id, processing.id);
    }

    #[test]
    fn staged_batches_are_invisible_until_atomic_publish() {
        let (mut database, scope_id, root) = resumable_setup();
        let job = database
            .create_resumable_scan_job(scope_id, &root)
            .expect("job should create");
        database
            .claim_scan_job(job.job_id, "runner", 60_000)
            .expect("job should claim");
        let root_entry = database
            .next_scan_queue_entry(job.job_id, "runner", 60_000)
            .expect("queue should read")
            .expect("root should exist");
        let root_observation = observation("/scope", NodeKind::Folder, None);
        let child = QueuedPath {
            path_raw: b"/scope/file.txt".to_vec(),
            path_key: "/scope/file.txt".to_string(),
            parent_identity_key: Some(root_observation.identity_key.clone()),
            is_root: false,
        };
        database
            .stage_scan_queue_entry(
                job.job_id,
                "runner",
                root_entry.id,
                Some(&root_observation),
                std::slice::from_ref(&child),
                &[],
                0,
                1,
                60_000,
            )
            .expect("root should stage");
        let child_entry = database
            .next_scan_queue_entry(job.job_id, "runner", 60_000)
            .expect("queue should read")
            .expect("child should exist");
        let child_observation = observation(
            "/scope/file.txt",
            NodeKind::File,
            Some(root_observation.identity_key.clone()),
        );
        let staged = database
            .stage_scan_queue_entry(
                job.job_id,
                "runner",
                child_entry.id,
                Some(&child_observation),
                &[],
                &[],
                0,
                1,
                60_000,
            )
            .expect("child should stage");

        assert_eq!(staged.processed_entries, 2);
        assert_eq!(database.stats().expect("stats should load").node_count, 0);
        let completed = database
            .finalize_resumable_scan_job(job.job_id, "runner")
            .expect("job should publish");
        assert_eq!(completed.status, ScanStatus::Completed);
        assert_eq!(database.stats().expect("stats should load").node_count, 2);
    }

    #[test]
    fn queued_and_running_extractions_can_be_cancelled_durably() {
        let (mut database, scope_id, node_id, _) = extraction_setup();
        let queued = database
            .create_extraction_job(scope_id, node_id)
            .expect("job should create");

        let cancelled = database
            .request_extraction_cancel(queued.job_id)
            .expect("queued job should cancel");
        assert_eq!(cancelled.status, ExtractionStatus::Cancelled);

        let running = database
            .create_extraction_job(scope_id, node_id)
            .expect("replacement job should create");
        database
            .claim_extraction_job(running.job_id, "extract-runner", 60_000)
            .expect("job should claim");
        let requested = database
            .request_extraction_cancel(running.job_id)
            .expect("running cancellation should persist");
        assert_eq!(requested.status, ExtractionStatus::Running);
        assert!(requested.cancel_requested);
        let acknowledged = database
            .cancel_extraction_job_from_runner(
                running.job_id,
                "extract-runner",
                "deskgraph.utf8-text",
                "1",
                2,
            )
            .expect("runner should acknowledge cancellation");
        assert_eq!(acknowledged.status, ExtractionStatus::Cancelled);
        assert_eq!(
            database
                .extraction_stats()
                .expect("stats should load")
                .cancelled_job_count,
            2
        );
    }

    #[test]
    fn durable_cancel_wins_when_provider_failure_is_recorded_afterward() {
        let (mut database, scope_id, node_id, _) = extraction_setup();
        let job = database
            .create_extraction_job(scope_id, node_id)
            .expect("job should create");
        database
            .claim_extraction_job(job.job_id, "failure-race-runner", 60_000)
            .expect("job should claim");
        let requested = database
            .request_extraction_cancel(job.job_id)
            .expect("running cancellation should persist");
        assert_eq!(requested.status, ExtractionStatus::Running);
        assert!(requested.cancel_requested);

        let terminal = database
            .fail_extraction_job(
                job.job_id,
                "failure-race-runner",
                "deskgraph.racing-provider",
                "1",
                "extraction_ocr_provider_failed",
                7,
            )
            .expect("durable cancellation must win the terminal-state race");
        assert_eq!(terminal.status, ExtractionStatus::Cancelled);
        assert!(terminal.cancel_requested);
        assert_eq!(terminal.error_code, None);
        assert_eq!(
            terminal.provider_id.as_deref(),
            Some("deskgraph.racing-provider")
        );
    }

    #[test]
    fn durable_cancel_prevents_atomic_publication_before_runner_acknowledgement() {
        let (mut database, scope_id, node_id, _) = extraction_setup();
        let job = database
            .create_extraction_job(scope_id, node_id)
            .expect("job should create");
        database
            .claim_extraction_job(job.job_id, "completion-race-runner", 60_000)
            .expect("job should claim");
        database
            .request_extraction_cancel(job.job_id)
            .expect("running cancellation should persist");

        let error = database
            .complete_extraction_job(
                job.job_id,
                "completion-race-runner",
                "deskgraph.utf8-text",
                "1",
                4,
                Some(1),
                4,
                1,
                &[ContentChunkWrite {
                    ordinal: 0,
                    text: "text".to_string(),
                    provenance: ContentChunkProvenanceWrite::ByteRange { start: 0, end: 4 },
                    trust_class: "untrusted_extracted_text",
                }],
            )
            .expect_err("durable cancellation must reject publication");
        assert!(matches!(error, DatabaseError::ExtractionOutputInvalid));
        assert_eq!(
            database
                .connection
                .query_row(
                    "SELECT COUNT(*) FROM content_chunks WHERE extraction_job_id = ?1",
                    [job.job_id],
                    |row| row.get::<_, i64>(0),
                )
                .expect("published chunks should count"),
            0
        );
        let terminal = database
            .cancel_extraction_job_from_runner(
                job.job_id,
                "completion-race-runner",
                "deskgraph.utf8-text",
                "1",
                1,
            )
            .expect("runner should acknowledge cancellation");
        assert_eq!(terminal.status, ExtractionStatus::Cancelled);
    }

    #[test]
    fn validated_ocr_source_must_still_match_current_manifest_at_insert() {
        let (mut database, scope_id, node_id, _) = extraction_setup();
        let validated = database
            .extractable_file(scope_id, node_id)
            .expect("validated source should load");
        database
            .connection
            .execute(
                "UPDATE files SET size_bytes = size_bytes + 1 WHERE node_id = ?1",
                [node_id],
            )
            .expect("fixture manifest should change");

        assert!(matches!(
            database.low_level_insert_screenshot_ocr_job_after_core_validation(&validated),
            Err(DatabaseError::ExtractableFileNotFound)
        ));
        assert!(
            database
                .recent_extraction_jobs()
                .expect("jobs should remain queryable")
                .is_empty()
        );
        let content = database
            .create_extraction_job(scope_id, node_id)
            .expect("stale OCR validation must not block a current content job");
        assert_eq!(content.operation, ExtractionOperation::Content);
        assert_eq!(content.status, ExtractionStatus::Queued);
    }

    #[test]
    fn low_level_ocr_insert_has_one_production_workspace_callsite() {
        fn collect_calls(
            root: &Path,
            workspace: &Path,
            calls: &mut Vec<String>,
        ) -> std::io::Result<()> {
            for entry in fs::read_dir(root)? {
                let entry = entry?;
                let file_type = entry.file_type()?;
                if file_type.is_symlink() {
                    continue;
                }
                let path = entry.path();
                if file_type.is_dir() {
                    collect_calls(&path, workspace, calls)?;
                } else if path.extension().and_then(|value| value.to_str()) == Some("rs")
                    && !path.ends_with("crates/database/src/lib.rs")
                {
                    let source = fs::read_to_string(&path)?;
                    let matches = source
                        .matches(
                            "low_level_insert_screenshot_ocr_job_with_policy_after_core_validation",
                        )
                        .count();
                    for _ in 0..matches {
                        calls.push(
                            path.strip_prefix(workspace)
                                .unwrap_or(&path)
                                .to_string_lossy()
                                .into_owned(),
                        );
                    }
                }
            }
            Ok(())
        }

        let workspace = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../..")
            .canonicalize()
            .expect("workspace should canonicalize");
        let mut calls = Vec::new();
        for root in ["apps", "crates", "tools"] {
            collect_calls(&workspace.join(root), &workspace, &mut calls)
                .expect("workspace Rust sources should remain readable");
        }
        assert_eq!(calls, vec!["crates/extractors/src/service.rs"]);
    }

    #[test]
    fn screenshot_ocr_lookup_prefers_interrupted_work_over_newer_terminal_history() {
        let (mut database, scope_id, node_id, _) = extraction_setup();
        let interrupted = database
            .create_screenshot_ocr_job(scope_id, node_id)
            .expect("OCR job should create");
        database
            .claim_extraction_job(interrupted.job_id, "expired-ocr-runner", 60_000)
            .expect("OCR job should claim");
        assert_eq!(
            database
                .recover_expired_extraction_jobs_at(i64::MAX)
                .expect("expired OCR lease should recover"),
            1
        );

        let location_id: i64 = database
            .connection
            .query_row(
                "SELECT location_id FROM extraction_jobs WHERE id = ?1",
                [interrupted.job_id],
                |row| row.get(0),
            )
            .expect("OCR location should load");
        for ordinal in 0..21_i64 {
            database
                .connection
                .execute(
                    "INSERT INTO extraction_jobs( \
                        scope_id, node_id, location_id, status, source_size_bytes, \
                        created_at_unix_ms, updated_at_unix_ms, operation \
                     ) VALUES (?1, ?2, ?3, 'completed', 4, ?4, ?4, 'content')",
                    params![scope_id, node_id, location_id, ordinal],
                )
                .expect("newer generic terminal history should insert");
        }
        database
            .connection
            .execute(
                "INSERT INTO extraction_jobs( \
                    scope_id, node_id, location_id, status, source_size_bytes, \
                    created_at_unix_ms, updated_at_unix_ms, operation \
                 ) VALUES (?1, ?2, ?3, 'completed', 4, 99, 99, 'screenshot_ocr')",
                params![scope_id, node_id, location_id],
            )
            .expect("newer terminal OCR history should insert");

        let found = database
            .screenshot_ocr_job_for_node(scope_id, node_id)
            .expect("OCR lookup should query")
            .expect("interrupted OCR should remain discoverable");
        assert_eq!(found.job_id, interrupted.job_id);
        assert_eq!(found.operation, ExtractionOperation::ScreenshotOcr);
        assert_eq!(found.status, ExtractionStatus::Interrupted);

        let (mut generic_database, generic_scope_id, generic_node_id, _) = extraction_setup();
        generic_database
            .create_extraction_job(generic_scope_id, generic_node_id)
            .expect("generic job should create");
        assert_eq!(
            generic_database
                .screenshot_ocr_job_for_node(generic_scope_id, generic_node_id)
                .expect("generic lookup should query"),
            None
        );
    }

    #[test]
    fn complete_extraction_atomically_replaces_only_valid_chunks() {
        let (mut database, scope_id, node_id, _) = extraction_setup();
        let first = database
            .create_extraction_job(scope_id, node_id)
            .expect("job should create");
        database
            .claim_extraction_job(first.job_id, "runner-1", 60_000)
            .expect("job should claim");
        let first_chunks = vec![
            ContentChunkWrite {
                ordinal: 0,
                text: "ab".to_string(),
                provenance: ContentChunkProvenanceWrite::ByteRange { start: 0, end: 2 },
                trust_class: "untrusted_extracted_text",
            },
            ContentChunkWrite {
                ordinal: 1,
                text: "cd".to_string(),
                provenance: ContentChunkProvenanceWrite::ByteRange { start: 2, end: 4 },
                trust_class: "untrusted_extracted_text",
            },
        ];
        let completed = database
            .complete_extraction_job(
                first.job_id,
                "runner-1",
                "deskgraph.utf8-text",
                "1",
                4,
                Some(1),
                4,
                3,
                &first_chunks,
            )
            .expect("valid chunks should publish");
        assert_eq!(completed.status, ExtractionStatus::Completed);
        assert_eq!(completed.chunk_count, 2);
        assert_eq!(
            database
                .extraction_stats()
                .expect("stats should load")
                .active_chunk_count,
            2
        );

        let invalid = database
            .create_extraction_job(scope_id, node_id)
            .expect("retry should create");
        database
            .claim_extraction_job(invalid.job_id, "runner-2", 60_000)
            .expect("retry should claim");
        let bounded_chunk = ContentChunkWrite {
            ordinal: 0,
            text: "abcd".to_string(),
            provenance: ContentChunkProvenanceWrite::ByteRange { start: 0, end: 4 },
            trust_class: "untrusted_extracted_text",
        };
        let error = database
            .complete_extraction_job(
                invalid.job_id,
                "runner-2",
                "deskgraph.utf8-text",
                "1",
                4,
                Some(1),
                MAX_EXTRACTION_OUTPUT_BYTES + 1,
                1,
                std::slice::from_ref(&bounded_chunk),
            )
            .expect_err("database must enforce the absolute output cap");
        assert!(matches!(error, DatabaseError::ExtractionOutputInvalid));
        let error = database
            .complete_extraction_job(
                invalid.job_id,
                "runner-2",
                "deskgraph.utf8-text",
                "1",
                4,
                Some(1),
                1,
                1,
                &[bounded_chunk],
            )
            .expect_err("declared output bytes must match staged chunk bytes");
        assert!(matches!(error, DatabaseError::ExtractionOutputInvalid));
        assert_eq!(
            database
                .extraction_stats()
                .expect("stats should load")
                .active_chunk_count,
            2
        );
        let invalid_chunk = ContentChunkWrite {
            ordinal: 0,
            text: "bad".to_string(),
            provenance: ContentChunkProvenanceWrite::ByteRange { start: 0, end: 3 },
            trust_class: "trusted",
        };
        let error = database
            .complete_extraction_job(
                invalid.job_id,
                "runner-2",
                "deskgraph.utf8-text",
                "1",
                4,
                Some(1),
                3,
                1,
                &[invalid_chunk],
            )
            .expect_err("invalid trust class must not publish");
        assert!(matches!(error, DatabaseError::ExtractionOutputInvalid));
        assert_eq!(
            database
                .extraction_stats()
                .expect("stats should load")
                .active_chunk_count,
            2
        );
        database
            .fail_extraction_job(
                invalid.job_id,
                "runner-2",
                "deskgraph.utf8-text",
                "1",
                "extraction_output_invalid",
                1,
            )
            .expect("invalid job should fail");

        let replacement = database
            .create_extraction_job(scope_id, node_id)
            .expect("replacement should create");
        database
            .claim_extraction_job(replacement.job_id, "runner-3", 60_000)
            .expect("replacement should claim");
        let replacement_chunk = ContentChunkWrite {
            ordinal: 0,
            text: "abcd".to_string(),
            provenance: ContentChunkProvenanceWrite::ByteRange { start: 0, end: 4 },
            trust_class: "untrusted_extracted_text",
        };
        database
            .complete_extraction_job(
                replacement.job_id,
                "runner-3",
                "deskgraph.utf8-text",
                "1",
                4,
                Some(1),
                4,
                1,
                &[replacement_chunk],
            )
            .expect("replacement should publish");
        let stats = database.extraction_stats().expect("stats should load");
        assert_eq!(stats.active_chunk_count, 1);
        assert_eq!(stats.extracted_file_count, 1);
        let inactive: i64 = database
            .connection
            .query_row(
                "SELECT COUNT(*) FROM content_chunks WHERE active = 0",
                [],
                |row| row.get(0),
            )
            .expect("inactive chunks should count");
        assert_eq!(inactive, 2);
    }

    #[test]
    fn image_metadata_is_structured_validated_and_atomically_replaced() {
        let (mut database, scope_id, node_id, _) = extraction_setup();
        let image_job = database
            .create_extraction_job(scope_id, node_id)
            .expect("image job should create");
        database
            .claim_extraction_job(image_job.job_id, "image-runner", 60_000)
            .expect("image job should claim");
        database
            .complete_extraction_job_with_image_metadata(
                image_job.job_id,
                "image-runner",
                "deskgraph.image-metadata",
                "1",
                4,
                Some(1),
                0,
                1,
                &[],
                Some(&ImageMetadataWrite {
                    format: ImageFormat::Png,
                    pixel_width: 2,
                    pixel_height: 2,
                }),
            )
            .expect("valid image metadata should publish");
        let stored = database
            .image_metadata_for_job(image_job.job_id)
            .expect("image metadata should load");
        assert_eq!(stored.format, ImageFormat::Png);
        assert_eq!((stored.pixel_width, stored.pixel_height), (2, 2));
        let stats = database.extraction_stats().expect("stats should load");
        assert_eq!(stats.active_chunk_count, 0);
        assert_eq!(stats.extracted_file_count, 1);

        let invalid_job = database
            .create_extraction_job(scope_id, node_id)
            .expect("invalid retry should create");
        database
            .claim_extraction_job(invalid_job.job_id, "invalid-image", 60_000)
            .expect("invalid retry should claim");
        let error = database
            .complete_extraction_job_with_image_metadata(
                invalid_job.job_id,
                "invalid-image",
                "deskgraph.image-metadata",
                "1",
                4,
                Some(1),
                0,
                1,
                &[],
                Some(&ImageMetadataWrite {
                    format: ImageFormat::Png,
                    pixel_width: 25_000,
                    pixel_height: 25_000,
                }),
            )
            .expect_err("dimension bomb must not publish");
        assert!(matches!(error, DatabaseError::ExtractionOutputInvalid));
        assert_eq!(
            database
                .connection
                .query_row(
                    "SELECT COUNT(*) FROM image_metadata WHERE active = 1",
                    [],
                    |row| row.get::<_, i64>(0),
                )
                .expect("active metadata should count"),
            1
        );
        database
            .fail_extraction_job(
                invalid_job.job_id,
                "invalid-image",
                "deskgraph.image-metadata",
                "1",
                "extraction_output_invalid",
                1,
            )
            .expect("invalid retry should fail");

        let text_job = database
            .create_extraction_job(scope_id, node_id)
            .expect("replacement should create");
        database
            .claim_extraction_job(text_job.job_id, "text-runner", 60_000)
            .expect("replacement should claim");
        database
            .complete_extraction_job(
                text_job.job_id,
                "text-runner",
                "deskgraph.utf8-text",
                "1",
                4,
                Some(1),
                4,
                1,
                &[ContentChunkWrite {
                    ordinal: 0,
                    text: "text".to_string(),
                    provenance: ContentChunkProvenanceWrite::ByteRange { start: 0, end: 4 },
                    trust_class: "untrusted_extracted_text",
                }],
            )
            .expect("text replacement should publish atomically");
        assert_eq!(
            database
                .connection
                .query_row(
                    "SELECT COUNT(*) FROM image_metadata WHERE active = 1",
                    [],
                    |row| row.get::<_, i64>(0),
                )
                .expect("active metadata should count"),
            0
        );
        assert_eq!(
            database
                .image_metadata_for_job(image_job.job_id)
                .expect("historical metadata should remain queryable")
                .pixel_width,
            2
        );
    }

    #[test]
    fn ocr_jobs_preserve_image_metadata_and_replace_only_ocr_chunks() {
        let (mut database, scope_id, node_id, _) = extraction_setup();
        let image_job = database
            .create_extraction_job(scope_id, node_id)
            .expect("image job should create");
        assert_eq!(image_job.operation, ExtractionOperation::Content);
        database
            .claim_extraction_job(image_job.job_id, "image-runner", 60_000)
            .expect("image job should claim");
        database
            .complete_extraction_job_with_image_metadata(
                image_job.job_id,
                "image-runner",
                "deskgraph.image-metadata",
                "1",
                4,
                Some(1),
                0,
                1,
                &[],
                Some(&ImageMetadataWrite {
                    format: ImageFormat::Png,
                    pixel_width: 2,
                    pixel_height: 2,
                }),
            )
            .expect("metadata should publish");

        let first_ocr = database
            .create_screenshot_ocr_job(scope_id, node_id)
            .expect("OCR job should create");
        assert_eq!(first_ocr.operation, ExtractionOperation::ScreenshotOcr);
        database
            .claim_extraction_job(first_ocr.job_id, "ocr-runner-1", 60_000)
            .expect("OCR job should claim");
        let first_text = "DeskGraph 桌面圖譜";
        database
            .complete_extraction_job(
                first_ocr.job_id,
                "ocr-runner-1",
                "deskgraph.macos-vision-ocr",
                "1",
                4,
                Some(1),
                u64::try_from(first_text.len()).expect("text length should fit"),
                2,
                &[ContentChunkWrite {
                    ordinal: 0,
                    text: first_text.to_string(),
                    provenance: ContentChunkProvenanceWrite::OcrObservation {
                        observation_number: 1,
                        fragment_index: 0,
                        bbox_x_ppm: 50_000,
                        bbox_y_ppm: 100_000,
                        bbox_width_ppm: 400_000,
                        bbox_height_ppm: 200_000,
                        confidence_basis_points: Some(9_876),
                    },
                    trust_class: "untrusted_extracted_text",
                }],
            )
            .expect("OCR chunks should publish");
        assert_eq!(
            database
                .connection
                .query_row(
                    "SELECT COUNT(*) FROM image_metadata WHERE active = 1",
                    [],
                    |row| { row.get::<_, i64>(0) }
                )
                .expect("metadata should count"),
            1
        );
        let stored: (String, i64, i64, i64, i64, i64, i64) = database
            .connection
            .query_row(
                "SELECT provenance_kind, source_unit_number, source_bbox_x_ppm, \
                    source_bbox_y_ppm, source_bbox_width_ppm, source_bbox_height_ppm, \
                    source_confidence_basis_points \
                 FROM content_chunks WHERE active = 1 AND provenance_kind = 'ocr_observation'",
                [],
                |row| {
                    Ok((
                        row.get(0)?,
                        row.get(1)?,
                        row.get(2)?,
                        row.get(3)?,
                        row.get(4)?,
                        row.get(5)?,
                        row.get(6)?,
                    ))
                },
            )
            .expect("OCR provenance should load");
        assert_eq!(
            stored,
            (
                "ocr_observation".to_string(),
                1,
                50_000,
                100_000,
                400_000,
                200_000,
                9_876,
            )
        );

        let metadata_refresh = database
            .create_extraction_job(scope_id, node_id)
            .expect("metadata refresh should create");
        database
            .claim_extraction_job(metadata_refresh.job_id, "image-runner-2", 60_000)
            .expect("metadata refresh should claim");
        database
            .complete_extraction_job_with_image_metadata(
                metadata_refresh.job_id,
                "image-runner-2",
                "deskgraph.image-metadata",
                "1",
                4,
                Some(1),
                0,
                1,
                &[],
                Some(&ImageMetadataWrite {
                    format: ImageFormat::Png,
                    pixel_width: 2,
                    pixel_height: 2,
                }),
            )
            .expect("metadata refresh should preserve OCR");
        assert_eq!(
            database
                .connection
                .query_row(
                    "SELECT COUNT(*) FROM content_chunks \
                     WHERE active = 1 AND provenance_kind = 'ocr_observation'",
                    [],
                    |row| row.get::<_, i64>(0),
                )
                .expect("active OCR should count"),
            1
        );

        let second_ocr = database
            .create_screenshot_ocr_job(scope_id, node_id)
            .expect("replacement OCR should create");
        database
            .claim_extraction_job(second_ocr.job_id, "ocr-runner-2", 60_000)
            .expect("replacement OCR should claim");
        database
            .complete_extraction_job(
                second_ocr.job_id,
                "ocr-runner-2",
                "deskgraph.macos-vision-ocr",
                "1",
                4,
                Some(1),
                0,
                1,
                &[],
            )
            .expect("no-text OCR should atomically replace prior OCR");
        assert_eq!(
            database
                .connection
                .query_row(
                    "SELECT COUNT(*) FROM content_chunks \
                     WHERE active = 1 AND provenance_kind = 'ocr_observation'",
                    [],
                    |row| row.get::<_, i64>(0),
                )
                .expect("active OCR should count"),
            0
        );
        assert_eq!(
            database
                .connection
                .query_row(
                    "SELECT COUNT(*) FROM image_metadata WHERE active = 1",
                    [],
                    |row| { row.get::<_, i64>(0) }
                )
                .expect("metadata should count"),
            1
        );

        let invalid = database
            .create_screenshot_ocr_job(scope_id, node_id)
            .expect("invalid OCR job should create");
        database
            .claim_extraction_job(invalid.job_id, "ocr-invalid", 60_000)
            .expect("invalid OCR job should claim");
        let error = database
            .complete_extraction_job(
                invalid.job_id,
                "ocr-invalid",
                "deskgraph.macos-vision-ocr",
                "1",
                4,
                Some(1),
                4,
                1,
                &[ContentChunkWrite {
                    ordinal: 0,
                    text: "text".to_string(),
                    provenance: ContentChunkProvenanceWrite::ByteRange { start: 0, end: 4 },
                    trust_class: "untrusted_extracted_text",
                }],
            )
            .expect_err("OCR jobs must reject non-OCR provenance");
        assert!(matches!(error, DatabaseError::ExtractionOutputInvalid));
        let error = database
            .complete_extraction_job(
                invalid.job_id,
                "ocr-invalid",
                "deskgraph.macos-vision-ocr",
                "1",
                4,
                Some(1),
                4,
                1,
                &[ContentChunkWrite {
                    ordinal: 0,
                    text: "text".to_string(),
                    provenance: ContentChunkProvenanceWrite::OcrObservation {
                        observation_number: 1,
                        fragment_index: 0,
                        bbox_x_ppm: 900_000,
                        bbox_y_ppm: 0,
                        bbox_width_ppm: 200_000,
                        bbox_height_ppm: 1,
                        confidence_basis_points: Some(10_000),
                    },
                    trust_class: "untrusted_extracted_text",
                }],
            )
            .expect_err("out-of-bounds OCR provenance must not publish");
        assert!(matches!(error, DatabaseError::ExtractionOutputInvalid));
        database
            .fail_extraction_job(
                invalid.job_id,
                "ocr-invalid",
                "deskgraph.macos-vision-ocr",
                "1",
                "extraction_output_invalid",
                1,
            )
            .expect("invalid OCR job should close");

        let invalid_content = database
            .create_extraction_job(scope_id, node_id)
            .expect("content job should create");
        database
            .claim_extraction_job(invalid_content.job_id, "content-invalid", 60_000)
            .expect("content job should claim");
        let error = database
            .complete_extraction_job(
                invalid_content.job_id,
                "content-invalid",
                "deskgraph.macos-vision-ocr",
                "1",
                4,
                Some(1),
                4,
                1,
                &[ContentChunkWrite {
                    ordinal: 0,
                    text: "text".to_string(),
                    provenance: ContentChunkProvenanceWrite::OcrObservation {
                        observation_number: 1,
                        fragment_index: 0,
                        bbox_x_ppm: 0,
                        bbox_y_ppm: 0,
                        bbox_width_ppm: 1,
                        bbox_height_ppm: 1,
                        confidence_basis_points: Some(10_000),
                    },
                    trust_class: "untrusted_extracted_text",
                }],
            )
            .expect_err("content jobs must reject OCR provenance");
        assert!(matches!(error, DatabaseError::ExtractionOutputInvalid));
    }

    #[test]
    fn ocr_confidence_absence_is_preserved_and_bbox_remains_required() {
        let (mut database, scope_id, node_id, _) = extraction_setup();
        let job = database
            .create_screenshot_ocr_job(scope_id, node_id)
            .expect("OCR job should create");
        database
            .claim_extraction_job(job.job_id, "windows-ocr-runner", 60_000)
            .expect("OCR job should claim");
        database
            .complete_extraction_job(
                job.job_id,
                "windows-ocr-runner",
                "deskgraph.windows-media-ocr",
                "1",
                4,
                Some(1),
                4,
                1,
                &[ContentChunkWrite {
                    ordinal: 0,
                    text: "text".to_string(),
                    provenance: ContentChunkProvenanceWrite::OcrObservation {
                        observation_number: 1,
                        fragment_index: 0,
                        bbox_x_ppm: 10_000,
                        bbox_y_ppm: 20_000,
                        bbox_width_ppm: 300_000,
                        bbox_height_ppm: 100_000,
                        confidence_basis_points: None,
                    },
                    trust_class: "untrusted_extracted_text",
                }],
            )
            .expect("provider without confidence should publish honest provenance");

        let stored: (Option<i64>, i64) = database
            .connection
            .query_row(
                "SELECT source_confidence_basis_points, source_bbox_x_ppm \
                 FROM content_chunks WHERE extraction_job_id = ?1",
                [job.job_id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .expect("OCR provenance should load");
        assert_eq!(stored, (None, 10_000));

        database
            .connection
            .execute(
                "UPDATE content_chunks SET source_bbox_x_ppm = NULL \
                 WHERE extraction_job_id = ?1",
                [job.job_id],
            )
            .expect_err("database guard must reject missing OCR bounds");
        assert_eq!(
            database
                .connection
                .query_row(
                    "SELECT source_bbox_x_ppm FROM content_chunks \
                     WHERE extraction_job_id = ?1",
                    [job.job_id],
                    |row| row.get::<_, i64>(0),
                )
                .expect("valid OCR bounds should remain"),
            10_000
        );
    }

    #[test]
    fn complete_extraction_stores_page_provenance_without_fake_byte_offsets() {
        let (mut database, scope_id, node_id, _) = extraction_setup();
        let job = database
            .create_extraction_job(scope_id, node_id)
            .expect("job should create");
        database
            .claim_extraction_job(job.job_id, "pdf-runner", 60_000)
            .expect("job should claim");
        database
            .complete_extraction_job(
                job.job_id,
                "pdf-runner",
                "deskgraph.pdf-text",
                "1",
                4,
                Some(1),
                4,
                1,
                &[ContentChunkWrite {
                    ordinal: 0,
                    text: "page".to_string(),
                    provenance: ContentChunkProvenanceWrite::PdfPage {
                        page_number: 2,
                        fragment_index: 0,
                    },
                    trust_class: "untrusted_extracted_text",
                }],
            )
            .expect("page provenance should publish");
        let stored: (String, Option<i64>, Option<i64>, Option<i64>, Option<i64>) = database
            .connection
            .query_row(
                "SELECT provenance_kind, source_byte_start, source_byte_end, source_page_number, source_fragment_index FROM content_chunks WHERE active = 1",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?)),
            )
            .expect("page provenance should load");

        assert_eq!(
            stored,
            ("pdf_page".to_string(), None, None, Some(2), Some(0))
        );
    }

    #[test]
    fn complete_extraction_stores_and_validates_office_provenance() {
        let (mut database, scope_id, node_id, _) = extraction_setup();
        let job = database
            .create_extraction_job(scope_id, node_id)
            .expect("job should create");
        database
            .claim_extraction_job(job.job_id, "office-runner", 60_000)
            .expect("job should claim");
        let chunks = [
            ContentChunkWrite {
                ordinal: 0,
                text: "docx".to_string(),
                provenance: ContentChunkProvenanceWrite::DocxParagraph {
                    paragraph_number: 2,
                    fragment_index: 0,
                },
                trust_class: "untrusted_extracted_text",
            },
            ContentChunkWrite {
                ordinal: 1,
                text: "pptx".to_string(),
                provenance: ContentChunkProvenanceWrite::PptxSlide {
                    slide_number: 3,
                    fragment_index: 1,
                },
                trust_class: "untrusted_extracted_text",
            },
            ContentChunkWrite {
                ordinal: 2,
                text: "cell".to_string(),
                provenance: ContentChunkProvenanceWrite::XlsxCell {
                    sheet_number: 4,
                    cell_reference: "XFD1048576".to_string(),
                    fragment_index: 2,
                },
                trust_class: "untrusted_extracted_text",
            },
        ];
        database
            .complete_extraction_job(
                job.job_id,
                "office-runner",
                "deskgraph.ooxml-text",
                "1",
                4,
                Some(1),
                12,
                1,
                &chunks,
            )
            .expect("Office provenance should publish");
        let mut statement = database
            .connection
            .prepare(
                "SELECT provenance_kind, source_unit_number, source_cell_reference, source_fragment_index \
                 FROM content_chunks WHERE active = 1 ORDER BY ordinal",
            )
            .expect("provenance query should prepare");
        let stored = statement
            .query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, Option<i64>>(1)?,
                    row.get::<_, Option<String>>(2)?,
                    row.get::<_, Option<i64>>(3)?,
                ))
            })
            .expect("provenance should query")
            .collect::<Result<Vec<_>, _>>()
            .expect("provenance rows should load");
        assert_eq!(
            stored,
            vec![
                ("docx_paragraph".to_string(), Some(2), None, Some(0)),
                ("pptx_slide".to_string(), Some(3), None, Some(1)),
                (
                    "xlsx_cell".to_string(),
                    Some(4),
                    Some("XFD1048576".to_string()),
                    Some(2),
                ),
            ]
        );
        drop(statement);

        let invalid = database
            .create_extraction_job(scope_id, node_id)
            .expect("invalid job should create");
        database
            .claim_extraction_job(invalid.job_id, "invalid-office-runner", 60_000)
            .expect("invalid job should claim");
        let error = database
            .complete_extraction_job(
                invalid.job_id,
                "invalid-office-runner",
                "deskgraph.ooxml-text",
                "1",
                4,
                Some(1),
                1,
                1,
                &[ContentChunkWrite {
                    ordinal: 0,
                    text: "x".to_string(),
                    provenance: ContentChunkProvenanceWrite::XlsxCell {
                        sheet_number: 1,
                        cell_reference: "XFE1".to_string(),
                        fragment_index: 0,
                    },
                    trust_class: "untrusted_extracted_text",
                }],
            )
            .expect_err("out-of-range cell reference must not publish");
        assert!(matches!(error, DatabaseError::ExtractionOutputInvalid));
        assert_eq!(
            database
                .extraction_stats()
                .expect("stats should remain available")
                .active_chunk_count,
            3
        );
    }

    #[test]
    fn manifest_change_invalidates_prior_content_chunks() {
        let (mut database, scope_id, node_id, root) = extraction_setup();
        let job = database
            .create_extraction_job(scope_id, node_id)
            .expect("job should create");
        database
            .claim_extraction_job(job.job_id, "runner", 60_000)
            .expect("job should claim");
        database
            .complete_extraction_job(
                job.job_id,
                "runner",
                "deskgraph.utf8-text",
                "1",
                4,
                Some(1),
                4,
                1,
                &[ContentChunkWrite {
                    ordinal: 0,
                    text: "abcd".to_string(),
                    provenance: ContentChunkProvenanceWrite::ByteRange { start: 0, end: 4 },
                    trust_class: "untrusted_extracted_text",
                }],
            )
            .expect("chunks should publish");

        let rescanned_node = publish_manifest_file(&mut database, scope_id, &root, 5);

        assert_eq!(rescanned_node, node_id);
        assert_eq!(
            database
                .extraction_stats()
                .expect("stats should load")
                .active_chunk_count,
            0
        );
    }

    #[test]
    fn expired_extraction_runner_requires_explicit_resume() {
        let (mut database, scope_id, node_id, _) = extraction_setup();
        let job = database
            .create_extraction_job(scope_id, node_id)
            .expect("job should create");
        database
            .claim_extraction_job(job.job_id, "old-runner", 60_000)
            .expect("job should claim");

        assert_eq!(
            database
                .recover_expired_extraction_jobs_at(i64::MAX)
                .expect("job should recover"),
            1
        );
        assert_eq!(
            database
                .extraction_job(job.job_id)
                .expect("job should load")
                .status,
            ExtractionStatus::Interrupted
        );
        let resumed = database
            .resume_extraction_job(job.job_id)
            .expect("job should resume");
        assert_eq!(resumed.status, ExtractionStatus::Queued);
        database
            .claim_extraction_job(job.job_id, "new-runner", 60_000)
            .expect("resumed job should claim");
    }

    #[test]
    fn exclusion_preview_and_apply_require_active_grant_but_core_watch_remains_revision_only() {
        let mut database = ManifestDatabase::open_in_memory().expect("database should initialize");
        let scope = database
            .add_scope(b"/scope", "/scope", "/scope", std::env::consts::OS)
            .expect("scope should persist without a platform grant");
        let core = database
            .bind_core_scope_policy_revision(scope.id)
            .expect("core revision should bind without a grant");
        let scan_id = database
            .create_scan_job_with_policy(core)
            .expect("core scan should start without a grant");
        database
            .complete_scan(scan_id, scope.id, &[], &[], 0, 0)
            .expect("core scan should complete without a grant");
        let snapshot = WatchSnapshot {
            kind: WatchSnapshotKind::File,
            size_bytes: Some(1),
            modified_unix_ns: Some(1),
            identity_key: Some(b"watch-no-grant".to_vec()),
        };
        database
            .record_watch_observation_with_policy_at(
                core,
                WatchObservationWrite {
                    scope_id: scope.id,
                    path_raw: b"/scope/watch.txt",
                    path_key: "/scope/watch.txt",
                    snapshot: &snapshot,
                    stable_after_unix_ms: 1,
                    ignored_reason: None,
                    reconciliation_kind: WatchReconciliationKind::FullScope,
                    observed_at_unix_ms: 0,
                },
            )
            .expect("core Watch should remain revision-only");

        let active_binding = ScopePolicyBinding {
            scope_id: scope.id,
            revision: 1,
        };
        let write = ScopeExclusionWrite {
            kind: ScopeExclusionKind::Folder,
            path_raw: b"/scope/private",
            path_key: "/scope/private",
            display_path: "/scope/private",
            identity_kind: TEST_EXCLUDED_IDENTITY_KIND,
            identity_key: TEST_EXCLUDED_FOLDER_IDENTITY,
        };
        assert!(matches!(
            database.preview_scope_exclusion_batch(active_binding, &[write]),
            Err(DatabaseError::ScopeAccessGrantNotActive)
        ));
        assert!(matches!(
            database.apply_scope_exclusion_batch(active_binding, &[write], 1),
            Err(DatabaseError::ScopeAccessGrantNotActive)
        ));
        assert_eq!(
            database
                .current_scope_policy_revision(scope.id)
                .unwrap()
                .revision,
            1
        );
        assert!(database.scope_exclusions(scope.id).unwrap().is_empty());
        assert_eq!(
            database
                .connection
                .query_row("SELECT COUNT(*) FROM privacy_purge_receipts", [], |row| row
                    .get::<_, i64>(0))
                .unwrap(),
            0
        );
    }

    #[test]
    fn forged_foreign_platform_active_grant_fails_closed_every_packaged_boundary() {
        let (mut database, scope_id, node_id, _) = extraction_setup();
        let execution_source = database
            .action_execution_source_for_path_key(scope_id, "/scope/file.txt")
            .expect("action source should load before grant validation");
        let foreign = foreign_platform();
        database
            .connection
            .execute(
                "UPDATE authorized_scopes SET platform=?2 WHERE id=?1",
                params![scope_id, foreign],
            )
            .expect("foreign scope fixture should persist");
        database
            .connection
            .execute(
                "INSERT INTO scope_access_grants(scope_id,platform,opaque_grant,state,updated_at_unix_ms) \
                 VALUES(?1,?2,X'666F72676564','active',0)",
                params![scope_id, foreign],
            )
            .expect("forged foreign active row should persist");
        let forged_binding = ScopePolicyBinding {
            scope_id,
            revision: 1,
        };

        assert!(matches!(
            database.bind_scope_policy_revision(scope_id),
            Err(DatabaseError::ScopeAccessGrantNotActive)
        ));
        assert!(
            !database
                .is_scope_policy_binding_current(forged_binding)
                .expect("forged binding currentness should load")
        );
        assert!(
            database
                .is_core_scope_policy_binding_current(ScopeRevisionBinding {
                    scope_id,
                    revision: 1,
                })
                .expect("core binding currentness should remain revision-only")
        );
        assert!(!database.scope_has_active_access_grant(scope_id).unwrap());
        assert!(database.active_scope_access_grant_ids().unwrap().is_empty());
        assert!(database.list_active_scope_records().unwrap().is_empty());
        assert!(database.list_active_scope_grants().unwrap().is_empty());
        assert!(matches!(
            database.active_scope_grant(scope_id),
            Err(DatabaseError::ScopeAccessGrantNotActive)
        ));
        assert!(
            database
                .watchable_scope_ids_with_active_access_grants()
                .unwrap()
                .is_empty()
        );
        assert!(
            database
                .lexical_search_candidates("file", lexical_filters(Some(scope_id)), 10)
                .expect("search should fail closed as an empty result")
                .is_empty()
        );

        let source = &execution_source.source;
        let sha256 = [0x5a; 32];
        let action = database.create_rename_action_plan_with_policy(
            forged_binding,
            ActionPlanWrite {
                scope_id,
                node_id,
                source_location_id: source.location_id,
                source_path_raw: &source.path_raw,
                source_path_key: &source.path_key,
                source_display_path: &source.display_path,
                destination_path_raw: b"/scope/renamed.txt",
                destination_path_key: "/scope/renamed.txt",
                destination_display_path: "/scope/renamed.txt",
                source_identity_kind: &source.identity_kind,
                source_identity_key: &source.identity_key,
                source_size_bytes: source.size_bytes,
                source_modified_unix_ns: source.modified_unix_ns,
                source_sha256: &sha256,
                source_hash_bytes: source.size_bytes,
                scope_root_identity_kind: &execution_source.scope_root_identity_kind,
                scope_root_identity_key: &execution_source.scope_root_identity_key,
                parent_identity_kind: &execution_source.parent_identity_kind,
                parent_identity_key: &execution_source.parent_identity_key,
                execution_strategy: ActionExecutionStrategy::Direct,
            },
        );
        assert!(matches!(
            action,
            Err(DatabaseError::ScopeAccessGrantNotActive)
        ));

        let write = ScopeExclusionWrite {
            kind: ScopeExclusionKind::Folder,
            path_raw: b"/scope/private",
            path_key: "/scope/private",
            display_path: "/scope/private",
            identity_kind: TEST_EXCLUDED_IDENTITY_KIND,
            identity_key: TEST_EXCLUDED_FOLDER_IDENTITY,
        };
        assert!(matches!(
            database.preview_scope_exclusion_batch(forged_binding, &[write]),
            Err(DatabaseError::ScopeAccessGrantNotActive)
        ));
        assert!(matches!(
            database.apply_scope_exclusion_batch(forged_binding, &[write], 1),
            Err(DatabaseError::ScopeAccessGrantNotActive)
        ));
        assert_eq!(
            database
                .current_scope_policy_revision(scope_id)
                .unwrap()
                .revision,
            1
        );
        assert!(database.scope_exclusions(scope_id).unwrap().is_empty());
    }

    #[test]
    fn stale_exclusion_binding_rolls_back_policy_data_receipt_and_capability() {
        let (mut database, scope_id, _, _) = extraction_setup();
        let binding = test_active_binding(&database, scope_id).expect("active binding should load");
        let first = ScopeExclusionWrite {
            kind: ScopeExclusionKind::Folder,
            path_raw: b"/scope/old-private",
            path_key: "/scope/old-private",
            display_path: "/scope/old-private",
            identity_kind: TEST_EXCLUDED_IDENTITY_KIND,
            identity_key: TEST_EXCLUDED_FOLDER_IDENTITY,
        };
        database
            .apply_scope_exclusion_batch(binding, &[first], 1)
            .expect("first policy update should commit");
        let location_count_before: i64 = database
            .connection
            .query_row(
                "SELECT COUNT(*) FROM locations WHERE scope_id=?1",
                [scope_id],
                |row| row.get(0),
            )
            .unwrap();
        let stale_target = ScopeExclusionWrite {
            kind: ScopeExclusionKind::File,
            path_raw: b"/scope/file.txt",
            path_key: "/scope/file.txt",
            display_path: "/scope/file.txt",
            identity_kind: TEST_EXCLUDED_IDENTITY_KIND,
            identity_key: TEST_EXCLUDED_FILE_IDENTITY,
        };
        assert!(matches!(
            database.apply_scope_exclusion_batch(binding, &[stale_target], 2),
            Err(DatabaseError::ScopePolicyRevisionStale)
        ));
        assert_eq!(
            database
                .current_scope_policy_revision(scope_id)
                .unwrap()
                .revision,
            2
        );
        assert_eq!(database.scope_exclusions(scope_id).unwrap().len(), 1);
        assert_eq!(
            database
                .connection
                .query_row(
                    "SELECT COUNT(*) FROM locations WHERE scope_id=?1",
                    [scope_id],
                    |row| row.get::<_, i64>(0)
                )
                .unwrap(),
            location_count_before
        );
        assert_eq!(
            database
                .connection
                .query_row("SELECT COUNT(*) FROM privacy_purge_receipts", [], |row| row
                    .get::<_, i64>(0))
                .unwrap(),
            1
        );
        assert_eq!(
            database
                .connection
                .query_row(
                    "SELECT COUNT(*) FROM privacy_purge_capabilities",
                    [],
                    |row| row.get::<_, i64>(0)
                )
                .unwrap(),
            0
        );
    }

    #[test]
    fn exclusion_identity_validation_and_corrupt_stored_identity_fail_closed() {
        let (mut database, scope_id, _, _) = extraction_setup();
        let binding = test_active_binding(&database, scope_id).expect("scope should bind");
        let weak = ScopeExclusionWrite {
            kind: ScopeExclusionKind::Folder,
            path_raw: b"/scope/private",
            path_key: "/scope/private",
            display_path: "/scope/private",
            identity_kind: "path_fallback",
            identity_key: TEST_EXCLUDED_FOLDER_IDENTITY,
        };
        assert!(matches!(
            database.preview_scope_exclusion_batch(binding, &[weak]),
            Err(DatabaseError::ScopeExclusionInputInvalid)
        ));
        let mismatched_kind = ScopeExclusionWrite {
            identity_kind: TEST_EXCLUDED_IDENTITY_KIND,
            identity_key: TEST_EXCLUDED_FILE_IDENTITY,
            ..weak
        };
        assert!(matches!(
            database.preview_scope_exclusion_batch(binding, &[mismatched_kind]),
            Err(DatabaseError::ScopeExclusionInputInvalid)
        ));

        let valid = ScopeExclusionWrite {
            identity_kind: TEST_EXCLUDED_IDENTITY_KIND,
            identity_key: TEST_EXCLUDED_FOLDER_IDENTITY,
            ..weak
        };
        database
            .apply_scope_exclusion_batch(binding, &[valid], 2)
            .expect("stable identity should persist");
        let debug = format!("{valid:?}");
        assert!(!debug.contains("d0000000000000000"));
        database
            .connection
            .execute(
                "UPDATE scope_exclusions SET identity_key=?2 WHERE scope_id=?1",
                params![scope_id, TEST_EXCLUDED_FILE_IDENTITY],
            )
            .expect("corrupt pre-release fixture should persist within SQL checks");
        assert!(matches!(
            database.scope_exclusion_matcher(scope_id),
            Err(DatabaseError::InvalidStoredValue)
        ));
    }

    #[test]
    fn privacy_purge_capability_is_next_revision_bound_immutable_and_rollback_clean() {
        let (mut database, scope_id, _, _) = extraction_setup();
        let binding = test_active_binding(&database, scope_id).expect("scope should activate");
        let other = database
            .add_scope(b"/other", "/other", "/other", std::env::consts::OS)
            .expect("second scope should persist");
        database
            .upsert_scope_access_grant(other.id, std::env::consts::OS, b"other-grant")
            .expect("second scope grant should persist");
        let insert_exclusion = "INSERT INTO scope_exclusions( \
             scope_id,kind,path_raw,path_key,display_path,identity_kind,identity_key,policy_revision,created_at_unix_ms) \
             VALUES(?1,'folder',X'2F73636F70652F70726976617465','/scope/private','/scope/private', \
                    'unix_device_inode',X'6430303030303030303030303030303030',2,1)";

        assert!(
            database
                .connection
                .execute(insert_exclusion, [scope_id])
                .is_err()
        );
        assert!(database
            .connection
            .execute(
                "INSERT INTO privacy_purge_capabilities(nonce,scope_id,from_revision,to_revision,created_at_unix_ms) \
                 VALUES(zeroblob(32),?1,2,3,1)",
                [scope_id],
            )
            .is_err());

        {
            let transaction = database
                .connection
                .transaction_with_behavior(TransactionBehavior::Immediate)
                .unwrap();
            transaction.execute(
                "INSERT INTO privacy_purge_capabilities(nonce,scope_id,from_revision,to_revision,created_at_unix_ms) \
                 VALUES(zeroblob(32),?1,1,2,1)",
                [scope_id],
            ).unwrap();
            assert!(transaction
                .execute(
                    "UPDATE privacy_purge_capabilities SET created_at_unix_ms=2 WHERE scope_id=?1",
                    [scope_id],
                )
                .is_err());
            assert!(transaction.execute(insert_exclusion, [scope_id]).is_err());
            transaction.rollback().unwrap();
        }
        {
            let transaction = database
                .connection
                .transaction_with_behavior(TransactionBehavior::Immediate)
                .unwrap();
            transaction.execute(
                "INSERT INTO privacy_purge_capabilities(nonce,scope_id,from_revision,to_revision,created_at_unix_ms) \
                 VALUES(zeroblob(32),?1,1,2,1)",
                [other.id],
            ).unwrap();
            transaction.execute(
                "UPDATE authorized_scopes SET policy_revision=2 WHERE id=?1 AND policy_revision=1",
                [scope_id],
            ).unwrap();
            assert!(transaction.execute(insert_exclusion, [scope_id]).is_err());
            transaction.rollback().unwrap();
        }
        {
            let transaction = database
                .connection
                .transaction_with_behavior(TransactionBehavior::Immediate)
                .unwrap();
            transaction.execute(
                "INSERT INTO privacy_purge_capabilities(nonce,scope_id,from_revision,to_revision,created_at_unix_ms) \
                 VALUES(zeroblob(32),?1,1,2,1)",
                [scope_id],
            ).unwrap();
            transaction.execute(
                "UPDATE authorized_scopes SET policy_revision=2 WHERE id=?1 AND policy_revision=1",
                [scope_id],
            ).unwrap();
            transaction.execute(insert_exclusion, [scope_id]).unwrap();
            assert!(
                transaction
                    .execute(
                        "DELETE FROM privacy_purge_capabilities WHERE scope_id=?1",
                        [scope_id],
                    )
                    .is_err()
            );
            transaction.rollback().unwrap();
        }
        assert_eq!(
            database
                .current_scope_policy_revision(scope_id)
                .unwrap()
                .revision,
            1
        );
        assert!(database.scope_exclusions(scope_id).unwrap().is_empty());
        assert_eq!(
            database
                .connection
                .query_row(
                    "SELECT COUNT(*) FROM privacy_purge_capabilities",
                    [],
                    |row| row.get::<_, i64>(0)
                )
                .unwrap(),
            0
        );

        database
            .apply_scope_exclusion_batch(
                binding,
                &[ScopeExclusionWrite {
                    kind: ScopeExclusionKind::Folder,
                    path_raw: b"/scope/private",
                    path_key: "/scope/private",
                    display_path: "/scope/private",
                    identity_kind: TEST_EXCLUDED_IDENTITY_KIND,
                    identity_key: TEST_EXCLUDED_FOLDER_IDENTITY,
                }],
                2,
            )
            .expect("normal apply should create its receipt then consume the capability");
        assert_eq!(
            database
                .current_scope_policy_revision(scope_id)
                .unwrap()
                .revision,
            2
        );
        assert_eq!(
            database
                .connection
                .query_row(
                    "SELECT COUNT(*) FROM privacy_purge_receipts WHERE scope_id=?1",
                    [scope_id],
                    |row| row.get::<_, i64>(0)
                )
                .unwrap(),
            1
        );
        assert_eq!(
            database
                .connection
                .query_row(
                    "SELECT COUNT(*) FROM privacy_purge_capabilities",
                    [],
                    |row| row.get::<_, i64>(0)
                )
                .unwrap(),
            0
        );
    }

    #[test]
    fn scope_root_revocation_atomically_wipes_grant_and_scope_derived_data() {
        let (mut database, scope_id, node_id, _) = extraction_setup();
        let initial_binding =
            test_active_binding(&database, scope_id).expect("scope should activate");
        let extraction = database
            .create_extraction_job(scope_id, node_id)
            .expect("content extraction should queue");
        database
            .claim_extraction_job(extraction.job_id, "revoke-runner", 60_000)
            .expect("content extraction should claim");
        database
            .complete_extraction_job(
                extraction.job_id,
                "revoke-runner",
                "deskgraph.utf8-text",
                "1",
                4,
                Some(1),
                20,
                1,
                &[ContentChunkWrite {
                    ordinal: 0,
                    text: "private root content".to_string(),
                    provenance: ContentChunkProvenanceWrite::ByteRange { start: 0, end: 4 },
                    trust_class: "untrusted_extracted_text",
                }],
            )
            .expect("content should publish");
        let scan_id: i64 = database
            .connection
            .query_row(
                "SELECT MAX(id) FROM scan_jobs WHERE scope_id=?1",
                [scope_id],
                |row| row.get(0),
            )
            .expect("scan should exist");
        database
            .connection
            .execute(
                "INSERT INTO scan_issues(scan_id, code, path_key, detail_code) \
                 VALUES (?1, 'source_unavailable', '/scope/private-path', 'fixture')",
                [scan_id],
            )
            .expect("path-bearing scan issue should persist");
        database
            .apply_scope_exclusion_batch(
                initial_binding,
                &[ScopeExclusionWrite {
                    kind: ScopeExclusionKind::Folder,
                    path_raw: b"/scope/excluded",
                    path_key: "/scope/excluded",
                    display_path: "/scope/excluded",
                    identity_kind: TEST_EXCLUDED_IDENTITY_KIND,
                    identity_key: TEST_EXCLUDED_FOLDER_IDENTITY,
                }],
                2,
            )
            .expect("non-matching exclusion should persist without removing the fixture");
        record_file_watch_event(&mut database, scope_id, "/scope/watched.txt", 5, None);
        let binding = database
            .bind_scope_policy_revision(scope_id)
            .expect("revised scope should bind");
        let preview = database
            .preview_scope_root_revocation(binding)
            .expect("revocation impact should preview without mutation");
        assert_eq!(preview.scope_id, scope_id);
        assert_eq!(preview.base_policy_revision, 2);
        assert_eq!(preview.exclusion_count, 1);
        assert!(preview.impact.conservative_location_count >= 2);
        assert_eq!(preview.impact.content_chunk_count, 1);
        assert_eq!(preview.impact.watch_event_count, 1);
        assert!(
            database
                .scope_has_active_access_grant(scope_id)
                .expect("preview must leave grant active")
        );

        let applied = database
            .apply_scope_root_revocation(binding, 3)
            .expect("root revocation should commit atomically");
        assert_eq!(applied.policy.revision, 3);
        assert_eq!(applied.receipt.exclusions_removed, 1);
        assert!(applied.receipt.purged_row_count > 0);
        let grant = database
            .scope_access_grant(scope_id)
            .expect("revoked grant should load")
            .expect("grant tombstone should remain");
        assert_eq!(grant.state, ScopeAccessGrantState::Revoked);
        assert_eq!(grant.opaque_grant, [0]);
        assert!(
            database
                .connection
                .execute(
                    "UPDATE scope_access_grants SET opaque_grant=X'7265757361626C65' \
                     WHERE scope_id=?1 AND state='revoked'",
                    [scope_id],
                )
                .is_err(),
            "an already-revoked row must reject reusable capability bytes"
        );
        let inserted_revoked_scope = database
            .add_scope(
                b"/scope/revoked-insert",
                "/scope/revoked-insert",
                "/scope/revoked-insert",
                std::env::consts::OS,
            )
            .expect("a separate scope should persist");
        assert!(
            database
                .connection
                .execute(
                    "INSERT INTO scope_access_grants( \
                         scope_id, platform, opaque_grant, state, updated_at_unix_ms \
                     ) VALUES (?1, ?2, X'7265757361626C65', 'revoked', 4)",
                    params![inserted_revoked_scope.id, std::env::consts::OS],
                )
                .is_err(),
            "a newly inserted revoked row must reject reusable capability bytes"
        );
        database
            .connection
            .execute(
                "INSERT INTO scope_access_grants( \
                     scope_id, platform, opaque_grant, state, updated_at_unix_ms \
                 ) VALUES (?1, ?2, X'00', 'revoked', 4)",
                params![inserted_revoked_scope.id, std::env::consts::OS],
            )
            .expect("the exact fixed tombstone should remain representable");
        for table in [
            "locations",
            "content_chunks",
            "extraction_jobs",
            "watch_events",
            "scan_issues",
            "scope_exclusions",
            "privacy_purge_capabilities",
        ] {
            let sql = format!("SELECT COUNT(*) FROM {table}");
            assert_eq!(
                database
                    .connection
                    .query_row(&sql, [], |row| row.get::<_, i64>(0))
                    .unwrap(),
                0,
                "{table} must not retain root-derived data"
            );
        }
        assert_eq!(
            database
                .connection
                .query_row(
                    "SELECT COUNT(*) FROM content_search_fts WHERE content_search_fts MATCH 'private'",
                    [],
                    |row| row.get::<_, i64>(0),
                )
                .unwrap(),
            0
        );
        assert_eq!(
            database
                .connection
                .query_row(
                    "SELECT COUNT(*) FROM scope_root_revocation_receipts WHERE scope_id=?1",
                    [scope_id],
                    |row| row.get::<_, i64>(0),
                )
                .unwrap(),
            1
        );
        assert!(
            database
                .connection
                .execute(
                    "DELETE FROM scope_root_revocation_receipts WHERE scope_id=?1",
                    [scope_id],
                )
                .is_err(),
            "revocation receipt must be immutable"
        );
    }

    #[test]
    fn scope_root_revocation_rejects_a_preview_when_derived_impact_changed() {
        let (database, scope_id, _, _) = extraction_setup();
        let binding = test_active_binding(&database, scope_id).expect("scope should become active");
        let preview = database
            .preview_scope_root_revocation(binding)
            .expect("root revocation should preview");
        database
            .connection
            .execute(
                "INSERT INTO scan_jobs( \
                     scope_id, status, started_at_unix_ms, policy_revision \
                 ) VALUES (?1, 'interrupted', 2, ?2)",
                params![scope_id, binding.revision],
            )
            .expect("derived state should change without a policy revision change");

        assert!(matches!(
            database.apply_scope_root_revocation_from_preview(preview, 3),
            Err(DatabaseError::ScopeRootRevocationPreviewStale)
        ));
        assert_eq!(
            database
                .current_scope_policy_revision(scope_id)
                .expect("policy should remain readable")
                .revision,
            binding.revision
        );
        assert!(
            database
                .scope_has_active_access_grant(scope_id)
                .expect("a stale confirmation must leave the grant active")
        );
        let fresh = database
            .preview_scope_root_revocation(binding)
            .expect("a fresh impact should preview");
        assert_eq!(
            fresh.impact.scan_job_count,
            preview.impact.scan_job_count + 1
        );
        database
            .apply_scope_root_revocation_from_preview(fresh, 4)
            .expect("the fresh exact preview should commit");
        assert_eq!(
            database
                .scope_access_grant_state(scope_id)
                .expect("grant state should load"),
            ScopeAccessGrantState::Revoked
        );
    }

    #[test]
    fn scope_filesystem_fence_drains_a_reader_before_revocation_and_denies_new_reads() {
        let directory = tempfile::tempdir().expect("tempdir should exist");
        let database_path = directory.path().join("manifest.sqlite3");
        let mut database =
            ManifestDatabase::open(&database_path).expect("database should initialize");
        let scope = database
            .add_scope_with_access_grant(
                b"/cooperative-fence",
                "/cooperative-fence",
                "/cooperative-fence",
                ScopeAccessGrantWrite {
                    scope_platform: std::env::consts::OS,
                    grant_platform: std::env::consts::OS,
                    opaque_grant: b"cooperative-fence-grant",
                    state: ScopeAccessGrantState::Active,
                },
            )
            .expect("scope and grant should persist");
        let preview = database
            .preview_scope_root_revocation(
                database
                    .bind_scope_policy_revision(scope.id)
                    .expect("scope should bind"),
            )
            .expect("revocation should preview");
        let reader = database
            .acquire_scope_filesystem_read_fence(scope.id)
            .expect("reader should acquire the shared fence");
        assert_eq!(reader.binding().scope_id, scope.id);

        let writer_path = database_path.clone();
        let (writer_started_tx, writer_started_rx) = std::sync::mpsc::sync_channel(1);
        let (writer_finished_tx, writer_finished_rx) = std::sync::mpsc::sync_channel(1);
        let writer = std::thread::spawn(move || {
            let writer_database =
                ManifestDatabase::open(&writer_path).expect("writer database should open");
            writer_started_tx
                .send(())
                .expect("writer start should be observable");
            writer_database
                .apply_scope_root_revocation_from_preview(preview, 2)
                .expect("the public API must fence and commit after readers drain");
            writer_finished_tx
                .send(())
                .expect("writer completion should be observable");
        });
        writer_started_rx
            .recv()
            .expect("writer should reach fence acquisition");
        assert!(
            writer_finished_rx
                .recv_timeout(Duration::from_millis(50))
                .is_err(),
            "exclusive revocation must wait for the active shared reader"
        );
        drop(reader);
        writer_finished_rx
            .recv_timeout(Duration::from_secs(2))
            .expect("revocation should finish after the reader drops");
        writer.join().expect("writer thread should not panic");

        let reopened = ManifestDatabase::open(&database_path).expect("database should reopen");
        assert!(matches!(
            reopened.acquire_scope_filesystem_read_fence(scope.id),
            Err(DatabaseError::ScopeAccessGrantNotActive)
        ));
    }

    #[test]
    fn scope_filesystem_fence_uses_policy_checks_for_an_in_memory_database() {
        let mut database =
            ManifestDatabase::open_in_memory().expect("in-memory database should initialize");
        let scope = database
            .add_scope_with_access_grant(
                b"/process-local-fence",
                "/process-local-fence",
                "/process-local-fence",
                ScopeAccessGrantWrite {
                    scope_platform: std::env::consts::OS,
                    grant_platform: std::env::consts::OS,
                    opaque_grant: b"process-local-fence-grant",
                    state: ScopeAccessGrantState::Active,
                },
            )
            .expect("scope and grant should persist");

        let reader = database
            .acquire_scope_filesystem_read_fence(scope.id)
            .expect("process-local reader should use repeated policy checks");
        assert_eq!(reader.binding().scope_id, scope.id);
    }

    #[test]
    fn scope_filesystem_fence_rejects_a_replaced_lock_inode() {
        let directory = tempfile::tempdir().expect("tempdir should exist");
        let database_path = directory.path().join("manifest.sqlite3");
        let mut database =
            ManifestDatabase::open(&database_path).expect("database should initialize");
        let scope = database
            .add_scope_with_access_grant(
                b"/replaced-fence",
                "/replaced-fence",
                "/replaced-fence",
                ScopeAccessGrantWrite {
                    scope_platform: std::env::consts::OS,
                    grant_platform: std::env::consts::OS,
                    opaque_grant: b"replaced-fence-grant",
                    state: ScopeAccessGrantState::Active,
                },
            )
            .expect("scope and grant should persist");
        let reader = database
            .acquire_scope_filesystem_read_fence(scope.id)
            .expect("reader should bind and hold the original inode");
        let fence_root = directory.path().join("scope-read-fences-v1");
        let data_path = fence_root.join(format!("scope-{}.lock", scope.id));
        let displaced_path = fence_root.join(format!("scope-{}.displaced", scope.id));
        fs::rename(&data_path, &displaced_path).expect("test should displace the lock inode");
        let replacement = OpenOptions::new()
            .read(true)
            .write(true)
            .create_new(true)
            .open(&data_path)
            .expect("test should create a replacement inode");
        drop(replacement);

        let reopened = ManifestDatabase::open(&database_path).expect("database should reopen");
        assert!(matches!(
            reopened.acquire_scope_filesystem_revocation_fence(scope.id),
            Err(DatabaseError::ScopeFilesystemFenceInvalid)
        ));
        assert!(
            reopened
                .scope_has_active_access_grant(scope.id)
                .expect("failed fence admission must not revoke the grant")
        );
        drop(reader);
    }

    #[test]
    fn scope_filesystem_revocation_fence_is_scope_and_revision_matched() {
        let directory = tempfile::tempdir().expect("tempdir should exist");
        let database_path = directory.path().join("manifest.sqlite3");
        let mut database =
            ManifestDatabase::open(&database_path).expect("database should initialize");
        let first = database
            .add_scope_with_access_grant(
                b"/first-fence-scope",
                "/first-fence-scope",
                "/first-fence-scope",
                ScopeAccessGrantWrite {
                    scope_platform: std::env::consts::OS,
                    grant_platform: std::env::consts::OS,
                    opaque_grant: b"first-fence-grant",
                    state: ScopeAccessGrantState::Active,
                },
            )
            .expect("first scope should persist");
        let second = database
            .add_scope_with_access_grant(
                b"/second-fence-scope",
                "/second-fence-scope",
                "/second-fence-scope",
                ScopeAccessGrantWrite {
                    scope_platform: std::env::consts::OS,
                    grant_platform: std::env::consts::OS,
                    opaque_grant: b"second-fence-grant",
                    state: ScopeAccessGrantState::Active,
                },
            )
            .expect("second scope should persist");
        let fence = database
            .acquire_scope_filesystem_revocation_fence(first.id)
            .expect("first scope fence should acquire");
        let second_binding = database
            .bind_scope_policy_revision(second.id)
            .expect("second scope should bind");

        assert!(matches!(
            database.apply_scope_root_revocation_with_fence(&fence, second_binding, 2),
            Err(DatabaseError::ScopeFilesystemFenceInvalid)
        ));
        assert!(
            database
                .scope_has_active_access_grant(second.id)
                .expect("mismatched fence must leave the grant active")
        );
    }

    #[test]
    fn scope_filesystem_revocation_fence_cannot_cross_manifest_domains() {
        let mut first =
            ManifestDatabase::open_in_memory().expect("first manifest should initialize");
        let first_scope = first
            .add_scope_with_access_grant(
                b"/first-manifest",
                "/first-manifest",
                "/first-manifest",
                ScopeAccessGrantWrite {
                    scope_platform: std::env::consts::OS,
                    grant_platform: std::env::consts::OS,
                    opaque_grant: b"first-manifest-grant",
                    state: ScopeAccessGrantState::Active,
                },
            )
            .expect("first scope should persist");
        let fence = first
            .acquire_scope_filesystem_revocation_fence(first_scope.id)
            .expect("first manifest fence should acquire");

        let mut second =
            ManifestDatabase::open_in_memory().expect("second manifest should initialize");
        let second_scope = second
            .add_scope_with_access_grant(
                b"/second-manifest",
                "/second-manifest",
                "/second-manifest",
                ScopeAccessGrantWrite {
                    scope_platform: std::env::consts::OS,
                    grant_platform: std::env::consts::OS,
                    opaque_grant: b"second-manifest-grant",
                    state: ScopeAccessGrantState::Active,
                },
            )
            .expect("second scope should persist");
        let second_binding = second
            .bind_scope_policy_revision(second_scope.id)
            .expect("second scope should bind");
        assert_eq!(fence.binding(), second_binding);

        assert!(matches!(
            second.apply_scope_root_revocation_with_fence(&fence, second_binding, 2),
            Err(DatabaseError::ScopeFilesystemFenceInvalid)
        ));
        assert!(
            second
                .scope_has_active_access_grant(second_scope.id)
                .expect("foreign-domain token must leave the second grant active")
        );
    }

    #[test]
    fn scope_filesystem_read_fence_cannot_cross_manifest_domains() {
        let mut first =
            ManifestDatabase::open_in_memory().expect("first manifest should initialize");
        let first_scope = first
            .add_scope_with_access_grant(
                b"/first-read-manifest",
                "/first-read-manifest",
                "/first-read-manifest",
                ScopeAccessGrantWrite {
                    scope_platform: std::env::consts::OS,
                    grant_platform: std::env::consts::OS,
                    opaque_grant: b"first-read-manifest-grant",
                    state: ScopeAccessGrantState::Active,
                },
            )
            .expect("first scope should persist");
        let fence = first
            .acquire_scope_filesystem_read_fence(first_scope.id)
            .expect("first manifest read fence should acquire");

        let mut second =
            ManifestDatabase::open_in_memory().expect("second manifest should initialize");
        let second_scope = second
            .add_scope_with_access_grant(
                b"/second-read-manifest",
                "/second-read-manifest",
                "/second-read-manifest",
                ScopeAccessGrantWrite {
                    scope_platform: std::env::consts::OS,
                    grant_platform: std::env::consts::OS,
                    opaque_grant: b"second-read-manifest-grant",
                    state: ScopeAccessGrantState::Active,
                },
            )
            .expect("second scope should persist");
        let second_binding = second
            .bind_scope_policy_revision(second_scope.id)
            .expect("second scope should bind");
        assert_eq!(fence.binding(), second_binding);

        assert!(matches!(
            second.validate_scope_filesystem_read_fence(&fence, second_binding),
            Err(DatabaseError::ScopeFilesystemFenceInvalid)
        ));
    }

    #[test]
    fn scope_filesystem_revocation_wait_is_bounded() {
        let directory = tempfile::tempdir().expect("tempdir should exist");
        let database_path = directory.path().join("manifest.sqlite3");
        let mut database =
            ManifestDatabase::open(&database_path).expect("database should initialize");
        let scope = database
            .add_scope_with_access_grant(
                b"/bounded-fence",
                "/bounded-fence",
                "/bounded-fence",
                ScopeAccessGrantWrite {
                    scope_platform: std::env::consts::OS,
                    grant_platform: std::env::consts::OS,
                    opaque_grant: b"bounded-fence-grant",
                    state: ScopeAccessGrantState::Active,
                },
            )
            .expect("scope and grant should persist");
        let reader = database
            .acquire_scope_filesystem_read_fence(scope.id)
            .expect("reader should acquire");
        let reopened = ManifestDatabase::open(&database_path).expect("database should reopen");
        let started = Instant::now();
        assert!(matches!(
            reopened.acquire_scope_filesystem_revocation_fence(scope.id),
            Err(DatabaseError::ScopeFilesystemFenceBusy)
        ));
        assert!(
            started.elapsed() < Duration::from_secs(3),
            "exclusive admission must return a retryable error instead of hanging"
        );
        drop(reader);
    }

    #[cfg(unix)]
    #[test]
    fn scope_filesystem_fence_rejects_a_symlinked_root_without_chmodding_its_target() {
        let directory = tempfile::tempdir().expect("tempdir should exist");
        let database_path = directory.path().join("manifest.sqlite3");
        let mut database =
            ManifestDatabase::open(&database_path).expect("database should initialize");
        let scope = database
            .add_scope_with_access_grant(
                b"/symlinked-fence-root",
                "/symlinked-fence-root",
                "/symlinked-fence-root",
                ScopeAccessGrantWrite {
                    scope_platform: std::env::consts::OS,
                    grant_platform: std::env::consts::OS,
                    opaque_grant: b"symlinked-fence-root-grant",
                    state: ScopeAccessGrantState::Active,
                },
            )
            .expect("scope and grant should persist");
        let target = directory.path().join("fence-target");
        fs::create_dir(&target).expect("target should exist");
        fs::set_permissions(&target, fs::Permissions::from_mode(0o755))
            .expect("target permissions should be explicit");
        std::os::unix::fs::symlink(&target, directory.path().join("scope-read-fences-v1"))
            .expect("fence root symlink should exist");

        assert!(matches!(
            database.acquire_scope_filesystem_read_fence(scope.id),
            Err(DatabaseError::ScopeFilesystemFenceInvalid)
        ));
        assert_eq!(
            fs::metadata(&target)
                .expect("target should remain")
                .permissions()
                .mode()
                & 0o777,
            0o755,
            "fail-closed validation must happen before any path-based chmod"
        );
    }

    #[cfg(unix)]
    #[test]
    fn scope_filesystem_fence_rejects_a_hard_link_without_chmodding_its_target() {
        let directory = tempfile::tempdir().expect("tempdir should exist");
        let database_path = directory.path().join("manifest.sqlite3");
        let mut database =
            ManifestDatabase::open(&database_path).expect("database should initialize");
        let scope = database
            .add_scope_with_access_grant(
                b"/hard-linked-fence",
                "/hard-linked-fence",
                "/hard-linked-fence",
                ScopeAccessGrantWrite {
                    scope_platform: std::env::consts::OS,
                    grant_platform: std::env::consts::OS,
                    opaque_grant: b"hard-linked-fence-grant",
                    state: ScopeAccessGrantState::Active,
                },
            )
            .expect("scope and grant should persist");
        let target = directory.path().join("unrelated-target");
        fs::write(&target, b"must not be changed").expect("target should exist");
        fs::set_permissions(&target, fs::Permissions::from_mode(0o644))
            .expect("target permissions should be explicit");
        let fence_root = directory.path().join("scope-read-fences-v1");
        fs::create_dir(&fence_root).expect("fence root should exist");
        fs::set_permissions(&fence_root, fs::Permissions::from_mode(0o700))
            .expect("fence root should be private");
        fs::hard_link(&target, fence_root.join(format!("scope-{}.lock", scope.id)))
            .expect("hard-linked fence entry should exist");

        assert!(matches!(
            database.acquire_scope_filesystem_read_fence(scope.id),
            Err(DatabaseError::ScopeFilesystemFenceInvalid)
        ));
        assert_eq!(
            fs::metadata(&target)
                .expect("target should remain")
                .permissions()
                .mode()
                & 0o777,
            0o644,
            "invalid lock entries must be rejected before any permission mutation"
        );
        assert_eq!(
            fs::read(&target).expect("target bytes should remain"),
            b"must not be changed"
        );
    }

    #[cfg(unix)]
    #[test]
    fn scope_filesystem_fence_children_open_relative_to_the_pinned_root() {
        let directory = tempfile::tempdir().expect("tempdir should exist");
        let database_path = directory.path().join("manifest.sqlite3");
        let mut database =
            ManifestDatabase::open(&database_path).expect("database should initialize");
        let first = database
            .add_scope_with_access_grant(
                b"/pinned-root-first",
                "/pinned-root-first",
                "/pinned-root-first",
                ScopeAccessGrantWrite {
                    scope_platform: std::env::consts::OS,
                    grant_platform: std::env::consts::OS,
                    opaque_grant: b"pinned-root-first-grant",
                    state: ScopeAccessGrantState::Active,
                },
            )
            .expect("first scope should persist");
        database
            .acquire_scope_filesystem_read_fence(first.id)
            .expect("first fence should create the private root");
        let second = database
            .add_scope_with_access_grant(
                b"/pinned-root-second",
                "/pinned-root-second",
                "/pinned-root-second",
                ScopeAccessGrantWrite {
                    scope_platform: std::env::consts::OS,
                    grant_platform: std::env::consts::OS,
                    opaque_grant: b"pinned-root-second-grant",
                    state: ScopeAccessGrantState::Active,
                },
            )
            .expect("second scope should persist");
        let fence_root = directory.path().join("scope-read-fences-v1");
        let pinned_root = File::open(&fence_root).expect("root descriptor should pin");
        let displaced_root = directory.path().join("scope-read-fences-displaced");
        let wrong_target = directory.path().join("wrong-fence-target");
        fs::rename(&fence_root, &displaced_root).expect("test should displace the root pathname");
        fs::create_dir(&wrong_target).expect("wrong target should exist");
        std::os::unix::fs::symlink(&wrong_target, &fence_root)
            .expect("replacement root symlink should exist");

        let entry_name = format!("scope-{}.gate", second.id);
        database
            .open_scope_filesystem_fence_file_at_unix(
                &pinned_root,
                second.id,
                ScopeFilesystemFenceRole::Gate,
                &entry_name,
                &fence_root,
            )
            .expect("relative open should stay inside the pinned root");
        assert!(displaced_root.join(&entry_name).is_file());
        assert!(
            fs::read_dir(&wrong_target)
                .expect("wrong target should remain readable")
                .next()
                .is_none(),
            "a swapped pathname must receive no created child"
        );
    }

    #[cfg(unix)]
    #[test]
    fn scope_filesystem_fence_converges_across_database_path_aliases() {
        let database_directory = tempfile::tempdir().expect("database tempdir should exist");
        let alias_directory = tempfile::tempdir().expect("alias tempdir should exist");
        let database_path = database_directory.path().join("manifest.sqlite3");
        let alias_path = alias_directory.path().join("manifest-alias.sqlite3");
        let mut database =
            ManifestDatabase::open(&database_path).expect("database should initialize");
        let scope = database
            .add_scope_with_access_grant(
                b"/aliased-cooperative-fence",
                "/aliased-cooperative-fence",
                "/aliased-cooperative-fence",
                ScopeAccessGrantWrite {
                    scope_platform: std::env::consts::OS,
                    grant_platform: std::env::consts::OS,
                    opaque_grant: b"aliased-cooperative-fence-grant",
                    state: ScopeAccessGrantState::Active,
                },
            )
            .expect("scope and grant should persist");
        let preview = database
            .preview_scope_root_revocation(
                database
                    .bind_scope_policy_revision(scope.id)
                    .expect("scope should bind"),
            )
            .expect("revocation should preview");
        std::os::unix::fs::symlink(&database_path, &alias_path)
            .expect("database alias should be created");
        let reader = database
            .acquire_scope_filesystem_read_fence(scope.id)
            .expect("reader should acquire the shared fence");

        let (writer_finished_tx, writer_finished_rx) = std::sync::mpsc::sync_channel(1);
        let writer = std::thread::spawn(move || {
            let writer_database =
                ManifestDatabase::open(&alias_path).expect("aliased writer database should open");
            let exclusive = writer_database
                .acquire_scope_filesystem_revocation_fence(scope.id)
                .expect("aliased writer should acquire the same exclusive fence");
            writer_database
                .apply_scope_root_revocation_from_preview_with_fence(&exclusive, preview, 2)
                .expect("revocation should commit after the aliased reader drains");
            writer_finished_tx
                .send(())
                .expect("writer completion should be observable");
        });
        assert!(
            writer_finished_rx
                .recv_timeout(Duration::from_millis(50))
                .is_err(),
            "database path aliases must not split the per-scope fence"
        );
        drop(reader);
        writer_finished_rx
            .recv_timeout(Duration::from_secs(2))
            .expect("aliased revocation should finish after the reader drops");
        writer.join().expect("writer thread should not panic");
    }

    #[test]
    fn scope_filesystem_fence_releases_after_a_reader_process_exits() {
        const CHILD_ENV: &str = "DESKGRAPH_SCOPE_FENCE_CHILD";
        const DATABASE_ENV: &str = "DESKGRAPH_SCOPE_FENCE_DATABASE";
        const SCOPE_ENV: &str = "DESKGRAPH_SCOPE_FENCE_SCOPE";
        const READY_ENV: &str = "DESKGRAPH_SCOPE_FENCE_READY";

        if std::env::var_os(CHILD_ENV).is_some() {
            let database_path =
                PathBuf::from(std::env::var_os(DATABASE_ENV).expect("child database path"));
            let scope_id = std::env::var(SCOPE_ENV)
                .expect("child scope id")
                .parse::<i64>()
                .expect("child scope id should parse");
            let ready_path = PathBuf::from(std::env::var_os(READY_ENV).expect("child ready path"));
            let database =
                ManifestDatabase::open(&database_path).expect("child database should open");
            let _reader = database
                .acquire_scope_filesystem_read_fence(scope_id)
                .expect("child should acquire the shared fence");
            fs::write(ready_path, b"ready").expect("child readiness should persist");
            std::thread::sleep(Duration::from_secs(30));
            return;
        }

        let directory = tempfile::tempdir().expect("tempdir should exist");
        let database_path = directory.path().join("manifest.sqlite3");
        let ready_path = directory.path().join("reader-ready");
        let mut database =
            ManifestDatabase::open(&database_path).expect("database should initialize");
        let scope = database
            .add_scope_with_access_grant(
                b"/child-fence",
                "/child-fence",
                "/child-fence",
                ScopeAccessGrantWrite {
                    scope_platform: std::env::consts::OS,
                    grant_platform: std::env::consts::OS,
                    opaque_grant: b"child-fence-grant",
                    state: ScopeAccessGrantState::Active,
                },
            )
            .expect("scope and grant should persist");
        let preview = database
            .preview_scope_root_revocation(
                database
                    .bind_scope_policy_revision(scope.id)
                    .expect("scope should bind"),
            )
            .expect("revocation should preview");
        let mut child = std::process::Command::new(
            std::env::current_exe().expect("test executable should resolve"),
        )
        .args([
            "--exact",
            "tests::scope_filesystem_fence_releases_after_a_reader_process_exits",
            "--nocapture",
        ])
        .env(CHILD_ENV, "1")
        .env(DATABASE_ENV, &database_path)
        .env(SCOPE_ENV, scope.id.to_string())
        .env(READY_ENV, &ready_path)
        .spawn()
        .expect("reader child should spawn");
        let ready_deadline = Instant::now() + Duration::from_secs(5);
        while !ready_path.exists() && Instant::now() < ready_deadline {
            std::thread::sleep(Duration::from_millis(10));
        }
        assert!(ready_path.exists(), "reader child must acquire the fence");

        let writer_path = database_path.clone();
        let (writer_finished_tx, writer_finished_rx) = std::sync::mpsc::sync_channel(1);
        let writer = std::thread::spawn(move || {
            let writer_database =
                ManifestDatabase::open(&writer_path).expect("writer database should open");
            let exclusive = writer_database
                .acquire_scope_filesystem_revocation_fence(scope.id)
                .expect("writer should acquire the exclusive fence");
            writer_database
                .apply_scope_root_revocation_from_preview_with_fence(&exclusive, preview, 2)
                .expect("revocation should commit after child exit");
            writer_finished_tx
                .send(())
                .expect("writer completion should be observable");
        });
        assert!(
            writer_finished_rx
                .recv_timeout(Duration::from_millis(50))
                .is_err(),
            "the reader process must hold the revoker before exit"
        );
        child.kill().expect("reader child should terminate");
        let child_status = child.wait().expect("reader child should be reaped");
        assert!(!child_status.success(), "the child should be force-stopped");
        writer_finished_rx
            .recv_timeout(Duration::from_secs(2))
            .expect("OS process exit must release the shared fence");
        writer.join().expect("writer thread should not panic");
    }

    #[test]
    fn reauthorization_requires_a_fresh_scan_for_watch_readiness_and_stats() {
        let (mut database, scope_id, _, root) = extraction_setup();
        let binding = test_active_binding(&database, scope_id)
            .expect("initial scope grant should become active");

        assert!(
            database
                .scope_has_completed_scan(scope_id)
                .expect("initial scan readiness should load")
        );
        assert_eq!(
            database
                .watchable_scope_ids_with_active_access_grants()
                .expect("initial watchability should load"),
            vec![scope_id]
        );
        assert_eq!(
            database
                .stats_with_active_access_grants()
                .expect("initial dashboard stats should load")
                .completed_scan_count,
            1
        );

        database
            .apply_scope_root_revocation(binding, 2)
            .expect("root revocation should commit");
        database
            .upsert_scope_access_grant(scope_id, std::env::consts::OS, b"reauthorized-grant")
            .expect("reauthorization should restore only the grant");

        assert!(
            !database
                .scope_has_completed_scan(scope_id)
                .expect("revoked scan history must not satisfy readiness")
        );
        assert!(
            database
                .watchable_scope_ids()
                .expect("revoked scan history must not make core watchable")
                .is_empty()
        );
        assert!(
            database
                .watchable_scope_ids_with_active_access_grants()
                .expect("revoked scan history must not make Desktop watchable")
                .is_empty()
        );
        let stale_stats = database
            .stats_with_active_access_grants()
            .expect("reauthorized dashboard stats should load");
        assert_eq!(stale_stats.node_count, 0);
        assert_eq!(stale_stats.file_count, 0);
        assert_eq!(stale_stats.folder_count, 0);
        assert_eq!(stale_stats.active_location_count, 0);
        assert_eq!(stale_stats.issue_count, 0);
        assert_eq!(stale_stats.completed_scan_count, 0);

        publish_manifest_file(&mut database, scope_id, &root, 4);

        assert!(
            database
                .scope_has_completed_scan(scope_id)
                .expect("fresh scan should restore readiness")
        );
        assert_eq!(
            database
                .watchable_scope_ids_with_active_access_grants()
                .expect("fresh scan should restore Desktop watchability"),
            vec![scope_id]
        );
        assert_eq!(
            database
                .stats_with_active_access_grants()
                .expect("fresh dashboard stats should load")
                .completed_scan_count,
            1
        );
    }

    #[test]
    fn scope_root_revocation_blocks_non_pristine_action_history_and_rolls_back() {
        let (mut database, scope_id, node_id, _) = extraction_setup();
        test_active_binding(&database, scope_id).expect("scope should activate");
        let preview = create_bound_rename_preview(&mut database, scope_id, node_id);
        insert_terminal_action_fixture(&mut database, preview.plan_id, "terminal-revoke-0001");
        let binding = database
            .bind_scope_policy_revision(scope_id)
            .expect("scope should bind");
        let location_count: i64 = database
            .connection
            .query_row(
                "SELECT COUNT(*) FROM locations WHERE scope_id=?1",
                [scope_id],
                |row| row.get(0),
            )
            .unwrap();

        assert!(matches!(
            database.apply_scope_root_revocation(binding, 2),
            Err(DatabaseError::ScopePrivacyPurgeBlocked)
        ));
        assert_eq!(
            database
                .current_scope_policy_revision(scope_id)
                .unwrap()
                .revision,
            binding.revision
        );
        assert!(
            database
                .scope_has_active_access_grant(scope_id)
                .expect("failed revocation must leave the runtime grant durable")
        );
        assert_eq!(
            database
                .connection
                .query_row(
                    "SELECT COUNT(*) FROM locations WHERE scope_id=?1",
                    [scope_id],
                    |row| row.get::<_, i64>(0),
                )
                .unwrap(),
            location_count
        );
        assert_eq!(
            database
                .connection
                .query_row(
                    "SELECT COUNT(*) FROM scope_root_revocation_receipts",
                    [],
                    |row| { row.get::<_, i64>(0) }
                )
                .unwrap(),
            0
        );
        assert_eq!(
            database
                .connection
                .query_row(
                    "SELECT COUNT(*) FROM privacy_purge_capabilities",
                    [],
                    |row| { row.get::<_, i64>(0) }
                )
                .unwrap(),
            0
        );
    }

    #[test]
    fn privacy_purge_target_capability_cannot_delete_a_non_target_action() {
        let (mut database, scope_id, node_id, _) = extraction_setup();
        let first = create_bound_rename_preview(&mut database, scope_id, node_id);
        let second = create_bound_rename_preview(&mut database, scope_id, node_id);
        let transaction = database
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .unwrap();
        transaction.execute(
            "INSERT INTO privacy_purge_capabilities(nonce,scope_id,from_revision,to_revision,created_at_unix_ms) \
             VALUES(zeroblob(32),?1,1,2,1)",
            [scope_id],
        ).unwrap();
        transaction
            .execute(
                "UPDATE authorized_scopes SET policy_revision=2 WHERE id=?1 AND policy_revision=1",
                [scope_id],
            )
            .unwrap();
        transaction.execute(
            "INSERT INTO privacy_purge_action_plan_targets(nonce,plan_id) VALUES(zeroblob(32),?1)",
            [first.plan_id],
        ).unwrap();
        assert!(
            transaction
                .execute(
                    "DELETE FROM action_journal_events WHERE plan_id=?1",
                    [second.plan_id],
                )
                .is_err()
        );
        assert!(
            transaction
                .execute("DELETE FROM action_plans WHERE id=?1", [second.plan_id])
                .is_err()
        );
        transaction.rollback().unwrap();
        assert_eq!(
            database
                .connection
                .query_row(
                    "SELECT COUNT(*) FROM action_plans WHERE id IN (?1,?2)",
                    params![first.plan_id, second.plan_id],
                    |row| row.get::<_, i64>(0)
                )
                .unwrap(),
            2
        );
    }

    #[test]
    fn action_and_cleanup_plan_insert_triggers_reject_platform_mismatch() {
        let (mut action_database, scope_id, node_id, _) = extraction_setup();
        let action = create_bound_rename_preview(&mut action_database, scope_id, node_id);
        action_database
            .connection
            .execute(
                "UPDATE authorized_scopes SET platform=?2 WHERE id=?1",
                params![scope_id, foreign_platform()],
            )
            .unwrap();
        assert!(
            clone_table_row_without_id(&action_database.connection, "action_plans", action.plan_id)
                .is_err()
        );

        let (mut cleanup_database, selection, source, keeper) = cleanup_exact_duplicate_setup();
        let sha256 = [9_u8; 32];
        let cleanup = cleanup_database
            .create_cleanup_action_plan(cleanup_exact_duplicate_plan_write(
                selection, &source, &keeper, &sha256, &sha256,
            ))
            .expect("cleanup preview should persist before mismatch");
        cleanup_database
            .connection
            .execute(
                "UPDATE authorized_scopes SET platform=?2 WHERE id=?1",
                params![selection.scope_id, foreign_platform()],
            )
            .unwrap();
        assert!(
            clone_table_row_without_id(
                &cleanup_database.connection,
                "cleanup_action_plans",
                cleanup.plan_id,
            )
            .is_err()
        );
    }

    #[test]
    fn stale_direct_scan_extraction_and_watch_inserts_and_updates_are_trigger_rejected() {
        let (mut database, scope_id, node_id, _) = extraction_setup();
        let binding = test_active_binding(&database, scope_id).expect("active binding should load");
        let location_id: i64 = database
            .connection
            .query_row(
                "SELECT id FROM locations WHERE scope_id=?1 AND node_id=?2",
                params![scope_id, node_id],
                |row| row.get(0),
            )
            .expect("location should load");
        database
            .connection
            .execute(
                "INSERT INTO extraction_jobs(scope_id,node_id,location_id,status,source_size_bytes, \
                    created_at_unix_ms,updated_at_unix_ms,policy_revision) \
                 VALUES(?1,?2,?3,'completed',4,0,1,1)",
                params![scope_id, node_id, location_id],
            )
            .expect("old-revision extraction fixture should persist");
        let extraction_id = database.connection.last_insert_rowid();
        database
            .connection
            .execute(
                "INSERT INTO watch_events(scope_id,status,path_raw,path_key,observed_kind,observed_size_bytes, \
                    observed_modified_unix_ns,observed_identity_key,observation_count,stable_after_unix_ms, \
                    created_at_unix_ms,updated_at_unix_ms,policy_revision) \
                 VALUES(?1,'completed',X'2F73636F70652F66696C652E747874','/scope/file.txt','file',4,1, \
                    X'7761746368',1,1,0,1,1)",
                [scope_id],
            )
            .expect("old-revision watch fixture should persist");
        let watch_id = database.connection.last_insert_rowid();
        let scan_id: i64 = database
            .connection
            .query_row(
                "SELECT id FROM scan_jobs WHERE scope_id=?1 AND status='completed' ORDER BY id DESC LIMIT 1",
                [scope_id],
                |row| row.get(0),
            )
            .expect("completed scan should load");

        let write = ScopeExclusionWrite {
            kind: ScopeExclusionKind::Folder,
            path_raw: b"/scope/unused-private",
            path_key: "/scope/unused-private",
            display_path: "/scope/unused-private",
            identity_kind: TEST_EXCLUDED_IDENTITY_KIND,
            identity_key: TEST_EXCLUDED_FOLDER_IDENTITY,
        };
        database
            .apply_scope_exclusion_batch(binding, &[write], 2)
            .expect("revision-only exclusion should commit");

        for result in [
            database.connection.execute(
                "INSERT INTO scan_jobs(scope_id,status,started_at_unix_ms) VALUES(?1,'completed',2)",
                [scope_id],
            ),
            database.connection.execute(
                "UPDATE scan_jobs SET issue_count=issue_count+1 WHERE id=?1",
                [scan_id],
            ),
            database.connection.execute(
                "INSERT INTO extraction_jobs(scope_id,node_id,location_id,status,source_size_bytes,created_at_unix_ms,updated_at_unix_ms) \
                 VALUES(?1,?2,?3,'failed',4,2,2)",
                params![scope_id, node_id, location_id],
            ),
            database.connection.execute(
                "UPDATE extraction_jobs SET error_code='stale-write' WHERE id=?1",
                [extraction_id],
            ),
            database.connection.execute(
                "INSERT INTO watch_events(scope_id,status,path_raw,path_key,observed_kind,observed_size_bytes, \
                    observed_modified_unix_ns,observed_identity_key,observation_count,stable_after_unix_ms,created_at_unix_ms,updated_at_unix_ms) \
                 VALUES(?1,'completed',X'2F73636F70652F6E65772E747874','/scope/new.txt','file',1,1,X'6E6577',1,2,2,2)",
                [scope_id],
            ),
            database.connection.execute(
                "UPDATE watch_events SET observation_count=observation_count+1 WHERE id=?1",
                [watch_id],
            ),
        ] {
            let error = result.expect_err("stale direct write must be rejected by a DB trigger");
            assert!(
                error.to_string().contains("scope_policy_revision_stale"),
                "unexpected trigger error: {error}"
            );
        }
        assert_eq!(
            database
                .connection
                .query_row(
                    "SELECT issue_count FROM scan_jobs WHERE id=?1",
                    [scan_id],
                    |row| row.get::<_, i64>(0)
                )
                .unwrap(),
            0
        );
        assert_eq!(
            database
                .connection
                .query_row(
                    "SELECT error_code FROM extraction_jobs WHERE id=?1",
                    [extraction_id],
                    |row| row.get::<_, Option<String>>(0)
                )
                .unwrap(),
            None
        );
        assert_eq!(
            database
                .connection
                .query_row(
                    "SELECT observation_count FROM watch_events WHERE id=?1",
                    [watch_id],
                    |row| row.get::<_, i64>(0)
                )
                .unwrap(),
            1
        );
    }

    #[test]
    fn hard_exclusion_keeps_allowed_sibling_metadata_and_content_searchable() {
        let mut database = ManifestDatabase::open_in_memory().expect("database should initialize");
        let scope = database
            .add_scope_with_access_grant(
                b"/scope",
                "/scope",
                "/scope",
                ScopeAccessGrantWrite {
                    scope_platform: std::env::consts::OS,
                    grant_platform: std::env::consts::OS,
                    opaque_grant: b"sibling-search-grant",
                    state: ScopeAccessGrantState::Active,
                },
            )
            .expect("scope and grant should persist");
        let scan_id = database
            .create_scan_job_with_policy(
                database.bind_core_scope_policy_revision(scope.id).unwrap(),
            )
            .expect("scan should start");
        let root = observation("/scope", NodeKind::Folder, None);
        let private = observation(
            "/scope/private",
            NodeKind::Folder,
            Some(root.identity_key.clone()),
        );
        let secret = observation(
            "/scope/private/secret.txt",
            NodeKind::File,
            Some(private.identity_key.clone()),
        );
        let allowed = observation(
            "/scope/public-allowed.txt",
            NodeKind::File,
            Some(root.identity_key.clone()),
        );
        database
            .complete_scan(
                scan_id,
                scope.id,
                &[root, private, secret, allowed],
                &[],
                0,
                0,
            )
            .expect("manifest should publish");
        for (path, text) in [
            ("/scope/private/secret.txt", "secret private body"),
            ("/scope/public-allowed.txt", "allowed sibling body"),
        ] {
            let (node_id, location_id): (i64, i64) = database
                .connection
                .query_row(
                    "SELECT node_id,id FROM locations WHERE scope_id=?1 AND path_key=?2",
                    params![scope.id, path],
                    |row| Ok((row.get(0)?, row.get(1)?)),
                )
                .expect("file location should load");
            database
                .connection
                .execute(
                    "INSERT INTO extraction_jobs(scope_id,node_id,location_id,status,source_size_bytes, \
                        created_at_unix_ms,finished_at_unix_ms,updated_at_unix_ms,policy_revision) \
                     VALUES(?1,?2,?3,'completed',4,0,1,1,1)",
                    params![scope.id, node_id, location_id],
                )
                .expect("content job should persist");
            let extraction_id = database.connection.last_insert_rowid();
            database
                .connection
                .execute(
                    "INSERT INTO content_chunks(scope_id,node_id,location_id,extraction_job_id,ordinal,text, \
                        provenance_kind,source_byte_start,source_byte_end,source_size_bytes,source_modified_unix_ns, \
                        trust_class,provider_id,provider_version,active,created_at_unix_ms) \
                     VALUES(?1,?2,?3,?4,0,?5,'byte_range',0,4,4,1,'untrusted_extracted_text','test','1',1,1)",
                    params![scope.id, node_id, location_id, extraction_id, text],
                )
                .expect("content chunk should persist");
        }

        let binding = database.bind_scope_policy_revision(scope.id).unwrap();
        let write = ScopeExclusionWrite {
            kind: ScopeExclusionKind::Folder,
            path_raw: b"/scope/private",
            path_key: "/scope/private",
            display_path: "/scope/private",
            identity_kind: TEST_EXCLUDED_IDENTITY_KIND,
            identity_key: TEST_EXCLUDED_FOLDER_IDENTITY,
        };
        database
            .apply_scope_exclusion_batch(binding, &[write], 2)
            .expect("private subtree should purge");
        ensure_scope_queryable(&database.connection, scope.id)
            .expect("an atomically pruned allowed sibling manifest should remain queryable");
        assert!(
            database
                .scope_has_completed_scan(scope.id)
                .expect("initial scan readiness should remain")
        );
        assert_eq!(
            database
                .watchable_scope_ids_with_active_access_grants()
                .expect("allowed sibling scope should remain watchable"),
            vec![scope.id]
        );
        let allowed_results = database
            .lexical_search_candidates("allowed", lexical_filters(Some(scope.id)), 10)
            .expect("allowed sibling should remain queryable");
        assert_eq!(allowed_results.len(), 2);
        assert!(
            allowed_results
                .iter()
                .all(|candidate| candidate.path_key == "/scope/public-allowed.txt")
        );
        assert!(
            allowed_results
                .iter()
                .any(|candidate| candidate.source == LexicalCandidateSource::MetadataPath)
        );
        assert!(
            allowed_results
                .iter()
                .any(|candidate| candidate.source == LexicalCandidateSource::ExtractedText)
        );
        assert!(
            database
                .lexical_search_candidates("secret", lexical_filters(Some(scope.id)), 10)
                .expect("excluded search should fail closed")
                .is_empty()
        );
    }

    #[test]
    fn execute_requested_action_blocks_privacy_purge_with_full_rollback() {
        let (mut database, scope_id, node_id, _) = extraction_setup();
        let preview = create_bound_rename_preview(&mut database, scope_id, node_id);
        database
            .start_action_command(ActionCommandWrite {
                plan_id: preview.plan_id,
                request_id: "purge_execute_requested",
                kind: ActionCommandKind::Execute,
                expected_sequence: 1,
            })
            .expect("execute request should persist");
        assert_action_safety_record_blocks_privacy_purge(&mut database, scope_id, preview.plan_id);
    }

    #[test]
    fn execution_needs_attention_blocks_privacy_purge_with_full_rollback() {
        let (mut database, scope_id, node_id, _) = extraction_setup();
        let preview = create_bound_rename_preview(&mut database, scope_id, node_id);
        let execute = database
            .start_action_command(ActionCommandWrite {
                plan_id: preview.plan_id,
                request_id: "purge_execution_attention",
                kind: ActionCommandKind::Execute,
                expected_sequence: 1,
            })
            .expect("execute request should persist");
        acquire_test_executor_lease(&mut database, preview.plan_id);
        let intent = database
            .append_action_journal_event(ActionJournalAppend {
                plan_id: preview.plan_id,
                command_request_id: execute.command_request_id,
                expected_sequence: execute.journal_sequence,
                expected_state: ActionPlanState::ExecuteRequested,
                kind: ActionJournalEventKind::DirectRenameIntent,
                executor_lease_owner_token: "test_executor_0001",
            })
            .expect("direct intent should persist");
        database
            .append_action_journal_event(ActionJournalAppend {
                plan_id: preview.plan_id,
                command_request_id: execute.command_request_id,
                expected_sequence: intent.journal_sequence,
                expected_state: ActionPlanState::DirectRenameIntent,
                kind: ActionJournalEventKind::ExecutionNeedsAttention,
                executor_lease_owner_token: "test_executor_0001",
            })
            .expect("execution attention receipt should persist");
        assert_action_safety_record_blocks_privacy_purge(&mut database, scope_id, preview.plan_id);
    }

    #[test]
    fn undo_needs_attention_blocks_privacy_purge_with_full_rollback() {
        let (mut database, scope_id, node_id, _) = extraction_setup();
        let preview = create_bound_rename_preview(&mut database, scope_id, node_id);
        let execute = database
            .start_action_command(ActionCommandWrite {
                plan_id: preview.plan_id,
                request_id: "purge_undo_execute",
                kind: ActionCommandKind::Execute,
                expected_sequence: 1,
            })
            .expect("execute request should persist");
        acquire_test_executor_lease(&mut database, preview.plan_id);
        let intent = database
            .append_action_journal_event(ActionJournalAppend {
                plan_id: preview.plan_id,
                command_request_id: execute.command_request_id,
                expected_sequence: execute.journal_sequence,
                expected_state: ActionPlanState::ExecuteRequested,
                kind: ActionJournalEventKind::DirectRenameIntent,
                executor_lease_owner_token: "test_executor_0001",
            })
            .expect("direct intent should persist");
        let executed = database
            .append_action_journal_event(ActionJournalAppend {
                plan_id: preview.plan_id,
                command_request_id: execute.command_request_id,
                expected_sequence: intent.journal_sequence,
                expected_state: ActionPlanState::DirectRenameIntent,
                kind: ActionJournalEventKind::ExecutionCompleted,
                executor_lease_owner_token: "test_executor_0001",
            })
            .expect("execution receipt should persist");
        let undo = database
            .start_action_command(ActionCommandWrite {
                plan_id: preview.plan_id,
                request_id: "purge_undo_attention",
                kind: ActionCommandKind::Undo,
                expected_sequence: executed.journal_sequence,
            })
            .expect("undo request should persist");
        let undo_intent = database
            .append_action_journal_event(ActionJournalAppend {
                plan_id: preview.plan_id,
                command_request_id: undo.command_request_id,
                expected_sequence: undo.journal_sequence,
                expected_state: ActionPlanState::UndoRequested,
                kind: ActionJournalEventKind::UndoRenameIntent,
                executor_lease_owner_token: "test_executor_0001",
            })
            .expect("undo intent should persist");
        database
            .append_action_journal_event(ActionJournalAppend {
                plan_id: preview.plan_id,
                command_request_id: undo.command_request_id,
                expected_sequence: undo_intent.journal_sequence,
                expected_state: ActionPlanState::UndoRenameIntent,
                kind: ActionJournalEventKind::UndoNeedsAttention,
                executor_lease_owner_token: "test_executor_0001",
            })
            .expect("undo attention receipt should persist");
        assert_action_safety_record_blocks_privacy_purge(&mut database, scope_id, preview.plan_id);
    }

    #[test]
    fn excluding_one_screenshot_member_closes_the_group_and_its_cleanup_source_only() {
        let (mut database, scope_id, _) = screenshot_group_setup();
        let scan_id: i64 = database
            .connection
            .query_row(
                "SELECT MAX(id) FROM scan_jobs WHERE scope_id=?1 AND status='completed'",
                [scope_id],
                |row| row.get(0),
            )
            .unwrap();
        insert_screenshot_group_source(&database, scope_id, scan_id, 2, "1");
        let candidate = database
            .discover_screenshot_group_candidates(scope_id)
            .expect("three-member screenshot group should persist")
            .1
            .remove(0);
        assert_eq!(candidate.members.len(), 3);
        let target = &candidate.members[1];
        let (identity_kind, identity_key, size_bytes, modified_unix_ns): (
            String,
            Vec<u8>,
            i64,
            Option<i64>,
        ) = database
            .connection
            .query_row(
                "SELECT n.identity_kind,n.identity_key,f.size_bytes,f.modified_unix_ns \
                 FROM nodes n JOIN files f ON f.node_id=n.id WHERE n.id=?1",
                [target.node_id],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
            )
            .unwrap();
        database.connection.execute(
            "INSERT INTO cleanup_action_plans( \
                 api_version,policy_version,operation,state,scope_id,source_kind,source_id,source_observation_id, \
                 target_node_id,target_location_id,target_identity_kind,target_identity_key,target_size_bytes, \
                 target_modified_unix_ns,target_sha256,target_hash_bytes,scope_root_node_id,scope_root_identity_kind, \
                 scope_root_identity_key,parent_node_id,parent_identity_kind,parent_identity_key,confirmation_required, \
                 action_authorized,execution_available,created_at_unix_ms,policy_revision \
             ) VALUES('deskgraph.cleanup-action-plan.v1','deskgraph.cleanup-action-policy.v1', \
                 'system_trash_preview','previewed',?1,'screenshot_review_group',?2,?3,?4,?5,?6,?7,?8,?9, \
                 zeroblob(32),?8,?4,?6,?7,?4,?6,?7,1,0,0,1,1)",
            params![
                scope_id,
                candidate.group_id,
                candidate.evidence.observation_id,
                target.node_id,
                target.location_id,
                identity_kind,
                identity_key,
                size_bytes,
                modified_unix_ns,
            ],
        ).expect("screenshot cleanup preview should persist");
        let cleanup_plan_id = database.connection.last_insert_rowid();
        database.connection.execute(
            "INSERT INTO cleanup_action_journal_events(api_version,plan_id,sequence,event_kind,created_at_unix_ms) \
             VALUES('deskgraph.cleanup-action-journal.v1',?1,1,'preview_created',1)",
            [cleanup_plan_id],
        ).unwrap();

        let binding = database.bind_scope_policy_revision(scope_id).unwrap();
        let write = ScopeExclusionWrite {
            kind: ScopeExclusionKind::File,
            path_raw: b"/scope/screenshot-0.png",
            path_key: "/scope/screenshot-0.png",
            display_path: "/scope/screenshot-0.png",
            identity_kind: TEST_EXCLUDED_IDENTITY_KIND,
            identity_key: TEST_EXCLUDED_FILE_IDENTITY,
        };
        let impact = database
            .preview_scope_exclusion_batch(binding, &[write])
            .unwrap();
        assert_eq!(impact.screenshot_group_count, 1);
        assert_eq!(impact.cleanup_action_plan_count, 1);
        database
            .apply_scope_exclusion_batch(binding, &[write], 2)
            .unwrap();

        for table in [
            "screenshot_group_members",
            "screenshot_group_observations",
            "screenshot_group_candidates",
            "cleanup_action_journal_events",
            "cleanup_action_plans",
        ] {
            let sql = format!("SELECT COUNT(*) FROM {table}");
            assert_eq!(
                database
                    .connection
                    .query_row(&sql, [], |row| row.get::<_, i64>(0))
                    .unwrap(),
                0,
                "{table} should be purged as one closed source"
            );
        }
        assert_eq!(database.connection.query_row(
            "SELECT COUNT(*) FROM locations WHERE scope_id=?1 AND path_key IN('/scope/screenshot-1.png','/scope/screenshot-2.png')",
            [scope_id],
            |row| row.get::<_,i64>(0),
        ).unwrap(), 2);
        assert_eq!(database.connection.query_row(
            "SELECT COUNT(*) FROM content_chunks c JOIN locations l ON l.id=c.location_id \
             WHERE l.scope_id=?1 AND l.path_key IN('/scope/screenshot-1.png','/scope/screenshot-2.png')",
            [scope_id],
            |row| row.get::<_,i64>(0),
        ).unwrap(), 2);
    }

    #[test]
    fn project_marker_and_action_destination_are_closed_over_privacy_targets() {
        let (mut database, scope_id, node_id, _) = extraction_setup();
        let preview = create_bound_rename_preview(&mut database, scope_id, node_id);
        let root_node_id = database
            .node_id_for_path_key(scope_id, "/scope")
            .expect("root lookup should pass")
            .expect("root should exist");
        let scan_id: i64 = database
            .connection
            .query_row(
                "SELECT id FROM scan_jobs WHERE scope_id=?1 AND status='completed' ORDER BY id DESC LIMIT 1",
                [scope_id],
                |row| row.get(0),
            )
            .expect("completed scan should load");
        database
            .connection
            .execute(
                "INSERT INTO nodes(kind,identity_kind,identity_key,created_at_unix_ms,updated_at_unix_ms) \
                 VALUES('file','test',X'70726F6A6563742D6D61726B6572',1,1)",
                [],
            )
            .expect("project marker node should persist");
        let marker_node_id = database.connection.last_insert_rowid();
        database
            .connection
            .execute(
                "INSERT INTO files(node_id,size_bytes,modified_unix_ns,link_count) VALUES(?1,4,1,1)",
                [marker_node_id],
            )
            .expect("marker file facts should persist");
        database
            .connection
            .execute(
                "INSERT INTO locations(scope_id,node_id,path_raw,path_key,display_path,present,last_seen_scan_id) \
                 VALUES(?1,?2,X'2F73636F70652F436172676F2E746F6D6C','/scope/Cargo.toml','/scope/Cargo.toml',1,?3)",
                params![scope_id, marker_node_id, scan_id],
            )
            .expect("marker location should persist");
        database
            .connection
            .execute(
                "INSERT INTO edges(scope_id,source_node_id,target_node_id,kind,active,last_seen_scan_id) \
                 VALUES(?1,?2,?3,'located_in',1,?4)",
                params![scope_id, marker_node_id, root_node_id, scan_id],
            )
            .expect("marker topology should persist");
        database
            .connection
            .execute(
                "INSERT INTO projects(api_version,scope_id,root_folder_node_id,created_at_unix_ms) \
                 VALUES('deskgraph.project-candidate.v1',?1,?2,1)",
                params![scope_id, root_node_id],
            )
            .expect("project should persist");
        let project_id = database.connection.last_insert_rowid();

        let binding = database.bind_scope_policy_revision(scope_id).unwrap();
        let writes = [
            ScopeExclusionWrite {
                kind: ScopeExclusionKind::File,
                path_raw: b"/scope/Cargo.toml",
                path_key: "/scope/Cargo.toml",
                display_path: "/scope/Cargo.toml",
                identity_kind: TEST_EXCLUDED_IDENTITY_KIND,
                identity_key: TEST_EXCLUDED_FILE_IDENTITY,
            },
            ScopeExclusionWrite {
                kind: ScopeExclusionKind::File,
                path_raw: b"/scope/renamed.txt",
                path_key: "/scope/renamed.txt",
                display_path: "/scope/renamed.txt",
                identity_kind: TEST_EXCLUDED_IDENTITY_KIND,
                identity_key: TEST_EXCLUDED_FILE_IDENTITY_2,
            },
        ];
        let impact = database
            .preview_scope_exclusion_batch(binding, &writes)
            .expect("closure impact should preview");
        assert_eq!(impact.project_count, 1);
        assert_eq!(impact.action_plan_count, 1);
        assert_eq!(impact.blocking_action_count, 0);
        database
            .apply_scope_exclusion_batch(binding, &writes, 10)
            .expect("pristine preview and project marker should purge");
        assert_eq!(
            database
                .connection
                .query_row(
                    "SELECT COUNT(*) FROM projects WHERE id=?1",
                    [project_id],
                    |row| row.get::<_, i64>(0)
                )
                .unwrap(),
            0
        );
        assert_eq!(
            database
                .connection
                .query_row(
                    "SELECT COUNT(*) FROM action_plans WHERE id=?1",
                    [preview.plan_id],
                    |row| row.get::<_, i64>(0)
                )
                .unwrap(),
            0
        );
        assert_eq!(
            database.connection.query_row(
                "SELECT COUNT(*) FROM locations WHERE scope_id=?1 AND path_key='/scope/file.txt'",
                [scope_id],
                |row| row.get::<_, i64>(0)
            ).unwrap(),
            1,
            "destination-only closure must not purge the action source"
        );
    }

    #[cfg(any(target_os = "macos", target_os = "linux"))]
    #[test]
    fn hard_exclusion_purges_fts_and_same_scope_hardlinks_but_keeps_source_and_cross_scope_node() {
        use std::os::unix::ffi::OsStrExt;

        let temp = tempfile::tempdir().expect("tempdir should create");
        let root = temp.path().join("scope");
        let private = root.join("private");
        std::fs::create_dir_all(&private).expect("private directory should create");
        let source = private.join("secret.txt");
        std::fs::write(&source, b"secret-source-bytes").expect("source should create");
        let bytes_before = std::fs::read(&source).expect("source should read");
        let modified_before = std::fs::metadata(&source)
            .expect("metadata should load")
            .modified()
            .expect("mtime should load");
        let root_key = comparison_key(&root);
        let private_key = comparison_key(&private);
        let source_key = comparison_key(&source);
        let hardlink_key = comparison_key(&root.join("public-hardlink.txt"));

        let mut database = ManifestDatabase::open_in_memory().expect("database should initialize");
        let scope = database
            .add_scope_with_access_grant(
                root.as_os_str().as_bytes(),
                &root_key,
                &root.to_string_lossy(),
                ScopeAccessGrantWrite {
                    scope_platform: std::env::consts::OS,
                    grant_platform: std::env::consts::OS,
                    opaque_grant: b"scope-grant",
                    state: ScopeAccessGrantState::Active,
                },
            )
            .expect("scope should persist");
        let outside = temp.path().join("outside");
        std::fs::create_dir_all(&outside).expect("outside should create");
        let outside_key = comparison_key(&outside);
        let outside_scope = database
            .add_scope_with_access_grant(
                outside.as_os_str().as_bytes(),
                &outside_key,
                &outside.to_string_lossy(),
                ScopeAccessGrantWrite {
                    scope_platform: std::env::consts::OS,
                    grant_platform: std::env::consts::OS,
                    opaque_grant: b"outside-grant",
                    state: ScopeAccessGrantState::Active,
                },
            )
            .expect("outside scope should persist");
        database.connection.execute(
            "INSERT INTO scan_jobs(scope_id,status,started_at_unix_ms,finished_at_unix_ms,policy_revision) VALUES(?1,'completed',1,2,1)",
            [scope.id],
        ).expect("scope scan should insert");
        let scan_id = database.connection.last_insert_rowid();
        database.connection.execute(
            "INSERT INTO scan_jobs(scope_id,status,started_at_unix_ms,finished_at_unix_ms,policy_revision) VALUES(?1,'completed',1,2,1)",
            [outside_scope.id],
        ).expect("outside scan should insert");
        let outside_scan_id = database.connection.last_insert_rowid();
        database.connection.execute(
            "INSERT INTO nodes(kind,identity_kind,identity_key,created_at_unix_ms,updated_at_unix_ms) VALUES('file','unix-dev-inode',x'0102',1,1)",
            [],
        ).expect("node should insert");
        let node_id = database.connection.last_insert_rowid();
        database.connection.execute(
            "INSERT INTO files(node_id,size_bytes,modified_unix_ns,link_count) VALUES(?1,19,1,3)",
            [node_id],
        ).expect("file should insert");
        for (scope_id, job_id, raw, key, display) in [
            (
                scope.id,
                scan_id,
                source.as_os_str().as_bytes(),
                source_key.as_str(),
                source.to_string_lossy().into_owned(),
            ),
            (
                scope.id,
                scan_id,
                root.join("public-hardlink.txt").as_os_str().as_bytes(),
                hardlink_key.as_str(),
                root.join("public-hardlink.txt")
                    .to_string_lossy()
                    .into_owned(),
            ),
            (
                outside_scope.id,
                outside_scan_id,
                outside.join("same-node.txt").as_os_str().as_bytes(),
                comparison_key(&outside.join("same-node.txt")).as_str(),
                outside.join("same-node.txt").to_string_lossy().into_owned(),
            ),
        ] {
            database.connection.execute(
                "INSERT INTO locations(scope_id,node_id,path_raw,path_key,display_path,present,last_seen_scan_id) VALUES(?1,?2,?3,?4,?5,1,?6)",
                params![scope_id,node_id,raw,key,display,job_id],
            ).expect("location should insert");
        }
        let source_location: i64 = database
            .connection
            .query_row(
                "SELECT id FROM locations WHERE scope_id=?1 AND path_key=?2",
                params![scope.id, source_key],
                |row| row.get(0),
            )
            .expect("source location should load");
        database.connection.execute(
            "INSERT INTO extraction_jobs(scope_id,node_id,location_id,status,source_size_bytes,output_bytes,chunk_count,elapsed_ms,created_at_unix_ms,finished_at_unix_ms,updated_at_unix_ms,policy_revision,operation) VALUES(?1,?2,?3,'completed',19,19,1,1,1,2,2,1,'content')",
            params![scope.id,node_id,source_location],
        ).expect("extraction job should insert");
        let extraction_id = database.connection.last_insert_rowid();
        database.connection.execute(
            "INSERT INTO content_chunks(scope_id,node_id,location_id,extraction_job_id,ordinal,text,provenance_kind,source_byte_start,source_byte_end,source_size_bytes,trust_class,provider_id,provider_version,active,created_at_unix_ms) VALUES(?1,?2,?3,?4,0,'secret searchable text','byte_range',0,19,19,'untrusted_extracted_text','test','1',1,2)",
            params![scope.id,node_id,source_location,extraction_id],
        ).expect("content should insert");

        let private_metadata = std::fs::symlink_metadata(&private).expect("private metadata");
        let private_identity =
            platform_identity(&private, &private_metadata, IdentityNodeKind::Folder)
                .expect("private folder must have stable identity");

        let write = ScopeExclusionWrite {
            kind: ScopeExclusionKind::Folder,
            path_raw: private.as_os_str().as_bytes(),
            path_key: &private_key,
            display_path: &private.to_string_lossy(),
            identity_kind: private_identity.kind,
            identity_key: &private_identity.key,
        };
        let binding = database
            .bind_scope_policy_revision(scope.id)
            .expect("binding should load");
        let preview = database
            .preview_scope_exclusion_batch(binding, &[write])
            .expect("preview should succeed");
        assert_eq!(preview.direct_location_count, 1);
        assert_eq!(preview.conservative_location_count, 2);
        let result = database
            .apply_scope_exclusion_batch(binding, &[write], 10)
            .expect("privacy purge should commit");
        assert_eq!(result.policy.revision, 2);
        assert_eq!(result.exclusions.len(), 1);
        assert_eq!(
            database
                .connection
                .query_row(
                    "SELECT COUNT(*) FROM locations WHERE scope_id=?1",
                    [scope.id],
                    |row| row.get::<_, i64>(0)
                )
                .unwrap(),
            0
        );
        assert_eq!(
            database
                .connection
                .query_row(
                    "SELECT COUNT(*) FROM locations WHERE scope_id=?1 AND node_id=?2",
                    params![outside_scope.id, node_id],
                    |row| row.get::<_, i64>(0)
                )
                .unwrap(),
            1
        );
        assert_eq!(
            database
                .connection
                .query_row("SELECT COUNT(*) FROM nodes WHERE id=?1", [node_id], |row| {
                    row.get::<_, i64>(0)
                })
                .unwrap(),
            1
        );
        assert_eq!(database.connection.query_row("SELECT COUNT(*) FROM content_search_fts WHERE content_search_fts MATCH 'secret'", [], |row| row.get::<_,i64>(0)).unwrap(), 0);
        assert_eq!(database.connection.query_row("SELECT COUNT(*) FROM location_search_fts WHERE location_search_fts MATCH 'secret'", [], |row| row.get::<_,i64>(0)).unwrap(), 0);
        assert_eq!(
            std::fs::read(&source).expect("source should remain"),
            bytes_before
        );
        assert_eq!(
            std::fs::metadata(&source).unwrap().modified().unwrap(),
            modified_before
        );
        assert!(matches!(
            database.apply_scope_exclusion_batch(binding, &[write], 11),
            Err(DatabaseError::ScopePolicyRevisionStale)
        ));
        assert_eq!(
            database
                .connection
                .query_row(
                    "SELECT COUNT(*) FROM privacy_purge_capabilities",
                    [],
                    |row| row.get::<_, i64>(0)
                )
                .unwrap(),
            0
        );
    }
}
