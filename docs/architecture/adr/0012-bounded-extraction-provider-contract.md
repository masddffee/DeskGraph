# ADR-012 — Bounded extraction providers receive controlled sources

- Status: Accepted
- Date: 2026-07-16
- Owners: DeskGraph maintainers

## Context

M2 must extract text, Markdown, source code, PDF, Office documents, image metadata, and screenshot OCR without executing document-controlled behavior or turning third-party parsers into arbitrary filesystem clients. Extracted text may contain prompt injection and must remain untrusted. Malformed or oversized input must fail one file without crashing or partially replacing previously usable content.

The core product must continue to work without Python, Docker, an API key, a downloaded model, or an LLM. OCR and complex document formats will require dependencies, but none may be selected before their real API, license, maintenance, packaging, platform behavior, and security limits are verified.

## Decision

- Rust orchestration owns scope, canonical path, stable identity, file stability, and metadata revalidation before opening a source.
- An extractor provider receives a controlled `Read + Seek` source plus validated metadata. It does not receive an arbitrary path or filesystem capability.
- Every provider declares a stable provider ID, version, supported media kinds, and limits. Routing uses validated metadata, extension hints, and bounded signature inspection; document text never selects executable behavior.
- Limits include source bytes, decompressed bytes where applicable, pages/sheets/slides, output bytes, chunks, and active processing time. Providers check cancellation between bounded work units.
- Plain text, Markdown, and source code use a dependency-free built-in UTF-8 provider first. Invalid encoding is recorded per file rather than guessed silently.
- OOXML providers may inspect only the required ZIP/XML parts. They never execute macros, scripts, relationships, external links, OLE objects, or embedded attachments.
- PDF providers extract text and metadata only. JavaScript, launch actions, attachments, multimedia, and external references are ignored.
- OCR uses a separate provider interface. Platform-native providers are preferred when maintainable; any fallback must be packaged without requiring a user-installed Python runtime. Traditional Chinese and English fixtures are mandatory before an OCR provider is complete.
- All extracted text is stored with the trust class `untrusted_extracted_text`, source node/location identity, provider/version, byte or structural offsets, and timestamps.
- Per-file output is staged. Only a complete, non-cancelled extraction atomically replaces that file's prior active chunks. Failure or cancellation records a fixed error code and preserves the prior complete extraction.
- Logs, health payloads, and ordinary job status never contain paths or extracted text.

## Consequences

Provider implementations are easier to fuzz and test because they operate on bounded streams rather than navigating the filesystem. Format-specific dependencies remain isolated and optional. The built-in UTF-8 provider supplies useful deterministic functionality before PDF, Office, OCR, embeddings, or models exist.

Some native OCR APIs prefer file URLs. Adapters must decode controlled bytes or use a core-owned, private temporary representation with identical limits; they may not expand the provider interface into arbitrary path access. Large-format performance will require measured streaming and decompression policies rather than reading whole archives into memory.

## Alternatives considered

- Giving every provider a filesystem path was rejected because it broadens scope enforcement and makes parser behavior harder to contain.
- A generic archive extractor was rejected because it increases archive-bomb, traversal, and attachment-execution risk.
- Requiring Python or a local model for the first extraction slice was rejected by the offline core-product requirement.
- Storing partial chunks during extraction was rejected because retrieval must not observe an incomplete document version as current truth.

## Validation and revisit trigger

Each provider requires valid, corrupt, oversized, cancelled, and provenance/offset fixtures. Archive-based providers additionally require traversal, decompression-limit, macro-enabled, external-link, and embedded-object fixtures. Revisit the controlled-source shape only when a verified native platform API cannot consume bounded bytes or streams; any exception needs a new ADR and must keep core-owned scope enforcement.
