use super::view_modes::{ViewMode, GRID_GAP, GRID_PADDING};
use crate::app::search_workspace::search::{RESULT_ROW_HEIGHT, VIRTUALIZATION_OVERSCAN};
use gloo_timers::future::TimeoutFuture;
use leptos::prelude::*;
use leptos::task::spawn_local;
use wasm_bindgen::JsCast;
use web_sys::HtmlDivElement;

use crate::app::settings::storage;
use crate::diagnostics;

const VIEW_MODE_STORAGE_KEY: &str = "everything-next-view-mode";

#[derive(Clone, Copy)]
pub(crate) struct ResultViewport {
    pub list_ref: NodeRef<leptos::html::Div>,
    pub scroll_top: RwSignal<f64>,
    pub height: RwSignal<f64>,
    pub grid_width: RwSignal<f64>,
    pub mode: RwSignal<ViewMode>,
    pub columns: RwSignal<u32>,
}

impl ResultViewport {
    pub fn new() -> Self {
        Self {
            list_ref: NodeRef::new(),
            scroll_top: RwSignal::new(0.0),
            height: RwSignal::new(640.0),
            grid_width: RwSignal::new(0.0),
            mode: RwSignal::new(load_view_mode()),
            columns: RwSignal::new(1),
        }
    }

    pub fn monitor_dimensions(self) {
        spawn_local(async move {
            loop {
                self.measure_dimensions();
                TimeoutFuture::new(120).await;
            }
        });
    }

    pub fn visible_start(self) -> Memo<u32> {
        Memo::new(move |_| {
            let mode = self.mode.get();
            let columns = self.columns.get();
            let first_row = (self.scroll_top.get() / mode.item_height()).floor() as u32;
            first_row
                .saturating_sub(VIRTUALIZATION_OVERSCAN)
                .saturating_mul(columns)
        })
    }

    pub fn visible_end(self, start: Memo<u32>, total: RwSignal<u32>) -> Memo<u32> {
        Memo::new(move |_| {
            let mode = self.mode.get();
            let columns = self.columns.get();
            let visible_rows = (self.height.get() / mode.item_height()).ceil() as u32;
            let count = visible_rows
                .saturating_add(VIRTUALIZATION_OVERSCAN * 2)
                .saturating_mul(columns);
            start.get().saturating_add(count).min(total.get())
        })
    }

    pub fn update_from_scroll_event(self, event: web_sys::Event) {
        let Some(element) = event
            .target()
            .and_then(|target| target.dyn_into::<HtmlDivElement>().ok())
        else {
            return;
        };
        self.scroll_top.set(f64::from(element.scroll_top()));
        self.height.set(f64::from(element.client_height()));
        self.update_width_and_columns(&element);
    }

    pub fn reset_scroll(self) {
        self.scroll_top.set(0.0);
        if let Some(list) = self.list_ref.get() {
            list.set_scroll_top(0);
            self.height.set(f64::from(list.client_height()));
        }
    }

    pub fn scroll_row_into_view(self, index: u32) {
        let Some(list) = self.list_ref.get() else {
            return;
        };
        let mode = self.mode.get_untracked();
        let columns = self.columns.get_untracked().max(1);
        let visual_row = index / columns;
        let top = if mode.is_grid() {
            GRID_PADDING + f64::from(visual_row) * mode.item_height()
        } else {
            f64::from(index) * RESULT_ROW_HEIGHT
        };
        let bottom = top + mode.item_height();
        let current_top = f64::from(list.scroll_top());
        let current_bottom = current_top + f64::from(list.client_height());
        if top < current_top {
            self.set_scroll_top(top.max(0.0));
        } else if bottom > current_bottom {
            self.set_scroll_top(bottom - f64::from(list.client_height()));
        }
    }

    pub fn set_mode(self, mode: ViewMode) {
        if mode == self.mode.get_untracked() {
            return;
        }
        let anchor = self.first_visible_index();
        self.mode.set(mode);
        let width = self
            .list_ref
            .get()
            .map(|list| f64::from(list.client_width()))
            .unwrap_or_else(|| self.grid_width.get_untracked());
        let columns = calculate_columns(mode, width);
        self.columns.set(columns);
        let target = if mode.is_grid() {
            f64::from(anchor / columns) * mode.item_height()
        } else {
            f64::from(anchor) * RESULT_ROW_HEIGHT
        };
        self.scroll_top.set(target);
        self.set_scroll_top_after_layout(target, mode);
        store_view_mode(mode);
    }

    pub fn canvas_height(self, total: u32) -> f64 {
        let mode = self.mode.get();
        if !mode.is_grid() {
            return f64::from(total) * RESULT_ROW_HEIGHT;
        }
        let rows = total.div_ceil(self.columns.get().max(1));
        GRID_PADDING * 2.0 + f64::from(rows) * mode.item_height()
    }

    pub fn item_style(self, index: u32) -> String {
        let mode = self.mode.get_untracked();
        if !mode.is_grid() {
            return format!(
                "transform: translateY({}px)",
                f64::from(index) * RESULT_ROW_HEIGHT
            );
        }

        let columns = self.columns.get_untracked().max(1);
        let width = self.grid_width.get_untracked();
        let cell_width = ((width - GRID_PADDING * 2.0 - GRID_GAP * f64::from(columns - 1))
            / f64::from(columns))
        .max(mode.min_width());
        let column = index % columns;
        let row = index / columns;
        let x = GRID_PADDING + f64::from(column) * (cell_width + GRID_GAP);
        let y = GRID_PADDING + f64::from(row) * mode.item_height();
        format!(
            "width: {cell_width}px; height: {}px; transform: translate3d({x}px, {y}px, 0)",
            mode.item_height() - GRID_GAP,
        )
    }

