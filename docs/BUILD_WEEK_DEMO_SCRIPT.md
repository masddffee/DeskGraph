# DeskGraph Build Week demo script

> **Target runtime: 2:45.** Read at a calm, conversational pace. This leaves
> 15 seconds below the three-minute submission cap for capture transitions.
> Record only synthetic files in a newly created demo folder. Do not show
> personal paths, file contents, access tokens, or private OCR text.

## Before recording

Run the deterministic local checks and use the separately generated CLI proof:

```bash
cargo test --workspace --all-features -- \
  --skip macos_recommended_watcher_delivers_a_live_file_hint \
  --skip macos_native_runtime_reconciles_create_modify_rename_and_delete \
  --test-threads=1
pnpm check
cargo run -p deskgraph-cli -- fixture demo --path /absolute/new/path/deskgraph-demo
```

The two skipped tests are opt-in macOS live-filesystem-event tests whose
FSEvents callback is unavailable on this host. They are not a reason to claim
native Watch Mode, packaged support, or cross-platform validation; individual
runs also did not receive a callback on this host.

## Timestamped shot list and voiceover

| Time      | Screen action                                                                                                                          | Voiceover                                                                                                                                                                                                                                                                                                                                     |
| --------- | -------------------------------------------------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| 0:00–0:18 | Home screen: local-only status and safety contract.                                                                                    | “Computers hold context that could make AI useful, but giving an AI your entire filesystem is not acceptable. DeskGraph is a local-first context graph: it keeps its SQLite manifest and search on this machine, and starts only with folders a person explicitly chooses.”                                                                   |
| 0:18–0:36 | Use the native picker to select a harmless synthetic folder; show that scan is still separate.                                         | “The WebView does not submit an arbitrary path. Native selection grants one local scope. Selecting it neither reads file contents nor starts a scan, so consent and indexing are separate actions.”                                                                                                                                           |
| 0:36–0:55 | Run and finish an Initial Manifest Scan; show metadata search.                                                                         | “Now I run a metadata-only Initial Manifest Scan. It is resumable and atomically publishes a completed manifest. Hidden entries and symlinks are not followed, and the search results explain whether a match comes from a filename or approved extracted text.”                                                                              |
| 0:55–1:16 | Explicitly extract the synthetic Markdown file, then repeat a Traditional Chinese or English content search.                           | “Content is opt-in. I explicitly choose this scanned file; the Rust backend rechecks the live grant and file identity, runs a bounded durable job, and labels its text untrusted. SQLite FTS then finds this Traditional Chinese or English text locally.”                                                                                    |
| 1:16–1:39 | Terminal: run or show the successful `fixture demo` JSON fields for Project, duplicate/version, Inbox, Preview, and unchanged sources. | “For repeatability, this one-command fixture creates harmless bilingual files and drives the real Rust and SQLite cores. It verifies extraction, search, Project and relation suggestions, plus a Smart Cleanup Preview. The report proves source files stayed unchanged: Preview requires confirmation, but execution is unavailable.”       |
| 1:39–1:57 | Start MCP and show one `search_files` call over a completed, explicitly granted scope.                                                 | “For agents, the local MCP server exposes one read-only search tool. It has no arbitrary path parameter and no write tool. Content snippets are opt-in and visibly untrusted.”                                                                                                                                                                |
| 1:57–2:23 | Show `git log` with the cited commits and the deterministic test command/result.                                                       | “Codex with GPT-5.6 was my development collaborator: it explored the codebase, helped implement and review safety-bounded slices, and ran validation gates under human direction. The dated commits show that progression. Codex also helped reproduce a Unix wrong-inode rename race, so the product deliberately refuses unsafe execution.” |
| 2:23–2:45 | Return to the Preview-only state and the limitation list.                                                                              | “This is a pre-release development build, not a finished v0.1 release. Vector or hybrid search, executable rename, move, Trash, Undo, installers, and cross-platform runtime validation remain gated. DeskGraph’s promise today is useful local context with explicit scope and honest refusal, not unsafe automation.”                       |

## Capture guardrails

- Do not imply the CLI fixture database and Desktop app database are one live
  shared state; they are deliberately separate.
- Do not call Watch Mode incremental or automatic; the demonstrated product
  retains a bounded reconciliation fallback and lacks complete incremental
  content re-indexing evidence.
- Do not show or describe any file action as executable. Current Rename and
  Cleanup workflows are Preview-only.
- Do not add root revocation to the timed shot list unless the real Desktop
  flow has first been rehearsed and every search/extraction shot is already
  captured. Revocation purges only DeskGraph's local derived state and access
  grant, never a source file; it is local development evidence, not packaged
  or hostile-process proof.
- Show the actual test output from the final submission commit. Do not replace
  counts or claim broader platform evidence than that output supports.
