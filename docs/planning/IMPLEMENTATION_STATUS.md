# Implementation Status

Last updated: 2026-07-18

Status vocabulary: `not started`, `in progress`, `blocked`, `verified locally`, `verified in CI`, `released`.

## Current milestones

M2 Content Intelligence remains on the critical path; M3 Hybrid Retrieval, M4 Project Graph, M5 Safe Organization, and M6 Watch Mode each have a bounded first slice — all are **in progress**. M0 remains open for remote CI evidence, and M1 remains open for complete Windows runtime, peak-memory, and latest live-UI evidence.

Five M2 vertical slices now run end to end. Explicit already-scanned text, Markdown, source-code, text-layer PDF, DOCX, PPTX, XLSX, PNG, JPEG, GIF, WebP, BMP, and TIFF files are resolved inside their authorized scope; the core revalidates canonical scope, exclusion policy, manifest snapshot, and open-file identity; bounded providers receive only a controlled `Read + Seek` or owned validated encoded bytes; durable SQLite jobs support cancellation and interrupted recovery. Text publishes atomically as tagged byte/page/paragraph/slide/cell/OCR-observation provenance plus complete untrusted SQLite/FTS chunks. Images publish signature-checked format and encoded dimensions into a separate structured table, with no pixel decode, EXIF, GPS, filename, or path. The macOS arm64 Apple Vision provider requires `zh-Hant` and `en-US`, keeps normalized top-left boxes and confidence, and has a real local OCR→FTS smoke. Windows provider code is package-identity gated, requests `zh-TW`/`en-US`, validates the resolved recognizers as Traditional Chinese/English, stores mandatory boxes with optional confidence, rejects untransformable rotation, and uses terminal-only async close plus bounded single-worker cleanup. Host policy/state-machine tests and Windows cfg check/Clippy pass. M2 is not complete because real Windows/MSIX OCR/cancellation/cleanup, the D-015 packaged-fallback bake-off and implementation, representative corpora, full cross-platform runtime, actual Apple cancellation, and 8 GB extraction benchmarks remain open. Verified Tesseract model hashes do not constitute an accepted runtime; PP-OCRv6 is now an equal candidate rather than an assumed future upgrade.

One M3 lexical vertical slice now runs end to end. Transactional SQLite FTS5 trigram indexes cover current authorized display paths and active extracted chunks; normalized quoted queries are limited to 3–256 Unicode characters, results/candidates are capped, stale content is filtered by source-of-truth joins, and deterministic fusion exposes exact filename/path/content explanations. Scope, metadata-vs-content source, ASCII-alphanumeric extension, and inclusive/exclusive UTC modified-time filters are validated by both retrieval and database boundaries. CLI and Desktop return user-requested paths and bounded snippets while logs omit query/path/text. This is not M3 completion: one- and two-character search, project/folder filters, vector/embedding adapters, semantic and “files like this” queries, hybrid fusion, representative multilingual evaluation, 100k/8 GB/cross-platform benchmarks, and live-UI evidence remain open.

Six M4 vertical slices now run end to end. An explicit already-scanned folder resolves to bounded deterministic facts and explainable Project Suggestions. Stable Project roots and exact-byte duplicate pairs have immutable evidence plus append-only correction. Two different current files can also produce a directional filename-version suggestion only when both pass canonical scope, manifest/platform/open-handle identity, and metadata validation before and after analysis; their UTF-8 names must share an NFC/lowercased base and extension and end in allowlisted numeric `vN` suffixes. The lower number is older, the higher is newer, evidence is immutable at 9000 basis points, and no content/time/size/model guess is used. Version decisions repeat live verification, bind append-only feedback to equivalent ordered nodes/base/extension/version/provider evidence, make same decisions idempotent, retain opposite corrections, and return changed directional evidence to `suggested`. Migrations 0011–0012 preserve the unified relation history and add no synthetic decision. Explicit relation responses may return selected paths; structured logs and recent Project/relation summaries remain path-free and historical relation summaries require verification. No M4 slice performs a file action. This is not M4 completion: file membership, entities/topics, related/similarity/general-version discovery, background duplicates, cross-pair/root learning, clustering, merge/split, retrieval filters, Project page, cross-platform runtime, and scale/memory evaluation remain open.

One M5 preview vertical slice now runs end to end without performing a file action. A same-folder scanned-file rename is canonical-scope, manifest snapshot, platform identity, metadata, and read-only open-handle validated; a portable single-component target and conflict-free destination are required; an immutable plan and sequence-1 event commit atomically. Explicit CLI and Desktop preview returns before/after paths and nine passed policy checks, while logs and recent summaries are path-free; the UI explicitly has no execute control. This is not an organizer or completed transaction engine: Move/folders, execution, immediate pre-action revalidation, destination verification/hash, cross-volume handling, fault injection, startup recovery, rollback, idempotent Undo, Windows runtime, and live interaction remain open.

One M6 core vertical slice now runs end to end. Untrusted path hints are canonical-scope validated, temporary/hidden entries are ignored, events coalesce durably per scope, and unchanged existence/kind/size/modified-time/platform identity plus read-only open-handle identity are required before an atomically linked resumable scan reconciles the manifest. Stabilizing and reconciling events resume after database reopen; rename storms preserve identity locally. CLI and Desktop status are path-free. This is not automatic Watch Mode: native OS adapters, efficient per-node reconciliation, incremental extraction/indexing, cloud-placeholder handling, background pause/resource/low-memory policy, notification preferences, Smart Inbox states, Windows runtime evidence, and live Desktop interaction remain open.

## Milestones

| Milestone                     | Status      | Current evidence                                                           | Next gate                                             |
| ----------------------------- | ----------- | -------------------------------------------------------------------------- | ----------------------------------------------------- |
| M0 Repository Foundation      | In progress | Local foundation slice, governance, lockfiles, checks, CLI, and desktop smoke verified | Green macOS/Windows/Linux CI matrix |
| M1 Manifest Graph             | In progress | Explicit scope → durable bounded queue/staging → atomic SQLite manifest publish → CLI/desktop progress and controls; 10k, permission, recovery, protected-tree, and adversarial local tests | Peak RSS, latest live UI smoke, cross-platform runtime CI |
| M2 Content Intelligence       | In progress | Text/Markdown/code, bounded PDF/Office/image metadata, macOS Vision runtime, and Windows OCR code/cfg → durable operation-specific job → open-file identity revalidation → atomic untrusted chunks or structured metadata → CLI/Desktop status | Real Windows/MSIX/language/cancel/cleanup evidence; independently audited packaged fallback; representative corpora, remote runtimes, native-cancel evidence, 8 GB benchmarks |
| M3 Hybrid Retrieval           | In progress | Offline path/content FTS5 trigram → bounded retrieval and scope/type/date/source filters → deterministic explanations → CLI/Desktop search → synthetic 10k benchmark | Project/folder filters and extended benchmarks, then audited vector/embedding adapters, fusion, evaluation |
| M4 Project Graph              | In progress | Bounded Folder Profile → correctable stable Project root → explicit exact-byte duplicate plus feedback → explicit numeric filename-version suggestion plus evidence-bound directional feedback → immutable provenance/path-free history; no file membership, model or action | Related/general-version signals, background discovery, membership correction, cross-pair/root learning, evaluation, retrieval filters, Project page |
| M5 Safe Organization          | In progress | Same-folder scanned-file rename → double policy/identity/open-handle validation → immutable plan plus atomic first journal event → explicit CLI/Desktop before/after preview and path-free history; no executor | Append-only executor state machine, Move/cross-volume, fault injection, recovery/rollback/Undo and execution UI |
| M6 Watch Mode and Smart Inbox | In progress | Untrusted hint → durable per-scope debounce → stability/open-handle identity gate → atomically linked resumable manifest reconciliation → path-free CLI/Desktop status | Native adapters, incremental extraction/indexing, resource controls, Smart Inbox |
| M7 Read-only MCP              | Not started | ADR only                                                                   | Scoped stdio query slice                              |
| M8 Product UI                 | Not started | Planning only                                                              | M0 creates only the shell/status slice                |
| M9 Release Engineering        | Not started | Planning only                                                              | CI foundation, then packages/updater/SBOM             |
| M10 Launch                    | Not started | Copy templates only                                                        | Verified public release first                         |

## M0 acceptance checklist

