use super::selection::ResultSelection;
use super::view_modes::{GRID_GAP, GRID_PADDING};
use super::viewport::ResultViewport;
use crate::app::search_workspace::search::{SearchResults, RESULT_ROW_HEIGHT};
use everything_core::SearchResult;
use leptos::prelude::*;
use wasm_bindgen::JsCast;
use web_sys::{Element, KeyboardEvent};

use crate::diagnostics;

const MENU_WIDTH: i32 = 272;
const MENU_HEIGHT: i32 = 324;
const VIEWPORT_MARGIN: i32 = 8;

#[derive(Clone)]
pub(crate) struct ContextMenuState {
    pub x: i32,
    pub y: i32,
    pub item: SearchResult,
}

#[derive(Clone, Copy)]
pub(crate) struct ResultContextMenu {
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
        viewport: ResultViewport,
    ) {
        let Some(index) = selection.focused_index.get_untracked() else {
            return;
        };
        let Some(item) = results.item_at(index) else {
            return;
        };
        selection.select_context_item(index);
        let (x, y) = keyboard_position(index, viewport);
        self.state.set(Some(ContextMenuState { x, y, item }));
    }
}

pub(crate) fn event_target_is_interactive(event: &KeyboardEvent) -> bool {
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

fn keyboard_position(index: u32, viewport: ResultViewport) -> (i32, i32) {
    let Some(list) = viewport.list_ref.get() else {
        return clamp_to_viewport(32, 96);
    };
    let rect = list.get_bounding_client_rect();
    let mode = viewport.mode.get_untracked();
    if !mode.is_grid() {
        let row_y = rect.top() + f64::from(index) * RESULT_ROW_HEIGHT
            - f64::from(list.scroll_top())
            + RESULT_ROW_HEIGHT;
        return clamp_to_viewport((rect.left() + 180.0) as i32, row_y as i32);
    }

    let columns = viewport.columns.get_untracked().max(1);
    let width = viewport.grid_width.get_untracked();
    let cell_width = ((width - GRID_PADDING * 2.0 - GRID_GAP * f64::from(columns - 1))
        / f64::from(columns))
    .max(120.0);
    let column = index % columns;
    let row = index / columns;
    let x = rect.left()
        + GRID_PADDING
        + f64::from(column) * (cell_width + GRID_GAP)
        + cell_width.min(180.0);
    let y = rect.top() + GRID_PADDING + f64::from(row + 1) * mode.item_height()
        - f64::from(list.scroll_top());
    clamp_to_viewport(x as i32, y as i32)
}

fn clamp_to_viewport(x: i32, y: i32) -> (i32, i32) {
    let (width, height) = web_sys::window()
        .map(|window| {
            let width = viewport_dimension(window.inner_width(), "width", 1280.0);
            let height = viewport_dimension(window.inner_height(), "height", 720.0);
            (width, height)
        })
        .unwrap_or((1280, 720));
    (
        x.clamp(VIEWPORT_MARGIN, (width - MENU_WIDTH).max(VIEWPORT_MARGIN)),
        y.clamp(VIEWPORT_MARGIN, (height - MENU_HEIGHT).max(VIEWPORT_MARGIN)),
    )
}

fn viewport_dimension(
    result: Result<wasm_bindgen::JsValue, wasm_bindgen::JsValue>,
    name: &str,
    fallback: f64,
) -> i32 {
    match result {
        Ok(value) => value.as_f64().unwrap_or_else(|| {
            diagnostics::warn(&format!(
                "Viewport {name} was not numeric; using {fallback}px."
            ));
            fallback
        }) as i32,
        Err(error) => {
            diagnostics::warn_js(
                &format!("Unable to read viewport {name}; using {fallback}px."),
                &error,
            );
            fallback as i32
        }
    }
}
