-- ADR-033 hard exclusions are authorization denials.  A policy change and its
-- logical privacy purge are committed atomically by the Rust database layer.
ALTER TABLE authorized_scopes ADD COLUMN policy_revision INTEGER NOT NULL DEFAULT 1
    CHECK (policy_revision >= 1);

ALTER TABLE scan_jobs ADD COLUMN policy_revision INTEGER NOT NULL DEFAULT 1
    CHECK (policy_revision >= 1);
ALTER TABLE extraction_jobs ADD COLUMN policy_revision INTEGER NOT NULL DEFAULT 1
    CHECK (policy_revision >= 1);
ALTER TABLE watch_events ADD COLUMN policy_revision INTEGER NOT NULL DEFAULT 1
    CHECK (policy_revision >= 1);
ALTER TABLE action_plans ADD COLUMN policy_revision INTEGER NOT NULL DEFAULT 1
    CHECK (policy_revision >= 1);
ALTER TABLE cleanup_action_plans ADD COLUMN policy_revision INTEGER NOT NULL DEFAULT 1
    CHECK (policy_revision >= 1);

-- Explicit UPDATEs document the mutable-job backfill contract for pre-0024
-- databases. Immutable action histories receive the same value from SQLite's
-- ADD COLUMN DEFAULT and must not be touched by their update-denial triggers.
UPDATE scan_jobs SET policy_revision = (
    SELECT policy_revision FROM authorized_scopes WHERE id = scan_jobs.scope_id
);
UPDATE extraction_jobs SET policy_revision = (
    SELECT policy_revision FROM authorized_scopes WHERE id = extraction_jobs.scope_id
);
UPDATE watch_events SET policy_revision = (
    SELECT policy_revision FROM authorized_scopes WHERE id = watch_events.scope_id
);

CREATE TABLE scope_exclusions (
    id INTEGER PRIMARY KEY,
    scope_id INTEGER NOT NULL REFERENCES authorized_scopes(id) ON DELETE CASCADE,
    kind TEXT NOT NULL CHECK (kind IN ('file', 'folder')),
    path_raw BLOB NOT NULL CHECK (length(path_raw) BETWEEN 1 AND 65536),
    path_key TEXT NOT NULL CHECK (length(path_key) BETWEEN 1 AND 65536),
    display_path TEXT NOT NULL CHECK (length(display_path) BETWEEN 1 AND 65536),
    identity_kind TEXT NOT NULL CHECK (
        length(identity_kind) BETWEEN 1 AND 128 AND identity_kind <> 'path_fallback'
    ),
    identity_key BLOB NOT NULL CHECK (length(identity_key) BETWEEN 1 AND 1024),
    policy_revision INTEGER NOT NULL CHECK (policy_revision >= 2),
    created_at_unix_ms INTEGER NOT NULL CHECK (created_at_unix_ms >= 0),
    UNIQUE(scope_id, path_key)
);

CREATE INDEX scope_exclusions_scope_path_idx
ON scope_exclusions(scope_id, path_key);

CREATE UNIQUE INDEX scope_exclusions_scope_identity_idx
ON scope_exclusions(scope_id, identity_kind, identity_key);

-- This is an unforgeable-at-the-public-API, transaction-local capability in
-- practice: Rust inserts a random nonce after BEGIN IMMEDIATE and consumes the
-- row before COMMIT.  A crash or rollback cannot leave it live.
CREATE TABLE privacy_purge_capabilities (
    nonce BLOB PRIMARY KEY CHECK (length(nonce) = 32),
    scope_id INTEGER NOT NULL UNIQUE REFERENCES authorized_scopes(id) ON DELETE CASCADE,
    from_revision INTEGER NOT NULL CHECK (from_revision >= 1),
    to_revision INTEGER NOT NULL CHECK (to_revision = from_revision + 1),
    created_at_unix_ms INTEGER NOT NULL CHECK (created_at_unix_ms >= 0)
) WITHOUT ROWID;

