# Release Readiness

Last reviewed: 2026-07-17

Overall status: **not release-ready**. Local implementation is in M2 with parallel M3 lexical, M4 project-graph, M5 rename-preview, and M6 durable watch-core slices, while M0 remote CI and M1 cross-platform/memory/live-UI evidence remain open.

| Gate                                           | Status             | Evidence required                                        |
| ---------------------------------------------- | ------------------ | -------------------------------------------------------- |
| macOS Apple Silicon package                    | Not started        | Signed/notarized clean-machine install and smoke         |
| macOS Intel or Universal package               | Not started        | Native or Universal clean-machine evidence               |
| Windows x64 installer                          | Not started        | Signed clean-VM install and smoke                        |
| Linux experimental package                     | Not started        | Clearly labeled build and smoke                          |
| Explicit authorized scopes                     | Verified locally   | Desktop/CLI authorization, component-aware protected-tree policy, symlink/reparse and platform hidden/system exclusions; Windows runtime fixtures remain |
| Initial manifest scan                          | In progress        | Release 10k idempotency/timing, durable progress/pause/resume, crash-reopen replay, atomic publish, and Unix permission fixture pass; memory, live updated-UI smoke, and remote CI remain |
| Incremental watch mode                         | In progress        | Durable core debounce/stability/atomic reconcile/restart/rename fixtures and path-free CLI/Desktop status pass locally; native OS adapters, incremental extraction/indexing, placeholder handling, resource policy, Windows/runtime/live-UI evidence remain |
| Extraction and OCR formats                     | In progress        | Text/Markdown/code, bounded text-layer PDF, allowlisted DOCX/PPTX/XLSX, and bounded PNG/JPEG/GIF/WebP/BMP/TIFF header metadata route through durable jobs to atomic untrusted chunks or structured metadata. Corrupt/encrypted/active-content/archive/XML/image-signature/probe/dimension/output fixtures pass locally. Screenshot OCR, representative corpora, native remote runtimes, and 8 GB residency remain |
| zh-TW and English                              | In progress        | Built-in UTF-8 and DOCX fixtures extract mixed zh-TW/English with exact byte or paragraph provenance; PDF ToUnicode and spreadsheet fixtures cover both languages. OCR and representative retrieval/extraction evaluation sets remain |
| Metadata/FTS/vector/hybrid retrieval           | In progress        | Offline path/content FTS5, bounded scope/type/date/source filters, deterministic explanations, CLI/Desktop and synthetic 10k p50/p95/index-size baseline pass locally; project/folder filters, vectors, embeddings, hybrid fusion, real/100k/8 GB evaluation and cross-platform/live-UI evidence remain |
| Project/folder/related/duplicate/version graph | In progress        | Folder Profiles, durable correctable Project roots, explicit ≤64 MiB full-byte duplicate suggestions/corrections, and conservative explicit-numeric filename-version suggestions plus evidence-bound directional feedback pass locally with immutable evidence and no model, membership or action; general discovery, related/similarity, background duplicates, cross-pair learning, evaluation and Project UI remain |
| Smart Inbox and explainable classification     | Not started        | UI states and safe suggestion behavior                   |
| Rename/move preview                            | In progress        | Same-folder file rename CLI/Desktop preview, canonical scope/identity/open-handle/portable-name/conflict policy, before/after UI, nine explanations and path-free history pass locally; Move, folders, execution UI, fresh Windows/live-UI evidence remain |
| Journal, crash recovery, undo                  | In progress        | Plan plus `preview_created` event commit atomically and reject mutation/deletion; no execution journal states, fault injection, recovery, rollback, or Undo yet |
| Read-only MCP                                  | Not started        | Scope escape/injection tests and no write tools          |
| 8 GB benchmark                                 | In progress        | M1 timing/count baseline published; release-build peak RSS and documented 8 GB hardware remain |
| Updater pipeline                               | Not started        | Signed metadata dry run and rollback                     |
| SBOM and checksums                             | Not started        | Release-attached, independently verified artifacts       |
| GitHub Release                                 | Blocked externally | Remote, auth, verified assets, public download smoke     |
| README/demo/launch assets                      | In progress        | Pre-release README exists; demo and launch assets remain |
| Post-launch issue/hotfix process               | In progress        | Issue/PR templates exist; labels and incident/hotfix drill remain |
| Critical/high security findings                | In progress        | npm: zero known vulnerabilities; current 492-package RustSec scan has zero vulnerabilities plus 17 existing Tauri/Linux warnings identical to the prior lock; isolated PDF, Office, and image-metadata closures are clean. Upstream warning resolution, full threat model, notices, SBOM, and platform review remain |
| Known data-loss bugs                           | Unknown            | Full action/recovery suite; zero known data-loss issues  |

## M0 readiness gate

