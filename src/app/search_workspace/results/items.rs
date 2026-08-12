use super::drag_selection::DragSelection;
use super::view::{ResultActions, ResultsLayout};
use super::{
    file_size, modified_date, FileIcon, FileVisual, ResultViewport, SelectionModifiers, ViewMode,
};
use crate::app::search_workspace::search::SearchResults;
use crate::diagnostics;
use everything_core::SearchResult;
use leptos::prelude::*;
use web_sys::MouseEvent;

#[component]
pub(super) fn ResultsCanvas(
    results: SearchResults,
    actions: ResultActions,
    layout: ResultsLayout,
    drag_selection: DragSelection,
) -> impl IntoView {
    let viewport = layout.viewport;
    let total = results.total;

    view! {
        <div
            class="virtual-canvas min-h-full min-w-0"
            class:icon-virtual-canvas=move || viewport.mode.get().is_grid()
            role="presentation"
            style:height=move || format!("{}px", viewport.canvas_height(total.get()))
        >
            <For
                each=move || layout.visible_start.get()..layout.visible_end.get()
                key=|index| *index
                children=move |index| view! {
                    {move || render_result(index, results, actions, viewport)}
                }
            />

            <Show when=move || drag_selection.rectangle.get().is_some()>
                {move || drag_selection.rectangle.get().map(|rectangle| view! {
                    <div
                        class="drag-selection-rectangle"
                        style=rectangle.style()
                        aria-hidden="true"
                    ></div>
                })}
            </Show>
        </div>
    }
}

fn render_result(
    index: u32,
    results: SearchResults,
    actions: ResultActions,
    viewport: ResultViewport,
) -> AnyView {
    let mode = viewport.mode.get();
    let columns = viewport.columns.get();
    let _width = viewport.grid_width.get();

    match results.item_at(index) {
        Some(item) if mode == ViewMode::Details => details_result(index, &item, actions, viewport),
        Some(item) => icon_result(index, &item, mode, columns, actions, viewport),
        None => result_skeleton(index, mode, columns, viewport),
    }
}

fn details_result(
    index: u32,
    item: &SearchResult,
    actions: ResultActions,
    viewport: ResultViewport,
) -> AnyView {
    let ResultActions {
        selection, files, ..
    } = actions;
    let selected = selection.indices;
    let focused_index = selection.focused_index;
    let list_ref = viewport.list_ref;
    let item_for_double = item.clone();
    let item_for_context = item.clone();

    view! {
        <div
            class="result-row [&.focused]:shadow-[inset_0_0_0_1px_color-mix(in_srgb,var(--muted)_55%,transparent)]"
            data-full-path=item.full_path.clone()
            class:selected=move || selected.with(|selection| selection.contains(index))
            class:focused=move || focused_index.get() == Some(index)
            style=viewport.item_style(index)
            role="row"
            aria-rowindex=index + 1
            aria-selected=move || selected.with(|selection| selection.contains(index))
            on:click=move |event: MouseEvent| select_result(&event, index, actions, list_ref)
            on:dblclick=move |_| files.open(item_for_double.full_path.clone())
            on:contextmenu=move |event: MouseEvent| {
                open_context_menu(&event, index, item_for_context.clone(), actions);
            }
        >
            <div class="cell col-name" role="gridcell">
                <FileIcon path=item.full_path.clone() is_dir=item.is_dir />
                <span class="file-name" title=item.name.clone()>{item.name.clone()}</span>
            </div>
            <div class="cell col-path" role="gridcell" title=item.parent_path.clone()>{item.parent_path.clone()}</div>
            <div class="cell col-type" role="gridcell">{file_type(&item.name, item.is_dir)}</div>
            <div class="cell col-size" role="gridcell">{file_size(item.size, item.is_dir)}</div>
            <div class="cell col-date" role="gridcell">{modified_date(item.modified_unix)}</div>
        </div>
    }
    .into_any()
}

