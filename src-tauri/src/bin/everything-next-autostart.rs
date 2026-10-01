#![cfg(windows)]
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::os::windows::process::CommandExt;
use std::process::Command;

const CREATE_NO_WINDOW: u32 = 0x0800_0000;

fn main() {
    let Ok(launcher) = std::env::current_exe() else {
        return;
    };
    let Some(package_root) = launcher.parent() else {
        return;
    };

    let app = package_root.join("EverythingNext.exe");
    let _ = Command::new(app)
        .arg("--autostart")
        .creation_flags(CREATE_NO_WINDOW)
        .spawn();
}
