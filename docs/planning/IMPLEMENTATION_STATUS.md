# Implementation Status

Last updated: 2026-07-16

Status vocabulary: `not started`, `in progress`, `blocked`, `verified locally`, `verified in CI`, `released`.

## Current milestone

M0 Repository Foundation — **in progress**.

The repository began with planning documents only. The active slice is the shared privacy-safe health report across Rust CLI and Tauri desktop.

## Milestones

| Milestone | Status | Current evidence | Next gate |
| --- | --- | --- | --- |
| M0 Repository Foundation | In progress | Assessment and task graph created; implementation and verification pending | All M0 checks plus cross-platform CI |
| M1 Manifest Graph | Not started | Planning only | Start `prompts/02_MANIFEST_GRAPH.md` after M0 handoff |
| M2 Content Intelligence | Not started | Planning only | Extractor contract and fixtures |
| M3 Hybrid Retrieval | Not started | Planning only | FTS fallback, vector adapter, fusion, evaluation |
| M4 Project Graph | Not started | Planning only | Explainable project relations and corrections |
| M5 Safe Organization | Not started | Safety rules only | Journaled preview/execute/recover/undo slice |
| M6 Watch Mode and Smart Inbox | Not started | Planning only | Stable incremental event slice |
| M7 Read-only MCP | Not started | ADR only | Scoped stdio query slice |
| M8 Product UI | Not started | Planning only | M0 creates only the shell/status slice |
| M9 Release Engineering | Not started | Planning only | CI foundation, then packages/updater/SBOM |
| M10 Launch | Not started | Copy templates only | Verified public release first |

## M0 acceptance checklist

| Acceptance criterion | Status | Evidence / blocker |
| --- | --- | --- |
| Monorepo established | In progress | Rust/pnpm/Tauri files pending |
| Rust format, lint, and tests configured | In progress | Toolchain installed; configuration pending |
| TypeScript format, lint, typecheck, and tests configured | In progress | Package install and lockfile pending |
| ADR template | In progress | Pending repository patch |
| Root and nested AGENTS instructions | In progress | Templates exist; nested installation pending |
| CI matrix | In progress | Workflow pending; remote CI unavailable until GitHub setup |
| License and governance | In progress | Pending repository patch |
| Architecture skeleton | In progress | Pending repository patch |
| Fresh clone instructions work | Not started | Requires clean-checkout validation |
| macOS, Windows, Linux CI pass | Blocked externally | No GitHub remote/auth yet |
| Desktop app opens | Not started | Requires implementation and smoke run |
| CLI health works | Not started | Requires implementation and test |
| No model/API key required | Not started | Must be proven by executed health path |
| README labels pre-release | In progress | Pending repository patch |

## Unresolved blockers

- No GitHub repository/remote and invalid GitHub authentication: remote Issues, Releases, and CI results do not exist.
- Signing, notarization, clean Windows/macOS VM validation, and launch accounts are external later-stage requirements.
- Local dependency installation requires network access; resolved lockfiles and audits must be generated before M0 verification.

## Next handoff

After every M0 item is either verified or explicitly external-only, continue at `prompts/02_MANIFEST_GRAPH.md`. M1 must begin with explicit scope configuration, canonical path policy, SQLite migration design, file identity, exclusions, and a safe metadata-only scan slice.
