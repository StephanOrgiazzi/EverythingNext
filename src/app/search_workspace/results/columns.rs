use crate::diagnostics;
use everything_core::{SortColumn, SortDirection, SortSpec};
use leptos::prelude::*;
use wasm_bindgen::JsCast;
use web_sys::{Element, HtmlDivElement, PointerEvent};

#[derive(Clone, Copy)]
struct ColumnWidths {
    name: f64,
    path: f64,
    file_type: f64,
    size: f64,
    date: f64,
}

#[derive(Clone, Copy)]
enum ColumnBoundary {
    NamePath,
    PathType,
    TypeSize,
    SizeDate,
}

#[derive(Clone, Copy)]
struct ColumnResize {
    boundary: ColumnBoundary,
    pointer_id: i32,
    start_x: f64,
    start_left: f64,
    start_right: f64,
    total_width: f64,
}

#[derive(Clone, Copy)]
pub(crate) struct ResultColumns {
    widths: RwSignal<Option<ColumnWidths>>,
    resize: RwSignal<Option<ColumnResize>>,
    header_ref: NodeRef<leptos::html::Div>,
}

impl ResultColumns {
    pub fn new() -> Self {
        Self {
            widths: RwSignal::new(None),
            resize: RwSignal::new(None),
            header_ref: NodeRef::new(),
        }
    }

    pub fn is_resizing(self) -> bool {
        self.resize.get().is_some()
    }

    pub fn layout_style(self, grid_width: f64) -> String {
        let mut style = if grid_width > 0.0 {
            format!("--grid-width:{grid_width:.2}px")
        } else {
            String::new()
        };
        if let Some(widths) = self.widths.get() {
            style.push_str(&format!(
                ";--col-name:{:.4}%;--col-path:{:.4}%;--col-type:{:.4}%;--col-size:{:.4}%;--col-date:{:.4}%",
                widths.name, widths.path, widths.file_type, widths.size, widths.date
            ));
        }
        style
    }

    pub fn update_resize(self, event: PointerEvent) {
        let Some(active) = self.resize.get_untracked() else {
            return;
        };
        if active.pointer_id != event.pointer_id() {
            return;
        }
        event.prevent_default();

        let (minimum_left, minimum_right) = match active.boundary {
            ColumnBoundary::NamePath => (180.0, 180.0),
            ColumnBoundary::PathType => (180.0, 80.0),
            ColumnBoundary::TypeSize => (80.0, 76.0),
            ColumnBoundary::SizeDate => (76.0, 130.0),
        };
        let pair_width = active.start_left + active.start_right;
        if pair_width <= minimum_left + minimum_right {
            return;
        }

        let delta = f64::from(event.client_x()) - active.start_x;
        let next_left = (active.start_left + delta).clamp(minimum_left, pair_width - minimum_right);
        let next_right = pair_width - next_left;
        let left_percent = next_left / active.total_width * 100.0;
        let right_percent = next_right / active.total_width * 100.0;

        self.widths.update(|current| {
            let Some(current) = current else {
                return;
            };
            match active.boundary {
                ColumnBoundary::NamePath => {
                    current.name = left_percent;
                    current.path = right_percent;
                }
                ColumnBoundary::PathType => {
                    current.path = left_percent;
                    current.file_type = right_percent;
                }
                ColumnBoundary::TypeSize => {
                    current.file_type = left_percent;
                    current.size = right_percent;
                }
                ColumnBoundary::SizeDate => {
                    current.size = left_percent;
                    current.date = right_percent;
                }
            }
        });
    }

    pub fn finish_resize(self, event: PointerEvent) {
        let Some(active) = self.resize.get_untracked() else {
            return;
        };
        if active.pointer_id != event.pointer_id() {
            return;
        }
        event.prevent_default();
        event.stop_propagation();
        self.resize.set(None);
    }

