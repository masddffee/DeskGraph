# DeskGraph

> **Pre-release development build — use only with test folders and keep backups.**

**Graphify your computer.**

DeskGraph is a local-first computer context graph that will connect, search, and safely organize files from folders you explicitly authorize. It is designed to expose narrowly scoped, read-only local context to AI agents without uploading your private filenames, paths, extracted content, OCR, embeddings, or graph data by default.

## Current state

The repository is implementing M2 Content Intelligence plus bounded M3 lexical, M4 project-graph, M5 safe-organization protocol, M6 watch-reconciliation, M7 read-only MCP, M8 product-UI, and M9a native-scope slices while M0/M1 external evidence remains open. The CLI and Tauri desktop can initialize a local SQLite manifest, explicitly authorize one or more non-overlapping folders in one native selection, run a metadata-only initial scan per folder, persist progress, pause or resume safely, recover interrupted scan/extraction work, report graph statistics, and search current local paths and active extracted text. Packaged Desktop selection is Rust-owned: the WebView submits no authorization path, every selected root and opaque grant commits in one bounded transaction, and schema guards prevent overlapping roots from becoming simultaneously active through any grant writer. An upgrade quarantines narrower legacy grants when a prior broad/narrow pair already exists; corrupt or unrestorable grants likewise require reauthorization. Authorization never starts a scan. The local macOS implementation creates and restores versioned security-scoped bookmarks with balanced live access; debug development receipts are rejected in release builds, and Windows/Linux release authorization fails closed until real platform adapters exist. Its sandbox profile permits outbound client sockets because the macOS WebKit networking subprocess fails closed without that entitlement, but the current product contains no remote content/upload client, requires no network to function, and its production CSP excludes development HTTP/WebSocket origins. This is not signed-package or clean-machine evidence. The complete current Desktop surface is available in English, Traditional Chinese, Simplified Chinese, and Japanese; language selection is local and does not expand the narrower Traditional Chinese/English extraction, search, or OCR evidence boundary. Rescans are idempotent in local tests; hard links share an identity, same-filesystem renames preserve identity, and symlinks and hidden entries are not followed.

The current content slices can extract bounded text from an explicitly selected, already-scanned text, Markdown, source-code, text-layer PDF, DOCX, PPTX, or XLSX file. They can also read encoded dimensions and format from PNG, JPEG, GIF, WebP, BMP, and TIFF headers without decoding pixels or collecting EXIF/GPS fields. A macOS development slice runs Apple Vision OCR over bounded PNG/JPEG bytes, requires `zh-Hant` and `en-US`, and stores normalized top-left boxes plus provider confidence. Windows provider code uses package-identity-gated `Windows.Media.Ocr`, requests `zh-TW` and `en-US`, validates the actual recognizers as Traditional Chinese and English, stores required boxes with absent confidence, and rejects rotated coordinates it cannot truthfully map. The shared job and publication layer revalidates the authorized scope, manifest snapshot, and actual open-file identity, supports durable cancellation/recovery, and atomically preserves the prior complete output on failure. Text is stored only as provenance-bearing `untrusted_extracted_text` chunks. PDF extraction rejects encryption, ignores active content and attachments, and records page/fragment provenance. Office extraction reads only allowlisted in-memory text parts, ignores macros/formulas/relationships/embeddings, rejects unsafe or bomb-like archives/XML, and records paragraph/slide/sheet-cell provenance instead of fabricated byte offsets. Image metadata is stored separately as bounded structured data, not as text or FTS content.

The current search slice uses bundled SQLite FTS5 trigram indexes for Traditional Chinese and English substring queries of 3–256 Unicode characters. It requires no embedding or model, returns bounded text snippets, filters out stale chunks and absent locations, and explains whether filename/path, extracted text, or both matched. Scope, source, extension, and modified-time filters plus a synthetic 10k p50/p95/index-size baseline pass locally. One- and two-character queries, project/folder filters, vector semantic search, hybrid fusion, representative/100k/8 GB evaluation, and cross-platform evidence remain open.

