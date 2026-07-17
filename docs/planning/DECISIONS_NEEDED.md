# Decisions Needed

Last reviewed: 2026-07-17

## Blocking now

D-013 must be resolved before M4 introduces cross-root learned scoring or merge/split semantics; it does not block deterministic related/duplicate/version work or safe work in other milestones. GitHub remote ownership blocks only remote CI/Issue/Release evidence.

## Open decisions

| ID    | Decision                                                    | Needed by                             | Options / constraints                                                                        | Default until decided                                                      |
| ----- | ----------------------------------------------------------- | ------------------------------------- | -------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------- |
| D-001 | GitHub owner and public repository location                 | M0 CI verification                    | Personal vs organization ownership; repository name and default branch protection            | Local Git only; do not invent a remote                                     |
| D-002 | Minimum supported macOS version and Intel strategy          | M9 design, benchmark fixtures earlier | Universal binary vs split arm64/x64; Tauri/WebKit support and clean machines determine floor | Build both architectures in CI design; no support claim yet                |
| D-003 | Windows installer format                                    | M9                                    | NSIS first vs MSI; signing and updater compatibility                                         | Evaluate Tauri-recommended NSIS and MSI; no decision from preference alone |
| D-004 | Opt-in telemetry and crash reporting                        | Before public beta                    | None, local-only export, or privacy-reviewed opt-in provider                                 | No telemetry and no network egress                                         |
| D-005 | Linux experimental artifact                                 | M9                                    | AppImage first, `.deb` optional; cannot delay macOS/Windows                                  | AppImage candidate, explicitly experimental                                |
| D-006 | Default scope presets and platform Screenshots discovery    | M1/M8                                 | Desktop/Downloads/Documents plus platform-aware screenshots; every path still user-confirmed | Show presets but grant nothing until explicit selection                    |
| D-007 | Vector adapter after SQLite manifest                        | M3                                    | SQLite binding is decided in ADR-010; evaluate vector API, license, multilingual behavior, and migrations | No vector dependency selected; deterministic lexical fallback required     |
| D-008 | OCR provider stack                                          | M2                                    | Native providers plus packaged cross-platform fallback; zh-TW + English                      | No Python/user-installed runtime; no provider selected                     |
| D-009 | Embedding model/runtime                                     | M3/M9                                 | Multilingual, int8, license, checksum, memory, model-removal support                         | Deterministic lexical retrieval remains required                           |
| D-010 | Product trademark/name clearance and reverse-DNS identifier | Before signed release                 | DeskGraph availability and legal review                                                      | Development identifier only; no trademark claim                            |
| D-011 | Exact Office ZIP/XML dependency feature set                  | M2 Office provider                    | Candidate `zip 8.6.0` no-default plus minimal stored/DEFLATE read support and `quick-xml 0.41.0` no-default must pass isolated closure, license, RustSec, API, and platform gates | Proposed ADR-014 only; do not add either dependency until the evidence gate passes |
| D-013 | Cross-root Project learning and merge/split semantics        | Later M4 scoring and correction slice | Whether/how accepted or rejected roots influence different candidates; merge/split identity and reversal events; explainable bounds and evaluation corpus | Feedback affects only the exact stable root; no cross-root score adjustment, merge, split, or automatic membership |

## Decisions made in M0

- Version B is the target; Version A is only an internal milestone.
- Project code license: Apache-2.0 (recorded in ADR-008).
- Rust workspace + Tauri 2 + React/TypeScript + pnpm follows the accepted architecture.
- Health data is a closed, privacy-safe schema shared by CLI and desktop; no paths, filenames, content, identifiers, or model output.

## Decisions made in M1

- Bundled SQLite through `rusqlite 0.40.1` is the manifest source of truth.
- File identity is separate from location: Unix device/inode and Windows volume serial/file index through an isolated `windows-sys` adapter.
- Unicode comparison keys use NFC; Windows additionally uses case-insensitive comparison keys. Canonical scope validation remains the security boundary.
- Initial scan is metadata-only and never follows symlinks or Windows reparse points.
- Resumable scans use a persistent path queue and job-scoped staging; only a completed job publishes the live manifest atomically. Pause state and an expiring runner lease survive process exit (ADR-011).

## Decisions made while entering M2

