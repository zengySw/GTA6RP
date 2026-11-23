# GRAPHIFY ANALYSIS

**Date:** 2026-11-23  
**Status:** Research only — no source code modified  
**Graph:** 640 nodes, 1257 edges, 30 communities  
**Extraction:** 98% EXTRACTED, 2% INFERRED, 0% AMBIGUOUS

---

## 1. Graph Summary

| Metric | Value |
|--------|-------|
| Total nodes | 640 |
| Total edges | 1257 |
| Communities | 30 |
| Extraction confidence | 98% EXTRACTED, 2% INFERRED |
| Average INFERRED confidence | 0.83 |
| Dangling-endpoint edges | 126 (expected — references to external crates/stdlib) |
| Collapsed edges | 80 (undirected) |

**Graphify outputs:** `graphify-out/graph.json`, `graphify-out/GRAPH_REPORT.md`, `graphify-out/manifest.json`, `graphify-out/cost.json`

---

## 2. God Nodes (Most Connected)

| Rank | Node | Degree | File | Role |
|------|------|--------|------|------|
| 1 | `ScreenState` | 38 | `crates/domain/src/screen.rs` | Central domain state enum |
| 2 | `StoreError` | 28 | `crates/local-store/src/error.rs` | Local store error type |
| 3 | `ScreenAction` | 27 | `crates/domain/src/screen.rs` | Central domain action enum |
| 4 | `InstallError` | 20 | `crates/installation/src/safe_root.rs` | Installation error type |
| 5 | `login()` | 16 | `apps/api/src/auth.rs` | API login handler |
| 6 | `refresh()` | 16 | `apps/api/src/auth.rs` | API refresh handler |
| 7 | `ApiError` | 16 | `apps/api/src/lib.rs` | API error type |
| 8 | `AppEvent` | 16 | `crates/application/src/events.rs` | Application event enum |
| 9 | `PathError` | 16 | `crates/domain/src/path.rs` | Domain path error type |
| 10 | `DownloadState` | 16 | `crates/local-store/src/model.rs` | Download lifecycle enum |

**Assessment:** `ScreenState` and `ScreenAction` being the top two nodes confirms the domain state machine is the central abstraction. `StoreError` at #2 reflects the local store's comprehensive error type.

---

## 3. Community Analysis

Graphify identified 30 communities. Key communities and their cohesion scores:

| Community | Label | Nodes | Cohesion | Assessment |
|-----------|-------|-------|----------|------------|
| 24 | Crate names | 5 | 0.40 | Workspace crate dependency nodes |
| 20 | LocalStore handle | 3 | 0.29 | `LocalStore`, `Settings`, `SettingsRow`, `StateRow` |
| 19 | UI state mapping | 6 | 0.48 | `apply_state()`, `title()`, `badge()`, etc. |
| 13 | CI pipeline | 12 | 0.25 | GitHub Actions workflow nodes |
| 16 | Screen state machine tests | 14 | 0.24 | Domain screen tests |
| 18 | Launcher mock data | 12 | 0.15 | Mock data, cards, models |
| 0 | API authentication | 28 | 0.07 | **Low cohesion** — Graphify mixed application use cases with API auth |
| 1 | API app & config | 29 | 0.08 | **Low cohesion** — Graphify mixed core reducer with API config |
| 6 | Application cancellation | 24 | 0.07 | **Low cohesion** — Graphify mixed domain path rules with cancellation |

**Graphify limitation:** Communities 0, 1, and 6 show low cohesion because Graphify's AST-based extraction creates edges based on naming similarities (e.g., `Config`, `app()`) that don't represent real architectural relationships. Manual code review confirms these are not actual coupling issues.

---

## 4. Surprising Connections

| Source | Target | Relation | Source Files | Assessment |
|--------|--------|----------|--------------|------------|
| Initial PostgreSQL database schema | PostgreSQL database | semantically_similar_to (INFERRED) | SCHEMA.md → README.md | Expected — same concept |
| Launcher architecture rules | Dependency direction rule | semantically_similar_to (INFERRED) | AGENTS.md → OVERVIEW.md | Expected — same rule stated twice |
| next_event() | timeout | references (INFERRED) | core.rs → docker-compose.yml | **False positive** — Graphify matched `timeout` in test code to `timeout` in docker-compose |
| Slint desktop UI framework | Cityscape hero image | references (EXTRACTED) | README.md → SVG asset | Expected — documentation reference |
| schedule_action() | ScreenAction | references (EXTRACTED) | main.rs → screen.rs | Expected — launcher uses domain actions |

