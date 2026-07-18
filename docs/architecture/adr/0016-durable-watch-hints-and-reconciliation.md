# ADR-016: Durable Watch Hints Use Stability Gates and Atomic Reconciliation

- Status: Accepted
- Date: 2026-07-16

## Context

Filesystem watcher events are lossy, duplicated, reordered, platform-specific, and vulnerable to event storms. A path in an event can disappear, be renamed, become a symlink/reparse point, or refer outside the authorized scope before DeskGraph processes it. Downloads can remain writable for long periods. Treating an event as graph truth would create duplicate identities, stale content, scope escapes, and partial live state after a crash.

## Decision

- `WatchEventSource` is an adapter boundary that emits untrusted `(scope_id, path)` hints. The initial decision selected no native adapter; the 2026-07-18 implementation addendum below records the separately audited native choice.
- The core revalidates the current canonical authorized scope before persisting or opening a hinted path. Existing symlinks/reparse points and out-of-scope paths fail closed; missing paths are resolved only through a canonical existing ancestor without accepting parent traversal.
- SQLite migration 0006 persists path-local watch state. Only one `stabilizing` event per scope exists at a time, so event and rename storms coalesce and reset the ordinary stability deadline. The coordinator separately caps that coalescing age at its bounded periodic-reconciliation interval, so continuous writes cannot postpone metadata reconciliation forever. A reconciliation already running may coexist with one later stabilizing event.
- The default stability window is one second and the accepted policy range is 250 ms to 60 seconds. A file must have unchanged existence/kind, size, modified time, and platform identity across checks; an existing file must also open read-only and match its open-handle identity. `.part`, `.crdownload`, and `.download` paths are ignored.
- When stable, ambiguous events and every unsupported incremental case link to a normal resumable full-scope scan in one SQLite transaction. Migration 0018 additionally permits the narrow same-path/same-identity regular-file metadata delta described below. Both paths publish through database-owned atomic transactions; a raw adapter event never mutates live graph rows.
- Stabilizing and reconciling state survives process restart. A linked ready/interrupted scan resumes through the existing lease/recovery path. Rename reconciliation preserves node identity where platform metadata permits.
- CLI and Desktop status expose only fixed states, IDs, counts, timestamps, scan IDs, and closed reason codes. Observed paths and content remain local database details and are absent from ordinary payloads/logs.

## Consequences

- The safe baseline remains full authorized-scope reconciliation. The accepted file-delta addendum accelerates only a proven existing-file modify; it is not a generic create/delete/rename/subtree indexer.
- Cloud-placeholder detection, incremental extraction/indexing, low-memory/background policies, and Smart Inbox remain separate gates.
- A native watcher dependency cannot be adopted without the normal official API, maintenance, platform, license, closure, and security audit.

## Implementation addendum — 2026-07-18

