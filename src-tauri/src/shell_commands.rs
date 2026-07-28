use crate::search::{resolve_selection, SearchState};
use everything_core::{SelectionRequest, MAX_CONCURRENT_THUMBNAIL_LOADS};
use std::sync::Arc;
use tauri::State;
use windows_shell::{VisualCache, VisualKind};

pub(crate) struct ShellState {
    icons: Arc<VisualCache>,
    icon_slots: Arc<tokio::sync::Semaphore>,
    thumbnails: Arc<VisualCache>,
    thumbnail_slots: Arc<tokio::sync::Semaphore>,
}

impl ShellState {
    pub(crate) fn new() -> Self {
        Self {
            icons: Arc::new(VisualCache::new(8 * 1024 * 1024)),
            icon_slots: Arc::new(tokio::sync::Semaphore::new(2)),
            thumbnails: Arc::new(VisualCache::new(24 * 1024 * 1024)),
            thumbnail_slots: Arc::new(tokio::sync::Semaphore::new(MAX_CONCURRENT_THUMBNAIL_LOADS)),
        }
    }
}

#[tauri::command]
pub(crate) async fn get_file_visual(
    state: State<'_, ShellState>,
    path: String,
    size: u32,
    thumbnail: bool,
) -> Result<Option<String>, String> {
    let (visuals, slots, kind, stopped_message) = if thumbnail {
        if !(32..=256).contains(&size) {
            return Err("File preview size must be between 32 and 256 pixels".into());
        }
        (
            state.thumbnails.clone(),
            state.thumbnail_slots.clone(),
            VisualKind::Thumbnail(size),
            "Thumbnail service has stopped",
        )
    } else {
        (
            state.icons.clone(),
            state.icon_slots.clone(),
            VisualKind::Icon,
            "Icon service has stopped",
        )
    };

    let permit = slots
        .acquire_owned()
        .await
        .map_err(|_| stopped_message.to_string())?;
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
pub(crate) async fn copy_files(
    search: State<'_, SearchState>,
    request: SelectionRequest,
) -> Result<(), String> {
    const MAX_CLIPBOARD_ITEMS: usize = 10_000;

    let paths = resolve_selection(&search, request, MAX_CLIPBOARD_ITEMS).await?;
    tauri::async_runtime::spawn_blocking(move || {
        windows_shell::copy_files(&paths).map_err(|error| error.to_string())
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
