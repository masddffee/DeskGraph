# DeskGraph

> **Pre-release M2/M3 — use only with test folders and keep backups.**

**Graphify your computer.**

DeskGraph is a local-first computer context graph that will connect, search, and safely organize files from folders you explicitly authorize. It is designed to expose narrowly scoped, read-only local context to AI agents without uploading your private filenames, paths, extracted content, OCR, embeddings, or graph data by default.

## Current state

The repository is implementing M2 Content Intelligence plus the deterministic M3 lexical baseline while M0/M1 external evidence remains open. The CLI and Tauri desktop can initialize a local SQLite manifest, explicitly authorize an existing folder, run a metadata-only initial scan, persist progress, pause or resume safely, recover interrupted work, report graph statistics, and search current local paths and active extracted text. Rescans are idempotent in local tests; hard links share an identity, same-filesystem renames preserve identity, and symlinks and hidden entries are not followed.

The current content slices can extract bounded UTF-8 from an explicitly selected, already-scanned text, Markdown, source-code, or text-layer PDF file. They revalidate the authorized scope, manifest snapshot, and actual open-file identity; store only provenance-bearing `untrusted_extracted_text` chunks; support durable cancellation/recovery; and atomically preserve the prior complete version on failure. PDF extraction uses a strictly bounded, path-free Rust adapter, rejects encrypted PDFs, ignores active content and attachments, and records page/fragment provenance instead of fabricated byte offsets.

The current search slice uses bundled SQLite FTS5 trigram indexes for Traditional Chinese and English substring queries of 3–256 Unicode characters. It requires no embedding or model, returns bounded text snippets, filters out stale chunks and absent locations, and explains whether filename/path, extracted text, or both matched. One- and two-character queries, vector semantic search, hybrid fusion, complete filters, evaluation, and p50/p95 benchmarks remain open.

DOCX, PPTX, XLSX, image metadata, OCR, vector/hybrid retrieval, watch mode, organization, undo, and MCP are planned but **not shipped**. Scanned/image-only PDFs require the future OCR provider. Peak-memory evidence, complete cross-platform runtime evidence, the latest UI smoke, and the installer/release pipeline are still open, so this is not a public v0.1 release.

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

Run the current bounded text/PDF extraction slice for one file already discovered by the scan:

```bash
cargo run -p deskgraph-cli -- extract start \
  --database ./deskgraph-dev.sqlite3 \
  --scope 1 \
  --path /absolute/path/to/test-folder/notes.md
cargo run -p deskgraph-cli -- extract stats --database ./deskgraph-dev.sqlite3
```

Use the same command with a `.pdf` path for a text-layer PDF. `extract start` opens only the manifest-backed file selected by the explicit path. Its JSON response and structured logs contain job IDs, fixed status/error codes, byte counts, chunks, and timing—not the path, filename, or extracted text. Automation may use `--node` instead of `--path`. Durable controls are available through `extract create/run/status/list/cancel/resume`.

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

Start the desktop application:

```bash
pnpm desktop:dev
```

The health report includes only the application version, OS/architecture, database lifecycle state, optional-provider state, and privacy flags. It does not include filesystem locations. The desktop shows paths in explicit scope management and user-invoked search results; its extraction and watch dashboards expose aggregate counts and fixed job states without paths or content. Search snippets are visibly labeled untrusted local text and rendered as text, never executable markup. The Watch panel explicitly reports that its native adapter is not connected.

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