The current M4 slices derive bounded Folder Profiles, persist correctable Project root candidates, discover up to 100 current marker-backed roots from one explicitly selected active scanned scope, compare two explicit current files as a bounded exact-duplicate suggestion, and recognize a conservative filename-version relation. Ordinary discovery is path-free; the current root path appears only in an explicit transient review, and accept/reject remains append-only feedback without membership or file actions. Root, exact-duplicate pair, and version decisions are append-only; every duplicate or version decision repeats its complete live verification. Version inference accepts only matching normalized base/extension names with explicit `-vN`, `_vN`, ` vN`, or `.vN` suffixes and orders the numeric versions. Version feedback is bound to that exact directional evidence, so changed direction or version numbers return to `suggested`. No relation creates file membership or a filesystem action. M5 now has an internal append-only command/journal protocol, immutable SHA-256/root/parent/source execution bindings, idempotent request receipts, and bounded recovery observations. ADR-026 records that current macOS/Linux rename syscalls cannot atomically condition the source leaf on the exact held inode; general Unix Rename/Move therefore remain Preview-only rather than inheriting the test adapter. ADR-027 accepts a package-identity-first process-fence topology: v0.1 keeps one Tauri Rust action host and Windows selects package family identity plus a protected private-namespace mutex. macOS `flock` remains only a candidate until a selected OS floor and signed App Sandbox/SIP container prove that a non-entitled same-user process cannot silently replace the fence entry; a failed proof leaves macOS production actions unavailable. M9a implements the local macOS picker/bookmark/grant foundation but not the signed protected-container proof, platform fence, or Windows package identity. CLI exposes Preview, Status, and path-free History; Desktop exposes Preview and path-free History. No production action adapter exists. Windows handle execution, System Trash, Move planning, user recovery/Undo, and the full fault/runtime matrix remain open. M6 feeds bounded native filesystem hints into durable reconciliation while retaining a five-minute full-scope safety path; one exact same-identity Unix file modification can use an atomic metadata delta, and every ambiguous case falls back to a resumable full-scope scan. Packaged Desktop Watch additionally requires the durable active grant and live runtime scope guard. It is not yet complete incremental Watch Mode or automatic content indexing, and Watch never initiates a file action.

The independently launched local stdio MCP slice exposes only bounded lexical `search_files` over an existing read-only manifest and launch-granted completed-scan scopes. It has no write tools or arbitrary path parameters; content snippets are opt-in and remain labeled untrusted. See [MCP setup and limitations](docs/MCP.md).

ADR-033's add-only hard-exclusion slice is implemented and workspace-verified locally: Settings uses native file/folder pickers rather than a WebView-supplied path; a current active platform grant, host platform, live runtime access, canonical stable identity/kind, and policy revision are revalidated before a short-lived Preview can be applied. The stable identity is persisted privately and enforced during scan publication, extraction and retrieval, so a same-scope hard-link alias cannot reintroduce excluded bytes. One immediate SQLite transaction records the exclusion, advances the policy revision, and purges affected derived manifest paths, FTS, extracted/OCR records, image metadata, graph/project/relation/screenshot/cleanup derivations and pristine Preview-only action data. Any action journal that progressed beyond its initial Preview blocks the change and remains available for recovery/audit. Vector embeddings are not implemented yet and must join this exclusion/revision/purge boundary before they ship. The receipt is path-free and the source filesystem is never changed. This is local workspace evidence, not a signed-package acceptance claim. Exclusion removal/revocation, signed macOS and packaged Windows acceptance, clean-machine, concurrent-race and crash matrices remain open.

Scope-exclusion removal/revocation, signed macOS App Sandbox scope/container acceptance and replacement proof, packaged Windows package-family identity, clean-machine/concurrent-race/crash evidence, Windows OCR runtime support and fallback routing, the packaged cross-platform OCR provider, both action-fence runtimes, vector/hybrid retrieval, Project file-membership, related/similarity and general version discovery, background Project/duplicate discovery, cross-pair learning, complete incremental Watch Mode, all organization execution/Undo including Move/System Trash, remaining MCP tools, and client/installer integration are **not implemented or shipped**. The atomic multi-root picker, add-only local exclusion/privacy-purge slice, Office, image-metadata, macOS OCR, Windows OCR code/cfg, local macOS bookmark foundation, native-hint/full-scope reconciliation, scoped MCP search, and fail-closed M5 protocol work are development evidence, not public-release support; Windows has not passed a real MSIX/package-identity/language/cancellation run, and scanned/image-only PDFs are not routed through OCR yet. Representative document/image/OCR corpora, peak-memory evidence, complete cross-platform runtime evidence, live interaction with the current authorization/Watch/OCR/rename controls, and the installer/release pipeline are open, so this is not a public v0.1 release.

