# Decisions Needed

Last reviewed: 2026-07-16

## Blocking now

No product decision blocks local M0 implementation. GitHub remote ownership blocks only remote CI/Issue/Release evidence.

## Open decisions

| ID | Decision | Needed by | Options / constraints | Default until decided |
| --- | --- | --- | --- | --- |
| D-001 | GitHub owner and public repository location | M0 CI verification | Personal vs organization ownership; repository name and default branch protection | Local Git only; do not invent a remote |
| D-002 | Minimum supported macOS version and Intel strategy | M9 design, benchmark fixtures earlier | Universal binary vs split arm64/x64; Tauri/WebKit support and clean machines determine floor | Build both architectures in CI design; no support claim yet |
| D-003 | Windows installer format | M9 | NSIS first vs MSI; signing and updater compatibility | Evaluate Tauri-recommended NSIS and MSI; no decision from preference alone |
| D-004 | Opt-in telemetry and crash reporting | Before public beta | None, local-only export, or privacy-reviewed opt-in provider | No telemetry and no network egress |
| D-005 | Linux experimental artifact | M9 | AppImage first, `.deb` optional; cannot delay macOS/Windows | AppImage candidate, explicitly experimental |
| D-006 | Default scope presets and platform Screenshots discovery | M1/M8 | Desktop/Downloads/Documents plus platform-aware screenshots; every path still user-confirmed | Show presets but grant nothing until explicit selection |
| D-007 | SQLite Rust binding and vector adapter | M1/M3 | Evaluate API, bundling, license, FTS5, Windows/macOS/Linux, and migration behavior | No dependency selected |
| D-008 | OCR provider stack | M2 | Native providers plus packaged cross-platform fallback; zh-TW + English | No Python/user-installed runtime; no provider selected |
| D-009 | Embedding model/runtime | M3/M9 | Multilingual, int8, license, checksum, memory, model-removal support | Deterministic lexical retrieval remains required |
| D-010 | Product trademark/name clearance and reverse-DNS identifier | Before signed release | DeskGraph availability and legal review | Development identifier only; no trademark claim |

## Decisions made in M0

- Version B is the target; Version A is only an internal milestone.
- Project code license: Apache-2.0 (recorded in ADR-008).
- Rust workspace + Tauri 2 + React/TypeScript + pnpm follows the accepted architecture.
- Health data is a closed, privacy-safe schema shared by CLI and desktop; no paths, filenames, content, identifiers, or model output.
