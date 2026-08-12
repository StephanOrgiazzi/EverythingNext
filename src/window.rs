use wasm_bindgen::prelude::*;

use crate::diagnostics;

#[wasm_bindgen]
extern "C" {
    type TauriWindow;

    #[wasm_bindgen(
        catch,
        js_namespace = ["window", "__TAURI__", "window"],
        js_name = getCurrentWindow
    )]
    fn get_current_window() -> Result<TauriWindow, JsValue>;

    #[wasm_bindgen(catch, method, structural, js_name = minimize)]
    async fn minimize_js(this: &TauriWindow) -> Result<JsValue, JsValue>;

    #[wasm_bindgen(catch, method, structural, js_name = toggleMaximize)]
    async fn toggle_maximize_js(this: &TauriWindow) -> Result<JsValue, JsValue>;

    #[wasm_bindgen(catch, method, structural, js_name = close)]
    async fn close_js(this: &TauriWindow) -> Result<JsValue, JsValue>;
}

#[derive(Clone, Copy)]
enum WindowAction {
    Minimize,
    ToggleMaximize,
    Close,
}

impl WindowAction {
    fn label(self) -> &'static str {
        match self {
            Self::Minimize => "minimize",
            Self::ToggleMaximize => "toggle-maximize",
            Self::Close => "close",
        }
    }
}

pub fn minimize() {
    spawn_action(WindowAction::Minimize);
}

pub fn toggle_maximize() {
    spawn_action(WindowAction::ToggleMaximize);
}

pub fn close() {
    spawn_action(WindowAction::Close);
}

fn spawn_action(action: WindowAction) {
    wasm_bindgen_futures::spawn_local(async move {
        if let Err(error) = perform_action(action).await {
            diagnostics::warn_js(
                &format!("Unable to perform window action '{}'.", action.label()),
                &error,
            );
        }
    });
}

async fn perform_action(action: WindowAction) -> Result<(), JsValue> {
    let app_window = get_current_window()?;
    match action {
        WindowAction::Minimize => app_window.minimize_js().await?,
        WindowAction::ToggleMaximize => app_window.toggle_maximize_js().await?,
        WindowAction::Close => app_window.close_js().await?,
    };
    Ok(())
}