- Exact `notify 8.2.0` passed the required audit and implements the target-native source behind this unchanged trust boundary: macOS FSEvents, Windows `ReadDirectoryChangesW`, Linux inotify and BSD/iOS kqueue.
- One process-wide source prefix-minimizes physical watch roots while routing outside the callback to all matching logical scopes. The callback can only place bounded raw hints or fixed recovery flags into a non-blocking queue and send a non-blocking wake token.
- Downstream validation remains bounded to one ordinary hint per logical scope per drain. If the same batch or an ordered rename event contains a second distinct path for that scope, the adapter retains the bounded hint and requests durable root recovery for that scope; an old temporary/hidden path therefore cannot silently discard a final path.
- Empty, relative, oversized, overflowed, rescan, source-error or unmatched events request whole-scope durable reconciliation. A five-minute periodic reconciliation remains enabled, so native delivery is an optimization and latency improvement rather than graph truth.
- Any logical or physical watch-set change is reported to the Tauri runtime and immediately requests whole-scope reconciliation, closing the gap between a completed Initial Manifest Scan and native registration. If a scope already has a stabilizing event, recovery uses an explicit root-only forced metadata transaction; that transaction rechecks the event's scope, exact stored authorized root bytes/key, and completed Initial Manifest Scan before linking work. Ordinary per-path reconciliation still cannot bypass its stability deadline.
- A stabilizing event's maximum coalescing age is the bounded periodic interval. At that age the same root-only durable recovery path starts even if new hints kept resetting the ordinary debounce. The next wake is capped by the original event age, and a linked multi-batch scan remains durable across later cycles or restart. A recovery request received while that scan is already reconciling remains pending; after the old snapshot finishes, the coordinator schedules and forces a fresh root scan so the request is not incorrectly satisfied by pre-signal enumeration.
- The database rejects watch observations until the scope has a completed Initial Manifest Scan. Scanner and hint paths share the same temporary-download suffix policy, preventing the periodic fallback from indexing partial downloads.
- Direct ignored observations update or create one latest terminal aggregate per scope and closed reason inside an immediate SQLite transaction, but never inspect, overwrite, or delete unrelated stabilizing work. Only an explicit `mark_watch_event_ignored_at(event_id, reason)` transition may merge that identified nonterminal row into an existing terminal aggregate; its latest snapshot and observation count transfer before the superseded row is removed. No file, manifest/graph/content row, action plan, or action-journal event is deleted.
- Local macOS arm64 native source and Desktop create/modify/rename/delete/identity tests pass outside the restricted Codex sandbox; modify acceptance directly verifies the published manifest size. Windows/Linux/macOS Intel runtime and large-tree evidence remain open; this addendum does not mark M6 complete.

## Existing-file metadata delta addendum — 2026-07-18

- Migration 0018 persists a closed `file_delta` or `full_scope` reconciliation kind. Escalation is monotonic: a different path, different identity, missing/folder hint, native ambiguity, overflow, rescan, source failure, periodic recovery, or an already-full event can never be downgraded. Whole-source recovery records every watchable scope in one `BEGIN IMMEDIATE` transaction, so a crash cannot retain only a subset of the recovery request.
- The delta allowlist is intentionally narrow and currently Unix-only: one already-present regular file, exact raw and comparison paths, unchanged stable platform identity, one link, one present location across every authorized scope, exact present root and parent identities, an active `located_in` edge to that parent, and no running/interrupted scan writer. A newly created or missing path, directory, rename, identity/kind replacement, hard link, nested/cross-scope location, fallback identity, topology mismatch, or any failed proof uses the existing full-scope scan.
- Unix validation opens `/` and each authorized-root component through held descriptor-relative `openat` calls with `O_NOFOLLOW`, `O_DIRECTORY`, `O_CLOEXEC`, and `O_NOCTTY`. Parent and leaf opens remain descriptor-relative; the leaf also uses `O_NONBLOCK` before proving it is a regular file. Two complete anchored snapshots must agree on root, parent, leaf, size, modified time, and link count, and the second set of handles remains held through database publication. No content is read. Windows returns an explicit ineligible result and therefore takes the full-scope path until an equivalent handle-relative adapter is independently accepted.
- Publication uses a separate SQLite immediate transaction, not the full-scope scan finalizer. It compare-and-swaps the durable event snapshot, exact root/parent/location/node identities, old file metadata, one-location/one-link rules, and absence of another scan writer; then it updates only that node's metadata, conservatively deactivates its prior content/image outputs, and completes the watch event without a scan job. Sibling locations and graph edges are not republished or tombstoned. A failed check rolls back and durably escalates to full scope.
- This optimization never starts extraction, OCR, classification, an LLM, an organization plan, or a filesystem action. Five-minute full reconciliation remains the missed-event safety net. Generic create/delete/rename/subtree deltas, incremental extraction/indexing, Windows handle-relative validation, native runtime/load evidence, and Smart Inbox remain separate gates.

## Rejected alternatives

- Trust raw watcher create/rename/delete events as manifest mutations.
- Keep debounce state only in memory.
- Read or extract a newly observed file before it is stable.
- Reuse the current full-scope scan finalizer for a single path; its scope-wide tombstone semantics would mark unrelated locations absent.
- Adopt rename stitching or `notify-debouncer-full` as graph truth; platform events cannot prove atomic manifest coverage and the extra file-ID map would add unbounded resident state.
- Allow an adapter or LLM to perform filesystem organization actions.
