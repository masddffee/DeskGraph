mod scope_access;

use std::collections::{HashMap, HashSet, VecDeque};
use std::path::{Path, PathBuf};
use std::sync::{
    Arc, Mutex, MutexGuard, TryLockError,
    atomic::{AtomicBool, AtomicU64, Ordering},
    mpsc::{Receiver, RecvTimeoutError, SyncSender, TrySendError, sync_channel},
};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use deskgraph_database::{
    CleanupActionSelection, ManifestDatabase, ScopeExclusionImpactPreview, ScopeExclusionKind,
    ScopeExclusionWrite, ScopeRootRevocationPreview,
};
use deskgraph_domain::{
    ActionPlanPreview, ActionPlanSummary, AuthorizedScope, CleanupActionPlanPreview,
    CleanupSourceDetail, ExtractionJobProgress, ExtractionOperation, ExtractionStats, HealthReport,
    ManifestStats, ProjectCandidateDetail, ProjectDecisionKind, ProjectDiscovery, ScanJobProgress,
    ScanStatus, SearchFilters, SearchFolderListResponse, SearchResponse, SmartCleanupInbox,
    SmartCleanupSourceKind, WatchEventProgress, collect_health_with_manifest,
};
#[cfg(all(test, target_os = "macos"))]
use deskgraph_domain::{WatchEventReason, WatchEventStatus};
#[cfg(test)]
use deskgraph_extractors::extraction_stats_at as read_extraction_stats_at;
use deskgraph_extractors::{
    ExtractionLimits, MediaKind, cancel_extraction_job_at, create_extraction_job_at,
    create_screenshot_ocr_job_at, extraction_job_at, media_kind_for_extension,
    recent_extraction_jobs_at as read_recent_extraction_jobs_at, resume_extraction_job_at,
    run_extraction_job_at,
};
use deskgraph_projects::{
    cleanup_source_detail_at, decide_current_project_candidate_at, discover_projects_at,
    project_candidate_detail_at, refresh_smart_cleanup_inbox_at,
};
use deskgraph_retrieval::{SearchRequest, SearchSourceFilter, search_at as run_search_at};
use deskgraph_scanner::{
    CoverageRootAuthorizationRequest, MAX_COVERAGE_ROOTS_PER_SELECTION, ScopeExclusionSelection,
    authorize_coverage_roots_with_access_grants, comparison_key, create_scan_job_with_active_grant,
    pause_scan_job, prepare_scope_exclusion_batch_with_active_grant,
    prepare_scope_exclusion_batch_with_revocation_fence, resume_scan_job_with_active_grant,
    run_scan_job_batch_with_active_grant, validated_scope_root,
};
#[cfg(test)]
use deskgraph_scanner::{authorize_scope_with_access_grant, run_scan_job_to_terminal};
use deskgraph_telemetry::{Service, init_privacy_safe_logging};
use deskgraph_transactions::{
    create_cleanup_preview_at, create_rename_preview_at, recent_action_plans_at,
};
use deskgraph_watcher::{
    NativeWatchCallbackRetirement, NativeWatchEventSource, NativeWatchSynchronizationBarrier,
    PollingWatchPolicy, WatchCoordinator, WatchPolicy,
    recent_watch_events_at as read_recent_watch_events_at,
};
use scope_access::{ActiveScopeAccess, prepare_selected_scope, restore_scope_access};
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager, State};
use tauri_plugin_dialog::DialogExt;
use tracing::{error, info};

const WATCH_RUNTIME_SHUTDOWN_TIMEOUT: Duration = Duration::from_secs(2);
const WATCH_RUNTIME_SHUTDOWN_POLL: Duration = Duration::from_millis(10);
const WATCH_NATIVE_RETRY_INTERVAL: Duration = Duration::from_secs(30);
const WATCH_GATE_RETRY_INTERVAL: Duration = Duration::from_millis(50);
const WATCH_NATIVE_SYNCHRONIZATION_TIMEOUT: Duration = Duration::from_secs(2);
const FOREGROUND_SCAN_BATCH_SIZE: usize = 256;
const WATCH_ADAPTER_NATIVE: &str = "native_with_periodic_reconciliation";
const WATCH_ADAPTER_PERIODIC_ONLY: &str = "periodic_reconciliation_only";
const MAX_PENDING_HARD_EXCLUSION_PREVIEWS: usize = 16;
const HARD_EXCLUSION_PREVIEW_TTL_MS: i64 = 5 * 60 * 1_000;
static HARD_EXCLUSION_PREVIEW_SEQUENCE: AtomicU64 = AtomicU64::new(1);
const MAX_PENDING_SCOPE_ROOT_REVOCATION_PREVIEWS: usize = 16;
const SCOPE_ROOT_REVOCATION_PREVIEW_TTL_MS: i64 = 5 * 60 * 1_000;
const SEARCH_FOLDER_LIST_LIMIT: u32 = 200;
static SCOPE_ROOT_REVOCATION_PREVIEW_SEQUENCE: AtomicU64 = AtomicU64::new(1);

struct ManifestState {
    database_path: PathBuf,
    database_gate: Arc<Mutex<()>>,
    path_read_gate: Arc<Mutex<()>>,
    watch_status: Arc<Mutex<WatchRuntimeStatus>>,
    watch_stop: Arc<AtomicBool>,
    watch_wake: SyncSender<()>,
    native_watch_sync: NativeWatchSynchronizationBarrier,
    native_watch_callback_retirement: NativeWatchCallbackRetirement,
    watch_thread: Mutex<Option<JoinHandle<()>>>,
    scope_accesses: Arc<Mutex<HashMap<i64, Arc<ActiveScopeAccess>>>>,
    hard_exclusion_previews: Mutex<HardExclusionPreviewRegistry>,
    scope_root_revocation_previews: Mutex<ScopeRootRevocationPreviewRegistry>,
}

/// This is deliberately the only IPC-selectable kind. The WebView chooses a
/// native picker mode, never a filesystem path; the scanner still validates
/// the actual selected entry before it can reach the database.
#[derive(Clone, Copy, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
enum HardExclusionEntryKind {
    File,
    Folder,
}

#[derive(Serialize)]
struct CoveragePolicyDetailResponse {
    api_version: &'static str,
    scope_id: i64,
    root_display_path: String,
    policy_revision: i64,
    exclusions: Vec<CoveragePolicyExclusionResponse>,
}

#[derive(Serialize)]
struct CoveragePolicyExclusionResponse {
    id: i64,
    scope_id: i64,
    display_path: String,
    entry_kind: &'static str,
    created_at_unix_ms: i64,
}

#[derive(Serialize)]
struct HardExclusionPreviewResponse {
    api_version: &'static str,
    preview_id: String,
    scope_id: i64,
    base_policy_revision: i64,
    expires_at_unix_ms: i64,
    items: Vec<HardExclusionPreviewItemResponse>,
    impact: HardExclusionImpactResponse,
    confirmable: bool,
    source_files_will_change: bool,
}

#[derive(Serialize)]
struct HardExclusionPreviewItemResponse {
    display_path: String,
    entry_kind: &'static str,
    disposition: &'static str,
}

#[derive(Clone, Copy, Serialize)]
struct HardExclusionImpactResponse {
    location_count: u64,
    content_chunk_count: u64,
    graph_fact_count: u64,
    derived_candidate_count: u64,
    /// Durable rename/move preview plans. These are deliberately reported
    /// separately from graph-derived candidates so a privacy purge is not
    /// mistaken for an operation on the source files.
    action_plan_count: u64,
    /// Smart Cleanup preview plans, also distinct from graph-derived
    /// candidates and never an instruction to trash a source file.
    cleanup_action_plan_count: u64,
    pending_job_count: u64,
    blocking_action_count: u64,
}

#[derive(Serialize)]
struct HardExclusionCommitResponse {
    api_version: &'static str,
    scope_id: i64,
    policy_revision: i64,
    exclusions: u64,
    purge: HardExclusionImpactResponse,
    source_files_changed: bool,
    automatic_scans_started: u64,
    automatic_extractions_started: u64,
}

#[derive(Serialize)]
struct ScopeRootRevocationPreviewResponse {
    api_version: &'static str,
    preview_id: String,
    scope_id: i64,
    base_policy_revision: i64,
    expires_at_unix_ms: i64,
    impact: HardExclusionImpactResponse,
    exclusion_count: u64,
    confirmable: bool,
    source_files_will_change: bool,
}

#[derive(Serialize)]
struct ScopeRootRevocationCommitResponse {
    api_version: &'static str,
    scope_id: i64,
    policy_revision: i64,
    purged: HardExclusionImpactResponse,
    exclusions_removed: u64,
    runtime_capability_dropped: bool,
    native_watch_sync_confirmed: bool,
    native_watch_callback_retired: bool,
    watch_runtime_stopped: bool,
    source_files_changed: bool,
    revoked_scope_scans_started: u64,
    revoked_scope_extractions_started: u64,
}

/// Path- and identity-bearing picker state never crosses IPC or ordinary
/// logging. It exists only until a one-time Settings confirmation.
struct PendingHardExclusionPreview {
    scope_id: i64,
    base_policy_revision: i64,
    expires_at_unix_ms: i64,
    expires_at: Instant,
    entry_kind: HardExclusionEntryKind,
    prepared: deskgraph_scanner::PreparedScopeExclusionBatch,
}

#[derive(Default)]
struct HardExclusionPreviewRegistry {
    entries: HashMap<String, PendingHardExclusionPreview>,
    insertion_order: VecDeque<String>,
}

struct PendingScopeRootRevocationPreview {
    database_preview: ScopeRootRevocationPreview,
    expires_at_unix_ms: i64,
    expires_at: Instant,
}

#[derive(Default)]
struct ScopeRootRevocationPreviewRegistry {
    entries: HashMap<String, PendingScopeRootRevocationPreview>,
    insertion_order: VecDeque<String>,
}

impl Drop for ManifestState {
    fn drop(&mut self) {
        self.watch_stop.store(true, Ordering::Release);
        notify_watch_wake(&self.watch_wake);
        if let Ok(handle) = self.watch_thread.get_mut()
            && let Some(handle) = handle.take()
        {
            let deadline = Instant::now() + WATCH_RUNTIME_SHUTDOWN_TIMEOUT;
            while !handle.is_finished() && Instant::now() < deadline {
                thread::sleep(WATCH_RUNTIME_SHUTDOWN_POLL);
            }
            if handle.is_finished() {
                let _ = handle.join();
            } else {
                error!(
                    event = "watch_runtime_shutdown_timed_out",
                    error_code = "watch_runtime_shutdown_timed_out"
                );
            }
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
enum WatchRuntimeState {
    Starting,
    Running,
    Degraded,
    Stopped,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
struct WatchRuntimeStatus {
    api_version: &'static str,
    state: WatchRuntimeState,
    adapter: &'static str,
    poll_interval_ms: i64,
    periodic_reconciliation_enabled: bool,
    last_cycle_unix_ms: Option<i64>,
    authorized_scope_count: u64,
    active_event_count: u64,
    deferred_scope_count: u64,
    degraded_scope_count: u64,
    native_watched_scope_count: u64,
    native_overflow_count: u64,
    next_wake_unix_ms: Option<i64>,
    last_error_code: Option<&'static str>,
}

impl WatchRuntimeStatus {
    const API_VERSION: &str = "deskgraph.watch-runtime.v2";

    fn starting(polling_policy: PollingWatchPolicy) -> Self {
        Self {
            api_version: Self::API_VERSION,
            state: WatchRuntimeState::Starting,
            adapter: WATCH_ADAPTER_PERIODIC_ONLY,
            poll_interval_ms: polling_policy.poll_interval_ms(),
            periodic_reconciliation_enabled: true,
            last_cycle_unix_ms: None,
            authorized_scope_count: 0,
            active_event_count: 0,
            deferred_scope_count: 0,
            degraded_scope_count: 0,
            native_watched_scope_count: 0,
            native_overflow_count: 0,
            next_wake_unix_ms: None,
            last_error_code: None,
        }
    }
}

fn lock_database(state: &ManifestState) -> Result<MutexGuard<'_, ()>, String> {
    state
        .database_gate
        .lock()
        .map_err(|_| "manifest_writer_gate_poisoned".to_string())
}

fn lock_scope_accesses(
    state: &ManifestState,
) -> Result<MutexGuard<'_, HashMap<i64, Arc<ActiveScopeAccess>>>, String> {
    state
        .scope_accesses
        .lock()
        .map_err(|_| "scope_access_registry_poisoned".to_string())
}

fn lock_path_read_gate(gate: &Mutex<()>) -> MutexGuard<'_, ()> {
    match gate.lock() {
        Ok(guard) => guard,
        Err(poisoned) => {
            // The fence carries no user data or mutable product state. A panic
            // has already unwound and closed the provider's owned handles, so
            // recovery is safer than permanently disabling privacy withdrawal
            // until process restart.
            error!(
                event = "path_read_gate_recovered",
                error_code = "path_read_gate_poisoned"
            );
            poisoned.into_inner()
        }
    }
}

fn lock_path_reads(state: &ManifestState) -> MutexGuard<'_, ()> {
    lock_path_read_gate(&state.path_read_gate)
}

fn lock_hard_exclusion_previews(
    state: &ManifestState,
) -> Result<MutexGuard<'_, HardExclusionPreviewRegistry>, String> {
    state
        .hard_exclusion_previews
        .lock()
        .map_err(|_| "hard_exclusion_preview_registry_poisoned".to_string())
}

fn lock_scope_root_revocation_previews(
    state: &ManifestState,
) -> Result<MutexGuard<'_, ScopeRootRevocationPreviewRegistry>, String> {
    state
        .scope_root_revocation_previews
        .lock()
        .map_err(|_| "scope_root_revocation_preview_registry_poisoned".to_string())
}

fn hard_exclusion_now_unix_ms() -> Result<i64, String> {
    let elapsed = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| "hard_exclusion_clock_unavailable".to_string())?;
    i64::try_from(elapsed.as_millis()).map_err(|_| "hard_exclusion_clock_unavailable".to_string())
}

fn scope_root_revocation_now_unix_ms() -> Result<i64, String> {
    let elapsed = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| "scope_root_revocation_clock_unavailable".to_string())?;
    i64::try_from(elapsed.as_millis())
        .map_err(|_| "scope_root_revocation_clock_unavailable".to_string())
}

fn hard_exclusion_preview_id(now_unix_ms: i64) -> String {
    let sequence = HARD_EXCLUSION_PREVIEW_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    // This lookup token is opaque to the WebView and excludes path/identity
    // data. It is deliberately not an authorization credential: confirmation
    // consumes it once and still revalidates the active grant, policy revision,
    // selected source identity/kind, and database transaction fence.
    format!("hep-{:x}-{:x}", now_unix_ms, sequence)
}

fn scope_root_revocation_preview_id(now_unix_ms: i64) -> String {
    let sequence = SCOPE_ROOT_REVOCATION_PREVIEW_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    format!("srrp-{:x}-{:x}", now_unix_ms, sequence)
}

impl HardExclusionPreviewRegistry {
    fn discard_expired_at(&mut self, now: Instant) {
        self.entries.retain(|_, preview| preview.expires_at > now);
        self.insertion_order
            .retain(|preview_id| self.entries.contains_key(preview_id));
    }

    fn insert_at(&mut self, preview: PendingHardExclusionPreview, now: Instant) -> String {
        self.discard_expired_at(now);
        while self.entries.len() >= MAX_PENDING_HARD_EXCLUSION_PREVIEWS {
            let Some(expired_or_oldest) = self.insertion_order.pop_front() else {
                self.entries.clear();
                break;
            };
            self.entries.remove(&expired_or_oldest);
        }
        let preview_id = hard_exclusion_preview_id(preview.expires_at_unix_ms);
        self.insertion_order.push_back(preview_id.clone());
        self.entries.insert(preview_id.clone(), preview);
        preview_id
    }

    /// Confirmation consumes state before revalidation or database work. A
    /// retry can therefore never turn a previously approved picker result into
    /// a reusable filesystem capability.
    fn take_for_confirmation_at(
        &mut self,
        preview_id: &str,
        now: Instant,
    ) -> Result<PendingHardExclusionPreview, &'static str> {
        let preview = self
            .entries
            .remove(preview_id)
            .ok_or("hard_exclusion_preview_not_found")?;
        self.insertion_order.retain(|id| id != preview_id);
        if preview.expires_at <= now {
            return Err("hard_exclusion_preview_expired");
        }
        Ok(preview)
    }

    fn discard(&mut self, preview_id: &str) {
        self.entries.remove(preview_id);
        self.insertion_order.retain(|id| id != preview_id);
    }

    fn discard_scope(&mut self, scope_id: i64) {
        self.entries
            .retain(|_, preview| preview.scope_id != scope_id);
        self.insertion_order
            .retain(|preview_id| self.entries.contains_key(preview_id));
    }
}

impl ScopeRootRevocationPreviewRegistry {
    fn discard_expired_at(&mut self, now: Instant) {
        self.entries.retain(|_, preview| preview.expires_at > now);
        self.insertion_order
            .retain(|preview_id| self.entries.contains_key(preview_id));
    }

    fn insert_at(&mut self, preview: PendingScopeRootRevocationPreview, now: Instant) -> String {
        self.discard_expired_at(now);
        while self.entries.len() >= MAX_PENDING_SCOPE_ROOT_REVOCATION_PREVIEWS {
            let Some(expired_or_oldest) = self.insertion_order.pop_front() else {
                self.entries.clear();
                break;
            };
            self.entries.remove(&expired_or_oldest);
        }
        let preview_id = scope_root_revocation_preview_id(preview.expires_at_unix_ms);
        self.insertion_order.push_back(preview_id.clone());
        self.entries.insert(preview_id.clone(), preview);
        preview_id
    }

    fn take_for_confirmation_at(
        &mut self,
        preview_id: &str,
        now: Instant,
    ) -> Result<PendingScopeRootRevocationPreview, &'static str> {
        let preview = self
            .entries
            .remove(preview_id)
            .ok_or("scope_root_revocation_preview_not_found")?;
        self.insertion_order.retain(|id| id != preview_id);
        if preview.expires_at <= now {
            return Err("scope_root_revocation_preview_expired");
        }
        Ok(preview)
    }

    fn discard(&mut self, preview_id: &str) {
        self.entries.remove(preview_id);
        self.insertion_order.retain(|id| id != preview_id);
    }

    fn discard_scope(&mut self, scope_id: i64) {
        self.entries
            .retain(|_, preview| preview.database_preview.scope_id != scope_id);
        self.insertion_order
            .retain(|preview_id| self.entries.contains_key(preview_id));
    }
}

fn scope_exclusion_kind_name(kind: ScopeExclusionKind) -> &'static str {
    match kind {
        ScopeExclusionKind::File => "file",
        ScopeExclusionKind::Folder => "folder",
    }
}

fn map_hard_exclusion_impact(impact: ScopeExclusionImpactPreview) -> HardExclusionImpactResponse {
    // Relations are derived candidates rather than graph facts. Action and
    // Cleanup records are plans, likewise not candidates. Action safety
    // records that cannot be removed under ADR-033 are reported separately.
    let graph_fact_count = impact.edge_count;
    let derived_candidate_count = [
        impact.project_count,
        impact.relation_count,
        impact.screenshot_group_count,
    ]
    .into_iter()
    .fold(0_u64, u64::saturating_add);
    HardExclusionImpactResponse {
        // Conservative location count also covers same-scope hard-link
        // withholding; reporting only direct rows would understate the purge.
        location_count: impact.conservative_location_count,
        content_chunk_count: impact.content_chunk_count,
        graph_fact_count,
        derived_candidate_count,
        action_plan_count: impact.action_plan_count,
        cleanup_action_plan_count: impact.cleanup_action_plan_count,
        pending_job_count: impact.pending_job_count,
        blocking_action_count: impact.blocking_action_count,
    }
}

fn with_active_scope_read_fence<T>(
    path_read_gate: &Mutex<()>,
    scope_accesses: &Mutex<HashMap<i64, Arc<ActiveScopeAccess>>>,
    scope_id: i64,
    operation: impl FnOnce() -> Result<T, String>,
) -> Result<T, String> {
    let _path_read_guard = lock_path_read_gate(path_read_gate);
    let access = {
        let scope_guard = scope_accesses
            .lock()
            .map_err(|_| "scope_access_registry_poisoned".to_string())?;
        Arc::clone(
            scope_guard
                .get(&scope_id)
                .ok_or("scope_reauthorization_required")?,
        )
    };
    let result = operation();
    drop(access);
    result
}

fn require_active_scope(state: &ManifestState, scope_id: i64) -> Result<(), String> {
    if lock_scope_accesses(state)?.contains_key(&scope_id) {
        Ok(())
    } else {
        Err("scope_reauthorization_required".to_string())
    }
}

fn active_scope_ids(state: &ManifestState) -> Result<HashSet<i64>, String> {
    Ok(lock_scope_accesses(state)?.keys().copied().collect())
}

fn lock_watch_status(
    status: &Mutex<WatchRuntimeStatus>,
) -> Result<MutexGuard<'_, WatchRuntimeStatus>, String> {
    status
        .lock()
        .map_err(|_| "watch_runtime_status_poisoned".to_string())
}

fn notify_watch_wake(wake: &SyncSender<()>) {
    match wake.try_send(()) {
        Ok(()) | Err(TrySendError::Full(())) | Err(TrySendError::Disconnected(())) => {}
    }
}

fn wait_for_watch_wake(
    wake: &Receiver<()>,
    stop: &AtomicBool,
    timeout: Duration,
) -> Result<(), ()> {
    if stop.load(Ordering::Acquire) {
        return Ok(());
    }
    match wake.recv_timeout(timeout) {
        Ok(()) | Err(RecvTimeoutError::Timeout) => Ok(()),
        Err(RecvTimeoutError::Disconnected) => Err(()),
    }
}

