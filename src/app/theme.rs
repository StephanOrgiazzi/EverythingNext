use js_sys::{Function, Reflect};
use leptos::prelude::*;
use wasm_bindgen::{JsCast, JsValue};

use super::browser_storage;
use crate::diagnostics;

const STORAGE_KEY: &str = "everything-modern.theme";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Theme {
    Light,
    Dark,
}

impl Theme {
    fn as_str(self) -> &'static str {
        match self {
            Self::Light => "light",
            Self::Dark => "dark",
        }
    }

    fn from_stored(value: &str) -> Option<Self> {
        match value {
            "light" => Some(Self::Light),
            "dark" => Some(Self::Dark),
            _ => None,
        }
    }
}

#[derive(Clone, Copy)]
pub struct ThemeState {
    current: RwSignal<Theme>,
}

impl ThemeState {
    pub fn new() -> Self {
        let initial = read_stored_theme().unwrap_or_else(preferred_theme);
        apply_to_document(initial);

        Self {
            current: RwSignal::new(initial),
        }
    }

    pub fn current(self) -> Theme {
        self.current.get()
    }

    pub fn set(self, theme: Theme) {
        apply_to_document(theme);
        write_stored_theme(theme);
        self.current.set(theme);
    }
}

#[component]
#[allow(
    non_snake_case,
    reason = "Leptos components conventionally use PascalCase names"
)]
pub fn ThemeSetting(state: ThemeState) -> impl IntoView {
    view! {
        <div class="theme-setting">
            <p class="settings-description">"Choose the app appearance."</p>
            <div class="theme-options" role="group" aria-label="Theme">
                <button
                    type="button"
                    class="theme-option"
                    class:active=move || state.current() == Theme::Light
                    aria-pressed=move || state.current() == Theme::Light
                    on:click=move |_| state.set(Theme::Light)
                >
                    "Light"
                </button>
                <button
                    type="button"
                    class="theme-option"
                    class:active=move || state.current() == Theme::Dark
                    aria-pressed=move || state.current() == Theme::Dark
                    on:click=move |_| state.set(Theme::Dark)
                >
                    "Dark"
                </button>
            </div>
        </div>
    }
}

fn preferred_theme() -> Theme {
    let prefers_dark = match prefers_dark_color_scheme() {
        Ok(prefers_dark) => prefers_dark,
        Err(error) => {
            diagnostics::warn_js("Unable to determine the preferred color scheme.", &error);
            false
        }
    };

    if prefers_dark {
        Theme::Dark
    } else {
        Theme::Light
    }
}

fn prefers_dark_color_scheme() -> Result<bool, JsValue> {
    let window =
        web_sys::window().ok_or_else(|| JsValue::from_str("browser window is unavailable"))?;
    let match_media =
        Reflect::get(window.as_ref(), &JsValue::from_str("matchMedia"))?.dyn_into::<Function>()?;
    let media_query = match_media.call1(
        window.as_ref(),
        &JsValue::from_str("(prefers-color-scheme: dark)"),
    )?;
    Reflect::get(&media_query, &JsValue::from_str("matches"))?
        .as_bool()
        .ok_or_else(|| JsValue::from_str("matchMedia returned a non-boolean matches value"))
}

fn apply_to_document(theme: Theme) {
    if let Some(root) = web_sys::window()
        .and_then(|window| window.document())
        .and_then(|document| document.document_element())
    {
        if let Err(error) = root.set_attribute("data-theme", theme.as_str()) {
            diagnostics::warn_js("Unable to apply the selected theme.", &error);
        }
    }
}

fn read_stored_theme() -> Option<Theme> {
    let value = browser_storage::read(STORAGE_KEY)?;
    let theme = Theme::from_stored(&value);
    if theme.is_none() {
        diagnostics::warn(&format!("Ignoring unknown stored theme: {value}"));
    }
    theme
}

fn write_stored_theme(theme: Theme) {
    browser_storage::write(STORAGE_KEY, theme.as_str());
}
