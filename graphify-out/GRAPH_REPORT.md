# Graph Report - gta6rp  (2026-11-23)

## Corpus Check
- Corpus is ~40,528 words - fits in a single context window. You may not need a graph.

## Summary
- 686 nodes · 1344 edges · 40 communities (24 shown, 16 thin omitted)
- Extraction: 98% EXTRACTED · 2% INFERRED · 0% AMBIGUOUS · INFERRED: 25 edges (avg confidence: 0.83)
- Token cost: 0 input · 0 output

## Community Hubs (Navigation)
- Community 0
- Community 1
- Community 2
- Community 3
- Community 4
- Community 5
- Community 6
- Community 7
- Community 8
- Community 9
- Community 10
- Community 11
- Community 12
- Community 13
- Community 14
- Community 15
- Community 16
- Community 17
- Community 18
- Community 19
- Community 20
- Community 21
- Community 22
- Community 23
- Community 24
- Community 25
- Community 26
- Community 27
- Community 29
- Community 30
- Community 31
- Community 33
- Community 34
- Community 35
- Community 36
- Community 37
- Community 38
- Community 39

## God Nodes (most connected - your core abstractions)
1. `ScreenState` - 37 edges
2. `StoreError` - 28 edges
3. `ScreenAction` - 25 edges
4. `InstallError` - 23 edges
5. `Settings` - 18 edges
6. `AppEvent` - 17 edges
7. `LocalStore` - 17 edges
8. `login()` - 16 edges
9. `refresh()` - 16 edges
10. `ApiError` - 16 edges

## Surprising Connections (you probably didn't know these)
- `Initial PostgreSQL database schema` --semantically_similar_to--> `PostgreSQL database`  [INFERRED] [semantically similar]
  docs/database/SCHEMA.md → README.md
- `Launcher architecture rules` --semantically_similar_to--> `Dependency direction rule`  [INFERRED] [semantically similar]
  AGENTS.md → docs/architecture/OVERVIEW.md
- `next_event()` --references--> `timeout`  [INFERRED]
  crates/application/src/core.rs → infrastructure/compose/docker-compose.yml
- `Slint desktop UI framework` --references--> `Cityscape hero image for Slint UI`  [EXTRACTED]
  README.md → apps/launcher/ui/assets/cityscape.svg
- `run_event_loop()` --references--> `EventBus`  [EXTRACTED]
  apps/launcher/src/app_bridge.rs → crates/application/src/events.rs

## Import Cycles
- 1-file cycle: `crates/application/src/events.rs -> crates/application/src/events.rs`
- 1-file cycle: `crates/domain/src/screen.rs -> crates/domain/src/screen.rs`
- 1-file cycle: `crates/application/src/use_cases.rs -> crates/application/src/use_cases.rs`
- 1-file cycle: `crates/domain/src/settings.rs -> crates/domain/src/settings.rs`
- 1-file cycle: `crates/installation/src/safe_root.rs -> crates/installation/src/safe_root.rs`
- 1-file cycle: `crates/local-store/src/store.rs -> crates/local-store/src/store.rs`
- 1-file cycle: `crates/application/src/core.rs -> crates/application/src/core.rs`
- 1-file cycle: `crates/application/src/cancellation.rs -> crates/application/src/cancellation.rs`
- 1-file cycle: `crates/application/src/shutdown.rs -> crates/application/src/shutdown.rs`
- 1-file cycle: `crates/domain/src/path.rs -> crates/domain/src/path.rs`
- 1-file cycle: `crates/local-store/src/model.rs -> crates/local-store/src/model.rs`

## Hyperedges (group relationships)
- **Architecture decision record family** — adr0001_slint_decision, adr0002_backend_decision, adr0003_secret_storage, adr0004_local_state, adr0005_free_space [EXTRACTED 1.00]
- **Audit document family** — audit_full_workspace, docs_audit_architecture_map, docs_audit_execution_flows, audit_rust_quality, audit_test_coverage, docs_audit_graphify_analysis [EXTRACTED 1.00]
- **Launcher technology stack** — readme_slint_ui, readme_axum_api, readme_postgresql, readme_sqlite_local_state, readme_rust_first [EXTRACTED 1.00]

