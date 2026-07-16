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
| Vitest | Test | 4.1.10 | `vitest.dev`, npm, `vitest-dev/vitest` | Ten frontend contract tests pass locally | MIT | Node environment only; no browser emulator dependency |
| pnpm | Build | 11.10.0 | `pnpm.io`, npm, `pnpm/pnpm` | Corepack activation, lockfile, frozen-compatible install, and peer check pass | MIT | Exact `packageManager`; strict peers and supply-chain policy check enabled |
| Prettier | Build | 3.9.5 | `prettier.io`, npm, `prettier/prettier` | Repository formatting check passes | MIT | Development only; planning/prompts are excluded to preserve the supplied SSOT text |
| cargo-audit | Audit tool, not shipped | 0.22.2 | crates.io, `docs.rs/cargo-audit`, `rustsec/rustsec` | RustSec official project; requires Rust 1.88+; local audit executed | Apache-2.0 OR MIT | Installed outside the project and not added to the application dependency graph |

## M1 selected dependencies

| Dependency | Scope | Selected version | Official source / API evidence | Maintenance and platform evidence | License | Security and decision |
| --- | --- | --- | --- | --- | --- | --- |
| `rusqlite` / `libsqlite3-sys` | Runtime | 0.40.1 / 0.38.1 | crates.io, `docs.rs/rusqlite`, `rusqlite/rusqlite`; official examples confirm `Connection`, transactions, batch pragmas, and the `bundled` feature | Current crates.io release inspected on 2026-07-16; upstream supports Rust desktop targets through SQLite and `cc` | MIT | Disable rusqlite defaults and enable only `bundled`; avoids an unknown system SQLite and the default WASM backend. Manifest migrations are embedded and checksummed. |
| `unicode-normalization` | Runtime | 0.1.25 | crates.io, `docs.rs/unicode-normalization`, `unicode-rs/unicode-normalization` | Current crates.io release inspected on 2026-07-16; pure Rust and platform-independent | MIT OR Apache-2.0 | NFC comparison keys reduce canonically equivalent path duplicates. This is not a security substitute for canonical scope validation. |
| `windows-sys` | Windows runtime only | 0.61.2 | crates.io, Microsoft `microsoft/windows-rs`, generated Windows API docs; verified signatures for `CreateFileW`, `GetFileInformationByHandle`, `CloseHandle`, and `BY_HANDLE_FILE_INFORMATION` | Current crates.io release inspected on 2026-07-16; Microsoft-maintained; Rust 1.71 minimum; Windows-only target dependency | MIT OR Apache-2.0 | Enable only `Win32_Foundation`, `Win32_Security`, and `Win32_Storage_FileSystem`. Unsafe calls stay in one identity adapter; metadata-only access uses shared handles and deterministic close. |
| `clap` | CLI runtime | 4.6.2 | crates.io, `docs.rs/clap`, `clap-rs/clap`; official derive and nested subcommand examples inspected | Current crates.io release inspected on 2026-07-16; Rust 1.85 minimum; pure Rust CLI parser | MIT OR Apache-2.0 | Schema-derived CLI rejects ambiguous input and supports future M1 job controls without custom parsing. |
| `tempfile` | Tests/bench fixtures only | 3.27.0 | crates.io, `docs.rs/tempfile`, `Stebalien/tempfile` | Current crates.io release inspected on 2026-07-16; Rust 1.63 minimum; cross-platform | MIT OR Apache-2.0 | Dev-only fixture isolation. Never used by product runtime or as a permanent-delete product capability. |

### M1 dependency verification notes

- Rust 1.97 `std::os::windows::fs::MetadataExt::{volume_serial_number,file_index,number_of_links}` was compiled against `x86_64-pc-windows-msvc` and rejected with `E0658 windows_by_handle`; it is not a viable stable implementation.
- The selected Microsoft binding exposes the required stable Win32 APIs. Windows CI must compile and run identity fixtures before M1 can be considered cross-platform verified.
- `rusqlite` currently defaults to `cache` and `ffi-sqlite-wasm-rs`; DeskGraph explicitly opts out of defaults and selects bundled native SQLite.
- The M1 lockfile resolves 456 crate dependencies. `cargo audit --no-fetch` against 1,160 cached RustSec advisories found zero known vulnerabilities and the same 17 Tauri Linux-path warnings tracked in R-016; the new M1 direct dependencies added no advisory.

## M2 text-extraction dependency decision

