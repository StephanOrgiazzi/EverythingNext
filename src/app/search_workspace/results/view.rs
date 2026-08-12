use super::drag_selection::DragSelection;
use super::items::ResultsCanvas;
use super::{
    result_count, ColumnHeaders, ResultColumns, ResultContextMenu, ResultSelection, ResultViewport,
    ViewMode,
};
use crate::app::icons;
use crate::app::search_workspace::file_actions::FileOperations;
use crate::app::search_workspace::search::SearchResults;
use everything_core::IndexSelection;
use gloo_timers::callback::Interval;
use leptos::prelude::*;
use web_sys::PointerEvent;

const SEARCH_TIPS: &[&str] = &[
    "Try ext:pdf to find PDF files.",
    "Use size:>10mb to find files larger than 10 MB.",
    "Find files from today or yesterday with date-modified:today | date-modified:yesterday.",
    "Use | for OR, like report | invoice.",
    "Prefix a term with ! to exclude it, like !backup.",
    "Use < > to group terms, like <report | invoice> ext:pdf.",
    "Put text in quotes to match it literally, like \"project plan\".",
];

#[derive(Clone, Copy)]
pub(in crate::app::search_workspace) struct ResultActions {
    pub selection: ResultSelection,
    pub files: FileOperations,
    pub menu: ResultContextMenu,
}

#[derive(Clone, Copy)]
pub(in crate::app::search_workspace) struct ResultsLayout {
    pub columns: ResultColumns,
    pub viewport: ResultViewport,
    pub visible_start: Memo<u32>,
    pub visible_end: Memo<u32>,
}

#[derive(Clone, Copy)]
pub(in crate::app::search_workspace) struct EngineStatusSignals {
    pub available: RwSignal<bool>,
    pub indexing: RwSignal<bool>,
    pub ready_volumes: RwSignal<u32>,
    pub total_volumes: RwSignal<u32>,
    pub message: RwSignal<String>,
}

#[component]
pub(in crate::app::search_workspace) fn ResultsView(
    results: SearchResults,
    actions: ResultActions,
    layout: ResultsLayout,
    engine: EngineStatusSignals,
) -> impl IntoView {
    let viewport = layout.viewport;
    let columns = layout.columns;

    view! {
        <section
            class="results-panel relative [&.icon-view]:grid-rows-[minmax(0,1fr)_24px] [&.icon-view_.column-header]:hidden [&.icon-view_.results-scroll]:row-start-1 [&.icon-view_.statusbar]:row-start-2"
            class:icon-view=move || viewport.mode.get().is_grid()
            class:resizing-columns=move || columns.is_resizing()
            data-view-mode=move || viewport.mode.get().key()
            style=move || format!(
                "{};--result-row-height:{}px",
                columns.layout_style(viewport.grid_width.get()),
                ViewMode::Details.item_height(),
            )
        >
            <ColumnHeaders columns sort=results.sort />
            <ResultsArea results actions layout />
            <ResultsStatusBar results selection=actions.selection engine />
        </section>
    }
}

#[component]
fn ResultsArea(
    results: SearchResults,
    actions: ResultActions,
    layout: ResultsLayout,
) -> impl IntoView {
    let viewport = layout.viewport;
    let selection = actions.selection;
    let menu = actions.menu;
    let total = results.total;
    let drag_selection = DragSelection::new();
    let on_scroll = move |event: web_sys::Event| {
        viewport.update_from_scroll_event(&event);
        menu.close();
    };

    view! {
        <div
            class="results-scroll focus-visible:shadow-none"
            node_ref=viewport.list_ref
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
            on:pointerdown=move |event: PointerEvent| {
                if drag_selection.begin(&event, viewport, selection) {
                    menu.close();
                }
            }
            on:pointermove=move |event: PointerEvent| {
                drag_selection.update(&event, total.get_untracked(), viewport, selection);
            }
            on:pointerup=move |event: PointerEvent| {
                drag_selection.finish(&event, total.get_untracked(), viewport, selection);
            }
            on:pointercancel=move |event: PointerEvent| {
                drag_selection.cancel(&event, viewport, selection);
            }
            on:lostpointercapture=move |event: PointerEvent| {
                drag_selection.lost_pointer_capture(&event, selection);
            }
            on:click=move |_| {
                if !drag_selection.consume_suppressed_click() {
                    selection.clear();
                }
                menu.close();
            }
        >
            <ResultsCanvas results actions layout drag_selection />
            <ResultsFeedback results files=actions.files />
        </div>
    }
}

