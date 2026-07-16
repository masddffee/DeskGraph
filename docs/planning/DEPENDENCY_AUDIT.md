# Dependency Audit

Last reviewed: 2026-07-16

## Policy

Every new runtime, build, model, binary, crate, npm package, and GitHub Action must record official source, resolved version, purpose, maintenance evidence, supported platforms, license, and security notes. Lockfiles and checksums are authoritative for actual resolved artifacts. Candidates are not approved merely because they appear in this file.

## M0 selected dependencies

| Dependency | Scope | Resolved version | Official source / API evidence | Maintenance and platform evidence | License | Security and decision |
| --- | --- | --- | --- | --- | --- | --- |
| Rust toolchain | Build | 1.97.0 with rustfmt/clippy | `rust-lang.org/tools/install`, official rustup static distribution | Current stable toolchain installed; local macOS arm64 checks pass; remote macOS/Windows/Linux pending | MIT OR Apache-2.0 | `rustup-init` SHA-256 verified before execution |
| Node.js | Build | 24.12.0 primary; 24.1.0 clean-clone check | `nodejs.org` official distribution and release metadata | Supported range is `^20.19.0 || ^22.13.0 || >=24.0.0`; CI pins 24.12.0 on all three OS runners | MIT | Build host only; no Node runtime shipped in the Tauri app |
| Tauri Rust (`tauri`, `tauri-build`) | Runtime/build | 2.11.5 / 2.6.3 | `v2.tauri.app`, crates.io, `tauri-apps/tauri` | Local Rust build, debug bundle, launch, and IPC smoke pass; cross-platform CI pending | MIT OR Apache-2.0 | Capability grants no filesystem/shell plugin; RustSec Linux-path warnings tracked as R-016 |
| Tauri JS API / CLI | Runtime/build | 2.11.1 / 2.11.4 | `v2.tauri.app`, npm, `tauri-apps/tauri` | Local install/build/bundle pass; cross-platform CI pending | MIT OR Apache-2.0 | No filesystem, shell, HTTP, opener, or updater plugin is installed |
| serde / serde_json | Runtime | 1.0.228 / 1.0.150 | crates.io, `serde.rs`, `serde-rs` repositories | Local compile/tests pass; Rust-supported targets claimed upstream | MIT OR Apache-2.0 | Closed health serialization schema; no M0 untrusted deserialization |
| tracing / tracing-subscriber | Runtime | 0.1.44 / 0.3.23 | crates.io, `tokio-rs/tracing` | Local compile/tests pass; active Tokio project | MIT | Fixed event fields, JSON stderr, no target/file/line/span, and explicit path redaction assertions |
| React / React DOM | Runtime UI | 19.2.7 / 19.2.7 | `react.dev`, npm, `facebook/react` | Local typecheck/tests/build and Tauri webview smoke pass | MIT | Presentation only; no direct filesystem capability or raw model output |
| Vite / React plugin | Build | 8.1.4 / 6.0.3 | `vite.dev`, npm, `vitejs` repositories | Local production build passes on Node 24.12.0 | MIT | Development/build only; frozen resolution in `pnpm-lock.yaml` |
| TypeScript | Build | 6.0.3 | `typescriptlang.org`, npm, `microsoft/TypeScript` | Strict typecheck passes | Apache-2.0 | Pinned exactly because TypeScript 7.0.2 exceeded typescript-eslint's declared peer range |
| ESLint / typescript-eslint | Build | 10.7.0 / 8.64.0 | `eslint.org`, `typescript-eslint.io`, npm | `pnpm peers check` and zero-warning lint pass | MIT | Development only; strict peers are enabled and cannot be silently auto-installed |
| Vitest | Test | 4.1.10 | `vitest.dev`, npm, `vitest-dev/vitest` | Four frontend contract tests pass locally | MIT | Node environment only; no browser emulator dependency |
| pnpm | Build | 11.10.0 | `pnpm.io`, npm, `pnpm/pnpm` | Corepack activation, lockfile, frozen-compatible install, and peer check pass | MIT | Exact `packageManager`; strict peers and supply-chain policy check enabled |
| Prettier | Build | 3.9.5 | `prettier.io`, npm, `prettier/prettier` | Repository formatting check passes | MIT | Development only; planning/prompts are excluded to preserve the supplied SSOT text |
| cargo-audit | Audit tool, not shipped | 0.22.2 | crates.io, `docs.rs/cargo-audit`, `rustsec/rustsec` | RustSec official project; requires Rust 1.88+; local audit executed | Apache-2.0 OR MIT | Installed outside the project and not added to the application dependency graph |

## GitHub Actions

Only official `actions/*` actions are permitted in M0. `actions/checkout` v4.2.2 is pinned to `11bd71901bbe5b1630ceea73d27597364c9af683`; `actions/setup-node` v6.4.0 is pinned to `48b55a011bda9f5d6aeb4c2d9c7362e8dae4041e`. CI permissions default to `contents: read`, and no secrets are exposed to fork pull requests. Remote execution evidence remains blocked until the GitHub repository exists.

## Executed verification

- `Cargo.lock` resolves 430 packages including four DeskGraph workspace packages. `cargo metadata` found no missing license metadata. License expressions include permissive licenses, MPL-2.0, and optional-license expressions containing LGPL; platform-specific redistribution and notices require another M9 review.
- `cargo tree --workspace --depth 1` recorded all direct versions. `cargo tree --target all -i ...` traced RustSec warnings to Tauri/Wry's Linux GTK3 stack and Tauri's URL-pattern parser chain.
- `cargo audit` loaded 1,160 RustSec advisories and found zero known vulnerabilities plus 17 warnings: ten unmaintained GTK3 binding crates, `proc-macro-error`, five unmaintained `unic-*` crates, and one `glib` unsound advisory. These are not suppressed and remain R-016.
- `pnpm audit --prod` and full `pnpm audit` found zero known vulnerabilities.
- `pnpm licenses list --json` failed with `ERR_PNPM_MISSING_PACKAGE_INDEX_FILE` under pnpm's SQLite-backed local store. The recorded equivalent scan read all installed package manifests: 145 unique packages, no missing license fields; 106 MIT, 18 Apache-2.0, 6 BSD-2-Clause, 2 BSD-3-Clause, 7 ISC, 2 MPL-2.0, 1 BlueOak-1.0.0, and 3 Apache-2.0 OR MIT.
- `pnpm peers check` reports no peer dependency issues after pinning TypeScript 6.0.3.

## Verification still required

- Execute frozen installs and all checks in remote macOS, Windows, and Linux CI.
- Revisit native transitive dependencies, redistribution notices, and R-016 when packaging begins.
- Audit every SQLite, OCR, embedding, vector, model, archive, PDF, and Office dependency separately before M1/M2/M3/M9 adoption.

## Rejected or deferred at M0

- No database crate: M0 reports the database honestly as `not_initialized`; SQLite begins in M1 after migration and binding evaluation.
- No OCR, vector, embedding, or LLM runtime: they are not needed for the foundation slice.
- No Tauri filesystem, shell, HTTP, opener, or updater plugin: M0 health requires none of those capabilities.
- No telemetry/crash SDK: privacy and governance decision is unresolved.
