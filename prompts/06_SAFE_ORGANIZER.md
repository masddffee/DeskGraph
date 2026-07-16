# Phase 06 — Safe File Organization

Implement milestone M5.

Build:
- immutable ActionPlan proposal
- policy validator
- before/after preview
- move and rename conflict resolution
- durable transaction journal
- executor
- rollback
- user-facing undo history
- startup crash recovery
- audit events

Hard constraints:
- do not implement permanent deletion
- LLM output never reaches filesystem APIs directly
- revalidate file identity immediately before execution
- prevent scope escape and path traversal
- handle cross-volume moves safely
- verify destination identity/hash
- make undo idempotent

Add fault injection tests for:
- permission denied
- destination conflict
- process termination
- external drive removal
- source changed after preview
- partial cross-volume copy

Acceptance:
- all transaction tests pass
- no non-journaled move/rename code path exists
- UI explains what happened and how to undo
