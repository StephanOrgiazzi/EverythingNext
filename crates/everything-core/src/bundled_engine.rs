use crate::EngineError;
use std::collections::HashSet;
use std::env;
use std::ffi::c_void;
use std::fs;
use std::os::windows::io::{AsRawHandle, FromRawHandle, OwnedHandle};
use std::os::windows::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Child, Command};

const CREATE_NO_WINDOW: u32 = 0x0800_0000;
const JOB_OBJECT_EXTENDED_LIMIT_INFORMATION_CLASS: i32 = 9;
const JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE: u32 = 0x0000_2000;
const DEFAULT_INSTANCE_NAME: &str = "";
const DEFAULT_SERVICE_INSTANCE_NAME: &str = "EverythingNext";

#[link(name = "kernel32")]
extern "system" {
    fn WaitNamedPipeW(name: *const u16, timeout: u32) -> i32;
    fn CreateJobObjectW(attributes: *const c_void, name: *const u16) -> *mut c_void;
    fn SetInformationJobObject(
        job: *mut c_void,
        information_class: i32,
        information: *const c_void,
        information_length: u32,
    ) -> i32;
    fn AssignProcessToJobObject(job: *mut c_void, process: *mut c_void) -> i32;
}

#[repr(C)]
struct JobObjectBasicLimitInformation {
    per_process_user_time_limit: i64,
    per_job_user_time_limit: i64,
    limit_flags: u32,
    minimum_working_set_size: usize,
    maximum_working_set_size: usize,
    active_process_limit: u32,
    affinity: usize,
    priority_class: u32,
    scheduling_class: u32,
}

#[repr(C)]
struct IoCounters {
    read_operation_count: u64,
    write_operation_count: u64,
    other_operation_count: u64,
    read_transfer_count: u64,
    write_transfer_count: u64,
    other_transfer_count: u64,
}

#[repr(C)]
struct JobObjectExtendedLimitInformation {
    basic_limit_information: JobObjectBasicLimitInformation,
    io_info: IoCounters,
    process_memory_limit: usize,
    job_memory_limit: usize,
    peak_process_memory_used: usize,
    peak_job_memory_used: usize,
}

struct JobObject {
    handle: OwnedHandle,
}

impl JobObject {
    fn kill_on_close() -> Result<Self, EngineError> {
        let raw_handle = unsafe { CreateJobObjectW(std::ptr::null(), std::ptr::null()) };
        if raw_handle.is_null() {
            return Err(EngineError::EngineStart(format!(
                "unable to create the engine job object: {}",
                std::io::Error::last_os_error()
            )));
        }

        let job = Self {
            handle: unsafe { OwnedHandle::from_raw_handle(raw_handle) },
        };
        let mut limits: JobObjectExtendedLimitInformation = unsafe { std::mem::zeroed() };
        limits.basic_limit_information.limit_flags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
        let configured = unsafe {
            SetInformationJobObject(
                job.handle.as_raw_handle(),
                JOB_OBJECT_EXTENDED_LIMIT_INFORMATION_CLASS,
                std::ptr::from_ref(&limits).cast(),
                std::mem::size_of::<JobObjectExtendedLimitInformation>() as u32,
            )
        };
        if configured == 0 {
            return Err(EngineError::EngineStart(format!(
                "unable to configure the engine job object: {}",
                std::io::Error::last_os_error()
            )));
        }
        Ok(job)
    }

    fn assign(&self, child: &Child) -> Result<(), EngineError> {
        let assigned =
            unsafe { AssignProcessToJobObject(self.handle.as_raw_handle(), child.as_raw_handle()) };
        if assigned == 0 {
            return Err(EngineError::EngineStart(format!(
                "unable to attach the bundled engine to its job object: {}",
                std::io::Error::last_os_error()
            )));
        }
        Ok(())
    }
}

pub(crate) struct ManagedEngine {
    executable: PathBuf,
    instance_name: String,
    _job: Option<JobObject>,
    child: Option<Child>,
}

