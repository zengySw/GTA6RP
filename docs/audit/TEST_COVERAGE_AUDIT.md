# TEST COVERAGE AUDIT

**Date:** 2026-11-23  
**Status:** Research only — no source code modified

---

## 1. Test Inventory by Crate

| Crate | Test Count | Passing | Failing | Files with Tests |
|-------|------------|---------|---------|------------------|
| `gta6rp-application` | 22 | 22 | 0 | core (4), events (6), cancellation (4), shutdown (4), use_cases (5) |
| `gta6rp-domain` | 23 | 23 | 0 | screen (11), path (15) |
| `gta6rp-installation` | 10 | 10 | 0 | safe_root (10) |
| `gta6rp-local-store` | 16 | 16 | 0 | model (6), store (10) |
| `gta6rp-api` | 0 | 0 | 0 | None |
| `gta6rp-launcher` | 0 | 0 | 0 | None |
| **Total** | **71** | **71** | **0** | |

---

## 2. Coverage Analysis by Component

### 2.1 `gta6rp-domain` — 23 tests (excellent)

| Module | Tests | Coverage |
|--------|-------|----------|
| `screen.rs` | 11 | State transitions, invalid actions, idempotent fail/repair, busy/primary_action flags, every action has at least one valid state |
| `path.rs` | 15 | Valid paths, absolute/traversal rejection, empty/control chars, Windows reserved names, length limits, components round-trip, join under root |
| `settings.rs` | 6 | Default validity, invalid language, out-of-range parallel downloads, zero download limit, valid install root |

**Assessment:** Comprehensive unit tests covering all domain invariants. Negative scenarios well-tested.

---

### 2.2 `gta6rp-application` — 22 tests (good)

| Module | Tests | Coverage |
|--------|-------|----------|
| `core.rs` | 4 | Valid transition + event emission, invalid action rejection, command order preservation, clean shutdown |
| `events.rs` | 6 | Delivery to subscribers, publish without subscribers, zero capacity, slow subscriber lag, independent subscribers |
| `cancellation.rs` | 4 | Start cancelled, wait resolves after cancel, already cancelled resolves immediately, clones share signal |
| `shutdown.rs` | 4 | Cooperative task shutdown, timeout on ignoring cancellation, task panic detection, empty supervisor |
| `use_cases.rs` | 5 | Load settings, invalid settings rejected without writing, valid settings persisted, startup reports state + recovery, startup surfaces storage failure |

**Assessment:** Good coverage of core reducer, event bus, cancellation, shutdown, and use cases. FakePort provides mock for `LocalStatePort`.

---

### 2.3 `gta6rp-installation` — 10 tests (good)

| Test | Coverage |
|------|----------|
| open_accepts_existing_directory | SafeRoot::open success |
| open_rejects_missing_directory | RootNotFound error |
| open_rejects_file | RootNotDirectory error |
| writes_creates_directories_and_content | write_atomic creates dirs, writes content |
| overwrites_existing_file_completely | Atomic overwrite |
| write_leaves_no_temporary_files | TempGuard cleanup |
| read_missing_file_reports_not_found | NotFound error |
| create_dir_all_creates_nested_directories | Directory creation |
| rejects_symlinked_ancestor | `#[cfg(unix)]` — symlink detection |
| rejects_symlinked_target | `#[cfg(unix)]` — symlink target detection |

**Assessment:** Good coverage. Symlink tests are `#[cfg(unix)]` only — Windows reparse point/junction validation is not tested in this Linux environment.

---

### 2.4 `gta6rp-local-store` — 16 tests (good)

| Module | Tests | Coverage |
|--------|-------|----------|
| `model.rs` | 6 | State string round-trip, transitional flags, valid task, invalid task (path, sha256, progress, url), queued task starts at zero |
| `store.rs` | 10 | Settings round-trip, validation enforced, active manifest round-trip, insert/list tasks, upsert updates progress, rejects invalid task, recovery of interrupted tasks, idempotent recovery, state survives reopen, prepare_startup via trait, persistence via trait, recovery via trait |

