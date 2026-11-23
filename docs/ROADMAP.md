# ROADMAP

**Date:** 2026-11-23  
**Status:** Analytical supplement to `PLAN.md` — not a replacement  
**Source:** Cross-reference of `PLAN.md` with actual code state, Graphify analysis, and audit findings

This document maps the current state of the project against the existing `PLAN.md`, identifies deviations, and provides a prioritized list of remaining work within the existing phase structure. It does not modify `PLAN.md`, does not add new phases, and does not change checkpoint statuses.

---

## 1. Phase-by-Phase Status vs. PLAN.md

### PHASE 0 — Technical Validation & Foundation

| MVP | Planned | Actual | Status | Notes |
|-----|---------|--------|--------|-------|
| 0.1 Slint spike | Window, images, navigation, cards | ✅ Implemented | Complete | Windows-specific validation (DPI, resize, RAM, CPU, license) not done — Linux-only environment |
| 0.2 Workspace | Cargo workspace, toolchain, fmt, clippy, test, CI | ✅ Implemented | Complete | All items done |
| 0.3 Architecture docs | AGENTS.md, OVERVIEW.md, ADRs | ✅ Implemented | Complete | ADR-0001 (Slint) still "Proposed" — pending spike |

**Checkpoint 0:** Not closed — Slint spike Windows validation incomplete (environment limitation).

---

### PHASE 1 — UI Foundation

| MVP | Planned | Actual | Status | Notes |
|-----|---------|--------|--------|-------|
| 1.1 Design system | Colors, typography, components | ✅ Implemented | Complete | `tokens.slint`, `components.slint` |
| 1.2 Application shell | All pages (Home, Play, Servers, Downloads, News, Account, Settings, Diagnostics, About) | ✅ Implemented | Complete | All pages exist in `app.slint` |
| 1.3 UI state | State machine, mock data, loading/empty/error | ✅ Implemented | Complete | Inline `Rc<Cell<ScreenState>>` + `Timer` |

**Checkpoint 1:** Not closed — DPI/resize validation incomplete (environment limitation).

---

### PHASE 2 — Backend Foundation

| MVP | Planned | Actual | Status | Notes |
|-----|---------|--------|--------|-------|
| 2.1 API | Axum app, health, request ID, tracing, errors, CORS, rate limiting, graceful shutdown | ✅ Implemented | Complete | All routes implemented |
| 2.2 Database | PostgreSQL, SQLx, migrations, initial tables, indexes, FKs, test DB profile | ✅ Implemented | Partially | Clean migration check against live PostgreSQL 18.7 **not done** (no PostgreSQL available) |
| 2.3 Contracts | DTOs, error format, OpenAPI, pagination | ✅ Implemented | Partially | Contract tests **not done** |

**Checkpoint 2:** Not closed — Clean migration check and contract tests incomplete.

---

### PHASE 3 — Accounts & Security

| MVP | Planned | Actual | Status | Notes |
|-----|---------|--------|--------|-------|
| 3.1 | Registration, login, logout, Argon2id, tokens, rotation, revocation, rate limits | ✅ Implemented | Complete | All auth handlers in `apps/api/src/auth.rs` |
| 3.2 | UI → auth service, secure storage (Windows Credential Manager), auto-refresh, session restore | ✅ Implemented | Complete | `auth_client.rs` with `std::thread` + `keyring` |
| 3.3 | Roles, permissions, server-side auth check, admin API protection, audit events | ❌ Not implemented | Missing | `roles`, `permissions`, `user_roles`, `role_permissions`, `audit_logs` tables exist but unused |

**Checkpoint 3:** Not closed — RBAC (3.3) not implemented; clean migration check blocked.

---

### PHASE 4 — Rust Core & Local State (CURRENT FOCUS)

