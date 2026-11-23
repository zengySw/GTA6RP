# Техническое исследование Phase C: Интеграция ApplicationCore в Launcher

Версия: 2026-11-23  
Статус: Техническое исследование (без изменений исходного кода)

## 1. Текущее состояние (базовая линия)

### 1.1 Launcher (apps/launcher)

| Компонент | Файл | Строки | Описание |
|-----------|------|--------|----------|
| Точка входа | `apps/launcher/src/main.rs` | 1-154 | `fn main() -> Result<(), slint::PlatformError>`, синхронный, без Tokio |
| Состояние UI | `apps/launcher/src/main.rs` | 17 | `Rc<Cell<ScreenState>>` — синхронное, UI-потоковое состояние |
| Машина состояний | `apps/launcher/src/main.rs` | 115-129 | `schedule_action()` с `slint::Timer::single_shot` (задержки 350–2200 мс) |
| Применение действия | `apps/launcher/src/main.rs` | 131-145 | `apply_action_weak()` — синхронный `ScreenState::apply()` + `apply_state()` |
| UI→строки | `apps/launcher/src/state.rs` | 1-93 | `use gta6rp_domain::screen::ScreenState;` — чистое отображение состояний в строки |
| Auth | `apps/launcher/src/auth_client.rs` | 1-493 | `std::thread` + блокирующий `reqwest`, `slint::invoke_from_event_loop` |
| Манифест Cargo | `apps/launcher/Cargo.toml` | 1-23 | Зависимости: `gta6rp-domain`, `reqwest` (blocking), `serde`, `slint`, `keyring` (windows). **Нет Tokio.** |

Основной цикл: `window.run()` (строка 112) блокирует главный поток Slint. Все обновления состояния происходят синхронно в колбэках UI.

### 1.2 Application core (crates/application)

| Компонент | Файл | Строки | Описание |
|-----------|------|--------|----------|
| Сборка crates | `crates/application/src/lib.rs` | 1-20 | Экспортирует `ApplicationCore`, `CoreControl`, `CoreHandle`, `EventBus`, `CancellationToken`, `ShutdownSupervisor`, `LocalStatePort`, use cases |
| Ядро | `crates/application/src/core.rs` | 1-313 | `ApplicationCore::spawn()` — паникует вне Tokio runtime (строка 38). `CoreControl::apply()` — `try_send` на mpsc (ёмкость 64). `CoreHandle::shutdown()` — `async`, потребляет `self` |
| EventBus | `crates/application/src/events.rs` | 1-80 | `tokio::sync::broadcast` канал. `publish()`, `subscribe()`, `subscriber_count()` |
| Отмена | `crates/application/src/cancellation.rs` | 1-78 | `watch`-based `CancellationToken` |
| Shutdown | `crates/application/src/shutdown.rs` | 1-90 | `ShutdownSupervisor` — `JoinSet` + grace timeout |
| Use cases | `crates/application/src/use_cases.rs` | 1-~200 | `LocalStatePort` трейт, `prepare_startup()`, `update_settings()`, `load_settings()` |

### 1.3 LocalStore (crates/local-store)

| Компонент | Файл | Описание |
|-----------|------|----------|
| Хранилище | `crates/local-store/src/store.rs` | `LocalStore::open()`, `settings()`, `save_settings(Settings)` (by value), `active_manifest_id()`, `upsert_task()`, `recover_interrupted()`. Реализует `LocalStatePort` (после Phase B) |
| Модель | `crates/local-store/src/model.rs` | `Settings` (доменный тип, валидация), `DownloadTask`, `DownloadState` |
| Ошибки | `crates/local-store/src/error.rs` | `StoreError` с вариантами `Database`, `Migration`, `Io`, `InvalidInput`, `Corrupt` |

## 2. Вариант A — Отдельный std::thread с собственным Tokio runtime

ApplicationCore запускается в отдельном потоке, который сам создаёт и обслуживает Tokio runtime. Взаимодействие со Slint — через `slint::invoke_from_event_loop`. Launcher остаётся без Tokio в зависимостях.

### 2.1 Создание и уничтожение ApplicationCore

