# RUST QUALITY AUDIT

**Date:** 2026-11-23  
**Status:** Research only — no source code modified

This audit examines ownership, borrowing, concurrency, async patterns, error handling, and code quality across the entire workspace.

---

## 1. Ownership and Borrowing Analysis

### 1.1 `gta6rp-domain`

| Type | Ownership Pattern | Assessment |
|------|------------------|------------|
| `RelativePath(String)` | Owned String, immutable after construction | ✅ Correct — value type |
| `ScreenState` / `ScreenAction` | Copy enums | ✅ Correct — trivial types |
| `InvalidTransition { state, action }` | Copy struct | ✅ Correct |
| `Settings { language: String, install_root: Option<PathBuf>, ... }` | Owned fields, Clone derive | ✅ Correct |
| `join_under(root: &Path, relative: &RelativePath)` | Borrowed inputs, returns owned PathBuf | ✅ Correct |

### 1.2 `gta6rp-application`

| Type | Ownership Pattern | Assessment |
|------|------------------|------------|
| `ApplicationCore` | Unit struct, `spawn()` creates task | ✅ Correct |
| `CoreControl { commands: mpsc::Sender, events: EventBus, state: watch::Receiver, cancellation: CancellationToken }` | All fields `Send + Sync`, `#[derive(Clone)]` | ✅ Correct — cloneable handle |
| `CoreHandle { control: CoreControl, join: Option<JoinHandle>, shutdown_timeout }` | Owns `JoinHandle`, `shutdown(mut self)` consumes | ✅ Correct — ownership transferred on shutdown |
| `CancellationToken { sender: Sender<bool> }` | `Sender` is `Clone + Send + Sync` | ✅ Correct |
| `EventBus { sender: Sender<AppEvent> }` | `Sender` is `Clone + Send + Sync` | ✅ Correct |
| `ShutdownSupervisor { token, tasks: JoinSet<()>, grace }` | Owns `JoinSet`, `shutdown(mut self)` consumes | ✅ Correct |

### 1.3 `gta6rp-installation`

| Type | Ownership Pattern | Assessment |
|------|------------------|------------|
| `SafeRoot { root: PathBuf }` | Owned PathBuf, `Clone` derive | ✅ Correct |
| `TempGuard { path: Option<PathBuf> }` | `Drop` removes temp file unless disarmed | ✅ Correct RAII pattern |
| `InstallError` variants hold `PathBuf` | Owned paths in error variants | ✅ Correct |

### 1.4 `gta6rp-local-store`

| Type | Ownership Pattern | Assessment |
|------|------------------|------------|
| `LocalStore { pool: SqlitePool }` | `#[derive(Clone, Debug)]` — `SqlitePool` is internally `Arc`-based | ✅ Correct — cheap clone |
| `DownloadTask` | All owned fields (`String`, `u64`, `i64`, `Option`), `Clone` derive | ✅ Correct |
| `SettingsRow` / `TaskRow` / `StateRow` | `sqlx::FromRow` structs, owned fields | ✅ Correct |

### 1.5 `gta6rp-launcher`

| Type | Ownership Pattern | Assessment |
|------|------------------|------------|
| `screen_state: Rc<Cell<ScreenState>>` | `Rc` + `Cell` — single-threaded interior mutability | ⚠️ Works for UI thread only, not shareable across threads |
| `access_token: Arc<Mutex<Option<String>>>` | `Arc<Mutex>` — shared across threads | ✅ Correct for cross-thread access |
| `busy: Arc<AtomicBool>` | `Arc<AtomicBool>` — lock-free | ✅ Correct |
| `weak_window: Weak<MainWindow>` | `Weak` — avoids reference cycles | ✅ Correct |

### 1.6 `gta6rp-api`

| Type | Ownership Pattern | Assessment |
|------|------------------|------------|
| `Config { database_url: String, ... }` | Owned fields, `#[derive(Clone)]` | ✅ Correct |
| `ApiError { status, body: contracts::ErrorEnvelope }` | `pub(crate)`, owned fields | ✅ Correct |
| `Pool` state in Axum | `State<PgPool>` extractor | ✅ Correct Axum pattern |

---

## 2. Concurrency and Async Analysis

### 2.1 `gta6rp-application` — Tokio usage

| Pattern | Location | Assessment |
|---------|----------|------------|
| `tokio::spawn(run(...))` | `core.rs:53` | ✅ Correct — spawns reducer task |
| `mpsc::channel(64)` | `core.rs:46` | ✅ Correct — bounded queue |
| `watch::channel(initial)` | `core.rs:47` | ✅ Correct — state broadcast |
| `tokio::select! { biased; ... }` | `core.rs:199` | ✅ Correct — biased selection ensures cancellation checked first |
| `CancellationToken` on `watch::channel(false)` | `cancellation.rs:20` | ✅ Correct — watch-based cancellation |
| `ShutdownSupervisor` with `JoinSet<()>` | `shutdown.rs:18` | ✅ Correct — tracks tasks |
| `tokio::time::timeout(grace, draining)` | `shutdown.rs:79` | ✅ Correct — bounded shutdown wait |

