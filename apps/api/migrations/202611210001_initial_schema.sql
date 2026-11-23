CREATE TABLE users (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    email VARCHAR(320) NOT NULL,
    password_hash TEXT NOT NULL,
    email_verified_at TIMESTAMPTZ,
    disabled_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    CONSTRAINT users_email_not_blank CHECK (length(trim(email)) > 0)
);

CREATE UNIQUE INDEX users_email_lower_unique ON users (lower(email));

CREATE TABLE roles (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    name VARCHAR(80) NOT NULL UNIQUE,
    description TEXT NOT NULL DEFAULT '',
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    CONSTRAINT roles_name_not_blank CHECK (length(trim(name)) > 0)
);

CREATE TABLE permissions (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    name VARCHAR(120) NOT NULL UNIQUE,
    description TEXT NOT NULL DEFAULT '',
    CONSTRAINT permissions_name_not_blank CHECK (length(trim(name)) > 0)
);

CREATE TABLE user_roles (
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    role_id UUID NOT NULL REFERENCES roles(id) ON DELETE CASCADE,
    assigned_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    assigned_by UUID REFERENCES users(id) ON DELETE SET NULL,
    PRIMARY KEY (user_id, role_id)
);
CREATE INDEX user_roles_role_id_idx ON user_roles (role_id);

CREATE TABLE role_permissions (
    role_id UUID NOT NULL REFERENCES roles(id) ON DELETE CASCADE,
    permission_id UUID NOT NULL REFERENCES permissions(id) ON DELETE CASCADE,
    PRIMARY KEY (role_id, permission_id)
);
CREATE INDEX role_permissions_permission_id_idx ON role_permissions (permission_id);

CREATE TABLE sessions (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    refresh_token_hash BYTEA NOT NULL UNIQUE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    expires_at TIMESTAMPTZ NOT NULL,
    revoked_at TIMESTAMPTZ,
    rotated_to_session_id UUID UNIQUE REFERENCES sessions(id) ON DELETE SET NULL,
    user_agent TEXT,
    client_ip INET,
    CONSTRAINT sessions_expiration_after_creation CHECK (expires_at > created_at),
    CONSTRAINT sessions_refresh_hash_sha256 CHECK (octet_length(refresh_token_hash) = 32)
);
CREATE INDEX sessions_user_id_idx ON sessions (user_id);
CREATE INDEX sessions_active_expiry_idx ON sessions (expires_at) WHERE revoked_at IS NULL;

CREATE TABLE launcher_releases (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    version VARCHAR(64) NOT NULL,
    channel VARCHAR(16) NOT NULL CHECK (channel IN ('stable', 'beta')),
    platform VARCHAR(32) NOT NULL,
    minimum_supported_version VARCHAR(64),
    download_url TEXT NOT NULL,
    file_size BIGINT NOT NULL CHECK (file_size >= 0),
    sha256 CHAR(64) NOT NULL CHECK (sha256 ~ '^[0-9a-f]{64}$'),
    signature BYTEA NOT NULL CHECK (octet_length(signature) = 64),
    signing_key_id VARCHAR(80) NOT NULL,
    release_notes TEXT NOT NULL DEFAULT '',
    published_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (version, channel, platform)
);
CREATE INDEX launcher_releases_lookup_idx
    ON launcher_releases (channel, platform, published_at DESC)
    WHERE published_at IS NOT NULL;

CREATE TABLE games (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    slug VARCHAR(80) NOT NULL UNIQUE,
    name VARCHAR(160) NOT NULL,
    platform VARCHAR(32) NOT NULL,
    enabled BOOLEAN NOT NULL DEFAULT false,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    CONSTRAINT games_slug_not_blank CHECK (length(trim(slug)) > 0),
    CONSTRAINT games_name_not_blank CHECK (length(trim(name)) > 0)
);

CREATE TABLE manifests (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    game_id UUID NOT NULL REFERENCES games(id) ON DELETE RESTRICT,
    manifest_id VARCHAR(160) NOT NULL UNIQUE,
    channel VARCHAR(16) NOT NULL CHECK (channel IN ('stable', 'beta')),
    platform VARCHAR(32) NOT NULL,
    schema_version SMALLINT NOT NULL CHECK (schema_version > 0),
    signature BYTEA NOT NULL CHECK (octet_length(signature) = 64),
    signing_key_id VARCHAR(80) NOT NULL,
    object_url TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    published_at TIMESTAMPTZ,
    created_by UUID REFERENCES users(id) ON DELETE SET NULL
);
CREATE INDEX manifests_game_channel_idx
    ON manifests (game_id, channel, platform, published_at DESC)
    WHERE published_at IS NOT NULL;

