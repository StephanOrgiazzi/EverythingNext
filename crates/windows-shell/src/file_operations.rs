use crate::ShellError;
use everything_core::validate_windows_name;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrashReport {
    pub deleted: usize,
    pub deleted_paths: Vec<String>,
    pub failures: Vec<String>,
}

pub fn open_path(path: &str) -> Result<(), ShellError> {
    ensure_exists(path)?;
    open::that(path).map_err(ShellError::Io)
}

fn explorer_legacy_select_raw_argument(path: &str) -> String {
    format!(r#"/select,"{path}""#)
}

pub fn reveal_path(path: &str) -> Result<(), ShellError> {
    ensure_exists(path)?;
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;

        std::process::Command::new("explorer.exe")
            .raw_arg(explorer_legacy_select_raw_argument(path))
            .spawn()?;
        Ok(())
    }
    #[cfg(not(windows))]
    {
        let parent = Path::new(path).parent().unwrap_or_else(|| Path::new(path));
        open::that(parent).map_err(ShellError::Io)
    }
}

pub fn rename_path(path: &str, new_name: &str) -> Result<PathBuf, ShellError> {
    let source = Path::new(path);
    ensure_exists(path)?;
    validate_windows_name(new_name).map_err(|_| ShellError::InvalidName(new_name.into()))?;

    let destination = source
        .parent()
        .ok_or_else(|| ShellError::InvalidPath(path.into()))?
        .join(new_name);

    if destination == source {
        return Ok(destination);
    }
    if destination.exists() {
        let same_entry = source
            .canonicalize()
            .ok()
            .zip(destination.canonicalize().ok())
            .is_some_and(|(source, destination)| source == destination);
        if !same_entry {
            return Err(ShellError::AlreadyExists(
                destination.to_string_lossy().into_owned(),
            ));
        }

        return rename_case_only_via_temporary_sibling(source, &destination);
    }

    std::fs::rename(source, &destination)?;
    Ok(destination)
}

fn rename_case_only_via_temporary_sibling(
    source: &Path,
    destination: &Path,
) -> Result<PathBuf, ShellError> {
    let temporary_sibling = unique_temporary_sibling(source)?;
    std::fs::rename(source, &temporary_sibling)?;
    if let Err(error) = std::fs::rename(&temporary_sibling, destination) {
        return match std::fs::rename(&temporary_sibling, source) {
            Ok(()) => Err(ShellError::Io(error)),
            Err(rollback_error) => Err(ShellError::Io(std::io::Error::new(
                error.kind(),
                format!(
                    "rename failed: {error}; rollback from '{}' to '{}' also failed: {rollback_error}",
                    temporary_sibling.display(),
                    source.display()
                ),
            ))),
        };
    }
    Ok(destination.to_path_buf())
}

pub fn trash_paths(paths: &[String]) -> TrashReport {
    let mut deleted_paths = Vec::new();
    let mut failures = Vec::new();
    let paths = normalized_operation_paths(paths);

    for path in &paths {
        if let Err(error) = ensure_exists(path) {
            failures.push(error.to_string());
            continue;
        }
        match trash::delete(path) {
            Ok(()) => deleted_paths.push(path.clone()),
            Err(error) => failures.push(format!("{path} : {error}")),
        }
    }

    TrashReport {
        deleted: deleted_paths.len(),
        deleted_paths,
        failures,
    }
}

fn normalized_operation_paths(paths: &[String]) -> Vec<String> {
    let mut paths = paths.to_vec();
    paths.sort_by(|left, right| {
        let left_depth = Path::new(left).components().count();
        let right_depth = Path::new(right).components().count();
        right_depth
            .cmp(&left_depth)
            .then_with(|| right.len().cmp(&left.len()))
            .then_with(|| left.cmp(right))
    });
    paths.dedup_by(|left, right| {
        if cfg!(windows) {
            left.eq_ignore_ascii_case(right)
        } else {
            left == right
        }
    });
    paths
}

