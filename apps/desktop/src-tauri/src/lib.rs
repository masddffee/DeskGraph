mod scope_access;

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::{
    Arc, Mutex, MutexGuard, TryLockError,
    atomic::{AtomicBool, Ordering},
    mpsc::{Receiver, RecvTimeoutError, SyncSender, TrySendError, sync_channel},
};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use deskgraph_database::{CleanupActionSelection, ManifestDatabase};
use deskgraph_domain::{
    ActionPlanPreview, ActionPlanSummary, AuthorizedScope, CleanupActionPlanPreview,
    CleanupSourceDetail, ExtractionJobProgress, ExtractionOperation, ExtractionStats, HealthReport,
    ManifestStats, ProjectCandidateDetail, ProjectDecisionKind, ProjectDiscovery, ScanJobProgress,
    ScanStatus, SearchFilters, SearchResponse, SmartCleanupInbox, SmartCleanupSourceKind,
    WatchEventProgress, collect_health_with_manifest,
};
#[cfg(test)]
use deskgraph_domain::{WatchEventReason, WatchEventStatus};
#[cfg(test)]
use deskgraph_extractors::extraction_stats_at as read_extraction_stats_at;
use deskgraph_extractors::{
    ExtractionLimits, cancel_extraction_job_at, create_screenshot_ocr_job_at, extraction_job_at,
    recent_extraction_jobs_at as read_recent_extraction_jobs_at, resume_extraction_job_at,
    run_extraction_job_at,
};
use deskgraph_projects::{
    cleanup_source_detail_at, decide_current_project_candidate_at, discover_projects_at,
    project_candidate_detail_at, refresh_smart_cleanup_inbox_at,
};
use deskgraph_retrieval::{SearchRequest, SearchSourceFilter, search_at as run_search_at};
#[cfg(test)]
use deskgraph_scanner::{authorize_scope, run_scan_job_to_terminal};
use deskgraph_scanner::{
    authorize_scope_with_access_grant, comparison_key, create_scan_job, pause_scan_job,
    resume_scan_job, run_scan_job_batch, validated_scope_root,
};
use deskgraph_telemetry::{Service, init_privacy_safe_logging};
use deskgraph_transactions::{
    create_cleanup_preview_at, create_rename_preview_at, recent_action_plans_at,
};
use deskgraph_watcher::{
    NativeWatchEventSource, PollingWatchPolicy, WatchCoordinator, WatchPolicy,
    recent_watch_events_at as read_recent_watch_events_at,
};
use scope_access::{ActiveScopeAccess, prepare_selected_scope, restore_scope_access};
use serde::Serialize;
use tauri::{AppHandle, Manager, State};
use tauri_plugin_dialog::DialogExt;
use tracing::{error, info};

const WATCH_RUNTIME_SHUTDOWN_TIMEOUT: Duration = Duration::from_secs(2);
const WATCH_RUNTIME_SHUTDOWN_POLL: Duration = Duration::from_millis(10);
const WATCH_NATIVE_RETRY_INTERVAL: Duration = Duration::from_secs(30);
const WATCH_GATE_RETRY_INTERVAL: Duration = Duration::from_millis(50);
const FOREGROUND_SCAN_BATCH_SIZE: usize = 256;
const WATCH_ADAPTER_NATIVE: &str = "native_with_periodic_reconciliation";
const WATCH_ADAPTER_PERIODIC_ONLY: &str = "periodic_reconciliation_only";

struct ManifestState {
    database_path: PathBuf,
    database_gate: Arc<Mutex<()>>,
    watch_status: Arc<Mutex<WatchRuntimeStatus>>,
    watch_stop: Arc<AtomicBool>,
    watch_wake: SyncSender<()>,
    watch_thread: Mutex<Option<JoinHandle<()>>>,
    scope_accesses: Arc<Mutex<HashMap<i64, ActiveScopeAccess>>>,
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
) -> Result<MutexGuard<'_, HashMap<i64, ActiveScopeAccess>>, String> {
    state
        .scope_accesses
        .lock()
        .map_err(|_| "scope_access_registry_poisoned".to_string())
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
    status: Arc<Mutex<WatchRuntimeStatus>>,
    scope_accesses: Arc<Mutex<HashMap<i64, ActiveScopeAccess>>>,
    polling_policy: PollingWatchPolicy,
}

