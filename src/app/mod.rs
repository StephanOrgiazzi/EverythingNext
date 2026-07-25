mod columns;
mod context_menu;
mod exclusions;
mod file_operations;
mod formatting;
mod icons;
mod results;
mod search;
mod selection;
mod theme;

use self::columns::{ColumnHeaders, ResultColumns};
use self::context_menu::{event_target_is_interactive, ResultContextMenu};
use self::exclusions::{ExcludedFoldersSetting, ExcludedFoldersState};
use self::file_operations::FileOperations;
use self::results::{FileIcon, ResultViewport};
use self::search::{SearchResults, RESULT_ROW_HEIGHT};
use self::selection::{FocusMove, ResultSelection, SelectionModifiers};
use self::theme::{ThemeSetting, ThemeState};
use crate::{backend, window};
use everything_core::IndexSelection;
use gloo_timers::future::TimeoutFuture;
use leptos::prelude::*;
use leptos::task::spawn_local;
use wasm_bindgen::JsCast;
use web_sys::{HtmlInputElement, KeyboardEvent, MouseEvent};

#[component]
#[allow(
    non_snake_case,
    reason = "Leptos components conventionally use PascalCase names"
)]
pub fn App() -> impl IntoView {
    let excluded_folders = ExcludedFoldersState::new();
    let results = SearchResults::new(excluded_folders.folders);
    let selection = ResultSelection::new();
    let files = FileOperations::new();
    let menu = ResultContextMenu::new();
    let columns = ResultColumns::new();
    let theme = ThemeState::new();

    let query = results.query;
    let total = results.total;
    let sort = results.sort;
    let loading = results.loading;
    let search_error = results.error;
    let selected = selection.indices;
    let focused_index = selection.focused_index;
    let error = files.error;
    let rename_target = files.rename_target;
    let rename_value = files.rename_value;
    let context_menu = menu.state;

    let viewport = ResultViewport::new();
    let engine_message = RwSignal::new("Connecting to Everything…".to_string());
    let engine_available = RwSignal::new(false);
    let settings_open = RwSignal::new(false);
    let search_ref = NodeRef::<leptos::html::Input>::new();
    let list_ref = viewport.list_ref;

    spawn_local(async move {
        loop {
            let status = backend::status().await;
            engine_available.set(status.available);
            engine_message.set(status.message);
            TimeoutFuture::new(3_000).await;
        }
    });

    viewport.monitor_dimensions();

    let visible_start = viewport.visible_start();
    let visible_end = viewport.visible_end(visible_start, total);
    results.monitor(visible_start, visible_end, move || {
        selection.clear();
        files.reset_for_new_search();
        viewport.reset_scroll();
    });

    let on_search_input = move |event: web_sys::Event| {
        if let Some(input) = event
            .target()
            .and_then(|target| target.dyn_into::<HtmlInputElement>().ok())
        {
            query.set(input.value());
        }
    };

    let on_scroll = move |event: web_sys::Event| {
        viewport.update_from_scroll_event(event);
        menu.close();
    };

    let on_keydown = move |event: KeyboardEvent| {
        let key = event.key();
        if key == "Escape" && settings_open.get_untracked() {
            event.prevent_default();
            settings_open.set(false);
            return;
        }

        if event.ctrl_key() && key.eq_ignore_ascii_case("l") {
            event.prevent_default();
            if let Some(input) = search_ref.get() {
                let _ = input.focus();
                input.select();
            }
            return;
        }

        if event_target_is_interactive(&event) {
            return;
        }

        if event.ctrl_key() && key.eq_ignore_ascii_case("a") {
            event.prevent_default();
            selection.select_all(total.get_untracked());
            return;
        }

        let page_step =
            ((viewport.height.get_untracked() / RESULT_ROW_HEIGHT).floor() as i32).max(1);
        match key.as_str() {
            "ArrowDown" => {
                event.prevent_default();
                move_selection_focus(FocusMove::Relative(1), &event, selection, results, viewport);
            }
            "ArrowUp" => {
                event.prevent_default();
                move_selection_focus(
                    FocusMove::Relative(-1),
                    &event,
                    selection,
                    results,
                    viewport,
                );
            }
            "PageDown" => {
                event.prevent_default();
                move_selection_focus(
                    FocusMove::Relative(page_step),
                    &event,
                    selection,
                    results,
                    viewport,
                );
            }
            "PageUp" => {
                event.prevent_default();
                move_selection_focus(
                    FocusMove::Relative(-page_step),
                    &event,
                    selection,
                    results,
                    viewport,
                );
            }
            "Home" => {
                event.prevent_default();
                move_selection_focus(FocusMove::Absolute(0), &event, selection, results, viewport);
            }
            "End" => {
                event.prevent_default();
                let last = total.get_untracked().saturating_sub(1);
                move_selection_focus(
                    FocusMove::Absolute(last),
                    &event,
                    selection,
                    results,
                    viewport,
                );
            }
            " " if event.ctrl_key() => {
                event.prevent_default();
                selection.toggle_focused();
            }
            "Enter" => {
                event.prevent_default();
                if let Some(item) = selection.focused_item(results) {
                    files.open(item.full_path);
                }
            }
            "Delete" => {
                event.prevent_default();
                files.begin_trash(selection, results);
            }
            "F2" => {
                event.prevent_default();
                if let Some(item) = selection.focused_item(results) {
                    files.begin_rename(item);
                }
            }
            "ContextMenu" => {
                event.prevent_default();
                menu.open_at_focused_row(results, selection, list_ref);
            }
            "F10" if event.shift_key() => {
                event.prevent_default();
                menu.open_at_focused_row(results, selection, list_ref);
            }
            "Escape" => {
                if context_menu.get_untracked().is_some() {
                    menu.close();
                } else {
                    selection.clear_indices();
                }
            }
            _ => {}
        }
    };

    view! {
        <main
            class="app-shell"
            tabindex="0"
            on:keydown=on_keydown
            on:click=move |_| menu.close()
            on:pointermove=move |event| columns.update_resize(event)
            on:pointerup=move |event| columns.finish_resize(event)
            on:pointercancel=move |event| columns.finish_resize(event)
        >
            <header class="titlebar" data-tauri-drag-region on:dblclick=move |_| window::toggle_maximize()>
                <div class="app-mark" aria-hidden="true">
                    <svg viewBox="0 0 256 256">
                        <rect x="14" y="14" width="228" height="228" rx="56" fill="#FFD76B"></rect>
                        <circle cx="108" cy="104" r="50" fill="none" stroke="#C95000" stroke-width="23"></circle>
                        <path d="M143.5 139.5 196 192" fill="none" stroke="#C95000" stroke-width="23" stroke-linecap="round"></path>
                    </svg>
                </div>
                <div class="app-title" data-tauri-drag-region>"Everything Modern"</div>
                <div class="titlebar-spacer" data-tauri-drag-region></div>
                <div class="window-controls" on:dblclick=move |event| event.stop_propagation()>
                    <button class="window-control" title="Minimize" aria-label="Minimize" on:click=move |event| { event.stop_propagation(); window::minimize(); }>{icons::minimize()}</button>
                    <button class="window-control" title="Maximize or restore" aria-label="Maximize or restore" on:click=move |event| { event.stop_propagation(); window::toggle_maximize(); }>{icons::maximize()}</button>
                    <button class="window-control close" title="Close" aria-label="Close" on:click=move |event| { event.stop_propagation(); window::close(); }>{icons::close()}</button>
                </div>
            </header>

            <div class="search-toolbar" role="search">
                <div class="search-box">
                    {icons::search()}
                    <input
                        node_ref=search_ref
                        type="search"
                        placeholder="Search Everything"
                        prop:value=move || query.get()
                        on:input=on_search_input
                        autofocus
                    />
                    <kbd>"Ctrl L"</kbd>
                </div>
            </div>

            <section class="command-bar" aria-label="Commands">
                <button class="command-button" title="Open" disabled=move || selected.with(|selection| selection.count() == 0) on:click=move |_| {
                    if let Some(item) = selection.focused_item(results) {
                        files.open(item.full_path);
                    }
                }>{icons::open()}<span>"Open"</span></button>
                <button class="command-button" title="Show in Explorer" disabled=move || selected.with(|selection| selection.count() == 0) on:click=move |_| {
                    if let Some(item) = selection.focused_item(results) {
                        files.reveal(item.full_path);
                    }
                }>{icons::folder_open()}<span>"Show in Explorer"</span></button>
                <span class="command-separator"></span>
                <button class="command-button danger-hover" title="Move to Recycle Bin" disabled=move || selected.with(|selection| selection.count() == 0) on:click=move |_| files.begin_trash(selection, results)>{icons::trash()}<span>"Delete"</span></button>
            </section>

            <div class="workspace">
                <aside class="sidebar">
                    <SidebarItem label="All files" icon=icons::home() item_query="*" query />
                    <SidebarItem label="Modified today" icon=icons::clock() item_query="dm:today" query />
                    <div class="sidebar-separator"></div>
                    <div class="sidebar-section-label">"Types"</div>
                    <SidebarItem label="Documents" icon=icons::document() item_query="ext:pdf;doc;docx;xls;xlsx;ppt;pptx;md;txt" query />
                    <SidebarItem label="Images" icon=icons::image() item_query="ext:png;jpg;jpeg;webp;gif;svg;avif" query />
                    <SidebarItem label="Videos" icon=icons::video() item_query="ext:mp4;mkv;avi;mov;webm" query />
                    <SidebarItem label="Audio" icon=icons::audio() item_query="ext:mp3;wav;flac;m4a;m4b;aac;ogg;opus;wma;aif;aiff;ape;mid;midi" query />
                    <SidebarItem label="Archives" icon=icons::archive() item_query="ext:zip;7z;rar;tar;gz" query />
                    <div class="sidebar-spacer" aria-hidden="true"></div>
                    <div class="sidebar-separator"></div>
                    <button
                        type="button"
                        class="sidebar-item settings-sidebar-item"
                        title="Settings"
                        aria-haspopup="dialog"
                        aria-expanded=move || settings_open.get()
                        on:click=move |_| settings_open.set(true)
                    >
                        {icons::settings()}<span>"Settings"</span>
                    </button>
                </aside>

                <section
                    class="results-panel"
                    class:resizing-columns=move || columns.is_resizing()
                    style=move || columns.layout_style(viewport.grid_width.get())
                >
                    <ColumnHeaders columns sort />

                    <div
                        class="results-scroll"
                        node_ref=list_ref
                        tabindex="0"
                        role="grid"
                        aria-label="Search results"
                        on:scroll=on_scroll
                        on:click=move |_| {
                            selection.clear();
                            menu.close();
                        }
                    >
                        <div class="virtual-canvas" style:height=move || format!("{}px", total.get() as f64 * RESULT_ROW_HEIGHT)>
                            {move || {
                                let start = visible_start.get();
                                let end = visible_end.get();
                                (start..end)
                                    .map(|index| {
                                        let maybe_item = results.item_at(index);
                                        match maybe_item {
                                            Some(item) => {
                                                let item_for_double = item.clone();
                                                let item_for_context = item.clone();
                                                view! {
                                                    <div
                                                        class="result-row"
                                                        class:selected=move || selected.with(|selection| selection.contains(index))
                                                        class:focused=move || focused_index.get() == Some(index)
                                                        style:transform=format!("translateY({}px)", index as f64 * RESULT_ROW_HEIGHT)
                                                        on:click=move |event: MouseEvent| {
                                                            event.stop_propagation();
                                                            selection.select_row(index, selection_modifiers(&event));
                                                            if let Some(list) = list_ref.get() { let _ = list.focus(); }
                                                            menu.close();
                                                        }
                                                        on:dblclick=move |_| {
                                                            files.open(item_for_double.full_path.clone());
                                                        }
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
                                                        <div class="cell col-name">
                                                            <FileIcon path=item.full_path.clone() />
                                                            <span class="file-name" title=item.name.clone()>{item.name.clone()}</span>
                                                        </div>
                                                        <div class="cell col-path" title=item.parent_path.clone()>{item.parent_path.clone()}</div>
                                                        <div class="cell col-size">{formatting::file_size(item.size, item.is_dir)}</div>
                                                        <div class="cell col-date">{formatting::modified_date(item.modified_unix)}</div>
                                                    </div>
                                                }.into_any()
                                            }
                                            None => view! {
                                                <div class="result-row skeleton-row" style:transform=format!("translateY({}px)", index as f64 * RESULT_ROW_HEIGHT)>
                                                    <div class="cell col-name"><span class="skeleton icon-skeleton"></span><span class="skeleton text-skeleton"></span></div>
                                                    <div class="cell col-path"><span class="skeleton path-skeleton"></span></div>
                                                    <div class="cell col-size"><span class="skeleton size-skeleton"></span></div>
                                                    <div class="cell col-date"><span class="skeleton date-skeleton"></span></div>
                                                </div>
                                            }.into_any(),
                                        }
                                    })
                                    .collect_view()
                            }}
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
                    <div class="error-banner" role="alert">
                        <span>{message}</span>
                        <button class="banner-close" title="Close" aria-label="Close" on:click=move |_| search_error.set(None)>{icons::close()}</button>
                    </div>
                })}
            </Show>

                        <Show when=move || error.get().is_some()>
                            <div class="error-banner" role="alert">
                                {icons::warning()}
                                <div><strong>"An operation failed"</strong><span>{move || error.get().unwrap_or_default()}</span></div>
                                <button class="banner-close" title="Close" aria-label="Close" on:click=move |_| error.set(None)>{icons::close()}</button>
                            </div>
                        </Show>
                    </div>

                    <footer class="statusbar">
                        <span>{move || formatting::result_count(total.get())}</span>
                        <span class="status-separator"></span>
                        <span>{move || format!("{} selected", selected.with(IndexSelection::count))}</span>
                        <Show when=move || !engine_available.get()>
                            <span class="status-separator"></span>
                            <span class="connection-warning" title=move || engine_message.get()>"Everything unavailable"</span>
                        </Show>
                        <span class="statusbar-spacer"></span>
                        <Show when=move || loading.get()><span class="loading-indicator"></span><span>"Searching…"</span></Show>
                    </footer>
                </section>
            </div>

            <Show when=move || context_menu.get().is_some()>
                {move || context_menu.get().map(|menu| view! {
                    <div class="context-menu" style:left=format!("{}px", menu.x) style:top=format!("{}px", menu.y) on:click=move |event| event.stop_propagation()>
                        <ContextAction icon=icons::open() label="Open" shortcut="Enter" on_click={
                            let path = menu.item.full_path.clone();
                            move || { files.open(path.clone()); context_menu.set(None); }
                        } />
                        <ContextAction icon=icons::folder_open() label="Show in Explorer" shortcut="" on_click={
                            let path = menu.item.full_path.clone();
                            move || { files.reveal(path.clone()); context_menu.set(None); }
                        } />
                        <div class="context-separator"></div>
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
                        <div class="context-separator"></div>
                        <ContextAction danger=true icon=icons::trash() label="Move to Recycle Bin" shortcut="Del" on_click=move || { files.begin_trash(selection, results); context_menu.set(None); } />
                    </div>
                })}
            </Show>

            <Show when=move || settings_open.get()>
                <div class="modal-backdrop" on:click=move |_| settings_open.set(false)>
                    <div
                        class="modal-card settings-modal"
                        role="dialog"
                        aria-modal="true"
                        aria-labelledby="settings-title"
                        on:click=move |event| event.stop_propagation()
                    >
                        <h2 id="settings-title">"Settings"</h2>
                        <section class="settings-section" data-setting="theme" aria-labelledby="theme-setting-title">
                            <h3 id="theme-setting-title">"Theme"</h3>
                            <div class="settings-control" data-settings-control="theme">
                                <ThemeSetting state=theme />
                            </div>
                        </section>
                        <section class="settings-section" data-setting="excluded-folders" aria-labelledby="excluded-folders-setting-title">
                            <h3 id="excluded-folders-setting-title">"Excluded folders"</h3>
                            <div class="settings-control" data-settings-control="excluded-folders">
                                <ExcludedFoldersSetting state=excluded_folders />
                            </div>
                        </section>
                        <div class="modal-actions">
                            <button
                                type="button"
                                class="dialog-button"
                                autofocus
                                on:click=move |_| settings_open.set(false)
                            >
                                "Close"
                            </button>
                        </div>
                    </div>
                </div>
            </Show>

            <Show when=move || rename_target.get().is_some()>
                {move || rename_target.get().map(|item| view! {
                    <div class="modal-backdrop" on:click=move |_| files.cancel_rename()>
                        <div class="modal-card" role="dialog" aria-modal="true" aria-label="Rename" on:click=move |event| event.stop_propagation()>
                            <h2>"Rename"</h2>
                            <p class="modal-description">{item.full_path.clone()}</p>
                            <input
                                class="modal-input"
                                type="text"
                                prop:value=move || rename_value.get()
                                on:input=move |event| {
                                    if let Some(input) = event.target().and_then(|target| target.dyn_into::<HtmlInputElement>().ok()) {
                                        rename_value.set(input.value());
                                    }
                                }
                                on:keydown=move |event: KeyboardEvent| {
                                    match event.key().as_str() {
                                        "Enter" => {
                                            event.prevent_default();
                                            files.submit_rename(results);
                                        }
                                        "Escape" => {
                                            event.prevent_default();
                                            files.cancel_rename();
                                        }
                                        _ => {}
                                    }
                                }
                                autofocus
                            />
                            <div class="modal-actions">
                                <button class="dialog-button" on:click=move |_| files.cancel_rename()>"Cancel"</button>
                                <button class="dialog-button primary" on:click=move |_| files.submit_rename(results)>"Rename"</button>
                            </div>
                        </div>
                    </div>
                })}
            </Show>

            <Show when=move || files.trash_is_preparing()>
                <div class="modal-backdrop">
                    <div class="modal-card" role="status" aria-live="polite">
                        <h2>"Preparing deletion…"</h2>
                        <p>"Everything Modern is capturing an immutable list of the selected files."</p>
                    </div>
                </div>
            </Show>

            <Show when=move || files.pending_trash().is_some()>
                {move || files.pending_trash().map(|pending| view! {
                    <div class="modal-backdrop">
                        <div class="modal-card" role="alertdialog" aria-modal="true" aria-label="Confirmation de suppression" on:click=move |event| event.stop_propagation()>
                            <h2>"Move to Recycle Bin?"</h2>
                            <p>{format!("{} item(s) will be moved to the Recycle Bin.", pending.count)}</p>
                            <div class="modal-actions">
                                <button class="dialog-button" disabled=move || files.trash_is_deleting() on:click=move |_| files.cancel_trash()>"Cancel"</button>
                                <button class="dialog-button danger" disabled=move || files.trash_is_deleting() on:click=move |_| files.submit_trash(selection, results)>
                                    {move || if files.trash_is_deleting() { "Deleting…" } else { "Move to Recycle Bin" }}
                                </button>
                            </div>
                        </div>
                    </div>
                })}
            </Show>
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
            class="sidebar-item"
            class:active=move || query.get() == item_query
            on:click=move |_| query.set(item_query.into())
        >
            {icon}<span>{label}</span>
        </button>
    }
}

#[component]
fn EmptyState(icon: AnyView, title: &'static str, message: &'static str) -> impl IntoView {
    view! { <div class="empty-state">{icon}<h2>{title}</h2><p>{message}</p></div> }
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
        <button class="context-action" class:danger=danger on:click=move |_| on_click()>
            {icon}<span>{label}</span><kbd>{shortcut}</kbd>
        </button>
    }
}
fn selection_modifiers(event: &MouseEvent) -> SelectionModifiers {
    SelectionModifiers {
        extend: event.shift_key(),
        preserve: event.ctrl_key(),
    }
}

fn move_selection_focus(
    movement: FocusMove,
    event: &KeyboardEvent,
    selection: ResultSelection,
    results: SearchResults,
    viewport: ResultViewport,
) {
    let modifiers = SelectionModifiers {
        extend: event.shift_key(),
        preserve: event.ctrl_key(),
    };
    if let Some(index) = selection.focus(movement, modifiers, results.total.get_untracked()) {
        viewport.scroll_row_into_view(index);
    }
}
