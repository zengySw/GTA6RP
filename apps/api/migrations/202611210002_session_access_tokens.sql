ALTER TABLE sessions
    ADD COLUMN access_token_hash BYTEA,
    ADD COLUMN access_expires_at TIMESTAMPTZ;

ALTER TABLE sessions
    ADD CONSTRAINT sessions_access_token_fields_paired
        CHECK ((access_token_hash IS NULL) = (access_expires_at IS NULL)),
    ADD CONSTRAINT sessions_access_hash_sha256
        CHECK (access_token_hash IS NULL OR octet_length(access_token_hash) = 32);

CREATE UNIQUE INDEX sessions_access_token_hash_unique
    ON sessions (access_token_hash)
    WHERE access_token_hash IS NOT NULL;
CREATE INDEX sessions_active_access_expiry_idx
    ON sessions (access_expires_at)
    WHERE revoked_at IS NULL AND access_token_hash IS NOT NULL;
