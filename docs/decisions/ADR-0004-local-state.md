# ADR-0004: Local state persistence

- Status: Accepted
- Date: 2026-11-23

## Context

The launcher needs to remember non-secret preferences, the selected installation
root, the active manifest id, and the state of download tasks so a restart or a
crash does not lose progress. Persistence must stay local to the machine,
survive process restarts, and never store reusable credentials.

## Decision

Use a local SQLite database through SQLx in `crates/local-store`.

- The database is a file opened with `create_if_missing`, WAL journal mode,
  foreign keys enabled, and a bounded busy timeout.
- Schema changes are append-only migrations embedded at build time with
  `sqlx::migrate!` and applied automatically on open.
- `local_settings` holds one row of non-secret preferences and the selected
  install root.
- `installation_state` holds the active manifest id.
- `download_tasks` holds per-file checkpoints (progress, state, attempts, last
  error) with a unique `(manifest_id, relative_path)` pair.
- On open, tasks left in a transitional state (`downloading`, `verifying`) are
  moved back to `paused` with an explanatory `last_error`, so they can resume.
- Credentials, refresh tokens, and private keys are never written to SQLite; the
  platform secure store handles those (ADR-0003).

## Alternatives

- A plain JSON/TOML preferences file would require ad-hoc atomic writes and
  would not give transactional task checkpoints.
- Storing credentials in SQLite is rejected for the reasons in ADR-0003.
- A remote store cannot provide offline recovery.

## Consequences and risks

- Schema changes must be introduced as new migration files; editing applied
  migrations would desynchronise existing installations.
- Relative download paths are validated with the domain path rules before being
  persisted, so the store cannot become a path-traversal source.
- Installation journal recovery (staging and apply) is a later Phase 6 concern;
  this ADR covers local state and download checkpoints only.
- SQLite is embedded (bundled) so no system SQLite dependency is required.
