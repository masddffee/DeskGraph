# Dependency Audit

Last reviewed: 2026-07-16

## Policy

Every new runtime, build, model, binary, crate, npm package, and GitHub Action must record official source, resolved version, purpose, maintenance evidence, supported platforms, license, and security notes. Lockfiles and checksums are authoritative for actual resolved artifacts. Candidates are not approved merely because they appear in this file.

## M0 selected dependencies

| Dependency | Scope | Version policy | Official source / API evidence | Maintenance | Platforms | License | Security and decision |
| --- | --- | --- | --- | --- | --- | --- | --- |
| Rust toolchain | Build | stable; local install observed 1.97.0 | `rust-lang.org/tools/install`, rustup static distribution | Active stable channel; rustfmt/clippy official components | macOS/Windows/Linux | MIT OR Apache-2.0 | Official `rustup-init` SHA-256 verified before execution |
| Tauri (`tauri`, `tauri-build`, `@tauri-apps/api`, CLI) | Runtime/build | major 2, pinned by Cargo/pnpm lockfiles | `v2.tauri.app`; crates.io; npm; `tauri-apps/tauri` | Tauri API 2.11.1 and CLI 2.11.4 were recently published at assessment time | macOS/Windows/Linux | MIT OR Apache-2.0 | System webview reduces bundled runtime; capability file grants no filesystem/shell plugin |
| serde / serde_json | Runtime | major 1, pinned by Cargo.lock | crates.io and `serde.rs` | Widely maintained Rust serialization ecosystem | Rust-supported targets | MIT OR Apache-2.0 | Only serializes a closed health schema; no untrusted deserialization in M0 |
| tracing / tracing-subscriber | Runtime | 0.1 / 0.3, pinned by Cargo.lock | crates.io and `tokio-rs/tracing` | Active Tokio project | Rust-supported targets | MIT | Logs only event names and status; path/content fields are prohibited |
| React / React DOM | Runtime UI | 19.2 line, pinned by pnpm lock | `react.dev`, npm, `facebook/react` | React 19.2.7 recently published at assessment time | Tauri webviews on desktop platforms | MIT | UI only; no filesystem API and no raw model output |
| Vite / official React plugin | Build | Vite 8 / plugin 6, pinned by pnpm lock | `vite.dev`, npm, `vitejs` GitHub | Vite 8.1.x and plugin 6.0.3 current at assessment time | Node 20.19+, 22.12+, or 24+; output is cross-platform web assets | MIT | Development/build-only; Node 24.12.0 satisfies requirements |
| TypeScript | Build | stable compatible line, pinned by pnpm lock | `typescriptlang.org`, npm, `microsoft/TypeScript` | Active Microsoft project | Any supported Node host | Apache-2.0 | Strict/no-emit configuration; compatibility verified by install/typecheck |
| ESLint + typescript-eslint | Build | ESLint 10 / typescript-eslint 8, pinned by pnpm lock | `eslint.org`, `typescript-eslint.io`, npm | Current releases and documented security policy | Node 20.19+, 22.13+, or 24+ | MIT | Development-only; lockfile and audit required; no auto-fix in CI |
| Vitest | Test | 4.x, pinned by pnpm lock | `vitest.dev`, npm, `vitest-dev/vitest` | Current Vite-native runner | macOS/Windows/Linux Node hosts | MIT | M0 tests pure utilities in Node environment; no browser emulator dependency |
| pnpm | Build | exact `packageManager` field | `pnpm.io`, npm, `pnpm/pnpm` | 11.10.0 current at assessment time | macOS/Windows/Linux | MIT | Deterministic lockfile; frozen installs in CI |
| Prettier | Build | current 3.x, pinned by pnpm lock | `prettier.io`, npm, `prettier/prettier` | Active formatter | Node-supported platforms | MIT | Development-only formatting check |

## GitHub Actions

Only official `actions/*` actions are permitted in M0. Every `uses:` entry must be pinned to a full commit SHA with the release tag retained in a comment. CI permissions default to `contents: read`. No secrets are exposed to fork pull requests.

## Verification still required after lockfile generation

- Record exact resolved direct dependency versions from `Cargo.lock` and `pnpm-lock.yaml`.
- Run `cargo tree` and `pnpm licenses list` (or an equivalent reproducible license report).
- Run Rust and npm advisory/audit tools; triage rather than silently suppress findings.
- Revisit native transitive dependencies when packaging begins.
- Audit every OCR, embedding, vector, model, archive, PDF, and Office dependency separately before M2/M3/M9 adoption.

## Rejected or deferred at M0

- No database crate: M0 reports the database honestly as `not_initialized`; SQLite begins in M1 after migration and binding evaluation.
- No OCR, vector, embedding, or LLM runtime: they are not needed for the foundation slice.
- No Tauri filesystem, shell, HTTP, opener, or updater plugin: M0 health requires none of those capabilities.
- No telemetry/crash SDK: privacy and governance decision is unresolved.
