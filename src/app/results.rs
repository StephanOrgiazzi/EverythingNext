use super::search::{RESULT_ROW_HEIGHT, VIRTUALIZATION_OVERSCAN};
use crate::backend;
use gloo_timers::future::TimeoutFuture;
use leptos::prelude::*;
use leptos::task::spawn_local;
use wasm_bindgen::JsCast;
use web_sys::HtmlDivElement;

#[derive(Clone, Copy)]
pub(super) struct ResultViewport {
    pub list_ref: NodeRef<leptos::html::Div>,
    pub scroll_top: RwSignal<f64>,
    pub height: RwSignal<f64>,
    pub grid_width: RwSignal<f64>,
}

impl ResultViewport {
    pub fn new() -> Self {
        Self {
            list_ref: NodeRef::new(),
            scroll_top: RwSignal::new(0.0),
            height: RwSignal::new(640.0),
            grid_width: RwSignal::new(0.0),
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
            ((self.scroll_top.get() / RESULT_ROW_HEIGHT).floor() as u32)
                .saturating_sub(VIRTUALIZATION_OVERSCAN)
        })
    }

    pub fn visible_end(self, start: Memo<u32>, total: RwSignal<u32>) -> Memo<u32> {
        Memo::new(move |_| {
            let count =
                (self.height.get() / RESULT_ROW_HEIGHT).ceil() as u32 + VIRTUALIZATION_OVERSCAN * 2;
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
        self.scroll_top.set(element.scroll_top() as f64);
        self.height.set(element.client_height() as f64);
    }

    pub fn reset_scroll(self) {
        self.scroll_top.set(0.0);
        if let Some(list) = self.list_ref.get() {
            list.set_scroll_top(0);
            self.height.set(list.client_height() as f64);
        }
    }

    pub fn scroll_row_into_view(self, index: u32) {
        let Some(list) = self.list_ref.get() else {
            return;
        };
        let top = index as f64 * RESULT_ROW_HEIGHT;
        let bottom = top + RESULT_ROW_HEIGHT;
        let current_top = list.scroll_top() as f64;
        let current_bottom = current_top + list.client_height() as f64;
        if top < current_top {
            list.set_scroll_top(top as i32);
        } else if bottom > current_bottom {
            list.set_scroll_top((bottom - list.client_height() as f64) as i32);
        }
    }

    fn measure_dimensions(self) {
        let Some(list) = self.list_ref.get() else {
            return;
        };
        let height = list.client_height() as f64;
        if (height - self.height.get_untracked()).abs() > 0.5 {
            self.height.set(height);
        }
        let width = list
            .query_selector(".virtual-canvas")
            .ok()
            .flatten()
            .map(|canvas| canvas.get_bounding_client_rect().width())
            .unwrap_or_else(|| list.client_width() as f64);
        if (width - self.grid_width.get_untracked()).abs() > 0.5 {
            self.grid_width.set(width);
        }
    }
}

#[component]
pub(super) fn FileIcon(path: String) -> impl IntoView {
    let source = RwSignal::new(None::<String>);
    Effect::new(move |_| {
        let path = path.clone();
        spawn_local(async move {
            source.set(backend::icon(&path).await);
        });
    });

    view! {
        <span class="file-icon">
            {move || source.get().map(|source| view! { <img src=source alt="" /> })}
        </span>
    }
}
