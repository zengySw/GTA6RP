# FULL WORKSPACE AUDIT

**Date:** 2026-11-23  
**Scope:** Entire GTA6RP Cargo workspace  
**Status:** Research only — no source code modified

---

## 1. Workspace Inventory

| Member | Type | Path | Status |
|--------|------|------|--------|
| `gta6rp-api` | Application | `apps/api/` | Implemented (Axum + PostgreSQL + SQLx) |
| `gta6rp-launcher` | Application | `apps/launcher/` | Implemented (Slint UI, mock data) |
| `gta6rp-domain` | Library | `crates/domain/` | Implemented (screen state machine, path rules, settings) |
| `gta6rp-application` | Library | `crates/application/` | Implemented (core, events, cancellation, shutdown, use_cases) |
| `gta6rp-installation` | Library | `crates/installation/` | Implemented (SafeRoot, atomic writes) |
| `gta6rp-local-store` | Library | `crates/local-store/` | Implemented (SQLite, migrations, LocalStatePort) |

**Workspace root:** `/home/zengy/gta6rp/Cargo.toml`  
**Members:** `apps/api`, `apps/launcher`, `crates/application`, `crates/domain`, `crates/installation`, `crates/local-store`  
**Edition:** 2024  
**Rust version:** 1.95 (toolchain pinned to 1.100.0)  
**Lints:** `unsafe_code = deny`, `unused_must_use = deny`, clippy `all` + `pedantic` at warn, CI uses `-D warnings`

---

## 2. Crate-by-Crate Audit

### 2.1 `gta6rp-domain` — Pure domain layer

**File:** `crates/domain/Cargo.toml`, `crates/domain/src/lib.rs`

| Submodule | File | Purpose | Public Interface | Tests |
|-----------|------|---------|------------------|-------|
| `path` | `crates/domain/src/path.rs` | Validates manifest-relative paths; rejects traversal, absolute paths, Windows reserved names | `RelativePath`, `PathError`, `join_under()`, `JoinError`, `MAX_PATH_LENGTH`, `MAX_COMPONENT_LENGTH` | 15 tests |
| `screen` | `crates/domain/src/screen.rs` | Main screen state machine | `ScreenState`, `ScreenAction`, `InvalidTransition`, `apply()`, `is_busy()`, `primary_action_available()` | 11 tests |
| `settings` | `crates/domain/src/settings.rs` | Non-secret launcher preferences | `Settings`, `SettingsError`, `MAX_PARALLEL_DOWNLOADS`, `validate()`, `Default` | 6 tests |

**Dependencies:** None (zero external crates)  
**Dependents:** `gta6rp-application`, `gta6rp-installation`, `gta6rp-local-store`, `gta6rp-launcher`  
**Architecture compliance:** ✅ Fully compliant — no Slint, HTTP, SQLx, or filesystem dependencies

**Assessment:** Excellent. Well-designed domain layer with typed errors, comprehensive validation, and full test coverage.

---

### 2.2 `gta6rp-application` — Application core

**File:** `crates/application/Cargo.toml`, `crates/application/src/lib.rs`

| Submodule | File | Purpose | Public Interface | Tests |
|-----------|------|---------|------------------|-------|
| `core` | `crates/application/src/core.rs` | State reducer task, bounded command queue | `ApplicationCore::spawn()`, `CoreHandle`, `CoreControl`, `apply()`, `shutdown()`, `current_state()`, `state()`, `events()` | 4 tests |
| `events` | `crates/application/src/events.rs` | Bounded broadcast event bus | `EventBus`, `AppEvent`, `StartupFailure` | 6 tests |
| `cancellation` | `crates/application/src/cancellation.rs` | Cooperative cancellation token | `CancellationToken`, `cancel()`, `is_cancelled()`, `cancelled()`, `Default` | 4 tests |
| `shutdown` | `crates/application/src/shutdown.rs` | Graceful shutdown supervisor | `ShutdownSupervisor`, `spawn()`, `cancellation_token()`, `len()`, `is_empty()`, `shutdown()` | 4 tests |
| `error` | `crates/application/src/error.rs` | Typed application errors | `ApplicationError` | 0 tests (trivial enum) |
| `use_cases` | `crates/application/src/use_cases.rs` | Local-state use cases + port trait | `LocalStatePort`, `StartupReport`, `UseCaseError`, `load_settings()`, `update_settings()`, `prepare_startup()` | 5 tests |

**Dependencies:** `gta6rp-domain`, `tokio` (rt, sync, time)  
**Dependents:** `gta6rp-local-store`  
**Architecture compliance:** ⚠️ Partially — well-designed but completely unused by launcher

