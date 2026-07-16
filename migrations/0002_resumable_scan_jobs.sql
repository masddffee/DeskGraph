ALTER TABLE scan_jobs ADD COLUMN control_state TEXT NOT NULL DEFAULT 'ready'
    CHECK (control_state IN ('ready', 'active', 'pause_requested', 'paused'));
ALTER TABLE scan_jobs ADD COLUMN queued_entries INTEGER NOT NULL DEFAULT 0;
ALTER TABLE scan_jobs ADD COLUMN processed_entries INTEGER NOT NULL DEFAULT 0;
ALTER TABLE scan_jobs ADD COLUMN elapsed_ms INTEGER NOT NULL DEFAULT 0;
ALTER TABLE scan_jobs ADD COLUMN updated_at_unix_ms INTEGER NOT NULL DEFAULT 0;
ALTER TABLE scan_jobs ADD COLUMN pause_requested INTEGER NOT NULL DEFAULT 0
    CHECK (pause_requested IN (0, 1));
ALTER TABLE scan_jobs ADD COLUMN runner_token TEXT;
ALTER TABLE scan_jobs ADD COLUMN lease_expires_at_unix_ms INTEGER;

CREATE TABLE scan_queue (
    id INTEGER PRIMARY KEY,
    scan_id INTEGER NOT NULL REFERENCES scan_jobs(id) ON DELETE CASCADE,
    path_raw BLOB NOT NULL,
    path_key TEXT NOT NULL,
    parent_identity_key BLOB,
    is_root INTEGER NOT NULL CHECK (is_root IN (0, 1)),
    state TEXT NOT NULL CHECK (state IN ('pending', 'processing', 'done')),
    UNIQUE(scan_id, path_key)
);

CREATE TABLE scan_staged_observations (
    id INTEGER PRIMARY KEY,
    scan_id INTEGER NOT NULL REFERENCES scan_jobs(id) ON DELETE CASCADE,
    kind TEXT NOT NULL CHECK (kind IN ('file', 'folder')),
    identity_kind TEXT NOT NULL,
    identity_key BLOB NOT NULL,
    parent_identity_key BLOB,
    path_raw BLOB NOT NULL,
    path_key TEXT NOT NULL,
    display_path TEXT NOT NULL,
    size_bytes INTEGER NOT NULL,
    modified_unix_ns INTEGER,
    link_count INTEGER,
    UNIQUE(scan_id, path_key)
);

CREATE TABLE scan_staged_issues (
    id INTEGER PRIMARY KEY,
    scan_id INTEGER NOT NULL REFERENCES scan_jobs(id) ON DELETE CASCADE,
    code TEXT NOT NULL,
    path_key TEXT,
    detail_code TEXT
);

CREATE INDEX scan_queue_next_idx ON scan_queue(scan_id, state, id);
CREATE INDEX scan_staged_observations_scan_idx ON scan_staged_observations(scan_id, id);
CREATE INDEX scan_staged_issues_scan_idx ON scan_staged_issues(scan_id, id);
