CREATE INDEX watch_events_active_deadline_idx
    ON watch_events (
        CASE status WHEN 'reconciling' THEN 0 ELSE 1 END,
        stable_after_unix_ms,
        id
    )
    WHERE status IN ('stabilizing', 'reconciling');