| Acceptance criterion                                     | Status             | Evidence / blocker                                                         |
| -------------------------------------------------------- | ------------------ | -------------------------------------------------------------------------- |
| Monorepo established                                     | Verified locally   | Rust workspace, pnpm workspace, Tauri/React desktop, CLI, and both lockfiles |
| Rust format, lint, and tests configured                  | Verified locally   | Rust 1.97.0; current workspace format and Clippy pass; 180 tests pass       |
| TypeScript format, lint, typecheck, and tests configured | Verified locally   | Format, ESLint, TypeScript, 19 Vitest tests, and Vite build pass            |
| ADR template                                             | Verified locally   | `docs/architecture/adr/0000-template.md`                                   |
| Root and nested AGENTS instructions                      | Verified locally   | Root plus Desktop, scanner, extractor, transaction, and watcher safety instructions |
| Cross-platform CI matrix configured                      | Verified locally   | Pinned-action workflow covers macOS, Windows, and Linux                    |
| Cross-platform CI matrix passes                          | Blocked externally | No GitHub remote/auth or CI run yet                                        |
| Apache-2.0 license decision                              | Verified locally   | Root `LICENSE`, authoritative ADR-008, and package metadata                |
| Security policy                                          | Verified locally   | `SECURITY.md`; private reporting channel remains external                  |
| Contribution guide                                       | Verified locally   | `CONTRIBUTING.md` documents checks and safety-sensitive changes            |
| Code of conduct                                          | Verified locally   | Contributor Covenant in `CODE_OF_CONDUCT.md`                               |
| Issue and pull-request templates                         | Verified locally   | Structured privacy-aware templates under `.github/`                        |
| Changelog                                                | Verified locally   | `CHANGELOG.md` with honest Unreleased foundation scope                     |
| Structured privacy-safe logging                          | Verified locally   | Fixed-field JSON stderr logs; CLI redaction assertions and live CLI/desktop events |
| Architecture skeleton                                    | Verified locally   | Rust domain/telemetry/CLI/Tauri shell plus ADR directory                   |
| Fresh clone instructions work                            | Verified locally   | Isolated `/private/tmp` clone passed frozen install, 9 Rust tests, and complete `pnpm check` |
| Desktop app opens                                        | Verified locally   | Debug `.app` bundled and opened; AX/screenshot shows Rust-backed `Foundation is connected` state |
| CLI health works                                         | Verified locally   | `cargo run -p deskgraph-cli -- health` returns privacy-safe JSON with status `ok` |
| No model/API key required                                | Verified locally   | CLI and desktop succeed with OCR, embeddings, and local LLM explicitly disabled |
| README labels pre-release                                | Verified locally   | First line states pre-release and not ready for personal file indexing     |

## Unresolved blockers

- No GitHub repository/remote and invalid GitHub authentication: remote Issues, Releases, and CI results do not exist.
- Signing, notarization, clean Windows/macOS VM validation, and launch accounts are external later-stage requirements.
- The current 493-package lock was scanned against 1,160 cached RustSec advisories: zero known vulnerabilities, 16 unmaintained warnings, and one `glib` unsound warning. The 17 warnings exactly match the prior 492-package lock and trace to existing Tauri/Linux transitive paths; the target-specific one-package `objc2-vision` delta adds no advisory. R-010/R-016 remain open for upstream replacement, notices, SBOM, and release-platform review rather than a missing scan.
- Windows open-handle identity and OCR Rust cfg adapters compile for `x86_64-pc-windows-msvc`; the OCR check/Clippy uses a host-pkg-config bypass only to typecheck Rust. A normal complete scanner/extractor check cannot be produced on this macOS host because bundled SQLite needs a Windows C/MSVC toolchain. No Windows link/runtime is claimed; remote Windows CI remains required.
- Local 10k timing and idempotency are measured, but peak RSS sampling was denied by the restricted runner and its escalation reviewer was unavailable due tool quota. This does not block code work; the 8 GB release gate remains open.

## M1 acceptance checklist

| Acceptance criterion | Status | Evidence / blocker |
| --- | --- | --- |
| Explicit persisted scope configuration | Verified locally | CLI and desktop require an existing user-entered directory; authorization never triggers a scan |
| Canonical path and system-root policy | Verified locally | Canonical authorization/rescan revalidation, scope containment on every observation, component-aware protected-tree and broad container-root denial |
| Symlink/junction/reparse loop defense | Verified locally on Unix; Windows runtime pending | `symlink_metadata` before canonicalization; no following; loop fixture passes; Windows adapter checks reparse attribute |
| Hidden/default-sensitive exclusions | Verified locally on macOS; Windows runtime pending | Dot entries plus macOS hidden flags and Windows hidden/system attributes are skipped; Windows boundary cross-compiles; preset UX remains M8 work |
| SQLite migrations documented and safe | Verified locally | Embedded migration, checksum mismatch rejection, WAL, foreign keys, FULL synchronous, reopen test |
| File/Folder nodes and `located_in` relations | Verified locally | Transactional observation upsert and active relation reconciliation |
| Rescan idempotency | Verified locally | Unit fixture and three 10k scans retain exactly 10,101 active nodes/locations |
| Move preserves identity where metadata permits | Verified locally on Unix | Rename and hard-link fixture retains node ID; Windows runtime fixture pending |
| Permission failures are recorded | Verified locally on Unix; Windows runtime pending | Restricted-directory fixture records `permission_denied` without failing the scan; issue remains staged until atomic publish |
| Persistent progress/pause/resume | Verified locally | Durable path queue, job-scoped staging, lease recovery, pause handshake, scope revalidation, replay after database reopen, and atomic publish tests pass |
| CLI graph statistics | Verified locally | `scan create/run/advance/status/list/pause/resume` expose validated progress without logging paths; foreground `scan start` remains available |
| Desktop graph statistics and usable scope UI | Verified locally except latest live smoke | Narrow Rust IPC, validated TypeScript contracts, backend-derived progress polling, paused/interrupted states, production build; live window smoke for the new controls blocked by local tool quota |
| Synthetic 10k generator and benchmark | Verified locally for timing/counts | Optimized durable scan: 4.489 s active / 4.84 s wall initial, 4.217 s active / 5.02 s wall rescan; 10,101 active nodes remain idempotent; peak RSS pending |

## M2 acceptance checklist

| Acceptance criterion | Status | Evidence / blocker |
| --- | --- | --- |
| Provider boundary receives no arbitrary path capability | Verified locally | Accepted ADR-012/ADR-024; document providers receive controlled `Read + Seek`, while `OcrProvider` receives only validated bounded encoded bytes, dimensions, limits and control; neither receives a path, URL, network client or process capability |
| Text, Markdown, and source-code extraction | Verified locally | Dependency-free UTF-8 provider, explicit extension routing, BOM/offset handling, zh-TW + English fixture, invalid-encoding isolation |
| Durable extraction jobs and cancellation | Verified locally at shared boundary; native runtime pending | Operation-tagged states, durable cancel request, bounded polling, OCR atomic cancellation monitor, lease recovery. Windows state-machine tests require Cancel before drain, terminal-only Close, post-result cancellation check, bounded caller return, and one cleanup worker; actual Vision/WinRT cancellation and cleanup remain native-platform gates |
| Scope, exclusion, identity, and TOCTOU revalidation | Verified locally on Unix; Windows runtime pending | Canonical root/source containment, hidden/symlink/reparse denial, creation-time manifest snapshot, actual open-handle identity before and after extraction; symlink-swap fixture passes |
| Bounded resource policy | Verified locally for shared/provider logic; native RSS pending | Defaults: 4 MiB general source, 8 MiB decompression unit/stored output, 512 PDF pages, 2,048 chunks, 5 seconds; generic absolute caps: 64 MiB source/decompression/output, 4,096 pages, 65,536 chunks, 64 KiB/chunk, 60 seconds. Office/image have structural/probe caps. OCR is tighter at 32 MiB source, 16,384 per dimension, 64 Mi pixels, 8 MiB output, 4,096 observations, 256 KiB per observation, and 60-second caller deadline. Windows permits at most one owned cleanup worker after timeout; actual cleanup duration and RSS remain runtime gates. |
| Atomic extraction publication and prior-version safety | Verified locally | Per-file transaction deactivates prior chunks or structured image metadata only after the complete replacement validates; failure/cancellation preserves the prior complete version; source changes invalidate stale outputs |
| Provenance, offsets, and untrusted classification | Verified locally for byte, PDF, Office, structured image metadata, and OCR | Migrations 0013–0015 preserve prior rows/FTS and add structural image/OCR provenance. Migration 0016 keeps normalized boxes mandatory while confidence is nullable. Windows stores `NULL` because `OcrWord` has no score and publishes source-aligned boxes only for absent/zero `TextAngle`; OCR remains `untrusted_extracted_text` with no fabricated byte offset. |
| Per-file error isolation | Verified locally at shared boundary; Windows runtime pending | Invalid UTF-8, corrupt/encrypted PDF, unsafe Office, malformed/mismatched images, unsupported OCR formats, resource limits, provider failure, cancellation, changed source, invalid boxes/confidence/angle and language policy produce fixed errors without partial publication. Real Windows decoder/identity/language/async failures still need native fixtures |
| Privacy-safe usable CLI | Verified locally | `extract start/create/ocr-start/ocr-create/run/status/list/cancel/resume/stats/image-metadata`; explicit `--path` resolves only an existing scanned node; binary tests prove path, filename, database path, extracted/OCR text do not enter ordinary job or metadata stdout/stderr |
| Desktop extraction status | Verified locally except live smoke | Narrow read-only Tauri IPC, runtime-validated TypeScript schemas, operation-aware Screenshot OCR label, empty/success/failure/cancel/interrupted states, 19 frontend tests, Vite and Tauri release builds; no Desktop OCR start control or latest window interaction is claimed |
| PDF text | Verified locally; platform/memory evidence pending | Strict bounded `lopdf 0.44.0` provider routes through manifest identity and atomic SQLite publication. zh-TW/English, corrupt, encrypted, JavaScript/Launch/URI/attachment inertness, decompression/page/output/cancel, page provenance, and service routing fixtures pass. Aggregate parser residency, representative corpus quality, and remote macOS Intel/Windows/Linux runtime remain open. |
| DOCX / PPTX / XLSX | Verified locally; platform/corpus/memory evidence pending | Accepted ADR-014's exact no-default `zip 8.6.0`/`quick-xml 0.41.0` provider allowlists only document/slides/worksheets/shared strings in memory; rejects unsafe, duplicate, encrypted, overlapping, unsupported, bomb-like, malformed, and over-limit input; ignores macros/formulas/relationships/embeddings; writes paragraph/slide/cell provenance atomically; and passes Manifest→SQLite→FTS integration. Remote runtimes, representative real Office corpora, latest live UI, and 8 GB residency remain open. |
| Image metadata | Verified locally; platform/corpus/memory evidence pending | Exact no-default `imagesize 0.15.0` with PNG/JPEG/GIF/WebP/BMP/TIFF only; controlled bounded header probe, extension/signature match, strict container checks, dimension/pixel caps, no pixel/EXIF/GPS decode, migration preservation, atomic Manifest→SQLite metadata, stale-source invalidation, fixed failure codes, and path-free CLI pass. Native remote runtimes, representative corpus, latest live UI, and 8 GB residency remain open. |
| Screenshot OCR with Traditional Chinese and English | macOS arm64 runtime verified locally; Windows provider code/cfg verified; M2 in progress | Exact `objc2-vision 0.3.2` routes bounded bytes through spatial/confidence provenance to atomic SQLite/FTS and has a real mixed-language smoke. Reused Microsoft `windows 0.61.3` provider code covers identity preflight, owned bytes, requested/resolved language policy, nullable confidence, strict angle/boxes, deterministic de-duplication, terminal-close lifecycle, bounded caller return and single cleanup worker. Host tests and Windows cfg check/Clippy pass; real Windows/MSIX/language/OCR/cancel/cleanup/RSS, fallback, corpus/8 GB and release evidence remain open. |