## Safety contract

- No permanent file deletion.
- Smart Cleanup can only move an explicitly confirmed, revalidated item to the operating-system trash through the durable transaction engine; it cannot empty trash, and Undo may claim success only while the exact trashed item still exists and matches its action receipt.
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

## Build Week one-command demo

Use a brand-new path to generate harmless bilingual sample files and run the real local backends
end to end. The command refuses to overwrite an existing path.

```bash
cargo run -p deskgraph-cli -- fixture demo --path /absolute/new/path/deskgraph-demo
```

The command creates a synthetic authorized scope plus a separate SQLite database, then verifies a
metadata scan, bounded Markdown/code extraction, Traditional Chinese and English FTS results, a
marker-backed Project candidate, exact-duplicate and numeric-version evidence, and Smart Cleanup
Inbox derivation plus a durable `system_trash_preview`. The Preview requires confirmation while
`action_authorized` and `execution_available` remain false. Its JSON report includes
`source_files_unchanged: true` and confirms that no organization action was performed. It uses no
OCR, model, API key, network service, Python, Docker, or Ollama. Because this is an explicit CLI
response, its stdout intentionally returns the generated demo paths and search snippets;
structured logs remain path/content-free.

This reproducible CLI proof has its own generated database. It does not populate the Desktop
application's private app-data database and must not be presented as though both processes share
one live state.

Run the M1 metadata-only CLI slice with a new local manifest and a test folder you explicitly choose:

```bash
cargo run -p deskgraph-cli -- manifest init --database ./deskgraph-dev.sqlite3
cargo run -p deskgraph-cli -- scope add --database ./deskgraph-dev.sqlite3 --path /absolute/path/to/test-folder
cargo run -p deskgraph-cli -- scan start --database ./deskgraph-dev.sqlite3 --scope 1
cargo run -p deskgraph-cli -- manifest stats --database ./deskgraph-dev.sqlite3
```

`scope add` canonicalizes the explicit local boundary and atomically records a versioned, path-free current-host CLI consent receipt. It does not scan. `scan start` reads names and filesystem metadata within that boundary but does not open file contents. Scope paths are returned only by explicit scope-management commands and UI; structured logs omit them.

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

Run the current bounded text/PDF/Office/image-metadata extraction slice for one file already discovered by the scan:

```bash
cargo run -p deskgraph-cli -- extract start \
  --database ./deskgraph-dev.sqlite3 \
  --scope 1 \
  --path /absolute/path/to/test-folder/notes.md
cargo run -p deskgraph-cli -- extract stats --database ./deskgraph-dev.sqlite3
```

Use the same command with a `.pdf`, `.docx`, `.pptx`, `.xlsx`, `.png`, `.jpg`, `.jpeg`, `.gif`, `.webp`, `.bmp`, `.tif`, or `.tiff` path. PDFs must contain a text layer; Office formulas, macros, relationships, external links, and embedded objects are never executed or traversed. Image inputs are signature-checked and only bounded encoded dimensions are stored; pixels, EXIF, GPS, filenames, and paths are not copied into image metadata. After an image job completes, read that structured result with its returned job ID:

```bash
cargo run -p deskgraph-cli -- extract image-metadata \
  --database ./deskgraph-dev.sqlite3 \
  --job 1
```

On macOS, run the current bounded PNG/JPEG Screenshot OCR development slice for an already-scanned image:

```bash
cargo run -p deskgraph-cli -- extract ocr-start \
  --database ./deskgraph-dev.sqlite3 \
  --scope 1 \
  --path /absolute/path/to/test-folder/Screenshot.png
```

