# Phase 04 — Hybrid Retrieval

Implement milestone M3.

Build:
- SQLite FTS5 indexing
- vector provider abstraction
- local multilingual embedding provider
- embedding cache keyed by model version and content hash
- hybrid rank fusion
- filters for type, date, project, folder and source
- diagnostics for why a result ranked
- desktop search UI and CLI search

Do not bind domain code directly to one vector extension. If using sqlite-vec, pin and isolate it behind an adapter because it is pre-v1.

Search queries must work for:
- exact filename
- OCR text
- semantic description
- mixed Chinese and English
- “files like this”
- recent project context

Acceptance:
- search p50 and p95 benchmark
- multilingual evaluation set
- deterministic fallback when embeddings are disabled
- result explanation visible in UI
