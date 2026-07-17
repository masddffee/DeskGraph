# ADR-023: Version Feedback Is Bound to Directional Evidence

- Status: Accepted
- Date: 2026-07-17

## Context

ADR-022 gives each stable unordered file pair an append-only history of directional filename-version observations. A user must be able to accept or reject the current `older` to `newer` interpretation, but relation-wide feedback is unsafe: either file can be renamed so that the direction or explicit version numbers change while the relation ID remains the same. Binding a decision only to the relation would silently apply an old judgment to new evidence.

## Decision

- Migration 0012 adds append-only `file_version_feedback_events`. Every event references both the version relation and the immutable version observation that supplied its evidence. Foreign keys, a per-relation sequence, and update/delete triggers preserve provenance and event order. Existing candidates receive no synthetic decision or backfill and remain `suggested`.
- Before accepting any version decision, the Rust relation service must run ADR-022's complete live scope, path, identity, metadata, open-handle, and filename verification and append a new immutable observation. Missing, stale, out-of-scope, aliased, renamed-during-check, or otherwise invalid sources cannot receive feedback.
- A decision applies only to equivalent directional evidence: the same relation, ordered older/newer node IDs, normalized base and extension, explicit older/newer version numbers, signal kind, confidence, creator, provider ID, provider version, and null model version. Location IDs, sizes, modification times, and observation time are revalidated current snapshots but are not part of the feedback identity; moving a still-authorized file or changing its bytes does not by itself reverse a filename-version judgment.
- The latest decision for equivalent evidence determines candidate state. A different direction, base, extension, version pair, signal, or provider starts as `suggested` even when the stable relation has prior feedback. If an earlier evidence tuple becomes current again, its latest explicit decision becomes current again.
- Repeating the latest decision for equivalent current evidence is idempotent. The opposite decision appends the next relation-wide sequence and becomes current for that evidence without rewriting history for any evidence tuple.
- The API exposes the immutable evidence observation ID on a version decision. The CLI uses a distinct `relation version-decide` command so exact-byte feedback and directional filename feedback cannot be confused. Explicit decision responses may contain the two current paths; recent relation history remains path-free and marked `verification_required`.
- Only an explicit local user command may create feedback. Acceptance is graph correction, not authorization to rename, move, merge, delete, or otherwise mutate either file. The slice performs no content read, model call, membership assignment, or filesystem mutation and adds no dependency.

## Consequences

- User correction survives repeated verification and authorized moves when the directional filename evidence is unchanged.
- A rename that changes direction or version evidence cannot inherit a stale decision; it returns to `suggested` until explicitly decided.
- The database retains a complete local audit trail across corrections and evidence changes without storing another copy of paths or filenames in feedback events.
- Generalized relation learning, version grouping, automatic discovery, organization actions, and model-derived evidence remain separate work.

## Rejected alternatives

- Reuse relation-wide exact-duplicate feedback for version candidates.
- Bind feedback only to the relation ID or latest mutable state.
- Bind feedback to an observation ID without recognizing later equivalent observations, which would discard a user decision after every harmless revalidation.
- Include paths, locations, sizes, modification times, or observation time in the evidence identity.
- Let a model, background rule, or verification command create user feedback.
- Treat acceptance as permission for a filesystem operation.