## Communities (40 total, 16 thin omitted)

### Community 0 - "Community 0"
Cohesion: 0.05
Nodes (36): database_path(), database_path_returns_valid_path(), LocalStateError, Installation, Io, Store, UseCase, LocalStateHandle (+28 more)

### Community 1 - "Community 1"
Cohesion: 0.06
Nodes (39): join_under(), JoinError, EscapesRoot, RelativePath, create_dir_all_creates_nested_directories(), ensure_free_space(), ensure_free_space_fails_when_not_enough_space(), ensure_free_space_succeeds_when_enough_space() (+31 more)

### Community 2 - "Community 2"
Cohesion: 0.08
Nodes (37): ACCESS_TOKEN_LIFETIME_SECONDS, AccountCredentials, api_error(), bearer_token(), create_session(), current_user(), internal_error(), is_token_format() (+29 more)

### Community 3 - "Community 3"
Cohesion: 0.07
Nodes (14): app(), Config, env_or(), error_for_request(), health(), HealthResponse, method_not_allowed(), not_found() (+6 more)

### Community 4 - "Community 4"
Cohesion: 0.08
Nodes (35): ACCESS_REFRESH_SECONDS, api_url(), AuthFailure, Network, Service, Unauthorized, bind(), Completion (+27 more)

### Community 5 - "Community 5"
Cohesion: 0.08
Nodes (21): accepts_ordinary_relative_paths(), components_round_trip(), is_reserved_device_name(), join_always_produces_a_path_under_root(), join_stays_under_root(), MAX_COMPONENT_LENGTH, MAX_PATH_LENGTH, PathError (+13 more)

### Community 6 - "Community 6"
Cohesion: 0.08
Nodes (16): detection_and_install_chain(), InvalidTransition, ScreenAction, .ALL, BeginSetup, DetectionFinished, DownloadCompleted, DownloadStarted (+8 more)

### Community 7 - "Community 7"
Cohesion: 0.13
Nodes (10): already_cancelled_token_resolves_immediately(), CancellationToken, clones_share_one_signal(), starts_running_and_cancels(), waiting_resolves_after_cancel_from_another_task(), cancels_and_joins_cooperative_tasks(), empty_supervisor_shuts_down_cleanly(), reports_task_failure_on_panic() (+2 more)

### Community 8 - "Community 8"
Cohesion: 0.10
Nodes (25): POSTGRES_DB, POSTGRES_PASSWORD, POSTGRES_USER, interval, retries, start_period, test, timeout (+17 more)

### Community 9 - "Community 9"
Cohesion: 0.10
Nodes (20): ADR-0001 Slint desktop UI framework, ADR-0002 Backend architecture, ADR-0003 Local secret storage, ADR-0004 Local state persistence, Cityscape hero image for Slint UI, Architecture overview, Initial PostgreSQL database schema, Database tables and relationships (+12 more)

### Community 10 - "Community 10"
Cohesion: 0.17
Nodes (15): AppEvent, ActionRejected, ScreenChanged, Shutdown, StartupCompleted, StartupFailed, delivers_events_to_subscribers(), each_subscriber_receives_independently() (+7 more)

### Community 11 - "Community 11"
Cohesion: 0.13
Nodes (18): apply_ui_state(), action_label(), badge(), detail(), notice(), title(), ScreenState, Detecting (+10 more)

### Community 12 - "Community 12"
Cohesion: 0.31
Nodes (8): ApplicationCore, applies_valid_transition_and_emits_event(), COMMAND_QUEUE_CAPACITY, CoreHandle, next_event(), preserves_command_order(), rejects_invalid_action_without_changing_state(), shutdown_stops_the_reducer()

