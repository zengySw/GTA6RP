use gta6rp_domain::screen::ScreenState;

pub fn title(state: ScreenState) -> &'static str {
    match state {
        ScreenState::NotInstalled => "Fixture not installed",
        ScreenState::Detecting => "Looking for the fixture",
        ScreenState::Verifying => "Verifying fixture files",
        ScreenState::UpdateAvailable => "Fixture setup available",
        ScreenState::Downloading => "Preparing the fixture",
        ScreenState::Paused => "Fixture setup paused",
        ScreenState::Ready => "Fixture is ready",
        ScreenState::Launching => "Starting the test application",
        ScreenState::Running => "Test application is running",
        ScreenState::RepairRequired => "Fixture repair required",
        ScreenState::Error => "Fixture preview error",
    }
}

pub fn detail(state: ScreenState) -> &'static str {
    match state {
        ScreenState::NotInstalled => {
            "The separate synthetic fixture has not been selected or installed."
        }
        ScreenState::Detecting => {
            "Checking the simulated fixture state. No files are being accessed."
        }
        ScreenState::Verifying => {
            "A preview of the verification state. No hashes are being calculated."
        }
        ScreenState::UpdateAvailable => "The mock provider found a sample fixture setup.",
        ScreenState::Downloading => {
            "Simulating progress only. No network requests or disk writes are made."
        }
        ScreenState::Paused => "The mock setup is paused. No background download exists.",
        ScreenState::Ready => "The mock fixture is ready. No game or fixture files are present.",
        ScreenState::Launching => "Showing the launch transition. No process is being started.",
        ScreenState::Running => {
            "The mock test application state is active; no process was launched."
        }
        ScreenState::RepairRequired => "The mock provider marked the fixture as needing repair.",
        ScreenState::Error => {
            "This error is simulated to preview recovery and notification states."
        }
    }
}

pub fn badge(state: ScreenState) -> &'static str {
    match state {
        ScreenState::NotInstalled => "NOT INSTALLED",
        ScreenState::Detecting => "CHECKING",
        ScreenState::Verifying => "VERIFYING",
        ScreenState::UpdateAvailable => "SETUP AVAILABLE",
        ScreenState::Downloading => "PREPARING",
        ScreenState::Paused => "PAUSED",
        ScreenState::Ready => "READY",
        ScreenState::Launching => "STARTING",
        ScreenState::Running => "RUNNING · MOCK",
        ScreenState::RepairRequired => "REPAIR REQUIRED",
        ScreenState::Error => "SIMULATED ERROR",
    }
}

pub fn action_label(state: ScreenState) -> &'static str {
    match state {
        ScreenState::NotInstalled | ScreenState::RepairRequired | ScreenState::Error => {
            "Simulate fixture install"
        }
        ScreenState::Ready => "Simulate launch",
        ScreenState::Running => "Reset preview",
        ScreenState::Detecting
        | ScreenState::Verifying
        | ScreenState::UpdateAvailable
        | ScreenState::Downloading
        | ScreenState::Paused
        | ScreenState::Launching => "Please wait…",
    }
}

pub fn notice(state: ScreenState) -> &'static str {
    match state {
        ScreenState::NotInstalled => "Mock state: fixture not installed.",
        ScreenState::Detecting => "Mock state: detecting fixture.",
        ScreenState::Verifying => "Mock state: verifying fixture.",
        ScreenState::UpdateAvailable => "Mock state: setup is available.",
        ScreenState::Downloading => "Mock state: preparing fixture.",
        ScreenState::Paused => "Mock state: setup paused.",
        ScreenState::Ready => "Mock state: fixture ready.",
        ScreenState::Launching => "Mock state: launching test application.",
        ScreenState::Running => "Mock state: test application running.",
        ScreenState::RepairRequired => "Mock state: repair required.",
        ScreenState::Error => "Mock state: simulated error. Retry or reset the preview.",
    }
}
