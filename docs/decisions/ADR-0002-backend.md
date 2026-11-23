# ADR-0002: Backend architecture

- Status: Accepted; API and initial database/contracts foundation implemented, data-backed modules deferred to later Phase 2 work
- Date: 2026-11-21

## Context

The launcher needs account, catalog, manifest, news, settings, and administrative APIs, with large immutable files delivered separately through object storage and a CDN.

## Decision

Use Axum and Tokio for a modular-monolith API, PostgreSQL with SQLx for metadata, and S3-compatible object storage plus a CDN for files. Keep business modules inside one deployable backend until operational evidence justifies a different boundary.

## Alternatives

- A microservice fleet would add deployment and consistency costs before product boundaries are proven.
- Serving large files through the API would couple application capacity to file delivery.

## Consequences and risks

- The API owns authorization, metadata, and audit decisions; the object store/CDN serves immutable blobs.
- API rate limiting, request IDs, tracing, graceful shutdown, PostgreSQL pooling, SQLx migrations, and the account/session handlers are implemented in `apps/api`.
- Registration stores an Argon2id password hash and never returns or logs the password. Opaque access and refresh tokens are returned once; only their SHA-256 hashes are stored. Login, refresh rotation, replay-triggered session revocation, logout, the current-user route, and a stricter per-IP login limit are implemented. Email verification remains outstanding.
- The migrations and Compose database profile still need a clean live-migration check. OpenAPI documents implemented routes; catalog routes and contract tests remain outstanding.
- No backend or database credentials may be embedded in the launcher.

