# Phase 05 — Project and Context Graph

Implement milestone M4.

Build:
- folder semantic profiles
- candidate entities and topics
- similarity edges
- version and duplicate relations
- project clustering
- confidence and provenance model
- user confirmation, merge, split and reject
- learning from accepted and rejected memberships
- project overview page

Use deterministic and embedding signals before any LLM.
An optional local LLM may only disambiguate low-confidence candidates and must return schema-validated JSON.

Each inferred edge must include:
- confidence
- provenance
- observed time
- creator/provider
- model version when applicable

Acceptance:
- users can understand why a file belongs to a project
- users can correct the graph
- corrections affect future suggestions
- low-confidence relations remain suggestions
