DROP TRIGGER file_relation_feedback_events_immutable_delete;
DROP TRIGGER file_relation_feedback_events_immutable_update;
DROP TRIGGER file_relation_observations_immutable_delete;
DROP TRIGGER file_relation_observations_immutable_update;
DROP TRIGGER file_relation_candidates_immutable_delete;
DROP TRIGGER file_relation_candidates_immutable_update;

CREATE TABLE file_relation_candidates_next (
    id INTEGER PRIMARY KEY,
    api_version TEXT NOT NULL CHECK (api_version = 'deskgraph.file-relation-candidate.v1'),
    relation_kind TEXT NOT NULL CHECK (relation_kind IN ('exact_duplicate', 'version')),
    scope_id INTEGER NOT NULL REFERENCES authorized_scopes(id),
    left_node_id INTEGER NOT NULL REFERENCES nodes(id),
    right_node_id INTEGER NOT NULL REFERENCES nodes(id),
    created_at_unix_ms INTEGER NOT NULL CHECK (created_at_unix_ms >= 0),
    CHECK (left_node_id < right_node_id),
    UNIQUE(relation_kind, scope_id, left_node_id, right_node_id)
);

INSERT INTO file_relation_candidates_next
SELECT * FROM file_relation_candidates;

CREATE TABLE file_relation_observations_next (
    id INTEGER PRIMARY KEY,
    relation_id INTEGER NOT NULL REFERENCES file_relation_candidates_next(id),
    left_location_id INTEGER NOT NULL REFERENCES locations(id),
    right_location_id INTEGER NOT NULL REFERENCES locations(id),
    source_size_bytes INTEGER NOT NULL CHECK (
        source_size_bytes > 0 AND source_size_bytes <= 67108864
    ),
    left_modified_unix_ns INTEGER,
    right_modified_unix_ns INTEGER,
    compared_bytes INTEGER NOT NULL CHECK (compared_bytes = source_size_bytes),
    confidence_basis_points INTEGER NOT NULL CHECK (confidence_basis_points = 10000),
    comparison_kind TEXT NOT NULL CHECK (comparison_kind = 'byte_for_byte'),
    created_by TEXT NOT NULL CHECK (created_by = 'system_rule'),
    provider_id TEXT NOT NULL CHECK (provider_id = 'deskgraph.byte-equality'),
    provider_version TEXT NOT NULL CHECK (provider_version = '1'),
    model_version TEXT CHECK (model_version IS NULL),
    observed_at_unix_ms INTEGER NOT NULL CHECK (observed_at_unix_ms >= 0),
    CHECK (left_location_id <> right_location_id)
);

INSERT INTO file_relation_observations_next
SELECT * FROM file_relation_observations;

CREATE TABLE file_relation_feedback_events_next (
    id INTEGER PRIMARY KEY,
    relation_id INTEGER NOT NULL REFERENCES file_relation_candidates_next(id),
    sequence INTEGER NOT NULL CHECK (sequence > 0),
    decision TEXT NOT NULL CHECK (decision IN ('accepted', 'rejected')),
    created_by TEXT NOT NULL CHECK (created_by = 'user'),
    created_at_unix_ms INTEGER NOT NULL CHECK (created_at_unix_ms >= 0),
    UNIQUE(relation_id, sequence)
);

INSERT INTO file_relation_feedback_events_next
SELECT * FROM file_relation_feedback_events;

DROP TABLE file_relation_feedback_events;
DROP TABLE file_relation_observations;
DROP TABLE file_relation_candidates;

ALTER TABLE file_relation_candidates_next RENAME TO file_relation_candidates;
ALTER TABLE file_relation_observations_next RENAME TO file_relation_observations;
ALTER TABLE file_relation_feedback_events_next RENAME TO file_relation_feedback_events;

CREATE TABLE file_version_observations (
    id INTEGER PRIMARY KEY,
    relation_id INTEGER NOT NULL REFERENCES file_relation_candidates(id),
    older_location_id INTEGER NOT NULL REFERENCES locations(id),
    newer_location_id INTEGER NOT NULL REFERENCES locations(id),
    older_size_bytes INTEGER NOT NULL CHECK (older_size_bytes >= 0),
    newer_size_bytes INTEGER NOT NULL CHECK (newer_size_bytes >= 0),
    older_modified_unix_ns INTEGER,
    newer_modified_unix_ns INTEGER,
    base_key TEXT NOT NULL CHECK (length(base_key) BETWEEN 1 AND 1024),
    extension_key TEXT NOT NULL CHECK (length(extension_key) <= 64),
    older_version INTEGER NOT NULL CHECK (older_version BETWEEN 1 AND 999999),
    newer_version INTEGER NOT NULL CHECK (newer_version BETWEEN 1 AND 999999),
    confidence_basis_points INTEGER NOT NULL CHECK (confidence_basis_points = 9000),
    signal_kind TEXT NOT NULL CHECK (signal_kind = 'explicit_numeric_suffix'),
    created_by TEXT NOT NULL CHECK (created_by = 'system_rule'),
    provider_id TEXT NOT NULL CHECK (provider_id = 'deskgraph.filename-version'),
    provider_version TEXT NOT NULL CHECK (provider_version = '1'),
    model_version TEXT CHECK (model_version IS NULL),
    observed_at_unix_ms INTEGER NOT NULL CHECK (observed_at_unix_ms >= 0),
    CHECK (older_location_id <> newer_location_id),
    CHECK (older_version < newer_version)
);

CREATE INDEX file_relation_candidates_recent_idx
ON file_relation_candidates(id DESC);

CREATE INDEX file_relation_observations_relation_idx
ON file_relation_observations(relation_id, observed_at_unix_ms DESC, id DESC);

CREATE INDEX file_relation_feedback_events_relation_idx
ON file_relation_feedback_events(relation_id, sequence DESC);

CREATE INDEX file_version_observations_relation_idx
ON file_version_observations(relation_id, observed_at_unix_ms DESC, id DESC);

CREATE TRIGGER file_relation_candidates_immutable_update
BEFORE UPDATE ON file_relation_candidates
BEGIN
    SELECT RAISE(ABORT, 'file_relation_candidates_immutable');
END;

CREATE TRIGGER file_relation_candidates_immutable_delete
BEFORE DELETE ON file_relation_candidates
BEGIN
    SELECT RAISE(ABORT, 'file_relation_candidates_immutable');
END;

CREATE TRIGGER file_relation_observations_immutable_update
BEFORE UPDATE ON file_relation_observations
BEGIN
    SELECT RAISE(ABORT, 'file_relation_observations_immutable');
END;

CREATE TRIGGER file_relation_observations_immutable_delete
BEFORE DELETE ON file_relation_observations
BEGIN
    SELECT RAISE(ABORT, 'file_relation_observations_immutable');
END;

CREATE TRIGGER file_relation_feedback_events_immutable_update
BEFORE UPDATE ON file_relation_feedback_events
BEGIN
    SELECT RAISE(ABORT, 'file_relation_feedback_events_immutable');
END;

CREATE TRIGGER file_relation_feedback_events_immutable_delete
BEFORE DELETE ON file_relation_feedback_events
BEGIN
    SELECT RAISE(ABORT, 'file_relation_feedback_events_immutable');
END;

CREATE TRIGGER file_version_observations_immutable_update
BEFORE UPDATE ON file_version_observations
BEGIN
    SELECT RAISE(ABORT, 'file_version_observations_immutable');
END;

CREATE TRIGGER file_version_observations_immutable_delete
BEFORE DELETE ON file_version_observations
BEGIN
    SELECT RAISE(ABORT, 'file_version_observations_immutable');
END;
