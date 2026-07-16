# External Actions Required

Last reviewed: 2026-07-16

No external step should block safe local implementation. Do not add real credentials to the repository.

## GitHub repository and CI (needed to close M0 remote evidence)

Current state: no local Git repository/remote at baseline; the configured `gh` account token is invalid.

Required owner action:

1. Re-authenticate with `gh auth login -h github.com` using the intended owner account.
2. Decide personal vs organization ownership and confirm the public repository name.
3. Create the empty public repository without auto-generated files, or provide its URL.
4. Add it as `origin`, push the local default branch, and enable GitHub Actions.
5. Protect the default branch after the initial CI workflow is green.
6. Verify the macOS, Windows, and Linux jobs from a clean remote checkout.

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

## Public release and launch accounts (needed in M10)

GitHub Release must be publicly verified before any social publication. Product Hunt, X, LinkedIn, Reddit, YouTube, domain/DNS, and website credentials remain owner-controlled. Without them, the repository will contain ready-to-post assets and an exact checklist only.
