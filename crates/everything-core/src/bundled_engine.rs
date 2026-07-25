use std::env;
use std::fs;
use std::os::windows::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Child, Command};
use std::thread;
use std::time::{Duration, Instant};

const CREATE_NO_WINDOW: u32 = 0x0800_0000;
const DEFAULT_INSTANCE_NAME: &str = "EverythingModern";

#[link(name = "kernel32")]
extern "system" {
    fn WaitNamedPipeW(name: *const u16, timeout: u32) -> i32;
}

pub(crate) struct ManagedEngine {
    executable: PathBuf,
    instance_name: String,
    child: Option<Child>,
}

impl ManagedEngine {
    pub(crate) fn start() -> Option<Self> {
        let executable = locate_engine()?;
        let instance_name = env::var("EVERYTHING_INSTANCE")
            .unwrap_or_else(|_| DEFAULT_INSTANCE_NAME.to_string());

        // SDK3 reads this variable when establishing its IPC3 connection.
        env::set_var("EVERYTHING_INSTANCE", &instance_name);

        let ipc_pipe = ipc_pipe_name(&instance_name);
        if named_pipe_available(&ipc_pipe) {
            return Some(Self::inactive(executable, instance_name));
        }

        let data_directory = engine_data_directory()?;
        if fs::create_dir_all(&data_directory).is_err() {
            return Some(Self::inactive(executable, instance_name));
        }

        let service_pipe = service_pipe_name(&instance_name);
        let config = data_directory.join("Everything.ini");
        let database = data_directory.join("Everything.db");
        if write_config(&config, &service_pipe).is_err() {
            return Some(Self::inactive(executable, instance_name));
        }

        let mut command = Command::new(&executable);
        if !instance_name.is_empty() {
            command.arg("-instance").arg(&instance_name);
        }
        command
            .arg("-first-instance")
            .arg("-startup")
            .arg("-config")
            .arg(&config)
            .arg("-db")
            .arg(&database)
            .arg("-service-pipe-name")
            .arg(&service_pipe)
            .creation_flags(CREATE_NO_WINDOW);

        let mut child = match command.spawn() {
            Ok(child) => child,
            Err(_) => return Some(Self::inactive(executable, instance_name)),
        };

        let deadline = Instant::now() + Duration::from_secs(8);
        while Instant::now() < deadline {
            match child.try_wait() {
                Ok(Some(_)) | Err(_) => {
                    return Some(Self::inactive(executable, instance_name));
                }
                Ok(None) => {}
            }
            if named_pipe_available(&ipc_pipe) {
                return Some(Self {
                    executable,
                    instance_name,
                    child: Some(child),
                });
            }
            thread::sleep(Duration::from_millis(50));
        }

        // Keep ownership when the process is still alive. SDK3 will retry the
        // connection later if the IPC pipe takes unusually long to appear.
        Some(Self {
            executable,
            instance_name,
            child: Some(child),
        })
    }

    pub(crate) fn stop(&mut self) {
        let Some(mut child) = self.child.take() else {
            return;
        };

        match child.try_wait() {
            Ok(Some(_)) | Err(_) => return,
            Ok(None) => {}
        }

        let mut command = Command::new(&self.executable);
        if !self.instance_name.is_empty() {
            command.arg("-instance").arg(&self.instance_name);
        }
        let _ = command
            .arg("-quit")
            .creation_flags(CREATE_NO_WINDOW)
            .status();

        for _ in 0..40 {
            match child.try_wait() {
                Ok(Some(_)) | Err(_) => return,
                Ok(None) => thread::sleep(Duration::from_millis(50)),
            }
        }

        let _ = child.kill();
        let _ = child.wait();
    }

    fn inactive(executable: PathBuf, instance_name: String) -> Self {
        Self {
            executable,
            instance_name,
            child: None,
        }
    }
}

fn write_config(path: &Path, service_pipe: &str) -> std::io::Result<()> {
    let config = format!(
        "[Everything]\n\
run_as_admin=0\n\
run_in_background=1\n\
show_in_taskbar=0\n\
show_tray_icon=0\n\
minimize_to_tray=0\n\
check_for_updates_on_startup=0\n\
beta_updates=0\n\
allow_multiple_instances=0\n\
ipc_enabled=1\n\
service_pipe_name={service_pipe}\n"
    );

    if fs::read_to_string(path).ok().as_deref() == Some(config.as_str()) {
        return Ok(());
    }
    fs::write(path, config)
}

fn engine_data_directory() -> Option<PathBuf> {
    env::var_os("LOCALAPPDATA")
        .map(PathBuf::from)
        .map(|path| path.join("EverythingModern").join("Engine"))
}

fn locate_engine() -> Option<PathBuf> {
    if let Some(explicit) = env::var_os("EVERYTHING_ENGINE_EXE") {
        let path = PathBuf::from(explicit);
        if path.is_file() {
            return Some(path);
        }
    }

    let mut candidates = Vec::new();
    if let Ok(current_executable) = env::current_exe() {
        if let Some(directory) = current_executable.parent() {
            candidates.push(directory.join("engine").join("Everything.exe"));
            candidates.push(
                directory
                    .join("resources")
                    .join("engine")
                    .join("Everything.exe"),
            );
        }
    }
    if let Ok(current_directory) = env::current_dir() {
        candidates.push(
            current_directory
                .join("src-tauri")
                .join("engine")
                .join("Everything.exe"),
        );
        candidates.push(current_directory.join("engine").join("Everything.exe"));
    }

    candidates.into_iter().find(|path| path.is_file())
}

fn ipc_pipe_name(instance_name: &str) -> String {
    if instance_name.is_empty() {
        r"\\.\PIPE\Everything IPC".to_string()
    } else {
        format!(r"\\.\PIPE\Everything IPC ({instance_name})")
    }
}

fn service_pipe_name(instance_name: &str) -> String {
    if instance_name.is_empty() {
        r"\\.\PIPE\Everything Service".to_string()
    } else {
        format!(r"\\.\PIPE\Everything Service ({instance_name})")
    }
}

fn named_pipe_available(name: &str) -> bool {
    let wide = name
        .encode_utf16()
        .chain(std::iter::once(0))
        .collect::<Vec<_>>();
    unsafe { WaitNamedPipeW(wide.as_ptr(), 0) != 0 }
}
