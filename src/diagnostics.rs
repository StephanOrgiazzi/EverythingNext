use wasm_bindgen::JsValue;

pub(crate) fn warn(message: &str) {
    web_sys::console::warn_1(&JsValue::from_str(message));
}

pub(crate) fn warn_js(context: &str, error: &JsValue) {
    web_sys::console::warn_2(&JsValue::from_str(context), error);
}
