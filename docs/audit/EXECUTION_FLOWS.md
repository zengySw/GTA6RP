# EXECUTION FLOWS

**Date:** 2026-11-23  
**Status:** Research only — no source code modified

This document maps the full lifecycle scenarios from launch to shutdown, showing the actual call sequence `UI → Application → Domain → Infrastructure → API/Storage`, where each sequence exists, where it is broken, and where it exists only in documentation.

---

## 1. Scenario: Launcher Startup

### 1.1 Actual execution path

```
User runs: cargo run -p gta6rp-launcher
    │
    ▼
main() [apps/launcher/src/main.rs:12]
    │
    ├─► MainWindow::new() [Slint]
    │
    ├─► mock_data::load(&window) [apps/launcher/src/mock_data.rs:5]
    │   └─► Sets servers, news, notice (mock data only)
    │
    ├─► auth_client::bind(&window) [apps/launcher/src/auth_client.rs:63]
    │   ├─► Reads GTA6RP_API_BASE_URL env var (default: http://127.0.0.1:8080/api/v1)
    │   ├─► Creates Arc<Mutex<Option<String>>> for access token
    │   ├─► Creates Arc<AtomicBool> for busy flag
    │   ├─► Binds on_submit_auth callback
    │   ├─► Binds on_sign_out callback
    │   └─► Calls restore_session() [line 164]
    │       ├─► spawns std::thread
    │       ├─► load_refresh_token() → Windows Credential Manager (or None on Linux)
    │       ├─► refresh_session() → HTTP POST to API /auth/refresh
    │       └─► slint::invoke_from_event_loop → updates UI
    │
    ├─► Rc::new(Cell::new(ScreenState::NotInstalled)) [line 17]
    │   └─► Inline state — NOT ApplicationCore
    │
    ├─► apply_state(&window, ScreenState::NotInstalled) [line 18]
    │   └─► Sets all Slint fixture_state_* properties
    │
    └─► window.run() [line 112]
        └─► Blocks on Slint event loop
```

### 1.2 Intended path (per PLAN.md Phase 4)

```
main()
    │
    ├─► LocalStore::open(path) [crates/local-store]
    ├─► prepare_startup(&store) [crates/application::use_cases]
    │   ├─► load_settings()
    │   ├─► active_manifest_id()
    │   └─► recover_interrupted()
    ├─► ApplicationCore::spawn(initial_state) [crates/application::core]
    │   └─► Returns CoreControl for UI
    ├─► Bridge EventBus → Slint via invoke_from_event_loop
    └─► window.run()
```

### 1.3 Gap analysis

| Step | Actual | Intended | Status |
|------|--------|----------|--------|
| LocalStore open | ❌ Not called | ✅ Required | **BROKEN** |
| prepare_startup | ❌ Not called | ✅ Required | **BROKEN** |
| ApplicationCore spawn | ❌ Not called | ✅ Required | **BROKEN** |
| EventBus → Slint bridge | ❌ Not built | ✅ Required | **BROKEN** |
| Inline Cell<Timer> state machine | ✅ Present | ❌ Should be replaced | **TEMPORARY** |
| auth_client::bind | ✅ Present | ✅ Present | **WORKING** |

---

## 2. Scenario: User Authentication (Sign In)

### 2.1 Actual execution path

```
User clicks "Sign in" button [app.slint:263]
    │
    ▼
on_submit_auth callback [main.rs:73]
    │
    ├─► busy_state.swap(true) — prevents double-submit
    │
    ├─► std::thread::spawn [line 88]
    │   ├─► login(&api_base, &email, &password) [line 349]
    │   │   ├─► make_client() → reqwest::blocking::Client
    │   │   ├─► post_json() → POST /auth/login
    │   │   └─► Returns TokenResponse {access_token, refresh_token, user}
    │   │
    │   ├─► store_refresh_token(&tokens.refresh_token) [line 460]
    │   │   └─► Windows: keyring::v1::Entry::set_password()
    │   │   └─► Non-Windows: returns Err(()) — sign-in fails
    │   │
    │   ├─► token_state.lock() → *current_token = Some(access_token)
    │   │
    │   └─► report_completion() → slint::invoke_from_event_loop
    │       ├─► set_account_signed_in(true)
    │       ├─► set_account_status("Signed in as {email}")
    │       ├─► schedule_refresh() → Timer::single_shot(14 min)
    │       └─► schedule_refresh spawns thread → refresh_session()
    │
    └─► Returns before thread completes (async from UI perspective)
```

### 2.2 Intended path (per PLAN.md Phase 3)

```
UI → Application Service (SignIn use case)
    │
    ├─► Validate credentials (domain)
    ├─► HTTP client → API /auth/login
    ├─► Store refresh token in platform secure store
    ├─► Store access token in application state
    └─► Return result to UI
```

