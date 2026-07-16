# Changelog

All notable changes to DeskGraph will be documented here. The project follows Semantic Versioning once release tags begin.

## [Unreleased]

### Added

- Repository assessment and Version B v0.1 task graph.
- M0 governance, architecture, CI, and privacy-safe health slice.
- M1 bundled SQLite manifest with checksummed migration, stable file identity, canonical scope policy, and metadata-only scanner.
- CLI commands and desktop UI for explicit folder authorization, initial scan, and manifest graph statistics.
- Synthetic 10,000-file fixture generator and local idempotent-scan benchmark.
- Durable bounded scan queue, staged atomic manifest publishing, pause/resume, lease-based crash recovery, and replay after process interruption.
- CLI and desktop progress controls with paused and interrupted states derived from the local SQLite source of truth.
- Component-aware protected-system scope denial and macOS/Windows hidden/system metadata exclusions.
- Batch-level active-runner timing validated against an optimized 10,000-file release scan.
- Accepted bounded extractor contract with controlled streams, fixed untrusted-text classification, resource limits, cancellation, and atomic per-file publication.
- Dependency-free UTF-8 extraction for text, Markdown, and source code with Traditional Chinese/English offset fixtures and bounded overlapping chunks.
- Durable SQLite extraction jobs, content-chunk provenance, source snapshot/open-handle revalidation, lease recovery, cancellation, and stale-content invalidation.
- Privacy-safe extraction CLI with explicit scanned-file paths plus durable job controls and aggregate statistics.
- Read-only Desktop extraction statistics and recent-job states through narrow Tauri IPC and runtime-validated TypeScript schemas.
- Tagged content provenance with a forward migration that preserves existing byte ranges and adds page/fragment locations without fabricated offsets.
- Strict bounded text-layer PDF extraction with exact no-default-feature `lopdf`, encrypted-input rejection, inert JavaScript/actions/attachments, sequential page cancellation, and Traditional Chinese/English fixtures.
- Deterministic offline SQLite FTS5 trigram indexes for current paths and active extracted chunks, with transactional backfill/synchronization, bounded quoted queries, Traditional Chinese/English fixtures, stale-content filtering, exact-filename fusion, and fixed ranking explanations.
- Privacy-aware lexical search commands in CLI and Tauri plus a Desktop search UI with scope selection, validated response schemas, bounded untrusted snippets, empty/error states, and no query/path/text logging.
- Workspace-only synthetic lexical benchmark with bounded corpus/iteration controls, no-overwrite policy, FTS index-size reporting, per-query p50/p95/max timing, and a checked-in 10k macOS arm64 baseline.
- Bounded lexical filters for authorized scope, metadata/extracted-text match source, normalized file extension, and inclusive/exclusive UTC modified-time range across CLI, Tauri, and Desktop UI.
- Durable watch-hint core with scope validation, per-scope storm coalescing, temporary-download exclusion, file stability/open-handle identity checks, atomic resumable-scan linkage, restart recovery, path-free CLI status, and an honest read-only Desktop panel.
- Immutable same-folder file rename previews with double scope/manifest/open-handle validation, portable-name and destination-conflict policy, atomic append-only SQLite journal creation, explicit before/after CLI output, and no filesystem execution path.