-- A capability can only authorize the immediate next revision from the scope's
-- current durable state. This prevents a forged capability from skipping
-- revisions before the transaction performs its compare-and-swap.
CREATE TRIGGER privacy_purge_capabilities_current_revision_insert
BEFORE INSERT ON privacy_purge_capabilities
WHEN NEW.from_revision <> (
    SELECT policy_revision FROM authorized_scopes WHERE id = NEW.scope_id
)
BEGIN SELECT RAISE(ABORT, 'privacy_purge_capability_revision_stale'); END;

-- A denial row can only be created as part of the exact current-scope privacy
-- transaction that advances to its revision. Direct inserts, cross-scope
-- capabilities, and inserts attempted before the scope revision advances all
-- fail closed.
CREATE TRIGGER scope_exclusions_privacy_capability_insert
BEFORE INSERT ON scope_exclusions
WHEN NOT EXISTS (
    SELECT 1 FROM privacy_purge_capabilities c
    WHERE c.scope_id = NEW.scope_id
      AND c.to_revision = NEW.policy_revision
      AND c.from_revision = NEW.policy_revision - 1
      AND NEW.policy_revision = (
          SELECT policy_revision FROM authorized_scopes WHERE id = NEW.scope_id
      )
)
BEGIN SELECT RAISE(ABORT, 'scope_exclusion_privacy_capability_required'); END;

-- Defaults exist only so SQLite can add the columns to old tables. They are
-- never an authorization fallback: every new or ordinary updated row must carry
-- the owning scope's exact current revision. Privacy purge is the sole bounded
-- exception because it must terminate rows from the previous revision.
CREATE TRIGGER scan_jobs_policy_revision_insert
BEFORE INSERT ON scan_jobs
WHEN NEW.policy_revision <> (SELECT policy_revision FROM authorized_scopes WHERE id = NEW.scope_id)
BEGIN SELECT RAISE(ABORT, 'scope_policy_revision_stale'); END;
CREATE TRIGGER scan_jobs_policy_revision_update
BEFORE UPDATE ON scan_jobs
WHEN NOT EXISTS (SELECT 1 FROM privacy_purge_capabilities WHERE scope_id = OLD.scope_id)
 AND NEW.policy_revision <> (SELECT policy_revision FROM authorized_scopes WHERE id = NEW.scope_id)
BEGIN SELECT RAISE(ABORT, 'scope_policy_revision_stale'); END;

CREATE TRIGGER extraction_jobs_policy_revision_insert
BEFORE INSERT ON extraction_jobs
WHEN NEW.policy_revision <> (SELECT policy_revision FROM authorized_scopes WHERE id = NEW.scope_id)
BEGIN SELECT RAISE(ABORT, 'scope_policy_revision_stale'); END;
CREATE TRIGGER extraction_jobs_policy_revision_update
BEFORE UPDATE ON extraction_jobs
WHEN NOT EXISTS (SELECT 1 FROM privacy_purge_capabilities WHERE scope_id = OLD.scope_id)
 AND NEW.policy_revision <> (SELECT policy_revision FROM authorized_scopes WHERE id = NEW.scope_id)
BEGIN SELECT RAISE(ABORT, 'scope_policy_revision_stale'); END;

CREATE TRIGGER watch_events_policy_revision_insert
BEFORE INSERT ON watch_events
WHEN NEW.policy_revision <> (SELECT policy_revision FROM authorized_scopes WHERE id = NEW.scope_id)
BEGIN SELECT RAISE(ABORT, 'scope_policy_revision_stale'); END;
CREATE TRIGGER watch_events_policy_revision_update
BEFORE UPDATE ON watch_events
WHEN NOT EXISTS (SELECT 1 FROM privacy_purge_capabilities WHERE scope_id = OLD.scope_id)
 AND NEW.policy_revision <> (SELECT policy_revision FROM authorized_scopes WHERE id = NEW.scope_id)
BEGIN SELECT RAISE(ABORT, 'scope_policy_revision_stale'); END;

CREATE TRIGGER action_plans_policy_revision_insert
BEFORE INSERT ON action_plans
WHEN NEW.policy_revision <> (SELECT policy_revision FROM authorized_scopes WHERE id = NEW.scope_id)
 OR NOT EXISTS (
     SELECT 1 FROM scope_access_grants g
     JOIN authorized_scopes s ON s.id = g.scope_id AND s.platform = g.platform
     WHERE s.id = NEW.scope_id AND g.state = 'active'
 )
