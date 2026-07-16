# ADR-017: Rename Preview Is Immutable and Journaled Before Any File Action

- Status: Accepted
- Date: 2026-07-16

## Context

DeskGraph must never let an LLM or an unvalidated UI request reach a filesystem mutation. A useful preview still needs a durable, reviewable source of truth: an in-memory before/after string can become stale, disappear after a crash, or be silently changed before execution. Destination conflicts, path traversal, case-only names, stale manifest data, symlink swaps, and weak fallback identities must fail before a plan can be treated as actionable.

## Decision

- `deskgraph.action-plan.v1` is an immutable, core-owned contract. The first bounded operation is a same-folder rename preview for one already-scanned file; folder rename, move, cross-volume copy, execution, rollback, recovery, and undo are not exposed by this slice.
- The source must be an absolute path inside a currently canonical explicit authorized scope and a present file location in the SQLite manifest. Symlink/reparse sources and path-fallback identities are denied.
- Before journaling, the core matches platform identity, size, and modified time against the manifest, then opens the source read-only and verifies the open-handle identity and metadata again.
- The new filename must be one portable Unicode component of at most 255 UTF-8 bytes. Empty/dot names, separators, control characters, Windows-invalid characters and device names, and trailing space/dot are denied. Existing destinations fail closed; an ASCII case-only alias to the same platform identity is recorded as `case_only_staged` for a future two-step executor.
- Migration 0007 writes the immutable plan and sequence-1 `preview_created` journal event in one SQLite transaction. Database triggers reject updates or deletes of either table. Future execution states require a forward migration that expands the append-only event vocabulary; they may not mutate the preview row.
- Explicit preview/status responses may return canonical before/after paths because the user requested them. Ordinary logs and the recent-plan list remain path-free.
- No executor or filesystem write API is part of this decision. A future executor must revalidate the source identity immediately before every action, journal state first, verify the destination, recover after termination, and provide idempotent undo before any UI execution control is enabled.

## Consequences

- A file changed after its last manifest scan cannot produce a preview until the manifest is reconciled again.
- The durable record is useful for review and future recovery design, but a preview is not executable and cannot be described as a completed transaction.
- Unicode case-only rename beyond the explicitly recognized filesystem behavior needs platform fixtures before execution support.
- The local transaction crate adds no registry, model, network, shell, Python, Docker, or native runtime dependency.

## Rejected alternatives

- Keep organization previews only in frontend memory.
- Let an LLM provide a destination path directly to filesystem APIs.
- Allow overwrite or implicit conflict resolution during preview.
- Add a rename command before durable recovery and undo exist.
