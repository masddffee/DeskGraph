# DeskGraph v0.1 Task Graph

Last updated: 2026-07-21

## OpenAI Build Week 48-hour delivery graph

This is a time-boxed submission path, not a replacement for the production v0.1 graph below.
The submission deadline is 2026-07-22 00:00 UTC (2026-07-22 08:00 Asia/Taipei).
All DeskGraph commits begin inside the competition submission period, so the repository history is
the evidence for work created with Codex and GPT-5.6.

```text
H0 Freeze truthful judge story and acceptance gates
├── H1 Real sample workspace + deterministic backend smoke
├── H2 Desktop judge journey + four-locale product copy
└── H3 README + Devpost copy + three-minute narrated demo script
    └── H4 Integrated format/lint/typecheck/test/build + live golden-path rehearsal
        └── H5 Video/repository URL/session ID/Devpost fields
            └── H6 Owner-approved push, public YouTube upload and final submission
```

### Hackathon critical path

1. **H0 — story:** submit to **Apps for Your Life** as “Graphify your computer without
   uploading it.” Demonstrate one coherent flow: authorize a synthetic local workspace, scan it,
   extract bounded local text, search it with explanations, review Project/duplicate/version/
   screenshot evidence, and create a non-executable Cleanup Preview. Show read-only MCP as the
   agent boundary. Do not claim vector search, executable cleanup, Undo, installers or complete
   cross-platform runtime.
2. **H1 + H2 + H3 — parallel build:** the sample/smoke, Desktop journey and submission package
   use separate file ownership and may proceed together. Every visible result must come from the
   real Rust/SQLite path; no screenshot, fixture or UI may imply a capability that is unavailable.
3. **H4 — merge gate:** run Rust format/Clippy/workspace tests and TypeScript format/lint/
   typecheck/tests/build, then rehearse the sample through the packaged or development Desktop.
4. **H5 — submission gate:** record a public, narrated video under three minutes that covers the
   product plus how Codex and GPT-5.6 were used; include the repository URL, setup/sample-data
   instructions and the `/feedback` session ID.
5. **H6 — external action:** pushing a public repository, uploading to YouTube and submitting the
   Devpost entry require explicit owner approval. Preserve a safety margin before the deadline.

### Hackathon acceptance criteria

- A judge can reproduce the bounded local workflow from a fresh clone with documented commands
  and generated synthetic data; no private user data is required.
- The Desktop first-run journey communicates the problem, local-only boundary and next action in
  English, Traditional Chinese, Simplified Chinese and Japanese.
- The demo never changes or deletes a source file. Rename and Cleanup stay Preview-only.
- The README distinguishes verified local behavior from production v0.1 roadmap claims and
  documents how Codex/GPT-5.6 accelerated the work.
- All local quality gates pass from the exact submitted commit and the working tree is clean.
- Required external URLs and `/feedback` session ID are present before final submission.

### 48-hour ownership and model budget

| Workstream | Dependency | Agent budget | Merge gate |
| --- | --- | --- | --- |
| H1 sample + backend smoke | H0 | Frontier coding model, high reasoning because it crosses Rust/SQLite safety contracts | Scoped Rust format, Clippy and tests |
| H2 Desktop journey | H0 | Balanced coding model, high reasoning for four-locale UX/state integrity | Desktop format, lint, typecheck, tests and build |
| H3 submission package | H0 | Balanced model, medium reasoning; source-backed writing only | Claim audit against implementation status and rules |
| H4 integration/rehearsal | H1-H3 | Primary agent; strongest final reviewer after merge | Full repository gates and real local smoke |
| H5/H6 external delivery | H4 | Owner plus primary agent | Public URL/video/session ID validation and explicit approval |

Rollback is concern-based: revert only the failing sample, UI or documentation commit. The
production graph and safety invariants remain authoritative throughout the sprint.

### Current H status