**Key issues:**
- `use_cases.rs` was previously not compiled (not in `lib.rs`) — **fixed in Phase B**
- `CoreControl` was not exported from crate root — **fixed in Phase B**
- `LocalStatePort` is not implemented by any consumer except `gta6rp-local-store`
- `ApplicationCore::spawn()` panics outside Tokio runtime — launcher has no Tokio dependency
- Zero integration between launcher and ApplicationCore

---

### 2.3 `gta6rp-installation` — Safe filesystem writes

**File:** `crates/installation/Cargo.toml`, `crates/installation/src/lib.rs`, `crates/installation/src/safe_root.rs`

| Component | File | Purpose | Public Interface | Tests |
|-----------|------|---------|------------------|-------|
| `SafeRoot` | `crates/installation/src/safe_root.rs` | Canonicalised installation root with symlink rejection and atomic writes | `open()`, `root()`, `resolve()`, `create_dir_all()`, `read()`, `exists()`, `write_atomic()` | 10 tests |
| `InstallError` | `crates/installation/src/safe_root.rs` | Typed filesystem errors | `RootNotFound`, `RootNotDirectory`, `EscapesRoot`, `SymlinkDetected`, `NotFound`, `PermissionDenied`, `Io` | Covered by SafeRoot tests |

**Dependencies:** `gta6rp-domain`  
**Dependents:** None  
**Architecture compliance:** ✅ Fully compliant

**Key issues:**
- Completely unused by any crate in the workspace
- Windows symlink/junction tests are `#[cfg(unix)]` only — Windows-specific validation is blocked in this Linux environment
- `TempGuard` uses `Drop` for cleanup — correct pattern

---

### 2.4 `gta6rp-local-store` — SQLite local state

**File:** `crates/local-store/Cargo.toml`, `crates/local-store/src/lib.rs`, `crates/local-store/src/store.rs`, `crates/local-store/src/model.rs`, `crates/local-store/src/error.rs`

| Component | File | Purpose | Public Interface | Tests |
|-----------|------|---------|------------------|-------|
| `LocalStore` | `crates/local-store/src/store.rs` | SQLite-backed persistence | `open()`, `open_in_memory()`, `close()`, `settings()`, `save_settings()`, `active_manifest_id()`, `set_active_manifest_id()`, `upsert_task()`, `tasks()`, `recover_interrupted()` | 10 tests |
| `LocalStatePort` impl | `crates/local-store/src/store.rs` | Port implementation for use cases | `load_settings()`, `save_settings()`, `active_manifest_id()`, `recover_interrupted()` | 3 integration tests |
| `DownloadTask` | `crates/local-store/src/model.rs` | Download checkpoint model | `queued()`, `validate()`, `as_str()`, `parse()`, `is_transitional()` | 6 tests |
| `DownloadState` | `crates/local-store/src/model.rs` | Download lifecycle enum | `Queued`, `Downloading`, `Paused`, `Verifying`, `Completed`, `Failed`, `Cancelled` | Covered by DownloadTask tests |
| `Settings` | Re-exported from `gta6rp_domain::settings` | Non-secret preferences | `validate()`, `Default` | Covered by domain tests |
| `StoreError` | `crates/local-store/src/error.rs` | Typed store errors | `Database`, `Migration`, `Io`, `InvalidInput`, `Corrupt` | 0 tests |

**Dependencies:** `gta6rp-application`, `gta6rp-domain`, `sqlx` (sqlite, macros, migrate, runtime-tokio)  
**Dependents:** None  
**Architecture compliance:** ✅ Fully compliant

**Key issues:**
- Completely unused by launcher
- `save_settings()` validates before write (domain validation enforced)
- `recover_interrupted()` moves `downloading`/`verifying` → `paused` — correct recovery pattern
- Migrations embedded at build time via `sqlx::migrate!()`
- `open_in_memory()` uses `max_connections(1)` to keep in-memory DB consistent

---

### 2.5 `gta6rp-api` — Axum backend API

**File:** `apps/api/Cargo.toml`, `apps/api/src/lib.rs`, `apps/api/src/auth.rs`, `apps/api/src/contracts.rs`, `apps/api/src/main.rs`

