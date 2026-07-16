# Implementation Status

Last updated: 2026-07-16

Status vocabulary: `not started`, `in progress`, `blocked`, `verified locally`, `verified in CI`, `released`.

## Current milestone

M2 Content Intelligence — **in progress**. M0 remains open for remote CI evidence, and M1 remains open for complete Windows runtime, peak-memory, and latest live-UI evidence.

The first M2 vertical slice now runs end to end without a new third-party parser: an explicit already-scanned text, Markdown, or source-code file is resolved inside its authorized scope; the core revalidates the canonical scope, exclusion policy, manifest snapshot, and open-file identity; a bounded UTF-8 provider receives only a controlled `Read + Seek`; durable SQLite jobs support cancellation and interrupted recovery; complete untrusted chunks atomically replace prior active content; CLI and Desktop expose privacy-safe progress and counts. Invalid UTF-8, changed files, symlink swaps, invalid limits, cancellation, expired leases, false output sizes, and failed replacement are covered locally. M2 is not complete because PDF, DOCX, PPTX, XLSX, image metadata, OCR, every-format corrupt/macro fixtures, full Windows runtime evidence, and an extraction benchmark on 8 GB hardware remain open.

## Milestones

| Milestone                     | Status      | Current evidence                                                           | Next gate                                             |
| ----------------------------- | ----------- | -------------------------------------------------------------------------- | ----------------------------------------------------- |
| M0 Repository Foundation      | In progress | Local foundation slice, governance, lockfiles, checks, CLI, and desktop smoke verified | Green macOS/Windows/Linux CI matrix |
| M1 Manifest Graph             | In progress | Explicit scope → durable bounded queue/staging → atomic SQLite manifest publish → CLI/desktop progress and controls; 10k, permission, recovery, protected-tree, and adversarial local tests | Peak RSS, latest live UI smoke, cross-platform runtime CI |
| M2 Content Intelligence       | In progress | Bounded provider → durable job → open-file identity revalidation → atomic untrusted chunks → CLI/Desktop status; text/Markdown/code and adversarial fixtures pass locally | Audit and implement PDF, Office, image metadata, and OCR providers one at a time |
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
| Rust format, lint, and tests configured                  | Verified locally   | Rust 1.97.0; current workspace format and Clippy pass; 63 tests pass        |
| TypeScript format, lint, typecheck, and tests configured | Verified locally   | Format, ESLint, TypeScript, 10 Vitest tests, and Vite build pass            |
| ADR template                                             | Verified locally   | `docs/architecture/adr/0000-template.md`                                   |
| Root and nested AGENTS instructions                      | Verified locally   | Root plus Desktop, scanner, extractor, and transaction safety instructions |
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
| Bounded resource policy | Verified locally for built-in provider | Defaults: 4 MiB source, 8 MiB stored output, 2,048 chunks, 5 seconds; absolute caps: 64 MiB source/output, 65,536 chunks, 64 KiB/chunk, 60 seconds; database independently validates output totals |
| Atomic content publication and prior-version safety | Verified locally | Per-file transaction deactivates prior chunks only after all chunks validate; failure/cancellation preserves the prior complete version; source changes invalidate stale content |
| Provenance, offsets, and untrusted classification | Verified locally for byte-oriented provider | Every chunk records scope/node/location/job, provider/version, source byte range/snapshot, ordinal, and fixed `untrusted_extracted_text` trust class |
| Per-file error isolation | Verified locally for text slice | Invalid UTF-8 and invalid limits produce fixed failed-job codes without aborting the process or publishing partial chunks |
| Privacy-safe usable CLI | Verified locally | `extract start/create/run/status/list/cancel/resume/stats`; explicit `--path` resolves only an existing scanned node; binary test proves path, filename, and extracted text do not enter stdout/stderr |
| Desktop extraction status | Verified locally except live smoke | Narrow read-only Tauri IPC, runtime-validated TypeScript schemas, empty/success/failure/cancel/interrupted labels, 10 frontend tests, Vite and Tauri release builds; latest window interaction not run |
| PDF text | Not started | Parser/API/license/security audit and valid/corrupt/action/attachment/limit/cancel/provenance fixtures required |
| DOCX / PPTX / XLSX | Not started | ZIP/XML dependency audit plus traversal, decompression, macro, external-link, embedded-object, limit, cancel, and provenance fixtures required |
| Image metadata | Not started | Bounded signature/metadata provider and corrupt/oversized fixtures required |
| Screenshot OCR with zh-TW and English | Not started | D-008 remains open; native and packaged fallback candidates require official API/platform/license/memory evaluation; no Python requirement allowed |

## Next handoff

Continue `prompts/03_EXTRACTORS_OCR.md`. Keep M1 evidence closure as a parallel release workstream: Windows junction/hidden-attribute runtime fixtures, peak RSS on an unrestricted 8 GB machine, latest desktop interaction smoke, and remote macOS/Windows/Linux CI. The next M2 format provider must not be selected until its official API, maintenance, platform support, license, packaging, and security limits are recorded in `DEPENDENCY_AUDIT.md`.

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
