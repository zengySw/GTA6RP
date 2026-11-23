# ARCHITECTURAL DECISIONS REQUIRED

**Date:** 2026-11-23  
**Status:** Decisions awaiting approval  
**Source:** Cross-reference of audit findings with PLAN.md

This document lists all architectural decisions that require approval before implementation can proceed. No source code has been modified for any of these decisions.

---

## Decision 1: Phase C — ApplicationCore Integration Variant

### Context
`gta6rp-application` is fully implemented (ApplicationCore, CoreControl, EventBus, CancellationToken, ShutdownSupervisor, use_cases, LocalStatePort) but completely unused by the launcher. The launcher still uses inline `Rc<Cell<ScreenState>>` + `slint::Timer` for state transitions.

`ApplicationCore::spawn()` panics if called outside a Tokio runtime. The launcher's `main()` is synchronous and has no Tokio dependency.

### Option 1A — Separate std::thread with own Tokio runtime

**Description:** Launcher spawns a `std::thread` that creates its own `tokio::runtime::Runtime`, runs `ApplicationCore::spawn()` on it, and communicates with Slint via `slint::invoke_from_event_loop`. `CoreControl` is passed back to the main thread via an `mpsc` channel.

**Pros:**
- No Tokio dependency added to `apps/launcher/Cargo.toml`
- Follows existing launcher concurrency model (`auth_client.rs` already uses `std::thread`)
- `window.run()` remains sole owner of the main thread — no risk of blocking Slint event loop
- `invoke_from_event_loop` pattern already proven in `auth_client.rs:218`
- Simpler code review justification (no new runtime in UI process)

**Cons:**
- Additional inter-thread channel for `CoreControl` transfer
- Shutdown coordination between thread, runtime, and Slint window
- Slightly more boilerplate

**Files that would change:**
- `apps/launcher/src/main.rs` — thread creation, runtime, CoreControl, shutdown
- `apps/launcher/src/app_bridge.rs` (new) — EventBus → Slint bridge
- `apps/launcher/src/local_state.rs` (new) — LocalStore lifecycle
- `apps/launcher/Cargo.toml` — no changes

**Recommendation:** ✅ **Recommended** — aligns with existing patterns, no new dependencies.

---

### Option 1B — Tokio runtime inside Launcher

**Description:** Add `tokio` to `apps/launcher/Cargo.toml`, create a Tokio runtime, run `ApplicationCore` inside it, and run Slint either in the main thread or a separate thread.

**Pros:**
- Single runtime, simpler shared access to `CoreControl`
- Less bridge code

**Cons:**
- New dependency on `apps/launcher`
- New concurrency model alongside `std::thread` (used by `auth_client`)
- `window.run()` blocks the thread — must run Slint in a separate thread or use `block_in_place`
- Risk of blocking Slint event loop if Tokio tasks run on same thread
- Violates current principle that launcher doesn't use Tokio directly

**Files that would change:**
- `apps/launcher/Cargo.toml` — add `tokio` dependency
- `apps/launcher/src/main.rs` — async runtime, Slint threading
- `apps/launcher/src/app_bridge.rs` (new)
- `apps/launcher/src/local_state.rs` (new)

**Recommendation:** ❌ Not recommended — adds complexity without clear benefit.

---

## Decision 2: Shared Contracts Crate

### Context
`gta6rp-api` defines `Page<T>`, `Pagination`, `ErrorEnvelope`, `ErrorDetails` in `apps/api/src/contracts.rs`. The launcher defines its own auth wire types (`Credentials`, `RefreshRequest`, `TokenResponse`, `UserResponse`, `RegisterResponse`) in `auth_client.rs:23-49` that mirror the API wire format but are independent definitions. `Page<T>` and `Pagination` are defined but never consumed by any route handler.

### Option 2A — Create `crates/contracts` shared crate

**Description:** Extract shared DTOs (pagination, error envelope, auth request/response) into a new `crates/contracts` crate that both `gta6rp-api` and `gta6rp-launcher` depend on.

**Pros:**
- Single source of truth for wire types
- Prevents divergence between launcher and API
- Enables compile-time validation of API contracts

