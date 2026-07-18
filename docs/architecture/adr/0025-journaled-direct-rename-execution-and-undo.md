# ADR-025: Journaled Rename Protocol and Fail-Closed Execution Gate

- Status: Accepted
- Date: 2026-07-18

## Context

ADR-017 intentionally stops at an immutable, same-folder rename preview. A preview alone
does not make a filesystem mutation safe: its source can change, a destination can appear,
the process can terminate between a syscall and durable verification, and a retry can issue
the same action twice. DeskGraph must define the smallest execution protocol without weakening
the local-first, explicit-scope, no-permanent-delete, and no-LLM-filesystem-action invariants.

This decision covers the durable protocol and evidence for one potential operation: a direct
rename of one regular file inside the same canonical parent directory. It does not accept a
production platform adapter and is not a general Move or cleanup engine.

## Decision

### Bounded operation and availability

- The candidate operation is a `rename` of one already-scanned, currently-present regular
  file in one currently-authorized scope. The canonical source and destination must have the
  same canonical parent; the source must have a strong platform identity, never
  `path_fallback`.
- Only `direct` plans may enter a future execution adapter. `case_only_staged`, folder rename, Move, cross-volume
  copy, System Trash, and every permanent-delete or empty-trash behavior are out of scope.
- CLI exposes Preview, Status, and path-free History; Desktop exposes Preview and path-free
  History. Execute, Undo, and action recovery commands must remain absent until a later platform ADR and this contract's complete
  fault/runtime matrix pass. A future command must be plan-ID-only and must not accept a new
  source or destination path at execution time.
- Any unproven platform, identity primitive, no-overwrite primitive, or runtime gate fails
  closed. Compile success is not platform execution evidence.

### Immutable preview execution binding

- A newly created executable preview writes, in the same durable SQLite transaction as its
  immutable ActionPlan and `preview_created` event, immutable execution evidence containing:
  the SHA-256 of the bytes read from the verified source handle; byte count; source strong
  identity; source metadata; and strong identities for the authorized root and canonical
  parent directory.
- The source handle identity, source metadata, root identity, and parent identity are checked
  before that evidence is committed. The SHA-256 is streamed from the verified handle; no
  unbounded in-memory copy is permitted. The first executable slice hashes at most 8 GiB with
  a 64 KiB buffer and a 90-second deadline; a larger or slower source fails closed and requires
  a later, separately accepted large-file design.
- Pre-migration preview rows without an execution binding are permanently non-executable and
  require a fresh preview; no migration may invent or backfill a hash for them. The persisted
  v1 plan row format remains immutable, while the presence of the separately immutable binding
  distinguishes new executable previews and the runtime read model is independently versioned.

### Canonical durable journal and idempotency

- The database is the canonical transaction record. Plans, execution evidence, command
  requests, attempts, observations, and journal events are append-only and reject update or
  delete at the database boundary.
- A command has a bounded opaque request identifier and immutable command kind. A database
  uniqueness constraint returns the same result for an identical retry. An Undo retry after
  `undone` performs no filesystem mutation.
- A database compare-and-swap transition, performed in an immediate transaction, gives one
  cross-process claimant the right to advance a plan. A second process, duplicate click, or
  lost-response retry observes the existing attempt instead of issuing another syscall.
- A mutable, short-lived per-plan executor lease is operational serialization, not audit
  history. Execution and recovery acquire and renew it using database CAS, and every internal
  intent/outcome append proves current lease ownership. Lease expiry lets recovery claim an
  abandoned action. No SQLite transaction remains open across hashing or a filesystem syscall;
  the append-only journal remains the authority. An expiring lease cannot fence a stopped but
  still-live process. Any future adapter must additionally hold an accepted crash-released
  OS-lifetime fence across command execution and recovery. Its namespace must be trusted private
  or otherwise resist unlink/rename replacement; an adjacent lock in an arbitrary writable
  database parent is insufficient. No process-fence implementation is accepted by this ADR.
  Recovery must fail closed while another live process owns the future fence.
