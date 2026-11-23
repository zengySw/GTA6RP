# GTA6RP Launcher

Rust-first desktop launcher prototype для Windows 10/11 x64 с Axum API бэкендом.
Клиент на Slint, сейчас в режиме synthetic fixture — не устанавливает и не запускает
настоящую GTA VI. До официального PC-релиза и валидации законного пути интеграции
используются только синтетические фикстуры.

## Стек технологий

| Область | Технология |
|---------|------------|
| Desktop UI | Slint (декларативный `.slint`, без браузера) |
| Desktop logic | Rust |
| Async runtime | Tokio |
| HTTP client | Reqwest (blocking, в фоновых потоках) |
| Backend | Axum |
| Database | PostgreSQL (backend) + SQLite (local state) |
| Database access | SQLx |
| Serialization | Serde |
| Logging | tracing |
| Hashing | SHA-256, Argon2id |
| Build | cargo (edition 2024, rust 1.95+) |

## Структура репозитория

```
gta6rp/
├── Cargo.toml                 # Рабочая область (workspace) с 6 crates
├── PLAN.md                    # Главный план разработки (11 фаз)
├── AGENTS.md                  # Инструкции для агентов-разработчиков
├── README.md                  # Этот файл
├── apps/
│   ├── api/                   # Axum бэкенд (auth, health, OpenAPI, PostgreSQL)
│   └── launcher/              # Slint десктоп-клиент (main, auth_client, UI)
├── crates/
│   ├── domain/                # Чистый домен: ScreenState, RelativePath, Settings
│   ├── application/           # ApplicationCore, EventBus, CancellationToken, LocalStatePort
│   ├── installation/          # SafeRoot, безопасная запись файлов, free space check
│   └── local-store/           # SQLite локальное состояние, миграции, восстановление
├── docs/
│   ├── architecture/          # OVERVIEW.md, PHASE_C_INTEGRATION.md, DECISIONS_REQUIRED.md
│   ├── api/                   # openapi.yaml
│   ├── database/              # SCHEMA.md
│   ├── decisions/             # ADR-0001..ADR-0005
│   └── audit/                 # Полный аудит кодовой базы
├── infrastructure/compose/    # Docker Compose для PostgreSQL
└── graphify-out/              # Граф знаний (граф кодовой базы)
```

## Архитектура

Основные правила (см. `AGENTS.md`):

1. **UI не работает с файловой системой, БД и CDN напрямую.**
2. **Domain не зависит от Slint, HTTP или SQLx.**
3. **Длительные операции выполняются в фоновых задачах.**
4. **Backend — модульный монолит, не набор микросервисов.**
5. **Клиент не исполняет произвольные команды, полученные с Backend.**
6. **UI не содержит secret-ключей.**

### Слои

- **`crates/domain`** — чистые доменные типы: конечный автомат экранов (`ScreenState`/`ScreenAction`), валидация относительных путей (`RelativePath`), настройки (`Settings`). Нет зависимостей от инфраструктуры.
- **`crates/application`** — оцияфно-независимые примитивы: `ApplicationCore` (редуктор состояний вне UI-потока), `EventBus` (broadcast канал событий), `CancellationToken`, `ShutdownSupervisor`, `LocalStatePort` (порт для локального состояния).
- **`crates/installation`** — безопасная запись файлов в канонизированный корень (`SafeRoot`), отказ от симлинков, атомарная запись через temp-файл + rename, проверка свободного места.
- **`crates/local-store`** — SQLite через SQLx: настройки, выбранный путь установки, активный manifest ID, чекпоинты скачивания, восстановление после аварийного завершения.
- **`apps/launcher`** — Slint UI, точка входа, `auth_client` (регистрация/логин/refresh/logout, Windows Credential Manager), мост `ApplicationCore → Slint` через `invoke_from_event_loop`.
- **`apps/api`** — Axum API: регистрация, логин, refresh-токены с ротацией, `/users/me`, health, OpenAPI, CORS, rate limiting, graceful shutdown.

