use everything_core::{EngineStatus, EverythingEngine, QueryRequest, SearchPage, SelectionRequest};
use std::collections::{HashMap, HashSet};
use std::ffi::OsStr;
#[cfg(all(windows, not(debug_assertions)))]
use std::os::windows::process::CommandExt;
#[cfg(all(windows, not(debug_assertions)))]
use std::process::Command;
use std::sync::{
    atomic::{AtomicU32, AtomicU64, Ordering},
    Arc, Mutex,
};
use tauri::{
    menu::{Menu, MenuItem},
    path::BaseDirectory,
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    AppHandle, Manager, Runtime, State, WindowEvent,
};
use tauri_plugin_window_state::StateFlags;
use windows_shell::IconCache;

const AUTOSTART_ARG: &str = "--autostart";
const AUTOSTART_VALUE_NAME: &str = "Everything Modern";
const TRAY_OPEN_ID: &str = "open";
const TRAY_QUIT_ID: &str = "quit";

#[derive(serde::Serialize)]
struct TrashOutcome {
    deleted: usize,
    failures: Vec<String>,
}

#[derive(serde::Serialize)]
struct TrashPreparation {
    snapshot_id: u64,
    count: usize,
}

struct AppState {
    engine: Arc<Mutex<Option<EverythingEngine>>>,
    engine_error: Option<String>,
    latest_generation: Arc<AtomicU32>,
    icons: Arc<IconCache>,
    icon_slots: Arc<tokio::sync::Semaphore>,
    trash_snapshots: Arc<Mutex<HashMap<u64, Vec<String>>>>,
    trash_in_flight: Arc<Mutex<HashSet<u64>>>,
    next_trash_snapshot: AtomicU64,
}

#[tauri::command]
async fn engine_status(state: State<'_, AppState>) -> Result<EngineStatus, String> {
    if let Some(error) = &state.engine_error {
        return Ok(EngineStatus {
            available: false,
            message: error.clone(),
            version: None,
        });
    }
    let engine = state.engine.clone();
    tauri::async_runtime::spawn_blocking(move || {
        engine
            .lock()
            .map_err(|_| "Verrou Everything empoisonné".to_string())?
            .as_ref()
            .map(|engine| engine.status())
            .ok_or_else(|| "Moteur Everything indisponible".to_string())
    })
    .await
    .map_err(|error| error.to_string())?
}

/// Invalide immédiatement les requêtes d'une génération précédente, avant même
/// que le debounce du frontend n'émette la prochaine recherche.
#[tauri::command]
fn begin_search_generation(state: State<'_, AppState>, request_id: u32) {
    state
        .latest_generation
        .fetch_max(request_id, Ordering::SeqCst);
}

#[tauri::command]
async fn search_everything(
    state: State<'_, AppState>,
    request: QueryRequest,
) -> Result<SearchPage, String> {
    state
        .latest_generation
        .fetch_max(request.request_id, Ordering::SeqCst);

    if request.request_id < state.latest_generation.load(Ordering::SeqCst) {
        return Err("Requête de recherche obsolète".into());
    }

    let engine = state.engine.clone();
    let latest_generation = state.latest_generation.clone();
    tauri::async_runtime::spawn_blocking(move || {
        if request.request_id < latest_generation.load(Ordering::SeqCst) {
            return Err("Requête de recherche obsolète".to_string());
        }

        let mut guard = engine
            .lock()
            .map_err(|_| "Verrou Everything empoisonné".to_string())?;

        // Une requête plus récente peut avoir été reçue pendant l'attente du verrou.
        if request.request_id < latest_generation.load(Ordering::SeqCst) {
            return Err("Requête de recherche obsolète".to_string());
        }

        let engine = guard.as_mut().ok_or_else(|| {
            "Everything SDK indisponible. Installez le SDK puis démarrez Everything.".to_string()
        })?;
        engine.query(request).map_err(|error| error.to_string())
    })
    .await
    .map_err(|error| error.to_string())?
}

#[tauri::command]
async fn get_file_icon(state: State<'_, AppState>, path: String) -> Result<Option<String>, String> {
    let permit = state
        .icon_slots
        .clone()
        .acquire_owned()
        .await
        .map_err(|_| "Service d’icônes arrêté".to_string())?;
    let icons = state.icons.clone();
    let result = tauri::async_runtime::spawn_blocking(move || {
        let _permit = permit;
        icons.get(&path).map_err(|error| error.to_string())
    })
    .await
    .map_err(|error| error.to_string())?;
    result
}

#[tauri::command]
async fn copy_text(text: String) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || {
        windows_shell::copy_text(&text).map_err(|error| error.to_string())
    })
    .await
    .map_err(|error| error.to_string())?
}

#[tauri::command]
async fn open_path(path: String) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || {
        windows_shell::open_path(&path).map_err(|error| error.to_string())
    })
    .await
    .map_err(|error| error.to_string())?
}

