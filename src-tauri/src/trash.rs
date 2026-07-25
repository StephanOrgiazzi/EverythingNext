use crate::search::{request_is_stale, SearchState};
use everything_core::{SelectionRequest, TrashOutcome, TrashPreparation};
use std::collections::{HashMap, HashSet};
use std::sync::{
    atomic::{AtomicU64, Ordering},
    Mutex,
};
use tauri::State;

const MAX_TRASH_ITEMS: usize = 10_000;
const MAX_SELECTION_RANGES: usize = 16_384;

pub(crate) struct TrashState {
    snapshots: Mutex<HashMap<u64, Vec<String>>>,
    in_flight: Mutex<HashSet<u64>>,
    next_snapshot: AtomicU64,
}

impl TrashState {
    pub(crate) fn new() -> Self {
        Self {
            snapshots: Mutex::new(HashMap::new()),
            in_flight: Mutex::new(HashSet::new()),
            next_snapshot: AtomicU64::new(1),
        }
    }

    fn store(&self, paths: Vec<String>) -> Result<TrashPreparation, String> {
        let snapshot_id = self.next_snapshot.fetch_add(1, Ordering::SeqCst);
        let count = paths.len();
        self.snapshots
            .lock()
            .map_err(|_| "Deletion storage is unavailable".to_string())?
            .insert(snapshot_id, paths);
        Ok(TrashPreparation { snapshot_id, count })
    }

    fn cancel(&self, snapshot_id: u64) {
        if let Ok(mut snapshots) = self.snapshots.lock() {
            snapshots.remove(&snapshot_id);
        }
    }

    fn take_for_execution(&self, snapshot_id: u64) -> Result<Vec<String>, String> {
        {
            let mut in_flight = self
                .in_flight
                .lock()
                .map_err(|_| "Deletion state is unavailable".to_string())?;
            if !in_flight.insert(snapshot_id) {
                return Err("This deletion is already in progress".into());
            }
        }

        match self.snapshots.lock() {
            Ok(mut snapshots) => match snapshots.remove(&snapshot_id) {
                Some(paths) => Ok(paths),
                None => {
                    self.finish_execution(snapshot_id);
                    Err("This confirmation has expired or was already used".into())
                }
            },
            Err(_) => {
                self.finish_execution(snapshot_id);
                Err("Deletion storage is unavailable".into())
            }
        }
    }

    fn finish_execution(&self, snapshot_id: u64) {
        if let Ok(mut in_flight) = self.in_flight.lock() {
            in_flight.remove(&snapshot_id);
        }
    }
}

#[tauri::command]
pub(crate) async fn prepare_trash_selection(
    search: State<'_, SearchState>,
    trash: State<'_, TrashState>,
    request: SelectionRequest,
) -> Result<TrashPreparation, String> {
    if request.ranges.is_empty() {
        return Err("No items selected".into());
    }
    if request.ranges.len() > MAX_SELECTION_RANGES {
        return Err("Invalid selection: too many disjoint ranges".into());
    }
    if request_is_stale(request.request_id, &search.latest_generation)
        || request.request_id != search.latest_generation.load(Ordering::SeqCst)
    {
        return Err("The search changed since the selection was made. Try again.".into());
    }

    let engine = search.engine.clone();
    let latest_generation = search.latest_generation.clone();
    let request_id = request.request_id;
    let paths = tauri::async_runtime::spawn_blocking(move || {
        let mut guard = engine
            .lock()
            .map_err(|_| "Everything lock was poisoned".to_string())?;
        let engine = guard
            .as_mut()
            .ok_or_else(|| "Everything engine is unavailable".to_string())?;
        engine
            .resolve_selection_cancellable(request, MAX_TRASH_ITEMS, || {
                request_is_stale(request_id, &latest_generation)
            })
            .map_err(|error| error.to_string())
    })
    .await
    .map_err(|error| error.to_string())??;

    if request_id != search.latest_generation.load(Ordering::SeqCst) {
        return Err("The search changed while preparing the operation.".into());
    }
    trash.store(paths)
}

#[tauri::command]
pub(crate) fn cancel_trash_snapshot(trash: State<'_, TrashState>, snapshot_id: u64) {
    trash.cancel(snapshot_id);
}

#[tauri::command]
pub(crate) async fn execute_trash_snapshot(
    trash: State<'_, TrashState>,
    snapshot_id: u64,
) -> Result<TrashOutcome, String> {
    let paths = trash.take_for_execution(snapshot_id)?;
    let result = tauri::async_runtime::spawn_blocking(move || {
        let report = windows_shell::trash_paths(&paths);
        TrashOutcome {
            deleted: report.deleted,
            failures: report.failures,
        }
    })
    .await
    .map_err(|error| error.to_string());
    trash.finish_execution(snapshot_id);
    result
}
