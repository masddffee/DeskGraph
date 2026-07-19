-- Smart Cleanup uses an independent, preview-only plan family. The existing
-- rename action_plans/action_journal_events contract stays closed and cannot
-- be widened into a System Trash executor.
CREATE TABLE cleanup_action_plans (
    id INTEGER PRIMARY KEY,
    api_version TEXT NOT NULL CHECK (
        api_version = 'deskgraph.cleanup-action-plan.v1'
    ),
    policy_version TEXT NOT NULL CHECK (
        policy_version = 'deskgraph.cleanup-action-policy.v1'
    ),
    operation TEXT NOT NULL CHECK (operation = 'system_trash_preview'),
    state TEXT NOT NULL CHECK (state = 'previewed'),
    scope_id INTEGER NOT NULL REFERENCES authorized_scopes(id),
    source_kind TEXT NOT NULL CHECK (
        source_kind IN ('exact_duplicate', 'version', 'screenshot_review_group')
    ),
    source_id INTEGER NOT NULL CHECK (source_id > 0),
    source_observation_id INTEGER NOT NULL CHECK (source_observation_id > 0),
    keeper_node_id INTEGER REFERENCES nodes(id),
    keeper_location_id INTEGER REFERENCES locations(id),
    keeper_identity_kind TEXT,
    keeper_identity_key BLOB,
    keeper_size_bytes INTEGER,
    keeper_modified_unix_ns INTEGER,
    keeper_sha256 BLOB,
    keeper_hash_bytes INTEGER,
    keeper_scope_root_node_id INTEGER REFERENCES nodes(id),
    keeper_scope_root_identity_kind TEXT,
    keeper_scope_root_identity_key BLOB,
    keeper_parent_node_id INTEGER REFERENCES nodes(id),
    keeper_parent_identity_kind TEXT,
    keeper_parent_identity_key BLOB,
    target_node_id INTEGER NOT NULL REFERENCES nodes(id),
    target_location_id INTEGER NOT NULL REFERENCES locations(id),
    target_identity_kind TEXT NOT NULL CHECK (
        length(target_identity_kind) BETWEEN 1 AND 128
        AND target_identity_kind <> 'path_fallback'
    ),
    target_identity_key BLOB NOT NULL CHECK (
        length(target_identity_key) BETWEEN 1 AND 4096
    ),
    target_size_bytes INTEGER NOT NULL CHECK (target_size_bytes >= 0),
    target_modified_unix_ns INTEGER,
    target_sha256 BLOB NOT NULL CHECK (length(target_sha256) = 32),
    target_hash_bytes INTEGER NOT NULL CHECK (
        target_hash_bytes = target_size_bytes
    ),
    scope_root_node_id INTEGER NOT NULL REFERENCES nodes(id),
    scope_root_identity_kind TEXT NOT NULL CHECK (
        length(scope_root_identity_kind) BETWEEN 1 AND 128
        AND scope_root_identity_kind <> 'path_fallback'
    ),
    scope_root_identity_key BLOB NOT NULL CHECK (
        length(scope_root_identity_key) BETWEEN 1 AND 4096
    ),
    parent_node_id INTEGER NOT NULL REFERENCES nodes(id),
    parent_identity_kind TEXT NOT NULL CHECK (
        length(parent_identity_kind) BETWEEN 1 AND 128
        AND parent_identity_kind <> 'path_fallback'
    ),
    parent_identity_key BLOB NOT NULL CHECK (
        length(parent_identity_key) BETWEEN 1 AND 4096
    ),
    confirmation_required INTEGER NOT NULL CHECK (confirmation_required = 1),
    action_authorized INTEGER NOT NULL CHECK (action_authorized = 0),
    execution_available INTEGER NOT NULL CHECK (execution_available = 0),
    created_at_unix_ms INTEGER NOT NULL CHECK (created_at_unix_ms >= 0),
    CHECK (keeper_node_id IS NULL OR keeper_node_id <> target_node_id),
    CHECK (
        (
            source_kind = 'screenshot_review_group'
            AND
            keeper_node_id IS NULL
            AND keeper_location_id IS NULL
            AND keeper_identity_kind IS NULL
            AND keeper_identity_key IS NULL
            AND keeper_size_bytes IS NULL
            AND keeper_modified_unix_ns IS NULL
            AND keeper_sha256 IS NULL
            AND keeper_hash_bytes IS NULL
            AND keeper_scope_root_node_id IS NULL
            AND keeper_scope_root_identity_kind IS NULL
            AND keeper_scope_root_identity_key IS NULL
            AND keeper_parent_node_id IS NULL
            AND keeper_parent_identity_kind IS NULL
            AND keeper_parent_identity_key IS NULL
        )
        OR
        (
            keeper_node_id IS NOT NULL
            AND keeper_location_id IS NOT NULL
            AND keeper_identity_kind IS NOT NULL
            AND length(keeper_identity_kind) BETWEEN 1 AND 128
            AND keeper_identity_kind <> 'path_fallback'
            AND keeper_identity_key IS NOT NULL
            AND length(keeper_identity_key) BETWEEN 1 AND 4096
            AND keeper_size_bytes IS NOT NULL
            AND keeper_size_bytes >= 0
            AND keeper_sha256 IS NOT NULL
            AND length(keeper_sha256) = 32
            AND keeper_hash_bytes IS NOT NULL
            AND keeper_hash_bytes = keeper_size_bytes
            AND keeper_scope_root_node_id IS NOT NULL
            AND keeper_scope_root_identity_kind IS NOT NULL
            AND length(keeper_scope_root_identity_kind) BETWEEN 1 AND 128
            AND keeper_scope_root_identity_kind <> 'path_fallback'
            AND keeper_scope_root_identity_key IS NOT NULL
            AND length(keeper_scope_root_identity_key) BETWEEN 1 AND 4096
            AND keeper_parent_node_id IS NOT NULL
            AND keeper_parent_identity_kind IS NOT NULL
            AND length(keeper_parent_identity_kind) BETWEEN 1 AND 128
            AND keeper_parent_identity_kind <> 'path_fallback'
            AND keeper_parent_identity_key IS NOT NULL
            AND length(keeper_parent_identity_key) BETWEEN 1 AND 4096
        )
    ),
    CHECK (
        source_kind <> 'exact_duplicate'
        OR (
            keeper_sha256 IS NOT NULL
            AND keeper_sha256 = target_sha256
            AND keeper_hash_bytes IS NOT NULL
            AND keeper_hash_bytes = target_hash_bytes
        )
    )
);

