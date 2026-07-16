# ADR-019: Project Candidates and User Feedback Are Append-Only

- Status: Accepted
- Date: 2026-07-16

## Context

ADR-018 exposes explainable project-root suggestions but deliberately persists no graph claim. M4 requires users to correct the graph and requires corrections to affect future suggestions. A mutable status column would erase decision history; automatically recreating a rejected suggestion would ignore the user; and accepting a root must not silently assign every descendant file to that Project.

## Decision

- `deskgraph.project-candidate.v1` gives a possible Project root a stable local identity keyed by `(scope_id, root_folder_node_id)`. The folder node, not its path, is the identity; explicit status responses resolve the current present location.
- Migration 0008 stores immutable Project root identities, immutable deterministic suggestion observations, normalized immutable signals, and append-only user feedback events. Database triggers reject update and delete for all four tables.
- Persisting a suggestion re-derives the bounded current Folder Profile at the database boundary. Observation time and every ordered signal must exactly match the current manifest facts. Provider ID/version, marker name, weight, confidence formula, creator, and null model version are schema-validated; callers cannot persist invented evidence.
- A new candidate starts as `suggested`. Only the explicit local `project decide` command can append `accepted` or `rejected`, and every decision is `created_by: user`. Repeating the current decision is idempotent; changing it appends the next sequence instead of rewriting history.
- The latest decision controls future results for that exact stable root. A rejected root remains `rejected` when the same evidence is proposed again; an accepted root remains `accepted`; a later opposite user decision corrects the state. Raw evidence remains visible for explanation.
- Accepting a Project root does not create `belongs_to` file memberships, move files, or authorize an LLM action. General feedback learning across different roots, membership scoring, merge/split, and edge correction remain later M4 gates.
- Explicit propose/decide/status responses may contain the current root path. Ordinary logs and the 20-item recent-candidate list expose only stable IDs, state, confidence, times, and decision sequence/time.
- The slice adds no registry dependency, model, embedding/vector runtime, API, network client, or filesystem mutation.

## Consequences

- User decisions survive restart and root-folder renames that preserve platform identity.
- A root absent from the current manifest is excluded from recent results and cannot receive a decision until reconciliation makes it current again; its append-only history is retained.
- Exact-root suppression proves the first correction-feedback loop without introducing opaque cross-user or cross-project learning.
- Future membership, merge, split, or learned scoring schemas require forward migrations and must preserve the immutable observation/feedback audit trail.

## Rejected alternatives

- Store only a mutable accepted/rejected column.
- Automatically accept high-confidence rule results.
- Recreate a fresh suggested state after the user rejects the same root/evidence.
- Treat acceptance of a root as acceptance of all descendant file memberships.
- Let a model or extracted content write Project feedback.
- Return root paths in ordinary lists or logs.
