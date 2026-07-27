use std::ffi::OsStr;
#[cfg(all(windows, not(debug_assertions)))]
use std::os::windows::process::CommandExt;
#[cfg(all(windows, not(debug_assertions)))]
use std::process::Command;
use std::sync::Mutex;
use tauri::{
    menu::{Menu, MenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    AppHandle, Manager, Runtime, State,
};

const AUTOSTART_ARG: &str = "--autostart";
const SEARCH_ARG: &str = "-s";
#[cfg(all(windows, not(debug_assertions)))]
const AUTOSTART_VALUE_NAME: &str = "Everything Next";
#[cfg(all(windows, not(debug_assertions)))]
const LEGACY_AUTOSTART_VALUE_NAME: &str = "Everything Modern";
const TRAY_OPEN_ID: &str = "open";
const TRAY_QUIT_ID: &str = "quit";

pub(crate) struct LaunchState {
    pending_search_query: Mutex<Option<String>>,
}

impl LaunchState {
    pub(crate) fn new(pending_search_query: Option<String>) -> Self {
        Self {
            pending_search_query: Mutex::new(pending_search_query),
        }
    }

    pub(crate) fn set_search_query(&self, query: String) -> Result<(), String> {
        let mut pending = self
            .pending_search_query
            .lock()
            .map_err(|_| "Launch query lock was poisoned".to_string())?;
        *pending = Some(query);
        Ok(())
    }

    fn take_search_query(&self) -> Result<Option<String>, String> {
        self.pending_search_query
            .lock()
            .map_err(|_| "Launch query lock was poisoned".to_string())
            .map(|mut pending| pending.take())
    }
}

#[tauri::command]
pub(crate) fn take_pending_search_query(
    state: State<'_, LaunchState>,
) -> Result<Option<String>, String> {
    state.take_search_query()
}

pub(crate) fn is_autostart_launch() -> bool {
    std::env::args_os().any(|arg| is_autostart_arg(&arg))
}

pub(crate) fn string_args_include_autostart(args: &[String]) -> bool {
    args.iter().any(|arg| is_autostart_arg(OsStr::new(arg)))
}

pub(crate) fn search_query_from_args<I, S>(args: I) -> Option<String>
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    let mut arguments = args.into_iter();
    while let Some(argument) = arguments.next() {
        if argument
            .as_ref()
            .to_string_lossy()
            .eq_ignore_ascii_case(SEARCH_ARG)
        {
            return arguments
                .next()
                .map(|query| query.as_ref().to_string_lossy().into_owned());
        }
    }
    None
}

pub(crate) fn show_main_window<R: Runtime>(app: &AppHandle<R>) {
    if let Some(window) = app.get_webview_window("main") {
        if let Err(error) = window.show() {
            eprintln!("Unable to show the main window: {error}");
        }
        if let Err(error) = window.unminimize() {
            eprintln!("Unable to restore the main window: {error}");
        }
        if let Err(error) = window.set_focus() {
            eprintln!("Unable to focus the main window: {error}");
        }
    }
}

pub(crate) fn install_tray<R: Runtime>(app: &tauri::App<R>) -> tauri::Result<()> {
    let open = MenuItem::with_id(app, TRAY_OPEN_ID, "Open", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, TRAY_QUIT_ID, "Quitter", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&open, &quit])?;
    let mut tray = TrayIconBuilder::new()
        .menu(&menu)
        .show_menu_on_left_click(false)
        .tooltip("Everything Next")
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
        if let Some(window) = app.get_webview_window("main") {
            window.set_icon(icon.clone())?;
        }
        tray = tray.icon(icon);
    }
    tray.build(app)?;
    Ok(())
}

pub(crate) fn ensure_autostart_registered() {
    if let Err(error) = register_windows_autostart() {
        eprintln!("Everything Next autostart: {error}");
    }
}

fn is_autostart_arg(arg: &OsStr) -> bool {
    arg == OsStr::new(AUTOSTART_ARG)
}

#[cfg(all(windows, not(debug_assertions)))]
fn register_windows_autostart() -> Result<(), String> {
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    const RUN_KEY: &str = r"HKCU\Software\Microsoft\Windows\CurrentVersion\Run";

    let executable = std::env::current_exe()
        .map_err(|error| format!("Unable to locate the executable: {error}"))?;
    let startup_command = format!("\"{}\" {AUTOSTART_ARG}", executable.display());

    let _ = Command::new("reg.exe")
        .args([
            "delete",
            RUN_KEY,
            "/v",
            LEGACY_AUTOSTART_VALUE_NAME,
            "/f",
        ])
        .creation_flags(CREATE_NO_WINDOW)
        .status();

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
        .map_err(|error| format!("Unable to register auto-start: {error}"))?;

    if status.success() {
        Ok(())
    } else {
        Err(format!(
            "reg.exe failed with exit code {}",
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

#[cfg(test)]
mod tests {
    use super::{is_autostart_arg, search_query_from_args};
    use std::ffi::OsStr;

    #[test]
    fn detects_only_the_exact_autostart_argument() {
        assert!(is_autostart_arg(OsStr::new("--autostart")));
        assert!(!is_autostart_arg(OsStr::new("--autostart=true")));
        assert!(!is_autostart_arg(OsStr::new("autostart")));
    }

    #[test]
    fn parses_the_everything_search_argument() {
        let arguments = ["EverythingNext.exe", "-s", "ext:pdf annual report"];

        assert_eq!(
            search_query_from_args(arguments),
            Some("ext:pdf annual report".to_string())
        );
    }

    #[test]
    fn search_argument_matching_is_case_insensitive() {
        let arguments = ["EverythingNext.exe", "-S", "invoice"];

        assert_eq!(search_query_from_args(arguments), Some("invoice".to_string()));
    }

    #[test]
    fn ignores_a_search_argument_without_a_query() {
        let arguments = ["EverythingNext.exe", "-s"];

        assert_eq!(search_query_from_args(arguments), None);
    }
}