The M0 implementation is verified locally. Governance, ADRs, health slice, lockfiles, log-redaction checks, local format/lint/typecheck/test/build, isolated fresh-clone setup, CLI execution, debug app bundle, and live IPC UI smoke have passed. M0 itself remains **in progress** until a GitHub remote exists and the macOS/Windows/Linux matrix is green.

## M1 local readiness note

The resumable scanner is safe to exercise with test folders: progress and pending paths persist in SQLite, live manifest rows are not replaced until a complete scan publishes atomically, pause is acknowledged between entries, expired work reopens as interrupted, and resume revalidates the original canonical authorization boundary. This is not yet a release claim: Windows junction/hidden-attribute runtime evidence, peak RSS, remote CI, and live UI interaction evidence remain open.

## M2 extraction local readiness note

The text/Markdown/code, text-layer PDF, DOCX/PPTX/XLSX, and image-metadata slices are safe to exercise with test files already present in an explicit scanned scope. Providers never receive an arbitrary path; source size and actual open-handle identity are revalidated; reads/decompression/pages/archive/XML/chunks/image probe/dimensions/time are bounded. PDF active content/attachments and Office macros/formulas/relationships/embeddings remain inert; all text stays `untrusted_extracted_text`; image metadata decodes no pixels or EXIF/GPS and publishes separately from FTS. Structural output publishes atomically. CLI status, image metadata, and Desktop dashboard payloads contain no paths or text. This is not an M2 completion or release claim: Screenshot OCR, representative Office/PDF/image corpora, full Windows/macOS Intel/Linux runtime tests, live Desktop interaction, and 8 GB extraction benchmarks remain open.

## M3 lexical local readiness note

The FTS5 baseline is safe to exercise on test scopes: search is read-only, stays in bundled SQLite, accepts bounded quoted queries of at least three Unicode characters, caps candidates/results/snippets, excludes absent locations and stale chunks, and exposes fixed ranking explanations. Scope, match-source, extension, and modified-time filters are validated twice and echoed after normalization. User-invoked CLI/Desktop results intentionally return authorized paths and bounded untrusted snippets; ordinary logs omit query/path/text. A reproducible 10k synthetic macOS arm64 baseline records p50/p95/max and FTS bytes. This is not an M3 or release claim: graph-backed project/folder filters, vector/embedding providers, hybrid/semantic behavior, representative/100k/8 GB/RSS/thermal evaluation, short-query strategy, live Desktop interaction, and remote platform runtime remain open.

## M4 project-graph local readiness note

The Folder Profile and exact-root correction slices are safe to exercise on a test folder after a completed scan: they use only current explicit-scope locations, immutable evidence and append-only user decisions, with no model, membership edge or file action. The exact-duplicate slice is safe only as an explicit development check for two canonical current non-empty files in the same scope: it denies symlink aliases and same identities, caps each source at 64 MiB, compares every byte through read-only handles, and revalidates identity/metadata after reading. An exact-pair decision repeats that verification before appending immutable feedback. The filename-version slice is also explicit-only: both files pass the same scope/open-handle checks twice, names must share an NFC/lowercased base and extension with allowlisted numeric `vN` suffixes, and no file content or timestamp orders the versions. A version decision repeats live verification and applies only to equivalent directional evidence; changed direction returns to `suggested`. Explicit relation responses intentionally include current paths; structured logs and history omit paths, names, database location and content. This is not M4 or release completion: file membership, entities/topics, related/similarity and general version discovery, background duplicate discovery, larger-file hashing, cross-pair learning, merge/split, retrieval integration, Project UI, latency/RSS/8 GB and cross-platform evidence remain open.

## M6 watch-core local readiness note

The durable reconciliation core is safe to exercise with explicit CLI hints on test scopes: events are untrusted, scope/path/symlink policy is revalidated, temporary downloads are ignored, a stable read-only identity snapshot is required, and only the existing atomic scanner can publish live metadata. Status is path-free and restart fixtures pass. This is not automatic Watch Mode or M6 completion: no native event adapter, incremental extraction/indexing, placeholder detection, low-memory/background resource policy, notifications, Smart Inbox, Windows runtime, 8 GB benchmark, or live Desktop interaction has passed.

## M5 rename-preview local readiness note

The bounded rename preview is safe to exercise on test files after a completed scan: it performs no filesystem mutation, denies symlink/reparse and path-fallback sources, matches manifest metadata and platform/open-handle identity, validates a portable same-folder name, fails on occupied destinations, and commits an immutable plan plus first append-only event atomically. Explicit CLI/Desktop preview returns paths and passed checks; logs and list/history summaries do not. The Desktop explicitly offers no execute control. This is not M5 completion or a usable organizer: Move, folder actions, execution, source revalidation immediately before action, destination verification/hash, cross-volume handling, process-kill/permission/disconnect fault injection, recovery, rollback, idempotent Undo, execution/recovery/Undo UI, live interaction, and Windows runtime evidence remain open.