| Gate | Status | Evidence / next action |
| --- | --- | --- |
| H0 story and gates | Verified locally | Judge story, truthful claim boundary and external actions are frozen in this graph |
| H1 sample and backend smoke | Verified locally | `fixture demo` real child process verifies scan, two extractions, bilingual FTS, Project, duplicate/version, Smart Cleanup and durable non-executable Preview without changing created sources |
| H2 Desktop journey | Verified locally except live rehearsal | Four-locale Home guidance, pre-scan routing and explicit identifier-only document extraction pass Rust/TS tests and builds |
| H3 submission package | Verified locally as draft | README quickstart, Devpost copy, judge instructions and narrated script exist; public URLs and Session ID remain empty |
| H4 integration | Automated gates verified locally | 444 deterministic Rust tests pass with zero failures while the two named macOS live tests whose FSEvents callback is unavailable on this host are explicitly filtered; individual runs receive no callback and do not verify native Watch. `pnpm check` passes 73 Vitest tests plus format/lint/typecheck/build. Run the real Desktop rehearsal before recording |
| H5 submission inputs | External action required | Obtain `/feedback` Session ID, public repository URL and narrated public/unlisted YouTube URL |
| H6 publish and submit | Not started | Requires explicit owner approval after H5 validation |

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
├── M9a Packaged runtime identity foundation
│   ├── M2 Windows OCR runtime evidence
│   └── M5 production action fence
├── Security, fixtures, benchmarks (continuous)
├── Docs/demo (continuous, claims gated by implementation)
└── M9 Remaining release engineering
    └── M10 Launch (only after verified public release)