**Создание:**
1. В `main()` создаётся `std::thread::spawn(move || { ... })`.
2. Внутри потока создаётся `tokio::runtime::Runtime::new().unwrap()`.
3. На runtime вызывается `rt.block_on(ApplicationCore::spawn(initial, capacity, timeout))`, возвращается `CoreHandle`.
4. `CoreControl` клонируется и передаётся обратно в основной поток (через `mpsc` канал или `Arc<Mutex<Option<CoreControl>>`).
5. `CoreHandle` остаётся в Tokio-потоке (владеет `join` и `shutdown_timeout`).

**Уничтожение:**
1. При закрытии окна Slint (`on_close_requested` или в конце `main()`) основной поток посылает сигнал завершения в Tokio-поток.
2. Tokio-поток вызывает `core_handle.shutdown().await` — отменяет `CancellationToken`, шлёт `Command::Shutdown`, ждёт `join` с таймаутом.
3. Runtime останавливается (`runtime.shutdown_background()` или drop `Runtime`).
4. Поток завершается.

### 2.2 Отправка команд из UI в Core (CoreControl)

1. UI колбэк (например, `on_primary_action`) получает клон `CoreControl`.
2. Вызывает `core_control.apply(ScreenAction::BeginSetup)` — `try_send(Command::Apply(action))` на ограниченную очередь mpsc (ёмкость 64).
3. Если очередь полна — возвращается `ApplicationError::CommandQueueFull`; UI может показать уведомление "пожалуйста, подождите".
4. Команда применяется в Tokio-потоке асинхронно; UI не блокируется.

**Важно:** `apply()` — синхронный, не требует `await`, безопасно вызывать из синхронного колбэка Slint.

### 2.3 Передача событий из Core в UI (EventBus + watch)

Поскольку запрещён polling через Slint Timer, используется **мостовая задача** в Tokio-потоке:

1. Создаётся `event_receiver = core_control.events().subscribe()` и `state_receiver = core_control.state()`.
2. Мостовая задача (`tokio::spawn`) в Tokio-потоке выбирает события через `tokio::select!`.
3. При получении `AppEvent::ScreenChanged(state)` вызывает `slint::invoke_from_event_loop` для обновления свойств окна.
4. `slint::invoke_from_event_loop` безопасно маршализует выполнение в поток событий Slint. Вызывается из Tokio-потока, поэтому не блокирует UI-поток (если только Slint не завис).
5. Альтернативно (или дополнительно) `watch::Receiver::changed()` мониторит текущее состояние.

### 2.4 Shutdown

1. Пользователь закрывает окно → Slint колбэк `on_close_requested`.
2. Основной поток посылает сигнал shutdown в Tokio-поток (отдельный `mpsc::Sender` или `Arc<AtomicBool>`).
3. Мостовая задача завершается (получает `AppEvent::Shutdown` или сигнал отмены).
4. Tokio-поток вызывает `core_handle.shutdown().await`:
   - `cancellation.cancel()`
   - `commands.send(Command::Shutdown).await`
   - `timeout(shutdown_timeout, join).await`
5. Если таймаут — `abort_all()` и возврат `ShutdownTimeout`.
6. `runtime.shutdown_background()` / drop `Runtime`.
7. Поток завершается, `main()` возвращает управление.

### 2.5 Обработка ошибок

| Ошибка | Источник | Куда передаётся |
|--------|----------|-----------------|
| `CommandQueueFull` | `CoreControl::apply()` | UI показывает уведомление, повтор позже |
| `CommandClosed` | `CoreControl::apply()` | Core остановлен; UI блокирует кнопки |
| `ShutdownTimeout` | `CoreHandle::shutdown()` | Логируется; поток принудительно завершается |
| `TaskFailed` | паника в задаче | Логируется; приложение может продолжить работу |
| `StoreError` | `LocalStore` | Через `UseCaseError::Storage` → UI уведомление |
| `SettingsError` | `Settings::validate()` | Через `UseCaseError::InvalidSettings` → UI уведомление |

Все ошибки передаются в UI через `slint::invoke_from_event_loop` с установкой `window.set_notice(...)` или аналогичного поля. Секреты и stack traces не попадают в UI.

### 2.6 LocalStore и жизненный цикл

