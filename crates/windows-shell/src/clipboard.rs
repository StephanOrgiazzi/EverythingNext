use crate::ShellError;

#[cfg(windows)]
pub fn copy_text(text: &str) -> Result<(), ShellError> {
    clipboard_win::set_clipboard(clipboard_win::formats::Unicode, text)
        .map_err(|error| ShellError::Clipboard(error.to_string()))
}

#[cfg(windows)]
pub fn copy_files(paths: &[String]) -> Result<(), ShellError> {
    use clipboard_win::{formats::FileList, raw, Clipboard, Setter};
    use std::path::Path;

    const PREFERRED_DROP_EFFECT: &str = "Preferred DropEffect";
    const DROP_EFFECT_COPY: u32 = 1;

    if paths.is_empty() {
        return Err(ShellError::Clipboard("No files were selected".into()));
    }
    if let Some(path) = paths
        .iter()
        .find(|path| path.trim().is_empty() || !Path::new(path).is_absolute())
    {
        return Err(ShellError::InvalidPath(path.clone()));
    }

    let preferred_drop_effect =
        clipboard_win::register_format(PREFERRED_DROP_EFFECT).ok_or_else(|| {
            ShellError::Clipboard("Unable to register the Windows copy format".into())
        })?;

    let _clipboard =
        Clipboard::new_attempts(10).map_err(|error| ShellError::Clipboard(error.to_string()))?;
    clipboard_win::empty().map_err(|error| ShellError::Clipboard(error.to_string()))?;
    FileList
        .write_clipboard(paths)
        .map_err(|error| ShellError::Clipboard(error.to_string()))?;
    raw::set_without_clear(preferred_drop_effect.get(), &DROP_EFFECT_COPY.to_ne_bytes())
        .map_err(|error| ShellError::Clipboard(error.to_string()))
}

#[cfg(not(windows))]
pub fn copy_text(_text: &str) -> Result<(), ShellError> {
    Err(ShellError::Clipboard(
        "The native clipboard requires Windows".into(),
    ))
}

#[cfg(not(windows))]
pub fn copy_files(_paths: &[String]) -> Result<(), ShellError> {
    Err(ShellError::Clipboard(
        "The native clipboard requires Windows".into(),
    ))
}
