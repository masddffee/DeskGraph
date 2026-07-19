# ADR-030: Cleanup ActionPlan Preview Is an Independent, Non-executable Family

- Status: Accepted
- Date: 2026-07-19

## Context

ADR-029 established a path-free, suggestion-only Smart Cleanup Inbox. The next safe step is to
bind one explicit target, its evidence and any keeper into an immutable preview. The existing
Rename ActionPlan and journal are deliberately closed to `rename` and `direct_rename_intent`;
widening those tables for System Trash would corrupt an already-audited state machine.

D-017 has not selected cross-platform Trash receipts, restore semantics or fault recovery. A
preview therefore cannot authorize, confirm or execute a Trash action.

## Decision

- Cleanup uses independent `cleanup_action_plans` and `cleanup_action_journal_events` tables.
  Migration 0022 does not modify or reuse the Rename plan, command, lease, receipt or journal
  family.
- One plan binds exactly one selected target to one current `suggested` exact-duplicate,
  directional-version or screenshot-review observation. A version plan only permits the older file
  as target and the newer file as keeper.
- Exact-duplicate and version plans require a keeper. Screenshot review remains item-by-item and may
  omit a keeper. SQLite enforces the complete optional binding shape; partial `NULL` bindings and
  keeper-less duplicate/version plans are invalid.
- The target and keeper are independently bound through read-only handles to their current location,
  strong identity, size, modification time, SHA-256, authorized root identity and parent identity.
  The source observation and active completed-scan scope are rechecked in the same
  `BEGIN IMMEDIATE` transaction that persists the plan.
- Exact duplicates are compared byte-for-byte through held handles. Their final SHA-256 and hashed
  byte counts must also match at the transaction, database-validation and SQLite-schema layers.
- The durable plan and sequence-1 `preview_created` event commit atomically and are immutable.
- The public domain preview is path-free and fixed to `confirmation_required: true`,
  `action_authorized: false` and `execution_available: false`. It contains no path, filename,
  content, OCR, embedding, raw hash, grant, Trash location or receipt.
- This slice has no CLI, Tauri or Desktop creation entry point. It is a reviewed core foundation,
  not a user-complete feature.
- No confirmation nonce, command request, executor lease, Trash adapter, recovery or Undo API is
  introduced. A future confirmation must rerun equivalent live pair validation and must not trust
  the historical preview alone.

## Consequences

- Cleanup and Rename retain independent schemas and state machines, so future Trash receipt and
  restore semantics cannot silently weaken Rename guarantees.
- The extra keeper hash and topology binding costs bounded local I/O at explicit preview time. No
  network, model or new dependency is introduced.
- A durable preview is historical evidence, not consent. It never moves a file and cannot be
  upgraded into execution without a new accepted ADR, D-017, platform runtime fault evidence and a
  user-visible confirmation contract.
- The next vertical slice must define an explicit local preview-detail disclosure boundary so a user
  can identify the selected files without placing paths in Inbox, history or logs.

## Rejected alternatives

- Add System Trash operations to the existing Rename tables or journal vocabulary.
- Store only a keeper node ID without its immutable identity, hash and topology.
- Allow a version candidate to target the newer file.
- Treat an earlier byte comparison as sufficient without comparing the final hashes.
- Add disabled or internal Execute, Trash, Delete, Recovery or Undo commands before D-017.