BEGIN SELECT RAISE(ABORT, 'scope_policy_revision_or_grant_stale'); END;
CREATE TRIGGER cleanup_action_plans_policy_revision_insert
BEFORE INSERT ON cleanup_action_plans
WHEN NEW.policy_revision <> (SELECT policy_revision FROM authorized_scopes WHERE id = NEW.scope_id)
 OR NOT EXISTS (
     SELECT 1 FROM scope_access_grants g
     JOIN authorized_scopes s ON s.id = g.scope_id AND s.platform = g.platform
     WHERE s.id = NEW.scope_id AND g.state = 'active'
 )
BEGIN SELECT RAISE(ABORT, 'scope_policy_revision_or_grant_stale'); END;

-- Transient, path-free target sets.  They exist only inside the same purge
-- transaction and disappear when its capability is consumed.
CREATE TABLE privacy_purge_location_targets (
    nonce BLOB NOT NULL REFERENCES privacy_purge_capabilities(nonce) ON DELETE CASCADE,
    location_id INTEGER NOT NULL REFERENCES locations(id) ON DELETE CASCADE,
    node_id INTEGER NOT NULL REFERENCES nodes(id) ON DELETE CASCADE,
    direct_match INTEGER NOT NULL CHECK (direct_match IN (0, 1)),
    PRIMARY KEY(nonce, location_id)
) WITHOUT ROWID;

CREATE TABLE privacy_purge_node_targets (
    nonce BLOB NOT NULL REFERENCES privacy_purge_capabilities(nonce) ON DELETE CASCADE,
    node_id INTEGER NOT NULL REFERENCES nodes(id) ON DELETE CASCADE,
    PRIMARY KEY(nonce, node_id)
) WITHOUT ROWID;

CREATE TABLE privacy_purge_project_targets (
    nonce BLOB NOT NULL REFERENCES privacy_purge_capabilities(nonce) ON DELETE CASCADE,
    project_id INTEGER NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    PRIMARY KEY(nonce, project_id)
) WITHOUT ROWID;

CREATE TABLE privacy_purge_action_plan_targets (
    nonce BLOB NOT NULL REFERENCES privacy_purge_capabilities(nonce) ON DELETE CASCADE,
    plan_id INTEGER NOT NULL REFERENCES action_plans(id) ON DELETE CASCADE,
    PRIMARY KEY(nonce, plan_id)
) WITHOUT ROWID;

CREATE TABLE privacy_purge_relation_targets (
    nonce BLOB NOT NULL REFERENCES privacy_purge_capabilities(nonce) ON DELETE CASCADE,
    relation_id INTEGER NOT NULL REFERENCES file_relation_candidates(id) ON DELETE CASCADE,
    PRIMARY KEY(nonce, relation_id)
) WITHOUT ROWID;

CREATE TABLE privacy_purge_screenshot_group_targets (
    nonce BLOB NOT NULL REFERENCES privacy_purge_capabilities(nonce) ON DELETE CASCADE,
    group_id INTEGER NOT NULL REFERENCES screenshot_group_candidates(id) ON DELETE CASCADE,
    PRIMARY KEY(nonce, group_id)
) WITHOUT ROWID;

CREATE TABLE privacy_purge_cleanup_action_plan_targets (
    nonce BLOB NOT NULL REFERENCES privacy_purge_capabilities(nonce) ON DELETE CASCADE,
    plan_id INTEGER NOT NULL REFERENCES cleanup_action_plans(id) ON DELETE CASCADE,
    PRIMARY KEY(nonce, plan_id)
) WITHOUT ROWID;

CREATE TABLE privacy_purge_receipts (
    id INTEGER PRIMARY KEY,
    scope_id INTEGER NOT NULL REFERENCES authorized_scopes(id),
    from_revision INTEGER NOT NULL CHECK (from_revision >= 1),
    to_revision INTEGER NOT NULL CHECK (to_revision = from_revision + 1),
    exclusions_added INTEGER NOT NULL CHECK (exclusions_added > 0),
    affected_location_count INTEGER NOT NULL CHECK (affected_location_count >= 0),
    affected_node_count INTEGER NOT NULL CHECK (affected_node_count >= 0),
    purged_row_count INTEGER NOT NULL CHECK (purged_row_count >= 0),
    created_at_unix_ms INTEGER NOT NULL CHECK (created_at_unix_ms >= 0),
    UNIQUE(scope_id, to_revision)
);

