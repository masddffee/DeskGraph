# ADR-028: Explainable Screenshot Review Groups Are Suggest-only

- Status: Accepted
- Date: 2026-07-19

## Context

Smart Cleanup needs to help a user review batches of screenshots, but a screenshot group is not
evidence that any member is disposable. The existing file-relation schema is deliberately binary
and binds exact-duplicate or directional-version feedback to two stable file identities. Reusing
that schema for a changing, multi-member group would weaken its evidence and correction semantics.

M2 already records bounded image metadata and optional screenshot-OCR provenance. OCR text is
untrusted local content and must not become a grouping or logging surface. Dimensions, timestamps,
filenames, OCR confidence, or a model score also cannot independently prove screenshot origin,
content similarity, duplication, age, keeper choice, or safe reclamation.

## Decision

- Screenshot review groups use an independent append-only multi-member schema. A stable candidate
  identifies one exact ordered node set inside one authorized scope; each observation stores a
  complete immutable evidence snapshot. A membership change creates a different candidate.
- The v1 rule is `same_dimensions_time_window_with_ocr`. An eligible member must be a current,
  present regular file with matching active M2 image metadata, a completed explicit
  `screenshot_ocr` job, and matching active OCR provenance. The rule reads no image bytes, OCR
  text, filename, embedding, thumbnail, or model output.
- Eligible formats are the existing PNG, JPEG, and WebP metadata formats. Members have exactly the
  same width and height and their modification timestamps fall within ten minutes of the first
  member. The deterministic order is modification time then stable node ID.
- Confidence is fixed at 6000 basis points. Product output must state that the signal only helps
  review similarly sized, recently captured-or-edited images. It does not claim screenshot origin,
  content similarity, duplication, a keeper, disposability, or reclaimable bytes.
- Evaluation is bounded to 2,000 eligible images, 20 groups, and 20 members per group. Exceeding a
  bound fails the whole evaluation with a fixed path-free error; DeskGraph never persists a
  truncated group or partial evaluation result.
- Discovery and path-bearing status require a completed scan plus a matching active platform access
  grant. A missing, `needs_reauthorization`, revoked, platform-mismatched, or invalid grant fails
  closed. Path-free history remains readable but reports `current_evidence: false` when access is
  no longer active.
- Every persisted observation revalidates scope, current location, file size and modification
  time, active image metadata, completed OCR job, active OCR chunk count, and provider provenance.
  Source selection, grouping, revalidation and all writes share one immediate SQLite transaction;
  evidence-equivalent rediscovery is idempotent. Status repeats current-evidence validation.
  Changed or missing evidence makes the candidate `not_current`; it is never silently rewritten.
- Explicit `suggest` and `status` responses may return current member paths. Recent history is
  path-free, carries `verification_required: true`, and reports whether a complete immutable
  observation exists for the candidate's current evidence. Ordinary logs contain only scope,
  group, node, count, timing, and fixed codes.
- Each member is independently selectable in the future Inbox. This slice has no accept-for-cleanup
  decision, keeper recommendation, ActionPlan, filesystem call, Trash adapter, or Undo control.
  D-017 and the accepted M5 transaction boundary remain mandatory before any confirmed selection
  can be moved to system Trash.
- The rule is local, deterministic, dependency-free, and model-free. It adds no network surface.

## Consequences

- DeskGraph gains auditable screenshot-review assistance without weakening exact duplicate/version
  relations or implying that a grouped file is safe to remove.
- A source edit, move, metadata refresh, OCR refresh, scope change, or membership change cannot
  inherit stale evidence.
- Requiring existing OCR provenance is conservative and means the first slice will not discover
  every screenshot automatically. Broader screenshot-origin detection needs separately evaluated,
  explainable evidence and must preserve the same no-disposability boundary.
- Smart Cleanup, Trash execution, crash recovery, and Undo remain incomplete product work.

## Rejected alternatives

- Add a third kind to the binary `file_relation_candidates` table.
- Treat same dimensions, time proximity, filename patterns, OCR confidence, perceptual similarity,
  or an LLM result as proof of duplication or disposability.
- Read or compare OCR text, image pixels, paths, or filenames during v1 grouping.
- Persist only the current mutable member set or carry feedback across changed evidence.
- Truncate oversized groups or partially persist an evaluation.
- Generate an executable cleanup plan from a screenshot group.
