# DeskGraph Project Context

## Problem and users

DeskGraph serves developers, designers, researchers, students, creators, and privacy-sensitive knowledge workers whose files are scattered across explicitly authorized folders. It should help them recover files from remembered context, discover project relationships, safely organize new material, and give AI agents narrowly scoped read-only context.

## Product promise

> **Graphify your computer.**

DeskGraph is a local-first computer context graph. It connects, searches, and safely organizes user-authorized files without uploading filenames, paths, extracted content, OCR, embeddings, or graph data by default.

## v0.1 success outcome

The 90-day-equivalent success outcome for this repository is a publicly downloadable Version B production open-source MVP:

- reproducible macOS Apple Silicon and Intel or Universal builds;
- Windows x64 installer;
- experimental Linux build that does not delay macOS or Windows;
- a useful no-model path from explicit scope selection through scan, search, project context, safe preview, journaled move/rename, undo, and read-only MCP;
- published tests, security evidence, 8 GB benchmarks, checksums, SBOM, documentation, demos, and release operations.

Version A is only an internal demonstrable milestone. Version C capabilities remain architectural extension points unless they do not endanger v0.1 delivery.

## Highest-risk assumptions

1. A whole-file context graph can stay useful and responsive on an 8 GB computer.
2. Cross-platform file identity and watch reconciliation can avoid duplicate or lost logical files.
3. Move and rename can remain journaled, crash-recoverable, and undoable across permissions, conflicts, and volumes.
4. Local OCR and multilingual retrieval can be packaged without Python, Docker, Ollama, an API key, or a mandatory LLM.
5. Users will trust inferred relations only when provenance, confidence, scope, and correction controls are visible.

These assumptions must be tested rather than converted into marketing claims.

## Product and engineering invariants

- No permanent deletion.
- No LLM filesystem handles or direct filesystem execution.
- Preview and policy validation precede any file action.
- Move and rename use durable, recoverable, idempotent transactions and support undo.
- Canonical scope validation precedes access; symlink and junction traversal cannot escape scope.
- SQLite is the source of truth.
- Rules first, embeddings second, optional LLM last.
- Core use remains available without models or external credentials.
- Untrusted extracted content never becomes executable instructions.
- A feature is not complete without acceptance evidence, tests, docs, and a usable entry point.

## Source-of-truth precedence

1. Applicable `AGENTS.md` safety rules.
2. Accepted ADRs.
3. Latest documents in `docs/planning/`.
4. Phase prompts in `prompts/`.
5. Existing implementation.
