CREATE TABLE action_command_requests (
    id INTEGER PRIMARY KEY,
    plan_id INTEGER NOT NULL REFERENCES action_plans(id),
    request_id TEXT NOT NULL CHECK (length(request_id) BETWEEN 8 AND 128),
    command_kind TEXT NOT NULL CHECK (command_kind IN ('execute', 'undo')),
    requested_sequence INTEGER NOT NULL CHECK (requested_sequence > 1),
    created_at_unix_ms INTEGER NOT NULL CHECK (created_at_unix_ms >= 0),
    UNIQUE(plan_id, request_id),
    UNIQUE(plan_id, requested_sequence)
);

-- Keep 0007 immutable. It is a deployed checksum contract and its narrow
-- `preview_created` vocabulary is intentionally insufficient for execution.
-- This table becomes the canonical runtime journal after the one-time copy.
CREATE TABLE action_journal_events (
    id INTEGER PRIMARY KEY,
    api_version TEXT NOT NULL CHECK (api_version = 'deskgraph.action-journal.v1'),
    plan_id INTEGER NOT NULL REFERENCES action_plans(id),
    sequence INTEGER NOT NULL CHECK (sequence > 0),
    event_kind TEXT NOT NULL CHECK (event_kind IN (
        'preview_created',
        'execute_requested',
        'execute_request_not_started',
        'direct_rename_intent',
        'execution_completed',
        'execution_not_applied',
        'execution_needs_attention',
        'undo_requested',
        'undo_request_not_started',
        'undo_rename_intent',
        'undo_completed',
        'undo_not_applied',
        'undo_needs_attention'
    )),
    command_request_id INTEGER REFERENCES action_command_requests(id),
    created_at_unix_ms INTEGER NOT NULL CHECK (created_at_unix_ms >= 0),
    UNIQUE(plan_id, sequence),
    CHECK (
        (sequence = 1 AND event_kind = 'preview_created' AND command_request_id IS NULL)
        OR
        (sequence > 1 AND event_kind <> 'preview_created' AND command_request_id IS NOT NULL)
    )
);

-- A new preview gets this immutable binding in the same transaction as its
-- plan and canonical sequence-one event. Existing previews deliberately do
-- not get a guessed binding and therefore remain non-executable.
CREATE TABLE action_execution_bindings (
    plan_id INTEGER PRIMARY KEY REFERENCES action_plans(id),
    api_version TEXT NOT NULL CHECK (api_version = 'deskgraph.action-execution-binding.v1'),
    source_hash_bytes INTEGER NOT NULL CHECK (source_hash_bytes >= 0),
    source_sha256 BLOB NOT NULL CHECK (length(source_sha256) = 32),
    scope_root_node_id INTEGER NOT NULL REFERENCES nodes(id),
    scope_root_identity_kind TEXT NOT NULL,
    scope_root_identity_key BLOB NOT NULL,
    parent_node_id INTEGER NOT NULL REFERENCES nodes(id),
    parent_identity_kind TEXT NOT NULL,
    parent_identity_key BLOB NOT NULL,
    created_at_unix_ms INTEGER NOT NULL CHECK (created_at_unix_ms >= 0),
    CHECK (length(scope_root_identity_kind) BETWEEN 1 AND 128),
    CHECK (length(scope_root_identity_key) BETWEEN 1 AND 4096),
    CHECK (length(parent_identity_kind) BETWEEN 1 AND 128),
    CHECK (length(parent_identity_key) BETWEEN 1 AND 4096)
);

-- This is deliberately operational state rather than audit history. A lease
-- coordinates an active protocol owner without keeping an SQLite transaction
-- open across a future filesystem syscall. Expiry permits observation of work
-- that may have been abandoned, but it is not an OS-lifetime process fence: a
-- stopped live process could outlast it. Production execution remains gated
-- until the separate process-fence and exact-source requirements are accepted.
CREATE TABLE action_executor_leases (
    plan_id INTEGER PRIMARY KEY REFERENCES action_plans(id),
    owner_token TEXT NOT NULL CHECK (length(owner_token) BETWEEN 16 AND 128),
    expires_at_unix_ms INTEGER NOT NULL CHECK (expires_at_unix_ms >= 0),
    heartbeat_at_unix_ms INTEGER NOT NULL CHECK (heartbeat_at_unix_ms >= 0)
);

-- The legacy journal is preserved as audit history but is not a runtime
-- append target after this migration.
INSERT INTO action_journal_events(
    api_version, plan_id, sequence, event_kind, command_request_id, created_at_unix_ms
)
SELECT
    'deskgraph.action-journal.v1', plan_id, sequence, event_kind, NULL,
    created_at_unix_ms
FROM action_plan_events;

CREATE INDEX action_journal_events_plan_sequence_idx
    ON action_journal_events(plan_id, sequence DESC);
CREATE INDEX action_journal_events_recovery_idx
    ON action_journal_events(event_kind, plan_id, sequence DESC);
CREATE INDEX action_command_requests_plan_idx
    ON action_command_requests(plan_id, id DESC);
CREATE INDEX action_executor_leases_expiry_idx
    ON action_executor_leases(expires_at_unix_ms, plan_id);

