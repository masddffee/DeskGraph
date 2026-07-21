# External Actions Required

Last reviewed: 2026-07-22

No external step should block safe local implementation. Do not add real credentials to the repository.

## OpenAI Build Week submission (deadline-critical)

Current state: the authenticated Devpost account is registered for OpenAI Build Week and owns
project draft `1343809`, currently named `Untitled` in `submission_pre_draft`. The submission
deadline is 2026-07-22 00:00 UTC (2026-07-22 08:00 Asia/Taipei). Local implementation and all
submission copy may continue without external mutation.

The deterministic local golden-path fixture and final local validation gates pass. A reviewed
95-second 1280×720 H.264 silent guided cut exists locally at the ignored path
`artifacts/demo/DeskGraph-Build-Week-demo-silent.mp4`; it uses real synthetic-scope Desktop
scan/search states plus separately verified CLI evidence and does not claim continuous live
operation. Its SHA-256 is
`bca6e8c72817919ae32fcfd69def3cff15f4e14655ab443342d0ab41462255e5`. Final voice recording,
mixing and upload remain deadline-critical owner/external work.

Required owner-approved actions before submission:

1. Obtain the `/feedback` Session ID for the Codex task where most core functionality was built.
2. Re-authenticate the intended GitHub account and verify that the public Apache-2.0 repository at
   `https://github.com/masddffee/DeskGraph` is reachable from the final Devpost entry.
3. Review the final under-three-minute guided cut, record the supplied 95-second Traditional
   Chinese voiceover against its timestamps, and approve mixing it into the final video. Upload the
   narrated result as an unlisted or public YouTube video; it must show the project working and
   explain how both Codex and GPT-5.6 were used.
4. Confirm submitter type and country of residence; select `Apps for Your Life` unless the final
   demonstrated audience changes materially.
5. Review the prepared name, tagline, write-up, repository URL, judge instructions, video URL and
   session ID, then explicitly authorize updating and submitting Devpost project `1343809`.
6. Validate the public URLs and final Devpost state before 2026-07-22 08:00 Asia/Taipei. Devpost
   changes cannot be made after the submission period except when the organizer explicitly allows
   a narrow correction.

Do not claim production v0.1, vector/hybrid search, executable system-trash cleanup, Undo,
installers, signing, notarization or cross-platform runtime in the hackathon entry unless those
gates independently pass before submission.

## GitHub repository and CI (needed to close M0 remote evidence)

Current state: local `main` tracks the public `git@github.com:masddffee/DeskGraph.git` remote. `gh auth status` still reports the configured `masddffee` token as invalid, and the current environment cannot resolve `github.com`, so the latest push-to-main CI, Issues, branch protection, vulnerability reporting, and Releases cannot be verified or changed here.

Required owner action:

1. Re-authenticate with `gh auth login -h github.com` using the intended owner account.
2. Confirm `masddffee/DeskGraph` remains the intended public repository and GitHub Actions is enabled.
3. Enable GitHub private vulnerability reporting and update `SECURITY.md` only if the final advisory path differs.
4. Protect the default branch after the current CI workflow is green.
5. Verify the macOS, Windows, and Linux jobs from a clean remote checkout.

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

ADR-027 also requires the owner to approve the final macOS application identifier, minimum
supported macOS version, and App Sandbox entitlement set. The local M9a foundation now provides a
Rust-owned native folder-selection command, opaque versioned security-scoped bookmark
create/resolve/start/stop ownership, stale refresh, and migration-backed
`needs_reauthorization` handling for legacy, corrupt, or unrestorable grants. The checked-in
entitlement configuration is only local configuration evidence: neither it nor an ad-hoc
signature proves a production sandbox, user-selection persistence, notarization, or a release
artifact. The current profile also contains `com.apple.security.network.client`: a current-host
ad-hoc A/B run shows that the sandboxed WebKit networking subprocess exits without it and that
adding only this entitlement restores the bundled local UI and native folder picker. This grants
an outbound socket capability, not a product upload feature. Clean-machine evidence must therefore
capture outbound connection attempts during first launch, selection, scan, extraction, search,
Watch and action Preview and verify that no filename, path, content, OCR, embedding or graph data
leaves the machine. The production CSP must continue to exclude development HTTP/WebSocket origins.

