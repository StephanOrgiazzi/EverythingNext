use leptos::prelude::*;
use leptos::task::spawn_local;

use super::storage;
use crate::backend;
use crate::diagnostics;

const STORAGE_KEY: &str = "everything-modern.excluded-folders";

#[derive(Clone, Copy)]
pub(in crate::app) struct ExcludedFoldersState {
    pub(in crate::app) folders: RwSignal<Vec<String>>,
}

impl ExcludedFoldersState {
    pub(in crate::app) fn new() -> Self {
        Self {
            folders: RwSignal::new(read_stored_folders()),
        }
    }

    fn add(self, raw_path: &str) -> Result<(), &'static str> {
        let path = validated_path(raw_path)?;
        if contains_case_insensitive(&self.folders.get_untracked(), &path) {
            return Err("This folder is already excluded.");
        }

        self.folders.update(|folders| folders.push(path));
        write_stored_folders(&self.folders.get_untracked());
        Ok(())
    }

    fn remove(self, path: &str) {
        self.folders
            .update(|folders| folders.retain(|folder| !folder.eq_ignore_ascii_case(path)));
        write_stored_folders(&self.folders.get_untracked());
    }
}

impl Default for ExcludedFoldersState {
    fn default() -> Self {
        Self::new()
    }
}

#[component]
#[allow(
    non_snake_case,
    reason = "Leptos components conventionally use PascalCase names"
)]
pub(in crate::app) fn ExcludedFoldersSetting(state: ExcludedFoldersState) -> impl IntoView {
    let validation_error = RwSignal::new(None::<String>);
    let is_picking = RwSignal::new(false);

    let pick_folder = move |_| {
        if is_picking.get_untracked() {
            return;
        }

        is_picking.set(true);
        validation_error.set(None);
        spawn_local(async move {
            match backend::pick_folder().await {
                Ok(Some(path)) => {
                    if let Err(message) = state.add(&path) {
                        validation_error.set(Some(message.into()));
                    }
                }
                Ok(None) => {}
                Err(message) => validation_error.set(Some(message)),
            }
            is_picking.set(false);
        });
    };

    view! {
        <div class="excluded-folders-setting grid gap-3">
            <p class="settings-description m-0 text-xs text-[var(--muted)]">
                "Folders excluded here are omitted from search results."
            </p>

            <div class="excluded-folder-controls flex gap-2">
                <button
                    class="h-[34px] rounded-[7px] border border-[var(--accent)] bg-[var(--accent)] px-[14px] text-white enabled:hover:brightness-[1.08] focus-visible:brightness-[1.12] disabled:opacity-65"
                    type="button"
                    disabled=move || is_picking.get()
                    aria-describedby="excluded-folder-error"
                    on:click=pick_folder
                >
                    {move || if is_picking.get() { "Choosing folder…" } else { "Choose folder…" }}
                </button>
            </div>
            <Show when=move || validation_error.get().is_some()>
                <p id="excluded-folder-error" class="settings-error m-0 text-xs text-[var(--danger)]" role="alert">
                    {move || validation_error.get().unwrap_or_default()}
                </p>
            </Show>

            <Show
                when=move || !state.folders.get().is_empty()
                fallback=|| {
                    view! {
                        <p class="excluded-folders-empty m-0 rounded-[7px] border border-dashed border-[var(--border)] p-[10px] text-center text-[var(--muted)]">"No folders are excluded."</p>
                    }
                }
            >
                <ul class="excluded-folders-list m-0 grid max-h-[180px] list-none gap-1 overflow-auto p-0">
                    <For
                        each=move || state.folders.get()
                        key=|path| path.to_ascii_lowercase()
                        children=move |path| {
                            let path_to_remove = path.clone();
                            view! {
                                <li class="flex min-w-0 items-center gap-2 rounded-[7px] border border-[var(--border-soft)] bg-[var(--surface-2)] py-[7px] pl-[10px] pr-2">
                                    <span class="min-w-0 flex-1 select-text overflow-hidden text-ellipsis whitespace-nowrap" title=path.clone()>{path.clone()}</span>
                                    <button
                                        class="h-[26px] rounded-[5px] bg-transparent px-2 text-[var(--danger)] hover:bg-[var(--hover)] focus-visible:bg-[var(--hover)]"
                                        type="button"
                                        aria-label="Remove excluded folder"
                                        on:click=move |_| state.remove(&path_to_remove)
                                    >
                                        "Remove"
                                    </button>
                                </li>
                            }
                        }
                    />
                </ul>
            </Show>
        </div>
    }
}

