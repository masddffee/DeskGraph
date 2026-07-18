# ADR-029: Smart Cleanup Inbox Is a Derived Suggest-only Read Model

- Status: Accepted
- Date: 2026-07-19

## Context

DeskGraph already has three independent, immutable evidence sources that can help a user review
possible cleanup work: bounded exact-byte duplicate pairs, explicit numeric filename-version pairs,
and explainable screenshot review groups. None of those sources proves that a file is disposable,
and relation feedback records graph correctness rather than permission to clean a file.

Persisting another mutable Inbox candidate table before keeper, selection, confirmation and Trash
semantics are accepted would duplicate source evidence and create competing currentness rules. A
history-only relation summary is also insufficient: exact duplicates require a fresh complete byte
comparison and version relations require fresh identity, metadata and filename revalidation.

## Decision

- Smart Cleanup Inbox v1 is a path-free, derived read model over the immutable evidence accepted by
  ADR-020 through ADR-023 and ADR-028. It adds no Inbox, ActionPlan, Trash, deletion or Undo table.
- A user explicitly refreshes one authorized, completed-scan scope. Refresh is never automatic. The
  scope must have a matching active access grant before DeskGraph resolves a stored path or opens a
  file.
- Exact-duplicate and version sources are included only after their existing live verification has
  appended a fresh immutable observation during that refresh. Screenshot groups are included only
  when their current membership and evidence resolve to an existing immutable observation.
- Only `suggested` relation sources enter this Inbox. `accepted` and `rejected` relation feedback are
  both excluded because neither state is cleanup consent. Screenshot groups remain suggest-only.
- Each item exposes only its source kind, stable source and observation IDs, scope ID, member count,
  fixed evidence score, observation time and closed safety flags. It never exposes paths, filenames,
  OCR text or provenance strings, normalized version-name keys, evidence keys, content, embeddings,
  grants or model output.
- Every item reports current evidence, required future verification, review assistance only, and
  `cleanup_authorized: false`. The response reports `action_authorized: false`. The Inbox cannot
  create an organization plan or call a filesystem API.
- Refresh examines at most 20 recent sources. The response reports evaluated and not-current counts
  and whether evaluation was complete; it never presents a bounded or failed partial result as a
  complete Inbox. Ordering is deterministic by source kind, newest observation, then stable ID.
- No reclaimable-space total, keeper, preselection or cleanup recommendation is calculated. Source
  membership can overlap and the product has not established storage allocation or system-Trash
  behavior. A future UI may show source sizes only after defining non-misleading semantics.
- Ordinary logs contain only scope, counts and fixed states/codes. The four bundled Desktop locales
  use equivalent suggestion-only and no-file-change wording.

## Consequences

- The Inbox has one source of truth per evidence kind and cannot drift from a second candidate
  lifecycle. Refresh can be slower because exact pairs reread all bounded bytes; it is explicit and
  capped rather than a background poll that would grow observation history without bound.
- Stale, changed or inaccessible sources are omitted and counted as not current. An unavailable
  scope fails before any source path is resolved.
- This slice is useful for review and product integration, but does not satisfy confirmed cleanup.
  D-017 and M5 still require an immutable candidate/keeper/selection/evidence-bound ActionPlan,
  Preview, policy validation, durable per-file transaction receipts, platform Trash, crash recovery
  and Undo before any selected file can move.

## Rejected alternatives

- Persist a fourth mutable cleanup-candidate table that copies relation or group evidence.
- Treat accepted relation feedback as permission to clean a file.
- Trust historical relation summaries as current byte or filename evidence.
- Automatically refresh exact duplicates in the background.
- Return path-bearing source structs through CLI or Desktop IPC.
- Estimate guaranteed reclaimable bytes before keeper, overlap and storage-allocation semantics exist.
- Expose a disabled or partial Trash/Delete/Execute control before D-017 and M5 are complete.
