//! Updates from the GitHub releases: checked in the background, downloaded quietly,
//! installed when the user clicks Restart. Nothing interrupts the work in between.

use std::sync::Mutex;
use std::time::Duration;

use serde::Serialize;
use tauri::{AppHandle, Manager};
use tauri_plugin_updater::{Update, UpdaterExt};

/// How often a running app looks for a new release.
const EVERY: Duration = Duration::from_secs(6 * 60 * 60);

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "state", rename_all = "camelCase")]
pub enum UpdateState {
    /// Nothing new, or not checked yet.
    Idle,
    #[serde(rename_all = "camelCase")]
    Downloading { version: String, percent: Option<u8> },
    #[serde(rename_all = "camelCase")]
    Ready { version: String, notes: Option<String> },
    /// The check or the download failed. Shown quietly; the next check tries again.
    #[serde(rename_all = "camelCase")]
    Failed { message: String },
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateStatus {
    pub current: String,
    #[serde(flatten)]
    pub state: UpdateState,
}

/// What the app keeps about updates: the state for the interface and, once downloaded,
/// the release itself, ready to install.
pub struct Updates {
    state: Mutex<UpdateState>,
    ready: Mutex<Option<(Update, Vec<u8>)>>,
}

impl Updates {
    pub fn new() -> Self {
        Self { state: Mutex::new(UpdateState::Idle), ready: Mutex::new(None) }
    }

    fn set(&self, state: UpdateState) {
        if let Ok(mut current) = self.state.lock() {
            *current = state;
        }
    }

    pub fn status(&self, app: &AppHandle) -> UpdateStatus {
        let state = self.state.lock().map(|s| s.clone()).unwrap_or(UpdateState::Idle);
        UpdateStatus { current: app.package_info().version.to_string(), state }
    }
}

/// Starts the background loop. Debug builds stay quiet unless `GEDOENS_UPDATES=1`,
/// so `cargo run` never replaces itself with a release.
pub fn start(app: &AppHandle) {
    if cfg!(debug_assertions) && std::env::var_os("GEDOENS_UPDATES").is_none() {
        return;
    }
    let app = app.clone();
    // A quiet thread of its own: check, then wait, for as long as the app runs.
    std::thread::spawn(move || loop {
        tauri::async_runtime::block_on(check(&app));
        std::thread::sleep(EVERY);
    });
}

/// Looks for a release and downloads it. Does nothing while one is already waiting.
pub async fn check(app: &AppHandle) {
    let updates = app.state::<Updates>();
    if updates.ready.lock().map(|r| r.is_some()).unwrap_or(false) {
        return;
    }
    let found = match app.updater() {
        Ok(updater) => updater.check().await,
        Err(error) => return updates.set(UpdateState::Failed { message: error.to_string() }),
    };
    let update = match found {
        Ok(Some(update)) => update,
        Ok(None) => return updates.set(UpdateState::Idle),
        Err(error) => return updates.set(UpdateState::Failed { message: error.to_string() }),
    };
    let version = update.version.clone();
    updates.set(UpdateState::Downloading { version: version.clone(), percent: Some(0) });

    let mut received = 0usize;
    let progress_app = app.clone();
    let progress_version = version.clone();
    let bytes = update
        .download(
            move |chunk, total| {
                received += chunk;
                let percent = total.filter(|t| *t > 0).map(|t| ((received as u64 * 100) / t).min(100) as u8);
                progress_app
                    .state::<Updates>()
                    .set(UpdateState::Downloading { version: progress_version.clone(), percent });
            },
            || {},
        )
        .await;
    match bytes {
        Ok(bytes) => {
            let notes = update.body.clone().filter(|b| !b.trim().is_empty());
            if let Ok(mut ready) = updates.ready.lock() {
                *ready = Some((update, bytes));
            }
            updates.set(UpdateState::Ready { version, notes });
        }
        Err(error) => updates.set(UpdateState::Failed { message: error.to_string() }),
    }
}

/// Installs the downloaded release and starts the new version.
pub fn install_and_restart(app: &AppHandle) -> Result<(), String> {
    let updates = app.state::<Updates>();
    let (update, bytes) = updates
        .ready
        .lock()
        .map_err(|_| "Internal error.".to_owned())?
        .take()
        .ok_or("There is no update ready to install.")?;
    update.install(bytes).map_err(|e| e.to_string())?;
    app.restart();
}
