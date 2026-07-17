# Dependency Audit

Last reviewed: 2026-07-17

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
| Vitest | Test | 4.1.10 | `vitest.dev`, npm, `vitest-dev/vitest` | Nineteen frontend contract tests pass locally | MIT | Node environment only; no browser emulator dependency |
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

## M2 extraction dependency decisions

The first M2 provider adds **no external dependency**. Plain text, Markdown, and source code use Rust standard-library `Read + Seek`, UTF-8 validation, bounded buffering, chunking, and time/cancellation checks. Durable jobs and content chunks reuse the already audited `rusqlite` database layer; open-file identity reuses the existing `unicode-normalization` and `windows-sys` boundary; `tempfile` remains test-only. The `Cargo.lock` changes for this slice only connect existing DeskGraph workspace crates and do not introduce a new registry package.

This initial decision keeps the core usable without Python, Docker, Ollama, a model, an API key, or network access. It did not preapprove later ZIP/XML, image, OCR, model, or native runtime candidates; each accepted later delta is recorded separately below.

### PDF text dependency selected

| Dependency | Scope | Selected version | Official source / API evidence | Maintenance and platform evidence | License | Security and decision |
| --- | --- | --- | --- | --- | --- | --- |
| `lopdf` | PDF runtime | `0.44.0`, exact, `default-features = false` | crates.io, `docs.rs/lopdf`, `J-F-Liu/lopdf`; verified `LoadOptions::max_decompressed_size`, `Document::load_mem_with_options`, `extract_text_with_limit`, `get_pages`, `is_encrypted`, and `was_encrypted` in the published source | Released 2026-07-10; upstream active when inspected 2026-07-16; Rust 1.88 minimum; minimal crate test passed on macOS arm64 and Windows x64 cross-check compiled; no native PDF library | MIT | Accepted by ADR-013 for strict, bounded, path-free text-layer extraction only. Default features are forbidden. Actions, JavaScript, attachments, annotations, multimedia, external references, passwords, write APIs, and unbounded extraction APIs are outside the adapter. |

The isolated no-default-feature graph resolves 53 registry packages. All report license expressions and all provide a permissive licensing path; final notices/SBOM remain an M9 gate. `cargo audit --no-fetch` with 1,160 cached advisories reported zero vulnerabilities and zero warnings for this minimal graph. By contrast, lopdf's upstream full-feature lock contains vulnerable `crossbeam-epoch 0.9.18` (`RUSTSEC-2026-0204`); that graph is rejected, and CI must keep proving that `crossbeam-*`/Rayon do not enter DeskGraph through this dependency.

The load API limits each eagerly decompressed object or cross-reference stream, and the extraction API limits each page and `/ToUnicode` stream. It does not expose a whole-document aggregate allocator budget. DeskGraph therefore also enforces source bytes, page count, sequential page processing, stored output/chunks, cooperative time/cancellation, and keeps peak residency on an 8 GB machine as an open release gate (R-005/R-007).

`pdf-extract 0.12.0` is rejected: its published `extract_text*`/`extract_text_by_pages*` functions call unbounded `Document::load*` and output traversal, use `lopdf 0.42`, and accept no decompression, page, output, time, or cancellation policy.

### Image metadata dependency selected

| Dependency | Scope | Selected version/features | Official source / API evidence | Maintenance and platform evidence | License | Security and decision |
| --- | --- | --- | --- | --- | --- | --- |
| `imagesize` | Encoded image format and dimensions only | `0.15.0`, exact, `default-features = false`; only `bmp`, `gif`, `jpeg`, `png`, `tiff`, `webp` | crates.io and `Roughsketch/imagesize`; inspected published `reader_type` plus `ImageType::reader_size` reader APIs and the complete selected-format source | Released 2026-07-09; pure Rust; selected graph has no transitive dependency; isolated test passed on macOS arm64 and `x86_64-pc-windows-msvc` checked with pinned Rust 1.97.0 | MIT | Accepted for bounded header metadata only. DeskGraph never calls the crate's filesystem helper, decodes pixels, enables HEIF/AVIF, or extracts EXIF/GPS/arbitrary strings. |

The adapter receives only a core-controlled in-memory probe cursor. DeskGraph independently caps source/probe bytes, read/seek operations, time/cancellation, each dimension, and total encoded pixels; it verifies extension-to-signature agreement plus stricter format headers before publication. Metadata is stored in a dedicated structured table with source/provider provenance and no path, filename, EXIF, GPS, or fake text chunk. Malformed, mismatched, truncated, over-limit, cancelled, and changed sources publish nothing.

