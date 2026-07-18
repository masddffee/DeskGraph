-- A scope may have one platform-owned opaque access grant. The database never
-- interprets the bytes: platform adapters own creation, restoration and
-- revocation. A missing row is intentionally non-active and must be treated as
-- `needs_reauthorization` by callers.
CREATE TABLE scope_access_grants (
    scope_id INTEGER PRIMARY KEY REFERENCES authorized_scopes(id) ON DELETE CASCADE,
    platform TEXT NOT NULL CHECK (platform IN ('macos', 'windows', 'linux')),
    opaque_grant BLOB NOT NULL CHECK (length(opaque_grant) BETWEEN 1 AND 1048576),
    state TEXT NOT NULL CHECK (state IN ('active', 'needs_reauthorization', 'revoked')),
    updated_at_unix_ms INTEGER NOT NULL CHECK (updated_at_unix_ms >= 0)
);

CREATE INDEX scope_access_grants_active_scope_idx
    ON scope_access_grants(state, scope_id)
    WHERE state = 'active';