    pub fn row_count(self, total: u32) -> u32 {
        total.div_ceil(self.columns.get().max(1))
    }

    pub fn page_step(self) -> i32 {
        let mode = self.mode.get_untracked();
        let rows = (self.height.get_untracked() / mode.item_height())
            .floor()
            .max(1.0) as i32;
        rows * i32::try_from(self.columns.get_untracked().max(1))
            .expect("the grid column count fits in i32")
    }

    pub fn navigation_delta(self, key: &str) -> Option<i32> {
        let columns = i32::try_from(self.columns.get_untracked().max(1))
            .expect("the grid column count fits in i32");
        match (self.mode.get_untracked(), key) {
            (_, "ArrowDown") => Some(columns),
            (_, "ArrowUp") => Some(-columns),
            (mode, "ArrowRight") if mode.is_grid() => Some(1),
            (mode, "ArrowLeft") if mode.is_grid() => Some(-1),
            _ => None,
        }
    }

    fn measure_dimensions(self) {
        let Some(list) = self.list_ref.get() else {
            return;
        };
        let height = f64::from(list.client_height());
        if (height - self.height.get_untracked()).abs() > 0.5 {
            self.height.set(height);
        }
        self.update_width_and_columns(&list);
    }

    fn update_width_and_columns(self, list: &HtmlDivElement) {
        let mode = self.mode.get_untracked();
        let width = if mode.is_grid() {
            f64::from(list.client_width())
        } else {
            match list.query_selector(".virtual-canvas") {
                Ok(Some(canvas)) => canvas.get_bounding_client_rect().width(),
                Ok(None) => f64::from(list.client_width()),
                Err(error) => {
                    diagnostics::warn_js("Unable to locate the result canvas.", &error);
                    f64::from(list.client_width())
                }
            }
        };
        if (width - self.grid_width.get_untracked()).abs() > 0.5 {
            self.grid_width.set(width);
        }

        let next_columns = calculate_columns(mode, f64::from(list.client_width()));
        let current_columns = self.columns.get_untracked().max(1);
        if next_columns != current_columns {
            let anchor = self.first_visible_index();
            self.columns.set(next_columns);
            if mode.is_grid() {
                self.set_scroll_top(f64::from(anchor / next_columns) * mode.item_height());
            }
        }
    }

    fn first_visible_index(self) -> u32 {
        let mode = self.mode.get_untracked();
        if mode.is_grid() {
            (self.scroll_top.get_untracked() / mode.item_height()).floor() as u32
                * self.columns.get_untracked().max(1)
        } else {
            (self.scroll_top.get_untracked() / RESULT_ROW_HEIGHT).floor() as u32
        }
    }

    fn set_scroll_top(self, top: f64) {
        let top = top.max(0.0);
        if let Some(list) = self.list_ref.get() {
            list.set_scroll_top(top as i32);
            self.scroll_top.set(f64::from(list.scroll_top()));
        } else {
            self.scroll_top.set(top);
        }
    }

    fn set_scroll_top_after_layout(self, top: f64, mode: ViewMode) {
        let callback = wasm_bindgen::closure::Closure::once_into_js(move |_timestamp: f64| {
            if self.mode.get_untracked() == mode {
                self.set_scroll_top(top);
            }
        });
        let scheduled = web_sys::window().is_some_and(|window| {
            window
                .request_animation_frame(callback.unchecked_ref())
                .is_ok()
        });
        if !scheduled && self.mode.get_untracked() == mode {
            self.set_scroll_top(top);
        }
    }
}

fn calculate_columns(mode: ViewMode, width: f64) -> u32 {
    if !mode.is_grid() {
        return 1;
    }
    let available = (width - GRID_PADDING * 2.0).max(mode.min_width());
    (((available + GRID_GAP) / (mode.min_width() + GRID_GAP)).floor() as u32)
        .clamp(1, mode.max_columns())
}

fn load_view_mode() -> ViewMode {
    let stored = storage::read(VIEW_MODE_STORAGE_KEY);
    match stored.as_deref() {
        Some("small") => ViewMode::Small,
        Some("medium") => ViewMode::Medium,
        Some("large") => ViewMode::Large,
        Some(value) => {
            diagnostics::warn(&format!(
                "Ignoring unknown stored result view mode: {value}"
            ));
            ViewMode::Details
        }
        _ => ViewMode::Details,
    }
}

fn store_view_mode(mode: ViewMode) {
    storage::write(VIEW_MODE_STORAGE_KEY, mode.key());
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn icon_columns_grow_with_the_viewport() {
        assert_eq!(calculate_columns(ViewMode::Medium, 520.0), 4);
        assert_eq!(calculate_columns(ViewMode::Medium, 540.0), 5);
        assert_eq!(calculate_columns(ViewMode::Medium, 1_700.0), 16);
    }

    #[test]
    fn icon_columns_keep_sensible_mode_limits() {
        assert_eq!(calculate_columns(ViewMode::Small, 4_000.0), 6);
        assert_eq!(calculate_columns(ViewMode::Medium, 4_000.0), 20);
        assert_eq!(calculate_columns(ViewMode::Large, 4_000.0), 14);
    }
}