pub(in crate::app) fn compose_query(raw: &str, excluded_folders: &[String]) -> String {
    let exclusions = excluded_folders
        .iter()
        .map(|folder| format!(r#"!<whole:path:"{folder}"|ancestor:"{folder}">"#))
        .collect::<Vec<_>>()
        .join(" ");

    match (raw.trim(), exclusions.is_empty()) {
        ("", _) => exclusions,
        (query, true) => query.to_string(),
        (query, false) => format!("{query} {exclusions}"),
    }
}

fn validated_path(raw_path: &str) -> Result<String, &'static str> {
    let trimmed = raw_path.trim();
    let unquoted = trimmed
        .strip_prefix('"')
        .and_then(|path| path.strip_suffix('"'))
        .unwrap_or(trimmed);
    let mut path = unquoted.replace('/', r"\");
    while path.len() > 3 && path.ends_with('\\') {
        path.pop();
    }

    if path.is_empty() {
        return Err("Enter a folder path.");
    }
    if path.chars().any(|character| {
        character.is_control() || matches!(character, '"' | '<' | '>' | '|' | '?' | '*')
    }) {
        return Err("The folder path contains invalid characters.");
    }
    if !is_absolute_windows_path(&path) {
        return Err("Enter an absolute Windows or UNC folder path.");
    }

    Ok(path)
}

fn contains_case_insensitive(folders: &[String], candidate: &str) -> bool {
    folders
        .iter()
        .any(|folder| folder.eq_ignore_ascii_case(candidate))
}

fn is_absolute_windows_path(path: &str) -> bool {
    let bytes = path.as_bytes();
    let drive_path = bytes.len() >= 3
        && bytes[0].is_ascii_alphabetic()
        && bytes[1] == b':'
        && matches!(bytes[2], b'\\' | b'/');

    drive_path || is_unc_path(path)
}

fn is_unc_path(path: &str) -> bool {
    let remainder = path.strip_prefix(r"\\").or_else(|| path.strip_prefix("//"));
    let Some(remainder) = remainder else {
        return false;
    };

    let mut components = remainder.split(['\\', '/']);
    matches!(
        (components.next(), components.next()),
        (Some(server), Some(share)) if !server.is_empty() && !share.is_empty()
    )
}

fn read_stored_folders() -> Vec<String> {
    let Some(value) = storage::read(STORAGE_KEY) else {
        return Vec::new();
    };

    let stored = match serde_json::from_str::<Vec<String>>(&value) {
        Ok(stored) => stored,
        Err(error) => {
            diagnostics::warn(&format!("Unable to parse stored excluded folders: {error}"));
            return Vec::new();
        }
    };
    let mut folders = Vec::new();
    for path in stored {
        let path = match validated_path(&path) {
            Ok(path) => path,
            Err(error) => {
                diagnostics::warn(&format!("Ignoring stored excluded folder: {error}"));
                continue;
            }
        };
        if !folders
            .iter()
            .any(|folder: &String| folder.eq_ignore_ascii_case(&path))
        {
            folders.push(path);
        }
    }
    folders
}

fn write_stored_folders(folders: &[String]) {
    let value = match serde_json::to_string(folders) {
        Ok(value) => value,
        Err(error) => {
            diagnostics::warn(&format!("Unable to serialize excluded folders: {error}"));
            return;
        }
    };

    storage::write(STORAGE_KEY, &value);
}

#[cfg(test)]
mod tests {
    use super::{
        compose_query, contains_case_insensitive, is_absolute_windows_path, validated_path,
    };

    #[test]
    fn accepts_absolute_drive_and_unc_paths() {
        assert!(is_absolute_windows_path(r"C:\Users\Ada"));
        assert!(is_absolute_windows_path("D:/Projects"));
        assert!(is_absolute_windows_path(r"\\server\share"));
        assert!(is_absolute_windows_path("//server/share/folder"));
    }

    #[test]
    fn rejects_relative_or_incomplete_paths() {
        assert!(!is_absolute_windows_path(r"Users\Ada"));
        assert!(!is_absolute_windows_path(r"C:Users\Ada"));
        assert!(!is_absolute_windows_path(r"\Users\Ada"));
        assert!(!is_absolute_windows_path(r"\\server"));
        assert!(!is_absolute_windows_path(r"\\server\"));
    }

    #[test]
    fn trims_valid_paths_and_rejects_invalid_characters() {
        assert_eq!(
            validated_path(r"  C:\Users\Ada  "),
            Ok(r"C:\Users\Ada".into())
        );
        assert_eq!(
            validated_path(r#""D:/Build Output/""#),
            Ok(r"D:\Build Output".into())
        );
        assert_eq!(
            validated_path(r#"C:\Users\"Ada"#),
            Err("The folder path contains invalid characters.")
        );
    }

    #[test]
    fn detects_duplicates_without_considering_case() {
        let folders = vec![r"C:\Users\Ada".to_string()];

        assert!(contains_case_insensitive(&folders, r"c:\users\ADA"));
        assert!(!contains_case_insensitive(&folders, r"C:\Users\Grace"));
    }

    #[test]
    fn composes_search_and_folder_exclusions() {
        let folders = vec![r"C:\Windows".to_string(), r"D:\Build Output".to_string()];

        assert_eq!(
            compose_query("report ext:pdf", &folders),
            r#"report ext:pdf !<whole:path:"C:\Windows"|ancestor:"C:\Windows"> !<whole:path:"D:\Build Output"|ancestor:"D:\Build Output">"#
        );
    }

    #[test]
    fn composes_exclusions_without_a_raw_query() {
        let folders = vec![r"\\server\share".to_string()];

        assert_eq!(
            compose_query("   ", &folders),
            r#"!<whole:path:"\\server\share"|ancestor:"\\server\share">"#
        );
        assert_eq!(compose_query(" name ", &[]), "name");
    }
}
