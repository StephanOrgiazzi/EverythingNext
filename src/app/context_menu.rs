use super::search::{SearchResults, RESULT_ROW_HEIGHT};
use super::selection::ResultSelection;
use everything_core::SearchResult;
use leptos::prelude::*;
use wasm_bindgen::JsCast;
use web_sys::{Element, KeyboardEvent};

const MENU_WIDTH: i32 = 272;
const MENU_HEIGHT: i32 = 324;
const VIEWPORT_MARGIN: i32 = 8;

#[derive(Clone)]
pub(super) struct ContextMenuState {
    pub x: i32,
    pub y: i32,
    pub item: SearchResult,
}

#[derive(Clone, Copy)]
pub(super) struct ResultContextMenu {
    pub state: RwSignal<Option<ContextMenuState>>,
}

impl ResultContextMenu {
    pub fn new() -> Self {
        Self {
            state: RwSignal::new(None),
        }
    }

    pub fn close(self) {
        self.state.set(None);
    }

    pub fn open_at_pointer(
        self,
        index: u32,
        item: SearchResult,
        x: i32,
        y: i32,
        selection: ResultSelection,
    ) {
        selection.select_context_item(index);
        let (x, y) = clamp_to_viewport(x, y);
        self.state.set(Some(ContextMenuState { x, y, item }));
    }

    pub fn open_at_focused_row(
        self,
        results: SearchResults,
        selection: ResultSelection,
        list_ref: NodeRef<leptos::html::Div>,
    ) {
        let Some(index) = selection.focused_index.get_untracked() else {
            return;
        };
        let Some(item) = results.item_at(index) else {
            return;
        };
        selection.select_context_item(index);
        let (x, y) = keyboard_position(index, list_ref);
        self.state.set(Some(ContextMenuState { x, y, item }));
    }
}

pub(super) fn event_target_is_interactive(event: &KeyboardEvent) -> bool {
    event
        .target()
        .and_then(|target| target.dyn_into::<Element>().ok())
        .is_some_and(|element| {
            matches!(
                element.tag_name().as_str(),
                "INPUT" | "TEXTAREA" | "SELECT" | "BUTTON"
            ) || element.get_attribute("contenteditable").as_deref() == Some("true")
        })
}

fn keyboard_position(index: u32, list_ref: NodeRef<leptos::html::Div>) -> (i32, i32) {
    let Some(list) = list_ref.get() else {
        return clamp_to_viewport(32, 96);
    };
    let rect = list.get_bounding_client_rect();
    let row_y = rect.top() + index as f64 * RESULT_ROW_HEIGHT - list.scroll_top() as f64
        + RESULT_ROW_HEIGHT;
    clamp_to_viewport((rect.left() + 180.0) as i32, row_y as i32)
}

fn clamp_to_viewport(x: i32, y: i32) -> (i32, i32) {
    let (width, height) = web_sys::window()
        .map(|window| {
            let width = window
                .inner_width()
                .ok()
                .and_then(|value| value.as_f64())
                .unwrap_or(1280.0) as i32;
            let height = window
                .inner_height()
                .ok()
                .and_then(|value| value.as_f64())
                .unwrap_or(720.0) as i32;
            (width, height)
        })
        .unwrap_or((1280, 720));
    (
        x.clamp(VIEWPORT_MARGIN, (width - MENU_WIDTH).max(VIEWPORT_MARGIN)),
        y.clamp(VIEWPORT_MARGIN, (height - MENU_HEIGHT).max(VIEWPORT_MARGIN)),
    )
}
