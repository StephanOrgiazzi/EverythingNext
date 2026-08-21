#![cfg(windows)]

use std::sync::atomic::{AtomicBool, Ordering};
use tauri_plugin_dialog::{DialogExt, MessageDialogButtons, MessageDialogKind};
use tauri_plugin_updater::UpdaterExt;

const UPDATE_ENDPOINT: &str =
    "https://github.com/StephanOrgiazzi/EverythingNext/releases/latest/download/latest.json";
const UPDATER_PUBLIC_KEY: Option<&str> = option_env!("TAURI_UPDATER_PUBLIC_KEY");
static UPDATE_CHECK_STARTED: AtomicBool = AtomicBool::new(false);

pub fn check_on_user_launch(app: tauri::AppHandle) {
    if UPDATER_PUBLIC_KEY.is_none() || UPDATE_CHECK_STARTED.swap(true, Ordering::AcqRel) {
        return;
    }

    tauri::async_runtime::spawn(async move {
        if let Err(error) = check_and_offer_update(app).await {
            eprintln!("Unable to check for Everything Next updates: {error}");
        }
    });
}

async fn check_and_offer_update(
    app: tauri::AppHandle,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let Some(public_key) = UPDATER_PUBLIC_KEY else {
        return Ok(());
    };

    let endpoint = UPDATE_ENDPOINT.parse()?;
    let updater = app
        .updater_builder()
        .pubkey(public_key)
        .endpoints(vec![endpoint])?
        .build()?;

    let Some(update) = updater.check().await? else {
        return Ok(());
    };

    let version = update.version.clone();
    let should_install = app
        .dialog()
        .message(format!(
            "Everything Next {version} is available. Install it now?"
        ))
        .title("Everything Next update")
        .buttons(MessageDialogButtons::OkCancelCustom(
            "Install".into(),
            "Later".into(),
        ))
        .blocking_show();

    if !should_install {
        return Ok(());
    }

    if let Err(error) = update.download_and_install(|_, _| {}, || {}).await {
        eprintln!("Unable to install the Everything Next update: {error}");
        app.dialog()
            .message(format!(
                "The update could not be installed. Please try again later.\n\n{error}"
            ))
            .title("Everything Next update failed")
            .kind(MessageDialogKind::Error)
            .buttons(MessageDialogButtons::Ok)
            .blocking_show();
    }

    Ok(())
}
