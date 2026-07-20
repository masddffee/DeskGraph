# OpenAI Build Week submission pack

> **Status: local draft, not submitted.** This document prepares the required
> submission material for the OpenAI Build Week track, without publishing a
> repository, uploading a video, or making any Devpost mutation.

## Submission facts

**Project name:** DeskGraph

**Tagline:** Graphify your computer — private local context for AI.

**One-sentence pitch:** DeskGraph builds a local SQLite context graph from
folders a person explicitly selects, then offers explainable local search and
a narrowly scoped read-only MCP search tool without uploading their filenames,
paths, content, OCR, embeddings, or graph by default.

DeskGraph is a working pre-release development build, not a public v0.1
release. The strongest honest demo is a synthetic local folder on macOS arm64:

- native selection of one or more explicit, non-overlapping folders;
- metadata-only initial manifest scan with durable progress and recovery;
- bounded extraction of an explicitly selected, already-scanned text,
  Markdown, source, text-layer PDF, DOCX, PPTX, or XLSX file;
- offline SQLite FTS5 lexical search for Traditional Chinese and English, with
  a bounded untrusted-text snippet and an explanation of the matching field;
- conservative Folder Profile, marker-backed Project candidate, exact
  duplicate/version suggestion, and screenshot-review-group discovery;
- Smart Cleanup Inbox and durable Cleanup Preview as review assistance only;
  no file is changed by either surface; and
- an independently launched local stdio MCP server with exactly one read-only
  `search_files` tool over launch-granted, already-scanned scopes.

The latest local verification record is 408 Rust tests passed with zero
failures (two named live macOS FSEvents tests intentionally filtered because
they require unsandboxed host events), plus `pnpm check` passing Prettier,
ESLint, TypeScript, 66 Vitest tests, and the Vite production build. See the
[implementation status](planning/IMPLEMENTATION_STATUS.md) for the full test
evidence and its boundaries.

## Do not claim these features

The submission must not describe DeskGraph as a public production release or
claim any of the following as complete:

- vector, semantic, or hybrid retrieval;
- general related-file or semantic similarity discovery;
- automatic/incremental content re-indexing Watch Mode;
- executing rename, move, Trash, recovery, or Undo actions;
- a signed/notarized installer, updater, or released cross-platform runtime;
- Windows OCR runtime support, Linux desktop support, or macOS Intel/Universal
  runtime validation; or
- 8 GB memory, clean-machine, signed-package, or full interaction acceptance
  evidence.

Existing Rename and Cleanup surfaces are deliberately **Preview-only**. This
is a safety decision, not an omitted demo control: DeskGraph must not mutate a
user's files until its identity, transaction, platform-fence, recovery, and
Undo guarantees have passed their remaining acceptance gates.

## Three-minute video script

Record only after the local build and synthetic demo scope are verified. Use a
synthetic folder with harmless documents; do not show personal paths, file
contents, access tokens, or private OCR text.

| Time      | Screen action                                                                                              | Voiceover                                                                                                                                                                                                                                                     |
| --------- | ---------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| 0:00–0:20 | Show the DeskGraph Home screen and its local-only status.                                                  | “Our computers contain useful context, but handing an AI our whole filesystem is not acceptable. DeskGraph graphifies only the folders a person explicitly chooses, and keeps the manifest and search local.”                                                 |
| 0:20–0:45 | Select a synthetic folder with the native picker; show that authorization and scanning are separate.       | “The WebView never submits an arbitrary path. Native selection creates the local scope, and selecting it does not read file contents or start a scan.”                                                                                                        |
| 0:45–1:10 | Start and complete an Initial Manifest Scan.                                                               | “The first scan records metadata inside this boundary. It is resumable and atomically publishes a completed manifest, while hidden entries and symlinks are not followed.”                                                                                    |
| 1:10–1:35 | Extract one synthetic Markdown or text-layer PDF file, then search a Traditional Chinese or English query. | “Content extraction is opt-in and bounded. Search is offline SQLite lexical retrieval, and the result explains whether the filename or extracted local text matched.”                                                                                         |
| 1:35–1:55 | Show a Project candidate and Smart Cleanup Inbox item, then open a Cleanup Preview.                        | “DeskGraph can surface conservative project, duplicate, version, and screenshot-review evidence. At this stage these are review aids only: the preview cannot execute a file operation.”                                                                      |
| 1:55–2:18 | Show the MCP launch command and a `search_files` call against the completed synthetic scope.               | “For agents, the local MCP server exposes one read-only search tool. It has no arbitrary path parameter, no write tool, and snippets are opt-in and labeled untrusted.”                                                                                       |
| 2:18–2:43 | Show the repository history, tests, and this README.                                                       | “I built DeskGraph with Codex and GPT-5.6 as an implementation collaborator: it helped decompose the Rust, Tauri, React, SQLite, and MCP work into testable vertical slices. Safety-critical decisions remain explicit in the code, ADRs, and passing tests.” |
| 2:43–3:00 | Return to the safety contract and current limitations.                                                     | “This is a working pre-release, not a finished v0.1 release. Vector search, executable organization, Undo, installers, and cross-platform runtime validation remain gated. The point is useful local context without pretending unsafe automation is ready.”  |

