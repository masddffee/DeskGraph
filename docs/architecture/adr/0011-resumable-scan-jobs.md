# ADR-011 — Resumable scan jobs publish atomically

- Status: Accepted
- Date: 2026-07-16
- Owners: DeskGraph maintainers

## Context

M1 requires visible progress, pause, resume, and crash recovery. The initial scanner discovers an entire scope in memory and publishes all observations in one SQLite transaction. That is atomic, but it cannot survive process exit and gives a user no safe control over a large or slow scope.

Writing each discovered entry directly into the live manifest would make progress durable, but a paused, failed, or crashed scan could temporarily publish an incomplete graph and incorrectly mark older locations absent. DeskGraph also needs to distinguish a genuinely abandoned runner from a second CLI or desktop connection that is only requesting status or pause.

## Decision

- Persist one queue row for every path that still needs metadata inspection. Queue rows retain lossless path bytes, normalized comparison keys, parent identity, and processing state.
- Persist observations and issues in job-scoped staging tables. A batch transaction records the processed queue item, newly discovered children, staged output, counters, and renewed runner lease together.
- Keep the previous completed manifest readable while a job is running, paused, interrupted, or failed. Publish staged observations, reconcile absent locations and relations, and mark the job completed in one final transaction.
- Bound each worker batch. Progress is the durable pair `processed_entries / queued_entries`; the denominator may grow while folders are expanded.
- Model control separately from terminal status. A running job may be ready, actively leased, pause-requested, or paused. Completed and failed jobs cannot resume. An interrupted job may resume after scope revalidation.
- Use a short renewable lease with an opaque process-local runner token. Opening the database recovers only active jobs whose lease expired; it never interrupts a valid concurrent runner.
- Pause requests are durable. A ready job pauses immediately; an active worker observes the request between entries, releases its lease, and leaves all remaining queue items pending.
- Reset a queue item left in `processing` to `pending` only when claiming an interrupted or expired job. Processing is idempotent because queue and staged rows are unique per job and normalized path key.
- Re-canonicalize the authorized root and validate every queued observation against it on every resumed worker run.
- Never log queue paths, scope paths, filenames, staged text, or raw database errors.

## Consequences

Pause and crash no longer require rescanning the entire tree, and the user never sees a half-published manifest. SQLite grows temporarily during a scan because both the previous live state and new staged state coexist. Completed-job staging and queue rows can be removed transactionally after publish; scan counters and bounded issue records remain for history.

The lease adds clock-based recovery behavior. It is not a distributed lock or security boundary; SQLite transactions and runner-token validation remain authoritative. A single unusually slow filesystem call can outlive a lease, so the worker must renew before and after each entry and refuse to stage output if ownership changed.

## Alternatives considered

- In-memory pause was rejected because it cannot survive application exit.
- Publishing every batch directly into live tables was rejected because partial results and stale-location reconciliation become user-visible.
- Holding one SQLite write transaction for the entire filesystem walk was rejected because it prevents responsive pause/status connections and can hold locks for minutes.
- Treating every `running` row as crashed whenever a process opens the database was rejected because a concurrent pause/status command would interrupt a healthy runner.

## Validation and revisit trigger

Acceptance requires pause-before-run, pause-during-run, resume, expired-lease recovery, process-interruption simulation, idempotent queue replay, atomic publish, scope-change rejection, and complete-rescan regression tests. Revisit the lease interval and queue compaction after measured slow/removable/network filesystem evidence or when M6 watcher reconciliation shares this job engine.
