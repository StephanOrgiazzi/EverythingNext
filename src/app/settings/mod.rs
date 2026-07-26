mod dialog;
pub(in crate::app) mod exclusions;
pub(in crate::app) mod storage;
mod theme;

pub(super) use dialog::SettingsDialog;
pub(in crate::app) use exclusions::compose_query;
pub(super) use exclusions::{ExcludedFoldersSetting, ExcludedFoldersState};
pub(super) use theme::{ThemeSetting, ThemeState};
