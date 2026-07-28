use super::file_actions::{FileActionDialogs, FileOperations};
use super::keyboard::KeyboardContext;
use super::results::{
    ResultColumns, ResultContextMenu, ResultContextMenuView, ResultSelection, ResultViewport,
    ResultsView, ResultsViewContext, ViewSwitcher,
};
use super::search::SearchResults;
use crate::app::icons;
use crate::app::settings::{ExcludedFoldersState, SettingsDialog, ThemeState};
use crate::{backend, window};
use gloo_timers::future::TimeoutFuture;
use leptos::prelude::*;
use leptos::task::spawn_local;
use wasm_bindgen::{closure::Closure, JsCast};
use web_sys::{Element, HtmlInputElement, MouseEvent};

const SIDEBAR_QUERIES: &[&str] = &[
    "*",
    "ext:pdf;doc;docx;xls;xlsx;ppt;pptx;md;txt",
    "ext:png;jpg;jpeg;webp;gif;svg;avif",
    "ext:mp4;mkv;avi;mov;webm",
    "ext:mp3;wav;flac;m4a;m4b;aac;ogg;opus;wma;aif;aiff;ape;mid;midi",
    "ext:zip;7z;rar;tar;gz",
];

fn target_accepts_text_input(event: &MouseEvent) -> bool {
    event
        .target()
        .and_then(|target| target.dyn_into::<Element>().ok())
        .is_some_and(|element| {
            matches!(element.tag_name().as_str(), "INPUT" | "TEXTAREA")
                || element
                    .closest("[contenteditable='true']")
                    .ok()
                    .flatten()
                    .is_some()
        })
}

fn apply_sidebar_query(current_query: &str, item_query: &str) -> String {
    let preserved_query = current_query
        .split_whitespace()
        .filter(|part| !SIDEBAR_QUERIES.contains(part))
        .collect::<Vec<_>>()
        .join(" ");

    match (preserved_query.is_empty(), item_query) {
        (true, _) => item_query.to_string(),
        (false, "*") => preserved_query,
        (false, _) => format!("{preserved_query} {item_query}"),
    }
}

fn sidebar_query_is_active(current_query: &str, item_query: &str) -> bool {
    let active_query = current_query
        .split_whitespace()
        .find(|part| SIDEBAR_QUERIES.contains(part));

    match item_query {
        "*" => active_query.is_none() || active_query == Some("*"),
        _ => active_query == Some(item_query),
    }
}

async fn apply_pending_search_query(
    query: RwSignal<String>,
    search_ref: NodeRef<leptos::html::Input>,
) {
    if let Ok(Some(pending_query)) = backend::take_pending_search_query().await {
        query.set(pending_query);
        if let Some(input) = search_ref.get() {
            let _ = input.focus();
            input.select();
        }
    }
}

