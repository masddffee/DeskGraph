use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::fs::{self, File, Metadata};
use std::path::{Component, Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use deskgraph_database::{
    DatabaseError, ManifestDatabase, QueuedPath, WatchObservationWrite, WatchSnapshot,
    WatchSnapshotKind,
};
use deskgraph_domain::{ScanStatus, WatchEventProgress, WatchEventReason, WatchEventStatus};
use deskgraph_identity::{
    IdentityNodeKind, comparison_key, has_hidden_or_system_attribute, is_symlink_or_reparse_point,
    path_from_raw, path_to_raw, platform_identity, platform_identity_for_open_file,
};
use deskgraph_scanner::{
    ScannerError, is_temporary_download_path, resume_scan_job, run_scan_job_batch,
    run_scan_job_to_terminal, validated_scope_root,
};

mod native;

pub use native::NativeWatchEventSource;
use native::{MAX_NATIVE_SIGNALS_PER_CYCLE, NativeWatchScope};

const DEFAULT_STABILITY_WINDOW_MS: i64 = 1_000;
const MIN_STABILITY_WINDOW_MS: i64 = 250;
const MAX_STABILITY_WINDOW_MS: i64 = 60_000;
const DEFAULT_POLL_INTERVAL_MS: i64 = 300_000;
const MIN_POLL_INTERVAL_MS: i64 = 5_000;
const MAX_POLL_INTERVAL_MS: i64 = 3_600_000;
const MAX_ACTIVE_EVENTS_PER_CYCLE: usize = 64;
const MAX_RECONCILIATIONS_PER_CYCLE: usize = 1;
const MAX_SCOPES_SCHEDULED_PER_CYCLE: usize = 4;
const COORDINATOR_RECONCILIATION_BATCH_SIZE: usize = 256;
const COORDINATOR_ACTIVE_RETRY_MS: i64 = 50;
const COORDINATOR_EVENT_CONTENTION_RETRY_MS: i64 = 1_000;
const COORDINATOR_SCOPE_SCHEDULE_RETRY_MS: i64 = 1_000;
const COORDINATOR_SCOPE_ERROR_RETRY_MS: i64 = 30_000;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct WatchPolicy {
    stability_window_ms: i64,
}

impl WatchPolicy {
    pub fn new(stability_window_ms: i64) -> Result<Self, WatcherError> {
        if !(MIN_STABILITY_WINDOW_MS..=MAX_STABILITY_WINDOW_MS).contains(&stability_window_ms) {
            return Err(WatcherError::InvalidPolicy);
        }
        Ok(Self {
            stability_window_ms,
        })
    }
}

impl Default for WatchPolicy {
    fn default() -> Self {
        Self {
            stability_window_ms: DEFAULT_STABILITY_WINDOW_MS,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PollingWatchPolicy {
    poll_interval_ms: i64,
}

impl PollingWatchPolicy {
    pub fn new(poll_interval_ms: i64) -> Result<Self, WatcherError> {
        if !(MIN_POLL_INTERVAL_MS..=MAX_POLL_INTERVAL_MS).contains(&poll_interval_ms) {
            return Err(WatcherError::InvalidPollingPolicy);
        }
        Ok(Self { poll_interval_ms })
    }

    pub fn poll_interval_ms(self) -> i64 {
        self.poll_interval_ms
    }
}

impl Default for PollingWatchPolicy {
    fn default() -> Self {
        Self {
            poll_interval_ms: DEFAULT_POLL_INTERVAL_MS,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WatchHint {
    pub scope_id: i64,
    pub path: PathBuf,
}

pub trait WatchEventSource {
    fn next_hint(&mut self) -> Result<Option<WatchHint>, WatcherError>;
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WatchCycleReport {
    pub api_version: &'static str,
    pub cycle_unix_ms: i64,
    pub authorized_scope_count: u64,
    pub active_event_count: u64,
    pub advanced_event_count: u64,
    pub completed_event_count: u64,
    pub deferred_event_count: u64,
    pub scheduled_scope_count: u64,
    pub deferred_scope_count: u64,
    pub degraded_scope_count: u64,
    pub native_signal_count: u64,
    pub native_hint_scope_count: u64,
    pub native_overflow_count: u64,
    pub native_reconcile_all: bool,
    pub native_source_failed: bool,
    pub native_more_pending: bool,
    pub forced_scope_reconciliation_count: u64,
    pub next_wake_unix_ms: i64,
    pub last_error_code: Option<&'static str>,
}

impl WatchCycleReport {
    pub const API_VERSION: &str = "deskgraph.watch-cycle.v1";
}

pub struct WatchCoordinator {
    database: ManifestDatabase,
    watch_policy: WatchPolicy,
    polling_policy: PollingWatchPolicy,
    next_poll_by_scope: BTreeMap<i64, i64>,
    deferred_scope_due_at: BTreeMap<i64, i64>,
    event_retry_at: BTreeMap<i64, i64>,
    scope_errors: BTreeMap<i64, &'static str>,
    force_reconciliation_scopes: BTreeSet<i64>,
}

impl WatchCoordinator {
    pub fn open(
        database_path: &Path,
        watch_policy: WatchPolicy,
        polling_policy: PollingWatchPolicy,
    ) -> Result<Self, WatcherError> {
        let database = ManifestDatabase::open(database_path)?;
        Ok(Self::from_database(database, watch_policy, polling_policy))
    }

    pub fn from_database(
        database: ManifestDatabase,
        watch_policy: WatchPolicy,
        polling_policy: PollingWatchPolicy,
    ) -> Self {
        Self {
            database,
            watch_policy,
            polling_policy,
            next_poll_by_scope: BTreeMap::new(),
            deferred_scope_due_at: BTreeMap::new(),
            event_retry_at: BTreeMap::new(),
            scope_errors: BTreeMap::new(),
            force_reconciliation_scopes: BTreeSet::new(),
        }
    }

    pub fn synchronize_native_event_source(
        &mut self,
        source: &mut NativeWatchEventSource,
    ) -> Result<bool, WatcherError> {
        let now_unix_ms = unix_ms()?;
        let watchable_scope_ids = self.database.watchable_scope_ids()?;
        let watchable_scope_set = watchable_scope_ids.iter().copied().collect::<BTreeSet<_>>();
        let mut scopes = Vec::with_capacity(watchable_scope_ids.len());
        for scope_id in watchable_scope_ids {
            match validated_scope_root(&self.database, scope_id) {
                Ok(root) => scopes.push(NativeWatchScope { scope_id, root }),
                Err(error) => {
                    self.scope_errors.insert(scope_id, error.code());
                    self.next_poll_by_scope.insert(scope_id, now_unix_ms);
                }
            }
        }
        self.force_reconciliation_scopes
            .retain(|scope_id| watchable_scope_set.contains(scope_id));
        source.synchronize(scopes)
    }

    pub fn request_all_scope_reconciliation_at(
        &mut self,
        now_unix_ms: i64,
    ) -> Result<(), WatcherError> {
        if now_unix_ms < 0 {
            return Err(WatcherError::InvalidTimestamp);
        }
        for scope_id in self.database.watchable_scope_ids()? {
            self.next_poll_by_scope.insert(scope_id, now_unix_ms);
            self.force_reconciliation_scopes.insert(scope_id);
        }
        Ok(())
    }

    pub fn request_all_scope_reconciliation(&mut self) -> Result<(), WatcherError> {
        self.request_all_scope_reconciliation_at(unix_ms()?)
    }

    pub fn run_cycle_with_native_event_source(
        &mut self,
        source: &NativeWatchEventSource,
    ) -> Result<WatchCycleReport, WatcherError> {
        self.run_cycle_with_native_event_source_at_time(source, unix_ms()?)
    }

    pub fn run_cycle_with_native_event_source_at_time(
        &mut self,
        source: &NativeWatchEventSource,
        now_unix_ms: i64,
    ) -> Result<WatchCycleReport, WatcherError> {
        self.run_cycle_with_native_batch_at_time(
            source.drain(MAX_NATIVE_SIGNALS_PER_CYCLE),
            now_unix_ms,
        )
    }

    fn run_cycle_with_native_batch_at_time(
        &mut self,
        batch: native::NativeWatchBatch,
        now_unix_ms: i64,
    ) -> Result<WatchCycleReport, WatcherError> {
        if now_unix_ms < 0 {
            return Err(WatcherError::InvalidTimestamp);
        }
        if batch.reconcile_all || batch.source_failed {
            self.request_all_scope_reconciliation_at(now_unix_ms)?;
        }
        for scope_id in &batch.reconcile_scope_ids {
            if self.database.scope_has_completed_scan(*scope_id)? {
                self.next_poll_by_scope.insert(*scope_id, now_unix_ms);
                self.force_reconciliation_scopes.insert(*scope_id);
            }
        }
        let hint_scope_count =
            u64::try_from(batch.hints.len()).map_err(|_| WatcherError::InvalidRuntimeCount)?;
        for hint in batch.hints {
            if !self.database.scope_has_completed_scan(hint.scope_id)? {
                continue;
            }
            if let Err(error) = observe_watch_path_at_time(
                &mut self.database,
                hint.scope_id,
                &hint.path,
                self.watch_policy,
                now_unix_ms,
            ) {
                self.scope_errors.insert(hint.scope_id, error.code());
                self.next_poll_by_scope.insert(hint.scope_id, now_unix_ms);
            }
        }

        let mut report = self.run_cycle_at_time(now_unix_ms)?;
        report.native_signal_count = batch.signal_count;
        report.native_hint_scope_count = hint_scope_count;
        report.native_overflow_count = batch.overflow_count;
        report.native_reconcile_all = batch.reconcile_all;
        report.native_source_failed = batch.source_failed;
        report.native_more_pending = batch.more_pending;
        if batch.source_failed {
            report.last_error_code = Some("watch_native_source_failed");
        }
        Ok(report)
    }

    pub fn run_cycle(&mut self) -> Result<WatchCycleReport, WatcherError> {
        self.run_cycle_at_time(unix_ms()?)
    }

    pub fn run_cycle_at_time(
        &mut self,
        now_unix_ms: i64,
    ) -> Result<WatchCycleReport, WatcherError> {
        if now_unix_ms < 0 {
            return Err(WatcherError::InvalidTimestamp);
        }

        let scopes = self.database.list_scopes()?;
        let scope_ids = self
            .database
            .watchable_scope_ids()?
            .into_iter()
            .collect::<BTreeSet<_>>();
        self.next_poll_by_scope
            .retain(|scope_id, _| scope_ids.contains(scope_id));
        self.deferred_scope_due_at
            .retain(|scope_id, _| scope_ids.contains(scope_id));
        self.scope_errors
            .retain(|scope_id, _| scope_ids.contains(scope_id));
        self.force_reconciliation_scopes
            .retain(|scope_id| scope_ids.contains(scope_id));
        for scope_id in &scope_ids {
            self.next_poll_by_scope
                .entry(*scope_id)
                .or_insert(now_unix_ms);
        }

        let active_before = self.database.active_watch_events()?;
        let mut report = WatchCycleReport {
            api_version: WatchCycleReport::API_VERSION,
            cycle_unix_ms: now_unix_ms,
            authorized_scope_count: u64::try_from(scopes.len())
                .map_err(|_| WatcherError::InvalidRuntimeCount)?,
            active_event_count: u64::try_from(active_before.len())
                .map_err(|_| WatcherError::InvalidRuntimeCount)?,
            advanced_event_count: 0,
            completed_event_count: 0,
            deferred_event_count: 0,
            scheduled_scope_count: 0,
            deferred_scope_count: 0,
            degraded_scope_count: 0,
            native_signal_count: 0,
            native_hint_scope_count: 0,
            native_overflow_count: 0,
            native_reconcile_all: false,
            native_source_failed: false,
            native_more_pending: false,
            forced_scope_reconciliation_count: 0,
            next_wake_unix_ms: now_unix_ms.saturating_add(self.polling_policy.poll_interval_ms),
            last_error_code: self.scope_errors.values().next().copied(),
        };
        let mut reconciliation_count = 0;

        for event in active_before.iter().take(MAX_ACTIVE_EVENTS_PER_CYCLE) {
            let next_poll = now_unix_ms.saturating_add(self.polling_policy.poll_interval_ms);
            self.next_poll_by_scope
                .entry(event.scope_id)
                .and_modify(|due_at| *due_at = (*due_at).max(next_poll))
                .or_insert(next_poll);
            if self
                .event_retry_at
                .get(&event.event_id)
                .is_some_and(|retry_at| *retry_at > now_unix_ms)
            {
                report.deferred_event_count = report.deferred_event_count.saturating_add(1);
                continue;
            }
            let created_at_unix_ms = self.database.watch_event_created_at(event.event_id)?;
            let maximum_stabilizing_age_reached = event.status == WatchEventStatus::Stabilizing
                && now_unix_ms.saturating_sub(created_at_unix_ms)
                    >= self.polling_policy.poll_interval_ms;
            let explicit_force_requested =
                self.force_reconciliation_scopes.contains(&event.scope_id);
            let force_reconciliation = event.status == WatchEventStatus::Stabilizing
                && (explicit_force_requested || maximum_stabilizing_age_reached);
            let is_due = force_reconciliation
                || event.status == WatchEventStatus::Reconciling
                || now_unix_ms >= event.stable_after_unix_ms;
            if !is_due {
                continue;
            }
            if reconciliation_count >= MAX_RECONCILIATIONS_PER_CYCLE {
                report.deferred_event_count = report.deferred_event_count.saturating_add(1);
                continue;
            }

            let advanced = if force_reconciliation {
                force_scope_metadata_reconciliation_batch_at_time(
                    &mut self.database,
                    event.event_id,
                    COORDINATOR_RECONCILIATION_BATCH_SIZE,
                    now_unix_ms,
                )
            } else {
                advance_watch_event_batch_at_time(
                    &mut self.database,
                    event.event_id,
                    self.watch_policy,
                    COORDINATOR_RECONCILIATION_BATCH_SIZE,
                    now_unix_ms,
                )
            };
            match advanced {
                Ok(progress) => {
                    if force_reconciliation {
                        self.force_reconciliation_scopes.remove(&event.scope_id);
                        report.forced_scope_reconciliation_count =
                            report.forced_scope_reconciliation_count.saturating_add(1);
                    }
                    self.event_retry_at.remove(&event.event_id);
                    reconciliation_count += 1;
                    report.advanced_event_count = report.advanced_event_count.saturating_add(1);
                    if progress.is_terminal() {
                        if progress.status == WatchEventStatus::Completed {
                            report.completed_event_count =
                                report.completed_event_count.saturating_add(1);
                            self.scope_errors.remove(&progress.scope_id);
                        } else if progress.status == WatchEventStatus::Failed {
                            self.scope_errors
                                .insert(progress.scope_id, watch_progress_error_code(&progress));
                        }
                        let next_poll = if self
                            .force_reconciliation_scopes
                            .contains(&progress.scope_id)
                        {
                            now_unix_ms
                        } else {
                            now_unix_ms.saturating_add(self.polling_policy.poll_interval_ms)
                        };
                        self.next_poll_by_scope.insert(progress.scope_id, next_poll);
                    }
                }
                Err(error) if is_retryable_scan_contention(&error) => {
                    report.deferred_event_count = report.deferred_event_count.saturating_add(1);
                    self.event_retry_at.insert(
                        event.event_id,
                        now_unix_ms.saturating_add(COORDINATOR_EVENT_CONTENTION_RETRY_MS),
                    );
                }
                Err(error) => {
                    self.scope_errors.insert(event.scope_id, error.code());
                    self.event_retry_at.insert(
                        event.event_id,
                        now_unix_ms.saturating_add(COORDINATOR_SCOPE_ERROR_RETRY_MS),
                    );
                }
            }
        }
        let unvisited_active_count = active_before
            .len()
            .saturating_sub(MAX_ACTIVE_EVENTS_PER_CYCLE);
        report.deferred_event_count = report
            .deferred_event_count
            .saturating_add(u64::try_from(unvisited_active_count).unwrap_or(u64::MAX));

        let active_scope_ids = self
            .database
            .active_watch_events()?
            .into_iter()
            .map(|event| event.scope_id)
            .collect::<BTreeSet<_>>();
        self.deferred_scope_due_at
            .retain(|scope_id, _| !active_scope_ids.contains(scope_id));
        let mut due_scopes = self
            .next_poll_by_scope
            .iter()
            .filter(|(scope_id, due_at)| {
                **due_at <= now_unix_ms && !active_scope_ids.contains(scope_id)
            })
            .map(|(scope_id, due_at)| (*scope_id, *due_at))
            .collect::<Vec<_>>();
        due_scopes.sort_unstable_by_key(|(scope_id, due_at)| {
            self.deferred_scope_due_at
                .get(scope_id)
                .map_or((1, *due_at, *scope_id), |original_due_at| {
                    (0, *original_due_at, *scope_id)
                })
        });

        for (scope_id, due_at) in due_scopes.iter().skip(MAX_SCOPES_SCHEDULED_PER_CYCLE) {
            self.deferred_scope_due_at
                .entry(*scope_id)
                .or_insert(*due_at);
            self.next_poll_by_scope.insert(
                *scope_id,
                now_unix_ms.saturating_add(COORDINATOR_SCOPE_SCHEDULE_RETRY_MS),
            );
        }

        for (scope_id, _) in due_scopes.into_iter().take(MAX_SCOPES_SCHEDULED_PER_CYCLE) {
            self.deferred_scope_due_at.remove(&scope_id);
            let scheduled = validated_scope_root(&self.database, scope_id)
                .map_err(WatcherError::from)
                .and_then(|root| {
                    observe_watch_path_at_time(
                        &mut self.database,
                        scope_id,
                        &root,
                        self.watch_policy,
                        now_unix_ms,
                    )
                });
            match scheduled {
                Ok(_) => {
                    report.scheduled_scope_count = report.scheduled_scope_count.saturating_add(1);
                    self.next_poll_by_scope.insert(
                        scope_id,
                        now_unix_ms.saturating_add(self.polling_policy.poll_interval_ms),
                    );
                }
                Err(error) => {
                    self.scope_errors.insert(scope_id, error.code());
                    self.next_poll_by_scope.insert(
                        scope_id,
                        now_unix_ms.saturating_add(COORDINATOR_SCOPE_ERROR_RETRY_MS),
                    );
                }
            }
        }

        let active_after = self.database.active_watch_events()?;
        let active_event_ids = active_after
            .iter()
            .map(|event| event.event_id)
            .collect::<BTreeSet<_>>();
        self.event_retry_at
            .retain(|event_id, _| active_event_ids.contains(event_id));
        report.active_event_count =
            u64::try_from(active_after.len()).map_err(|_| WatcherError::InvalidRuntimeCount)?;
        report.degraded_scope_count = u64::try_from(self.scope_errors.len())
            .map_err(|_| WatcherError::InvalidRuntimeCount)?;
        report.deferred_scope_count = u64::try_from(self.deferred_scope_due_at.len())
            .map_err(|_| WatcherError::InvalidRuntimeCount)?;
        report.last_error_code = self.scope_errors.values().next().copied();
        let mut next_active_wake = Vec::with_capacity(active_after.len());
        for event in &active_after {
            let normal_wake = match event.status {
                WatchEventStatus::Stabilizing => {
                    let maximum_age_wake = self
                        .database
                        .watch_event_created_at(event.event_id)?
                        .saturating_add(self.polling_policy.poll_interval_ms);
                    event.stable_after_unix_ms.min(maximum_age_wake)
                }
                WatchEventStatus::Reconciling => {
                    now_unix_ms.saturating_add(COORDINATOR_ACTIVE_RETRY_MS)
                }
                _ => now_unix_ms.saturating_add(self.polling_policy.poll_interval_ms),
            };
            next_active_wake.push(
                self.event_retry_at
                    .get(&event.event_id)
                    .copied()
                    .unwrap_or(normal_wake)
                    .max(normal_wake),
            );
        }
        let next_poll_wake = self.next_poll_by_scope.values().copied();
        report.next_wake_unix_ms = next_active_wake
            .into_iter()
            .chain(next_poll_wake)
            .min()
            .unwrap_or_else(|| now_unix_ms.saturating_add(self.polling_policy.poll_interval_ms))
            .max(now_unix_ms.saturating_add(COORDINATOR_ACTIVE_RETRY_MS));
        Ok(report)
    }
}

#[derive(Debug)]
pub enum WatcherError {
    Database(DatabaseError),
    Scanner(ScannerError),
    InvalidPolicy,
    InvalidPollingPolicy,
    InvalidRuntimeCount,
    InvalidTimestamp,
    ObservedPathMustBeAbsolute,
    ObservedPathOutsideScope,
    ObservedPathDecodeFailed,
    SymlinkOrReparsePointDenied,
    SourceUnavailable,
    SourceIdentityChanged,
    EventSourceFailed,
}

impl WatcherError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::Database(error) => error.code(),
            Self::Scanner(error) => error.code(),
            Self::InvalidPolicy => "watch_policy_invalid",
            Self::InvalidPollingPolicy => "watch_polling_policy_invalid",
            Self::InvalidRuntimeCount => "watch_runtime_count_out_of_range",
            Self::InvalidTimestamp => "watch_timestamp_invalid",
            Self::ObservedPathMustBeAbsolute => "watch_path_must_be_absolute",
            Self::ObservedPathOutsideScope => "watch_path_outside_scope",
            Self::ObservedPathDecodeFailed => "watch_path_decode_failed",
            Self::SymlinkOrReparsePointDenied => "watch_symlink_or_reparse_denied",
            Self::SourceUnavailable => "watch_source_unavailable",
            Self::SourceIdentityChanged => "watch_source_identity_changed",
            Self::EventSourceFailed => "watch_event_source_failed",
        }
    }
}

impl fmt::Display for WatcherError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.code())
    }
}

impl std::error::Error for WatcherError {}

impl From<DatabaseError> for WatcherError {
    fn from(error: DatabaseError) -> Self {
        Self::Database(error)
    }
}

impl From<ScannerError> for WatcherError {
    fn from(error: ScannerError) -> Self {
        Self::Scanner(error)
    }
}

fn is_retryable_scan_contention(error: &WatcherError) -> bool {
    matches!(
        error,
        WatcherError::Database(DatabaseError::ScanJobAlreadyActive | DatabaseError::ScanJobBusy)
            | WatcherError::Scanner(ScannerError::Database(
                DatabaseError::ScanJobAlreadyActive | DatabaseError::ScanJobBusy
            ))
    )
}

fn watch_progress_error_code(progress: &WatchEventProgress) -> &'static str {
    match progress.reason {
        Some(WatchEventReason::SourceUnavailable) => "watch_source_unavailable",
        Some(WatchEventReason::ReconcileFailed) => "watch_reconciliation_failed",
        _ => "watch_event_failed",
    }
}

#[derive(Debug)]
struct ValidatedHint {
    path: PathBuf,
    path_raw: Vec<u8>,
    path_key: String,
    snapshot: WatchSnapshot,
}

enum EvaluatedHint {
    Track(ValidatedHint),
    Ignore(ValidatedHint, WatchEventReason),
}

pub fn observe_watch_path_at(
    database_path: &Path,
    scope_id: i64,
    observed_path: &Path,
    policy: WatchPolicy,
) -> Result<WatchEventProgress, WatcherError> {
    let mut database = ManifestDatabase::open(database_path)?;
    observe_watch_path(&mut database, scope_id, observed_path, policy)
}

pub fn observe_watch_path(
    database: &mut ManifestDatabase,
    scope_id: i64,
    observed_path: &Path,
    policy: WatchPolicy,
) -> Result<WatchEventProgress, WatcherError> {
    observe_watch_path_at_time(database, scope_id, observed_path, policy, unix_ms()?)
}

pub fn ingest_next_source_hint_at_time(
    database: &mut ManifestDatabase,
    source: &mut impl WatchEventSource,
    policy: WatchPolicy,
    now_unix_ms: i64,
) -> Result<Option<WatchEventProgress>, WatcherError> {
    let Some(hint) = source.next_hint()? else {
        return Ok(None);
    };
    observe_watch_path_at_time(database, hint.scope_id, &hint.path, policy, now_unix_ms).map(Some)
}

pub fn observe_watch_path_at_time(
    database: &mut ManifestDatabase,
    scope_id: i64,
    observed_path: &Path,
    policy: WatchPolicy,
    now_unix_ms: i64,
) -> Result<WatchEventProgress, WatcherError> {
    if !database.scope_has_completed_scan(scope_id)? {
        return Err(DatabaseError::WatchScopeInitialScanRequired.into());
    }
    let stable_after = stable_after(now_unix_ms, policy)?;
    match evaluate_hint(database, scope_id, observed_path)? {
        EvaluatedHint::Track(hint) => database
            .record_watch_observation_at(WatchObservationWrite {
                scope_id,
                path_raw: &hint.path_raw,
                path_key: &hint.path_key,
                snapshot: &hint.snapshot,
                stable_after_unix_ms: stable_after,
                ignored_reason: None,
                observed_at_unix_ms: now_unix_ms,
            })
            .map(|event| event.progress)
            .map_err(Into::into),
        EvaluatedHint::Ignore(hint, reason) => database
            .record_watch_observation_at(WatchObservationWrite {
                scope_id,
                path_raw: &hint.path_raw,
                path_key: &hint.path_key,
                snapshot: &hint.snapshot,
                stable_after_unix_ms: now_unix_ms,
                ignored_reason: Some(reason),
                observed_at_unix_ms: now_unix_ms,
            })
            .map(|event| event.progress)
            .map_err(Into::into),
    }
}

pub fn advance_watch_event_at(
    database_path: &Path,
    event_id: i64,
    policy: WatchPolicy,
) -> Result<WatchEventProgress, WatcherError> {
    let mut database = ManifestDatabase::open(database_path)?;
    advance_watch_event(&mut database, event_id, policy)
}

pub fn advance_watch_event(
    database: &mut ManifestDatabase,
    event_id: i64,
    policy: WatchPolicy,
) -> Result<WatchEventProgress, WatcherError> {
    advance_watch_event_at_time(database, event_id, policy, unix_ms()?)
}

pub fn advance_watch_event_at_time(
    database: &mut ManifestDatabase,
    event_id: i64,
    policy: WatchPolicy,
    now_unix_ms: i64,
) -> Result<WatchEventProgress, WatcherError> {
    advance_watch_event_with_mode(
        database,
        event_id,
        policy,
        now_unix_ms,
        ReconciliationRunMode::ToTerminal,
    )
}

fn advance_watch_event_batch_at_time(
    database: &mut ManifestDatabase,
    event_id: i64,
    policy: WatchPolicy,
    batch_size: usize,
    now_unix_ms: i64,
) -> Result<WatchEventProgress, WatcherError> {
    advance_watch_event_with_mode(
        database,
        event_id,
        policy,
        now_unix_ms,
        ReconciliationRunMode::Batch(batch_size),
    )
}

fn force_scope_metadata_reconciliation_batch_at_time(
    database: &mut ManifestDatabase,
    event_id: i64,
    batch_size: usize,
    now_unix_ms: i64,
) -> Result<WatchEventProgress, WatcherError> {
    if now_unix_ms < 0 {
        return Err(WatcherError::InvalidTimestamp);
    }
    let event = database.watch_event(event_id)?;
    if event.progress.is_terminal() {
        return Ok(event.progress);
    }
    if event.progress.status != WatchEventStatus::Stabilizing {
        return Err(WatcherError::Database(
            DatabaseError::InvalidWatchEventState,
        ));
    }

    // Native overflow/source recovery and maximum coalescing age reconcile
    // metadata only through the same atomic scanner. They never authorize
    // content extraction or a filesystem action.
    let canonical_root = validated_scope_root(database, event.progress.scope_id)?;
    let root = QueuedPath {
        path_raw: path_to_raw(&canonical_root),
        path_key: comparison_key(&canonical_root),
        parent_identity_key: None,
        is_root: true,
    };
    database.begin_forced_watch_metadata_reconciliation_at(event_id, &root, now_unix_ms)?;
    finish_reconciliation(
        database,
        event_id,
        now_unix_ms,
        ReconciliationRunMode::Batch(batch_size),
    )
}

#[derive(Clone, Copy)]
enum ReconciliationRunMode {
    ToTerminal,
    Batch(usize),
}

fn advance_watch_event_with_mode(
    database: &mut ManifestDatabase,
    event_id: i64,
    policy: WatchPolicy,
    now_unix_ms: i64,
    run_mode: ReconciliationRunMode,
) -> Result<WatchEventProgress, WatcherError> {
    if now_unix_ms < 0 {
        return Err(WatcherError::InvalidTimestamp);
    }
    let event = database.watch_event(event_id)?;
    if event.progress.is_terminal() {
        return Ok(event.progress);
    }
    if event.progress.status == WatchEventStatus::Reconciling {
        return finish_reconciliation(database, event_id, now_unix_ms, run_mode);
    }
    if event.progress.status != WatchEventStatus::Stabilizing {
        return Err(WatcherError::Database(
            DatabaseError::InvalidWatchEventState,
        ));
    }
    if now_unix_ms < event.progress.stable_after_unix_ms {
        return Ok(event.progress);
    }

    let observed_path =
        path_from_raw(&event.path_raw).map_err(|_| WatcherError::ObservedPathDecodeFailed)?;
    let evaluated = match evaluate_hint(database, event.progress.scope_id, &observed_path) {
        Ok(evaluated) => evaluated,
        Err(WatcherError::SourceUnavailable) => {
            return database
                .fail_watch_event_at(event_id, WatchEventReason::SourceUnavailable, now_unix_ms)
                .map_err(Into::into);
        }
        Err(error) => return Err(error),
    };
    let hint = match evaluated {
        EvaluatedHint::Ignore(_, reason) => {
            return database
                .mark_watch_event_ignored_at(event_id, reason, now_unix_ms)
                .map_err(Into::into);
        }
        EvaluatedHint::Track(hint) => hint,
    };
    if hint.path_key != event.path_key || hint.snapshot != event.snapshot {
        return record_changed_snapshot(
            database,
            event.progress.scope_id,
            &hint,
            policy,
            now_unix_ms,
        );
    }
    if hint.snapshot.kind == WatchSnapshotKind::File && !open_file_matches_snapshot(&hint)? {
        let refreshed = match evaluate_hint(database, event.progress.scope_id, &hint.path)? {
            EvaluatedHint::Track(hint) => hint,
            EvaluatedHint::Ignore(_, reason) => {
                return database
                    .mark_watch_event_ignored_at(event_id, reason, now_unix_ms)
                    .map_err(Into::into);
            }
        };
        return record_changed_snapshot(
            database,
            event.progress.scope_id,
            &refreshed,
            policy,
            now_unix_ms,
        );
    }

    let canonical_root = validated_scope_root(database, event.progress.scope_id)?;
    let root = QueuedPath {
        path_raw: path_to_raw(&canonical_root),
        path_key: comparison_key(&canonical_root),
        parent_identity_key: None,
        is_root: true,
    };
    database.begin_watch_reconciliation_at(event_id, &root, now_unix_ms)?;
    finish_reconciliation(database, event_id, now_unix_ms, run_mode)
}

pub fn watch_event_at(
    database_path: &Path,
    event_id: i64,
) -> Result<WatchEventProgress, WatcherError> {
    ManifestDatabase::open(database_path)?
        .watch_event(event_id)
        .map(|event| event.progress)
        .map_err(Into::into)
}

pub fn recent_watch_events_at(
    database_path: &Path,
) -> Result<Vec<WatchEventProgress>, WatcherError> {
    ManifestDatabase::open(database_path)?
        .recent_watch_events()
        .map_err(Into::into)
}

fn finish_reconciliation(
    database: &mut ManifestDatabase,
    event_id: i64,
    now_unix_ms: i64,
    run_mode: ReconciliationRunMode,
) -> Result<WatchEventProgress, WatcherError> {
    let event = database.watch_event(event_id)?;
    let scan_job_id = event
        .progress
        .scan_job_id
        .ok_or(DatabaseError::InvalidWatchEventState)?;
    let scan = database.scan_job(scan_job_id)?;
    let run_scan = |database: &mut ManifestDatabase| match run_mode {
        ReconciliationRunMode::ToTerminal => run_scan_job_to_terminal(database, scan_job_id),
        ReconciliationRunMode::Batch(batch_size) => {
            run_scan_job_batch(database, scan_job_id, batch_size)
        }
    };
    let scan = match scan.status {
        ScanStatus::Interrupted => {
            resume_scan_job(database, scan_job_id)?;
            run_scan(database)
        }
        ScanStatus::Running => run_scan(database),
        ScanStatus::Paused => return Ok(event.progress),
        ScanStatus::Completed => Ok(scan),
        ScanStatus::Failed => {
            return database
                .fail_watch_event_at(event_id, WatchEventReason::ReconcileFailed, now_unix_ms)
                .map_err(Into::into);
        }
    };
    match scan {
        Ok(scan) if scan.status == ScanStatus::Completed => database
            .complete_watch_reconciliation_at(event_id, now_unix_ms)
            .map_err(Into::into),
        Ok(_) => Ok(database.watch_event(event_id)?.progress),
        Err(error) => {
            database.fail_watch_event_at(
                event_id,
                WatchEventReason::ReconcileFailed,
                now_unix_ms,
            )?;
            Err(error.into())
        }
    }
}

fn record_changed_snapshot(
    database: &mut ManifestDatabase,
    scope_id: i64,
    hint: &ValidatedHint,
    policy: WatchPolicy,
    now_unix_ms: i64,
) -> Result<WatchEventProgress, WatcherError> {
    database
        .record_watch_observation_at(WatchObservationWrite {
            scope_id,
            path_raw: &hint.path_raw,
            path_key: &hint.path_key,
            snapshot: &hint.snapshot,
            stable_after_unix_ms: stable_after(now_unix_ms, policy)?,
            ignored_reason: None,
            observed_at_unix_ms: now_unix_ms,
        })
        .map(|event| event.progress)
        .map_err(Into::into)
}

fn evaluate_hint(
    database: &ManifestDatabase,
    scope_id: i64,
    observed_path: &Path,
) -> Result<EvaluatedHint, WatcherError> {
    if !observed_path.is_absolute() {
        return Err(WatcherError::ObservedPathMustBeAbsolute);
    }
    let canonical_root = validated_scope_root(database, scope_id)?;
    let metadata = match fs::symlink_metadata(observed_path) {
        Ok(metadata) => Some(metadata),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
        Err(_) => return Err(WatcherError::SourceUnavailable),
    };
    let (path, snapshot, platform_hidden) = if let Some(metadata) = metadata {
        if is_symlink_or_reparse_point(&metadata) {
            return Err(WatcherError::SymlinkOrReparsePointDenied);
        }
        let canonical =
            fs::canonicalize(observed_path).map_err(|_| WatcherError::SourceUnavailable)?;
        if !canonical.starts_with(&canonical_root) {
            return Err(WatcherError::ObservedPathOutsideScope);
        }
        let (kind, identity_kind, size_bytes) = if metadata.is_file() {
            (
                WatchSnapshotKind::File,
                IdentityNodeKind::File,
                Some(metadata.len()),
            )
        } else if metadata.is_dir() {
            (WatchSnapshotKind::Folder, IdentityNodeKind::Folder, None)
        } else {
            let hint = missing_hint(canonical);
            return Ok(EvaluatedHint::Ignore(
                hint,
                WatchEventReason::UnsupportedEntry,
            ));
        };
        let identity = platform_identity(&canonical, &metadata, identity_kind)
            .map_err(|_| WatcherError::SourceIdentityChanged)?;
        (
            canonical,
            WatchSnapshot {
                kind,
                size_bytes,
                modified_unix_ns: modified_unix_ns(&metadata),
                identity_key: Some(identity.key),
            },
            has_hidden_or_system_attribute(&metadata),
        )
    } else {
        (
            resolve_missing_path(&canonical_root, observed_path)?,
            WatchSnapshot {
                kind: WatchSnapshotKind::Missing,
                size_bytes: None,
                modified_unix_ns: None,
                identity_key: None,
            },
            false,
        )
    };
    if !path.starts_with(&canonical_root) {
        return Err(WatcherError::ObservedPathOutsideScope);
    }
    let hint = ValidatedHint {
        path_raw: path_to_raw(&path),
        path_key: comparison_key(&path),
        path,
        snapshot,
    };
    if is_temporary_download_path(&hint.path) {
        return Ok(EvaluatedHint::Ignore(
            hint,
            WatchEventReason::TemporaryDownload,
        ));
    }
    if platform_hidden || has_hidden_component(&canonical_root, &hint.path) {
        return Ok(EvaluatedHint::Ignore(hint, WatchEventReason::HiddenEntry));
    }
    Ok(EvaluatedHint::Track(hint))
}

fn missing_hint(path: PathBuf) -> ValidatedHint {
    ValidatedHint {
        path_raw: path_to_raw(&path),
        path_key: comparison_key(&path),
        path,
        snapshot: WatchSnapshot {
            kind: WatchSnapshotKind::Missing,
            size_bytes: None,
            modified_unix_ns: None,
            identity_key: None,
        },
    }
}

fn resolve_missing_path(root: &Path, observed_path: &Path) -> Result<PathBuf, WatcherError> {
    let mut ancestor = observed_path.to_path_buf();
    let mut suffix = Vec::new();
    loop {
        match fs::symlink_metadata(&ancestor) {
            Ok(metadata) => {
                if is_symlink_or_reparse_point(&metadata) {
                    return Err(WatcherError::SymlinkOrReparsePointDenied);
                }
                let canonical =
                    fs::canonicalize(&ancestor).map_err(|_| WatcherError::SourceUnavailable)?;
                if !canonical.starts_with(root) {
                    return Err(WatcherError::ObservedPathOutsideScope);
                }
                let mut resolved = canonical;
                for component in suffix.iter().rev() {
                    resolved.push(component);
                }
                return Ok(resolved);
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                let name = ancestor
                    .file_name()
                    .ok_or(WatcherError::ObservedPathOutsideScope)?;
                if !matches!(
                    Path::new(name).components().next(),
                    Some(Component::Normal(_))
                ) {
                    return Err(WatcherError::ObservedPathOutsideScope);
                }
                suffix.push(name.to_os_string());
                ancestor = ancestor
                    .parent()
                    .ok_or(WatcherError::ObservedPathOutsideScope)?
                    .to_path_buf();
            }
            Err(_) => return Err(WatcherError::SourceUnavailable),
        }
    }
}

fn open_file_matches_snapshot(hint: &ValidatedHint) -> Result<bool, WatcherError> {
    let file = match File::open(&hint.path) {
        Ok(file) => file,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(false),
        Err(_) => return Err(WatcherError::SourceUnavailable),
    };
    let metadata = file
        .metadata()
        .map_err(|_| WatcherError::SourceUnavailable)?;
    let identity =
        platform_identity_for_open_file(&file, &hint.path, &metadata, IdentityNodeKind::File)
            .map_err(|_| WatcherError::SourceIdentityChanged)?;
    Ok(
        hint.snapshot.identity_key.as_deref() == Some(identity.key.as_slice())
            && hint.snapshot.size_bytes == Some(metadata.len())
            && hint.snapshot.modified_unix_ns == modified_unix_ns(&metadata),
    )
}

fn stable_after(now_unix_ms: i64, policy: WatchPolicy) -> Result<i64, WatcherError> {
    if now_unix_ms < 0 {
        return Err(WatcherError::InvalidTimestamp);
    }
    now_unix_ms
        .checked_add(policy.stability_window_ms)
        .ok_or(WatcherError::InvalidTimestamp)
}

fn has_hidden_component(root: &Path, path: &Path) -> bool {
    path.strip_prefix(root).is_ok_and(|relative| {
        relative.components().any(|component| {
            matches!(component, Component::Normal(name) if name.to_string_lossy().starts_with('.'))
        })
    })
}

fn modified_unix_ns(metadata: &Metadata) -> Option<i64> {
    let duration = metadata.modified().ok()?.duration_since(UNIX_EPOCH).ok()?;
    i64::try_from(duration.as_nanos()).ok()
}

fn unix_ms() -> Result<i64, WatcherError> {
    let duration = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| WatcherError::InvalidTimestamp)?;
    i64::try_from(duration.as_millis()).map_err(|_| WatcherError::InvalidTimestamp)
}

