# Implementation Status

Last updated: 2026-07-16

Status vocabulary: `not started`, `in progress`, `blocked`, `verified locally`, `verified in CI`, `released`.

## Current milestones

M2 Content Intelligence remains on the critical path; M3 Hybrid Retrieval, M4 Project Graph, M5 Safe Organization, and M6 Watch Mode each have a bounded first slice — all are **in progress**. M0 remains open for remote CI evidence, and M1 remains open for complete Windows runtime, peak-memory, and latest live-UI evidence.

Two M2 vertical slices now run end to end. Explicit already-scanned text, Markdown, source-code, and text-layer PDF files are resolved inside their authorized scope; the core revalidates the canonical scope, exclusion policy, manifest snapshot, and open-file identity; bounded providers receive only a controlled `Read + Seek`; durable SQLite jobs support cancellation and interrupted recovery; tagged byte/page provenance and complete untrusted chunks publish atomically; CLI and Desktop expose privacy-safe progress and counts. Invalid UTF-8, corrupt/encrypted PDF, active PDF content and attachments, decompression/page/output limits, changed files, symlink swaps, cancellation, expired leases, false output sizes, and failed replacement are covered locally. M2 is not complete because Office formats, image metadata, OCR, remaining adversarial fixtures, full Windows runtime evidence, and an extraction benchmark on 8 GB hardware remain open.

One M3 lexical vertical slice now runs end to end. Transactional SQLite FTS5 trigram indexes cover current authorized display paths and active extracted chunks; normalized quoted queries are limited to 3–256 Unicode characters, results/candidates are capped, stale content is filtered by source-of-truth joins, and deterministic fusion exposes exact filename/path/content explanations. Scope, metadata-vs-content source, ASCII-alphanumeric extension, and inclusive/exclusive UTC modified-time filters are validated by both retrieval and database boundaries. CLI and Desktop return user-requested paths and bounded snippets while logs omit query/path/text. This is not M3 completion: one- and two-character search, project/folder filters, vector/embedding adapters, semantic and “files like this” queries, hybrid fusion, representative multilingual evaluation, 100k/8 GB/cross-platform benchmarks, and live-UI evidence remain open.

Three M4 vertical slices now run end to end. An explicit already-scanned folder resolves to its current manifest identity, then streams at most 100,000 present descendant locations into deterministic facts and explainable marker-based Project Suggestions. A Project root candidate persists by stable folder identity with immutable observations/signals and append-only explicit user accept/reject events; same decisions are idempotent, opposite decisions append corrections, and a rejected root stays rejected on a later proposal. Two different current files in one authorized scope can now produce an `exact_duplicate` suggestion only after canonical/non-symlink policy, manifest/platform/open-handle identity, non-empty equal size up to 64 MiB, complete byte equality, a cooperative deadline, post-read revalidation, and atomic immutable observation persistence pass. Hard-link aliases are one identity, not duplicates. Explicit responses may return the selected paths; logs and recent Project summaries remain path-free. No M4 slice uses a model or performs a file action. This is not M4 completion: file membership, entities/topics, related/similarity/version relations, background duplicate discovery, relation feedback, clustering, cross-root learning, merge/split, retrieval filters, Project page, cross-platform runtime, and scale/memory evaluation remain open.

One M5 preview vertical slice now runs end to end without performing a file action. A same-folder scanned-file rename is canonical-scope, manifest snapshot, platform identity, metadata, and read-only open-handle validated; a portable single-component target and conflict-free destination are required; an immutable plan and sequence-1 event commit atomically. Explicit CLI and Desktop preview returns before/after paths and nine passed policy checks, while logs and recent summaries are path-free; the UI explicitly has no execute control. This is not an organizer or completed transaction engine: Move/folders, execution, immediate pre-action revalidation, destination verification/hash, cross-volume handling, fault injection, startup recovery, rollback, idempotent Undo, Windows runtime, and live interaction remain open.