## Next handoff

Continue `prompts/03_EXTRACTORS_OCR.md` from the macOS runtime slice and Windows code/cfg slice. Run the exact real Windows/MSIX identity, requested/resolved language, mixed/no-text/corrupt/limit, `TextAngle`, native cancellation/cleanup/single-worker recovery, atomic FTS and RSS fixtures listed in `EXTERNAL_ACTIONS_REQUIRED.md`. In parallel, run D-015's same-corpus Tesseract versus PP-OCRv6 bake-off before accepting or implementing a fallback runtime; no fallback behavior is currently claimed and no dormant router should be added. Add representative Office/PDF/image/OCR corpora and 8 GB evidence without relabeling compile fixtures as runtime completion. Continue `prompts/04_HYBRID_SEARCH.md` from the verified lexical slice: add project/folder filters only after their graph source of truth exists, then establish a versioned SQLite embedding/exact-search baseline and 100k/representative/RSS/remote evidence before selecting a disposable ANN accelerator. Keep Windows scanner fixtures, latest Desktop smoke, and remote CI as parallel evidence workstreams.

For `prompts/05_PROJECT_GRAPH.md`, continue from ADR-018 through ADR-023's Folder Profile, exact-root correction, bounded exact-duplicate observation/feedback, explicit numeric filename-version suggestion, and evidence-bound directional correction without converting any suggestion or acceptance into automatic membership or action. Next add deterministic related candidates with provenance/current-data invalidation. Keep background duplicate discovery/larger-file hashing separate, resolve D-013 before cross-pair/root learned scoring and D-016 before merge/split, and require membership correction/evaluation before Project/folder retrieval filters or a Project page can claim source-of-truth behavior.

For `prompts/07_WATCH_SMART_INBOX.md`, continue from ADR-016 by auditing/selecting native event adapters per platform or implementing an explicitly documented dependency-free polling adapter; then connect missed-event reconciliation and incremental extraction/indexing without bypassing the stability gate. Keep Smart Inbox, generated rules, and file actions outside that adapter.

For `prompts/06_SAFE_ORGANIZER.md`, continue from ADR-017 by designing the append-only execution/recovery/undo state machine before adding any filesystem operation. Implement immediate pre-action identity checks, destination verification, Move/cross-volume copy-verify-commit and fault injection before exposing Desktop execution control. The current preview is deliberately non-executable.

## M4 project-graph acceptance checklist

| Acceptance criterion | Status | Evidence / blocker |
| --- | --- | --- |
| Current authorized folder identity | Verified locally | CLI path is canonicalized only to resolve an existing `(scope_id, folder_node_id)`; database accepts only a present folder location in that scope |
| Bounded Folder Profile | Verified locally | Streams current descendant locations with a hard 100,000-entry cap and one-row overflow detection; limit violations return no partial profile |
| Deterministic semantic facts | Verified locally | Fixed categories report direct/descendant counts, total bytes and latest modification time without a model or filesystem retraversal |
| Explainable Project Suggestion | Verified locally | Direct strong markers produce basis-point confidence plus all marker provenance, completed-scan observation time, `system_rule`, fixed provider/version and `model_version: null`; README alone is insufficient |
| Low-confidence behavior | Verified locally for root candidates | Every heuristic begins `suggested`; only explicit user action can accept/reject it; no `belongs_to` file edge, automatic membership, move, rename, or LLM action exists |
| Privacy-safe usable CLI | Verified locally | `folder profile`, `project propose/decide/status/list`, `relation duplicate/verify/decide/list/version/version-verify/version-decide`; explicit profile/root and relation-detail responses may return selected paths, while structured stderr plus Project/relation lists omit paths, filenames, database paths and content |
| No new external dependency | Verified locally | Local `deskgraph-projects` reuses database/domain/scanner/identity plus fixed-size standard-library reads; no registry hash/graph package, model, embedding runtime, API, or network client added |
| Persisted Project root candidates | Verified locally | Migration 0008 keys roots by stable scope/folder identity and stores immutable rule observations plus normalized signals; database re-derives current profile facts and rejects invented/stale evidence |
| Append-only user accept/reject correction | Verified locally for exact root | Explicit user events are append-only; same-decision retry is idempotent; opposite decision appends the next sequence; rejected roots remain rejected on later proposals and can be corrected to accepted |
| File membership edges | Not started | Accepted root does not imply descendant membership; `belongs_to` candidate/confirmation/current-data invalidation contracts and fixtures required |
| Exact duplicate relation candidates | Verified locally for explicit pairs ≤64 MiB | Migration 0009 persists stable ordered endpoints and append-only immutable full-byte observations; canonical scope, symlink/hard-link, manifest/open-handle identity, content, limit, stale-source and live verify fixtures pass; a relation begins `suggested` and only explicit user feedback can change its state |
| Append-only exact-pair correction | Verified locally | Migration 0010 stores immutable user events; decide repeats live byte verification, same-decision retry is idempotent, opposite decisions append sequence corrections, later observations preserve the state, history is path-free/`verification_required`, and no decision creates a file action |
| Explicit numeric filename-version candidates | Verified locally | ADR-022/migration 0011 share one NFC/lowercase parser across filesystem/database boundaries; allowlisted suffix, base/extension/direction, identity/current snapshot, stale/ambiguous input, migration preservation, immutable observation and privacy fixtures pass; every new evidence tuple begins `suggested` |
| Related, similarity and general version relations | Not started | Deterministic/embedding signals, provenance, evaluation corpora, correction and current-data invalidation required; explicit numeric suffixes do not cover fuzzy similarity, dates, `final`, or semantic revisions |
| Evidence-bound version correction | Verified locally | ADR-023/migration 0012 bind immutable user events to version observations and apply them only to equivalent ordered-node/base/extension/version/provider evidence; decide repeats live verification, retries are idempotent, opposite corrections append, changed direction returns `suggested`, restored equivalent evidence recovers its latest decision, history/logs are path-free, and no file action exists |
| Background duplicate discovery and larger files | Not started | Requires a bounded scheduler, current-data index and separately audited strong-hash design; explicit 64 MiB pair checks are not release-scale discovery |
| Cross-root learned scoring | Not started | D-013 open; versioned features/model/calibration, bounded influence, poisoning/reset/rollback and evaluation evidence required; exact-root feedback is not general learned scoring |
| Project merge/split | Not started | D-016 open; stable identity, membership provenance, reversible events, conflict/current-data behavior and retrieval invalidation required independently of ML |
| Project overview page | Not started | Must wait for backend source-of-truth identities, corrections, loading/empty/error/accessibility states and live interaction evidence |
| Scale and platform evidence | In progress | Small local macOS fixtures and sparse over-limit denial pass; duplicate latency/RSS corpora, 100k/8 GB and macOS Intel/Windows/Linux runtime remain open |

## M5 rename-preview acceptance checklist