### Community 13 - "Community 13"
Cohesion: 0.14
Nodes (8): DownloadState, Cancelled, Completed, Downloading, Failed, Paused, Queued, Verifying

### Community 14 - "Community 14"
Cohesion: 0.25
Nodes (11): active_manifest_round_trip(), inserts_and_lists_tasks(), local_state_port_prepare_startup_on_fresh_store(), local_state_port_recover_interrupted_through_trait(), local_state_port_settings_persistence_through_trait(), recovers_interrupted_tasks_only(), rejects_invalid_task(), sample_task() (+3 more)

### Community 15 - "Community 15"
Cohesion: 0.16
Nodes (3): run_event_loop(), apply_settings_to_ui(), main()

### Community 16 - "Community 16"
Cohesion: 0.16
Nodes (5): optional_i64(), StateRow, TEST_COUNTER, TestDir, to_i64()

### Community 17 - "Community 17"
Cohesion: 0.21
Nodes (6): apply(), Command, Apply, Shutdown, CoreControl, run()

### Community 18 - "Community 18"
Cohesion: 0.24
Nodes (6): StoreError, Corrupt, Database, InvalidInput, Io, Migration

### Community 19 - "Community 19"
Cohesion: 0.21
Nodes (4): LocalStore, Settings, settings_round_trip(), SettingsRow

### Community 20 - "Community 20"
Cohesion: 0.15
Nodes (12): jobs, rust, name, on, pull_request, push, runs-on, steps (+4 more)

### Community 21 - "Community 21"
Cohesion: 0.29
Nodes (7): DownloadTask, is_lowercase_sha256(), now_millis(), queued_task_starts_with_zero_progress(), rejects_unsafe_or_inconsistent_task(), valid_task(), valid_task_passes_validation()

### Community 23 - "Community 23"
Cohesion: 0.29
Nodes (5): ApplicationError, CommandClosed, CommandQueueFull, ShutdownTimeout, TaskFailed

### Community 24 - "Community 24"
Cohesion: 0.80
Nodes (5): gta6rp-application, gta6rp-domain, gta6rp-installation, gta6rp-launcher, gta6rp-local-store

## Knowledge Gaps
- **85 isolated node(s):** `gta6rp-api`, `Page<T>`, `SignedIn`, `Registered`, `SignedOut` (+80 more)
  These have ≤1 connection - possible missing edges or undocumented components. (Counts symbols only; 237 node(s) total have ≤1 connection when file, concept and rationale nodes are included.)
- **16 thin communities (<3 nodes) omitted from report** — run `graphify query` to explore isolated nodes.

## Suggested Questions
_Questions this graph is uniquely positioned to answer:_

- **Why does `ScreenState` connect `Community 11` to `Community 6`, `Community 10`, `Community 12`, `Community 15`, `Community 17`?**
  _High betweenness centrality (0.079) - this node is a cross-community bridge._
- **What connects `gta6rp-api`, `Page<T>`, `SignedIn` to the rest of the system?**
  _85 weakly-connected nodes found - possible documentation gaps or missing edges._
- **Should `Community 0` be split into smaller, more focused modules?**
  _Cohesion score 0.05 - nodes in this community are weakly interconnected._
- **Why does `timeout` connect `Community 8` to `Community 12`?**
  _High betweenness centrality (0.064) - this node is a cross-community bridge._
- **Should `Community 1` be split into smaller, more focused modules?**
  _Cohesion score 0.061971830985915494 - nodes in this community are weakly interconnected._
- **Why does `ScreenAction` connect `Community 6` to `Community 17`, `Community 10`, `Community 12`, `Community 15`?**
  _High betweenness centrality (0.053) - this node is a cross-community bridge._
- **Should `Community 2` be split into smaller, more focused modules?**
  _Cohesion score 0.07643600180913614 - nodes in this community are weakly interconnected._