use js_sys::{Function, Reflect};
use leptos::prelude::*;
use wasm_bindgen::{JsCast, JsValue};

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
    let prefers_dark = (|| {
        let window = web_sys::window()?;
        let match_media = Reflect::get(window.as_ref(), &JsValue::from_str("matchMedia"))
            .ok()?
            .dyn_into::<Function>()
            .ok()?;
        let media_query = match_media
            .call1(
                window.as_ref(),
                &JsValue::from_str("(prefers-color-scheme: dark)"),
            )
            .ok()?;
        Reflect::get(&media_query, &JsValue::from_str("matches"))
            .ok()?
            .as_bool()
    })()
    .unwrap_or(false);

    if prefers_dark {
        Theme::Dark
    } else {
        Theme::Light
    }
}

fn apply_to_document(theme: Theme) {
    if let Some(root) = web_sys::window()
        .and_then(|window| window.document())
        .and_then(|document| document.document_element())
    {
        let _ = root.set_attribute("data-theme", theme.as_str());
    }
}

fn local_storage() -> Option<JsValue> {
    let window = web_sys::window()?;
    let storage = Reflect::get(window.as_ref(), &JsValue::from_str("localStorage")).ok()?;
    (!storage.is_null() && !storage.is_undefined()).then_some(storage)
}

fn read_stored_theme() -> Option<Theme> {
    let storage = local_storage()?;
    let get_item = Reflect::get(&storage, &JsValue::from_str("getItem"))
        .ok()?
        .dyn_into::<Function>()
        .ok()?;
    let value = get_item
        .call1(&storage, &JsValue::from_str(STORAGE_KEY))
        .ok()?
        .as_string()?;
    Theme::from_stored(&value)
}

fn write_stored_theme(theme: Theme) {
    let Some(storage) = local_storage() else {
        return;
    };
    let Ok(set_item) = Reflect::get(&storage, &JsValue::from_str("setItem")) else {
        return;
    };
    let Ok(set_item) = set_item.dyn_into::<Function>() else {
        return;
    };
    let _ = set_item.call2(
        &storage,
        &JsValue::from_str(STORAGE_KEY),
        &JsValue::from_str(theme.as_str()),
    );
}
