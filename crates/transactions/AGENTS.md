# Transaction Engine Instructions

This is safety-critical code.

- No filesystem action without a durable transaction record.
- Use an explicit state machine.
- Make operations idempotent.
- Validate source identity immediately before execution.
- Verify destination after execution.
- Persist enough information for rollback and crash recovery.
- Test permission errors, conflicts, cross-volume moves, disconnects and process termination.
- Never add permanent deletion.
- Any change requires integration and fault-injection tests.
