use super::{
    file_size, modified_date, result_count, ColumnHeaders, FileIcon, FileVisual, ResultColumns,
    ResultContextMenu, ResultSelection, ResultViewport, SelectionModifiers, ViewMode,
};
use crate::app::icons;
use crate::app::search_workspace::file_actions::FileOperations;
use crate::app::search_workspace::search::SearchResults;
use crate::diagnostics;
use everything_core::IndexSelection;
use leptos::prelude::*;
use web_sys::MouseEvent;

#[derive(Clone, Copy)]
pub(in crate::app::search_workspace) struct ResultsViewContext {
    pub results: SearchResults,
    pub selection: ResultSelection,
    pub files: FileOperations,
    pub menu: ResultContextMenu,
    pub columns: ResultColumns,
    pub viewport: ResultViewport,
    pub visible_start: Memo<u32>,
    pub visible_end: Memo<u32>,
    pub engine_available: RwSignal<bool>,
    pub engine_message: RwSignal<String>,
}

#[component]
pub(in crate::app::search_workspace) fn ResultsView(context: ResultsViewContext) -> impl IntoView {
    let ResultsViewContext {
        results,
        selection,
        files,
        menu,
        columns,
        viewport,
        visible_start,
        visible_end,
        engine_available,
        engine_message,
    } = context;
    let SearchResults {
        query,
        total,
        sort,
        loading,
        error: search_error,
        ..
    } = results;
    let selected = selection.indices;
    let focused_index = selection.focused_index;
    let error = files.error;
    let list_ref = viewport.list_ref;
    let on_scroll = move |event: web_sys::Event| {
        viewport.update_from_scroll_event(event);
        menu.close();
    };

    view! {
        <section
            class="results-panel relative [&.icon-view]:grid-rows-[minmax(0,1fr)_24px] [&.icon-view_.column-header]:hidden [&.icon-view_.results-scroll]:row-start-1 [&.icon-view_.statusbar]:row-start-2"
            class:icon-view=move || viewport.mode.get().is_grid()
            class:resizing-columns=move || columns.is_resizing()
            data-view-mode=move || viewport.mode.get().key()
            style=move || columns.layout_style(viewport.grid_width.get())
        >
            <ColumnHeaders columns sort />

            <div
                class="results-scroll focus-visible:shadow-none"
                node_ref=list_ref
                tabindex="0"
                role="grid"
                aria-label=move || if viewport.mode.get().is_grid() {
                    "Search results in icon view"
                } else {
                    "Search results"
                }
                aria-colcount=move || if viewport.mode.get().is_grid() {
                    viewport.columns.get()
                } else {
                    5
                }
                aria-rowcount=move || viewport.row_count(total.get())
                on:scroll=on_scroll
                on:click=move |_| {
                    selection.clear();
                    menu.close();
                }
            >
                <div
                    class="virtual-canvas min-h-full min-w-0"
                    class:icon-virtual-canvas=move || viewport.mode.get().is_grid()
                    role="presentation"
                    style:height=move || format!("{}px", viewport.canvas_height(total.get()))
                >
                    <For
                        each=move || visible_start.get()..visible_end.get()
                        key=|index| *index
                        children=move |index| view! {
                            {move || {
                                let mode = viewport.mode.get();
                                let columns = viewport.columns.get();
                                let _width = viewport.grid_width.get();
                                let maybe_item = results.item_at(index);
                                match maybe_item {
                                    Some(item) => {
                                        let item_for_double = item.clone();
                                        let item_for_context = item.clone();
                                        let item_style = viewport.item_style(index);
                                        if mode == ViewMode::Details {
                                            view! {
                                                <div
                                                    class="result-row [&.focused]:shadow-[inset_0_0_0_1px_color-mix(in_srgb,var(--muted)_55%,transparent)]"
                                                    data-full-path=item.full_path.clone()
                                                    class:selected=move || selected.with(|selection| selection.contains(index))
                                                    class:focused=move || focused_index.get() == Some(index)
                                                    style=item_style
                                                    role="row"
                                                    aria-rowindex=index + 1
                                                    aria-selected=move || selected.with(|selection| selection.contains(index))
                                                    on:click=move |event: MouseEvent| {
                                                        event.stop_propagation();
                                                        selection.select_row(index, selection_modifiers(&event));
                                                        if let Some(list) = list_ref.get() {
                                                            if let Err(error) = list.focus() {
                                                                diagnostics::warn_js("Unable to focus the result list.", &error);
                                                            }
                                                        }
                                                        menu.close();
                                                    }
                                                    on:dblclick=move |_| files.open(item_for_double.full_path.clone())
                                                    on:contextmenu=move |event: MouseEvent| {
                                                        event.prevent_default();
                                                        event.stop_propagation();
                                                        menu.open_at_pointer(
                                                            index,
                                                            item_for_context.clone(),
                                                            event.client_x(),
                                                            event.client_y(),
                                                            selection,
                                                        );
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
                                            }.into_any()
                                        } else {
                                            let metadata = if mode == ViewMode::Small {
                                                item.parent_path.clone()
                                            } else {
                                                [
                                                    file_size(item.size, item.is_dir),
                                                    modified_date(item.modified_unix),
                                                ]
                                                    .into_iter()
                                                    .filter(|value| !value.is_empty())
                                                    .collect::<Vec<_>>()
                                                    .join(" · ")
                                            };
                                            let title = if item.parent_path.is_empty() {
                                                item.name.clone()
                                            } else {
                                                format!("{}\n{}", item.name, item.parent_path)
                                            };
                                            let aria_label = if item.parent_path.is_empty() {
                                                item.name.clone()
                                            } else {
                                                format!("{}, {}", item.name, item.parent_path)
                                            };
                                            view! {
                                                <div
                                                    class=format!(
                                                        "icon-result icon-result-{} {} absolute flex min-w-0 overflow-hidden rounded-md border border-transparent bg-transparent will-change-transform hover:bg-[var(--hover)] [&.focused]:shadow-[inset_0_0_0_1px_color-mix(in_srgb,var(--muted)_55%,transparent)] [&.selected]:border-[var(--selected-border)] [&.selected]:bg-[var(--selected)] [&_.file-icon]:size-[var(--view-icon-size)] [&_.file-icon_img]:size-[var(--view-icon-size)] [&_.file-icon_img]:object-contain",
                                                        mode.key(),
                                                        mode.utility_classes(),
                                                    )
                                                    class:selected=move || selected.with(|selection| selection.contains(index))
                                                    class:focused=move || focused_index.get() == Some(index)
                                                    style=item_style
                                                    role="gridcell"
                                                    aria-rowindex=index / columns + 1
                                                    aria-colindex=index % columns + 1
                                                    aria-selected=move || selected.with(|selection| selection.contains(index))
                                                    aria-label=aria_label
                                                    title=title
                                                    on:click=move |event: MouseEvent| {
                                                        event.stop_propagation();
                                                        selection.select_row(index, selection_modifiers(&event));
                                                        if let Some(list) = list_ref.get() {
                                                            if let Err(error) = list.focus() {
                                                                diagnostics::warn_js("Unable to focus the result list.", &error);
                                                            }
                                                        }
                                                        menu.close();
                                                    }
                                                    on:dblclick=move |_| files.open(item_for_double.full_path.clone())
                                                    on:contextmenu=move |event: MouseEvent| {
                                                        event.prevent_default();
                                                        event.stop_propagation();
                                                        menu.open_at_pointer(
                                                            index,
                                                            item_for_context.clone(),
                                                            event.client_x(),
                                                            event.client_y(),
                                                            selection,
                                                        );
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
                                            }.into_any()
                                        }
                                    }
                                    None if mode == ViewMode::Details => view! {
                                            <div class="result-row skeleton-row" style=viewport.item_style(index) role="row" aria-rowindex=index + 1>
                                                <div class="cell col-name" role="gridcell"><span class="skeleton icon-skeleton"></span><span class="skeleton text-skeleton"></span></div>
                                                <div class="cell col-path" role="gridcell"><span class="skeleton path-skeleton"></span></div>
                                                <div class="cell col-type" role="gridcell"><span class="skeleton type-skeleton"></span></div>
                                                <div class="cell col-size" role="gridcell"><span class="skeleton size-skeleton"></span></div>
                                                <div class="cell col-date" role="gridcell"><span class="skeleton date-skeleton"></span></div>
                                            </div>
                                        }.into_any(),
                                    None => view! {
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
                                        }.into_any(),
                                }
                            }}
                        }
                    />
                </div>

                <Show when=move || query.get().trim().is_empty()>
                    <EmptyState
                        icon=icons::search()
                        title="Start typing"
                        message="All Everything search functions and operators are supported."
                    />
                </Show>
                <Show when=move || !query.get().trim().is_empty() && !loading.get() && total.get() == 0 && search_error.get().is_none()>
                    <EmptyState
                        icon=icons::empty()
                        title="No results"
                        message="Try a less restrictive search or check the syntax."
                    />
                </Show>
                <Show when=move || search_error.get().is_some()>
                    {move || search_error.get().map(|message| view! {
                        <div class="error-banner absolute left-1/2 top-6 flex w-[min(540px,calc(100%_-_48px))] -translate-x-1/2 gap-3 rounded-[9px] border border-[color-mix(in_srgb,var(--danger)_38%,var(--border))] bg-[color-mix(in_srgb,var(--surface-solid)_92%,var(--danger))] px-[15px] py-[13px] text-[var(--danger)] shadow-[var(--shadow)] [&_span]:text-xs [&_span]:text-[var(--text)]" role="alert">
                            <span>{message}</span>
                            <button class="banner-close focus-visible:bg-[var(--hover)]" title="Close" aria-label="Close" on:click=move |_| search_error.set(None)>{icons::close()}</button>
                        </div>
                    })}
                </Show>

                <Show when=move || error.get().is_some()>
                    <div class="error-banner absolute left-1/2 top-6 flex w-[min(540px,calc(100%_-_48px))] -translate-x-1/2 gap-3 rounded-[9px] border border-[color-mix(in_srgb,var(--danger)_38%,var(--border))] bg-[color-mix(in_srgb,var(--surface-solid)_92%,var(--danger))] px-[15px] py-[13px] text-[var(--danger)] shadow-[var(--shadow)] [&>div]:grid [&>div]:gap-[3px] [&_span]:text-xs [&_span]:text-[var(--text)]" role="alert">
                        {icons::warning()}
                        <div><strong>"An operation failed"</strong><span>{move || error.get().unwrap_or_default()}</span></div>
                        <button class="banner-close focus-visible:bg-[var(--hover)]" title="Close" aria-label="Close" on:click=move |_| error.set(None)>{icons::close()}</button>
                    </div>
                </Show>
            </div>

            <footer class="statusbar flex items-center gap-2 border-t border-[var(--border)] bg-[var(--surface-2)] px-2.5 text-xs text-[var(--muted)]">
                <span>{move || result_count(total.get())}</span>
                <span class="status-separator h-3 w-px bg-[var(--border)]"></span>
                <span>{move || format!("{} selected", selected.with(IndexSelection::count))}</span>
                <Show when=move || !engine_available.get()>
                    <span class="status-separator h-3 w-px bg-[var(--border)]"></span>
                    <span class="connection-warning" title=move || engine_message.get()>"Indexing..."</span>
                </Show>
                <span class="statusbar-spacer flex-1"></span>
                <Show when=move || loading.get()><span class="loading-indicator"></span><span>"Searching…"</span></Show>
            </footer>
        </section>
    }
}

#[component]
pub(in crate::app::search_workspace) fn ResultContextMenuView(
    context: ResultsViewContext,
) -> impl IntoView {
    let ResultsViewContext {
        results,
        selection,
        files,
        menu,
        ..
    } = context;
    let context_menu = menu.state;

    view! {
        <Show when=move || context_menu.get().is_some()>
            {move || context_menu.get().map(|menu| view! {
                <div class="context-menu fixed z-[100] w-[260px] rounded-[9px] border border-[var(--border)] bg-[color-mix(in_srgb,var(--surface-solid)_94%,transparent)] p-[5px] shadow-[var(--shadow)] backdrop-blur-[28px] backdrop-saturate-[1.3]" style:left=format!("{}px", menu.x) style:top=format!("{}px", menu.y) on:click=move |event| event.stop_propagation()>
                    <ContextAction icon=icons::open() label="Open" shortcut="Enter" on_click={
                        let path = menu.item.full_path.clone();
                        move || { files.open(path.clone()); context_menu.set(None); }
                    } />
                    <ContextAction icon=icons::folder_open() label="Show in Explorer" shortcut="" on_click={
                        let path = menu.item.full_path.clone();
                        move || { files.reveal(path.clone()); context_menu.set(None); }
                    } />
                    <div class="context-separator mx-[7px] my-1 h-px bg-[var(--border)]"></div>
                    <ContextAction icon=icons::copy() label="Copy name" shortcut="" on_click={
                        let name = menu.item.name.clone();
                        move || { files.copy(name.clone()); context_menu.set(None); }
                    } />
                    <ContextAction icon=icons::copy() label="Copy path" shortcut="" on_click={
                        let path = menu.item.parent_path.clone();
                        move || { files.copy(path.clone()); context_menu.set(None); }
                    } />
                    <ContextAction icon=icons::copy() label="Copy full path" shortcut="" on_click={
                        let path = menu.item.full_path.clone();
                        move || { files.copy(path.clone()); context_menu.set(None); }
                    } />
                    <ContextAction icon=icons::edit() label="Rename" shortcut="F2" on_click={
                        let item = menu.item.clone();
                        move || { files.begin_rename(item.clone()); context_menu.set(None); }
                    } />
                    <div class="context-separator mx-[7px] my-1 h-px bg-[var(--border)]"></div>
                    <ContextAction danger=true icon=icons::trash() label="Move to Recycle Bin" shortcut="Del" on_click=move || { files.begin_trash(selection, results); context_menu.set(None); } />
                </div>
            })}
        </Show>
    }
}

#[component]
fn EmptyState(icon: AnyView, title: &'static str, message: &'static str) -> impl IntoView {
    view! {
        <div class="empty-state pointer-events-none absolute inset-0 grid place-content-center justify-items-center p-10 text-center text-[var(--muted)] [&>.native-icon]:mb-3 [&>.native-icon]:h-[46px] [&>.native-icon]:w-[46px] [&>.native-icon]:text-[42px] [&>.native-icon]:text-[color-mix(in_srgb,var(--muted)_70%,transparent)] [&>h2]:mb-[5px] [&>h2]:text-[17px] [&>h2]:font-semibold [&>h2]:text-[var(--text)] [&>p]:max-w-[430px] [&>p]:leading-[1.5]">
            {icon}<h2>{title}</h2><p>{message}</p>
        </div>
    }
}

#[component]
fn ContextAction<F>(
    icon: AnyView,
    label: &'static str,
    shortcut: &'static str,
    #[prop(default = false)] danger: bool,
    on_click: F,
) -> impl IntoView
where
    F: Fn() + Send + Sync + 'static,
{
    view! {
        <button
            class="context-action grid h-[34px] w-full grid-cols-[22px_1fr_auto] items-center gap-2 rounded-[5px] bg-transparent px-2 text-left hover:bg-[var(--hover)] focus-visible:bg-[var(--hover)] [&>kbd]:border-0 [&>kbd]:bg-transparent"
            class:danger=danger
            class=("text-[var(--danger)]", danger)
            on:click=move |_| on_click()
        >
            {icon}<span>{label}</span><kbd>{shortcut}</kbd>
        </button>
    }
}

fn file_type(name: &str, is_dir: bool) -> String {
    if is_dir {
        return "Folder".into();
    }

    name.rsplit_once('.')
        .filter(|(stem, extension)| !stem.is_empty() && !extension.is_empty())
        .map(|(_, extension)| extension.to_uppercase())
        .unwrap_or_else(|| "File".into())
}

fn selection_modifiers(event: &MouseEvent) -> SelectionModifiers {
    SelectionModifiers {
        extend: event.shift_key(),
        preserve: event.ctrl_key(),
    }
}