**Assessment:** Good coverage including integration tests through `LocalStatePort` trait and `prepare_startup()`.

---

### 2.5 `gta6rp-api` — 0 tests (critical gap)

| Missing | Details |
|---------|---------|
| Unit tests | None in `lib.rs`, `auth.rs`, or `contracts.rs` |
| Integration tests | None — no test that starts the API and hits endpoints |
| Contract tests | None — Phase 2.3 explicitly marks this as incomplete |
| Auth handler tests | None for register/login/refresh/logout/current_user |
| Error handling tests | None for rate limiting, CORS, malformed JSON |

**Critical:** The API has the most complex logic (Argon2id hashing, token rotation, session revocation, replay attack detection) and zero test coverage.

---

### 2.6 `gta6rp-launcher` — 0 tests (critical gap)

| Missing | Details |
|---------|---------|
| Unit tests | None in `main.rs`, `state.rs`, `auth_client.rs`, `mock_data.rs` |
| UI tests | None — no Slint widget testing |
| Auth client tests | None — no mock HTTP server tests |
| State machine tests | None — domain tests cover `ScreenState::apply()` but not the launcher's `apply_action_weak()` / `schedule_action()` |

**Critical:** The launcher has no test coverage at all. Any refactoring (e.g., replacing `Cell<Timer>` with `ApplicationCore`) would have zero regression detection.

---

## 3. Test Gaps by Scenario

| Scenario | Covered | Location | Gap |
|----------|---------|----------|-----|
| State transition (valid) | ✅ | `domain/src/screen.rs` | — |
| State transition (invalid) | ✅ | `domain/src/screen.rs` | — |
| Path validation (valid) | ✅ | `domain/src/path.rs` | — |
| Path validation (invalid) | ✅ | `domain/src/path.rs` | — |
| Settings validation | ✅ | `domain/src/settings.rs` | — |
| Settings persist/read | ✅ | `local-store/src/store.rs` | — |
| Download task upsert | ✅ | `local-store/src/store.rs` | — |
| Recovery of interrupted tasks | ✅ | `local-store/src/store.rs` | — |
| State survives reopen | ✅ | `local-store/src/store.rs` | — |
| ApplicationCore reducer | ✅ | `application/src/core.rs` | — |
| EventBus delivery | ✅ | `application/src/events.rs` | — |
| Cancellation | ✅ | `application/src/cancellation.rs` | — |
| Shutdown timeout/panic | ✅ | `application/src/shutdown.rs` | — |
| Use cases with FakePort | ✅ | `application/src/use_cases.rs` | — |
| SafeRoot write/read | ✅ | `installation/src/safe_root.rs` | — |
| SafeRoot symlink rejection | ⚠️ | `installation/src/safe_root.rs` | `#[cfg(unix)]` only |
| API auth handlers | ❌ | None | **Critical gap** |
| API rate limiting | ❌ | None | **Critical gap** |
| API contract tests | ❌ | None | Phase 2.3 incomplete |
| Launcher state machine | ❌ | None | **Critical gap** |
| Launcher auth client | ❌ | None | **Critical gap** |
| Launcher UI rendering | ❌ | None | **Critical gap** |
| Launcher shutdown | ❌ | None | **Critical gap** |
| End-to-end install flow | ❌ | None | Phase 6 not started |
| End-to-end download flow | ❌ | None | Phase 5 not started |
| Failure injection | ❌ | None | Phase 11.2 not started |

---

## 4. Tests That Only Check Internal Implementation

