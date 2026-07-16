# ADR-015 — SQLite FTS5 trigram is the deterministic lexical baseline

- Status: Accepted
- Date: 2026-07-16
- Owners: DeskGraph maintainers

## Context

M3 requires metadata and full-text search to remain useful when embeddings, a model, an API key, and network access are unavailable. Search must cover Traditional Chinese and English, expose why a result ranked, ignore stale extraction versions, and keep query work bounded.

The already selected bundled SQLite is the local source of truth. Its built-in FTS5 `unicode61` tokenizer treats each contiguous run of Unicode letters and numbers as one token. That makes an unsegmented Chinese run a single token and does not provide the required general substring behavior. SQLite's built-in `trigram` tokenizer indexes contiguous sequences of three Unicode characters and supports case-insensitive substring matching without a new extension or runtime dependency.

SQLite documents that FTS5 full-text queries shorter than three Unicode characters cannot match a trigram index. Using `LIKE` without a usable trigram may fall back to a linear scan, which would violate the first bounded search contract on a large local corpus.

## Decision

- Use bundled SQLite FTS5 with the built-in case-insensitive `trigram` tokenizer for the v0.1 deterministic lexical baseline.
- Maintain separate external-content indexes for authorized location display paths and extracted content chunks. The source tables remain authoritative; FTS stores the index, not an additional product source of truth.
- Migration 0005 creates synchronization triggers and runs an initial `rebuild` so existing locations and chunks are indexed in the same migration transaction.
- Every metadata result joins a currently `present` location. Every content result joins both an `active` chunk and its currently `present` location. Historical or stale extraction rows may remain durable but cannot appear in current search.
- Accept only normalized queries from 3 through 256 Unicode characters. Reject empty, shorter, longer, or non-whitespace control-character queries with fixed error codes. Do not silently run a linear-scan fallback for shorter terms.
- Treat the entire normalized query as one quoted FTS phrase. Escape embedded quotes and bind the value as a SQL parameter; do not expose raw FTS operators or interpolate user input into SQL.
- Limit results to 50 and FTS candidates to 200. Order each source by FTS rank with stable identity tie-breakers, then deterministically fuse location and content hits. Exact filename matches rank first.
- Return a closed result contract with local path, an optional bounded text snippet, matched fields, a fixed explanation code, and final lexical rank. Extracted snippets remain `untrusted_extracted_text` and the UI renders them as text, never markup or instructions.
- Log only fixed events, counts, timing, mode, and scope IDs. Search queries, paths, filenames, and snippets must not enter ordinary logs.
- Keep vector and embedding providers outside this decision. Later hybrid retrieval must preserve this lexical-only path as the deterministic fallback.

## Consequences

Metadata and extracted-content search work offline with no new registry package, model, extension download, native service, or platform-specific runtime. Substring matching behaves consistently for queries of at least three Unicode characters across Traditional Chinese and English, and the application can explain whether path metadata, extracted text, or both produced a result.

One- and two-character queries are deliberately rejected in this slice. Future support requires an audited tokenizer or another bounded index; it may not be added as an unindexed scan. Trigram indexes consume more space than word-token indexes, so M3 disk, p50/p95 latency, and 8 GB corpus benchmarks remain open. FTS ranking is lexical only and cannot satisfy semantic-description or “files like this” acceptance criteria.

External-content consistency becomes a migration invariant. Any later migration that rebuilds `locations` or `content_chunks` must explicitly preserve or recreate the associated FTS tables and triggers, then run an integrity/rebuild check.

## Alternatives considered

- `unicode61` alone was rejected because contiguous Chinese text does not provide general substring matching.
- An unindexed `LIKE '%query%'` fallback was rejected because short queries may scan the complete local corpus.
- A custom tokenizer was deferred because it adds native API and maintenance surface before the baseline benchmark proves it necessary.
- A vector extension or embedding model was rejected for this slice because lexical search must remain independently usable and their versions, licenses, memory, packaging, and multilingual quality are not yet approved.
- Copying paths and full text into standalone FTS content tables was rejected because it duplicates sensitive local content and creates a second synchronization truth.

## Validation and revisit trigger

Required local evidence is: FTS5 creation on the bundled SQLite build, transactional migration and existing-row rebuild, synchronization triggers, Traditional Chinese and English path/content fixtures, exact filename fusion, stale/inactive/present filtering, quoted operator-like input, fixed query/candidate/result limits, CLI and Desktop contracts, and no-query/path/snippet logging. Full workspace format, clippy, tests, frontend lint/typecheck/tests/build, and Tauri build must pass.

Before M3 is complete, add p50/p95 latency and disk-size benchmarks on documented corpora, a multilingual evaluation set, metadata/date/folder/source filters, vector-provider abstraction, embedding cache, hybrid fusion, and semantic fallback tests. Revisit the tokenizer if those measurements show unacceptable index size/latency or if real Traditional Chinese evaluation requires bounded one- or two-character search.