A per-app-container `flock` remains a candidate until the selected OS version's SIP contract and a
Developer-ID-signed hostile probe prove that a non-entitled same-user process cannot silently
open, unlink, rename, or recreate its entry without a user-authorized exception. If that proof
fails, macOS production actions remain unavailable. Do not add an App Group unless a later
accepted helper-topology ADR names the exact group, members, Team ID binding, IPC contract, and
uninstall behavior.

On clean Developer-ID-signed/notarized arm64 and Intel-or-Universal installs, preserve evidence
for: the exact OS version and SIP/container contract; container identity validation without
recording the path in ordinary logs; first scope selection; bookmark restore after restart; stale
bookmark refresh; user revocation; legacy/corrupt/unrestorable grants becoming
`needs_reauthorization`; denied unselected folders; unsigned/unsandboxed action denial; and a
non-entitled same-user probe that attempts to open/unlink/rename/recreate the fence entry without
user consent. Only after that proof passes may the matrix cover two-process fence contention,
paused-owner exclusion beyond the SQLite lease, forced-crash release, close-on-exec, app update,
repair/reinstall and uninstall behavior. The run must prove the fence is acquired before the
action database opens and must not enable the ADR-026-rejected general Unix Rename/Move adapter.

Exact secret names and validation commands will be fixed when the Tauri packaging workflow is implemented and reviewed against current official documentation.

## Windows code signing (needed in M9)

Provide a protected certificate or signing service, its non-repository secret references, timestamp authority, and access policy. Verify signatures on a clean Windows VM before calling artifacts production-ready.

ADR-027 requires one stable Windows package identity shared by native OCR and the action fence.
The owner must approve the package name, publisher subject/identity, installer channel and update
identity; provide the matching signing certificate; and state whether distribution uses Store
identity or a reviewed non-Store full MSIX/App Installer identity. MSI/NSIS alone is insufficient
for this gate. Do not claim AppContainer: the accepted v0.1 design is packaged classic desktop
medium-integrity code unless separately changed. The current non-macOS release branch fails closed
for native scope grants; it is not a substitute for package-family identity.

On a clean Windows x64 VM, record package-family identity stability across MSIX install and App
Installer update, unpackaged denial, repair, uninstall and reinstall. Then prove the protected
private namespace, boundary descriptor/DACL, same-native-thread mutex ownership/release, non-inheritable handles,
two-process contention, paused-owner exclusion beyond the SQLite lease, terminated-owner
`WAIT_ABANDONED` recovery-only behavior, namespace squatting/replacement denial, fail-before-
database ordering, and no SQLite WAL interference. Package names, SIDs, filesystem paths and
security descriptors must not enter ordinary product logs.

## ADR-033 hard-exclusion acceptance matrix (needed before M1/M9 release claims)

The local implementation is add-only and is not a signed-package claim. Run this matrix only on
disposable, explicitly authorized synthetic scopes after an Initial Manifest Scan has indexed files
both inside and outside a candidate exclusion. Preserve path-free receipts and test artefacts; do
not put ordinary paths, file names, OCR text, embeddings, grants, package identities or source
contents in product logs.

On signed macOS arm64 plus Intel or the final Universal artifact, and on packaged Windows x64 with
the accepted stable package-family identity, prove all of the following:

- add an exclusion after metadata, FTS, OCR/content, graph/project/relation/screenshot/Cleanup and
  safe Preview-only action derivations exist; verify that only derived records inside the canonical
  target are purged, no source file is changed, and unrelated in-scope records remain;
- restart after apply; attempt a stale, revoked, malformed, foreign-platform or otherwise inactive
  grant; and verify denial before traversal, query, return, action policy or persisted publication;
- race apply against scan, Watch reconciliation, extraction/OCR, search, MCP, Project/Cleanup and
  action-policy requests; kill the process at each durable boundary, reopen the database, and prove
  atomic all-or-nothing policy/revision/purge/receipt state with no reachable excluded derivative;
- verify canonical directory/file identity and symlink/reparse/hard-link behavior, policy-revision
  invalidation, path-free ordinary receipts/history/logs, and that no network egress carries local
  path, content, OCR, embedding or graph data during the complete flow;
- on Windows additionally prove the package identity used for this matrix survives install, update,
  repair and restart, and that unpackaged or foreign-package execution fails closed.

Removal/revocation and any retention policy for non-preview action receipts remain separate release
work; this matrix must not be interpreted as permission to expose a source-mutating action.

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

