mod app_bridge;
mod auth_client;
mod local_state;
mod mock_data;
mod state;

slint::include_modules!();

use std::sync::mpsc;
use std::thread;
use std::time::Duration;

use gta6rp_application::ApplicationCore;
use gta6rp_domain::screen::{ScreenAction, ScreenState};

fn main() -> Result<(), slint::PlatformError> {
    let window = MainWindow::new()?;
    mock_data::load(&window);
    auth_client::bind(&window);

    // Channels for handing CoreControl, StartupReport, free space info, and
    // shutdown signal between threads.
    let (control_tx, control_rx) = mpsc::channel();
    let (startup_tx, startup_rx) = mpsc::channel();
    let (freespace_tx, freespace_rx) = mpsc::channel();
    let (shutdown_tx, shutdown_rx) = mpsc::channel();

    let weak_window = window.as_weak();

    // ApplicationCore and LocalStore run in a dedicated system thread with
    // their own Tokio runtime. The Slint event loop keeps the main thread.
    let thread = thread::spawn(move || {
        let rt = tokio::runtime::Runtime::new().expect("failed to create Tokio runtime");

        // Open local state and run startup preparation.
        let (local_state, startup_report) = rt
            .block_on(local_state::LocalStateHandle::open())
            .expect("failed to open local state");
        let _ = startup_tx.send(startup_report);

        // Check free space on the database directory for display in UI.
        if let Ok(db_path) = local_state::database_path()
            && let Some(parent) = db_path.parent()
        {
            let result = local_state::LocalStateHandle::check_free_space(parent, 0);
            let _ = freespace_tx.send(result);
        }

        // Spawn the application core.
        let core_handle = rt.block_on(async {
            ApplicationCore::spawn(ScreenState::NotInstalled, 16, Duration::from_secs(5))
        });
        let control = core_handle.control();
        let events = control.events();
        let _ = control_tx.send(control);

        // Bridge EventBus updates to the Slint UI thread.
        let window_weak = weak_window.clone();
        rt.block_on(app_bridge::run_event_loop(events, window_weak));

        // Wait for shutdown signal from the UI thread, then stop the core
        // and close the local store.
        let _ = shutdown_rx.recv();
        let _ = rt.block_on(core_handle.shutdown());
        rt.block_on(local_state.close());
    });

    let core_control = control_rx
        .recv()
        .expect("failed to receive CoreControl from application thread");
    let startup_report = startup_rx
        .recv()
        .expect("failed to receive startup report from application thread");
    let freespace_info = freespace_rx.recv().ok().and_then(Result::ok);

    // Apply initial state from the core to the UI.
    let initial_state = core_control.current_state();
    app_bridge::apply_ui_state(&window, initial_state);

    // Apply loaded settings to the UI.
    let settings = startup_report.settings;
    apply_settings_to_ui(&window, &settings, freespace_info.as_ref());

    let control_primary = core_control.clone();
    window.on_primary_action(move || {
        let action = match control_primary.current_state() {
            ScreenState::NotInstalled | ScreenState::RepairRequired | ScreenState::Error => {
                ScreenAction::BeginSetup
            }
            ScreenState::Ready => ScreenAction::LaunchStarted,
            ScreenState::Running => ScreenAction::ResetPreview,
            _ => return,
        };
        let _ = control_primary.apply(action);
    });

    let control_error = core_control.clone();
    window.on_simulate_error(move || {
        let _ = control_error.apply(ScreenAction::Fail);
    });

    let control_repair = core_control.clone();
    window.on_simulate_repair(move || {
        let _ = control_repair.apply(ScreenAction::RequireRepair);
    });

    let control_pause = core_control.clone();
    window.on_toggle_pause(move || {
        let action = match control_pause.current_state() {
            ScreenState::Downloading => ScreenAction::PauseRequested,
            ScreenState::Paused => ScreenAction::ResumeRequested,
            _ => return,
        };
        let _ = control_pause.apply(action);
    });

    window.run()?;

    // Signal shutdown to the background thread, then join it.
    let _ = shutdown_tx.send(());
    let _ = thread.join();
    Ok(())
}

/// Apply settings values and free space info to Slint UI properties.
fn apply_settings_to_ui(
    window: &MainWindow,
    settings: &gta6rp_domain::settings::Settings,
    freespace_info: Option<&gta6rp_installation::FreeSpaceInfo>,
) {
    let mut notice = format!(
        "Настройки загружены: язык={}, параллельных загрузок={}",
        settings.language, settings.max_parallel_downloads
    );
    if let Some(info) = freespace_info {
        use std::fmt::Write;
        let _ = write!(
            notice,
            ". Свободно места: {} байт из {}",
            info.available_bytes, info.total_bytes
        );
    }
    window.set_notice(notice.into());
}