    fn begin_resize(self, event: PointerEvent, boundary: ColumnBoundary) {
        event.prevent_default();
        event.stop_propagation();

        let Some(header) = self.header_ref.get() else {
            return;
        };
        let Some(measured) = measure_column_widths(&header) else {
            return;
        };
        let total_width =
            measured.name + measured.path + measured.file_type + measured.size + measured.date;
        if total_width <= 0.0 {
            return;
        }

        self.widths.set(Some(ColumnWidths {
            name: measured.name / total_width * 100.0,
            path: measured.path / total_width * 100.0,
            file_type: measured.file_type / total_width * 100.0,
            size: measured.size / total_width * 100.0,
            date: measured.date / total_width * 100.0,
        }));

        let (start_left, start_right) = match boundary {
            ColumnBoundary::NamePath => (measured.name, measured.path),
            ColumnBoundary::PathType => (measured.path, measured.file_type),
            ColumnBoundary::TypeSize => (measured.file_type, measured.size),
            ColumnBoundary::SizeDate => (measured.size, measured.date),
        };
        self.resize.set(Some(ColumnResize {
            boundary,
            pointer_id: event.pointer_id(),
            start_x: f64::from(event.client_x()),
            start_left,
            start_right,
            total_width,
        }));

        if let Some(element) = event
            .current_target()
            .and_then(|target| target.dyn_into::<Element>().ok())
        {
            if let Err(error) = element.set_pointer_capture(event.pointer_id()) {
                diagnostics::warn_js("Unable to capture the column-resize pointer.", &error);
            }
        }
    }
}

#[component]
pub(crate) fn ColumnHeaders(columns: ResultColumns, sort: RwSignal<SortSpec>) -> impl IntoView {
    view! {
        <div class="column-header grid w-[var(--grid-width,100%)] border-b border-[var(--border)] bg-[var(--surface-2)]" node_ref=columns.header_ref>
            <div class="column-heading col-name relative min-w-0 border-r border-[var(--border-soft)]">
                <SortHeader label="Name" column=SortColumn::Name sort />
                <ColumnResizer boundary=ColumnBoundary::NamePath columns />
            </div>
            <div class="column-heading col-path relative min-w-0 border-r border-[var(--border-soft)]">
                <SortHeader label="Path" column=SortColumn::Path sort />
                <ColumnResizer boundary=ColumnBoundary::PathType columns />
            </div>
            <div class="column-heading col-type relative min-w-0 border-r border-[var(--border-soft)]">
                <SortHeader label="Type" column=SortColumn::Extension sort />
                <ColumnResizer boundary=ColumnBoundary::TypeSize columns />
            </div>
            <div class="column-heading col-size relative min-w-0 border-r border-[var(--border-soft)]">
                <SortHeader label="Size" column=SortColumn::Size sort />
                <ColumnResizer boundary=ColumnBoundary::SizeDate columns />
            </div>
            <div class="column-heading col-date relative min-w-0 border-r border-[var(--border-soft)]">
                <SortHeader label="Date modified" column=SortColumn::Modified sort />
            </div>
        </div>
    }
}

#[component]
fn SortHeader(label: &'static str, column: SortColumn, sort: RwSignal<SortSpec>) -> impl IntoView {
    view! {
        <button class="column-button flex size-full min-w-0 items-center gap-[5px] bg-transparent px-[10px] text-left text-xs text-[var(--muted)] hover:bg-[var(--hover)] hover:text-[var(--text)] focus-visible:bg-[var(--hover)]" on:click=move |_| {
            sort.update(|current| {
                if current.column == column {
                    current.direction = current.direction.toggle();
                } else {
                    current.column = column;
                    current.direction = SortDirection::Ascending;
                }
            });
        }>
            <span>{label}</span>
            <span class="sort-arrow text-[var(--accent)] opacity-0 [&.visible]:opacity-100" class:visible=move || sort.get().column == column>
                {move || if sort.get().direction == SortDirection::Ascending { "↑" } else { "↓" }}
            </span>
        </button>
    }
}

#[component]
fn ColumnResizer(boundary: ColumnBoundary, columns: ResultColumns) -> impl IntoView {
    view! {
        <span
            class="column-resizer"
            role="separator"
            aria-orientation="vertical"
            on:pointerdown=move |event| columns.begin_resize(event, boundary)
            on:click=move |event| event.stop_propagation()
        ></span>
    }
}

fn measure_column_widths(header: &HtmlDivElement) -> Option<ColumnWidths> {
    Some(ColumnWidths {
        name: measure_column(header, ".column-heading.col-name")?,
        path: measure_column(header, ".column-heading.col-path")?,
        file_type: measure_column(header, ".column-heading.col-type")?,
        size: measure_column(header, ".column-heading.col-size")?,
        date: measure_column(header, ".column-heading.col-date")?,
    })
}

fn measure_column(header: &HtmlDivElement, selector: &str) -> Option<f64> {
    match header.query_selector(selector) {
        Ok(element) => element.map(|element| element.get_bounding_client_rect().width()),
        Err(error) => {
            diagnostics::warn_js(
                &format!("Unable to measure result column '{selector}'."),
                &error,
            );
            None
        }
    }
}