One M6 core vertical slice now runs end to end. Untrusted path hints are canonical-scope validated, temporary/hidden entries are ignored, events coalesce durably per scope, and unchanged existence/kind/size/modified-time/platform identity plus read-only open-handle identity are required before an atomically linked resumable scan reconciles the manifest. Stabilizing and reconciling events resume after database reopen; rename storms preserve identity locally. CLI and Desktop status are path-free. This is not automatic Watch Mode: native OS adapters, efficient per-node reconciliation, incremental extraction/indexing, cloud-placeholder handling, background pause/resource/low-memory policy, notification preferences, Smart Inbox states, Windows runtime evidence, and live Desktop interaction remain open.

## Milestones

| Milestone                     | Status      | Current evidence                                                           | Next gate                                             |
| ----------------------------- | ----------- | -------------------------------------------------------------------------- | ----------------------------------------------------- |
| M0 Repository Foundation      | In progress | Local foundation slice, governance, lockfiles, checks, CLI, and desktop smoke verified | Green macOS/Windows/Linux CI matrix |
| M1 Manifest Graph             | In progress | Explicit scope → durable bounded queue/staging → atomic SQLite manifest publish → CLI/desktop progress and controls; 10k, permission, recovery, protected-tree, and adversarial local tests | Peak RSS, latest live UI smoke, cross-platform runtime CI |
| M2 Content Intelligence       | In progress | Text/Markdown/code plus bounded text-layer PDF → durable job → open-file identity revalidation → tagged provenance → atomic untrusted chunks → CLI/Desktop status | Audit and implement Office, image metadata, and OCR providers one at a time; benchmark on 8 GB |
| M3 Hybrid Retrieval           | In progress | Offline path/content FTS5 trigram → bounded retrieval and scope/type/date/source filters → deterministic explanations → CLI/Desktop search → synthetic 10k benchmark | Project/folder filters and extended benchmarks, then audited vector/embedding adapters, fusion, evaluation |
| M4 Project Graph              | In progress | Bounded Folder Profile → correctable stable Project root plus explicit ≤64 MiB full-byte exact-duplicate candidate → immutable evidence → privacy-aware CLI; no file membership, model or action | Version/related signals, background duplicate discovery, relation/file membership correction, cross-root learning, evaluation, retrieval filters, Project page |
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
| Rust format, lint, and tests configured                  | Verified locally   | Rust 1.97.0; current workspace format and Clippy pass; 119 tests pass       |
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
- The last complete pre-PDF all-target RustSec scan reported zero known vulnerabilities and 17 warnings, including unmaintained GTK3 bindings and one `glib` unsound advisory on Tauri's Linux path; the isolated no-default-feature PDF closure is RustSec-clean. The current 488-package lock adds only local workspace retrieval, benchmark, watcher, transaction, and project packages after the 483-package PDF state, but the post-PDF full-lock scan was rejected by the local tool quota and must be rerun; tracked as R-010/R-016.
- Windows open-handle file-identity adapter compiles for `x86_64-pc-windows-msvc`, but complete scanner/extractor cross-checks cannot be produced on this macOS host because bundled SQLite needs a Windows C/MSVC toolchain. Remote Windows CI remains required.
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
| Provider boundary receives no arbitrary path capability | Verified locally | Accepted ADR-012; `ExtractorProvider` receives only a controlled `Read + Seek`, validated metadata, limits, and a cancellation signal |
| Text, Markdown, and source-code extraction | Verified locally | Dependency-free UTF-8 provider, explicit extension routing, BOM/offset handling, zh-TW + English fixture, invalid-encoding isolation |
| Durable extraction jobs and cancellation | Verified locally | Queued/running/completed/failed/cancelled/interrupted states, durable cancel request, bounded polling, expiring runner lease, explicit resume after recovery |
| Scope, exclusion, identity, and TOCTOU revalidation | Verified locally on Unix; Windows runtime pending | Canonical root/source containment, hidden/symlink/reparse denial, creation-time manifest snapshot, actual open-handle identity before and after extraction; symlink-swap fixture passes |
| Bounded resource policy | Verified locally for text and PDF providers | Defaults: 4 MiB source, 8 MiB decompression unit/stored output, 512 PDF pages, 2,048 chunks, 5 seconds; absolute caps: 64 MiB source/decompression/output, 4,096 pages, 65,536 chunks, 64 KiB/chunk, 60 seconds; database independently validates output totals |
| Atomic content publication and prior-version safety | Verified locally | Per-file transaction deactivates prior chunks only after all chunks validate; failure/cancellation preserves the prior complete version; source changes invalidate stale content |
| Provenance, offsets, and untrusted classification | Verified locally for byte and PDF providers | Migration preserves existing exact byte ranges; PDF chunks store page/fragment with byte columns null; every chunk records source identity/snapshot, provider/version, ordinal, and fixed `untrusted_extracted_text` trust class |
| Per-file error isolation | Verified locally for text and PDF slices | Invalid UTF-8, corrupt/encrypted PDF, decompression/page/output limits, and invalid policy produce fixed errors without aborting the process or publishing partial chunks |
| Privacy-safe usable CLI | Verified locally | `extract start/create/run/status/list/cancel/resume/stats`; explicit `--path` resolves only an existing scanned node; binary test proves path, filename, and extracted text do not enter stdout/stderr |
| Desktop extraction status | Verified locally except live smoke | Narrow read-only Tauri IPC, runtime-validated TypeScript schemas, empty/success/failure/cancel/interrupted labels, 10 frontend tests, Vite and Tauri release builds; latest window interaction not run |
| PDF text | Verified locally; platform/memory evidence pending | Strict bounded `lopdf 0.44.0` provider routes through manifest identity and atomic SQLite publication. zh-TW/English, corrupt, encrypted, JavaScript/Launch/URI/attachment inertness, decompression/page/output/cancel, page provenance, and service routing fixtures pass. Aggregate parser residency, remote macOS Intel/Windows/Linux runtime, and post-integration full-lock RustSec scan remain open. |
| DOCX / PPTX / XLSX | Research in progress | Proposed ADR-014 defines allowlisted in-memory parts and safety/fixture gates; `zip 8.6.0` and `quick-xml 0.41.0` remain unapproved until exact closure, license, RustSec, API, and platform evidence can run |
| Image metadata | Not started | Bounded signature/metadata provider and corrupt/oversized fixtures required |
| Screenshot OCR with zh-TW and English | Not started | D-008 remains open; native and packaged fallback candidates require official API/platform/license/memory evaluation; no Python requirement allowed |