## Быстрый старт

### Требования

- Rust 1.100.0 с `rustfmt` и `clippy`
- Windows 10/11 x64 (целевая платформа UI, разработка возможна и на Linux)

### Сборка и запуск

```sh
# Собрать всё
cargo build --workspace

# Проверить форматирование
cargo fmt --all -- --check

# Проверить линтер
cargo clippy --workspace --all-targets --all-features -- -D warnings

# Запустить все тесты
cargo test --workspace

# Запустить лаунчер
cargo run -p gta6rp-launcher

# Запустить API бэкенд (требуется PostgreSQL)
cargo run -p gta6rp-api
```

Для локальной разработки скопируйте `.env.example` в `.env` и настройте `POSTGRES_PASSWORD` и `DATABASE_URL`:

```sh
cp .env.example .env
docker compose -f infrastructure/compose/docker-compose.yml up -d postgres
export DATABASE_URL=postgres://gta6rp:your-password@127.0.0.1:54329/gta6rp
cargo run -p gta6rp-api
```

Launcher соединяется с `GTA6RP_API_BASE_URL` (по умолчанию `http://127.0.0.1:8080/api/v1`).

## Дорожная карта

Проект разделён на 11 фаз (см. `PLAN.md`):

| Фаза | Название | Статус |
|------|----------|--------|
| 0 | Technical Validation & Foundation | В процессе (Slint spike) |
| 1 | UI Foundation | В процессе |
| 2 | Backend Foundation | Частично (API реализован, контрактные тесты не начато) |
| 3 | Accounts & Security | Частично (auth реализован, RBAC нет) |
| 4 | Rust Core & Local State | В процессе (ApplicationCore, LocalStore подключены) |
| 5 | Download Engine | Не начато |
| 6 | Manifest, Verification & Installation | Не начато |
| 7 | Game Manager | Не начато |
| 8 | Launcher Updates & Releases | Не начато |
| 9 | Servers, News & Settings | Не начато |
| 10 | Admin API & Admin Panel | Не начато |
| 11 | QA, Security & Release Candidate | Не начато |

## Важные ограничения

- **До официального PC-релиза GTA VI настоящая интеграция и запуск не считаются реализованными.** Вместо этого используется synthetic fixture — искусственная папка с тестовыми файлами.
- Пока не реализуются RP-модификации, игровые интеграции, скрипты внутри игры, внедрение кода, обход DRM/античита и распространение игровых бинарников без прав.
- Лаuncher не устанавливает и не запускает GTA VI.

## Граф знаний

Проект analysing через graphify. Результат находится в `graphify-out/`:
- `graph.json` — сырые данные графа
- `GRAPH_REPORT.md` — текстовый отчёт с communautés, узлами, связями

## Лицензия

Этот проект лицензирован под **Zengy Source-Available License 1.0 (ZSAL-1.0)**.

Вы свободны:

- Просматривать, изучать и скачивать исходный код.
- Использовать проект для персональных и некоммерческих целей.
- Модифицировать исходный код.
- Создавать и публиковать форки.
- Распространять оригинальные или модифицированные версии бесплатно.

Вы обязаны:

- Сохранять исходные уведомления об авторских правах и лицензию.
- Давать соответствующую кредит(original author).
- Чётко идентифицировать ваши изменения.
- Делать исходный код модификаций доступным при распространении модифицированных версий.
- Распространять производные работы под той же лицензией.

**Коммерческое использование не разрешено без предварительного письменного разрешения правообладателя.**

Это включает продажу программного обеспечения, предоставление платного доступа, включение в коммерческие продукты или использование как часть платных услуг.

Для коммерческих лицензионных запросов, пожалуйста, свяжитесь с правообладателем.

См. файл [LICENSE](LICENSE) для полного текста.

**Примечание:** Это Source-Available лицензия, а не лицензия, одобренная OSI (Open Source Initiative).