impl ManagedEngine {
    pub(crate) fn start() -> Result<Self, EngineError> {
        let instance_name =
            env::var("EVERYTHING_INSTANCE").unwrap_or_else(|_| DEFAULT_INSTANCE_NAME.to_string());
        if !instance_name.is_empty() && !valid_instance_name(&instance_name) {
            return Err(EngineError::InvalidInstance(
                "use 1-64 ASCII letters, digits, dots, underscores, or hyphens".to_string(),
            ));
        }

        configure_sdk_instance(&instance_name);

        let executable = locate_engine().ok_or(EngineError::EngineNotFound)?;
        let ipc_pipe = ipc_pipe_name(&instance_name);
        if named_pipe_available(&ipc_pipe) {
            if instance_name.is_empty() {
                return Err(EngineError::DefaultInstanceInUse);
            }
            return Ok(Self::inactive(executable, instance_name));
        }

        let data_directory = engine_data_directory(&instance_name)
            .ok_or_else(|| EngineError::EngineSetup("LOCALAPPDATA is not available".to_string()))?;
        fs::create_dir_all(&data_directory).map_err(|error| {
            EngineError::EngineSetup(format!(
                "unable to create {}: {error}",
                data_directory.display()
            ))
        })?;

        let service_pipe = service_pipe_name(&instance_name);
        let config = data_directory.join("Everything.ini");
        let database = data_directory.join("Everything.db");
        write_config(&config, &service_pipe).map_err(|error| {
            EngineError::EngineSetup(format!("unable to update {}: {error}", config.display()))
        })?;

        let job = JobObject::kill_on_close()?;
        let mut child = spawn_engine_without_waiting_for_index(
            &executable,
            &instance_name,
            &config,
            &database,
        )?;
        if let Err(error) = job.assign(&child) {
            let _ = child.kill();
            let _ = child.wait();
            return Err(error);
        }
        Ok(Self {
            executable,
            instance_name,
            _job: Some(job),
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
        match command
            .arg("-quit")
            .creation_flags(CREATE_NO_WINDOW)
            .status()
        {
            Ok(status) if !status.success() => {
                eprintln!("Everything shutdown command exited with status {status}");
            }
            Err(error) => eprintln!("Unable to send the Everything shutdown command: {error}"),
            Ok(_) => {}
        }

        for _ in 0..40 {
            match child.try_wait() {
                Ok(Some(_)) | Err(_) => return,
                Ok(None) => std::thread::sleep(std::time::Duration::from_millis(50)),
            }
        }

        if let Err(error) = child.kill() {
            eprintln!("Unable to force-stop the bundled Everything process: {error}");
        }
        if let Err(error) = child.wait() {
            eprintln!("Unable to reap the bundled Everything process: {error}");
        }
    }

    fn inactive(executable: PathBuf, instance_name: String) -> Self {
        Self {
            executable,
            instance_name,
            _job: None,
            child: None,
        }
    }
}

fn configure_sdk_instance(instance_name: &str) {
    env::set_var("EVERYTHING_INSTANCE", instance_name);
}

fn spawn_engine_without_waiting_for_index(
    executable: &Path,
    instance_name: &str,
    config: &Path,
    database: &Path,
) -> Result<Child, EngineError> {
    engine_command(executable, instance_name, config, database)
        .spawn()
        .map_err(|error| EngineError::EngineStart(error.to_string()))
}

fn engine_command(
    executable: &Path,
    instance_name: &str,
    config: &Path,
    database: &Path,
) -> Command {
    let mut command = Command::new(executable);
    if !instance_name.is_empty() {
        command.arg("-instance").arg(instance_name);
    }
    command
        .arg("-first-instance")
        .arg("-startup")
        .arg("-config")
        .arg(config)
        .arg("-db")
        .arg(database)
        .creation_flags(CREATE_NO_WINDOW);
    command
}

fn write_config(path: &Path, service_pipe: &str) -> std::io::Result<()> {
    let existing = match fs::read_to_string(path) {
        Ok(existing) => existing,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => String::new(),
        Err(error) => return Err(error),
    };
    let config = merge_config(&existing, service_pipe);

    if existing == config {
        return Ok(());
    }
    fs::write(path, config)
}

fn merge_config(existing: &str, service_pipe: &str) -> String {
    let desired = [
        ("run_as_admin", "0".to_string()),
        ("run_in_background", "1".to_string()),
        ("show_in_taskbar", "0".to_string()),
        ("show_tray_icon", "0".to_string()),
        ("minimize_to_tray", "0".to_string()),
        ("check_for_updates_on_startup", "0".to_string()),
        ("beta_updates", "0".to_string()),
        ("alpha_instance", "0".to_string()),
        ("allow_multiple_instances", "0".to_string()),
        ("ipc_enabled", "1".to_string()),
        ("service_pipe_name", service_pipe.to_string()),
        ("auto_include_fixed_volumes", "1".to_string()),
        ("auto_include_fixed_refs_volumes", "1".to_string()),
        ("auto_include_fixed_fat_volumes", "1".to_string()),
    ];
    let newline = if existing.contains("\r\n") {
        "\r\n"
    } else {
        "\n"
    };
    let mut output = Vec::new();
    let mut seen = HashSet::new();
    let mut in_everything = false;
    let mut found_everything = false;

    let append_missing = |output: &mut Vec<String>, seen: &mut HashSet<&str>| {
        for (key, value) in &desired {
            if seen.insert(*key) {
                output.push(format!("{key}={value}"));
            }
        }
    };

    for line in existing.lines() {
        let trimmed = line.trim();
        let section = trimmed.strip_prefix('\u{feff}').unwrap_or(trimmed);
        if section.starts_with('[') && section.ends_with(']') {
            if in_everything {
                append_missing(&mut output, &mut seen);
            }
            in_everything = section[1..section.len() - 1].eq_ignore_ascii_case("Everything");
            found_everything |= in_everything;
            output.push(line.to_string());
            continue;
        }

        if in_everything {
            if let Some((raw_key, _)) = line.split_once('=') {
                if let Some((key, value)) = desired
                    .iter()
                    .find(|(key, _)| raw_key.trim().eq_ignore_ascii_case(key))
                {
                    if seen.insert(*key) {
                        output.push(format!("{key}={value}"));
                    }
                    continue;
                }
            }
        }
        output.push(line.to_string());
    }

    if in_everything {
        append_missing(&mut output, &mut seen);
    } else if !found_everything {
        if !output.is_empty() && !output.last().is_some_and(String::is_empty) {
            output.push(String::new());
        }
        output.push("[Everything]".to_string());
        append_missing(&mut output, &mut seen);
    }

    format!("{}{newline}", output.join(newline))
}

fn engine_data_directory(instance_name: &str) -> Option<PathBuf> {
    env::var_os("LOCALAPPDATA").map(PathBuf::from).map(|path| {
        let base = path.join("EverythingNext").join("Engine");
        if instance_name == DEFAULT_INSTANCE_NAME {
            base
        } else {
            base.join(instance_storage_key(instance_name))
        }
    })
}

fn instance_storage_key(instance_name: &str) -> String {
    let hash = instance_name
        .as_bytes()
        .iter()
        .fold(0xcbf2_9ce4_8422_2325_u64, |hash, byte| {
            (hash ^ u64::from(*byte)).wrapping_mul(0x0000_0100_0000_01b3)
        });
    format!("instance-{hash:016x}")
}

fn valid_instance_name(instance_name: &str) -> bool {
    !instance_name.is_empty()
        && instance_name.len() <= 64
        && instance_name
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-'))
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
    let service_instance = if instance_name.is_empty() {
        DEFAULT_SERVICE_INSTANCE_NAME
    } else {
        instance_name
    };
    format!(r"\\.\PIPE\Everything Service ({service_instance})")
}

fn named_pipe_available(name: &str) -> bool {
    let wide = name
        .encode_utf16()
        .chain(std::iter::once(0))
        .collect::<Vec<_>>();
    unsafe { WaitNamedPipeW(wide.as_ptr(), 0) != 0 }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn merges_owned_settings_without_discarding_existing_configuration() {
        let existing = "[Everything]\r\ncustom_setting=keep\r\nshow_tray_icon=1\r\n\r\n[Other]\r\nvalue=keep\r\n";
        let merged = merge_config(existing, r"\\.\PIPE\Everything Service (Test)");

        assert!(merged.contains("custom_setting=keep\r\n"));
        assert!(merged.contains("show_tray_icon=0\r\n"));
        assert!(merged.contains("[Other]\r\nvalue=keep\r\n"));
        assert_eq!(merged.matches("show_tray_icon=").count(), 1);
    }

    #[test]
    fn adds_the_everything_section_when_missing() {
        let merged = merge_config("[Other]\nvalue=keep\n", "test-pipe");

        assert!(merged.contains("[Other]\nvalue=keep\n\n[Everything]\n"));
        assert!(merged.contains("service_pipe_name=test-pipe\n"));
    }

    #[test]
    fn recognizes_a_utf8_bom_without_duplicating_the_section() {
        let merged = merge_config("\u{feff}[Everything]\nshow_tray_icon=1\n", "test-pipe");

        assert_eq!(merged.matches("[Everything]").count(), 1);
        assert!(merged.contains("show_tray_icon=0\n"));
    }

    #[test]
    fn enables_indexing_for_all_fixed_volume_types() {
        let existing = "\
[Everything]
alpha_instance=1
auto_include_fixed_volumes=0
auto_include_fixed_refs_volumes=0
auto_include_fixed_fat_volumes=0
";
        let merged = merge_config(existing, "test-pipe");

        assert!(merged.contains("alpha_instance=0\n"));
        assert!(merged.contains("auto_include_fixed_volumes=1\n"));
        assert!(merged.contains("auto_include_fixed_refs_volumes=1\n"));
        assert!(merged.contains("auto_include_fixed_fat_volumes=1\n"));
        assert_eq!(merged.matches("alpha_instance=").count(), 1);
        assert_eq!(merged.matches("auto_include_fixed_volumes=").count(), 1);
        assert_eq!(
            merged.matches("auto_include_fixed_refs_volumes=").count(),
            1
        );
        assert_eq!(merged.matches("auto_include_fixed_fat_volumes=").count(), 1);
    }

    #[test]
    fn overridden_instances_have_distinct_storage_keys() {
        assert_ne!(
            instance_storage_key("EverythingNextDev"),
            instance_storage_key("EverythingNextTest")
        );
    }

    #[test]
    fn instance_names_are_safe_for_process_arguments_and_storage() {
        assert!(valid_instance_name("EverythingNext.Dev-1"));
        assert!(!valid_instance_name(""));
        assert!(!valid_instance_name("bad instance"));
        assert!(!valid_instance_name("bad\" -quit"));
    }

    #[test]
    fn default_instance_uses_the_standard_ipc_pipe_and_private_service_pipe() {
        assert_eq!(ipc_pipe_name(""), r"\\.\PIPE\Everything IPC");
        assert_eq!(
            service_pipe_name(""),
            r"\\.\PIPE\Everything Service (EverythingNext)"
        );
    }

    #[test]
    fn default_runtime_command_omits_the_instance_argument() {
        let command = engine_command(
            Path::new("Everything.exe"),
            "",
            Path::new("Everything.ini"),
            Path::new("Everything.db"),
        );
        let arguments = command
            .get_args()
            .map(|argument| argument.to_string_lossy().into_owned())
            .collect::<Vec<_>>();

        assert!(!arguments.iter().any(|argument| argument == "-instance"));
        assert!(arguments
            .iter()
            .any(|argument| argument == "-first-instance"));
    }

    #[test]
    fn runtime_command_does_not_reapply_the_service_pipe_setting() {
        let command = engine_command(
            Path::new("Everything.exe"),
            "EverythingNext",
            Path::new("Everything.ini"),
            Path::new("Everything.db"),
        );
        let arguments = command
            .get_args()
            .map(|argument| argument.to_string_lossy().into_owned())
            .collect::<Vec<_>>();

        assert!(!arguments
            .iter()
            .any(|argument| argument == "-service-pipe-name"));
        assert!(arguments
            .windows(2)
            .any(|pair| pair == ["-instance", "EverythingNext"]));
    }
}
