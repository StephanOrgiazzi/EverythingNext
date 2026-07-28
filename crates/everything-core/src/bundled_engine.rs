use crate::EngineError;
use std::collections::HashSet;
use std::env;
use std::ffi::c_void;
use std::fs;
use std::mem::size_of;
use std::os::windows::io::{AsRawHandle, FromRawHandle, OwnedHandle};
use std::os::windows::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Child, Command};

const CREATE_NO_WINDOW: u32 = 0x0800_0000;
const JOB_OBJECT_EXTENDED_LIMIT_INFORMATION_CLASS: i32 = 9;
const JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE: u32 = 0x0000_2000;
const DEFAULT_SERVICE_INSTANCE_NAME: &str = "EverythingNext";
const DRIVE_FIXED: u32 = 3;

#[link(name = "kernel32")]
extern "system" {
    fn GetLogicalDrives() -> u32;
    fn GetDriveTypeW(root_path_name: *const u16) -> u32;
    fn GetVolumeInformationW(
        root_path_name: *const u16,
        volume_name_buffer: *mut u16,
        volume_name_size: u32,
        volume_serial_number: *mut u32,
        maximum_component_length: *mut u32,
        file_system_flags: *mut u32,
        file_system_name_buffer: *mut u16,
        file_system_name_size: u32,
    ) -> i32;
}

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

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct IndexedVolume {
    pub(crate) root: String,
    file_system: VolumeFileSystem,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum VolumeFileSystem {
    Ntfs,
    Refs,
    Fat,
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
                size_of::<JobObjectExtendedLimitInformation>() as u32,
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
    pub(crate) fn start(volume: &IndexedVolume) -> Result<Self, EngineError> {
        let service_instance = configured_service_instance();
        if !valid_instance_name(&service_instance) {
            return Err(EngineError::InvalidInstance(
                "use 1-64 ASCII letters, digits, dots, underscores, or hyphens".to_string(),
            ));
        }
        let instance_name = volume_instance_name(&service_instance, &volume.root);

        let executable = locate_engine().ok_or(EngineError::EngineNotFound)?;
        let ipc_pipe = ipc_pipe_name(&instance_name);
        if named_pipe_available(&ipc_pipe) {
            return Ok(Self::inactive(executable, instance_name));
        }

        let data_directory = engine_data_directory(&service_instance, &volume.root)
            .ok_or_else(|| EngineError::EngineSetup("LOCALAPPDATA is not available".to_string()))?;
        fs::create_dir_all(&data_directory).map_err(|error| {
            EngineError::EngineSetup(format!(
                "unable to create {}: {error}",
                data_directory.display()
            ))
        })?;

        let service_pipe = service_pipe_name(&service_instance);
        let config = data_directory.join("Everything.ini");
        let database = data_directory.join("Everything.db");
        write_config(&config, &service_pipe, volume).map_err(|error| {
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

    pub(crate) fn instance_name(&self) -> &str {
        &self.instance_name
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

fn write_config(path: &Path, service_pipe: &str, volume: &IndexedVolume) -> std::io::Result<()> {
    let existing = match fs::read_to_string(path) {
        Ok(existing) => existing,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => String::new(),
        Err(error) => return Err(error),
    };
    let config = merge_config(&existing, service_pipe, volume);

    if existing == config {
        return Ok(());
    }
    fs::write(path, config)
}

fn merge_config(existing: &str, service_pipe: &str, volume: &IndexedVolume) -> String {
    let (ntfs_paths, ntfs_includes) = volume_config(volume, VolumeFileSystem::Ntfs);
    let (refs_paths, refs_includes) = volume_config(volume, VolumeFileSystem::Refs);
    let (fat_paths, fat_includes) = volume_config(volume, VolumeFileSystem::Fat);
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
        ("auto_include_fixed_volumes", "0".to_string()),
        ("auto_include_fixed_refs_volumes", "0".to_string()),
        ("auto_include_fixed_fat_volumes", "0".to_string()),
        ("ntfs_volume_paths", ntfs_paths),
        ("ntfs_volume_includes", ntfs_includes),
        ("refs_volume_paths", refs_paths),
        ("refs_volume_includes", refs_includes),
        ("fat_volume_paths", fat_paths),
        ("fat_volume_includes", fat_includes),
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

fn volume_config(volume: &IndexedVolume, file_system: VolumeFileSystem) -> (String, String) {
    if volume.file_system == file_system {
        (volume.root.clone(), "1".to_string())
    } else {
        (String::new(), String::new())
    }
}

fn engine_data_directory(service_instance: &str, volume_root: &str) -> Option<PathBuf> {
    env::var_os("LOCALAPPDATA").map(PathBuf::from).map(|path| {
        path.join("EverythingNext")
            .join("Engine")
            .join("Volumes")
            .join(instance_storage_key(&format!(
                "{service_instance}/{volume_root}"
            )))
    })
}

fn instance_storage_key(instance_name: &str) -> String {
    let hash = stable_hash(instance_name);
    format!("instance-{hash:016x}")
}

fn valid_instance_name(instance_name: &str) -> bool {
    !instance_name.is_empty()
        && instance_name.len() <= 64
        && instance_name
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-'))
}

fn configured_service_instance() -> String {
    env::var("EVERYTHING_INSTANCE").unwrap_or_else(|_| DEFAULT_SERVICE_INSTANCE_NAME.to_string())
}

fn volume_instance_name(service_instance: &str, volume_root: &str) -> String {
    let volume_key = volume_root
        .trim_end_matches(['\\', ':'])
        .to_ascii_uppercase();
    let candidate = format!("{service_instance}-{volume_key}");
    if candidate.len() <= 64 {
        candidate
    } else {
        format!(
            "{}-{:016x}",
            &service_instance[..service_instance.len().min(47)],
            stable_hash(volume_root)
        )
    }
}

fn stable_hash(value: &str) -> u64 {
    value
        .as_bytes()
        .iter()
        .fold(0xcbf2_9ce4_8422_2325_u64, |hash, byte| {
            (hash ^ u64::from(*byte)).wrapping_mul(0x0000_0100_0000_01b3)
        })
}

pub(crate) fn fixed_volumes() -> Result<Vec<IndexedVolume>, EngineError> {
    let drive_mask = unsafe { GetLogicalDrives() };
    if drive_mask == 0 {
        return Err(EngineError::EngineSetup(
            "unable to enumerate fixed volumes".to_string(),
        ));
    }

    let mut volumes = Vec::new();
    for drive_index in 0..26_u32 {
        if drive_mask & (1 << drive_index) == 0 {
            continue;
        }
        let letter = char::from_u32(u32::from(b'A') + drive_index)
            .expect("drive indices always map to ASCII letters");
        let root = format!("{letter}:\\");
        let wide_root = root
            .encode_utf16()
            .chain(std::iter::once(0))
            .collect::<Vec<_>>();
        if unsafe { GetDriveTypeW(wide_root.as_ptr()) } != DRIVE_FIXED {
            continue;
        }

        let mut file_system = [0_u16; 32];
        let succeeded = unsafe {
            GetVolumeInformationW(
                wide_root.as_ptr(),
                std::ptr::null_mut(),
                0,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                file_system.as_mut_ptr(),
                file_system.len() as u32,
            )
        };
        if succeeded == 0 {
            continue;
        }
        let length = file_system
            .iter()
            .position(|value| *value == 0)
            .unwrap_or(file_system.len());
        let file_system = String::from_utf16_lossy(&file_system[..length]);
        let file_system = match file_system.to_ascii_uppercase().as_str() {
            "NTFS" => VolumeFileSystem::Ntfs,
            "REFS" => VolumeFileSystem::Refs,
            "FAT" | "FAT32" | "EXFAT" => VolumeFileSystem::Fat,
            _ => continue,
        };
        volumes.push(IndexedVolume {
            root: format!("{letter}:"),
            file_system,
        });
    }
    volumes.sort_by(|left, right| left.root.cmp(&right.root));
    if volumes.is_empty() {
        return Err(EngineError::EngineSetup(
            "no supported fixed volume was found".to_string(),
        ));
    }
    Ok(volumes)
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

    fn ntfs_volume() -> IndexedVolume {
        IndexedVolume {
            root: "C:".to_string(),
            file_system: VolumeFileSystem::Ntfs,
        }
    }

    #[test]
    fn merges_owned_settings_without_discarding_existing_configuration() {
        let existing = "[Everything]\r\ncustom_setting=keep\r\nshow_tray_icon=1\r\n\r\n[Other]\r\nvalue=keep\r\n";
        let merged = merge_config(
            existing,
            r"\\.\PIPE\Everything Service (Test)",
            &ntfs_volume(),
        );

        assert!(merged.contains("custom_setting=keep\r\n"));
        assert!(merged.contains("show_tray_icon=0\r\n"));
        assert!(merged.contains("[Other]\r\nvalue=keep\r\n"));
        assert_eq!(merged.matches("show_tray_icon=").count(), 1);
    }

    #[test]
    fn adds_the_everything_section_when_missing() {
        let merged = merge_config("[Other]\nvalue=keep\n", "test-pipe", &ntfs_volume());

        assert!(merged.contains("[Other]\nvalue=keep\n\n[Everything]\n"));
        assert!(merged.contains("service_pipe_name=test-pipe\n"));
    }

    #[test]
    fn recognizes_a_utf8_bom_without_duplicating_the_section() {
        let merged = merge_config(
            "\u{feff}[Everything]\nshow_tray_icon=1\n",
            "test-pipe",
            &ntfs_volume(),
        );

        assert_eq!(merged.matches("[Everything]").count(), 1);
        assert!(merged.contains("show_tray_icon=0\n"));
    }

    #[test]
    fn isolates_each_engine_to_its_selected_volume() {
        let existing = "\
[Everything]
alpha_instance=1
auto_include_fixed_volumes=0
auto_include_fixed_refs_volumes=0
auto_include_fixed_fat_volumes=0
";
        let merged = merge_config(existing, "test-pipe", &ntfs_volume());

        assert!(merged.contains("alpha_instance=0\n"));
        assert!(merged.contains("auto_include_fixed_volumes=0\n"));
        assert!(merged.contains("auto_include_fixed_refs_volumes=0\n"));
        assert!(merged.contains("auto_include_fixed_fat_volumes=0\n"));
        assert!(merged.contains("ntfs_volume_paths=C:\n"));
        assert!(merged.contains("ntfs_volume_includes=1\n"));
        assert!(merged.contains("refs_volume_paths=\n"));
        assert!(merged.contains("fat_volume_paths=\n"));
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
    fn volume_instances_share_the_service_but_have_distinct_ipc_names() {
        assert_eq!(
            volume_instance_name("EverythingNext", "C:"),
            "EverythingNext-C"
        );
        assert_eq!(
            volume_instance_name("EverythingNext", "D:"),
            "EverythingNext-D"
        );
        assert_ne!(
            ipc_pipe_name(&volume_instance_name("EverythingNext", "C:")),
            ipc_pipe_name(&volume_instance_name("EverythingNext", "D:"))
        );
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
