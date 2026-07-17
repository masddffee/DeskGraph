# DeskGraph

> **Pre-release development build — use only with test folders and keep backups.**

**Graphify your computer.**

DeskGraph is a local-first computer context graph that will connect, search, and safely organize files from folders you explicitly authorize. It is designed to expose narrowly scoped, read-only local context to AI agents without uploading your private filenames, paths, extracted content, OCR, embeddings, or graph data by default.

## Current state

The repository is implementing M2 Content Intelligence plus bounded M3 lexical, M4 project-graph, M5 rename-preview, and M6 watch-reconciliation slices while M0/M1 external evidence remains open. The CLI and Tauri desktop can initialize a local SQLite manifest, explicitly authorize an existing folder, run a metadata-only initial scan, persist progress, pause or resume safely, recover interrupted work, report graph statistics, and search current local paths and active extracted text. Rescans are idempotent in local tests; hard links share an identity, same-filesystem renames preserve identity, and symlinks and hidden entries are not followed.

The current content slices can extract bounded text from an explicitly selected, already-scanned text, Markdown, source-code, text-layer PDF, DOCX, PPTX, or XLSX file. They revalidate the authorized scope, manifest snapshot, and actual open-file identity; store only provenance-bearing `untrusted_extracted_text` chunks; support durable cancellation/recovery; and atomically preserve the prior complete version on failure. PDF extraction rejects encryption, ignores active content and attachments, and records page/fragment provenance. Office extraction reads only allowlisted in-memory text parts, ignores macros/formulas/relationships/embeddings, rejects unsafe or bomb-like archives/XML, and records paragraph/slide/sheet-cell provenance instead of fabricated byte offsets.

The current search slice uses bundled SQLite FTS5 trigram indexes for Traditional Chinese and English substring queries of 3–256 Unicode characters. It requires no embedding or model, returns bounded text snippets, filters out stale chunks and absent locations, and explains whether filename/path, extracted text, or both matched. Scope, source, extension, and modified-time filters plus a synthetic 10k p50/p95/index-size baseline pass locally. One- and two-character queries, project/folder filters, vector semantic search, hybrid fusion, representative/100k/8 GB evaluation, and cross-platform evidence remain open.

The current M4 slices derive bounded Folder Profiles, persist correctable Project root candidates, compare two explicit current files as a bounded exact-duplicate suggestion, and recognize a conservative filename-version relation. Root, exact-duplicate pair, and version decisions are append-only; every duplicate or version decision repeats its complete live verification. Version inference accepts only matching normalized base/extension names with explicit `-vN`, `_vN`, ` vN`, or `.vN` suffixes and orders the numeric versions. Version feedback is bound to that exact directional evidence, so changed direction or version numbers return to `suggested`. No relation creates file membership or a filesystem action. The M5 slice only journals a same-folder file rename preview, and the M6 slice only reconciles explicit watch hints—neither exposes an automatic file action.

Image metadata, OCR, vector/hybrid retrieval, Project file-membership, related/similarity and general version discovery, background duplicate discovery, cross-pair learning, automatic native Watch Mode, executable organization, recovery/undo, and MCP are **not implemented or shipped**. The Office providers are locally verified development slices, not public-release support; scanned/image-only PDFs still require the future OCR provider. Representative document corpora, peak-memory evidence, complete cross-platform runtime evidence, the latest UI smoke, and the installer/release pipeline are open, so this is not a public v0.1 release.

## Safety contract

- No permanent file deletion.
- No LLM can execute filesystem operations.
- Every future move or rename must be previewed, policy-validated, durably journaled, crash-recoverable, and undoable.
- No path is accessed outside an explicit user scope.
- Extracted document text is always untrusted data and is never executed.
- The core product must work without a local LLM, API key, Python, Docker, or Ollama.

## Prerequisites

