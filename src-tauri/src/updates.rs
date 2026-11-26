//! Updates from GitHub releases (tauri-plugin-updater).
//! The release workflow signs each build and publishes `latest.json`. Heyo checks it at startup and
//! every few hours, but never installs on its own: it tells the user (a banner in the popup, a tray
//! menu item) and installs only when they click.

use crate::{AppState, log, popup, rebuild_tray_menu};
use std::time::Duration;
use tauri::{AppHandle, Manager};
use tauri_plugin_dialog::{DialogExt, MessageDialogButtons};
use tauri_plugin_updater::{Update, UpdaterExt};

const FIRST_CHECK_DELAY: Duration = Duration::from_secs(20);
const CHECK_EVERY: Duration = Duration::from_secs(6 * 60 * 60);

/// Periodic checks, when "Check for updates" is on. Skipped in development builds.
pub fn start_background_checks(app: &AppHandle) {
    if cfg!(debug_assertions) {
        return;
    }
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(FIRST_CHECK_DELAY).await;
        loop {
            if app.state::<AppState>().settings().check_updates {
                check(&app, false).await;
            }
            tokio::time::sleep(CHECK_EVERY).await;
        }
    });
}

/// Version of the update waiting to be installed, if any.
pub fn pending_version(app: &AppHandle) -> Option<String> {
    app.state::<AppState>()
        .pending_update
        .lock()
        .unwrap()
        .as_ref()
        .map(|u| u.version.clone())
}

/// `manual` = the user asked from the tray menu: tell them the outcome either way,
/// and offer to install right away.
pub async fn check(app: &AppHandle, manual: bool) {
    let current = app.package_info().version.to_string();
    let found = match app.updater() {
        Ok(updater) => updater.check().await,
        Err(e) => Err(e),
    };
    match found {
        Ok(Some(update)) => {
            let version = update.version.clone();
            log(&format!("update v{version} available (current v{current})"));
            *app.state::<AppState>().pending_update.lock().unwrap() = Some(update);
            rebuild_tray_menu(app);
            if manual {
                let app2 = app.clone();
                app.dialog()
                    .message(format!(
                        "Heyo v{version} is available (you have v{current})."
                    ))
                    .title("Heyo")
                    .buttons(MessageDialogButtons::OkCancelCustom(
                        "Install and restart".into(),
                        "Later".into(),
                    ))
                    .show(move |install| {
                        if install {
                            tauri::async_runtime::spawn(
                                async move { install_pending(&app2).await },
                            );
                        }
                    });
            }
        }
        Ok(None) => {
            if manual {
                message(app, &format!("Heyo is up to date (v{current})."));
            }
        }
        Err(e) => {
            log(&format!("update check failed: {e}"));
            if manual {
                message(app, &format!("Couldn't check for updates: {e}"));
            }
        }
    }
}

/// Downloads and installs the pending update, then restarts. Errors are shown to the user.
pub async fn install_pending(app: &AppHandle) {
    let update: Option<Update> = app
        .state::<AppState>()
        .pending_update
        .lock()
        .unwrap()
        .clone();
    let Some(update) = update else { return };
    let _ = popup(app).hide();
    log(&format!("installing update v{}", update.version));
    match update.download_and_install(|_, _| {}, || {}).await {
        Ok(()) => app.restart(),
        Err(e) => {
            log(&format!("update install failed: {e}"));
            message(app, &format!("Couldn't install the update: {e}"));
        }
    }
}

fn message(app: &AppHandle, text: &str) {
    app.dialog().message(text).title("Heyo").show(|_| {});
}