fn start_manifest_state_with_accesses(
    database_path: PathBuf,
    scope_accesses: HashMap<i64, ActiveScopeAccess>,
) -> ManifestState {
    let database_gate = Arc::new(Mutex::new(()));
    let watch_stop = Arc::new(AtomicBool::new(false));
    let (watch_wake, watch_wake_receiver) = sync_channel(1);
    let polling_policy = PollingWatchPolicy::default();
    let watch_status = Arc::new(Mutex::new(WatchRuntimeStatus::starting(polling_policy)));
    let thread_database_path = database_path.clone();
    let thread_database_gate = Arc::clone(&database_gate);
    let thread_watch_stop = Arc::clone(&watch_stop);
    let thread_watch_wake = watch_wake.clone();
    let thread_watch_status = Arc::clone(&watch_status);
    let scope_accesses = Arc::new(Mutex::new(scope_accesses));
    let thread_scope_accesses = Arc::clone(&scope_accesses);
    let runtime = WatchCoordinatorRuntime {
        database_path: thread_database_path,
        database_gate: thread_database_gate,
        stop: thread_watch_stop,
        wake: thread_watch_wake,
        wake_receiver: watch_wake_receiver,
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
        watch_status,
        watch_stop,
        watch_wake,
        watch_thread: Mutex::new(watch_thread),
        scope_accesses,
    }
}

fn run_watch_coordinator(runtime: WatchCoordinatorRuntime) {
    let WatchCoordinatorRuntime {
        database_path,
        database_gate,
        stop,
        wake,
        wake_receiver,
        status,
        scope_accesses,
        polling_policy,
    } = runtime;
    let mut coordinator = match WatchCoordinator::open_requiring_active_platform_grants(
        &database_path,
        WatchPolicy::default(),
        polling_policy,
    ) {
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
        if native_source.is_none() && Instant::now() >= next_native_retry {
            let callback_wake = wake.clone();
            match NativeWatchEventSource::new(Arc::new(move || {
                notify_watch_wake(&callback_wake);
            })) {
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
                }
                Err(_) => {
                    native_source = None;
                    native_error = Some("watch_native_adapter_unavailable");
                    next_native_retry = Instant::now() + WATCH_NATIVE_RETRY_INTERVAL;
                    reconcile_after_native_change = true;
                }
            }
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

#[tauri::command]
async fn select_and_authorize_scope(
    app: AppHandle,
    state: State<'_, ManifestState>,
) -> Result<Option<AuthorizedScope>, String> {
    let Some(selected) = app.dialog().file().blocking_pick_folder() else {
        return Ok(None);
    };
    let selected_path = selected
        .into_path()
        .map_err(|_| "scope_selection_invalid".to_string())?;
    let prepared = prepare_selected_scope(&selected_path).map_err(str::to_string)?;
    let _database_guard = lock_database(&state)?;
    let scope = authorize_scope_with_access_grant_at(
        &state.database_path,
        &prepared.resolved_path,
        prepared.platform,
        &prepared.opaque_grant,
    )
    .map_err(str::to_string)?;
    lock_scope_accesses(&state)?.insert(scope.id, prepared.access);
    drop(_database_guard);
    wake_watch_runtime(&state);
    info!(event = "scope_authorized", scope_id = scope.id);
    Ok(Some(scope))
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
    let progress = tauri::async_runtime::spawn_blocking(move || {
        run_screenshot_ocr_job_at(&database_path, job_id).map_err(str::to_string)
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
            || filters.modified_since_unix_seconds.is_some()
            || filters.modified_before_unix_seconds.is_some()
            || filters.source != SearchSourceFilter::All,
        mode = "lexical"
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
    let database = ManifestDatabase::open(path).map_err(|error| error.code())?;
    authorize_scope(&database, requested_path).map_err(|error| error.code())
}

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

fn create_manifest_scan_at(path: &Path, scope_id: i64) -> Result<ScanJobProgress, &'static str> {
    let mut database = ManifestDatabase::open(path).map_err(|error| error.code())?;
    create_scan_job(&mut database, scope_id).map_err(|error| error.code())
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
            run_scan_job_batch(&mut database, job_id, FOREGROUND_SCAN_BATCH_SIZE)
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
            source: filters.source,
            extension: filters.extension.as_deref(),
            modified_since_unix_seconds: filters.modified_since_unix_seconds,
            modified_before_unix_seconds: filters.modified_before_unix_seconds,
            limit,
        },
    )
    .map_err(|error| error.code())
}

fn pause_manifest_scan_at(path: &Path, job_id: i64) -> Result<ScanJobProgress, &'static str> {
    let mut database = ManifestDatabase::open(path).map_err(|error| error.code())?;
    pause_scan_job(&mut database, job_id).map_err(|error| error.code())
}

