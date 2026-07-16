# Implementation Status

Last updated: 2026-07-16

Status vocabulary: `not started`, `in progress`, `blocked`, `verified locally`, `verified in CI`, `released`.

## Current milestone

M1 Manifest Graph — **in progress**. M0 Repository Foundation remains open only for external remote CI evidence.

The M1 vertical slice now runs end to end: an explicit folder is canonicalized and persisted as an authorization boundary; a metadata-only scanner records stable File/Folder nodes and `located_in` relations in a checksummed bundled SQLite schema; durable queue/staging tables preserve progress across process exit; CLI and Tauri/React expose create, progress, pause, resume, interrupted recovery, and graph statistics. Local adversarial, permission-denied, crash-reopen, and 10,000-file evidence passes. M1 is not complete because platform-sensitive exclusions, complete Windows runtime fixtures, peak-memory evidence, remote CI, and a live smoke of the new paused/resume UI remain open.

## Milestones

| Milestone                     | Status      | Current evidence                                                           | Next gate                                             |
| ----------------------------- | ----------- | -------------------------------------------------------------------------- | ----------------------------------------------------- |
| M0 Repository Foundation      | In progress | Local foundation slice, governance, lockfiles, checks, CLI, and desktop smoke verified | Green macOS/Windows/Linux CI matrix |
| M1 Manifest Graph             | In progress | Explicit scope → durable bounded queue/staging → atomic SQLite manifest publish → CLI/desktop progress and controls; 10k, permission, recovery, and adversarial local tests | Platform-sensitive exclusions, peak RSS, cross-platform runtime CI |
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
- Windows file-identity adapter compiles for `x86_64-pc-windows-msvc`, but a complete scanner cross-check cannot be produced on this macOS host because bundled SQLite needs a Windows C/MSVC toolchain. Remote Windows CI remains required.
- Local 10k timing and idempotency are measured, but peak RSS sampling was denied by the restricted runner and its escalation reviewer was unavailable due tool quota. This does not block code work; the 8 GB release gate remains open.

## M1 acceptance checklist

| Acceptance criterion | Status | Evidence / blocker |
| --- | --- | --- |
| Explicit persisted scope configuration | Verified locally | CLI and desktop require an existing user-entered directory; authorization never triggers a scan |
| Canonical path and system-root policy | Verified locally | Canonical authorization/rescan revalidation, scope containment on every observation, protected-root denial |
| Symlink/junction/reparse loop defense | Verified locally on Unix; Windows runtime pending | `symlink_metadata` before canonicalization; no following; loop fixture passes; Windows adapter checks reparse attribute |
| Hidden/default-sensitive exclusions | Verified locally | Hidden entries are skipped and recorded; broader platform preset/exclusion UX remains M1/M8 work |
| SQLite migrations documented and safe | Verified locally | Embedded migration, checksum mismatch rejection, WAL, foreign keys, FULL synchronous, reopen test |
| File/Folder nodes and `located_in` relations | Verified locally | Transactional observation upsert and active relation reconciliation |
| Rescan idempotency | Verified locally | Unit fixture and three 10k scans retain exactly 10,101 active nodes/locations |
| Move preserves identity where metadata permits | Verified locally on Unix | Rename and hard-link fixture retains node ID; Windows runtime fixture pending |
| Permission failures are recorded | Verified locally on Unix; Windows runtime pending | Restricted-directory fixture records `permission_denied` without failing the scan; issue remains staged until atomic publish |
| Persistent progress/pause/resume | Verified locally | Durable path queue, job-scoped staging, lease recovery, pause handshake, scope revalidation, replay after database reopen, and atomic publish tests pass |
| CLI graph statistics | Verified locally | `scan create/run/advance/status/list/pause/resume` expose validated progress without logging paths; foreground `scan start` remains available |
| Desktop graph statistics and usable scope UI | Verified locally except latest live smoke | Narrow Rust IPC, validated TypeScript contracts, backend-derived progress polling, paused/interrupted states, production build; live window smoke for the new controls blocked by local tool quota |
| Synthetic 10k generator and benchmark | Verified locally for timing/counts | 10k/100 folder generator; 1.366 s initial and 1.260 s rescan scanner time; peak RSS pending |

## Next handoff

Continue `prompts/02_MANIFEST_GRAPH.md`, not Prompt 03. The next coherent slice is the platform exclusion hardening: protected-system descendants, Windows hidden/system attributes, platform preset boundaries, and adversarial fixtures. Then capture peak RSS and attach macOS/Windows/Linux runtime CI evidence when those environments exist.

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
- 10k benchmark — 10,000 files + 101 folders, 0 issues; initial scanner 1.366 s, rescan 1.260 s; 10,101 active nodes after repeated scans. Peak RSS not captured.
- `cargo audit --no-fetch` — zero known vulnerabilities and the existing 17 tracked warnings.

## M1 durable-scan verification evidence — 2026-07-16

- `cargo fmt --all -- --check` — passed.
- `cargo clippy --workspace --all-targets --all-features --offline -- -D warnings` — passed.
- `cargo test --workspace --all-features --offline` — 34 passed, 0 failed.
- Durable database tests — active pause request is acknowledged between entries; expired `processing` work becomes `interrupted` on database reopen and replays after explicit resume; staged observations remain invisible until one final transaction.
- Scanner tests — bounded batch progress, pause without partial publish, resume to completion, pre-resume scope revalidation, and Unix `permission_denied` isolation pass.
- `pnpm --dir apps/desktop test` — 7 passed, 0 failed; all scan job states and narrow IPC argument contracts validated.
- `pnpm --dir apps/desktop lint`, `typecheck`, and `build` — passed.
- `pnpm tauri build --no-bundle` with the pinned Cargo path — release build passed and produced `target/release/deskgraph-desktop`.
- Live Tauri smoke for the new paused/resume controls — not run; Computer Use launch approval was rejected because the local tool quota was exhausted. The prior M1 manifest UI live smoke remains valid only for the older scope/scan/statistics screen.
