use leptos::prelude::*;
use wasm_bindgen::JsValue;

use super::storage;
use crate::diagnostics;

const STORAGE_KEY: &str = "everything-next.theme";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Theme {
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
pub(in crate::app) struct ThemeState {
    current: RwSignal<Theme>,
}

impl ThemeState {
    pub(in crate::app) fn new() -> Self {
        let initial = read_stored_theme().unwrap_or_else(preferred_theme);
        apply_to_document(initial);

        Self {
            current: RwSignal::new(initial),
        }
    }

    fn current(self) -> Theme {
        self.current.get()
    }

    fn set(self, theme: Theme) {
        apply_to_document(theme);
        write_stored_theme(theme);
        self.current.set(theme);
    }
}

#[component]
pub(in crate::app) fn ThemeSetting(state: ThemeState) -> impl IntoView {
    view! {
        <div class="theme-setting flex items-center justify-between gap-6 max-[560px]:flex-col max-[560px]:items-stretch max-[560px]:gap-[10px]">
            <p class="settings-description text-xs text-[var(--muted)]">"Choose the app appearance."</p>
            <div class="theme-options flex shrink-0 rounded-[7px] border border-[var(--border)] bg-[var(--surface-2)] p-0.5 max-[560px]:self-start" role="group" aria-label="Theme">
                <button
                    type="button"
                    class="theme-option h-7 min-w-[62px] rounded-[5px] bg-transparent px-[10px] text-[var(--muted)] hover:bg-[var(--hover)] hover:text-[var(--text)] focus-visible:bg-[var(--hover)] [&.active]:bg-[var(--surface-solid)] [&.active]:text-[var(--text)] [&.active]:shadow-[0_1px_3px_rgba(0,0,0,.12)]"
                    class:active=move || state.current() == Theme::Light
                    aria-pressed=move || state.current() == Theme::Light
                    on:click=move |_| state.set(Theme::Light)
                >
                    "Light"
                </button>
                <button
                    type="button"
                    class="theme-option h-7 min-w-[62px] rounded-[5px] bg-transparent px-[10px] text-[var(--muted)] hover:bg-[var(--hover)] hover:text-[var(--text)] focus-visible:bg-[var(--hover)] [&.active]:bg-[var(--surface-solid)] [&.active]:text-[var(--text)] [&.active]:shadow-[0_1px_3px_rgba(0,0,0,.12)]"
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
    let media_query = window
        .match_media("(prefers-color-scheme: dark)")?
        .ok_or_else(|| JsValue::from_str("matchMedia returned no media query list"))?;
    Ok(media_query.matches())
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
    let value = storage::read(STORAGE_KEY)?;
    let theme = Theme::from_stored(&value);
    if theme.is_none() {
        diagnostics::warn(&format!("Ignoring unknown stored theme: {value}"));
    }
    theme
}

fn write_stored_theme(theme: Theme) {
    storage::write(STORAGE_KEY, theme.as_str());
}