- Rust stable as pinned by `rust-toolchain.toml`, with `rustfmt` and `clippy`.
- Node.js 24.12 or a compatible supported release.
- Corepack and pnpm 11.10.0.
- Tauri 2 platform prerequisites for your operating system.

## Fresh-clone setup

```bash
corepack enable
corepack prepare pnpm@11.10.0 --activate
pnpm install --frozen-lockfile
cargo test --workspace
pnpm check
```

Run the privacy-safe CLI health check:

```bash
cargo run -p deskgraph-cli -- health
```

Run the M1 metadata-only CLI slice with a new local manifest and a test folder you explicitly choose:

```bash
cargo run -p deskgraph-cli -- manifest init --database ./deskgraph-dev.sqlite3
cargo run -p deskgraph-cli -- scope add --database ./deskgraph-dev.sqlite3 --path /absolute/path/to/test-folder
cargo run -p deskgraph-cli -- scan start --database ./deskgraph-dev.sqlite3 --scope 1
cargo run -p deskgraph-cli -- manifest stats --database ./deskgraph-dev.sqlite3
```

`scope add` canonicalizes and stores the explicit local boundary. It does not scan. `scan start` reads names and filesystem metadata within that boundary but does not open file contents. Scope paths are returned only by explicit scope-management commands and UI; structured logs omit them.

For a durable job that can be inspected, paused, resumed, or advanced in bounded batches:

```bash
cargo run -p deskgraph-cli -- scan create --database ./deskgraph-dev.sqlite3 --scope 1
cargo run -p deskgraph-cli -- scan status --database ./deskgraph-dev.sqlite3 --job 1
cargo run -p deskgraph-cli -- scan advance --database ./deskgraph-dev.sqlite3 --job 1 --batch-size 256
cargo run -p deskgraph-cli -- scan pause --database ./deskgraph-dev.sqlite3 --job 1
cargo run -p deskgraph-cli -- scan resume --database ./deskgraph-dev.sqlite3 --job 1
cargo run -p deskgraph-cli -- scan run --database ./deskgraph-dev.sqlite3 --job 1
```

Scan observations stay in job-scoped staging while work is running or paused. The visible manifest is replaced only after the complete job publishes in one SQLite transaction.

Run the current bounded text/PDF/Office extraction slice for one file already discovered by the scan:

```bash
cargo run -p deskgraph-cli -- extract start \
  --database ./deskgraph-dev.sqlite3 \
  --scope 1 \
  --path /absolute/path/to/test-folder/notes.md
cargo run -p deskgraph-cli -- extract stats --database ./deskgraph-dev.sqlite3
```

Use the same command with a `.pdf`, `.docx`, `.pptx`, or `.xlsx` path. PDFs must contain a text layer; Office formulas, macros, relationships, external links, and embedded objects are never executed or traversed. `extract start` opens only the manifest-backed file selected by the explicit path. Its JSON response and structured logs contain job IDs, fixed status/error codes, byte counts, chunks, and timing—not the path, filename, or extracted text. Automation may use `--node` instead of `--path`. Durable controls are available through `extract create/run/status/list/cancel/resume`.

Search current metadata and active extracted text without a model:

```bash
cargo run -p deskgraph-cli -- search \
  --database ./deskgraph-dev.sqlite3 \
  --query "專案 context" \
  --scope 1 \
  --source content \
  --extension md
```

Search is an explicit content-returning operation: its stdout intentionally contains matching authorized paths and bounded snippets for the user who requested them. Structured stderr logs omit the query, paths, filenames, and snippets. Omit `--scope` to search all scopes in this local database; `--source` accepts `all`, `metadata`, or `content`; `--extension` accepts one 1–16 character ASCII-alphanumeric suffix with or without a leading dot. Optional `--modified-since` is inclusive and `--modified-before` is exclusive; both use UTC Unix seconds. `--limit` accepts 1–50. Queries shorter than three Unicode characters fail closed instead of scanning the corpus.

