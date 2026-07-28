use crate::search::{resolve_selection, SearchState};
use everything_core::{SelectionRequest, TrashOutcome, TrashPreparation};
use std::collections::{HashMap, HashSet};
use std::sync::{
    atomic::{AtomicU64, Ordering},
    Mutex,
};
use tauri::State;

const MAX_TRASH_ITEMS: usize = 10_000;

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
    let paths = resolve_selection(&search, request, MAX_TRASH_ITEMS).await?;
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
