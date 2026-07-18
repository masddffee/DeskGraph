ALTER TABLE watch_events ADD COLUMN reconciliation_kind TEXT NOT NULL DEFAULT 'full_scope'
    CHECK (reconciliation_kind IN ('file_delta', 'full_scope'));

CREATE INDEX watch_events_stabilizing_kind_idx
    ON watch_events(scope_id, reconciliation_kind, stable_after_unix_ms, id)
    WHERE status = 'stabilizing';
