# Implementation Status

Last updated: 2026-07-16

Status vocabulary: `not started`, `in progress`, `blocked`, `verified locally`, `verified in CI`, `released`.

## Current milestone

M0 Repository Foundation — **in progress**.

The repository began with planning documents only. The shared privacy-safe health report now runs through the Rust CLI and the complete Tauri IPC → React desktop path. Local M0 implementation is verified; M0 remains in progress because the required macOS/Windows/Linux remote CI evidence does not exist yet.

## Milestones

| Milestone                     | Status      | Current evidence                                                           | Next gate                                             |
| ----------------------------- | ----------- | -------------------------------------------------------------------------- | ----------------------------------------------------- |
| M0 Repository Foundation      | In progress | Local foundation slice, governance, lockfiles, checks, CLI, and desktop smoke verified | Green macOS/Windows/Linux CI matrix |
| M1 Manifest Graph             | Not started | Planning only                                                              | Start `prompts/02_MANIFEST_GRAPH.md` after M0 handoff |
| M2 Content Intelligence       | Not started | Planning only                                                              | Extractor contract and fixtures                       |
| M3 Hybrid Retrieval           | Not started | Planning only                                                              | FTS fallback, vector adapter, fusion, evaluation      |
| M4 Project Graph              | Not started | Planning only                                                              | Explainable project relations and corrections         |
| M5 Safe Organization          | Not started | Safety rules only                                                          | Journaled preview/execute/recover/undo slice          |
| M6 Watch Mode and Smart Inbox | Not started | Planning only                                                              | Stable incremental event slice                        |
| M7 Read-only MCP              | Not started | ADR only                                                                   | Scoped stdio query slice                              |
| M8 Product UI                 | Not started | Planning only                                                              | M0 creates only the shell/status slice                |
| M9 Release Engineering        | Not started | Planning only                                                              | CI foundation, then packages/updater/SBOM             |
| M10 Launch                    | Not started | Copy templates only                                                        | Verified public release first                         |

## M0 acceptance checklist

| Acceptance criterion                                     | Status             | Evidence / blocker                                                         |
| -------------------------------------------------------- | ------------------ | -------------------------------------------------------------------------- |
| Monorepo established                                     | Verified locally   | Rust workspace, pnpm workspace, Tauri/React desktop, CLI, and both lockfiles |
| Rust format, lint, and tests configured                  | Verified locally   | Rust 1.97.0; format and Clippy pass; 9 tests pass                           |
| TypeScript format, lint, typecheck, and tests configured | Verified locally   | Peer check, format, ESLint, TypeScript, 4 Vitest tests, and Vite build pass |
| ADR template                                             | Verified locally   | `docs/architecture/adr/0000-template.md`                                   |
| Root and nested AGENTS instructions                      | Verified locally   | Root plus `apps/desktop/AGENTS.md` and `crates/transactions/AGENTS.md`     |
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
- RustSec reports zero known vulnerabilities but 17 warnings in the all-target lockfile, including unmaintained GTK3 bindings and one `glib` unsound advisory on Tauri's Linux path; tracked as R-016.

## Next handoff

Local work can continue at `prompts/02_MANIFEST_GRAPH.md` while M0 remains open for remote CI evidence. M1 must begin with explicit scope configuration, canonical path policy, SQLite migration design, file identity, exclusions, and a safe metadata-only scan slice.

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
