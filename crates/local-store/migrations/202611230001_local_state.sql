-- Local launcher state. Secrets are never stored here; credentials live in the
-- platform secure store (see ADR-0003).

CREATE TABLE local_settings (
    id INTEGER PRIMARY KEY CHECK (id = 1),
    language TEXT NOT NULL DEFAULT 'en' CHECK (length(trim(language)) > 0),
    install_root TEXT,
    auto_start INTEGER NOT NULL DEFAULT 0 CHECK (auto_start IN (0, 1)),
    close_to_tray INTEGER NOT NULL DEFAULT 0 CHECK (close_to_tray IN (0, 1)),
    notifications_enabled INTEGER NOT NULL DEFAULT 1 CHECK (notifications_enabled IN (0, 1)),
    max_parallel_downloads INTEGER NOT NULL DEFAULT 3
        CHECK (max_parallel_downloads BETWEEN 1 AND 8),
    download_limit_bytes_per_second INTEGER
        CHECK (download_limit_bytes_per_second IS NULL
            OR download_limit_bytes_per_second > 0),
    updated_at_ms INTEGER NOT NULL
);

INSERT INTO local_settings (id, updated_at_ms) VALUES (1, 0);

CREATE TABLE installation_state (
    id INTEGER PRIMARY KEY CHECK (id = 1),
    active_manifest_id TEXT,
    updated_at_ms INTEGER NOT NULL
);

INSERT INTO installation_state (id, updated_at_ms) VALUES (1, 0);

CREATE TABLE download_tasks (
    id TEXT PRIMARY KEY,
    manifest_id TEXT NOT NULL CHECK (length(trim(manifest_id)) > 0),
    relative_path TEXT NOT NULL CHECK (length(trim(relative_path)) > 0),
    url TEXT NOT NULL CHECK (length(trim(url)) > 0),
    expected_size INTEGER NOT NULL CHECK (expected_size >= 0),
    downloaded_bytes INTEGER NOT NULL DEFAULT 0
        CHECK (downloaded_bytes >= 0 AND downloaded_bytes <= expected_size),
    sha256 TEXT NOT NULL CHECK (length(sha256) = 64),
    state TEXT NOT NULL CHECK (
        state IN ('queued', 'downloading', 'paused', 'verifying', 'completed', 'failed', 'cancelled')
    ),
    attempts INTEGER NOT NULL DEFAULT 0 CHECK (attempts >= 0),
    last_error TEXT,
    created_at_ms INTEGER NOT NULL,
    updated_at_ms INTEGER NOT NULL,
    UNIQUE (manifest_id, relative_path)
);

CREATE INDEX download_tasks_state_idx ON download_tasks (state);
