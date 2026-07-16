# M1 10,000-file manifest scan benchmark

Date: 2026-07-16

Status: local development evidence, not a release or 8 GB certification.

## Environment

- macOS 26.5.1 (25F80), Apple Silicon (`arm64`).
- Rust 1.97.0.
- Optimized release CLI build; bundled SQLite through `rusqlite 0.40.1`.
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
| Initial scan      |         4.489 s |            4.84 s | 10,000 |     101 |          0 / 0 |                 10,101 |
| Idempotent rescan |         4.217 s |            5.02 s | 10,000 |     101 |          0 / 0 |                 10,101 |

The current scanner uses a durable path queue, job-scoped staging, bounded batches, and one final atomic manifest publish. After repeated completed scans, manifest statistics still reported exactly 10,000 files, 101 folders, 10,101 active locations, and 10,101 distinct active nodes. No logical duplicates accumulated.

An earlier per-entry timer reported an invalid 2 ms because sub-millisecond samples were truncated and SQLite staging was excluded. That result was rejected. The scanner now persists active runner wall time once per bounded batch; the current values above were captured only after that fix and cross-checked with `/usr/bin/time -p`.

## Limitations and next evidence

- Peak RSS was not captured: `/usr/bin/time -l` completed the scan but the restricted runner denied `sysctl kern.clockrate`, so macOS emitted no rusage fields. Timing and manifest counts remain valid; memory is explicitly unverified.
- Permission-denied isolation, persistent pause/resume, crash-reopen replay, protected-system descendants, Finder hidden flags, and filesystem case behavior are locally tested on macOS. M1 still needs Windows junction/hidden-attribute runtime fixtures, a live smoke of the latest pause/resume UI, and remote CI evidence.
- The v0.1 8 GB gate remains open. It requires a release build on documented 8 GB hardware, idle and peak RSS, CPU, database size, thermals, and repeated-scan results.
