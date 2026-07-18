# External Actions Required

Last reviewed: 2026-07-19

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

Native Watch evidence must run outside filesystem-event-suppressing sandboxes on macOS arm64, macOS Intel/Universal, Windows x64 and Linux x64. For each platform, start from an explicit authorization with a completed Initial Manifest Scan and prove create, modify with direct manifest metadata verification, final rename, delete, nested-scope routing, Initial-Scan-to-registration gap recovery, root replacement/failure recovery, temporary-download exclusion followed by final-name inclusion (including an ordered old-temporary/final-path rename), sustained-writer maximum-coalescing-age recovery, recovery received during an already-running multi-batch scan, queue/kernel event storm recovery, restart, and five-minute missed-event reconciliation. Prove that a direct ignored observation cannot cancel unrelated stabilizing work. Verify that ordinary runtime payloads and logs remain path-free; callbacks remain non-blocking under saturation; ignored-event operational history stays bounded without deleting user data or action history; no content extraction or filesystem action starts automatically; and native adapter loss degrades to the periodic safety path. Record CPU, idle wakeups, reconciliation latency and start/peak/end RSS on documented 8 GB hardware. Local macOS arm64 tests are implementation evidence, not clean-machine or installer evidence. The expanded local `.crdownload` → final-name host test still requires rerun because Codex external execution was denied after the usage quota was reached.

Image-metadata Windows evidence must cover PNG/JPEG/GIF/WebP/BMP/TIFF routing, extension/signature mismatch, malformed/truncated headers, WebP declared length, source/probe/operation/dimension/pixel limits, cancellation, source change, migration preservation, atomic replacement/invalidation, and path-free CLI output. The isolated Rust Windows compile proves only dependency portability, not the complete SQLite/filesystem runtime.

macOS Screenshot OCR clean-machine evidence must run on arm64 and Intel or the final Universal artifact. Use the committed `deskgraph-macos-vision-runner` with a private, license-reviewed asset manifest/root and a new non-repository run path; record the exact corpus/manifest/run digests and commit, then evaluate the run without publishing OCR text or asset paths. The packaged app must access Apple Vision, report both `zh-Hant` and `en-US`, recognize the controlled mixed fixture, complete a no-text image, persist valid top-left spatial/confidence provenance, return both languages through FTS, cancel actual native work without partial publication, and record peak/start/end RSS on documented 8 GB hardware. The restricted development runner's fixed provider failure and the outside-sandbox local pass are useful environment evidence but do not prove installer entitlements or clean-machine support.

Windows Screenshot OCR evidence must run the production module on Windows 10/11 x64 with both package identity present and absent. Cover MSIX/external-location packaging, an unpackaged CLI path, requested `zh-TW`/`en-US` resolving to acceptable actual recognizers, missing/incompatible language capabilities, mixed Traditional Chinese/English, no-text, corrupt/signature mismatch, source/dimension/pixel/output/observation/word limits, nullable confidence, exact de-duplication, zero/absent and non-zero `TextAngle`, source change, atomic SQLite/FTS replacement, and path-free status/logs. Exercise cancellation and deadline at every WinRT async stage; prove `Close()` occurs only after terminal status, a detached cleanup worker eventually releases the one-worker gate, a stuck cleanup makes later OCR fail closed without accumulating workers, and restart recovers availability. Record CPU plus start/peak/end RSS on documented 8 GB hardware. Do not install OCR Language Features on Demand silently. Missing identity/language currently returns a fixed error; separately verify the packaged fallback only after its dependency/runtime gate is accepted and implemented.

Filename-version Windows evidence must additionally cover separator/case normalization, Traditional Chinese names, extension mismatch, unsupported/leading-zero/same-number suffixes, reparse/hard-link identity denial, stale manifest/open-handle invalidation, migration preservation of existing duplicate feedback, immutable observations and evidence-bound decisions, idempotent retry/opposite correction, changed-direction suggestion reset, unchanged files, and path-free history/log output.

## M5 platform action fault matrix (required before any execution control)

ADR-026 resolves D-018 by rejecting general macOS/Linux Rename/Move execution for user-authorized scopes. The leaf-name adapter remains test-only and all production targets currently return `action_platform_rename_unsupported`; clean-machine testing must not enable it by configuration or patch around that gate. Run the complete matrix for the separately reviewed Windows exact-handle adapter and for any future Unix design only after a new OS primitive or managed-namespace ADR supersedes ADR-026. D-019 must additionally accept a packaged-private process fence. Linux evidence remains experimental and cannot delay required macOS/Windows release evidence; this does not turn the rejected Unix adapter into an experimental production feature.

For each accepted platform adapter, use disposable test scopes and prove normal Rename and Undo plus: two competing processes; repeated request/lost response; a child process paused after durable intent for longer than the database lease while recovery attempts to claim work; process kill at every durable boundary; database reopen; permission denial; parent durability failure; source/root/parent replacement; symlink/reparse and post-preview hard-link insertion; same-size/same-mtime content replacement; destination creation and overwrite denial; both names present or absent; post-action identity/hash mismatch; removable-volume disconnect; request/result/log privacy; and bounded 8 GiB/90-second hash behavior. A future accepted process fence must be released by crash, prevent recovery while a stopped live executor holds it, live in a trusted private or otherwise replacement-resistant namespace, resist unlink/rename/recreate attempts by another process, and not interfere with SQLite WAL locking. No such fence is currently implemented or accepted. No automatic syscall retry, rollback-by-guess, delete, permanent-delete, empty-trash, shell command, LLM, MCP, Watch or Inbox action is permitted.

macOS/Linux testing must include a deterministic adversarial interleaving that replaces the ordinary source leaf after final identity revalidation and before the namespace mutation. If the accepted design cannot make that interleaving fail before moving the replacement file, the adapter remains unavailable. Windows must use a separately reviewed handle-bound `FILE_RENAME_INFO`/`SetFileInformationByHandle` design with `ReplaceIfExists = false`, reparse-safe root/parent/source handles and real NTFS runtime evidence; a path-based fallback is forbidden.

## Public release and launch accounts (needed in M10)

GitHub Release must be publicly verified before any social publication. Product Hunt, X, LinkedIn, Reddit, YouTube, domain/DNS, and website credentials remain owner-controlled. Without them, the repository will contain ready-to-post assets and an exact checklist only.
