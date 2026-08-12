use crate::ShellError;
use parking_lot::Mutex;
use std::collections::{HashMap, VecDeque};
use std::path::Path;
use std::time::UNIX_EPOCH;

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum VisualKind {
    Icon,
    Thumbnail(u32),
}

pub struct VisualCache {
    inner: Mutex<VisualCacheInner>,
    max_bytes: usize,
    max_entries: usize,
}

struct VisualCacheInner {
    entries: HashMap<String, CachedVisual>,
    order: VecDeque<String>,
    bytes: usize,
}

#[derive(Clone)]
struct CachedVisual {
    source: String,
    bytes: usize,
}

impl VisualCache {
    #[must_use]
    pub fn new(max_bytes: usize) -> Self {
        Self {
            inner: Mutex::new(VisualCacheInner {
                entries: HashMap::new(),
                order: VecDeque::new(),
                bytes: 0,
            }),
            max_bytes: max_bytes.max(1024 * 1024),
            max_entries: 256,
        }
    }

    pub fn get(&self, path: &str, kind: VisualKind) -> Result<Option<String>, ShellError> {
        let key = visual_cache_key(path, kind);
        {
            let mut cache = self.inner.lock();
            if let Some(cached) = cache.entries.get(&key).cloned() {
                touch(&mut cache.order, &key);
                return Ok(Some(cached.source));
            }
        }

        let Some(source) = extract_visual_data_uri(path, kind)? else {
            return Ok(None);
        };
        let result = source.clone();
        let bytes = source.len();
        let mut cache = self.inner.lock();
        if let Some(cached) = cache.entries.get(&key).cloned() {
            return Ok(Some(cached.source));
        }

        cache
            .entries
            .insert(key.clone(), CachedVisual { source, bytes });
        cache.order.push_back(key);
        cache.bytes = cache.bytes.saturating_add(bytes);

        while cache.bytes > self.max_bytes || cache.order.len() > self.max_entries {
            let Some(oldest) = cache.order.pop_front() else {
                break;
            };
            if let Some(removed) = cache.entries.remove(&oldest) {
                cache.bytes = cache.bytes.saturating_sub(removed.bytes);
            }
        }

        Ok(Some(result))
    }
}

fn touch(order: &mut VecDeque<String>, key: &str) {
    if let Some(position) = order.iter().position(|candidate| candidate == key) {
        order.remove(position);
    }
    order.push_back(key.to_string());
}

fn visual_cache_key(path: &str, kind: VisualKind) -> String {
    let normalized = path.replace('/', "\\");
    let normalized = normalized.to_lowercase();
    let metadata = std::fs::metadata(Path::new(path)).ok();
    let length = metadata.as_ref().map_or(0, std::fs::Metadata::len);
    let modified = metadata
        .and_then(|metadata| metadata.modified().ok())
        .and_then(|modified| modified.duration_since(UNIX_EPOCH).ok())
        .map_or(0, |duration| duration.as_nanos());

    format!("{normalized}\u{1f}{kind:?}\u{1f}{length}\u{1f}{modified}")
}

