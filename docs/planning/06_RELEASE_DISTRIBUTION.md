# Release and Distribution Plan

## 1. Versioning

- SemVer。
- `v0.1.0-alpha.N` internal / early tester。
- `v0.1.0-beta.N` public beta。
- `v0.1.0` first production open-source release。
- Database schema version independent from app version。

## 2. Release Assets

### macOS

- Universal DMG when practical。
- Separate arm64 / x64 fallback。
- Developer ID signing。
- Notarization。
- SHA-256。

### Windows

- NSIS or MSI。
- x64 first。
- Code signing。
- SHA-256。
- Optional portable zip。

### Linux

- AppImage experimental。
- `.deb` optional。
- Marked community-supported until tested.

### Other

- CLI binaries。
- SBOM。
- `checksums.txt`。
- `latest.json` for updater。
- Release notes。
- Known limitations。

## 3. GitHub Actions

Workflows：

- `ci.yml`
- `security.yml`
- `benchmark-smoke.yml`
- `release.yml`
- `nightly.yml`
- `docs.yml`

Requirements：

- Pin Actions to commit SHA。
- Minimal permissions。
- OIDC where possible。
- Protected environments for signing。
- Secrets never available to fork PRs。
- Artifact retention policy。
- Reproducible lockfiles。

## 4. Release Process

1. Freeze `main`。
2. Update changelog。
3. Run full test matrix。
4. Run security scan。
5. Run clean VM smoke tests。
6. Create release candidate tag。
7. Build signed assets。
8. Verify checksums。
9. Install and run each asset。
10. Promote tag to stable。
11. Publish GitHub Release。
12. Update updater manifest。
13. Publish website。
14. Start launch sequence。
15. Monitor crash / issues。

## 5. Auto Updater

- Signed update metadata。
- Stable / beta channels。
- No silent forced update。
- Show release notes。
- Support skip version。
- Updater failure must not corrupt installed app。
- `latest.json` generated from Release assets。
- Rollback to prior version documented。

## 6. External Credentials Checklist

Codex 應建立 `EXTERNAL_ACTIONS_REQUIRED.md`，包含：

- Apple signing variables。
- Apple notarization variables。
- Windows certificate location / secret。
- GitHub environment secrets。
- Domain DNS。
- Product Hunt / X / YouTube accounts。

不得把真實 secret 寫入 repository。
