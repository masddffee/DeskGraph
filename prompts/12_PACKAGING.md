# Phase 12 — Packaging, Signing and Updater

Implement milestone M9.

Create:
- macOS universal or split signed packages
- Windows signed installer
- experimental Linux package
- checksums
- SBOM
- signed updater metadata
- stable and beta channels
- GitHub Actions release workflow
- clean VM smoke workflow
- rollback instructions

The updater must not be able to install unsigned metadata or artifacts.

If signing credentials are unavailable:
- build unsigned internal artifacts
- configure protected GitHub environments and secret names
- generate exact credential instructions in EXTERNAL_ACTIONS_REQUIRED.md
- do not label unsigned artifacts as production-ready

Acceptance:
- tag-driven release candidate
- verified assets
- installer starts and performs first scan
- updater dry run succeeds