fn unique_temporary_sibling(source: &Path) -> Result<PathBuf, ShellError> {
    let parent = source
        .parent()
        .ok_or_else(|| ShellError::InvalidPath(source.to_string_lossy().into_owned()))?;
    let process = std::process::id();
    for attempt in 0..1_024_u32 {
        let candidate = parent.join(format!(".everything-next-rename-{process}-{attempt}.tmp"));
        if !candidate.exists() {
            return Ok(candidate);
        }
    }
    Err(ShellError::AlreadyExists(
        "Unable to reserve a temporary name for renaming".into(),
    ))
}

fn ensure_exists(path: &str) -> Result<(), ShellError> {
    if path.trim().is_empty() {
        return Err(ShellError::InvalidPath(path.into()));
    }
    if Path::new(path).exists() {
        Ok(())
    } else {
        Err(ShellError::MissingPath(path.into()))
    }
}

#[cfg(test)]
mod tests {
    use super::{explorer_legacy_select_raw_argument, normalized_operation_paths, rename_path};
    use everything_core::validate_windows_name;
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn explorer_select_argument_quotes_only_the_path() {
        assert_eq!(
            explorer_legacy_select_raw_argument(r"C:\Users\Jean Dupont\rapport, été.pdf"),
            r#"/select,"C:\Users\Jean Dupont\rapport, été.pdf""#
        );
    }

    #[test]
    fn accepts_normal_windows_names() {
        for name in [
            "rapport final.pdf",
            "photo_été.png",
            "archive.tar.gz",
            "README",
        ] {
            assert!(validate_windows_name(name).is_ok(), "{name}");
        }
    }

    #[test]
    fn rejects_invalid_and_reserved_windows_names() {
        for name in [
            "",
            " fichier.txt",
            "fichier.txt ",
            "fichier.",
            "a/b",
            "a\\b",
            "a:b",
            "CON",
            "con.txt",
            "NUL.md",
            "COM1.log",
            "COM¹.txt",
            "LPT9",
            "LPT².log",
            "CONIN$",
            "CONOUT$.txt",
            "CLOCK$",
        ] {
            assert!(validate_windows_name(name).is_err(), "{name}");
        }
    }

    #[test]
    fn file_operations_are_deduplicated_and_deepest_first() {
        let paths = normalized_operation_paths(&[
            r"C:\Root".to_string(),
            r"C:\Root\Child\file.txt".to_string(),
            r"C:\Root\Child".to_string(),
            r"C:\Root\Child\file.txt".to_string(),
        ]);
        assert_eq!(paths.len(), 3);
        assert!(paths[0].ends_with("file.txt"));
        assert!(paths[2].ends_with("Root"));
    }

    #[test]
    fn renames_a_real_file_and_rejects_collisions() {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        let directory = std::env::temp_dir().join(format!(
            "everything-next-test-{}-{unique}",
            std::process::id()
        ));
        fs::create_dir_all(&directory).expect("create test directory");
        let source = directory.join("before.txt");
        let collision = directory.join("collision.txt");
        fs::write(&source, b"test").expect("create source");
        fs::write(&collision, b"existing").expect("create collision");

        let renamed = rename_path(&source.to_string_lossy(), "after.txt").expect("rename");
        assert!(renamed.exists());
        assert!(!source.exists());
        assert!(rename_path(&renamed.to_string_lossy(), "collision.txt").is_err());

        fs::remove_dir_all(&directory).expect("cleanup");
    }

    #[cfg(windows)]
    #[test]
    fn supports_case_only_renames() {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        let directory = std::env::temp_dir().join(format!(
            "everything-next-case-test-{}-{unique}",
            std::process::id()
        ));
        fs::create_dir_all(&directory).expect("create test directory");
        let source = directory.join("document.txt");
        fs::write(&source, b"test").expect("create source");

        let renamed =
            rename_path(&source.to_string_lossy(), "DOCUMENT.txt").expect("case-only rename");
        assert_eq!(
            renamed.file_name().and_then(|name| name.to_str()),
            Some("DOCUMENT.txt")
        );
        assert!(renamed.exists());

        fs::remove_dir_all(&directory).expect("cleanup");
    }
}
