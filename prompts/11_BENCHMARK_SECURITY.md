# Phase 11 — Benchmark and Security Release Gate

Implement and run the complete plan in `05_TEST_SECURITY_BENCHMARK.md`.

Deliver:
- benchmark harness
- Synthetic-10K and Synthetic-100K
- multilingual retrieval evaluation
- transaction fault injection
- threat model
- privacy architecture
- SBOM
- dependency and license report
- known limitations

Do not tune the report to hide failures. Publish measured hardware, OS, app version and model configuration.

Fix all critical and high issues before release.

Acceptance:
- no known data-loss bug
- clean install smoke tests
- undo suite passes
- 8GB report exists
- benchmark results embedded in README