fn wake_watch_runtime(state: &ManifestState) {
    notify_watch_wake(&state.watch_wake);
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct WatchRuntimeRetirementOutcome {
    native_watch_callback_retired: bool,
    watch_runtime_stopped: bool,
}

/// A revocation has already committed when this path is reached, so it must
/// never turn that durable privacy withdrawal into an IPC error.  Callback
/// admission is closed and its path queue is cleared *before* the coordinator
/// is stopped.  This is the fail-closed boundary when a platform watcher or
/// coordinator cannot be joined promptly.
fn retire_watch_runtime_after_revocation_timeout(
    state: &ManifestState,
) -> WatchRuntimeRetirementOutcome {
    let mut native_watch_callback_retired = state
        .native_watch_callback_retirement
        .retire_and_clear(WATCH_RUNTIME_SHUTDOWN_TIMEOUT);
    state.watch_stop.store(true, Ordering::Release);
    wake_watch_runtime(state);

    let Ok(mut watch_thread) = state.watch_thread.lock() else {
        error!(
            event = "watch_runtime_revocation_shutdown_lock_failed",
            error_code = "watch_runtime_revocation_shutdown_lock_failed"
        );
        return WatchRuntimeRetirementOutcome {
            native_watch_callback_retired,
            watch_runtime_stopped: false,
        };
    };
    let Some(handle) = watch_thread.as_ref() else {
        return WatchRuntimeRetirementOutcome {
            native_watch_callback_retired,
            watch_runtime_stopped: true,
        };
    };
    let deadline = Instant::now() + WATCH_RUNTIME_SHUTDOWN_TIMEOUT;
    while !handle.is_finished() && Instant::now() < deadline {
        thread::sleep(WATCH_RUNTIME_SHUTDOWN_POLL);
    }
    if !handle.is_finished() {
        error!(
            event = "watch_runtime_revocation_shutdown_timed_out",
            error_code = "watch_runtime_revocation_shutdown_timed_out"
        );
        return WatchRuntimeRetirementOutcome {
            native_watch_callback_retired,
            watch_runtime_stopped: false,
        };
    }
    let handle = watch_thread
        .take()
        .expect("finished watch thread should still have a handle");
    if handle.join().is_err() {
        error!(
            event = "watch_runtime_revocation_shutdown_panicked",
            error_code = "watch_runtime_revocation_shutdown_panicked"
        );
        return WatchRuntimeRetirementOutcome {
            native_watch_callback_retired,
            watch_runtime_stopped: false,
        };
    }
    if !native_watch_callback_retired {
        native_watch_callback_retired = state
            .native_watch_callback_retirement
            .retire_and_clear(Duration::ZERO);
    }
    WatchRuntimeRetirementOutcome {
        native_watch_callback_retired,
        watch_runtime_stopped: true,
    }
}

#[cfg(test)]
fn start_manifest_state(database_path: PathBuf) -> ManifestState {
    start_manifest_state_with_accesses(database_path, HashMap::new())
}

struct WatchCoordinatorRuntime {
    database_path: PathBuf,
    database_gate: Arc<Mutex<()>>,
    stop: Arc<AtomicBool>,
    wake: SyncSender<()>,
    wake_receiver: Receiver<()>,
    native_watch_sync: NativeWatchSynchronizationBarrier,
    native_watch_callback_retirement: NativeWatchCallbackRetirement,
    status: Arc<Mutex<WatchRuntimeStatus>>,
    scope_accesses: Arc<Mutex<HashMap<i64, Arc<ActiveScopeAccess>>>>,
    polling_policy: PollingWatchPolicy,
}

fn start_manifest_state_with_accesses(
    database_path: PathBuf,
    scope_accesses: HashMap<i64, ActiveScopeAccess>,
) -> ManifestState {
    let database_gate = Arc::new(Mutex::new(()));
    let path_read_gate = Arc::new(Mutex::new(()));
    let watch_stop = Arc::new(AtomicBool::new(false));
    let (watch_wake, watch_wake_receiver) = sync_channel(1);
    let native_watch_sync = NativeWatchSynchronizationBarrier::default();
    let native_watch_callback_retirement = NativeWatchCallbackRetirement::default();
    let polling_policy = PollingWatchPolicy::default();
    let watch_status = Arc::new(Mutex::new(WatchRuntimeStatus::starting(polling_policy)));
    let thread_database_path = database_path.clone();
    let thread_database_gate = Arc::clone(&database_gate);
    let thread_watch_stop = Arc::clone(&watch_stop);
    let thread_watch_wake = watch_wake.clone();
    let thread_watch_status = Arc::clone(&watch_status);
    let scope_accesses = Arc::new(Mutex::new(
        scope_accesses
            .into_iter()
            .map(|(scope_id, access)| (scope_id, Arc::new(access)))
            .collect(),
    ));
    let thread_scope_accesses = Arc::clone(&scope_accesses);
    let runtime = WatchCoordinatorRuntime {
        database_path: thread_database_path,
        database_gate: thread_database_gate,
        stop: thread_watch_stop,
        wake: thread_watch_wake,
        wake_receiver: watch_wake_receiver,
        native_watch_sync: native_watch_sync.clone(),
        native_watch_callback_retirement: native_watch_callback_retirement.clone(),
        status: thread_watch_status,
        scope_accesses: thread_scope_accesses,
        polling_policy,
    };
    let watch_thread = thread::Builder::new()
        .name("deskgraph-watch-coordinator".to_string())
        .spawn(move || run_watch_coordinator(runtime));
    let watch_thread = match watch_thread {
        Ok(handle) => Some(handle),
        Err(_) => {
            if let Ok(mut status) = watch_status.lock() {
                status.state = WatchRuntimeState::Degraded;
                status.last_error_code = Some("watch_runtime_thread_start_failed");
            }
            None
        }
    };

    ManifestState {
        database_path,
        database_gate,
        path_read_gate,
        watch_status,
        watch_stop,
        watch_wake,
        native_watch_sync,
        native_watch_callback_retirement,
        watch_thread: Mutex::new(watch_thread),
        scope_accesses,
        hard_exclusion_previews: Mutex::new(HardExclusionPreviewRegistry::default()),
        scope_root_revocation_previews: Mutex::new(ScopeRootRevocationPreviewRegistry::default()),
    }
}

fn run_watch_coordinator(runtime: WatchCoordinatorRuntime) {
    let WatchCoordinatorRuntime {
        database_path,
        database_gate,
        stop,
        wake,
        wake_receiver,
        native_watch_sync,
        native_watch_callback_retirement,
        status,
        scope_accesses,
        polling_policy,
    } = runtime;
    // Opening a manifest configures SQLite's WAL mode. Serialize this startup
    // path with every other in-process manifest open so the coordinator cannot
    // race an IPC preview on the journal-mode pragma.
    let coordinator_open = {
        let _database_guard = match database_gate.lock() {
            Ok(guard) => guard,
            Err(_) => {
                if let Ok(mut status) = status.lock() {
                    status.state = WatchRuntimeState::Degraded;
                    status.last_error_code = Some("manifest_writer_gate_poisoned");
                }
                error!(
                    event = "watch_runtime_start_failed",
                    error_code = "manifest_writer_gate_poisoned"
                );
                return;
            }
        };
        WatchCoordinator::open_requiring_active_platform_grants(
            &database_path,
            WatchPolicy::default(),
            polling_policy,
        )
    };
    let mut coordinator = match coordinator_open {
        Ok(coordinator) => coordinator,
        Err(error) => {
            if let Ok(mut status) = status.lock() {
                status.state = WatchRuntimeState::Degraded;
                status.last_error_code = Some(error.code());
            }
            error!(
                event = "watch_runtime_start_failed",
                error_code = error.code()
            );
            return;
        }
    };
    let mut native_source = None;
    let mut native_error = Some("watch_native_adapter_starting");
    let mut next_native_retry = Instant::now();
    let mut reconcile_after_native_change = false;

    while !stop.load(Ordering::Acquire) {
        let native_synchronization_pass = native_watch_sync.begin_pass();
        let registration_only_pass = native_watch_sync.has_pending(native_synchronization_pass);
        let runtime_active_scope_ids = match scope_accesses.lock() {
            Ok(accesses) => accesses.keys().copied().collect::<Vec<_>>(),
            Err(_) => {
                if let Ok(mut status) = status.lock() {
                    status.state = WatchRuntimeState::Degraded;
                    status.last_error_code = Some("scope_access_registry_poisoned");
                }
                return;
            }
        };
        coordinator.replace_runtime_active_scope_ids(runtime_active_scope_ids);
        if !registration_only_pass && native_source.is_none() && Instant::now() >= next_native_retry
        {
            let callback_wake = wake.clone();
            match NativeWatchEventSource::new_with_retirement(
                Arc::new(move || {
                    notify_watch_wake(&callback_wake);
                }),
                native_watch_callback_retirement.clone(),
            ) {
                Ok(source) => {
                    native_source = Some(source);
                    native_error = None;
                    reconcile_after_native_change = true;
                }
                Err(_) => {
                    native_error = Some("watch_native_adapter_unavailable");
                    next_native_retry = Instant::now() + WATCH_NATIVE_RETRY_INTERVAL;
                }
            }
        }

        if let Some(source) = native_source.as_mut() {
            match coordinator.synchronize_native_event_source(source) {
                Ok(changed) => {
                    reconcile_after_native_change |= changed;
                    native_watch_sync.acknowledge(native_synchronization_pass);
                }
                Err(_) => {
                    native_source = None;
                    native_error = Some("watch_native_adapter_unavailable");
                    next_native_retry = Instant::now() + WATCH_NATIVE_RETRY_INTERVAL;
                    reconcile_after_native_change = true;
                    // Dropping the failed source closes all of its native
                    // registrations. There is no stale subscription left to
                    // wait for, even though the adapter is now degraded.
                    native_watch_sync.acknowledge(native_synchronization_pass);
                }
            }
        } else {
            // No native source means there is no OS registration to retire.
            native_watch_sync.acknowledge(native_synchronization_pass);
        }

        if registration_only_pass {
            // A privacy-withdrawal wake is an immediate native-registration
            // barrier only. It cannot retry an adapter, request a remaining-
            // scope reconciliation, or run the polling scheduler in this pass.
            continue;
        }

        let cycle = match database_gate.try_lock() {
            Ok(_database_guard) => {
                if stop.load(Ordering::Acquire) {
                    break;
                }
                if reconcile_after_native_change {
                    if let Err(error) = coordinator.request_all_scope_reconciliation() {
                        Err(error)
                    } else {
                        reconcile_after_native_change = false;
                        match native_source.as_ref() {
                            Some(source) => coordinator.run_cycle_with_native_event_source(source),
                            None => coordinator.run_cycle(),
                        }
                    }
                } else {
                    match native_source.as_ref() {
                        Some(source) => coordinator.run_cycle_with_native_event_source(source),
                        None => coordinator.run_cycle(),
                    }
                }
            }
            Err(TryLockError::WouldBlock) => {
                if wait_for_watch_wake(&wake_receiver, &stop, WATCH_GATE_RETRY_INTERVAL).is_err() {
                    break;
                }
                continue;
            }
            Err(TryLockError::Poisoned(_)) => {
                if let Ok(mut status) = status.lock() {
                    status.state = WatchRuntimeState::Degraded;
                    status.last_error_code = Some("manifest_writer_gate_poisoned");
                }
                break;
            }
        };
        match cycle {
            Ok(report) => {
                let native_failed = report.native_source_failed
                    || native_source
                        .as_ref()
                        .is_some_and(NativeWatchEventSource::source_failed);
                if native_failed {
                    native_source = None;
                    native_error = Some("watch_native_source_failed");
                    next_native_retry = Instant::now() + WATCH_NATIVE_RETRY_INTERVAL;
                    reconcile_after_native_change = true;
                } else if native_source.is_some() {
                    native_error = None;
                }
                let watched_scope_count = native_source
                    .as_ref()
                    .map_or(0, NativeWatchEventSource::watched_scope_count);
                let watched_scope_count = u64::try_from(watched_scope_count).unwrap_or(u64::MAX);
                let adapter = if native_source.is_some() {
                    WATCH_ADAPTER_NATIVE
                } else {
                    WATCH_ADAPTER_PERIODIC_ONLY
                };
                let error_code = native_error.or(report.last_error_code);
                let mut next_wake_unix_ms = report.next_wake_unix_ms;
                let mut wait_ms = report
                    .next_wake_unix_ms
                    .saturating_sub(report.cycle_unix_ms)
                    .max(1);
                if native_source.is_none() {
                    let retry_ms = i64::try_from(
                        next_native_retry
                            .saturating_duration_since(Instant::now())
                            .as_millis(),
                    )
                    .unwrap_or(i64::MAX)
                    .max(1);
                    wait_ms = wait_ms.min(retry_ms);
                    next_wake_unix_ms = report.cycle_unix_ms.saturating_add(wait_ms);
                }
                if let Ok(mut status) = status.lock() {
                    status.state = if error_code.is_some() {
                        WatchRuntimeState::Degraded
                    } else {
                        WatchRuntimeState::Running
                    };
                    status.adapter = adapter;
                    status.last_cycle_unix_ms = Some(report.cycle_unix_ms);
                    status.authorized_scope_count = report.authorized_scope_count;
                    status.active_event_count = report.active_event_count;
                    status.deferred_scope_count = report.deferred_scope_count;
                    status.degraded_scope_count = report.degraded_scope_count;
                    status.native_watched_scope_count = watched_scope_count;
                    status.native_overflow_count = status
                        .native_overflow_count
                        .saturating_add(report.native_overflow_count);
                    status.next_wake_unix_ms = Some(next_wake_unix_ms);
                    status.last_error_code = error_code;
                }
                if report.scheduled_scope_count > 0
                    || report.advanced_event_count > 0
                    || report.native_signal_count > 0
                    || report.native_reconcile_all
                    || report.forced_scope_reconciliation_count > 0
                    || error_code.is_some()
                {
                    info!(
                        event = "watch_runtime_cycle",
                        authorized_scope_count = report.authorized_scope_count,
                        active_event_count = report.active_event_count,
                        scheduled_scope_count = report.scheduled_scope_count,
                        advanced_event_count = report.advanced_event_count,
                        completed_event_count = report.completed_event_count,
                        deferred_event_count = report.deferred_event_count,
                        deferred_scope_count = report.deferred_scope_count,
                        degraded_scope_count = report.degraded_scope_count,
                        native_signal_count = report.native_signal_count,
                        native_hint_scope_count = report.native_hint_scope_count,
                        native_overflow_count = report.native_overflow_count,
                        native_reconcile_all = report.native_reconcile_all,
                        native_more_pending = report.native_more_pending,
                        forced_scope_reconciliation_count =
                            report.forced_scope_reconciliation_count,
                        native_watched_scope_count = watched_scope_count,
                        error_code
                    );
                }

                if stop.load(Ordering::Acquire) {
                    break;
                }
                let Ok(wait_duration) = u64::try_from(wait_ms) else {
                    continue;
                };
                if wait_for_watch_wake(&wake_receiver, &stop, Duration::from_millis(wait_duration))
                    .is_err()
                {
                    if let Ok(mut status) = status.lock() {
                        status.state = WatchRuntimeState::Degraded;
                        status.last_error_code = Some("watch_runtime_wake_channel_closed");
                    }
                    break;
                }
            }
            Err(error) => {
                if let Ok(mut status) = status.lock() {
                    status.state = WatchRuntimeState::Degraded;
                    status.last_error_code = Some(error.code());
                }
                error!(
                    event = "watch_runtime_cycle_failed",
                    error_code = error.code()
                );
                if wait_for_watch_wake(&wake_receiver, &stop, Duration::from_secs(5)).is_err() {
                    break;
                }
            }
        }
    }
    if let Ok(mut status) = status.lock() {
        status.state = WatchRuntimeState::Stopped;
        status.adapter = WATCH_ADAPTER_PERIODIC_ONLY;
        status.last_error_code = None;
        status.deferred_scope_count = 0;
        status.degraded_scope_count = 0;
        status.native_watched_scope_count = 0;
        status.next_wake_unix_ms = None;
    }
}

#[tauri::command]
fn health(state: State<'_, ManifestState>) -> Result<HealthReport, String> {
    let scope_count = u32::try_from(active_scope_ids(&state)?.len())
        .map_err(|_| "authorized_scope_count_out_of_range".to_string())?;
    let report = collect_health_with_manifest(scope_count);
    info!(
        event = "health_check_completed",
        status = report.status,
        database_state = ?report.database.state
    );
    Ok(report)
}

#[tauri::command]
fn manifest_status(state: State<'_, ManifestState>) -> Result<ManifestStats, String> {
    manifest_status_with_active_access_grants_at(&state.database_path).map_err(str::to_string)
}

#[tauri::command]
fn authorized_scopes(state: State<'_, ManifestState>) -> Result<Vec<AuthorizedScope>, String> {
    let active = active_scope_ids(&state)?;
    authorized_scopes_at(&state.database_path)
        .map(|scopes| {
            scopes
                .into_iter()
                .filter(|scope| active.contains(&scope.id))
                .collect()
        })
        .map_err(str::to_string)
}

/// Settings-only coverage inspection. It deliberately returns the current
/// root, durable revision, and explicit exclusions, but accepts no path from
/// the WebView.
#[tauri::command]
fn coverage_policy_detail(
    state: State<'_, ManifestState>,
    scope_id: i64,
) -> Result<CoveragePolicyDetailResponse, String> {
    require_active_scope(&state, scope_id)?;
    let _database_guard = lock_database(&state)?;
    let database = ManifestDatabase::open(&state.database_path).map_err(|error| error.code())?;
    let binding = database
        .bind_scope_policy_revision(scope_id)
        .map_err(|error| error.code())?;
    let scope = database
        .scope_record(scope_id)
        .map_err(|error| error.code())?;
    let exclusions = database
        .scope_exclusions(scope_id)
        .map_err(|error| error.code())?
        .into_iter()
        .map(|exclusion| CoveragePolicyExclusionResponse {
            id: exclusion.id,
            scope_id: exclusion.scope_id,
            display_path: exclusion.display_path,
            entry_kind: scope_exclusion_kind_name(exclusion.kind),
            created_at_unix_ms: exclusion.created_at_unix_ms,
        })
        .collect();
    Ok(CoveragePolicyDetailResponse {
        api_version: "deskgraph.coverage-policy.v1",
        scope_id,
        root_display_path: scope.display_path,
        policy_revision: binding.revision,
        exclusions,
    })
}

#[tauri::command]
fn preview_scope_root_revocation(
    state: State<'_, ManifestState>,
    scope_id: i64,
) -> Result<ScopeRootRevocationPreviewResponse, String> {
    preview_scope_root_revocation_for_state(&state, scope_id)
}

fn preview_scope_root_revocation_for_state(
    state: &ManifestState,
    scope_id: i64,
) -> Result<ScopeRootRevocationPreviewResponse, String> {
    let now_unix_ms = scope_root_revocation_now_unix_ms()?;
    let expires_at_unix_ms = now_unix_ms
        .checked_add(SCOPE_ROOT_REVOCATION_PREVIEW_TTL_MS)
        .ok_or("scope_root_revocation_clock_unavailable")?;
    let expires_at = Instant::now()
        .checked_add(Duration::from_millis(
            SCOPE_ROOT_REVOCATION_PREVIEW_TTL_MS as u64,
        ))
        .ok_or("scope_root_revocation_clock_unavailable")?;
    let _database_guard = lock_database(state)?;
    let scope_guard = lock_scope_accesses(state)?;
    if !scope_guard.contains_key(&scope_id) {
        return Err("scope_reauthorization_required".to_string());
    }
    let database = ManifestDatabase::open(&state.database_path).map_err(|error| error.code())?;
    let binding = database
        .bind_scope_policy_revision(scope_id)
        .map_err(|error| error.code())?;
    let preview = database
        .preview_scope_root_revocation(binding)
        .map_err(|error| error.code().to_string())?;
    let impact = map_hard_exclusion_impact(preview.impact);
    let confirmable = impact.blocking_action_count == 0;
    drop(scope_guard);
    let preview_id = lock_scope_root_revocation_previews(state)?.insert_at(
        PendingScopeRootRevocationPreview {
            database_preview: preview,
            expires_at_unix_ms,
            expires_at,
        },
        Instant::now(),
    );
    Ok(ScopeRootRevocationPreviewResponse {
        api_version: "deskgraph.scope-root-revocation-preview.v1",
        preview_id,
        scope_id,
        base_policy_revision: preview.base_policy_revision,
        expires_at_unix_ms,
        impact,
        exclusion_count: preview.exclusion_count,
        confirmable,
        source_files_will_change: false,
    })
}

#[tauri::command]
fn confirm_scope_root_revocation(
    state: State<'_, ManifestState>,
    preview_id: String,
) -> Result<ScopeRootRevocationCommitResponse, String> {
    confirm_scope_root_revocation_for_state(&state, preview_id)
}

fn confirm_scope_root_revocation_for_state(
    state: &ManifestState,
    preview_id: String,
) -> Result<ScopeRootRevocationCommitResponse, String> {
    if preview_id.is_empty() || preview_id.len() > 128 {
        return Err("scope_root_revocation_preview_not_found".to_string());
    }
    let pending = lock_scope_root_revocation_previews(state)?
        .take_for_confirmation_at(&preview_id, Instant::now())
        .map_err(str::to_string)?;

    // Lock order is process read gate -> database gate -> runtime registry ->
    // cross-process scope fence -> SQLite mutation. The first gate drains
    // in-process work; the scope fence then drains cooperating CLI/Desktop
    // readers before the privacy commit and capability drop.
    let _path_read_guard = lock_path_reads(state);
    let _database_guard = lock_database(state)?;
    let mut scope_guard = lock_scope_accesses(state)?;
    if !scope_guard.contains_key(&pending.database_preview.scope_id) {
        return Err("scope_reauthorization_required".to_string());
    }
    if pending.expires_at <= Instant::now() {
        return Err("scope_root_revocation_preview_expired".to_string());
    }
    let now_unix_ms = scope_root_revocation_now_unix_ms()?;
    let database = ManifestDatabase::open(&state.database_path).map_err(|error| error.code())?;
    // This cross-process advisory fence drains any CLI or Desktop filesystem
    // read that started before revocation. It is acquired before the SQLite
    // mutation and remains held until the runtime capability is dropped.
    let filesystem_revocation_fence = database
        .acquire_scope_filesystem_revocation_fence(pending.database_preview.scope_id)
        .map_err(|error| error.code())?;
    let binding = database
        .bind_scope_policy_revision(pending.database_preview.scope_id)
        .map_err(|error| error.code())?;
    if binding.revision != pending.database_preview.base_policy_revision {
        return Err("scope_policy_changed".to_string());
    }
    let applied = database
        .apply_scope_root_revocation_from_preview_with_fence(
            &filesystem_revocation_fence,
            pending.database_preview,
            now_unix_ms,
        )
        .map_err(|error| error.code().to_string())?;
    let purged = map_hard_exclusion_impact(applied.purged);
    // Request while the registry lock is still held. Any Watch pass that can
    // acknowledge this ticket must therefore read the registry only after the
    // revoked capability has been removed below.
    let native_watch_ticket = state.native_watch_sync.request();
    let access = scope_guard.remove(&pending.database_preview.scope_id);
    drop(access);
    match state.hard_exclusion_previews.lock() {
        Ok(mut previews) => previews.discard_scope(pending.database_preview.scope_id),
        Err(poisoned) => {
            error!(
                event = "hard_exclusion_preview_cleanup_recovered",
                error_code = "hard_exclusion_preview_registry_poisoned"
            );
            poisoned
                .into_inner()
                .discard_scope(pending.database_preview.scope_id);
        }
    }
    match state.scope_root_revocation_previews.lock() {
        Ok(mut previews) => previews.discard_scope(pending.database_preview.scope_id),
        Err(poisoned) => {
            error!(
                event = "scope_root_revocation_preview_cleanup_recovered",
                error_code = "scope_root_revocation_preview_registry_poisoned"
            );
            poisoned
                .into_inner()
                .discard_scope(pending.database_preview.scope_id);
        }
    }
    drop(scope_guard);
    drop(_database_guard);
    drop(_path_read_guard);
    wake_watch_runtime(state);
    // Revocation is already durably committed at this point. A native
    // registration acknowledgement timeout is therefore a fail-closed
    // post-commit condition, not a failed mutation: immediately terminate the
    // entire coordinator so its native source is dropped. The UI must require
    // a restart rather than imply that automatic monitoring remains active.
    let native_watch_sync_confirmed = state
        .native_watch_sync
        .wait_for(native_watch_ticket, WATCH_NATIVE_SYNCHRONIZATION_TIMEOUT);
    let watch_retirement = if native_watch_sync_confirmed {
        WatchRuntimeRetirementOutcome {
            native_watch_callback_retired: false,
            watch_runtime_stopped: false,
        }
    } else {
        let outcome = retire_watch_runtime_after_revocation_timeout(state);
        error!(
            event = "scope_root_revocation_native_watch_sync_timed_out",
            error_code = "scope_root_revocation_native_watch_sync_timed_out",
            native_watch_callback_retired = outcome.native_watch_callback_retired,
            watch_runtime_stopped = outcome.watch_runtime_stopped
        );
        outcome
    };
    let response = ScopeRootRevocationCommitResponse {
        api_version: "deskgraph.scope-root-revocation-commit.v1",
        scope_id: applied.policy.scope_id,
        policy_revision: applied.policy.revision,
        purged,
        exclusions_removed: applied.receipt.exclusions_removed,
        runtime_capability_dropped: true,
        native_watch_sync_confirmed,
        native_watch_callback_retired: watch_retirement.native_watch_callback_retired,
        watch_runtime_stopped: watch_retirement.watch_runtime_stopped,
        source_files_changed: false,
        revoked_scope_scans_started: 0,
        revoked_scope_extractions_started: 0,
    };
    info!(
        event = "scope_root_revoked",
        scope_id = response.scope_id,
        policy_revision = response.policy_revision,
        exclusions_removed = response.exclusions_removed,
        runtime_capability_dropped = true,
        native_watch_sync_confirmed,
        native_watch_callback_retired = response.native_watch_callback_retired,
        watch_runtime_stopped = response.watch_runtime_stopped,
        source_files_changed = false,
        revoked_scope_scans_started = 0_u64,
        revoked_scope_extractions_started = 0_u64
    );
    Ok(response)
}

#[tauri::command]
fn discard_scope_root_revocation(
    state: State<'_, ManifestState>,
    preview_id: String,
) -> Result<(), String> {
    if preview_id.len() <= 128 {
        lock_scope_root_revocation_previews(&state)?.discard(&preview_id);
    }
    Ok(())
}

/// Opens a native multi-file or multi-folder picker. The selected paths remain
/// in this process, are revalidated by the scanner, and are never command
/// arguments or response identifiers.
#[tauri::command]
async fn select_hard_exclusions_preview(
    app: AppHandle,
    state: State<'_, ManifestState>,
    scope_id: i64,
    entry_kind: HardExclusionEntryKind,
) -> Result<Option<HardExclusionPreviewResponse>, String> {
    require_active_scope(&state, scope_id)?;
    let selected = match entry_kind {
        HardExclusionEntryKind::File => app.dialog().file().blocking_pick_files(),
        HardExclusionEntryKind::Folder => app.dialog().file().blocking_pick_folders(),
    };
    let Some(selected) = selected else {
        return Ok(None);
    };
    let selected_paths = selected
        .into_iter()
        .map(|entry| {
            entry
                .into_path()
                .map_err(|_| "hard_exclusion_selection_invalid".to_string())
        })
        .collect::<Result<Vec<_>, _>>()?;
    select_hard_exclusions_preview_from_native_paths(&state, scope_id, entry_kind, &selected_paths)
        .map(Some)
}

fn select_hard_exclusions_preview_from_native_paths(
    state: &ManifestState,
    scope_id: i64,
    entry_kind: HardExclusionEntryKind,
    selected_paths: &[PathBuf],
) -> Result<HardExclusionPreviewResponse, String> {
    require_active_scope(state, scope_id)?;
    let now_unix_ms = hard_exclusion_now_unix_ms()?;
    let expires_at_unix_ms = now_unix_ms
        .checked_add(HARD_EXCLUSION_PREVIEW_TTL_MS)
        .ok_or("hard_exclusion_clock_unavailable")?;
    let expires_at = Instant::now()
        .checked_add(Duration::from_millis(HARD_EXCLUSION_PREVIEW_TTL_MS as u64))
        .ok_or("hard_exclusion_clock_unavailable")?;
    let _database_guard = lock_database(state)?;
    let mut database =
        ManifestDatabase::open(&state.database_path).map_err(|error| error.code())?;
    let binding = database
        .bind_scope_policy_revision(scope_id)
        .map_err(|error| error.code())?;
    let selections = selected_paths
        .iter()
        .map(|path| ScopeExclusionSelection {
            requested_path: path.as_path(),
        })
        .collect::<Vec<_>>();
    let prepared =
        prepare_scope_exclusion_batch_with_active_grant(&database, scope_id, &selections)
            .map_err(|error| error.code().to_string())?;
    if prepared
        .exclusions
        .iter()
        .any(|exclusion| !hard_exclusion_kind_matches(entry_kind, exclusion.kind))
    {
        return Err("hard_exclusion_selection_kind_changed".to_string());
    }
    let writes = prepared
        .exclusions
        .iter()
        .map(|exclusion| ScopeExclusionWrite {
            kind: exclusion.kind,
            path_raw: &exclusion.path_raw,
            path_key: &exclusion.path_key,
            display_path: &exclusion.display_path,
            identity_kind: &exclusion.identity_kind,
            identity_key: &exclusion.identity_key,
        })
        .collect::<Vec<_>>();
    let impact = database
        .preview_scope_exclusion_batch(binding, &writes)
        .map_err(|error| error.code().to_string())?;
    let impact = map_hard_exclusion_impact(impact);
    let confirmable = impact.blocking_action_count == 0;
    let items = prepared
        .exclusions
        .iter()
        .map(|exclusion| HardExclusionPreviewItemResponse {
            display_path: exclusion.display_path.clone(),
            entry_kind: scope_exclusion_kind_name(exclusion.kind),
            disposition: "will_add",
        })
        .collect();
    let pending = PendingHardExclusionPreview {
        scope_id,
        base_policy_revision: binding.revision,
        expires_at_unix_ms,
        expires_at,
        entry_kind,
        prepared,
    };
    let preview_id = lock_hard_exclusion_previews(state)?.insert_at(pending, Instant::now());
    Ok(HardExclusionPreviewResponse {
        api_version: "deskgraph.hard-exclusion-preview.v1",
        preview_id,
        scope_id,
        base_policy_revision: binding.revision,
        expires_at_unix_ms,
        items,
        impact,
        confirmable,
        source_files_will_change: false,
    })
}

#[tauri::command]
fn confirm_hard_exclusion_preview(
    state: State<'_, ManifestState>,
    preview_id: String,
) -> Result<HardExclusionCommitResponse, String> {
    confirm_hard_exclusion_preview_for_state(&state, preview_id)
}

fn confirm_hard_exclusion_preview_for_state(
    state: &ManifestState,
    preview_id: String,
) -> Result<HardExclusionCommitResponse, String> {
    if preview_id.is_empty() || preview_id.len() > 128 {
        return Err("hard_exclusion_preview_not_found".to_string());
    }
    let pending = lock_hard_exclusion_previews(state)?
        .take_for_confirmation_at(&preview_id, Instant::now())
        .map_err(str::to_string)?;
    require_active_scope(state, pending.scope_id)?;
    let _path_read_guard = lock_path_reads(state);
    let _database_guard = lock_database(state)?;
    let active_access = {
        let scope_guard = lock_scope_accesses(state)?;
        Arc::clone(
            scope_guard
                .get(&pending.scope_id)
                .ok_or("scope_reauthorization_required")?,
        )
    };
    if pending.expires_at <= Instant::now() {
        return Err("hard_exclusion_preview_expired".to_string());
    }
    let now_unix_ms = hard_exclusion_now_unix_ms()?;
    let mut database =
        ManifestDatabase::open(&state.database_path).map_err(|error| error.code())?;
    let cross_process_policy_fence = database
        .acquire_scope_filesystem_revocation_fence(pending.scope_id)
        .map_err(|error| error.code())?;
    let binding = database
        .bind_scope_policy_revision(pending.scope_id)
        .map_err(|error| error.code())?;
    if binding.revision != pending.base_policy_revision {
        return Err("scope_policy_changed".to_string());
    }
    let selections = pending
        .prepared
        .exclusions
        .iter()
        .map(|exclusion| ScopeExclusionSelection {
            requested_path: exclusion.resolved_path.as_path(),
        })
        .collect::<Vec<_>>();
    let revalidated = prepare_scope_exclusion_batch_with_revocation_fence(
        &database,
        &cross_process_policy_fence,
        pending.scope_id,
        &selections,
    )
    .map_err(|error| error.code().to_string())?;
    if revalidated.exclusions.len() != pending.prepared.exclusions.len()
        || revalidated
            .exclusions
            .iter()
            .zip(&pending.prepared.exclusions)
            .any(|(current, expected)| !same_prepared_hard_exclusion(current, expected))
        || revalidated
            .exclusions
            .iter()
            .any(|exclusion| !hard_exclusion_kind_matches(pending.entry_kind, exclusion.kind))
    {
        return Err("hard_exclusion_selection_changed".to_string());
    }
    let writes = revalidated
        .exclusions
        .iter()
        .map(|exclusion| ScopeExclusionWrite {
            kind: exclusion.kind,
            path_raw: &exclusion.path_raw,
            path_key: &exclusion.path_key,
            display_path: &exclusion.display_path,
            identity_kind: &exclusion.identity_kind,
            identity_key: &exclusion.identity_key,
        })
        .collect::<Vec<_>>();
    // `apply_scope_exclusion_batch` repeats policy binding and blocker checks
    // inside its BEGIN IMMEDIATE transaction; this is the authoritative race
    // fence, not the advisory preview above.
    let applied = database
        .apply_scope_exclusion_batch_with_fence(
            &cross_process_policy_fence,
            binding,
            &writes,
            now_unix_ms,
        )
        .map_err(|error| error.code().to_string())?;
    let purge = map_hard_exclusion_impact(applied.purged);
    let response = HardExclusionCommitResponse {
        api_version: "deskgraph.hard-exclusion-commit.v1",
        scope_id: applied.policy.scope_id,
        policy_revision: applied.policy.revision,
        exclusions: applied.receipt.exclusions_added,
        purge,
        source_files_changed: false,
        automatic_scans_started: 0,
        automatic_extractions_started: 0,
    };
    drop(active_access);
    drop(_database_guard);
    drop(_path_read_guard);
    // Do not wake the ordinary Watch coordinator here: its wake signal can
    // enter a general reconciliation cycle, which would contradict this IPC
    // contract's explicit zero automatic scans/extractions. The committed
    // policy revision is already a durable fail-closed fence; Watch observes
    // it on its normal cycle without treating this policy change as a scan.
    info!(
        event = "hard_exclusion_policy_applied",
        scope_id = response.scope_id,
        policy_revision = response.policy_revision,
        exclusion_count = response.exclusions,
        source_files_changed = false,
        automatic_scans_started = 0_u64,
        automatic_extractions_started = 0_u64
    );
    Ok(response)
}

#[tauri::command]
fn discard_hard_exclusion_preview(
    state: State<'_, ManifestState>,
    preview_id: String,
) -> Result<(), String> {
    if preview_id.len() <= 128 {
        lock_hard_exclusion_previews(&state)?.discard(&preview_id);
    }
    Ok(())
}

fn hard_exclusion_kind_matches(
    entry_kind: HardExclusionEntryKind,
    prepared_kind: ScopeExclusionKind,
) -> bool {
    matches!(
        (entry_kind, prepared_kind),
        (HardExclusionEntryKind::File, ScopeExclusionKind::File)
            | (HardExclusionEntryKind::Folder, ScopeExclusionKind::Folder)
    )
}

fn same_prepared_hard_exclusion(
    current: &deskgraph_scanner::PreparedScopeExclusion,
    expected: &deskgraph_scanner::PreparedScopeExclusion,
) -> bool {
    current.resolved_path == expected.resolved_path
        && current.path_raw == expected.path_raw
        && current.path_key == expected.path_key
        && current.kind == expected.kind
        && current.identity_kind == expected.identity_kind
        && current.identity_key == expected.identity_key
}

#[tauri::command]
async fn select_and_authorize_scopes(
    app: AppHandle,
    state: State<'_, ManifestState>,
) -> Result<Option<Vec<AuthorizedScope>>, String> {
    let Some(selected) = app.dialog().file().blocking_pick_folders() else {
        return Ok(None);
    };
    if selected.is_empty() {
        return Err("coverage_set_empty".to_string());
    }
    if selected.len() > MAX_COVERAGE_ROOTS_PER_SELECTION {
        return Err("coverage_set_too_large".to_string());
    }
    let selected_paths = selected
        .into_iter()
        .map(|path| {
            path.into_path()
                .map_err(|_| "scope_selection_invalid".to_string())
        })
        .collect::<Result<Vec<_>, _>>()?;
    let prepared = selected_paths
        .iter()
        .map(|path| prepare_selected_scope(path).map_err(str::to_string))
        .collect::<Result<Vec<_>, _>>()?;
    let requests = prepared
        .iter()
        .map(|prepared| CoverageRootAuthorizationRequest {
            requested_path: &prepared.resolved_path,
            grant_platform: prepared.platform,
            opaque_grant: &prepared.opaque_grant,
        })
        .collect::<Vec<_>>();
    let _database_guard = lock_database(&state)?;
    let mut active_accesses = lock_scope_accesses(&state)?;
    active_accesses
        .try_reserve(prepared.len())
        .map_err(|_| "scope_access_registry_capacity_exceeded".to_string())?;
    let scopes = authorize_coverage_roots_with_access_grants_at(&state.database_path, &requests)
        .map_err(str::to_string)?;
    drop(requests);
    let mut replaced_accesses = Vec::new();
    for (scope, prepared) in scopes.iter().zip(prepared) {
        if let Some(replaced) = active_accesses.insert(scope.id, Arc::new(prepared.access)) {
            replaced_accesses.push(replaced);
        }
    }
    drop(active_accesses);
    drop(_database_guard);
    drop(replaced_accesses);
    wake_watch_runtime(&state);
    info!(
        event = "coverage_roots_authorized",
        authorized_root_count = scopes.len()
    );
    Ok(Some(scopes))
}

#[tauri::command]
fn create_manifest_scan(
    state: State<'_, ManifestState>,
    scope_id: i64,
) -> Result<ScanJobProgress, String> {
    require_active_scope(&state, scope_id)?;
    let _database_guard = lock_database(&state)?;
    let progress =
        create_manifest_scan_at(&state.database_path, scope_id).map_err(str::to_string)?;
    drop(_database_guard);
    wake_watch_runtime(&state);
    log_scan_progress("metadata_scan_created", &progress);
    Ok(progress)
}

#[tauri::command]
async fn run_manifest_scan(
    state: State<'_, ManifestState>,
    job_id: i64,
) -> Result<ScanJobProgress, String> {
    let pending = scan_job_status_at(&state.database_path, job_id).map_err(str::to_string)?;
    require_active_scope(&state, pending.scope_id)?;
    let database_path = state.database_path.clone();
    let database_gate = Arc::clone(&state.database_gate);
    let watch_wake = state.watch_wake.clone();
    let progress = tauri::async_runtime::spawn_blocking(move || {
        run_manifest_scan_with_gate(&database_path, job_id, &database_gate, &watch_wake)
            .map_err(str::to_string)
    })
    .await
    .map_err(|_| "scan_worker_failed".to_string())??;
    log_scan_progress("metadata_scan_runner_stopped", &progress);
    Ok(progress)
}

#[tauri::command]
fn scan_job_status(
    state: State<'_, ManifestState>,
    job_id: i64,
) -> Result<ScanJobProgress, String> {
    let progress = scan_job_status_at(&state.database_path, job_id).map_err(str::to_string)?;
    require_active_scope(&state, progress.scope_id)?;
    Ok(progress)
}

#[tauri::command]
fn recent_scan_jobs(state: State<'_, ManifestState>) -> Result<Vec<ScanJobProgress>, String> {
    let active = active_scope_ids(&state)?;
    recent_scan_jobs_at(&state.database_path)
        .map(|jobs| {
            jobs.into_iter()
                .filter(|job| active.contains(&job.scope_id))
                .collect()
        })
        .map_err(str::to_string)
}

/// Returns durable per-scope scan readiness instead of asking the WebView to
/// infer it from the bounded recent-job history.
#[tauri::command]
fn project_scope_has_completed_scan(
    state: State<'_, ManifestState>,
    scope_id: i64,
) -> Result<bool, String> {
    require_active_scope(&state, scope_id)?;
    project_scope_has_completed_scan_at(&state.database_path, scope_id).map_err(str::to_string)
}

#[tauri::command]
fn content_extraction_stats(state: State<'_, ManifestState>) -> Result<ExtractionStats, String> {
    content_extraction_stats_with_active_access_grants_at(&state.database_path)
        .map_err(str::to_string)
}

#[tauri::command]
fn recent_content_extractions(
    state: State<'_, ManifestState>,
) -> Result<Vec<ExtractionJobProgress>, String> {
    let active = active_scope_ids(&state)?;
    recent_content_extractions_at(&state.database_path)
        .map(|jobs| {
            jobs.into_iter()
                .filter(|job| active.contains(&job.scope_id))
                .collect()
        })
        .map_err(str::to_string)
}

/// Queues bounded extraction for one explicitly selected, already-scanned
/// text-bearing node. The WebView supplies only stable manifest identifiers;
/// current access, exclusion policy, media kind, and source identity are all
/// checked again by Rust before text can be published.
#[tauri::command]
fn create_content_extraction_job(
    state: State<'_, ManifestState>,
    scope_id: i64,
    node_id: i64,
) -> Result<ExtractionJobProgress, String> {
    require_active_scope(&state, scope_id)?;
    let progress =
        create_content_extraction_job_for_database(&state.database_path, scope_id, node_id)
            .map_err(str::to_string)?;
    log_content_extraction_progress("content_extraction_created", &progress);
    Ok(progress)
}

/// Runs only a previously-created content job. No path, filename, query, or
/// document text crosses this IPC boundary.
#[tauri::command]
async fn run_content_extraction_job(
    state: State<'_, ManifestState>,
    job_id: i64,
) -> Result<ExtractionJobProgress, String> {
    let pending =
        require_content_extraction_job(&state.database_path, job_id).map_err(str::to_string)?;
    require_active_scope(&state, pending.scope_id)?;
    let database_path = state.database_path.clone();
    let path_read_gate = Arc::clone(&state.path_read_gate);
    let scope_accesses = Arc::clone(&state.scope_accesses);
    let scope_id = pending.scope_id;
    let progress = tauri::async_runtime::spawn_blocking(move || {
        with_active_scope_read_fence(&path_read_gate, &scope_accesses, scope_id, || {
            run_content_extraction_job_at(&database_path, job_id).map_err(str::to_string)
        })
    })
    .await
    .map_err(|_| "content_extraction_worker_failed".to_string())??;
    log_content_extraction_progress("content_extraction_runner_stopped", &progress);
    Ok(progress)
}

/// Queues OCR for an already-scanned image. The core service revalidates the
/// authorized scope and node identity; callers never provide a filesystem path.
#[tauri::command]
fn create_screenshot_ocr_job(
    state: State<'_, ManifestState>,
    scope_id: i64,
    node_id: i64,
) -> Result<ExtractionJobProgress, String> {
    require_active_scope(&state, scope_id)?;
    let progress = create_screenshot_ocr_job_for_database(&state.database_path, scope_id, node_id)
        .map_err(str::to_string)?;
    log_screenshot_ocr_progress("screenshot_ocr_created", &progress);
    Ok(progress)
}

/// Runs only a previously-created screenshot OCR job. It deliberately accepts
/// no path or content and is moved off Tauri's async runtime while native OCR runs.
#[tauri::command]
async fn run_screenshot_ocr_job(
    state: State<'_, ManifestState>,
    job_id: i64,
) -> Result<ExtractionJobProgress, String> {
    let pending =
        require_screenshot_ocr_job(&state.database_path, job_id).map_err(str::to_string)?;
    require_active_scope(&state, pending.scope_id)?;
    let database_path = state.database_path.clone();
    let path_read_gate = Arc::clone(&state.path_read_gate);
    let scope_accesses = Arc::clone(&state.scope_accesses);
    let scope_id = pending.scope_id;
    let progress = tauri::async_runtime::spawn_blocking(move || {
        with_active_scope_read_fence(&path_read_gate, &scope_accesses, scope_id, || {
            run_screenshot_ocr_job_at(&database_path, job_id).map_err(str::to_string)
        })
    })
    .await
    .map_err(|_| "screenshot_ocr_worker_failed".to_string())??;
    log_screenshot_ocr_progress("screenshot_ocr_runner_stopped", &progress);
    Ok(progress)
}

#[tauri::command]
fn screenshot_ocr_job_status(
    state: State<'_, ManifestState>,
    job_id: i64,
) -> Result<ExtractionJobProgress, String> {
    let progress =
        screenshot_ocr_job_status_at(&state.database_path, job_id).map_err(str::to_string)?;
    require_active_scope(&state, progress.scope_id)?;
    Ok(progress)
}

/// Looks up only the most actionable screenshot OCR job for one already-scanned
/// node. This avoids exposing a filesystem path or document text while letting
/// the desktop recover interrupted work that has fallen outside the recent-jobs
/// dashboard window.
#[tauri::command]
fn screenshot_ocr_job_for_node(
    state: State<'_, ManifestState>,
    scope_id: i64,
    node_id: i64,
) -> Result<Option<ExtractionJobProgress>, String> {
    require_active_scope(&state, scope_id)?;
    screenshot_ocr_job_for_node_at(&state.database_path, scope_id, node_id).map_err(str::to_string)
}

#[tauri::command]
fn cancel_screenshot_ocr_job(
    state: State<'_, ManifestState>,
    job_id: i64,
) -> Result<ExtractionJobProgress, String> {
    let pending =
        require_screenshot_ocr_job(&state.database_path, job_id).map_err(str::to_string)?;
    require_active_scope(&state, pending.scope_id)?;
    let progress =
        cancel_screenshot_ocr_job_at(&state.database_path, job_id).map_err(str::to_string)?;
    log_screenshot_ocr_progress("screenshot_ocr_cancel_requested", &progress);
    Ok(progress)
}

/// Re-queues only an interrupted screenshot OCR job. The client supplies no
/// filesystem path or extracted content; the core service revalidates source
/// identity before returning it to the queue.
#[tauri::command]
fn resume_screenshot_ocr_job(
    state: State<'_, ManifestState>,
    job_id: i64,
) -> Result<ExtractionJobProgress, String> {
    let pending =
        require_screenshot_ocr_job(&state.database_path, job_id).map_err(str::to_string)?;
    require_active_scope(&state, pending.scope_id)?;
    let progress =
        resume_screenshot_ocr_job_at(&state.database_path, job_id).map_err(str::to_string)?;
    log_screenshot_ocr_progress("screenshot_ocr_resume_requested", &progress);
    Ok(progress)
}

#[tauri::command]
fn recent_watch_events(state: State<'_, ManifestState>) -> Result<Vec<WatchEventProgress>, String> {
    let active = active_scope_ids(&state)?;
    recent_watch_events_for_database(&state.database_path)
        .map(|events| {
            events
                .into_iter()
                .filter(|event| active.contains(&event.scope_id))
                .collect()
        })
        .map_err(str::to_string)
}

#[tauri::command]
fn watch_runtime_status(state: State<'_, ManifestState>) -> Result<WatchRuntimeStatus, String> {
    lock_watch_status(&state.watch_status).map(|status| status.clone())
}

#[tauri::command]
fn create_rename_preview(
    state: State<'_, ManifestState>,
    scope_id: i64,
    source_path: String,
    new_name: String,
) -> Result<ActionPlanPreview, String> {
    require_active_scope(&state, scope_id)?;
    let preview = create_rename_preview_for_database(
        &state.database_path,
        scope_id,
        Path::new(&source_path),
        &new_name,
    )
    .map_err(str::to_string)?;
    info!(
        event = "rename_preview_created",
        plan_id = preview.plan_id,
        scope_id = preview.scope_id,
        node_id = preview.node_id,
        execution_strategy = ?preview.execution_strategy
    );
    Ok(preview)
}

#[tauri::command]
fn recent_action_plans(state: State<'_, ManifestState>) -> Result<Vec<ActionPlanSummary>, String> {
    let active = active_scope_ids(&state)?;
    recent_action_plans_for_database(&state.database_path)
        .map(|plans| {
            plans
                .into_iter()
                .filter(|plan| active.contains(&plan.scope_id))
                .collect()
        })
        .map_err(str::to_string)
}

/// Revalidates existing immutable evidence for one explicitly selected scope.
/// The response is path-free and cannot authorize or execute a file action.
#[tauri::command]
async fn refresh_cleanup_inbox(
    state: State<'_, ManifestState>,
    scope_id: i64,
) -> Result<SmartCleanupInbox, String> {
    require_active_scope(&state, scope_id)?;
    let database_path = state.database_path.clone();
    let database_gate = Arc::clone(&state.database_gate);
    let inbox = tauri::async_runtime::spawn_blocking(move || {
        let _database_guard = database_gate
            .lock()
            .map_err(|_| "manifest_writer_gate_poisoned".to_string())?;
        refresh_smart_cleanup_inbox_at(&database_path, scope_id)
            .map_err(|error| error.code().to_string())
    })
    .await
    .map_err(|_| "smart_cleanup_inbox_worker_failed".to_string())??;
    info!(
        event = "smart_cleanup_inbox_refreshed",
        scope_id = inbox.scope_id,
        item_count = inbox.items.len(),
        evaluated_source_count = inbox.evaluated_source_count,
        not_current_source_count = inbox.not_current_source_count,
        evaluation_complete = inbox.evaluation_complete,
        action_authorized = false
    );
    Ok(inbox)
}

/// Returns transient, path-bearing member detail only after an explicit local
/// review request. Paths are never persisted in Cleanup plans, history, logs,
/// preferences, or the path-free Inbox.
#[tauri::command]
async fn get_cleanup_source_detail(
    state: State<'_, ManifestState>,
    scope_id: i64,
    source_kind: SmartCleanupSourceKind,
    source_id: i64,
    source_observation_id: i64,
) -> Result<CleanupSourceDetail, String> {
    require_active_scope(&state, scope_id)?;
    let database_path = state.database_path.clone();
    let database_gate = Arc::clone(&state.database_gate);
    let scope_accesses = Arc::clone(&state.scope_accesses);
    let detail = tauri::async_runtime::spawn_blocking(move || {
        let _database_guard = database_gate
            .lock()
            .map_err(|_| "manifest_writer_gate_poisoned".to_string())?;
        let scope_guard = scope_accesses
            .lock()
            .map_err(|_| "scope_access_registry_poisoned".to_string())?;
        if !scope_guard.contains_key(&scope_id) {
            return Err("scope_reauthorization_required".to_string());
        }
        cleanup_source_detail_at(
            &database_path,
            scope_id,
            source_kind,
            source_id,
            source_observation_id,
        )
        .map_err(|error| error.code().to_string())
    })
    .await
    .map_err(|_| "cleanup_source_detail_worker_failed".to_string())??;
    info!(
        event = "cleanup_source_detail_opened",
        scope_id = detail.scope_id,
        source_kind = ?detail.source_kind,
        source_id = detail.source_id,
        source_observation_id = detail.source_observation_id,
        member_count = detail.members.len(),
        action_authorized = false,
        execution_available = false
    );
    Ok(detail)
}

/// Creates one immutable Cleanup Preview from explicit member IDs. This
/// command performs no file mutation and has no confirmation, Trash, Delete,
/// Execute, recovery, or Undo companion.
#[tauri::command]
async fn create_cleanup_preview(
    state: State<'_, ManifestState>,
    scope_id: i64,
    source_kind: SmartCleanupSourceKind,
    source_id: i64,
    source_observation_id: i64,
    target_node_id: i64,
    keeper_node_id: Option<i64>,
) -> Result<CleanupActionPlanPreview, String> {
    require_active_scope(&state, scope_id)?;
    let database_path = state.database_path.clone();
    let database_gate = Arc::clone(&state.database_gate);
    let scope_accesses = Arc::clone(&state.scope_accesses);
    let preview = tauri::async_runtime::spawn_blocking(move || {
        let _database_guard = database_gate
            .lock()
            .map_err(|_| "manifest_writer_gate_poisoned".to_string())?;
        let scope_guard = scope_accesses
            .lock()
            .map_err(|_| "scope_access_registry_poisoned".to_string())?;
        if !scope_guard.contains_key(&scope_id) {
            return Err("scope_reauthorization_required".to_string());
        }
        create_cleanup_preview_at(
            &database_path,
            CleanupActionSelection {
                scope_id,
                source_kind,
                source_id,
                source_observation_id,
                keeper_node_id,
                target_node_id,
            },
        )
        .map_err(|error| error.code().to_string())
    })
    .await
    .map_err(|_| "cleanup_preview_worker_failed".to_string())??;
    info!(
        event = "cleanup_preview_created",
        plan_id = preview.plan_id,
        scope_id = preview.scope_id,
        source_kind = ?preview.source_kind,
        source_id = preview.source_id,
        source_observation_id = preview.source_observation_id,
        target_node_id = preview.target_node_id,
        keeper_node_id = preview.keeper_node_id,
        action_authorized = false,
        execution_available = false
    );
    Ok(preview)
}

/// Discovers bounded Project roots from the current manifest for one explicit
/// active scope. The response is path-free and creates no membership or file
/// action capability.
#[tauri::command]
async fn discover_projects(
    state: State<'_, ManifestState>,
    scope_id: i64,
) -> Result<ProjectDiscovery, String> {
    require_active_scope(&state, scope_id)?;
    let database_path = state.database_path.clone();
    let database_gate = Arc::clone(&state.database_gate);
    let scope_accesses = Arc::clone(&state.scope_accesses);
    let discovery = tauri::async_runtime::spawn_blocking(move || {
        let _database_guard = database_gate
            .lock()
            .map_err(|_| "manifest_writer_gate_poisoned".to_string())?;
        let scope_guard = scope_accesses
            .lock()
            .map_err(|_| "scope_access_registry_poisoned".to_string())?;
        if !scope_guard.contains_key(&scope_id) {
            return Err("scope_reauthorization_required".to_string());
        }
        discover_projects_at(&database_path, scope_id).map_err(|error| error.code().to_string())
    })
    .await
    .map_err(|_| "project_discovery_worker_failed".to_string())??;
    info!(
        event = "project_discovery_completed",
        scope_id = discovery.scope_id,
        candidate_count = discovery.candidates.len(),
        evaluated_root_count = discovery.evaluated_root_count,
        evaluation_complete = discovery.evaluation_complete,
        automatic_membership_created = false,
        file_actions_available = false
    );
    Ok(discovery)
}

/// Resolves one explicitly selected Project root and current marker evidence.
/// The path-bearing response is transient and never written to ordinary logs.
#[tauri::command]
async fn get_project_candidate_detail(
    state: State<'_, ManifestState>,
    scope_id: i64,
    project_id: i64,
) -> Result<ProjectCandidateDetail, String> {
    require_active_scope(&state, scope_id)?;
    let database_path = state.database_path.clone();
    let database_gate = Arc::clone(&state.database_gate);
    let scope_accesses = Arc::clone(&state.scope_accesses);
    let detail = tauri::async_runtime::spawn_blocking(move || {
        let _database_guard = database_gate
            .lock()
            .map_err(|_| "manifest_writer_gate_poisoned".to_string())?;
        let scope_guard = scope_accesses
            .lock()
            .map_err(|_| "scope_access_registry_poisoned".to_string())?;
        if !scope_guard.contains_key(&scope_id) {
            return Err("scope_reauthorization_required".to_string());
        }
        project_candidate_detail_at(&database_path, scope_id, project_id)
            .map_err(|error| error.code().to_string())
    })
    .await
    .map_err(|_| "project_candidate_detail_worker_failed".to_string())??;
    info!(
        event = "project_candidate_detail_opened",
        scope_id = detail.candidate.scope_id,
        project_id = detail.candidate.project_id,
        root_folder_node_id = detail.candidate.root_folder_node_id,
        state = ?detail.candidate.state,
        current_evidence = true,
        automatic_membership_created = false,
        file_actions_available = false
    );
    Ok(detail)
}

/// Appends one explicit local correction after current marker evidence is
/// revalidated. Accept/reject never creates membership or mutates files.
#[tauri::command]
async fn decide_project_candidate(
    state: State<'_, ManifestState>,
    scope_id: i64,
    project_id: i64,
    decision: ProjectDecisionKind,
) -> Result<ProjectCandidateDetail, String> {
    require_active_scope(&state, scope_id)?;
    let database_path = state.database_path.clone();
    let database_gate = Arc::clone(&state.database_gate);
    let scope_accesses = Arc::clone(&state.scope_accesses);
    let detail = tauri::async_runtime::spawn_blocking(move || {
        let _database_guard = database_gate
            .lock()
            .map_err(|_| "manifest_writer_gate_poisoned".to_string())?;
        let scope_guard = scope_accesses
            .lock()
            .map_err(|_| "scope_access_registry_poisoned".to_string())?;
        if !scope_guard.contains_key(&scope_id) {
            return Err("scope_reauthorization_required".to_string());
        }
        decide_current_project_candidate_at(&database_path, scope_id, project_id, decision)
            .map_err(|error| error.code().to_string())
    })
    .await
    .map_err(|_| "project_candidate_decision_worker_failed".to_string())??;
    info!(
        event = "project_candidate_decision_recorded",
        scope_id = detail.candidate.scope_id,
        project_id = detail.candidate.project_id,
        root_folder_node_id = detail.candidate.root_folder_node_id,
        state = ?detail.candidate.state,
        automatic_membership_created = false,
        file_actions_available = false
    );
    Ok(detail)
}

#[tauri::command]
fn search_local(
    state: State<'_, ManifestState>,
    query: String,
    filters: SearchFilters,
    limit: Option<u32>,
) -> Result<SearchResponse, String> {
    if let Some(scope_id) = filters.scope_id {
        require_active_scope(&state, scope_id)?;
    }
    let active = active_scope_ids(&state)?;
    let mut response =
        search_local_at(&state.database_path, &query, &filters, limit).map_err(str::to_string)?;
    response
        .results
        .retain(|result| active.contains(&result.scope_id));
    response.result_count = u64::try_from(response.results.len())
        .map_err(|_| "search_result_count_out_of_range".to_string())?;
    info!(
        event = "local_search_completed",
        scope_id = filters.scope_id,
        result_count = response.result_count,
        elapsed_ms = response.elapsed_ms,
        filters_applied = filters.extension.is_some()
            || filters.folder_node_id.is_some()
            || filters.modified_since_unix_seconds.is_some()
            || filters.modified_before_unix_seconds.is_some()
            || filters.source != SearchSourceFilter::All,
        mode = "lexical"
    );
    Ok(response)
}

/// Returns path-bearing folder choices only for a direct, active user request.
/// The response is bounded and its redacted Debug implementation keeps paths
/// out of ordinary diagnostics.
#[tauri::command]
fn list_search_folders(
    state: State<'_, ManifestState>,
    scope_id: i64,
) -> Result<SearchFolderListResponse, String> {
    require_active_scope(&state, scope_id)?;
    let response = list_search_folders_for_database(
        &state.database_path,
        scope_id,
        Some(SEARCH_FOLDER_LIST_LIMIT),
    )
    .map_err(str::to_string)?;
    info!(
        event = "search_folders_listed",
        scope_id,
        folder_count = response.folder_count,
        truncated = response.truncated
    );
    Ok(response)
}

#[tauri::command]
fn pause_manifest_scan(
    state: State<'_, ManifestState>,
    job_id: i64,
) -> Result<ScanJobProgress, String> {
    let pending = scan_job_status_at(&state.database_path, job_id).map_err(str::to_string)?;
    require_active_scope(&state, pending.scope_id)?;
    let _database_guard = lock_database(&state)?;
    let progress = pause_manifest_scan_at(&state.database_path, job_id).map_err(str::to_string)?;
    drop(_database_guard);
    wake_watch_runtime(&state);
    log_scan_progress("metadata_scan_pause_requested", &progress);
    Ok(progress)
}

#[tauri::command]
fn resume_manifest_scan(
    state: State<'_, ManifestState>,
    job_id: i64,
) -> Result<ScanJobProgress, String> {
    let pending = scan_job_status_at(&state.database_path, job_id).map_err(str::to_string)?;
    require_active_scope(&state, pending.scope_id)?;
    let _database_guard = lock_database(&state)?;
    let progress = resume_manifest_scan_at(&state.database_path, job_id).map_err(str::to_string)?;
    drop(_database_guard);
    wake_watch_runtime(&state);
    log_scan_progress("metadata_scan_resumed", &progress);
    Ok(progress)
}

fn initialize_manifest(path: &Path) -> Result<(), &'static str> {
    ManifestDatabase::open(path)
        .map(|_| ())
        .map_err(|error| error.code())
}

