use crate::api;
use everything_core::{
    IndexSelection, QueryRequest, SearchResult, SortColumn, SortDirection, SortSpec,
};
use gloo_timers::future::TimeoutFuture;
use leptos::prelude::*;
use leptos::task::spawn_local;
use std::collections::{BTreeMap, HashSet};
use wasm_bindgen::JsCast;
use web_sys::{Element, HtmlDivElement, HtmlInputElement, KeyboardEvent, MouseEvent, PointerEvent};

const ROW_HEIGHT: f64 = 34.0;
const PAGE_SIZE: u32 = 256;
const PAGE_CACHE_LIMIT: usize = 8;
const OVERSCAN: u32 = 8;
const AUDIO_QUERY: &str = "ext:mp3;wav;flac;m4a;m4b;aac;ogg;opus;wma;aif;aiff;ape;mid;midi";

#[derive(Clone)]
struct ContextMenuState {
    x: i32,
    y: i32,
    item: SearchResult,
}

#[derive(Clone, Copy)]
enum FocusMove {
    Relative(i32),
    Absolute(u32),
}

#[derive(Clone)]
struct TrashPending {
    count: usize,
    snapshot_id: u64,
}

#[derive(Clone, Copy)]
struct ColumnWidths {
    name: f64,
    path: f64,
    size: f64,
    date: f64,
}

