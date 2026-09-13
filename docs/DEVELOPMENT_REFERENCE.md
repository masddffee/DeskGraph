# DeskGraph Development Reference

This document preserves the command-level development and verification material that was previously embedded in the root README.

It is intentionally detailed. It is **not** the canonical completion-status ledger. For the latest milestone-by-milestone implementation state, verified evidence, blockers, and explicit non-claims, use [`planning/IMPLEMENTATION_STATUS.md`](planning/IMPLEMENTATION_STATUS.md).

DeskGraph is still a pre-release development build. Use synthetic or disposable test folders and keep backups.

## Prerequisites

- Rust stable as pinned by `rust-toolchain.toml`, with `rustfmt` and `clippy`.
- Node.js 24.12 or a compatible supported release.
- Corepack and pnpm 11.10.0.
- Tauri 2 platform prerequisites for the host operating system.

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

## Recorded deterministic verification path

The Build Week development snapshot recorded 452 deterministic Rust tests and 76 Vitest tests passing while explicitly skipping only two opt-in macOS live FSEvents tests whose callback was unavailable on that host:

```bash
cargo test --workspace --all-features -- \
  --skip macos_recommended_watcher_delivers_a_live_file_hint \
  --skip macos_native_runtime_reconciles_create_modify_rename_and_delete \
  --test-threads=1
pnpm check
```

That evidence is local development evidence. It is not signed-package, clean-machine, 8 GB-memory, Windows-runtime, or cross-platform release certification.

For the dated judge-facing package, see [`HACKATHON_SUBMISSION.md`](HACKATHON_SUBMISSION.md) and [`BUILD_WEEK_DEMO_SCRIPT.md`](BUILD_WEEK_DEMO_SCRIPT.md).

## One-command synthetic demo

Use a brand-new path to generate harmless bilingual sample files and exercise the real local backends end to end. The command refuses to overwrite an existing destination.

```bash
cargo run -p deskgraph-cli -- fixture demo --path /absolute/new/path/deskgraph-demo
```

The fixture creates a synthetic authorized scope plus a separate SQLite database and exercises metadata scanning, bounded Markdown/code extraction, Traditional Chinese and English FTS results, a marker-backed Project candidate, exact-duplicate and numeric-version evidence, Smart Cleanup Inbox derivation, and a durable non-executable organization preview.

The fixture uses no OCR, model, API key, remote service, Python, Docker, or Ollama. Its stdout intentionally returns generated demo paths and search snippets because this is an explicit user-facing command; structured logs remain path/content-free.

The generated database is separate from the Desktop application's private app-data database. Do not present the CLI fixture as though it populates a live Desktop workspace.

## Manifest and scope

Initialize a local manifest, explicitly authorize one test folder, perform the metadata-only scan, and inspect manifest statistics:

```bash
cargo run -p deskgraph-cli -- manifest init --database ./deskgraph-dev.sqlite3
cargo run -p deskgraph-cli -- scope add \
  --database ./deskgraph-dev.sqlite3 \
  --path /absolute/path/to/test-folder
cargo run -p deskgraph-cli -- scan start \
  --database ./deskgraph-dev.sqlite3 \
  --scope 1
cargo run -p deskgraph-cli -- manifest stats \
  --database ./deskgraph-dev.sqlite3
```

`scope add` canonicalizes the explicit local boundary and records a versioned consent receipt. It does not start a scan. The initial manifest scan reads names and filesystem metadata inside the authorized boundary; content extraction is a separate capability.

Scope paths are returned only through explicit scope-management or other user-requested responses. Structured logs omit them.

### Durable scan jobs

Create a scan job that can be inspected, paused, resumed, or advanced in bounded batches:

```bash
cargo run -p deskgraph-cli -- scan create \
  --database ./deskgraph-dev.sqlite3 \
  --scope 1
cargo run -p deskgraph-cli -- scan status \
  --database ./deskgraph-dev.sqlite3 \
  --job 1
cargo run -p deskgraph-cli -- scan advance \
  --database ./deskgraph-dev.sqlite3 \
  --job 1 \
  --batch-size 256
cargo run -p deskgraph-cli -- scan pause \
  --database ./deskgraph-dev.sqlite3 \
  --job 1
cargo run -p deskgraph-cli -- scan resume \
  --database ./deskgraph-dev.sqlite3 \
  --job 1
cargo run -p deskgraph-cli -- scan run \
  --database ./deskgraph-dev.sqlite3 \
  --job 1
```

