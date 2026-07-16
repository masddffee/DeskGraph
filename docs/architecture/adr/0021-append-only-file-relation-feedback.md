# ADR-021: File Relation Feedback Is Append-Only and Reverified

- Status: Accepted
- Date: 2026-07-16

## Context

ADR-020 creates an explainable exact-duplicate suggestion, but a relation that users cannot correct is not a safe graph source of truth. A mutable state column would erase feedback history, while accepting or rejecting an old byte-equality observation without rereading both files could apply a decision to stale content.

## Decision

- Migration 0010 adds append-only `file_relation_feedback_events`. Update and delete triggers protect every event. A relation remains immutable and keeps its prior observations.
- A new exact-duplicate relation begins `suggested`. Only an explicit local user command may append `accepted` or `rejected`; every event records `created_by: user`, a per-relation sequence, and decision time.
- Repeating the latest decision is idempotent. An opposite decision appends the next sequence and becomes the current state without rewriting earlier feedback.
- The Rust relation service must run ADR-020's complete live verification and append a new immutable byte-equality observation before asking the database to append feedback. Stale, changed, absent, out-of-scope, aliased, oversized, or different files cannot receive a new decision.
- The latest decision affects later checks for the exact stable scope/node pair only. A rejected pair remains rejected after later successful byte verification; a later explicit accept corrects it. Feedback does not affect other files, Project roots, similarity scores, or version inference.
- Explicit duplicate/verify/decide responses may contain the two current paths. The recent relation-history list is path-free, labels itself `verification_required`, and does not claim that an old observation is current.
- The slice performs no merge, delete, rename, move, deduplication, membership assignment, model call, or filesystem mutation and adds no registry dependency.

## Consequences

- Exact-duplicate suggestions now have a durable, auditable correction loop without turning acceptance into an organization action.
- Reverification before every decision trades extra bounded reads for honest current-data semantics.
- A path-free history can support later UI work without exposing local locations or representing history as live verification.
- Relation feedback remains exact-pair suppression only. Cross-relation learning, automatic grouping, merge/split, and file membership need separate decisions and evaluation.

## Rejected alternatives

- Store only a mutable relation state.
- Accept or reject a relation without live byte revalidation.
- Let a model, background rule, or extracted content create feedback.
- Propagate one pair's feedback to other candidate pairs.
- Treat `accepted` as authorization to merge, delete, move, or rename files.
- Return paths in recent relation history or ordinary logs.
