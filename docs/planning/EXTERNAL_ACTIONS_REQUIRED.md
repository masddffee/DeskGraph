# External Actions Required

Last reviewed: 2026-07-16

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

The current macOS host can compile the isolated Windows open-handle identity adapter, but it cannot compile bundled `libsqlite3-sys` for `x86_64-pc-windows-msvc` because no Windows MSVC C headers/toolchain are installed. This is not a request to weaken bundled SQLite or add an unaudited cross toolchain. On the Windows x64 runner or clean VM, run at minimum:

```text
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
pnpm install --frozen-lockfile
pnpm check
pnpm --filter @deskgraph/desktop tauri build --no-bundle
```

Windows evidence must include junction/reparse and hidden/system scanner fixtures, open-handle extraction identity, cancellation/interrupted recovery, text/Markdown/code extraction, bounded text-layer PDF extraction with page provenance and adversarial fixtures, Folder Profile separator/marker/limit behavior with path-free logs, and a privacy-safe Desktop/CLI smoke. A successful Rust-only cross-check on macOS is not a substitute.

## Public release and launch accounts (needed in M10)

GitHub Release must be publicly verified before any social publication. Product Hunt, X, LinkedIn, Reddit, YouTube, domain/DNS, and website credentials remain owner-controlled. Without them, the repository will contain ready-to-post assets and an exact checklist only.
