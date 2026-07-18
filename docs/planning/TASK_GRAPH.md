# DeskGraph v0.1 Task Graph

Last updated: 2026-07-18

## Dependency graph

```text
M0 Foundation
├── M1 Manifest Graph
│   ├── M2 Content Intelligence
│   │   └── M3 Hybrid Retrieval
│   │       ├── M4 Project Graph
│   │       │   ├── M6 Watch + Smart Inbox / Cleanup evidence
│   │       │   └── M7 Read-only MCP
│   │       └── M8 Search/Product UI slices
│   └── M5 Safe Organization contracts
│       ├── M6 Smart Cleanup system-trash actions
│       ├── M7 preview_organization_plan
│       └── M8 Preview + History/Undo UI
├── Security, fixtures, benchmarks (continuous)
├── Docs/demo (continuous, claims gated by implementation)
└── M9 Release Engineering foundation
    └── M10 Launch (only after verified public release)
```

## Critical path

1. **M0** — establish reproducible repository, tests, privacy-safe health slice, CI.
2. **M1** — authorize scopes and create an idempotent, identity-preserving SQLite manifest graph.
3. **M2** — safely extract required formats and OCR into provenance-bearing chunks.
4. **M3** — deliver model-optional multilingual hybrid retrieval with diagnostics.
5. **M4** — create explainable/correctable project, folder, related, duplicate, and version relations.
6. **M5** — prove preview → validate → durable execute → recover → undo for Move, Rename and system-trash moves, with no permanent-delete or empty-trash path.
7. **M6 + M8** — make ingestion continuous and Smart Inbox／Smart Cleanup suggestions, explicit confirmation, history and Undo usable.
8. **M7** — expose only read-only, minimum-necessary, scope-enforced MCP context.
9. **M9** — measure security/performance, package, sign, verify, generate SBOM/checksums/updater, publish.
10. **M10** — launch only a publicly downloadable, verified build and operate issues/hotfixes.

## Parallel workstreams

| Workstream                                       | Can start | Must not claim complete before                  |
| ------------------------------------------------ | --------- | ----------------------------------------------- |
| Governance, ADRs, threat model, dependency audit | M0        | Evidence and review for each milestone          |
| Synthetic/adversarial fixture tooling            | M0/M1     | Relevant integration tests consume fixtures     |
| Desktop shell/accessibility/state patterns       | M0        | Backend source-of-truth behavior exists         |
| Release CI/SBOM/checksum scaffolding             | M0        | Real cross-platform assets are verified         |
| README, diagrams, demo scripts                   | M0        | Claims reflect the current build                |
| Transaction state-machine design/fault harness   | After M0  | M1 identity/scope primitives integrate          |
| Provider/model evaluation                        | After M0  | License/checksum/memory/package evidence exists |

## Construction steps

Each step should fit a logical commit/PR, preserve a buildable default branch, include tests/docs, and state rollback.

### Step 0 — Assessment and SSOT

- Context: repository contains planning only.
- Output: project context, assessment, task graph, six required status files.
- Verify: all baseline claims match filesystem/Git/tool evidence.
- Exit: M0 slice and blockers are explicit.

### Step 1 — M0 health vertical slice

- Context: no implementation exists; health must be useful without DB/models.
- Build: Rust workspace, shared health schema, CLI JSON command, Tauri command, React status UI, privacy-safe logging.
- Tests: schema/serialization/privacy tests, frontend formatter/state tests, CLI log redaction assertions, and the exact Tauri invoke contract.
- Verify: Rust/TS format, lint, typecheck, unit tests, builds, CLI run, desktop runtime smoke showing the Rust → Tauri IPC → React success state, and logs containing no path/content/user data.
- Rollback: remove the new workspace/app files; planning baseline remains intact.
- Exit: local implementation is verified and every acceptance item has evidence or an explicit external blocker; M0 stays in progress until the macOS/Windows/Linux remote matrix is green.

### Step 2 — M1 scope and manifest schema

- Context: begin from `prompts/02_MANIFEST_GRAPH.md`; SQLite/file identity dependencies are unaudited until selected.
- Build: scope allowlist, canonical policy, exclusions, migrations, File/Folder graph, scan job state.
- Tests: symlink/junction/case/Unicode/permission/idempotency fixtures.
- Verify: 10k scan, rescan, move identity, scope escape.
- Rollback: forward-only development migration or documented reversible migration before release.
- Exit: CLI and UI show real graph statistics for authorized scopes.

### Step 3 — M2 extraction/OCR