fn restore_scope_access_registry(
    path: &Path,
) -> Result<HashMap<i64, ActiveScopeAccess>, &'static str> {
    let database = ManifestDatabase::open(path).map_err(|error| error.code())?;
    let grants = database
        .list_active_scope_grants()
        .map_err(|error| error.code())?;
    let mut restored_accesses = HashMap::new();

    for grant in grants {
        let _read_fence = match database.acquire_scope_filesystem_read_fence(grant.scope_id) {
            Ok(fence) => fence,
            Err(error)
                if matches!(
                    error.code(),
                    "scope_access_grant_not_active"
                        | "scope_access_grant_not_found"
                        | "scope_policy_revision_stale"
                ) =>
            {
                continue;
            }
            Err(error) => return Err(error.code()),
        };
        let restored = match restore_scope_access(&grant.platform, &grant.opaque_grant) {
            Ok(restored) => restored,
            Err(error_code) => {
                database
                    .mark_scope_access_grant_needs_reauthorization(grant.scope_id)
                    .map_err(|error| error.code())?;
                info!(
                    event = "scope_access_restore_failed",
                    scope_id = grant.scope_id,
                    error_code
                );
                continue;
            }
        };
        let canonical_root = match validated_scope_root(&database, grant.scope_id) {
            Ok(root) => root,
            Err(error) => {
                database
                    .mark_scope_access_grant_needs_reauthorization(grant.scope_id)
                    .map_err(|database_error| database_error.code())?;
                info!(
                    event = "scope_access_restore_failed",
                    scope_id = grant.scope_id,
                    error_code = error.code()
                );
                continue;
            }
        };
        if let Some(resolved_path) = restored.resolved_path.as_deref() {
            let resolved = match std::fs::canonicalize(resolved_path) {
                Ok(resolved) => resolved,
                Err(_) => {
                    database
                        .mark_scope_access_grant_needs_reauthorization(grant.scope_id)
                        .map_err(|error| error.code())?;
                    info!(
                        event = "scope_access_restore_failed",
                        scope_id = grant.scope_id,
                        error_code = "scope_canonicalization_failed"
                    );
                    continue;
                }
            };
            if comparison_key(&resolved) != comparison_key(&canonical_root) {
                database
                    .mark_scope_access_grant_needs_reauthorization(grant.scope_id)
                    .map_err(|error| error.code())?;
                info!(
                    event = "scope_access_restore_failed",
                    scope_id = grant.scope_id,
                    error_code = "authorized_scope_identity_changed"
                );
                continue;
            }
        }
        if let Some(refreshed_grant) = restored.refreshed_grant.as_deref() {
            database
                .upsert_scope_access_grant(grant.scope_id, &grant.platform, refreshed_grant)
                .map_err(|error| error.code())?;
        }
        restored_accesses.insert(grant.scope_id, restored.access);
    }

    Ok(restored_accesses)
}

