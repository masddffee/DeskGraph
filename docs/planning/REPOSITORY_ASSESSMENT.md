# Repository Assessment

Assessment date: 2026-07-16 (Asia/Taipei)

## Executive finding

DeskGraph began this assessment as an uninitialized project-planning corpus, not an implementation repository. It contained product, architecture, milestone, security, release, launch, and phase-prompt documents, but no Git metadata, source code, package manifests, lockfiles, tests, CI, build configuration, installers, Issues, or Releases.

The correct starting point is M0 Repository Foundation. No later milestone can be credited from planning text alone.

## Evidence captured before implementation

| Area | Observed state | Assessment |
| --- | --- | --- |
| Repository contents | 31 analyzable files: 30 Markdown documents and one JSON manifest, approximately 7,390 words | Planning corpus only |
| Git | No `.git` directory; `git status` returned “not a git repository” | No history, branch, tags, or commits |
| Remote | No Git configuration | No authoritative GitHub repository can be identified |
| GitHub auth | Existing `gh` account token is invalid | Issues, PRs, CI runs, and Releases cannot be queried or created |
| Source code | No `src/`, `apps/`, `crates/`, or `packages/` implementation directories | M0–M10 implementation absent |
| Rust | `rustc` and `cargo` initially absent; official `rustup-init` was downloaded and SHA-256 verified during M0 preparation | Toolchain setup was required before validation |
| Node | Node.js 24.12.0 and npm 11.6.2 available | Satisfies current Vite 8 and ESLint 10 runtime requirements |
| Package manager | Corepack available; no pinned pnpm or lockfile | Must pin and install pnpm |
| Tests | No test files or harness | No behavioral evidence |
| CI | No `.github/workflows/` | No platform evidence |
| Build/release | No Cargo, Tauri, npm, installer, updater, SBOM, or checksum configuration | Not releasable |
| Governance | Root safety instructions and two nested-instruction templates existed at repository root | Nested files were not yet installed in their intended scopes |
| Planning status | Required status, risk, dependency, decision, readiness, and external-action files absent | Must be created in M0 |
| Hygiene | `.DS_Store` files present; no `.gitignore` | Must prevent local artifacts from entering history |

## Specification integrity observations

- `docs/planning/MANIFEST.json` references `prompts/00_MASTER_ORCHESTRATOR.md`, but that file is absent. The requested execution starts at `prompts/01_FOUNDATION.md`, so this does not block M0; the inconsistency remains recorded.
- `apps-desktop.AGENTS.md` and `crates-transactions.AGENTS.md` are instruction templates at repository root. Their intended nested locations did not exist.
- The accepted ADR set is `docs/planning/09_DECISIONS_ADR.md` (ADR-001 through ADR-007). Its decisions are binding: SQLite, optional LLM, no delete, read-only MCP, graph-as-infrastructure, post-install models, and provider interfaces.
- The phase prompts and milestone numbering intentionally differ after M7: desktop UX is milestone M8 while Phase 08 is its execution prompt; optional local AI, benchmark/security, packaging, docs, release, and launch are cross-cutting release phases. The task graph preserves milestone names as the status SSOT and uses phase prompts as execution gates.

## Safety posture at baseline

There was no executable data-loss path because there was no executable product. That is not evidence that safety controls exist. Scope validation, path canonicalization, symlink/junction defenses, transaction durability, crash recovery, undo, MCP scope enforcement, model verification, and updater verification were all unimplemented.

## M0 Acceptance Criteria — baseline and target evidence

| Criterion | Baseline | Required evidence before M0 can be marked complete |
| --- | --- | --- |
| Fresh clone instructions work | Not met | Follow README setup from a clean checkout |
| CI passes on macOS, Windows, Linux | Not met | Green GitHub Actions matrix on all three OS families |
| Desktop app opens | Not met | Local launch smoke plus CI build; later clean-machine smoke |
| CLI health command works | Not met | Executed command and assertions for privacy-safe JSON |
| No model or API key required | Unproven | CLI and desktop health path succeeds with providers disabled |
| README labels project pre-release | Not met | README visible and accurate |

## Milestone mapping at baseline

| Milestone | Baseline status | Evidence |
| --- | --- | --- |
| M0 Repository Foundation | Not started | No repository/tooling/app/CLI |
| M1 Manifest Graph | Not started | No scope, scanner, identity, DB, or graph |
| M2 Content Intelligence | Not started | No extractor or OCR contracts |
| M3 Hybrid Retrieval | Not started | No FTS/vector/fusion/search |
| M4 Project Graph | Not started | No profiles, relations, clustering, or correction flow |
| M5 Safe Organization | Not started | No planner, validator, transaction journal, executor, recovery, or undo |
| M6 Watch Mode and Smart Inbox | Not started | No watcher, reconciliation, stability, or inbox |
| M7 MCP | Not started | No server or scoped read-only tools |
| M8 Product UI | Not started | No desktop application |
| M9 Release Engineering | Not started | No packages, signing, updater, SBOM, or release workflow |
| M10 Launch | Not started | No public download, demo, launch assets, or operations |

## First vertical slice

M0 will implement one coherent, user-visible and testable path:

1. a shared Rust health-report contract with no filesystem paths;
2. a CLI `deskgraph health` command that emits structured JSON;
3. a narrowly scoped Tauri command exposing the same report;
4. a React pre-release status screen with loading, success, and error states;
5. unit tests in Rust and TypeScript plus format, lint, typecheck, build, and CI configuration.

The database and providers are reported honestly as not initialized or disabled. The slice does not pretend M1 functionality exists.

## Assessment conclusion

DeskGraph has a strong safety-oriented specification but zero implementation evidence at baseline. M0 must create the repository contract, runnable health slice, governance, CI, and verification foundation without weakening any invariant. Only then can `prompts/02_MANIFEST_GRAPH.md` begin.
