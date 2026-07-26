use super::viewport::ResultViewport;
use gloo_timers::future::TimeoutFuture;
use leptos::prelude::*;
use leptos::task::spawn_local;
use wasm_bindgen::JsCast;
use web_sys::{HtmlElement, KeyboardEvent, MouseEvent};

use crate::diagnostics;

pub(super) const GRID_GAP: f64 = 8.0;
pub(super) const GRID_PADDING: f64 = 10.0;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) enum ViewMode {
    #[default]
    Details,
    Small,
    Medium,
    Large,
}

impl ViewMode {
    pub const ALL: [Self; 4] = [Self::Details, Self::Small, Self::Medium, Self::Large];

    pub fn key(self) -> &'static str {
        match self {
            Self::Details => "details",
            Self::Small => "small",
            Self::Medium => "medium",
            Self::Large => "large",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Details => "Details",
            Self::Small => "Small icons",
            Self::Medium => "Medium icons",
            Self::Large => "Large icons",
        }
    }

    pub fn item_height(self) -> f64 {
        match self {
            Self::Details => 34.0,
            Self::Small => 46.0,
            Self::Medium => 132.0,
            Self::Large => 184.0,
        }
    }

    pub fn visual_size(self) -> Option<u32> {
        match self {
            Self::Details | Self::Small => None,
            Self::Medium => Some(64),
            Self::Large => Some(96),
        }
    }

    pub fn is_grid(self) -> bool {
        self != Self::Details
    }

    pub fn min_width(self) -> f64 {
        match self {
            Self::Details => f64::INFINITY,
            Self::Small => 360.0,
            Self::Medium => 100.0,
            Self::Large => 120.0,
        }
    }

    pub fn max_columns(self) -> u32 {
        match self {
            Self::Details => 1,
            Self::Small => 2,
            Self::Medium => 10,
            Self::Large => 8,
        }
    }

    fn icon_path(self) -> &'static str {
        match self {
            Self::Details => {
                "M4 5h3v3H4V5Zm5 0h11v3H9V5ZM4 10.5h3v3H4v-3Zm5 0h11v3H9v-3ZM4 16h3v3H4v-3Zm5 0h11v3H9v-3Z"
            }
            Self::Small => {
                "M3 4h7v7H3V4Zm2 2v3h3V6H5Zm9-2h7v7h-7V4Zm2 2v3h3V6h-3ZM3 14h7v7H3v-7Zm2 2v3h3v-3H5Zm9-2h7v7h-7v-7Zm2 2v3h3v-3h-3Z"
            }
            Self::Medium => {
                "M3 3h8v8H3V3Zm2 2v4h4V5H5Zm8-2h8v8h-8V3Zm2 2v4h4V5h-4ZM3 13h8v8H3v-8Zm2 2v4h4v-4H5Zm8-2h8v8h-8v-8Zm2 2v4h4v-4h-4Z"
            }
            Self::Large => {
                "M2 2h9v9H2V2Zm2 2v5h5V4H4Zm9-2h9v9h-9V2Zm2 2v5h5V4h-5ZM2 13h9v9H2v-9Zm2 2v5h5v-5H4Zm9-2h9v9h-9v-9Zm2 2v5h5v-5h-5Z"
            }
        }
    }
}

