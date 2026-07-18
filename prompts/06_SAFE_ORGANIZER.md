# Phase 06 — Safe File Organization

Implement milestone M5.

Build:
- immutable ActionPlan proposal
- policy validator
- before/after preview
- move and rename conflict resolution
- move-to-system-trash plans and platform adapters
- durable transaction journal
- executor
- rollback
- user-facing undo history
- startup crash recovery
- audit events

Hard constraints:
- do not implement permanent deletion
- do not implement empty-trash or any standalone, permanent, non-journaled unlink/remove-file path; only a durable cross-volume Move may remove its original source name after copy → verify → commit, with no overwrite, crash recovery, and Undo. The system-trash adapter has no generic delete fallback
- LLM output never reaches filesystem APIs directly
- cleanup suggestions never authorize execution; require an exact user selection and confirmation for every item or bounded batch
- every selected cleanup item creates an immutable ActionPlan containing candidate and optional keeper IDs, exact current evidence IDs, source identity, SHA-256, expected bytes, and a bounded confirmation nonce; any candidate, keeper, evidence, scope, identity, hash, or size change makes it stale and requires a new preview and confirmation
- revalidate file identity immediately before execution
- prevent scope escape and path traversal
- handle cross-volume moves safely
- verify destination identity/hash
- make undo idempotent
- journal each file independently even when the user confirms a batch
- cap a v0.1 batch at 100 items and 100 GiB expected bytes; execute serially, stop after the first non-completed outcome, preserve completed items as independently undoable, mark the rest `not_started`, and never perform batch rollback
- treat system trash as an action-bound OS capability, never as an authorized path: the adapter may act only on the plan-bound source, persist/verify only its opaque receipt and exact item identity, and must not enumerate, search, index, or expose trash paths or contents to general APIs, MCP, LLMs, Graph, Search, or Inbox
- restore from system trash only when the exact item still exists and identity matches; if it was emptied or changed outside DeskGraph, record `needs_attention` honestly and never fabricate Undo success

Add fault injection tests for:
- permission denied
- destination conflict
- process termination
- external drive removal
- source changed after preview
- partial cross-volume copy
- trash destination collision or unavailable platform trash
- process termination before and after the platform trash call
- system trash emptied or item changed outside DeskGraph before Undo
- cleanup candidate or keeper refreshed between preview, confirmation, and execute
- 100-item/100-GiB batch boundary, first-failure stop, and per-item completed/not_started/needs_attention outcomes

Acceptance:
- all transaction tests pass
- no non-journaled move/rename/trash code path exists
- macOS Trash and Windows Recycle Bin runtime matrices pass; Linux freedesktop Trash is an experimental, non-blocking artifact gate
- no permanent-delete or empty-trash capability exists in the product, CLI, Desktop, MCP, generated rules, or recovery paths
- UI explains what happened and how to undo