#[cfg(test)]
fn health_at(path: &Path) -> Result<HealthReport, &'static str> {
    let stats = manifest_status_at(path)?;
    let scope_count = u32::try_from(stats.authorized_scope_count)
        .map_err(|_| "authorized_scope_count_out_of_range")?;
    Ok(collect_health_with_manifest(scope_count))
}

#[cfg(test)]
fn manifest_status_at(path: &Path) -> Result<ManifestStats, &'static str> {
    ManifestDatabase::open(path)
        .and_then(|database| database.stats())
        .map_err(|error| error.code())
}

fn manifest_status_with_active_access_grants_at(
    path: &Path,
) -> Result<ManifestStats, &'static str> {
    ManifestDatabase::open(path)
        .and_then(|database| database.stats_with_active_access_grants())
        .map_err(|error| error.code())
}

fn authorized_scopes_at(path: &Path) -> Result<Vec<AuthorizedScope>, &'static str> {
    ManifestDatabase::open(path)
        .and_then(|database| database.list_scopes())
        .map_err(|error| error.code())
}

#[cfg(test)]
fn authorize_scope_at(path: &Path, requested_path: &Path) -> Result<AuthorizedScope, &'static str> {
    // Test fixtures still need a durable native grant to exercise the same
    // scan admission path as production. Deliberately drop the prepared live
    // access at return: tests that construct a Desktop state without adding
    // it to `scope_accesses` continue to prove commands fail closed without a
    // runtime capability.
    let prepared = prepare_selected_scope(requested_path)?;
    authorize_scope_with_access_grant_at(
        path,
        &prepared.resolved_path,
        prepared.platform,
        &prepared.opaque_grant,
    )
}

#[cfg(test)]
fn authorize_scope_with_access_grant_at(
    path: &Path,
    requested_path: &Path,
    grant_platform: &str,
    opaque_grant: &[u8],
) -> Result<AuthorizedScope, &'static str> {
    let mut database = ManifestDatabase::open(path).map_err(|error| error.code())?;
    authorize_scope_with_access_grant(&mut database, requested_path, grant_platform, opaque_grant)
        .map_err(|error| error.code())
}

fn authorize_coverage_roots_with_access_grants_at(
    path: &Path,
    requests: &[CoverageRootAuthorizationRequest<'_>],
) -> Result<Vec<AuthorizedScope>, &'static str> {
    let mut database = ManifestDatabase::open(path).map_err(|error| error.code())?;
    authorize_coverage_roots_with_access_grants(&mut database, requests)
        .map_err(|error| error.code())
}

fn create_manifest_scan_at(path: &Path, scope_id: i64) -> Result<ScanJobProgress, &'static str> {
    let mut database = ManifestDatabase::open(path).map_err(|error| error.code())?;
    create_scan_job_with_active_grant(&mut database, scope_id).map_err(|error| error.code())
}

#[cfg(test)]
fn run_manifest_scan_at(path: &Path, job_id: i64) -> Result<ScanJobProgress, &'static str> {
    let mut database = ManifestDatabase::open(path).map_err(|error| error.code())?;
    run_scan_job_to_terminal(&mut database, job_id).map_err(|error| error.code())
}

