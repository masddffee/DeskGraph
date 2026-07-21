use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::mpsc::{Receiver, SyncSender, TryRecvError, TrySendError, sync_channel};
use std::sync::{Arc, Condvar, Mutex, Weak};
use std::time::{Duration, Instant};

use notify::{Config, Event, EventHandler, RecommendedWatcher, RecursiveMode, Watcher};

use crate::{WatchHint, WatcherError};

pub(super) const MAX_NATIVE_SIGNALS_PER_CYCLE: usize = 64;
const NATIVE_EVENT_QUEUE_CAPACITY: usize = 256;
const MAX_NATIVE_PATHS_PER_EVENT: usize = 2;
const MAX_NATIVE_PATH_BYTES_PER_EVENT: usize = 16 * 1024;

#[derive(Clone, Default)]
pub struct NativeWatchSynchronizationBarrier {
    state: Arc<(Mutex<NativeWatchSynchronizationState>, Condvar)>,
}

#[derive(Default)]
struct NativeWatchSynchronizationState {
    requested_generation: u64,
    acknowledged_generation: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct NativeWatchSynchronizationTicket {
    generation: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct NativeWatchSynchronizationPass {
    generation: u64,
}

/// A process-local kill switch for the native callback.  The Desktop owns a
/// clone before the watcher thread starts, so a privacy revocation can stop
/// path admission even when that thread or the platform watcher cannot be
/// joined promptly.
#[derive(Clone, Default)]
pub struct NativeWatchCallbackRetirement {
    lifecycle: Arc<(Mutex<NativeCallbackLifecycle>, Condvar)>,
    queue: Arc<Mutex<Option<Weak<NativeEventQueueControl>>>>,
}

#[derive(Default)]
struct NativeCallbackLifecycle {
    retired: bool,
    active_callbacks: usize,
}

struct NativeCallbackLease {
    lifecycle: Arc<(Mutex<NativeCallbackLifecycle>, Condvar)>,
}

impl Drop for NativeCallbackLease {
    fn drop(&mut self) {
        let (state, wake) = &*self.lifecycle;
        let mut state = state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        state.active_callbacks = state.active_callbacks.saturating_sub(1);
        if state.active_callbacks == 0 {
            wake.notify_all();
        }
    }
}

impl NativeWatchCallbackRetirement {
    fn begin_callback(&self) -> Option<NativeCallbackLease> {
        let (state, _) = &*self.lifecycle;
        let mut state = state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if state.retired {
            return None;
        }
        state.active_callbacks = state.active_callbacks.saturating_add(1);
        Some(NativeCallbackLease {
            lifecycle: Arc::clone(&self.lifecycle),
        })
    }

    fn run_if_admitted(&self, operation: impl FnOnce()) -> bool {
        let (state, _) = &*self.lifecycle;
        let state = state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if state.retired {
            return false;
        }
        operation();
        true
    }

    fn register_queue(&self, queue: &Arc<NativeEventQueueControl>) -> bool {
        let (state, _) = &*self.lifecycle;
        let state = state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if state.retired {
            return false;
        }
        let mut registered = self
            .queue
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        *registered = Some(Arc::downgrade(queue));
        true
    }

    pub fn is_retired(&self) -> bool {
        let (state, _) = &*self.lifecycle;
        state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .retired
    }

    /// Permanently closes callback admission, removes already queued paths and
    /// waits only for callbacks that entered before retirement.  Queue clearing
    /// is safe even when the wait expires: all enqueue mutations re-check the
    /// same lifecycle mutex after admission, so no retired callback can refill
    /// the queue after the clear.
    pub fn retire_and_clear(&self, timeout: Duration) -> bool {
        let deadline = Instant::now() + timeout;
        {
            let (state, _) = &*self.lifecycle;
            let mut state = state
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            state.retired = true;
        }

        let queue = self
            .queue
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .as_ref()
            .and_then(Weak::upgrade);
        if let Some(queue) = queue {
            queue.clear();
        }

        let (state, wake) = &*self.lifecycle;
        let mut state = state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        while state.active_callbacks > 0 {
            let now = Instant::now();
            if now >= deadline {
                return false;
            }
            let remaining = deadline.saturating_duration_since(now);
            state = match wake.wait_timeout(state, remaining) {
                Ok((state, _)) => state,
                Err(poisoned) => poisoned.into_inner().0,
            };
        }
        true
    }
}

impl NativeWatchSynchronizationBarrier {
    /// Requests one path-free synchronization acknowledgement from the watch
    /// runtime. The caller must request this only after it has removed the
    /// runtime capability, then wake the runtime without holding its database
    /// or scope-access locks.
    pub fn request(&self) -> NativeWatchSynchronizationTicket {
        let (state, _) = &*self.state;
        let mut state = state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        state.requested_generation = state.requested_generation.saturating_add(1);
        NativeWatchSynchronizationTicket {
            generation: state.requested_generation,
        }
    }

    /// Captures the request generation before the runtime reads the scope
    /// registry for one synchronization pass. A request that arrives later
    /// cannot be acknowledged by an older scope snapshot.
    pub fn begin_pass(&self) -> NativeWatchSynchronizationPass {
        let (state, _) = &*self.state;
        let state = state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        NativeWatchSynchronizationPass {
            generation: state.requested_generation,
        }
    }

    /// Acknowledges only the generation captured before this pass read runtime
    /// scope state. Call this only after native `synchronize` succeeds.
    pub fn acknowledge(&self, pass: NativeWatchSynchronizationPass) {
        let (state, wake) = &*self.state;
        let mut state = state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if pass.generation > state.acknowledged_generation {
            state.acknowledged_generation = pass.generation;
            wake.notify_all();
        }
    }

    /// Returns whether this pass is responsible for acknowledging a request.
    /// It is path-free and lets a runtime distinguish an explicit
    /// registration-only wake from ordinary watcher work.
    pub fn has_pending(&self, pass: NativeWatchSynchronizationPass) -> bool {
        let (state, _) = &*self.state;
        let state = state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        pass.generation > state.acknowledged_generation
    }

    /// Waits for the watch runtime to acknowledge the requested generation.
    /// `false` means the post-mutation synchronization was not confirmed
    /// before the deadline; it never rolls back or implies that revocation was
    /// not durably applied. The result carries no path or scope identity.
    pub fn wait_for(&self, ticket: NativeWatchSynchronizationTicket, timeout: Duration) -> bool {
        let deadline = Instant::now() + timeout;
        let (state, wake) = &*self.state;
        let mut state = state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        while state.acknowledged_generation < ticket.generation {
            let now = Instant::now();
            if now >= deadline {
                return false;
            }
            let remaining = deadline.saturating_duration_since(now);
            state = match wake.wait_timeout(state, remaining) {
                Ok((state, _)) => state,
                Err(poisoned) => poisoned.into_inner().0,
            };
        }
        true
    }
}

pub(super) struct NativeWatchScope {
    pub scope_id: i64,
    pub root: PathBuf,
}

struct RawNativeEvent {
    paths: Vec<PathBuf>,
}

struct NativeCallbackState {
    sender: SyncSender<RawNativeEvent>,
    queue: Arc<NativeEventQueueControl>,
    retirement: NativeWatchCallbackRetirement,
    wake: Arc<dyn Fn() + Send + Sync>,
}

impl NativeCallbackState {
    fn request_reconciliation(&self, overflowed: bool) {
        if self.retirement.run_if_admitted(|| {
            self.queue.reconcile_all.store(true, Ordering::Release);
            if overflowed {
                self.queue.overflow_count.fetch_add(1, Ordering::Relaxed);
            }
        }) {
            (self.wake)();
        }
    }

    fn fail_source(&self) {
        if self.retirement.run_if_admitted(|| {
            self.queue.source_failed.store(true, Ordering::Release);
            self.queue.reconcile_all.store(true, Ordering::Release);
        }) {
            (self.wake)();
        }
    }
}

impl EventHandler for NativeCallbackState {
    fn handle_event(&mut self, event: notify::Result<Event>) {
        let Some(_callback_lease) = self.retirement.begin_callback() else {
            return;
        };
        let Ok(event) = event else {
            self.fail_source();
            return;
        };
        if event.need_rescan() {
            self.request_reconciliation(false);
            return;
        }
        if event.paths.is_empty()
            || event.paths.len() > MAX_NATIVE_PATHS_PER_EVENT
            || event.paths.iter().any(|path| !path.is_absolute())
        {
            self.request_reconciliation(event.paths.len() > MAX_NATIVE_PATHS_PER_EVENT);
            return;
        }
        let Some(path_bytes) = event.paths.iter().try_fold(0_usize, |total, path| {
            total.checked_add(path.as_os_str().len())
        }) else {
            self.request_reconciliation(true);
            return;
        };
        if path_bytes > MAX_NATIVE_PATH_BYTES_PER_EVENT {
            self.request_reconciliation(true);
            return;
        }

        let mut wake = false;
        let mut overflowed = false;
        let mut disconnected = false;
        let admitted = self.retirement.run_if_admitted(|| {
            self.queue.queued_count.fetch_add(1, Ordering::AcqRel);
            match self.sender.try_send(RawNativeEvent { paths: event.paths }) {
                Ok(()) => wake = true,
                Err(TrySendError::Full(_)) => {
                    self.queue.queued_count.fetch_sub(1, Ordering::AcqRel);
                    self.queue.reconcile_all.store(true, Ordering::Release);
                    self.queue.overflow_count.fetch_add(1, Ordering::Relaxed);
                    overflowed = true;
                }
                Err(TrySendError::Disconnected(_)) => {
                    self.queue.queued_count.fetch_sub(1, Ordering::AcqRel);
                    self.queue.source_failed.store(true, Ordering::Release);
                    self.queue.reconcile_all.store(true, Ordering::Release);
                    disconnected = true;
                }
            }
        });
        if admitted && (wake || overflowed || disconnected) {
            (self.wake)();
        }
    }
}

struct NativeEventQueueControl {
    receiver: Mutex<Receiver<RawNativeEvent>>,
    queued_count: AtomicUsize,
    reconcile_all: AtomicBool,
    source_failed: AtomicBool,
    overflow_count: AtomicU64,
}

impl NativeEventQueueControl {
    fn clear(&self) {
        let receiver = self
            .receiver
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        while receiver.try_recv().is_ok() {}
        self.queued_count.store(0, Ordering::Release);
        self.reconcile_all.store(false, Ordering::Release);
        self.source_failed.store(false, Ordering::Release);
        self.overflow_count.store(0, Ordering::Release);
    }
}

struct NativeEventQueue {
    control: Arc<NativeEventQueueControl>,
    #[cfg(test)]
    test_sender: SyncSender<RawNativeEvent>,
    wake: Arc<dyn Fn() + Send + Sync>,
}

impl NativeEventQueue {
    fn new(
        capacity: usize,
        wake: Arc<dyn Fn() + Send + Sync>,
        retirement: NativeWatchCallbackRetirement,
    ) -> Result<(NativeCallbackState, Self), WatcherError> {
        let (sender, receiver) = sync_channel(capacity);
        let control = Arc::new(NativeEventQueueControl {
            receiver: Mutex::new(receiver),
            queued_count: AtomicUsize::new(0),
            reconcile_all: AtomicBool::new(false),
            source_failed: AtomicBool::new(false),
            overflow_count: AtomicU64::new(0),
        });
        if !retirement.register_queue(&control) {
            return Err(WatcherError::EventSourceFailed);
        }
        Ok((
            NativeCallbackState {
                sender: sender.clone(),
                queue: Arc::clone(&control),
                retirement,
                wake: Arc::clone(&wake),
            },
            Self {
                control,
                #[cfg(test)]
                test_sender: sender,
                wake,
            },
        ))
    }

    fn drain(&self, logical_scopes: &BTreeMap<i64, PathBuf>, limit: usize) -> NativeWatchBatch {
        let mut hints_by_scope = BTreeMap::<i64, PathBuf>::new();
        let mut reconcile_scope_ids = BTreeSet::new();
        let reconcile_all = self.control.reconcile_all.swap(false, Ordering::AcqRel);
        let mut signal_count = 0_u64;
        let receiver = self
            .control
            .receiver
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());

        for _ in 0..limit {
            let raw = match receiver.try_recv() {
                Ok(raw) => raw,
                Err(TryRecvError::Empty | TryRecvError::Disconnected) => break,
            };
            self.control.queued_count.fetch_sub(1, Ordering::AcqRel);
            signal_count = signal_count.saturating_add(1);
            for path in raw.paths {
                for (scope_id, root) in logical_scopes {
                    if path.starts_with(root) {
                        match hints_by_scope.entry(*scope_id) {
                            std::collections::btree_map::Entry::Vacant(entry) => {
                                entry.insert(path.clone());
                            }
                            std::collections::btree_map::Entry::Occupied(entry)
                                if entry.get() != &path =>
                            {
                                // One ordinary hint per logical scope bounds
                                // downstream validation work. A second distinct
                                // path (including ordered rename old/new paths)
                                // cannot be dropped: it requests durable root
                                // recovery for that scope instead.
                                reconcile_scope_ids.insert(*scope_id);
                            }
                            std::collections::btree_map::Entry::Occupied(_) => {}
                        }
                    }
                }
                // Raw paths are only positive, currently-authorized hints.
                // An unmatched path can be a delayed callback from a revoked
                // root or an unrelated native detail. It must not widen the
                // still-active coverage into reconciliation. Overflow, rescan
                // and source-failure flags remain durable recovery signals.
            }
        }

        let more_pending = self.control.queued_count.load(Ordering::Acquire) > 0;
        if more_pending {
            (self.wake)();
        }
        NativeWatchBatch {
            hints: hints_by_scope
                .into_iter()
                .map(|(scope_id, path)| WatchHint { scope_id, path })
                .collect(),
            reconcile_scope_ids,
            reconcile_all,
            source_failed: self.control.source_failed.load(Ordering::Acquire),
            more_pending,
            signal_count,
            overflow_count: self.control.overflow_count.swap(0, Ordering::AcqRel),
        }
    }

    #[cfg(test)]
    fn enqueue_test_event(&self, paths: Vec<PathBuf>) {
        self.control.queued_count.fetch_add(1, Ordering::AcqRel);
        if self.test_sender.try_send(RawNativeEvent { paths }).is_err() {
            self.control.queued_count.fetch_sub(1, Ordering::AcqRel);
            panic!("test queue should have capacity");
        }
    }
}

pub(super) struct NativeWatchBatch {
    pub hints: Vec<WatchHint>,
    pub reconcile_scope_ids: BTreeSet<i64>,
    pub reconcile_all: bool,
    pub source_failed: bool,
    pub more_pending: bool,
    pub signal_count: u64,
    pub overflow_count: u64,
}

pub struct NativeWatchEventSource {
    watcher: RecommendedWatcher,
    queue: NativeEventQueue,
    retirement: NativeWatchCallbackRetirement,
    logical_scopes: BTreeMap<i64, PathBuf>,
    physical_roots: BTreeSet<PathBuf>,
}

impl NativeWatchEventSource {
    pub fn new(wake: Arc<dyn Fn() + Send + Sync>) -> Result<Self, WatcherError> {
        Self::new_with_retirement(wake, NativeWatchCallbackRetirement::default())
    }