```

## Critical path

1. **M0** — establish reproducible repository, tests, privacy-safe health slice, CI.
2. **M1** — authorize one explicit Coverage Set, enforce hard exclusions with privacy purge, and create an idempotent, identity-preserving SQLite manifest graph.
3. **M2** — safely extract required formats and OCR into provenance-bearing chunks.
4. **M3** — deliver model-optional multilingual hybrid retrieval with diagnostics.
5. **M4** — create explainable/correctable project, folder, related, duplicate, and version relations.
6. **M9a** — establish verifiable Windows package family identity and macOS App Sandbox scope/container identity before any production action fence or Windows OCR runtime claim; macOS also needs a supported-version protected-container replacement proof or remains unavailable.
7. **M5** — prove preview → validate → durable execute → recover → undo for accepted platform Rename/Move/system-trash operations, with no permanent-delete or empty-trash path. General Unix Rename/Move remains Preview-only under ADR-026.
8. **M6 + M8** — make ingestion continuous and Smart Inbox／Smart Cleanup suggestions, explicit confirmation, history and Undo usable.
9. **M7** — expose only read-only, minimum-necessary, scope-enforced MCP context.
10. **M9** — finish security/performance evidence, installers, signing, updater, SBOM/checksums, and publishing.
11. **M10** — launch only a publicly downloadable, verified build and operate issues/hotfixes.

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

### Step 2 — M1 coverage policy and manifest schema

- Context: begin from `prompts/02_MANIFEST_GRAPH.md`; SQLite/file identity dependencies are unaudited until selected.
- Build: atomic native multi-root authorization, canonical Coverage Set, add-only durable hard exclusions and policy revision, privacy purge, File/Folder graph, scan job state.
- Current: Settings uses Rust-owned native file/folder pickers; a bounded one-shot Preview is revalidated against active grant, host platform, live scope, canonical stable identity/kind and policy revision before a single immediate transaction adds the exclusion, advances revision, purges affected derived data and records a path-free receipt. Public Scanner and Watch source reads require active grant plus shared fence; public root/exclusion mutation auto-acquires or validates a manifest/scope/revision token. The two-second gate-and-data protocol, immutable SQLite lock identities and descriptor-relative Unix/Windows opens are locally verified cooperative-process hardening. Native-unwatch timeout returns committed success, retires callback admission and queued hints before bounded shutdown, reports three exact state booleans and requests restart without overstating OS registration closure. The local workspace gate is green; the read fence is not a hostile-process boundary or ADR-027 action fence, and signed, clean-machine and packaged cross-platform acceptance remain open.
- Tests: all-or-nothing picker/grant persistence; root overlap; symlink/junction/case/Unicode/permission/idempotency; pre-scan exclusion; post-index FTS/OCR/Graph purge; unscanned hard-link identity closure and rescan denial; exact root-revocation Preview, stale impact, tombstone/backfill guards, complete derived purge, fresh-scan reauthorization, reader drain/alias/child crash/lock replacement/reparse, source immutability and native-unwatch disablement. Add exclusion removal, foreign-grant, SQLite page-remnant, hostile/non-cooperating process, cross-platform stress and packaged platform matrices before M1 exit.
- Verify: 10k scan, rescan, move identity, scope escape.
- Rollback: forward-only development migration or documented reversible migration before release.
- Exit: CLI and UI show real graph statistics for confirmed coverage; excluded paths never enter or remain in current manifest/search/MCP/graph/action state, and source files are byte-for-byte unchanged by privacy purge.

### Step 3 — M2 extraction/OCR

- Current: text/Markdown/code, bounded text-layer PDF, allowlisted DOCX/PPTX/XLSX, bounded image header metadata, and macOS arm64 Apple Vision OCR route through durable controlled-source jobs into atomic untrusted SQLite/FTS chunks or structured metadata. Eligible Desktop Search results expose an explicit Screenshot OCR action through scope/node-only create/lookup and job-ID-only run/status/cancel/resume; queued capacity is retried only by explicit user action, interrupted work remains discoverable beyond recent history, the Rust core remains authoritative, and job payloads contain no path or OCR text. Windows `Windows.Media.Ocr` provider code is wired into the same bounded operation without runtime support being claimed. Migration 0016 keeps boxes mandatory and confidence optional. Windows code adds package-identity preflight, requested/resolved language policy, zero-angle source boxes, exact de-duplication, terminal-only close, bounded caller return, and a one-worker cleanup gate. Host policy/state-machine tests and Windows cfg check/Clippy pass; this is not Windows runtime evidence.
- ADR-024 resolves D-008's native-first architecture, and the path-free macOS provider is locally verified through real Vision→SQLite→FTS. D-015 reopens only the fallback implementation. A bounded macOS Vision evidence runner now binds a private manifest/root to exact corpus/image bytes, reuses production OCR validation, and emits evaluator-ready sensitive output; its one-case synthetic outside-sandbox pass is functional evidence, not the bake-off corpus. Next: build equivalent Windows, Tesseract `eng`+`chi_tra`, and PP-OCRv6 small/tiny runners; run real Windows/MSIX identity/language/OCR/cancel/cleanup/RSS fixtures; and compare every provider through one licensed versioned corpus plus identical package/RSS/cancel gates before accepting a runtime. macOS arm64/Intel and Windows x64 block adoption; Linux experimental evidence is tracked separately and cannot delay them. Do not add fallback routing until the selected provider and capability-preflight/failure-policy E2E tests ship together. Keep representative Office/PDF/image/OCR corpora, scanned-PDF routing, and 8 GB residency as separate required evidence.
- Exit each provider only with corrupt/active-content/limit/cancel/provenance fixtures and a usable CLI/Desktop entry point.

### Step 4 — M3 retrieval

- Current: the offline SQLite FTS5 path/content baseline, deterministic explanations, bounded scope/graph-backed-folder/type/date/source filters, CLI/Desktop entry points, and a reproducible release-mode synthetic 10k/50 p50/p95/index-size report are verified locally. Folder selectors require one current eligible location and combine current graph membership with a segment-safe subtree fence; this is not 100k, real-corpus, 8 GB, or cross-platform evidence.
- Next: connect the Project filter only after M4 persists file-membership source of truth and correction state, then add versioned SQLite embedding rows with a bounded exact-search baseline. Rebuild ANN from version-matched embedding rows; only recompute those rows from content hashes plus the exact model manifest. Audit and adopt an ANN provider only if representative vector-count recall@k/result-consistency and p95/RSS/build/update evidence shows a net release-budget benefit, with atomic model-version invalidation.
- Exit with no-model deterministic fallback and zh-TW/English evaluation.

### Step 5 — M4 context graph

- Current: ADR-018 through ADR-028 provide bounded Folder Profiles, stable correctable Project roots, full-byte exact-duplicate suggestions with reverified exact-pair feedback, explicit numeric filename-version suggestions with evidence-bound directional correction, and one explicit low-confidence screenshot-review source. ADR-032 adds an explicit, bounded, manifest-only Project Discovery page: path-free lists evaluate at most 100 roots, one current path appears only in transient review, and accept/reject revalidates evidence without creating membership or actions. Screenshot grouping requires an active platform grant plus current M2 image metadata and completed OCR-provider provenance; source selection through immutable write shares one immediate transaction, unchanged evidence is idempotent, and member/provenance/grant changes fail closed. ADR-029 derives a bounded, explicit-refresh, path-free Smart Cleanup Inbox from current suggested sources: duplicate/version evidence live-reverifies, screenshot membership binds its current immutable observation, relation feedback is not cleanup consent, and CLI/four-language Desktop expose no action capability. It never reads OCR text merely for Inbox aggregation or claims keeper, disposability, reclaimable space, cleanup authorization, or a file action.
- Next: add deterministic related candidates and broader evaluated screenshot-origin evidence, then group correction. Time proximity, filename similarity or model confidence alone cannot prove screenshot origin or disposability. Background Project/duplicate discovery, pagination beyond 100 roots and larger-file hashing need separate bounded designs. Resolve D-013 before cross-root learned scoring and D-016 before merge/split; add file-membership correction and retrieval filters only after those source-of-truth contracts pass. Exact duplicates, evidence-backed versions and screenshot groups may feed Smart Cleanup suggestions only; none authorizes a file action.
- Exit with explainable low-confidence behavior and correction feedback evidence.

### Step 5.5 — M9a packaged runtime identity foundation

- Context: ADR-027 makes a verifiable OS package/container identity a prerequisite for the action process fence; Windows native OCR already depends on package identity. This is a narrow dependency inversion, not early completion of M9.
- Current local foundation: the Rust-owned native picker accepts no WebView path, atomically stores the canonical scope plus an opaque platform grant, restores macOS security-scoped bookmarks into balanced live guards, refreshes stale grants, durably downgrades legacy/corrupt/unrestorable rows to `needs_reauthorization`, and intersects packaged Watch eligibility with both the durable active grant and live runtime registry. General IPC, histories and aggregates hide inactive grants and never expose the BLOB. Debug development receipts are rejected in release builds; Windows/Linux release authorization fails closed. The checked-in macOS profile contains only App Sandbox, user-selected read/write, app-scoped bookmarks, and the outbound client entitlement required by the current-host WebKit NetworkProcess A/B; `network.server` is absent and production CSP excludes development origins. This is configuration/development evidence only, not permission to upload local data or a signed release claim.
- Next build: sign the macOS App Sandbox entitlements with the accepted bundle identity and selected OS floor, prove clean-machine bookmark lifecycle, no product-data egress and the SIP-protected container candidate, then implement a Windows packaged identity shared by OCR and the future protected private-namespace fence. D-003 must select full MSIX/Store or an explicitly reviewed alternative; MSI/NSIS alone does not satisfy package-family identity. Preserve the current local-first, network-independent core and explicit-scope contracts.
- Tests: release rejection of development/unavailable grants; command-level denial for missing live access; bookmark restore/revocation/stale refresh and scope escape; a non-entitled same-user macOS fence-entry replacement probe without user-authorized exception; package-family stability; path-free failures; installer update/repair/uninstall behavior; and clean-machine macOS arm64/Intel-or-Universal plus Windows x64 evidence.
- Exit: the packaged app can prove its platform identity and explicitly authorized scope without enabling Execute, recovery, Undo, or adding a helper/daemon. Only then may Step 6 implement the process fence.

### Step 6 — M5 transaction safety

- Current: ADR-017 plus ADR-025's protocol foundation bind a same-folder scanned-file preview to canonical scope, manifest/metadata/platform/open-handle identity, portable name, conflict policy and bounded SHA-256/root/parent/source evidence. Migration 0019 supplies immutable request receipts, a closed append-only command/recovery state machine and lease coordination. ADR-026 resolves D-018 by rejecting general Unix Rename/Move execution: a deterministic last-check counterexample proves the macOS/Linux no-replace pathname syscall can move a replacement inode. Every production adapter fails closed before database/journal/mutation side effects; CLI exposes explicit before/after Preview, Status and path-free History, while Desktop exposes Preview and path-free History only.
- ADR-027 resolves D-019's identity-first architecture without claiming a runtime: v0.1 keeps one Tauri Rust action host; Windows requires package family identity plus a protected private namespace/named mutex. macOS `flock` is only a candidate after signed App Sandbox scope handoff, a selected OS floor, and proof that the protected container rejects a non-entitled same-user fence-entry replacement; otherwise it remains unavailable. No generic AppData lock or single-instance plugin is accepted.
- Next: complete Step 5.5, then implement the two platform fences and prove fail-before-database ordering plus pause/kill/fork/exec/handle/namespace/installer behavior. Independently implement and run the Windows exact-handle Rename adapter/fault matrix; general Unix remains Preview-only unless a future OS primitive or managed-namespace ADR supersedes ADR-026. Then add Move/cross-volume planning and action-bound platform-trash adapters with immediate revalidation, destination hash or exact trash receipt/identity proof, startup recovery and idempotent Undo. Resolve D-017 before exposing any executable trash action.
- Exit only after fault injection, cross-volume, conflict, source-change, process-kill, platform-trash recovery and idempotent undo pass, with no permanent-delete or empty-trash capability.

### Step 7 — M6/M8 continuous product workflow

- Current: ADR-016 and the durable untrusted-hint → per-scope debounce → stability/open-handle identity → atomically linked manifest reconcile core pass locally, including rename and two restart states; CLI/Desktop status is a path-free v2 contract that distinguishes native-plus-periodic operation from periodic-only degradation. The complete currently implemented Desktop surface also has typed in-bundle English/Traditional Chinese catalogs, safe first-launch locale detection, an always-available keyboard-accessible selector, explicit local preference persistence, and packaged macOS arm64 switch/restart evidence without a new network source or Tauri permission.
- Current event path: exact audited `notify 8.2.0` feeds macOS FSEvents, Windows `ReadDirectoryChangesW` and Linux inotify into one bounded non-blocking source; callbacks never touch SQLite or the filesystem. Watch-set changes, overflow, `need_rescan`, source failure or unmatched paths force whole-scope recovery; a second distinct path in one logical scope requests per-scope recovery, so ordered temporary→final renames cannot silently lose the final path. A five-minute periodic reconciliation remains enabled. The original event age also caps continuous coalescing at five minutes through a canonical root-only durable metadata transaction that rebinds exact authorization and completed-scan eligibility. Recovery requested during a multi-batch scan remains pending and forces a fresh root scan after the old snapshot finishes. Normal hints keep the stability gate; direct ignored observations cannot cancel unrelated stabilizing work and explicit ignored transitions merge into bounded operational aggregates. No content extraction or filesystem action is authorized. The Tauri core owns a single in-process metadata-scan gate and deadline-driven coordinator; extraction/OCR cancellation and other durable engines do not acquire that mutex, although SQLite-level contention remains to be benchmarked. Only completed-scan scopes are watchable; packaged Desktop additionally intersects the durable active grant with the live runtime scope guard before registration or reconciliation. Each cycle drains at most 64 native signals, schedules at most four due scopes, advances one full-scope reconciliation batch, preserves original due order for backlog-first fairness, and backs active-scan contention off for one second. macOS arm64 live create/modify/rename/delete previously passed with direct metadata verification; deterministic temporary→final fixtures pass, while the expanded live temp→final host run is pending after external execution quota denial. Windows/Linux/macOS Intel runtime remains open. Next: replace routine full scans after a hint with efficient per-node reconciliation and connect explicit incremental extraction/indexing. Add tray/autostart behavior and measured pause/battery/thermal/8 GB controls before claiming background Watch. Revisit a per-user writer daemon under D-014 only if UI-closed acceptance proves the current topology insufficient. Then add separate suggest-only Smart Inbox and Smart Cleanup Inbox state models plus full onboarding/search/review/preview/history flows. Cleanup sources are limited to current exact duplicates, evidence-backed older versions and explainable screenshot groups; all confirmed actions delegate to M5.
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

## Inserted local authorization-fence checkpoint — 2026-07-21

- Closed local implementation: all public Scanner source reads require active grant plus shared fence; root revocation/hard exclusion require automatic or scope/revision-bound fence; immutable lock identity registry and two-second gate-and-data admission prevent cooperating-process split-lock and unbounded drain.
- Acceptance evidence: 437 deterministic Rust tests pass with zero failures and only two FSEvents-host tests filtered; the two live tests do not pass on this host and therefore do not validate native Watch. `pnpm check` passes 73 tests plus format/lint/typecheck/build.
- Still on the critical path: signed macOS, Windows package/runtime, hostile-process, clean-machine, cross-platform race/stress and complete Watch/installer/release evidence.