- Extractor providers receive only core-controlled bounded streams, never arbitrary paths; output remains untrusted and publishes per file only after complete success (ADR-012).
- Plain text, Markdown, and source code use a dependency-free built-in UTF-8 provider. Invalid encoding is isolated per file rather than silently guessed.
- Extraction jobs and content chunks are durable SQLite state. Failure or cancellation preserves the prior complete extraction; a changed source invalidates stale active chunks.
- Ordinary CLI/Desktop extraction status contains stable IDs, fixed codes, counts, and timings only. Paths and extracted text are excluded.
- D-008 remains open: the generic extractor contract does not preselect or claim an OCR stack.
- PDF text uses exact `lopdf 0.44.0` with default features disabled, strict bounded in-memory APIs, sequential page processing, no password handling, and no active-content traversal (ADR-013).
- Content-chunk provenance is tagged: source text uses byte ranges; PDF uses page and fragment indexes. Structural formats never receive fabricated byte offsets.
- ADR-014 proposes a shared, path-free, allowlisted OOXML ZIP/XML adapter. D-011 remains open because the exact minimal dependency closures could not be audited after local Cargo registry access was rejected by the exhausted tool quota.

## Decisions made while entering M3

- Bundled SQLite FTS5 `trigram` is the deterministic Traditional Chinese/English lexical substring baseline (ADR-015).
- Queries shorter than three Unicode characters fail closed; DeskGraph does not substitute an unindexed full-corpus scan.
- External-content indexes follow `locations` and `content_chunks`, while present/active source-of-truth joins determine current visibility.
- Search queries, paths, and snippets may appear only in an explicit user-requested result payload, never ordinary logs or extraction/status payloads.
- Lexical filters are a closed no-model contract: explicit authorized scope, metadata path vs active extracted text, one normalized ASCII-alphanumeric extension, inclusive `modified_since` and exclusive `modified_before` UTC Unix seconds. Applied normalized values are returned to the caller; project/folder filters wait for graph-backed identities.
- D-007 and D-009 remain open: no vector extension, embedding runtime, or model is selected by the lexical slice.

## Decisions made while entering M6

- Filesystem events are untrusted hints. Durable per-scope debounce and a bounded stability gate must complete before the existing atomic manifest scanner reconciles live state (ADR-016).
- The first core uses a one-second default stability window, closed temporary-download suffixes, size/modified-time/platform-identity snapshots, and read-only open-handle identity verification.
- Watch status is path-free outside explicit adapter/user input. Native OS adapters, incremental extraction/indexing, cloud-placeholder policy, background resource controls, and Smart Inbox remain unselected or unimplemented.

## Decisions made while entering M5

- An organization preview is an immutable core-owned record, not mutable frontend or LLM output. The first slice supports only a same-folder rename preview for a present scanned file (ADR-017).
- A preview requires canonical explicit scope containment, a strong manifest identity, matching size/modified time, a matching read-only open handle, a portable single-component target name, and a conflict-free destination.
- Plan plus sequence-1 `preview_created` event commit atomically; both tables reject update/delete. Explicit preview/status may return before/after paths, while logs and recent-plan summaries remain path-free.
- No executor, move, rollback, recovery, undo, or Desktop execution control may be added until their append-only state machine and fault-injection acceptance are defined and pass.

## Decisions made while entering M4

- Folder Profiles are read-only, on-demand derivations from current `present` manifest locations and are bounded to 100,000 descendants; overflow fails without a partial result (ADR-018).
- Deterministic direct-child markers may create an explainable Project Suggestion only. README is supporting provenance, not a sufficient strong marker; no heuristic creates `belongs_to` or automatic membership.
- Every suggestion carries basis-point confidence, provenance, completed-scan observation time, `system_rule` creator, fixed provider/version and `model_version: null`.
- Explicit Folder Profile responses may return the selected folder path; structured logs omit paths and member names.
- D-012 is resolved by ADR-019 for durable Project root candidates and exact-root correction. No embedding, vector runtime, model, API, or external dependency is selected by this slice.
- Project root identities, deterministic suggestion observations/signals, and user feedback events are immutable/append-only. Same-decision retries are idempotent; an opposite decision appends a correction.
- A rejected root remains rejected on later proposals with the same stable identity, and an accepted root remains accepted. This is exact-root feedback only and creates no file membership edge.
- ADR-020 adds only explicit same-scope exact-duplicate suggestions for non-empty files up to 64 MiB. Full byte equality, fixed provenance, immutable observations and current source revalidation are required; it adds no merge, deletion, hashing dependency, or file action.
- ADR-021 adds exact-pair feedback only: every explicit accept/reject repeats ADR-020 live verification, events are append-only, retries are idempotent, opposite decisions append corrections, and path-free history never claims current verification. It does not authorize file operations or influence other pairs.
- ADR-022 adds only explicit numeric filename-version suggestions. Same normalized base/extension plus allowlisted `vN` suffixes determine direction; timestamps, sizes, contents, `final`, and models do not.
- ADR-023 binds append-only version feedback to equivalent ordered-node/base/extension/version/provider evidence. Every decision repeats live verification; changed direction returns to `suggested`, and acceptance never authorizes a file action.
- D-013 remains open for cross-root learning and merge/split behavior; the default is no inferred influence beyond the root the user explicitly corrected.
