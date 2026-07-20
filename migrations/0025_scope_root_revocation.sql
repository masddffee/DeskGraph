-- ADR-033 root revocation is a privacy withdrawal, not a grant-state toggle.
-- The Rust database layer advances the policy revision, revokes the opaque OS
-- capability, purges the complete scope-derived index, and writes this
-- path-free receipt in one BEGIN IMMEDIATE transaction.
CREATE TABLE scope_root_revocation_receipts (
    id INTEGER PRIMARY KEY,
    scope_id INTEGER NOT NULL REFERENCES authorized_scopes(id),
    from_revision INTEGER NOT NULL CHECK (from_revision >= 1),
    to_revision INTEGER NOT NULL CHECK (to_revision = from_revision + 1),
    affected_location_count INTEGER NOT NULL CHECK (affected_location_count >= 0),
    affected_node_count INTEGER NOT NULL CHECK (affected_node_count >= 0),
    exclusions_removed INTEGER NOT NULL CHECK (exclusions_removed >= 0),
    purged_row_count INTEGER NOT NULL CHECK (purged_row_count >= 0),
    created_at_unix_ms INTEGER NOT NULL CHECK (created_at_unix_ms >= 0),
    UNIQUE(scope_id, to_revision)
);

CREATE TRIGGER scope_root_revocation_receipts_immutable_update
BEFORE UPDATE ON scope_root_revocation_receipts BEGIN
    SELECT RAISE(ABORT, 'scope_root_revocation_receipts_immutable');
END;
CREATE TRIGGER scope_root_revocation_receipts_immutable_delete
BEFORE DELETE ON scope_root_revocation_receipts BEGIN
    SELECT RAISE(ABORT, 'scope_root_revocation_receipts_immutable');
END;

-- A durable grant may only enter the revoked state while the matching privacy
-- capability is live and the scope has already advanced to that capability's
-- next revision. The opaque bookmark/token is replaced by a fixed tombstone by
-- the same transaction, so revocation retains no reusable OS capability.
CREATE TRIGGER scope_access_grants_revocation_privacy_capability_update
BEFORE UPDATE ON scope_access_grants
WHEN NEW.state = 'revoked'
 AND OLD.state <> 'revoked'
 AND NOT EXISTS (
     SELECT 1 FROM privacy_purge_capabilities c
     JOIN authorized_scopes s ON s.id = c.scope_id
     WHERE c.scope_id = OLD.scope_id
       AND c.to_revision = s.policy_revision
       AND c.from_revision = s.policy_revision - 1
 )
BEGIN SELECT RAISE(ABORT, 'scope_revocation_privacy_capability_required'); END;

-- The transaction-local capability remains consumable only after one of the
-- two accepted path-free receipts exists for its exact revision.
DROP TRIGGER privacy_purge_capabilities_guarded_delete;
CREATE TRIGGER privacy_purge_capabilities_guarded_delete
BEFORE DELETE ON privacy_purge_capabilities
WHEN OLD.to_revision <> (
        SELECT policy_revision FROM authorized_scopes WHERE id = OLD.scope_id
     )
  OR NOT (
        EXISTS (
            SELECT 1 FROM privacy_purge_receipts r
            WHERE r.scope_id = OLD.scope_id AND r.to_revision = OLD.to_revision
        )
        OR EXISTS (
            SELECT 1 FROM scope_root_revocation_receipts r
            WHERE r.scope_id = OLD.scope_id AND r.to_revision = OLD.to_revision
        )
     )
BEGIN SELECT RAISE(ABORT, 'privacy_purge_capability_not_consumable'); END;