#[cfg(test)]
mod tests {
    use super::*;
    use deskgraph_scanner::{
        authorize_scope, create_scan_job, run_scan_job_to_terminal, scan_scope,
    };
    use std::collections::VecDeque;
    use std::sync::Arc;

    fn setup() -> (tempfile::TempDir, ManifestDatabase, i64) {
        let directory = tempfile::tempdir().expect("fixture root should exist");
        let mut database = ManifestDatabase::open_in_memory().expect("database should initialize");
        let scope = authorize_scope(&database, directory.path()).expect("scope should authorize");
        scan_scope(&mut database, scope.id).expect("initial scan should complete");
        (directory, database, scope.id)
    }

    enum ScriptedSourceStep {
        Hint(WatchHint),
        Empty,
        Fail,
    }

    struct ScriptedWatchEventSource {
        steps: VecDeque<ScriptedSourceStep>,
    }

    impl WatchEventSource for ScriptedWatchEventSource {
        fn next_hint(&mut self) -> Result<Option<WatchHint>, WatcherError> {
            match self.steps.pop_front().unwrap_or(ScriptedSourceStep::Empty) {
                ScriptedSourceStep::Hint(hint) => Ok(Some(hint)),
                ScriptedSourceStep::Empty => Ok(None),
                ScriptedSourceStep::Fail => Err(WatcherError::EventSourceFailed),
            }
        }
    }

