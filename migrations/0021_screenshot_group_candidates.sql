CREATE TABLE screenshot_group_candidates (
    id INTEGER PRIMARY KEY,
    api_version TEXT NOT NULL CHECK (
        api_version = 'deskgraph.screenshot-group-candidate.v1'
    ),
    scope_id INTEGER NOT NULL REFERENCES authorized_scopes(id),
    membership_key TEXT NOT NULL CHECK (length(membership_key) BETWEEN 3 AND 511),
    created_at_unix_ms INTEGER NOT NULL CHECK (created_at_unix_ms >= 0),
    UNIQUE(scope_id, membership_key)
);

CREATE TABLE screenshot_group_observations (
    id INTEGER PRIMARY KEY,
    group_id INTEGER NOT NULL REFERENCES screenshot_group_candidates(id),
    evidence_key TEXT NOT NULL CHECK (length(evidence_key) BETWEEN 16 AND 16384),
    member_count INTEGER NOT NULL CHECK (member_count BETWEEN 2 AND 20),
    confidence_basis_points INTEGER NOT NULL CHECK (confidence_basis_points = 6000),
    rule_kind TEXT NOT NULL CHECK (rule_kind = 'same_dimensions_time_window_with_ocr'),
    created_by TEXT NOT NULL CHECK (created_by = 'system_rule'),
    provider_id TEXT NOT NULL CHECK (provider_id = 'deskgraph.screenshot-group-rules'),
    provider_version TEXT NOT NULL CHECK (provider_version = '1'),
    model_version TEXT CHECK (model_version IS NULL),
    observed_at_unix_ms INTEGER NOT NULL CHECK (observed_at_unix_ms >= 0),
    UNIQUE(group_id, evidence_key)
);

CREATE TABLE screenshot_group_members (
    observation_id INTEGER NOT NULL REFERENCES screenshot_group_observations(id),
    ordinal INTEGER NOT NULL CHECK (ordinal BETWEEN 1 AND 20),
    node_id INTEGER NOT NULL REFERENCES nodes(id),
    location_id INTEGER NOT NULL REFERENCES locations(id),
    image_metadata_id INTEGER NOT NULL REFERENCES image_metadata(id),
    ocr_extraction_job_id INTEGER NOT NULL REFERENCES extraction_jobs(id),
    source_size_bytes INTEGER NOT NULL CHECK (source_size_bytes BETWEEN 1 AND 67108864),
    source_modified_unix_ns INTEGER NOT NULL,
    format TEXT NOT NULL CHECK (format IN ('png', 'jpeg', 'webp')),
    pixel_width INTEGER NOT NULL CHECK (pixel_width BETWEEN 1 AND 100000),
    pixel_height INTEGER NOT NULL CHECK (pixel_height BETWEEN 1 AND 100000),
    ocr_chunk_count INTEGER NOT NULL CHECK (ocr_chunk_count BETWEEN 1 AND 65536),
    ocr_provider_id TEXT NOT NULL CHECK (length(ocr_provider_id) BETWEEN 1 AND 128),
    ocr_provider_version TEXT NOT NULL CHECK (length(ocr_provider_version) BETWEEN 1 AND 128),
    PRIMARY KEY(observation_id, ordinal),
    UNIQUE(observation_id, node_id),
    CHECK (pixel_width * pixel_height <= 500000000)
);

CREATE INDEX screenshot_group_candidates_recent_idx
ON screenshot_group_candidates(id DESC);

CREATE INDEX screenshot_group_observations_group_idx
ON screenshot_group_observations(group_id, observed_at_unix_ms DESC, id DESC);

CREATE INDEX screenshot_group_members_node_idx
ON screenshot_group_members(node_id, observation_id);

CREATE TRIGGER screenshot_group_candidates_immutable_update
BEFORE UPDATE ON screenshot_group_candidates
BEGIN
    SELECT RAISE(ABORT, 'screenshot_group_candidates_immutable');
END;

CREATE TRIGGER screenshot_group_candidates_immutable_delete
BEFORE DELETE ON screenshot_group_candidates
BEGIN
    SELECT RAISE(ABORT, 'screenshot_group_candidates_immutable');
END;

CREATE TRIGGER screenshot_group_observations_immutable_update
BEFORE UPDATE ON screenshot_group_observations
BEGIN
    SELECT RAISE(ABORT, 'screenshot_group_observations_immutable');
END;

CREATE TRIGGER screenshot_group_observations_immutable_delete
BEFORE DELETE ON screenshot_group_observations
BEGIN
    SELECT RAISE(ABORT, 'screenshot_group_observations_immutable');
END;

CREATE TRIGGER screenshot_group_members_immutable_update
BEFORE UPDATE ON screenshot_group_members
BEGIN
    SELECT RAISE(ABORT, 'screenshot_group_members_immutable');
END;

CREATE TRIGGER screenshot_group_members_immutable_delete
BEFORE DELETE ON screenshot_group_members
BEGIN
    SELECT RAISE(ABORT, 'screenshot_group_members_immutable');
END;