| MVP | Planned | Actual | Status | Notes |
|-----|---------|--------|--------|-------|
| 4.1 | Application state, use cases, typed errors, event channel UI/core, cancellation, graceful shutdown | ✅ All implemented | Complete | `ApplicationCore` wired to Slint via `app_bridge.rs` + `invoke_from_event_loop`. `use_cases` (`LocalStatePort`, `prepare_startup()`, `update_settings()`, `load_settings()`) implemented and connected to `LocalStore`. Event channel delivers `ScreenChanged`, `ActionRejected`, `StartupCompleted`, `StartupFailed`, `Shutdown` events to UI without polling. |
| 4.2 | SQLite, local migrations, settings, selected path, task state, active manifest ID, recovery | ✅ All implemented | Complete | `gta6rp-local-store` fully implemented and wired to launcher via `LocalStateHandle`. Settings persist across restarts, interrupted download tasks recover on startup. |
| 4.3 | Safe path resolver, traversal protection, symlink check, allowed roots, temp+atomic rename, access denied error, free space check | ✅ All implemented | Complete | `gta6rp-installation` fully implemented. `free_space_info()` and `ensure_free_space()` added for free space checking. Free space check integrated into launcher startup and displayed in UI notice. |

**Checkpoint 4:** Closed — Core tested without UI (22 tests pass). Local state survives restart (`state_survives_reopen` test passes). UI receives events without polling (EventBus + `invoke_from_event_loop` bridge). File operations limited by `SafeRoot` + `RelativePath` validation. Recovery tests pass (`recovers_interrupted_tasks_only` test passes).

---

### PHASE 5 — Download Engine

| MVP | Planned | Actual | Status | Notes |
|-----|---------|--------|--------|-------|
| 5.1 | Download one HTTPS file, streaming, status check, size check, progress, SHA-256, atomic move | ❌ Not implemented | Missing | No download engine crate exists |
| 5.2 | Queue, parallelism, pause/resume/cancel, retry, rate limit, SQLite persistence, resume after restart | ❌ Not implemented | Missing | `local-store` has `download_tasks` table but no downloader writes to it |
| 5.3 | Network loss, HTTP errors, Range, partial file, disk full, corrupted file, RAM limit, single destination | ❌ Not implemented | Missing | — |

**Checkpoint 5:** Not closed — No download engine exists.

---

### PHASE 6 — Manifest, Verification & Installation

| MVP | Planned | Actual | Status | Notes |
|-----|---------|--------|--------|-------|
| 6.1 | Schema v1, file entries, platform/channel, manifest ID, signature metadata, Serde validation | ❌ Not implemented | Missing | No manifest engine crate |
| 6.2 | Test key pair, manifest signature, signature verification, rejection | ❌ Not implemented | Missing | Ed25519 mentioned in PLAN.md but no implementation |
| 6.3 | Local state comparison, missing/changed/extra files, install plan, disk space check | ❌ Not implemented | Missing | `SafeRoot` exists but no diff/comparison logic |
| 6.4 | Staging, pre-apply check, journal, safe apply, recovery, rollback | ❌ Not implemented | Missing | `SafeRoot::write_atomic()` exists but no journal/rollback |

**Checkpoint 6:** Not closed — No manifest/verification/installation implementation.

---

### PHASE 7 — Game Manager

| MVP | Planned | Actual | Status | Notes |
|-----|---------|--------|--------|-------|
| 7.1 | Synthetic fixture, directory structure, test manifest, test app, FixtureGameProvider | ❌ Not implemented | Missing | — |
| 7.2 | Directory selection, availability check, disk space, state display, install/repair/update | ❌ Not implemented | Missing | `SafeRoot::open()` exists but not wired to UI |
| 7.3 | Launch test app, PID, monitoring, completion handling, single instance, errors | ❌ Not implemented | Missing | No process manager crate |

**Checkpoint 7:** Not closed — No game manager implementation.

---

### PHASE 8 — Launcher Updates & Releases

| MVP | Planned | Actual | Status | Notes |
|-----|---------|--------|--------|-------|
| 8.1 | Release metadata, versioning, channels, min version, release notes, signature | ❌ Not implemented | Missing | `launcher_releases` table exists but no API routes |
| 8.2 | Update check at startup, download, signature check, handoff to updater, rollback | ❌ Not implemented | Missing | — |
| 8.3 | Windows installer, uninstaller, upgrade, publisher signature, clean VM test | ❌ Not implemented | Missing | — |