    pub fn new_with_retirement(
        wake: Arc<dyn Fn() + Send + Sync>,
        retirement: NativeWatchCallbackRetirement,
    ) -> Result<Self, WatcherError> {
        let (callback, queue) =
            NativeEventQueue::new(NATIVE_EVENT_QUEUE_CAPACITY, wake, retirement.clone())?;
        let watcher =
            RecommendedWatcher::new(callback, Config::default().with_follow_symlinks(false))
                .map_err(|_| WatcherError::EventSourceFailed)?;
        if retirement.is_retired() {
            return Err(WatcherError::EventSourceFailed);
        }
        Ok(Self {
            watcher,
            queue,
            retirement,
            logical_scopes: BTreeMap::new(),
            physical_roots: BTreeSet::new(),
        })
    }

    pub fn watched_scope_count(&self) -> usize {
        self.logical_scopes.len()
    }

    pub fn source_failed(&self) -> bool {
        !self.retirement.is_retired() && self.queue.control.source_failed.load(Ordering::Acquire)
    }

    #[cfg(test)]
    pub(crate) fn enqueue_test_event(&self, paths: Vec<PathBuf>) {
        self.queue.enqueue_test_event(paths);
    }

    pub(super) fn synchronize(
        &mut self,
        desired: Vec<NativeWatchScope>,
    ) -> Result<bool, WatcherError> {
        if self.retirement.is_retired() {
            return Err(WatcherError::EventSourceFailed);
        }
        let desired_logical = desired
            .into_iter()
            .map(|scope| (scope.scope_id, scope.root))
            .collect::<BTreeMap<_, _>>();
        let desired_physical = minimal_physical_roots(desired_logical.values());
        // Only a new logical root or a retargeted existing scope leaves a
        // registration gap. Removing a root must still unregister it now, but
        // cannot justify reopening the remaining authorized coverage.
        let requires_registration_gap_reconciliation = desired_logical
            .iter()
            .any(|(scope_id, root)| self.logical_scopes.get(scope_id) != Some(root));

        for root in desired_physical.difference(&self.physical_roots) {
            self.watcher
                .watch(root, RecursiveMode::Recursive)
                .map_err(|_| WatcherError::EventSourceFailed)?;
        }
        for root in self.physical_roots.difference(&desired_physical) {
            self.watcher
                .unwatch(root)
                .map_err(|_| WatcherError::EventSourceFailed)?;
        }

        self.logical_scopes = desired_logical;
        self.physical_roots = desired_physical;
        Ok(requires_registration_gap_reconciliation)
    }