- Current: text/Markdown/code, bounded text-layer PDF, allowlisted DOCX/PPTX/XLSX, bounded image header metadata, and macOS arm64 Apple Vision OCR route through durable controlled-source jobs into atomic untrusted SQLite/FTS chunks or structured metadata. Eligible Desktop Search results expose an explicit Screenshot OCR action through scope/node-only create/lookup and job-ID-only run/status/cancel/resume; queued capacity is retried only by explicit user action, interrupted work remains discoverable beyond recent history, the Rust core remains authoritative, and job payloads contain no path or OCR text. Windows `Windows.Media.Ocr` provider code is wired into the same bounded operation without runtime support being claimed. Migration 0016 keeps boxes mandatory and confidence optional. Windows code adds package-identity preflight, requested/resolved language policy, zero-angle source boxes, exact de-duplication, terminal-only close, bounded caller return, and a one-worker cleanup gate. Host policy/state-machine tests and Windows cfg check/Clippy pass; this is not Windows runtime evidence.
- ADR-024 resolves D-008's native-first architecture, and the path-free macOS provider is locally verified through real Vision→SQLite→FTS. D-015 reopens only the fallback implementation. A bounded macOS Vision evidence runner now binds a private manifest/root to exact corpus/image bytes, reuses production OCR validation, and emits evaluator-ready sensitive output; its one-case synthetic outside-sandbox pass is functional evidence, not the bake-off corpus. Next: build equivalent Windows, Tesseract `eng`+`chi_tra`, and PP-OCRv6 small/tiny runners; run real Windows/MSIX identity/language/OCR/cancel/cleanup/RSS fixtures; and compare every provider through one licensed versioned corpus plus identical package/RSS/cancel gates before accepting a runtime. macOS arm64/Intel and Windows x64 block adoption; Linux experimental evidence is tracked separately and cannot delay them. Do not add fallback routing until the selected provider and capability-preflight/failure-policy E2E tests ship together. Keep representative Office/PDF/image/OCR corpora, scanned-PDF routing, and 8 GB residency as separate required evidence.
- Exit each provider only with corrupt/active-content/limit/cancel/provenance fixtures and a usable CLI/Desktop entry point.

### Step 4 — M3 retrieval

- Current: the offline SQLite FTS5 path/content baseline, deterministic explanations, bounded scope/type/date/source filters, CLI/Desktop entry points, and a reproducible synthetic 10k p50/p95/index-size report are verified locally; this is not 100k, real-corpus, 8 GB, or cross-platform evidence.
- Next: connect project/folder filters only after M4 persists their source-of-truth identities/correction state, then add versioned SQLite embedding rows with a bounded exact-search baseline. Rebuild ANN from version-matched embedding rows; only recompute those rows from content hashes plus the exact model manifest. Audit and adopt an ANN provider only if representative vector-count recall@k/result-consistency and p95/RSS/build/update evidence shows a net release-budget benefit, with atomic model-version invalidation.
- Exit with no-model deterministic fallback and zh-TW/English evaluation.

### Step 5 — M4 context graph

- Current: ADR-018 through ADR-023 provide bounded Folder Profiles, stable correctable Project roots, full-byte exact-duplicate suggestions with reverified exact-pair feedback, and explicit numeric filename-version suggestions with evidence-bound directional correction. Version decisions revalidate current files, preserve append-only provenance, and return changed directional evidence to `suggested`; none reads content for ordering or creates a file action.
- Next: add deterministic related candidates and explainable screenshot groups with provenance/current-data invalidation and evaluation. Screenshot grouping depends on current M2 image metadata and any used OCR/provider provenance; time proximity, filename similarity or model confidence alone cannot prove disposability. Background duplicate discovery and larger-file hashing need a separate bounded design. Resolve D-013 before cross-root learned scoring and D-016 before merge/split; add file-membership correction, retrieval filters, and a backend-owned Project page only after those source-of-truth contracts pass. Exact duplicates, evidence-backed versions and screenshot groups may feed Smart Cleanup suggestions only; none authorizes a file action.
- Exit with explainable low-confidence behavior and correction feedback evidence.

### Step 6 — M5 transaction safety

- Current: ADR-017 plus ADR-025's protocol foundation bind a same-folder scanned-file preview to canonical scope, manifest/metadata/platform/open-handle identity, portable name, conflict policy and bounded SHA-256/root/parent/source evidence. Migration 0019 supplies immutable request receipts, a closed append-only command/recovery state machine and lease coordination. ADR-026 resolves D-018 by rejecting general Unix Rename/Move execution: a deterministic last-check counterexample proves the macOS/Linux no-replace pathname syscall can move a replacement inode. Every production adapter fails closed before database/journal/mutation side effects; CLI exposes explicit before/after Preview, Status and path-free History, while Desktop exposes Preview and path-free History only.
- Next: resolve D-019 and prove packaged-private process fencing plus child-process pause/kill/descriptor/replacement behavior. Independently implement and run the Windows exact-handle Rename adapter/fault matrix; general Unix remains Preview-only unless a future OS primitive or managed-namespace ADR supersedes ADR-026. Then add Move/cross-volume planning and action-bound platform-trash adapters with immediate revalidation, destination hash or exact trash receipt/identity proof, startup recovery and idempotent Undo. Resolve D-017 before exposing any executable trash action.
- Exit only after fault injection, cross-volume, conflict, source-change, process-kill, platform-trash recovery and idempotent undo pass, with no permanent-delete or empty-trash capability.

### Step 7 — M6/M8 continuous product workflow

