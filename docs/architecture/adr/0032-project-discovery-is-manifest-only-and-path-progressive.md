# ADR-032: Project Discovery Is Manifest-Only and Reveals Paths Progressively

- Status: Accepted
- Date: 2026-07-19

## Context

ADR-018 defines deterministic direct-marker Project Suggestions, and ADR-019 stores exact-root suggestions plus append-only user feedback. The Desktop still requires users to propose one known root at a time and has no backend-backed Project Discovery flow. Automatically returning every root path would widen ordinary IPC and history beyond ADR-019, while rescanning the filesystem or accepting marker facts from the WebView would bypass the manifest and authorization boundaries.

## Decision

- An explicit local `discover_projects(scope_id)` request reads only current SQLite manifest rows. It accepts no path, marker list, content, model output, or filesystem capability from the WebView.
- Discovery requires a completed scan, a durable active platform grant, and—at the Tauri boundary—the matching live scope capability held by the current process. Each command repeats these checks while holding the existing writer gate.
- Candidate roots are folders with a current direct child matching ADR-018's strong marker rules. README remains supporting evidence only. The query correlates the marker location with its exact parent location so a multiply located identity cannot lend a filename from another parent.
- One request evaluates at most 100 stable root node IDs in deterministic order. If more roots exist, the response returns the verified first 100 with `evaluation_complete: false`; it never claims the scope was fully evaluated.
- The discovery response contains only path-free candidate summaries, bounds, completion state, and explicit `automatic_membership_created: false` / `file_actions_available: false` flags. Logs contain IDs and aggregate counts only.
- A root path and ordered marker evidence are returned only after the user explicitly requests one candidate detail. Detail re-derives the current Folder Profile and appends the current immutable suggestion observation before returning `user_requested_path: true` and `current_evidence: true`.
- Accept or reject also repeats current evidence validation and appends ADR-019 feedback. It does not create file membership, move, rename, Trash, delete, or authorize any other action. Repeated discovery preserves the latest accepted or rejected state for the stable root identity.
- The Desktop must expose authorization/scan-required, loading, empty, bounded-partial, error, suggested, accepted, and rejected states in every supported UI language. Late path-bearing detail or decision responses are discarded after scope, refresh, close, or view changes.
- This slice adds no migration, registry dependency, model, network client, filesystem traversal, background discovery, learned scoring, Project membership, merge, or split behavior.

## Consequences

- Users can discover explainable Project roots without knowing their node IDs or supplying paths one by one.
- Ordinary discovery remains privacy-minimal; one path appears only within the user's explicit transient review.
- More than 100 current roots require a future pagination policy. The visible partial state is usable but cannot be presented as a complete inventory.
- Accepted roots still are not Project membership graphs. Membership scoring/correction, related files, clustering, merge/split, and retrieval filters remain later M4 work.

## Rejected alternatives

- Traverse the authorized filesystem again during Project Discovery.
- Let React provide paths, markers, confidence, or Project evidence.
- Return all root paths in the discovery list or structured logs.
- Auto-accept high-confidence roots or treat a root decision as descendant membership.
- Hide truncation or persist an unbounded result set.
