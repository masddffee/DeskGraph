ALTER TABLE extraction_jobs ADD COLUMN operation TEXT NOT NULL DEFAULT 'content'
    CHECK (operation IN ('content', 'screenshot_ocr'));

DROP TRIGGER IF EXISTS content_chunks_search_ai;
DROP TRIGGER IF EXISTS content_chunks_search_ad;
DROP TRIGGER IF EXISTS content_chunks_search_au;

ALTER TABLE content_chunks RENAME TO content_chunks_v5;

CREATE TABLE content_chunks (
    id INTEGER PRIMARY KEY,
    scope_id INTEGER NOT NULL REFERENCES authorized_scopes(id),
    node_id INTEGER NOT NULL REFERENCES nodes(id),
    location_id INTEGER NOT NULL REFERENCES locations(id),
    extraction_job_id INTEGER NOT NULL REFERENCES extraction_jobs(id),
    ordinal INTEGER NOT NULL,
    text TEXT NOT NULL,
    provenance_kind TEXT NOT NULL CHECK (
        provenance_kind IN (
            'byte_range',
            'pdf_page',
            'docx_paragraph',
            'pptx_slide',
            'xlsx_cell',
            'ocr_observation'
        )
    ),
    source_byte_start INTEGER,
    source_byte_end INTEGER,
    source_page_number INTEGER,
    source_unit_number INTEGER,
    source_cell_reference TEXT,
    source_fragment_index INTEGER,
    source_bbox_x_ppm INTEGER,
    source_bbox_y_ppm INTEGER,
    source_bbox_width_ppm INTEGER,
    source_bbox_height_ppm INTEGER,
    source_confidence_basis_points INTEGER,
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
            AND source_unit_number IS NULL
            AND source_cell_reference IS NULL
            AND source_fragment_index IS NULL
            AND source_bbox_x_ppm IS NULL
            AND source_bbox_y_ppm IS NULL
            AND source_bbox_width_ppm IS NULL
            AND source_bbox_height_ppm IS NULL
            AND source_confidence_basis_points IS NULL
        )
        OR
        (
            provenance_kind = 'pdf_page'
            AND source_byte_start IS NULL
            AND source_byte_end IS NULL
            AND source_page_number IS NOT NULL
            AND source_page_number > 0
            AND source_unit_number IS NULL
            AND source_cell_reference IS NULL
            AND source_fragment_index IS NOT NULL
            AND source_fragment_index >= 0
            AND source_bbox_x_ppm IS NULL
            AND source_bbox_y_ppm IS NULL
            AND source_bbox_width_ppm IS NULL
            AND source_bbox_height_ppm IS NULL
            AND source_confidence_basis_points IS NULL
        )
        OR
        (
            provenance_kind IN ('docx_paragraph', 'pptx_slide')
            AND source_byte_start IS NULL
            AND source_byte_end IS NULL
            AND source_page_number IS NULL
            AND source_unit_number IS NOT NULL
            AND source_unit_number > 0
            AND source_cell_reference IS NULL
            AND source_fragment_index IS NOT NULL
            AND source_fragment_index >= 0
            AND source_bbox_x_ppm IS NULL
            AND source_bbox_y_ppm IS NULL
            AND source_bbox_width_ppm IS NULL
            AND source_bbox_height_ppm IS NULL
            AND source_confidence_basis_points IS NULL
        )
        OR
        (
            provenance_kind = 'xlsx_cell'
            AND source_byte_start IS NULL
            AND source_byte_end IS NULL
            AND source_page_number IS NULL
            AND source_unit_number IS NOT NULL
            AND source_unit_number > 0
            AND source_cell_reference IS NOT NULL
            AND length(source_cell_reference) BETWEEN 2 AND 10
            AND source_fragment_index IS NOT NULL
            AND source_fragment_index >= 0
            AND source_bbox_x_ppm IS NULL
            AND source_bbox_y_ppm IS NULL
            AND source_bbox_width_ppm IS NULL
            AND source_bbox_height_ppm IS NULL
            AND source_confidence_basis_points IS NULL
        )
        OR
        (
            provenance_kind = 'ocr_observation'
            AND source_byte_start IS NULL
            AND source_byte_end IS NULL
            AND source_page_number IS NULL
            AND source_unit_number IS NOT NULL
            AND source_unit_number > 0
            AND source_cell_reference IS NULL
            AND source_fragment_index IS NOT NULL
            AND source_fragment_index >= 0
            AND source_bbox_x_ppm BETWEEN 0 AND 1000000
            AND source_bbox_y_ppm BETWEEN 0 AND 1000000
            AND source_bbox_width_ppm BETWEEN 1 AND 1000000
            AND source_bbox_height_ppm BETWEEN 1 AND 1000000
            AND source_bbox_x_ppm + source_bbox_width_ppm <= 1000000
            AND source_bbox_y_ppm + source_bbox_height_ppm <= 1000000
            AND source_confidence_basis_points BETWEEN 0 AND 10000
        )
    )
);

INSERT INTO content_chunks (
    id, scope_id, node_id, location_id, extraction_job_id, ordinal, text,
    provenance_kind, source_byte_start, source_byte_end, source_page_number,
    source_unit_number, source_cell_reference, source_fragment_index,
    source_bbox_x_ppm, source_bbox_y_ppm, source_bbox_width_ppm,
    source_bbox_height_ppm, source_confidence_basis_points, source_size_bytes,
    source_modified_unix_ns, trust_class, provider_id, provider_version, active,
    created_at_unix_ms
)
SELECT
    id, scope_id, node_id, location_id, extraction_job_id, ordinal, text,
    provenance_kind, source_byte_start, source_byte_end, source_page_number,
    source_unit_number, source_cell_reference, source_fragment_index,
    NULL, NULL, NULL, NULL, NULL, source_size_bytes, source_modified_unix_ns,
    trust_class, provider_id, provider_version, active, created_at_unix_ms
FROM content_chunks_v5;

DROP TABLE content_chunks_v5;

CREATE INDEX content_chunks_active_node_idx
    ON content_chunks(scope_id, active, node_id, ordinal);

CREATE TRIGGER content_chunks_search_ai AFTER INSERT ON content_chunks BEGIN
    INSERT INTO content_search_fts(rowid, text)
    VALUES (new.id, new.text);
END;

CREATE TRIGGER content_chunks_search_ad AFTER DELETE ON content_chunks BEGIN
    INSERT INTO content_search_fts(content_search_fts, rowid, text)
    VALUES ('delete', old.id, old.text);
END;

CREATE TRIGGER content_chunks_search_au AFTER UPDATE OF text ON content_chunks BEGIN
    INSERT INTO content_search_fts(content_search_fts, rowid, text)
    VALUES ('delete', old.id, old.text);
    INSERT INTO content_search_fts(rowid, text)
    VALUES (new.id, new.text);
END;

INSERT INTO content_search_fts(content_search_fts) VALUES ('rebuild');
