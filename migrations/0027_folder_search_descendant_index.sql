-- Folder-scoped lexical retrieval walks `located_in` from a selected parent
-- folder to its children. Keep that recursive, parent-to-child lookup covered
-- without changing the original source-to-parent index used by manifest writes.
CREATE INDEX edges_scope_kind_active_target_source_idx
    ON edges(scope_id, kind, active, target_node_id, source_node_id);