CREATE TRIGGER action_plan_events_sealed_insert
BEFORE INSERT ON action_plan_events
BEGIN
    SELECT RAISE(ABORT, 'action_plan_events_sealed');
END;

CREATE TRIGGER action_journal_events_immutable_update
BEFORE UPDATE ON action_journal_events
BEGIN
    SELECT RAISE(ABORT, 'action_journal_events_immutable');
END;

CREATE TRIGGER action_journal_events_immutable_delete
BEFORE DELETE ON action_journal_events
BEGIN
    SELECT RAISE(ABORT, 'action_journal_events_immutable');
END;

CREATE TRIGGER action_command_requests_immutable_update
BEFORE UPDATE ON action_command_requests
BEGIN
    SELECT RAISE(ABORT, 'action_command_requests_immutable');
END;

CREATE TRIGGER action_command_requests_immutable_delete
BEFORE DELETE ON action_command_requests
BEGIN
    SELECT RAISE(ABORT, 'action_command_requests_immutable');
END;

CREATE TRIGGER action_execution_bindings_immutable_update
BEFORE UPDATE ON action_execution_bindings
BEGIN
    SELECT RAISE(ABORT, 'action_execution_bindings_immutable');
END;

CREATE TRIGGER action_execution_bindings_immutable_delete
BEFORE DELETE ON action_execution_bindings
BEGIN
    SELECT RAISE(ABORT, 'action_execution_bindings_immutable');
END;

CREATE TRIGGER action_journal_events_sequence_monotonic
BEFORE INSERT ON action_journal_events
WHEN NEW.sequence <> COALESCE(
    (SELECT MAX(sequence) + 1 FROM action_journal_events WHERE plan_id = NEW.plan_id),
    1
)
BEGIN
    SELECT RAISE(ABORT, 'action_journal_sequence_not_next');
END;

CREATE TRIGGER action_journal_events_command_plan_matches
BEFORE INSERT ON action_journal_events
WHEN NEW.command_request_id IS NOT NULL
 AND NOT EXISTS (
    SELECT 1 FROM action_command_requests request
    WHERE request.id = NEW.command_request_id AND request.plan_id = NEW.plan_id
 )
BEGIN
    SELECT RAISE(ABORT, 'action_journal_command_plan_mismatch');
END;

CREATE TRIGGER action_journal_events_command_kind_and_sequence
BEFORE INSERT ON action_journal_events
WHEN NEW.sequence > 1
 AND NOT EXISTS (
    SELECT 1 FROM action_command_requests request
    WHERE request.id = NEW.command_request_id
      AND request.plan_id = NEW.plan_id
      AND (
        (NEW.event_kind = 'execute_requested'
         AND request.command_kind = 'execute'
         AND request.requested_sequence = NEW.sequence)
        OR
        (NEW.event_kind IN (
            'execute_request_not_started', 'direct_rename_intent',
            'execution_completed', 'execution_not_applied', 'execution_needs_attention'
         ) AND request.command_kind = 'execute')
        OR
        (NEW.event_kind = 'undo_requested'
         AND request.command_kind = 'undo'
         AND request.requested_sequence = NEW.sequence)
        OR
        (NEW.event_kind IN (
            'undo_request_not_started', 'undo_rename_intent',
            'undo_completed', 'undo_not_applied', 'undo_needs_attention'
         ) AND request.command_kind = 'undo')
      )
 )
BEGIN
    SELECT RAISE(ABORT, 'action_journal_command_kind_or_sequence_mismatch');
END;

CREATE TRIGGER action_journal_events_transition_valid
BEFORE INSERT ON action_journal_events
WHEN NEW.sequence > 1
 AND NOT EXISTS (
    SELECT 1 FROM action_journal_events previous
    WHERE previous.plan_id = NEW.plan_id
      AND previous.sequence = NEW.sequence - 1
      AND (
        (previous.event_kind IN (
            'preview_created', 'execute_request_not_started', 'execution_not_applied'
         )
         AND NEW.event_kind = 'execute_requested')
        OR
        (previous.event_kind = 'execute_requested'
         AND NEW.event_kind IN ('execute_request_not_started', 'direct_rename_intent'))
        OR
        (previous.event_kind = 'direct_rename_intent'
         AND NEW.event_kind IN (
            'execution_completed', 'execution_not_applied', 'execution_needs_attention'
         ))
        OR
        (previous.event_kind IN (
            'execution_completed', 'undo_request_not_started', 'undo_not_applied'
         )
         AND NEW.event_kind = 'undo_requested')
        OR
        (previous.event_kind = 'undo_requested'
         AND NEW.event_kind IN ('undo_request_not_started', 'undo_rename_intent'))
        OR
        (previous.event_kind = 'undo_rename_intent'
         AND NEW.event_kind IN ('undo_completed', 'undo_not_applied', 'undo_needs_attention'))
      )
      AND (
        NEW.event_kind IN ('execute_requested', 'undo_requested')
        OR previous.command_request_id = NEW.command_request_id
      )
 )
BEGIN
    SELECT RAISE(ABORT, 'action_journal_invalid_transition');
END;
