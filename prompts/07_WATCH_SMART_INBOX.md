# Phase 07 — Watch Mode and Smart Inbox

Implement milestone M6.

Build:
- OS filesystem watcher abstraction
- event debounce and reconciliation
- file stability checks
- incremental extraction/indexing
- Smart Inbox states
- background pause/resume
- battery and resource controls
- notification preferences

States:
- indexed
- auto-confident
- needs review
- unclassified
- conflict
- unavailable
- failed

Default behavior:
- suggest only
- no automatic moves until user explicitly enables a rule
- generated rules are previewable and editable

Acceptance:
- temporary downloads are ignored until stable
- rename storms do not create duplicates
- watcher resumes after app restart
- low-memory mode works
