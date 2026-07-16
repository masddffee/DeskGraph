CREATE TABLE extraction_jobs (
    id INTEGER PRIMARY KEY,
    scope_id INTEGER NOT NULL REFERENCES authorized_scopes(id),
    node_id INTEGER NOT NULL REFERENCES nodes(id),
    location_id INTEGER NOT NULL REFERENCES locations(id),
    status TEXT NOT NULL CHECK (
        status IN ('queued', 'running', 'completed', 'failed', 'cancelled', 'interrupted')
    ),
    cancel_requested INTEGER NOT NULL DEFAULT 0 CHECK (cancel_requested IN (0, 1)),
    provider_id TEXT,
    provider_version TEXT,
    error_code TEXT,
    source_size_bytes INTEGER NOT NULL,
    source_modified_unix_ns INTEGER,
    output_bytes INTEGER NOT NULL DEFAULT 0,
    chunk_count INTEGER NOT NULL DEFAULT 0,
    elapsed_ms INTEGER NOT NULL DEFAULT 0,
    runner_token TEXT,
    lease_expires_at_unix_ms INTEGER,
    created_at_unix_ms INTEGER NOT NULL,
    started_at_unix_ms INTEGER,
    finished_at_unix_ms INTEGER,
    updated_at_unix_ms INTEGER NOT NULL
);

CREATE UNIQUE INDEX extraction_jobs_active_node_idx
    ON extraction_jobs(scope_id, node_id)
    WHERE status IN ('queued', 'running', 'interrupted');
CREATE INDEX extraction_jobs_status_idx ON extraction_jobs(status, id);

CREATE TABLE content_chunks (
    id INTEGER PRIMARY KEY,
    scope_id INTEGER NOT NULL REFERENCES authorized_scopes(id),
    node_id INTEGER NOT NULL REFERENCES nodes(id),
    location_id INTEGER NOT NULL REFERENCES locations(id),
    extraction_job_id INTEGER NOT NULL REFERENCES extraction_jobs(id),
    ordinal INTEGER NOT NULL,
    text TEXT NOT NULL,
    source_byte_start INTEGER NOT NULL,
    source_byte_end INTEGER NOT NULL,
    source_size_bytes INTEGER NOT NULL,
    source_modified_unix_ns INTEGER,
    trust_class TEXT NOT NULL CHECK (trust_class = 'untrusted_extracted_text'),
    provider_id TEXT NOT NULL,
    provider_version TEXT NOT NULL,
    active INTEGER NOT NULL CHECK (active IN (0, 1)),
    created_at_unix_ms INTEGER NOT NULL,
    UNIQUE(extraction_job_id, ordinal),
    CHECK (source_byte_start <= source_byte_end)
);

CREATE INDEX content_chunks_active_node_idx
    ON content_chunks(scope_id, active, node_id, ordinal);
