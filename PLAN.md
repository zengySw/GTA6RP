# GTA6RP Launcher --- план разработки

**Версия:** 1.0\
**Дата:** 21 ноября 2026\
**Платформа первой версии:** Windows 10/11 x64\
**Подход:** Rust-first

## 1. Ключевое архитектурное решение

Если цель --- приложение преимущественно на Rust, я рекомендую
отказаться от первоначальной связки Tauri + React. Tauri использует
WebView и веб-интерфейс. Это не плохая технология, но она не
соответствует требованию «почти всё на Rust».

Предлагаемый стек:

  Область               Технология
  --------------------- -----------------------
  Desktop UI            Slint
  Desktop logic         Rust
  Async runtime         Tokio
  HTTP client           Reqwest
  Backend               Axum
  Database              PostgreSQL
  Database access       SQLx
  Local state           SQLite + SQLx
  Serialization         Serde
  Logging               tracing
  Hashing               SHA-256
  Manifest signatures   Ed25519
  File storage          S3-compatible storage
  Distribution          CDN
  CI                    GitHub Actions

Slint использует декларативные `.slint` файлы, а прикладная логика,
модели, обработчики и сервисы пишутся на Rust. Это нативный GUI-подход
без React, Node.js и встроенного браузера.

**Важное условие:** до полномасштабной разработки провести Slint spike:
проверить Rockstar-inspired дизайн, изображения, DPI, resize, RAM, CPU,
cold start и лицензию. Если прототип не проходит требования,
пересмотреть UI framework через ADR, а не менять стек молча.