fn icon_result(
    index: u32,
    item: &SearchResult,
    mode: ViewMode,
    columns: u32,
    actions: ResultActions,
    viewport: ResultViewport,
) -> AnyView {
    let ResultActions {
        selection, files, ..
    } = actions;
    let selected = selection.indices;
    let focused_index = selection.focused_index;
    let list_ref = viewport.list_ref;
    let item_for_double = item.clone();
    let item_for_context = item.clone();
    let metadata = icon_metadata(item, mode);
    let title = item_title(item, "\n");
    let aria_label = item_title(item, ", ");

    view! {
        <div
            class=format!(
                "icon-result icon-result-{} {} absolute flex min-w-0 overflow-hidden rounded-md border border-transparent bg-transparent will-change-transform hover:bg-[var(--hover)] [&.focused]:shadow-[inset_0_0_0_1px_color-mix(in_srgb,var(--muted)_55%,transparent)] [&.selected]:border-[var(--selected-border)] [&.selected]:bg-[var(--selected)] [&_.file-icon]:size-[var(--view-icon-size)] [&_.file-icon_img]:size-[var(--view-icon-size)] [&_.file-icon_img]:object-contain",
                mode.key(),
                mode.utility_classes(),
            )
            class:selected=move || selected.with(|selection| selection.contains(index))
            class:focused=move || focused_index.get() == Some(index)
            style=viewport.item_style(index)
            role="gridcell"
            aria-rowindex=index / columns + 1
            aria-colindex=index % columns + 1
            aria-selected=move || selected.with(|selection| selection.contains(index))
            aria-label=aria_label
            title=title
            on:click=move |event: MouseEvent| select_result(&event, index, actions, list_ref)
            on:dblclick=move |_| files.open(item_for_double.full_path.clone())
            on:contextmenu=move |event: MouseEvent| {
                open_context_menu(&event, index, item_for_context.clone(), actions);
            }
        >
            {match mode.visual_size() {
                Some(visual_size) => view! {
                    <FileVisual
                        path=item.full_path.clone()
                        is_dir=item.is_dir
                        visual_size
                        file_size=item.size
                        modified_unix=item.modified_unix
                        load=true
                    />
                }.into_any(),
                None => view! {
                    <span class="icon-result-visual grid shrink-0 place-items-center">
                        <FileIcon path=item.full_path.clone() is_dir=item.is_dir />
                    </span>
                }.into_any(),
            }}
            <div class="icon-result-text flex min-w-0 flex-col gap-0.5 overflow-hidden">
                <span class="icon-result-name max-w-full overflow-hidden text-ellipsis text-[var(--text)]">{item.name.clone()}</span>
                <span class="icon-result-metadata block max-w-full overflow-hidden text-ellipsis whitespace-nowrap text-[11px] text-[var(--muted)]">{metadata}</span>
            </div>
        </div>
    }
    .into_any()
}

fn result_skeleton(index: u32, mode: ViewMode, columns: u32, viewport: ResultViewport) -> AnyView {
    if mode == ViewMode::Details {
        return view! {
            <div class="result-row skeleton-row" style=viewport.item_style(index) role="row" aria-rowindex=index + 1>
                <div class="cell col-name" role="gridcell"><span class="skeleton icon-skeleton"></span><span class="skeleton text-skeleton"></span></div>
                <div class="cell col-path" role="gridcell"><span class="skeleton path-skeleton"></span></div>
                <div class="cell col-type" role="gridcell"><span class="skeleton type-skeleton"></span></div>
                <div class="cell col-size" role="gridcell"><span class="skeleton size-skeleton"></span></div>
                <div class="cell col-date" role="gridcell"><span class="skeleton date-skeleton"></span></div>
            </div>
        }
        .into_any();
    }

    view! {
        <div
            class=format!(
                "icon-result icon-result-{} skeleton-tile {} pointer-events-none absolute flex min-w-0 overflow-hidden rounded-md border border-transparent bg-transparent will-change-transform [&_.file-icon]:size-[var(--view-icon-size)] [&_.file-icon_img]:size-[var(--view-icon-size)] [&_.file-icon_img]:object-contain",
                mode.key(),
                mode.utility_classes(),
            )
            style=viewport.item_style(index)
            role="gridcell"
            aria-rowindex=index / columns + 1
            aria-colindex=index % columns + 1
        >
            <span class="icon-tile-skeleton icon-tile-skeleton-icon block size-[var(--view-icon-size,48px)] shrink-0 animate-[shimmer_1.4s_linear_infinite] rounded-lg bg-[linear-gradient(90deg,var(--hover),color-mix(in_srgb,var(--hover)_45%,transparent),var(--hover))] bg-[length:200%_100%]"></span>
            <span class="icon-tile-skeleton icon-tile-skeleton-label block h-[9px] w-[min(140px,70%)] rounded-full"></span>
        </div>
    }
    .into_any()
}

fn select_result(
    event: &MouseEvent,
    index: u32,
    actions: ResultActions,
    list_ref: NodeRef<leptos::html::Div>,
) {
    event.stop_propagation();
    actions
        .selection
        .select_row(index, selection_modifiers(event));
    if let Some(list) = list_ref.get() {
        if let Err(error) = list.focus() {
            diagnostics::warn_js("Unable to focus the result list.", &error);
        }
    }
    actions.menu.close();
}

fn open_context_menu(event: &MouseEvent, index: u32, item: SearchResult, actions: ResultActions) {
    event.prevent_default();
    event.stop_propagation();
    actions.menu.open_at_pointer(
        index,
        item,
        event.client_x(),
        event.client_y(),
        actions.selection,
    );
}

fn icon_metadata(item: &SearchResult, mode: ViewMode) -> String {
    if mode == ViewMode::Small {
        return item.parent_path.clone();
    }
    [
        file_size(item.size, item.is_dir),
        modified_date(item.modified_unix),
    ]
    .into_iter()
    .filter(|value| !value.is_empty())
    .collect::<Vec<_>>()
    .join(" · ")
}

fn item_title(item: &SearchResult, separator: &str) -> String {
    if item.parent_path.is_empty() {
        item.name.clone()
    } else {
        format!("{}{separator}{}", item.name, item.parent_path)
    }
}

fn file_type(name: &str, is_dir: bool) -> String {
    if is_dir {
        return "Folder".into();
    }

    name.rsplit_once('.')
        .filter(|(stem, extension)| !stem.is_empty() && !extension.is_empty())
        .map_or_else(|| "File".into(), |(_, extension)| extension.to_uppercase())
}

fn selection_modifiers(event: &MouseEvent) -> SelectionModifiers {
    SelectionModifiers {
        extend: event.shift_key(),
        preserve: event.ctrl_key(),
    }
}