## Next handoff

Continue `prompts/03_EXTRACTORS_OCR.md`. Complete D-011's exact dependency gate before accepting ADR-014 or implementing Office; do not add the ZIP/XML candidates while the isolated closure and audit evidence is unavailable. In parallel, continue `prompts/04_HYBRID_SEARCH.md` from the verified lexical slice and bounded type/date/source filters: add project/folder filters only after their source-of-truth graph model exists, and extend the benchmark to 100k, representative corpora, peak RSS and remote platforms; vector/embedding candidates remain unselected. Image metadata and OCR remain separate provider decisions. Keep M1 evidence closure as a parallel release workstream: Windows junction/hidden-attribute runtime fixtures, peak RSS on an unrestricted 8 GB machine, latest desktop interaction smoke, and remote macOS/Windows/Linux CI.

For `prompts/05_PROJECT_GRAPH.md`, continue from ADR-018/ADR-019/ADR-020's derived Folder Profile, exact-root append-only correction, and explicit bounded exact-duplicate observation without converting any suggestion into automatic file membership or action. Add deterministic version and related candidates with provenance/current-data invalidation next; design background duplicate discovery and larger-file hashing separately, resolve D-013 before cross-root learned scoring or merge/split, then add relation/membership correction and evaluation before Project/folder retrieval filters or a Project page can claim source-of-truth behavior.

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
| Privacy-safe usable CLI | Verified locally | `folder profile`, `project propose/decide/status/list`, `relation duplicate/verify`; explicit profile/root/relation responses may return selected paths, while structured stderr and `project list` omit paths, filenames, database paths and content |
| No new external dependency | Verified locally | Local `deskgraph-projects` reuses database/domain/scanner/identity plus fixed-size standard-library reads; no registry hash/graph package, model, embedding runtime, API, or network client added |
| Persisted Project root candidates | Verified locally | Migration 0008 keys roots by stable scope/folder identity and stores immutable rule observations plus normalized signals; database re-derives current profile facts and rejects invented/stale evidence |
| Append-only user accept/reject correction | Verified locally for exact root | Explicit user events are append-only; same-decision retry is idempotent; opposite decision appends the next sequence; rejected roots remain rejected on later proposals and can be corrected to accepted |
| File membership edges | Not started | Accepted root does not imply descendant membership; `belongs_to` candidate/confirmation/current-data invalidation contracts and fixtures required |
| Exact duplicate relation candidates | Verified locally for explicit pairs ≤64 MiB | Migration 0009 persists stable ordered endpoints and append-only immutable full-byte observations; canonical scope, symlink/hard-link, manifest/open-handle identity, content, limit, stale-source and live verify fixtures pass; every result remains `suggested` |
| Related, similarity and version relations | Not started | Deterministic/embedding signals, provenance, evaluation corpora, correction and current-data invalidation required; exact byte equality is not fuzzy similarity or version inference |
| Background duplicate discovery and larger files | Not started | Requires a bounded scheduler, current-data index and separately audited strong-hash design; explicit 64 MiB pair checks are not release-scale discovery |
| Cross-root learning and merge/split | Not started | D-013 open; explainable influence bounds, reversible identity events and evaluation evidence required; exact-root feedback is not general learned scoring |
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
| Vector semantic search and embedding cache | Not started | D-007/D-009 open; dependency, model, license, checksum, memory, unload, and multilingual evidence required |
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
- Security evidence — isolated 53-package no-default-feature closure scanned 1,160 cached RustSec advisories with zero findings. The post-integration 483-package full-lock rerun was rejected by the local tool quota; do not reuse the older 457-package result as current evidence.
- Remaining PDF gates — aggregate peak RSS on documented 8 GB hardware, remote macOS Intel/Windows/Linux runtime, latest live Desktop smoke, full-lock RustSec rerun, and broader real-world corpus quality/latency measurement.