    #[test]
    fn scripted_source_is_ingested_only_through_the_existing_safety_gate() {
        let (directory, mut database, scope_id) = setup();
        let file = directory.path().join("source.md");
        fs::write(&file, "local").expect("fixture should write");
        let outside = tempfile::tempdir().expect("outside fixture should exist");
        let outside_file = outside.path().join("outside.md");
        fs::write(&outside_file, "outside").expect("outside fixture should write");
        let mut source = ScriptedWatchEventSource {
            steps: VecDeque::from([
                ScriptedSourceStep::Hint(WatchHint {
                    scope_id,
                    path: file,
                }),
                ScriptedSourceStep::Empty,
                ScriptedSourceStep::Hint(WatchHint {
                    scope_id,
                    path: outside_file,
                }),
                ScriptedSourceStep::Fail,
            ]),
        };

        let observed = ingest_next_source_hint_at_time(
            &mut database,
            &mut source,
            WatchPolicy::default(),
            1_000,
        )
        .expect("authorized source hint should ingest")
        .expect("source should yield a hint");
        assert_eq!(observed.status, WatchEventStatus::Stabilizing);
        assert_eq!(
            ingest_next_source_hint_at_time(
                &mut database,
                &mut source,
                WatchPolicy::default(),
                1_100,
            )
            .expect("empty source should be valid"),
            None
        );
        assert!(matches!(
            ingest_next_source_hint_at_time(
                &mut database,
                &mut source,
                WatchPolicy::default(),
                1_200,
            ),
            Err(WatcherError::ObservedPathOutsideScope)
        ));
        assert!(matches!(
            ingest_next_source_hint_at_time(
                &mut database,
                &mut source,
                WatchPolicy::default(),
                1_300,
            ),
            Err(WatcherError::EventSourceFailed)
        ));
    }