| Component | File | Purpose | Public Interface | Tests |
|-----------|------|---------|------------------|-------|
| `lib.rs` | `apps/api/src/lib.rs` | API router, config, middleware | `Config::from_env()`, `app()`, `serve()`, `health()`, `openapi()`, `ApiError` | 0 tests |
| `auth.rs` | `apps/api/src/auth.rs` | Registration, login, refresh, logout, current_user | `register()`, `login()`, `refresh()`, `logout()`, `current_user()` (all `pub(super)`) | 0 tests |
| `contracts.rs` | `apps/api/src/contracts.rs` | Shared DTOs | `Pagination`, `PaginationError`, `Page<T>`, `ErrorEnvelope` (`pub(crate)`), `ErrorDetails` (`pub(crate)`) | 0 tests |
| `main.rs` | `apps/api/src/main.rs` | Binary entry point | `main()` | 0 tests |

**Dependencies:** argon2, axum, chrono, serde, sha2, tokio, tower, tower-governor, tower-http, tracing, tracing-subscriber, uuid, sqlx (postgres)  
**Dependents:** None  
**Architecture compliance:** ⚠️ Partially — well-implemented auth, but isolated from workspace

**Key issues:**
- **Zero intra-workspace dependencies** — `gta6rp-api` does not depend on any other crate in the workspace
- `Page<T>` and `Pagination` defined in `contracts.rs` but **never consumed** by any route handler (dead code)
- `ErrorEnvelope` and `ErrorDetails` are `pub(crate)` — not accessible to launcher
- No contract tests (Phase 2.3 incomplete)
- No clean migration check against live PostgreSQL (Phase 2.2 incomplete)
- Routes: `/api/v1/health`, `/api/v1/openapi.yaml`, `/api/v1/auth/{register,login,refresh,logout}`, `/api/v1/users/me`
- Catalog, games, manifests, servers, news, admin modules are **not implemented** (deferred per PLAN.md)

---

### 2.6 `gta6rp-launcher` — Slint desktop UI

**File:** `apps/launcher/Cargo.toml`, `apps/launcher/src/main.rs`, `apps/launcher/src/state.rs`, `apps/launcher/src/auth_client.rs`, `apps/launcher/src/mock_data.rs`, `apps/launcher/build.rs`, `apps/launcher/ui/app.slint`, `apps/launcher/ui/components.slint`, `apps/launcher/ui/theme/tokens.slint`

| Component | File | Purpose | Public Interface | Tests |
|-----------|------|---------|------------------|-------|
| `main.rs` | `apps/launcher/src/main.rs` | Entry point, UI state machine wiring | `main()` | 0 tests |
| `state.rs` | `apps/launcher/src/state.rs` | UI string mapping for `ScreenState` | `title()`, `detail()`, `badge()`, `action_label()`, `notice()` | 0 tests |
| `auth_client.rs` | `apps/launcher/src/auth_client.rs` | Auth HTTP client, token refresh, Windows Credential Manager | `bind()` | 0 tests |
| `mock_data.rs` | `apps/launcher/src/mock_data.rs` | Mock server/news data | `load()` | 0 tests |
| `build.rs` | `apps/launcher/build.rs` | Slint UI compilation | `main()` | 0 tests |
| `app.slint` | `apps/launcher/ui/app.slint` | Main UI component | `MainWindow` with callbacks | 0 tests |
| `components.slint` | `apps/launcher/ui/components.slint` | Reusable UI components | `InfoCard`, `TextField`, `ActionButton`, `NavItem`, `FeedbackPanel`, `ModalDialog` | 0 tests |
| `tokens.slint` | `apps/launcher/ui/theme/tokens.slint` | Design tokens | `Theme` global | 0 tests |

**Dependencies:** `gta6rp-domain`, `reqwest` (blocking), `serde`, `slint`, `keyring` (windows only), `slint-build` (build dep)  
**Dependents:** None  
**Architecture compliance:** ⚠️ Partially — UI callbacks are I/O-free, but state machine is inline (not using ApplicationCore)

**Key issues:**
- **Zero tests** — no launcher tests at all
- **Does not use** `gta6rp-application`, `gta6rp-local-store`, or `gta6rp-installation`
- Uses `Rc<Cell<ScreenState>>` + `slint::Timer` for state transitions instead of `ApplicationCore`
- `auth_client.rs` uses `std::thread` + blocking `reqwest` — correct for offloading from UI thread
- Windows Credential Manager for refresh tokens; stub returns unavailable on non-Windows
- All Slint pages exist but most are mock/not connected (Downloads, Settings, Diagnostics, About)

---

## 3. Cross-Cutting Findings

### 3.1 Components that are completely disconnected

