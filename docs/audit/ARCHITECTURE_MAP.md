# ARCHITECTURE MAP

**Date:** 2026-11-23  
**Status:** Research only — no source code modified  
**Sources:** Source code inspection, Cargo.toml dependency graph, Graphify analysis (640 nodes, 1257 edges, 30 communities)

---

## 1. Dependency Graph (Cargo-level)

```
gta6rp-domain (zero deps)
├── gta6rp-application (depends on domain, tokio)
├── gta6rp-installation (depends on domain)
├── gta6rp-local-store (depends on domain, application, sqlx-sqlite)
└── gta6rp-launcher (depends on domain, reqwest, serde, slint, keyring[windows])

gta6rp-api (zero workspace deps — only external crates: axum, sqlx-postgres, argon2, etc.)
```

**Key observation:** `gta6rp-api` is an isolated island — no intra-workspace dependencies.  
**Key observation:** `gta6rp-launcher` only depends on `gta6rp-domain` — does not use `application`, `local-store`, or `installation`.

---

## 2. Layer Mapping (per AGENTS.md rules)

| Layer | Intended Components | Actual Components | Compliance |
|-------|--------------------|-------------------|------------|
| UI | Slint window, UI models | `apps/launcher/src/main.rs`, `ui/app.slint`, `ui/components.slint`, `ui/theme/tokens.slint` | ✅ UI callbacks are I/O-free |
| UI Models / View State | `state.rs` mapping | `apps/launcher/src/state.rs` | ✅ Pure `ScreenState → &str` mapping |
| Application Services | Account, Installation, Download, Update, Catalog, Diagnostics | None implemented as services | ❌ Not implemented |
| Domain / Application Core | Game Provider, Manifest Engine, Download Engine, Verification Engine, Process Manager, Local State | `crates/domain/` (screen, path, settings), `crates/application/` (core, events, cancellation, shutdown, use_cases) | ⚠️ Core exists but unused by launcher |
| Infrastructure | HTTP/CDN, SQLite, Filesystem, OS Integration, Logging | `crates/local-store/` (SQLite), `crates/installation/` (filesystem), `auth_client.rs` (HTTP) | ⚠️ Exists but not wired into launcher |

---

## 3. Component Detail Map

### 3.1 `gta6rp-domain` — Pure domain (zero infra deps)

```
┌─────────────────────────────────────────┐
│         gta6rp-domain                   │
│  ┌─────────┐ ┌─────────┐ ┌─────────┐   │
│  │  path   │ │ screen  │ │ settings│   │
│  └─────────┘ └─────────┘ └─────────┘   │
│ RelativePath  ScreenState  Settings    │
│ PathError     ScreenAction SettingsError│
│ join_under    InvalidTransition validate()│
└─────────────────────────────────────────┘
        ▲         ▲         ▲
        │         │         │
    consumers   consumers   consumers
```

**Consumed by:** `gta6rp-application`, `gta6rp-installation`, `gta6rp-local-store`, `gta6rp-launcher`  
**No dependencies on:** Slint, HTTP, SQLx, filesystem

---

### 3.2 `gta6rp-application` — Application core (unused by launcher)

```
┌─────────────────────────────────────────┐
│       gta6rp-application                │
│  ┌─────────┐ ┌─────────┐ ┌─────────┐   │
│  │  core   │ │ events  │ │cancellation│ │
│  └─────────┘ └─────────┘ └─────────┘   │
│  ┌─────────┐ ┌─────────┐ ┌─────────┐   │
│  │ shutdown│ │  error  │ │ use_cases │ │
│  └─────────┘ └─────────┘ └─────────┘   │
│ ApplicationCore  EventBus  CancellationToken│
│ CoreHandle       AppEvent   ShutdownSupervisor│
│ CoreControl      StartupFailure LocalStatePort│
│                  UseCaseError  StartupReport│
│                  load_settings() update_settings()│
│                  prepare_startup()             │
└─────────────────────────────────────────┘
        ▲
    consumed by gta6rp-local-store only
```

**NOT consumed by:** `gta6rp-launcher` — this is the primary architectural gap

---

### 3.3 `gta6rp-local-store` — SQLite persistence (unused by launcher)

```
┌─────────────────────────────────────────┐
│       gta6rp-local-store                │
│  ┌─────────┐ ┌─────────┐ ┌─────────┐   │
│  │  store  │ │  model  │ │  error  │   │
│  └─────────┘ └─────────┘ └─────────┘   │
│ LocalStore   DownloadTask   StoreError│
│   └─ implements LocalStatePort          │
│   Settings (from domain)   DownloadState│
└─────────────────────────────────────────┘
        ▲
    NOT consumed by launcher
```

---

### 3.4 `gta6rp-installation` — Safe filesystem (completely unused)

```
┌─────────────────────────────────────────┐
│      gta6rp-installation                │
│  ┌─────────────────────────────────┐   │
│  │       safe_root                 │   │
│  │  SafeRoot  InstallError         │   │
│  └─────────────────────────────────┘   │
└─────────────────────────────────────────┘
        ▲
    NOT consumed by any crate
```

---

### 3.5 `gta6rp-api` — Isolated backend

