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
- Bounded DOCX/PPTX/XLSX extraction with exact no-default ZIP/XML dependencies, allowlisted in-memory parts, namespace-aware streaming, inert macros/formulas/relationships/embeddings, archive/XML resource defenses, structural paragraph/slide/cell provenance, forward-safe migration, and atomic Manifest-to-FTS integration.
- Bounded PNG/JPEG/GIF/WebP/BMP/TIFF header metadata with exact minimal-feature `imagesize`, no pixel/EXIF/GPS decoding, signature/extension checks, dimension and probe limits, atomic structured SQLite publication, stale-source invalidation, and a path-free CLI result.
- Bounded macOS Apple Vision Screenshot OCR for PNG/JPEG bytes with runtime `zh-Hant`/`en-US` capability checks, durable cancellation, deadline and resource limits, normalized spatial/confidence provenance, atomic SQLite/FTS publication, stale-source invalidation, path-free job status, and explicit CLI commands.
- Bounded Windows `Windows.Media.Ocr` provider code for owned PNG/JPEG bytes with package-identity preflight, requested `zh-TW`/`en-US` passes plus actual recognizer validation, absent-confidence provenance, zero-angle source-box policy, exact spatial de-duplication, terminal-only async close, bounded caller return, and single-worker cancellation cleanup. Host policy tests and Windows cfg checks pass; real Windows/MSIX/runtime, memory, installer, and fallback evidence remain open.
- Forward-only OCR provenance migration that keeps bounding boxes mandatory while preserving explicit absence when a provider exposes no confidence score.
- Workspace-only bounded OCR provider evaluator with exact corpus/run digest binding, strict executable JSON fixtures, canonical-text CER, separate attempt/completed latency, failure histograms, per-reading RSS provenance, path/text-free reports, and explicit missing-evidence output. It is decision tooling only; no fallback provider or representative corpus is selected.
- Bounded macOS Apple Vision evaluation runner with an explicit private asset manifest/root, symlink/reparse and open-handle identity checks, exact image/corpus/manifest digest binding, shared production OCR limits/output validation, fixed redacted errors, versioned text reconstruction, sensitive `0600` no-clobber output, and runner-to-evaluator integration tests.
- Explicit Desktop Screenshot OCR controls on eligible search results, with scope/node-only create/lookup, job-ID-only run/status/cancel/resume, durable retry/recovery states, backend media/scope/identity revalidation, path/text-free job payloads, and no automatic OCR.
- Dependency-free English and Traditional Chinese Desktop UI catalogs with allowlisted browser-language detection, an always-available keyboard-accessible selector, safe local preference persistence, localized loading/empty/error/action states, and locale-aware number/UTC-date display.
- Deterministic offline SQLite FTS5 trigram indexes for current paths and active extracted chunks, with transactional backfill/synchronization, bounded quoted queries, Traditional Chinese/English fixtures, stale-content filtering, exact-filename fusion, and fixed ranking explanations.
- Privacy-aware lexical search commands in CLI and Tauri plus a Desktop search UI with scope selection, validated response schemas, bounded untrusted snippets, empty/error states, and no query/path/text logging.
- Workspace-only synthetic lexical benchmark with bounded corpus/iteration controls, no-overwrite policy, FTS index-size reporting, per-query p50/p95/max timing, and a checked-in 10k macOS arm64 baseline.
- Bounded lexical filters for authorized scope, metadata/extracted-text match source, normalized file extension, and inclusive/exclusive UTC modified-time range across CLI, Tauri, and Desktop UI.
- Durable watch-hint core with scope validation, per-scope storm coalescing, temporary-download exclusion, file stability/open-handle identity checks, atomic resumable-scan linkage, restart recovery, path-free CLI status, and an honest read-only Desktop panel.
- Immutable same-folder file rename previews with double scope/manifest/open-handle validation, portable-name and destination-conflict policy, atomic append-only SQLite journal creation, explicit before/after CLI output, and no filesystem execution path.
- Narrow Desktop rename-preview IPC and a validated UI with explicit before/after paths, nine passed policy checks, path-free recent history, loading/empty/error states, and no execute control.
- Bounded, read-only Folder Profiles derived from current authorized manifest locations with deterministic category facts and fail-closed 100,000-entry limits.
- Explainable, model-free Project Suggestions from direct project markers with basis-point confidence, complete marker provenance, observation time, creator/provider/version metadata, README-only rejection, privacy-aware CLI output, and no automatic membership edge.
- Durable Project root candidates keyed by stable manifest identity with immutable rule observations, normalized signals, database-side current-evidence validation, and append-only user feedback.
- Privacy-aware `project propose`, `decide`, `status`, and path-free `list` CLI flows with idempotent repeated decisions, opposite-decision correction sequences, rejected-root suppression, and no automatic file membership or filesystem action.
- Bounded exact-duplicate relation candidates for two explicit canonical files with full byte equality, canonical-scope and open-handle identity revalidation, 64 MiB/resource limits, stable endpoint ordering, immutable observations, fixed provenance, and no file action.
- Privacy-aware `relation duplicate` and live `relation verify` CLI flows; explicit responses return the selected paths while structured logs omit paths, filenames, database locations, and content.
- Append-only exact-pair relation feedback with mandatory live byte revalidation before every explicit accept/reject, idempotent repeated decisions, correction sequences, preserved state on later observations, and a path-free `relation list` history marked for verification.
- Deterministic filename-version candidates for explicit numeric suffixes with Unicode-normalized base/extension matching, directional version evidence, double manifest/open-handle validation, immutable observations, migration preservation of existing relation feedback, privacy-safe CLI verification/history, and no content read or file action.
- Evidence-bound filename-version feedback with mandatory live revalidation, append-only observation provenance, idempotent retries, auditable opposite corrections, fresh `suggested` state for changed direction, and privacy-safe `relation version-decide` output without a file action.