### 2.3 Gap analysis

| Step | Actual | Intended | Status |
|------|--------|----------|--------|
| Credential validation | ❌ None client-side | ✅ Should validate | **MISSING** |
| Use case pattern | ❌ Inline in auth_client | ✅ Should be use case | **VIOLATION** |
| Platform secure store | ✅ Windows Credential Manager | ✅ Present | **WORKING (Windows only)** |
| Non-Windows secure store | ❌ Stub returns unavailable | ✅ Should work | **BROKEN (Linux)** |
| Token refresh | ✅ Timer-based | ✅ Present | **WORKING** |
| Access token usage | ❌ Not used for API calls | ⚠️ Should authorize requests | **NOT IMPLEMENTED** |

---

## 3. Scenario: Fixture State Transition (Mock)

### 3.1 Actual execution path

```
User clicks "Simulate fixture install" [app.slint:209]
    │
    ▼
on_primary_action callback [main.rs:22]
    │
    ├─► Match current_state → ScreenAction::BeginSetup
    │
    ├─► apply_action_weak() [main.rs:131]
    │   ├─► current_state.get().apply(action) → Detecting
    │   ├─► window.upgrade()
    │   ├─► apply_state() → sets all Slint properties
    │   └─► current_state.set(next)
    │
    ├─► schedule_action() chains [main.rs:35-63]
    │   ├─► Timer::single_shot(350ms) → DetectionFinished → Verifying
    │   ├─► Timer::single_shot(700ms) → VerificationFinished → UpdateAvailable
    │   ├─► Timer::single_shot(1150ms) → DownloadStarted → Downloading
    │   └─► Timer::single_shot(2200ms) → DownloadCompleted → Ready
    │
    └─► UI updates synchronously
```

### 3.2 Intended path (per PLAN.md Phase 4)

```
UI → CoreControl::apply(ScreenAction::BeginSetup)
    │
    ├─► mpsc::try_send(Command::Apply(action)) → bounded queue (64)
    │
    ▼
ApplicationCore reducer task (Tokio)
    │
    ├─► current_state.apply(action) → next state
    ├─► watch::send(next) → state channel
    ├─► EventBus::publish(ScreenChanged(next))
    │
    ▼
Bridge task → slint::invoke_from_event_loop → UI update
```

### 3.3 Gap analysis

| Step | Actual | Intended | Status |
|------|--------|----------|--------|
| State machine | ✅ `ScreenState::apply()` (domain) | ✅ Present | **WORKING** |
| Async reducer | ❌ Synchronous in UI thread | ✅ Should be background task | **VIOLATION** |
| Bounded command queue | ❌ Not used | ✅ Required | **MISSING** |
| Event bus to UI | ❌ Not built | ✅ Required | **MISSING** |
| Timer-based sequencing | ✅ Present | ❌ Should be replaced | **TEMPORARY** |
| `try_send` error handling | ❌ Not used | ✅ Required | **MISSING** |

---

## 4. Scenario: Settings Persistence

### 4.1 Actual execution path

```
NO SETTINGS PERSISTENCE EXISTS IN THE LAUNCHER
```

The `apps/launcher/ui/app.slint` Settings page shows "NOT SAVED" badges [lines 277-279], but there is no code to read or write settings. `gta6rp-local-store` implements `Settings` persistence but is not imported by the launcher.

### 4.2 Intended path (per PLAN.md Phase 4.2)

```
UI → CoreControl → ApplicationCore → Use case update_settings()
    │
    ├─► LocalStatePort::save_settings(settings)
    │   └─► LocalStore::save_settings() → SQLite UPDATE
    │
    ▼
Result → EventBus → UI
```

### 4.3 Gap analysis

| Step | Actual | Intended | Status |
|------|--------|----------|--------|
| Settings read | ❌ Not implemented | ✅ Required | **MISSING** |
| Settings write | ❌ Not implemented | ✅ Required | **MISSING** |
| LocalStore open | ❌ Not called | ✅ Required | **MISSING** |
| prepare_startup | ❌ Not called | ✅ Required | **MISSING** |
| Settings validation | ✅ Domain `Settings::validate()` | ✅ Present | **AVAILABLE BUT UNUSED** |

---

## 5. Scenario: Download Task Persistence

### 5.1 Actual execution path

```
NO DOWNLOAD TASK PERSISTENCE EXISTS IN THE LAUNCHER
```

The Downloads page shows "No active download tasks" [app.slint:235] with an "NOT CONFIGURED" install location badge [line 236].

### 5.2 Intended path (per PLAN.md Phase 5)

