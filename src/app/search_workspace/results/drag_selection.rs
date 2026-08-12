mod geometry;

use self::geometry::{selection_in_rectangle, DragSelectionRect, SelectionLayout};
use super::selection::SelectionSnapshot;
use super::{ResultSelection, ResultViewport};
use crate::diagnostics;
use gloo_timers::future::TimeoutFuture;
use leptos::prelude::*;
use leptos::task::spawn_local;
use std::sync::Arc;
use wasm_bindgen::JsCast;
use web_sys::{Element, HtmlDivElement, PointerEvent};

const DRAG_THRESHOLD: f64 = 3.0;

#[derive(Clone, Copy)]
struct CanvasGeometry {
    viewport_left: f64,
    viewport_top: f64,
    viewport_width: f64,
    viewport_height: f64,
    canvas_width: f64,
    canvas_height: f64,
}

impl CanvasGeometry {
    fn measure(list: &HtmlDivElement) -> Self {
        let bounds = list.get_bounding_client_rect();
        Self {
            viewport_left: bounds.left(),
            viewport_top: bounds.top(),
            viewport_width: f64::from(list.client_width()),
            viewport_height: f64::from(list.client_height()),
            canvas_width: f64::from(list.scroll_width()),
            canvas_height: f64::from(list.scroll_height()),
        }
    }

    fn contains(self, event: &PointerEvent) -> bool {
        let x = f64::from(event.client_x()) - self.viewport_left;
        let y = f64::from(event.client_y()) - self.viewport_top;
        x >= 0.0 && y >= 0.0 && x < self.viewport_width && y < self.viewport_height
    }

    fn canvas_point(self, event: &PointerEvent, list: &HtmlDivElement) -> Point {
        Point {
            x: (f64::from(event.client_x()) - self.viewport_left + f64::from(list.scroll_left()))
                .clamp(0.0, self.canvas_width),
            y: (f64::from(event.client_y()) - self.viewport_top + f64::from(list.scroll_top()))
                .clamp(0.0, self.canvas_height),
        }
    }
}

#[derive(Clone, Copy)]
struct Point {
    x: f64,
    y: f64,
}

#[derive(Clone)]
struct DragState {
    pointer_id: i32,
    origin: Point,
    geometry: CanvasGeometry,
    baseline: Arc<SelectionSnapshot>,
    additive: bool,
    dragged: bool,
}

#[derive(Clone, Copy)]
pub(super) struct DragSelection {
    state: RwSignal<Option<DragState>>,
    pub rectangle: RwSignal<Option<DragSelectionRect>>,
    suppress_click: RwSignal<bool>,
}

impl DragSelection {
    pub fn new() -> Self {
        Self {
            state: RwSignal::new(None),
            rectangle: RwSignal::new(None),
            suppress_click: RwSignal::new(false),
        }
    }

    pub fn begin(
        self,
        event: &PointerEvent,
        viewport: ResultViewport,
        selection: ResultSelection,
    ) -> bool {
        if !can_begin_drag(event) {
            return false;
        }

        let Some(list) = viewport.list_ref.get() else {
            return false;
        };
        let geometry = CanvasGeometry::measure(&list);
        if !geometry.contains(event) {
            return false;
        }
        if let Err(error) = list.set_pointer_capture(event.pointer_id()) {
            diagnostics::warn_js("Unable to capture the selection pointer.", &error);
            return false;
        }

        let additive = event.ctrl_key();
        let baseline = Arc::new(selection.snapshot());
        let origin = geometry.canvas_point(event, &list);
        self.state.set(Some(DragState {
            pointer_id: event.pointer_id(),
            origin,
            geometry,
            baseline,
            additive,
            dragged: false,
        }));
        self.rectangle.set(None);
        self.suppress_click.set(false);
        if !additive {
            selection.clear();
        }

        if let Err(error) = list.focus() {
            diagnostics::warn_js("Unable to focus the result list.", &error);
        }
        event.prevent_default();
        true
    }