1. `LocalStore::open(path)` вызывается в Tokio-потоке при старте (требует Tokio для sqlx).
2. `prepare_startup(&store)` выполняется в Tokio-потоке: загрузка настроек, активный manifest, восстановление прерванных задач.
3. Результат `StartupReport` отправляется в UI через мост (событие `StartupCompleted` или `StartupFailed`).
4. `LocalStore` — `Clone` (оборачивает `SqlitePool`), может быть передан в фоновые задачи (скачивание).
5. При shutdown: `store.close().await` в Tokio-потоке перед остановкой runtime.
6. Путь к БД: решается до открытия (например, `dirs::config_dir()` или рядом с ejecutable). install_root — это настройка, которая сама хранится в БД (курица-яйцо: нужен путь по умолчанию до первого чтения).

### 2.7 Избегание deadlock, race conditions, утечек

| Риск | Причина | Устранение |
|------|---------|------------|
| Deadlock `invoke_from_event_loop` | Tokio-поток ждёт Slint, Slint ждёт что-то от Tokio | `invoke_from_event_loop` вызывается только из Tokio-потока, никогда из UI-потока. UI-поток никогда не блокируется, ожидая Tokio. |
| Race на `CoreControl` | Клоны передаются между потоками | `CoreControl: Clone`, все поля (`mpsc::Sender`, `EventBus`, `watch::Receiver`, `CancellationToken`) — `Send + Sync`. Безопасная передача по значению. |
| Утечка задач | Мостовая задача не завершается | Мостовая задача завершается на `AppEvent::Shutdown` или `cancelled()`. `CoreHandle::shutdown()` гарантирует остановку reducer задачи. |
| Блокировка mpsc | `try_send` на полную очередь | `try_send` никогда не блокируется; возвращает `CommandQueueFull`. |
| Блокировка sqlx | Синхронные вызовы в UI | Все вызовы `LocalStore` — `async`, выполняются в Tokio-потоке. UI не вызывает их напрямую. |
| Паника в runtime | `Runtime::new()` внутри потока | Обернуть в `Result`, логировать, корректно завершить поток. |

### 2.8 Тестирование интеграции

1. **Unit-тест моста:** `tokio::test` с мок-окном (или без окна, проверяя вызовы `invoke_from_event_loop` через `slint::testing` если доступно).
2. **Integration-тест:** Запуск `ApplicationCore` в тестовом Tokio runtime, отправка команд через `CoreControl`, проверка событий через `EventBus::subscribe()`.
3. **Тест shutdown:** Проверить, что `core_handle.shutdown()` завершает задачу в рамках таймаута.
4. **Тест LocalStore + use_cases:** `LocalStore::open_in_memory()`, `prepare_startup()`, сохранение/восстановление.
5. **Launcher-тесты:** Сейчас 0 тестов. После Phase C можно добавить тест, который создаёт окно, шлёт команду через `CoreControl`, проверяет изменение свойств Slint.

### 2.9 Файлы для изменения (Вариант A)

| Файл | Изменение |
|------|-----------|
| `apps/launcher/src/main.rs` | Создание `std::thread`, `tokio::runtime::Runtime`, `ApplicationCore::spawn`, мостовая задача, shutdown логика, замена `Rc<Cell<ScreenState>>` + `Timer` на `CoreControl` |
| `apps/launcher/src/app_bridge.rs` (новый) | Мост EventBus/watch → Slint через `invoke_from_event_loop` |
| `apps/launcher/src/local_state.rs` (новый) | Инициализация `LocalStore`, `prepare_startup`, жизненный цикл БД |
| `apps/launcher/Cargo.toml` | **Без изменений** (Tokio не добавляется) |
| `crates/application/src/core.rs` | Возможно, добавить метод для получения `CoreHandle` из потока или уточнить API shutdown (без изменения поведения) |

## 3. Вариант B — Tokio runtime внутри Launcher

Launcher получает Tokio зависимость и runtime. ApplicationCore запускается внутри него. Slint продолжает использовать собственный event loop.

### 3.1 Создание и уничтожение ApplicationCore