**False positive identified:** `next_event()` → `timeout` is a Graphify extraction artifact. The `timeout` in `core.rs` tests refers to `tokio::time::timeout`, not docker-compose's `timeout`. This should not be interpreted as a real architectural connection.

---

## 5. Isolated Nodes (76)

The graph reports 76 nodes with ≤1 connection. Key isolated nodes:

| Node | Degree | File | Assessment |
|------|--------|------|------------|
| `gta6rp-api` | 0 | `apps/api/Cargo.toml` | **Real isolation** — no workspace crate dependencies |
| `gta6rp-launcher` | 1 | `apps/launcher/Cargo.toml` | **Real isolation** — only depends on `gta6rp-domain` |
| `Page<T>` | 1 | `apps/api/src/contracts.rs` | **Dead code** — defined but never consumed |
| `gta6rp-application` | 1 | `crates/application/Cargo.toml` | **Unused by launcher** — only Cargo dependency edge |
| `SignedIn`, `Registered` | ≤1 | `auth_client.rs` | Enum variants — expected leaf nodes |

---

## 6. Graph vs. Reality Verification

### 6.1 Edges confirmed by source code

| Graph Edge | Source | Verified |
|------------|--------|----------|
| `schedule_action()` → `ScreenAction` | `main.rs:35-63` | ✅ Yes — `schedule_action()` takes `ScreenAction` parameter |
| `ScreenState` → 11 variants | `screen.rs:12-24` | ✅ Yes — enum variants |
| `ScreenAction` → 12 variants | `screen.rs:28-41` | ✅ Yes — enum variants |
| `AppEvent` → 5 variants | `events.rs:16-30` | ✅ Yes — enum variants |
| `LocalStore` → `LocalStatePort` | `store.rs:235-253` | ✅ Yes — impl block |
| `gta6rp-launcher` → `gta6rp-domain` | `launcher/Cargo.toml:10` | ✅ Yes — Cargo dependency |

### 6.2 Missing edges (graph does not show real runtime connections)

| Real Connection | Why Graphify Misses It |
|-----------------|------------------------|
| Launcher → API via HTTP (`reqwest::blocking`) | String URL, not a compile-time dependency |
| Launcher → `ApplicationCore` | Not imported — no code connection exists |
| Launcher → `LocalStore` | Not imported — no code connection exists |
| API → shared DTOs crate | No shared crate exists |

### 6.3 Graph false positives

| Graph Edge | Why It's Incorrect |
|------------|-------------------|
| `next_event()` → `timeout` (docker-compose) | `timeout` in test code is `tokio::time::timeout`, not docker-compose config |
| Community 0 mixing API auth with application use cases | Naming similarity, not real coupling |
| Community 1 mixing core reducer with API config | Naming similarity (`Config`, `app()`), not real coupling |

---

## 7. Graph Limitations

1. **AST-only extraction:** Graphify extracts code structurally (AST) and documents semantically (manual). It cannot detect runtime dependencies like HTTP URLs or string-based configuration.

2. **No semantic edge for "unused":** The graph shows `Page<T>` with 1 edge (to `.new()`), but cannot show that it's never called by any route handler. This requires manual code review.

3. **Community misclassification:** Low-cohesion communities (0, 1, 6) group unrelated concepts by naming similarity, not architectural coupling.

4. **External crate references:** 126 dangling edges are references to external crates not in the workspace — expected, not a problem.

---

## 8. Reference to Other Audit Documents

- `FULL_WORKSPACE_AUDIT.md` — Complete crate inventory
- `ARCHITECTURE_MAP.md` — Dependency graph and layer mapping (validated against Graphify)
- `EXECUTION_FLOWS.md` — Runtime call sequences (some not visible in static graph)
- `RUST_QUALITY_AUDIT.md` — Code quality details
- `TEST_COVERAGE_AUDIT.md` — Test coverage gaps (76 isolated nodes correlate with untested components)
- `ROADMAP.md` — Prioritized plan based on graph findings
