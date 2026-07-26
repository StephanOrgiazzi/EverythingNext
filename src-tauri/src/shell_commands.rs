use std::sync::Arc;
use tauri::State;
use windows_shell::{VisualCache, VisualKind};

pub(crate) struct ShellState {
    visuals: Arc<VisualCache>,
    visual_slots: Arc<tokio::sync::Semaphore>,
}

impl ShellState {
    pub(crate) fn new() -> Self {
        Self {
            visuals: Arc::new(VisualCache::new(24 * 1024 * 1024)),
            visual_slots: Arc::new(tokio::sync::Semaphore::new(8)),
        }
    }
}

#[tauri::command]
pub(crate) async fn get_file_visual(
    state: State<'_, ShellState>,
    path: String,
    thumbnail: bool,
) -> Result<Option<String>, String> {
    let permit = state
        .visual_slots
        .clone()
        .acquire_owned()
        .await
        .map_err(|_| "File preview service has stopped".to_string())?;
    let visuals = state.visuals.clone();
    let kind = if thumbnail {
        VisualKind::Thumbnail
    } else {
        VisualKind::Icon
    };
    tauri::async_runtime::spawn_blocking(move || {
        let _permit = permit;
        visuals.get(&path, kind).map_err(|error| error.to_string())
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
