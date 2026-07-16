# DeskGraph

> **Pre-release foundation — not ready for personal file indexing.**

**Graphify your computer.**

DeskGraph is a local-first computer context graph that will connect, search, and safely organize files from folders you explicitly authorize. It is designed to expose narrowly scoped, read-only local context to AI agents without uploading your private filenames, paths, extracted content, OCR, embeddings, or graph data by default.

## Current state

The repository is implementing M0 Repository Foundation. The first runnable slice is a privacy-safe health check shared by the CLI and Tauri desktop shell. Scanning, extraction, search, organization, undo, and MCP are planned but **not shipped**.

## Safety contract

- No permanent file deletion.
- No LLM can execute filesystem operations.
- Every future move or rename must be previewed, policy-validated, durably journaled, crash-recoverable, and undoable.
- No path is accessed outside an explicit user scope.
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

Start the desktop application:

```bash
pnpm desktop:dev
```

The health report includes only the application version, OS/architecture, database lifecycle state, optional-provider state, and privacy flags. It does not include filesystem locations.

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
