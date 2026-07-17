# External Actions Required

Last reviewed: 2026-07-18

No external step should block safe local implementation. Do not add real credentials to the repository.

## GitHub repository and CI (needed to close M0 remote evidence)

Current state: local `main` exists with logical M0 commits, but there is no remote. `gh auth status` still reports the configured `masddffee` token as invalid, so Issues, remote CI, branch protection, and Releases cannot be verified or changed.

Required owner action:

1. Re-authenticate with `gh auth login -h github.com` using the intended owner account.
2. Decide personal vs organization ownership and confirm the public repository name.
3. Create the empty public repository without auto-generated files, or provide its URL.
4. Add it as `origin`, push the local default branch, and enable GitHub Actions.
5. Enable GitHub private vulnerability reporting and update `SECURITY.md` with the final repository advisory path.
6. Protect the default branch after the initial CI workflow is green.
7. Verify the macOS, Windows, and Linux jobs from a clean remote checkout.

Validation:

```text
gh auth status
git remote -v
gh run list --workflow ci.yml
```

## Apple signing and notarization (needed in M9)

Provide through a protected GitHub environment, never repository files:

- Apple Developer ID Application certificate exported as a base64 secret;
- certificate password;
- Apple Team ID;
- notarization credentials using an App Store Connect API key or approved Apple ID flow;
- updater signing private key stored as an environment secret.

Exact secret names and validation commands will be fixed when the Tauri packaging workflow is implemented and reviewed against current official documentation.

## Windows code signing (needed in M9)

Provide a protected certificate or signing service, its non-repository secret references, timestamp authority, and access policy. Verify signatures on a clean Windows VM before calling artifacts production-ready.

## Clean-machine validation (needed in M9)

Provide or authorize clean test machines/VMs for:

- supported macOS arm64;
- macOS x64 or Universal validation;
- Windows x64;
- one documented Linux experimental target.

The current macOS host can compile the isolated Windows adapters. With `LIBSQLITE3_SYS_USE_PKG_CONFIG=1 PKG_CONFIG_ALLOW_CROSS=1`, it also typechecks and runs Clippy over the Windows OCR Rust cfg, but that workaround uses host SQLite metadata and `cargo check` does not link. A normal target check still stops while compiling bundled `libsqlite3-sys` because no Windows MSVC C headers/toolchain are installed. None of this proves a Windows executable or runtime. This is not a request to weaken bundled SQLite or add an unaudited cross toolchain. On the Windows x64 runner or clean VM, run at minimum:

```text
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
pnpm install --frozen-lockfile
pnpm check
pnpm --filter @deskgraph/desktop tauri build --no-bundle
```

Windows evidence must include junction/reparse and hidden/system scanner fixtures, open-handle extraction identity, cancellation/interrupted recovery, text/Markdown/code extraction, bounded text-layer PDF extraction with page provenance and adversarial fixtures, and DOCX/PPTX/XLSX extraction with paragraph/slide/cell provenance plus unsafe-name/duplicate/encrypted/overlap/unsupported-compression/decompression/XML-limit/active-part fixtures. It must also cover Folder Profile separator/marker/limit behavior, Project candidate migration/current-evidence validation, append-only root accept/reject idempotency/correction, rejected-root suppression, and exact-duplicate canonical/junction/hard-link/identity/size/content/stale/limit/append-only behavior. Relation feedback must prove live revalidation before decide, idempotent retry, opposite-decision correction, state retention after a later observation, immutable events, unchanged files, path-free list output, and redacted structured logs. Finish with a privacy-safe Desktop/CLI smoke. A successful Rust-only cross-check on macOS is not a substitute.

Image-metadata Windows evidence must cover PNG/JPEG/GIF/WebP/BMP/TIFF routing, extension/signature mismatch, malformed/truncated headers, WebP declared length, source/probe/operation/dimension/pixel limits, cancellation, source change, migration preservation, atomic replacement/invalidation, and path-free CLI output. The isolated Rust Windows compile proves only dependency portability, not the complete SQLite/filesystem runtime.

macOS Screenshot OCR clean-machine evidence must run on arm64 and Intel or the final Universal artifact. It must verify that the packaged app can access Apple Vision, reports both `zh-Hant` and `en-US`, recognizes the controlled mixed fixture, completes a no-text image, persists valid top-left spatial/confidence provenance, returns both languages through FTS, cancels actual native work without partial publication, and records peak/start/end RSS on documented 8 GB hardware. The restricted development runner's fixed provider failure and the outside-sandbox local pass are useful environment evidence but do not prove installer entitlements or clean-machine support.

Windows Screenshot OCR evidence must run the production module on Windows 10/11 x64 with both package identity present and absent. Cover MSIX/external-location packaging, an unpackaged CLI path, requested `zh-TW`/`en-US` resolving to acceptable actual recognizers, missing/incompatible language capabilities, mixed Traditional Chinese/English, no-text, corrupt/signature mismatch, source/dimension/pixel/output/observation/word limits, nullable confidence, exact de-duplication, zero/absent and non-zero `TextAngle`, source change, atomic SQLite/FTS replacement, and path-free status/logs. Exercise cancellation and deadline at every WinRT async stage; prove `Close()` occurs only after terminal status, a detached cleanup worker eventually releases the one-worker gate, a stuck cleanup makes later OCR fail closed without accumulating workers, and restart recovers availability. Record CPU plus start/peak/end RSS on documented 8 GB hardware. Do not install OCR Language Features on Demand silently. Missing identity/language currently returns a fixed error; separately verify the packaged fallback only after its dependency/runtime gate is accepted and implemented.

Filename-version Windows evidence must additionally cover separator/case normalization, Traditional Chinese names, extension mismatch, unsupported/leading-zero/same-number suffixes, reparse/hard-link identity denial, stale manifest/open-handle invalidation, migration preservation of existing duplicate feedback, immutable observations and evidence-bound decisions, idempotent retry/opposite correction, changed-direction suggestion reset, unchanged files, and path-free history/log output.

## Public release and launch accounts (needed in M10)

GitHub Release must be publicly verified before any social publication. Product Hunt, X, LinkedIn, Reddit, YouTube, domain/DNS, and website credentials remain owner-controlled. Without them, the repository will contain ready-to-post assets and an exact checklist only.