| Acceptance criterion | Status | Evidence / blocker |
| --- | --- | --- |
| Immutable core-owned ActionPlan | Verified locally for rename preview | Versioned closed operation/state/strategy/policy contracts; plan rows reject update/delete |
| Explicit authorized scope and manifest source | Verified locally on macOS | Absolute canonical source must be a present scanned file inside the current canonical scope; outside, missing and symlink/reparse sources fail closed; Windows runtime pending |
| Strong identity, metadata and open-handle validation | Verified locally on macOS | Path-fallback identity is denied; platform identity, size and modified time match manifest and a read-only handle before journaling; immediate execution-time check remains future work |
| Portable target-name policy | Verified locally | One Unicode component, 255 UTF-8-byte cap, traversal/control/Windows-invalid/device/trailing-space-dot denial |
| Destination conflict and case-only planning | Verified locally on current filesystem | Existing unrelated file/folder/symlink fails closed; an ASCII case-only alias to the same identity records `case_only_staged`; broader platform/Unicode fixtures pending |
| Durable preview journal | Verified locally | Migration 0007 atomically commits immutable plan plus sequence-1 `preview_created`; both tables reject update/delete; reopen fixture passes |
| Privacy-safe usable CLI | Verified locally | `organize rename-preview/status/list`; explicit preview/status contains before/after paths, logs omit paths/names, list summaries are path-free |
| No filesystem action path | Verified locally for this crate/CLI | No production rename/move/copy/delete call or execute subcommand exists; source and destination remain unchanged in integration tests |
| Move, folder and cross-volume plans | Not started | Immutable operation schemas, identity/copy/hash/space/removable-drive policy and fixtures required |
| Executor and destination verification | Not started | Append-only journal-first state machine, immediate source check, destination identity/hash verification and no-overwrite behavior required |
| Crash recovery, rollback and idempotent Undo | Not started | Startup recovery matrix and process-kill/partial-copy/disconnect tests required before any execution UI |
| Desktop preview and path-free history UI | Verified locally except live smoke | Narrow create/list IPC, strict TypeScript schemas, scope/source/name form, before/after paths, nine explanations, loading/empty/error/success states, explicit no-execute copy; production build passes |
| Desktop execution/recovery/Undo UI | Not started | Must wait for the proven executor/recovery/Undo backend and all conflict/recovery states; no execution control may be enabled early |

## M6 watch-core acceptance checklist

| Acceptance criterion | Status | Evidence / blocker |
| --- | --- | --- |
| Watch event source abstraction | Verified locally for core contract | Public `WatchEventSource` yields untrusted scope/path hints; no native adapter is selected or claimed |
| Durable debounce and event storms | Verified locally | Migration 0006 allows one stabilizing event per scope; repeated/rename hints coalesce and reset the bounded deadline |
| Stability and read-only identity gate | Verified locally on macOS | Existence/kind/size/mtime/platform identity must remain unchanged; file open-handle identity must match before reconcile; Windows runtime pending |
| Temporary and hidden input handling | Verified locally | `.part`, `.crdownload`, `.download`, dot-hidden and platform-hidden paths do not start reconciliation |
| Atomic manifest reconciliation | Verified locally | Watch event and normal resumable scan job link in one transaction; live manifest still publishes only after a complete scan |
| Restart recovery | Verified locally | Both stabilizing and atomically linked reconciling events survive reopen and reach completion; failed scanner state remains fixed-code |
| Rename storm identity | Verified locally on Unix | Old/new hints coalesce; full reconciliation preserves node identity and removes the stale location without duplicates |
| Path-free CLI and Desktop state | Verified locally except live UI smoke | `watch observe/advance/status/list`, narrow read-only Tauri list command, strict TS parser, honest adapter-pending panel; binary/Rust tests exclude paths and content |
| Native macOS/Windows/Linux adapters | Not started | Dependency/API/platform audit and missed-event fixtures required; manual CLI hint is not automatic Watch Mode |
| Incremental extraction/indexing | Not started | Manifest reconcile invalidates stale content safely but does not enqueue re-extraction or embeddings |
| Cloud placeholders, low-memory/background controls | Not started | Provider/platform detection, pause/resume, battery/thermal and 8 GB evidence required |
| Smart Inbox, notifications, generated rules | Not started | Suggest-only state model and UI belong to later M6 slices; no file action is permitted here |

## M3 acceptance checklist

| Acceptance criterion | Status | Evidence / blocker |
| --- | --- | --- |
| SQLite FTS5 indexing | Verified locally | Migration 0005 creates external-content trigram indexes, synchronization triggers, and transactional rebuilds for existing locations/chunks on the bundled SQLite build |
| Deterministic no-model fallback | Verified locally | Accepted ADR-015; lexical path has no model/API/network/new registry dependency and reports `embeddings_enabled: false` |
| Current-data safety | Verified locally | Metadata joins `present` locations; content joins `active` chunks and `present` locations; manifest-change fixture proves stale text cannot surface |
| Bounded query and result policy | Verified locally | 3–256 Unicode characters, no non-whitespace controls, quoted FTS phrase, bound SQL parameters, 50 results and 100 candidates per source/200 total maximum; short queries fail closed |
| Traditional Chinese and English lexical search | Verified locally for substring baseline | Mixed path/content fixtures and Desktop helper pass; complete multilingual relevance set remains open |
| Metadata and FTS search | Verified locally for path/content substring plus bounded scope/type/date/source filters | Exact filename boost, path/content fusion, snippets, scope, metadata-vs-content source, normalized extension and inclusive/exclusive modified-time filters, fixed explanations; project/folder filters await their graph model |
| CLI search | Verified locally | Explicit `search --database --query [--scope] [--source] [--extension] [--modified-since] [--modified-before] [--limit]`; normalized filters are echoed in the response while binary tests prove stderr logs omit query/path/text |
| Desktop search UI | Verified locally except live smoke | Narrow read-only Tauri command with one structured filter contract, strict TypeScript parser, query/scope/source/type/date form, loading/empty/error/results, applied-filter summary, visible explanations and untrusted-text label; Vite/Tauri release builds pass |
| Vector semantic search and embedding cache | Not started | D-007/D-009 open; versioned SQLite rows/model manifest are the rebuild source, exact search is the first measured baseline, and an ANN accelerator requires dependency, license, checksum, recovery, memory, update and multilingual evidence |
| Hybrid fusion and “files like this” | Not started | Lexical fusion is not vector/lexical hybrid; semantic and recent-project-context acceptance remain open |
| Search p50/p95 and index-size evidence | Verified locally for synthetic 10k baseline | Current filtered-SQL release run: zh-TW p50/p95 5.679/5.931 ms; English 11.392/16.119 ms; exact filename 2.121/2.249 ms; miss 0.082/0.090 ms; 11,993,088-byte DB and 5,181,440-byte FTS indexes. One macOS arm64 run only; 100k, real corpus, 8 GB/RSS and remote platforms remain open. |

## Verification evidence — 2026-07-16

- `cargo fmt --all -- --check` — passed.
- `cargo clippy --workspace --all-targets --all-features -- -D warnings` — passed.
- `cargo test --workspace --all-features` — 9 passed, 0 failed.
- `pnpm peers check` — no peer dependency issues.
- `pnpm check` — format, lint, typecheck, 4 tests, and production web build passed.
- `cargo run -p deskgraph-cli -- health` — status `ok`; database `not_initialized`; all optional providers `disabled`; no location fields.
- `pnpm --filter @deskgraph/desktop tauri build --debug --bundles app` — produced and launched `DeskGraph.app` on macOS arm64.
- Desktop accessibility and screenshot smoke — showed `Foundation is connected`, zero scopes, no network required, and no filesystem locations.
- `pnpm audit` and `pnpm audit --prod` — zero known vulnerabilities.
- `cargo audit` — zero known vulnerabilities and 17 warnings; warnings remain open, not suppressed.
- Isolated clean clone — `pnpm install --frozen-lockfile`, 9 Rust tests, and complete `pnpm check` passed without relying on the working tree's build outputs.

## M1 verification evidence — 2026-07-16

- `cargo fmt --all -- --check` — passed.
- `cargo clippy --workspace --all-targets --all-features --offline -- -D warnings` — passed.
- `cargo test --workspace --all-features --offline` — 25 passed, 0 failed, including the fixture-generator contract.
- `cargo check -p deskgraph-identity --target x86_64-pc-windows-msvc --all-features --offline` — passed for the isolated Windows file-identity boundary.
- Complete scanner Windows cross-check — blocked locally by missing Windows C/MSVC headers required to compile bundled SQLite; must run in Windows CI.
- `pnpm check` — format, lint, typecheck, 7 Vitest tests, and production web build passed.
- `pnpm --filter @deskgraph/desktop tauri build --debug --bundles app` — produced `DeskGraph.app` with bundled SQLite.
- Live desktop accessibility and screenshot smoke — manifest ready, zero auto-authorized scopes, explicit path field, separate authorize/scan actions, graph metrics visible.
- CLI end-to-end smoke — initialize manifest, authorize explicit temp scope, scan, and stats all returned validated JSON without path fields in logs.
- 10k pre-durable benchmark — superseded by the durable release benchmark below; counts remain a historical idempotency cross-check, but its timing must not be used for current performance claims.
- `cargo audit --no-fetch` — zero known vulnerabilities and the existing 17 tracked warnings.

