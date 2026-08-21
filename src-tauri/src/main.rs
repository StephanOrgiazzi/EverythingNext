#![cfg(windows)]
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod desktop;
mod search;
mod shell_commands;
mod trash;
mod updates;

use desktop::{take_pending_search_query, LaunchState};
use search::{begin_search_generation, engine_status, search_everything, SearchState};
use shell_commands::{
    copy_files, copy_text, get_file_visual, open_path, rename_path, reveal_path, ShellState,
};
use tauri::{Emitter, Manager, WindowEvent};
use tauri_plugin_window_state::StateFlags;
use trash::{cancel_trash_snapshot, execute_trash_snapshot, prepare_trash_selection, TrashState};

const OPEN_SEARCH_QUERY_EVENT: &str = "open-search-query";

fn main() -> tauri::Result<()> {
    let initial_search_query = desktop::search_query_from_args(std::env::args_os());
    let builder =
        tauri::Builder::default().plugin(tauri_plugin_single_instance::init(|app, args, _cwd| {
            if let Some(query) = desktop::search_query_from_args(&args) {
                match app.state::<LaunchState>().set_search_query(query) {
                    Ok(()) => {
                        if let Err(error) = app.emit(OPEN_SEARCH_QUERY_EVENT, ()) {
                            eprintln!("Unable to notify the UI about the launch query: {error}");
                        }
                    }
                    Err(error) => eprintln!("Unable to forward the launch search query: {error}"),
                }
            }
            if !desktop::string_args_include_autostart(&args) {
                desktop::show_main_window(app);
                updates::check_on_user_launch(app.clone());
            }
        }));

    builder
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(
            tauri_plugin_window_state::Builder::default()
                .with_state_flags(StateFlags::all().difference(StateFlags::VISIBLE))
                .build(),
        )
        .setup(move |app| {
            let autostart_launch = desktop::is_autostart_launch();
            if !autostart_launch {
                desktop::ensure_autostart_registered();
            }

            app.manage(LaunchState::new(initial_search_query));
            app.manage(SearchState::initialize(app));
            app.manage(ShellState::new());
            app.manage(TrashState::initialize(app));
            desktop::install_tray(app)?;

            if !autostart_launch {
                desktop::show_main_window(app.handle());
                updates::check_on_user_launch(app.handle().clone());
            }
            Ok(())
        })
        .on_window_event(|window, event| {
            if let WindowEvent::CloseRequested { api, .. } = event {
                api.prevent_close();
                if let Err(error) = window.hide() {
                    eprintln!("Unable to hide the main window: {error}");
                }
            }
        })
        .invoke_handler(tauri::generate_handler![
            engine_status,
            begin_search_generation,
            search_everything,
            take_pending_search_query,
            get_file_visual,
            copy_files,
            copy_text,
            open_path,
            reveal_path,
            rename_path,
            prepare_trash_selection,
            execute_trash_snapshot,
            cancel_trash_snapshot,
        ])
        .run(tauri::generate_context!())
}
