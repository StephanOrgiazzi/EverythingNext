use crate::search::{resolve_selection, SearchState};
use everything_core::{SearchPage, SelectionRequest, TrashOutcome, TrashPreparation};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{
    atomic::{AtomicU64, Ordering},
    Mutex,
};
use tauri::{Manager, Runtime, State};

const MAX_TRASH_ITEMS: usize = 10_000;
const DELETED_PATHS_FILE: &str = "deleted-paths.txt";

pub(crate) struct TrashState {
    snapshots: Mutex<HashMap<u64, Vec<String>>>,
    in_flight: Mutex<HashSet<u64>>,
    deleted_paths: Mutex<HashSet<String>>,
    deleted_paths_file: Option<PathBuf>,
    next_snapshot: AtomicU64,
}

impl TrashState {
    pub(crate) fn initialize<R: Runtime>(app: &tauri::App<R>) -> Self {
        let deleted_paths_file = app
            .path()
            .app_local_data_dir()
            .ok()
            .map(|directory| directory.join(DELETED_PATHS_FILE));
        let deleted_paths = deleted_paths_file
            .as_deref()
            .and_then(|path| fs::read_to_string(path).ok())
            .map(|contents| {
                contents
                    .lines()
                    .map(normalized_path)
                    .filter(|path| !path.is_empty())
                    .collect()
            })
            .unwrap_or_default();

        Self {
            snapshots: Mutex::new(HashMap::new()),
            in_flight: Mutex::new(HashSet::new()),
            deleted_paths: Mutex::new(deleted_paths),
            deleted_paths_file,
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

        let Ok(mut snapshots) = self.snapshots.lock() else {
            self.finish_execution(snapshot_id);
            return Err("Deletion storage is unavailable".into());
        };
        let paths = snapshots.remove(&snapshot_id);
        drop(snapshots);
        let Some(paths) = paths else {
            self.finish_execution(snapshot_id);
            return Err("This confirmation has expired or was already used".into());
        };
        Ok(paths)
    }

    fn finish_execution(&self, snapshot_id: u64) {
        if let Ok(mut in_flight) = self.in_flight.lock() {
            in_flight.remove(&snapshot_id);
        }
    }

    fn remember_deleted(&self, paths: &[String]) {
        let Ok(mut deleted_paths) = self.deleted_paths.lock() else {
            return;
        };
        deleted_paths.extend(paths.iter().map(|path| normalized_path(path)));
        self.persist_deleted_paths(&deleted_paths);
    }

    pub(crate) fn filter_deleted(&self, page: &mut SearchPage) {
        let Ok(mut deleted_paths) = self.deleted_paths.lock() else {
            return;
        };
        if deleted_paths.is_empty() {
            return;
        }

        let mut removed = 0_u32;
        let mut revived = false;
        page.items.retain(|item| {
            let path = normalized_path(&item.full_path);
            if !deleted_paths.contains(&path) {
                return true;
            }
            if Path::new(&item.full_path).exists() {
                deleted_paths.remove(&path);
                revived = true;
                true
            } else {
                removed = removed.saturating_add(1);
                false
            }
        });
        page.total = page.total.saturating_sub(removed);
        if revived {
            self.persist_deleted_paths(&deleted_paths);
        }
    }

    fn persist_deleted_paths(&self, deleted_paths: &HashSet<String>) {
        let Some(path) = &self.deleted_paths_file else {
            return;
        };
        let Some(directory) = path.parent() else {
            return;
        };
        if let Err(error) = fs::create_dir_all(directory) {
            eprintln!("Unable to create deleted-path storage: {error}");
            return;
        }
        let mut paths = deleted_paths.iter().map(String::as_str).collect::<Vec<_>>();
        paths.sort_unstable();
        if let Err(error) = fs::write(path, paths.join("\n")) {
            eprintln!("Unable to persist deleted paths: {error}");
        }
    }
}

fn normalized_path(path: &str) -> String {
    path.to_lowercase()
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

#[expect(
    clippy::needless_pass_by_value,
    reason = "Tauri command injection requires State as an owned extractor"
)]
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
    let result = tauri::async_runtime::spawn_blocking(move || windows_shell::trash_paths(&paths))
        .await
        .map_err(|error| error.to_string());
    if let Ok(outcome) = &result {
        trash.remember_deleted(&outcome.deleted_paths);
    }
    trash.finish_execution(snapshot_id);
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use everything_core::SearchResult;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn filters_persisted_deletions_but_allows_recreated_paths() {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        let directory = std::env::temp_dir().join(format!(
            "everything-next-trash-state-{}-{unique}",
            std::process::id()
        ));
        fs::create_dir_all(&directory).expect("create test directory");
        let missing = directory.join("missing.txt");
        let recreated = directory.join("recreated.txt");
        fs::write(&recreated, b"restored").expect("create restored file");
        let missing_path = missing.to_string_lossy().into_owned();
        let recreated_path = recreated.to_string_lossy().into_owned();
        let deleted_paths_file = directory.join(DELETED_PATHS_FILE);
        let state = TrashState {
            snapshots: Mutex::new(HashMap::new()),
            in_flight: Mutex::new(HashSet::new()),
            deleted_paths: Mutex::new(HashSet::from([
                normalized_path(&missing_path),
                normalized_path(&recreated_path),
            ])),
            deleted_paths_file: Some(deleted_paths_file.clone()),
            next_snapshot: AtomicU64::new(1),
        };
        let item = |path: String| SearchResult {
            name: Path::new(&path)
                .file_name()
                .expect("file name")
                .to_string_lossy()
                .into_owned(),
            parent_path: directory.to_string_lossy().into_owned(),
            full_path: path,
            size: Some(1),
            modified_unix: None,
            is_dir: false,
        };
        let mut page = SearchPage {
            request_id: 1,
            offset: 0,
            total: 2,
            items: vec![item(missing_path.clone()), item(recreated_path.clone())],
        };

        state.filter_deleted(&mut page);

        assert_eq!(page.total, 1);
        assert_eq!(page.items.len(), 1);
        assert_eq!(page.items[0].full_path, recreated_path);
        let persisted = fs::read_to_string(deleted_paths_file).expect("read deleted paths");
        assert!(persisted.contains(&normalized_path(&missing_path)));
        assert!(!persisted.contains(&normalized_path(&recreated_path)));

        fs::remove_dir_all(directory).expect("cleanup");
    }
}