CREATE TRIGGER privacy_purge_capabilities_immutable_update
BEFORE UPDATE ON privacy_purge_capabilities
BEGIN SELECT RAISE(ABORT, 'privacy_purge_capability_immutable'); END;

CREATE TRIGGER privacy_purge_capabilities_guarded_delete
BEFORE DELETE ON privacy_purge_capabilities
WHEN OLD.to_revision <> (
        SELECT policy_revision FROM authorized_scopes WHERE id = OLD.scope_id
     )
  OR NOT EXISTS (
        SELECT 1 FROM privacy_purge_receipts r
        WHERE r.scope_id = OLD.scope_id AND r.to_revision = OLD.to_revision
     )
BEGIN SELECT RAISE(ABORT, 'privacy_purge_capability_not_consumable'); END;

CREATE TRIGGER privacy_purge_receipts_immutable_update
BEFORE UPDATE ON privacy_purge_receipts BEGIN
    SELECT RAISE(ABORT, 'privacy_purge_receipts_immutable');
END;
CREATE TRIGGER privacy_purge_receipts_immutable_delete
BEFORE DELETE ON privacy_purge_receipts BEGIN
    SELECT RAISE(ABORT, 'privacy_purge_receipts_immutable');
END;

-- Only DELETE triggers are widened. UPDATE remains unconditionally immutable.
-- A live scope capability is insufficient by itself: every root/child delete
-- must also be present in the nonce-specific conservative target closure.
DROP TRIGGER action_plans_immutable_delete;
CREATE TRIGGER action_plans_immutable_delete BEFORE DELETE ON action_plans
WHEN NOT EXISTS (SELECT 1 FROM privacy_purge_capabilities c JOIN privacy_purge_action_plan_targets t ON t.nonce=c.nonce AND t.plan_id=OLD.id WHERE c.scope_id=OLD.scope_id)
BEGIN SELECT RAISE(ABORT, 'action_plans_immutable'); END;
DROP TRIGGER action_plan_events_immutable_delete;
CREATE TRIGGER action_plan_events_immutable_delete BEFORE DELETE ON action_plan_events
WHEN NOT EXISTS (SELECT 1 FROM privacy_purge_capabilities c JOIN privacy_purge_action_plan_targets t ON t.nonce=c.nonce AND t.plan_id=OLD.plan_id JOIN action_plans p ON p.id=OLD.plan_id WHERE c.scope_id=p.scope_id)
BEGIN SELECT RAISE(ABORT, 'action_plan_events_immutable'); END;
DROP TRIGGER action_journal_events_immutable_delete;
CREATE TRIGGER action_journal_events_immutable_delete BEFORE DELETE ON action_journal_events
WHEN NOT EXISTS (SELECT 1 FROM privacy_purge_capabilities c JOIN privacy_purge_action_plan_targets t ON t.nonce=c.nonce AND t.plan_id=OLD.plan_id JOIN action_plans p ON p.id=OLD.plan_id WHERE c.scope_id=p.scope_id)
BEGIN SELECT RAISE(ABORT, 'action_journal_events_immutable'); END;
DROP TRIGGER action_command_requests_immutable_delete;
CREATE TRIGGER action_command_requests_immutable_delete BEFORE DELETE ON action_command_requests
WHEN NOT EXISTS (SELECT 1 FROM privacy_purge_capabilities c JOIN privacy_purge_action_plan_targets t ON t.nonce=c.nonce AND t.plan_id=OLD.plan_id JOIN action_plans p ON p.id=OLD.plan_id WHERE c.scope_id=p.scope_id)
BEGIN SELECT RAISE(ABORT, 'action_command_requests_immutable'); END;
DROP TRIGGER action_execution_bindings_immutable_delete;
CREATE TRIGGER action_execution_bindings_immutable_delete BEFORE DELETE ON action_execution_bindings
WHEN NOT EXISTS (SELECT 1 FROM privacy_purge_capabilities c JOIN privacy_purge_action_plan_targets t ON t.nonce=c.nonce AND t.plan_id=OLD.plan_id JOIN action_plans p ON p.id=OLD.plan_id WHERE c.scope_id=p.scope_id)
BEGIN SELECT RAISE(ABORT, 'action_execution_bindings_immutable'); END;