## M1 durable-scan verification evidence — 2026-07-16

- `cargo fmt --all -- --check` — passed.
- `cargo clippy --workspace --all-targets --all-features --offline -- -D warnings` — passed.
- `cargo test --workspace --all-features --offline` — 38 passed, 0 failed.
- Durable database tests — active pause request is acknowledged between entries; expired `processing` work becomes `interrupted` on database reopen and replays after explicit resume; staged observations remain invisible until one final transaction.
- Scanner tests — bounded batch progress, pause without partial publish, resume to completion, pre-resume scope revalidation, and Unix `permission_denied` isolation pass.
- `pnpm --dir apps/desktop test` — 7 passed, 0 failed; all scan job states and narrow IPC argument contracts validated.
- `pnpm --dir apps/desktop lint`, `typecheck`, and `build` — passed.
- `pnpm tauri build --no-bundle` with the pinned Cargo path — release build passed and produced `target/release/deskgraph-desktop`.
- Live Tauri smoke for the new paused/resume controls — not run; Computer Use launch approval was rejected because the local tool quota was exhausted. The prior M1 manifest UI live smoke remains valid only for the older scope/scan/statistics screen.
- Platform exclusion hardening — `/System/...`, `/usr/...`, and other protected descendants are denied; broad container roots such as `/Users` require a more specific child; a real Finder hidden-flag fixture and filesystem case-behavior fixture pass on macOS; Windows hidden/system attributes are implemented and the boundary cross-compiles, with runtime fixtures still pending.
- Optimized durable 10k benchmark — initial 4.489 s active / 4.84 s wall; rescan 4.217 s active / 5.02 s wall; 10,000 files, 101 folders, 0 issues, and 10,101 active nodes after repeated scans.
- Peak RSS attempt — `/usr/bin/time -l` could execute the scan, but the sandbox denied `sysctl kern.clockrate` and emitted no maximum-resident-set field. The 8 GB gate remains open.

## M2 text-extraction vertical-slice evidence — 2026-07-16

- `cargo fmt --all -- --check` — passed.
- `cargo test --workspace --all-features --offline` — 63 passed, 0 failed after the complete CLI/Desktop slice.
- `cargo clippy --workspace --all-targets --all-features --offline -- -D warnings` — passed.
- Extractor tests — 18 passed: explicit routing, zh-TW/English byte offsets, BOM provenance, source/output/chunk/time caps, overlap accounting, invalid UTF-8, cancellation, source change, read errors, symlink swap, prior-version preservation, and atomic service flow.
- Database tests — 12 passed: migration checksum, durable cancellation, atomic valid replacement, rejection of invalid trust/size declarations, stale-content invalidation, expired-runner interruption, and explicit resume.
- CLI tests — 6 unit + 3 binary integration tests passed; explicit path extraction completed and neither the private path, filename, nor extracted text appeared in stdout/stderr.
- `cargo check -p deskgraph-identity --target x86_64-pc-windows-msvc --all-features --offline` — passed for open-handle identity code.
- Complete extractor Windows cross-check — blocked locally because bundled `libsqlite3-sys` requires Windows MSVC C headers/toolchain unavailable on this macOS host; Windows CI remains required.
- `pnpm check` — Prettier, ESLint, TypeScript, 10 Vitest tests, and Vite production build passed.
- `pnpm --filter @deskgraph/desktop tauri build --no-bundle` with the pinned Cargo path — release build passed and produced `target/release/deskgraph-desktop`.
- `cargo audit --no-fetch` — 1,160 cached advisories, 457 lockfile packages, zero known vulnerabilities, and the same 17 tracked warnings under R-016.
- Latest extraction dashboard live-window smoke — not run; the earlier Computer Use launch approval was rejected because the local tool quota was exhausted. No visual/runtime claim is inferred from the successful production build.
- Dependency delta — no new external parser, OCR, model, Python, Docker, or network dependency; M2 reuses audited workspace SQLite/identity/test dependencies and the Rust standard library.

## M2 bounded-PDF vertical-slice evidence — 2026-07-16

- `cargo fmt --all -- --check` — passed.
- `cargo test --workspace --all-features --offline --quiet` — 73 passed, 0 failed: CLI 6 + 3 integration, database 14, Desktop Rust 4, domain 4, extractors 26, identity 2, scanner 12, telemetry 2.
- `cargo clippy --workspace --all-targets --all-features --offline -- -D warnings` — passed.
- PDF/extractor tests — 26 passed total. PDF fixtures cover Traditional Chinese and English ToUnicode text, exact page/fragment provenance, corrupt input, empty-password encryption rejection, inert JavaScript/Launch/URI/attachments, compressed-page limits, page/output caps, cancellation between pages, and manifest-to-provider routing.
- Provenance/database tests — 14 passed. Migration v3→v4 preserves exact byte ranges; PDF rows require page/fragment and store no fake byte offsets; publication remains atomic.
- `pnpm check` — Prettier, ESLint, TypeScript, 10 Vitest tests, and Vite production build passed.
- `pnpm --filter @deskgraph/desktop tauri build --no-bundle` — passed and produced `target/release/deskgraph-desktop` with the PDF provider.
- Minimal dependency cross-platform evidence — exact `lopdf 0.44.0`, default features disabled, has no Rayon/crossbeam entry in DeskGraph's tree; isolated macOS arm64 test and `x86_64-pc-windows-msvc` check passed. The complete extractor Windows cross-check still stops at bundled SQLite because this macOS host lacks Windows MSVC C headers.
- Security evidence — isolated 53-package no-default-feature closure scanned 1,160 cached RustSec advisories with zero findings. The post-integration 483-package full-lock rerun was rejected by the local tool quota at this slice; the later 491-package result above now supersedes that missing evidence.
- Remaining PDF gates — aggregate peak RSS on documented 8 GB hardware, remote macOS Intel/Windows/Linux runtime, latest live Desktop smoke, and broader real-world corpus quality/latency measurement.

## M3 lexical-search vertical-slice evidence — 2026-07-16

- `cargo fmt --all -- --check` — passed.
- `cargo clippy --workspace --all-targets --all-features --offline -- -D warnings` — passed.
- `cargo test --workspace --all-features --offline` — 91 passed, 0 failed: CLI 6 + 5 integration, database 16, Desktop Rust 6, domain 6, extractors 26, identity 2, retrieval 3, scanner 12, search benchmark 2, telemetry 2, watcher 5.
- Database/retrieval fixtures — bundled FTS5 migration/backfill and triggers pass; Traditional Chinese/English path and content substring search passes; stale active-content filtering, query/candidate limits, quote escaping, exact filename fusion, fixed explanations, source selection, extension normalization, date bounds, and invalid-filter rejection pass.
- CLI binary fixture — requested filtered local context and normalized filter diagnostics are returned on stdout; query, private text, filename, and scope path are absent from structured stderr logs.
- `pnpm check` — Prettier, ESLint, TypeScript, 16 Vitest tests, and Vite production build passed.
- `pnpm --filter @deskgraph/desktop tauri build --no-bundle` with `/Users/wetom/.cargo/bin` explicitly on `PATH` — passed and produced `target/release/deskgraph-desktop`.
- Synthetic search benchmark — rerun against the current filtered SQL path in release mode with a 10,000-document, 1.3 MB zh-TW/English corpus and 50 iterations per case. p95: zh-TW content 5.931 ms, English content 16.119 ms, exact filename 2.249 ms, miss 0.090 ms. SQLite file 11,993,088 bytes; FTS shadow tables 5,181,440 bytes. Full report: `benchmarks/results/search-10k-macos-arm64-2026-07-16.json`.
- Dependency delta — M3 added no registry package, vector extension, embedding/model runtime, API, or network client. Its retrieval and benchmark crates are local workspace packages; the then-current 488-package lock also included the later local watcher, transaction, and project crates.
- Evidence still open — latest live Desktop interaction, remote macOS Intel/Windows/Linux runtime, representative and 100k corpora, peak RSS/8 GB/thermal evidence, one/two-character strategy, graph-backed project/folder filters, vector/embedding/hybrid behavior, and multilingual relevance evaluation. The checked-in 10k result is not a release SLO; the later 491-package RustSec result above closes only this slice's missing full-lock scan.

## M6 durable-watch-core vertical-slice evidence — 2026-07-16

- Shared workspace gates — Rust format and all-feature Clippy passed; 91 Rust tests passed; `pnpm check` passed with 16 Vitest tests; the no-bundle Tauri release build produced `target/release/deskgraph-desktop`.
- Watch-core fixtures — temporary download ignore, changing-snapshot deadline reset, scope/symlink/missing-path escape denial, rename-storm coalescing with identity preservation, stabilizing restart, and atomically linked reconciling restart all pass. CLI/Desktop status payload tests contain no observed path or content.
- Dependency delta — M6 added only the local workspace `deskgraph-watcher` crate. No registry package, native watcher, async runtime, API, or network client was added; `Cargo.lock` contained 486 packages at that slice, before the later local transaction crate.
- Evidence still open — native platform adapters or a documented polling adapter, missed-event schedules, efficient per-node reconciliation, incremental extraction/indexing, cloud-placeholder handling, background pause/resource/low-memory policy, Smart Inbox, load/8 GB/thermal evidence, Windows runtime, latest live Desktop interaction, and remote platform CI. The later 491-package RustSec result above closes only this slice's missing full-lock scan.

