CREATE TABLE authorized_scopes (
    id INTEGER PRIMARY KEY,
    path_raw BLOB NOT NULL,
    path_key TEXT NOT NULL UNIQUE,
    display_path TEXT NOT NULL,
    platform TEXT NOT NULL,
    created_at_unix_ms INTEGER NOT NULL
);

CREATE TABLE scan_jobs (
    id INTEGER PRIMARY KEY,
    scope_id INTEGER NOT NULL REFERENCES authorized_scopes(id),
    status TEXT NOT NULL CHECK (status IN ('running', 'completed', 'failed', 'interrupted')),
    discovered_files INTEGER NOT NULL DEFAULT 0,
    discovered_folders INTEGER NOT NULL DEFAULT 0,
    skipped_entries INTEGER NOT NULL DEFAULT 0,
    issue_count INTEGER NOT NULL DEFAULT 0,
    started_at_unix_ms INTEGER NOT NULL,
    finished_at_unix_ms INTEGER
);

CREATE TABLE nodes (
    id INTEGER PRIMARY KEY,
    kind TEXT NOT NULL CHECK (kind IN ('file', 'folder')),
    identity_kind TEXT NOT NULL,
    identity_key BLOB NOT NULL UNIQUE,
    created_at_unix_ms INTEGER NOT NULL,
    updated_at_unix_ms INTEGER NOT NULL
);

CREATE TABLE files (
    node_id INTEGER PRIMARY KEY REFERENCES nodes(id),
    size_bytes INTEGER NOT NULL,
    modified_unix_ns INTEGER,
    link_count INTEGER
);

CREATE TABLE folders (
    node_id INTEGER PRIMARY KEY REFERENCES nodes(id)
);

CREATE TABLE locations (
    id INTEGER PRIMARY KEY,
    scope_id INTEGER NOT NULL REFERENCES authorized_scopes(id),
    node_id INTEGER NOT NULL REFERENCES nodes(id),
    path_raw BLOB NOT NULL,
    path_key TEXT NOT NULL,
    display_path TEXT NOT NULL,
    present INTEGER NOT NULL CHECK (present IN (0, 1)),
    last_seen_scan_id INTEGER NOT NULL REFERENCES scan_jobs(id),
    UNIQUE(scope_id, path_key)
);

CREATE TABLE edges (
    id INTEGER PRIMARY KEY,
    scope_id INTEGER NOT NULL REFERENCES authorized_scopes(id),
    source_node_id INTEGER NOT NULL REFERENCES nodes(id),
    target_node_id INTEGER NOT NULL REFERENCES nodes(id),
    kind TEXT NOT NULL CHECK (kind = 'located_in'),
    active INTEGER NOT NULL CHECK (active IN (0, 1)),
    last_seen_scan_id INTEGER NOT NULL REFERENCES scan_jobs(id),
    UNIQUE(scope_id, source_node_id, target_node_id, kind)
);

CREATE TABLE scan_issues (
    id INTEGER PRIMARY KEY,
    scan_id INTEGER NOT NULL REFERENCES scan_jobs(id),
    code TEXT NOT NULL,
    path_key TEXT,
    detail_code TEXT
);

CREATE INDEX locations_present_node_idx ON locations(present, node_id);
CREATE INDEX edges_active_source_idx ON edges(active, source_node_id);
CREATE INDEX scan_jobs_scope_status_idx ON scan_jobs(scope_id, status);