- Event sequence is strictly increasing and database-enforced. The allowed transition graph is
  closed and is derived from append-only events; there is no mutable `status` field whose value
  can replace journal history.
- Every filesystem mutation has a committed, durable intent event before the syscall. A
  successful syscall is never treated as committed merely because a caller received success.

### Immediate execution checks and platform primitives

- Immediately before an intent can be committed, the core revalidates the explicit scope,
  canonical root and parent identities, source regular-file/no-link status, source strong
  identity, metadata, and stored SHA-256. It verifies that the destination is absent and is
  still a single leaf name within the same canonical parent.
- The executor never calls `std::fs::rename`, generic delete APIs, `unlink`, or
  `remove_file`. It never overwrites a destination.
- Test-only macOS and Linux prototypes use descriptor-relative parent handling plus
  `renameatx_np(RENAME_EXCL | RENAME_NOFOLLOW_ANY)` or
  `renameat2(RENAME_NOREPLACE)`. These primitives prevent destination replacement and link
  traversal but still address the source by leaf name; they cannot atomically condition the
  rename on the exact open inode. They therefore do not satisfy the accepted production
  boundary. All production targets return `action_platform_rename_unsupported` before a
  command event, lock-file creation, or source mutation. Windows is independently unavailable
  pending a reviewed handle-bound `FILE_RENAME_INFO` implementation with
  `ReplaceIfExists = false` and real runtime evidence.
- After the platform syscall, the executor requires the platform's accepted parent-directory
  durability primitive to succeed; failure is ambiguous and cannot report completion. It then verifies the
  destination's strong identity and SHA-256, verifies source absence, rechecks parent/root
  identity, and only then appends `execution_completed` or `undo_completed`.

### Append-only states and observation-only recovery

The minimal closed event vocabulary is:

`preview_created`, `execute_requested`, `execute_request_not_started`,
`direct_rename_intent`, `execution_completed`, `execution_not_applied`,
`execution_needs_attention`, `undo_requested`, `undo_request_not_started`,
`undo_rename_intent`, `undo_completed`, `undo_not_applied`, and
`undo_needs_attention`.

- A fold of these events yields only `previewed`, `execute_requested`,
  `direct_rename_intent`, `executed`, `undo_requested`, `undo_rename_intent`, `undone`, or
  `needs_attention`.
- Undo requires a separate explicit durable user intent. It revalidates the exact destination
  receipt/evidence, source absence, root/parent identities, and uses the same identity-bound,
  no-overwrite platform primitive before `undo_completed`.
- The internal recovery protocol is observation-only. It inspects source and destination
  against the stored identities and hash, then appends one closed completed, not-applied, or
  needs-attention journal event. It never automatically retries, overwrites, deletes, cleans up, or chooses
  between ambiguous files.
- If source and destination both exist, both are absent, a root/parent changed, a link/reparse
  point appears, an identity/hash differs, a receipt is missing, or observation cannot complete,
  the only safe result is `needs_attention`.

### Privacy and presentation

- Explicit plan-detail responses may show the user-requested source and destination paths.
  Ordinary logs, history/list responses, command errors, telemetry, and recovery summaries are
  path-free and contain only closed state/outcome codes, IDs, and bounded counts.
- LLM output, generated rules, MCP, Watch, Inbox, and background workers cannot invoke this
  protocol. No production mutation entry point exists under this ADR.

## Required crash and fault matrix for a future adapter

The rows below define required behavior; deterministic protocol fixtures do not constitute
platform acceptance. Child-process termination and real platform fault injection remain open.

