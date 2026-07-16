CREATE TABLE action_plans (
    id INTEGER PRIMARY KEY,
    api_version TEXT NOT NULL CHECK (api_version = 'deskgraph.action-plan.v1'),
    policy_version TEXT NOT NULL CHECK (policy_version = 'deskgraph.action-policy.v1'),
    operation TEXT NOT NULL CHECK (operation = 'rename'),
    execution_strategy TEXT NOT NULL CHECK (execution_strategy IN ('direct', 'case_only_staged')),
    scope_id INTEGER NOT NULL REFERENCES authorized_scopes(id),
    node_id INTEGER NOT NULL REFERENCES nodes(id),
    source_location_id INTEGER NOT NULL REFERENCES locations(id),
    source_path_raw BLOB NOT NULL,
    source_path_key TEXT NOT NULL,
    source_display_path TEXT NOT NULL,
    destination_path_raw BLOB NOT NULL,
    destination_path_key TEXT NOT NULL,
    destination_display_path TEXT NOT NULL,
    source_identity_kind TEXT NOT NULL,
    source_identity_key BLOB NOT NULL,
    source_size_bytes INTEGER NOT NULL CHECK (source_size_bytes >= 0),
    source_modified_unix_ns INTEGER,
    created_at_unix_ms INTEGER NOT NULL
);

CREATE TABLE action_plan_events (
    id INTEGER PRIMARY KEY,
    plan_id INTEGER NOT NULL REFERENCES action_plans(id),
    sequence INTEGER NOT NULL CHECK (sequence > 0),
    event_kind TEXT NOT NULL CHECK (event_kind = 'preview_created'),
    created_at_unix_ms INTEGER NOT NULL,
    UNIQUE(plan_id, sequence)
);

CREATE INDEX action_plans_recent_idx ON action_plans(id DESC);
CREATE INDEX action_plan_events_plan_idx ON action_plan_events(plan_id, sequence DESC);

CREATE TRIGGER action_plans_immutable_update
BEFORE UPDATE ON action_plans
BEGIN
    SELECT RAISE(ABORT, 'action_plans_immutable');
END;

CREATE TRIGGER action_plans_immutable_delete
BEFORE DELETE ON action_plans
BEGIN
    SELECT RAISE(ABORT, 'action_plans_immutable');
END;

CREATE TRIGGER action_plan_events_immutable_update
BEFORE UPDATE ON action_plan_events
BEGIN
    SELECT RAISE(ABORT, 'action_plan_events_immutable');
END;

CREATE TRIGGER action_plan_events_immutable_delete
BEFORE DELETE ON action_plan_events
BEGIN
    SELECT RAISE(ABORT, 'action_plan_events_immutable');
END;
