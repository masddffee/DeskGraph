# DeskGraph v0.1 Task Graph

Last updated: 2026-07-16

## Dependency graph

```text
M0 Foundation
├── M1 Manifest Graph
│   ├── M2 Content Intelligence
│   │   └── M3 Hybrid Retrieval
│   │       ├── M4 Project Graph
│   │       │   ├── M6 Watch + Smart Inbox
│   │       │   └── M7 Read-only MCP
│   │       └── M8 Search/Product UI slices
│   └── M5 Safe Organization contracts
│       ├── M6 Watch + Smart Inbox actions
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
6. **M5** — prove preview → validate → durable execute → recover → undo with no delete path.
7. **M6 + M8** — make ingestion continuous and the safe product workflow usable.
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

- Build format providers and bounded queue one format at a time.
- Exit each provider only with corrupt/macro/limit/cancel/provenance fixtures.

### Step 4 — M3 retrieval

- Current: the offline SQLite FTS5 path/content baseline, deterministic explanations, CLI, and Desktop entry points are verified locally; corpus benchmarks and complete filters remain open.
- Next: add reproducible p50/p95/index-size evidence and bounded filters, then audit a vector adapter/provider before semantic or hybrid implementation.
- Exit with no-model deterministic fallback and zh-TW/English evaluation.

### Step 5 — M4 context graph

- Build folder profiles, duplicates/versions, related files, project discovery, provenance, user correction.
- Exit with explainable low-confidence behavior and correction feedback evidence.

### Step 6 — M5 transaction safety

- Build immutable plan and policy validator before executor; implement state machine/journal/recovery/undo before UI execution control.
- Exit only after fault injection, cross-volume, conflict, source-change, process-kill, and idempotent undo pass.

### Step 7 — M6/M8 continuous product workflow

- Build watcher reconciliation/stability/resource controls and Smart Inbox; connect full onboarding/search/preview/history flows.
- Exit with keyboard/accessibility/loading/empty/partial/paused/error states and E2E tests.

### Step 8 — M7 MCP

- Build stdio server over identity-based read services only.
- Exit with no arbitrary paths, no write tools, scope/injection tests, minimal response fields, and setup docs.

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
