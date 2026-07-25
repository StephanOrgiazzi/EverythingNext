mod clipboard;
mod file_operations;
mod icons;
mod visuals;

pub use clipboard::copy_text;
pub use file_operations::{open_path, rename_path, reveal_path, trash_paths, TrashReport};
pub use icons::IconCache;
pub use visuals::VisualCache;

#[derive(Debug, thiserror::Error)]
pub enum ShellError {
    #[error("The file or folder no longer exists: {0}")]
    MissingPath(String),
    #[error("Invalid path: {0}")]
    InvalidPath(String),
    #[error("Invalid Windows file name: {0}")]
    InvalidName(String),
    #[error("An item with this name already exists: {0}")]
    AlreadyExists(String),
    #[error("System operation failed: {0}")]
    Io(#[from] std::io::Error),
    #[error("Unable to write to the clipboard: {0}")]
    Clipboard(String),
    #[error("Unable to create the icon: {0}")]
    Icon(String),
    #[error("Unable to create the file preview: {0}")]
    Visual(String),
}
