ALTER TABLE content_chunks RENAME TO content_chunks_v3;

CREATE TABLE content_chunks (
    id INTEGER PRIMARY KEY,
    scope_id INTEGER NOT NULL REFERENCES authorized_scopes(id),
    node_id INTEGER NOT NULL REFERENCES nodes(id),
    location_id INTEGER NOT NULL REFERENCES locations(id),
    extraction_job_id INTEGER NOT NULL REFERENCES extraction_jobs(id),
    ordinal INTEGER NOT NULL,
    text TEXT NOT NULL,
    provenance_kind TEXT NOT NULL CHECK (provenance_kind IN ('byte_range', 'pdf_page')),
    source_byte_start INTEGER,
    source_byte_end INTEGER,
    source_page_number INTEGER,
    source_fragment_index INTEGER,
    source_size_bytes INTEGER NOT NULL,
    source_modified_unix_ns INTEGER,
    trust_class TEXT NOT NULL CHECK (trust_class = 'untrusted_extracted_text'),
    provider_id TEXT NOT NULL,
    provider_version TEXT NOT NULL,
    active INTEGER NOT NULL CHECK (active IN (0, 1)),
    created_at_unix_ms INTEGER NOT NULL,
    UNIQUE(extraction_job_id, ordinal),
    CHECK (
        (
            provenance_kind = 'byte_range'
            AND source_byte_start IS NOT NULL
            AND source_byte_end IS NOT NULL
            AND source_byte_start >= 0
            AND source_byte_start <= source_byte_end
            AND source_page_number IS NULL
            AND source_fragment_index IS NULL
        )
        OR
        (
            provenance_kind = 'pdf_page'
            AND source_byte_start IS NULL
            AND source_byte_end IS NULL
            AND source_page_number IS NOT NULL
            AND source_page_number > 0
            AND source_fragment_index IS NOT NULL
            AND source_fragment_index >= 0
        )
    )
);

INSERT INTO content_chunks (
    id, scope_id, node_id, location_id, extraction_job_id, ordinal, text,
    provenance_kind, source_byte_start, source_byte_end, source_page_number,
    source_fragment_index, source_size_bytes, source_modified_unix_ns, trust_class,
    provider_id, provider_version, active, created_at_unix_ms
)
SELECT
    id, scope_id, node_id, location_id, extraction_job_id, ordinal, text,
    'byte_range', source_byte_start, source_byte_end, NULL, NULL,
    source_size_bytes, source_modified_unix_ns, trust_class, provider_id,
    provider_version, active, created_at_unix_ms
FROM content_chunks_v3;

DROP TABLE content_chunks_v3;

CREATE INDEX content_chunks_active_node_idx
    ON content_chunks(scope_id, active, node_id, ordinal);
