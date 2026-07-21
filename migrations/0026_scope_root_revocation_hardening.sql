-- Defense in depth for ADR-033 root withdrawal. Migration 0025 already gates
-- the active -> revoked state transition on a transaction-local privacy purge
-- capability. This forward-only hardening also requires every row whose
-- resulting state is revoked to contain the exact fixed tombstone. Updates
-- must preserve row identity and platform, including updates to an already
-- revoked row, so reusable platform capability bytes cannot be reintroduced.
UPDATE scope_access_grants
SET opaque_grant = X'00'
WHERE state = 'revoked' AND opaque_grant <> X'00';

CREATE TRIGGER scope_access_grants_revocation_tombstone_update
BEFORE UPDATE ON scope_access_grants
WHEN NEW.state = 'revoked'
 AND (
     NEW.scope_id <> OLD.scope_id
     OR NEW.platform <> OLD.platform
     OR NEW.opaque_grant <> X'00'
 )
BEGIN SELECT RAISE(ABORT, 'scope_revocation_grant_tombstone_required'); END;

CREATE TRIGGER scope_access_grants_revocation_tombstone_insert
BEFORE INSERT ON scope_access_grants
WHEN NEW.state = 'revoked'
 AND NEW.opaque_grant <> X'00'
BEGIN SELECT RAISE(ABORT, 'scope_revocation_grant_tombstone_required'); END;

-- A lock path alone is not a stable cross-process identity: an accidentally
-- removed/recreated entry would split cooperating readers and revokers across
-- two inodes. Bind each per-scope admission file to its first observed stable
-- platform identity. Later replacement is rejected instead of silently
-- establishing a second lock domain. These rows contain no user path data.
CREATE TABLE scope_filesystem_fence_identities (
    scope_id INTEGER NOT NULL CHECK (scope_id > 0),
    role TEXT NOT NULL CHECK (role IN ('root', 'gate', 'data')),
    identity_kind TEXT NOT NULL,
    identity_key BLOB NOT NULL,
    PRIMARY KEY (scope_id, role)
) WITHOUT ROWID;

CREATE TRIGGER scope_filesystem_fence_identities_immutable_update
BEFORE UPDATE ON scope_filesystem_fence_identities
BEGIN SELECT RAISE(ABORT, 'scope_filesystem_fence_identity_immutable'); END;

CREATE TRIGGER scope_filesystem_fence_identities_immutable_delete
BEFORE DELETE ON scope_filesystem_fence_identities
BEGIN SELECT RAISE(ABORT, 'scope_filesystem_fence_identity_immutable'); END;
