CREATE VIRTUAL TABLE location_search_fts USING fts5(
    display_path,
    content = 'locations',
    content_rowid = 'id',
    tokenize = 'trigram'
);

CREATE TRIGGER locations_search_ai AFTER INSERT ON locations BEGIN
    INSERT INTO location_search_fts(rowid, display_path)
    VALUES (new.id, new.display_path);
END;

CREATE TRIGGER locations_search_ad AFTER DELETE ON locations BEGIN
    INSERT INTO location_search_fts(location_search_fts, rowid, display_path)
    VALUES ('delete', old.id, old.display_path);
END;

CREATE TRIGGER locations_search_au AFTER UPDATE OF display_path ON locations BEGIN
    INSERT INTO location_search_fts(location_search_fts, rowid, display_path)
    VALUES ('delete', old.id, old.display_path);
    INSERT INTO location_search_fts(rowid, display_path)
    VALUES (new.id, new.display_path);
END;

INSERT INTO location_search_fts(location_search_fts) VALUES ('rebuild');

CREATE VIRTUAL TABLE content_search_fts USING fts5(
    text,
    content = 'content_chunks',
    content_rowid = 'id',
    tokenize = 'trigram'
);

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