CREATE TABLE manifest_files (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    manifest_id UUID NOT NULL REFERENCES manifests(id) ON DELETE CASCADE,
    path TEXT NOT NULL,
    file_size BIGINT NOT NULL CHECK (file_size >= 0),
    sha256 CHAR(64) NOT NULL CHECK (sha256 ~ '^[0-9a-f]{64}$'),
    download_url TEXT NOT NULL,
    UNIQUE (manifest_id, path),
    CONSTRAINT manifest_files_path_not_blank CHECK (length(trim(path)) > 0)
);

CREATE TABLE servers (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    slug VARCHAR(100) NOT NULL UNIQUE,
    name VARCHAR(180) NOT NULL,
    description TEXT NOT NULL DEFAULT '',
    region VARCHAR(80) NOT NULL,
    language VARCHAR(32) NOT NULL,
    game_version VARCHAR(64),
    status VARCHAR(20) NOT NULL DEFAULT 'offline'
        CHECK (status IN ('online', 'offline', 'maintenance')),
    players_online INTEGER NOT NULL DEFAULT 0 CHECK (players_online >= 0),
    player_capacity INTEGER NOT NULL CHECK (player_capacity > 0),
    connect_address TEXT,
    enabled BOOLEAN NOT NULL DEFAULT true,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    CONSTRAINT servers_players_within_capacity CHECK (players_online <= player_capacity)
);
CREATE INDEX servers_public_catalog_idx ON servers (region, status, name) WHERE enabled;

CREATE TABLE server_status_snapshots (
    id BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    server_id UUID NOT NULL REFERENCES servers(id) ON DELETE CASCADE,
    status VARCHAR(20) NOT NULL CHECK (status IN ('online', 'offline', 'maintenance')),
    players_online INTEGER NOT NULL CHECK (players_online >= 0),
    recorded_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX server_status_snapshots_server_time_idx
    ON server_status_snapshots (server_id, recorded_at DESC);

CREATE TABLE news (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    slug VARCHAR(180) NOT NULL UNIQUE,
    title VARCHAR(240) NOT NULL,
    summary VARCHAR(600) NOT NULL DEFAULT '',
    body_markdown TEXT NOT NULL,
    status VARCHAR(16) NOT NULL DEFAULT 'draft'
        CHECK (status IN ('draft', 'published', 'archived')),
    published_at TIMESTAMPTZ,
    author_id UUID REFERENCES users(id) ON DELETE SET NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    CONSTRAINT news_title_not_blank CHECK (length(trim(title)) > 0)
);
CREATE INDEX news_published_at_idx ON news (published_at DESC) WHERE status = 'published';

CREATE TABLE user_settings (
    user_id UUID PRIMARY KEY REFERENCES users(id) ON DELETE CASCADE,
    language VARCHAR(16) NOT NULL DEFAULT 'en',
    install_path TEXT,
    auto_start BOOLEAN NOT NULL DEFAULT false,
    close_to_tray BOOLEAN NOT NULL DEFAULT false,
    max_parallel_downloads SMALLINT NOT NULL DEFAULT 3
        CHECK (max_parallel_downloads BETWEEN 1 AND 8),
    download_limit_bytes_per_second BIGINT
        CHECK (download_limit_bytes_per_second IS NULL OR download_limit_bytes_per_second > 0),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE audit_logs (
    id BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    actor_user_id UUID REFERENCES users(id) ON DELETE SET NULL,
    action VARCHAR(120) NOT NULL,
    entity_type VARCHAR(80) NOT NULL,
    entity_id UUID,
    metadata JSONB NOT NULL DEFAULT '{}'::jsonb,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    CONSTRAINT audit_logs_action_not_blank CHECK (length(trim(action)) > 0)
);
CREATE INDEX audit_logs_created_at_idx ON audit_logs (created_at DESC);
CREATE INDEX audit_logs_actor_created_at_idx ON audit_logs (actor_user_id, created_at DESC);
