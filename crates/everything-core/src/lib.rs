mod model;
mod windows_name;

pub use model::*;
pub use windows_name::{validate_windows_name, WindowsNameError};

pub const MAX_CONCURRENT_THUMBNAIL_LOADS: usize = 8;
