use wasm_bindgen::prelude::*;

use crate::diagnostics;

#[wasm_bindgen(inline_js = r#"
export async function everythingNextWindowAction(action) {
  const getCurrentWindow = window.__TAURI__?.window?.getCurrentWindow;
  if (!getCurrentWindow) return;
  const appWindow = getCurrentWindow();
  if (action === "minimize") await appWindow.minimize();
  if (action === "toggle-maximize") await appWindow.toggleMaximize();
  if (action === "close") await appWindow.close();
}
"#)]
extern "C" {
    #[wasm_bindgen(catch, js_name = everythingNextWindowAction)]
    async fn window_action_js(action: &str) -> Result<JsValue, JsValue>;
}

pub fn minimize() {
    spawn_action("minimize");
}

pub fn toggle_maximize() {
    spawn_action("toggle-maximize");
}

pub fn close() {
    spawn_action("close");
}

fn spawn_action(action: &'static str) {
    wasm_bindgen_futures::spawn_local(async move {
        if let Err(error) = window_action_js(action).await {
            diagnostics::warn_js(
                &format!("Unable to perform window action '{action}'."),
                &error,
            );
        }
    });
}