    #[test]
    fn source_hint_cannot_turn_authorization_into_an_initial_scan() {
        let directory = tempfile::tempdir().expect("fixture root should exist");
        let mut database = ManifestDatabase::open_in_memory().expect("database should initialize");
        let scope = authorize_scope(&database, directory.path()).expect("scope should authorize");
        let file = directory.path().join("not-scanned.md");
        fs::write(&file, "private").expect("fixture should write");
        let mut source = ScriptedWatchEventSource {
            steps: VecDeque::from([ScriptedSourceStep::Hint(WatchHint {
                scope_id: scope.id,
                path: file,
            })]),
        };

        assert!(matches!(
            ingest_next_source_hint_at_time(
                &mut database,
                &mut source,
                WatchPolicy::default(),
                1_000,
            ),
            Err(WatcherError::Database(
                DatabaseError::WatchScopeInitialScanRequired
            ))
        ));
        assert_eq!(
            database
                .stats()
                .expect("manifest stats should load")
                .completed_scan_count,
            0
        );
    }

    #[test]
    fn native_rescan_signal_advances_periodic_reconciliation_without_bypassing_bounds() {
        let (_directory, database, _scope_id) = setup();
        let polling_policy = PollingWatchPolicy::new(MIN_POLL_INTERVAL_MS)
            .expect("test polling policy should be valid");
        let mut coordinator =
            WatchCoordinator::from_database(database, WatchPolicy::default(), polling_policy);

        let scheduled = coordinator
            .run_cycle_at_time(1_000)
            .expect("startup reconciliation should schedule");
        assert_eq!(scheduled.scheduled_scope_count, 1);
        let completed = coordinator
            .run_cycle_at_time(2_000)
            .expect("startup reconciliation should complete");
        assert_eq!(completed.completed_event_count, 1);

        let native = coordinator
            .run_cycle_with_native_batch_at_time(
                native::NativeWatchBatch {
                    hints: Vec::new(),
                    reconcile_scope_ids: BTreeSet::new(),
                    reconcile_all: true,
                    source_failed: false,
                    more_pending: false,
                    signal_count: 0,
                    overflow_count: 1,
                },
                3_000,
            )
            .expect("native overflow should use the durable root path");
        assert_eq!(native.scheduled_scope_count, 1);
        assert!(native.native_reconcile_all);
        assert_eq!(native.native_overflow_count, 1);
        assert_eq!(native.native_hint_scope_count, 0);
    }