Native Watch evidence must run outside filesystem-event-suppressing sandboxes on macOS arm64, macOS Intel/Universal, Windows x64 and Linux x64. For each platform, start from an explicit authorization with a completed Initial Manifest Scan and prove create, modify with direct manifest metadata verification, final rename, delete, nested-scope routing, Initial-Scan-to-registration gap recovery, root replacement/failure recovery, temporary-download exclusion followed by final-name inclusion (including an ordered old-temporary/final-path rename), sustained-writer maximum-coalescing-age recovery, recovery received during an already-running multi-batch scan, queue/kernel event storm recovery, restart, and five-minute missed-event reconciliation. Prove that a direct ignored observation cannot cancel unrelated stabilizing work. Verify that ordinary runtime payloads and logs remain path-free; callbacks remain non-blocking under saturation; ignored-event operational history stays bounded without deleting user data or action history; no content extraction or filesystem action starts automatically; and native adapter loss degrades to the periodic safety path. Record CPU, idle wakeups, reconciliation latency and start/peak/end RSS on documented 8 GB hardware. Local macOS arm64 tests are implementation evidence, not clean-machine or installer evidence. The expanded local `.crdownload` → final-name host test remains pending and needs an unsandboxed rerun that actually receives the native callback.

Image-metadata Windows evidence must cover PNG/JPEG/GIF/WebP/BMP/TIFF routing, extension/signature mismatch, malformed/truncated headers, WebP declared length, source/probe/operation/dimension/pixel limits, cancellation, source change, migration preservation, atomic replacement/invalidation, and path-free CLI output. The isolated Rust Windows compile proves only dependency portability, not the complete SQLite/filesystem runtime.

macOS Screenshot OCR clean-machine evidence must run on arm64 and Intel or the final Universal artifact. Use the committed `deskgraph-macos-vision-runner` with a private, license-reviewed asset manifest/root and a new non-repository run path; record the exact corpus/manifest/run digests and commit, then evaluate the run without publishing OCR text or asset paths. The packaged app must access Apple Vision, report both `zh-Hant` and `en-US`, recognize the controlled mixed fixture, complete a no-text image, persist valid top-left spatial/confidence provenance, return both languages through FTS, cancel actual native work without partial publication, and record peak/start/end RSS on documented 8 GB hardware. The restricted development runner's fixed provider failure and the outside-sandbox local pass are useful environment evidence but do not prove installer entitlements or clean-machine support.

Windows Screenshot OCR evidence must run the production module on Windows 10/11 x64 with both package identity present and absent. Cover MSIX/external-location packaging, an unpackaged CLI path, requested `zh-TW`/`en-US` resolving to acceptable actual recognizers, missing/incompatible language capabilities, mixed Traditional Chinese/English, no-text, corrupt/signature mismatch, source/dimension/pixel/output/observation/word limits, nullable confidence, exact de-duplication, zero/absent and non-zero `TextAngle`, source change, atomic SQLite/FTS replacement, and path-free status/logs. Exercise cancellation and deadline at every WinRT async stage; prove `Close()` occurs only after terminal status, a detached cleanup worker eventually releases the one-worker gate, a stuck cleanup makes later OCR fail closed without accumulating workers, and restart recovers availability. Record CPU plus start/peak/end RSS on documented 8 GB hardware. Do not install OCR Language Features on Demand silently. Missing identity/language currently returns a fixed error; separately verify the packaged fallback only after its dependency/runtime gate is accepted and implemented.

Filename-version Windows evidence must additionally cover separator/case normalization, Traditional Chinese names, extension mismatch, unsupported/leading-zero/same-number suffixes, reparse/hard-link identity denial, stale manifest/open-handle invalidation, migration preservation of existing duplicate feedback, immutable observations and evidence-bound decisions, idempotent retry/opposite correction, changed-direction suggestion reset, unchanged files, and path-free history/log output.

## M5 platform action fault matrix (required before any execution control)

ADR-026 resolves D-018 by rejecting general macOS/Linux Rename/Move execution for user-authorized scopes. The leaf-name adapter remains test-only and all production targets currently return `action_platform_rename_unsupported`; clean-machine testing must not enable it by configuration or patch around that gate. Run the complete matrix for the separately reviewed Windows exact-handle adapter and for any future Unix design only after a new OS primitive or managed-namespace ADR supersedes ADR-026. ADR-027 resolves D-019's architecture, but its packaged identity, platform fence implementations, and real runtime evidence remain mandatory before any action entry is exposed. Linux evidence remains experimental and cannot delay required macOS/Windows release evidence; this does not turn the rejected Unix adapter into an experimental production feature.