#[component]
#[allow(
    non_snake_case,
    reason = "Leptos components conventionally use PascalCase names"
)]
pub(in crate::app) fn SearchWorkspace() -> impl IntoView {
    let excluded_folders = ExcludedFoldersState::new();
    let results = SearchResults::new(excluded_folders.folders);
    let selection = ResultSelection::new();
    let files = FileOperations::new();
    let menu = ResultContextMenu::new();
    let columns = ResultColumns::new();
    let theme = ThemeState::new();

    let query = results.query;
    let total = results.total;
    let selected = selection.indices;

    let viewport = ResultViewport::new();
    let engine_message = RwSignal::new("Connecting to Everything…".to_string());
    let engine_available = RwSignal::new(false);
    let engine_indexing = RwSignal::new(true);
    let ready_volumes = RwSignal::new(0_u32);
    let total_volumes = RwSignal::new(0_u32);
    let settings_open = RwSignal::new(false);
    let view_menu_open = RwSignal::new(false);
    let search_ref = NodeRef::<leptos::html::Input>::new();

    spawn_local(async move {
        loop {
            let status = backend::status().await;
            let poll_delay_ms = if status.indexing { 500 } else { 3_000 };
            let previous_ready = ready_volumes.get_untracked();
            engine_available.set(status.available);
            engine_indexing.set(status.indexing);
            engine_message.set(status.message);
            ready_volumes.set(status.ready_volumes);
            total_volumes.set(status.total_volumes);
            if status.ready_volumes > previous_ready && !query.get_untracked().trim().is_empty() {
                results.refresh_incrementally();
            }
            TimeoutFuture::new(poll_delay_ms).await;
        }
    });

    let event_query = query;
    let event_search_ref = search_ref;
    let launch_callback = Closure::<dyn FnMut()>::new(move || {
        spawn_local(apply_pending_search_query(event_query, event_search_ref));
    });
    spawn_local(async move {
        let listening = backend::listen_for_search_query(
            launch_callback.as_ref().unchecked_ref::<js_sys::Function>(),
        )
        .await
        .is_ok();
        apply_pending_search_query(query, search_ref).await;
        if listening {
            launch_callback.forget();
        }
    });

    viewport.monitor_dimensions();

    let visible_start = viewport.visible_start();
    let visible_end = viewport.visible_end(visible_start, total);
    results.monitor(visible_start, visible_end, move |preserve_view| {
        selection.clear();
        files.reset_for_new_search();
        if !preserve_view {
            viewport.reset_scroll();
        }
    });

    let results_view = ResultsViewContext {
        results,
        selection,
        files,
        menu,
        columns,
        viewport,
        visible_start,
        visible_end,
        engine_available,
        engine_indexing,
        ready_volumes,
        total_volumes,
        engine_message,
    };

    let on_search_input = move |event: web_sys::Event| {
        if let Some(input) = event
            .target()
            .and_then(|target| target.dyn_into::<HtmlInputElement>().ok())
        {
            query.set(input.value());
        }
    };

    let keyboard = KeyboardContext {
        settings_open,
        search_ref,
        selection,
        results,
        viewport,
        files,
        menu,
        last_initial: RwSignal::new(None),
    };
    let on_keydown = move |event| keyboard.handle_keydown(event);

    view! {
        <main
            class="app-shell grid h-screen grid-rows-[32px_48px_42px_minmax(0,1fr)] bg-[var(--bg)] outline-none"
            tabindex="0"
            on:keydown=on_keydown
            on:contextmenu=move |event: MouseEvent| {
                if !target_accepts_text_input(&event) {
                    event.prevent_default();
                }
            }
            on:click=move |_| {
                menu.close();
                view_menu_open.set(false);
            }
            on:pointermove=move |event| columns.update_resize(event)
            on:pointerup=move |event| columns.finish_resize(event)
            on:pointercancel=move |event| columns.finish_resize(event)
        >
            <header class="titlebar flex items-center gap-[9px] bg-[var(--header-bg)] pl-[10px]" data-tauri-drag-region>
                <div class="app-mark grid size-5 place-items-center bg-transparent text-[0] text-transparent shadow-none [&_svg]:size-5" aria-hidden="true">
                    <svg viewBox="0 0 256 256">
                        <rect x="14" y="14" width="228" height="228" rx="56" fill="#FFD76B"></rect>
                        <circle cx="108" cy="104" r="50" fill="none" stroke="#1992CA" stroke-width="23"></circle>
                        <path d="M143.5 139.5 196 192" fill="none" stroke="#1992CA" stroke-width="23" stroke-linecap="round"></path>
                    </svg>
                </div>
                <div class="app-title text-xs font-semibold tracking-[.01em]" data-tauri-drag-region>"Everything Next"</div>
                <div class="titlebar-spacer h-full flex-1" data-tauri-drag-region></div>
                <div class="window-controls flex self-stretch" on:dblclick=move |event| event.stop_propagation()>
                    <button class="window-control grid h-full w-[46px] place-items-center rounded-none bg-transparent hover:bg-[var(--hover)] active:bg-[var(--pressed)] focus-visible:bg-[var(--hover)] [&>.native-icon]:size-3 [&>.native-icon]:pointer-events-none [&>.native-icon]:text-[10px]" title="Minimize" aria-label="Minimize" on:click=move |event| { event.stop_propagation(); window::minimize(); }>{icons::minimize()}</button>
                    <button class="window-control grid h-full w-[46px] place-items-center rounded-none bg-transparent hover:bg-[var(--hover)] active:bg-[var(--pressed)] focus-visible:bg-[var(--hover)] [&>.native-icon]:size-3 [&>.native-icon]:pointer-events-none [&>.native-icon]:text-[10px]" title="Maximize or restore" aria-label="Maximize or restore" on:click=move |event| { event.stop_propagation(); window::toggle_maximize(); }>{icons::maximize()}</button>
                    <button class="window-control close grid h-full w-[46px] place-items-center rounded-none bg-transparent hover:bg-[#c42b1c] hover:text-white active:bg-[var(--pressed)] focus-visible:bg-[var(--hover)] [&>.native-icon]:size-3 [&>.native-icon]:pointer-events-none [&>.native-icon]:text-[10px]" title="Close" aria-label="Close" on:click=move |event| { event.stop_propagation(); window::close(); }>{icons::close()}</button>
                </div>
            </header>

            <div class="search-toolbar flex min-w-0 items-center bg-[var(--header-bg)] px-[10px] py-[6px]" role="search">
                <div class="search-box flex h-[34px] w-full min-w-0 items-center gap-2 rounded border border-[var(--border-soft)] bg-[var(--search-bg)] px-[10px] focus-within:border-[var(--border-soft)] focus-within:shadow-none [&>.native-icon]:text-[var(--muted)] [&>input]:min-w-0 [&>input]:flex-1 [&>input]:select-text [&>input]:border-0 [&>input]:bg-transparent [&>input]:outline-none [&>input::-webkit-search-cancel-button]:hidden">
                    {icons::search()}
                    <input
                        node_ref=search_ref
                        type="search"
                        placeholder="Search Everything"
                        prop:value=move || query.get()
                        on:input=on_search_input
                        autofocus
                    />
                </div>
            </div>

            <section class="command-bar relative z-10 flex items-center gap-0.5 border-y border-[var(--border)] border-t-[var(--border-soft)] bg-[var(--surface)] px-[10px] py-1 backdrop-blur-[20px] backdrop-saturate-[1.15]" aria-label="Commands">
                <button class="command-button flex h-8 items-center gap-2 rounded-[5px] bg-transparent px-[10px] enabled:hover:bg-[var(--hover)] enabled:active:bg-[var(--pressed)] focus-visible:bg-[var(--hover)] disabled:text-[var(--muted)] disabled:opacity-50 [&_.native-icon]:size-[17px] [&_.native-icon]:text-base" title="Open" disabled=move || selected.with(|selection| selection.count() != 1) on:click=move |_| {
                    if let Some(item) = selection.focused_item(results) {
                        files.open(item.full_path);
                    }
                }>{icons::open()}<span>"Open"</span></button>
                <button class="command-button flex h-8 items-center gap-2 rounded-[5px] bg-transparent px-[10px] enabled:hover:bg-[var(--hover)] enabled:active:bg-[var(--pressed)] focus-visible:bg-[var(--hover)] disabled:text-[var(--muted)] disabled:opacity-50 [&_.native-icon]:size-[17px] [&_.native-icon]:text-base" title="Show in Explorer" disabled=move || selected.with(|selection| selection.count() != 1) on:click=move |_| {
                    if let Some(item) = selection.focused_item(results) {
                        files.reveal(item.full_path);
                    }
                }>{icons::folder_open()}<span>"Show in Explorer"</span></button>
                <span class="command-separator mx-[5px] h-5 w-px bg-[var(--border)]"></span>
                <button class="command-button danger-hover flex h-8 items-center gap-2 rounded-[5px] bg-transparent px-[10px] enabled:hover:bg-[var(--hover)] enabled:hover:text-[var(--danger)] enabled:active:bg-[var(--pressed)] focus-visible:bg-[var(--hover)] disabled:text-[var(--muted)] disabled:opacity-50 [&_.native-icon]:size-[17px] [&_.native-icon]:text-base" title="Move to Recycle Bin" disabled=move || selected.with(|selection| selection.count() == 0) on:click=move |_| files.begin_trash(selection, results)>{icons::trash()}<span>"Delete"</span></button>
                <ViewSwitcher viewport open=view_menu_open />
            </section>

            <div class="workspace grid min-h-0 grid-cols-[204px_minmax(0,1fr)] max-[850px]:grid-cols-[58px_minmax(0,1fr)]">
                <aside class="sidebar flex min-h-0 flex-col border-r border-[var(--border)] bg-[var(--sidebar-bg)] px-[6px] py-2 max-[850px]:items-center">
                    <SidebarItem label="All files" icon=icons::home() item_query="*" query />
                    <div class="sidebar-separator mx-[10px] mb-0.5 mt-[7px] h-px bg-[var(--border-soft)]"></div>
                    <div class="sidebar-section-label px-[10px] pb-[5px] pt-[9px] text-[11px] font-semibold text-[var(--muted)] max-[850px]:hidden">"Types"</div>
                    <SidebarItem label="Documents" icon=icons::document() item_query="ext:pdf;doc;docx;xls;xlsx;ppt;pptx;md;txt" query />
                    <SidebarItem label="Images" icon=icons::image() item_query="ext:png;jpg;jpeg;webp;gif;svg;avif" query />
                    <SidebarItem label="Videos" icon=icons::video() item_query="ext:mp4;mkv;avi;mov;webm" query />
                    <SidebarItem label="Audio" icon=icons::audio() item_query="ext:mp3;wav;flac;m4a;m4b;aac;ogg;opus;wma;aif;aiff;ape;mid;midi" query />
                    <SidebarItem label="Archives" icon=icons::archive() item_query="ext:zip;7z;rar;tar;gz" query />
                    <div class="sidebar-spacer flex-1" aria-hidden="true"></div>
                    <div class="sidebar-separator mx-[10px] mb-0.5 mt-[7px] h-px bg-[var(--border-soft)]"></div>
                    <button
                        type="button"
                        class="sidebar-item settings-sidebar-item relative grid h-8 w-full grid-cols-[20px_minmax(0,1fr)] items-center gap-[10px] rounded-[5px] bg-transparent px-[9px] text-left hover:bg-[var(--hover)] focus-visible:bg-[var(--hover)] [&_.native-icon]:size-5 [&_.native-icon]:translate-y-px [&_.native-icon]:text-base [&_.native-icon]:text-[var(--muted)] [&>span:not(.native-icon)]:min-w-0 [&>span:not(.native-icon)]:leading-5 max-[850px]:w-[38px] max-[850px]:grid-cols-[20px] max-[850px]:justify-center max-[850px]:px-0 max-[850px]:[&>span]:hidden"
                        title="Settings"
                        aria-haspopup="dialog"
                        aria-expanded=move || settings_open.get()
                        on:click=move |_| settings_open.set(true)
                    >
                        {icons::settings()}<span>"Settings"</span>
                    </button>
                </aside>

                <ResultsView context=results_view />
            </div>

            <ResultContextMenuView context=results_view />

            <SettingsDialog open=settings_open theme excluded_folders />

            <FileActionDialogs files selection results />
        </main>
    }
}