```
UI → Download Service → LocalStore::upsert_task()
    │
    ├─► Insert/update download checkpoint
    ├─► On restart: recover_interrupted() → downloading/verifying → paused
    │
    ▼
EventBus → UI progress updates
```

### 5.3 Gap analysis

| Step | Actual | Intended | Status |
|------|--------|----------|--------|
| Download queue | ❌ Not implemented | ✅ Required | **MISSING** |
| Task persistence | ❌ Not implemented | ✅ Required | **MISSING** |
| Recovery after crash | ❌ Not implemented | ✅ Required | **MISSING** |
| Progress tracking | ❌ Not implemented | ✅ Required | **MISSING** |

---

## 6. Scenario: Safe File Installation

### 6.1 Actual execution path

```
NO FILE INSTALLATION EXISTS IN THE LAUNCHER
```

`gta6rp-installation` implements `SafeRoot` with `write_atomic()`, `resolve()`, `reject_symlink_components()`, but nothing calls it.

### 6.2 Intended path (per PLAN.md Phase 6)

```
Manifest → Diff → SafeRoot::resolve() → SafeRoot::write_atomic()
    │
    ├─► RelativePath validation (domain::path)
    ├─► Symlink rejection
    ├─► Temp file + rename
    │
    ▼
Installation state update → LocalStore
```

### 6.3 Gap analysis

| Step | Actual | Intended | Status |
|------|--------|----------|--------|
| Path validation | ✅ Domain `RelativePath::parse()` | ✅ Present | **AVAILABLE BUT UNUSED** |
| Safe root | ✅ `SafeRoot` implemented | ✅ Present | **AVAILABLE BUT UNUSED** |
| Atomic write | ✅ `write_atomic()` | ✅ Present | **AVAILABLE BUT UNUSED** |
| Symlink detection | ✅ `reject_symlink_components()` | ✅ Present | **AVAILABLE BUT UNUSED** |

---

## 7. Scenario: Application Shutdown

### 7.1 Actual execution path

```
User closes window
    │
    ▼
window.run() returns [main.rs:112]
    │
    └─► main() returns → process exits
```

No shutdown coordination:
- `auth_client` background threads may still be running
- No `ApplicationCore::shutdown()` called
- No `LocalStore::close()` called
- No `CancellationToken` cancelled

### 7.2 Intended path (per PLAN.md Phase 4.1)

```
Window close → shutdown signal
    │
    ├─► CoreHandle::shutdown() → cancel + join reducer task
    ├─► LocalStore::close() → release SQLite connections
    ├─► Background download tasks cancelled
    │
    ▼
Process exits cleanly
```

### 7.3 Gap analysis

| Step | Actual | Intended | Status |
|------|--------|----------|--------|
| Core shutdown | ❌ Not called | ✅ Required | **MISSING** |
| Store close | ❌ Not called | ✅ Required | **MISSING** |
| Cancellation | ❌ Not used | ✅ Required | **MISSING** |
| Thread cleanup | ⚠️ Process exit kills threads | ✅ Graceful preferred | **INCOMPLETE** |

---

## 8. Scenario: API Request from Launcher

### 8.1 Actual execution path

```
auth_client thread → reqwest::blocking::Client
    │
    ├─► POST http://127.0.0.1:8080/api/v1/auth/login
    │   ├─► Axum route handler [apps/api/src/auth.rs]
    │   ├─► SQLx query → PostgreSQL
    │   ├─► Argon2id verification via spawn_blocking
    │   └─► JSON TokenResponse
    │
    └─► Result parsed → Completion enum → invoke_from_event_loop → UI
```

### 8.2 Gap analysis

| Step | Actual | Intended | Status |
|------|--------|----------|--------|
| HTTP client | ✅ reqwest blocking in thread | ✅ Present | **WORKING** |
| API routes | ✅ Auth routes implemented | ✅ Present | **WORKING** |
| Shared DTOs | ❌ Launcher defines own types | ✅ Should share contracts | **VIOLATION** |
| Access token usage | ❌ Token stored but not sent | ⚠️ Should authorize requests | **NOT IMPLEMENTED** |
| Rate limit handling | ✅ Server-side 429 → client error | ✅ Present | **WORKING** |

---

## 9. Reference to Other Audit Documents

- `FULL_WORKSPACE_AUDIT.md` — Complete crate inventory
- `ARCHITECTURE_MAP.md` — Dependency graph and layer mapping
- `RUST_QUALITY_AUDIT.md` — Rust/async/ownership details
- `TEST_COVERAGE_AUDIT.md` — Test coverage for each scenario
- `GRAPHIFY_ANALYSIS.md` — Graph validation of call relationships
- `ROADMAP.md` — Prioritized plan to fix these gaps
- `DECISIONS_REQUIRED.md` — Decisions needed (Phase C variant, contracts crate, etc.)