## M5 rename-preview vertical-slice evidence — 2026-07-16

- `cargo fmt --all -- --check` — passed.
- `cargo clippy --workspace --all-targets --all-features --offline -- -D warnings` — passed.
- `cargo test --workspace --all-features --offline` — 102 passed, 0 failed: CLI 6 + 6 integration, database 18, Desktop Rust 7, domain 7, extractors 26, identity 2, retrieval 3, scanner 12, search benchmark 2, telemetry 2, transactions 6, watcher 5.
- Transaction fixtures — durable reopen, immutable plan, append-only first journal event, no-op/traversal/Windows-invalid/reserved-name denial, destination conflict, stale manifest metadata, scope escape, symlink source, case-only strategy and no filesystem rename all pass.
- CLI binary fixture — explicit canonical before/after paths and nine fixed policy checks return on stdout; source remains and destination remains absent; structured stderr omits source/destination names and paths; `organize list` omits both paths from stdout and stderr.
- Desktop boundary fixtures — Rust proves explicit preview payloads contain requested paths while recent-history payloads do not, and the source/destination remain unchanged. TypeScript accepts only the closed rename/previewed/allowed/nine-check contract, rejects unknown operations/journal shapes/path-bearing summaries, and verifies exact create/list IPC arguments.
- `pnpm check` — Prettier, ESLint, TypeScript, 19 Vitest tests, and Vite production build passed.
- `pnpm --filter @deskgraph/desktop tauri build --no-bundle` with `/Users/wetom/.cargo/bin` explicitly on `PATH` — passed and produced `target/release/deskgraph-desktop` with the non-executing preview/history UI.
- Dependency delta — M5 added only the local workspace `deskgraph-transactions` crate; `Cargo.lock` contains 487 packages. No registry package, filesystem plugin, model, API, network, shell, Python, Docker, or native runtime was added.
- Evidence still open — Windows/macOS Intel/Linux runtime, latest live Desktop interaction, Move/folder/cross-volume contracts, append-only executor states, source/destination execution verification, permission/conflict/partial-copy/disconnect/process-kill fault injection, recovery, rollback, idempotent Undo, and Desktop execution/recovery/Undo controls. The later 491-package RustSec result above closes only this slice's missing full-lock scan.

## M4 folder-profile vertical-slice evidence — 2026-07-16

- `cargo fmt --all -- --check` — passed.
- `cargo clippy --workspace --all-targets --all-features --offline -- -D warnings` — passed.
- `cargo test --workspace --all-features --offline` — 108 passed, 0 failed: CLI 6 + 7 integration, database 18, Desktop Rust 7, domain 8, extractors 26, identity 2, projects 4, retrieval 3, scanner 12, search benchmark 2, telemetry 2, transactions 6, watcher 5.
- Project/profile fixtures — direct and nested folder counts/categories, sibling exclusion, Cargo plus README provenance, model-free suggestion metadata, README-not-strong behavior through the rule boundary, and fail-closed entry limit pass; no source file changes.
- CLI binary fixture — the requested canonical folder path and bounded profile return on stdout; descendant private filename and all folder/member paths are absent from structured stderr; source fixtures still exist.
- `pnpm check` — Prettier, ESLint, TypeScript, 19 Vitest tests, and Vite production build passed.
- `pnpm --filter @deskgraph/desktop tauri build --no-bundle` with `/Users/wetom/.cargo/bin` explicitly on `PATH` — passed and produced `target/release/deskgraph-desktop`; no M4 Desktop UI is claimed.
- Dependency delta — M4 added only the local workspace `deskgraph-projects` crate; `Cargo.lock` contains 488 packages. No registry package, model, embedding/vector runtime, API, network, shell, Python, Docker, Ollama, or native runtime was added.
- Evidence still open at the Folder Profile slice — Windows/macOS Intel/Linux runtime, 100k/8 GB/RSS evaluation, persisted Project/edge candidate schemas, related/similar/duplicate/version relations, confirmation/reject/merge/split, correction feedback, retrieval filters, Project page and live Desktop interaction. The later 491-package RustSec result above closes this slice's missing full-lock scan; the following correctable-candidate slice closes only stable Project root persistence and exact-root accept/reject feedback.

## M4 correctable-project-candidate vertical-slice evidence — 2026-07-16

- `cargo fmt --all -- --check` — passed.
- `cargo clippy --workspace --all-targets --all-features --offline -- -D warnings` — passed.
- `cargo test --workspace --all-features --offline` — 113 passed, 0 failed: CLI 6 + 8 integration, database 19, Desktop Rust 7, domain 9, extractors 26, identity 2, projects 6, retrieval 3, scanner 12, search benchmark 2, telemetry 2, transactions 6, watcher 5.
- Migration/database fixtures — immutable Project roots/suggestion observations/normalized signals and append-only feedback events migrate/reopen; exact current-manifest signal/time/provider/confidence validation rejects invented evidence; update/delete triggers pass.
- Correction fixtures — proposed → rejected → proposed again remains rejected → accepted appends sequence 2 → repeated accepted remains sequence 2; an unmarked folder cannot be persisted; accepted root still creates no file membership.
- CLI binary fixture — explicit propose/decide responses return the current root path and evidence; all structured stderr logs omit database/root/member paths and names; `project list` exposes only path-free summaries; fixture files remain unchanged.
- `pnpm check` — Prettier, ESLint, TypeScript, 19 Vitest tests, and Vite production build passed.
- `pnpm --filter @deskgraph/desktop tauri build --no-bundle` with `/Users/wetom/.cargo/bin` explicitly on `PATH` — passed and produced `target/release/deskgraph-desktop`; no Project Desktop UI is claimed.
- Dependency delta — migration 0008 and the local project/domain/database/CLI changes add no registry package; `Cargo.lock` remains 488 packages. `serde_json` is only an already-resolved test dependency.
- Evidence still open at the correctable-root slice — Windows/macOS Intel/Linux runtime, 100k/8 GB/RSS evaluation, file membership edges, related/similarity/duplicate/version relations, cross-root learning, merge/split, Project/folder retrieval filters, Project page and live Desktop interaction. The later 491-package RustSec result above closes this slice's missing full-lock scan; the following slice closes only explicit bounded exact-byte duplicate observations.

## M4 bounded-exact-duplicate vertical-slice evidence — 2026-07-16

- `cargo fmt --all -- --check` — passed.
- `cargo clippy --workspace --all-targets --all-features --offline -- -D warnings` — passed.
- `cargo test --workspace --all-features --offline --quiet` — 119 passed, 0 failed: CLI 6 + 9 integration, database 20, Desktop Rust 7, domain 10, extractors 26, identity 2, projects 9, retrieval 3, scanner 12, search benchmark 2, telemetry 2, transactions 6, watcher 5.
- Relation/service fixtures — exact non-empty byte equality, reversed endpoint identity reuse, hard-link alias exclusion, canonical-path requirement, symlinked-parent and outside-scope denial, same-size content difference, stale source, empty file, sparse 64 MiB-plus-one denial, post-read identity/metadata revalidation and unchanged source files pass locally.
- Migration/database fixtures — stable ordered `exact_duplicate` relation identity, append-only immutable observations, current manifest snapshot validation, fixed comparison/provider/confidence/model-null schema, update/delete triggers and absent-source invalidation pass.
- CLI binary fixture — `relation duplicate` and live `relation verify` return explicit current paths and complete evidence; structured stderr omits database/file paths, filenames and content; both files remain byte-identical and unchanged.
- `pnpm check` — Prettier, ESLint, TypeScript, 19 Vitest tests, and Vite production build passed.
- `pnpm --filter @deskgraph/desktop tauri build --no-bundle` with `/Users/wetom/.cargo/bin` explicitly on `PATH` — passed and produced `target/release/deskgraph-desktop`; no M4 Desktop UI is claimed.
- Dependency delta — ADR-020/migration 0009 and relation code add no registry package; `Cargo.lock` remains 488 packages and only records the existing local identity dependency for `deskgraph-projects`.
- Evidence still open at the bounded exact-duplicate slice — Windows/macOS Intel/Linux runtime, duplicate latency/RSS and 8 GB evaluation, background discovery, larger-file strong hashing, related/similarity/version signals, file membership, cross-pair/root learning, merge/split, retrieval filters, Project page and live Desktop interaction. The later 491-package RustSec result above closes this slice's missing full-lock scan; the following slice closes only exact-pair accept/reject correction.

## M4 reverified relation-feedback vertical-slice evidence — 2026-07-16