DROP TRIGGER cleanup_action_plans_immutable_delete;
CREATE TRIGGER cleanup_action_plans_immutable_delete BEFORE DELETE ON cleanup_action_plans
WHEN NOT EXISTS (SELECT 1 FROM privacy_purge_capabilities c JOIN privacy_purge_cleanup_action_plan_targets t ON t.nonce=c.nonce AND t.plan_id=OLD.id WHERE c.scope_id=OLD.scope_id)
BEGIN SELECT RAISE(ABORT, 'cleanup_action_plans_immutable'); END;
DROP TRIGGER cleanup_action_journal_events_immutable_delete;
CREATE TRIGGER cleanup_action_journal_events_immutable_delete BEFORE DELETE ON cleanup_action_journal_events
WHEN NOT EXISTS (SELECT 1 FROM privacy_purge_capabilities c JOIN privacy_purge_cleanup_action_plan_targets t ON t.nonce=c.nonce AND t.plan_id=OLD.plan_id JOIN cleanup_action_plans p ON p.id=OLD.plan_id WHERE c.scope_id=p.scope_id)
BEGIN SELECT RAISE(ABORT, 'cleanup_action_journal_events_immutable'); END;

DROP TRIGGER projects_immutable_delete;
CREATE TRIGGER projects_immutable_delete BEFORE DELETE ON projects
WHEN NOT EXISTS (SELECT 1 FROM privacy_purge_capabilities c JOIN privacy_purge_project_targets t ON t.nonce=c.nonce AND t.project_id=OLD.id WHERE c.scope_id=OLD.scope_id)
BEGIN SELECT RAISE(ABORT, 'projects_immutable'); END;
DROP TRIGGER project_suggestions_immutable_delete;
CREATE TRIGGER project_suggestions_immutable_delete BEFORE DELETE ON project_suggestions
WHEN NOT EXISTS (SELECT 1 FROM privacy_purge_capabilities c JOIN privacy_purge_project_targets t ON t.nonce=c.nonce AND t.project_id=OLD.project_id JOIN projects p ON p.id=OLD.project_id WHERE c.scope_id=p.scope_id)
BEGIN SELECT RAISE(ABORT, 'project_suggestions_immutable'); END;
DROP TRIGGER project_suggestion_signals_immutable_delete;
CREATE TRIGGER project_suggestion_signals_immutable_delete BEFORE DELETE ON project_suggestion_signals
WHEN NOT EXISTS (SELECT 1 FROM privacy_purge_capabilities c JOIN project_suggestions s ON s.id=OLD.suggestion_id JOIN privacy_purge_project_targets t ON t.nonce=c.nonce AND t.project_id=s.project_id JOIN projects p ON p.id=s.project_id WHERE c.scope_id=p.scope_id)
BEGIN SELECT RAISE(ABORT, 'project_suggestion_signals_immutable'); END;
DROP TRIGGER project_feedback_events_immutable_delete;
CREATE TRIGGER project_feedback_events_immutable_delete BEFORE DELETE ON project_feedback_events
WHEN NOT EXISTS (SELECT 1 FROM privacy_purge_capabilities c JOIN privacy_purge_project_targets t ON t.nonce=c.nonce AND t.project_id=OLD.project_id JOIN projects p ON p.id=OLD.project_id WHERE c.scope_id=p.scope_id)
BEGIN SELECT RAISE(ABORT, 'project_feedback_events_immutable'); END;

