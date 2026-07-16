# M1 10,000-file manifest scan benchmark

Date: 2026-07-16

Status: local development evidence, not a release or 8 GB certification.

## Environment

- macOS 26.5.1 (25F80), Apple Silicon (`arm64`).
- Rust 1.97.0.
- Debug CLI build; bundled SQLite through `rusqlite 0.40.1`.
- Synthetic tree: 10,000 text files across 100 child folders; 10,101 total nodes including the authorized root.
- Scanner reads filesystem metadata only. It does not open file contents.

## Commands

```bash
deskgraph fixture generate --path /private/tmp/deskgraph-m1-10k --files 10000 --directories 100
deskgraph manifest init --database /private/tmp/deskgraph-m1-10k.sqlite3
deskgraph scope add --database /private/tmp/deskgraph-m1-10k.sqlite3 --path /private/tmp/deskgraph-m1-10k
deskgraph scan start --database /private/tmp/deskgraph-m1-10k.sqlite3 --scope 1
deskgraph manifest stats --database /private/tmp/deskgraph-m1-10k.sqlite3
```

The generator refuses an existing destination and caps the requested size. It never deletes or overwrites an existing fixture.

## Results

| Run               | Scanner elapsed | Process wall time |  Files | Folders | Skipped/issues | Active nodes after run |
| ----------------- | --------------: | ----------------: | -----: | ------: | -------------: | ---------------------: |
| Initial scan      |         1.366 s |            2.40 s | 10,000 |     101 |          0 / 0 |                 10,101 |
| Idempotent rescan |         1.260 s |            2.07 s | 10,000 |     101 |          0 / 0 |                 10,101 |

After three completed scans, manifest statistics still reported exactly 10,000 files, 101 folders, 10,101 active locations, and 10,101 distinct active nodes. No logical duplicates accumulated.

## Limitations and next evidence

- Peak RSS was not captured: the restricted local runner denied `ps` process inspection, and the escalation request could not be reviewed because the tool approval quota was exhausted. Timing and manifest counts remain valid; memory is explicitly unverified.
- This run used a debug build on macOS arm64. M1 still needs Windows runtime identity fixtures, macOS case-behavior coverage, permission-denied fixtures, persistent pause/resume, and remote CI evidence.
- The v0.1 8 GB gate remains open. It requires a release build on documented 8 GB hardware, idle and peak RSS, CPU, database size, thermals, and repeated-scan results.
