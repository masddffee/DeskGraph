CREATE TRIGGER content_chunks_ocr_provenance_insert_guard
BEFORE INSERT ON content_chunks
WHEN NEW.provenance_kind = 'ocr_observation' AND (
    NEW.source_byte_start IS NOT NULL
    OR NEW.source_byte_end IS NOT NULL
    OR NEW.source_page_number IS NOT NULL
    OR NEW.source_unit_number IS NULL
    OR NEW.source_unit_number <= 0
    OR NEW.source_cell_reference IS NOT NULL
    OR NEW.source_fragment_index IS NULL
    OR NEW.source_fragment_index < 0
    OR NEW.source_bbox_x_ppm IS NULL
    OR NEW.source_bbox_y_ppm IS NULL
    OR NEW.source_bbox_width_ppm IS NULL
    OR NEW.source_bbox_height_ppm IS NULL
    OR NEW.source_bbox_x_ppm NOT BETWEEN 0 AND 1000000
    OR NEW.source_bbox_y_ppm NOT BETWEEN 0 AND 1000000
    OR NEW.source_bbox_width_ppm NOT BETWEEN 1 AND 1000000
    OR NEW.source_bbox_height_ppm NOT BETWEEN 1 AND 1000000
    OR NEW.source_bbox_x_ppm + NEW.source_bbox_width_ppm > 1000000
    OR NEW.source_bbox_y_ppm + NEW.source_bbox_height_ppm > 1000000
    OR (
        NEW.source_confidence_basis_points IS NOT NULL
        AND NEW.source_confidence_basis_points NOT BETWEEN 0 AND 10000
    )
)
BEGIN
    SELECT RAISE(ABORT, 'invalid OCR provenance');
END;

CREATE TRIGGER content_chunks_ocr_provenance_update_guard
BEFORE UPDATE OF
    provenance_kind,
    source_byte_start,
    source_byte_end,
    source_page_number,
    source_unit_number,
    source_cell_reference,
    source_fragment_index,
    source_bbox_x_ppm,
    source_bbox_y_ppm,
    source_bbox_width_ppm,
    source_bbox_height_ppm,
    source_confidence_basis_points
ON content_chunks
WHEN NEW.provenance_kind = 'ocr_observation' AND (
    NEW.source_byte_start IS NOT NULL
    OR NEW.source_byte_end IS NOT NULL
    OR NEW.source_page_number IS NOT NULL
    OR NEW.source_unit_number IS NULL
    OR NEW.source_unit_number <= 0
    OR NEW.source_cell_reference IS NOT NULL
    OR NEW.source_fragment_index IS NULL
    OR NEW.source_fragment_index < 0
    OR NEW.source_bbox_x_ppm IS NULL
    OR NEW.source_bbox_y_ppm IS NULL
    OR NEW.source_bbox_width_ppm IS NULL
    OR NEW.source_bbox_height_ppm IS NULL
    OR NEW.source_bbox_x_ppm NOT BETWEEN 0 AND 1000000
    OR NEW.source_bbox_y_ppm NOT BETWEEN 0 AND 1000000
    OR NEW.source_bbox_width_ppm NOT BETWEEN 1 AND 1000000
    OR NEW.source_bbox_height_ppm NOT BETWEEN 1 AND 1000000
    OR NEW.source_bbox_x_ppm + NEW.source_bbox_width_ppm > 1000000
    OR NEW.source_bbox_y_ppm + NEW.source_bbox_height_ppm > 1000000
    OR (
        NEW.source_confidence_basis_points IS NOT NULL
        AND NEW.source_confidence_basis_points NOT BETWEEN 0 AND 10000
    )
)
BEGIN
    SELECT RAISE(ABORT, 'invalid OCR provenance');
END;