Scan observations remain in job-scoped staging while a scan is running or paused. The visible manifest is replaced only after the complete job publishes atomically.

## Content extraction

Run bounded extraction for an already-scanned file:

```bash
cargo run -p deskgraph-cli -- extract start \
  --database ./deskgraph-dev.sqlite3 \
  --scope 1 \
  --path /absolute/path/to/test-folder/notes.md
cargo run -p deskgraph-cli -- extract stats \
  --database ./deskgraph-dev.sqlite3
```

The current extraction surface includes text, Markdown, source code, text-layer PDF, DOCX, PPTX, XLSX, and bounded encoded image metadata for PNG, JPEG, GIF, WebP, BMP, and TIFF.

- PDF extraction ignores active content and attachments and records page/fragment provenance.
- Office extraction reads allowlisted in-memory text parts and does not execute macros, formulas, relationships, external links, or embedded objects.
- Image metadata is signature-checked and stores bounded encoded format/dimensions without copying pixels, EXIF, GPS, filenames, or paths into image-metadata records.
- Publication revalidates the authorized scope, manifest snapshot, and open-file identity before replacing prior complete output.

Read the structured result for a completed image-metadata job:

```bash
cargo run -p deskgraph-cli -- extract image-metadata \
  --database ./deskgraph-dev.sqlite3 \
  --job 1
```

Automation may use `--node` instead of `--path`. Durable extraction controls are available through `extract create`, `run`, `status`, `list`, `cancel`, and `resume`.

Ordinary job JSON and structured logs contain fixed IDs, statuses/error codes, byte counts, chunk counts, and timing rather than paths, filenames, or extracted text.

## Screenshot OCR development path

On macOS, start the bounded PNG/JPEG screenshot OCR path for an already-scanned image:

```bash
cargo run -p deskgraph-cli -- extract ocr-start \
  --database ./deskgraph-dev.sqlite3 \
  --scope 1 \
  --path /absolute/path/to/test-folder/Screenshot.png
```

The current macOS provider uses Apple Vision, requires Traditional Chinese (`zh-Hant`) and English (`en-US`) recognition, and publishes text to FTS only after complete-result and source-identity validation.

The development boundary caps source bytes, dimensions, total pixels, output size, observation count, and caller processing time. OCR status/log payloads contain fixed IDs, counts, and codes rather than recognized text or paths; an explicit later search response may return an authorized path and bounded untrusted snippet.

Restricted application sandboxes may deny Vision processing even when the language probe succeeds. Signed-package, clean-machine entitlement, memory, and complete cross-platform runtime evidence remain release work.

Windows provider code and cfg checks are present, but Windows OCR runtime support is not a release claim. It requires package identity and installed recognizers for requested `zh-TW` and `en-US`; the application does not install language features. See the implementation ledger for the current Windows/MSIX, cancellation/cleanup, packaging, quality, and fallback gates.

## Model-free lexical search

Search current metadata and active extracted text without a model:

```bash
cargo run -p deskgraph-cli -- search \
  --database ./deskgraph-dev.sqlite3 \
  --query "專案 context" \
  --scope 1 \
  --source content \
  --extension md
```

The current lexical path uses SQLite FTS5 trigram indexes. Search is an explicit content-returning operation, so stdout intentionally contains matching authorized paths and bounded snippets for the user who requested them. Structured stderr logs omit query text, paths, filenames, and snippets.

Useful options:

- omit `--scope` to search all authorized scopes in the local database;
- `--source` accepts `all`, `metadata`, or `content`;
- `--extension` accepts one 1–16 character ASCII-alphanumeric suffix, with or without a leading dot;
- `--modified-since` is inclusive and `--modified-before` is exclusive, both using UTC Unix seconds;
- `--limit` accepts 1–50;
- queries shorter than three Unicode characters fail closed rather than scanning the corpus.

Vector search, semantic retrieval, and hybrid fusion are separate open work; see [`planning/IMPLEMENTATION_STATUS.md`](planning/IMPLEMENTATION_STATUS.md).

## Folder profile and Project roots

Read one bounded, model-free Folder Profile after scanning the folder:

```bash
cargo run -p deskgraph-cli -- folder profile \
  --database ./deskgraph-dev.sqlite3 \
  --scope 1 \
  --path /absolute/path/to/test-folder
```

The explicit response contains the selected canonical folder path, bounded aggregate counts, category counts, and any marker-based Project Suggestion. Ordinary structured logs omit the selected path and descendant names.

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

