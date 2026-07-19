-- A pre-release database may already contain a broad root and one or more
-- narrower roots from the former one-folder-at-a-time flow. Keep the broadest
-- capability active and fail closed on every active descendant until a later
-- scope-removal flow can purge that historical row. No indexed data or source
-- file is deleted by this compatibility step.
UPDATE scope_access_grants
SET state = 'needs_reauthorization'
WHERE state = 'active'
  AND EXISTS (
      SELECT 1
      FROM authorized_scopes AS descendant
      JOIN authorized_scopes AS ancestor
        ON ancestor.id <> descendant.id
       AND ancestor.platform = descendant.platform
      JOIN scope_access_grants AS ancestor_grant
        ON ancestor_grant.scope_id = ancestor.id
       AND ancestor_grant.state = 'active'
      WHERE descendant.id = scope_access_grants.scope_id
        AND substr(descendant.path_key, 1, length(ancestor.path_key)) = ancestor.path_key
        AND (
            substr(ancestor.path_key, -1, 1) =
                CASE WHEN descendant.platform = 'windows' THEN char(92) ELSE '/' END
            OR substr(descendant.path_key, length(ancestor.path_key) + 1, 1) =
                CASE WHEN descendant.platform = 'windows' THEN char(92) ELSE '/' END
        )
  );

-- Newly active coverage roots on the same platform must never be exact
-- ancestors or descendants of one another. Exact path_key conflicts remain
-- valid because native reauthorization intentionally updates the existing
-- root and grant. Historical inactive rows do not prevent broad reauthorization.
CREATE TRIGGER authorized_scopes_reject_overlap_insert
BEFORE INSERT ON authorized_scopes
WHEN EXISTS (
    SELECT 1
    FROM authorized_scopes AS existing
    JOIN scope_access_grants AS existing_grant
      ON existing_grant.scope_id = existing.id
     AND existing_grant.state = 'active'
    WHERE existing.platform = NEW.platform
      AND existing.path_key <> NEW.path_key
      AND (
          (
              substr(NEW.path_key, 1, length(existing.path_key)) = existing.path_key
              AND (
                  substr(existing.path_key, -1, 1) =
                      CASE WHEN NEW.platform = 'windows' THEN char(92) ELSE '/' END
                  OR substr(NEW.path_key, length(existing.path_key) + 1, 1) =
                      CASE WHEN NEW.platform = 'windows' THEN char(92) ELSE '/' END
              )
          )
          OR
          (
              substr(existing.path_key, 1, length(NEW.path_key)) = NEW.path_key
              AND (
                  substr(NEW.path_key, -1, 1) =
                      CASE WHEN NEW.platform = 'windows' THEN char(92) ELSE '/' END
                  OR substr(existing.path_key, length(NEW.path_key) + 1, 1) =
                      CASE WHEN NEW.platform = 'windows' THEN char(92) ELSE '/' END
              )
          )
      )
)
BEGIN
    SELECT RAISE(ABORT, 'authorized_scope_overlap');
END;

CREATE TRIGGER authorized_scopes_reject_overlap_update
BEFORE UPDATE OF path_key, platform ON authorized_scopes
WHEN EXISTS (
    SELECT 1
    FROM authorized_scopes AS existing
    JOIN scope_access_grants AS existing_grant
      ON existing_grant.scope_id = existing.id
     AND existing_grant.state = 'active'
    WHERE existing.id <> OLD.id
      AND existing.platform = NEW.platform
      AND existing.path_key <> NEW.path_key
      AND (
          (
              substr(NEW.path_key, 1, length(existing.path_key)) = existing.path_key
              AND (
                  substr(existing.path_key, -1, 1) =
                      CASE WHEN NEW.platform = 'windows' THEN char(92) ELSE '/' END
                  OR substr(NEW.path_key, length(existing.path_key) + 1, 1) =
                      CASE WHEN NEW.platform = 'windows' THEN char(92) ELSE '/' END
              )
          )
          OR
          (
              substr(existing.path_key, 1, length(NEW.path_key)) = NEW.path_key
              AND (
                  substr(NEW.path_key, -1, 1) =
                      CASE WHEN NEW.platform = 'windows' THEN char(92) ELSE '/' END
                  OR substr(existing.path_key, length(NEW.path_key) + 1, 1) =
                      CASE WHEN NEW.platform = 'windows' THEN char(92) ELSE '/' END
              )
          )
      )
)
BEGIN
    SELECT RAISE(ABORT, 'authorized_scope_overlap');
END;

CREATE TRIGGER scope_access_grants_reject_overlap_insert
BEFORE INSERT ON scope_access_grants
WHEN NEW.state = 'active'
 AND EXISTS (
    SELECT 1
    FROM authorized_scopes AS selected
    JOIN authorized_scopes AS existing
      ON existing.id <> selected.id
     AND existing.platform = selected.platform
    JOIN scope_access_grants AS existing_grant
      ON existing_grant.scope_id = existing.id
     AND existing_grant.state = 'active'
    WHERE selected.id = NEW.scope_id
      AND (
          (
              substr(selected.path_key, 1, length(existing.path_key)) = existing.path_key
              AND (
                  substr(existing.path_key, -1, 1) =
                      CASE WHEN selected.platform = 'windows' THEN char(92) ELSE '/' END
                  OR substr(selected.path_key, length(existing.path_key) + 1, 1) =
                      CASE WHEN selected.platform = 'windows' THEN char(92) ELSE '/' END
              )
          )
          OR
          (
              substr(existing.path_key, 1, length(selected.path_key)) = selected.path_key
              AND (
                  substr(selected.path_key, -1, 1) =
                      CASE WHEN selected.platform = 'windows' THEN char(92) ELSE '/' END
                  OR substr(existing.path_key, length(selected.path_key) + 1, 1) =
                      CASE WHEN selected.platform = 'windows' THEN char(92) ELSE '/' END
              )
          )
      )
 )
BEGIN
    SELECT RAISE(ABORT, 'authorized_scope_overlap');
END;

CREATE TRIGGER scope_access_grants_reject_overlap_update
BEFORE UPDATE OF scope_id, platform, state ON scope_access_grants
WHEN NEW.state = 'active'
 AND EXISTS (
    SELECT 1
    FROM authorized_scopes AS selected
    JOIN authorized_scopes AS existing
      ON existing.id <> selected.id
     AND existing.platform = selected.platform
    JOIN scope_access_grants AS existing_grant
      ON existing_grant.scope_id = existing.id
     AND existing_grant.state = 'active'
    WHERE selected.id = NEW.scope_id
      AND (
          (
              substr(selected.path_key, 1, length(existing.path_key)) = existing.path_key
              AND (
                  substr(existing.path_key, -1, 1) =
                      CASE WHEN selected.platform = 'windows' THEN char(92) ELSE '/' END
                  OR substr(selected.path_key, length(existing.path_key) + 1, 1) =
                      CASE WHEN selected.platform = 'windows' THEN char(92) ELSE '/' END
              )
          )
          OR
          (
              substr(existing.path_key, 1, length(selected.path_key)) = selected.path_key
              AND (
                  substr(selected.path_key, -1, 1) =
                      CASE WHEN selected.platform = 'windows' THEN char(92) ELSE '/' END
                  OR substr(existing.path_key, length(selected.path_key) + 1, 1) =
                      CASE WHEN selected.platform = 'windows' THEN char(92) ELSE '/' END
              )
          )
      )
 )
BEGIN
    SELECT RAISE(ABORT, 'authorized_scope_overlap');
END;