| Boundary                                                                                   | Required recovery result                                                                                                            |
| ------------------------------------------------------------------------------------------ | ----------------------------------------------------------------------------------------------------------------------------------- |
| Before `execute_requested`                                                                 | Remain `previewed`; no filesystem action.                                                                                           |
| After `execute_requested`, before durable rename intent                                    | Append `execute_request_not_started`; return to `previewed`.                                                                        |
| After durable rename intent, before syscall                                                | If source is exact and destination absent, append `execution_not_applied`; return to `previewed`.                                   |
| Syscall error, process kill, lost response, or crash after syscall but before verification | Observe both names and evidence; append recovered verified, not-applied, or `needs_attention`; never infer from the OS error alone. |
| After destination verification but before caller response                                  | Reopen derives `executed`; retry returns the recorded result and does not rename again.                                             |
| After `undo_requested`, before durable Undo intent                                         | Append `undo_request_not_started`; remain `executed`.                                                                               |
| After durable Undo intent, before syscall                                                  | If destination is exact and source absent, append `undo_not_applied`; remain `executed`.                                            |
| Undo syscall error, process kill, lost response, or crash before verification              | Observe both names and evidence; append recovered verified, not-applied, or `needs_attention`; never retry automatically.           |

## Acceptance requirements

- Before any adapter is enabled, Rust integration and fault-injection tests must prove normal execute/Undo, same request retry,
  cross-process single-claim CAS, response-loss retry, stale source/hash, destination race,
  permission denial, scope revocation, source/root/parent replacement, link/reparse insertion,
  process termination at every durable boundary, database reopen, parent durability failure,
  verification mismatch, and every crash-matrix row.
- Current tests must continue to prove no plan/action table can be updated or deleted, event sequence/transition checks
  are database-enforced, no production path calls `std::fs::rename` or a generic delete API,
  and ordinary payloads/logs are path-free.
- macOS and Linux require a superseding accepted design that closes or explicitly resolves the
  source-leaf identity race plus real platform-runtime fixtures. Windows requires real runtime
  fixtures for handle-bound rename. Unsupported targets remain unavailable. This ADR does not
  make any target release-ready by itself.
- CLI help must prove execution/recovery/Undo commands are absent while the gate is closed.
  Desktop execute stays absent until a later end-to-end product acceptance decision.

## Unix leaf-name residual risk

Directory descriptors and no-follow component handling bind the root and parent, while
no-replace flags prevent destination overwrite. Unix rename syscalls nevertheless operate on a
source leaf name rather than a source file handle; another actor can replace an ordinary file
between final observation and the syscall, causing the wrong file to move. Post-action
verification detects the mismatch only after mutation and is therefore insufficient. The
prototype is test-only and production stays fail closed. D-018 must select a primitive/threat
boundary and a superseding ADR before Unix execution can be exposed.

## Consequences

- This provides a narrowly auditable protocol and binding foundation, not a shipped executor
  or completion of M5. Move,
  cross-volume copy/verify/commit, case-only rename, folders, System Trash, and Desktop action
  controls retain independent ADRs, state machines, and platform fault matrices.
- Preview generation becomes more expensive because it hashes the source, but this is required
  to detect content changes that identity/size/mtime alone cannot prove absent.
- Older preview records remain readable and auditable but cannot be upgraded into executable
  plans.
- No user-visible execution result exists until an accepted adapter proves platform durability,
  exact-source safety, process fencing, and post-action verification; ambiguous external state
  is deliberately less convenient than unsafe recovery.

## Rejected alternatives

- Treat a successful `std::fs::rename` return as a completed transaction.
- Revalidate only the preview-time metadata or use paths as source identity.
- Allow destination overwrite, best-effort cleanup, automatic retry, or automatic rollback.
- Store mutable plan state, accept an unbound execute request, or deduplicate only in memory.
- Make case-only, folder, cross-volume, Move, or System Trash behavior a hidden branch of this
  direct-rename operation.
- Expose execution through LLM output, MCP, Watch, Inbox, generated rules, or a Desktop control
  before this ADR's runtime and fault-injection acceptance passes.
