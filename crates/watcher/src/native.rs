use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::mpsc::{Receiver, SyncSender, TryRecvError, TrySendError, sync_channel};

use notify::{Config, Event, EventHandler, RecommendedWatcher, RecursiveMode, Watcher};

use crate::{WatchHint, WatcherError};

pub(super) const MAX_NATIVE_SIGNALS_PER_CYCLE: usize = 64;
const NATIVE_EVENT_QUEUE_CAPACITY: usize = 256;
const MAX_NATIVE_PATHS_PER_EVENT: usize = 2;
const MAX_NATIVE_PATH_BYTES_PER_EVENT: usize = 16 * 1024;

pub(super) struct NativeWatchScope {
    pub scope_id: i64,
    pub root: PathBuf,
}

struct RawNativeEvent {
    paths: Vec<PathBuf>,
}

struct NativeCallbackState {
    sender: SyncSender<RawNativeEvent>,
    queued_count: Arc<AtomicUsize>,
    reconcile_all: Arc<AtomicBool>,
    source_failed: Arc<AtomicBool>,
    overflow_count: Arc<AtomicU64>,
    wake: Arc<dyn Fn() + Send + Sync>,
}

impl NativeCallbackState {
    fn request_reconciliation(&self, overflowed: bool) {
        self.reconcile_all.store(true, Ordering::Release);
        if overflowed {
            self.overflow_count.fetch_add(1, Ordering::Relaxed);
        }
        (self.wake)();
    }

    fn fail_source(&self) {
        self.source_failed.store(true, Ordering::Release);
        self.request_reconciliation(false);
    }
}

impl EventHandler for NativeCallbackState {
    fn handle_event(&mut self, event: notify::Result<Event>) {
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

        self.queued_count.fetch_add(1, Ordering::AcqRel);
        match self.sender.try_send(RawNativeEvent { paths: event.paths }) {
            Ok(()) => (self.wake)(),
            Err(TrySendError::Full(_)) => {
                self.queued_count.fetch_sub(1, Ordering::AcqRel);
                self.request_reconciliation(true);
            }
            Err(TrySendError::Disconnected(_)) => {
                self.queued_count.fetch_sub(1, Ordering::AcqRel);
                self.fail_source();
            }
        }
    }
}

struct NativeEventQueue {
    receiver: Receiver<RawNativeEvent>,
    queued_count: Arc<AtomicUsize>,
    reconcile_all: Arc<AtomicBool>,
    source_failed: Arc<AtomicBool>,
    overflow_count: Arc<AtomicU64>,
    wake: Arc<dyn Fn() + Send + Sync>,
}

impl NativeEventQueue {
    fn new(capacity: usize, wake: Arc<dyn Fn() + Send + Sync>) -> (NativeCallbackState, Self) {
        let (sender, receiver) = sync_channel(capacity);
        let queued_count = Arc::new(AtomicUsize::new(0));
        let reconcile_all = Arc::new(AtomicBool::new(false));
        let source_failed = Arc::new(AtomicBool::new(false));
        let overflow_count = Arc::new(AtomicU64::new(0));
        (
            NativeCallbackState {
                sender,
                queued_count: Arc::clone(&queued_count),
                reconcile_all: Arc::clone(&reconcile_all),
                source_failed: Arc::clone(&source_failed),
                overflow_count: Arc::clone(&overflow_count),
                wake: Arc::clone(&wake),
            },
            Self {
                receiver,
                queued_count,
                reconcile_all,
                source_failed,
                overflow_count,
                wake,
            },
        )
    }

    fn drain(&self, logical_scopes: &BTreeMap<i64, PathBuf>, limit: usize) -> NativeWatchBatch {
        let mut hints_by_scope = BTreeMap::<i64, PathBuf>::new();
        let mut reconcile_scope_ids = BTreeSet::new();
        let mut reconcile_all = self.reconcile_all.swap(false, Ordering::AcqRel);
        let mut signal_count = 0_u64;

        for _ in 0..limit {
            let raw = match self.receiver.try_recv() {
                Ok(raw) => raw,
                Err(TryRecvError::Empty | TryRecvError::Disconnected) => break,
            };
            self.queued_count.fetch_sub(1, Ordering::AcqRel);
            signal_count = signal_count.saturating_add(1);
            for path in raw.paths {
                let mut matched = false;
                for (scope_id, root) in logical_scopes {
                    if path.starts_with(root) {
                        matched = true;
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
                if !matched {
                    reconcile_all = true;
                }
            }
        }

        let more_pending = self.queued_count.load(Ordering::Acquire) > 0;
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
            source_failed: self.source_failed.load(Ordering::Acquire),
            more_pending,
            signal_count,
            overflow_count: self.overflow_count.swap(0, Ordering::AcqRel),
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
    logical_scopes: BTreeMap<i64, PathBuf>,
    physical_roots: BTreeSet<PathBuf>,
}

impl NativeWatchEventSource {
    pub fn new(wake: Arc<dyn Fn() + Send + Sync>) -> Result<Self, WatcherError> {
        let (callback, queue) = NativeEventQueue::new(NATIVE_EVENT_QUEUE_CAPACITY, wake);
        let watcher =
            RecommendedWatcher::new(callback, Config::default().with_follow_symlinks(false))
                .map_err(|_| WatcherError::EventSourceFailed)?;
        Ok(Self {
            watcher,
            queue,
            logical_scopes: BTreeMap::new(),
            physical_roots: BTreeSet::new(),
        })
    }

    pub fn watched_scope_count(&self) -> usize {
        self.logical_scopes.len()
    }

    pub fn source_failed(&self) -> bool {
        self.queue.source_failed.load(Ordering::Acquire)
    }

    pub(super) fn synchronize(
        &mut self,
        desired: Vec<NativeWatchScope>,
    ) -> Result<bool, WatcherError> {
        let desired_logical = desired
            .into_iter()
            .map(|scope| (scope.scope_id, scope.root))
            .collect::<BTreeMap<_, _>>();
        let desired_physical = minimal_physical_roots(desired_logical.values());
        let changed =
            desired_logical != self.logical_scopes || desired_physical != self.physical_roots;

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
        Ok(changed)
    }

    pub(super) fn drain(&self, limit: usize) -> NativeWatchBatch {
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
    #[cfg(target_os = "macos")]
    use std::time::{Duration, Instant};

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

    #[test]
    fn callback_queue_is_nonblocking_and_overflow_requests_full_reconciliation() {
        let (wake_count, wake) = wake_counter();
        let (mut callback, queue) = NativeEventQueue::new(1, wake);
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
    fn callback_rescan_and_source_errors_never_forward_vendor_details() {
        let (_, wake) = wake_counter();
        let (mut callback, queue) = NativeEventQueue::new(2, wake);
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
        let (mut callback, queue) = NativeEventQueue::new(2, wake);
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
        let (mut callback, queue) = NativeEventQueue::new(2, wake);
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
    fn synchronize_reports_watch_set_changes_once() {
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
        assert!(
            !source
                .synchronize(desired())
                .expect("identical watch set should pass")
        );
        assert!(
            source
                .synchronize(Vec::new())
                .expect("watch removal should pass")
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