#[tauri::command]
async fn reveal_path(path: String) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || {
        windows_shell::reveal_path(&path).map_err(|error| error.to_string())
    })
    .await
    .map_err(|error| error.to_string())?
}

#[tauri::command]
async fn rename_path(path: String, new_name: String) -> Result<String, String> {
    tauri::async_runtime::spawn_blocking(move || {
        windows_shell::rename_path(&path, &new_name)
            .map(|path| path.to_string_lossy().into_owned())
            .map_err(|error| error.to_string())
    })
    .await
    .map_err(|error| error.to_string())?
}

#[tauri::command]
async fn prepare_trash_selection(
    state: State<'_, AppState>,
    request: SelectionRequest,
) -> Result<TrashPreparation, String> {
    const MAX_TRASH_ITEMS: usize = 10_000;
    if request.ranges.is_empty() {
        return Err("Aucun élément sélectionné".into());
    }
    if request.ranges.len() > 16_384 {
        return Err("Sélection invalide : trop de plages disjointes".into());
    }
    if request.request_id != state.latest_generation.load(Ordering::SeqCst) {
        return Err("La recherche a changé depuis la sélection. Recommencez l’opération.".into());
    }

    let engine = state.engine.clone();
    let latest_generation = state.latest_generation.clone();
    let request_id = request.request_id;
    let paths = tauri::async_runtime::spawn_blocking(move || {
        let mut guard = engine
            .lock()
            .map_err(|_| "Verrou Everything empoisonné".to_string())?;
        let engine = guard
            .as_mut()
            .ok_or_else(|| "Moteur Everything indisponible".to_string())?;
        engine
            .resolve_selection_cancellable(request, MAX_TRASH_ITEMS, || {
                request_id != latest_generation.load(Ordering::SeqCst)
            })
            .map_err(|error| error.to_string())
    })
    .await
    .map_err(|error| error.to_string())??;

    if request_id != state.latest_generation.load(Ordering::SeqCst) {
        return Err("La recherche a changé pendant la préparation de l’opération.".into());
    }

    let snapshot_id = state.next_trash_snapshot.fetch_add(1, Ordering::SeqCst);
    let count = paths.len();
    state
        .trash_snapshots
        .lock()
        .map_err(|_| "Stockage des suppressions indisponible".to_string())?
        .insert(snapshot_id, paths);

    Ok(TrashPreparation { snapshot_id, count })
}

#[tauri::command]
fn cancel_trash_snapshot(state: State<'_, AppState>, snapshot_id: u64) {
    if let Ok(mut snapshots) = state.trash_snapshots.lock() {
        snapshots.remove(&snapshot_id);
    }
}

#[tauri::command]
async fn execute_trash_snapshot(
    state: State<'_, AppState>,
    snapshot_id: u64,
) -> Result<TrashOutcome, String> {
    {
        let mut in_flight = state
            .trash_in_flight
            .lock()
            .map_err(|_| "État des suppressions indisponible".to_string())?;
        if !in_flight.insert(snapshot_id) {
            return Err("Cette suppression est déjà en cours".into());
        }
    }

    let paths = match state.trash_snapshots.lock() {
        Ok(mut snapshots) => match snapshots.remove(&snapshot_id) {
            Some(paths) => paths,
            None => {
                if let Ok(mut in_flight) = state.trash_in_flight.lock() {
                    in_flight.remove(&snapshot_id);
                }
                return Err("Cette confirmation a expiré ou a déjà été utilisée".into());
            }
        },
        Err(_) => {
            if let Ok(mut in_flight) = state.trash_in_flight.lock() {
                in_flight.remove(&snapshot_id);
            }
            return Err("Stockage des suppressions indisponible".into());
        }
    };

    let result = tauri::async_runtime::spawn_blocking(move || {
        let report = windows_shell::trash_paths(&paths);
        TrashOutcome {
            deleted: report.deleted,
            failures: report.failures,
        }
    })
    .await
    .map_err(|error| error.to_string());

    if let Ok(mut in_flight) = state.trash_in_flight.lock() {
        in_flight.remove(&snapshot_id);
    }
    result
}

fn initialize_engine<R: tauri::Runtime>(
    app: &tauri::App<R>,
) -> (Option<EverythingEngine>, Option<String>) {
    let bundled_dll = app
        .path()
        .resolve("Everything64.dll", BaseDirectory::Resource)
        .ok()
        .filter(|path| path.is_file());

    let result = bundled_dll
        .as_deref()
        .map(|path| EverythingEngine::from_dll_path(path))
        .unwrap_or_else(EverythingEngine::new);

    match result {
        Ok(engine) => (Some(engine), None),
        Err(error) => (None, Some(error.to_string())),
    }
}

fn is_autostart_arg(arg: &OsStr) -> bool {
    arg == OsStr::new(AUTOSTART_ARG)
}

