# Release Readiness

Last reviewed: 2026-07-16

Overall status: **not release-ready**. The project is in M0 foundation work.

| Gate | Status | Evidence required |
| --- | --- | --- |
| macOS Apple Silicon package | Not started | Signed/notarized clean-machine install and smoke |
| macOS Intel or Universal package | Not started | Native or Universal clean-machine evidence |
| Windows x64 installer | Not started | Signed clean-VM install and smoke |
| Linux experimental package | Not started | Clearly labeled build and smoke |
| Explicit authorized scopes | Not started | Scope UX, canonical policy, adversarial tests |
| Initial manifest scan | Not started | Idempotent 10k scan evidence |
| Incremental watch mode | Not started | Reconciliation/stability/restart tests |
| Extraction and OCR formats | Not started | Fixture suite, limits, corrupt-file isolation |
| zh-TW and English | Not started | OCR and retrieval evaluation set |
| Metadata/FTS/vector/hybrid retrieval | Not started | Deterministic fallback and p50/p95 report |
| Project/folder/related/duplicate/version graph | Not started | Provenance, correction, evaluation |
| Smart Inbox and explainable classification | Not started | UI states and safe suggestion behavior |
| Rename/move preview | Not started | Before/after/scope/policy UI and tests |
| Journal, crash recovery, undo | Not started | Fault injection and idempotent undo suite |
| Read-only MCP | Not started | Scope escape/injection tests and no write tools |
| 8 GB benchmark | Not started | Published hardware/OS/config/results |
| Updater pipeline | Not started | Signed metadata dry run and rollback |
| SBOM and checksums | Not started | Release-attached, independently verified artifacts |
| GitHub Release | Blocked externally | Remote, auth, verified assets, public download smoke |
| README/demo/launch assets | Not started | Claims match shipped build and public links |
| Post-launch issue/hotfix process | Planning only | Templates, labels, incident/hotfix drill |
| Critical/high security findings | Unknown | Complete threat model and scans; zero open critical/high |
| Known data-loss bugs | Unknown | Full action/recovery suite; zero known data-loss issues |

## M0 readiness gate

M0 can be locally complete when the monorepo, governance, ADRs, health slice, lockfiles, docs, and all local format/lint/typecheck/test/build commands pass. It cannot be marked `verified in CI` until a GitHub remote exists and the macOS/Windows/Linux matrix is green.