- Current: ADR-016 and the durable untrusted-hint → per-scope debounce → stability/open-handle identity → atomically linked manifest reconcile core pass locally, including rename and two restart states; CLI/Desktop status is a path-free v2 contract that distinguishes native-plus-periodic operation from periodic-only degradation. The complete currently implemented Desktop surface also has typed in-bundle English/Traditional Chinese catalogs, safe first-launch locale detection, an always-available keyboard-accessible selector, explicit local preference persistence, and packaged macOS arm64 switch/restart evidence without a new network source or Tauri permission.
- Current event path: exact audited `notify 8.2.0` feeds macOS FSEvents, Windows `ReadDirectoryChangesW` and Linux inotify into one bounded non-blocking source; callbacks never touch SQLite or the filesystem. Watch-set changes, overflow, `need_rescan`, source failure or unmatched paths force whole-scope recovery; a second distinct path in one logical scope requests per-scope recovery, so ordered temporary→final renames cannot silently lose the final path. A five-minute periodic reconciliation remains enabled. The original event age also caps continuous coalescing at five minutes through a canonical root-only durable metadata transaction that rebinds exact authorization and completed-scan eligibility. Recovery requested during a multi-batch scan remains pending and forces a fresh root scan after the old snapshot finishes. Normal hints keep the stability gate; direct ignored observations cannot cancel unrelated stabilizing work and explicit ignored transitions merge into bounded operational aggregates. No content extraction or filesystem action is authorized. The Tauri core owns a single in-process metadata-scan gate and deadline-driven coordinator; extraction/OCR cancellation and other durable engines do not acquire that mutex, although SQLite-level contention remains to be benchmarked. Only completed-scan scopes are watchable. Each cycle drains at most 64 native signals, schedules at most four due scopes, advances one full-scope reconciliation batch, preserves original due order for backlog-first fairness, and backs active-scan contention off for one second. macOS arm64 live create/modify/rename/delete previously passed with direct metadata verification; deterministic temporary→final fixtures pass, while the expanded live temp→final host run is pending after external execution quota denial. Windows/Linux/macOS Intel runtime remains open. Next: replace routine full scans after a hint with efficient per-node reconciliation and connect explicit incremental extraction/indexing. Add tray/autostart behavior and measured pause/battery/thermal/8 GB controls before claiming background Watch. Revisit a per-user writer daemon under D-014 only if UI-closed acceptance proves the current topology insufficient. Then add separate suggest-only Smart Inbox and Smart Cleanup Inbox state models plus full onboarding/search/review/preview/history flows. Cleanup sources are limited to current exact duplicates, evidence-backed older versions and explainable screenshot groups; all confirmed actions delegate to M5.
- Exit only after event/load storms, temporary downloads, rename/move reconciliation, restart, low-memory/8 GB, macOS and Windows required runtime, separately labeled Linux experimental evidence that cannot delay them, keyboard/accessibility/loading/empty/partial/paused/error states, explainable cleanup review/selection/confirmation/history/Undo, stale-evidence invalidation, external-empty handling, and no-file-action-by-default E2E acceptance pass.

### Step 8 — M7 MCP

- Current: an independently launched stdio server exposes exactly one read-only `search_files` tool over an existing-schema SQLite read-only/query-only capability. Launch-time scope grants must be positive, present, and backed by a completed scan; every call revalidates scope eligibility. Closed input schemas accept no paths, content snippets are explicit and labeled untrusted, protocol and response sizes are bounded, structured stderr omits query/path/content, and real child-process lifecycle/scope/injection/main-database-and-source-immutability tests pass. SQLite's narrow WAL/SHM coordination-sidecar exception is documented. The exact bundled macOS/Linux VFS plus DeskGraph link checks satisfy the current no-follow proof; Windows and other unproven targets fail closed. It does not require daemon IPC or acquire a writer lease.
- Next: add only the remaining minimum-necessary read tools after their source milestones are truthful, then package and smoke supported clients/platforms. Keep remote transport, write tools, scan/extraction triggers, filesystem actions, and model execution out of M7.
- Exit remains open until the accepted M7 tool set has complete scope/injection/minimum-field tests, cross-platform stdio/client setup evidence, packaged documentation, and no arbitrary paths or write tools.

### Step 9 — M9 release gate

- Run 10k/100k and 8 GB evidence, security/advisory/license audits, SBOM/checksums, installer/updater/signing and clean-machine smoke.
- Exit only when every release gate is green or an honest known limitation is explicitly allowed by the accepted release policy.

### Step 10 — M10 launch and operate

- Publish GitHub v0.1.0 and verify public install before posts.
- Run documented issue triage, installer/data-safety priority, and hotfix process.

## Plan mutation protocol

- New safety work may be inserted immediately and must declare dependencies and acceptance evidence.
- A step may split when it cannot remain buildable/reviewable as one change.
- Reordering is allowed only when no dependency or invariant is bypassed.
- Deferred Version C work must stay out of the critical path and be recorded rather than partially advertised.
- Any scope reduction affecting Version B requires an explicit user decision and planning/status update.
