# ADR-031: Cleanup Preview Detail Is Explicit, Local, and Transient

- Status: Accepted
- Date: 2026-07-19
- Owners: DeskGraph maintainers

## Context

ADR-029 keeps the Smart Cleanup Inbox path-free and suggestion-only. ADR-030 adds an immutable,
non-executable Cleanup ActionPlan Preview core but deliberately exposes no public creation entry.
A user cannot safely choose a duplicate, older version, or screenshot target from opaque member
numbers alone. The Desktop therefore needs a narrow way to identify the current local files
without turning paths into Inbox, history, log, preference, analytics, or durable-plan data.

## Decision

- The path-free Inbox may open one source only after an explicit user action.
- `get_cleanup_source_detail` accepts only scope, source and observation IDs. Rust requires the
  live Desktop scope capability and durable active grant, then revalidates the completed scan,
  current suggested observation and source-specific evidence.
- Exact duplicates receive a fresh bounded full-byte verification; either member may be the
  target and the other is the keeper. Versions permit only older target and newer keeper.
  Screenshot groups permit one explicitly selected target and no keeper in this slice.
- The versioned detail response may contain each current member's display path, node ID and
  bounded size only. Paths are rendered as text in the current local review and are cleared on
  close, refresh, scope change, leaving Inbox, error, or successful Preview creation.
- Detail paths are not persisted, logged, cached, copied into the path-free Inbox or Preview,
  stored in preferences, exposed through CLI/MCP, or sent to any remote service.
- `create_cleanup_preview` accepts only source/member IDs, holds the Desktop database gate and
  live scope registry for the worker, and delegates to the ADR-030 transaction core. The result is
  an immutable, path-free sequence-1 Preview with `confirmation_required: true`,
  `action_authorized: false`, and `execution_available: false`.
- There is no Cleanup confirmation, Trash, Delete, Execute, Recovery, Undo, background, rule,
  LLM, CLI, or MCP entry. D-017 remains required before any filesystem action is designed.

## Consequences

- Users can identify the exact local target and keeper before sealing a durable Preview.
- Opening relation detail can create a newer verified observation; Preview creation must use the
  returned observation ID rather than the earlier Inbox ID.
- A path is intentionally visible only in the explicit local review surface. Future telemetry,
  history, export, or session persistence must not reuse this DTO without a new decision.
- This slice still cannot move any file. A durable Preview is evidence, not consent or execution
  authority.
