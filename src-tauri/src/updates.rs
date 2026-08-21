#![cfg(windows)]

use tauri_plugin_dialog::{DialogExt, MessageDialogButtons};
use tauri_plugin_updater::UpdaterExt;

const UPDATE_ENDPOINT: &str =
    "https://github.com/StephanOrgiazzi/EverythingNext/releases/latest/download/latest.json";
const UPDATER_PUBLIC_KEY: Option<&str> = option_env!("TAURI_UPDATER_PUBLIC_KEY");

pub fn check_on_start(app: tauri::AppHandle) {
    if UPDATER_PUBLIC_KEY.is_none() {
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

    update.download_and_install(|_, _| {}, || {}).await?;
    Ok(())
}