**Создание:**
1. `apps/launcher/Cargo.toml` добавляет `tokio` с features `rt`, `macros`, `time`.
2. `main()` становится `#[tokio::main]` или вручную создаётся `tokio::runtime::Runtime`.
3. Проблема: `slint::Window::run()` блокирует текущий поток. Нужно либо:
   a. Запустить Slint в отдельном потоке (`std::thread::spawn { window.run() }`), а Tokio оставить в главном потоке, или
   b. Запустить Tokio в отдельном потоке, а Slint в главном потоке.
4. `ApplicationCore::spawn()` вызывается на Tokio runtime.
5. `CoreHandle` хранится в Tokio контексте; `CoreControl` клонируется для UI.

**Уничтожение:**
1. При закрытии окна — сигнал в Tokio контекст.
2. `core_handle.shutdown().await` вызывается в Tokio runtime.
3. Если Slint в отдельном потоке — он завершается независимо.
4. Runtime останавливается, процесс завершается.

### 3.2 Отправка команд из UI в Core

То же, что в Варианте A: `core_control.apply(action)` через `try_send`. Разница только в том, где живёт runtime.

### 3.3 Передача событий из Core в UI

То же самое: мостовая задача в Tokio runtime подписывается на `EventBus` и вызывает `slint::invoke_from_event_loop` для обновления Slint свойств.

### 3.4 Shutdown

1. Сигнал закрытия окна → асинхронный обработчик в Tokio runtime.
2. `core_handle.shutdown().await` — graceful остановка reducer.
3. Мостовая задача завершается.
4. Если Slint в отдельном потоке — он продолжает работать до ручного закрытия или получает сигнал quit.
5. Runtime останавливается, процесс завершается.

### 3.5 Обработка ошибок

Те же категории, что и Вариант A. Разница: ошибки можно передавать напрямую через shared memory (Arc<Mutex>) без необходимости межпоточной коммуникации, но `invoke_from_event_loop` всё ещё нужен для обновления Slint.

### 3.6 LocalStore и жизненный цикл

То же, что Вариант A, но `LocalStore::open()` вызывается напрямую в async контексте Tokio без межпоточной передачи.

### 3.7 Избегание deadlock, race conditions, утечек

| Риск | Причина | Устранение |
|------|---------|------------|
| Deadlock Tokio ↔ Slint | Tokio задача ждёт `invoke_from_event_loop`, Slint event loop заблокирован | Никогда не вызывать `invoke_from_event_loop` из UI-потока. Все вызовы из Tokio задач. |
| Блокировка UI от Tokio | Tokio задачи выполняются в этом же потоке (если single-thread) | Использовать `multi_thread` runtime или запускать Slint в отдельном потоке. |
| Блокировка Slint от Tokio | `block_on` в главном потоке где тоже работает Slint | Не вызывать `block_on` там, где работает Slint. Разделить потоки. |
| Утечка `CoreHandle` | `shutdown()` не вызывается | Гарантировать вызов в обработчике закрытия окна. |

### 3.8 Тестирование интеграции

`tokio::test` для логики моста, `slint::testing` для UI. Можно тестировать `ApplicationCore` + `LocalStore` в одном тестовом runtime без реального окна.

### 3.9 Файлы для изменения (Вариант B)

| Файл | Изменение |
|------|-----------|
| `apps/launcher/Cargo.toml` | Добавить `tokio = { version = "1.0", features = ["rt", "macros", "time"] }` |
| `apps/launcher/src/main.rs` | Стать async или создать runtime, запустить Slint в отдельном потоке, интегрировать `ApplicationCore` |
| `apps/launcher/src/app_bridge.rs` (новый) | Мост EventBus → Slint |
| `apps/launcher/src/local_state.rs` (новый) | `LocalStore` жизненный цикл |

## 4. Сравнительная таблица двух архитектур

