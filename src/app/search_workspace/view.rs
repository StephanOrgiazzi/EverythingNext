use super::file_actions::{FileActionDialogs, FileOperations};
use super::keyboard::KeyboardContext;
use super::results::{
    EngineStatusSignals, ResultActions, ResultColumns, ResultContextMenu, ResultContextMenuView,
    ResultSelection, ResultViewport, ResultsLayout, ResultsView, ViewSwitcher,
};
use super::search::SearchResults;
use crate::app::icons;
use crate::app::settings::{ExcludedFoldersState, SettingsDialog, ThemeState};
use crate::{backend, window};
use gloo_timers::future::TimeoutFuture;
use leptos::ev;
use leptos::leptos_dom::helpers::window_event_listener;
use leptos::prelude::*;
use leptos::task::spawn_local;
use wasm_bindgen::{closure::Closure, JsCast};
use web_sys::{Element, HtmlInputElement, MouseEvent};

#[derive(Clone, Copy)]
struct SidebarFilter {
    label: &'static str,
    query: &'static str,
    icon: fn() -> AnyView,
    section_label: Option<&'static str>,
}

const SIDEBAR_FILTERS: [SidebarFilter; 7] = [
    SidebarFilter {
        label: "All files",
        query: "*",
        icon: icons::home,
        section_label: None,
    },
    SidebarFilter {
        label: "Folders",
        query: "folder:",
        icon: icons::folder,
        section_label: Some("Types"),
    },
    SidebarFilter {
        label: "Documents",
        query: "ext:pdf;doc;docx;xls;xlsx;ppt;pptx;md;txt",
        icon: icons::document,
        section_label: None,
    },
    SidebarFilter {
        label: "Images",
        query: "ext:png;jpg;jpeg;webp;gif;svg;avif",
        icon: icons::image,
        section_label: None,
    },
    SidebarFilter {
        label: "Videos",
        query: "ext:mp4;mkv;avi;mov;webm",
        icon: icons::video,
        section_label: None,
    },
    SidebarFilter {
        label: "Audio",
        query: "ext:mp3;wav;flac;m4a;m4b;aac;ogg;opus;wma;aif;aiff;ape;mid;midi",
        icon: icons::audio,
        section_label: None,
    },
    SidebarFilter {
        label: "Archives",
        query: "ext:zip;7z;rar;tar;gz",
        icon: icons::archive,
        section_label: None,
    },
];

fn is_sidebar_query(query: &str) -> bool {
    SIDEBAR_FILTERS.iter().any(|filter| filter.query == query)
}

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
        .filter(|part| !is_sidebar_query(part))
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
        .find(|part| is_sidebar_query(part));

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

fn start_engine_monitor(results: SearchResults, engine: EngineStatusSignals) {
    let query = results.query;
    spawn_local(async move {
        loop {
            let status = backend::status().await;
            let poll_delay_ms = if status.indexing { 500 } else { 3_000 };
            let previous_ready = engine.ready_volumes.get_untracked();
            engine.available.set(status.available);
            engine.indexing.set(status.indexing);
            engine.message.set(status.message);
            engine.ready_volumes.set(status.ready_volumes);
            engine.total_volumes.set(status.total_volumes);
            if status.ready_volumes > previous_ready && !query.get_untracked().trim().is_empty() {
                results.refresh_incrementally();
            }
            TimeoutFuture::new(poll_delay_ms).await;
        }
    });
}

