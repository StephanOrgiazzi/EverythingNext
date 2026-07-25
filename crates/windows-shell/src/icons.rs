use crate::ShellError;
use base64::Engine;
use image::{DynamicImage, ImageFormat, RgbaImage};
use parking_lot::Mutex;
use std::collections::{HashMap, VecDeque};
use std::io::Cursor;
use std::path::Path;

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
    use super::icon_cache_key;

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