fn resume_manifest_scan_at(path: &Path, job_id: i64) -> Result<ScanJobProgress, &'static str> {
    let mut database = ManifestDatabase::open(path).map_err(|error| error.code())?;
    resume_scan_job(&mut database, job_id).map_err(|error| error.code())
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
            select_and_authorize_scope,
            create_manifest_scan,
            run_manifest_scan,
            scan_job_status,
            recent_scan_jobs,
            project_scope_has_completed_scan,
            content_extraction_stats,
            recent_content_extractions,
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
    fn native_scope_grant_persists_restores_and_stays_out_of_ipc_scope_shape() {
        let directory = tempfile::tempdir().expect("fixture root should exist");
        let database_path = directory.path().join("app-data/manifest.sqlite3");
        let scope_path = directory.path().join("selected-scope");
        std::fs::create_dir(&scope_path).expect("scope should create");
        initialize_manifest(&database_path).expect("manifest should initialize");

        let prepared = prepare_selected_scope(&scope_path).expect("selection should prepare");
        let scope = authorize_scope_with_access_grant_at(
            &database_path,
            &prepared.resolved_path,
            prepared.platform,
            &prepared.opaque_grant,
        )
        .expect("scope and grant should persist atomically");
        drop(prepared.access);

        let restored = restore_scope_access_registry(&database_path)
            .expect("stored selection should restore safely");
        assert!(restored.contains_key(&scope.id));
        let ordinary_scopes = authorized_scopes_at(&database_path).expect("scopes should load");
        let payload = serde_json::to_string(&ordinary_scopes).expect("scopes should serialize");
        assert!(!payload.contains("opaque_grant"));
        assert!(!payload.contains("access_grant"));
        assert_eq!(ordinary_scopes, vec![scope]);
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
            search_local(
                app.state(),
                "private".to_string(),
                SearchFilters {
                    scope_id: Some(scope.id),
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
        let scope =
            authorize_scope_at(&database_path, &scope_path).expect("scope should authorize");
        let scan = create_manifest_scan_at(&database_path, scope.id).expect("scan should create");
        run_manifest_scan_at(&database_path, scan.job_id).expect("scan should complete");
        let canonical_scope =
            std::fs::canonicalize(&scope_path).expect("scope should canonicalize");
        let original_key =
            deskgraph_scanner::comparison_key(&canonical_scope.join("native-original.md"));
        let renamed_key =
            deskgraph_scanner::comparison_key(&canonical_scope.join("native-renamed.md"));

        let state = start_manifest_state(database_path.clone());
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

        let original = scope_path.join("native-original.md");
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

        let renamed = scope_path.join("native-renamed.md");
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

        let payload = serde_json::to_string(&(
            content_extraction_stats_at(&database_path).expect("stats should load"),
            recent_content_extractions_at(&database_path).expect("jobs should load"),
        ))
        .expect("payload should serialize");
        assert!(!payload.contains(private_text));
        assert!(!payload.contains("private-notes.md"));
        assert!(!payload.contains(scope_path.to_string_lossy().as_ref()));
        assert!(payload.contains("deskgraph.extraction-stats.v1"));
        assert!(payload.contains("\"status\":\"completed\""));
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

        let response = search_local_at(
            &database_path,
            "專案脈絡",
            &SearchFilters {
                scope_id: Some(scope.id),
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
    fn rename_preview_helper_changes_no_file_and_history_is_path_free() {
        let directory = tempfile::tempdir().expect("fixture root should exist");
        let database_path = directory.path().join("app-data/manifest.sqlite3");
        let scope_path = directory.path().join("authorized-actions-private");
        let source_path = scope_path.join("private-draft.md");
        let destination_path = scope_path.join("private-final.md");
        std::fs::create_dir(&scope_path).expect("scope should create");
        std::fs::write(&source_path, "private action text").expect("file should create");
        initialize_manifest(&database_path).expect("manifest should initialize");
        let scope =
            authorize_scope_at(&database_path, &scope_path).expect("scope should authorize");
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
}
