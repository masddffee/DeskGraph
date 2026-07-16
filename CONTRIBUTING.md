# Contributing to DeskGraph

DeskGraph is pre-release. Contributions must preserve the safety invariants in `AGENTS.md` and accepted ADRs.

## Before changing code

1. Read the applicable `AGENTS.md`, current ADRs, and `docs/planning/IMPLEMENTATION_STATUS.md`.
2. Confirm the change belongs to the active milestone.
3. Audit any new dependency before adding it.
4. Plan the smallest coherent vertical slice with tests and user-visible documentation.

## Required checks

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
pnpm format:check
pnpm lint
pnpm typecheck
pnpm test
pnpm build
```

## Safety-sensitive changes

File actions, scope policies, identity, extraction, MCP, model downloads, and updater code require adversarial and integration tests. Transaction changes require fault-injection tests. Never weaken an invariant to make a test pass.

## Pull requests

- Explain the user outcome and security boundary.
- Link acceptance criteria and tests.
- State performance or memory impact when relevant.
- Record known limitations and external validation still required.
- Keep generated, local, and secret files out of commits.

By participating, you agree to follow `CODE_OF_CONDUCT.md`.