The first M2 provider adds **no external dependency**. Plain text, Markdown, and source code use Rust standard-library `Read + Seek`, UTF-8 validation, bounded buffering, chunking, and time/cancellation checks. Durable jobs and content chunks reuse the already audited `rusqlite` database layer; open-file identity reuses the existing `unicode-normalization` and `windows-sys` boundary; `tempfile` remains test-only. The `Cargo.lock` changes for this slice only connect existing DeskGraph workspace crates and do not introduce a new registry package.

This decision keeps the core usable without Python, Docker, Ollama, a model, an API key, or network access. It does not approve any PDF, ZIP/XML, image, OCR, model, or native runtime candidate.

### M2 dependencies still requiring selection and audit

| Capability | Current status | Required evidence before adoption |
| --- | --- | --- |
| PDF text | Unselected | Official API and repository, active maintenance, macOS/Windows/Linux packaging, license, advisories, JavaScript/action/attachment behavior, page/byte/time limits, corrupt fixtures |
| DOCX / PPTX / XLSX | Unselected | ZIP and XML APIs, traversal/decompression defenses, macro/external-link/embedded-object behavior, structural limits, platform packaging, license, advisories, corrupt fixtures |
| Image metadata | Unselected | Bounded signature/metadata API, supported formats, malformed/oversized behavior, license, advisories, platform behavior |
| Screenshot OCR | Unselected; D-008 open | Native API availability plus packaged cross-platform fallback, zh-TW/English quality, model/runtime license, checksums, memory/unload behavior, offline packaging without user-installed Python |

No candidate may enter `Cargo.toml`, `package.json`, build scripts, or release assets until its row is replaced by verified source/API/version/license/security evidence and an accepted provider-specific decision.

## GitHub Actions

Only official `actions/*` actions are permitted in M0. `actions/checkout` v4.2.2 is pinned to `11bd71901bbe5b1630ceea73d27597364c9af683`; `actions/setup-node` v6.4.0 is pinned to `48b55a011bda9f5d6aeb4c2d9c7362e8dae4041e`. CI permissions default to `contents: read`, and no secrets are exposed to fork pull requests. Remote execution evidence remains blocked until the GitHub repository exists.

## Executed verification

- `Cargo.lock` resolves 457 packages including eight DeskGraph workspace packages. `cargo metadata --offline --no-deps` found no missing workspace license metadata. License expressions include permissive licenses, MPL-2.0, and optional-license expressions containing LGPL; platform-specific redistribution and notices require another M9 review.
- `cargo tree --workspace --depth 1` recorded all direct versions. `cargo tree --target all -i ...` traced RustSec warnings to Tauri/Wry's Linux GTK3 stack and Tauri's URL-pattern parser chain.
- `cargo audit --no-fetch` loaded 1,160 cached RustSec advisories and scanned all 457 lockfile packages: zero known vulnerabilities plus the same 17 warnings—ten unmaintained GTK3 binding crates, `proc-macro-error`, five unmaintained `unic-*` crates, and one `glib` unsound advisory. These are not suppressed and remain R-016.
- `pnpm audit --prod` and full `pnpm audit` found zero known vulnerabilities.
- `pnpm licenses list --json` failed with `ERR_PNPM_MISSING_PACKAGE_INDEX_FILE` under pnpm's SQLite-backed local store. The recorded equivalent scan read all installed package manifests: 145 unique packages, no missing license fields; 106 MIT, 18 Apache-2.0, 6 BSD-2-Clause, 2 BSD-3-Clause, 7 ISC, 2 MPL-2.0, 1 BlueOak-1.0.0, and 3 Apache-2.0 OR MIT.
- `pnpm peers check` reports no peer dependency issues after pinning TypeScript 6.0.3.

## Verification still required

- Execute frozen installs and all checks in remote macOS, Windows, and Linux CI.
- Revisit native transitive dependencies, redistribution notices, and R-016 when packaging begins.
- Re-run the Rust dependency and license audit after every future lockfile change.
- Audit every OCR, embedding, vector, model, archive, PDF, and Office dependency separately before M2/M3/M9 adoption.

## Rejected or deferred at M0

- M0 intentionally had no database crate. M1 adopts the audited SQLite binding above; the health contract may report it ready only after initialization succeeds.
- No OCR, vector, embedding, or LLM runtime: they are not needed for the foundation slice.
- No Tauri filesystem, shell, HTTP, opener, or updater plugin: M0 health requires none of those capabilities.
- No telemetry/crash SDK: privacy and governance decision is unresolved.
