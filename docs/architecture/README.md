# Architecture

Rust owns filesystem, database, graph, retrieval, planning, execution, transaction, and MCP behavior. Tauri + React/TypeScript owns presentation. SQLite is the source of truth, and OCR, embeddings, vector search, and local LLMs remain lazy provider interfaces.

Accepted decisions are recorded in `docs/planning/09_DECISIONS_ADR.md` and new ADRs in `docs/architecture/adr/`.

M0 implements only the shared health contract, CLI, desktop shell, privacy-safe structured logging, and build/test foundation. Later directories are created only when their milestone produces a tested vertical slice.
