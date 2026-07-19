# Architecture

Rust owns filesystem, database, graph, retrieval, planning, execution, transaction, and MCP behavior. Tauri + React/TypeScript owns presentation. SQLite is the source of truth, and OCR, embeddings, vector search, and local LLMs remain lazy provider interfaces.

Accepted decisions are recorded in `docs/planning/09_DECISIONS_ADR.md` and new ADRs in `docs/architecture/adr/`.

The current Project Graph boundary is defined by ADR-018 through ADR-023, ADR-028 through ADR-029, and ADR-032: derived Folder Profiles, immutable/correctable Project roots, bounded exact-byte duplicate candidates, append-only exact-pair feedback that requires live revalidation, conservative explicit-numeric filename-version suggestions, evidence-bound directional corrections, explainable screenshot review, suggest-only Cleanup aggregation, and bounded manifest-only Project Discovery with path-progressive review.

M0 implements only the shared health contract, CLI, desktop shell, privacy-safe structured logging, and build/test foundation. Later directories are created only when their milestone produces a tested vertical slice.
