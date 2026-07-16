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