The reproducible synthetic lexical benchmark and the latest local evidence are documented under [benchmarks](benchmarks/README.md). The checked-in 10k result is a macOS arm64 development baseline, not an 8 GB or cross-platform release claim.

Read one bounded, model-free Folder Profile after scanning the folder:

```bash
cargo run -p deskgraph-cli -- folder profile \
  --database ./deskgraph-dev.sqlite3 \
  --scope 1 \
  --path /absolute/path/to/test-folder
```

The explicit response contains the selected canonical folder path, aggregate direct/descendant counts, category counts, and any marker-based Project Suggestion. The computation reads only current manifest locations, stops at 100,000 descendants, and returns no partial profile on overflow. Structured logs omit the selected path and descendant names.

Persist and explicitly correct a Project root candidate without assigning file membership:

```bash
cargo run -p deskgraph-cli -- project propose \
  --database ./deskgraph-dev.sqlite3 \
  --scope 1 \
  --path /absolute/path/to/test-folder
cargo run -p deskgraph-cli -- project decide \
  --database ./deskgraph-dev.sqlite3 \
  --project 1 \
  --decision reject
cargo run -p deskgraph-cli -- project status \
  --database ./deskgraph-dev.sqlite3 \
  --project 1
cargo run -p deskgraph-cli -- project list \
  --database ./deskgraph-dev.sqlite3
```

`propose` re-derives and validates current manifest evidence before persistence; it does not accept the candidate. Only `decide` appends an explicit `accepted` or `rejected` user event. Repeating the current decision is idempotent, while an opposite decision appends the next correction sequence. Explicit propose/decide/status responses may contain the current root path; `project list` and structured logs remain path-free. Acceptance confirms only the stable root candidate: file membership, cross-root learning, merge/split, general related/similarity/version relations, retrieval filters, and the Project UI remain unimplemented.

Check two canonical, already-scanned files for exact byte equality without changing them:

```bash
cargo run -p deskgraph-cli -- relation duplicate \
  --database ./deskgraph-dev.sqlite3 \
  --scope 1 \
  --left /canonical/path/to/test-folder/copy-a.bin \
  --right /canonical/path/to/test-folder/copy-b.bin
cargo run -p deskgraph-cli -- relation verify \
  --database ./deskgraph-dev.sqlite3 \
  --relation 1
cargo run -p deskgraph-cli -- relation decide \
  --database ./deskgraph-dev.sqlite3 \
  --relation 1 \
  --decision reject
cargo run -p deskgraph-cli -- relation list \
  --database ./deskgraph-dev.sqlite3
```

Both paths must be canonical, non-symlink files with different stable identities in the same authorized scope. DeskGraph revalidates manifest metadata and read-only open-handle identities, then compares every byte in 64 KiB chunks with a 64 MiB maximum and cooperative five-second deadline. Empty, oversized, changed, aliased, different, or unreadable files produce no observation. A successful check or verify appends immutable local evidence and returns the two explicit paths. `decide` performs that complete live verification again before appending an explicit user `accepted` or `rejected` event; repeated decisions are idempotent and opposite decisions remain auditable corrections. `relation list` returns path-free history labeled `verification_required`. Structured logs omit paths, filenames, database path, and content. A decision never merges, deletes, renames, moves, or otherwise organizes either file. Background discovery, larger-file hashing, fuzzy similarity, general version discovery, and cross-pair learning remain unimplemented.

Suggest and revalidate a conservative filename-version relation without reading file content:

```bash
cargo run -p deskgraph-cli -- relation version \
  --database ./deskgraph-dev.sqlite3 \
  --scope 1 \
  --first /canonical/path/to/test-folder/企劃-v1.md \
  --second /canonical/path/to/test-folder/企劃-v2.md
cargo run -p deskgraph-cli -- relation version-verify \
  --database ./deskgraph-dev.sqlite3 \
  --relation 2
cargo run -p deskgraph-cli -- relation version-decide \
  --database ./deskgraph-dev.sqlite3 \
  --relation 2 \
  --decision accept
```

