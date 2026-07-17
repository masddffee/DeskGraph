CREATE TABLE image_metadata (
    id INTEGER PRIMARY KEY,
    scope_id INTEGER NOT NULL REFERENCES authorized_scopes(id),
    node_id INTEGER NOT NULL REFERENCES nodes(id),
    location_id INTEGER NOT NULL REFERENCES locations(id),
    extraction_job_id INTEGER NOT NULL UNIQUE REFERENCES extraction_jobs(id),
    format TEXT NOT NULL CHECK (format IN ('png', 'jpeg', 'gif', 'webp', 'bmp', 'tiff')),
    pixel_width INTEGER NOT NULL CHECK (pixel_width BETWEEN 1 AND 100000),
    pixel_height INTEGER NOT NULL CHECK (pixel_height BETWEEN 1 AND 100000),
    source_size_bytes INTEGER NOT NULL CHECK (source_size_bytes BETWEEN 1 AND 67108864),
    source_modified_unix_ns INTEGER,
    provider_id TEXT NOT NULL,
    provider_version TEXT NOT NULL,
    active INTEGER NOT NULL CHECK (active IN (0, 1)),
    created_at_unix_ms INTEGER NOT NULL,
    CHECK (pixel_width * pixel_height <= 500000000)
);

CREATE INDEX image_metadata_active_node_idx
    ON image_metadata(scope_id, active, node_id);

CREATE UNIQUE INDEX image_metadata_one_active_node_idx
    ON image_metadata(scope_id, node_id)
    WHERE active = 1;