#[component]
#[allow(
    non_snake_case,
    reason = "Leptos components conventionally use PascalCase names"
)]
pub(crate) fn ViewSwitcher(viewport: ResultViewport, open: RwSignal<bool>) -> impl IntoView {
    let trigger_ref = NodeRef::<leptos::html::Button>::new();

    let open_menu = move |focus: MenuFocus| {
        open.set(true);
        let active = viewport.mode.get_untracked();
        spawn_local(async move {
            TimeoutFuture::new(0).await;
            let index = match focus {
                MenuFocus::Active => ViewMode::ALL
                    .iter()
                    .position(|mode| *mode == active)
                    .unwrap_or(0),
                MenuFocus::Last => ViewMode::ALL.len() - 1,
            };
            focus_option(index);
        });
    };

    view! {
        <span class="command-bar-spacer" aria-hidden="true"></span>
        <div class="view-switcher">
            <button
                node_ref=trigger_ref
                type="button"
                class="command-button view-button"
                aria-haspopup="menu"
                aria-expanded=move || open.get()
                aria-label=move || format!(
                    "Choisir le mode d’affichage. Mode actuel : {}",
                    viewport.mode.get().label(),
                )
                title=move || format!("Affichage : {}", viewport.mode.get().label())
                on:click=move |event: MouseEvent| {
                    event.stop_propagation();
                    open.update(|open| *open = !*open);
                }
                on:keydown=move |event: KeyboardEvent| {
                    match event.key().as_str() {
                        "ArrowDown" => {
                            event.prevent_default();
                            open_menu(MenuFocus::Active);
                        }
                        "ArrowUp" => {
                            event.prevent_default();
                            open_menu(MenuFocus::Last);
                        }
                        "Escape" if open.get_untracked() => {
                            event.prevent_default();
                            open.set(false);
                        }
                        _ => {}
                    }
                }
            >
                {move || view_mode_icon(viewport.mode.get(), "view-button-icon")}
                <span class="view-button-label">{move || viewport.mode.get().label()}</span>
                <svg class="view-chevron" viewBox="0 0 24 24" aria-hidden="true">
                    <path d="m7 9 5 5 5-5 1.4 1.4-6.4 6.4-6.4-6.4L7 9Z"></path>
                </svg>
            </button>

            <div
                class="view-menu"
                role="menu"
                hidden=move || !open.get()
                on:click=move |event| event.stop_propagation()
            >
                {ViewMode::ALL
                    .into_iter()
                    .enumerate()
                    .map(|(index, mode)| {
                        view! {
                            <button
                                id=format!("view-option-{index}")
                                type="button"
                                class="view-option"
                                class:active=move || viewport.mode.get() == mode
                                role="menuitemradio"
                                aria-checked=move || viewport.mode.get() == mode
                                on:click=move |_| {
                                    viewport.set_mode(mode);
                                    open.set(false);
                                    focus_trigger(trigger_ref);
                                }
                                on:keydown=move |event: KeyboardEvent| {
                                    handle_option_key(event, index, open, trigger_ref);
                                }
                            >
                                {view_mode_icon(mode, "view-option-icon")}
                                <span>{mode.label()}</span>
                                <span class="view-option-check" aria-hidden="true">"✓"</span>
                            </button>
                        }
                    })
                    .collect_view()}
            </div>
        </div>
    }
}

#[derive(Clone, Copy)]
enum MenuFocus {
    Active,
    Last,
}

fn view_mode_icon(mode: ViewMode, class: &'static str) -> impl IntoView {
    view! {
        <svg class=class viewBox="0 0 24 24" aria-hidden="true">
            <path d=mode.icon_path()></path>
        </svg>
    }
}

fn handle_option_key(
    event: KeyboardEvent,
    current: usize,
    open: RwSignal<bool>,
    trigger_ref: NodeRef<leptos::html::Button>,
) {
    let last = ViewMode::ALL.len() - 1;
    let target = match event.key().as_str() {
        "ArrowDown" => Some((current + 1) % ViewMode::ALL.len()),
        "ArrowUp" => Some((current + last) % ViewMode::ALL.len()),
        "Home" => Some(0),
        "End" => Some(last),
        "Escape" => {
            event.prevent_default();
            event.stop_propagation();
            open.set(false);
            focus_trigger(trigger_ref);
            None
        }
        "Tab" => {
            open.set(false);
            None
        }
        _ => None,
    };
    if let Some(target) = target {
        event.prevent_default();
        focus_option(target);
    }
}

fn focus_option(index: usize) {
    let Some(element) = web_sys::window()
        .and_then(|window| window.document())
        .and_then(|document| document.get_element_by_id(&format!("view-option-{index}")))
        .and_then(|element| element.dyn_into::<HtmlElement>().ok())
    else {
        return;
    };
    if let Err(error) = element.focus() {
        diagnostics::warn_js("Unable to focus a view-mode option.", &error);
    }
}

fn focus_trigger(trigger_ref: NodeRef<leptos::html::Button>) {
    if let Some(trigger) = trigger_ref.get() {
        if let Err(error) = trigger.focus() {
            diagnostics::warn_js("Unable to focus the view-mode trigger.", &error);
        }
    }
}
