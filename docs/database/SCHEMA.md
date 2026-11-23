# Initial database schema

The PostgreSQL schema is created by the embedded SQLx migrations in `apps/api/migrations`. API startup applies pending migrations before binding the HTTP listener. Migrations are append-only; schema changes should be introduced as new migration files.

## Tables and relationships

- `users` owns account identity and an Argon2id password hash. A case-insensitive unique index prevents duplicate email addresses.
- `roles`, `permissions`, `user_roles`, and `role_permissions` define the authorization graph.
- `sessions` references users and stores only 32-byte SHA-256 hashes for access and refresh tokens. It does not store bearer tokens in plaintext. Access tokens expire after 15 minutes; refresh tokens expire after 30 days and rotate transactionally.
- `launcher_releases` stores signed release metadata, platform, channel, size, and SHA-256 digest.
- `games`, `manifests`, and `manifest_files` describe published game/fixture manifests. The API stores metadata and object URLs, not large blobs.
- `servers` and `server_status_snapshots` represent the catalog and status history.
- `news` stores draft and published articles.
- `user_settings` stores launcher preferences separately from account credentials.
- `audit_logs` records administrative actions and JSON metadata.

UUIDs are used for externally addressable entities. Audit and status-history rows use generated integer identities. Timestamps use `TIMESTAMPTZ`; application timestamps should be UTC. Foreign keys and delete behavior are explicit. Catalog, session, release, news, and audit lookup paths have indexes in the initial migration.

The schema is a foundation, not a complete auth or administration implementation. Password hashing and refresh-token rotation are implemented; email verification, permission checks, and audit writes remain future work. The live migrations still need to be exercised against a clean PostgreSQL 18.7 instance before Phase 2.2 is accepted.

