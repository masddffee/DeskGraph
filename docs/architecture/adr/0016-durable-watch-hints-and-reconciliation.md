# ADR-016: Durable Watch Hints Use Stability Gates and Atomic Reconciliation

- Status: Accepted
- Date: 2026-07-16

## Context

Filesystem watcher events are lossy, duplicated, reordered, platform-specific, and vulnerable to event storms. A path in an event can disappear, be renamed, become a symlink/reparse point, or refer outside the authorized scope before DeskGraph processes it. Downloads can remain writable for long periods. Treating an event as graph truth would create duplicate identities, stale content, scope escapes, and partial live state after a crash.

## Decision

- `WatchEventSource` is an adapter boundary that emits untrusted `(scope_id, path)` hints. No native adapter is selected by this ADR.
- The core revalidates the current canonical authorized scope before persisting or opening a hinted path. Existing symlinks/reparse points and out-of-scope paths fail closed; missing paths are resolved only through a canonical existing ancestor without accepting parent traversal.
- SQLite migration 0006 persists path-local watch state. Only one `stabilizing` event per scope exists at a time, so event and rename storms coalesce and reset the stability deadline. A reconciliation already running may coexist with one later stabilizing event.
- The default stability window is one second and the accepted policy range is 250 ms to 60 seconds. A file must have unchanged existence/kind, size, modified time, and platform identity across checks; an existing file must also open read-only and match its open-handle identity. `.part`, `.crdownload`, and `.download` paths are ignored.
- When stable, the watch event and a normal resumable scan job are linked in one SQLite transaction. The existing scanner then revalidates the scope and publishes a complete manifest atomically. Events never write live graph rows directly.
- Stabilizing and reconciling state survives process restart. A linked ready/interrupted scan resumes through the existing lease/recovery path. Rename reconciliation preserves node identity where platform metadata permits.
- CLI and Desktop status expose only fixed states, IDs, counts, timestamps, scan IDs, and closed reason codes. Observed paths and content remain local database details and are absent from ordinary payloads/logs.

## Consequences

- This safe baseline performs a full authorized-scope reconciliation after a stable hint; it is correct but not yet an efficient per-node incremental indexer.
- Native macOS/Windows/Linux adapters, missed-event reconciliation schedules, cloud-placeholder detection, incremental extraction/indexing, low-memory/background policies, and Smart Inbox remain separate gates.
- A native watcher dependency cannot be adopted without the normal official API, maintenance, platform, license, closure, and security audit.

## Rejected alternatives

- Trust raw watcher create/rename/delete events as manifest mutations.
- Keep debounce state only in memory.
- Read or extract a newly observed file before it is stable.
- Allow an adapter or LLM to perform filesystem organization actions.