CREATE TABLE cleanup_action_journal_events (
    id INTEGER PRIMARY KEY,
    api_version TEXT NOT NULL CHECK (
        api_version = 'deskgraph.cleanup-action-journal.v1'
    ),
    plan_id INTEGER NOT NULL REFERENCES cleanup_action_plans(id),
    sequence INTEGER NOT NULL CHECK (sequence = 1),
    event_kind TEXT NOT NULL CHECK (event_kind = 'preview_created'),
    created_at_unix_ms INTEGER NOT NULL CHECK (created_at_unix_ms >= 0),
    UNIQUE(plan_id, sequence)
);

CREATE INDEX cleanup_action_plans_recent_idx
ON cleanup_action_plans(id DESC);

CREATE INDEX cleanup_action_journal_events_plan_idx
ON cleanup_action_journal_events(plan_id, sequence DESC);

CREATE TRIGGER cleanup_action_plans_immutable_update
BEFORE UPDATE ON cleanup_action_plans
BEGIN
    SELECT RAISE(ABORT, 'cleanup_action_plans_immutable');
END;

CREATE TRIGGER cleanup_action_plans_immutable_delete
BEFORE DELETE ON cleanup_action_plans
BEGIN
    SELECT RAISE(ABORT, 'cleanup_action_plans_immutable');
END;

CREATE TRIGGER cleanup_action_journal_events_immutable_update
BEFORE UPDATE ON cleanup_action_journal_events
BEGIN
    SELECT RAISE(ABORT, 'cleanup_action_journal_events_immutable');
END;

CREATE TRIGGER cleanup_action_journal_events_immutable_delete
BEFORE DELETE ON cleanup_action_journal_events
BEGIN
    SELECT RAISE(ABORT, 'cleanup_action_journal_events_immutable');
END;