- `cargo fmt --all -- --check` — passed.
- `cargo clippy --workspace --all-targets --all-features --offline -- -D warnings` — passed.
- `cargo test --workspace --all-features --offline --quiet` — 120 passed, 0 failed: CLI 6 + 9 integration, database 20, Desktop Rust 7, domain 11, extractors 26, identity 2, projects 9, retrieval 3, scanner 12, search benchmark 2, telemetry 2, transactions 6, watcher 5.
- Migration/database fixtures — migration 0010 appends immutable user feedback, rejects update/delete, preserves the latest exact-pair state after new observations, makes repeated decisions idempotent, appends opposite-decision sequence 2, validates current manifest snapshots, and returns path-free historical summaries marked `verification_required`.
- Relation/service fixtures — every accept/reject runs ADR-020's complete bounded live byte comparison first; rejected → later observation remains rejected → accepted → repeated accepted remains sequence 2; both files remain unchanged.
- CLI binary fixture — `relation decide` returns explicit verified paths and decision provenance, while `relation list` exposes only scope/node IDs, state, confidence, times, and `verification_required`; all structured stderr plus list stdout omit database/file paths, filenames, and content.
- `pnpm check` — Prettier, ESLint, TypeScript, 19 Vitest tests, and Vite production build passed.
- `pnpm --filter @deskgraph/desktop tauri build --no-bundle` with `/Users/wetom/.cargo/bin` explicitly on `PATH` — passed and produced `target/release/deskgraph-desktop`; no relation Desktop UI is claimed.
- Dependency delta — ADR-021/migration 0010 and relation feedback code add no registry package; `Cargo.lock` remains 488 packages and is unchanged by this slice.
- Code commit — `b1abc8c feat: add reverified relation feedback`.
- Evidence still open — Windows/macOS Intel/Linux runtime, duplicate latency/RSS and 8 GB evaluation, background discovery, larger-file strong hashing, deterministic related/version signals, file membership and correction, cross-pair/root learning, merge/split, retrieval filters, Project page and live Desktop interaction. The later 491-package RustSec result above closes only this slice's missing full-lock scan.

## M4 explicit-filename-version vertical-slice evidence — 2026-07-16

- `cargo fmt --all -- --check` — passed on the committed source state.
- `cargo clippy --workspace --all-targets --all-features --offline -- -D warnings` — passed.
- `cargo test --workspace --all-features --offline --quiet` — 126 passed, 0 failed: CLI 6 + 10 integration, database 22, Desktop Rust 7, domain 13, extractors 26, identity 2, projects 10, retrieval 3, scanner 12, search benchmark 2, telemetry 2, transactions 6, watcher 5.
- Shared rule fixtures — Traditional Chinese/English names, four allowlisted `vN` separators, case/NFC normalization, no-leading-zero/range/single-suffix limits, same-number/base/extension/unsupported-name rejection, and fixed 9000-basis-point model-free provenance pass.
- Migration/database fixtures — migration 0011 rebuilds the immutable unified relation parent, preserves pre-version exact relation IDs/observations/rejected feedback and a clean `foreign_key_check`, appends directional version observations, reuses reversed-input identity, rejects mutation/deletion, invalidates absent snapshots, and lists path-free history marked `verification_required`.
- Relation/service fixtures — canonical scope, non-symlink/reparse, different stable identity, manifest/platform/open-handle metadata before and after name analysis, stale-source denial, deterministic older/newer output, no content comparison, and unchanged file evidence pass locally.
- CLI binary fixture — `relation version` and `relation version-verify` return explicit current paths and directional evidence; structured stderr plus `relation list` omit database/file paths, normalized filename base, filenames, and contents.
- `pnpm check` — Prettier, ESLint, TypeScript, 19 Vitest tests, and Vite production build passed.
- `pnpm --filter @deskgraph/desktop tauri build --no-bundle` with `/Users/wetom/.cargo/bin` explicitly on `PATH` — passed and produced `target/release/deskgraph-desktop`; no version-relation Desktop UI is claimed.
- Dependency delta — no new registry package; the existing audited `unicode-normalization 0.1.25` becomes a direct domain dependency so database/service share one parser. `Cargo.lock` remains 488 packages and changes only the local dependency list.
- Code commit — `06f8466 feat: add explicit file version candidates`.
- Evidence still open — date/semantic/general version discovery, related/similarity relations, background discovery, evaluation corpora, file membership/correction, Windows/macOS Intel/Linux runtime, 8 GB/RSS, Project page, and live Desktop interaction.

## M4 evidence-bound-version-feedback vertical-slice evidence — 2026-07-17

- Migration/domain — migration 0012 adds observation-referenced append-only feedback with relation-sequence uniqueness, composite foreign-key provenance, supporting indexes, and immutable update/delete triggers. Version candidate API v2 exposes the bound evidence observation ID; existing candidates receive no backfilled decision.
- Database/service fixtures — equivalent re-observations retain their latest decision, repeated decisions remain sequence-idempotent, opposite corrections append, changed ordered nodes/version evidence returns `suggested`, restored equivalent evidence recovers its latest decision, and changed file metadata prevents a decision before any event is written.
- CLI/privacy fixture — `relation version-decide` performs live scope/path/identity/metadata/open-handle/name revalidation, returns explicit current endpoints plus decision provenance, and does not change either file. Structured logs and `relation list` omit database/file paths, normalized filename base, filenames, and contents.
- Final local gates — Rust format, all-target/all-feature Clippy, and 127 workspace tests pass; `pnpm check` passes Prettier, ESLint, TypeScript, 19 Vitest tests, and Vite build; Tauri no-bundle release build produces `target/release/deskgraph-desktop`.
- Dependency delta — no registry, model, API, network, Python, Docker, Ollama, or filesystem dependency was added; `Cargo.lock` remains unchanged.
- Code commit — `06341f8 feat: add evidence-bound version feedback`.
- Evidence still open — no version-relation Desktop UI is claimed; Windows/macOS Intel/Linux runtime, 8 GB/RSS/scale evaluation, general version discovery, related/similarity relations, background discovery, file membership/correction, Project page, and live Desktop interaction remain.

## M2 bounded-Office vertical-slice evidence — 2026-07-17

- Provider boundary — the same explicit-scope Manifest job and controlled `Read + Seek` path used by text/PDF now routes `.docx`, `.pptx`, and `.xlsx` to `deskgraph.ooxml-text`; no archive entry is written to disk and no relationship, external link, macro, formula, embedded object, attachment, shell, process, or network path is executed.
- Archive/XML bounds — exact allowlisted parts only; 4,096 archive-entry and 1,024 selected-part caps; 200:1 compression-ratio cap; claimed and actual decompression limits; encryption, unsafe names, duplicate entries, unsupported compression, overlapping ranges, DTD, processing instructions, unsupported references, over-depth/attribute/event/text/unit/shared-string limits, cancellation, and time limits fail with fixed per-file codes and no partial publication.
- Structural provenance — migration 0013 preserves existing byte/page chunks and FTS rows, then adds DOCX paragraph, PPTX slide, and XLSX sheet/cell plus fragment provenance. Excel references are independently bounded to `A1:XFD1048576` at provider and database boundaries. All content remains `untrusted_extracted_text`.
- End-to-end fixtures — mixed Traditional Chinese/English DOCX, numeric slide order, XLSX shared/inline/numeric cells, formula-value suppression, namespace isolation, corrupt XML/ZIP, traversal, duplicate/encrypted/overlapping archives, inert active parts, invalid shared indexes, structure/decompression/output/chunk limits, cooperative cancellation, migration preservation, database validation, and Manifest→atomic SQLite→FTS routing pass locally.
- Final local gates — `cargo fmt --all -- --check` passed; all-target/all-feature Clippy passed with `-D warnings`; 140 workspace Rust tests passed; `pnpm check` passed Prettier, ESLint, TypeScript, 19 Vitest tests, and Vite production build; Tauri no-bundle release produced `target/release/deskgraph-desktop`.
- Dependency evidence — exact no-default `zip 8.6.0` with only `deflate-flate2-zlib-rs` and `quick-xml 0.41.0` are accepted by ADR-014. The lock grows from 488 to 491 packages (`zip`, `typed-path`, `zlib-rs`; `quick-xml` was already transitive). The current full 491-package scan against 1,160 cached RustSec advisories reports zero vulnerabilities and the same 16 unmaintained plus one `glib` unsound warnings as the prior lock; none belongs to the OOXML delta.
- Platform evidence — the isolated exact dependency/API fixture passed macOS arm64 and checked `x86_64-pc-windows-msvc`. The complete extractor cross-check on this macOS host stops at bundled `libsqlite3-sys` because Windows MSVC C headers are unavailable; native Windows CI remains mandatory and bundled SQLite is not weakened.
- Code commit — `27cd202 feat: add bounded Office text extraction`.
- Evidence still open at this historical Office slice — representative real-world Office corpus quality/latency, aggregate peak RSS on documented 8 GB hardware, native macOS Intel/Windows/Linux runtime, latest live Desktop interaction, and extraction scheduling/resource policy. The following sections close only the local image-metadata and macOS arm64 OCR slices; DeskGraph remains **do not ship**.

## M2 bounded-image-metadata vertical-slice evidence — 2026-07-17