For each accepted platform adapter, use disposable test scopes and prove normal Rename and Undo plus: two competing processes; repeated request/lost response; a child process paused after durable intent for longer than the database lease while recovery attempts to claim work; process kill at every durable boundary; database reopen; permission denial; parent durability failure; source/root/parent replacement; symlink/reparse and post-preview hard-link insertion; same-size/same-mtime content replacement; destination creation and overwrite denial; both names present or absent; post-action identity/hash mismatch; removable-volume disconnect; request/result/log privacy; and bounded 8 GiB/90-second hash behavior. ADR-027's accepted process-fence contract must be released by crash, prevent recovery while a stopped live executor holds it, live in its specified trusted namespace, resist replacement/inheritance attempts, and not interfere with SQLite WAL locking. No ADR-027 production action fence is currently implemented or runtime-accepted; the cooperative scope read/revocation fence is a distinct narrower control. No automatic syscall retry, rollback-by-guess, delete, permanent-delete, empty-trash, shell command, LLM, MCP, Watch or Inbox action is permitted.

macOS/Linux testing must include a deterministic adversarial interleaving that replaces the ordinary source leaf after final identity revalidation and before the namespace mutation. If the accepted design cannot make that interleaving fail before moving the replacement file, the adapter remains unavailable. Windows must use a separately reviewed handle-bound `FILE_RENAME_INFO`/`SetFileInformationByHandle` design with `ReplaceIfExists = false`, reparse-safe root/parent/source handles and real NTFS runtime evidence; a path-based fallback is forbidden.

### D-017 system-trash and conditional Undo matrix

Run this only with disposable synthetic files in a signed sandbox/package identity after the platform-spike code is separately reviewed. It must not touch ordinary user folders, run through shell commands, enumerate Trash/Recycle Bin, read private Finder metadata or parse `$Recycle.Bin` internals.

On macOS arm64 plus Intel/Universal, test `FileManager.trashItem(at:resultingItemURL:)` with path and file-reference URLs under a hostile uncoordinated process that repeatedly replaces the source leaf between final validation and the call. Record which exact identity reaches Trash; a test pass is runtime evidence, not a substitute for an Apple guarantee. For an accepted exact-source design, create both pre-action and returned-URL security-scoped bookmarks, persist them only in the private local database, terminate and relaunch the signed app, resolve/start/stop both, and verify the same identity/hash. Cover same-name Trash collisions, APFS internal and external writable volumes, stale/unresolvable bookmarks, user-empty Trash, changed Trash item, scope revocation, detached volume and process kill before/after prepared intent, Trash return, bookmark creation and receipt commit. Undo must reject cross-volume copy/remove behavior and any existing or concurrently created destination; after a successful restore, verify exact identity/hash/name/parent before journaling success.

On Windows 10 and 11 x64, run a dedicated STA/message-loop spike using `ITransferSource::RecycleItem`. Require a non-null recycled `IShellItem`, capture an absolute PIDL through the documented Shell API, serialize it as an opaque private receipt, terminate the app, restart Explorer, sign out/in where automation permits, reboot, and reconstruct it before any Undo claim. Bind the Recycle Bin transfer handler and restore only with `ITransferSource::MoveItem(..., TSF_NORMAL)`; never enable overwrite, rename-on-collision or copy-delete fallback. Revalidate exact identity/hash before and after every call. Cover source replacement, existing and raced destination collision, PIDL parse/bind failure, Recycle Bin empty/change, OneDrive/redirected folders, scope revocation, volume detach, process kill at each durable boundary and Windows update compatibility. Any missing/changed/unresolvable receipt becomes `needs_attention`; it must not trigger a search of Recycle Bin or a manual path guess.

Neither platform may expose Confirm, Trash, Execute, automatic recovery or Undo until its exact-source mutation, durable receipt and platform-fence matrix passes. Batch acceptance comes later: first prove one file, then 1–100 items and at most 100 GiB, serial execution, per-item receipts/outcomes, stop on the first non-completed item and no batch rollback.

## Public release and launch accounts (needed in M10)

GitHub Release must be publicly verified before any social publication. Product Hunt, X, LinkedIn, Reddit, YouTube, domain/DNS, and website credentials remain owner-controlled. Without them, the repository will contain ready-to-post assets and an exact checklist only.