fn listen_for_launch_queries(query: RwSignal<String>, search_ref: NodeRef<leptos::html::Input>) {
    let launch_callback = Closure::<dyn FnMut()>::new(move || {
        spawn_local(apply_pending_search_query(query, search_ref));
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
}

fn install_keyboard_listener(
    results: SearchResults,
    actions: ResultActions,
    viewport: ResultViewport,
    settings_open: RwSignal<bool>,
    search_ref: NodeRef<leptos::html::Input>,
) {
    let keyboard = KeyboardContext {
        settings_open,
        search_ref,
        selection: actions.selection,
        results,
        viewport,
        files: actions.files,
        menu: actions.menu,
        last_initial: RwSignal::new(None),
    };
    let listener = window_event_listener(ev::keydown, move |event| keyboard.handle_keydown(&event));
    on_cleanup(move || listener.remove());
}

#[component]
pub(in crate::app) fn SearchWorkspace() -> impl IntoView {
    let excluded_folders = ExcludedFoldersState::new();
    let results = SearchResults::new(excluded_folders.folders);
    let selection = ResultSelection::new();
    let files = FileOperations::new();
    let menu = ResultContextMenu::new();
    let columns = ResultColumns::new();
    let theme = ThemeState::new();

    let viewport = ResultViewport::new();
    let engine = EngineStatusSignals {
        available: RwSignal::new(false),
        indexing: RwSignal::new(true),
        ready_volumes: RwSignal::new(0),
        total_volumes: RwSignal::new(0),
        message: RwSignal::new("Connecting to Everything…".to_string()),
    };
    let settings_open = RwSignal::new(false);
    let view_menu_open = RwSignal::new(false);
    let search_ref = NodeRef::<leptos::html::Input>::new();
    let query = results.query;

    start_engine_monitor(results, engine);
    listen_for_launch_queries(query, search_ref);
    viewport.monitor_dimensions();

    let visible_start = viewport.visible_start();
    let visible_end = viewport.visible_end(visible_start, results.total);
    results.monitor(visible_start, visible_end, move |preserve_view| {
        selection.clear();
        files.reset_for_new_search();
        if !preserve_view {
            viewport.reset_scroll();
        }
    });

    let actions = ResultActions {
        selection,
        files,
        menu,
    };
    let layout = ResultsLayout {
        columns,
        viewport,
        visible_start,
        visible_end,
    };
    install_keyboard_listener(results, actions, viewport, settings_open, search_ref);

    view! {
        <main
            class="app-shell grid h-screen grid-rows-[32px_48px_42px_minmax(0,1fr)] bg-[var(--bg)] outline-none"
            tabindex="0"
            on:contextmenu=move |event: MouseEvent| {
                if !target_accepts_text_input(&event) {
                    event.prevent_default();
                }
            }
            on:click=move |_| {
                menu.close();
                view_menu_open.set(false);
            }
            on:pointermove=move |event| columns.update_resize(&event)
            on:pointerup=move |event| columns.finish_resize(&event)
            on:pointercancel=move |event| columns.finish_resize(&event)
        >
            <AppTitleBar />
            <SearchToolbar query search_ref />
            <CommandBar results actions viewport view_menu_open />
            <WorkspaceContent query settings_open results actions layout engine />

            <ResultContextMenuView results actions />

            <SettingsDialog open=settings_open theme excluded_folders />

            <FileActionDialogs files selection results />
        </main>
    }
}

#[component]
fn AppTitleBar() -> impl IntoView {
    view! {
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
    }
}

#[component]
fn SearchToolbar(
    query: RwSignal<String>,
    search_ref: NodeRef<leptos::html::Input>,
) -> impl IntoView {
    let on_input = move |event: web_sys::Event| {
        if let Some(input) = event
            .target()
            .and_then(|target| target.dyn_into::<HtmlInputElement>().ok())
        {
            query.set(input.value());
        }
    };

    view! {
        <div class="search-toolbar flex min-w-0 items-center bg-[var(--header-bg)] px-[10px] py-[6px]" role="search">
            <div class="search-box flex h-[34px] w-full min-w-0 items-center gap-2 rounded border border-[var(--border-soft)] bg-[var(--search-bg)] px-[10px] focus-within:border-[var(--border-soft)] focus-within:shadow-none [&>.native-icon]:text-[var(--muted)] [&>input]:min-w-0 [&>input]:flex-1 [&>input]:select-text [&>input]:border-0 [&>input]:bg-transparent [&>input]:outline-none [&>input::-webkit-search-cancel-button]:hidden">
                {icons::search()}
                <input node_ref=search_ref type="search" placeholder="Search Everything" prop:value=move || query.get() on:input=on_input autofocus />
            </div>
        </div>
    }
}

#[component]
fn CommandBar(
    results: SearchResults,
    actions: ResultActions,
    viewport: ResultViewport,
    view_menu_open: RwSignal<bool>,
) -> impl IntoView {
    let selection = actions.selection;
    let selected = selection.indices;
    let files = actions.files;

    view! {
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
    }
}

#[component]
fn WorkspaceContent(
    query: RwSignal<String>,
    settings_open: RwSignal<bool>,
    results: SearchResults,
    actions: ResultActions,
    layout: ResultsLayout,
    engine: EngineStatusSignals,
) -> impl IntoView {
    view! {
        <div class="workspace grid min-h-0 grid-cols-[204px_minmax(0,1fr)] max-[850px]:grid-cols-[58px_minmax(0,1fr)]">
            <Sidebar query settings_open />
            <ResultsView results actions layout engine />
        </div>
    }
}

#[component]
fn Sidebar(query: RwSignal<String>, settings_open: RwSignal<bool>) -> impl IntoView {
    view! {
        <aside class="sidebar flex min-h-0 flex-col border-r border-[var(--border)] bg-[var(--sidebar-bg)] px-[6px] py-2 max-[850px]:items-center">
            {SIDEBAR_FILTERS.into_iter().map(|filter| view! {
                {filter.section_label.map(|label| view! {
                    <div class="sidebar-separator mx-[10px] mb-0.5 mt-[7px] h-px bg-[var(--border-soft)]"></div>
                    <div class="sidebar-section-label px-[10px] pb-[5px] pt-[9px] text-[11px] font-semibold text-[var(--muted)] max-[850px]:hidden">{label}</div>
                })}
                <SidebarItem filter query />
            }).collect_view()}
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
    }
}

#[component]
fn SidebarItem(filter: SidebarFilter, query: RwSignal<String>) -> impl IntoView {
    let item_query = filter.query;
    view! {
        <button
            class="sidebar-item relative grid h-8 w-full grid-cols-[20px_minmax(0,1fr)] items-center gap-[10px] rounded-[5px] bg-transparent px-[9px] text-left before:absolute before:left-px before:h-4 before:w-[3px] before:rounded-sm before:bg-[var(--accent)] before:content-[''] before:hidden hover:bg-[var(--hover)] focus-visible:bg-[var(--hover)] [&.active]:bg-[var(--hover)] [&.active]:before:block [&.active_.native-icon]:text-[var(--text)] [&_.native-icon]:size-5 [&_.native-icon]:text-base [&_.native-icon]:text-[var(--muted)] [&>span:not(.native-icon)]:min-w-0 [&>span:not(.native-icon)]:leading-5 max-[850px]:w-[38px] max-[850px]:grid-cols-[20px] max-[850px]:justify-center max-[850px]:px-0 max-[850px]:before:left-0 max-[850px]:[&>span]:hidden"
            class:active=move || sidebar_query_is_active(&query.get(), item_query)
            on:click=move |_| query.update(|current| *current = apply_sidebar_query(current, item_query))
        >
            {(filter.icon)()}<span>{filter.label}</span>
        </button>
    }
}

#[cfg(test)]
mod tests {
    use super::{apply_sidebar_query, sidebar_query_is_active, SIDEBAR_FILTERS};

    fn filter_query(label: &str) -> &'static str {
        SIDEBAR_FILTERS
            .iter()
            .find(|filter| filter.label == label)
            .expect("the tested sidebar filter exists")
            .query
    }

    #[test]
    fn sidebar_filter_preserves_the_search_query() {
        let audio_query = filter_query("Audio");
        let folders_query = filter_query("Folders");
        assert_eq!(
            apply_sidebar_query("annual report", audio_query),
            format!("annual report {audio_query}")
        );
        assert_eq!(
            apply_sidebar_query("annual report", folders_query),
            format!("annual report {folders_query}")
        );
    }

    #[test]
    fn changing_sidebar_filter_keeps_only_the_new_filter() {
        let audio_query = filter_query("Audio");
        let images_query = filter_query("Images");
        assert_eq!(
            apply_sidebar_query(&format!("annual report {images_query}"), audio_query),
            format!("annual report {audio_query}")
        );
    }

    #[test]
    fn all_files_removes_the_sidebar_filter_but_keeps_the_search_query() {
        let audio_query = filter_query("Audio");
        assert_eq!(
            apply_sidebar_query(&format!("annual report {audio_query}"), "*"),
            "annual report"
        );
    }

    #[test]
    fn active_sidebar_item_is_detected_alongside_a_search_query() {
        let audio_query = filter_query("Audio");
        let images_query = filter_query("Images");
        let query = format!("annual report {audio_query}");

        assert!(sidebar_query_is_active(&query, audio_query));
        assert!(!sidebar_query_is_active(&query, images_query));
        assert!(!sidebar_query_is_active(&query, "*"));
        assert!(sidebar_query_is_active("annual report", "*"));
    }
}
