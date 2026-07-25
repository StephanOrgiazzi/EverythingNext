use std::sync::Arc;
use tauri::State;
use windows_shell::IconCache;

pub(crate) struct ShellState {
    icons: Arc<IconCache>,
    icon_slots: Arc<tokio::sync::Semaphore>,
}

impl ShellState {
    pub(crate) fn new() -> Self {
        Self {
            icons: Arc::new(IconCache::new(512)),
            icon_slots: Arc::new(tokio::sync::Semaphore::new(4)),
        }
    }
}

#[tauri::command]
pub(crate) async fn get_file_icon(
    state: State<'_, ShellState>,
    path: String,
) -> Result<Option<String>, String> {
    let permit = state
        .icon_slots
        .clone()
        .acquire_owned()
        .await
        .map_err(|_| "Icon service has stopped".to_string())?;
    let icons = state.icons.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let _permit = permit;
        icons.get(&path).map_err(|error| error.to_string())
    })
    .await
    .map_err(|error| error.to_string())?
}

#[tauri::command]
pub(crate) async fn copy_text(text: String) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || {
        windows_shell::copy_text(&text).map_err(|error| error.to_string())
    })
    .await
    .map_err(|error| error.to_string())?
}

#[tauri::command]
pub(crate) async fn open_path(path: String) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || {
        windows_shell::open_path(&path).map_err(|error| error.to_string())
    })
    .await
    .map_err(|error| error.to_string())?
}

#[tauri::command]
pub(crate) async fn reveal_path(path: String) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || {
        windows_shell::reveal_path(&path).map_err(|error| error.to_string())
    })
    .await
    .map_err(|error| error.to_string())?
}

#[tauri::command]
pub(crate) async fn rename_path(path: String, new_name: String) -> Result<String, String> {
    tauri::async_runtime::spawn_blocking(move || {
        windows_shell::rename_path(&path, &new_name)
            .map(|path| path.to_string_lossy().into_owned())
            .map_err(|error| error.to_string())
    })
    .await
    .map_err(|error| error.to_string())?
}