    #[test]
    fn native_ordered_temporary_to_final_rename_cannot_lose_the_final_file() {
        let (directory, database, scope_id) = setup();
        let temporary = directory.path().join("report.crdownload");
        let final_path = directory.path().join("report.pdf");
        fs::write(&temporary, "complete").expect("temporary fixture should write");
        fs::rename(&temporary, &final_path).expect("fixture should reach its final name");
        let polling_policy = PollingWatchPolicy::new(MIN_POLL_INTERVAL_MS)
            .expect("test polling policy should be valid");
        let mut coordinator =
            WatchCoordinator::from_database(database, WatchPolicy::default(), polling_policy);

        let scheduled = coordinator
            .run_cycle_with_native_batch_at_time(
                native::NativeWatchBatch {
                    hints: vec![WatchHint {
                        scope_id,
                        path: temporary,
                    }],
                    reconcile_scope_ids: BTreeSet::from([scope_id]),
                    reconcile_all: false,
                    source_failed: false,
                    more_pending: false,
                    signal_count: 1,
                    overflow_count: 0,
                },
                1_000,
            )
            .expect("ordered rename ambiguity should request bounded scope recovery");
        assert_eq!(scheduled.scheduled_scope_count, 1);
        assert_eq!(scheduled.forced_scope_reconciliation_count, 0);

        let reconciled = coordinator
            .run_cycle_at_time(1_001)
            .expect("scope recovery should force a fresh root scan");
        assert_eq!(reconciled.forced_scope_reconciliation_count, 1);
        assert_eq!(reconciled.completed_event_count, 1);
        assert!(
            coordinator
                .database
                .node_id_for_path_key(
                    scope_id,
                    &comparison_key(
                        &fs::canonicalize(&final_path).expect("final fixture should canonicalize")
                    )
                )
                .expect("final node lookup should pass")
                .is_some(),
            "the final path must not wait for the periodic fallback"
        );
    }