**No `Mutex`/`RwLock` held across `.await`** — confirmed by code review. ✅

**No blocking calls inside async** — confirmed. ✅

### 2.2 `gta6rp-launcher` — std::thread usage

| Pattern | Location | Assessment |
|---------|----------|------------|
| `std::thread::spawn(move || { ... })` | `auth_client.rs:88, 135, 178, 292` | ✅ Correct — offloads blocking I/O from UI thread |
| `Arc<Mutex<Option<String>>>` | `auth_client.rs:66` | ✅ Correct — shared token state |
| `Arc<AtomicBool>` | `auth_client.rs:67` | ✅ Correct — lock-free busy flag |
| `slint::invoke_from_event_loop(move || { ... })` | `auth_client.rs:218` | ✅ Correct — marshals to UI thread |
| `Timer::single_shot(...)` | `auth_client.rs:282` | ✅ Correct — Slint timer for refresh |
| `reqwest::blocking::Client` | `auth_client.rs:11` | ✅ Correct — blocking client in background thread |

**No Tokio in launcher** — `apps/launcher/Cargo.toml` has no `tokio` dependency. This means `ApplicationCore::spawn()` cannot be called from `main()` (it panics outside Tokio runtime).

### 2.3 `gta6rp-api` — Tokio usage

| Pattern | Location | Assessment |
|---------|----------|------------|
| `#[tokio::main]` | `main.rs:4` | ✅ Correct |
| `tokio::spawn(async move { ... })` | `lib.rs:89` | ✅ Correct — rate limit cleanup task |
| `tokio::task::spawn_blocking(...)` | `auth.rs:123, 213` | ✅ Correct — Argon2id hashing offloaded |
| `PgPoolOptions::new().connect(...)` | `lib.rs:280` | ✅ Correct — async DB connection |
| `axum::serve(...).with_graceful_shutdown(...)` | `lib.rs:294` | ✅ Correct — graceful shutdown |

---

## 3. Error Handling Analysis

### 3.1 Typed errors by crate

| Crate | Error Type | Variants | `thiserror`/`anyhow`? | Source chaining |
|-------|-----------|----------|----------------------|-----------------|
| `domain` | `PathError` | 12 variants | Manual `impl Display` + `std::error::Error` | ❌ No source (all unit variants) |
| `domain` | `InvalidTransition` | Unit struct | Manual | ❌ No source |
| `domain` | `SettingsError` | 4 variants | Manual | ❌ No source |
| `domain` | `JoinError` | 1 variant (`EscapesRoot(PathBuf)`) | Manual | ❌ No source |
| `application` | `ApplicationError` | 4 variants (unit) | Manual | ❌ No source |
| `application` | `UseCaseError<E>` | 2 variants (generic) | Manual | ✅ `source()` returns inner |
| `installation` | `InstallError` | 7 variants | Manual | ✅ `source()` for `Io` only |
| `local-store` | `StoreError` | 5 variants | Manual | ✅ `source()` for `Database`, `Migration`, `Io` |
| `api` | `ApiError` | `pub(crate)` struct | Manual | ❌ No `source()` |

**Assessment:** All error types are manually implemented with `Display` + `std::error::Error`. No `thiserror` or `anyhow` used. This is consistent and appropriate for a workspace that wants explicit control over error types.

### 3.2 Error propagation chains

| Layer | Errors | Propagation |
|-------|--------|-------------|
| Domain → Application | `PathError`, `SettingsError` | Converted via `From` impls where appropriate |
| Application → LocalStore | `UseCaseError<E>` wraps `StoreError` | ✅ Generic `E` type parameter |
| LocalStore → SQLx | `StoreError::Database(sqlx::Error)` | ✅ `From<sqlx::Error>` |
| Launcher → Auth | `&'static str` | ⚠️ String literals, not typed errors |
| API handlers | `ApiError` | ✅ Typed, mapped to HTTP status codes |

### 3.3 `unwrap()` / `expect()` usage

| File | Count | Context | Assessment |
|------|-------|---------|------------|
| `auth_client.rs` | 0 | Uses `?` and `match` | ✅ Good |
| `main.rs` (launcher) | 0 | Uses `?` and `if let` | ✅ Good |
| `lib.rs` (api) | 1 | `EnvFilter::try_from_default_env().unwrap_or_else(...)` | ✅ Has fallback |
| `main.rs` (api) | 0 | Uses `?` | ✅ Good |
| `safe_root.rs` | 0 | Uses `?` and `match` | ✅ Good |
| `store.rs` | 0 | Uses `?` and `map_err` | ✅ Good |
| `core.rs` | 0 | Uses `?` and `match` | ✅ Good |
| `use_cases.rs` tests | Multiple | `.expect()` in test code | ✅ Acceptable in tests |
| `screen.rs` tests | Multiple | `.expect()` in test code | ✅ Acceptable in tests |