## M3 lexical-search vertical-slice evidence — 2026-07-16

- `cargo fmt --all -- --check` — passed.
- `cargo clippy --workspace --all-targets --all-features --offline -- -D warnings` — passed.
- `cargo test --workspace --all-features --offline` — 91 passed, 0 failed: CLI 6 + 5 integration, database 16, Desktop Rust 6, domain 6, extractors 26, identity 2, retrieval 3, scanner 12, search benchmark 2, telemetry 2, watcher 5.
- Database/retrieval fixtures — bundled FTS5 migration/backfill and triggers pass; Traditional Chinese/English path and content substring search passes; stale active-content filtering, query/candidate limits, quote escaping, exact filename fusion, fixed explanations, source selection, extension normalization, date bounds, and invalid-filter rejection pass.
- CLI binary fixture — requested filtered local context and normalized filter diagnostics are returned on stdout; query, private text, filename, and scope path are absent from structured stderr logs.
- `pnpm check` — Prettier, ESLint, TypeScript, 16 Vitest tests, and Vite production build passed.
- `pnpm --filter @deskgraph/desktop tauri build --no-bundle` with `/Users/wetom/.cargo/bin` explicitly on `PATH` — passed and produced `target/release/deskgraph-desktop`.
- Synthetic search benchmark — rerun against the current filtered SQL path in release mode with a 10,000-document, 1.3 MB zh-TW/English corpus and 50 iterations per case. p95: zh-TW content 5.931 ms, English content 16.119 ms, exact filename 2.249 ms, miss 0.090 ms. SQLite file 11,993,088 bytes; FTS shadow tables 5,181,440 bytes. Full report: `benchmarks/results/search-10k-macos-arm64-2026-07-16.json`.
- Dependency delta — M3 added no registry package, vector extension, embedding/model runtime, API, or network client. Its retrieval and benchmark crates are local workspace packages; the current 488-package lock also includes the later local watcher, transaction, and project crates.
- Evidence still open — latest live Desktop interaction, remote macOS Intel/Windows/Linux runtime, current full-lock RustSec scan, representative and 100k corpora, peak RSS/8 GB/thermal evidence, one/two-character strategy, graph-backed project/folder filters, vector/embedding/hybrid behavior, and multilingual relevance evaluation. The checked-in 10k result is not a release SLO.

