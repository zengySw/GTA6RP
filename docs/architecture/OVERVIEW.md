# Architecture overview

## Product boundary

The first release targets Windows 10/11 x64. Before an official PC release and a validated legal installation and launch path exist, the client uses a synthetic fixture and a separate test application. The repository does not distribute game binaries.

## Initial workspace

- `apps/launcher`: Slint UI and Rust desktop entry point. UI content is mock data while the Phase 0 Windows validation remains open. Integrated with `gta6rp-application` (ApplicationCore reducer, EventBus) for state management and `gta6rp-local-store` (SQLite) for persistent settings. Free space checking via `gta6rp-installation`.
- `apps/api`: Axum API foundation with health, request IDs, JSON errors, CORS, rate limiting, tracing, graceful shutdown, PostgreSQL pool, and embedded SQLx migrations. The account/session module is implemented; catalog and administrative modules are deferred.
- `crates/domain`: pure domain types with no infrastructure dependencies. Holds the main screen state machine, the validation rules for manifest-relative paths, and non-secret launcher settings (`Settings`).
- `crates/application`: UI-independent orchestration primitives: a bounded event bus, a cooperative cancellation token, a graceful-shutdown supervisor, typed errors, a state-reducer task that applies domain transitions off the UI thread, and local-state use cases (`LocalStatePort`, `prepare_startup()`, `update_settings()`, `load_settings()`).
- `crates/installation`: safe writes confined to a canonicalised installation root. Validates relative paths, refuses symlinked components, and places files with a temporary file plus rename. Also provides free space checking (`free_space_info()`, `ensure_free_space()`).
- `crates/local-store`: SQLite local state with embedded migrations for settings, the selected install root, the active manifest id, and download-task checkpoints, including recovery of interrupted tasks after an unclean restart. Implements `LocalStatePort`.
- `infrastructure/compose/docker-compose.yml`: local persistent PostgreSQL and an isolated ephemeral test database profile.
- Future crates for contracts, downloading, manifests, verification, fixture provider, and secure storage are added when their boundaries have real code.
- Future `apps/api`: Axum modular monolith backed by PostgreSQL and SQLx.

## Dependency direction

The UI calls application services through UI models. Application services depend on domain types and ports. Infrastructure implements those ports. Domain code does not depend on Slint, HTTP, SQLx, or the filesystem. Long-running work stays off the UI thread.

**Launcher runtime model:** `ApplicationCore` runs in a dedicated system thread with its own Tokio runtime. The Slint event loop keeps the main thread. Communication between threads uses `mpsc` channels for `CoreControl` handoff and shutdown signaling, and `slint::invoke_from_event_loop` for marshaling `AppEvent` updates from the Tokio thread to the Slint UI thread. No polling through Slint Timer is used.

## Phase 0 decisions

Slint is the selected UI spike, not yet an accepted production framework. The spike must be evaluated on Windows x64 for visual quality, image loading, DPI scaling, resize/focus behavior, cold start, RAM, idle CPU, and licensing before closing Checkpoint 0. See [ADR-0001](../decisions/ADR-0001-slint.md).