**Cons:**
- New crate to maintain
- API's `ErrorEnvelope`/`ErrorDetails` are currently `pub(crate)` — would need to be made public
- Launcher's auth types are simple `&'static str`-based results — may not map cleanly to shared types
- Adds dependency from launcher to a contracts crate

**Consensus needed:** Is the duplication worth eliminating at this stage?

---

### Option 2B — Accept duplication, defer shared crate

**Description:** Keep launcher's local auth types and API's contracts separate. Re-evaluate when more endpoints need sharing.

**Pros:**
- No new crate
- Launcher types are intentionally simple (string-based errors)
- Can be refactored later when more API endpoints are consumed by launcher

**Cons:**
- Type divergence risk
- `Page<T>` / `Pagination` remain dead code

**Recommendation:** Defer decision until Phase 5+ when download/manifest endpoints need sharing.

---

## Decision 3: `Page<T>` / `Pagination` Dead Code

### Context
`Page<T>` and `Pagination` are defined in `apps/api/src/contracts.rs:51,11` but never consumed by any route handler. All routes return simple JSON (`HealthResponse`, `TokenResponse`, `UserResponse`, `RegisterResponse`).

### Option 3A — Remove dead code

**Description:** Delete `Page<T>`, `Pagination`, `PaginationError`, `DEFAULT_PAGE_SIZE`, `MAX_PAGE_SIZE` from `contracts.rs`.

**Pros:**
- Eliminates dead code
- Reduces confusion about what's implemented

**Cons:**
- Removes forward-looking pagination infrastructure
- OpenAPI.yaml documents Pagination schema (would need updating)

---

### Option 3B — Keep as forward-looking infrastructure

**Description:** Leave the code in place, document that it's intended for future list endpoints.

**Pros:**
- Preserves design for future endpoints
- OpenAPI already references it

**Cons:**
- Dead code in repository
- No tests verify it works

**Recommendation:** Keep for now but add a `// TODO` or comment indicating it's for future list endpoints. Remove if not used by Phase 9.

---

## Decision 4: LocalStore Database Path

### Context
`LocalStore::open(path)` requires a file path. The `install_root` setting is itself stored in the database, creating a chicken-and-egg problem: the DB path must be decided before settings can be read.

### Option 4A — Fixed default path (platform config dir)

**Description:** Use a platform-appropriate config directory (e.g., `dirs::config_dir()` or `~/AppData/Local/GTA6RP` on Windows) for the SQLite file.

**Pros:**
- Simple, standard location
- Works on first launch without user configuration

**Cons:**
- Requires `dirs` crate or manual platform path logic
- User may want to relocate

---

### Option 4B — Executable-adjacent path

**Description:** Store the database next to the launcher executable.

**Pros:**
- Simple, no new dependency
- Portable (works from USB, etc.)

**Cons:**
- May not have write permissions in `Program Files`
- Not standard for user data on Windows

**Recommendation:** Option 4A with a fallback to Option 4B. Decision needed on whether to add `dirs` dependency.

---

## Decision 5: Windows Symlink/Junction Test Coverage

### Context
`crates/installation/src/safe_root.rs` has symlink rejection tests marked `#[cfg(unix)]`. Windows reparse point/junction handling is listed as a separate task in PLAN.md MVP 4.3. The current Linux environment cannot test Windows-specific behavior.

### Option 5A — Add `#[cfg(windows)]` tests

**Description:** Write Windows-specific tests for reparse point/junction detection that run on Windows CI.

**Pros:**
- Tests Windows-specific behavior where it matters
- Uses existing CI Windows runner

**Cons:**
- Cannot be verified in this Linux environment
- Requires Windows CI to pass

---

### Option 5B — Defer Windows-specific testing

**Description:** Keep `#[cfg(unix)]` tests only. Document that Windows reparse testing is deferred to Windows CI.

**Pros:**
- No untestable code in current environment
- Honest about limitations

**Cons:**
- Windows-specific path traversal via junctions untested until CI

**Recommendation:** Option 5B — document the gap and let Windows CI cover it.

---

## Decision 6: `Page<T>` Confusion in Graphify Communities

### Context
Graphify's community detection placed `Page<T>` in "API contracts" community alongside `DEFAULT_PAGE_SIZE`, `MAX_PAGE_SIZE`, `ErrorEnvelope`, etc. This is correct (they're all in `contracts.rs`), but `Page<T>` is dead code.

### Decision needed

No code change required — this is a documentation/audit decision. The graph correctly identifies `Page<T>` as part of the contracts community but cannot detect that it's unused. Manual code review (this audit) confirms it's dead code.

---

## Summary Table

| Decision | Options | Recommendation | Blocker For |
|----------|---------|----------------|-------------|
| 1. Phase C integration | 1A (thread + Tokio) / 1B (Tokio in launcher) | **1A** | Phase C implementation |
| 2. Shared contracts crate | 2A (create) / 2B (defer) | **2B** (defer) | API-launcher type sharing |
| 3. `Page<T>` dead code | 3A (remove) / 3B (keep) | **3B** (keep with comment) | Phase 9+ |
| 4. LocalStore DB path | 4A (config dir) / 4B (executable-adjacent) | **4A with 4B fallback** | LocalStore wiring to launcher |
| 5. Windows symlink tests | 5A (cfg windows) / 5B (defer) | **5B** (defer to CI) | Phase 4.3 completion |
| 6. Graphify community confusion | Documentation only | N/A | None |

---

## Reference to Other Audit Documents

- `FULL_WORKSPACE_AUDIT.md` — Complete crate inventory
- `ARCHITECTURE_MAP.md` — Dependency graph showing isolation
- `EXECUTION_FLOWS.md` — Scenarios affected by these decisions
- `RUST_QUALITY_AUDIT.md` — Code quality context
- `TEST_COVERAGE_AUDIT.md` — Test gaps related to decisions
- `GRAPHIFY_ANALYSIS.md` — Graph validation (76 isolated nodes)
- `ROADMAP.md` — Prioritized plan with these decisions as prerequisites