| Критерий | Вариант A (отдельный поток + Tokio) | Вариант B (Tokio в Launcher) |
|----------|-------------------------------------|------------------------------|
| Зависимость Tokio в launcher | Нет | Да (`apps/launcher/Cargo.toml`) |
| Konkurrenция в главном потоке | Только Slint event loop | Tokio + Slint (требует разделения потоков) |
| Изменение `main.rs` | Значительное (thread + runtime + мост) | Значительное (async runtime + мост) |
| Сложность shutdown | Поток + runtime остановка | Runtime остановка + возможно поток Slint |
| Риск deadlock Tokio↔Slint | Низкий (полное разделение потоков) | Средний (требует аккуратного разделения) |
| Передача `CoreControl` в UI | Через межпоточный канал | Через `Arc`/`Clone` в том же процессе |
| Тестируемость | Хорошая (отдельные тесты для thread и Tokio) | Хорошая (все в одном runtime) |
| Соответствие AGENTS.md | Полное (UI callback без blocking) | Полное (если не блокировать UI loop) |
| Количество новых файлов | 2-3 (`app_bridge.rs`, `local_state.rs`) | 2-3 (`app_bridge.rs`, `local_state.rs`) |
| Изменения в `crates/application` | Минимальные | Минимальные |
| Backward compatibility | Полная (launcher не знает о Tokio) | Требует добавления зависимости |

## 5. Плюсы и минусы

### Вариант A

**Плюсы:**
- Не добавляет Tokio в `apps/launcher` — сохраняет текущую структуру зависимостей.
- Полное разделение ответственности: Slint владеет главный поток, Tokio — фоновый.
- Не блокирует UI под никаким сценарием (мпск очередь ограничена, `try_send` не блокирует).
- `auth_client.rs` уже использует `std::thread` — этот вариант соответствует существующей модели параллелизма в launcher.
- Проще обосновать код-ревьюерам: нет внедрения нового рантайма в UI процесс.

**Минусы:**
- Дополнительный межпоточный канал для передачи `CoreControl` из потока в основной.
- Необходимость аккуратного управления остановкой потока и runtime.
- Немного больше boilerplate для shutdown координации.

### Вариант B

**Плюсы:**
- Простая модель: один процесс, один рантайм, общий доступ к `CoreControl` без межпоточной сериализации.
- Меньше кода для моста (всё в одном адресном пространстве).

**Минусы:**
- Требует добавления Tokio в `apps/launcher` — новая зависимость и новая модель параллелизма.
- Риск блокировки Slint event loop, если Tokio задачи выполняются в том же потоке.
- Требует решения проблемы: `window.run()` блокирует поток, куда девать Tokio?
- Нарушает текущий принцип "launcher не использует Tokio" (auth_client использует `std::thread`).

## 6. Рекомендация

**Рекомендуется Вариант A (отдельный std::thread с собственным Tokio runtime).**

Обоснование для текущей структуры проекта:

1. **AGENTS.md** требует: "Keep UI callbacks free of filesystem, database, network, and hashing work." Вариант A полностью сохраняет это: все Tokio/sqlx операции изолированы в фоновом потоке.
2. **Существующая модель параллелизма** в launcher уже основана на `std::thread` (см. `auth_client.rs`). Вариант A следует этой модели, а не вводит новую.
3. **Нет необходимости менять `apps/launcher/Cargo.toml`** — меньше зависимостей, проще аудит безопасности, быстрее сборка на Windows CI.
4. **`window.run()` остаётся единственным владельцем главного потока** — нет риска блокировки Slint event loop Tokio задачами.
5. **Применяется уже проверенный паттерн** из `auth_client.rs`: фоновый поток + `slint::invoke_from_event_loop` для обновления UI.

Вариант B следует рассмотреть только если в будущем потребуется тесная интеграция множества Tokio-задач с UI (например, прямое использование `tokio::select!` в UI колбэках), что сейчас не требуется.

## 7. Пример предполагаемой структуры кода и жизненного цикла (Вариант A)

### 7.1 Жизненный цикл

```
1. main() создаёт MainWindow, вызывает mock_data::load() и auth_client::bind()
2. main() создаёт std::thread с Tokio runtime
3. Поток вызывает ApplicationCore::spawn() → CoreHandle
4. CoreControl клонируется и передаётся в основной поток через mpsc канал
5. Основной поток получает CoreControl, заменяет Rc<Cell<ScreenState>> на CoreControl
6. UI колбэки вызывают core_control.apply(action) вместо apply_action_weak()
7. Мостовая задача в Tokio-потоке слушает EventBus и вызывает invoke_from_event_loop для обновления Slint
8. При закрытии окна: сигнал shutdown → core_handle.shutdown().await → остановка runtime → завершение потока
9. main() возвращает управление
```

