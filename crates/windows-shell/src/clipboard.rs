use crate::ShellError;

#[cfg(windows)]
pub fn copy_text(text: &str) -> Result<(), ShellError> {
    clipboard_win::set_clipboard(clipboard_win::formats::Unicode, text)
        .map_err(|error| ShellError::Clipboard(error.to_string()))
}

#[cfg(not(windows))]
pub fn copy_text(_text: &str) -> Result<(), ShellError> {
    Err(ShellError::Clipboard(
        "The native clipboard requires Windows".into(),
    ))
}