    pub fn update(
        self,
        event: &PointerEvent,
        total: u32,
        viewport: ResultViewport,
        selection: ResultSelection,
    ) {
        let Some(mut state) = self.state.get_untracked() else {
            return;
        };
        if state.pointer_id != event.pointer_id() {
            return;
        }
        let Some(list) = viewport.list_ref.get() else {
            return;
        };
        let current = state.geometry.canvas_point(event, &list);
        if !state.dragged && !passed_drag_threshold(state.origin, current) {
            return;
        }

        state.dragged = true;
        let rectangle =
            DragSelectionRect::between((state.origin.x, state.origin.y), (current.x, current.y));
        let layout = SelectionLayout {
            mode: viewport.mode.get_untracked(),
            columns: viewport.columns.get_untracked(),
            width: viewport.grid_width.get_untracked(),
        };
        let next = selection_in_rectangle(
            rectangle,
            total,
            layout,
            &state.baseline.indices,
            state.additive,
        );
        selection.replace_indices(next);
        self.rectangle.set(Some(rectangle));
        self.state.set(Some(state));
        event.prevent_default();
    }

    pub fn finish(
        self,
        event: &PointerEvent,
        total: u32,
        viewport: ResultViewport,
        selection: ResultSelection,
    ) {
        let Some(state) = self.active_state(event.pointer_id()) else {
            return;
        };

        self.update(event, total, viewport, selection);
        let completed = self
            .active_state(event.pointer_id())
            .expect("the drag remains active until it is committed");
        self.suppress_click
            .set(completed.dragged || completed.additive);
        self.clear_drag_state();
        if let Some(list) = viewport.list_ref.get() {
            let _ = list.release_pointer_capture(state.pointer_id);
        }
        self.clear_click_suppression_after_event();
    }

    pub fn cancel(
        self,
        event: &PointerEvent,
        viewport: ResultViewport,
        selection: ResultSelection,
    ) {
        let Some(state) = self.rollback(event.pointer_id(), selection) else {
            return;
        };
        if let Some(list) = viewport.list_ref.get() {
            let _ = list.release_pointer_capture(state.pointer_id);
        }
    }

    pub fn lost_pointer_capture(self, event: &PointerEvent, selection: ResultSelection) {
        self.rollback(event.pointer_id(), selection);
    }

    pub fn consume_suppressed_click(self) -> bool {
        let suppress = self.suppress_click.get_untracked();
        if suppress {
            self.suppress_click.set(false);
        }
        suppress
    }

    fn active_state(self, pointer_id: i32) -> Option<DragState> {
        self.state
            .get_untracked()
            .filter(|state| state.pointer_id == pointer_id)
    }

    fn rollback(self, pointer_id: i32, selection: ResultSelection) -> Option<DragState> {
        let state = self.active_state(pointer_id)?;
        self.clear_drag_state();
        self.suppress_click.set(false);
        selection.restore(&state.baseline);
        Some(state)
    }

    fn clear_drag_state(self) {
        self.state.set(None);
        self.rectangle.set(None);
    }

    fn clear_click_suppression_after_event(self) {
        let suppress_click = self.suppress_click;
        spawn_local(async move {
            TimeoutFuture::new(0).await;
            suppress_click.set(false);
        });
    }
}

fn can_begin_drag(event: &PointerEvent) -> bool {
    event.button() == 0
        && event.is_primary()
        && event.pointer_type() != "touch"
        && !event_target_is_result(event)
}

fn event_target_is_result(event: &PointerEvent) -> bool {
    event
        .target()
        .and_then(|target| target.dyn_into::<Element>().ok())
        .is_some_and(|element| {
            element
                .closest(".result-row:not(.skeleton-row), .icon-result:not(.skeleton-tile)")
                .ok()
                .flatten()
                .is_some()
        })
}

fn passed_drag_threshold(origin: Point, current: Point) -> bool {
    (current.x - origin.x).abs() >= DRAG_THRESHOLD || (current.y - origin.y).abs() >= DRAG_THRESHOLD
}