**Checkpoint 8:** Not closed — No update mechanism.

---

### PHASE 9 — Servers, News & Settings

| MVP | Planned | Actual | Status | Notes |
|-----|---------|--------|--------|-------|
| 9.1 | Server CRUD, public list, card, region/version, status/players, maintenance, caching | ❌ Not implemented | Missing | `servers` table exists but no API routes; mock data in launcher |
| 9.2 | News CRUD, publication, covers, dates, pagination, launcher display | ❌ Not implemented | Missing | `news` table exists but no API routes; mock data in launcher |
| 9.3 | Install path, language, auto-start, close behavior, speed limit, parallelism, notifications, persistence | ❌ Not implemented | Missing | `user_settings` table exists but no API routes; `local-store` has settings but launcher doesn't use it |

**Checkpoint 9:** Not closed — No backend catalog routes; launcher settings not persisted.

---

### PHASE 10 — Admin API & Admin Panel

| MVP | Planned | Actual | Status | Notes |
|-----|---------|--------|--------|-------|
| 10.1 | Permissions, user/role/server/news/release management, manifest publishing, audit logs | ❌ Not implemented | Missing | RBAC tables exist but unused |
| 10.2 | Manifest tool CLI | ❌ Not implemented | Missing | — |
| 10.3 | Admin login, dashboard, users, servers, news, releases, manifests, audit logs | ❌ Not implemented | Missing | — |

**Checkpoint 10:** Not closed — No admin implementation.

---

### PHASE 11 — QA, Security & Release Candidate

| MVP | Planned | Actual | Status | Notes |
|-----|---------|--------|--------|-------|
| 11.1 | Domain, manifest, downloader, PostgreSQL, HTTP, Windows filesystem, UI state, E2E tests | ❌ Not implemented | Missing | Domain tests exist; all others missing |
| 11.2 | Failure injection (network loss, crash, disk full, corrupted manifest, etc.) | ❌ Not implemented | Missing | — |
| 11.3 | Performance (cold start, RAM, CPU, SHA-256, throughput, UI responsiveness) | ❌ Not implemented | Missing | — |
| 11.4 | Release build, signing, installer, upgrade, VM test, release notes, rollback | ❌ Not implemented | Missing | — |

**Checkpoint 11:** Not closed — No QA/security/release work.

---

## 2. Deviations from PLAN.md

| Deviation | Location | Impact |
|-----------|----------|--------|
| `gta6rp-application` implemented but unused by launcher | PLAN.md MVP 4.1 | Core pattern exists but launcher still uses inline `Cell<Timer>` |
| `gta6rp-local-store` implemented but unused by launcher | PLAN.md MVP 4.2 | Settings persistence exists but launcher shows "NOT SAVED" |
| `gta6rp-installation` implemented but unused by launcher | PLAN.md MVP 4.3 | Safe file writes exist but no installation flow |
| `Page<T>` / `Pagination` defined but never consumed | PLAN.md MVP 2.3 | Dead code in `contracts.rs` |
| No shared contracts crate | PLAN.md section 4 | Launcher duplicates auth DTOs in `auth_client.rs` |
| API has zero workspace crate dependencies | PLAN.md architecture | API is isolated island; no type sharing with launcher |
| Checkpoint 0 Windows validation incomplete | PLAN.md PHASE 0 | Environment limitation (Linux only) |
| Checkpoint 2 clean migration check incomplete | PLAN.md MVP 2.2 | Environment limitation (no PostgreSQL) |
| Checkpoint 3 RBAC incomplete | PLAN.md MVP 3.3 | Not implemented |

---

## 3. Missing Tasks Within Existing Phases (Not New Phases)

These are gaps within existing PLAN.md phases that need to be addressed:

