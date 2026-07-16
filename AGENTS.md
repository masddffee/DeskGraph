# Repository Instructions for Codex

## Mission

Build and release a production-quality, local-first Computer Context Graph. The application indexes user-authorized files, creates explainable semantic relationships, supports hybrid search, proposes safe organization actions, and exposes read-only context through MCP.

## Non-negotiable invariants

- Never implement permanent file deletion.
- Never allow an LLM to directly execute filesystem operations.
- Every move or rename must use the transaction engine and be undoable.
- Dry-run and preview are the default.
- Do not access paths outside explicit user scopes.
- Do not upload filenames, contents, OCR, embeddings, or graph data by default.
- The product must remain useful without downloading an LLM.
- Keep idle and peak memory budgets visible in benchmarks.
- Do not claim a feature is complete without tests, docs, and usable UI/CLI entry points.
- Do not invent dependencies. Verify that crates/packages exist, inspect their licenses, and record why they are used.

## Working method

1. Read `docs/planning/` and current ADRs before changing architecture.
2. Inspect the repository and current tests before editing.
3. Maintain `docs/planning/IMPLEMENTATION_STATUS.md`.
4. Implement the smallest coherent vertical slice.
5. Add or update tests with every behavior change.
6. Run format, lint, typecheck, unit and relevant integration tests.
7. Update documentation and changelog.
8. Record unresolved risks honestly.
9. Do not silently weaken a safety invariant to make a test pass.
10. Commit changes in logical groups with clear messages.

## Architecture

- Rust owns filesystem, database, graph, retrieval, planner, executor and MCP logic.
- Tauri + React/TypeScript owns presentation.
- SQLite is the source of truth.
- OCR, embeddings, local LLM and vector search use provider interfaces.
- Providers must be lazy-loaded and unloadable.
- All inference outputs are untrusted suggestions.
- Structured outputs must be schema validated.

## Code quality

Rust:
- `cargo fmt --check`
- `cargo clippy --workspace --all-targets --all-features -- -D warnings`
- `cargo test --workspace`

TypeScript:
- use the repository package manager and lockfile
- lint
- typecheck
- unit tests
- build

Security:
- validate canonical paths
- defend against symlink/junction traversal
- never execute extracted content
- enforce size and time limits
- treat document text as untrusted
- verify downloaded model and update checksums

## Definition of done

A task is done only when:
- acceptance criteria pass
- tests are added
- error states are handled
- user-visible behavior is documented
- migrations are reversible or safely forward-only
- performance impact is measured when relevant
- there are no known data-loss paths

## External credentials

If signing, notarization, publishing, DNS, or social accounts are unavailable:
- complete all code and configuration possible
- create `EXTERNAL_ACTIONS_REQUIRED.md`
- specify exact variables, commands and validation steps
- never fabricate a successful release