Документация: [Slint](https://docs.slint.dev/latest/docs/slint/), [Slint
FAQ и лицензирование](https://slint.dev/faqs), [Slint
renderers](https://docs.slint.dev/latest/docs/slint/guide/backends-and-renderers/backends_and_renderers/).

## 2. Продуктовые цели и границы

Первая версия должна позволять: - установить и запустить сам лаунчер; -
зарегистрироваться и войти; - подключиться к Backend; - выбрать каталог
тестовой игровой установки; - скачать набор файлов; - поставить загрузку
на паузу и продолжить; - восстановить загрузку после перезапуска; -
проверить файлы по подписанному манифесту; - восстановить отсутствующие
или повреждённые файлы; - обновить сам лаунчер; - посмотреть список
RP-серверов и новости; - изменить настройки; - собрать диагностический
отчёт; - запустить тестовое приложение через Fixture Game Provider.

Пока не реализуются RP-модификации, игровые интеграции, внутриигровые
скрипты, внедрение кода, обход DRM/античита и распространение игровых
бинарников без прав.

До официальной PC-версии GTA VI настоящая интеграция и запуск не
считаются реализованными. Вместо этого используется synthetic fixture:
искусственная папка с тестовыми файлами и отдельное тестовое приложение.

## 3. Архитектура

``` text
GTA6RP Desktop
├── Slint UI
├── UI Models / View State
├── Application Services
│   ├── Account Service
│   ├── Installation Service
│   ├── Download Service
│   ├── Update Service
│   ├── Server Catalog
│   └── Diagnostics
├── Domain / Application Core
│   ├── Game Provider
│   ├── Manifest Engine
│   ├── Download Engine
│   ├── Verification Engine
│   ├── Process Manager
│   └── Local State
└── Infrastructure
    ├── HTTP / CDN
    ├── SQLite
    ├── Filesystem
    ├── OS Integration
    └── Logging

GTA6RP Backend
├── Axum API
├── Authentication
├── Users / Roles
├── Launcher Releases
├── Game Catalog
├── Manifests
├── Server Catalog
├── News
├── User Settings
├── Admin API
└── PostgreSQL + Object Storage + CDN
```

### Архитектурные правила

1.  UI не работает с файловой системой, БД и CDN напрямую.
2.  Domain не зависит от Slint, HTTP или SQLx.
3.  Длительные операции выполняются в фоновых задачах.
4.  Backend --- модульный монолит, не набор микросервисов.
5.  Большие файлы идут через Object Storage/CDN, а не через API.
6.  Манифесты версионируются и подписываются.
7.  Клиент не исполняет произвольные команды, полученные с Backend.
8.  Game Provider отделён от Process Manager.
9.  Тестовая реализация не выдаётся за поддержку настоящей GTA VI.

## 4. Структура репозитория

Использовать Cargo workspace. Не дробить каждый файл на отдельный crate:
отдельный crate нужен только при чёткой границе ответственности.

``` text
GTA6RP/
├── Cargo.toml
├── Cargo.lock
├── rust-toolchain.toml
├── .cargo/
├── apps/
│   ├── launcher/
│   │   ├── Cargo.toml
│   │   ├── build.rs
│   │   ├── ui/
│   │   │   ├── app.slint
│   │   │   ├── pages/
│   │   │   ├── components/
│   │   │   ├── theme/
│   │   │   └── assets/
│   │   └── src/
│   │       ├── main.rs
│   │       ├── app/
│   │       ├── state/
│   │       ├── commands/
│   │       └── bootstrap.rs
│   ├── api/
│   │   ├── Cargo.toml
│   │   └── src/
│   └── manifest-tool/
├── crates/
│   ├── domain/
│   ├── application/
│   ├── contracts/
│   ├── config/
│   ├── errors/
│   ├── logging/
│   ├── http-client/
│   ├── auth-client/
│   ├── download-engine/
│   ├── manifest-engine/
│   ├── verification/
│   ├── installation/
│   ├── game-provider/
│   ├── process-manager/
│   ├── local-store/
│   └── test-fixtures/
├── server/
│   ├── modules/
│   │   ├── auth/
│   │   ├── users/
│   │   ├── launcher/
│   │   ├── games/
│   │   ├── manifests/
│   │   ├── servers/
│   │   ├── news/
│   │   ├── settings/
│   │   └── admin/
│   ├── database/
│   │   ├── migrations/
│   │   └── seeds/
│   └── jobs/
├── infrastructure/
│   ├── docker/
│   ├── compose/
│   ├── storage/
│   ├── deployment/
│   └── monitoring/
├── tests/
│   ├── integration/
│   ├── e2e/
│   ├── fixtures/
│   └── performance/
├── docs/
│   ├── architecture/
│   ├── decisions/
│   ├── api/
│   ├── database/
│   ├── security/
│   ├── testing/
│   ├── release/
│   └── roadmap/
├── scripts/
├── AGENTS.md
└── README.md
```

### Границы crates

-   `domain`: сущности и правила предметной области, без
    инфраструктурных зависимостей.
-   `application`: use cases и порты.
-   `contracts`: версии DTO и wire-типы.
-   `download-engine`: очередь, retries, resume и планирование.
-   `manifest-engine`: валидация и сравнение манифестов.
-   `verification`: хеши и проверка подписей.
-   `installation`: безопасное применение изменений.
-   `game-provider`: обнаружение и подготовка запуска игры.
-   `process-manager`: общие операции с процессами ОС.
-   `local-store`: SQLite и локальные миграции.
-   `test-fixtures`: искусственные игровые установки и тестовые файлы.

## 5. Подсистемы

### 5.1 UI

Страницы: - Home; - Play; - Servers; - Downloads; - News; - Account; -
Settings; - Diagnostics; - About.

Визуальное направление: Rockstar-inspired, но без копирования логотипов
и защищённых ассетов. Тёмная основа, крупные изображения, выразительная
типографика, контрастные акценты, сдержанные анимации.

Состояния главного экрана: `NotInstalled`, `Detecting`, `Verifying`,
`UpdateAvailable`, `Downloading`, `Ready`, `Launching`, `Running`,
`RepairRequired`, `Error`.

Правила производительности: - никаких I/O и хеширования в UI callback; -
не копировать большие списки файлов в UI; - обновлять прогресс
ограниченно, например 5--10 раз в секунду; - не перерисовывать всё окно
из-за каждого полученного байта; - ограничивать кэш изображений; -
проверить DPI 100/125/150/200%; - фоновые операции должны поддерживать
отмену, если она безопасна.

### 5.2 Application Core

Use cases: `CheckLauncherUpdate`, `SignIn`, `SignOut`, `LoadHome`,
`DetectGameInstallation`, `SelectGameDirectory`,
`GetInstallationStatus`, `FetchManifest`, `PlanInstallation`,
`StartDownload`, `PauseDownload`, `ResumeDownload`, `CancelDownload`,
`VerifyInstallation`, `RepairInstallation`, `ApplyUpdate`, `GetServers`,
`GetNews`, `UpdateSettings`, `ExportDiagnostics`, `LaunchGame`.

Use cases не должны зависеть от UI framework.

### 5.3 Download Engine

Функции: - очередь задач; - ограничение параллелизма; - HTTP Range при
поддержке сервером; - pause/resume/cancel; - retry с exponential backoff
и jitter; - ограничение скорости; - проверка HTTP status, Content-Range
и размера; - потоковая запись на диск; - проверка SHA-256; - проверка
свободного места; - сохранение состояния в SQLite; - восстановление
после перезапуска; - атомарная фиксация завершённого файла.

Начать с 2--4 параллельных загрузок и измерить. Не загружать файл
целиком в RAM. Не считать файл завершённым до проверки хеша. Не
разрешать двум задачам писать в один destination.

Состояния:
`Queued → Downloading → Paused/Retrying/Verifying → Completed/Failed/Cancelled`.

### 5.4 Manifest Engine

Типы: - `launcher`; - `game`; - `content` --- зарезервирован для будущих
RP-пакетов, сейчас не реализуется.

Пример формата:

``` json
{
  "schemaVersion": 1,
  "manifestId": "fixture-win64-001",
  "channel": "stable",
  "platform": "windows-x64",
  "createdAt": "2026-11-21T12:00:00Z",
  "files": [
    {
      "path": "data/example.bin",
      "size": 1048576,
      "sha256": "EXPECTED_SHA256",
      "download": {
        "url": "https://cdn.example.invalid/objects/example"
      }
    }
  ]
}
```

Правила: - schema version обязательна; - манифест подписан Ed25519; -
клиент содержит только публичный ключ; - подпись проверяется до файловых
операций; - неизвестная версия схемы отклоняется; - запрещены абсолютные
пути, `..`, UNC и опасные NTFS path forms; - дублирующиеся пути
отклоняются; - URL проходит HTTPS и allowlist-проверку; - опубликованный
manifest ID неизменяем; - для изменений публикуется новая версия; -
поддерживается rollback.

### 5.5 Installation Engine

Алгоритм: 1. Получить и проверить манифест. 2. Сравнить с локальным
состоянием. 3. Построить install plan. 4. Проверить место на диске. 5.
Скачать изменения в staging. 6. Проверить размеры и хеши. 7. Подготовить
список замен. 8. Применить изменения с журналом операций. 9.
Зафиксировать активную версию. 10. Сохранить возможность восстановления.

Не менять рабочую установку по мере скачивания. Не удалять
пользовательские файлы за пределами области, которой владеет лаунчер.

### 5.6 Game Provider и Process Manager

Game Provider отвечает за особенности конкретной игры/платформы. Process
Manager --- за общие операции ОС.

Предусмотреть `FixtureGameProvider`, который обнаруживает искусственную
установку и запускает тестовое приложение. Не использовать shell для
формирования команды. Не передавать произвольные команды с Backend.

Реальная GTA VI поддерживается только после появления официальной
PC-версии, проверки легального механизма установки/запуска и реальных
тестов.

### 5.7 Local Store

SQLite хранит настройки, выбранный путь, активный manifest ID, очередь
загрузок, незавершённые операции и диагностические метаданные.

Не хранить пароли, refresh token в открытом виде, закрытые ключи и
секреты Backend. Секреты --- через Windows Credential Manager/DPAPI с
проверенной реализацией.

## 6. Backend

### Модули

-   `auth`: register/login/refresh/logout;
-   `users`: профиль;
-   `roles`: роли и permissions;
-   `launcher`: версии и release metadata;
-   `games`: каталог и платформы;
-   `manifests`: публикация и выдача манифестов;
-   `servers`: RP-серверы;
-   `news`: новости;
-   `settings`: настройки пользователя;
-   `admin`: административные операции;
-   `audit`: журнал действий.

### API conventions

-   Prefix `/api/v1`;
-   JSON;
-   UTC timestamps в RFC 3339;
-   UUID;
-   единый формат ошибок;
-   request ID;
-   pagination;
-   rate limits;
-   OpenAPI;
-   contract tests.

### Основные endpoints

  Method   Endpoint                            Назначение
  -------- ----------------------------------- ------------------------
  POST     `/auth/register`                    Регистрация
  POST     `/auth/login`                       Вход
  POST     `/auth/refresh`                     Обновление сессии
  POST     `/auth/logout`                      Выход
  GET      `/users/me`                         Профиль
  GET      `/launcher/release`                 Версия лаунчера
  GET      `/launcher/config`                  Публичная конфигурация
  GET      `/games`                            Каталог игр
  GET      `/games/{id}/manifests/{channel}`   Манифест
  GET      `/servers`                          Список серверов
  GET      `/servers/{id}`                     Информация о сервере
  GET      `/news`                             Новости
  GET      `/users/me/settings`                Настройки
  PATCH    `/users/me/settings`                Изменение настроек

Большие файлы раздаются через CDN/object storage. API отдаёт метаданные
и URL.

### Authentication

-   Argon2id для паролей;
-   короткоживущий access token;
-   refresh token rotation;
-   отзыв сессий;
-   rate limiting;
-   отсутствие общего API secret в клиентском бинарнике;
-   защищённое хранилище refresh token;
-   RBAC на Backend;
-   audit log административных действий.

### Database

Начальные сущности: `users`, `sessions`, `roles`, `permissions`,
`user_roles`, `role_permissions`, `launcher_releases`, `games`,
`manifests`, `manifest_files`, `servers`, `server_status_snapshots`,
`news`, `user_settings`, `audit_logs`.

PostgreSQL хранит метаданные. Object Storage хранит файлы релизов и
тестовые fixture-данные. CDN распространяет неизменяемые объекты.
Закрытые ключи подписи и storage credentials не попадают в клиент.

## 7. План фаз, MVP и чекпоинты

Каждый чекпоинт --- обязательная точка приёмки. Фаза не закрывается
только по факту написания кода.

### PHASE 0 --- Technical Validation & Foundation

**Цель:** доказать, что стек подходит, и подготовить воспроизводимую
основу.

#### MVP 0.1 --- Slint spike

-   [x] Создать минимальное Slint-приложение.
-   [x] Сделать окно с изображением, навигацией и карточками.
-   [ ] Проверить загрузку крупных изображений.
-   [ ] Проверить DPI 100/125/150/200%.
-   [ ] Проверить resize и потерю фокуса.
-   [ ] Измерить cold start, RAM и idle CPU.
-   [ ] Проверить Windows x64 build.
-   [ ] Проверить лицензию Slint.

#### MVP 0.2 --- Workspace

-   [x] Создать Cargo workspace.
-   [x] Зафиксировать Rust toolchain.
-   [x] Настроить rustfmt.
-   [x] Настроить Clippy.
-   [x] Настроить cargo test.
-   [x] Создать CI.
-   [x] Добавить `.editorconfig` и `.gitignore`.
-   [x] Настроить env-конфигурацию без секретов.

#### MVP 0.3 --- Architecture docs

-   [x] Создать `AGENTS.md`.
-   [x] Создать `docs/architecture/OVERVIEW.md`.
-   [x] Описать границы crates.
-   [x] Создать ADR по UI framework.
-   [x] Создать ADR по Backend.
-   [x] Создать ADR по local secrets.
-   [x] Зафиксировать scope и non-goals.

**CHECKPOINT 0** - \[ \] Slint spike подходит визуально и технически. -
\[ \] RAM/CPU/startup измерены и записаны. - \[ \] Workspace собирается
на чистом окружении. - \[ \] CI проходит. - \[ \] Ключевые ADR
утверждены. - \[ \] Нет незакрытого критического технического риска.

**Результат:** утверждённый стек и воспроизводимый фундамент.

### PHASE 1 --- UI Foundation

#### MVP 1.1 --- Design system

-   [x] Цвета, типографика, spacing.
-   [x] Кнопки, карточки, поля, диалоги.
-   [x] Hover/pressed/disabled/focus states.
-   [x] Loading/error components.
-   [x] Локальные тестовые изображения.

#### MVP 1.2 --- Application shell

-   [x] Главное окно и навигация.
-   [x] Home.
-   [x] Play.
-   [x] Servers.
-   [x] Downloads.
-   [x] News.
-   [x] Account.
-   [x] Settings.
-   [x] Diagnostics.
-   [x] About.

#### MVP 1.3 --- UI state

-   [x] Конечный автомат главного экрана.
-   [x] Mock data provider.
-   [x] Loading/empty/error states.
-   [x] Уведомления.
-   [x] UI callbacks не выполняют I/O; длительные mock-переходы используют таймеры.

**CHECKPOINT 1** - \[ \] Все страницы доступны. - \[ \] UI работает на
mock data. - \[ \] В UI callback нет I/O. - \[ \] DPI и resize
проверены. - \[ \] Длинные названия и большие значения не ломают
layout. - \[ \] Дизайн утверждён.

### PHASE 2 --- Backend Foundation

#### MVP 2.1 --- API

-   [x] Axum app.
-   [x] Конфигурация окружений.
-   [x] Health endpoint.
-   [x] Request ID.
-   [x] tracing.
-   [x] Единый JSON error envelope для 404/405/429.
-   [x] Ограниченный CORS.
-   [x] Rate limiting.
-   [x] Graceful shutdown.

#### MVP 2.2 --- Database

-   [x] PostgreSQL через Docker Compose.
-   [x] SQLx.
-   [x] Миграции.
-   [x] Начальные таблицы.
-   [x] Индексы и foreign keys.
-   [x] Test database через изолированный Compose profile.
-   [ ] Проверка чистой миграции.

#### MVP 2.3 --- Contracts

-   [x] Общие DTO для страниц результатов и ошибок.
-   [x] Единый формат ошибок.
-   [x] OpenAPI для реализованных маршрутов.
-   [x] Общая пагинация с проверкой `limit` (1-100).
-   [x] Validation параметров пагинации.
-   [ ] Contract tests.

**CHECKPOINT 2** - \[ \] API стартует с чистой конфигурацией. - \[ \]
Миграции проходят. - \[ \] Health checks работают. - \[ \] OpenAPI
соответствует маршрутам. - \[ \] Integration tests проходят.

### PHASE 3 --- Accounts & Security

#### MVP 3.1

-   [x] Регистрация.
-   [x] Вход и выход.
-   [x] Argon2id.
-   [x] Access token.
-   [x] Refresh rotation.
-   [x] Отзыв сессий.
-   [x] Rate limits.
-   [x] Защита от brute force (отдельный IP-лимит для login).

#### MVP 3.2

-   [x] Подключить UI к auth service.
-   [x] Secure storage для refresh token (Windows Credential Manager).
-   [x] Автообновление сессии.
-   [x] Восстановление после рестарта.
-   [x] Не логировать токены и пароли.

#### MVP 3.3

-   [ ] Roles и permissions.
-   [ ] Серверная проверка прав.
-   [ ] Защита admin API.
-   [ ] Audit events.

**CHECKPOINT 3** - \[ \] Вход/выход работают. - \[ \] Refresh rotation
проверен. - \[ \] Отозванная сессия недействительна. - \[ \] Секреты
отсутствуют в логах. - \[ \] RBAC проверяется Backend.

### PHASE 4 --- Rust Core & Local State

#### MVP 4.1

-   [x] Application state (screen-state reducer в `gta6rp-application`, тестируется без UI).
-   [x] Use cases (`use_cases.rs`: `LocalStatePort`, `prepare_startup()`, `update_settings()`, `load_settings()`).
-   [x] Typed errors.
-   [x] Event channel UI/core (сторона core готова; мост в Slint UI подключён через `app_bridge.rs` + `invoke_from_event_loop`).
-   [x] Cancellation.
-   [x] Graceful shutdown.

#### MVP 4.2

-   [x] SQLite.
-   [x] Local migrations.
-   [x] Настройки (подключены к лаунчеру через `LocalStateHandle`).
-   [x] Выбранный путь.
-   [x] Состояние задач.
-   [x] Активный manifest ID.
-   [x] Recovery после аварии (checkpoint'ы загрузок; journal установки — PHASE 6).

#### MVP 4.3

-   [x] Safe path resolver.
-   [x] Защита от path traversal.
-   [x] Проверка symlink (Windows reparse points/junctions — отдельная задача).
-   [x] Разрешённые корни.
-   [x] Temp file + atomic rename.
-   [x] Access denied возвращается типизированной ошибкой.
-   [x] Проверка свободного места (`ensure_free_space()`, `free_space_info()` в `crates/installation`).

**CHECKPOINT 4** - [x] Core тестируется без UI. - [x] Local state
переживает рестарт. - [x] UI получает события без частого polling. -
[x] Файловые операции ограничены. - [x] Recovery tests проходят.

### PHASE 5 --- Download Engine

#### MVP 5.1

-   [ ] Скачать один HTTPS-файл.
-   [ ] Потоковая запись.
-   [ ] Проверка HTTP status.
-   [ ] Проверка размера.
-   [ ] Progress и speed.
-   [ ] SHA-256.
-   [ ] Атомарное перемещение готового файла.

#### MVP 5.2

-   [ ] Очередь.
-   [ ] Ограниченный параллелизм.
-   [ ] Pause/resume/cancel.
-   [ ] Retry с backoff и jitter.
-   [ ] Rate limit.
-   [ ] SQLite persistence.
-   [ ] Resume после рестарта.

#### MVP 5.3

-   [ ] Потеря сети.
-   [ ] HTTP 404/403/429/5xx.
-   [ ] Range/Content-Range.
-   [ ] Сервер без Range.
-   [ ] Частичный файл.
-   [ ] Недостаток места.
-   [ ] Повреждённый файл.
-   [ ] Ограничение RAM.
-   [ ] Защита от записи в один destination.

**CHECKPOINT 5** - \[ \] Файл скачивается и проверяется. - \[ \]
Загрузка возобновляется после рестарта. - \[ \] Повреждённый файл не
считается готовым. - \[ \] Потеря интернета не ломает очередь. - \[ \]
UI отзывчив. - \[ \] RAM не зависит линейно от размера файла. - \[ \]
Integration tests с локальным HTTP server проходят.

### PHASE 6 --- Manifest, Verification & Installation

#### MVP 6.1

-   [ ] Schema version 1.
-   [ ] File entries.
-   [ ] Platform/channel.
-   [ ] Manifest ID.
-   [ ] Signature metadata.
-   [ ] Serde validation.
-   [ ] Тестовые манифесты.

#### MVP 6.2

-   [ ] Тестовая пара ключей.
-   [ ] Подпись manifest.
-   [ ] Проверка корректной подписи.
-   [ ] Отклонение изменённого manifest.
-   [ ] Отклонение неизвестного ключа.
-   [ ] Key rotation design.
-   [ ] Закрытый ключ вне клиента и репозитория.

#### MVP 6.3

-   [ ] Сравнение локального состояния.
-   [ ] Missing/changed/extra files.
-   [ ] Install plan.
-   [ ] Общий объём изменений.
-   [ ] Проверка свободного места.

#### MVP 6.4

-   [ ] Staging.
-   [ ] Проверка перед применением.
-   [ ] Journal.
-   [ ] Безопасное применение.
-   [ ] Recovery незавершённой операции.
-   [ ] Rollback.
-   [ ] Защита пользовательских файлов.

**CHECKPOINT 6** - \[ \] Подписанный manifest принимается. - \[ \]
Изменённый manifest отклоняется. - \[ \] Diff корректен. - \[ \]
Повреждённые файлы восстанавливаются. - \[ \] Crash во время установки
восстанавливается. - \[ \] Rollback проверен. - \[ \] Windows tests
проходят.

### PHASE 7 --- Game Manager

#### MVP 7.1

-   [ ] Synthetic fixture.
-   [ ] Тестовая структура каталогов.
-   [ ] Тестовый manifest.
-   [ ] Тестовое приложение.
-   [ ] FixtureGameProvider.
-   [ ] Обнаружение и проверка установки.

#### MVP 7.2

-   [ ] Выбор каталога.
-   [ ] Проверка доступности.
-   [ ] Проверка свободного места.
-   [ ] Отображение состояния.
-   [ ] Установка fixture.
-   [ ] Проверка и восстановление.
-   [ ] Обновление.

#### MVP 7.3

-   [ ] Запуск тестового приложения.
-   [ ] PID.
-   [ ] Мониторинг процесса.
-   [ ] Обработка завершения.
-   [ ] Запрет повторного запуска.
-   [ ] Ошибки отсутствующего executable.
-   [ ] Безопасные аргументы запуска.

**CHECKPOINT 7** - \[ \] Fixture обнаруживается и устанавливается. - \[
\] Повреждённая установка восстанавливается. - \[ \] Тестовое приложение
запускается. - \[ \] Состояние процесса корректно. - \[ \] Game Provider
независим от UI. - \[ \] Реальная GTA VI не заявлена поддерживаемой без
проверки.

### PHASE 8 --- Launcher Updates & Releases

#### MVP 8.1

-   [ ] Release metadata.
-   [ ] Версионирование.
-   [ ] Stable/beta channels.
-   [ ] Minimum supported version.
-   [ ] Release notes.
-   [ ] Подпись metadata.

#### MVP 8.2

-   [ ] Проверка обновлений при старте.
-   [ ] Скачивание.
-   [ ] Проверка подписи и целостности.
-   [ ] Передача управления updater/bootstrapper.
-   [ ] Проверка запуска новой версии.
-   [ ] Rollback.

#### MVP 8.3

-   [ ] Windows installer.
-   [ ] Uninstaller.
-   [ ] Upgrade поверх старой версии.
-   [ ] Подпись издателя.
-   [ ] Чистая Windows VM.

**CHECKPOINT 8** - \[ \] Обновление работает. - \[ \] Неподписанный
пакет отклоняется. - \[ \] Прерванное обновление восстанавливается. - \[
\] Неудачный релиз не лишает возможности запустить клиент. - \[ \]
Installer/uninstaller протестированы.

### PHASE 9 --- Servers, News & Settings

#### MVP 9.1

-   [ ] CRUD серверов в Backend.
-   [ ] Публичный список.
-   [ ] Карточка.
-   [ ] Регион и версия.
-   [ ] Статус и количество игроков.
-   [ ] Maintenance.
-   [ ] Кэширование.

#### MVP 9.2

-   [ ] CRUD новостей.
-   [ ] Публикация.
-   [ ] Обложки.
-   [ ] Даты.
-   [ ] Пагинация.
-   [ ] Отображение в Launcher.

#### MVP 9.3

-   [ ] Путь установки.
-   [ ] Язык.
-   [ ] Автозапуск.
-   [ ] Поведение при закрытии.
-   [ ] Ограничение скорости.
-   [ ] Параллелизм.
-   [ ] Уведомления.
-   [ ] Persistence настроек.

**CHECKPOINT 9** - \[ \] Серверы и новости загружаются с Backend. - \[
\] Offline/maintenance отображаются корректно. - \[ \] Настройки
сохраняются. - \[ \] Ошибки API не ломают приложение.

### PHASE 10 --- Admin API & Admin Panel

#### MVP 10.1

-   [ ] Permissions.
-   [ ] Управление пользователями.
-   [ ] Управление ролями.
-   [ ] Управление серверами.
-   [ ] Управление новостями.
-   [ ] Управление release metadata.
-   [ ] Публикация manifests.
-   [ ] Audit logs.

#### MVP 10.2 --- Manifest tool

-   [ ] CLI сканирования каталога.
-   [ ] SHA-256 и размеры.
-   [ ] Генерация manifest.
-   [ ] Проверка путей.
-   [ ] Подписание локальным ключом.
-   [ ] Dry-run.
-   [ ] Загрузка объектов.
-   [ ] Публикация metadata.
-   [ ] Проверка опубликованной версии.

#### MVP 10.3

-   [ ] Admin login.
-   [ ] Dashboard.
-   [ ] Users.
-   [ ] Servers.
-   [ ] News.
-   [ ] Releases.
-   [ ] Manifests.
-   [ ] Audit logs.

Admin UI --- отдельное приложение позднее. Рассмотреть Leptos, если
требуется Rust-first веб-панель. Не смешивать её с desktop UI.

**CHECKPOINT 10** - \[ \] Все admin actions проверяются Backend. - \[ \]
Публикация воспроизводима. - \[ \] Закрытый ключ не отправляется на
сервер. - \[ \] Критичные действия попадают в audit log. - \[ \] Обычный
пользователь не имеет admin-доступа. - \[ \] Тестовый релиз публикуется
end-to-end.

### PHASE 11 --- QA, Security & Release Candidate

#### MVP 11.1 --- Tests

-   [ ] Domain unit tests.
-   [ ] Manifest tests.
-   [ ] Downloader state machine tests.
-   [ ] PostgreSQL integration tests.
-   [ ] HTTP integration tests.
-   [ ] Windows filesystem tests.
-   [ ] UI state tests.
-   [ ] E2E install fixture.
-   [ ] E2E update fixture.
-   [ ] E2E rollback fixture.

#### MVP 11.2 --- Failure injection

-   [ ] Потеря интернета во время загрузки.
-   [ ] Принудительное завершение во время staging.
-   [ ] Недостаток места.
-   [ ] Повреждённый manifest.
-   [ ] Неверная подпись.
-   [ ] Изменённый файл.
-   [ ] Отсутствующий файл.
-   [ ] HTTP 404/429/500.
-   [ ] CDN timeout.
-   [ ] Отозванная сессия.
-   [ ] Недоступная БД.
-   [ ] Повторный запуск после сбоя.

#### MVP 11.3 --- Performance

-   [ ] Cold start.
-   [ ] Idle RAM.
-   [ ] Idle CPU.
-   [ ] Скорость SHA-256.
-   [ ] Throughput загрузки.
-   [ ] UI responsiveness под нагрузкой.
-   [ ] Большой manifest.
-   [ ] Большое количество файлов.
-   [ ] Долгая сессия.
-   [ ] Сравнение с baseline Фазы 0.

#### MVP 11.4 --- Release

-   [ ] Release build.
-   [ ] Удаление debug-only функций.
-   [ ] Подпись бинарников.
-   [ ] Installer/uninstaller.
-   [ ] Проверка upgrade.
-   [ ] Чистая Windows VM.
-   [ ] Release notes.
-   [ ] Rollback procedure.
-   [ ] Support/diagnostics guide.

**CHECKPOINT 11 --- RELEASE CANDIDATE** - \[ \] Нет открытых
Critical/High security issues. - \[ \] Обязательные сценарии проходят. -
\[ \] Нет известных потерь пользовательских данных. - \[ \] Установка и
удаление проверены. - \[ \] Обновление и rollback проверены. - \[ \]
Performance результаты задокументированы. - \[ \] Backup/restore Backend
проверены. - \[ \] Релиз воспроизводится из CI.

## 8. Сквозные требования качества

### Rust quality

-   `cargo fmt --all -- --check`;
-   `cargo clippy --workspace --all-targets --all-features -- -D warnings`
    (обоснованные исключения документировать);
-   `cargo test --workspace`;
-   не использовать `unwrap()`/`expect()` в обычных runtime-путях без
    причины;
-   не подавлять warnings ради сборки;
-   `unsafe` только при документированной необходимости и review;
-   не блокировать async runtime;
-   не держать Mutex/RwLock guard через `.await`;
-   ограничивать фоновые задачи;
-   поддерживать cancellation там, где она безопасна;
-   не включать секреты в error context.

### Security

-   TLS;
-   подписанные manifests;
-   проверка подписи до файловых операций;
-   секреты вне репозитория и логов;
-   path traversal protection;
-   проверка URL;
-   запрет произвольного исполнения с Backend;
-   отсутствие скрытых shell-команд;
-   проверка обновлений;
-   audit log;
-   dependency audit.

### Reliability

-   загрузка переживает рестарт;
-   установка восстанавливается после сбоя;
-   операции идемпотентны, где возможно;
-   ошибка одного файла не повреждает весь каталог;
-   retry ограничен;
-   состояние восстанавливается из core/local store;
-   логи позволяют найти причину сбоя.

### Performance

-   тяжёлый I/O не выполняется в UI thread;
-   хеширование --- в worker pool;
-   ограниченные буферы и параллелизм;
-   агрегированный прогресс;
-   ограниченный cache;
-   отсутствие активного polling в idle;
-   оптимизация только по результатам профилирования.

## 9. ADR --- решения, которые надо фиксировать

  ID         Тема                  Предварительное решение
  ---------- --------------------- ------------------------------
  ADR-0001   GUI                   Slint, после spike
  ADR-0002   Desktop runtime       Native Rust
  ADR-0003   Backend               Axum
  ADR-0004   Async                 Tokio
  ADR-0005   Server DB             PostgreSQL + SQLx
  ADR-0006   Local DB              SQLite + SQLx
  ADR-0007   File distribution     S3-compatible + CDN
  ADR-0008   Manifest signatures   Ed25519
  ADR-0009   Local secrets         Windows secure storage
  ADR-0010   Install transaction   Staging + journal + rollback
  ADR-0011   Game integration      Provider abstraction
  ADR-0012   Admin UI              Отдельное приложение позднее

Каждый ADR содержит контекст, проблему, варианты, принятое решение,
последствия, риски, дату и статус.

## 10. AI workflow и AGENTS.md

### Роли

-   Architect --- архитектура и ADR.
-   Rust Core Developer --- domain/application/filesystem.
-   UI Developer --- Slint и UI models.
-   Backend Developer --- Axum, SQLx, PostgreSQL.
-   Download Engineer --- downloader.
-   Security Reviewer --- threat model и аудит.
-   QA Engineer --- тесты.
-   Release Engineer --- CI, installer, signing.

Не запускать всех агентов одновременно. Параллелить только независимые
задачи с зафиксированными контрактами.

### Workflow каждой задачи

1.  Прочитать AGENTS.md.
2.  Прочитать относящиеся документы.
3.  Изучить текущую реализацию.
4.  Сформулировать план.
5.  Проверить актуальную официальную документацию.
6.  Согласовать изменения контрактов.
7.  Реализовать ограниченный законченный объём.
8.  Добавить тесты.
9.  Запустить fmt, clippy, tests и build.
10. Проверить diff.
11. Обновить документацию.
12. Отчитаться.

### Формат отчёта

``` text
TASK:
CHANGES:
ARCHITECTURE:
TESTS:
SECURITY:
KNOWN ISSUES:
CHECKPOINT:
```

### Запреты

-   не переписывать соседние модули без причины;
-   не менять публичный API без согласования;
-   не добавлять зависимости без проверки;
-   не дублировать сервисы;
-   не выдавать заглушку за готовую функцию;
-   не отключать тесты ради сборки;
-   не менять архитектуру молча;
-   не использовать `unwrap_or_default()` для сокрытия ошибок;
-   не выполнять destructive filesystem operations без проверки root и
    плана;
-   не называть fixture поддержкой GTA VI.

## 11. CI/CD и окружения

Окружения: `local`, `test`, `staging`, `production`. У каждого свои
credentials, БД и storage bucket.

На Pull Request: 1. fmt; 2. clippy; 3. unit tests; 4. integration tests;
5. миграции; 6. OpenAPI/contracts check; 7. Windows x64 build; 8.
dependency audit; 9. secret scan.

На release: 1. Release build; 2. подпись артефактов; 3. checksum; 4.
release metadata; 5. публикация в staging; 6. smoke tests; 7. ручное
подтверждение production; 8. публикация; 9. проверка доступности; 10.
сохранение предыдущего релиза.

## 12. Итоговая таблица MVP

  MVP   Результат                              Зависимость
  ----- -------------------------------------- ----------------------
  0     Проверенная технология и workspace     ---
  1     UI на mock data                        0
  2     Backend foundation                     0
  3     Аккаунты и безопасность                2
  4     Rust Core и local state                0
  5     Downloader                             4
  6     Manifest, verification, installation   5
  7     Game Manager на fixture                6
  8     Обновление лаунчера                    2, 4
  9     Servers, News, Settings                1, 2
  10    Admin API и публикация                 2, 6
  11    QA, Security, Release Candidate        Все обязательные MVP

### Definition of Done

MVP завершён, когда: - функция реализована; - контракты
задокументированы; - ошибки обработаны; - тесты добавлены и проходят; -
негативные сценарии проверены; - нет известных Critical security
issues; - CI проходит; - документация обновлена; - результат
воспроизводится на чистом окружении.

## 13. Первый практический шаг

Начинать с PHASE 0, а не с полного интерфейса или загрузчика.

1.  Создать Slint spike на Windows x64.
2.  Собрать экран с фоновым изображением, навигацией и карточками.
3.  Измерить RAM, CPU, startup и responsiveness.
4.  Проверить лицензию.
5.  Создать Cargo workspace.
6.  Зафиксировать ADR.
7.  Подготовить AGENTS.md.

После CHECKPOINT 0 переходить к полноценной разработке.

**Главное правило проекта:** сначала доказать технологию и архитектуру
небольшими работающими прототипами, затем строить продукт. Количество
написанного кода не является мерой готовности.
