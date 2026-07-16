# ADR-009 — Shared privacy-safe health contract

- Status: Accepted
- Date: 2026-07-16

## Context

M0 needs one runnable slice across Rust CLI and Tauri UI before filesystem scope, SQLite, or optional providers exist. A diagnostic command can easily leak paths or imply unavailable features are working.

## Decision

Define the health report in the Rust domain crate and reuse it in the CLI and Tauri command. The closed schema contains only product/version, OS/architecture, lifecycle states, and privacy flags. It never contains filenames, filesystem locations, environment variables, user identifiers, content, OCR, embeddings, or graph data.

Database and provider states are reported honestly as not initialized or disabled. Structured logs contain fixed event names and lifecycle states only.

## Consequences

The CLI and desktop cannot drift on diagnostic meaning. The contract remains useful without a model, database, network, or API key. M1 may add an actual database probe only if it preserves the same privacy boundary and tests.

## Alternatives considered

Separate frontend and CLI status objects were rejected because they can diverge. Reporting application data directories was rejected because it adds no M0 health value and discloses local context.

## Validation and revisit trigger

Rust serialization, CLI integration, and frontend tests must cover schema and no-location behavior. Revisit when M1 introduces a real database lifecycle, not before.
