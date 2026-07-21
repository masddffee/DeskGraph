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
- an exact two-stage local root-withdrawal control that previews the derived
  purge, rejects stale impact, drops the DeskGraph capability, requires a fresh
  scan after reauthorization, and never changes source files;
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

The latest local verification record is 444 deterministic Rust tests passed with zero
failures using the deterministic command below. It explicitly skips only
`macos_recommended_watcher_delivers_a_live_file_hint` and
`macos_native_runtime_reconciles_create_modify_rename_and_delete`: opt-in
macOS live tests whose FSEvents callback is unavailable on this host. Those tests were
also run individually and did not receive a callback, so this is not native Watch
verification. `pnpm check` also
passed Prettier, ESLint, TypeScript, 73 Vitest tests, and the Vite production
build. See the [implementation status](planning/IMPLEMENTATION_STATUS.md) for
the complete evidence and its boundaries.

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

## Video script (2:45 target)

Record only after the local build and synthetic demo scope are verified. Use a
synthetic folder with harmless documents; do not show personal paths, file
contents, access tokens, or private OCR text. The timestamped shot list and
voiceover are in [BUILD_WEEK_DEMO_SCRIPT.md](BUILD_WEEK_DEMO_SCRIPT.md).
It is deliberately 2:45 at the planned pace, leaving 15 seconds under the
three-minute limit for natural pauses or capture transitions.

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

Codex running GPT-5.6 was used as a development collaborator: it investigated
the codebase, helped split work into safety-bounded vertical slices,
implemented and reviewed changes under human direction, and ran Rust and
TypeScript validation gates. It did not receive a user's DeskGraph database or
files by default, and it never directly executes filesystem operations in the
product. In one concrete safety review, Codex helped reproduce a Unix
wrong-inode race between final validation and a pathname rename. The outcome
was to reject that production execution adapter and keep Rename/Move
Preview-only, rather than weaken the invariant for a demo.

### Dated implementation evidence

The commits below evidence incremental repository changes; they do not prove
individual authorship or replace the required Codex Session ID.

| Date       | Commit               | Evidence                                                       |
| ---------- | -------------------- | -------------------------------------------------------------- |
| 2026-07-16 | `5a70b23`            | Durable metadata scan readiness recorded.                      |
| 2026-07-17 | `72f3711`, `2da444c` | Durable Watch reconciliation and rename Preview protocol.      |
| 2026-07-18 | `f725a7e`            | Scoped, read-only MCP search.                                  |
| 2026-07-19 | `8f80679`, `8ad11b9` | Wrong-inode race stays gated; durable Cleanup Preview added.   |
| 2026-07-20 | `d46f0a5`, `ca42ce3` | Hard exclusions enforced; verified Build Week flow documented. |
| 2026-07-21 | `3005ebf`            | Root withdrawal and cooperative read fencing hardened.         |

Run `git show <commit>` locally for the dated diff. The Devpost field still
needs the actual Session ID obtained from `/feedback`; do not infer it from
git history.

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
cargo test --workspace --all-features -- \
  --skip macos_recommended_watcher_delivers_a_live_file_hint \
  --skip macos_native_runtime_reconciles_create_modify_rename_and_delete \
  --test-threads=1
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
- [x] Create and visually review a local 95-second 1280×720 H.264 silent guided
      cut using real synthetic-scope Desktop scan/search states plus separately
      verified CLI evidence. It is intentionally not represented as continuous
      live operation. The local artifact is ignored by git; SHA-256:
      `bca6e8c72817919ae32fcfd69def3cff15f4e14655ab443342d0ab41462255e5`.
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
