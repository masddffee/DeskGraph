CREATE TABLE file_relation_candidates (
    id INTEGER PRIMARY KEY,
    api_version TEXT NOT NULL CHECK (api_version = 'deskgraph.file-relation-candidate.v1'),
    relation_kind TEXT NOT NULL CHECK (relation_kind = 'exact_duplicate'),
    scope_id INTEGER NOT NULL REFERENCES authorized_scopes(id),
    left_node_id INTEGER NOT NULL REFERENCES nodes(id),
    right_node_id INTEGER NOT NULL REFERENCES nodes(id),
    created_at_unix_ms INTEGER NOT NULL CHECK (created_at_unix_ms >= 0),
    CHECK (left_node_id < right_node_id),
    UNIQUE(relation_kind, scope_id, left_node_id, right_node_id)
);

CREATE TABLE file_relation_observations (
    id INTEGER PRIMARY KEY,
    relation_id INTEGER NOT NULL REFERENCES file_relation_candidates(id),
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

CREATE INDEX file_relation_candidates_recent_idx
ON file_relation_candidates(id DESC);

CREATE INDEX file_relation_observations_relation_idx
ON file_relation_observations(relation_id, observed_at_unix_ms DESC, id DESC);

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