    #[test]
    fn native_overflow_forces_an_active_scope_through_durable_root_reconciliation() {
        let (directory, mut database, scope_id) = setup();
        let file = directory.path().join("overflow.md");
        fs::write(&file, "local").expect("fixture should write");
        observe_watch_path_at_time(
            &mut database,
            scope_id,
            &file,
            WatchPolicy::default(),
            1_000,
        )
        .expect("active event should persist");
        let polling_policy = PollingWatchPolicy::new(MIN_POLL_INTERVAL_MS)
            .expect("test polling policy should be valid");
        let mut coordinator =
            WatchCoordinator::from_database(database, WatchPolicy::default(), polling_policy);

        let report = coordinator
            .run_cycle_with_native_batch_at_time(
                native::NativeWatchBatch {
                    hints: Vec::new(),
                    reconcile_scope_ids: BTreeSet::new(),
                    reconcile_all: true,
                    source_failed: false,
                    more_pending: false,
                    signal_count: 0,
                    overflow_count: 1,
                },
                1_100,
            )
            .expect("overflow must not be blocked by a stabilizing event");

        assert_eq!(report.forced_scope_reconciliation_count, 1);
        assert_eq!(report.completed_event_count, 1);
        assert_eq!(report.active_event_count, 0);
        assert_eq!(
            coordinator
                .database
                .stats()
                .expect("manifest stats should load")
                .file_count,
            1
        );
    }

    #[test]
    fn recovery_during_a_multibatch_scan_runs_a_fresh_root_scan_afterward() {
        let (directory, database, scope_id) = setup();
        for index in 0..300 {
            fs::write(
                directory.path().join(format!("before-{index:03}.md")),
                "before",
            )
            .expect("large fixture should write");
        }
        let polling_policy = PollingWatchPolicy::new(MIN_POLL_INTERVAL_MS)
            .expect("test polling policy should be valid");
        let mut coordinator =
            WatchCoordinator::from_database(database, WatchPolicy::default(), polling_policy);

        let scheduled = coordinator
            .run_cycle_at_time(1_000)
            .expect("root reconciliation should schedule");
        assert_eq!(scheduled.scheduled_scope_count, 1);
        let first_batch = coordinator
            .run_cycle_at_time(2_000)
            .expect("large reconciliation should advance one bounded batch");
        assert_eq!(first_batch.advanced_event_count, 1);
        assert_eq!(first_batch.completed_event_count, 0);
        assert_eq!(first_batch.active_event_count, 1);

        let after_signal = directory.path().join("after-rescan-signal.md");
        fs::write(&after_signal, "after").expect("post-signal fixture should write");
        let old_scan_completed = coordinator
            .run_cycle_with_native_batch_at_time(
                native::NativeWatchBatch {
                    hints: Vec::new(),
                    reconcile_scope_ids: BTreeSet::new(),
                    reconcile_all: true,
                    source_failed: false,
                    more_pending: false,
                    signal_count: 0,
                    overflow_count: 1,
                },
                2_100,
            )
            .expect("recovery intent should survive the existing scan");
        assert_eq!(old_scan_completed.completed_event_count, 1);
        assert_eq!(old_scan_completed.forced_scope_reconciliation_count, 0);
        assert_eq!(old_scan_completed.scheduled_scope_count, 1);
        assert_eq!(old_scan_completed.active_event_count, 1);

        let followup_started = coordinator
            .run_cycle_at_time(2_101)
            .expect("a fresh forced root scan should start");
        assert_eq!(followup_started.forced_scope_reconciliation_count, 1);
        assert_eq!(followup_started.completed_event_count, 0);
        assert_eq!(followup_started.active_event_count, 1);
        let followup_completed = coordinator
            .run_cycle_at_time(2_102)
            .expect("the fresh root scan should complete");
        assert_eq!(followup_completed.completed_event_count, 1);
        assert_eq!(followup_completed.active_event_count, 0);
        assert_eq!(
            coordinator
                .database
                .stats()
                .expect("manifest stats should load")
                .completed_scan_count,
            3
        );
        assert!(
            coordinator
                .database
                .node_id_for_path_key(
                    scope_id,
                    &comparison_key(
                        &fs::canonicalize(&after_signal)
                            .expect("post-signal fixture should canonicalize")
                    )
                )
                .expect("node lookup should pass")
                .is_some(),
            "the follow-up scan must capture changes after the old root was enumerated"
        );
    }

    #[test]
    fn native_watch_set_changes_close_the_registration_gap_with_a_root_reconciliation() {
        let (directory, database, scope_id) = setup();
        let missed_before_registration = directory.path().join("before-registration.md");
        fs::write(&missed_before_registration, "local").expect("fixture should write");
        let polling_policy = PollingWatchPolicy::new(MIN_POLL_INTERVAL_MS)
            .expect("test polling policy should be valid");
        let mut coordinator =
            WatchCoordinator::from_database(database, WatchPolicy::default(), polling_policy);
        let mut source =
            NativeWatchEventSource::new(Arc::new(|| {})).expect("native source should initialize");

        assert!(
            coordinator
                .synchronize_native_event_source(&mut source)
                .expect("watch set should synchronize"),
            "the first registration must be reported to the runtime"
        );
        coordinator
            .request_all_scope_reconciliation_at(1_000)
            .expect("watch-set change should request reconciliation");
        let scheduled = coordinator
            .run_cycle_at_time(1_000)
            .expect("root reconciliation should schedule durably");
        assert_eq!(scheduled.scheduled_scope_count, 1);
        let completed = coordinator
            .run_cycle_at_time(1_001)
            .expect("forced reconciliation should not wait for debounce");

        assert_eq!(completed.forced_scope_reconciliation_count, 1);
        assert_eq!(completed.completed_event_count, 1);
        assert_eq!(
            coordinator
                .database
                .stats()
                .expect("manifest stats should load")
                .file_count,
            1
        );
        assert!(
            coordinator
                .database
                .node_id_for_path_key(
                    scope_id,
                    &comparison_key(
                        &fs::canonicalize(&missed_before_registration)
                            .expect("fixture should canonicalize")
                    )
                )
                .expect("node lookup should pass")
                .is_some()
        );
    }

    #[test]
    fn sustained_churn_cannot_postpone_metadata_reconciliation_past_the_poll_interval() {
        let (directory, mut database, scope_id) = setup();
        let file = directory.path().join("continuous.log");
        fs::write(&file, "one").expect("fixture should write");
        let event = observe_watch_path_at_time(
            &mut database,
            scope_id,
            &file,
            WatchPolicy::default(),
            1_000,
        )
        .expect("first observation should persist");
        fs::write(&file, "two-two").expect("fixture should change");
        observe_watch_path_at_time(
            &mut database,
            scope_id,
            &file,
            WatchPolicy::default(),
            3_000,
        )
        .expect("second observation should coalesce");
        fs::write(&file, "three-three-three").expect("fixture should change again");
        let latest = observe_watch_path_at_time(
            &mut database,
            scope_id,
            &file,
            WatchPolicy::default(),
            5_500,
        )
        .expect("latest observation should coalesce");
        assert_eq!(latest.event_id, event.event_id);
        assert_eq!(latest.stable_after_unix_ms, 6_500);

        let polling_policy = PollingWatchPolicy::new(MIN_POLL_INTERVAL_MS)
            .expect("test polling policy should be valid");
        let mut coordinator =
            WatchCoordinator::from_database(database, WatchPolicy::default(), polling_policy);
        let waiting = coordinator
            .run_cycle_at_time(5_501)
            .expect("pre-bound cycle should remain stable");
        assert_eq!(
            waiting.next_wake_unix_ms, 6_000,
            "the first observation age, not the latest debounce, bounds the wake"
        );

        let forced = coordinator
            .run_cycle_at_time(6_000)
            .expect("maximum coalescing age should force metadata reconciliation");
        assert_eq!(forced.forced_scope_reconciliation_count, 1);
        assert_eq!(forced.completed_event_count, 1);
        assert_eq!(forced.active_event_count, 0);
        let node_id = coordinator
            .database
            .node_id_for_path_key(
                scope_id,
                &comparison_key(&fs::canonicalize(&file).expect("fixture should canonicalize")),
            )
            .expect("node lookup should pass")
            .expect("reconciled node should exist");
        assert_eq!(
            coordinator
                .database
                .extractable_file(scope_id, node_id)
                .expect("manifest metadata should load")
                .size_bytes,
            17
        );
    }

    #[test]
    fn native_source_failure_is_path_free_and_keeps_periodic_work_running() {
        let (_directory, database, _scope_id) = setup();
        let polling_policy = PollingWatchPolicy::new(MIN_POLL_INTERVAL_MS)
            .expect("test polling policy should be valid");
        let mut coordinator =
            WatchCoordinator::from_database(database, WatchPolicy::default(), polling_policy);

        let report = coordinator
            .run_cycle_with_native_batch_at_time(
                native::NativeWatchBatch {
                    hints: Vec::new(),
                    reconcile_scope_ids: BTreeSet::new(),
                    reconcile_all: true,
                    source_failed: true,
                    more_pending: false,
                    signal_count: 0,
                    overflow_count: 0,
                },
                1_000,
            )
            .expect("source failure should degrade to polling");

        assert!(report.native_source_failed);
        assert_eq!(report.scheduled_scope_count, 1);
        assert_eq!(report.last_error_code, Some("watch_native_source_failed"));
    }