```
┌─────────────────────────────────────────┐
│           gta6rp-api                    │
│  ┌─────────┐ ┌─────────┐ ┌─────────┐   │
│  │  lib    │ │  auth   │ │contracts│   │
│  └─────────┘ └─────────┘ └─────────┘   │
│ Config   register/login/ Page<T>       │
│ app()   refresh/logout   Pagination    │
│ serve()  current_user   ErrorEnvelope   │
│ health()                        (pub(crate))│
└─────────────────────────────────────────┘
        ▲
    NO workspace crate dependents
```

---

### 3.6 `gta6rp-launcher` — Slint UI (uses only domain)

```
┌─────────────────────────────────────────┐
│       gta6rp-launcher                   │
│  ┌─────────┐ ┌─────────┐ ┌─────────┐   │
│  │  main   │ │  state  │ │auth_client│ │
│  └─────────┘ └─────────┘ └─────────┘   │
│ Rc<Cell<ScreenState>>  title/detail/  bind(), login(), │
│ Timer + apply_action_weak() badge/notice  refresh(), logout│
│ schedule_action()                         restore_session()│
│                                           schedule_refresh()│
└─────────────────────────────────────────┘
        ▲
    only depends on gta6rp-domain
```

---

## 4. Runtime Call Flow (Launcher)

```
User clicks button
    │
    ▼
Slint callback (main.rs:22)
    │
    ├─► apply_action_weak() → ScreenState::apply() → next state
    │   └─► apply_state() → window.set_*() (Slint properties)
    │
    ├─► schedule_action() → Timer::single_shot → delayed action
    │
    └─► auth_client::bind() callbacks → std::thread::spawn
        │
        ├─► reqwest::blocking::Client → HTTP → API at 127.0.0.1:8080
        │   └─► slint::invoke_from_event_loop → UI update
        │
        └─► keyring (Windows) → Credential Manager → token storage
```

**No call to:** `ApplicationCore::spawn()`, `CoreControl::apply()`, `LocalStore::open()`, `LocalStatePort`, `SafeRoot::write_atomic()`

---

## 5. Runtime Call Flow (API)

```
HTTP request → Axum Router → auth handler → SQLx query → PostgreSQL
    │
    ├─► Middleware: request ID, CORS, rate limiting, tracing
    ├─► bcrypt/argon2 hashing via spawn_blocking
    ├─► SHA-256 token hashing
    └─► JSON response with ErrorEnvelope
```

**No call to:** Any workspace crate

---

## 6. Graphify Community Analysis

Graphify identified 30 communities. Key findings:

### God Nodes (most connected)
| Rank | Node | Degree | File |
|------|------|--------|------|
| 1 | `ScreenState` | 38 | `crates/domain/src/screen.rs` |
| 2 | `StoreError` | 28 | `crates/local-store/src/error.rs` |
| 3 | `ScreenAction` | 27 | `crates/domain/src/screen.rs` |
| 4 | `InstallError` | 20 | `crates/installation/src/safe_root.rs` |
| 5-10 | `login()`, `refresh()`, `ApiError`, `AppEvent`, `PathError`, `DownloadState` | 16 each | Various |

### Communities with low cohesion (<0.10)
- Community 0 "API authentication": 0.07 — contains both application use cases AND API auth concepts (misclassified by Graphify due to shared naming patterns)
- Community 4 "Domain screen states": 0.07 — contains API app & config (Graphify misclassified due to `Config`/`app()` naming)
- Community 6 "Application cancellation": 0.07 — contains domain path rules (Graphify misclassified)

**Graphify limitation:** Some communities mix unrelated concepts because AST-based naming similarities create false edges. Manual code review confirms these are not real architectural relationships.

### Isolated nodes (76)
- `gta6rp-api` — degree 0 (completely isolated crate)
- `gta6rp-launcher` — degree 1 (only `depends_on gta6rp-domain` from Cargo.toml)
- `Page<T>` — degree 1 (only `.new()` method)
- `gta6rp-application` — degree 1 (only `depends_on gta6rp-domain` from Cargo.toml)

---

## 7. Architectural Violations Identified

| Violation | Location | Severity |
|-----------|----------|----------|
| Launcher does not use ApplicationCore | `apps/launcher/src/main.rs` | High — core pattern unused |
| Launcher does not use LocalStore | `apps/launcher/src/main.rs` | High — no persistence |
| Launcher does not use Installation | `apps/launcher/src/main.rs` | High — no safe file writes |
| gta6rp-api has zero workspace deps | `apps/api/Cargo.toml` | Medium — isolated island |
| Page<T>/Pagination dead code | `apps/api/src/contracts.rs:51` | Low — unused types |
| No contracts crate | PLAN.md section 4 | Medium — DTOs not shared |
| Launcher defines duplicate auth DTOs | `apps/launcher/src/auth_client.rs:23-49` | Medium — duplication |
| Windows symlink tests are `#[cfg(unix)]` | `crates/installation/src/safe_root.rs:412` | Medium — Windows validation untested on Linux |

---

## 8. Reference to Other Audit Documents

- `FULL_WORKSPACE_AUDIT.md` — Complete crate inventory and status
- `EXECUTION_FLOWS.md` — Detailed scenario call sequences
- `RUST_QUALITY_AUDIT.md` — Rust/async/ownership/concurrence audit
- `TEST_COVERAGE_AUDIT.md` — Test coverage gaps
- `GRAPHIFY_ANALYSIS.md` — Graphify results and validation
- `ROADMAP.md` — Prioritized development plan
- `DECISIONS_REQUIRED.md` — Architectural decisions needing approval
