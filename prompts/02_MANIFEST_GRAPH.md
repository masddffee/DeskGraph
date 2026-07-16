# Phase 02 — Manifest Graph

Implement milestone M1.

Build:
- explicit scope configuration
- safe directory scanner
- file identity service
- exclusion engine
- SQLite migrations
- File and Folder nodes
- located_in and identity relations
- scan jobs with progress, pause and resume
- deduplicated incremental rescans

Requirements:
- never scan system or hidden sensitive directories by default
- canonicalize and validate every path
- handle symlinks, junctions, hardlinks, Unicode and case sensitivity
- do not follow loops
- record permission failures without stopping the scan
- preserve identity across path changes when platform metadata permits

Create a 10k synthetic fixture generator and benchmark.

Acceptance:
- scan is idempotent
- no duplicate logical files after rescan
- exclusions are tested
- app and CLI can display graph statistics
- database migrations are documented
