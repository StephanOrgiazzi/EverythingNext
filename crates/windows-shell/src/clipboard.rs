use crate::ShellError;

#[cfg(windows)]
pub fn copy_text(text: &str) -> Result<(), ShellError> {
    clipboard_win::set_clipboard(clipboard_win::formats::Unicode, text)
        .map_err(|error| ShellError::Clipboard(error.to_string()))
}

#[cfg(windows)]
pub fn copy_files(paths: &[String]) -> Result<(), ShellError> {
    use clipboard_win::{formats::FileList, Clipboard, Setter};

    if paths.is_empty() {
        return Err(ShellError::Clipboard("No files were selected".into()));
    }

    let _clipboard =
        Clipboard::new_attempts(10).map_err(|error| ShellError::Clipboard(error.to_string()))?;
    FileList
        .write_clipboard(paths)
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
