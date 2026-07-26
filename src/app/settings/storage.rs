use crate::diagnostics;

pub(in crate::app) fn read(key: &str) -> Option<String> {
    let storage = local_storage()?;
    match storage.get_item(key) {
        Ok(value) => value,
        Err(error) => {
            diagnostics::warn_js(
                &format!("Unable to read local storage key '{key}'."),
                &error,
            );
            None
        }
    }
}

pub(in crate::app) fn write(key: &str, value: &str) {
    let Some(storage) = local_storage() else {
        return;
    };
    if let Err(error) = storage.set_item(key, value) {
        diagnostics::warn_js(
            &format!("Unable to write local storage key '{key}'."),
            &error,
        );
    }
}

fn local_storage() -> Option<web_sys::Storage> {
    let Some(window) = web_sys::window() else {
        diagnostics::warn("Unable to access local storage: browser window is unavailable.");
        return None;
    };
    match window.local_storage() {
        Ok(Some(storage)) => Some(storage),
        Ok(None) => {
            diagnostics::warn("Local storage is unavailable.");
            None
        }
        Err(error) => {
            diagnostics::warn_js("Unable to access local storage.", &error);
            None
        }
    }
}
