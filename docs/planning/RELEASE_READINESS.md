# Release Readiness

Last reviewed: 2026-07-16

Overall status: **not release-ready**. Local implementation has entered M1 while M0 remote CI evidence remains open.

| Gate                                           | Status             | Evidence required                                        |
| ---------------------------------------------- | ------------------ | -------------------------------------------------------- |
| macOS Apple Silicon package                    | Not started        | Signed/notarized clean-machine install and smoke         |
| macOS Intel or Universal package               | Not started        | Native or Universal clean-machine evidence               |
| Windows x64 installer                          | Not started        | Signed clean-VM install and smoke                        |
| Linux experimental package                     | Not started        | Clearly labeled build and smoke                          |
| Explicit authorized scopes                     | Verified locally   | Desktop/CLI authorization, component-aware protected-tree policy, symlink/reparse and platform hidden/system exclusions; Windows runtime fixtures remain |
| Initial manifest scan                          | In progress        | Release 10k idempotency/timing, durable progress/pause/resume, crash-reopen replay, atomic publish, and Unix permission fixture pass; memory, live updated-UI smoke, and remote CI remain |
| Incremental watch mode                         | Not started        | Reconciliation/stability/restart tests                   |
| Extraction and OCR formats                     | Not started        | Fixture suite, limits, corrupt-file isolation            |
| zh-TW and English                              | Not started        | OCR and retrieval evaluation set                         |
| Metadata/FTS/vector/hybrid retrieval           | Not started        | Deterministic fallback and p50/p95 report                |
| Project/folder/related/duplicate/version graph | Not started        | Provenance, correction, evaluation                       |
| Smart Inbox and explainable classification     | Not started        | UI states and safe suggestion behavior                   |
| Rename/move preview                            | Not started        | Before/after/scope/policy UI and tests                   |
| Journal, crash recovery, undo                  | Not started        | Fault injection and idempotent undo suite                |
| Read-only MCP                                  | Not started        | Scope escape/injection tests and no write tools          |
| 8 GB benchmark                                 | In progress        | M1 timing/count baseline published; release-build peak RSS and documented 8 GB hardware remain |
| Updater pipeline                               | Not started        | Signed metadata dry run and rollback                     |
| SBOM and checksums                             | Not started        | Release-attached, independently verified artifacts       |
| GitHub Release                                 | Blocked externally | Remote, auth, verified assets, public download smoke     |
| README/demo/launch assets                      | In progress        | Pre-release README exists; demo and launch assets remain |
| Post-launch issue/hotfix process               | In progress        | Issue/PR templates exist; labels and incident/hotfix drill remain |
| Critical/high security findings                | In progress        | npm: zero known vulnerabilities; RustSec: zero vulnerabilities plus 17 open warnings; full threat model pending |
| Known data-loss bugs                           | Unknown            | Full action/recovery suite; zero known data-loss issues  |

## M0 readiness gate

The M0 implementation is verified locally. Governance, ADRs, health slice, lockfiles, log-redaction checks, local format/lint/typecheck/test/build, isolated fresh-clone setup, CLI execution, debug app bundle, and live IPC UI smoke have passed. M0 itself remains **in progress** until a GitHub remote exists and the macOS/Windows/Linux matrix is green.

## M1 local readiness note

The resumable scanner is safe to exercise with test folders: progress and pending paths persist in SQLite, live manifest rows are not replaced until a complete scan publishes atomically, pause is acknowledged between entries, expired work reopens as interrupted, and resume revalidates the original canonical authorization boundary. This is not yet a release claim: Windows junction/hidden-attribute runtime evidence, peak RSS, remote CI, and live UI interaction evidence remain open.