The provider receives encoded bytes rather than a path, checks that both Traditional Chinese (`zh-Hant`) and English (`en-US`) recognition are available, and publishes text to FTS only after the complete result and source identity pass validation. Source bytes are capped at 32 MiB, dimensions at 16,384 per side, total pixels at 64 Mi pixels, output at 8 MiB, observations at 4,096, and caller processing at 60 seconds absolute maximum. OCR status/log payloads contain fixed IDs, counts and codes but no path or recognized text; an explicit later `search` response may return the authorized path and bounded untrusted snippet. Restricted application sandboxes may deny Vision processing even when the language probe succeeds, so release packaging and clean-machine entitlement/runtime evidence remain open.

Windows provider code and Rust cfg checks are included, but Windows runtime support is not. It requires package identity and installed recognizers to which the requested `zh-TW` and `en-US` languages resolve; it never installs language features. Cancellation cleanup may continue in one gated background worker after the caller deadline, and the native provider remains unavailable until that cleanup finishes. Real Windows/MSIX, language, OCR quality, cancellation/cleanup, memory, and installer evidence plus the packaged fallback are still required before users should run this path.

`extract start` opens only the manifest-backed file selected by the explicit path. Its ordinary job JSON and structured logs contain job IDs, fixed status/error codes, byte counts, chunks, and timing—not the path, filename, or extracted text. Automation may use `--node` instead of `--path`. Durable controls are available through `extract create/run/status/list/cancel/resume`.

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

`observe` validates the current authorized scope and persists a path-free status response. Events within one scope coalesce until the one-second stability deadline; run `advance` at or after `stable_after_unix_ms` to revalidate size, modified time, identity, and read-only access before an atomic manifest reconciliation.

While the Desktop process is open, a fallback coordinator uses a five-minute polling interval for scopes that already completed an explicit Initial Manifest Scan. Each cycle schedules at most four due scopes and advances at most one full-scope reconciliation batch, so backlog may delay completion beyond five minutes. Older deferred scopes retain priority over newly due scopes, foreground-scan contention waits one second before retry, and active lookup uses a partial status/deadline index. It resumes durable active events after restart, sleeps until the next deadline or foreground wake, and reports only closed path-free runtime fields, including deferred scope count. Authorization alone never starts scanning. Shutdown requests stop and waits for at most two seconds; a blocked worker may be detached and rely on atomic scan publication plus restart recovery rather than confirmed graceful termination. This remains a development fallback rather than native or incremental Watch Mode: native OS events, per-node reconciliation, incremental content re-extraction/indexing, cloud-placeholder handling, pause/battery/thermal policy, tray/autostart behavior, and 8 GB evidence are open.

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

The explicit preview/status response returns canonical before/after paths and passed policy checks; ordinary logs and `organize list` remain path-free. New previews bind the authorized root, parent, strong source identity, metadata, and a bounded SHA-256 snapshot to the durable plan; files above 8 GiB or requiring more than 90 seconds to hash fail closed. The internal command journal and recovery state machine are not a user execution capability. Because the current Unix rename primitives address a mutable source leaf name rather than the exact held inode, production builds deliberately return `action_platform_rename_unsupported`, and neither CLI nor Desktop exposes Execute, Undo, or action recovery controls. Move, folders, case-only execution, cross-volume behavior, System Trash, platform acceptance, complete process-kill/permission/durability/runtime evidence, and execution UI remain unimplemented.

Start the desktop application:

```bash
pnpm desktop:dev
```

The health report includes only the application version, OS/architecture, database lifecycle state, optional-provider state, and privacy flags. It does not include filesystem locations. Explicit scope management, user-invoked search, the CLI Folder Profile, and explicit before/after rename preview may return the path the user requested; ordinary logs plus extraction, watch, and recent action-history payloads omit paths and content. Search snippets are visibly labeled untrusted local text and rendered as text, never executable markup. The English/Traditional Chinese Watch panel explicitly labels its five-minute-interval bounded polling fallback, exposes deferred scope count, and does not claim native or incremental behavior; the organizer panel explicitly reports that no execute control exists.

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
- [Read-only MCP setup](docs/MCP.md)
- [OpenAI Build Week submission pack](docs/HACKATHON_SUBMISSION.md)
- [Contributing](CONTRIBUTING.md)
- [Security policy](SECURITY.md)

DeskGraph is licensed under [Apache-2.0](LICENSE).
