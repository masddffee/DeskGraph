# ADR-018: Folder Profiles Are Bounded Derivations and Project Membership Remains a Suggestion

- Status: Accepted
- Date: 2026-07-16

## Context

Project discovery needs useful evidence before embeddings, clustering, or an LLM are available. It must not turn filenames into opaque membership edges, scan outside an authorized scope, allocate an unbounded tree in memory, or let a heuristic become graph truth. The current SQLite manifest already contains identity-separated, present File/Folder locations produced by an atomic completed scan.

## Decision

- `deskgraph.folder-profile.v1` is a read-only, on-demand derivation from current `present` manifest locations. A folder is selected by `(scope_id, folder_node_id)`; an explicit CLI path is canonicalized only to resolve that existing manifest identity.
- One profile streams at most 100,000 descendant locations. The database requests one extra row and fails closed with `folder_profile_entry_limit_exceeded`; it never returns a partial profile.
- The profile reports direct and descendant location counts, total file bytes, latest file modification time, and a fixed extension-based category distribution. These are deterministic manifest facts, not semantic claims.
- Project marker rules inspect only direct children. `Cargo.toml`, `package.json`, `pyproject.toml`, `go.mod`, `Package.swift`, `*.xcodeproj`, and `*.sln` are strong signals; README variants are explanation-only supporting evidence.
- A strong marker creates a `ProjectSuggestion`, never a `belongs_to` edge or automatic membership. The suggestion includes bounded basis-point confidence, all observed marker provenance, completed-scan observation time, `system_rule` creator, fixed provider ID/version, and `model_version: null`. Additional strong markers can raise confidence, capped at 9,500 basis points.
- The explicit profile response may contain the selected folder path because the user requested it. Structured logs contain only IDs, aggregate counts, and whether a suggestion exists; descendant filenames and paths are not returned.
- The slice adds no registry dependency, model, embedding runtime, API, network client, or filesystem mutation. It remains useful with no LLM, Python, Docker, Ollama, or API key.
- Persisted project identities/edges, related/duplicate/version relations, clustering, confirmation/rejection/merge/split, correction feedback, retrieval filters, and a Project page require later decisions and tests. No M4 completion claim is allowed before those gates pass.

## Consequences

- Profiles describe the last atomically published manifest and may be stale until scan/watch reconciliation completes; `observed_at_unix_ms` makes that boundary visible.
- Direct markers provide an explainable bootstrap but cannot establish all real projects or monorepo boundaries.
- README alone does not create a project suggestion.
- A future persistence model can consume the versioned profile contract without granting heuristic code direct file access or turning suggestions into automatic membership.

## Rejected alternatives

- Require embeddings or an LLM before showing any project evidence.
- Treat every README-bearing folder as a project.
- Persist an inferred `belongs_to` edge without confirmation and correction semantics.
- Return all descendant paths to the caller or include them in logs.
- Traverse the live filesystem again instead of reading the current authorized manifest.