### 7.2 Структура кода (схематично)

**`apps/launcher/src/main.rs`:**
```rust
fn main() -> Result<(), slint::PlatformError> {
    let window = MainWindow::new()?;
    mock_data::load(&window);
    auth_client::bind(&window);

    let (control_tx, control_rx) = std::sync::mpsc::channel();
    let (shutdown_tx, shutdown_rx) = std::sync::mpsc::channel();

    let thread = std::thread::spawn(move || {
        let rt = tokio::runtime::Runtime::new().expect("Tokio runtime");
        let core_handle = rt.block_on(ApplicationCore::spawn(
            ScreenState::NotInstalled, 16, Duration::from_secs(5),
        ));
        let control = core_handle.control();
        let _ = control_tx.send(control);

        // Мост EventBus → Slint
        let mut events = control.events().subscribe();
        rt.block_on(async move {
            while let Ok(event) = events.recv().await {
                match event {
                    AppEvent::ScreenChanged(state) => {
                        let window = window.as_weak();
                        slint::invoke_from_event_loop(move || {
                            if let Some(w) = window.upgrade() {
                                w.set_fixture_state(format!("{state:?}").into());
                                w.set_fixture_state_title(state::title(state).into());
                                // ... остальные поля
                            }
                        });
                    }
                    AppEvent::Shutdown => break,
                    _ => {}
                }
            }
        });

        let _ = shutdown_rx.recv();
        let _ = rt.block_on(core_handle.shutdown());
    });

    let core_control = control_rx.recv().expect("CoreControl");

    window.on_primary_action(move || {
        let action = ...;
        if let Err(e) = core_control.apply(action) {
            window.set_notice(format!("{e}").into());
        }
    });

    window.on_close_requested(move || {
        let _ = shutdown_tx.send(());
        thread.join().expect("thread join");
    });

    window.run()
}
```

**`apps/launcher/src/app_bridge.rs` (новый):**
- Мостовая задача, подписывается на `core_control.events()` и `core_control.state()`.
- Маршализует `AppEvent` в вызовы `window.set_*()` через `slint::invoke_from_event_loop`.

**`apps/launcher/src/local_state.rs` (новый):**
- Открывает `LocalStore` в Tokio-потоке.
- Вызывает `prepare_startup()` при старте.
- Закрывает `LocalStore` при shutdown.

## 8. Риски и способы их устранения

| Риск | Вероятность | Влияние | Способ устранения |
|------|-------------|---------|-------------------|
| Deadlock между Tokio и Slint | Низкая | Критический | `invoke_from_event_loop` только из Tokio-потока; UI-поток nunca не ждёт Tokio |
| Блокировка UI при полной очереди mpsc | Низкая | Средний | `try_send` не блокирует; возвращает `CommandQueueFull`, UI показывает уведомление |
| Утечка задач при shutdown | Средняя | Средний | `CoreHandle::shutdown()` гарантирует остановку; мостовая задача завершается на `Shutdown` |
| Паника в Tokio runtime | Низкая | Критический | `Runtime::new()` обёрнута в `Result`, паника логируется, поток завершается корректно |
| Несоответствие сигнатур `save_settings` | Низкая | Средний | Исправлено в Phase B: `Settings` по значению, валидация в `update_settings` перед вызовом |
| Отсутствие тестов launcher | Высокая | Средний | Добавить тесты после Phase C (создание окна, команда через `CoreControl`, проверка свойств Slint) |

## 9. Ограничения (соблюдаются)

- Не используется polling через Slint Timer для чтения событий Core — используется мостовая задача + `invoke_from_event_loop`.
- Не блокируется Slint event loop — все Tokio/sqlx операции в фоновом потоке.
- Не меняются HTTP-контракты API и авторизация — `auth_client.rs` без изменений.
- Не переписывается `ApplicationCore` без необходимости — используется как есть, только экспорт `CoreControl`.
- Не добавляются абстракции без конкретной необходимости — только мост EventBus → Slint.

---