## M6 durable-watch-core vertical-slice evidence — 2026-07-16

- Shared workspace gates — Rust format and all-feature Clippy passed; 91 Rust tests passed; `pnpm check` passed with 16 Vitest tests; the no-bundle Tauri release build produced `target/release/deskgraph-desktop`.
- Watch-core fixtures — temporary download ignore, changing-snapshot deadline reset, scope/symlink/missing-path escape denial, rename-storm coalescing with identity preservation, stabilizing restart, and atomically linked reconciling restart all pass. CLI/Desktop status payload tests contain no observed path or content.
- Dependency delta — M6 added only the local workspace `deskgraph-watcher` crate. No registry package, native watcher, async runtime, API, or network client was added; `Cargo.lock` contained 486 packages at that slice, before the later local transaction crate.
- Evidence still open — native platform adapters or a documented polling adapter, missed-event schedules, efficient per-node reconciliation, incremental extraction/indexing, cloud-placeholder handling, background pause/resource/low-memory policy, Smart Inbox, load/8 GB/thermal evidence, Windows runtime, latest live Desktop interaction, remote platform CI, and the current full-lock RustSec scan.

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
- Evidence still open — current full-lock RustSec scan, Windows/macOS Intel/Linux runtime, latest live Desktop interaction, Move/folder/cross-volume contracts, append-only executor states, source/destination execution verification, permission/conflict/partial-copy/disconnect/process-kill fault injection, recovery, rollback, idempotent Undo, and Desktop execution/recovery/Undo controls.

## M4 folder-profile vertical-slice evidence — 2026-07-16

- `cargo fmt --all -- --check` — passed.
- `cargo clippy --workspace --all-targets --all-features --offline -- -D warnings` — passed.
- `cargo test --workspace --all-features --offline` — 108 passed, 0 failed: CLI 6 + 7 integration, database 18, Desktop Rust 7, domain 8, extractors 26, identity 2, projects 4, retrieval 3, scanner 12, search benchmark 2, telemetry 2, transactions 6, watcher 5.
- Project/profile fixtures — direct and nested folder counts/categories, sibling exclusion, Cargo plus README provenance, model-free suggestion metadata, README-not-strong behavior through the rule boundary, and fail-closed entry limit pass; no source file changes.
- CLI binary fixture — the requested canonical folder path and bounded profile return on stdout; descendant private filename and all folder/member paths are absent from structured stderr; source fixtures still exist.
- `pnpm check` — Prettier, ESLint, TypeScript, 19 Vitest tests, and Vite production build passed.
- `pnpm --filter @deskgraph/desktop tauri build --no-bundle` with `/Users/wetom/.cargo/bin` explicitly on `PATH` — passed and produced `target/release/deskgraph-desktop`; no M4 Desktop UI is claimed.
- Dependency delta — M4 added only the local workspace `deskgraph-projects` crate; `Cargo.lock` contains 488 packages. No registry package, model, embedding/vector runtime, API, network, shell, Python, Docker, Ollama, or native runtime was added.
- Evidence still open at the Folder Profile slice — current full-lock RustSec scan, Windows/macOS Intel/Linux runtime, 100k/8 GB/RSS evaluation, persisted Project/edge candidate schemas, related/similar/duplicate/version relations, confirmation/reject/merge/split, correction feedback, retrieval filters, Project page and live Desktop interaction. The following correctable-candidate slice closes only stable Project root persistence and exact-root accept/reject feedback.

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
- Evidence still open at the correctable-root slice — current full-lock RustSec scan, Windows/macOS Intel/Linux runtime, 100k/8 GB/RSS evaluation, file membership edges, related/similarity/duplicate/version relations, cross-root learning, merge/split, Project/folder retrieval filters, Project page and live Desktop interaction. The following slice closes only explicit bounded exact-byte duplicate observations.

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
- Evidence still open — current full-lock RustSec scan, Windows/macOS Intel/Linux runtime, duplicate latency/RSS and 8 GB evaluation, background discovery, larger-file strong hashing, related/similarity/version signals, relation feedback, file membership, cross-root learning, merge/split, retrieval filters, Project page and live Desktop interaction.
