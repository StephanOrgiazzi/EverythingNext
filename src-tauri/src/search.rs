use crate::trash::TrashState;
use everything_core::{EngineStatus, EverythingEngine, QueryRequest, SearchPage, SelectionRequest};
use std::sync::{
    atomic::{AtomicU32, Ordering},
    Arc, Mutex,
};
use tauri::{path::BaseDirectory, Manager, State};

pub(crate) struct SearchState {
    pub engine: Arc<Mutex<Option<EverythingEngine>>>,
    engine_error: Option<String>,
    pub latest_generation: Arc<AtomicU32>,
}

impl SearchState {
    pub(crate) fn initialize<R: tauri::Runtime>(app: &tauri::App<R>) -> Self {
        let bundled_dll = app
            .path()
            .resolve("Everything3_x64.dll", BaseDirectory::Resource)
            .ok()
            .filter(|path| path.is_file());
        let result = bundled_dll
            .as_deref()
            .map(EverythingEngine::from_dll_path)
            .unwrap_or_else(EverythingEngine::new);
        let (engine, engine_error) = match result {
            Ok(engine) => (Some(engine), None),
            Err(error) => (None, Some(error.to_string())),
        };

        Self {
            engine: Arc::new(Mutex::new(engine)),
            engine_error,
            latest_generation: Arc::new(AtomicU32::new(0)),
        }
    }
}

#[tauri::command]
pub(crate) async fn engine_status(state: State<'_, SearchState>) -> Result<EngineStatus, String> {
    if let Some(error) = &state.engine_error {
        return Ok(EngineStatus {
            available: false,
            indexing: false,
            ready_volumes: 0,
            total_volumes: 0,
            message: error.clone(),
            version: None,
        });
    }

    let engine = state.engine.clone();
    tauri::async_runtime::spawn_blocking(move || {
        engine
            .lock()
            .map_err(|_| "Everything lock was poisoned".to_string())?
            .as_ref()
            .map(EverythingEngine::status)
            .ok_or_else(|| "Everything engine is unavailable".to_string())
    })
    .await
    .map_err(|error| error.to_string())?
}

#[tauri::command]
pub(crate) fn begin_search_generation(state: State<'_, SearchState>, request_id: u32) {
    state
        .latest_generation
        .fetch_max(request_id, Ordering::SeqCst);
}

#[tauri::command]
pub(crate) async fn search_everything(
    state: State<'_, SearchState>,
    trash: State<'_, TrashState>,
    request: QueryRequest,
) -> Result<SearchPage, String> {
    state
        .latest_generation
        .fetch_max(request.request_id, Ordering::SeqCst);
    if request_is_stale(request.request_id, &state.latest_generation) {
        return Err("Stale search request".into());
    }

    let engine = state.engine.clone();
    let latest_generation = state.latest_generation.clone();
    let mut page = tauri::async_runtime::spawn_blocking(move || {
        if request_is_stale(request.request_id, &latest_generation) {
            return Err("Stale search request".to_string());
        }

        let mut guard = engine
            .lock()
            .map_err(|_| "Everything lock was poisoned".to_string())?;
        if request_is_stale(request.request_id, &latest_generation) {
            return Err("Stale search request".to_string());
        }

        let engine = guard.as_mut().ok_or_else(|| {
            "Everything SDK3 is unavailable. Install SDK3, then start Everything 1.5.".to_string()
        })?;
        engine.query(request).map_err(|error| error.to_string())
    })
    .await
    .map_err(|error| error.to_string())??;
    trash.filter_deleted(&mut page);
    Ok(page)
}

pub(crate) async fn resolve_selection(
    state: &SearchState,
    request: SelectionRequest,
    max_items: usize,
) -> Result<Vec<String>, String> {
    const MAX_SELECTION_RANGES: usize = 16_384;

    if request.ranges.is_empty() {
        return Err("No items selected".into());
    }
    if request.ranges.len() > MAX_SELECTION_RANGES {
        return Err("Invalid selection: too many disjoint ranges".into());
    }
    if request_is_stale(request.request_id, &state.latest_generation)
        || request.request_id != state.latest_generation.load(Ordering::SeqCst)
    {
        return Err("The search changed since the selection was made. Try again.".into());
    }

    let engine = state.engine.clone();
    let latest_generation = state.latest_generation.clone();
    let request_id = request.request_id;
    let paths = tauri::async_runtime::spawn_blocking(move || {
        let mut guard = engine
            .lock()
            .map_err(|_| "Everything lock was poisoned".to_string())?;
        let engine = guard
            .as_mut()
            .ok_or_else(|| "Everything engine is unavailable".to_string())?;
        engine
            .resolve_selection_cancellable(request, max_items, || {
                request_is_stale(request_id, &latest_generation)
            })
            .map_err(|error| error.to_string())
    })
    .await
    .map_err(|error| error.to_string())??;

    if request_id != state.latest_generation.load(Ordering::SeqCst) {
        return Err("The search changed while preparing the operation.".into());
    }
    Ok(paths)
}

pub(crate) fn request_is_stale(request_id: u32, latest_generation: &AtomicU32) -> bool {
    request_id < latest_generation.load(Ordering::SeqCst)
}