fn shell_compatible_path(path: &str) -> String {
    let normalized = path.replace('/', "\\");
    if let Some(unc_path) = normalized.strip_prefix(r"\\?\UNC\") {
        format!(r"\\{unc_path}")
    } else {
        normalized
            .strip_prefix(r"\\?\")
            .unwrap_or(&normalized)
            .to_string()
    }
}

struct BitmapGuard(windows::Win32::Graphics::Gdi::HBITMAP);

impl Drop for BitmapGuard {
    fn drop(&mut self) {
        use windows::Win32::Graphics::Gdi::{DeleteObject, HGDIOBJ};

        if self.0 .0.is_null() {
            return;
        }
        unsafe {
            if !DeleteObject(HGDIOBJ(self.0 .0)).as_bool() {
                let error = windows::core::Error::from_thread();
                eprintln!("Unable to release Shell bitmap: {error}");
            }
        }
    }
}

struct DcGuard(windows::Win32::Graphics::Gdi::HDC);

impl Drop for DcGuard {
    fn drop(&mut self) {
        use windows::Win32::Graphics::Gdi::DeleteDC;

        unsafe {
            if !DeleteDC(self.0).as_bool() {
                let error = windows::core::Error::from_thread();
                eprintln!("Unable to release bitmap device context: {error}");
            }
        }
    }
}

fn extract_visual_data_uri(path: &str, kind: VisualKind) -> Result<Option<String>, ShellError> {
    use windows::core::HSTRING;
    use windows::Win32::Foundation::{RPC_E_CHANGED_MODE, SIZE};
    use windows::Win32::System::Com::{CoInitialize, CoUninitialize};
    use windows::Win32::UI::Shell::{
        IShellItemImageFactory, SHCreateItemFromParsingName, SIIGBF_RESIZETOFIT,
    };

    struct ComGuard(bool);

    impl Drop for ComGuard {
        fn drop(&mut self) {
            if self.0 {
                unsafe { CoUninitialize() };
            }
        }
    }

    let initialized = unsafe { CoInitialize(None) };
    let _com = if initialized.is_ok() {
        ComGuard(true)
    } else if initialized == RPC_E_CHANGED_MODE {
        ComGuard(false)
    } else {
        return Err(ShellError::Visual(format!(
            "COM initialization failed: {initialized:?}"
        )));
    };

    let path = shell_compatible_path(path);
    let VisualKind::Thumbnail(size) = kind else {
        return extract_jumbo_icon_data_uri(&path);
    };

    let requested_size = i32::try_from(size)
        .map_err(|_| ShellError::Visual("The requested preview size is too large".into()))?;
    let shell_path = HSTRING::from(&path);
    let factory: IShellItemImageFactory = unsafe { SHCreateItemFromParsingName(&shell_path, None) }
        .map_err(|error| {
            ShellError::Visual(format!("Unable to create the Windows Shell item: {error}"))
        })?;
    let bitmap = match unsafe {
        factory.GetImage(
            SIZE {
                cx: requested_size,
                cy: requested_size,
            },
            SIIGBF_RESIZETOFIT,
        )
    } {
        Ok(bitmap) => BitmapGuard(bitmap),
        Err(_) => return Ok(None),
    };

    bitmap_to_data_uri(bitmap.0).map(Some)
}

fn extract_jumbo_icon_data_uri(path: &str) -> Result<Option<String>, ShellError> {
    use std::mem::size_of;
    use windows::core::HSTRING;
    use windows::Win32::Storage::FileSystem::FILE_FLAGS_AND_ATTRIBUTES;
    use windows::Win32::UI::Controls::{IImageList, ILD_TRANSPARENT};
    use windows::Win32::UI::Shell::{
        SHGetFileInfoW, SHGetImageList, SHFILEINFOW, SHGFI_SYSICONINDEX, SHIL_JUMBO,
    };
    use windows::Win32::UI::WindowsAndMessaging::{DestroyIcon, GetIconInfo, ICONINFO};

    struct IconGuard(windows::Win32::UI::WindowsAndMessaging::HICON);

    impl Drop for IconGuard {
        fn drop(&mut self) {
            if let Err(error) = unsafe { DestroyIcon(self.0) } {
                eprintln!("Unable to release Shell icon: {error}");
            }
        }
    }

    let mut file_info = SHFILEINFOW::default();
    let found = unsafe {
        SHGetFileInfoW(
            &HSTRING::from(path),
            FILE_FLAGS_AND_ATTRIBUTES(0),
            Some(&raw mut file_info),
            u32::try_from(size_of::<SHFILEINFOW>()).expect("SHFILEINFOW size fits in u32"),
            SHGFI_SYSICONINDEX,
        )
    };
    if found == 0 {
        return Ok(None);
    }

    let image_list: IImageList =
        unsafe { SHGetImageList(i32::try_from(SHIL_JUMBO).expect("SHIL_JUMBO fits in i32")) }
            .map_err(|error| {
                ShellError::Visual(format!(
                    "Unable to access the jumbo Shell image list: {error}"
                ))
            })?;
    let icon = IconGuard(
        unsafe { image_list.GetIcon(file_info.iIcon, ILD_TRANSPARENT.0) }.map_err(|error| {
            ShellError::Visual(format!("Unable to read the jumbo Shell icon: {error}"))
        })?,
    );
    let mut icon_info = ICONINFO::default();
    unsafe { GetIconInfo(icon.0, &raw mut icon_info) }.map_err(|error| {
        ShellError::Visual(format!("Unable to inspect the jumbo Shell icon: {error}"))
    })?;
    let color_bitmap = BitmapGuard(icon_info.hbmColor);
    let _mask_bitmap = BitmapGuard(icon_info.hbmMask);
    if color_bitmap.0 .0.is_null() {
        return Ok(None);
    }

    bitmap_to_data_uri(color_bitmap.0).map(Some)
}

fn bitmap_to_data_uri(
    bitmap: windows::Win32::Graphics::Gdi::HBITMAP,
) -> Result<String, ShellError> {
    use base64::Engine;
    use image::{DynamicImage, ImageFormat, RgbaImage};
    use std::io::Cursor;
    use std::mem::size_of;
    use windows::Win32::Graphics::Gdi::{
        CreateCompatibleDC, GetDIBits, GetObjectW, BITMAP, BITMAPINFO, BITMAPINFOHEADER, BI_RGB,
        DIB_RGB_COLORS, HGDIOBJ,
    };

    let mut description = BITMAP::default();
    let described = unsafe {
        GetObjectW(
            HGDIOBJ(bitmap.0),
            i32::try_from(size_of::<BITMAP>()).expect("BITMAP size fits in i32"),
            Some((&raw mut description).cast()),
        )
    };
    if described == 0 || description.bmWidth <= 0 || description.bmHeight == 0 {
        return Err(ShellError::Visual(
            "Windows returned an invalid bitmap".into(),
        ));
    }

    let width = u32::try_from(description.bmWidth)
        .map_err(|_| ShellError::Visual("Windows returned an invalid bitmap width".into()))?;
    let height = description.bmHeight.unsigned_abs();
    let height_i32 = i32::try_from(height)
        .map_err(|_| ShellError::Visual("Windows returned an invalid bitmap height".into()))?;
    let byte_count = width
        .checked_mul(height)
        .and_then(|pixels| pixels.checked_mul(4))
        .ok_or_else(|| ShellError::Visual("The preview bitmap is too large".into()))?;
    let buffer_len = usize::try_from(byte_count)
        .map_err(|_| ShellError::Visual("The preview bitmap is too large".into()))?;
    let mut bgra = vec![0_u8; buffer_len];
    let mut info = BITMAPINFO {
        bmiHeader: BITMAPINFOHEADER {
            biSize: u32::try_from(size_of::<BITMAPINFOHEADER>())
                .expect("BITMAPINFOHEADER size fits in u32"),
            biWidth: description.bmWidth,
            biHeight: -height_i32,
            biPlanes: 1,
            biBitCount: 32,
            biCompression: BI_RGB.0,
            biSizeImage: byte_count,
            ..Default::default()
        },
        ..Default::default()
    };
    let dc = DcGuard(unsafe { CreateCompatibleDC(None) });
    if dc.0.is_invalid() {
        return Err(ShellError::Visual(
            "Unable to create a bitmap device context".into(),
        ));
    }
    let copied = unsafe {
        GetDIBits(
            dc.0,
            bitmap,
            0,
            height,
            Some(bgra.as_mut_ptr().cast()),
            &raw mut info,
            DIB_RGB_COLORS,
        )
    };
    if copied != height_i32 {
        return Err(ShellError::Visual(
            "Unable to read the preview bitmap".into(),
        ));
    }

    let has_alpha = bgra.chunks_exact(4).any(|pixel| pixel[3] != 0);
    let mut rgba = Vec::with_capacity(bgra.len());
    for pixel in bgra.chunks_exact(4) {
        let alpha = if has_alpha { pixel[3] } else { 255 };
        let unpremultiply = |channel: u8| {
            if alpha == 0 || alpha == 255 {
                channel
            } else {
                u8::try_from((u16::from(channel) * 255 / u16::from(alpha)).min(255))
                    .expect("unpremultiplied channel fits in u8")
            }
        };
        rgba.extend_from_slice(&[
            unpremultiply(pixel[2]),
            unpremultiply(pixel[1]),
            unpremultiply(pixel[0]),
            alpha,
        ]);
    }

    let image = RgbaImage::from_raw(width, height, rgba)
        .ok_or_else(|| ShellError::Visual("Windows returned an invalid RGBA buffer".into()))?;
    let mut png = Cursor::new(Vec::new());
    DynamicImage::ImageRgba8(image)
        .write_to(&mut png, ImageFormat::Png)
        .map_err(|error| ShellError::Visual(error.to_string()))?;
    let encoded = base64::engine::general_purpose::STANDARD.encode(png.into_inner());
    Ok(format!("data:image/png;base64,{encoded}"))
}

#[cfg(test)]
mod tests {
    use super::{shell_compatible_path, visual_cache_key, VisualKind};

    #[test]
    fn visual_cache_keys_include_the_path_kind_and_size() {
        assert_ne!(
            visual_cache_key(r"C:\Pictures\first.png", VisualKind::Icon),
            visual_cache_key(r"C:\Pictures\second.png", VisualKind::Icon)
        );
        assert_eq!(
            visual_cache_key("C:/Pictures/first.png", VisualKind::Icon),
            visual_cache_key(r"C:\Pictures\first.png", VisualKind::Icon)
        );
        assert_ne!(
            visual_cache_key(r"C:\Pictures\first.png", VisualKind::Icon),
            visual_cache_key(r"C:\Pictures\first.png", VisualKind::Thumbnail(64))
        );
        assert_ne!(
            visual_cache_key(r"C:\Pictures\first.png", VisualKind::Thumbnail(64)),
            visual_cache_key(r"C:\Pictures\first.png", VisualKind::Thumbnail(128))
        );
    }

    #[test]
    fn shell_paths_use_standard_windows_syntax() {
        assert_eq!(
            shell_compatible_path(r"\\?\C:\Pictures\first.png"),
            r"C:\Pictures\first.png"
        );
        assert_eq!(
            shell_compatible_path(r"\\?\UNC\server\share\first.png"),
            r"\\server\share\first.png"
        );
        assert_eq!(
            shell_compatible_path("C:/Pictures/first.png"),
            r"C:\Pictures\first.png"
        );
    }
}
