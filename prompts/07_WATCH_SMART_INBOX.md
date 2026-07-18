# Phase 07 — Watch Mode and Smart Inbox

Implement milestone M6.

Build:
- OS filesystem watcher abstraction
- event debounce and reconciliation
- file stability checks
- incremental extraction/indexing
- Smart Inbox states
- Smart Cleanup Inbox candidates for exact duplicates, evidence-backed older versions, and explainable screenshot groups
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
- cleanup rules may prepare suggestions but can never move an item to system trash without a fresh explicit user confirmation
- duplicate suggestions require exact current-file verification and user-selected keeper; version suggestions cannot rely on timestamps, filenames such as `final`, or model confidence alone
- screenshot grouping is review assistance, never proof that an image is disposable; each item remains independently selectable
- screenshot groups require current M2 image metadata and, when used, OCR/provider provenance; time proximity, similar filenames, or model confidence alone cannot prove disposability
- every confirmed cleanup selection delegates to the M5 system-trash ActionPlan and transaction engine; Inbox code never calls filesystem APIs
- confirmation is bound to the immutable candidate/keeper/evidence snapshot in that ActionPlan; background refresh cannot silently replace it

Acceptance:
- temporary downloads are ignored until stable
- rename storms do not create duplicates
- watcher resumes after app restart
- low-memory mode works
- cleanup candidates show current evidence, keeper/items, selection, count and estimated bytes without claiming reclaimable space as guaranteed
- changed/stale evidence invalidates the suggestion before confirmation
- v0.1 batches contain at most 100 items and 100 GiB expected bytes; each item stays visible/selectable and returns an independent `completed`, `not_started`, or `needs_attention` outcome
- no cleanup candidate, generated rule, notification, LLM output, or batch bypasses Preview, Policy Validation, durable journal, platform-trash execution, verification, crash recovery, and Undo
