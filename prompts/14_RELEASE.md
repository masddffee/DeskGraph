# Phase 14 — Publish v0.1

Prepare and publish the first public release.

Before publishing:
- read release gates
- verify git status and tag
- run full CI, security and benchmarks
- verify installers on clean systems
- confirm README download links
- verify updater metadata
- create release notes and known limitations

When GitHub authentication and permissions are available:
- create the stable tag
- publish GitHub Release
- attach all binaries, checksums and SBOM
- update website
- open launch discussion
- enable issue templates and discussions

If blocked by credentials, complete a release candidate and create EXTERNAL_ACTIONS_REQUIRED.md with exact commands.

After release:
- verify public downloads
- install from public release
- run smoke test
- record release status
