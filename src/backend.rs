use everything_core::{
    EngineStatus, QueryRequest, SearchPage, SelectionRequest, TrashOutcome, TrashPreparation,
};
use serde::de::DeserializeOwned;
use serde::Serialize;
use serde_wasm_bindgen::{from_value, to_value};
use wasm_bindgen::prelude::*;

#[wasm_bindgen(inline_js = r#"
export async function everythingModernInvoke(command, args) {
  return await window.__TAURI__.core.invoke(command, args);
}

export function everythingModernHasTauri() {
  return typeof window.__TAURI__?.core?.invoke === "function";
}

export async function everythingModernPickFolder() {
  return await window.__TAURI__.dialog.open({
    directory: true,
    multiple: false,
    title: "Choose a folder to exclude",
  });
}
"#)]
extern "C" {
    #[wasm_bindgen(catch, js_name = everythingModernInvoke)]
    async fn tauri_invoke(command: &str, args: JsValue) -> Result<JsValue, JsValue>;

    #[wasm_bindgen(js_name = everythingModernHasTauri)]
    fn has_tauri() -> bool;

    #[wasm_bindgen(catch, js_name = everythingModernPickFolder)]
    async fn tauri_pick_folder() -> Result<JsValue, JsValue>;
}

mod command {
    pub const ENGINE_STATUS: &str = "engine_status";
    pub const BEGIN_SEARCH_GENERATION: &str = "begin_search_generation";
    pub const SEARCH: &str = "search_everything";
    pub const FILE_ICON: &str = "get_file_icon";
    pub const FILE_VISUAL: &str = "get_file_visual";
    pub const OPEN_PATH: &str = "open_path";
    pub const REVEAL_PATH: &str = "reveal_path";
    pub const RENAME_PATH: &str = "rename_path";
    pub const PREPARE_TRASH: &str = "prepare_trash_selection";
    pub const EXECUTE_TRASH: &str = "execute_trash_snapshot";
    pub const CANCEL_TRASH: &str = "cancel_trash_snapshot";
    pub const COPY_TEXT: &str = "copy_text";
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct RequestIdArgs {
    request_id: u32,
}

#[derive(Serialize)]
struct RequestArgs<T> {
    request: T,
}

#[derive(Serialize)]
struct PathArgs<'a> {
    path: &'a str,
}

#[derive(Serialize)]
struct VisualArgs<'a> {
    path: &'a str,
    size: u32,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct RenameArgs<'a> {
    path: &'a str,
    new_name: &'a str,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SnapshotArgs {
    snapshot_id: u64,
}

#[derive(Serialize)]
struct TextArgs<'a> {
    text: &'a str,
}

async fn invoke<T: DeserializeOwned, A: Serialize>(command: &str, args: &A) -> Result<T, String> {
    let value = to_value(args).map_err(|error| error.to_string())?;
    let response = tauri_invoke(command, value)
        .await
        .map_err(js_error_to_string)?;
    from_value(response).map_err(|error| error.to_string())
}

pub async fn status() -> EngineStatus {
    if !has_tauri() {
        return EngineStatus {
            available: false,
            message: "Browser preview — run `cargo tauri dev` to use Everything.".into(),
            version: None,
        };
    }
    match invoke(command::ENGINE_STATUS, &()).await {
        Ok(status) => status,
        Err(error) => EngineStatus {
            available: false,
            message: error,
            version: None,
        },
    }
}

pub async fn begin_generation(request_id: u32) -> Result<(), String> {
    invoke::<serde_json::Value, _>(
        command::BEGIN_SEARCH_GENERATION,
        &RequestIdArgs { request_id },
    )
    .await
    .map(|_| ())
}

pub async fn search(request: QueryRequest) -> Result<SearchPage, String> {
    if !has_tauri() {
        return Ok(preview::search(request));
    }
    invoke(command::SEARCH, &RequestArgs { request }).await
}

pub async fn icon(path: &str) -> Result<Option<String>, String> {
    invoke::<Option<String>, _>(command::FILE_ICON, &PathArgs { path }).await
}

pub async fn visual(path: &str, size: u32) -> Result<Option<String>, String> {
    invoke::<Option<String>, _>(command::FILE_VISUAL, &VisualArgs { path, size }).await
}

pub async fn open(path: &str) -> Result<(), String> {
    invoke(command::OPEN_PATH, &PathArgs { path }).await
}

pub async fn reveal(path: &str) -> Result<(), String> {
    invoke(command::REVEAL_PATH, &PathArgs { path }).await
}

pub async fn rename(path: &str, new_name: &str) -> Result<String, String> {
    invoke(command::RENAME_PATH, &RenameArgs { path, new_name }).await
}

pub async fn prepare_trash(request: SelectionRequest) -> Result<TrashPreparation, String> {
    invoke(command::PREPARE_TRASH, &RequestArgs { request }).await
}

pub async fn execute_trash(snapshot_id: u64) -> Result<TrashOutcome, String> {
    invoke(command::EXECUTE_TRASH, &SnapshotArgs { snapshot_id }).await
}

pub async fn cancel_trash(snapshot_id: u64) -> Result<(), String> {
    invoke::<serde_json::Value, _>(command::CANCEL_TRASH, &SnapshotArgs { snapshot_id })
        .await
        .map(|_| ())
}

pub async fn copy_text(text: &str) -> Result<(), String> {
    invoke(command::COPY_TEXT, &TextArgs { text }).await
}

pub async fn pick_folder() -> Result<Option<String>, String> {
    if !has_tauri() {
        return Err("The native folder picker is only available in the desktop app.".into());
    }

    let selection = tauri_pick_folder().await.map_err(js_error_to_string)?;
    if selection.is_null() || selection.is_undefined() {
        return Ok(None);
    }

    selection
        .as_string()
        .map(Some)
        .ok_or_else(|| "The folder picker returned an invalid path.".into())
}

fn js_error_to_string(value: JsValue) -> String {
    value
        .as_string()
        .or_else(|| js_sys::Error::from(value).message().as_string())
        .unwrap_or_else(|| "Unknown JavaScript error".into())
}

mod preview {
    use everything_core::{QueryRequest, SearchPage, SearchResult};

    pub(super) fn search(request: QueryRequest) -> SearchPage {
        let total = if request.query.trim().is_empty() {
            0
        } else {
            12_480
        };
        let mut items = Vec::new();
        let end = (request.offset + request.limit).min(total);

        for index in request.offset..end {
            let is_dir = index % 11 == 0;
            let extension = match index % 6 {
                0 => "rs",
                1 => "pdf",
                2 => "png",
                3 => "md",
                4 => "zip",
                _ => "txt",
            };
            let name = if is_dir {
                format!("Projet {index:04}")
            } else {
                format!(
                    "{}-resultat-{:05}.{}",
                    request.query.replace(' ', "-"),
                    index,
                    extension
                )
            };
            let path = if is_dir {
                format!(r"C:\Users\Public\Documents\{name}")
            } else {
                format!(r"C:\Users\Public\Documents\Everything Modern\{name}")
            };
            items.push(SearchResult {
                id: format!("mock-{index}"),
                name,
                parent_path: path
                    .rsplit_once('\\')
                    .map(|(p, _)| p.to_string())
                    .unwrap_or_default(),
                full_path: path,
                size: (!is_dir).then_some((index as u64 + 1) * 48_713),
                modified_unix: Some(1_720_000_000 + (index as i64 * 733)),
                is_dir,
            });
        }

        SearchPage {
            request_id: request.request_id,
            offset: request.offset,
            total,
            items,
        }
    }
}