DROP TRIGGER file_relation_candidates_immutable_delete;
CREATE TRIGGER file_relation_candidates_immutable_delete BEFORE DELETE ON file_relation_candidates
WHEN NOT EXISTS (SELECT 1 FROM privacy_purge_capabilities c JOIN privacy_purge_relation_targets t ON t.nonce=c.nonce AND t.relation_id=OLD.id WHERE c.scope_id=OLD.scope_id)
BEGIN SELECT RAISE(ABORT, 'file_relation_candidates_immutable'); END;
DROP TRIGGER file_relation_observations_immutable_delete;
CREATE TRIGGER file_relation_observations_immutable_delete BEFORE DELETE ON file_relation_observations
WHEN NOT EXISTS (SELECT 1 FROM privacy_purge_capabilities c JOIN privacy_purge_relation_targets t ON t.nonce=c.nonce AND t.relation_id=OLD.relation_id JOIN file_relation_candidates r ON r.id=OLD.relation_id WHERE c.scope_id=r.scope_id)
BEGIN SELECT RAISE(ABORT, 'file_relation_observations_immutable'); END;
DROP TRIGGER file_relation_feedback_events_immutable_delete;
CREATE TRIGGER file_relation_feedback_events_immutable_delete BEFORE DELETE ON file_relation_feedback_events
WHEN NOT EXISTS (SELECT 1 FROM privacy_purge_capabilities c JOIN privacy_purge_relation_targets t ON t.nonce=c.nonce AND t.relation_id=OLD.relation_id JOIN file_relation_candidates r ON r.id=OLD.relation_id WHERE c.scope_id=r.scope_id)
BEGIN SELECT RAISE(ABORT, 'file_relation_feedback_events_immutable'); END;
DROP TRIGGER file_version_observations_immutable_delete;
CREATE TRIGGER file_version_observations_immutable_delete BEFORE DELETE ON file_version_observations
WHEN NOT EXISTS (SELECT 1 FROM privacy_purge_capabilities c JOIN privacy_purge_relation_targets t ON t.nonce=c.nonce AND t.relation_id=OLD.relation_id JOIN file_relation_candidates r ON r.id=OLD.relation_id WHERE c.scope_id=r.scope_id)
BEGIN SELECT RAISE(ABORT, 'file_version_observations_immutable'); END;
DROP TRIGGER file_version_feedback_events_immutable_delete;
CREATE TRIGGER file_version_feedback_events_immutable_delete BEFORE DELETE ON file_version_feedback_events
WHEN NOT EXISTS (SELECT 1 FROM privacy_purge_capabilities c JOIN privacy_purge_relation_targets t ON t.nonce=c.nonce AND t.relation_id=OLD.relation_id JOIN file_relation_candidates r ON r.id=OLD.relation_id WHERE c.scope_id=r.scope_id)
BEGIN SELECT RAISE(ABORT, 'file_version_feedback_events_immutable'); END;

DROP TRIGGER screenshot_group_candidates_immutable_delete;
CREATE TRIGGER screenshot_group_candidates_immutable_delete BEFORE DELETE ON screenshot_group_candidates
WHEN NOT EXISTS (SELECT 1 FROM privacy_purge_capabilities c JOIN privacy_purge_screenshot_group_targets t ON t.nonce=c.nonce AND t.group_id=OLD.id WHERE c.scope_id=OLD.scope_id)
BEGIN SELECT RAISE(ABORT, 'screenshot_group_candidates_immutable'); END;
DROP TRIGGER screenshot_group_observations_immutable_delete;
CREATE TRIGGER screenshot_group_observations_immutable_delete BEFORE DELETE ON screenshot_group_observations
WHEN NOT EXISTS (SELECT 1 FROM privacy_purge_capabilities c JOIN privacy_purge_screenshot_group_targets t ON t.nonce=c.nonce AND t.group_id=OLD.group_id JOIN screenshot_group_candidates g ON g.id=OLD.group_id WHERE c.scope_id=g.scope_id)
BEGIN SELECT RAISE(ABORT, 'screenshot_group_observations_immutable'); END;
DROP TRIGGER screenshot_group_members_immutable_delete;
CREATE TRIGGER screenshot_group_members_immutable_delete BEFORE DELETE ON screenshot_group_members
WHEN NOT EXISTS (SELECT 1 FROM privacy_purge_capabilities c JOIN screenshot_group_observations o ON o.id=OLD.observation_id JOIN privacy_purge_screenshot_group_targets t ON t.nonce=c.nonce AND t.group_id=o.group_id JOIN screenshot_group_candidates g ON g.id=o.group_id WHERE c.scope_id=g.scope_id)
BEGIN SELECT RAISE(ABORT, 'screenshot_group_members_immutable'); END;