    #[test]
    fn polling_policy_is_bounded_for_resource_safety() {
        assert!(matches!(
            PollingWatchPolicy::new(MIN_POLL_INTERVAL_MS - 1),
            Err(WatcherError::InvalidPollingPolicy)
        ));
        assert_eq!(
            PollingWatchPolicy::new(MIN_POLL_INTERVAL_MS)
                .expect("minimum interval should be accepted")
                .poll_interval_ms(),
            MIN_POLL_INTERVAL_MS
        );
        assert!(matches!(
            PollingWatchPolicy::new(MAX_POLL_INTERVAL_MS + 1),
            Err(WatcherError::InvalidPollingPolicy)
        ));
    }

    #[test]
    fn polling_never_turns_authorization_into_an_implicit_initial_scan() {
        let directory = tempfile::tempdir().expect("fixture root should exist");
        let scope_path = directory.path().join("scope");
        fs::create_dir(&scope_path).expect("scope should create");
        let database = ManifestDatabase::open_in_memory().expect("database should initialize");
        let _scope = authorize_scope(&database, &scope_path).expect("scope should authorize");
        let polling_policy = PollingWatchPolicy::new(MIN_POLL_INTERVAL_MS)
            .expect("test polling policy should be valid");
        let mut coordinator =
            WatchCoordinator::from_database(database, WatchPolicy::default(), polling_policy);

        let before_scan = coordinator
            .run_cycle_at_time(1_000)
            .expect("unscanned scope should be skipped");
        assert_eq!(before_scan.authorized_scope_count, 1);
        assert_eq!(before_scan.scheduled_scope_count, 0);
        assert_eq!(before_scan.active_event_count, 0);
        assert_eq!(before_scan.next_wake_unix_ms, 1_000 + MIN_POLL_INTERVAL_MS);
    }

    #[test]
    fn invalid_scope_stays_degraded_until_a_successful_reconciliation() {
        let directory = tempfile::tempdir().expect("fixture root should exist");
        let database_path = directory.path().join("manifest.sqlite3");
        let scope_path = directory.path().join("scope");
        let moved_path = directory.path().join("scope-moved");
        fs::create_dir(&scope_path).expect("scope should create");
        fs::write(scope_path.join("note.md"), "local").expect("fixture should write");
        let mut database = ManifestDatabase::open(&database_path).expect("database should open");
        let scope = authorize_scope(&database, &scope_path).expect("scope should authorize");
        scan_scope(&mut database, scope.id).expect("initial scan should complete");
        drop(database);
        fs::rename(&scope_path, &moved_path).expect("scope should move");

        let polling_policy = PollingWatchPolicy::new(MIN_POLL_INTERVAL_MS)
            .expect("test polling policy should be valid");
        let mut coordinator =
            WatchCoordinator::open(&database_path, WatchPolicy::default(), polling_policy)
                .expect("coordinator should start");
        let failed = coordinator
            .run_cycle_at_time(1_000)
            .expect("invalid scope should degrade without crashing");
        assert_eq!(failed.degraded_scope_count, 1);
        assert_eq!(
            failed.last_error_code,
            Some("scope_canonicalization_failed")
        );
        assert_eq!(
            failed.next_wake_unix_ms,
            1_000 + COORDINATOR_SCOPE_ERROR_RETRY_MS
        );

        let still_degraded = coordinator
            .run_cycle_at_time(1_100)
            .expect("intermediate cycle should retain failure");
        assert_eq!(still_degraded.degraded_scope_count, 1);
        assert_eq!(
            still_degraded.last_error_code,
            Some("scope_canonicalization_failed")
        );
        assert_eq!(still_degraded.scheduled_scope_count, 0);

        fs::rename(&moved_path, &scope_path).expect("scope should restore");
        let rescheduled = coordinator
            .run_cycle_at_time(31_000)
            .expect("restored scope should schedule");
        assert_eq!(rescheduled.scheduled_scope_count, 1);
        assert_eq!(rescheduled.degraded_scope_count, 1);
        let recovered = coordinator
            .run_cycle_at_time(32_000)
            .expect("stable restored scope should reconcile");
        assert_eq!(recovered.completed_event_count, 1);
        assert_eq!(recovered.degraded_scope_count, 0);
        assert_eq!(recovered.last_error_code, None);
    }

    #[test]
    fn polling_scope_backlog_is_rate_limited_and_reported() {
        let directory = tempfile::tempdir().expect("fixture root should exist");
        let mut database = ManifestDatabase::open_in_memory().expect("database should initialize");
        let mut initial_scope_ids = Vec::new();
        for index in 0..9 {
            let scope_path = directory.path().join(format!("scope-{index}"));
            fs::create_dir(&scope_path).expect("scope should create");
            let scope = authorize_scope(&database, &scope_path).expect("scope should authorize");
            scan_scope(&mut database, scope.id).expect("initial scan should complete");
            initial_scope_ids.push(scope.id);
        }
        let polling_policy = PollingWatchPolicy::new(MIN_POLL_INTERVAL_MS)
            .expect("test polling policy should be valid");
        let mut coordinator =
            WatchCoordinator::from_database(database, WatchPolicy::default(), polling_policy);

        let first = coordinator
            .run_cycle_at_time(1_000)
            .expect("first polling cycle should be bounded");
        assert_eq!(first.scheduled_scope_count, 4);
        assert_eq!(first.deferred_scope_count, 5);
        assert_eq!(
            first.next_wake_unix_ms,
            1_000 + COORDINATOR_SCOPE_SCHEDULE_RETRY_MS
        );

        let before_retry = coordinator
            .run_cycle_at_time(1_050)
            .expect("deferred scopes should not create a burst");
        assert_eq!(before_retry.scheduled_scope_count, 0);
        assert_eq!(before_retry.deferred_scope_count, 5);
        assert_eq!(
            before_retry.next_wake_unix_ms,
            1_000 + COORDINATOR_SCOPE_SCHEDULE_RETRY_MS
        );

        for index in 9..13 {
            let scope_path = directory.path().join(format!("scope-{index}"));
            fs::create_dir(&scope_path).expect("new scope should create");
            let scope = authorize_scope(&coordinator.database, &scope_path)
                .expect("scope should authorize");
            scan_scope(&mut coordinator.database, scope.id)
                .expect("new initial scan should complete");
        }
        let second = coordinator
            .run_cycle_at_time(2_000)
            .expect("older backlog should win over new due scopes");
        assert_eq!(second.scheduled_scope_count, 4);
        assert_eq!(second.deferred_scope_count, 5);
        for scope_id in &initial_scope_ids[4..8] {
            assert!(
                !coordinator.deferred_scope_due_at.contains_key(scope_id),
                "the oldest deferred scopes must be scheduled first"
            );
        }
        assert!(
            coordinator
                .deferred_scope_due_at
                .contains_key(&initial_scope_ids[8])
        );

        for index in 13..17 {
            let scope_path = directory.path().join(format!("scope-{index}"));
            fs::create_dir(&scope_path).expect("new scope should create");
            let scope = authorize_scope(&coordinator.database, &scope_path)
                .expect("scope should authorize");
            scan_scope(&mut coordinator.database, scope.id)
                .expect("new initial scan should complete");
        }
        coordinator
            .run_cycle_at_time(3_000)
            .expect("continuous arrivals must not starve the oldest backlog");
        assert!(
            !coordinator
                .deferred_scope_due_at
                .contains_key(&initial_scope_ids[8]),
            "an older deferred scope must not be starved by continuously arriving scopes"
        );
    }

    #[test]
    fn active_scan_contention_uses_a_bounded_retry_deadline() {
        let (directory, mut database, scope_id) = setup();
        let changed_file = directory.path().join("changed.md");
        fs::write(&changed_file, "changed").expect("fixture should write");
        observe_watch_path_at_time(
            &mut database,
            scope_id,
            &changed_file,
            WatchPolicy::default(),
            1_000,
        )
        .expect("watch event should stabilize");
        let foreground_scan =
            create_scan_job(&mut database, scope_id).expect("foreground scan should start");
        let polling_policy = PollingWatchPolicy::new(MIN_POLL_INTERVAL_MS)
            .expect("test polling policy should be valid");
        let mut coordinator =
            WatchCoordinator::from_database(database, WatchPolicy::default(), polling_policy);

        let contended = coordinator
            .run_cycle_at_time(2_000)
            .expect("scan contention should be deferred");
        assert_eq!(contended.deferred_event_count, 1);
        assert_eq!(
            contended.next_wake_unix_ms,
            2_000 + COORDINATOR_EVENT_CONTENTION_RETRY_MS
        );

        let before_retry = coordinator
            .run_cycle_at_time(2_050)
            .expect("contention should not create a 50 ms retry loop");
        assert_eq!(before_retry.deferred_event_count, 1);
        assert_eq!(
            before_retry.next_wake_unix_ms,
            2_000 + COORDINATOR_EVENT_CONTENTION_RETRY_MS
        );

        run_scan_job_to_terminal(&mut coordinator.database, foreground_scan.job_id)
            .expect("foreground scan should complete");
        let recovered = coordinator
            .run_cycle_at_time(3_000)
            .expect("watch reconciliation should resume after contention");
        assert_eq!(recovered.completed_event_count, 1);
        assert_eq!(recovered.active_event_count, 0);
    }