fn run_manifest_scan_with_gate(
    path: &Path,
    job_id: i64,
    database_gate: &Mutex<()>,
    watch_wake: &SyncSender<()>,
) -> Result<ScanJobProgress, &'static str> {
    let mut database = {
        let _database_guard = database_gate
            .lock()
            .map_err(|_| "manifest_writer_gate_poisoned")?;
        ManifestDatabase::open(path).map_err(|error| error.code())?
    };
    loop {
        let progress = {
            let _database_guard = database_gate
                .lock()
                .map_err(|_| "manifest_writer_gate_poisoned")?;
            run_scan_job_batch_with_active_grant(&mut database, job_id, FOREGROUND_SCAN_BATCH_SIZE)
                .map_err(|error| error.code())?
        };
        notify_watch_wake(watch_wake);
        if progress.is_terminal()
            || matches!(
                progress.status,
                ScanStatus::Paused | ScanStatus::Interrupted
            )
        {
            return Ok(progress);
        }
        thread::yield_now();
    }
}

fn scan_job_status_at(path: &Path, job_id: i64) -> Result<ScanJobProgress, &'static str> {
    ManifestDatabase::open(path)
        .and_then(|database| database.scan_job(job_id))
        .map_err(|error| error.code())
}

fn recent_scan_jobs_at(path: &Path) -> Result<Vec<ScanJobProgress>, &'static str> {
    ManifestDatabase::open(path)
        .and_then(|database| database.recent_scan_jobs())
        .map_err(|error| error.code())
}

fn project_scope_has_completed_scan_at(path: &Path, scope_id: i64) -> Result<bool, &'static str> {
    ManifestDatabase::open(path)
        .and_then(|database| database.scope_has_completed_scan(scope_id))
        .map_err(|error| error.code())
}

#[cfg(test)]
fn content_extraction_stats_at(path: &Path) -> Result<ExtractionStats, &'static str> {
    read_extraction_stats_at(path).map_err(|error| error.code())
}

fn content_extraction_stats_with_active_access_grants_at(
    path: &Path,
) -> Result<ExtractionStats, &'static str> {
    ManifestDatabase::open(path)
        .and_then(|database| database.extraction_stats_with_active_access_grants())
        .map_err(|error| error.code())
}

fn recent_content_extractions_at(path: &Path) -> Result<Vec<ExtractionJobProgress>, &'static str> {
    read_recent_extraction_jobs_at(path).map_err(|error| error.code())
}

fn create_content_extraction_job_for_database(
    path: &Path,
    scope_id: i64,
    node_id: i64,
) -> Result<ExtractionJobProgress, &'static str> {
    let database = ManifestDatabase::open(path).map_err(|error| error.code())?;
    let source = database
        .extractable_file(scope_id, node_id)
        .map_err(|error| error.code())?;
    let extension = Path::new(&source.path_key)
        .extension()
        .and_then(|extension| extension.to_str())
        .ok_or("extraction_media_kind_unsupported")?;
    let media_kind =
        media_kind_for_extension(extension).ok_or("extraction_media_kind_unsupported")?;
    if matches!(media_kind, MediaKind::Image(_)) {
        return Err("extraction_media_kind_unsupported");
    }
    drop(database);
    create_extraction_job_at(path, scope_id, node_id).map_err(|error| error.code())
}

fn run_content_extraction_job_at(
    path: &Path,
    job_id: i64,
) -> Result<ExtractionJobProgress, &'static str> {
    require_content_extraction_job(path, job_id)?;
    run_extraction_job_at(path, job_id, ExtractionLimits::default()).map_err(|error| error.code())
}

fn require_content_extraction_job(
    path: &Path,
    job_id: i64,
) -> Result<ExtractionJobProgress, &'static str> {
    let progress = extraction_job_at(path, job_id).map_err(|error| error.code())?;
    if progress.operation != ExtractionOperation::Content {
        return Err("content_extraction_job_required");
    }
    Ok(progress)
}

fn create_screenshot_ocr_job_for_database(
    path: &Path,
    scope_id: i64,
    node_id: i64,
) -> Result<ExtractionJobProgress, &'static str> {
    create_screenshot_ocr_job_at(path, scope_id, node_id).map_err(|error| error.code())
}

fn run_screenshot_ocr_job_at(
    path: &Path,
    job_id: i64,
) -> Result<ExtractionJobProgress, &'static str> {
    require_screenshot_ocr_job(path, job_id)?;
    run_extraction_job_at(path, job_id, ExtractionLimits::default()).map_err(|error| error.code())
}

fn screenshot_ocr_job_status_at(
    path: &Path,
    job_id: i64,
) -> Result<ExtractionJobProgress, &'static str> {
    require_screenshot_ocr_job(path, job_id)
}

fn screenshot_ocr_job_for_node_at(
    path: &Path,
    scope_id: i64,
    node_id: i64,
) -> Result<Option<ExtractionJobProgress>, &'static str> {
    ManifestDatabase::open(path)
        .and_then(|database| database.screenshot_ocr_job_for_node(scope_id, node_id))
        .map_err(|error| error.code())
}

fn cancel_screenshot_ocr_job_at(
    path: &Path,
    job_id: i64,
) -> Result<ExtractionJobProgress, &'static str> {
    require_screenshot_ocr_job(path, job_id)?;
    cancel_extraction_job_at(path, job_id).map_err(|error| error.code())
}

fn resume_screenshot_ocr_job_at(
    path: &Path,
    job_id: i64,
) -> Result<ExtractionJobProgress, &'static str> {
    require_screenshot_ocr_job(path, job_id)?;
    resume_extraction_job_at(path, job_id).map_err(|error| error.code())
}

fn require_screenshot_ocr_job(
    path: &Path,
    job_id: i64,
) -> Result<ExtractionJobProgress, &'static str> {
    let progress = extraction_job_at(path, job_id).map_err(|error| error.code())?;
    if progress.operation != ExtractionOperation::ScreenshotOcr {
        return Err("screenshot_ocr_job_required");
    }
    Ok(progress)
}

fn recent_watch_events_for_database(path: &Path) -> Result<Vec<WatchEventProgress>, &'static str> {
    read_recent_watch_events_at(path).map_err(|error| error.code())
}

fn create_rename_preview_for_database(
    database_path: &Path,
    scope_id: i64,
    source_path: &Path,
    new_name: &str,
) -> Result<ActionPlanPreview, &'static str> {
    create_rename_preview_at(database_path, scope_id, source_path, new_name)
        .map_err(|error| error.code())
}

fn recent_action_plans_for_database(path: &Path) -> Result<Vec<ActionPlanSummary>, &'static str> {
    recent_action_plans_at(path).map_err(|error| error.code())
}

fn search_local_at(
    path: &Path,
    query: &str,
    filters: &SearchFilters,
    limit: Option<u32>,
) -> Result<SearchResponse, &'static str> {
    run_search_at(
        path,
        SearchRequest {
            query,
            scope_id: filters.scope_id,
            folder_node_id: filters.folder_node_id,
            source: filters.source,
            extension: filters.extension.as_deref(),
            modified_since_unix_seconds: filters.modified_since_unix_seconds,
            modified_before_unix_seconds: filters.modified_before_unix_seconds,
            limit,
        },
    )
    .map_err(|error| error.code())
}

fn list_search_folders_for_database(
    path: &Path,
    scope_id: i64,
    limit: Option<u32>,
) -> Result<SearchFolderListResponse, &'static str> {
    ManifestDatabase::open(path)
        .map_err(|error| error.code())?
        .list_search_folders(scope_id, limit)
        .map_err(|error| error.code())
}

fn pause_manifest_scan_at(path: &Path, job_id: i64) -> Result<ScanJobProgress, &'static str> {
    let mut database = ManifestDatabase::open(path).map_err(|error| error.code())?;
    pause_scan_job(&mut database, job_id).map_err(|error| error.code())
}

fn resume_manifest_scan_at(path: &Path, job_id: i64) -> Result<ScanJobProgress, &'static str> {
    let mut database = ManifestDatabase::open(path).map_err(|error| error.code())?;
    resume_scan_job_with_active_grant(&mut database, job_id).map_err(|error| error.code())
}

fn log_scan_progress(event: &'static str, progress: &ScanJobProgress) {
    info!(
        event = event,
        scope_id = progress.scope_id,
        job_id = progress.job_id,
        status = ?progress.status,
        queued_entries = progress.queued_entries,
        processed_entries = progress.processed_entries,
        discovered_files = progress.discovered_files,
        discovered_folders = progress.discovered_folders,
        skipped_entries = progress.skipped_entries,
        issue_count = progress.issue_count,
        elapsed_ms = progress.elapsed_ms
    );
}

fn log_screenshot_ocr_progress(event: &'static str, progress: &ExtractionJobProgress) {
    info!(
        event = event,
        scope_id = progress.scope_id,
        job_id = progress.job_id,
        node_id = progress.node_id,
        status = ?progress.status,
        operation = ?progress.operation,
        output_bytes = progress.output_bytes,
        chunk_count = progress.chunk_count,
        elapsed_ms = progress.elapsed_ms,
        cancel_requested = progress.cancel_requested
    );
}

fn log_content_extraction_progress(event: &'static str, progress: &ExtractionJobProgress) {
    info!(
        event,
        scope_id = progress.scope_id,
        node_id = progress.node_id,
        job_id = progress.job_id,
        status = ?progress.status,
        chunk_count = progress.chunk_count,
        output_bytes = progress.output_bytes,
        error_code = progress.error_code
    );
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let _logger_installed = init_privacy_safe_logging(Service::Desktop);
    info!(event = "desktop_starting");

    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let app_data = app
                .path()
                .app_data_dir()
                .map_err(|_| "app_data_path_unavailable")?;
            let database_path = app_data.join("manifest.sqlite3");
            initialize_manifest(&database_path)?;
            let scope_accesses = restore_scope_access_registry(&database_path)?;
            app.manage(start_manifest_state_with_accesses(
                database_path,
                scope_accesses,
            ));
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            health,
            manifest_status,
            authorized_scopes,
            coverage_policy_detail,
            select_and_authorize_scopes,
            preview_scope_root_revocation,
            confirm_scope_root_revocation,
            discard_scope_root_revocation,
            select_hard_exclusions_preview,
            confirm_hard_exclusion_preview,
            discard_hard_exclusion_preview,
            create_manifest_scan,
            run_manifest_scan,
            scan_job_status,
            recent_scan_jobs,
            project_scope_has_completed_scan,
            content_extraction_stats,
            recent_content_extractions,
            create_content_extraction_job,
            run_content_extraction_job,
            create_screenshot_ocr_job,
            run_screenshot_ocr_job,
            screenshot_ocr_job_status,
            screenshot_ocr_job_for_node,
            cancel_screenshot_ocr_job,
            resume_screenshot_ocr_job,
            recent_watch_events,
            watch_runtime_status,
            create_rename_preview,
            recent_action_plans,
            refresh_cleanup_inbox,
            get_cleanup_source_detail,
            create_cleanup_preview,
            discover_projects,
            get_project_candidate_detail,
            decide_project_candidate,
            list_search_folders,
            search_local,
            pause_manifest_scan,
            resume_manifest_scan
        ])
        .run(tauri::generate_context!())
        .expect("DeskGraph desktop runtime failed");
}

#[cfg(test)]
mod tests {
    use super::*;
    use deskgraph_domain::{ExtractionOperation, ExtractionStatus, LifecycleState};
    use deskgraph_extractors::{ExtractionLimits, create_extraction_job_at, run_extraction_job_at};
    use deskgraph_watcher::{WatchPolicy, observe_watch_path_at};

    fn scanned_file_fixture(
        file_name: &str,
        contents: impl AsRef<[u8]>,
    ) -> (tempfile::TempDir, PathBuf, AuthorizedScope, i64) {
        let directory = tempfile::tempdir().expect("fixture root should exist");
        let database_path = directory.path().join("app-data/manifest.sqlite3");
        let scope_path = directory.path().join("authorized-private");
        let source_path = scope_path.join(file_name);
        std::fs::create_dir(&scope_path).expect("scope should create");
        std::fs::write(&source_path, contents).expect("file should create");

        initialize_manifest(&database_path).expect("manifest should initialize");
        let scope =
            authorize_scope_at(&database_path, &scope_path).expect("scope should authorize");
        let scan = create_manifest_scan_at(&database_path, scope.id).expect("scan should create");
        run_manifest_scan_at(&database_path, scan.job_id).expect("scan should complete");
        let database = ManifestDatabase::open(&database_path).expect("database should open");
        let node_id = database
            .node_id_for_path_key(
                scope.id,
                &deskgraph_scanner::comparison_key(
                    &std::fs::canonicalize(&source_path).expect("source should canonicalize"),
                ),
            )
            .expect("node lookup should pass")
            .expect("source node should exist");
        (directory, database_path, scope, node_id)
    }

    fn png_bytes(width: u32, height: u32, private_marker: &[u8]) -> Vec<u8> {
        let mut bytes = vec![0_u8; 32];
        bytes[..8].copy_from_slice(b"\x89PNG\r\n\x1a\n");
        bytes[8..12].copy_from_slice(&13_u32.to_be_bytes());
        bytes[12..16].copy_from_slice(b"IHDR");
        bytes[16..20].copy_from_slice(&width.to_be_bytes());
        bytes[20..24].copy_from_slice(&height.to_be_bytes());
        bytes.extend_from_slice(private_marker);
        bytes
    }

    #[test]
    fn macos_bundle_security_configuration_is_explicit_and_bounded() {
        let entitlements = include_str!("../Entitlements.plist");
        for required_key in [
            "com.apple.security.app-sandbox",
            "com.apple.security.files.user-selected.read-write",
            "com.apple.security.files.bookmarks.app-scope",
            "com.apple.security.network.client",
        ] {
            assert_eq!(
                entitlements.matches(required_key).count(),
                1,
                "required macOS entitlement must appear exactly once: {required_key}"
            );
        }
        assert!(!entitlements.contains("com.apple.security.network.server"));

        let config: serde_json::Value = serde_json::from_str(include_str!("../tauri.conf.json"))
            .expect("Tauri configuration should remain valid JSON");
        let security = &config["app"]["security"];
        let production_csp = security["csp"]
            .as_str()
            .expect("production CSP should be configured");
        let development_csp = security["devCsp"]
            .as_str()
            .expect("development CSP should be configured separately");

        assert!(!production_csp.contains("localhost:1420"));
        assert!(!production_csp.contains("ws://"));
        assert!(development_csp.contains("http://localhost:1420"));
        assert!(development_csp.contains("ws://localhost:1420"));
    }

    #[test]
    fn initialized_health_uses_the_shared_domain_contract() {
        let directory = tempfile::tempdir().expect("fixture root should exist");
        let database_path = directory.path().join("manifest.sqlite3");
        initialize_manifest(&database_path).expect("manifest should initialize");

        let report = health_at(&database_path).expect("health should load");
        assert_eq!(report.database.state, LifecycleState::Ready);
        assert_eq!(report.privacy.authorized_scope_count, 0);
    }