## Devpost description draft

### What I built

DeskGraph is a local-first computer context graph for the files people choose
to authorize. It builds a local SQLite manifest, optionally extracts bounded
text from selected scanned documents, and makes that information searchable
without requiring an API key, Python, Docker, Ollama, or a cloud upload.

The current build demonstrates explicit native folder authorization,
metadata-only scan, bounded text/PDF/Office extraction, offline Traditional
Chinese and English lexical search, conservative project/duplicate/version
signals, a suggestion-only Smart Cleanup Inbox, and a read-only MCP
`search_files` tool. Results explain their matching source and extracted text
is always treated as untrusted data.

### How I built it

Rust owns filesystem policy, SQLite state, extraction jobs, graph/retrieval
logic, transactions, and MCP. Tauri plus React/TypeScript provides the local
desktop interface. SQLite FTS5 provides the current offline lexical retrieval;
Apple Vision OCR is a bounded macOS development provider for explicit PNG/JPEG
inputs.

Codex running GPT-5.6 was used as a development collaborator to investigate
the codebase, break work into safety-bounded vertical slices, implement and
review changes, and run the repository's Rust and TypeScript gates. The git
history documents the incremental implementation period from 2026-07-16
through 2026-07-20, including the manifest scan, extraction, search,
read-only MCP, cleanup-preview, and hard-exclusion slices. It did not receive
or upload a user's DeskGraph database or files as part of the product design.

### Why it matters

Many “AI file organizer” ideas require a person to trust opaque automation
with their entire computer. DeskGraph starts with the narrower useful problem:
give a person and their AI agent a controllable view of explicitly selected
local context. It prefers preview, evidence, and refusal over an unsafe file
mutation. That boundary is what makes future organization automation worth
building.

### Current limitations

DeskGraph is pre-release and must be tested only with a synthetic folder and
backups. The current usable search is lexical, not vector or hybrid. Cleanup
and Rename are preview-only; no production file action, Trash, or Undo exists.
The current verified desktop evidence is macOS arm64 development evidence, not
a signed, notarized, cross-platform release.

## Judge setup and test path

Use a fresh clone and a harmless synthetic folder. Do not use a personal home
directory or a production worktree as the demo scope.

```bash
corepack enable
corepack prepare pnpm@11.10.0 --activate
pnpm install --frozen-lockfile
cargo test --workspace --all-features
pnpm check

# Optional local desktop development run after Tauri platform prerequisites:
pnpm desktop:dev
```

For a CLI-only, offline test path, create a temporary folder with a text file,
then replace the example absolute path below with that folder:

```bash
cargo run -p deskgraph-cli -- manifest init --database ./deskgraph-demo.sqlite3
cargo run -p deskgraph-cli -- scope add --database ./deskgraph-demo.sqlite3 --path /absolute/path/to/synthetic-folder
cargo run -p deskgraph-cli -- scan start --database ./deskgraph-demo.sqlite3 --scope 1
cargo run -p deskgraph-cli -- search --database ./deskgraph-demo.sqlite3 --query "local context" --scope 1
```

The full setup, sample extraction, search, preview-only organization, and MCP
instructions are in the [README](../README.md) and
[MCP guide](MCP.md). The repository is Apache-2.0 licensed in
[`LICENSE`](../LICENSE).

## Required-field checklist before submission

- [ ] **Submitter type:** select the submitter's actual type; do not infer it
      from this repository.
- [ ] **Country:** select the submitter's actual country; do not infer it from
      this repository.
- [ ] **Category:** select the actual Build Week category after checking the
      current Devpost form.
- [ ] **Public repository URL:** make the repository public, then paste its
      final URL into Devpost. This workspace currently has no configured git
      remote, so this action is intentionally not represented as complete here.
- [ ] **Judge instructions:** paste the “Judge setup and test path” above and
      point judges to the synthetic-scope limitation.
- [ ] **Dev-tool instructions:** paste the “How I built it” section and state
      specifically how Codex/GPT-5.6 was used.
- [ ] **Codex Session ID (Devpost field 27950):** run `/feedback` in Codex,
      copy the Session ID it returns, and paste that exact value into Devpost.
      No Session ID has been obtained for this submission package; do not
      invent, reuse, or guess one.
- [ ] Confirm the public repository includes `LICENSE`, `README.md`, this
      setup/test path, and the source required to run the documented sample.
- [ ] Upload an unlisted or public YouTube video under three minutes with a
      voiceover. It must cover what was built, how Codex/GPT-5.6 was used, and
      the real pre-release limitations above.
- [ ] Add the video URL, public repository URL, name, tagline, and Devpost
      description draft from this document.
- [ ] Re-run the documented test commands on the final submission commit and
      replace any test counts in this document only with their actual output.
- [ ] Do not state or imply that file execution, Trash/Undo, semantic/vector
      retrieval, installers, or cross-platform release support is available.

## Evidence pointers

- [README current state and safety contract](../README.md)
- [Current implementation status and verification evidence](planning/IMPLEMENTATION_STATUS.md)
- [Read-only MCP contract](MCP.md)
- Run `git log --reverse --date=short` locally to inspect the dated
  implementation sequence; do not link Git internals in a public submission.
