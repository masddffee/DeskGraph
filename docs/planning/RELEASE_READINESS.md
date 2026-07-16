# Release Readiness

Last reviewed: 2026-07-16

Overall status: **not release-ready**. Local implementation is in M2 with one parallel M3 lexical slice while M0 remote CI and M1 cross-platform/memory/live-UI evidence remain open.

| Gate                                           | Status             | Evidence required                                        |
| ---------------------------------------------- | ------------------ | -------------------------------------------------------- |
| macOS Apple Silicon package                    | Not started        | Signed/notarized clean-machine install and smoke         |
| macOS Intel or Universal package               | Not started        | Native or Universal clean-machine evidence               |
| Windows x64 installer                          | Not started        | Signed clean-VM install and smoke                        |
| Linux experimental package                     | Not started        | Clearly labeled build and smoke                          |
| Explicit authorized scopes                     | Verified locally   | Desktop/CLI authorization, component-aware protected-tree policy, symlink/reparse and platform hidden/system exclusions; Windows runtime fixtures remain |
| Initial manifest scan                          | In progress        | Release 10k idempotency/timing, durable progress/pause/resume, crash-reopen replay, atomic publish, and Unix permission fixture pass; memory, live updated-UI smoke, and remote CI remain |
| Incremental watch mode                         | Not started        | Reconciliation/stability/restart tests                   |
| Extraction and OCR formats                     | In progress        | Text/Markdown/code and bounded text-layer PDF providers, durable cancellation, tagged provenance, atomic untrusted chunks, corrupt/encrypted/active-content/decompression fixtures pass locally; Office, image metadata, OCR, Windows runtime, 8 GB residency, and remaining corpora remain |
| zh-TW and English                              | In progress        | Built-in UTF-8 extraction has exact byte-offset mixed zh-TW/English fixtures; PDF ToUnicode fixture extracts both with page provenance; OCR and retrieval evaluation sets remain |
| Metadata/FTS/vector/hybrid retrieval           | In progress        | Offline path/content FTS5, bounded scope/type/date/source filters, deterministic explanations, CLI/Desktop and synthetic 10k p50/p95/index-size baseline pass locally; project/folder filters, vectors, embeddings, hybrid fusion, real/100k/8 GB evaluation and cross-platform/live-UI evidence remain |
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
| Critical/high security findings                | In progress        | npm: zero known vulnerabilities; last pre-PDF all-target RustSec scan had zero vulnerabilities plus 17 open warnings; isolated PDF closure is clean; post-integration full-lock rerun and full threat model pending |
| Known data-loss bugs                           | Unknown            | Full action/recovery suite; zero known data-loss issues  |

## M0 readiness gate

The M0 implementation is verified locally. Governance, ADRs, health slice, lockfiles, log-redaction checks, local format/lint/typecheck/test/build, isolated fresh-clone setup, CLI execution, debug app bundle, and live IPC UI smoke have passed. M0 itself remains **in progress** until a GitHub remote exists and the macOS/Windows/Linux matrix is green.

## M1 local readiness note

The resumable scanner is safe to exercise with test folders: progress and pending paths persist in SQLite, live manifest rows are not replaced until a complete scan publishes atomically, pause is acknowledged between entries, expired work reopens as interrupted, and resume revalidates the original canonical authorization boundary. This is not yet a release claim: Windows junction/hidden-attribute runtime evidence, peak RSS, remote CI, and live UI interaction evidence remain open.

## M2 extraction local readiness note

The text/Markdown/code and text-layer PDF slices are safe to exercise with test files already present in an explicit scanned scope. Providers never receive an arbitrary path; source size and actual open-handle identity are revalidated; reads/decompression/pages/chunks/time are bounded; PDF active content and attachments remain inert; all text stays `untrusted_extracted_text`; and publication is atomic. CLI status and Desktop dashboard payloads contain no paths or text. This is not an M2 completion or release claim: DOCX, PPTX, XLSX, image metadata, screenshot OCR, later dependency audits, full Windows/macOS Intel/Linux runtime tests, live Desktop interaction, post-integration full-lock RustSec rerun, real-world PDF corpus evaluation, and 8 GB extraction benchmarks remain open.

## M3 lexical local readiness note

The FTS5 baseline is safe to exercise on test scopes: search is read-only, stays in bundled SQLite, accepts bounded quoted queries of at least three Unicode characters, caps candidates/results/snippets, excludes absent locations and stale chunks, and exposes fixed ranking explanations. Scope, match-source, extension, and modified-time filters are validated twice and echoed after normalization. User-invoked CLI/Desktop results intentionally return authorized paths and bounded untrusted snippets; ordinary logs omit query/path/text. A reproducible 10k synthetic macOS arm64 baseline records p50/p95/max and FTS bytes. This is not an M3 or release claim: graph-backed project/folder filters, vector/embedding providers, hybrid/semantic behavior, representative/100k/8 GB/RSS/thermal evaluation, short-query strategy, live Desktop interaction, and remote platform runtime remain open.
