## Outcome

Describe the user-visible or developer outcome.

## Safety boundary

State effects on authorized scopes, filesystem actions, local data, models, network access, MCP, and recovery. Write “none” where applicable.

## Acceptance evidence

- [ ] Behavior tests added or updated
- [ ] Rust format, clippy, and relevant tests pass
- [ ] TypeScript format, lint, typecheck, tests, and build pass
- [ ] User-visible behavior and known limitations are documented
- [ ] New dependencies are audited in `docs/planning/DEPENDENCY_AUDIT.md`
- [ ] No acceptance criterion is claimed without evidence

## Rollback and external validation

Describe safe rollback and any CI, signing, notarization, clean-machine, or account action still required.