| Component | Problem | Evidence |
|-----------|---------|----------|
| `gta6rp-application` | Not imported by launcher | `apps/launcher/Cargo.toml` has no `gta6rp-application` dep; `main.rs` has no imports from it |
| `gta6rp-local-store` | Not imported by launcher | Same as above |
| `gta6rp-installation` | Not imported by launcher | Same as above |
| `gta6rp-api` | No workspace crate deps | `apps/api/Cargo.toml` has only external crates |
| `Page<T>` / `Pagination` | Defined but never used | `apps/api/src/contracts.rs:51`; repo-wide grep confirms only self-references |

### 3.2 Duplicate type definitions

The launcher defines its own auth wire types in `auth_client.rs:23-49` (`Credentials`, `RefreshRequest`, `TokenResponse`, `UserResponse`, `RegisterResponse`) that mirror the API's types but are not shared.

### 3.3 Missing crates from PLAN.md vision

PLAN.md section 4 lists these crates as future/needed but they don't exist:
- `contracts` — shared DTOs (not created)
- `download-engine` — not created
- `manifest-engine` — not created
- `verification` — not created
- `game-provider` — not created
- `process-manager` — not created
- `config` — not created
- `errors` — not created
- `logging` — not created
- `http-client` — not created
- `auth-client` — not created (auth logic is inline in launcher)
- `test-fixtures` — not created

### 3.4 Missing apps from PLAN.md vision

- `apps/manifest-tool` — not created

---

## 4. Configuration & Infrastructure

| File | Purpose | Status |
|------|---------|--------|
| `.env.example` | Example environment variables | Present, has placeholder secrets |
| `.gitignore` | Git ignore rules | Present, ignores `*.db`, `*.exe`, `target/` |
| `rust-toolchain.toml` | Toolchain pin | Pins Rust 1.100.0, minimal profile, rustfmt + clippy |
| `docker-compose.yml` | PostgreSQL + test DB | Present, postgres:18.7-bookworm, ports 54329/54330 |
| `ci.yml` | CI workflow | Windows runner, fmt/check/clippy/test |
| `openapi.yaml` | API contract | Present, documents implemented routes only |

---

## 5. Migration Files

| Migration | File | Purpose |
|-----------|------|---------|
| Initial schema | `apps/api/migrations/202611210001_initial_schema.sql` | users, roles, permissions, sessions, launcher_releases, games, manifests, manifest_files, servers, news, user_settings, audit_logs |
| Access tokens | `apps/api/migrations/202611210002_session_access_tokens.sql` | Adds `access_token_hash`, `access_expires_at` to sessions |
| Local state | `crates/local-store/migrations/202611230001_local_state.sql` | local_settings, installation_state, download_tasks |

---

## 6. Docs & ADRs

| Document | Path | Status |
|----------|------|--------|
| PLAN.md | Root | Full roadmap, 11 phases, checkpoints |
| AGENTS.md | Root | Project instructions |
| README.md | Root | Project overview |
| OVERVIEW.md | `docs/architecture/` | Architecture overview |
| SCHEMA.md | `docs/database/` | DB schema summary |
| ADR-0001 | `docs/decisions/` | Slint UI framework (Proposed) |
| ADR-0002 | `docs/decisions/` | Backend architecture (Accepted) |
| ADR-0003 | `docs/decisions/` | Local secret storage (Proposed) |
| ADR-0004 | `docs/decisions/` | Local state persistence (Accepted) |
| PHASE_C_INTEGRATION.md | `docs/architecture/` | Phase C integration research |

---

## 7. Verification Status

| Check | Command | Result |
|-------|---------|--------|
| Format | `cargo fmt --all -- --check` | PASS |
| Clippy | `cargo clippy --workspace --all-targets --all-features -- -D warnings` | PASS |
| Tests | `cargo test --workspace` | PASS (71 tests) |
| Build | `cargo check --workspace` | PASS |

**Test breakdown:**
- `gta6rp-application`: 22 tests
- `gta6rp-domain`: 23 tests
- `gta6rp-installation`: 10 tests
- `gta6rp-local-store`: 16 tests
- `gta6rp-api`: 0 tests
- `gta6rp-launcher`: 0 tests

---

## 8. Summary of Architectural Gaps

1. **Launcher does not use ApplicationCore** — inline `Rc<Cell<ScreenState>>` + `Timer` instead of the reducer pattern
2. **Launcher does not use LocalStore** — no persistent settings, install root, or download checkpoint persistence
3. **Launcher does not use Installation** — no safe filesystem writes
4. **API is compile-time isolated** — no shared types with launcher
5. **Page<T>/Pagination are dead code** — defined but never consumed
6. **Zero launcher tests** — no regression coverage for UI behavior
7. **No contracts crate** — DTOs not shared between API and launcher
8. **Multiple missing crates** from PLAN.md vision (download-engine, manifest-engine, verification, etc.)
