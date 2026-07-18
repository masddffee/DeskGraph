# GitHub Issue Backlog

Create these as GitHub Issues with labels, dependencies and acceptance criteria.

## Epic: Foundation

- REP-001 Bootstrap Rust/Tauri monorepo
- REP-002 Add CI matrix
- REP-003 Add repository governance files
- REP-004 Add structured logging
- REP-005 Add configuration and feature flags

## Epic: Filesystem Manifest

- FSC-001 Scope allowlist
- FSC-002 Cross-platform scanner
- FSC-003 File identity
- FSC-004 Exclusion rules
- FSC-005 Scan job persistence
- FSC-006 Synthetic fixture generator
- FSC-007 Symlink/junction safety
- FSC-008 Cloud placeholder handling

## Epic: Extraction

- EXT-001 Extractor provider contract
- EXT-002 Text and Markdown
- EXT-003 Source code
- EXT-004 PDF
- EXT-005 DOCX
- EXT-006 PPTX
- EXT-007 XLSX
- EXT-008 Image metadata
- EXT-009 OCR provider
- EXT-010 Size/time/decompression limits

## Epic: Graph

- GRF-001 Node/edge schema
- GRF-002 Edge provenance
- GRF-003 Similarity relations
- GRF-004 Duplicate/version relations
- GRF-005 Folder profiles
- GRF-006 Project discovery
- GRF-007 User correction feedback
- GRF-008 Project merge/split
- GRF-009 M2-provenance-backed screenshot grouping and current-evidence invalidation

## Epic: Search

- SRC-001 FTS5 indexing
- SRC-002 Embedding provider
- SRC-003 Vector adapter
- SRC-004 Hybrid score fusion
- SRC-005 Search diagnostics
- SRC-006 Multilingual evaluation
- SRC-007 Search UI
- SRC-008 Related files

## Epic: Safe Actions

- ACT-001 ActionPlan schema
- ACT-002 Policy validator
- ACT-003 Preview
- ACT-004 Transaction state machine
- ACT-005 Move/rename executor
- ACT-006 Conflict resolver
- ACT-007 Undo
- ACT-008 Crash recovery
- ACT-009 Fault injection
- ACT-010 Cross-platform system-trash ActionPlan and adapter
- ACT-011 System-trash crash recovery, conflict handling, and idempotent Undo
- ACT-012 Closed by ADR-026: retain general Unix Rename/Move Preview-only and keep the source-leaf counterexample as a production-gate regression
- ACT-013 Closed at architecture level by ADR-027: packaged identity precedes the action fence and the Tauri Rust core remains the sole v0.1 action host
- ACT-014 Gate the macOS App Sandbox/SIP container `flock` candidate on D-002 and a signed non-entitled same-user replacement probe; if accepted, prove fail-before-database, pause/crash, close-on-exec, ownership/bookmark/update/repair/uninstall matrix
- ACT-015 Windows protected private-namespace mutex fence; prove package-family/DACL, native-thread ownership, busy/abandoned recovery, handle inheritance, update, repair, and uninstall matrix

## Epic: Watch and Inbox

- WAT-001 Watcher abstraction
- WAT-002 Stability detection
- WAT-003 Event reconciliation
- WAT-004 Smart Inbox
- WAT-005 Resource scheduler
- WAT-006 Generated rules
- WAT-007 Smart Cleanup candidate source, M2 evidence dependency, and stale-evidence invalidation

## Epic: Desktop UX

- UI-001 Onboarding
- UI-002 First scan
- UI-003 Dashboard
- UI-004 Search
- UI-005 Projects
- UI-006 Inbox
- UI-007 Preview
- UI-008 History/Undo
- UI-009 Settings
- UI-010 Privacy diagnostics
- UI-011 Smart Cleanup evidence, selection, confirmation, history, and Undo

## Epic: MCP

- MCP-001 Server bootstrap
- MCP-002 Scope middleware
- MCP-003 Search tools
- MCP-004 Context tools
- MCP-005 Explain relation
- MCP-006 Audit log
- MCP-007 Client setup docs
- MCP-008 Injection tests

## Epic: Release

- REL-001 macOS packaging
- REL-002 Windows packaging
- REL-003 Linux experimental
- REL-004 Signing
- REL-005 Updater
- REL-006 SBOM/checksums
- REL-007 Clean VM smoke
- REL-008 GitHub release workflow
- REL-009 Required macOS Trash and Windows Recycle Bin acceptance matrix
- REL-010 Linux experimental freedesktop Trash artifact and evidence
- REL-011 macOS signed App Sandbox entitlement, native folder selection, security-scoped bookmark migration, supported-version SIP guarantee, hostile replacement probe, and packaged-container evidence
- REL-012 Windows package identity shared by native OCR and the action-fence private namespace

## Epic: Launch

- MKT-001 README
- MKT-002 Demo GIF
- MKT-003 Product video
- MKT-004 Landing page
- MKT-005 Show HN
- MKT-006 Product Hunt
- MKT-007 Social launch kit
- MKT-008 Analytics dashboard