Both current files pass the same canonical scope, symlink/reparse, manifest, platform identity, metadata, and read-only open-handle checks before and after name analysis. The normalized base and extension must match, and each stem must end in `-vN`, `_vN`, ` vN`, or `.vN`, where `N` is 1–999999 without a leading zero. Modification time, size, terms such as `final`, and file content never determine order. Explicit output contains both current paths and rule evidence; logs and `relation list` remain path-free. `version-decide` repeats live verification before appending a user decision. Repeated decisions for equivalent evidence are idempotent, opposite decisions remain auditable, and a rename that changes ordered nodes, base, extension, or version numbers produces a fresh `suggested` state. Acceptance is graph feedback only; general discovery, date/semantic versions, similarity, membership, and file actions remain unimplemented.

Exercise the durable watch-reconciliation core with an explicit hint:

```bash
cargo run -p deskgraph-cli -- watch observe \
  --database ./deskgraph-dev.sqlite3 \
  --scope 1 \
  --path /absolute/path/to/test-folder/notes.md
cargo run -p deskgraph-cli -- watch advance \
  --database ./deskgraph-dev.sqlite3 \
  --event 1
cargo run -p deskgraph-cli -- watch list --database ./deskgraph-dev.sqlite3
```

`observe` validates the current authorized scope and persists a path-free status response. Events within one scope coalesce until the one-second stability deadline; run `advance` at or after `stable_after_unix_ms` to revalidate size, modified time, identity, and read-only access before an atomic manifest reconciliation. This is a core/CLI development slice, not automatic Watch Mode: native OS event adapters, incremental content re-extraction/indexing, cloud-placeholder handling, and background resource controls remain unimplemented.

Create a durable same-folder file rename preview without changing the filesystem:

```bash
cargo run -p deskgraph-cli -- organize rename-preview \
  --database ./deskgraph-dev.sqlite3 \
  --scope 1 \
  --source /absolute/path/to/test-folder/draft.md \
  --new-name final.md
cargo run -p deskgraph-cli -- organize status \
  --database ./deskgraph-dev.sqlite3 \
  --plan 1
cargo run -p deskgraph-cli -- organize list \
  --database ./deskgraph-dev.sqlite3
```

The explicit preview/status response returns canonical before/after paths and passed policy checks; ordinary logs and `organize list` remain path-free. The Desktop has the same backend-owned preview form and a path-free recent history. The source must still match its scanned identity and metadata, and the destination must be free. This slice intentionally has no execute command or button: move, conflict resolution beyond fail-closed/case-only planning, crash recovery, rollback, Undo, and Desktop execution controls remain unimplemented.

Start the desktop application:

```bash
pnpm desktop:dev
```

The health report includes only the application version, OS/architecture, database lifecycle state, optional-provider state, and privacy flags. It does not include filesystem locations. Explicit scope management, user-invoked search, the CLI Folder Profile, and explicit before/after rename preview may return the path the user requested; ordinary logs plus extraction, watch, and recent action-history payloads omit paths and content. Search snippets are visibly labeled untrusted local text and rendered as text, never executable markup. The Watch panel explicitly reports that its native adapter is not connected, and the organizer panel explicitly reports that no execute control exists.

## Development verification

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
pnpm format:check
pnpm lint
pnpm typecheck
pnpm test
pnpm build
```

## Planning and contribution

- [Project context](PROJECT_CONTEXT.md)
- [Repository assessment](docs/planning/REPOSITORY_ASSESSMENT.md)
- [Implementation status](docs/planning/IMPLEMENTATION_STATUS.md)
- [v0.1 task graph](docs/planning/TASK_GRAPH.md)
- [Contributing](CONTRIBUTING.md)
- [Security policy](SECURITY.md)

DeskGraph is licensed under [Apache-2.0](LICENSE).