#[component]
fn ResultsFeedback(results: SearchResults, files: FileOperations) -> impl IntoView {
    let query = results.query;
    let total = results.total;
    let loading = results.loading;
    let search_error = results.error;
    let operation_error = files.error;
    let tip_index = RwSignal::new(0usize);
    let tip_interval = Interval::new(6_000, move || {
        tip_index.update(|index| *index = (*index + 1) % SEARCH_TIPS.len());
    });
    tip_interval.forget();

    view! {
        <Show when=move || query.get().trim().is_empty()>
            <EmptyState
                icon=icons::search()
                title="Start typing"
                message=TextProp::from(move || SEARCH_TIPS[tip_index.get()])
            />
        </Show>
        <Show when=move || !query.get().trim().is_empty() && !loading.get() && total.get() == 0 && search_error.get().is_none()>
            <EmptyState
                icon=icons::empty()
                title="No results"
                message=TextProp::from("Try a less restrictive search or check the syntax.")
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
        <Show when=move || operation_error.get().is_some()>
            <div class="error-banner absolute left-1/2 top-6 flex w-[min(540px,calc(100%_-_48px))] -translate-x-1/2 gap-3 rounded-[9px] border border-[color-mix(in_srgb,var(--danger)_38%,var(--border))] bg-[color-mix(in_srgb,var(--surface-solid)_92%,var(--danger))] px-[15px] py-[13px] text-[var(--danger)] shadow-[var(--shadow)] [&>div]:grid [&>div]:gap-[3px] [&_span]:text-xs [&_span]:text-[var(--text)]" role="alert">
                {icons::warning()}
                <div><strong>"An operation failed"</strong><span>{move || operation_error.get().unwrap_or_default()}</span></div>
                <button class="banner-close focus-visible:bg-[var(--hover)]" title="Close" aria-label="Close" on:click=move |_| operation_error.set(None)>{icons::close()}</button>
            </div>
        </Show>
    }
}

#[component]
fn ResultsStatusBar(
    results: SearchResults,
    selection: ResultSelection,
    engine: EngineStatusSignals,
) -> impl IntoView {
    let total = results.total;
    let loading = results.loading;
    let selected = selection.indices;

    view! {
        <footer class="statusbar flex items-center gap-2 border-t border-[var(--border)] bg-[var(--surface-2)] px-2.5 text-xs text-[var(--muted)]">
            <span>{move || result_count(total.get())}</span>
            <span class="status-separator h-3 w-px bg-[var(--border)]"></span>
            <span>{move || format!("{} selected", selected.with(IndexSelection::count))}</span>
            <Show when=move || engine.indexing.get() || !engine.available.get()>
                <span class="status-separator h-3 w-px bg-[var(--border)]"></span>
                <span class="connection-warning" title=move || engine.message.get()>
                    {move || engine_status_label(engine)}
                </span>
            </Show>
            <span class="statusbar-spacer flex-1"></span>
            <Show when=move || loading.get()><span class="loading-indicator"></span><span>"Searching…"</span></Show>
        </footer>
    }
}

fn engine_status_label(engine: EngineStatusSignals) -> String {
    if !engine.indexing.get() {
        return "Index unavailable".into();
    }
    if !engine.available.get() {
        return "Indexing first drive…".into();
    }
    format!(
        "{}/{} drives ready · Indexing…",
        engine.ready_volumes.get(),
        engine.total_volumes.get(),
    )
}

