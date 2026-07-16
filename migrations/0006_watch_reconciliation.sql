CREATE TABLE watch_events (
    id INTEGER PRIMARY KEY,
    scope_id INTEGER NOT NULL REFERENCES authorized_scopes(id),
    status TEXT NOT NULL CHECK (status IN ('stabilizing', 'reconciling', 'completed', 'ignored', 'failed')),
    path_raw BLOB NOT NULL,
    path_key TEXT NOT NULL,
    observed_kind TEXT NOT NULL CHECK (observed_kind IN ('missing', 'file', 'folder')),
    observed_size_bytes INTEGER,
    observed_modified_unix_ns INTEGER,
    observed_identity_key BLOB,
    observation_count INTEGER NOT NULL CHECK (observation_count > 0),
    stable_after_unix_ms INTEGER NOT NULL,
    scan_job_id INTEGER REFERENCES scan_jobs(id),
    reason TEXT CHECK (reason IN ('temporary_download', 'hidden_entry', 'unsupported_entry', 'source_unavailable', 'reconcile_failed')),
    created_at_unix_ms INTEGER NOT NULL,
    updated_at_unix_ms INTEGER NOT NULL,
    CHECK (
        (observed_kind = 'missing' AND observed_size_bytes IS NULL AND observed_modified_unix_ns IS NULL AND observed_identity_key IS NULL)
        OR (observed_kind = 'file' AND observed_size_bytes IS NOT NULL AND observed_identity_key IS NOT NULL)
        OR (observed_kind = 'folder' AND observed_size_bytes IS NULL AND observed_identity_key IS NOT NULL)
    ),
    CHECK ((status = 'reconciling' AND scan_job_id IS NOT NULL) OR status <> 'reconciling')
);

CREATE UNIQUE INDEX watch_events_stabilizing_scope_idx
    ON watch_events(scope_id)
    WHERE status = 'stabilizing';
CREATE INDEX watch_events_recent_idx ON watch_events(id DESC);
CREATE INDEX watch_events_scan_job_idx ON watch_events(scan_job_id) WHERE scan_job_id IS NOT NULL;
