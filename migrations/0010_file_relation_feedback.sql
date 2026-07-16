CREATE TABLE file_relation_feedback_events (
    id INTEGER PRIMARY KEY,
    relation_id INTEGER NOT NULL REFERENCES file_relation_candidates(id),
    sequence INTEGER NOT NULL CHECK (sequence > 0),
    decision TEXT NOT NULL CHECK (decision IN ('accepted', 'rejected')),
    created_by TEXT NOT NULL CHECK (created_by = 'user'),
    created_at_unix_ms INTEGER NOT NULL CHECK (created_at_unix_ms >= 0),
    UNIQUE(relation_id, sequence)
);

CREATE INDEX file_relation_feedback_events_relation_idx
ON file_relation_feedback_events(relation_id, sequence DESC);

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