    pub(super) fn drain(&self, limit: usize) -> NativeWatchBatch {
        if self.retirement.is_retired() {
            return NativeWatchBatch {
                hints: Vec::new(),
                reconcile_scope_ids: BTreeSet::new(),
                reconcile_all: false,
                source_failed: false,
                more_pending: false,
                signal_count: 0,
                overflow_count: 0,
            };
        }
        self.queue.drain(&self.logical_scopes, limit)
    }
}

fn minimal_physical_roots<'a>(roots: impl Iterator<Item = &'a PathBuf>) -> BTreeSet<PathBuf> {
    let mut candidates = roots.cloned().collect::<Vec<_>>();
    candidates.sort_by(|left, right| {
        left.components()
            .count()
            .cmp(&right.components().count())
            .then_with(|| left.cmp(right))
    });
    let mut minimal = BTreeSet::new();
    for root in candidates {
        if !minimal
            .iter()
            .any(|existing: &PathBuf| root.starts_with(existing))
        {
            minimal.insert(root);
        }
    }
    minimal
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::AtomicUsize;
    use std::time::Duration;
    #[cfg(target_os = "macos")]
    use std::time::Instant;

    use notify::event::Flag;
    use notify::{Error, EventKind};

    use super::*;

    fn wake_counter() -> (Arc<AtomicUsize>, Arc<dyn Fn() + Send + Sync>) {
        let count = Arc::new(AtomicUsize::new(0));
        let callback_count = Arc::clone(&count);
        let wake = Arc::new(move || {
            callback_count.fetch_add(1, Ordering::Relaxed);
        });
        (count, wake)
    }

    fn native_queue(
        capacity: usize,
        wake: Arc<dyn Fn() + Send + Sync>,
    ) -> (NativeCallbackState, NativeEventQueue) {
        NativeEventQueue::new(capacity, wake, NativeWatchCallbackRetirement::default())
            .expect("native queue should initialize")
    }

    #[test]
    fn synchronization_barrier_requires_a_pass_that_started_after_the_request() {
        let barrier = NativeWatchSynchronizationBarrier::default();
        assert!(!barrier.has_pending(barrier.begin_pass()));
        let stale_pass = barrier.begin_pass();
        let ticket = barrier.request();
        assert!(barrier.has_pending(barrier.begin_pass()));

        barrier.acknowledge(stale_pass);
        assert!(
            !barrier.wait_for(ticket, Duration::ZERO),
            "a pass with an older scope snapshot cannot acknowledge revocation"
        );

        let current_pass = barrier.begin_pass();
        barrier.acknowledge(current_pass);
        assert!(!barrier.has_pending(current_pass));
        assert!(
            barrier.wait_for(ticket, Duration::ZERO),
            "a successful current pass should acknowledge without any reconciliation request"
        );
    }

    #[test]
    fn callback_queue_is_nonblocking_and_overflow_requests_full_reconciliation() {
        let (wake_count, wake) = wake_counter();
        let (mut callback, queue) = native_queue(1, wake);
        let root = PathBuf::from("/authorized");
        let scopes = BTreeMap::from([(7, root.clone())]);

        callback.handle_event(Ok(Event::new(EventKind::Any).add_path(root.join("one.md"))));
        callback.handle_event(Ok(Event::new(EventKind::Any).add_path(root.join("two.md"))));

        let batch = queue.drain(&scopes, MAX_NATIVE_SIGNALS_PER_CYCLE);
        assert_eq!(batch.signal_count, 1);
        assert_eq!(batch.hints.len(), 1);
        assert!(batch.reconcile_all);
        assert_eq!(batch.overflow_count, 1);
        assert!(wake_count.load(Ordering::Relaxed) >= 2);
    }

    #[test]
    fn callback_retirement_clears_paths_and_rejects_future_events() {
        let retirement = NativeWatchCallbackRetirement::default();
        let (wake_count, wake) = wake_counter();
        let (mut callback, queue) =
            NativeEventQueue::new(2, wake, retirement.clone()).expect("queue should initialize");
        let root = PathBuf::from("/authorized");
        let scopes = BTreeMap::from([(7, root.clone())]);

        callback.handle_event(Ok(
            Event::new(EventKind::Any).add_path(root.join("queued-before-retirement.md"))
        ));
        assert_eq!(queue.control.queued_count.load(Ordering::Acquire), 1);
        assert!(retirement.retire_and_clear(Duration::from_millis(100)));
        let wakes_after_retirement = wake_count.load(Ordering::Acquire);

        let cleared = queue.drain(&scopes, MAX_NATIVE_SIGNALS_PER_CYCLE);
        assert_eq!(cleared.signal_count, 0);
        assert!(!cleared.reconcile_all);
        assert!(!cleared.source_failed);
        assert_eq!(cleared.overflow_count, 0);

        callback.handle_event(Ok(
            Event::new(EventKind::Any).add_path(root.join("rejected-after-retirement.md"))
        ));
        assert_eq!(wake_count.load(Ordering::Acquire), wakes_after_retirement);
        assert_eq!(queue.control.queued_count.load(Ordering::Acquire), 0);
    }

    #[test]
    fn callback_retirement_is_bounded_and_still_clears_an_in_flight_queue() {
        let retirement = NativeWatchCallbackRetirement::default();
        let (callback_started_tx, callback_started_rx) = sync_channel(1);
        let (release_callback_tx, release_callback_rx) = sync_channel(1);
        let release_callback_rx = Arc::new(Mutex::new(release_callback_rx));
        let wake: Arc<dyn Fn() + Send + Sync> = Arc::new(move || {
            callback_started_tx
                .send(())
                .expect("callback start should be observed");
            release_callback_rx
                .lock()
                .expect("release receiver should lock")
                .recv()
                .expect("callback should be released");
        });
        let (mut callback, queue) =
            NativeEventQueue::new(2, wake, retirement.clone()).expect("queue should initialize");

        let callback_thread = std::thread::spawn(move || {
            callback.handle_event(Ok(
                Event::new(EventKind::Any).add_path(PathBuf::from("/authorized/in-flight.md"))
            ));
        });
        callback_started_rx
            .recv()
            .expect("callback should reach the bounded wake");

        assert!(
            !retirement.retire_and_clear(Duration::from_millis(10)),
            "a stuck pre-retirement callback must not block the command forever"
        );
        assert_eq!(queue.control.queued_count.load(Ordering::Acquire), 0);
        assert_eq!(
            queue
                .drain(&BTreeMap::new(), MAX_NATIVE_SIGNALS_PER_CYCLE)
                .signal_count,
            0
        );

        release_callback_tx
            .send(())
            .expect("callback release should be delivered");
        callback_thread
            .join()
            .expect("callback thread should finish");
        assert!(retirement.retire_and_clear(Duration::from_millis(100)));
    }

    #[test]
    fn retired_callback_handle_prevents_native_source_creation() {
        let retirement = NativeWatchCallbackRetirement::default();
        assert!(retirement.retire_and_clear(Duration::ZERO));
        let (_, wake) = wake_counter();
        assert!(NativeWatchEventSource::new_with_retirement(wake, retirement).is_err());
    }

    #[test]
    fn callback_rescan_and_source_errors_never_forward_vendor_details() {
        let (_, wake) = wake_counter();
        let (mut callback, queue) = native_queue(2, wake);
        let rescan = Event::new(EventKind::Other).set_flag(Flag::Rescan);
        callback.handle_event(Ok(rescan));
        callback.handle_event(Err(Error::generic("private vendor path")));

        let batch = queue.drain(&BTreeMap::new(), MAX_NATIVE_SIGNALS_PER_CYCLE);
        assert!(batch.reconcile_all);
        assert!(batch.source_failed);
        assert_eq!(batch.signal_count, 0);
    }

    #[test]
    fn routing_happens_outside_the_callback_and_includes_nested_scopes() {
        let (_, wake) = wake_counter();
        let (mut callback, queue) = native_queue(2, wake);
        let root = PathBuf::from("/authorized");
        let nested = root.join("project");
        let scopes = BTreeMap::from([(1, root), (2, nested.clone())]);
        callback.handle_event(Ok(
            Event::new(EventKind::Any).add_path(nested.join("note.md"))
        ));

        let batch = queue.drain(&scopes, MAX_NATIVE_SIGNALS_PER_CYCLE);
        assert_eq!(
            batch
                .hints
                .iter()
                .map(|hint| hint.scope_id)
                .collect::<Vec<_>>(),
            vec![1, 2]
        );
        assert!(!batch.reconcile_all);
    }

    #[test]
    fn ordered_temporary_to_final_rename_requests_scope_reconciliation() {
        let (_, wake) = wake_counter();
        let (mut callback, queue) = native_queue(2, wake);
        let root = PathBuf::from("/authorized");
        let scopes = BTreeMap::from([(7, root.clone())]);
        callback.handle_event(Ok(Event::new(EventKind::Any)
            .add_path(root.join("report.crdownload"))
            .add_path(root.join("report.pdf"))));

        let batch = queue.drain(&scopes, MAX_NATIVE_SIGNALS_PER_CYCLE);

        assert_eq!(batch.signal_count, 1);
        assert_eq!(batch.hints.len(), 1);
        assert_eq!(
            batch.hints[0].path,
            root.join("report.crdownload"),
            "the first bounded hint may remain, but the second path must not be silently lost"
        );
        assert_eq!(batch.reconcile_scope_ids, BTreeSet::from([7]));
        assert!(!batch.reconcile_all);
    }

    #[test]
    fn physical_roots_are_prefix_minimized_without_losing_logical_scopes() {
        let parent = PathBuf::from("/authorized");
        let nested = parent.join("project");
        let separate = PathBuf::from("/other");
        let roots = [nested, separate.clone(), parent.clone()];

        assert_eq!(
            minimal_physical_roots(roots.iter()),
            BTreeSet::from([parent, separate])
        );
    }

    #[test]
    fn synchronize_requires_reconciliation_only_for_new_or_retargeted_logical_coverage() {
        let directory = tempfile::tempdir().expect("fixture root should exist");
        let root = directory
            .path()
            .canonicalize()
            .expect("fixture root should canonicalize");
        let (_, wake) = wake_counter();
        let mut source = NativeWatchEventSource::new(wake).expect("source should start");
        let desired = || {
            vec![NativeWatchScope {
                scope_id: 1,
                root: root.clone(),
            }]
        };

        assert!(
            source
                .synchronize(desired())
                .expect("first registration should pass")
        );
        assert_eq!(source.watched_scope_count(), 1);
        assert!(
            !source
                .synchronize(desired())
                .expect("identical watch set should pass")
        );
        assert!(
            !source
                .synchronize(Vec::new())
                .expect("watch removal should pass"),
            "pure removal must not request a registration-gap reconciliation"
        );
        assert_eq!(source.watched_scope_count(), 0);

        let retargeted = root.join("retargeted");
        std::fs::create_dir(&retargeted).expect("retarget root should exist");
        assert!(
            source
                .synchronize(vec![NativeWatchScope {
                    scope_id: 1,
                    root: retargeted,
                }])
                .expect("retarget should pass"),
            "changing an existing logical root must close its registration gap"
        );
    }

    #[test]
    fn unmatched_native_path_is_discarded_without_reconciling_active_coverage() {
        let (_, wake) = wake_counter();
        let (mut callback, queue) = native_queue(2, wake);
        let unmatched = PathBuf::from("/revoked");
        let active = PathBuf::from("/active");
        callback.handle_event(Ok(
            Event::new(EventKind::Any).add_path(unmatched.join("old.md"))
        ));

        let batch = queue.drain(&BTreeMap::from([(2, active)]), MAX_NATIVE_SIGNALS_PER_CYCLE);

        assert_eq!(batch.signal_count, 1);
        assert!(batch.hints.is_empty());
        assert!(batch.reconcile_scope_ids.is_empty());
        assert!(
            !batch.reconcile_all,
            "an unmatched path must not rescan the remaining coverage"
        );
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn macos_recommended_watcher_delivers_a_live_file_hint() {
        let directory = tempfile::tempdir().expect("fixture root should exist");
        let root = directory
            .path()
            .canonicalize()
            .expect("fixture root should canonicalize");
        let (_, wake) = wake_counter();
        let mut source = NativeWatchEventSource::new(wake).expect("source should start");
        assert!(
            source
                .synchronize(vec![NativeWatchScope {
                    scope_id: 1,
                    root: root.clone(),
                }])
                .expect("scope should register")
        );

        let file = root.join("native.md");
        std::fs::write(&file, "one").expect("file should create");
        let deadline = Instant::now() + Duration::from_secs(10);
        while Instant::now() < deadline {
            let batch = source.drain(MAX_NATIVE_SIGNALS_PER_CYCLE);
            if batch.signal_count > 0 || batch.reconcile_all {
                assert_eq!(batch.hints.first().map(|hint| hint.scope_id), Some(1));
                return;
            }
            std::thread::sleep(Duration::from_millis(50));
        }
        panic!("native source should deliver a path-free signal within the deadline");
    }
}