| Test | File | What it actually tests | Assessment |
|------|------|----------------------|------------|
| `detection_and_install_chain` | `screen.rs` | Chains `ScreenState::apply()` calls | ✅ Behavior — tests domain transitions |
| `detecting_and_install_chain` | `screen.rs` | Same chain | ✅ Behavior |
| `out_of_order_actions_are_rejected` | `screen.rs` | Invalid transitions return `Err` | ✅ Behavior |
| `every_action_resolves_for_at_least_one_state` | `screen.rs` | Every action has ≥1 valid state | ✅ Invariant |
| `components_round_trip` | `path.rs` | `components()` iterator | ⚠️ Implementation detail |
| `join_always_produces_a_path_under_root` | `path.rs` | `join_under()` result | ✅ Behavior |
| `delivers_events_to_subscribers` | `events.rs` | Broadcast delivery | ✅ Behavior |
| `slow_subscriber_observes_lag` | `events.rs` | Broadcast lag behavior | ✅ Behavior |
| `waiting_resolves_after_cancel_from_another_task` | `cancellation.rs` | Cross-task cancellation | ✅ Behavior |
| `reports_task_failure_on_panic` | `shutdown.rs` | Panic detection | ✅ Behavior |

No tests were found that test only trivial implementation details without behavioral value.

---

## 5. Mock/Fake Implementations

| Fake | File | Purpose | Used in production? |
|------|------|---------|---------------------|
| `FakePort` | `application/src/use_cases.rs` (tests) | Mocks `LocalStatePort` for use case tests | ❌ No |
| `TestDir` | `installation/src/safe_root.rs` (tests) | Temp directory cleanup | ❌ No |
| `TestDir` | `local-store/src/store.rs` (tests) | Temp directory cleanup | ❌ No |

**No production mock implementations exist.** The `LocalStatePort` trait has no production mock — `LocalStore` is the only implementation and it's not used by the launcher.

---

## 6. Unstable/Flaky Test Risks

| Test | Risk | Mitigation |
|------|------|------------|
| `waiting_resolves_after_cancel_from_another_task` | 50ms timeout might be tight on slow CI | 50ms is generous for a `cancelled().await` that should not resolve |
| `already_cancelled_token_resolves_immediately` | 50ms timeout | Low risk — immediate resolution |
| `reports_timeout_when_a_task_ignores_cancellation` | 50ms grace + 30s sleep | Low risk — timeout fires at 50ms |
| `state_survives_reopen` | Uses `std::env::temp_dir()` — concurrent test runs could collide | Uses `AtomicU64` counter + process ID for uniqueness |
| `recover_interrupted_tasks_only` | No timing dependency | Low risk |

---

## 7. Missing Test Categories

| Category | Status | Required for |
|----------|--------|--------------|
| API unit tests | ❌ Missing | Phase 11.1 |
| API integration tests | ❌ Missing | Phase 11.1 |
| Contract tests | ❌ Missing | Phase 2.3 |
| Launcher unit tests | ❌ Missing | Phase 11.1 |
| Launcher UI tests | ❌ Missing | Phase 11.1 |
| Download engine tests | ❌ Missing | Phase 5 |
| Manifest tests | ❌ Missing | Phase 11.1 |
| Downloader state machine tests | ❌ Missing | Phase 11.1 |
| PostgreSQL integration tests | ❌ Missing | Phase 11.1 (blocked: no PostgreSQL in this env) |
| E2E install fixture | ❌ Missing | Phase 11.1 |
| E2E update fixture | ❌ Missing | Phase 11.1 |
| E2E rollback fixture | ❌ Missing | Phase 11.1 |
| Failure injection tests | ❌ Missing | Phase 11.2 |
| Performance tests | ❌ Missing | Phase 11.3 |

---

## 8. Reference to Other Audit Documents

- `FULL_WORKSPACE_AUDIT.md` — Complete crate inventory and test counts
- `ARCHITECTURE_MAP.md` — Component dependencies relevant to test strategy
- `EXECUTION_FLOWS.md` — Scenarios that need test coverage
- `RUST_QUALITY_AUDIT.md` — Code quality relevant to testability
- `GRAPHIFY_ANALYSIS.md` — Graph validation of test relationships
- `ROADMAP.md` — Prioritized plan including test gaps