#[component]
fn SidebarItem(
    label: &'static str,
    icon: AnyView,
    item_query: &'static str,
    query: RwSignal<String>,
) -> impl IntoView {
    view! {
        <button
            class="sidebar-item relative grid h-8 w-full grid-cols-[20px_minmax(0,1fr)] items-center gap-[10px] rounded-[5px] bg-transparent px-[9px] text-left before:absolute before:left-px before:h-4 before:w-[3px] before:rounded-sm before:bg-[var(--accent)] before:content-[''] before:hidden hover:bg-[var(--hover)] focus-visible:bg-[var(--hover)] [&.active]:bg-[var(--hover)] [&.active]:before:block [&.active_.native-icon]:text-[var(--text)] [&_.native-icon]:size-5 [&_.native-icon]:text-base [&_.native-icon]:text-[var(--muted)] [&>span:not(.native-icon)]:min-w-0 [&>span:not(.native-icon)]:leading-5 max-[850px]:w-[38px] max-[850px]:grid-cols-[20px] max-[850px]:justify-center max-[850px]:px-0 max-[850px]:before:left-0 max-[850px]:[&>span]:hidden"
            class:active=move || sidebar_query_is_active(&query.get(), item_query)
            on:click=move |_| query.update(|current| *current = apply_sidebar_query(current, item_query))
        >
            {icon}<span>{label}</span>
        </button>
    }
}

