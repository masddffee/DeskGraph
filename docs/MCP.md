# DeskGraph MCP

DeskGraph currently provides one independently launched, local stdio MCP server with one read-only tool: `search_files`. It searches only an existing DeskGraph SQLite manifest and only scope IDs granted when the process starts. It cannot authorize or scan folders, start extraction, preview or execute organization, rename, move, delete, or call a model.

目前 DeskGraph 提供一個獨立啟動的本機 stdio MCP server，且只有一個唯讀工具：`search_files`。它只搜尋既有的 DeskGraph SQLite manifest，也只能使用程序啟動時明確授權的 scope ID；不能授權或掃描資料夾、啟動抽取、預覽或執行整理、重新命名、移動、刪除，亦不會呼叫模型。

## Build and prepare a scope

Build the standalone binary:

```bash
cargo build --release -p deskgraph-mcp
```

Use DeskGraph Desktop or the CLI to authorize and complete an Initial Manifest Scan first. The CLI prints the scope ID from `scope add`; it can also list existing IDs:

```bash
cargo run -p deskgraph-cli -- scope list --database /absolute/path/to/deskgraph.sqlite3
```

The database path must be absolute and must already be a regular, non-symlink file with the exact supported DeskGraph migration history. Every granted scope must exist and have a completed scan. The MCP process fails closed instead of creating, migrating, repairing, or opening the database read-write.

## Launch

Run directly, repeating `--scope-id` only for the minimum scopes the client needs:

```bash
/absolute/path/to/deskgraph-mcp \
  --database /absolute/path/to/deskgraph.sqlite3 \
  --scope-id 1 \
  --scope-id 4
```

MCP protocol messages use stdout. Privacy-safe session audit events use stderr and are not a durable audit journal.

## Configure a client

Codex CLI:

```bash
codex mcp add deskgraph -- \
  /absolute/path/to/deskgraph-mcp \
  --database /absolute/path/to/deskgraph.sqlite3 \
  --scope-id 1
```

Equivalent Codex `config.toml` entry:

```toml
[mcp_servers.deskgraph]
command = "/absolute/path/to/deskgraph-mcp"
args = ["--database", "/absolute/path/to/deskgraph.sqlite3", "--scope-id", "1"]
```

In ChatGPT Desktop, open **Settings → MCP servers → Add server**, select **STDIO**, enter the executable plus its argument list, save, and restart ChatGPT. ChatGPT on the web cannot launch this local stdio process. Other MCP clients should use the same direct executable and argument array; do not wrap the command in `sh -c` or interpolate untrusted values.

## Tool contract and privacy boundary

`search_files` requires a Traditional Chinese or English lexical query of 3–256 Unicode characters and one launch-granted `scope_id`. Optional filters are metadata versus extracted content, ASCII-alphanumeric extension, UTC modification bounds, and a result limit of 1–20. Content snippets are absent by default and are available only when `source` is `content` and `include_snippet` is explicitly true.

Tool-content JSON is capped at 24 KiB and the complete JSON-RPC response frame has a verified 64 KiB budget. String request IDs are capped at 128 bytes before the SDK so an echoed ID cannot break that output budget. Each path and explicitly requested snippet is capped at 2 KiB. Paths carry `untrusted_file_metadata`; snippets carry `untrusted_extracted_text`; both include a fixed never-follow-instructions boundary. Input frames over 64 KiB are discarded. Read-only searches configure a two-second SQLite progress deadline and a one-second lock-wait ceiling, after which a fixed error is returned; deterministic interruption cleanup and subsequent-request recovery pass, while wall-clock lock/runtime evidence remains a later platform gate. The server exposes no arbitrary path input and no second tool.

DeskGraph itself performs no network request or upload in this MCP process. However, the MCP client or the model provider chosen by the user may transmit returned paths or snippets elsewhere. Grant the fewest scopes possible, keep snippets disabled unless needed, and review that client's privacy policy.

SQLite WAL sidecars are a narrow storage exception to “read-only”: the main manifest is opened with SQLite `READ_ONLY` plus `query_only`, but SQLite may create or update the adjacent `-wal`/`-shm` coordination files so a reader can coexist with the Desktop writer. The server never changes manifest rows or source files and never falls back to a read-write main-database connection. SQLite officially supports read-only WAL access when valid sidecars already exist, when they can be created, or when a database is immutable; for reliable live-Desktop use, keep the database directory and any sidecars accessible to the same local account. DeskGraph deliberately does not use `immutable=1`, because the Desktop writer may still be active and its WAL state must not be ignored.

The current MCP slice is enabled only on macOS and Linux, whose exact bundled SQLite VFS opens both WAL and SHM files with OS `O_NOFOLLOW`; DeskGraph additionally validates existing sidecars as ordinary non-link files before and after opening. Other targets fail closed. In particular, SQLite's Windows VFS can follow a reparse point when opening a WAL shared-memory sidecar, so DeskGraph will not enable this server there until a safe sidecar/VFS or writer-published snapshot design passes junction, race, ACL and live-writer tests. Main-database ancestor swap-and-restore hardening remains an explicit M7 gate because rusqlite does not expose SQLite's held main-file descriptor for identity comparison.

## Current limitations

This is the first macOS-arm64-verified M7 vertical slice, not complete MCP support. Windows fails closed as described above, and Linux/macOS Intel runtime remain unverified. It has no semantic/vector/hybrid tool, project or related-file tool, folder profile tool, organization-preview tool, resource subscription, remote transport, bundled client registration, or release installer integration. Those remain gated by their own product milestones and acceptance tests.