fn is_autostart_launch() -> bool {
    std::env::args_os().any(|arg| is_autostart_arg(&arg))
}

fn string_args_include_autostart(args: &[String]) -> bool {
    args.iter().any(|arg| is_autostart_arg(OsStr::new(arg)))
}

#[cfg(all(windows, not(debug_assertions)))]
fn register_windows_autostart() -> Result<(), String> {
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    const RUN_KEY: &str = r"HKCU\Software\Microsoft\Windows\CurrentVersion\Run";

    let executable = std::env::current_exe()
        .map_err(|error| format!("Impossible de localiser l’exécutable : {error}"))?;
    let startup_command = format!("\"{}\" {AUTOSTART_ARG}", executable.display());
    let status = Command::new("reg.exe")
        .args([
            "add",
            RUN_KEY,
            "/v",
            AUTOSTART_VALUE_NAME,
            "/t",
            "REG_SZ",
            "/d",
            &startup_command,
            "/f",
        ])
        .creation_flags(CREATE_NO_WINDOW)
        .status()
        .map_err(|error| format!("Impossible d’enregistrer l’auto-démarrage : {error}"))?;

    if status.success() {
        Ok(())
    } else {
        Err(format!(
            "reg.exe a échoué avec le code {}",
            status
                .code()
                .map_or_else(|| "inconnu".to_string(), |code| code.to_string())
        ))
    }
}

#[cfg(any(not(windows), debug_assertions))]
fn register_windows_autostart() -> Result<(), String> {
    Ok(())
}

fn show_main_window<R: Runtime>(app: &AppHandle<R>) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.show();
        let _ = window.unminimize();
        let _ = window.set_focus();
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let mut builder = tauri::Builder::default();

    #[cfg(desktop)]
    {
        builder = builder.plugin(tauri_plugin_single_instance::init(|app, args, _cwd| {
            if !string_args_include_autostart(&args) {
                show_main_window(app);
            }
        }));
    }

    builder
        .plugin(
            tauri_plugin_window_state::Builder::default()
                .with_state_flags(StateFlags::all().difference(StateFlags::VISIBLE))
                .build(),
        )
        .setup(|app| {
            let autostart_launch = is_autostart_launch();

            if !autostart_launch {
                if let Err(error) = register_windows_autostart() {
                    eprintln!("Everything Modern autostart: {error}");
                }
            }

            let (engine, engine_error) = initialize_engine(app);
            app.manage(AppState {
                engine: Arc::new(Mutex::new(engine)),
                engine_error,
                latest_generation: Arc::new(AtomicU32::new(0)),
                icons: Arc::new(IconCache::new(512)),
                icon_slots: Arc::new(tokio::sync::Semaphore::new(4)),
                trash_snapshots: Arc::new(Mutex::new(HashMap::new())),
                trash_in_flight: Arc::new(Mutex::new(HashSet::new())),
                next_trash_snapshot: AtomicU64::new(1),
            });

            let open = MenuItem::with_id(app, TRAY_OPEN_ID, "Ouvrir", true, None::<&str>)?;
            let quit = MenuItem::with_id(app, TRAY_QUIT_ID, "Quitter", true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&open, &quit])?;
            let mut tray = TrayIconBuilder::new()
                .menu(&menu)
                .show_menu_on_left_click(false)
                .tooltip("Everything Modern")
                .on_menu_event(|app, event| match event.id().as_ref() {
                    TRAY_OPEN_ID => show_main_window(app),
                    TRAY_QUIT_ID => app.exit(0),
                    _ => {}
                })
                .on_tray_icon_event(|tray, event| {
                    if matches!(
                        event,
                        TrayIconEvent::Click {
                            button: MouseButton::Left,
                            button_state: MouseButtonState::Up,
                            ..
                        }
                    ) {
                        show_main_window(tray.app_handle());
                    }
                });
            if let Some(icon) = app.default_window_icon().cloned() {
                tray = tray.icon(icon);
            }
            tray.build(app)?;

            if !autostart_launch {
                show_main_window(app.handle());
            }

            Ok(())
        })
        .on_window_event(|window, event| {
            if let WindowEvent::CloseRequested { api, .. } = event {
                api.prevent_close();
                let _ = window.hide();
            }
        })
        .invoke_handler(tauri::generate_handler![
            engine_status,
            begin_search_generation,
            search_everything,
            get_file_icon,
            copy_text,
            open_path,
            reveal_path,
            rename_path,
            prepare_trash_selection,
            execute_trash_snapshot,
            cancel_trash_snapshot,
        ])
        .run(tauri::generate_context!())
        .expect("error while running Everything Modern");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_only_the_exact_autostart_argument() {
        assert!(is_autostart_arg(OsStr::new("--autostart")));
        assert!(!is_autostart_arg(OsStr::new("--autostart=true")));
        assert!(!is_autostart_arg(OsStr::new("autostart")));
    }
}
