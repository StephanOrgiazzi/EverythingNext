use base64::Engine;
use image::{DynamicImage, ImageFormat, RgbaImage};
use parking_lot::Mutex;
use std::collections::{HashMap, VecDeque};
use std::io::Cursor;
use std::path::{Path, PathBuf};

#[derive(Debug, thiserror::Error)]
pub enum ShellError {
    #[error("Le fichier ou dossier n’existe plus : {0}")]
    MissingPath(String),
    #[error("Chemin invalide : {0}")]
    InvalidPath(String),
    #[error("Nom de fichier invalide sous Windows : {0}")]
    InvalidName(String),
    #[error("Un élément portant déjà ce nom existe : {0}")]
    AlreadyExists(String),
    #[error("Opération système impossible : {0}")]
    Io(#[from] std::io::Error),
    #[error("Impossible d’écrire dans le presse-papiers : {0}")]
    Clipboard(String),
    #[error("Impossible de produire l’icône : {0}")]
    Icon(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrashReport {
    pub deleted: usize,
    pub failures: Vec<String>,
}

pub struct IconCache {
    inner: Mutex<IconCacheInner>,
    capacity: usize,
}

struct IconCacheInner {
    entries: HashMap<String, Option<String>>,
    order: VecDeque<String>,
}

impl IconCache {
    pub fn new(capacity: usize) -> Self {
        Self {
            inner: Mutex::new(IconCacheInner {
                entries: HashMap::new(),
                order: VecDeque::new(),
            }),
            capacity: capacity.max(32),
        }
    }

    pub fn get(&self, path: &str) -> Result<Option<String>, ShellError> {
        let key = icon_cache_key(path);
        {
            let mut cache = self.inner.lock();
            if let Some(cached) = cache.entries.get(&key).cloned() {
                if let Some(position) = cache.order.iter().position(|candidate| candidate == &key) {
                    cache.order.remove(position);
                }
                cache.order.push_back(key.clone());
                return Ok(cached);
            }
        }

        let icon = extract_icon_data_uri(path)?;
        let mut cache = self.inner.lock();

        // Une requête concurrente peut avoir rempli la même clé pendant
        // l’extraction Shell ; on conserve alors le premier résultat.
        if let Some(cached) = cache.entries.get(&key).cloned() {
            return Ok(cached);
        }

        cache.entries.insert(key.clone(), icon.clone());
        cache.order.push_back(key);
        while cache.order.len() > self.capacity {
            if let Some(oldest) = cache.order.pop_front() {
                cache.entries.remove(&oldest);
            }
        }
        Ok(icon)
    }
}

#[cfg(windows)]
pub fn copy_text(text: &str) -> Result<(), ShellError> {
    clipboard_win::set_clipboard(clipboard_win::formats::Unicode, text)
        .map_err(|error| ShellError::Clipboard(error.to_string()))
}

#[cfg(not(windows))]
pub fn copy_text(_text: &str) -> Result<(), ShellError> {
    Err(ShellError::Clipboard(
        "le presse-papiers natif nécessite Windows".into(),
    ))
}

pub fn open_path(path: &str) -> Result<(), ShellError> {
    ensure_exists(path)?;
    open::that(path).map_err(ShellError::Io)
}

pub fn reveal_path(path: &str) -> Result<(), ShellError> {
    ensure_exists(path)?;
    #[cfg(windows)]
    {
        // L’argument complet est transmis séparément afin de préserver les
        // espaces, virgules et caractères non ASCII du chemin.
        std::process::Command::new("explorer.exe")
            .arg(format!("/select,{}", path))
            .spawn()?;
        return Ok(());
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
    validate_windows_name(new_name)?;

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

        // Windows ne garantit pas qu’un renommage ne changeant que la casse
        // fonctionne en une étape. Un nom temporaire dans le même dossier
        // rend l’opération fiable sans traverser de volume.
        let temporary = unique_temporary_sibling(source)?;
        std::fs::rename(source, &temporary)?;
        if let Err(error) = std::fs::rename(&temporary, &destination) {
            let _ = std::fs::rename(&temporary, source);
            return Err(ShellError::Io(error));
        }
        return Ok(destination);
    }

    std::fs::rename(source, &destination)?;
    Ok(destination)
}

pub fn trash_paths(paths: &[String]) -> TrashReport {
    let mut deleted = 0;
    let mut failures = Vec::new();
    let paths = normalized_operation_paths(paths);

    for path in &paths {
        if let Err(error) = ensure_exists(path) {
            failures.push(error.to_string());
            continue;
        }
        match trash::delete(path) {
            Ok(()) => deleted += 1,
            Err(error) => failures.push(format!("{} : {}", path, error)),
        }
    }

    TrashReport { deleted, failures }
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
        let candidate = parent.join(format!(".everything-modern-rename-{process}-{attempt}.tmp"));
        if !candidate.exists() {
            return Ok(candidate);
        }
    }
    Err(ShellError::AlreadyExists(
        "impossible de réserver un nom temporaire pour le renommage".into(),
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

fn validate_windows_name(name: &str) -> Result<(), ShellError> {
    let trimmed = name.trim();
    if trimmed.is_empty() || trimmed != name {
        return Err(ShellError::InvalidName(name.into()));
    }
    if name.ends_with('.') || name.ends_with(' ') {
        return Err(ShellError::InvalidName(name.into()));
    }
    if name.chars().any(|character| {
        character < '\u{20}'
            || matches!(
                character,
                '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*'
            )
    }) {
        return Err(ShellError::InvalidName(name.into()));
    }

    let stem = name
        .split('.')
        .next()
        .unwrap_or_default()
        .trim_end_matches(|character| character == '.' || character == ' ')
        .to_ascii_uppercase();
    let reserved = matches!(
        stem.as_str(),
        "CON" | "PRN" | "AUX" | "NUL" | "CONIN$" | "CONOUT$" | "CLOCK$"
    ) || stem.strip_prefix("COM").is_some_and(|suffix| {
        matches!(
            suffix,
            "1" | "2" | "3" | "4" | "5" | "6" | "7" | "8" | "9" | "¹" | "²" | "³"
        )
    }) || stem.strip_prefix("LPT").is_some_and(|suffix| {
        matches!(
            suffix,
            "1" | "2" | "3" | "4" | "5" | "6" | "7" | "8" | "9" | "¹" | "²" | "³"
        )
    });
    if reserved {
        return Err(ShellError::InvalidName(name.into()));
    }

    // Limite usuelle d’un composant NTFS, exprimée en unités UTF-16.
    if name.encode_utf16().count() > 255 {
        return Err(ShellError::InvalidName(name.into()));
    }

    Ok(())
}

fn icon_cache_key(path: &str) -> String {
    let source = Path::new(path);
    let normalized = path.replace('/', "\\");
    let normalized = if cfg!(windows) {
        normalized.to_lowercase()
    } else {
        normalized
    };

    let extension = source
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    let path_sensitive = source.is_dir()
        || matches!(
            extension.as_str(),
            "lnk" | "url" | "ico" | "exe" | "dll" | "cpl" | "scr" | "msc" | "appref-ms"
        );

    if path_sensitive {
        format!("path:{normalized}")
    } else {
        format!("extension:{extension}")
    }
}

#[cfg(windows)]
fn extract_icon_data_uri(path: &str) -> Result<Option<String>, ShellError> {
    let icon = file_icon_provider::get_file_icon(path, 20)
        .map_err(|error| ShellError::Icon(error.to_string()))?;
    let image = RgbaImage::from_raw(icon.width, icon.height, icon.pixels)
        .ok_or_else(|| ShellError::Icon("tampon RGBA invalide".into()))?;
    let mut png = Cursor::new(Vec::new());
    DynamicImage::ImageRgba8(image)
        .write_to(&mut png, ImageFormat::Png)
        .map_err(|error| ShellError::Icon(error.to_string()))?;
    let encoded = base64::engine::general_purpose::STANDARD.encode(png.into_inner());
    Ok(Some(format!("data:image/png;base64,{encoded}")))
}

#[cfg(not(windows))]
fn extract_icon_data_uri(_path: &str) -> Result<Option<String>, ShellError> {
    Ok(None)
}

#[cfg(test)]
mod tests {
    use super::{icon_cache_key, normalized_operation_paths, rename_path, validate_windows_name};
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

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
            "everything-modern-test-{}-{unique}",
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
            "everything-modern-case-test-{}-{unique}",
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

    #[test]
    fn icon_cache_keys_are_path_specific() {
        let first = icon_cache_key(r"C:\Apps\first.lnk");
        let second = icon_cache_key(r"C:\Apps\second.lnk");
        assert_ne!(first, second);
        assert_eq!(
            icon_cache_key("C:/Apps/tool.exe"),
            icon_cache_key(r"C:\Apps\tool.exe")
        );
        assert_eq!(
            icon_cache_key(r"C:\Documents\one.txt"),
            icon_cache_key(r"D:\Other\two.txt")
        );
    }
}