| Phase | Missing Task | Why It Matters |
|-------|-------------|----------------|
| 2.2 | Clean migration check against live PostgreSQL 18.7 | Required for Checkpoint 2 |
| 2.3 | Contract tests | Required for Checkpoint 2 |
| 3.3 | Roles, permissions, server-side auth check, admin API protection, audit events | Required for Checkpoint 3 |
| 4.1 | Use cases wired to launcher | Required for Checkpoint 4 |
| 4.1 | Event channel UI/core — Slint bridge | Required for Checkpoint 4 |
| 4.3 | Free space check | Listed as missing in PLAN.md |
| 5.1–5.3 | Entire download engine | Required for Checkpoint 5 |
| 6.1–6.4 | Entire manifest/verification/installation | Required for Checkpoint 6 |
| 7.1–7.3 | Entire game manager | Required for Checkpoint 7 |
| 8.1–8.3 | Entire update mechanism | Required for Checkpoint 8 |
| 9.1–9.3 | Backend catalog routes, settings persistence | Required for Checkpoint 9 |
| 10.1–10.3 | Entire admin API | Required for Checkpoint 10 |
| 11.1–11.4 | Entire QA/security/release | Required for Checkpoint 11 |

---

## 4. Technical Dependencies and Blockers

| Blocker | Affects | Resolution Required |
|---------|---------|---------------------|
| No PostgreSQL/Docker in environment | Phase 2.2 clean migration check, Phase 11.1 PostgreSQL integration tests | Environment provisioning or defer to CI |
| No Windows in environment | Phase 0 spike, Phase 6.4 Windows tests, symlink/junction tests | CI Windows runner (exists in `.github/workflows/ci.yml`) |
| Launcher has no Tokio dependency | Phase 4.1 — `ApplicationCore::spawn()` panics outside Tokio | Phase C integration decision (Variant A vs B) |
| `Page<T>` / `Pagination` dead code | Phase 2.3 — contract tests | Either wire to routes or remove |
| No shared contracts crate | API ↔ launcher type sharing | Either create `crates/contracts` or accept duplication |
| Zero launcher tests | Phase 11.1 — UI state tests | Need to add launcher test suite |
| `LocalStore` not used by launcher | Phase 4.2 — settings persistence | Phase C integration |
| `ApplicationCore` not used by launcher | Phase 4.1 — application state | Phase C integration |

---

## 5. Recommended Sequence Within Existing Phases

The following order respects the dependency table in PLAN.md section 12:

1. **Phase 4.1 completion** — Wire `ApplicationCore` + `EventBus` to Slint (Phase C, requires architectural decision)
2. **Phase 4.2 completion** — Wire `LocalStore` to launcher (settings persistence, install root, active manifest)
3. **Phase 4.3 completion** — Free space check (missing item)
4. **Phase 2.2/2.3 completion** — Clean migration check, contract tests (when PostgreSQL available)
5. **Phase 3.3 completion** — RBAC, permissions, admin protection, audit
6. **Phase 5** — Download engine (depends on Phase 4)
7. **Phase 6** — Manifest/verification/installation (depends on Phase 5)
8. **Phase 7** — Game manager (depends on Phase 6)
9. **Phase 8** — Launcher updates (depends on Phase 2, 4)
10. **Phase 9** — Servers, news, settings (depends on Phase 1, 2)
11. **Phase 10** — Admin API (depends on Phase 2, 6)
12. **Phase 11** — QA, security, release candidate (depends on all)

---

## 6. Reference to Other Audit Documents

- `FULL_WORKSPACE_AUDIT.md` — Complete crate inventory and implementation status
- `ARCHITECTURE_MAP.md` — Dependency graph and layer mapping
- `EXECUTION_FLOWS.md` — Detailed scenario call sequences
- `RUST_QUALITY_AUDIT.md` — Code quality assessment
- `TEST_COVERAGE_AUDIT.md` — Test coverage gaps by phase
- `GRAPHIFY_ANALYSIS.md` — Graph validation (76 isolated nodes correlate with unimplemented phases)
- `DECISIONS_REQUIRED.md` — Architectural decisions needing approval
