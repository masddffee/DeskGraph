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
release. The strongest honest demo separates the Desktop experience from the
one-command CLI backend proof; they use different local databases:

- native selection of one or more explicit, non-overlapping folders;
- metadata-only initial manifest scan with durable progress and recovery;
- bounded extraction of an explicitly selected, already-scanned text,
  Markdown, source, text-layer PDF, DOCX, PPTX, or XLSX file;
- offline SQLite FTS5 lexical search for Traditional Chinese and English, with
  a bounded untrusted-text snippet and an explanation of the matching field;
- conservative Folder Profile, marker-backed Project candidate, exact
  duplicate/version suggestion, and screenshot-review-group discovery;
- a deterministic bilingual CLI fixture that verifies real extraction,
  Traditional Chinese/English FTS, Project, duplicate/version, and Smart
  Cleanup Inbox plus a durable non-executable Cleanup Preview without changing
  its created source files;
- Smart Cleanup Inbox and durable Cleanup Preview as review assistance only;
  neither authorizes a file action; and
- an independently launched local stdio MCP server with exactly one read-only
  `search_files` tool over launch-granted, already-scanned scopes.

The latest local verification record is 413 Rust tests passed with zero
failures (two named live macOS FSEvents tests intentionally filtered because
they require unsandboxed host events), plus `pnpm check` passing Prettier,
ESLint, TypeScript, 70 Vitest tests, and the Vite production build. See the
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

| Time      | Screen action                                                                                                                                             | Voiceover                                                                                                                                                                                                                                                    |
| --------- | --------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| 0:00–0:20 | Show the DeskGraph Home screen and its local-only status.                                                                                                 | “Our computers contain useful context, but handing an AI our whole filesystem is not acceptable. DeskGraph graphifies only the folders a person explicitly chooses, and keeps the manifest and search local.”                                                |
| 0:20–0:45 | Select a synthetic folder with the native picker; show that authorization and scanning are separate.                                                      | “The WebView never submits an arbitrary path. Native selection creates the local scope, and selecting it does not read file contents or start a scan.”                                                                                                       |
| 0:45–1:08 | Start and complete an Initial Manifest Scan, then search metadata.                                                                                        | “The first scan records metadata inside this boundary. It is resumable and atomically publishes a completed manifest, while hidden entries and symlinks are not followed.”                                                                                   |
| 1:08–1:35 | Only if the final integrated build passes: explicitly extract the synthetic Markdown result, then repeat a Traditional Chinese or English content search. | “Content extraction starts only when I choose this scanned file. The Rust backend revalidates the local grant and file identity, runs a bounded durable job, and SQLite explains that the result matched extracted text.”                                    |
| 1:35–1:58 | In Terminal, run the one-command `fixture demo`; show Project, duplicate/version, Smart Cleanup Preview, and unchanged-source fields.                     | “This separate synthetic CLI proof exercises the same real Rust and SQLite cores. Its durable Preview requires confirmation but cannot execute; it does not share the Desktop database or perform an organization action.”                                   |
| 1:58–2:20 | Show the MCP launch command and a `search_files` call against its explicitly granted completed scope.                                                     | “For agents, the local MCP server exposes one read-only search tool. It has no arbitrary path parameter, no write tool, and snippets are opt-in and labeled untrusted.”                                                                                      |
| 2:20–2:45 | Show the repository history, tests, and the Preview-only ADR.                                                                                             | “Codex with GPT-5.6 built and reviewed these vertical slices. One adversarial review reproduced a wrong-inode race in Unix rename, so we rejected unsafe execution and kept organization Preview-only instead of hiding the risk for this demo.”             |
| 2:45–3:00 | Return to the safety contract and current limitations.                                                                                                    | “This is a working pre-release, not a finished v0.1 release. Vector search, executable organization, Undo, installers, and cross-platform runtime validation remain gated. The point is useful local context without pretending unsafe automation is ready.” |

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
review changes, and run the repository's Rust and TypeScript gates. For one
concrete safety decision, an adversarial Codex review reproduced a Unix
wrong-inode race between final validation and a pathname rename. We therefore
rejected that production execution adapter and kept Rename/Move Preview-only,
instead of weakening the invariant for a demo. The git history documents the
incremental implementation period from 2026-07-16 through 2026-07-20,
including manifest scan, extraction, search, read-only MCP, cleanup-preview,
and hard-exclusion slices. The product does not upload a user's DeskGraph
database or files to Codex or another service by default.

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

For the CLI-only offline proof, provide a brand-new path. The command refuses
to overwrite an existing entry:

```bash
cargo run -p deskgraph-cli -- fixture demo --path /absolute/new/path/deskgraph-demo
```

The JSON report is successful only after real local backends verify seven
generated files, two bounded extractions, Traditional Chinese and English FTS
matches, a marker-backed Project, exact-duplicate and numeric-version evidence,
and a Smart Cleanup Inbox containing both relation kinds. It also proves the
created source files are unchanged and creates a durable Preview whose
`action_authorized` and `execution_available` flags are false. This command's
database is deliberately separate from Desktop app-data; do not imply that the
UI will display its derived state.

The full setup, sample extraction, search, preview-only organization, and MCP
instructions are in the [README](../README.md) and
[MCP guide](MCP.md). The repository is Apache-2.0 licensed in
[`LICENSE`](../LICENSE).

## Required-field checklist before submission

- [ ] **Submitter type:** select the submitter's actual type; do not infer it
      from this repository.
- [ ] **Country:** select the submitter's actual country; do not infer it from
      this repository.
- [ ] **Category:** confirm `Apps for Your Life`, the current submission-plan
      choice, or document why the final demonstrated audience requires a
      different category.
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
