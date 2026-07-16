# Decisions Needed

Last reviewed: 2026-07-16

## Blocking now

No product decision blocks local M0 implementation. GitHub remote ownership blocks only remote CI/Issue/Release evidence.

## Open decisions

| ID    | Decision                                                    | Needed by                             | Options / constraints                                                                        | Default until decided                                                      |
| ----- | ----------------------------------------------------------- | ------------------------------------- | -------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------- |
| D-001 | GitHub owner and public repository location                 | M0 CI verification                    | Personal vs organization ownership; repository name and default branch protection            | Local Git only; do not invent a remote                                     |
| D-002 | Minimum supported macOS version and Intel strategy          | M9 design, benchmark fixtures earlier | Universal binary vs split arm64/x64; Tauri/WebKit support and clean machines determine floor | Build both architectures in CI design; no support claim yet                |
| D-003 | Windows installer format                                    | M9                                    | NSIS first vs MSI; signing and updater compatibility                                         | Evaluate Tauri-recommended NSIS and MSI; no decision from preference alone |
| D-004 | Opt-in telemetry and crash reporting                        | Before public beta                    | None, local-only export, or privacy-reviewed opt-in provider                                 | No telemetry and no network egress                                         |
| D-005 | Linux experimental artifact                                 | M9                                    | AppImage first, `.deb` optional; cannot delay macOS/Windows                                  | AppImage candidate, explicitly experimental                                |
| D-006 | Default scope presets and platform Screenshots discovery    | M1/M8                                 | Desktop/Downloads/Documents plus platform-aware screenshots; every path still user-confirmed | Show presets but grant nothing until explicit selection                    |
| D-007 | Vector adapter after SQLite manifest                        | M3                                    | SQLite binding is decided in ADR-010; evaluate vector API, license, multilingual behavior, and migrations | No vector dependency selected; deterministic lexical fallback required     |
| D-008 | OCR provider stack                                          | M2                                    | Native providers plus packaged cross-platform fallback; zh-TW + English                      | No Python/user-installed runtime; no provider selected                     |
| D-009 | Embedding model/runtime                                     | M3/M9                                 | Multilingual, int8, license, checksum, memory, model-removal support                         | Deterministic lexical retrieval remains required                           |
| D-010 | Product trademark/name clearance and reverse-DNS identifier | Before signed release                 | DeskGraph availability and legal review                                                      | Development identifier only; no trademark claim                            |

## Decisions made in M0

- Version B is the target; Version A is only an internal milestone.
- Project code license: Apache-2.0 (recorded in ADR-008).
- Rust workspace + Tauri 2 + React/TypeScript + pnpm follows the accepted architecture.
- Health data is a closed, privacy-safe schema shared by CLI and desktop; no paths, filenames, content, identifiers, or model output.

## Decisions made in M1

- Bundled SQLite through `rusqlite 0.40.1` is the manifest source of truth.
- File identity is separate from location: Unix device/inode and Windows volume serial/file index through an isolated `windows-sys` adapter.
- Unicode comparison keys use NFC; Windows additionally uses case-insensitive comparison keys. Canonical scope validation remains the security boundary.
- Initial scan is metadata-only and never follows symlinks or Windows reparse points.
- Resumable scans use a persistent path queue and job-scoped staging; only a completed job publishes the live manifest atomically. Pause state and an expiring runner lease survive process exit (ADR-011).