    #[test]
    fn native_coverage_grants_persist_restore_and_stay_out_of_ipc_scope_shape() {
        let directory = tempfile::tempdir().expect("fixture root should exist");
        let database_path = directory.path().join("app-data/manifest.sqlite3");
        let desktop_path = directory.path().join("Desktop");
        let documents_path = directory.path().join("Documents");
        std::fs::create_dir(&desktop_path).expect("Desktop should create");
        std::fs::create_dir(&documents_path).expect("Documents should create");
        initialize_manifest(&database_path).expect("manifest should initialize");

        let prepared = [desktop_path, documents_path]
            .iter()
            .map(|path| prepare_selected_scope(path).expect("selection should prepare"))
            .collect::<Vec<_>>();
        let requests = prepared
            .iter()
            .map(|prepared| CoverageRootAuthorizationRequest {
                requested_path: &prepared.resolved_path,
                grant_platform: prepared.platform,
                opaque_grant: &prepared.opaque_grant,
            })
            .collect::<Vec<_>>();
        let scopes = authorize_coverage_roots_with_access_grants_at(&database_path, &requests)
            .expect("coverage roots and grants should persist atomically");
        drop(requests);
        drop(prepared);

        let restored = restore_scope_access_registry(&database_path)
            .expect("stored selection should restore safely");
        assert_eq!(restored.len(), 2);
        assert!(scopes.iter().all(|scope| restored.contains_key(&scope.id)));
        let ordinary_scopes = authorized_scopes_at(&database_path).expect("scopes should load");
        let payload = serde_json::to_string(&ordinary_scopes).expect("scopes should serialize");
        assert!(!payload.contains("opaque_grant"));
        assert!(!payload.contains("access_grant"));
        assert_eq!(ordinary_scopes, scopes);
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn invalid_bookmark_is_durably_downgraded_to_reauthorization_required() {
        let directory = tempfile::tempdir().expect("fixture root should exist");
        let database_path = directory.path().join("app-data/manifest.sqlite3");
        let scope_path = directory.path().join("selected-scope");
        std::fs::create_dir(&scope_path).expect("scope should create");
        initialize_manifest(&database_path).expect("manifest should initialize");
        let scope = authorize_scope_with_access_grant_at(
            &database_path,
            &scope_path,
            std::env::consts::OS,
            b"not-a-security-scoped-bookmark",
        )
        .expect("fixture grant should persist");

        assert!(
            restore_scope_access_registry(&database_path)
                .expect("invalid grant should fail closed without blocking startup")
                .is_empty()
        );
        let database = ManifestDatabase::open(&database_path).expect("database should open");
        assert_eq!(
            database
                .scope_access_grant_state(scope.id)
                .expect("grant state should load"),
            deskgraph_database::ScopeAccessGrantState::NeedsReauthorization
        );
    }

    #[test]
    fn desktop_commands_fail_closed_for_a_completed_scope_without_a_live_grant() {
        let private_marker = "scope-grant-command-private";
        let image = png_bytes(64, 64, private_marker.as_bytes());
        let (_directory, database_path, scope, node_id) =
            scanned_file_fixture("private-screenshot.png", image);
        let scan_job = recent_scan_jobs_at(&database_path)
            .expect("scan jobs should load")
            .into_iter()
            .find(|job| job.scope_id == scope.id)
            .expect("completed scan job should exist");
        let ocr_job = create_screenshot_ocr_job_for_database(&database_path, scope.id, node_id)
            .expect("OCR fixture job should queue before runtime gating");
        ManifestDatabase::open(&database_path)
            .expect("fixture database should open")
            .mark_scope_access_grant_needs_reauthorization(scope.id)
            .expect("fixture grant should become inactive before Desktop startup");
        let source_path = Path::new(&scope.display_path).join("private-screenshot.png");

        let app = tauri::test::mock_builder()
            .manage(start_manifest_state(database_path.clone()))
            .build(tauri::test::mock_context(tauri::test::noop_assets()))
            .expect("mock Desktop app should build");

        assert!(
            authorized_scopes(app.state())
                .expect("scope list should load")
                .is_empty()
        );
        assert_eq!(
            project_scope_has_completed_scan(app.state(), scope.id)
                .expect_err("Project readiness should require a live grant"),
            "scope_reauthorization_required"
        );
        let manifest = manifest_status(app.state()).expect("manifest status should load");
        assert_eq!(manifest.authorized_scope_count, 0);
        assert_eq!(manifest.node_count, 0);
        assert_eq!(manifest.completed_scan_count, 0);
        let extraction =
            content_extraction_stats(app.state()).expect("extraction stats should load");
        assert_eq!(extraction.active_chunk_count, 0);
        assert_eq!(extraction.completed_job_count, 0);
        assert!(
            recent_scan_jobs(app.state())
                .expect("scan jobs should filter")
                .is_empty()
        );
        assert!(
            recent_content_extractions(app.state())
                .expect("extraction jobs should filter")
                .is_empty()
        );
        assert!(
            recent_watch_events(app.state())
                .expect("watch history should filter")
                .is_empty()
        );
        assert!(
            recent_action_plans(app.state())
                .expect("action history should filter")
                .is_empty()
        );

        for error in [
            create_manifest_scan(app.state(), scope.id)
                .expect_err("scan create must require a live grant"),
            scan_job_status(app.state(), scan_job.job_id)
                .expect_err("scan status must require a live grant"),
            pause_manifest_scan(app.state(), scan_job.job_id)
                .expect_err("scan pause must require a live grant"),
            resume_manifest_scan(app.state(), scan_job.job_id)
                .expect_err("scan resume must require a live grant"),
            create_screenshot_ocr_job(app.state(), scope.id, node_id)
                .expect_err("OCR create must require a live grant"),
            create_content_extraction_job(app.state(), scope.id, node_id)
                .expect_err("content extraction create must require a live grant"),
            screenshot_ocr_job_status(app.state(), ocr_job.job_id)
                .expect_err("OCR status must require a live grant"),
            screenshot_ocr_job_for_node(app.state(), scope.id, node_id)
                .expect_err("OCR lookup must require a live grant"),
            cancel_screenshot_ocr_job(app.state(), ocr_job.job_id)
                .expect_err("OCR cancel must require a live grant"),
            resume_screenshot_ocr_job(app.state(), ocr_job.job_id)
                .expect_err("OCR resume must require a live grant"),
            create_rename_preview(
                app.state(),
                scope.id,
                source_path.to_string_lossy().into_owned(),
                "renamed.png".to_string(),
            )
            .expect_err("rename preview must require a live grant"),
            list_search_folders(app.state(), scope.id)
                .expect_err("folder choices must require a live grant"),
            search_local(
                app.state(),
                "private".to_string(),
                SearchFilters {
                    scope_id: Some(scope.id),
                    folder_node_id: None,
                    source: SearchSourceFilter::All,
                    extension: None,
                    modified_since_unix_seconds: None,
                    modified_before_unix_seconds: None,
                },
                Some(10),
            )
            .expect_err("scoped search must require a live grant"),
        ] {
            assert_eq!(error, "scope_reauthorization_required");
        }

        assert_eq!(
            tauri::async_runtime::block_on(run_manifest_scan(app.state(), scan_job.job_id))
                .expect_err("scan run must require a live grant"),
            "scope_reauthorization_required"
        );
        assert_eq!(
            tauri::async_runtime::block_on(run_screenshot_ocr_job(app.state(), ocr_job.job_id,))
                .expect_err("OCR run must require a live grant"),
            "scope_reauthorization_required"
        );
        assert_eq!(
            tauri::async_runtime::block_on(refresh_cleanup_inbox(app.state(), scope.id))
                .expect_err("Cleanup Inbox refresh must require a live grant"),
            "scope_reauthorization_required"
        );
        assert_eq!(
            tauri::async_runtime::block_on(get_cleanup_source_detail(
                app.state(),
                scope.id,
                SmartCleanupSourceKind::ExactDuplicate,
                1,
                1,
            ))
            .expect_err("Cleanup detail must require a live grant"),
            "scope_reauthorization_required"
        );
        assert_eq!(
            tauri::async_runtime::block_on(create_cleanup_preview(
                app.state(),
                scope.id,
                SmartCleanupSourceKind::ExactDuplicate,
                1,
                1,
                node_id,
                Some(node_id + 1),
            ))
            .expect_err("Cleanup Preview must require a live grant"),
            "scope_reauthorization_required"
        );
        assert_eq!(
            tauri::async_runtime::block_on(discover_projects(app.state(), scope.id))
                .expect_err("Project discovery must require a live grant"),
            "scope_reauthorization_required"
        );
        assert_eq!(
            tauri::async_runtime::block_on(get_project_candidate_detail(app.state(), scope.id, 1,))
                .expect_err("Project detail must require a live grant"),
            "scope_reauthorization_required"
        );
        assert_eq!(
            tauri::async_runtime::block_on(decide_project_candidate(
                app.state(),
                scope.id,
                1,
                ProjectDecisionKind::Accepted,
            ))
            .expect_err("Project correction must require a live grant"),
            "scope_reauthorization_required"
        );
        let all_scope_search = search_local(
            app.state(),
            "private".to_string(),
            SearchFilters {
                scope_id: None,
                folder_node_id: None,
                source: SearchSourceFilter::All,
                extension: None,
                modified_since_unix_seconds: None,
                modified_before_unix_seconds: None,
            },
            Some(10),
        )
        .expect("unscoped search should return only active scopes");
        assert_eq!(all_scope_search.result_count, 0);
        assert!(all_scope_search.results.is_empty());

        assert!(source_path.exists());
        assert_eq!(
            recent_scan_jobs_at(&database_path)
                .expect("underlying scan jobs should remain")
                .len(),
            1
        );
        assert!(
            recent_action_plans_for_database(&database_path)
                .expect("no action should be created")
                .is_empty()
        );
    }

    #[test]
    fn cleanup_detail_and_preview_commands_revalidate_without_changing_files() {
        let directory = tempfile::tempdir().expect("fixture root should exist");
        let database_path = directory.path().join("app-data/manifest.sqlite3");
        let scope_path = directory.path().join("authorized-cleanup");
        let first_path = scope_path.join("report.md");
        let second_path = scope_path.join("report copy.md");
        std::fs::create_dir(&scope_path).expect("scope should create");
        std::fs::write(&first_path, b"same local bytes").expect("first file should create");
        std::fs::write(&second_path, b"same local bytes").expect("second file should create");

        initialize_manifest(&database_path).expect("manifest should initialize");
        let prepared =
            prepare_selected_scope(&scope_path).expect("test scope access should prepare");
        let scope = authorize_scope_with_access_grant_at(
            &database_path,
            &prepared.resolved_path,
            prepared.platform,
            &prepared.opaque_grant,
        )
        .expect("scope and grant should authorize");
        let scan = create_manifest_scan_at(&database_path, scope.id).expect("scan should create");
        run_manifest_scan_at(&database_path, scan.job_id).expect("scan should complete");
        let canonical_first =
            std::fs::canonicalize(&first_path).expect("first path should canonicalize");
        let canonical_second =
            std::fs::canonicalize(&second_path).expect("second path should canonicalize");
        deskgraph_projects::check_exact_duplicate_at(
            &database_path,
            scope.id,
            &canonical_first,
            &canonical_second,
        )
        .expect("duplicate evidence should create");
        let mut accesses = HashMap::new();
        accesses.insert(scope.id, prepared.access);
        let app = tauri::test::mock_builder()
            .manage(start_manifest_state_with_accesses(
                database_path.clone(),
                accesses,
            ))
            .build(tauri::test::mock_context(tauri::test::noop_assets()))
            .expect("mock Desktop app should build");

        let inbox = tauri::async_runtime::block_on(refresh_cleanup_inbox(app.state(), scope.id))
            .expect("Inbox should refresh");
        let item = inbox
            .items
            .into_iter()
            .find(|item| item.source_kind == SmartCleanupSourceKind::ExactDuplicate)
            .expect("duplicate item should be present");
        let detail = tauri::async_runtime::block_on(get_cleanup_source_detail(
            app.state(),
            item.scope_id,
            item.source_kind,
            item.source_id,
            item.source_observation_id,
        ))
        .expect("explicit detail should revalidate");
        assert_eq!(detail.members.len(), 2);
        assert!(detail.user_requested_paths);
        assert!(!detail.action_authorized);
        assert!(!detail.execution_available);
        assert!(
            detail
                .members
                .iter()
                .any(|member| member.display_path.ends_with("report.md"))
        );

        let target = &detail.members[0];
        let keeper = &detail.members[1];
        let preview = tauri::async_runtime::block_on(create_cleanup_preview(
            app.state(),
            detail.scope_id,
            detail.source_kind,
            detail.source_id,
            detail.source_observation_id,
            target.node_id,
            Some(keeper.node_id),
        ))
        .expect("durable Preview should create");
        assert_eq!(preview.journal_sequence, 1);
        assert!(!preview.policy.action_authorized);
        assert!(!preview.policy.execution_available);
        let payload = serde_json::to_string(&preview).expect("preview should serialize");
        assert!(!payload.contains("report.md"));
        assert!(!payload.contains("report copy.md"));
        assert_eq!(
            std::fs::read(&first_path).expect("first file should remain"),
            b"same local bytes"
        );
        assert_eq!(
            std::fs::read(&second_path).expect("second file should remain"),
            b"same local bytes"
        );
    }

    #[test]
    fn project_discovery_detail_and_correction_are_local_explainable_and_non_mutating() {
        let directory = tempfile::tempdir().expect("fixture root should exist");
        let database_path = directory.path().join("app-data/manifest.sqlite3");
        let scope_path = directory.path().join("authorized-projects");
        let marker_path = scope_path.join("Cargo.toml");
        let source_path = scope_path.join("src/lib.rs");
        std::fs::create_dir_all(source_path.parent().expect("source parent should exist"))
            .expect("scope should create");
        std::fs::write(&marker_path, "[package]\nname = \"deskgraph-test\"")
            .expect("marker should create");
        std::fs::write(&source_path, "pub fn local_only() {}").expect("source should create");

        initialize_manifest(&database_path).expect("manifest should initialize");
        let prepared = prepare_selected_scope(&scope_path).expect("test access should prepare");
        let scope = authorize_scope_with_access_grant_at(
            &database_path,
            &prepared.resolved_path,
            prepared.platform,
            &prepared.opaque_grant,
        )
        .expect("scope and grant should authorize");
        let scan = create_manifest_scan_at(&database_path, scope.id).expect("scan should create");
        run_manifest_scan_at(&database_path, scan.job_id).expect("scan should complete");
        let mut accesses = HashMap::new();
        accesses.insert(scope.id, prepared.access);
        let app = tauri::test::mock_builder()
            .manage(start_manifest_state_with_accesses(
                database_path.clone(),
                accesses,
            ))
            .build(tauri::test::mock_context(tauri::test::noop_assets()))
            .expect("mock Desktop app should build");

        assert!(
            project_scope_has_completed_scan(app.state(), scope.id)
                .expect("durable Project readiness should load")
        );

        let discovery = tauri::async_runtime::block_on(discover_projects(app.state(), scope.id))
            .expect("Project discovery should complete");
        assert_eq!(discovery.candidates.len(), 1);
        assert!(discovery.evaluation_complete);
        assert!(!discovery.automatic_membership_created);
        assert!(!discovery.file_actions_available);
        let discovery_payload =
            serde_json::to_string(&discovery).expect("discovery should serialize");
        assert!(!discovery_payload.contains("authorized-projects"));
        assert!(!discovery_payload.contains("Cargo.toml"));
        assert!(!discovery_payload.contains("lib.rs"));

        let summary = &discovery.candidates[0];
        let detail = tauri::async_runtime::block_on(get_project_candidate_detail(
            app.state(),
            scope.id,
            summary.project_id,
        ))
        .expect("explicit detail should load");
        assert!(detail.user_requested_path);
        assert!(detail.current_evidence);
        assert_eq!(detail.candidate.scope_id, scope.id);
        assert!(
            detail
                .candidate
                .display_path
                .ends_with("authorized-projects")
        );
        assert_eq!(detail.candidate.suggestion.provenance.len(), 1);

        let rejected = tauri::async_runtime::block_on(decide_project_candidate(
            app.state(),
            scope.id,
            summary.project_id,
            ProjectDecisionKind::Rejected,
        ))
        .expect("user correction should append");
        assert_eq!(
            rejected.candidate.state,
            deskgraph_domain::ProjectCandidateState::Rejected
        );
        assert_eq!(
            rejected
                .candidate
                .latest_decision
                .as_ref()
                .map(|decision| decision.sequence),
            Some(1)
        );
        assert!(marker_path.exists());
        assert!(source_path.exists());
        assert_eq!(
            std::fs::read_to_string(&source_path).expect("source should remain unchanged"),
            "pub fn local_only() {}"
        );
    }

    #[test]
    fn watch_runtime_starts_path_free_and_stops_with_its_managed_state() {
        let directory = tempfile::tempdir().expect("fixture root should exist");
        let database_path = directory.path().join("manifest.sqlite3");
        initialize_manifest(&database_path).expect("manifest should initialize");
        let state = start_manifest_state(database_path);
        let status_handle = Arc::clone(&state.watch_status);

        let mut observed = None;
        for _ in 0..40 {
            let status = status_handle
                .lock()
                .expect("watch status should not be poisoned")
                .clone();
            if status.state == WatchRuntimeState::Running && status.last_cycle_unix_ms.is_some() {
                observed = Some(status);
                break;
            }
            thread::sleep(Duration::from_millis(25));
        }
        let observed = observed.expect("watch runtime should complete a startup cycle");
        let payload = serde_json::to_string(&observed).expect("status should serialize");
        assert_eq!(observed.api_version, "deskgraph.watch-runtime.v2");
        assert_eq!(observed.adapter, WATCH_ADAPTER_NATIVE);
        assert!(observed.periodic_reconciliation_enabled);
        assert_eq!(observed.native_watched_scope_count, 0);
        assert_eq!(observed.last_error_code, None);
        assert!(!payload.contains("/Users/"));
        assert!(!payload.contains("manifest.sqlite3"));

        drop(state);
        assert_eq!(
            status_handle
                .lock()
                .expect("watch status should remain readable")
                .state,
            WatchRuntimeState::Stopped
        );
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn macos_native_runtime_reconciles_create_modify_rename_and_delete() {
        fn wait_until(mut predicate: impl FnMut() -> bool, message: &str) {
            for _ in 0..240 {
                if predicate() {
                    return;
                }
                thread::sleep(Duration::from_millis(50));
            }
            panic!("{message}");
        }

        let directory = tempfile::tempdir().expect("fixture root should exist");
        let database_path = directory.path().join("manifest.sqlite3");
        let scope_path = directory.path().join("authorized");
        std::fs::create_dir(&scope_path).expect("scope should create");
        initialize_manifest(&database_path).expect("manifest should initialize");
        let prepared = prepare_selected_scope(&scope_path).expect("scope access should prepare");
        let scope_root = prepared.resolved_path.clone();
        let scope = authorize_scope_with_access_grant_at(
            &database_path,
            &scope_root,
            prepared.platform,
            &prepared.opaque_grant,
        )
        .expect("scope and grant should authorize");
        let scan = create_manifest_scan_at(&database_path, scope.id).expect("scan should create");
        run_manifest_scan_at(&database_path, scan.job_id).expect("scan should complete");
        let canonical_scope =
            std::fs::canonicalize(&scope_root).expect("scope should canonicalize");
        let original_key =
            deskgraph_scanner::comparison_key(&canonical_scope.join("native-original.md"));
        let renamed_key =
            deskgraph_scanner::comparison_key(&canonical_scope.join("native-renamed.md"));

        let mut accesses = HashMap::new();
        accesses.insert(scope.id, prepared.access);
        let state = start_manifest_state_with_accesses(database_path.clone(), accesses);
        wait_until(
            || {
                let status = state
                    .watch_status
                    .lock()
                    .expect("watch status should be readable")
                    .clone();
                status.state == WatchRuntimeState::Running
                    && status.adapter == WATCH_ADAPTER_NATIVE
                    && status.native_watched_scope_count == 1
                    && status.active_event_count == 0
            },
            "native watcher should register after the initial reconciliation",
        );

        let original = scope_root.join("native-original.md");
        std::fs::write(&original, "one").expect("native create should succeed");
        let created = (0..240).any(|_| {
            let found = {
                let database =
                    ManifestDatabase::open(&database_path).expect("database should open");
                database
                    .node_id_for_path_key(scope.id, &original_key)
                    .expect("node lookup should pass")
                    .is_some()
            };
            if !found {
                thread::sleep(Duration::from_millis(50));
            }
            found
        });
        if !created {
            let status = state
                .watch_status
                .lock()
                .expect("watch status should be readable")
                .clone();
            let events = recent_watch_events_for_database(&database_path)
                .expect("watch history should load");
            panic!(
                "native create should reconcile into the manifest; status={status:?}; events={events:?}"
            );
        }
        let original_node_id = ManifestDatabase::open(&database_path)
            .expect("database should open")
            .node_id_for_path_key(scope.id, &original_key)
            .expect("node lookup should pass")
            .expect("created node should exist");

        std::fs::write(&original, "a longer second value").expect("native modify should succeed");
        wait_until(
            || {
                ManifestDatabase::open(&database_path)
                    .expect("database should open")
                    .extractable_file(scope.id, original_node_id)
                    .expect("manifest file should remain available")
                    .size_bytes
                    == u64::try_from("a longer second value".len())
                        .expect("fixture length should fit")
            },
            "native modify should publish the updated manifest metadata",
        );

        let renamed = scope_root.join("native-renamed.md");
        std::fs::rename(&original, &renamed).expect("native rename should succeed");
        wait_until(
            || {
                let database =
                    ManifestDatabase::open(&database_path).expect("database should open");
                database
                    .node_id_for_path_key(scope.id, &renamed_key)
                    .expect("renamed node lookup should pass")
                    == Some(original_node_id)
                    && database
                        .node_id_for_path_key(scope.id, &original_key)
                        .expect("old node lookup should pass")
                        .is_none()
            },
            "native rename should preserve the stable node identity",
        );

        std::fs::remove_file(&renamed).expect("native delete should succeed");
        wait_until(
            || {
                ManifestDatabase::open(&database_path)
                    .expect("database should open")
                    .node_id_for_path_key(scope.id, &renamed_key)
                    .expect("deleted node lookup should pass")
                    .is_none()
            },
            "native delete should reconcile out of the live manifest",
        );

        let temporary_download = scope_path.join("native-download.crdownload");
        let final_download = scope_path.join("native-download.pdf");
        let temporary_download_key =
            deskgraph_scanner::comparison_key(&canonical_scope.join("native-download.crdownload"));
        let final_download_key =
            deskgraph_scanner::comparison_key(&canonical_scope.join("native-download.pdf"));
        std::fs::write(&temporary_download, "complete").expect("temporary download should succeed");
        wait_until(
            || {
                recent_watch_events_for_database(&database_path)
                    .expect("watch history should load")
                    .iter()
                    .any(|event| {
                        event.reason == Some(WatchEventReason::TemporaryDownload)
                            && event.status == WatchEventStatus::Ignored
                    })
            },
            "native temporary download should reach the ignored aggregate",
        );
        assert!(
            ManifestDatabase::open(&database_path)
                .expect("database should open")
                .node_id_for_path_key(scope.id, &temporary_download_key)
                .expect("temporary node lookup should pass")
                .is_none(),
            "temporary download must remain outside the live manifest"
        );

        std::fs::rename(&temporary_download, &final_download)
            .expect("final download rename should succeed");
        wait_until(
            || {
                ManifestDatabase::open(&database_path)
                    .expect("database should open")
                    .node_id_for_path_key(scope.id, &final_download_key)
                    .expect("final download lookup should pass")
                    .is_some()
            },
            "native final rename should enter the manifest without waiting for fallback",
        );

        let payload = serde_json::to_string(
            &state
                .watch_status
                .lock()
                .expect("watch status should be readable")
                .clone(),
        )
        .expect("watch status should serialize");
        assert!(!payload.contains("native-original.md"));
        assert!(!payload.contains("native-renamed.md"));
        assert!(!payload.contains(scope_path.to_string_lossy().as_ref()));
        drop(state);
    }

    #[test]
    fn watch_wait_does_not_lose_a_notification_before_the_wait_lock() {
        let (wake, wake_receiver) = sync_channel(1);
        let stop = AtomicBool::new(false);
        notify_watch_wake(&wake);

        let started = Instant::now();
        wait_for_watch_wake(&wake_receiver, &stop, Duration::from_secs(5))
            .expect("notification should remain observable");

        assert!(
            started.elapsed() < Duration::from_millis(100),
            "a notification between cycle completion and wait locking must not be lost"
        );
    }

    #[test]
    fn watch_runtime_shutdown_is_bounded_when_the_writer_gate_is_busy() {
        let directory = tempfile::tempdir().expect("fixture root should exist");
        let database_path = directory.path().join("manifest.sqlite3");
        initialize_manifest(&database_path).expect("manifest should initialize");
        let state = start_manifest_state(database_path);
        let status_handle = Arc::clone(&state.watch_status);
        let gate = Arc::clone(&state.database_gate);
        let database_guard = gate.lock().expect("test should hold writer gate");
        wake_watch_runtime(&state);
        thread::sleep(Duration::from_millis(25));
        assert!(
            manifest_status_at(&state.database_path).is_ok(),
            "read-only status must not wait for the in-process writer gate"
        );
        assert!(
            lock_watch_status(&state.watch_status).is_ok(),
            "path-free runtime status must remain readable"
        );

        let started = Instant::now();
        drop(state);
        assert!(
            started.elapsed() < Duration::from_millis(2_500),
            "managed shutdown must not wait indefinitely for a busy writer"
        );
        drop(database_guard);

        for _ in 0..40 {
            if status_handle
                .lock()
                .expect("watch status should remain readable")
                .state
                == WatchRuntimeState::Stopped
            {
                return;
            }
            thread::sleep(Duration::from_millis(25));
        }
        panic!("detached worker should stop after the writer gate is released");
    }

    #[test]
    fn revocation_sync_timeout_disables_the_entire_watch_runtime() {
        let directory = tempfile::tempdir().expect("fixture root should exist");
        let database_path = directory.path().join("manifest.sqlite3");
        initialize_manifest(&database_path).expect("manifest should initialize");
        let state = start_manifest_state(database_path);

        let outcome = retire_watch_runtime_after_revocation_timeout(&state);
        assert!(
            outcome.native_watch_callback_retired,
            "the callback must reject new hints and clear its queue"
        );
        assert!(
            outcome.watch_runtime_stopped,
            "the coordinator must stop so its owned native source is dropped"
        );
        assert!(
            state.watch_stop.load(Ordering::Acquire),
            "a timed-out revocation must terminally disable automatic monitoring"
        );
        assert!(
            state
                .watch_thread
                .lock()
                .expect("watch thread registry should be readable")
                .is_none(),
            "a joined coordinator must not remain available for a later automatic retry"
        );
        let status = lock_watch_status(&state.watch_status)
            .expect("watch status should be readable")
            .clone();
        assert_eq!(status.state, WatchRuntimeState::Stopped);
        assert_eq!(status.native_watched_scope_count, 0);
        assert_eq!(status.next_wake_unix_ms, None);
    }

    #[test]
    fn tauri_health_payload_excludes_filesystem_locations() {
        let directory = tempfile::tempdir().expect("fixture root should exist");
        let database_path = directory.path().join("manifest.sqlite3");
        initialize_manifest(&database_path).expect("manifest should initialize");
        let payload =
            serde_json::to_string(&health_at(&database_path).expect("health should load"))
                .expect("health must serialize");

        assert!(!payload.contains("/Users/"));
        assert!(!payload.contains("C:\\Users\\"));
        assert!(payload.contains("\"filesystem_locations_included\":false"));
    }

    #[test]
    fn manifest_helpers_cover_authorize_scan_and_status() {
        let directory = tempfile::tempdir().expect("fixture root should exist");
        let database_path = directory.path().join("app-data/manifest.sqlite3");
        let scope_path = directory.path().join("authorized");
        std::fs::create_dir(&scope_path).expect("scope should create");
        std::fs::write(scope_path.join("note.md"), "metadata fixture").expect("file should create");

        initialize_manifest(&database_path).expect("manifest should initialize");
        let scope =
            authorize_scope_at(&database_path, &scope_path).expect("scope should authorize");
        let job = create_manifest_scan_at(&database_path, scope.id).expect("job should create");
        let paused = pause_manifest_scan_at(&database_path, job.job_id).expect("job should pause");
        assert_eq!(paused.status, deskgraph_domain::ScanStatus::Paused);
        resume_manifest_scan_at(&database_path, job.job_id).expect("job should resume");
        let report =
            run_manifest_scan_at(&database_path, job.job_id).expect("metadata scan should pass");
        let status = manifest_status_at(&database_path).expect("status should load");

        assert_eq!(report.discovered_files, 1);
        assert_eq!(
            scan_job_status_at(&database_path, job.job_id)
                .expect("job status should load")
                .status,
            deskgraph_domain::ScanStatus::Completed
        );
        assert_eq!(
            recent_scan_jobs_at(&database_path)
                .expect("recent jobs should load")
                .first()
                .map(|recent| recent.job_id),
            Some(job.job_id)
        );
        assert_eq!(status.authorized_scope_count, 1);
        assert_eq!(status.file_count, 1);
        assert_eq!(
            authorized_scopes_at(&database_path).expect("scopes should load"),
            vec![scope]
        );
    }

    #[test]
    fn extraction_helpers_expose_counts_without_paths_or_text() {
        let directory = tempfile::tempdir().expect("fixture root should exist");
        let database_path = directory.path().join("app-data/manifest.sqlite3");
        let scope_path = directory.path().join("authorized-private");
        let source_path = scope_path.join("private-notes.md");
        let private_text = "私人內容不得出現在狀態 payload";
        std::fs::create_dir(&scope_path).expect("scope should create");
        std::fs::write(&source_path, private_text).expect("file should create");

        initialize_manifest(&database_path).expect("manifest should initialize");
        let scope =
            authorize_scope_at(&database_path, &scope_path).expect("scope should authorize");
        let scan = create_manifest_scan_at(&database_path, scope.id).expect("scan should create");
        run_manifest_scan_at(&database_path, scan.job_id).expect("scan should complete");
        let database = ManifestDatabase::open(&database_path).expect("database should open");
        let node_id = database
            .node_id_for_path_key(
                scope.id,
                &deskgraph_scanner::comparison_key(
                    &std::fs::canonicalize(&source_path).expect("source should canonicalize"),
                ),
            )
            .expect("node lookup should pass")
            .expect("source node should exist");
        drop(database);
        let job = create_extraction_job_at(&database_path, scope.id, node_id)
            .expect("extraction should create");
        run_extraction_job_at(&database_path, job.job_id, ExtractionLimits::default())
            .expect("extraction should complete");

        ManifestDatabase::open(&database_path)
            .expect("fixture database should open")
            .mark_scope_access_grant_needs_reauthorization(scope.id)
            .expect("fixture grant should become inactive before filtered stats");

        let inactive_manifest = manifest_status_with_active_access_grants_at(&database_path)
            .expect("inactive manifest stats should load");
        let inactive_extraction =
            content_extraction_stats_with_active_access_grants_at(&database_path)
                .expect("inactive extraction stats should load");
        assert_eq!(inactive_manifest.authorized_scope_count, 0);
        assert_eq!(inactive_manifest.node_count, 0);
        assert_eq!(inactive_manifest.completed_scan_count, 0);
        assert_eq!(inactive_extraction.active_chunk_count, 0);
        assert_eq!(inactive_extraction.extracted_file_count, 0);
        assert_eq!(inactive_extraction.completed_job_count, 0);

        let database = ManifestDatabase::open(&database_path).expect("database should open");
        database
            .upsert_scope_access_grant(scope.id, std::env::consts::OS, b"opaque-test-grant")
            .expect("fixture grant should become active");
        drop(database);
        assert_eq!(
            manifest_status_with_active_access_grants_at(&database_path)
                .expect("active manifest stats should load"),
            manifest_status_at(&database_path).expect("general manifest stats should load")
        );
        assert_eq!(
            content_extraction_stats_with_active_access_grants_at(&database_path)
                .expect("active extraction stats should load"),
            content_extraction_stats_at(&database_path)
                .expect("general extraction stats should load")
        );

        let database = ManifestDatabase::open(&database_path).expect("database should reopen");
        database
            .mark_scope_access_grant_revoked(scope.id)
            .expect("fixture grant should revoke");
        drop(database);
        assert_eq!(
            manifest_status_with_active_access_grants_at(&database_path)
                .expect("revoked manifest stats should load")
                .node_count,
            0
        );
        assert_eq!(
            content_extraction_stats_with_active_access_grants_at(&database_path)
                .expect("revoked extraction stats should load")
                .active_chunk_count,
            0
        );

        let revoked_stats =
            content_extraction_stats_at(&database_path).expect("stats should load after purge");
        let revoked_jobs =
            recent_content_extractions_at(&database_path).expect("jobs should load after purge");
        assert_eq!(revoked_stats.completed_job_count, 0);
        assert!(revoked_jobs.is_empty());
        let payload = serde_json::to_string(&(revoked_stats, revoked_jobs))
            .expect("payload should serialize");
        assert!(!payload.contains(private_text));
        assert!(!payload.contains("private-notes.md"));
        assert!(!payload.contains(scope_path.to_string_lossy().as_ref()));
        assert!(payload.contains("deskgraph.extraction-stats.v1"));
        assert!(!payload.contains("\"status\":\"completed\""));
    }

    #[test]
    fn desktop_content_extraction_is_explicit_bounded_and_searchable() {
        let private_text = "Selected document phrase 僅本機抽取";
        let (_directory, database_path, scope, node_id) =
            scanned_file_fixture("selected-document.md", private_text);

        let queued = create_content_extraction_job_for_database(&database_path, scope.id, node_id)
            .expect("explicit content extraction should queue");
        assert_eq!(queued.operation, ExtractionOperation::Content);
        assert_eq!(queued.status, ExtractionStatus::Queued);

        let completed = run_content_extraction_job_at(&database_path, queued.job_id)
            .expect("bounded content extraction should complete");
        assert_eq!(completed.status, ExtractionStatus::Completed);
        assert!(completed.chunk_count > 0);

        let database = ManifestDatabase::open(&database_path).expect("database should reopen");
        database
            .upsert_scope_access_grant(scope.id, std::env::consts::OS, b"content-search-grant")
            .expect("search fixture grant should become active");
        drop(database);

        let response = search_local_at(
            &database_path,
            "僅本機抽取",
            &SearchFilters {
                scope_id: Some(scope.id),
                folder_node_id: None,
                source: SearchSourceFilter::ExtractedText,
                extension: Some("md".to_string()),
                modified_since_unix_seconds: None,
                modified_before_unix_seconds: None,
            },
            Some(10),
        )
        .expect("extracted text search should pass");
        assert_eq!(response.result_count, 1);
        assert_eq!(response.results[0].node_id, node_id);

        let payload = serde_json::to_string(&completed).expect("progress should serialize");
        assert!(!payload.contains("selected-document.md"));
        assert!(!payload.contains(private_text));
        assert!(!payload.contains("\"path\""));
        assert!(!payload.contains("\"text\""));
    }

    #[test]
    fn desktop_content_extraction_rejects_images_before_job_creation() {
        let image = png_bytes(64, 64, b"private image marker");
        let (_directory, database_path, scope, node_id) =
            scanned_file_fixture("selected-screenshot.png", image);
        let before = recent_content_extractions_at(&database_path)
            .expect("recent jobs should load before rejection");

        assert_eq!(
            create_content_extraction_job_for_database(&database_path, scope.id, node_id)
                .expect_err("images must remain on the explicit OCR path"),
            "extraction_media_kind_unsupported"
        );
        assert_eq!(
            recent_content_extractions_at(&database_path)
                .expect("rejection must not create a durable job"),
            before
        );
    }

    #[test]
    fn content_extraction_helpers_reject_ocr_job_ids() {
        let image = png_bytes(64, 64, b"private OCR marker");
        let (_directory, database_path, scope, node_id) =
            scanned_file_fixture("selected-screenshot.png", image);
        let ocr_job = create_screenshot_ocr_job_for_database(&database_path, scope.id, node_id)
            .expect("OCR job should queue");

        assert_eq!(
            require_content_extraction_job(&database_path, ocr_job.job_id)
                .expect_err("content status must reject OCR job IDs"),
            "content_extraction_job_required"
        );
        assert_eq!(
            run_content_extraction_job_at(&database_path, ocr_job.job_id)
                .expect_err("content runner must reject OCR job IDs"),
            "content_extraction_job_required"
        );
        assert_eq!(
            extraction_job_at(&database_path, ocr_job.job_id)
                .expect("rejected OCR job should stay queued")
                .status,
            ExtractionStatus::Queued
        );
    }

    #[test]
    fn content_extraction_commands_require_a_live_native_grant() {
        let (_directory, database_path, scope, node_id) =
            scanned_file_fixture("selected-document.md", "private local content");
        let queued = create_content_extraction_job_for_database(&database_path, scope.id, node_id)
            .expect("content fixture job should queue");
        let app = tauri::test::mock_builder()
            .manage(start_manifest_state(database_path))
            .build(tauri::test::mock_context(tauri::test::noop_assets()))
            .expect("mock Desktop app should build");

        assert_eq!(
            create_content_extraction_job(app.state(), scope.id, node_id)
                .expect_err("create must require a live native grant"),
            "scope_reauthorization_required"
        );
        assert_eq!(
            tauri::async_runtime::block_on(run_content_extraction_job(app.state(), queued.job_id,))
                .expect_err("run must require a live native grant"),
            "scope_reauthorization_required"
        );
        assert_eq!(
            extraction_job_at(&app.state::<ManifestState>().database_path, queued.job_id)
                .expect("denied job should remain queued")
                .status,
            ExtractionStatus::Queued
        );
    }

    #[test]
    fn screenshot_ocr_helpers_queue_cancel_and_expose_path_free_progress() {
        let private_text = "OCR private text must not enter job progress";
        let image = png_bytes(640, 480, private_text.as_bytes());
        let (_directory, database_path, scope, node_id) =
            scanned_file_fixture("private-screenshot.png", image);

        let queued = create_screenshot_ocr_job_for_database(&database_path, scope.id, node_id)
            .expect("screenshot OCR should queue");
        assert_eq!(queued.operation, ExtractionOperation::ScreenshotOcr);
        assert_eq!(queued.status, ExtractionStatus::Queued);
        assert_eq!(
            screenshot_ocr_job_status_at(&database_path, queued.job_id)
                .expect("OCR status should load"),
            queued
        );

        let cancelled = cancel_screenshot_ocr_job_at(&database_path, queued.job_id)
            .expect("queued OCR should cancel durably");
        assert_eq!(cancelled.status, ExtractionStatus::Cancelled);
        assert!(cancelled.cancel_requested);
        assert_eq!(cancelled.chunk_count, 0);
        assert_eq!(cancelled.output_bytes, 0);
        assert_eq!(
            screenshot_ocr_job_status_at(&database_path, queued.job_id)
                .expect("cancelled OCR status should load"),
            cancelled
        );

        let payload = serde_json::to_string(&cancelled).expect("progress should serialize");
        assert!(payload.contains("deskgraph.extraction-job.v2"));
        assert!(!payload.contains("private-screenshot.png"));
        assert!(!payload.contains(private_text));
        assert!(!payload.contains("authorized-private"));
        assert!(!payload.contains("\"path\""));
        assert!(!payload.contains("\"text\""));
    }

    #[test]
    fn screenshot_ocr_helpers_reject_unknown_and_non_ocr_jobs() {
        let (_directory, database_path, scope, node_id) =
            scanned_file_fixture("private-note.md", "private generic content");

        assert_eq!(
            create_screenshot_ocr_job_for_database(&database_path, scope.id, node_id + 9_999)
                .expect_err("unknown nodes must be rejected"),
            "extractable_file_not_found"
        );

        let generic = create_extraction_job_at(&database_path, scope.id, node_id)
            .expect("generic extraction should queue");
        assert_eq!(generic.operation, ExtractionOperation::Content);
        assert_eq!(
            screenshot_ocr_job_status_at(&database_path, generic.job_id)
                .expect_err("OCR status must not expose generic jobs"),
            "screenshot_ocr_job_required"
        );
        assert_eq!(
            run_screenshot_ocr_job_at(&database_path, generic.job_id)
                .expect_err("OCR runner must not execute generic jobs"),
            "screenshot_ocr_job_required"
        );
        assert_eq!(
            cancel_screenshot_ocr_job_at(&database_path, generic.job_id)
                .expect_err("OCR cancellation must not affect generic jobs"),
            "screenshot_ocr_job_required"
        );
        assert_eq!(
            extraction_job_at(&database_path, generic.job_id)
                .expect("generic job should remain readable to core")
                .status,
            ExtractionStatus::Queued
        );
    }

    #[test]
    fn screenshot_ocr_creation_rejects_non_images_without_side_effects() {
        let private_text = "this text must never be OCR job payload";
        let (_directory, database_path, scope, node_id) =
            scanned_file_fixture("private-format.md", private_text);
        let before = recent_content_extractions_at(&database_path)
            .expect("recent jobs should load before failed creation");

        assert_eq!(
            create_screenshot_ocr_job_for_database(&database_path, scope.id, node_id)
                .expect_err("non-image OCR must be rejected before a job is inserted"),
            "extraction_media_kind_unsupported"
        );
        assert_eq!(
            recent_content_extractions_at(&database_path)
                .expect("failed creation must not insert a job"),
            before
        );

        let generic = create_extraction_job_at(&database_path, scope.id, node_id)
            .expect("a rejected OCR request must not block ordinary content extraction");
        assert_eq!(generic.operation, ExtractionOperation::Content);
        let payload = serde_json::to_string(&generic).expect("progress should serialize");
        assert!(!payload.contains("private-format.md"));
        assert!(!payload.contains(private_text));
        assert!(!payload.contains("authorized-private"));
        assert!(!payload.contains("\"path\""));
        assert!(!payload.contains("\"text\""));
    }

    #[test]
    fn screenshot_ocr_resume_requires_an_interrupted_ocr_job_and_keeps_payload_private() {
        let private_text = "interrupted OCR private text must not be exposed";
        let image = png_bytes(640, 480, private_text.as_bytes());
        let (_directory, database_path, scope, node_id) =
            scanned_file_fixture("private-interrupted.png", image);
        let queued = create_screenshot_ocr_job_for_database(&database_path, scope.id, node_id)
            .expect("valid screenshot OCR should queue");

        assert_eq!(
            resume_screenshot_ocr_job_at(&database_path, queued.job_id)
                .expect_err("a non-interrupted OCR job must not resume"),
            "invalid_extraction_job_state"
        );
        assert_eq!(
            extraction_job_at(&database_path, queued.job_id)
                .expect("non-interrupted OCR state should remain readable")
                .status,
            ExtractionStatus::Queued
        );

        let mut database = ManifestDatabase::open(&database_path).expect("database should open");
        database
            .claim_extraction_job(queued.job_id, "expired-ocr-runner", 60_000)
            .expect("OCR job should claim for recovery fixture");
        assert_eq!(
            database
                .recover_expired_extraction_jobs_at(i64::MAX)
                .expect("expired OCR lease should recover"),
            1
        );
        assert_eq!(
            database
                .extraction_job(queued.job_id)
                .expect("interrupted OCR should load")
                .status,
            ExtractionStatus::Interrupted
        );
        drop(database);

        let resumed = resume_screenshot_ocr_job_at(&database_path, queued.job_id)
            .expect("interrupted screenshot OCR should requeue");
        assert_eq!(resumed.operation, ExtractionOperation::ScreenshotOcr);
        assert_eq!(resumed.status, ExtractionStatus::Queued);
        assert!(!resumed.cancel_requested);

        let (_generic_directory, generic_database_path, generic_scope, generic_node_id) =
            scanned_file_fixture("private-generic.md", "private generic content");
        let generic =
            create_extraction_job_at(&generic_database_path, generic_scope.id, generic_node_id)
                .expect("generic extraction should queue");
        assert_eq!(
            resume_screenshot_ocr_job_at(&generic_database_path, generic.job_id)
                .expect_err("generic jobs must not use screenshot OCR resume"),
            "screenshot_ocr_job_required"
        );
        assert_eq!(
            extraction_job_at(&generic_database_path, generic.job_id)
                .expect("generic state should remain readable")
                .status,
            ExtractionStatus::Queued
        );

        let payload = serde_json::to_string(&resumed).expect("progress should serialize");
        assert!(payload.contains("deskgraph.extraction-job.v2"));
        assert!(!payload.contains("private-interrupted.png"));
        assert!(!payload.contains(private_text));
        assert!(!payload.contains("authorized-private"));
        assert!(!payload.contains("\"path\""));
        assert!(!payload.contains("\"text\""));
    }

    #[test]
    fn screenshot_ocr_node_lookup_is_operation_scoped_and_path_free() {
        let private_text = "node lookup OCR text must stay private";
        let image = png_bytes(640, 480, private_text.as_bytes());
        let (_directory, database_path, scope, node_id) =
            scanned_file_fixture("private-node-lookup.png", image);
        let queued = create_screenshot_ocr_job_for_database(&database_path, scope.id, node_id)
            .expect("valid screenshot OCR should queue");
        let mut database = ManifestDatabase::open(&database_path).expect("database should open");
        database
            .claim_extraction_job(queued.job_id, "expired-node-lookup-runner", 60_000)
            .expect("OCR job should claim for recovery fixture");
        database
            .recover_expired_extraction_jobs_at(i64::MAX)
            .expect("expired OCR lease should recover");
        drop(database);

        let found = screenshot_ocr_job_for_node_at(&database_path, scope.id, node_id)
            .expect("node lookup should query")
            .expect("interrupted OCR should be found");
        assert_eq!(found.job_id, queued.job_id);
        assert_eq!(found.operation, ExtractionOperation::ScreenshotOcr);
        assert_eq!(found.status, ExtractionStatus::Interrupted);
        let payload = serde_json::to_string(&found).expect("progress should serialize");
        assert!(!payload.contains("private-node-lookup.png"));
        assert!(!payload.contains(private_text));
        assert!(!payload.contains("authorized-private"));
        assert!(!payload.contains("\"path\""));
        assert!(!payload.contains("\"text\""));

        let (_generic_directory, generic_database_path, generic_scope, generic_node_id) =
            scanned_file_fixture("private-node-lookup.md", "generic private text");
        create_extraction_job_at(&generic_database_path, generic_scope.id, generic_node_id)
            .expect("generic job should queue");
        assert_eq!(
            screenshot_ocr_job_for_node_at(
                &generic_database_path,
                generic_scope.id,
                generic_node_id,
            )
            .expect("generic node lookup should query"),
            None
        );
    }

    #[test]
    fn search_helper_returns_bounded_user_requested_context() {
        let directory = tempfile::tempdir().expect("fixture root should exist");
        let database_path = directory.path().join("app-data/manifest.sqlite3");
        let scope_path = directory.path().join("authorized-search");
        let source_path = scope_path.join("專案-context.md");
        let private_text = "Traditional Chinese 專案脈絡 and English context";
        std::fs::create_dir(&scope_path).expect("scope should create");
        std::fs::write(&source_path, private_text).expect("file should create");

        initialize_manifest(&database_path).expect("manifest should initialize");
        let scope = authorize_scope_with_access_grant_at(
            &database_path,
            &scope_path,
            std::env::consts::OS,
            b"search-helper-test-grant",
        )
        .expect("scope should authorize with an active grant");
        let scan = create_manifest_scan_at(&database_path, scope.id).expect("scan should create");
        run_manifest_scan_at(&database_path, scan.job_id).expect("scan should complete");
        let database = ManifestDatabase::open(&database_path).expect("database should open");
        let node_id = database
            .node_id_for_path_key(
                scope.id,
                &deskgraph_scanner::comparison_key(
                    &std::fs::canonicalize(&source_path).expect("source should canonicalize"),
                ),
            )
            .expect("node lookup should pass")
            .expect("source node should exist");
        drop(database);
        let job = create_extraction_job_at(&database_path, scope.id, node_id)
            .expect("extraction should create");
        run_extraction_job_at(&database_path, job.job_id, ExtractionLimits::default())
            .expect("extraction should complete");

        let response = search_local_at(
            &database_path,
            "專案脈絡",
            &SearchFilters {
                scope_id: Some(scope.id),
                folder_node_id: None,
                source: SearchSourceFilter::ExtractedText,
                extension: Some("MD".to_string()),
                modified_since_unix_seconds: None,
                modified_before_unix_seconds: None,
            },
            Some(10),
        )
        .expect("search should pass");
        assert_eq!(response.result_count, 1);
        assert_eq!(response.filters.source, SearchSourceFilter::ExtractedText);
        assert_eq!(response.filters.extension.as_deref(), Some("md"));
        assert_eq!(response.results[0].node_id, node_id);
        assert_eq!(response.results[0].explanation, "extracted_text_substring");
        assert!(
            response.results[0]
                .snippet
                .as_deref()
                .unwrap_or_default()
                .contains("專案脈絡")
        );
    }

    #[test]
    fn desktop_folder_search_lists_user_requested_paths_and_filters_descendants() {
        let directory = tempfile::tempdir().expect("fixture root should exist");
        let database_path = directory.path().join("app-data/manifest.sqlite3");
        let scope_path = directory.path().join("authorized-folder-search");
        let nested_path = scope_path.join("nested/deep");
        let sibling_path = scope_path.join("sibling");
        std::fs::create_dir_all(&nested_path).expect("nested folder should create");
        std::fs::create_dir_all(&sibling_path).expect("sibling folder should create");
        std::fs::write(nested_path.join("sharedcontext-note.md"), "nested")
            .expect("nested file should create");
        std::fs::write(sibling_path.join("sharedcontext-note.md"), "sibling")
            .expect("sibling file should create");

        initialize_manifest(&database_path).expect("manifest should initialize");
        let prepared = prepare_selected_scope(&scope_path).expect("test access should prepare");
        let scope = authorize_scope_with_access_grant_at(
            &database_path,
            &prepared.resolved_path,
            prepared.platform,
            &prepared.opaque_grant,
        )
        .expect("scope and grant should authorize");
        let scan = create_manifest_scan_at(&database_path, scope.id).expect("scan should create");
        run_manifest_scan_at(&database_path, scan.job_id).expect("scan should complete");
        let mut accesses = HashMap::new();
        accesses.insert(scope.id, prepared.access);
        let app = tauri::test::mock_builder()
            .manage(start_manifest_state_with_accesses(
                database_path.clone(),
                accesses,
            ))
            .build(tauri::test::mock_context(tauri::test::noop_assets()))
            .expect("mock Desktop app should build");

        let folders =
            list_search_folders(app.state(), scope.id).expect("explicit folder list should load");
        let nested_display = std::fs::canonicalize(scope_path.join("nested"))
            .expect("nested folder should canonicalize")
            .to_string_lossy()
            .into_owned();
        let nested_folder = folders
            .folders
            .iter()
            .find(|folder| folder.display_path == nested_display)
            .expect("nested folder should be selectable");
        assert_eq!(folders.scope_id, scope.id);
        assert_eq!(folders.folder_count, folders.folders.len() as u64);
        assert!(!folders.truncated);
        assert!(
            serde_json::to_string(&folders)
                .expect("user-requested response should serialize")
                .contains(&nested_display)
        );
        assert!(!format!("{folders:?}").contains(&nested_display));

        let response = search_local(
            app.state(),
            "sharedcontext".to_string(),
            SearchFilters {
                scope_id: Some(scope.id),
                folder_node_id: Some(nested_folder.folder_node_id),
                source: SearchSourceFilter::MetadataPath,
                extension: Some("md".to_string()),
                modified_since_unix_seconds: None,
                modified_before_unix_seconds: None,
            },
            Some(10),
        )
        .expect("folder-scoped search should pass");
        assert_eq!(response.result_count, 1);
        assert_eq!(
            response.filters.folder_node_id,
            Some(nested_folder.folder_node_id)
        );
        assert!(response.results[0].display_path.contains("nested/deep"));
        assert!(!response.results[0].display_path.contains("sibling"));
    }

    #[test]
    fn rename_preview_helper_changes_no_file_and_history_is_path_free() {
        let directory = tempfile::tempdir().expect("fixture root should exist");
        let database_path = directory.path().join("app-data/manifest.sqlite3");
        let scope_path = directory.path().join("authorized-actions-private");
        let source_path = scope_path.join("private-draft.md");
        let destination_path = scope_path.join("private-final.md");
        std::fs::create_dir(&scope_path).expect("scope should create");
        std::fs::write(&source_path, "private action text").expect("file should create");
        initialize_manifest(&database_path).expect("manifest should initialize");
        let scope = authorize_scope_with_access_grant_at(
            &database_path,
            &scope_path,
            std::env::consts::OS,
            b"rename-preview-helper-test-grant",
        )
        .expect("scope should authorize with an active grant");
        let scan = create_manifest_scan_at(&database_path, scope.id).expect("scan should create");
        run_manifest_scan_at(&database_path, scan.job_id).expect("scan should complete");

        let preview = create_rename_preview_for_database(
            &database_path,
            scope.id,
            &source_path,
            "private-final.md",
        )
        .expect("preview should create");
        assert_eq!(preview.state, deskgraph_domain::ActionPlanState::Previewed);
        assert!(source_path.exists());
        assert!(!destination_path.exists());
        let explicit_payload =
            serde_json::to_string(&preview).expect("explicit preview should serialize");
        assert!(explicit_payload.contains("private-draft.md"));
        assert!(explicit_payload.contains("private-final.md"));

        let summary_payload = serde_json::to_string(
            &recent_action_plans_for_database(&database_path)
                .expect("path-free plan history should load"),
        )
        .expect("plan summaries should serialize");
        assert!(summary_payload.contains("deskgraph.action-plan-summary.v2"));
        assert!(!summary_payload.contains("private-draft.md"));
        assert!(!summary_payload.contains("private-final.md"));
        assert!(!summary_payload.contains(scope_path.to_string_lossy().as_ref()));
    }

    #[test]
    fn watch_helper_exposes_only_path_free_durable_states() {
        let directory = tempfile::tempdir().expect("fixture root should exist");
        let database_path = directory.path().join("app-data/manifest.sqlite3");
        let scope_path = directory.path().join("authorized-watch-private");
        let source_path = scope_path.join("private-watch.md");
        std::fs::create_dir(&scope_path).expect("scope should create");
        std::fs::write(&source_path, "private watch text").expect("file should create");
        initialize_manifest(&database_path).expect("manifest should initialize");
        let scope =
            authorize_scope_at(&database_path, &scope_path).expect("scope should authorize");
        let scan = create_manifest_scan_at(&database_path, scope.id).expect("scan should create");
        run_manifest_scan_at(&database_path, scan.job_id).expect("scan should complete");
        observe_watch_path_at(
            &database_path,
            scope.id,
            &source_path,
            WatchPolicy::default(),
        )
        .expect("watch hint should persist");

        let payload = serde_json::to_string(
            &recent_watch_events_for_database(&database_path).expect("watch states should load"),
        )
        .expect("watch states should serialize");
        assert!(payload.contains("deskgraph.watch-event.v1"));
        assert!(payload.contains("stabilizing"));
        assert!(!payload.contains("private-watch.md"));
        assert!(!payload.contains("private watch text"));
        assert!(!payload.contains(scope_path.to_string_lossy().as_ref()));
    }

    fn pending_hard_exclusion_preview(
        expires_at_unix_ms: i64,
        expires_at: Instant,
    ) -> PendingHardExclusionPreview {
        PendingHardExclusionPreview {
            scope_id: 1,
            base_policy_revision: 1,
            expires_at_unix_ms,
            expires_at,
            entry_kind: HardExclusionEntryKind::File,
            prepared: deskgraph_scanner::PreparedScopeExclusionBatch {
                scope_id: 1,
                exclusions: vec![deskgraph_scanner::PreparedScopeExclusion {
                    resolved_path: PathBuf::from("/native-only-picker-state"),
                    display_path: "/native-only-picker-state".to_string(),
                    path_raw: b"/native-only-picker-state".to_vec(),
                    path_key: "/native-only-picker-state".to_string(),
                    kind: ScopeExclusionKind::File,
                    identity_kind: "test".to_string(),
                    identity_key: b"identity".to_vec(),
                }],
            },
        }
    }

    fn pending_scope_root_revocation_preview(
        expires_at_unix_ms: i64,
        expires_at: Instant,
    ) -> PendingScopeRootRevocationPreview {
        PendingScopeRootRevocationPreview {
            database_preview: ScopeRootRevocationPreview {
                scope_id: 1,
                base_policy_revision: 1,
                impact: ScopeExclusionImpactPreview::default(),
                exclusion_count: 0,
            },
            expires_at_unix_ms,
            expires_at,
        }
    }

    #[test]
    fn scope_root_revocation_preview_registry_is_bounded_expiring_and_one_time() {
        let now = Instant::now();
        let display_expiry = 20_001_i64;
        let mut registry = ScopeRootRevocationPreviewRegistry::default();
        let preview_id = registry.insert_at(
            pending_scope_root_revocation_preview(display_expiry, now + Duration::from_millis(1)),
            now,
        );
        assert!(registry.take_for_confirmation_at(&preview_id, now).is_ok());
        assert_eq!(
            registry
                .take_for_confirmation_at(&preview_id, now)
                .err()
                .expect("confirmation must consume the preview"),
            "scope_root_revocation_preview_not_found"
        );
        let expired_id = registry.insert_at(
            pending_scope_root_revocation_preview(display_expiry, now),
            now,
        );
        assert_eq!(
            registry
                .take_for_confirmation_at(&expired_id, now)
                .err()
                .expect("expired preview must fail closed"),
            "scope_root_revocation_preview_expired"
        );
        let oldest_id = registry.insert_at(
            pending_scope_root_revocation_preview(display_expiry, now + Duration::from_secs(1)),
            now,
        );
        for offset in 1..=MAX_PENDING_SCOPE_ROOT_REVOCATION_PREVIEWS {
            registry.insert_at(
                pending_scope_root_revocation_preview(
                    display_expiry + offset as i64,
                    now + Duration::from_secs(1 + offset as u64),
                ),
                now,
            );
        }
        assert_eq!(
            registry.entries.len(),
            MAX_PENDING_SCOPE_ROOT_REVOCATION_PREVIEWS
        );
        assert_eq!(
            registry
                .take_for_confirmation_at(&oldest_id, now)
                .err()
                .expect("oldest preview must be evicted"),
            "scope_root_revocation_preview_not_found"
        );
        registry.discard_scope(1);
        assert!(registry.entries.is_empty());
    }

    #[test]
    fn hard_exclusion_preview_registry_is_bounded_expiring_and_one_time() {
        let now = Instant::now();
        let display_expiry = 10_001_i64;
        let mut registry = HardExclusionPreviewRegistry::default();
        let preview_id = registry.insert_at(
            pending_hard_exclusion_preview(display_expiry, now + Duration::from_millis(1)),
            now,
        );
        assert!(registry.take_for_confirmation_at(&preview_id, now).is_ok());
        assert_eq!(
            registry
                .take_for_confirmation_at(&preview_id, now)
                .err()
                .expect("a confirmation must consume the preview"),
            "hard_exclusion_preview_not_found"
        );

        let expired_id =
            registry.insert_at(pending_hard_exclusion_preview(display_expiry, now), now);
        assert_eq!(
            registry
                .take_for_confirmation_at(&expired_id, now)
                .err()
                .expect("expired preview must not be confirmable"),
            "hard_exclusion_preview_expired"
        );

        let first_id = registry.insert_at(
            pending_hard_exclusion_preview(display_expiry, now + Duration::from_secs(1)),
            now,
        );
        for offset in 1..=MAX_PENDING_HARD_EXCLUSION_PREVIEWS {
            registry.insert_at(
                pending_hard_exclusion_preview(
                    display_expiry + offset as i64,
                    now + Duration::from_secs(1 + offset as u64),
                ),
                now,
            );
        }
        assert_eq!(registry.entries.len(), MAX_PENDING_HARD_EXCLUSION_PREVIEWS);
        assert_eq!(
            registry
                .take_for_confirmation_at(&first_id, now)
                .err()
                .expect("oldest preview must be evicted at the fixed bound"),
            "hard_exclusion_preview_not_found"
        );

        registry.discard("unknown-preview");
        let discard_id = registry.insert_at(
            pending_hard_exclusion_preview(display_expiry, now + Duration::from_secs(2)),
            now,
        );
        registry.discard(&discard_id);
        registry.discard(&discard_id);
        assert!(
            !registry.entries.contains_key(&discard_id),
            "discard must remain idempotent after a preview is gone"
        );
    }

    #[test]
    fn hard_exclusion_commit_serializes_without_native_picker_paths() {
        let source_marker = "native-picker-path-must-not-cross-ipc";
        let commit = HardExclusionCommitResponse {
            api_version: "deskgraph.hard-exclusion-commit.v1",
            scope_id: 7,
            policy_revision: 3,
            exclusions: 1,
            purge: HardExclusionImpactResponse {
                location_count: 2,
                content_chunk_count: 3,
                graph_fact_count: 4,
                derived_candidate_count: 5,
                action_plan_count: 6,
                cleanup_action_plan_count: 7,
                pending_job_count: 8,
                blocking_action_count: 0,
            },
            source_files_changed: false,
            automatic_scans_started: 0,
            automatic_extractions_started: 0,
        };
        let payload = serde_json::to_string(&commit).expect("commit response should serialize");
        assert!(payload.contains("deskgraph.hard-exclusion-commit.v1"));
        assert!(!payload.contains(source_marker));
        assert!(!payload.contains("native-only-picker-state"));
        assert!(!payload.contains("identity"));
    }

    #[test]
    fn hard_exclusion_impact_keeps_facts_candidates_and_plans_distinct() {
        let response = map_hard_exclusion_impact(ScopeExclusionImpactPreview {
            edge_count: 2,
            project_count: 3,
            relation_count: 5,
            screenshot_group_count: 7,
            action_plan_count: 11,
            cleanup_action_plan_count: 13,
            blocking_action_count: 17,
            ..ScopeExclusionImpactPreview::default()
        });

        assert_eq!(response.graph_fact_count, 2);
        assert_eq!(response.derived_candidate_count, 15);
        assert_eq!(response.action_plan_count, 11);
        assert_eq!(response.cleanup_action_plan_count, 13);
        assert_eq!(response.blocking_action_count, 17);
    }

    #[test]
    fn live_scope_root_revocation_drops_runtime_access_and_keeps_source_files() {
        let directory = tempfile::tempdir().expect("fixture root should exist");
        let database_path = directory.path().join("app-data/manifest.sqlite3");
        let scope_path = directory.path().join("authorized-coverage");
        let source_path = scope_path.join("private-derived.md");
        std::fs::create_dir_all(&scope_path).expect("fixture folder should create");
        std::fs::write(&source_path, "source must remain on disk")
            .expect("fixture file should create");
        let source_bytes_before = std::fs::read(&source_path).expect("source bytes should load");
        let source_modified_before = std::fs::metadata(&source_path)
            .and_then(|metadata| metadata.modified())
            .expect("source modified time should load");
        let directory_entries_before = std::fs::read_dir(&scope_path)
            .expect("scope entries should load")
            .map(|entry| entry.expect("scope entry should load").file_name())
            .collect::<Vec<_>>();
        initialize_manifest(&database_path).expect("manifest should initialize");

        let prepared_access = prepare_selected_scope(&scope_path).expect("scope should prepare");
        let requests = [CoverageRootAuthorizationRequest {
            requested_path: &prepared_access.resolved_path,
            grant_platform: prepared_access.platform,
            opaque_grant: &prepared_access.opaque_grant,
        }];
        let scope = authorize_coverage_roots_with_access_grants_at(&database_path, &requests)
            .expect("scope and grant should persist")
            .remove(0);
        let scan = create_manifest_scan_at(&database_path, scope.id).expect("scan should create");
        run_manifest_scan_at(&database_path, scan.job_id).expect("scan should complete");
        let mut accesses = HashMap::new();
        accesses.insert(scope.id, prepared_access.access);
        let state = start_manifest_state_with_accesses(database_path.clone(), accesses);

        let preview = preview_scope_root_revocation_for_state(&state, scope.id)
            .expect("root revocation should preview");
        assert!(preview.confirmable);
        assert!(!preview.source_files_will_change);
        assert!(preview.impact.location_count >= 2);
        assert_eq!(preview.impact.action_plan_count, 0);
        assert_eq!(preview.impact.cleanup_action_plan_count, 0);
        let preview_id = preview.preview_id.clone();
        let commit = confirm_scope_root_revocation_for_state(&state, preview.preview_id)
            .expect("root revocation should commit");
        assert!(commit.runtime_capability_dropped);
        assert!(commit.native_watch_sync_confirmed);
        assert!(!commit.native_watch_callback_retired);
        assert!(!commit.watch_runtime_stopped);
        assert!(!commit.source_files_changed);
        assert_eq!(commit.purged.action_plan_count, 0);
        assert_eq!(commit.purged.cleanup_action_plan_count, 0);
        assert_eq!(commit.revoked_scope_scans_started, 0);
        assert_eq!(commit.revoked_scope_extractions_started, 0);
        assert!(
            source_path.exists(),
            "revocation must not mutate source files"
        );
        assert_eq!(
            std::fs::read(&source_path).expect("source bytes should remain readable"),
            source_bytes_before
        );
        assert_eq!(
            std::fs::metadata(&source_path)
                .and_then(|metadata| metadata.modified())
                .expect("source modified time should remain readable"),
            source_modified_before
        );
        assert_eq!(
            std::fs::read_dir(&scope_path)
                .expect("scope entries should remain readable")
                .map(|entry| entry.expect("scope entry should load").file_name())
                .collect::<Vec<_>>(),
            directory_entries_before
        );
        assert!(
            !lock_scope_accesses(&state)
                .expect("runtime registry should load")
                .contains_key(&scope.id),
            "the live OS capability must be dropped before IPC success"
        );
        assert_eq!(
            confirm_scope_root_revocation_for_state(&state, preview_id)
                .err()
                .expect("preview reuse must fail"),
            "scope_root_revocation_preview_not_found"
        );

        let database = ManifestDatabase::open(&database_path).expect("database should reopen");
        assert_eq!(
            database
                .scope_access_grant_state(scope.id)
                .expect("grant state should load"),
            deskgraph_database::ScopeAccessGrantState::Revoked
        );
        assert_eq!(
            database
                .stats()
                .expect("manifest stats should load")
                .active_location_count,
            0
        );
    }

    #[test]
    fn scope_root_revocation_waits_for_an_in_flight_path_read() {
        let directory = tempfile::tempdir().expect("fixture root should exist");
        let database_path = directory.path().join("app-data/manifest.sqlite3");
        let scope_path = directory.path().join("authorized-coverage");
        std::fs::create_dir_all(&scope_path).expect("fixture folder should create");
        initialize_manifest(&database_path).expect("manifest should initialize");

        let prepared_access = prepare_selected_scope(&scope_path).expect("scope should prepare");
        let requests = [CoverageRootAuthorizationRequest {
            requested_path: &prepared_access.resolved_path,
            grant_platform: prepared_access.platform,
            opaque_grant: &prepared_access.opaque_grant,
        }];
        let scope = authorize_coverage_roots_with_access_grants_at(&database_path, &requests)
            .expect("scope and grant should persist")
            .remove(0);
        let mut accesses = HashMap::new();
        accesses.insert(scope.id, prepared_access.access);
        let state = Arc::new(start_manifest_state_with_accesses(
            database_path.clone(),
            accesses,
        ));
        let preview = preview_scope_root_revocation_for_state(&state, scope.id)
            .expect("root revocation should preview");

        let reader_gate = Arc::clone(&state.path_read_gate);
        let reader_accesses = Arc::clone(&state.scope_accesses);
        let (reader_started_tx, reader_started_rx) = sync_channel(1);
        let (release_reader_tx, release_reader_rx) = sync_channel(1);
        let reader = thread::spawn(move || {
            with_active_scope_read_fence(&reader_gate, &reader_accesses, scope.id, || {
                reader_started_tx
                    .send(())
                    .expect("reader start should be observed");
                release_reader_rx.recv().expect("reader should be released");
                Ok::<(), String>(())
            })
        });
        reader_started_rx
            .recv()
            .expect("reader should acquire the path fence");

        let revocation_state = Arc::clone(&state);
        let (revocation_started_tx, revocation_started_rx) = sync_channel(1);
        let revocation = thread::spawn(move || {
            revocation_started_tx
                .send(())
                .expect("revocation start should be observed");
            confirm_scope_root_revocation_for_state(&revocation_state, preview.preview_id)
        });
        revocation_started_rx
            .recv()
            .expect("revocation should start");
        thread::sleep(Duration::from_millis(25));
        assert!(
            !revocation.is_finished(),
            "revocation must wait until the active read operation releases its fence"
        );

        release_reader_tx
            .send(())
            .expect("reader release should be delivered");
        reader
            .join()
            .expect("reader thread should not panic")
            .expect("reader fence should complete");
        let commit = revocation
            .join()
            .expect("revocation thread should not panic")
            .expect("revocation should commit after the read completes");
        assert!(commit.runtime_capability_dropped);
        assert!(commit.native_watch_sync_confirmed);
        assert!(!commit.native_watch_callback_retired);
        assert!(!commit.watch_runtime_stopped);
        assert!(
            !lock_scope_accesses(&state)
                .expect("runtime registry should load")
                .contains_key(&scope.id)
        );
    }

    #[test]
    fn scope_root_revocation_rejects_changed_impact_and_requires_a_fresh_preview() {
        let directory = tempfile::tempdir().expect("fixture root should exist");
        let database_path = directory.path().join("app-data/manifest.sqlite3");
        let scope_path = directory.path().join("authorized-coverage");
        std::fs::create_dir_all(&scope_path).expect("fixture folder should create");
        initialize_manifest(&database_path).expect("manifest should initialize");

        let prepared_access = prepare_selected_scope(&scope_path).expect("scope should prepare");
        let requests = [CoverageRootAuthorizationRequest {
            requested_path: &prepared_access.resolved_path,
            grant_platform: prepared_access.platform,
            opaque_grant: &prepared_access.opaque_grant,
        }];
        let scope = authorize_coverage_roots_with_access_grants_at(&database_path, &requests)
            .expect("scope and grant should persist")
            .remove(0);
        let mut accesses = HashMap::new();
        accesses.insert(scope.id, prepared_access.access);
        let state = start_manifest_state_with_accesses(database_path.clone(), accesses);
        let stale_preview = preview_scope_root_revocation_for_state(&state, scope.id)
            .expect("root revocation should preview");
        create_manifest_scan_at(&database_path, scope.id)
            .expect("derived scan state should change after preview");

        assert_eq!(
            confirm_scope_root_revocation_for_state(&state, stale_preview.preview_id)
                .err()
                .expect("changed impact must fail closed"),
            "scope_root_revocation_preview_stale"
        );
        assert!(
            lock_scope_accesses(&state)
                .expect("runtime registry should load")
                .contains_key(&scope.id),
            "a stale preview must leave live runtime access intact"
        );
        let database = ManifestDatabase::open(&database_path).expect("database should reopen");
        assert!(
            database
                .scope_has_active_access_grant(scope.id)
                .expect("a stale preview must leave the durable grant active")
        );

        let fresh_preview = preview_scope_root_revocation_for_state(&state, scope.id)
            .expect("the changed impact should be previewed again");
        confirm_scope_root_revocation_for_state(&state, fresh_preview.preview_id)
            .expect("the fresh exact preview should commit");
        assert!(
            !lock_scope_accesses(&state)
                .expect("runtime registry should load")
                .contains_key(&scope.id)
        );
    }

    #[test]
    fn live_hard_exclusion_purges_derived_data_without_changing_the_source() {
        let directory = tempfile::tempdir().expect("fixture root should exist");
        let database_path = directory.path().join("app-data/manifest.sqlite3");
        let scope_path = directory.path().join("authorized-coverage");
        let excluded_folder = scope_path.join("excluded-folder");
        let source_path = excluded_folder.join("private-derived.md");
        std::fs::create_dir_all(&excluded_folder).expect("fixture folder should create");
        std::fs::write(&source_path, "source must remain on disk")
            .expect("fixture file should create");
        initialize_manifest(&database_path).expect("manifest should initialize");

        let prepared_access = prepare_selected_scope(&scope_path).expect("scope should prepare");
        let requests = [CoverageRootAuthorizationRequest {
            requested_path: &prepared_access.resolved_path,
            grant_platform: prepared_access.platform,
            opaque_grant: &prepared_access.opaque_grant,
        }];
        let scopes = authorize_coverage_roots_with_access_grants_at(&database_path, &requests)
            .expect("scope and grant should persist");
        let scope = scopes
            .into_iter()
            .next()
            .expect("one scope should authorize");
        let scan = create_manifest_scan_at(&database_path, scope.id).expect("scan should create");
        run_manifest_scan_at(&database_path, scan.job_id).expect("scan should complete");
        let mut accesses = HashMap::new();
        accesses.insert(scope.id, prepared_access.access);
        let state = start_manifest_state_with_accesses(database_path.clone(), accesses);

        let preview = select_hard_exclusions_preview_from_native_paths(
            &state,
            scope.id,
            HardExclusionEntryKind::Folder,
            std::slice::from_ref(&excluded_folder),
        )
        .expect("native selection should prepare a hard-exclusion preview");
        assert!(preview.confirmable);
        assert!(!preview.source_files_will_change);
        let commit = confirm_hard_exclusion_preview_for_state(&state, preview.preview_id)
            .expect("confirmed policy should apply atomically");
        assert_eq!(commit.exclusions, 1);
        assert!(!commit.source_files_changed);
        assert_eq!(commit.automatic_scans_started, 0);
        assert_eq!(commit.automatic_extractions_started, 0);
        assert!(
            source_path.exists(),
            "a policy purge must never remove the source file"
        );

        let database = ManifestDatabase::open(&database_path).expect("database should open");
        assert_eq!(
            database
                .scope_exclusions(scope.id)
                .expect("exclusions should load")
                .len(),
            1
        );
        let source_key = comparison_key(
            &std::fs::canonicalize(&source_path).expect("source should canonicalize after purge"),
        );
        assert!(
            database
                .node_id_for_path_key(scope.id, &source_key)
                .expect("node lookup should succeed")
                .is_none(),
            "derived manifest data below an exclusion must be purged"
        );
    }
}