    #[test]
    fn polling_coordinator_recovers_after_restart_and_reconciles_metadata_only() {
        let directory = tempfile::tempdir().expect("fixture root should exist");
        let database_path = directory.path().join("manifest.sqlite3");
        let scope_path = directory.path().join("scope");
        fs::create_dir(&scope_path).expect("scope should create");
        let mut database = ManifestDatabase::open(&database_path).expect("database should open");
        let scope = authorize_scope(&database, &scope_path).expect("scope should authorize");
        scan_scope(&mut database, scope.id).expect("initial scan should complete");
        drop(database);

        let new_file = scope_path.join("automatic.md");
        fs::write(&new_file, "not extracted").expect("fixture should write");
        let polling_policy = PollingWatchPolicy::new(MIN_POLL_INTERVAL_MS)
            .expect("test polling policy should be valid");
        let mut first_runtime =
            WatchCoordinator::open(&database_path, WatchPolicy::default(), polling_policy)
                .expect("coordinator should start");
        let scheduled = first_runtime
            .run_cycle_at_time(1_000)
            .expect("first polling cycle should schedule");
        assert_eq!(scheduled.scheduled_scope_count, 1);
        assert_eq!(scheduled.active_event_count, 1);
        drop(first_runtime);

        let mut restarted =
            WatchCoordinator::open(&database_path, WatchPolicy::default(), polling_policy)
                .expect("coordinator should restart");
        let completed = restarted
            .run_cycle_at_time(2_000)
            .expect("restart cycle should resume the durable event");
        assert_eq!(completed.advanced_event_count, 1);
        assert_eq!(completed.completed_event_count, 1);
        assert_eq!(completed.active_event_count, 0);
        assert_eq!(completed.last_error_code, None);
        drop(restarted);

        let database = ManifestDatabase::open(&database_path).expect("database should reopen");
        assert_eq!(database.stats().expect("stats should load").file_count, 1);
        let extraction = database
            .extraction_stats()
            .expect("extraction stats should load");
        assert_eq!(extraction.active_chunk_count, 0);
        assert_eq!(extraction.completed_job_count, 0);
        assert_eq!(
            database
                .active_watch_events()
                .expect("active events should load"),
            Vec::new()
        );
    }

    #[test]
    fn temporary_download_is_ignored_without_a_scan() {
        let (directory, mut database, scope_id) = setup();
        let download = directory.path().join("archive.crdownload");
        fs::write(&download, "partial").expect("temporary file should write");

        let event = observe_watch_path_at_time(
            &mut database,
            scope_id,
            &download,
            WatchPolicy::default(),
            1_000,
        )
        .expect("temporary observation should be recorded safely");

        assert_eq!(event.status, WatchEventStatus::Ignored);
        assert_eq!(event.reason, Some(WatchEventReason::TemporaryDownload));
        let second_download = directory.path().join("other.part");
        fs::write(&second_download, "partial-two").expect("temporary file should write");
        let coalesced = observe_watch_path_at_time(
            &mut database,
            scope_id,
            &second_download,
            WatchPolicy::default(),
            1_100,
        )
        .expect("temporary observation should coalesce safely");
        assert_eq!(coalesced.event_id, event.event_id);
        assert_eq!(coalesced.observation_count, 2);
        assert_eq!(
            database
                .watch_event(event.event_id)
                .expect("coalesced event should load")
                .path_key,
            comparison_key(&fs::canonicalize(&second_download).expect("path should canonicalize"))
        );
        assert_eq!(
            database
                .recent_watch_events()
                .expect("watch history should load")
                .len(),
            1
        );
        assert_eq!(
            database
                .stats()
                .expect("stats should load")
                .completed_scan_count,
            1
        );
    }

    #[test]
    fn changing_snapshot_restarts_the_stability_window() {
        let (directory, mut database, scope_id) = setup();
        let file = directory.path().join("notes.md");
        fs::write(&file, "one").expect("file should write");
        let event = observe_watch_path_at_time(
            &mut database,
            scope_id,
            &file,
            WatchPolicy::default(),
            1_000,
        )
        .expect("observation should persist");
        fs::write(&file, "a longer second version").expect("file should change");

        let changed = advance_watch_event_at_time(
            &mut database,
            event.event_id,
            WatchPolicy::default(),
            2_000,
        )
        .expect("changed snapshot should remain stabilizing");
        assert_eq!(changed.status, WatchEventStatus::Stabilizing);
        assert_eq!(changed.observation_count, 2);
        assert_eq!(changed.stable_after_unix_ms, 3_000);

        let completed = advance_watch_event_at_time(
            &mut database,
            event.event_id,
            WatchPolicy::default(),
            3_000,
        )
        .expect("stable snapshot should reconcile");
        assert_eq!(completed.status, WatchEventStatus::Completed);
        assert_eq!(
            database
                .stats()
                .expect("stats should load")
                .completed_scan_count,
            2
        );
    }

    #[test]
    fn rename_storm_coalesces_and_preserves_identity_after_restart() {
        let directory = tempfile::tempdir().expect("fixture root should exist");
        let database_path = directory.path().join("manifest.sqlite3");
        let scope_path = directory.path().join("scope");
        fs::create_dir(&scope_path).expect("scope should create");
        let old_path = scope_path.join("old-name.md");
        let new_path = scope_path.join("new-name.md");
        fs::write(&old_path, "local context").expect("fixture should write");
        let mut database = ManifestDatabase::open(&database_path).expect("database should open");
        let scope = authorize_scope(&database, &scope_path).expect("scope should authorize");
        scan_scope(&mut database, scope.id).expect("initial scan should complete");
        let old_key = comparison_key(&fs::canonicalize(&old_path).expect("path should exist"));
        let original_node = database
            .node_id_for_path_key(scope.id, &old_key)
            .expect("node lookup should pass")
            .expect("node should exist");

        fs::rename(&old_path, &new_path).expect("fixture rename should pass");
        let event = observe_watch_path_at_time(
            &mut database,
            scope.id,
            &old_path,
            WatchPolicy::default(),
            1_000,
        )
        .expect("missing old path should be observed");
        let coalesced = observe_watch_path_at_time(
            &mut database,
            scope.id,
            &new_path,
            WatchPolicy::default(),
            1_100,
        )
        .expect("new path should coalesce");
        assert_eq!(coalesced.event_id, event.event_id);
        assert_eq!(coalesced.observation_count, 2);
        drop(database);

        let mut reopened = ManifestDatabase::open(&database_path).expect("database should reopen");
        let completed = advance_watch_event_at_time(
            &mut reopened,
            event.event_id,
            WatchPolicy::default(),
            2_100,
        )
        .expect("persisted event should resume");
        assert_eq!(completed.status, WatchEventStatus::Completed);
        let new_key = comparison_key(&fs::canonicalize(&new_path).expect("new path should exist"));
        assert_eq!(
            reopened
                .node_id_for_path_key(scope.id, &new_key)
                .expect("new node lookup should pass"),
            Some(original_node)
        );
        assert_eq!(
            reopened
                .node_id_for_path_key(scope.id, &old_key)
                .expect("old node lookup should pass"),
            None
        );
    }

    #[test]
    fn reconciling_event_resumes_its_atomically_linked_scan_after_restart() {
        let directory = tempfile::tempdir().expect("fixture root should exist");
        let database_path = directory.path().join("manifest.sqlite3");
        let scope_path = directory.path().join("scope");
        fs::create_dir(&scope_path).expect("scope should create");
        let watched_file = scope_path.join("restart.md");
        fs::write(&watched_file, "before restart").expect("fixture should write");
        let mut database = ManifestDatabase::open(&database_path).expect("database should open");
        let scope = authorize_scope(&database, &scope_path).expect("scope should authorize");
        scan_scope(&mut database, scope.id).expect("initial scan should complete");
        fs::write(&watched_file, "after restart").expect("fixture should change");
        let event = observe_watch_path_at_time(
            &mut database,
            scope.id,
            &watched_file,
            WatchPolicy::default(),
            1_000,
        )
        .expect("event should persist");
        let canonical_root =
            validated_scope_root(&database, scope.id).expect("root should validate");
        let root = QueuedPath {
            path_raw: path_to_raw(&canonical_root),
            path_key: comparison_key(&canonical_root),
            parent_identity_key: None,
            is_root: true,
        };
        let reconciling = database
            .begin_watch_reconciliation_at(event.event_id, &root, 2_000)
            .expect("event and scan should link atomically");
        assert_eq!(reconciling.status, WatchEventStatus::Reconciling);
        drop(database);

        let mut reopened = ManifestDatabase::open(&database_path).expect("database should reopen");
        let completed = advance_watch_event_at_time(
            &mut reopened,
            event.event_id,
            WatchPolicy::default(),
            2_100,
        )
        .expect("linked ready scan should resume");
        assert_eq!(completed.status, WatchEventStatus::Completed);
        assert!(completed.scan_job_id.is_some());
    }

    #[cfg(unix)]
    #[test]
    fn scope_escape_and_symlink_hints_are_denied() {
        use std::os::unix::fs::symlink;

        let (directory, mut database, scope_id) = setup();
        let outside = tempfile::tempdir().expect("outside root should exist");
        let outside_file = outside.path().join("outside.md");
        fs::write(&outside_file, "outside").expect("outside fixture should write");
        let link = directory.path().join("escape-link");
        symlink(&outside_file, &link).expect("symlink should create");

        assert!(matches!(
            observe_watch_path_at_time(
                &mut database,
                scope_id,
                &outside_file,
                WatchPolicy::default(),
                1_000
            ),
            Err(WatcherError::ObservedPathOutsideScope)
        ));
        assert!(matches!(
            observe_watch_path_at_time(
                &mut database,
                scope_id,
                &link,
                WatchPolicy::default(),
                1_000
            ),
            Err(WatcherError::SymlinkOrReparsePointDenied)
        ));
        let missing_escape = directory
            .path()
            .join("missing")
            .join("..")
            .join("..")
            .join("not-there.md");
        assert!(matches!(
            observe_watch_path_at_time(
                &mut database,
                scope_id,
                &missing_escape,
                WatchPolicy::default(),
                1_000
            ),
            Err(WatcherError::ObservedPathOutsideScope)
        ));
    }
}
