use everything_core::{
    EngineStatus, QueryRequest, SearchPage, SearchResult, SelectionRequest, SortColumn,
    SortDirection, SortSpec,
};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_wasm_bindgen::{from_value, to_value};
use wasm_bindgen::prelude::*;

#[wasm_bindgen(inline_js = r#"
export async function everythingModernInvoke(command, args) {
  const invoke = window.__TAURI__?.core?.invoke;
  if (!invoke) throw new Error("Tauri API unavailable");
  return await invoke(command, args);
}

export async function everythingModernWindowAction(action) {
  const getCurrentWindow = window.__TAURI__?.window?.getCurrentWindow;
  if (!getCurrentWindow) return;
  const appWindow = getCurrentWindow();
  if (action === "minimize") await appWindow.minimize();
  if (action === "toggle-maximize") await appWindow.toggleMaximize();
  if (action === "close") await appWindow.close();
}
"#)]
extern "C" {
    #[wasm_bindgen(catch, js_name = everythingModernInvoke)]
    async fn tauri_invoke(command: &str, args: JsValue) -> Result<JsValue, JsValue>;

    #[wasm_bindgen(catch, js_name = everythingModernWindowAction)]
    async fn window_action_js(action: &str) -> Result<JsValue, JsValue>;
}

async fn invoke<T: DeserializeOwned, A: Serialize>(command: &str, args: &A) -> Result<T, String> {
    let value = to_value(args).map_err(|error| error.to_string())?;
    let response = tauri_invoke(command, value)
        .await
        .map_err(js_error_to_string)?;
    from_value(response).map_err(|error| error.to_string())
}

pub async fn status() -> EngineStatus {
    match invoke("engine_status", &serde_json::json!({})).await {
        Ok(status) => status,
        Err(error) if error.contains("Tauri API unavailable") => EngineStatus {
            available: false,
            message: "Aperçu navigateur — lancez `cargo tauri dev` pour utiliser Everything.".into(),
            version: None,
        },
        Err(error) => EngineStatus {
            available: false,
            message: error,
            version: None,
        },
    }
}


pub async fn begin_generation(request_id: u32) {
    let _ = invoke::<serde_json::Value, _>(
        "begin_search_generation",
        &serde_json::json!({ "requestId": request_id }),
    )
    .await;
}

pub async fn search(request: QueryRequest) -> Result<SearchPage, String> {
    match invoke("search_everything", &serde_json::json!({ "request": request })).await {
        Ok(page) => Ok(page),
        Err(error) if error.contains("Tauri API unavailable") => Ok(mock_page(request)),
        Err(error) => Err(error),
    }
}

pub async fn icon(path: &str) -> Option<String> {
    invoke::<Option<String>, _>("get_file_icon", &serde_json::json!({ "path": path }))
        .await
        .ok()
        .flatten()
}

pub async fn open(path: &str) -> Result<(), String> {
    invoke("open_path", &serde_json::json!({ "path": path })).await
}

pub async fn reveal(path: &str) -> Result<(), String> {
    invoke("reveal_path", &serde_json::json!({ "path": path })).await
}

pub async fn rename(path: &str, new_name: &str) -> Result<String, String> {
    invoke(
        "rename_path",
        &serde_json::json!({ "path": path, "newName": new_name }),
    )
    .await
}

#[derive(Debug, Clone, Deserialize)]
pub struct TrashOutcome {
    pub deleted: usize,
    pub failures: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct TrashPreparation {
    pub snapshot_id: u64,
    pub count: usize,
}

pub async fn prepare_trash(request: SelectionRequest) -> Result<TrashPreparation, String> {
    invoke(
        "prepare_trash_selection",
        &serde_json::json!({ "request": request }),
    )
    .await
}

pub async fn execute_trash(snapshot_id: u64) -> Result<TrashOutcome, String> {
    invoke(
        "execute_trash_snapshot",
        &serde_json::json!({ "snapshotId": snapshot_id }),
    )
    .await
}

pub async fn cancel_trash(snapshot_id: u64) {
    let _ = invoke::<serde_json::Value, _>(
        "cancel_trash_snapshot",
        &serde_json::json!({ "snapshotId": snapshot_id }),
    )
    .await;
}

pub async fn copy_text(text: &str) -> Result<(), String> {
    invoke("copy_text", &serde_json::json!({ "text": text })).await
}

pub fn minimize_window() {
    spawn_window_action("minimize");
}

pub fn toggle_maximize_window() {
    spawn_window_action("toggle-maximize");
}

pub fn close_window() {
    spawn_window_action("close");
}

fn spawn_window_action(action: &'static str) {
    wasm_bindgen_futures::spawn_local(async move {
        let _ = window_action_js(action).await;
    });
}

fn js_error_to_string(value: JsValue) -> String {
    value
        .as_string()
        .or_else(|| js_sys::Error::from(value).message().as_string())
        .unwrap_or_else(|| "Erreur JavaScript inconnue".into())
}

fn mock_page(request: QueryRequest) -> SearchPage {
    let total = if request.query.trim().is_empty() { 0 } else { 12_480 };
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
            format!("Projet {:04}", index)
        } else {
            format!("{}-resultat-{:05}.{}", request.query.replace(' ', "-"), index, extension)
        };
        let path = if is_dir {
            format!(r"C:\Users\Public\Documents\{}", name)
        } else {
            format!(r"C:\Users\Public\Documents\Everything Modern\{}", name)
        };
        items.push(SearchResult {
            id: format!("mock-{index}"),
            name,
            parent_path: path.rsplit_once('\\').map(|(p, _)| p.to_string()).unwrap_or_default(),
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

#[allow(dead_code)]
fn _keep_model_variants_linked() -> SortSpec {
    SortSpec {
        column: SortColumn::Name,
        direction: SortDirection::Ascending,
    }
}