#[derive(Clone, Copy)]
enum ColumnBoundary {
    NamePath,
    PathSize,
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

#[component]
pub fn App() -> impl IntoView {
    let query = RwSignal::new(String::new());
    let generation = RwSignal::new(0_u32);
    let refresh_token = RwSignal::new(0_u32);
    let pages = RwSignal::new(BTreeMap::<u32, Vec<SearchResult>>::new());
    let loading_pages = RwSignal::new(HashSet::<(u32, u32)>::new());
    let total = RwSignal::new(0_u32);
    let scroll_top = RwSignal::new(0_f64);
    let viewport_height = RwSignal::new(640_f64);
    let results_grid_width = RwSignal::new(0_f64);
    let sort = RwSignal::new(SortSpec::default());
    let loading = RwSignal::new(false);
    let render_latency_ms = RwSignal::new(None::<f64>);
    let search_error = RwSignal::new(None::<String>);
    let error = RwSignal::new(None::<String>);
    let engine_message = RwSignal::new("Connexion à Everything…".to_string());
    let engine_available = RwSignal::new(false);
    let selected = RwSignal::new(IndexSelection::default());
    let focused_index = RwSignal::new(None::<u32>);
    let selection_anchor = RwSignal::new(None::<u32>);
    let rename_target = RwSignal::new(None::<SearchResult>);
    let rename_value = RwSignal::new(String::new());
    let trash_pending = RwSignal::new(None::<TrashPending>);
    let trash_preparing = RwSignal::new(false);
    let trash_in_flight = RwSignal::new(false);
    let context_menu = RwSignal::new(None::<ContextMenuState>);
    let column_widths = RwSignal::new(None::<ColumnWidths>);
    let column_resize = RwSignal::new(None::<ColumnResize>);
    let search_ref = NodeRef::<leptos::html::Input>::new();
    let list_ref = NodeRef::<leptos::html::Div>::new();
    let column_header_ref = NodeRef::<leptos::html::Div>::new();

    spawn_local(async move {
        loop {
            let status = api::status().await;
            engine_available.set(status.available);
            engine_message.set(status.message);
            TimeoutFuture::new(3_000).await;
        }
    });

    // Le WebView ne déclenche pas forcément un événement de scroll lors d’un
    // redimensionnement. Cette mesure légère maintient la fenêtre virtualisée
    // alignée sur la hauteur réellement visible.
    spawn_local(async move {
        loop {
            if let Some(list) = list_ref.get() {
                let height = list.client_height() as f64;
                if (height - viewport_height.get_untracked()).abs() > 0.5 {
                    viewport_height.set(height);
                }
                let width = list
                    .query_selector(".virtual-canvas")
                    .ok()
                    .flatten()
                    .map(|canvas| canvas.get_bounding_client_rect().width())
                    .unwrap_or_else(|| list.client_width() as f64);
                if (width - results_grid_width.get_untracked()).abs() > 0.5 {
                    results_grid_width.set(width);
                }
            }
            TimeoutFuture::new(120).await;
        }
    });

    Effect::new(move |_| {
        let current_query = query.get();
        let current_sort = sort.get();
        let _refresh = refresh_token.get();
        let next_generation = generation.get_untracked().saturating_add(1);
        generation.set(next_generation);
        spawn_local(async move {
            api::begin_generation(next_generation).await;
        });
        pages.set(BTreeMap::new());
        loading_pages.set(HashSet::new());
        total.set(0);
        render_latency_ms.set(None);
        selected.set(IndexSelection::default());
        focused_index.set(None);
        rename_target.set(None);
        if let Some(pending) = trash_pending.get_untracked() {
            spawn_local(async move {
                api::cancel_trash(pending.snapshot_id).await;
            });
        }
        trash_pending.set(None);
        search_error.set(None);
        scroll_top.set(0.0);
        if let Some(list) = list_ref.get() {
            list.set_scroll_top(0);
            viewport_height.set(list.client_height() as f64);
        }

        if current_query.trim().is_empty() {
            loading.set(false);
            return;
        }

        loading.set(true);
        spawn_local(async move {
            TimeoutFuture::new(55).await;
            if generation.get_untracked() != next_generation {
                return;
            }
            request_page(
                current_query,
                0,
                current_sort,
                next_generation,
                generation,
                pages,
                loading_pages,
                total,
                loading,
                render_latency_ms,
                search_error,
            );
        });
    });

    let visible_start = Memo::new(move |_| {
        ((scroll_top.get() / ROW_HEIGHT).floor() as u32).saturating_sub(OVERSCAN)
    });
    let visible_end = Memo::new(move |_| {
        let count = (viewport_height.get() / ROW_HEIGHT).ceil() as u32 + OVERSCAN * 2;
        visible_start.get().saturating_add(count).min(total.get())
    });

    Effect::new(move |_| {
        let start = visible_start.get();
        let end = visible_end.get();
        let current_query = query.get();
        if current_query.trim().is_empty() || end == 0 {
            return;
        }
        let current_generation = generation.get();
        let current_sort = sort.get();
        let first_page = start / PAGE_SIZE;
        let last_page = end.saturating_sub(1) / PAGE_SIZE;
        let max_page = total.get().saturating_sub(1) / PAGE_SIZE;
        for page in first_page.saturating_sub(1)..=last_page.saturating_add(1).min(max_page) {
            request_page(
                current_query.clone(),
                page,
                current_sort,
                current_generation,
                generation,
                pages,
                loading_pages,
                total,
                loading,
                render_latency_ms,
                search_error,
            );
        }
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
        if let Some(element) = event
            .target()
            .and_then(|target| target.dyn_into::<HtmlDivElement>().ok())
        {
            scroll_top.set(element.scroll_top() as f64);
            viewport_height.set(element.client_height() as f64);
            context_menu.set(None);
        }
    };

    let on_keydown = move |event: KeyboardEvent| {
        let key = event.key();
        if event.ctrl_key() && key.eq_ignore_ascii_case("l") {
            event.prevent_default();
            if let Some(input) = search_ref.get() {
                let _ = input.focus();
                input.select();
            }
            return;
        }

        // Les raccourcis de la liste ne doivent jamais détourner les touches
        // utilisées dans le champ de recherche ou sur un bouton.
        if is_interactive_target(&event) {
            return;
        }

        if event.ctrl_key() && key.eq_ignore_ascii_case("a") {
            event.prevent_default();
            let count = total.get_untracked();
            selected.update(|selection| selection.select_all(count));
            if count > 0 && focused_index.get_untracked().is_none() {
                focused_index.set(Some(0));
                selection_anchor.set(Some(0));
            }
            return;
        }

        let page_step = ((viewport_height.get_untracked() / ROW_HEIGHT).floor() as i32).max(1);
        match key.as_str() {
            "ArrowDown" => {
                event.prevent_default();
                move_focus(
                    FocusMove::Relative(1),
                    event.shift_key(),
                    event.ctrl_key(),
                    total,
                    focused_index,
                    selection_anchor,
                    selected,
                    list_ref,
                );
            }
            "ArrowUp" => {
                event.prevent_default();
                move_focus(
                    FocusMove::Relative(-1),
                    event.shift_key(),
                    event.ctrl_key(),
                    total,
                    focused_index,
                    selection_anchor,
                    selected,
                    list_ref,
                );
            }
            "PageDown" => {
                event.prevent_default();
                move_focus(
                    FocusMove::Relative(page_step),
                    event.shift_key(),
                    event.ctrl_key(),
                    total,
                    focused_index,
                    selection_anchor,
                    selected,
                    list_ref,
                );
            }
            "PageUp" => {
                event.prevent_default();
                move_focus(
                    FocusMove::Relative(-page_step),
                    event.shift_key(),
                    event.ctrl_key(),
                    total,
                    focused_index,
                    selection_anchor,
                    selected,
                    list_ref,
                );
            }
            "Home" => {
                event.prevent_default();
                move_focus(
                    FocusMove::Absolute(0),
                    event.shift_key(),
                    event.ctrl_key(),
                    total,
                    focused_index,
                    selection_anchor,
                    selected,
                    list_ref,
                );
            }
            "End" => {
                event.prevent_default();
                let last = total.get_untracked().saturating_sub(1);
                move_focus(
                    FocusMove::Absolute(last),
                    event.shift_key(),
                    event.ctrl_key(),
                    total,
                    focused_index,
                    selection_anchor,
                    selected,
                    list_ref,
                );
            }
            " " if event.ctrl_key() => {
                event.prevent_default();
                if let Some(index) = focused_index.get_untracked() {
                    selected.update(|selection| selection.toggle(index));
                    selection_anchor.set(Some(index));
                }
            }
            "Enter" => {
                event.prevent_default();
                if let Some(item) = focused_item(focused_index, pages) {
                    let error = error;
                    spawn_local(async move {
                        if let Err(message) = api::open(&item.full_path).await {
                            error.set(Some(message));
                        }
                    });
                }
            }
            "Delete" => {
                event.prevent_default();
                begin_trash(
                    selected,
                    query,
                    sort,
                    generation,
                    trash_pending,
                    trash_preparing,
                    error,
                );
            }
            "F2" => {
                event.prevent_default();
                if let Some(item) = focused_item(focused_index, pages) {
                    begin_rename(item, rename_target, rename_value);
                }
            }
            "ContextMenu" => {
                event.prevent_default();
                open_keyboard_context_menu(
                    focused_index,
                    pages,
                    selected,
                    selection_anchor,
                    list_ref,
                    context_menu,
                );
            }
            "F10" if event.shift_key() => {
                event.prevent_default();
                open_keyboard_context_menu(
                    focused_index,
                    pages,
                    selected,
                    selection_anchor,
                    list_ref,
                    context_menu,
                );
            }
            "Escape" => {
                if context_menu.get_untracked().is_some() {
                    context_menu.set(None);
                } else {
                    selected.set(IndexSelection::default());
                    selection_anchor.set(None);
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
            on:click=move |_| context_menu.set(None)
            on:pointermove=move |event| update_column_resize(event, column_widths, column_resize)
            on:pointerup=move |event| finish_column_resize(event, column_resize)
            on:pointercancel=move |event| finish_column_resize(event, column_resize)
        >
            <header class="titlebar" data-tauri-drag-region on:dblclick=move |_| api::toggle_maximize_window()>
                <div class="app-mark" aria-hidden="true">"E"</div>
                <div class="app-title" data-tauri-drag-region>"Everything Modern"</div>
                <div class="titlebar-spacer" data-tauri-drag-region></div>
                <div class="window-controls" on:dblclick=move |event| event.stop_propagation()>
                    <button class="window-control" title="Réduire" aria-label="Réduire" on:click=move |event| { event.stop_propagation(); api::minimize_window(); }>{icon_minimize()}</button>
                    <button class="window-control" title="Agrandir ou restaurer" aria-label="Agrandir ou restaurer" on:click=move |event| { event.stop_propagation(); api::toggle_maximize_window(); }>{icon_maximize()}</button>
                    <button class="window-control close" title="Fermer" aria-label="Fermer" on:click=move |event| { event.stop_propagation(); api::close_window(); }>{icon_close()}</button>
                </div>
            </header>

            <div class="search-toolbar" role="search">
                <div class="search-box">
                    {icon_search()}
                    <input
                        node_ref=search_ref
                        type="search"
                        placeholder="Rechercher dans Everything"
                        prop:value=move || query.get()
                        on:input=on_search_input
                        autofocus
                    />
                    <kbd>"Ctrl L"</kbd>
                </div>
            </div>

            <section class="command-bar" aria-label="Commandes">
                <button class="command-button" title="Ouvrir" on:click=move |_| {
                    if let Some(item) = focused_item(focused_index, pages) {
                        open_item(item.full_path, error);
                    }
                }>{icon_open()}<span>"Ouvrir"</span></button>
                <button class="command-button" title="Afficher dans l’Explorateur" on:click=move |_| {
                    if let Some(item) = focused_item(focused_index, pages) {
                        reveal_item(item.full_path, error);
                    }
                }>{icon_folder_open()}<span>"Afficher dans l’Explorateur"</span></button>
                <span class="command-separator"></span>
                <button class="command-button danger-hover" title="Mettre à la Corbeille" on:click=move |_| begin_trash(selected, query, sort, generation, trash_pending, trash_preparing, error)>{icon_trash()}<span>"Supprimer"</span></button>
            </section>

            <div class="workspace">
                <aside class="sidebar">
                    <SidebarItem label="Tous les fichiers" icon=icon_home() active=move || query.get() == "*" on_click=move || query.set("*".into()) />
                    <SidebarItem label="Modifiés aujourd’hui" icon=icon_clock() active=move || query.get() == "dm:today" on_click=move || query.set("dm:today".into()) />
                    <div class="sidebar-separator"></div>
                    <div class="sidebar-section-label">"Types"</div>
                    <SidebarItem label="Documents" icon=icon_document() active=move || query.get() == "ext:pdf;doc;docx;xls;xlsx;ppt;pptx;md;txt" on_click=move || query.set("ext:pdf;doc;docx;xls;xlsx;ppt;pptx;md;txt".into()) />
                    <SidebarItem label="Images" icon=icon_image() active=move || query.get() == "ext:png;jpg;jpeg;webp;gif;svg;avif" on_click=move || query.set("ext:png;jpg;jpeg;webp;gif;svg;avif".into()) />
                    <SidebarItem label="Vidéos" icon=icon_video() active=move || query.get() == "ext:mp4;mkv;avi;mov;webm" on_click=move || query.set("ext:mp4;mkv;avi;mov;webm".into()) />
                    <SidebarItem label="Audio" icon=icon_audio() active=move || query.get() == AUDIO_QUERY on_click=move || query.set(AUDIO_QUERY.into()) />
                    <SidebarItem label="Archives" icon=icon_archive() active=move || query.get() == "ext:zip;7z;rar;tar;gz" on_click=move || query.set("ext:zip;7z;rar;tar;gz".into()) />
                </aside>

                <section
                    class="results-panel"
                    class:resizing-columns=move || column_resize.get().is_some()
                    style=move || column_layout_style(column_widths.get(), results_grid_width.get())
                >
                    <div class="column-header" node_ref=column_header_ref>
                        <div class="column-heading col-name">
                            <SortHeader label="Nom" column=SortColumn::Name sort=sort />
                            <ColumnResizer boundary=ColumnBoundary::NamePath header_ref=column_header_ref widths=column_widths resizing=column_resize />
                        </div>
                        <div class="column-heading col-path">
                            <SortHeader label="Chemin" column=SortColumn::Path sort=sort />
                            <ColumnResizer boundary=ColumnBoundary::PathSize header_ref=column_header_ref widths=column_widths resizing=column_resize />
                        </div>
                        <div class="column-heading col-size">
                            <SortHeader label="Taille" column=SortColumn::Size sort=sort />
                            <ColumnResizer boundary=ColumnBoundary::SizeDate header_ref=column_header_ref widths=column_widths resizing=column_resize />
                        </div>
                        <div class="column-heading col-date">
                            <SortHeader label="Modifié le" column=SortColumn::Modified sort=sort />
                        </div>
                    </div>

                    <div
                        class="results-scroll"
                        node_ref=list_ref
                        tabindex="0"
                        role="grid"
                        aria-label="Résultats de recherche"
                        on:scroll=on_scroll
                        on:click=move |_| {
                            selected.set(IndexSelection::default());
                            focused_index.set(None);
                            selection_anchor.set(None);
                            context_menu.set(None);
                        }
                    >
                        <div class="virtual-canvas" style:height=move || format!("{}px", total.get() as f64 * ROW_HEIGHT)>
                            {move || {
                                let start = visible_start.get();
                                let end = visible_end.get();
                                (start..end)
                                    .map(|index| {
                                        let maybe_item = item_at(index, pages);
                                        match maybe_item {
                                            Some(item) => {
                                                let item_for_double = item.clone();
                                                let item_for_context = item.clone();
                                                view! {
                                                    <div
                                                        class="result-row"
                                                        class:selected=move || selected.with(|selection| selection.contains(index))
                                                        class:focused=move || focused_index.get() == Some(index)
                                                        style:transform=format!("translateY({}px)", index as f64 * ROW_HEIGHT)
                                                        on:click=move |event: MouseEvent| {
                                                            event.stop_propagation();
                                                            select_row(
                                                                index,
                                                                &event,
                                                                selected,
                                                                focused_index,
                                                                selection_anchor,
                                                            );
                                                            if let Some(list) = list_ref.get() { let _ = list.focus(); }
                                                            context_menu.set(None);
                                                        }
                                                        on:dblclick=move |_| {
                                                            open_item(item_for_double.full_path.clone(), error);
                                                        }
                                                        on:contextmenu=move |event: MouseEvent| {
                                                            event.prevent_default();
                                                            event.stop_propagation();
                                                            selected.update(|selection| {
                                                                if !selection.contains(index) {
                                                                    selection.select_only(index);
                                                                }
                                                            });
                                                            focused_index.set(Some(index));
                                                            selection_anchor.set(Some(index));
                                                            let (x, y) = clamp_context_position(event.client_x(), event.client_y());
                                                            context_menu.set(Some(ContextMenuState {
                                                                x,
                                                                y,
                                                                item: item_for_context.clone(),
                                                            }));
                                                        }
                                                    >
                                                        <div class="cell col-name">
                                                            <FileIcon path=item.full_path.clone() is_dir=item.is_dir />
                                                            <span class="file-name" title=item.name.clone()>{item.name.clone()}</span>
                                                        </div>
                                                        <div class="cell col-path" title=item.parent_path.clone()>{item.parent_path.clone()}</div>
                                                        <div class="cell col-size">{format_size(item.size, item.is_dir)}</div>
                                                        <div class="cell col-date">{format_date(item.modified_unix)}</div>
                                                    </div>
                                                }.into_any()
                                            }
                                            None => view! {
                                                <div class="result-row skeleton-row" style:transform=format!("translateY({}px)", index as f64 * ROW_HEIGHT)>
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
                                icon=icon_search_large()
                                title="Commencez à taper"
                                message="Toutes les fonctions et opérateurs de recherche Everything sont acceptés."
                            />
                        </Show>
                        <Show when=move || !query.get().trim().is_empty() && !loading.get() && total.get() == 0 && search_error.get().is_none()>
                            <EmptyState
                                icon=icon_empty()
                                title="Aucun résultat"
                                message="Essayez une recherche moins restrictive ou vérifiez la syntaxe."
                            />
                        </Show>
                        <Show when=move || search_error.get().is_some()>
                {move || search_error.get().map(|message| view! {
                    <div class="error-banner" role="alert">
                        <span>{message}</span>
                        <button class="banner-close" title="Fermer" aria-label="Fermer" on:click=move |_| search_error.set(None)>{icon_close()}</button>
                    </div>
                })}
            </Show>

            <Show when=move || error.get().is_some()>
                            <div class="error-banner" role="alert">
                                {icon_warning()}
                                <div><strong>"Une opération a échoué"</strong><span>{move || error.get().unwrap_or_default()}</span></div>
                                <button class="banner-close" title="Fermer" aria-label="Fermer" on:click=move |_| error.set(None)>{icon_close()}</button>
                            </div>
                        </Show>
                    </div>

                    <footer class="statusbar">
                        <span>{move || format_result_count(total.get())}</span>
                        <span class="status-separator"></span>
                        <span>{move || format!("{} sélectionné(s)", selected.with(IndexSelection::count))}</span>
                        <Show when=move || !engine_available.get()>
                            <span class="status-separator"></span>
                            <span class="connection-warning" title=move || engine_message.get()>"Everything indisponible"</span>
                        </Show>
                        <span class="statusbar-spacer"></span>
                        <Show when=move || loading.get()><span class="loading-indicator"></span><span>"Recherche…"</span></Show>
                    </footer>
                </section>
            </div>

            <Show when=move || context_menu.get().is_some()>
                {move || context_menu.get().map(|menu| view! {
                    <div class="context-menu" style:left=format!("{}px", menu.x) style:top=format!("{}px", menu.y) on:click=move |event| event.stop_propagation()>
                        <ContextAction icon=icon_open() label="Ouvrir" shortcut="Entrée" on_click={
                            let path = menu.item.full_path.clone();
                            move || { context_menu.set(None); open_item(path.clone(), error); }
                        } />
                        <ContextAction icon=icon_folder_open() label="Afficher dans l’Explorateur" shortcut="" on_click={
                            let path = menu.item.full_path.clone();
                            move || { context_menu.set(None); reveal_item(path.clone(), error); }
                        } />
                        <div class="context-separator"></div>
                        <ContextAction icon=icon_copy() label="Copier le nom" shortcut="" on_click={
                            let name = menu.item.name.clone();
                            move || { copy_text(name.clone(), error); context_menu.set(None); }
                        } />
                        <ContextAction icon=icon_copy() label="Copier le chemin" shortcut="" on_click={
                            let path = menu.item.parent_path.clone();
                            move || { copy_text(path.clone(), error); context_menu.set(None); }
                        } />
                        <ContextAction icon=icon_copy() label="Copier le chemin complet" shortcut="" on_click={
                            let path = menu.item.full_path.clone();
                            move || { copy_text(path.clone(), error); context_menu.set(None); }
                        } />
                        <ContextAction icon=icon_edit() label="Renommer" shortcut="F2" on_click={
                            let item = menu.item.clone();
                            move || { context_menu.set(None); begin_rename(item.clone(), rename_target, rename_value); }
                        } />
                        <div class="context-separator"></div>
                        <ContextAction danger=true icon=icon_trash() label="Mettre à la Corbeille" shortcut="Suppr" on_click=move || { context_menu.set(None); begin_trash(selected, query, sort, generation, trash_pending, trash_preparing, error); } />
                    </div>
                })}
            </Show>

            <Show when=move || rename_target.get().is_some()>
                {move || rename_target.get().map(|item| view! {
                    <div class="modal-backdrop" on:click=move |_| rename_target.set(None)>
                        <div class="modal-card" role="dialog" aria-modal="true" aria-label="Renommer" on:click=move |event| event.stop_propagation()>
                            <h2>"Renommer"</h2>
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
                                            submit_rename(rename_target, rename_value, refresh_token, error);
                                        }
                                        "Escape" => {
                                            event.prevent_default();
                                            rename_target.set(None);
                                        }
                                        _ => {}
                                    }
                                }
                                autofocus
                            />
                            <div class="modal-actions">
                                <button class="dialog-button" on:click=move |_| rename_target.set(None)>"Annuler"</button>
                                <button class="dialog-button primary" on:click=move |_| submit_rename(rename_target, rename_value, refresh_token, error)>"Renommer"</button>
                            </div>
                        </div>
                    </div>
                })}
            </Show>

            <Show when=move || trash_preparing.get()>
                <div class="modal-backdrop">
                    <div class="modal-card" role="status" aria-live="polite">
                        <h2>"Préparation de la suppression…"</h2>
                        <p>"Everything Modern capture une liste immuable des fichiers sélectionnés."</p>
                    </div>
                </div>
            </Show>

            <Show when=move || trash_pending.get().is_some()>
                {move || trash_pending.get().map(|pending| view! {
                    <div class="modal-backdrop">
                        <div class="modal-card" role="alertdialog" aria-modal="true" aria-label="Confirmation de suppression" on:click=move |event| event.stop_propagation()>
                            <h2>"Mettre à la Corbeille ?"</h2>
                            <p>{format!("{} élément(s) seront déplacés vers la Corbeille.", pending.count)}</p>
                            <div class="modal-actions">
                                <button class="dialog-button" disabled=move || trash_in_flight.get() on:click=move |_| cancel_trash(trash_pending)>"Annuler"</button>
                                <button class="dialog-button danger" disabled=move || trash_in_flight.get() on:click=move |_| submit_trash(trash_pending, selected, refresh_token, error, trash_in_flight)>
                                    {move || if trash_in_flight.get() { "Suppression…" } else { "Mettre à la Corbeille" }}
                                </button>
                            </div>
                        </div>
                    </div>
                })}
            </Show>
        </main>
    }
}

fn request_page(
    query: String,
    page_index: u32,
    sort: SortSpec,
    request_generation: u32,
    generation: RwSignal<u32>,
    pages: RwSignal<BTreeMap<u32, Vec<SearchResult>>>,
    loading_pages: RwSignal<HashSet<(u32, u32)>>,
    total: RwSignal<u32>,
    loading: RwSignal<bool>,
    render_latency_ms: RwSignal<Option<f64>>,
    search_error: RwSignal<Option<String>>,
) {
    let loading_key = (request_generation, page_index);
    if pages.with_untracked(|cache| cache.contains_key(&page_index))
        || loading_pages.with_untracked(|set| set.contains(&loading_key))
    {
        return;
    }
    loading_pages.update(|set| {
        set.insert(loading_key);
    });
    loading.set(true);

    spawn_local(async move {
        let request = QueryRequest {
            query,
            offset: page_index.saturating_mul(PAGE_SIZE),
            limit: PAGE_SIZE,
            sort,
            request_id: request_generation,
        };
        let result = api::search(request).await;
        loading_pages.update(|set| {
            set.remove(&loading_key);
        });

        if generation.get_untracked() != request_generation {
            return;
        }

        match result {
            Ok(page) => {
                let received_at = js_sys::Date::now();
                total.set(page.total);
                pages.update(|cache| {
                    cache.insert(page_index, page.items);
                    while cache.len() > PAGE_CACHE_LIMIT {
                        if let Some(key) = cache
                            .keys()
                            .copied()
                            .max_by_key(|key| key.abs_diff(page_index))
                        {
                            cache.remove(&key);
                        } else {
                            break;
                        }
                    }
                });
                record_next_frame_latency(received_at, render_latency_ms);
                search_error.set(None);
            }
            Err(message) if message.contains("obsolète") => {}
            Err(message) => search_error.set(Some(message)),
        }
        loading.set(loading_pages.with_untracked(|set| {
            set.iter()
                .any(|(request_id, _)| *request_id == request_generation)
        }));
    });
}

#[component]
fn SidebarItem<F, A>(label: &'static str, icon: AnyView, active: F, on_click: A) -> impl IntoView
where
    F: Fn() -> bool + Send + Sync + 'static,
    A: Fn() + Send + Sync + 'static,
{
    view! {
        <button class="sidebar-item" class:active=active on:click=move |_| on_click()>
            {icon}<span>{label}</span>
        </button>
    }
}

#[component]
fn SortHeader(label: &'static str, column: SortColumn, sort: RwSignal<SortSpec>) -> impl IntoView {
    let column_for_click = column;
    let column_for_active = column;
    view! {
        <button class="column-button" on:click=move |_| {
            sort.update(|current| {
                if current.column == column_for_click {
                    current.direction = current.direction.toggle();
                } else {
                    current.column = column_for_click;
                    current.direction = SortDirection::Ascending;
                }
            });
        }>
            <span>{label}</span>
            <span class="sort-arrow" class:visible=move || sort.get().column == column_for_active>
                {move || if sort.get().direction == SortDirection::Ascending { "↑" } else { "↓" }}
            </span>
        </button>
    }
}

#[component]
fn ColumnResizer(
    boundary: ColumnBoundary,
    header_ref: NodeRef<leptos::html::Div>,
    widths: RwSignal<Option<ColumnWidths>>,
    resizing: RwSignal<Option<ColumnResize>>,
) -> impl IntoView {
    view! {
        <span
            class="column-resizer"
            role="separator"
            aria-orientation="vertical"
            on:pointerdown=move |event| begin_column_resize(event, boundary, header_ref, widths, resizing)
            on:click=move |event| event.stop_propagation()
        ></span>
    }
}

#[component]
fn FileIcon(path: String, is_dir: bool) -> impl IntoView {
    let source = RwSignal::new(None::<String>);
    let path_for_effect = path.clone();
    Effect::new(move |_| {
        let path = path_for_effect.clone();
        spawn_local(async move {
            source.set(api::icon(&path).await);
        });
    });

    view! {
        <span class="file-icon" class:folder=is_dir>
            <Show when=move || source.get().is_some() fallback=move || if is_dir { icon_folder() } else { icon_file() }>
                <img src=move || source.get().unwrap_or_default() alt="" />
            </Show>
        </span>
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

fn begin_column_resize(
    event: PointerEvent,
    boundary: ColumnBoundary,
    header_ref: NodeRef<leptos::html::Div>,
    widths: RwSignal<Option<ColumnWidths>>,
    resizing: RwSignal<Option<ColumnResize>>,
) {
    event.prevent_default();
    event.stop_propagation();

    let Some(header) = header_ref.get() else {
        return;
    };
    let Some(measured) = measure_column_widths(&header) else {
        return;
    };
    let total_width = measured.name + measured.path + measured.size + measured.date;
    if total_width <= 0.0 {
        return;
    }

    widths.set(Some(ColumnWidths {
        name: measured.name / total_width * 100.0,
        path: measured.path / total_width * 100.0,
        size: measured.size / total_width * 100.0,
        date: measured.date / total_width * 100.0,
    }));

    let (start_left, start_right) = match boundary {
        ColumnBoundary::NamePath => (measured.name, measured.path),
        ColumnBoundary::PathSize => (measured.path, measured.size),
        ColumnBoundary::SizeDate => (measured.size, measured.date),
    };
    resizing.set(Some(ColumnResize {
        boundary,
        pointer_id: event.pointer_id(),
        start_x: event.client_x() as f64,
        start_left,
        start_right,
        total_width,
    }));

    if let Some(element) = event
        .current_target()
        .and_then(|target| target.dyn_into::<Element>().ok())
    {
        let _ = element.set_pointer_capture(event.pointer_id());
    }
}

fn update_column_resize(
    event: PointerEvent,
    widths: RwSignal<Option<ColumnWidths>>,
    resizing: RwSignal<Option<ColumnResize>>,
) {
    let Some(active) = resizing.get_untracked() else {
        return;
    };
    if active.pointer_id != event.pointer_id() {
        return;
    }
    event.prevent_default();

    let (minimum_left, minimum_right) = match active.boundary {
        ColumnBoundary::NamePath => (180.0, 180.0),
        ColumnBoundary::PathSize => (180.0, 76.0),
        ColumnBoundary::SizeDate => (76.0, 130.0),
    };
    let pair_width = active.start_left + active.start_right;
    if pair_width <= minimum_left + minimum_right {
        return;
    }

    let delta = event.client_x() as f64 - active.start_x;
    let next_left = (active.start_left + delta).clamp(minimum_left, pair_width - minimum_right);
    let next_right = pair_width - next_left;
    let left_percent = next_left / active.total_width * 100.0;
    let right_percent = next_right / active.total_width * 100.0;

    widths.update(|current| {
        let Some(current) = current else {
            return;
        };
        match active.boundary {
            ColumnBoundary::NamePath => {
                current.name = left_percent;
                current.path = right_percent;
            }
            ColumnBoundary::PathSize => {
                current.path = left_percent;
                current.size = right_percent;
            }
            ColumnBoundary::SizeDate => {
                current.size = left_percent;
                current.date = right_percent;
            }
        }
    });
}

fn finish_column_resize(event: PointerEvent, resizing: RwSignal<Option<ColumnResize>>) {
    let Some(active) = resizing.get_untracked() else {
        return;
    };
    if active.pointer_id != event.pointer_id() {
        return;
    }
    event.prevent_default();
    event.stop_propagation();
    resizing.set(None);
}

fn measure_column_widths(header: &HtmlDivElement) -> Option<ColumnWidths> {
    Some(ColumnWidths {
        name: measure_column(header, ".column-heading.col-name")?,
        path: measure_column(header, ".column-heading.col-path")?,
        size: measure_column(header, ".column-heading.col-size")?,
        date: measure_column(header, ".column-heading.col-date")?,
    })
}

fn measure_column(header: &HtmlDivElement, selector: &str) -> Option<f64> {
    header
        .query_selector(selector)
        .ok()
        .flatten()
        .map(|element| element.get_bounding_client_rect().width())
}

fn column_layout_style(widths: Option<ColumnWidths>, grid_width: f64) -> String {
    let mut style = if grid_width > 0.0 {
        format!("--grid-width:{grid_width:.2}px")
    } else {
        String::new()
    };
    if let Some(widths) = widths {
        style.push_str(&format!(
            ";--col-name:{:.4}%;--col-path:{:.4}%;--col-size:{:.4}%;--col-date:{:.4}%",
            widths.name, widths.path, widths.size, widths.date
        ));
    }
    style
}

fn item_at(index: u32, pages: RwSignal<BTreeMap<u32, Vec<SearchResult>>>) -> Option<SearchResult> {
    let page = index / PAGE_SIZE;
    let within = (index % PAGE_SIZE) as usize;
    pages.with(|cache| {
        cache
            .get(&page)
            .and_then(|items| items.get(within))
            .cloned()
    })
}

fn focused_item(
    focused_index: RwSignal<Option<u32>>,
    pages: RwSignal<BTreeMap<u32, Vec<SearchResult>>>,
) -> Option<SearchResult> {
    focused_index
        .get_untracked()
        .and_then(|index| item_at(index, pages))
}

fn select_row(
    index: u32,
    event: &MouseEvent,
    selected: RwSignal<IndexSelection>,
    focused_index: RwSignal<Option<u32>>,
    selection_anchor: RwSignal<Option<u32>>,
) {
    focused_index.set(Some(index));
    if event.shift_key() {
        let anchor = selection_anchor.get_untracked().unwrap_or(index);
        selected.update(|selection| {
            if event.ctrl_key() {
                selection.add_range(anchor, index);
            } else {
                selection.select_range(anchor, index);
            }
        });
    } else if event.ctrl_key() {
        selected.update(|selection| selection.toggle(index));
        selection_anchor.set(Some(index));
    } else {
        selected.update(|selection| selection.select_only(index));
        selection_anchor.set(Some(index));
    }
}

fn move_focus(
    movement: FocusMove,
    extend_selection: bool,
    ctrl_modifier: bool,
    total: RwSignal<u32>,
    focused_index: RwSignal<Option<u32>>,
    selection_anchor: RwSignal<Option<u32>>,
    selected: RwSignal<IndexSelection>,
    list_ref: NodeRef<leptos::html::Div>,
) {
    let count = total.get_untracked();
    if count == 0 {
        return;
    }

    let next = match movement {
        FocusMove::Absolute(index) => index.min(count - 1),
        FocusMove::Relative(delta) => match focused_index.get_untracked() {
            Some(current) => (current as i64 + delta as i64).clamp(0, count as i64 - 1) as u32,
            None if delta < 0 => count - 1,
            None => 0,
        },
    };

    focused_index.set(Some(next));
    if extend_selection {
        let anchor = selection_anchor.get_untracked().unwrap_or(next);
        selected.update(|selection| {
            if ctrl_modifier {
                selection.add_range(anchor, next);
            } else {
                selection.select_range(anchor, next);
            }
        });
    } else if !ctrl_modifier {
        selection_anchor.set(Some(next));
        selected.update(|selection| selection.select_only(next));
    }

    if let Some(list) = list_ref.get() {
        let top = next as f64 * ROW_HEIGHT;
        let bottom = top + ROW_HEIGHT;
        let current_top = list.scroll_top() as f64;
        let current_bottom = current_top + list.client_height() as f64;
        if top < current_top {
            list.set_scroll_top(top as i32);
        } else if bottom > current_bottom {
            list.set_scroll_top((bottom - list.client_height() as f64) as i32);
        }
    }
}

fn is_interactive_target(event: &KeyboardEvent) -> bool {
    event
        .target()
        .and_then(|target| target.dyn_into::<Element>().ok())
        .is_some_and(|element| {
            matches!(
                element.tag_name().as_str(),
                "INPUT" | "TEXTAREA" | "SELECT" | "BUTTON"
            ) || element.get_attribute("contenteditable").as_deref() == Some("true")
        })
}

fn clamp_context_position(x: i32, y: i32) -> (i32, i32) {
    const MENU_WIDTH: i32 = 272;
    const MENU_HEIGHT: i32 = 324;
    const MARGIN: i32 = 8;
    let (width, height) = web_sys::window()
        .map(|window| {
            let width = window
                .inner_width()
                .ok()
                .and_then(|value| value.as_f64())
                .unwrap_or(1280.0) as i32;
            let height = window
                .inner_height()
                .ok()
                .and_then(|value| value.as_f64())
                .unwrap_or(720.0) as i32;
            (width, height)
        })
        .unwrap_or((1280, 720));
    (
        x.clamp(MARGIN, (width - MENU_WIDTH).max(MARGIN)),
        y.clamp(MARGIN, (height - MENU_HEIGHT).max(MARGIN)),
    )
}

fn open_keyboard_context_menu(
    focused_index: RwSignal<Option<u32>>,
    pages: RwSignal<BTreeMap<u32, Vec<SearchResult>>>,
    selected: RwSignal<IndexSelection>,
    selection_anchor: RwSignal<Option<u32>>,
    list_ref: NodeRef<leptos::html::Div>,
    context_menu: RwSignal<Option<ContextMenuState>>,
) {
    let Some(index) = focused_index.get_untracked() else {
        return;
    };
    let Some(item) = item_at(index, pages) else {
        return;
    };
    selected.update(|selection| {
        if !selection.contains(index) {
            selection.select_only(index);
        }
    });
    selection_anchor.set(Some(index));
    let (x, y) = keyboard_context_position(Some(index), list_ref);
    context_menu.set(Some(ContextMenuState { x, y, item }));
}

fn keyboard_context_position(
    focused_index: Option<u32>,
    list_ref: NodeRef<leptos::html::Div>,
) -> (i32, i32) {
    let Some(list) = list_ref.get() else {
        return clamp_context_position(32, 96);
    };
    let rect = list.get_bounding_client_rect();
    let index = focused_index.unwrap_or(0);
    let row_y = rect.top() + index as f64 * ROW_HEIGHT - list.scroll_top() as f64 + ROW_HEIGHT;
    clamp_context_position((rect.left() + 180.0) as i32, row_y as i32)
}

fn validate_rename_input(name: &str) -> Result<(), String> {
    if name.trim().is_empty() || name.trim() != name {
        return Err("Le nom ne peut pas être vide ni commencer ou finir par un espace.".into());
    }
    if name.ends_with('.')
        || name.chars().any(|character| {
            matches!(
                character,
                '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*'
            )
        })
    {
        return Err("Ce nom contient un caractère interdit sous Windows.".into());
    }
    Ok(())
}

fn open_item(path: String, error: RwSignal<Option<String>>) {
    spawn_local(async move {
        if let Err(message) = api::open(&path).await {
            error.set(Some(message));
        }
    });
}

fn reveal_item(path: String, error: RwSignal<Option<String>>) {
    spawn_local(async move {
        if let Err(message) = api::reveal(&path).await {
            error.set(Some(message));
        }
    });
}

fn begin_rename(
    item: SearchResult,
    rename_target: RwSignal<Option<SearchResult>>,
    rename_value: RwSignal<String>,
) {
    rename_value.set(item.name.clone());
    rename_target.set(Some(item));
}

fn submit_rename(
    rename_target: RwSignal<Option<SearchResult>>,
    rename_value: RwSignal<String>,
    refresh_token: RwSignal<u32>,
    error: RwSignal<Option<String>>,
) {
    let Some(item) = rename_target.get_untracked() else {
        return;
    };
    let new_name = rename_value.get_untracked();
    if new_name == item.name {
        rename_target.set(None);
        return;
    }
    if let Err(message) = validate_rename_input(&new_name) {
        error.set(Some(message));
        return;
    }

    spawn_local(async move {
        match api::rename(&item.full_path, &new_name).await {
            Ok(_) => {
                rename_target.set(None);
                refresh_results(refresh_token);
            }
            Err(message) => error.set(Some(message)),
        }
    });
}

fn begin_trash(
    selected: RwSignal<IndexSelection>,
    query: RwSignal<String>,
    sort: RwSignal<SortSpec>,
    generation: RwSignal<u32>,
    trash_pending: RwSignal<Option<TrashPending>>,
    trash_preparing: RwSignal<bool>,
    error: RwSignal<Option<String>>,
) {
    let selection = selected.get_untracked();
    if selection.is_empty()
        || trash_preparing.get_untracked()
        || trash_pending.get_untracked().is_some()
    {
        return;
    }
    trash_preparing.set(true);
    let request = everything_core::SelectionRequest {
        query: query.get_untracked(),
        sort: sort.get_untracked(),
        request_id: generation.get_untracked(),
        ranges: selection.ranges(),
    };
    spawn_local(async move {
        match api::prepare_trash(request).await {
            Ok(prepared) => trash_pending.set(Some(TrashPending {
                count: prepared.count,
                snapshot_id: prepared.snapshot_id,
            })),
            Err(message) => error.set(Some(message)),
        }
        trash_preparing.set(false);
    });
}

fn cancel_trash(trash_pending: RwSignal<Option<TrashPending>>) {
    let Some(pending) = trash_pending.get_untracked() else {
        return;
    };
    trash_pending.set(None);
    spawn_local(async move {
        api::cancel_trash(pending.snapshot_id).await;
    });
}

fn submit_trash(
    trash_pending: RwSignal<Option<TrashPending>>,
    selected: RwSignal<IndexSelection>,
    refresh_token: RwSignal<u32>,
    error: RwSignal<Option<String>>,
    trash_in_flight: RwSignal<bool>,
) {
    if trash_in_flight.get_untracked() {
        return;
    }
    let Some(pending) = trash_pending.get_untracked() else {
        return;
    };
    trash_in_flight.set(true);
    spawn_local(async move {
        match api::execute_trash(pending.snapshot_id).await {
            Ok(outcome) => {
                trash_pending.set(None);
                selected.set(IndexSelection::default());
                refresh_results(refresh_token);
                if !outcome.failures.is_empty() {
                    error.set(Some(format!(
                        "{} élément(s) supprimé(s), {} échec(s) :\n{}",
                        outcome.deleted,
                        outcome.failures.len(),
                        outcome.failures.join("\n")
                    )));
                }
            }
            Err(message) => error.set(Some(message)),
        }
        trash_in_flight.set(false);
    });
}

fn record_next_frame_latency(received_at: f64, target: RwSignal<Option<f64>>) {
    let callback = wasm_bindgen::closure::Closure::once_into_js(move |_timestamp: f64| {
        target.set(Some(js_sys::Date::now() - received_at));
    });
    let scheduled = web_sys::window().is_some_and(|window| {
        window
            .request_animation_frame(callback.unchecked_ref())
            .is_ok()
    });
    if !scheduled {
        target.set(Some(js_sys::Date::now() - received_at));
    }
}

fn refresh_results(refresh_token: RwSignal<u32>) {
    refresh_token.update(|value| *value = value.saturating_add(1));
}

fn copy_text(text: String, error: RwSignal<Option<String>>) {
    spawn_local(async move {
        if let Err(message) = api::copy_text(&text).await {
            error.set(Some(message));
        }
    });
}

fn format_size(size: Option<u64>, is_dir: bool) -> String {
    if is_dir {
        return String::new();
    }
    let Some(bytes) = size else {
        return "—".into();
    };
    const UNITS: [&str; 5] = ["o", "Ko", "Mo", "Go", "To"];
    let mut value = bytes as f64;
    let mut unit = 0;
    while value >= 1024.0 && unit < UNITS.len() - 1 {
        value /= 1024.0;
        unit += 1;
    }
    if unit == 0 {
        format!("{} {}", bytes, UNITS[unit])
    } else {
        format!("{value:.1} {}", UNITS[unit])
    }
}

fn format_date(timestamp: Option<i64>) -> String {
    let Some(timestamp) = timestamp else {
        return "—".into();
    };
    let date = js_sys::Date::new(&wasm_bindgen::JsValue::from_f64(timestamp as f64 * 1000.0));
    format!(
        "{:02}/{:02}/{} {:02}:{:02}",
        date.get_date(),
        date.get_month() + 1,
        date.get_full_year(),
        date.get_hours(),
        date.get_minutes(),
    )
}

fn format_result_count(total: u32) -> String {
    let text = total.to_string();
    let mut output = String::new();
    for (index, ch) in text.chars().rev().enumerate() {
        if index > 0 && index % 3 == 0 {
            output.push(' ');
        }
        output.push(ch);
    }
    format!("{} résultat(s)", output.chars().rev().collect::<String>())
}

fn svg(path: &'static str) -> AnyView {
    view! { <svg viewBox="0 0 24 24" aria-hidden="true"><path d=path></path></svg> }.into_any()
}
fn icon_search() -> AnyView {
    svg("M10.8 4.2a6.6 6.6 0 1 0 4.08 11.79l4.32 4.32 1.41-1.42-4.32-4.31A6.6 6.6 0 0 0 10.8 4.2Zm0 2a4.6 4.6 0 1 1 0 9.2 4.6 4.6 0 0 1 0-9.2Z")
}
fn icon_open() -> AnyView {
    svg("M5 4h6v2H6v12h12v-5h2v7H4V4h1Zm8-1h8v8h-2V6.41l-8.3 8.3-1.4-1.42L17.58 5H13V3Z")
}
fn icon_folder_open() -> AnyView {
    svg("M3 5h7l2 2h9v3h-2V9h-7.8l-2-2H5v10.2L7.2 11H22l-3.4 9H3V5Zm4.2 8L5.4 18h11.8l1.9-5H7.2Z")
}
fn icon_trash() -> AnyView {
    svg("M8 4V2h8v2h5v2H3V4h5Zm-2 4h12l-1 14H7L6 8Zm3 2 .6 10h1L10 10H9Zm5 0-.6 10h1L15 10h-1Z")
}
fn icon_home() -> AnyView {
    svg("m12 3 9 8h-3v10h-5v-6h-2v6H6V11H3l9-8Zm0 2.7L7.5 9.7V19H9v-6h6v6h1.5V9.7L12 5.7Z")
}
fn icon_clock() -> AnyView {
    svg("M12 2a10 10 0 1 0 0 20 10 10 0 0 0 0-20Zm0 2a8 8 0 1 1 0 16 8 8 0 0 1 0-16Zm-1 3v6l5 3 1-1.7-4-2.3V7h-2Z")
}
fn icon_document() -> AnyView {
    svg("M6 2h8l5 5v15H6V2Zm2 2v16h9V8h-4V4H8Zm7 .4V6h1.6L15 4.4ZM10 11h5v2h-5v-2Zm0 4h5v2h-5v-2Z")
}
fn icon_image() -> AnyView {
    svg("M4 3h16a2 2 0 0 1 2 2v14a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2Zm0 2v11l4-4 3 3 3-3 6 6V5H4Zm3 2a2 2 0 1 1 0 4 2 2 0 0 1 0-4Z")
}
fn icon_video() -> AnyView {
    svg("M4 5h12a2 2 0 0 1 2 2v2l4-2v10l-4-2v2a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2V7a2 2 0 0 1 2-2Zm0 2v10h12V7H4Zm14 4.2v1.6l2 .9V10.3l-2 .9Z")
}
fn icon_audio() -> AnyView {
    svg("M12 3v10.55A4 4 0 1 0 14 17V7h5V3h-7Zm-4 12a2 2 0 1 1 0 4 2 2 0 0 1 0-4Zm6-10h3v2h-3V5Z")
}
fn icon_archive() -> AnyView {
    svg("M4 3h16v5h-1v13H5V8H4V3Zm2 2v1h12V5H6Zm1 3v11h10V8H7Zm3 3h4v2h-4v-2Z")
}
fn icon_folder() -> AnyView {
    svg("M3 5h7l2 2h9v12H3V5Zm2 2v10h14V9h-7.8l-2-2H5Z")
}
fn icon_file() -> AnyView {
    svg("M6 2h8l5 5v15H6V2Zm2 2v16h9V8h-4V4H8Zm7 .4V6h1.6L15 4.4Z")
}
fn icon_copy() -> AnyView {
    svg("M8 7h12v15H8V7Zm2 2v11h8V9h-8ZM4 2h12v3h-2V4H6v11h1v2H4V2Z")
}
fn icon_edit() -> AnyView {
    svg("m16.7 3.3 4 4L9 19H5v-4L16.7 3.3Zm0 2.8L7 15.8V17h1.2l9.7-9.7-1.2-1.2ZM4 21h16v2H4v-2Z")
}
fn icon_warning() -> AnyView {
    svg("M12 2 1 21h22L12 2Zm0 4 7.5 13h-15L12 6Zm-1 4v5h2v-5h-2Zm0 7v2h2v-2h-2Z")
}
fn icon_search_large() -> AnyView {
    icon_search()
}
fn icon_empty() -> AnyView {
    svg("M4 4h16v16H4V4Zm2 2v12h12V6H6Zm2 3h8v2H8V9Zm0 4h5v2H8v-2Z")
}
fn icon_minimize() -> AnyView {
    svg("M5 12h14v1H5v-1Z")
}
fn icon_maximize() -> AnyView {
    svg("M5 5h14v14H5V5Zm1.5 1.5v11h11v-11h-11Z")
}
fn icon_close() -> AnyView {
    svg("m6.7 5.3 5.3 5.3 5.3-5.3 1.4 1.4-5.3 5.3 5.3 5.3-1.4 1.4-5.3-5.3-5.3 5.3-1.4-1.4 5.3-5.3-5.3-5.3 1.4-1.4Z")
}
