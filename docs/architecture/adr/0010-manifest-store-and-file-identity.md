# ADR-010 — Manifest store and file identity

- Status: Accepted
- Date: 2026-07-16
- Owners: DeskGraph maintainers

## Context

M1 needs a durable local manifest that supports idempotent rescans, hard links, moves, Unicode paths, case differences, crash recovery, and later watch-mode reconciliation. A path cannot be the primary file identity: paths change, multiple paths can name the same file, and case behavior differs by platform. DeskGraph must not follow symlinks or escape an explicitly authorized scope while discovering metadata.

The stable Rust 1.97 standard library exposes Unix device/inode metadata, but its Windows volume serial and file-index accessors are still guarded by the unstable `windows_by_handle` feature. A Windows-target compile probe confirmed that those accessors cannot be used by this release.

## Decision

- Use SQLite through `rusqlite`, with bundled SQLite, as the local manifest source of truth.
- Apply embedded, checksummed, forward-only schema migrations before manifest access. Reject a database when an already-applied migration checksum differs.
- Store stable graph node IDs separately from locations. A location is a scope-bound observation, not file identity.
- On Unix, identify files and directories by filesystem device and inode.
- On Windows, use Microsoft `windows-sys` bindings for `CreateFileW` and `GetFileInformationByHandle`, and derive identity from volume serial number plus file index. Handles are opened for metadata only, shared for read/write/delete, and always closed.
- Use a normalized comparison key for scope and location matching while retaining a lossless platform-native path representation. Normalize Unicode to NFC; additionally case-fold the comparison key on Windows.
- Use lexical traversal beneath a canonical authorized root. Inspect entries with `symlink_metadata`, do not follow symlinks or junction/reparse-point-like entries, and record the skip as a scan issue.
- Read metadata only in M1. File content extraction begins behind a separate M2 boundary.
- Preserve a path-derived fallback identity only for platforms or filesystems where a stable metadata identity is unavailable. Mark fallback observations explicitly so later reconciliation cannot mistake them for stable identity.

## Consequences

Moves on the same filesystem preserve graph node identity when the operating system supplies stable metadata. Hard links resolve to one node with multiple observed locations. SQLite remains a single local file and needs no service, API key, Python, Docker, or model runtime.

The Windows binding introduces a small target-specific unsafe boundary. It must remain isolated, documented, and covered by Windows compilation and integration tests. Bundled SQLite increases binary size but avoids depending on an unknown system SQLite feature set. Paths remain sensitive local data: they may be stored in the local manifest but must not enter logs, telemetry, network requests, or ordinary health payloads.

## Alternatives considered

- Path-only identity was rejected because it duplicates nodes after moves and cannot represent hard links safely.
- Rust standard-library Windows metadata accessors were rejected because the required API is unstable on the pinned toolchain.
- A filesystem watcher as the initial source of truth was rejected because watcher streams are lossy and platform-specific; M6 will reconcile them against this manifest.
- A server database or graph database was rejected by ADR-001 and the local-first product boundary.

## Validation and revisit trigger

Acceptance requires idempotent rescan tests, hard-link and move-identity tests, symlink-loop and scope-escape fixtures, Unicode/case fixtures, migration tests, Windows target compilation, and a measured 10,000-file scan. Revisit the identity implementation when Rust exposes stable equivalent Windows APIs, or when a supported filesystem cannot provide sufficiently stable metadata identity.
