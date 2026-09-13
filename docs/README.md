# DeskGraph Documentation

This directory contains the detailed product, architecture, implementation, evidence, and release material that does not belong in the reviewer-first root README.

The root [`README.md`](../README.md) is intentionally concise. For implementation truth, use the canonical documents linked below rather than inferring release status from the overview alone.

## Start here

| Question | Read |
| --- | --- |
| What is DeskGraph trying to become? | [`planning/01_PRODUCT_DEFINITION.md`](planning/01_PRODUCT_DEFINITION.md) |
| What is actually implemented right now? | [`planning/IMPLEMENTATION_STATUS.md`](planning/IMPLEMENTATION_STATUS.md) |
| How is the system structured? | [`architecture/README.md`](architecture/README.md) and [`planning/02_ARCHITECTURE.md`](planning/02_ARCHITECTURE.md) |
| How does the read-only MCP boundary work? | [`MCP.md`](MCP.md) |
| How do I run the detailed CLI/development flows? | [`DEVELOPMENT_REFERENCE.md`](DEVELOPMENT_REFERENCE.md) |
| What performance evidence exists? | [`benchmarks/M1_10K_MANIFEST_SCAN.md`](benchmarks/M1_10K_MANIFEST_SCAN.md) |
| What are the security and verification expectations? | [`planning/05_TEST_SECURITY_BENCHMARK.md`](planning/05_TEST_SECURITY_BENCHMARK.md) and [`../SECURITY.md`](../SECURITY.md) |
| Which architectural decisions are accepted? | [`planning/09_DECISIONS_ADR.md`](planning/09_DECISIONS_ADR.md) and [`architecture/adr/`](architecture/adr/) |

## Canonical status and evidence

### Implementation status

[`planning/IMPLEMENTATION_STATUS.md`](planning/IMPLEMENTATION_STATUS.md) is the canonical detailed status ledger. It records milestone-by-milestone implementation boundaries, verified evidence, open gates, and explicit non-claims.

Do not create a second competing status document. Shorter documents should link back to this ledger when they summarize implementation state.

### Benchmarks

Current checked-in benchmark evidence lives under [`benchmarks/`](benchmarks/). Results are development evidence unless a document explicitly states a release-grade environment and gate.

### Architecture decisions

Accepted decisions are recorded in [`planning/09_DECISIONS_ADR.md`](planning/09_DECISIONS_ADR.md) and focused ADRs under [`architecture/adr/`](architecture/adr/). These take precedence over older planning prose when they conflict.

## Product and planning

- [`planning/01_PRODUCT_DEFINITION.md`](planning/01_PRODUCT_DEFINITION.md) — positioning, users, JTBD, v0.1 scope, and success metrics.
- [`planning/02_ARCHITECTURE.md`](planning/02_ARCHITECTURE.md) — planned system architecture and boundaries.
- [`planning/03_PLAN_VARIANTS.md`](planning/03_PLAN_VARIANTS.md) — alternative implementation strategies considered.
- [`planning/04_MILESTONES.md`](planning/04_MILESTONES.md) — milestone definitions.
- [`planning/05_TEST_SECURITY_BENCHMARK.md`](planning/05_TEST_SECURITY_BENCHMARK.md) — test, security, and benchmark gates.
- [`planning/06_RELEASE_DISTRIBUTION.md`](planning/06_RELEASE_DISTRIBUTION.md) — release and distribution plan.
- [`planning/09_DECISIONS_ADR.md`](planning/09_DECISIONS_ADR.md) — accepted design decisions.
- [`planning/10_ISSUE_BACKLOG.md`](planning/10_ISSUE_BACKLOG.md) — backlog and follow-up work.
- [`planning/DECISIONS_NEEDED.md`](planning/DECISIONS_NEEDED.md) — unresolved decisions.
- [`planning/DEPENDENCY_AUDIT.md`](planning/DEPENDENCY_AUDIT.md) — dependency review and constraints.
- [`planning/EXTERNAL_ACTIONS_REQUIRED.md`](planning/EXTERNAL_ACTIONS_REQUIRED.md) — work that requires explicit external or owner action.
- [`planning/REPOSITORY_ASSESSMENT.md`](planning/REPOSITORY_ASSESSMENT.md) — repository assessment snapshot.

## Development and operation

[`DEVELOPMENT_REFERENCE.md`](DEVELOPMENT_REFERENCE.md) preserves the detailed command-level workflows that used to live in the root README: manifest scanning, durable jobs, extraction, OCR, lexical search, project/relation review, watch reconciliation, organization preview, desktop startup, and verification commands.

It is an operational reference, **not** the source of truth for current completion status. When a command reference and the implementation ledger disagree, use [`planning/IMPLEMENTATION_STATUS.md`](planning/IMPLEMENTATION_STATUS.md).

## Demo and submission evidence

- [`HACKATHON_SUBMISSION.md`](HACKATHON_SUBMISSION.md) — OpenAI Build Week submission/evidence package.
- [`BUILD_WEEK_DEMO_SCRIPT.md`](BUILD_WEEK_DEMO_SCRIPT.md) — recorded demo script and presentation flow.

These are dated submission artifacts, not current release claims.

## Source-of-truth precedence

For implementation and safety questions, use this order:

1. Applicable `AGENTS.md` rules.
2. Accepted ADRs.
3. [`planning/IMPLEMENTATION_STATUS.md`](planning/IMPLEMENTATION_STATUS.md) and current planning documents.
4. Current code and tests.
5. Historical demo/submission material.

The repository remains pre-release until the release gates documented in the status and planning material are satisfied.