The isolated lock contains only the fixture root and `imagesize`; `cargo tree` confirms the six selected features and no transitive package. The published source scan found no `unsafe`, network, or process execution in the selected parsing path. `cargo audit --no-fetch` against 1,160 cached advisories reports zero findings for the isolated graph. The exact dependency adds one package to DeskGraph's lock and no new full-lock advisory. HEIF/AVIF remains deliberately excluded because its nested box traversal and format-specific size arithmetic have not passed DeskGraph's separate resource/safety contract; support is not implied by upstream capability.

### Screenshot OCR architecture and selected macOS binding

ADR-024 selects a native-first architecture, not a blanket approval for every OCR runtime. Apple Vision is the macOS provider, `Windows.Media.Ocr` is the Windows provider after a separate binding/runtime gate, and a packaged in-process Tesseract adapter is the fallback after its native-binary/model gate. PaddleOCR/ONNX Runtime is deferred; `ocrs` is rejected for v0.1 because its official model support does not cover Traditional Chinese.

| Dependency | Scope | Selected version/features | Official source / API evidence | Maintenance and platform evidence | License | Security and decision |
| --- | --- | --- | --- | --- | --- | --- |
| `objc2-vision` | macOS OCR runtime binding only | `0.3.2`, exact, `default-features = false`; `VNObservation`, `VNRecognizeTextRequest`, `VNRequest`, `VNRequestHandler`, `VNTypes`, `block2`, `objc2-core-foundation`, `std` | Apple [`VNRecognizeTextRequest`](https://developer.apple.com/documentation/vision/vnrecognizetextrequest)/[`VNImageRequestHandler`](https://developer.apple.com/documentation/vision/vnimagerequesthandler); published crate source and [`docs.rs/objc2-vision`](https://docs.rs/objc2-vision/0.3.2/objc2_vision/); verified bytes-only `initWithData:options:`, language capability, accurate recognition, top candidates, confidence, normalized bounding boxes and synchronous perform APIs; progress-handler-to-`cancel` binding compiles but runtime cancellation remains an implementation gate | [`madsmtm/objc2`](https://github.com/madsmtm/objc2) is the upstream repository; published metadata supports macOS arm64/x64 and Rust 1.71+. Rust 1.97 isolated compile and local macOS arm64 language/recognition probes pass; Intel and clean remote runtime remain | Zlib OR Apache-2.0 OR MIT | Accepted only as a target-specific thin binding. No model, network, subprocess, path/URL input, dynamic discovery, or user-installed runtime. Runtime buffers and output remain DeskGraph-bounded; OS model cache unload cannot be claimed. |

The exact published crate archive is 69,317 bytes with SHA-256 `bfc194758a2d5d7540b1ad283bfb9ca318ec608991892326e95b428230b2689b`. The complete isolated lock has nine packages: the fixture plus `objc2-vision 0.3.2`, `block2 0.6.2`, `dispatch2 0.3.1`, `objc2 0.6.4`, `objc2-core-foundation 0.3.2`, `objc2-encode 4.1.0`, `objc2-foundation 0.3.2`, and `bitflags 2.13.1`. All except `objc2-vision` already exist in DeskGraph's current lock, so adoption adds one package. `cargo audit --no-fetch` scanned the nine-package lock against 1,160 cached advisories and returned no finding.

The local runtime reported both `en-US` and `zh-Hant`. A controlled mixed-language PNG recognized `DeskGraph OCR` and `桌面圖譜 安全整理` with valid normalized bounding boxes when `zh-Hant` was ordered before `en-US`. The restricted runner returned an opaque nil platform error for the recognition call while its language probe still succeeded; the same bytes-only Swift and Rust calls passed outside that sandbox. This is local arm64 dependency/API evidence only, not macOS Intel, clean-machine, memory, corpus, cancellation, release, or M2 completion evidence.

### M2 OCR dependencies still requiring selection and audit

| Capability | Current status | Required evidence before adoption |
| --- | --- | --- |
| Windows native OCR binding | Architecture selected; dependency unaccepted | Exact `windows` feature graph, `OcrEngine`/`SoftwareBitmap` API, Language OCR Feature on Demand present/missing behavior, cancellation, Windows x64 runtime, license and lock audit |
| Packaged Tesseract fallback | Architecture selected; dependency/runtime/model unaccepted | Exact Tesseract/Leptonica/binding/data versions, `eng` + `chi_tra` bytes-only initialization, hashes, no CLI/dynamic discovery, cancellation/deadline, arm64/x64/Windows/Linux packaging, notices/SBOM, RSS/unload evidence |

### Office OOXML dependencies selected

ADR-014 accepts exact `zip =8.6.0` (`zip-rs/zip2`, MIT, Rust 1.88+) with `default-features = false` plus only `deflate-flate2-zlib-rs`, and exact `quick-xml =0.41.0` (`tafia/quick-xml`, MIT, Rust 1.79+) with `default-features = false`. The selected graph contains no archive crypto, Bzip2, Deflate64, LZMA, PPMd, XZ, Zstandard, async, Serde, encoding, or native C compression feature.

The archive API exposes central-directory construction, entry count, encrypted status, compressed/uncompressed sizes, overlap detection, enclosed-name validation, compression method, and bounded `Read`. The XML API exposes streaming `NsReader`, bounded namespace declarations, and explicit start/end/text/CDATA/DTD/processing-instruction/general-reference events. DeskGraph resolves only the transitional/strict WordprocessingML, DrawingML, and SpreadsheetML namespaces needed to prevent cross-namespace tag confusion; it does not use recursive extraction, entry writes, relationship traversal, password APIs, entity expansion, or whole-document deserialization.

The isolated exact lock contains 14 registry packages: `adler2 2.0.1`, `cfg-if 1.0.4`, `crc32fast 1.5.0`, `equivalent 1.0.2`, `flate2 1.1.9`, `hashbrown 0.17.1`, `indexmap 2.14.0`, `memchr 2.8.3`, `miniz_oxide 0.8.9`, `quick-xml 0.41.0`, `simd-adler32 0.3.10`, `typed-path 0.12.3`, `zip 8.6.0`, and `zlib-rs 0.6.6`; only the selected feature tree is active at build time. Their expressions are MIT, Apache-2.0, BSD-like, Unlicense, or Zlib compatible; notices/SBOM remain an M9 gate. With 1,160 cached advisories, `cargo audit --no-fetch --json` reports zero vulnerabilities and zero warnings. `quick-xml 0.41.0` is the patched floor for namespace-resolver issue `RUSTSEC-2026-0195`; the provider intentionally uses that patched resolver with a 128-declaration per-element cap. The minimal graph tests on macOS arm64 and checks for Windows x64 using Rust 1.97.0.

The selected adapter never writes archive entries to disk or follows relationships. It selects only exact DOCX/PPTX/XLSX text parts, rejects encryption/overlap/unsafe or duplicate selected names/unsupported compression, bounds claimed and actual decompression plus structure/output/time, rejects DTD and unsupported entities, keeps macros/formulas/external links/embeddings inert, and requires explicit paragraph/slide/cell provenance.

The exact two dependencies entered the Rust workspace only with the accepted feature set. No high-level Office parser, additional archive/XML feature, native binary, model, package-owned build script, or relationship-following behavior is approved. Provider adversarial fixtures and the complete-lock RustSec delta now pass locally; remote runtimes, representative real-world corpora, redistribution notices/SBOM, and 8 GB measurement remain required before M2 completion.

## M3 lexical-search dependency decision

The first M3 slice adds no external package. ADR-015 reuses the already selected bundled SQLite from `rusqlite 0.40.1` / `libsqlite3-sys 0.38.1` and its built-in FTS5 `trigram` tokenizer. SQLite's official FTS5 documentation confirms external-content indexes, synchronization triggers and rebuilds, trigram substring behavior, the three-Unicode-character minimum, `rank`/BM25 ordering, and bounded `snippet()` output. Local migration and multilingual tests prove the selected bundled build exposes FTS5.

No vector extension, tokenizer extension, embedding runtime, model, API, or network client is selected by this decision. The workspace adds only path-based local `deskgraph-retrieval` and `deskgraph-search-benchmark` crates; they introduce no registry dependency and keep future vector adapters outside the database and domain contracts. The benchmark tool reuses audited `clap`, `rusqlite`, `serde`, and `serde_json`, refuses to overwrite an existing database, and is not shipped with the product.

## M6 durable watch-core dependency decision

The first M6 slice adds only the local path-based `deskgraph-watcher` workspace crate. It reuses the audited database/domain/identity/scanner layers and Rust standard library; no native watcher, async runtime, network client, or registry package was added. Native adapter candidates remain unapproved until official API, maintenance, platform, license, and security evidence exists.

## M5 rename-preview dependency decision

The first M5 slice adds only the local path-based `deskgraph-transactions` workspace crate. It reuses the audited database/domain/identity/scanner layers and Rust standard library. No registry package, file-operation plugin, async runtime, model, network client, shell, Python, Docker, or native runtime was added. `serde_json` and `tempfile` remain test-only workspace dependencies.

## M4 folder-profile dependency decision

The M4 Folder Profile slice adds only the local path-based `deskgraph-projects` workspace crate. The later Project root correction, exact-duplicate relation, exact-pair feedback, and filename-version flows add no registry package: they promote the already-audited local scanner/identity layers to runtime use and rely on SQLite plus bounded Rust code rather than a hashing or graph library. ADR-022 adds a direct `deskgraph-domain` use of the already locked/audited pure-Rust `unicode-normalization 0.1.25` package (MIT OR Apache-2.0, platform-independent, official `unicode-rs/unicode-normalization`) so database and filesystem boundaries share one NFC rule; `Cargo.lock` remains 488 packages and changes only the local package dependency list. `serde_json` and `tempfile` remain test-only. No new package, embedding/vector runtime, hash package, model, API, network client, shell, Python, Docker, Ollama, or native runtime was added. ADR-018 through ADR-022 require deterministic current manifest facts, immutable evidence, exact-root/exact-pair correction, bounded byte equality, and explicit filename evidence before future similarity, larger-file hashing, or learned scoring.

## GitHub Actions

Only official `actions/*` actions are permitted in M0. `actions/checkout` v4.2.2 is pinned to `11bd71901bbe5b1630ceea73d27597364c9af683`; `actions/setup-node` v6.4.0 is pinned to `48b55a011bda9f5d6aeb4c2d9c7362e8dae4041e`. CI permissions default to `contents: read`, and no secrets are exposed to fork pull requests. Remote execution evidence remains blocked until the GitHub repository exists.

## Executed verification

- Before the PDF dependency, `Cargo.lock` resolved 457 packages including eight DeskGraph workspace packages. `cargo metadata --offline --no-deps` found no missing workspace license metadata. License expressions include permissive licenses, MPL-2.0, and optional-license expressions containing LGPL; platform-specific redistribution and notices require another M9 review. PDF integration produced 483 packages; local workspace-only retrieval, benchmark, watcher, transaction, and project crates brought the lock to 488. Office integration produced 491 packages by adding `zip 8.6.0` (MIT), `typed-path 0.12.3` (MIT OR Apache-2.0), and `zlib-rs 0.6.6` (Zlib); `quick-xml 0.41.0` was already locked transitively and became an exact direct dependency. Image metadata produces the current 492-package lock by adding only `imagesize 0.15.0` (MIT).
- `cargo tree --workspace --depth 1` recorded all direct versions. `cargo tree --target all -i ...` traced RustSec warnings to Tauri/Wry's Linux GTK3 stack and Tauri's URL-pattern parser chain.
- The current full-lock scan loaded 1,160 cached RustSec advisories and scanned 492 lockfile packages: zero known vulnerabilities plus 17 warnings—ten unmaintained GTK3 binding crates, `proc-macro-error`, five unmaintained `unic-*` crates, and one `glib` unsound advisory. The prior 491-package lock has the identical warning set, so the image-metadata delta adds no advisory. The isolated 53-package PDF, 14-package Office, and one-package image-metadata closures separately returned zero findings. R-016 remains open because current evidence does not repair Tauri's Linux transitive stack.
- `pnpm audit --prod` and full `pnpm audit` found zero known vulnerabilities.
- `pnpm licenses list --json` failed with `ERR_PNPM_MISSING_PACKAGE_INDEX_FILE` under pnpm's SQLite-backed local store. The recorded equivalent scan read all installed package manifests: 145 unique packages, no missing license fields; 106 MIT, 18 Apache-2.0, 6 BSD-2-Clause, 2 BSD-3-Clause, 7 ISC, 2 MPL-2.0, 1 BlueOak-1.0.0, and 3 Apache-2.0 OR MIT.
- `pnpm peers check` reports no peer dependency issues after pinning TypeScript 6.0.3.

## Verification still required

- Execute frozen installs and all checks in remote macOS, Windows, and Linux CI.
- Revisit native transitive dependencies, redistribution notices, and R-016 when packaging begins.
- Re-run the Rust dependency and license audit after every future lockfile change.
- Audit every OCR, embedding, vector, model, and future archive/document dependency separately before adoption; rerun the full lock and license review after every accepted delta.

## Rejected or deferred at M0

- M0 intentionally had no database crate. M1 adopts the audited SQLite binding above; the health contract may report it ready only after initialization succeeds.
- No OCR, vector, embedding, or LLM runtime: they are not needed for the foundation slice.
- No Tauri filesystem, shell, HTTP, opener, or updater plugin: M0 health requires none of those capabilities.
- No telemetry/crash SDK: privacy and governance decision is unresolved.