- Provider/privacy boundary — `.png`, `.jpg`/`.jpeg`, `.gif`, `.webp`, `.bmp`, and `.tif`/`.tiff` route from an explicit Manifest job to `deskgraph.image-metadata`. The provider receives only controlled bytes, reads a bounded prefix, decodes no pixels, opens no paths, performs no network/process work, and collects no EXIF, GPS, filename, or path fields.
- Bounds and validation — source size is capped at 64 MiB, probe bytes at 2 MiB by default/8 MiB absolute, reader operations at 65,536, each dimension at 100,000, and total encoded pixels at 500 million. Signature must match the manifest extension; strict PNG IHDR, JPEG SOI, GIF version, WebP RIFF/chunk/declared-length, BMP DIB, and TIFF byte-order/type checks fail closed with fixed codes.
- Durable publication — forward migration 0014 adds structured `image_metadata` with one active row per node and source/provider provenance. Text chunks and image metadata replace each other atomically, historical job rows remain queryable, manifest changes invalidate stale metadata, and the migration fixture preserves existing chunks plus FTS results.
- Usable slice — `extract start` runs the image job and `extract image-metadata --job` returns the versioned structured result. CLI integration proves stdout/stderr omit the image filename, scope/source/database paths, and content. Corrupt, fake, extension-mismatched, undersized WebP, dimension-bomb, probe-cap, cancellation, source-change, and no-partial-publication fixtures pass.
- Final local gates — `cargo fmt --all -- --check` passed; all-target/all-feature offline Clippy passed with `-D warnings`; 149 workspace Rust tests passed; `pnpm check` passed Prettier, ESLint, TypeScript, 19 Vitest tests, and Vite production build; the final Tauri no-bundle release build produced `target/release/deskgraph-desktop`.
- Dependency evidence — exact `imagesize 0.15.0`, default features disabled, enables only BMP/GIF/JPEG/PNG/TIFF/WebP and adds no transitive package. Its isolated macOS arm64 test and Rust 1.97 Windows x64 check pass. The full lock grows from 491 to 492 packages and scans 1,160 cached RustSec advisories with zero vulnerabilities and the same 16 unmaintained plus one `glib` unsound warnings; none belongs to `imagesize`.
- Code commit — `5f7b863 feat: add bounded image metadata extraction`.
- Evidence still open at this historical image-metadata slice — representative image corpus quality/latency, native macOS Intel/Windows/Linux runtime, latest live Desktop interaction, and aggregate peak RSS on documented 8 GB hardware. The following section closes only the local macOS arm64 PNG/JPEG OCR slice; Windows/fallback OCR and release evidence remain open, so DeskGraph remains **do not ship**.

## M2 bounded-macOS-Screenshot-OCR vertical-slice evidence — 2026-07-17

- Provider/privacy boundary — `OcrProvider` receives only core-validated bounded PNG/JPEG encoded bytes, dimensions, limits, deadline and cancellation state. The target-specific Apple Vision adapter receives no path, URL, network/process capability or ambient model directory; runtime requires both `zh-Hant` and `en-US` and requests Traditional Chinese first.
- Bounds and provenance — OCR independently caps source at 32 MiB, each dimension at 16,384, total pixels at 64 Mi pixels, output at 8 MiB, observations at 4,096, one observation at 256 KiB, and active processing at 60 seconds. Migration 0015 preserves existing jobs/content/image metadata/FTS and stores operation, one-based observation, fragment, normalized top-left bounding box and confidence basis points; every chunk remains `untrusted_extracted_text`.
- Durable/atomic behavior — an operation-specific job opens only an existing scanned node, validates scope/exclusion/manifest/open-handle identity, polls durable cancellation on a separate SQLite connection, and publishes OCR chunks only after the complete provider output and final source check. No-text completes with zero chunks; corrupt/unsupported input, source/pixel/output/observation/chunk/deadline/provenance limits, provider failure, cancellation and source change publish no partial replacement. Source change invalidates stale searchable OCR; ordinary provider failure/cancellation preserves a prior complete version.
- Usable slice — `extract ocr-start` and `ocr-create` expose the durable operation; CLI binary coverage proves the queued OCR status and structured logs omit database/scope/source paths and filename. Desktop recent-job status distinguishes Screenshot OCR but intentionally has no OCR start control. A real current-binary macOS arm64 run recognized `DeskGraph OCR` and `桌面圖譜 安全整理`, persisted two spatial/confidence observations and returned both through FTS.
- Environment evidence — the restricted runner returned fixed `extraction_ocr_provider_failed` with zero output, while the same bytes and binary completed outside that sandbox in 1,153 ms with 51,661 source bytes, 38 output bytes and two chunks. This boundary is exposed rather than hidden; installer entitlement and clean-machine validation remain open.
- Final local gates — `cargo fmt --all -- --check`, all-target/all-feature offline Clippy with `-D warnings`, 166 workspace Rust tests, `pnpm check` with 19 Vitest tests and Vite production build, and Tauri no-bundle release build pass. `cargo audit --no-fetch` scans 493 lock packages against 1,160 cached advisories with zero vulnerabilities and the existing 16 unmaintained plus one `glib` unsound warnings.
- Dependency evidence — exact target-specific no-default `objc2-vision 0.3.2` plus already-present `block2 0.6.2`, `objc2 0.6.4`, and `objc2-foundation 0.3.2` use only the accepted Vision/Foundation feature set. The lock grows from 492 to 493 packages; the one-package delta has no cached advisory. Archive checksum and isolated nine-package evidence remain recorded in ADR-024 and `DEPENDENCY_AUDIT.md`.
- Code commits — `d24e811 feat: add bounded macOS screenshot OCR`; `e8ae893 test: harden screenshot OCR boundaries`.
- Evidence still open — actual runtime `VNRequest.cancel`, representative mixed/no-text/adversarial real-image corpus quality and latency, peak/start/end RSS on documented 8 GB hardware, macOS Intel/Universal and clean-machine runtime, Windows native runtime/language feature, D-015 packaged fallback selection/implementation, OCR for scanned PDFs, Desktop start flow, installer/SBOM/checksums and release evidence. The macOS arm64 slice is locally verified; M2 and DeskGraph remain **do not ship**.

## M2 bounded-Windows-Screenshot-OCR code/cfg evidence — 2026-07-18

- Provider/privacy boundary — the internal provider contract now transfers an owned, core-bounded PNG/JPEG byte vector rather than a path. The Windows adapter receives no URL, network/process capability, ambient model directory, or arbitrary file handle. `GetCurrentPackageFullName` reads only the required buffer length and never retrieves/logs the package name.
- Language/provenance — separate `zh-TW` and `en-US` requests validate the actual `RecognizerLanguage` as Traditional Chinese (never `zh-Hans`) or English. Word rectangles are unioned into normalized top-left line boxes and exact text/box duplicates are removed without output reordering. Migration 0016 keeps every box mandatory and confidence nullable; Windows writes `NULL`. Only absent/zero `TextAngle` may publish source-aligned boxes.
- Lifecycle/bounds — a fresh MTA worker balances `RoInitialize`/`RoUninitialize`. Every WinRT async operation checks durable cancellation/deadline; while status remains readable, `Cancel()` is followed by drain and `Close()` only after `Completed`, `Canceled`, or `Error`. A failed `Status()` requests cancel and releases without falsely calling `Close()`. The caller waits in bounded 10 ms intervals and may detach cleanup after its deadline. A process-wide one-worker gate prevents a stuck native operation from accumulating more workers and makes subsequent OCR fail closed until cleanup or restart.
- Verification — 66 extractor tests pass, including cancellation drain, terminal-close ordering, completion race, deadline, status failure, bounded worker-result wait, language policy, `TextAngle`, boxes, nullable confidence, de-duplication, atomic replacement and prior-output safety. Final repository gates pass: Rust format, all-target/all-feature Clippy with `-D warnings`, 180 workspace Rust tests, `pnpm check` with Prettier/ESLint/TypeScript/19 Vitest tests/Vite production build, and the Tauri no-bundle release build at `target/release/deskgraph-desktop`. With `LIBSQLITE3_SYS_USE_PKG_CONFIG=1 PKG_CONFIG_ALLOW_CROSS=1`, Windows x64 Rust cfg `cargo check` and Clippy pass; a normal cross-check stops in bundled `libsqlite3-sys` because this macOS host lacks Windows C/MSVC headers. This is typecheck evidence only, not link or runtime.
- Dependency/security — direct Windows target edges reuse locked `windows 0.61.3`, `windows-future 0.2.1`, and `windows-sys 0.61.2`; the lock stays at 493 packages. The current `cargo audit --no-fetch` scan reports zero vulnerabilities plus the same 17 accepted Tauri/Linux warnings. Exact sources, checksums, licenses, MSRVs, API and feature-unification evidence are in ADR-024 and `DEPENDENCY_AUDIT.md`.
- Code commits — `15c728d fix: preserve honest OCR confidence provenance`; `59e91b2 feat: add bounded Windows screenshot OCR`.
- Evidence still open — true Windows 10/11 x64 build/link/runtime; MSIX/external-location and unpackaged identity behavior; present/missing/mismatched recognizers; mixed/no-text/corrupt/limit/rotation OCR; native cancel/cleanup/single-worker recovery; CPU/RSS on documented 8 GB hardware; installer; fallback routing; representative corpus and release evidence. Windows OCR is not safe to advertise or ship yet; M2 and DeskGraph remain **do not ship**.
