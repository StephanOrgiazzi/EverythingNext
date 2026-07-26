mod desktop;
mod search;
mod shell_commands;
mod trash;

use search::{begin_search_generation, engine_status, search_everything, SearchState};
use shell_commands::{
    copy_text, get_file_icon, get_file_visual, open_path, rename_path, reveal_path, ShellState,
};
use tauri::{Manager, WindowEvent};
use tauri_plugin_window_state::StateFlags;
use trash::{cancel_trash_snapshot, execute_trash_snapshot, prepare_trash_selection, TrashState};

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let mut builder = tauri::Builder::default();

    #[cfg(desktop)]
    {
        builder = builder.plugin(tauri_plugin_single_instance::init(|app, args, _cwd| {
            if !desktop::string_args_include_autostart(&args) {
                desktop::show_main_window(app);
            }
        }));
    }

    builder
        .plugin(tauri_plugin_dialog::init())
        .plugin(
            tauri_plugin_window_state::Builder::default()
                .with_state_flags(StateFlags::all().difference(StateFlags::VISIBLE))
                .build(),
        )
        .setup(|app| {
            let autostart_launch = desktop::is_autostart_launch();
            if !autostart_launch {
                desktop::ensure_autostart_registered();
            }

            app.manage(SearchState::initialize(app));
            app.manage(ShellState::new());
            app.manage(TrashState::new());
            desktop::install_tray(app)?;

            if !autostart_launch {
                desktop::show_main_window(app.handle());
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
            get_file_icon,
            get_file_visual,
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
