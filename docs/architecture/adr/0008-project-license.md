# ADR-008 — Apache-2.0 project license

- Status: Accepted
- Date: 2026-07-16

## Context

DeskGraph needs a clear permissive open-source license before accepting code contributions. The foundation prompt requires an explicit MIT or Apache-2.0 decision.

## Decision

License original DeskGraph project code under Apache License 2.0. Third-party dependencies, model artifacts, icons, fonts, and datasets retain their own audited licenses and notices.

## Consequences

Apache-2.0 permits commercial and private use, modification, and distribution and includes an explicit patent grant and patent-termination clause. Contributions intentionally submitted to the project are accepted under the same license unless explicitly agreed otherwise.

## Alternatives considered

MIT is simpler and compatible with the intended ecosystem, but does not contain the same explicit patent language. Dual licensing is unnecessary for M0 and can be revisited only with contributor-impact review.

## Validation and revisit trigger

The root `LICENSE`, README, package metadata, and release artifacts must consistently declare Apache-2.0. Revisit only for a documented legal or ecosystem requirement.