`propose` re-derives and validates current evidence before persistence; it does not accept a candidate. `decide` appends an explicit user correction. Explicit propose/decide/status responses may contain the current root path; ordinary listing and structured logs remain path-free.

Acceptance is graph feedback only. It does not create file membership or perform a filesystem action.

## Exact-duplicate review

Compare two canonical, already-scanned files for exact byte equality without changing them:

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

The comparison requires canonical non-symlink files with different stable identities in the same authorized scope. DeskGraph revalidates manifest metadata and read-only open-handle identities and performs a bounded byte comparison.

A successful observation or explicit verification may return the two requested paths. `relation list` remains path-free and labels history as requiring fresh verification. A decision never merges, deletes, renames, moves, or otherwise organizes either file.

## Conservative filename-version review

Suggest and revalidate a directional numeric filename-version relation without reading file content:

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

The normalized base name and extension must match. The current rule accepts explicit numeric suffixes such as `-vN`, `_vN`, ` vN`, or `.vN` with bounded positive integers. Modification time, file size, words such as `final`, and content do not determine version order.

A user decision remains graph feedback only; it does not authorize a filesystem action.

## Watch reconciliation development path

Inject an explicit watch hint and advance its durable reconciliation state:

```bash
cargo run -p deskgraph-cli -- watch observe \
  --database ./deskgraph-dev.sqlite3 \
  --scope 1 \
  --path /absolute/path/to/test-folder/notes.md
cargo run -p deskgraph-cli -- watch advance \
  --database ./deskgraph-dev.sqlite3 \
  --event 1
cargo run -p deskgraph-cli -- watch list \
  --database ./deskgraph-dev.sqlite3
```

Hints are treated as untrusted. Ambiguous events fall back to full-scope reconciliation. The Desktop development coordinator also retains a bounded periodic safety path for eligible scanned scopes.

This is not a claim of complete incremental Watch Mode or automatic incremental content indexing. Create/delete/rename/subtree deltas, complete incremental extraction/indexing, cloud-placeholder behavior, power/thermal policies, production-scale evidence, and platform-complete runtime remain open as documented in the implementation ledger.

## Organization preview

Create a durable same-folder rename **preview** without changing the filesystem:

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

The explicit preview/status response returns requested before/after paths and policy checks; ordinary history and structured logs remain path-free.

The internal transaction/journal foundation is not a user execution capability. Current production builds deliberately keep general file-action execution unavailable until the required platform identity/fence and runtime safety evidence exists. CLI/Desktop do not expose production Execute/Undo/action-recovery controls for this path.

## Desktop development

Start the Tauri desktop application:

```bash
pnpm desktop:dev
```

The desktop health surface exposes bounded application/runtime state rather than filesystem locations. Explicit scope management and user-requested search/review surfaces may return the local paths the user explicitly requested.

Extracted snippets are treated as untrusted local text and rendered as data, not executable markup or agent instructions.

The UI currently includes English, Traditional Chinese, Simplified Chinese, and Japanese catalogs. UI localization does not expand the narrower extraction/search/OCR language evidence boundary.

## Full development verification commands

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

A successful local command run proves only that environment and revision. Release claims additionally require the platform, packaging, memory, security, and runtime evidence documented in the planning/status material.

## Privacy and safety invariants

- No default whole-disk scan; access begins from explicit user scope.
- No permanent file deletion or trash-emptying capability.
- No LLM receives direct filesystem execution capability.
- Extracted/OCR text remains untrusted data.
- File-action planning must be previewed, revalidated, durably journaled, and recoverable before production execution can exist.
- Core scan/search paths are designed to work without a mandatory LLM, API key, Python, Docker, or Ollama.
- Structured logs minimize paths, filenames, queries, extracted text, and OCR content; explicit user-requested responses may return the data needed to satisfy that request.

## Evidence boundaries

For current claims, use [`planning/IMPLEMENTATION_STATUS.md`](planning/IMPLEMENTATION_STATUS.md). In particular, do not infer any of the following merely from development code or local tests:

- public v0.1 release readiness;
- signed/notarized macOS packaging;
- Windows installer/MSIX runtime acceptance;
- complete Windows OCR runtime support;
- clean-machine or no-egress proof;
- 8 GB release certification;
- complete vector/hybrid retrieval;
- complete incremental Watch Mode;
- production filesystem Execute/Undo support.

The checked-in 10,000-file manifest benchmark is documented at [`benchmarks/M1_10K_MANIFEST_SCAN.md`](benchmarks/M1_10K_MANIFEST_SCAN.md).