#[component]
pub(in crate::app::search_workspace) fn ResultContextMenuView(
    results: SearchResults,
    actions: ResultActions,
) -> impl IntoView {
    let ResultActions {
        selection,
        files,
        menu,
    } = actions;
    let context_menu = menu.state;

    view! {
        <Show when=move || context_menu.get().is_some()>
            {move || context_menu.get().map(|menu| view! {
                <div class="context-menu fixed z-[100] w-[260px] rounded-[9px] border border-[var(--border)] bg-[color-mix(in_srgb,var(--surface-solid)_94%,transparent)] p-[5px] shadow-[var(--shadow)] backdrop-blur-[28px] backdrop-saturate-[1.3]" style:left=format!("{}px", menu.x) style:top=format!("{}px", menu.y) on:click=move |event| event.stop_propagation()>
                    <ContextAction disabled=selection.indices.with(|selection| selection.count() != 1) icon=icons::open() label="Open" shortcut="Enter" on_click={
                        let path = menu.item.full_path.clone();
                        move || { files.open(path.clone()); context_menu.set(None); }
                    } />
                    <ContextAction disabled=selection.indices.with(|selection| selection.count() != 1) icon=icons::folder_open() label="Show in Explorer" shortcut="" on_click={
                        let path = menu.item.full_path.clone();
                        move || { files.reveal(path.clone()); context_menu.set(None); }
                    } />
                    <div class="context-separator mx-[7px] my-1 h-px bg-[var(--border)]"></div>
                    <ContextAction icon=icons::copy() label="Copy file" shortcut="Ctrl+C" on_click={
                        move || { files.copy_files(selection, results); context_menu.set(None); }
                    } />
                    <ContextAction icon=icons::copy() label="Copy path" shortcut="" on_click={
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
fn EmptyState(
    icon: AnyView,
    title: &'static str,
    #[prop(default = TextProp::default())] message: TextProp,
) -> impl IntoView {
    let message_for_view = Signal::derive(move || message.get());

    view! {
        <div class="empty-state pointer-events-none absolute inset-0 grid place-content-center justify-items-center p-10 text-center text-[var(--muted)] [&>.native-icon]:mb-3 [&>.native-icon]:h-[46px] [&>.native-icon]:w-[46px] [&>.native-icon]:text-[42px] [&>.native-icon]:text-[color-mix(in_srgb,var(--muted)_70%,transparent)] [&>h2]:mb-[5px] [&>h2]:text-[17px] [&>h2]:font-semibold [&>h2]:text-[var(--text)] [&>p]:max-w-[430px] [&>p]:leading-[1.5]">
            {icon}<h2>{title}</h2>
            <Show when=move || !message_for_view.get().is_empty()>
                <p>{move || message_for_view.get()}</p>
            </Show>
        </div>
    }
}

#[component]
fn ContextAction<F>(
    icon: AnyView,
    label: &'static str,
    shortcut: &'static str,
    #[prop(default = false)] danger: bool,
    #[prop(default = false)] disabled: bool,
    on_click: F,
) -> impl IntoView
where
    F: Fn() + Send + Sync + 'static,
{
    view! {
        <button
            class="context-action grid h-[34px] w-full grid-cols-[22px_1fr_auto] items-center gap-2 rounded-[5px] bg-transparent px-2 text-left enabled:hover:bg-[var(--hover)] focus-visible:bg-[var(--hover)] disabled:text-[var(--muted)] disabled:opacity-50 [&>kbd]:border-0 [&>kbd]:bg-transparent"
            class:danger=danger
            class=("text-[var(--danger)]", danger)
            disabled=disabled
            on:click=move |_| on_click()
        >
            {icon}<span>{label}</span><kbd>{shortcut}</kbd>
        </button>
    }
}