#[cfg(test)]
mod tests {
    use super::{apply_sidebar_query, sidebar_query_is_active};

    const AUDIO_QUERY: &str = "ext:mp3;wav;flac;m4a;m4b;aac;ogg;opus;wma;aif;aiff;ape;mid;midi";
    const IMAGE_QUERY: &str = "ext:png;jpg;jpeg;webp;gif;svg;avif";

    #[test]
    fn sidebar_filter_preserves_the_search_query() {
        assert_eq!(
            apply_sidebar_query("annual report", AUDIO_QUERY),
            format!("annual report {AUDIO_QUERY}")
        );
    }

    #[test]
    fn changing_sidebar_filter_keeps_only_the_new_filter() {
        assert_eq!(
            apply_sidebar_query(&format!("annual report {IMAGE_QUERY}"), AUDIO_QUERY),
            format!("annual report {AUDIO_QUERY}")
        );
    }

    #[test]
    fn all_files_removes_the_sidebar_filter_but_keeps_the_search_query() {
        assert_eq!(
            apply_sidebar_query(&format!("annual report {AUDIO_QUERY}"), "*"),
            "annual report"
        );
    }

    #[test]
    fn active_sidebar_item_is_detected_alongside_a_search_query() {
        let query = format!("annual report {AUDIO_QUERY}");

        assert!(sidebar_query_is_active(&query, AUDIO_QUERY));
        assert!(!sidebar_query_is_active(&query, IMAGE_QUERY));
        assert!(!sidebar_query_is_active(&query, "*"));
        assert!(sidebar_query_is_active("annual report", "*"));
    }
}