**No `unwrap()` or `expect()` on ordinary runtime paths in production code.** ✅

### 3.4 `unsafe` code

Zero `unsafe` blocks in the entire workspace. ✅

---

## 4. Clone / Allocation Analysis

### 4.1 Unnecessary cloning

| Location | Pattern | Assessment |
|----------|---------|------------|
| `update_settings()` | `settings.clone()` before `port.save_settings()` | ✅ Necessary — returns validated `Settings` to caller |
| `FakePort` tests | `Mutex<Settings>` cloned on read | ✅ Test code only |
| `auth_client` | `email.to_string()`, `password.to_string()` | ✅ Necessary — moves into thread |
| `report_completion` | `Arc::clone(access_token)`, `api_base.to_owned()` | ✅ Necessary — moves into closure |

### 4.2 `Clone` derives

| Type | `Clone` derive | Justification |
|------|---------------|---------------|
| `CoreControl` | Yes | Needs to be handed to multiple consumers |
| `LocalStore` | Yes | `SqlitePool` is internally `Arc`, cheap to clone |
| `Settings` | Yes | Value type, small |
| `DownloadTask` | Yes | Value type for test/comparison |
| `DownloadState` | Yes | Copy enum |
| `ScreenState` / `ScreenAction` | Copy | Trivial types |
| `AppEvent` | Clone | Needed for broadcast channel |
| `CancellationToken` | Clone | Needed for sharing across tasks |

---

## 5. Send / Sync Analysis

| Type | Send | Sync | Notes |
|------|------|------|-------|
| `CoreControl` | ✅ | ✅ | All fields are Send + Sync |
| `CoreHandle` | ❌ (holds `JoinHandle`) | ❌ | `JoinHandle` is Send but not Sync; `shutdown(mut self)` consumes |
| `CancellationToken` | ✅ | ✅ | `watch::Sender<bool>` is Send + Sync |
| `EventBus` | ✅ | ✅ | `broadcast::Sender` is Send + Sync |
| `LocalStore` | ✅ | ✅ | `SqlitePool` is internally `Arc` |
| `SafeRoot` | ✅ | ✅ | `PathBuf` is Send + Sync |
| `Arc<Mutex<Option<String>>>` | ✅ | ✅ | Standard shared mutable pattern |
| `Arc<AtomicBool>` | ✅ | ✅ | Lock-free |

---

## 6. Potential Issues

### 6.1 `CoreHandle` is not `Sync`

`CoreHandle` owns `Option<JoinHandle<()>>`. `JoinHandle` is `Send` but not `Sync`. This means `CoreHandle` cannot be shared across threads via `Arc<CoreHandle>`. However, `CoreControl` (which is `Clone + Send + Sync`) is designed to be the shareable handle. This is a correct separation of concerns.

### 6.2 `ShutdownSupervisor` not used by `ApplicationCore`

`ApplicationCore` manages its own single task via `CoreHandle::shutdown()`. `ShutdownSupervisor` exists but is unused. This is not a bug, but it means `ApplicationCore` cannot supervise groups of background tasks (e.g., download tasks) without additional integration.

### 6.3 Launcher uses `Rc<Cell<ScreenState>>` instead of `CoreControl`

The launcher's inline state machine uses `Rc<Cell<ScreenState>>` which is single-threaded. If the launcher is ever modified to use `ApplicationCore`, this must be replaced with `CoreControl` which is `Send + Sync`.

### 6.4 No `Arc<Mutex>` for `LocalStore` in launcher

Since `LocalStore` is not used by the launcher, this is not an issue yet. When integrated, `LocalStore` is already `Clone` (cheap), so no `Arc<Mutex>` is needed.

### 6.5 `auth_client` uses `Mutex` for token state

`Arc<Mutex<Option<String>>>` for the access token. This is correct but the `Mutex` is held briefly (just for get/set). No `Mutex` held across `.await` (since auth_client uses blocking `std::thread`, not async).

### 6.6 Dangling Graphify edges

Graphify reports 126 dangling-endpoint edges — these are references to external crates (chrono, uuid, serde, etc.) not present in the workspace graph. This is expected and not a code issue.

---

## 7. Reference to Other Audit Documents

- `FULL_WORKSPACE_AUDIT.md` — Complete crate inventory
- `ARCHITECTURE_MAP.md` — Dependency graph and layer mapping
- `EXECUTION_FLOWS.md` — Detailed scenario call sequences
- `TEST_COVERAGE_AUDIT.md` — Test coverage for error handling
- `GRAPHIFY_ANALYSIS.md` — Graph validation
- `DECISIONS_REQUIRED.md` — Decisions needed (Phase C variant, etc.)
