use super::super::settings::compose_query;
use crate::backend;
use everything_core::{QueryRequest, SearchResult, SelectionRange, SelectionRequest, SortSpec};
use gloo_timers::future::TimeoutFuture;
use leptos::prelude::*;
use leptos::task::spawn_local;
use std::collections::{BTreeMap, HashSet};
use wasm_bindgen::JsCast;

pub(super) const PAGE_SIZE: u32 = 256;
pub(super) const RESULT_ROW_HEIGHT: f64 = 34.0;
pub(super) const VIRTUALIZATION_OVERSCAN: u32 = 8;

const PAGE_CACHE_LIMIT: usize = 8;

#[derive(Clone, Copy)]
pub(super) struct SearchResults {
    pub(super) query: RwSignal<String>,
    excluded_folders: RwSignal<Vec<String>>,
    generation: RwSignal<u32>,
    refresh_token: RwSignal<u32>,
    preserve_view_on_refresh: RwSignal<bool>,
    pages: RwSignal<BTreeMap<u32, Vec<SearchResult>>>,
    loading_pages: RwSignal<HashSet<(u32, u32)>>,
    pub(super) total: RwSignal<u32>,
    pub(super) sort: RwSignal<SortSpec>,
    pub(super) loading: RwSignal<bool>,
    pub(super) render_latency_ms: RwSignal<Option<f64>>,
    pub(super) error: RwSignal<Option<String>>,
}

impl SearchResults {
    pub fn new(excluded_folders: RwSignal<Vec<String>>) -> Self {
        Self {
            query: RwSignal::new(String::new()),
            excluded_folders,
            generation: RwSignal::new(0),
            refresh_token: RwSignal::new(0),
            preserve_view_on_refresh: RwSignal::new(false),
            pages: RwSignal::new(BTreeMap::new()),
            loading_pages: RwSignal::new(HashSet::new()),
            total: RwSignal::new(0),
            sort: RwSignal::new(SortSpec::default()),
            loading: RwSignal::new(false),
            render_latency_ms: RwSignal::new(None),
            error: RwSignal::new(None),
        }
    }

    fn begin_generation(self, preserve_view: bool) -> u32 {
        let next_generation = self.generation.get_untracked().saturating_add(1);
        self.generation.set(next_generation);
        self.pages.set(BTreeMap::new());
        self.loading_pages.set(HashSet::new());
        if !preserve_view {
            self.total.set(0);
            self.render_latency_ms.set(None);
        }
        self.error.set(None);
        next_generation
    }

    fn request_page(self, query: String, page_index: u32, sort: SortSpec, request_generation: u32) {
        let loading_key = (request_generation, page_index);
        if self
            .pages
            .with_untracked(|cache| cache.contains_key(&page_index))
            || self
                .loading_pages
                .with_untracked(|set| set.contains(&loading_key))
        {
            return;
        }

        self.loading_pages.update(|set| {
            set.insert(loading_key);
        });
        self.loading.set(true);

        spawn_local(async move {
            let request = QueryRequest {
                query,
                offset: page_index.saturating_mul(PAGE_SIZE),
                limit: PAGE_SIZE,
                sort,
                request_id: request_generation,
            };
            let result = backend::search(request).await;
            self.loading_pages.update(|set| {
                set.remove(&loading_key);
            });

            if self.generation.get_untracked() != request_generation {
                return;
            }

            match result {
                Ok(page) => {
                    let received_at = js_sys::Date::now();
                    self.total.set(page.total);
                    self.pages.update(|cache| {
                        cache.insert(page_index, page.items);
                        evict_distant_pages(cache, page_index);
                    });
                    record_next_frame_latency(received_at, self.render_latency_ms);
                    self.error.set(None);
                }
                Err(message) => self.error.set(Some(message)),
            }
            self.loading.set(self.loading_pages.with_untracked(|set| {
                set.iter()
                    .any(|(request_id, _)| *request_id == request_generation)
            }));
        });
    }

    pub fn item_at(self, index: u32) -> Option<SearchResult> {
        let page = index / PAGE_SIZE;
        let within_page = usize::try_from(index % PAGE_SIZE)
            .expect("an index within a page always fits in usize");
        self.pages.with(|cache| {
            cache
                .get(&page)
                .and_then(|items| items.get(within_page))
                .cloned()
        })
    }

    pub async fn find_by_initial(self, initial: char, start: u32) -> Option<u32> {
        let total = self.total.get_untracked();
        if total == 0 {
            return None;
        }

        let generation = self.generation.get_untracked();
        let start = start.min(total);
        if let Some(index) = self
            .find_by_initial_in_range(initial, start, total, generation)
            .await
        {
            return Some(index);
        }
        self.find_by_initial_in_range(initial, 0, start, generation)
            .await
    }

    async fn find_by_initial_in_range(
        self,
        initial: char,
        start: u32,
        end: u32,
        generation: u32,
    ) -> Option<u32> {
        let mut offset = start;
        while offset < end && self.generation.get_untracked() == generation {
            let limit = PAGE_SIZE.min(end - offset);
            let request = QueryRequest {
                query: compose_query(
                    &self.query.get_untracked(),
                    &self.excluded_folders.get_untracked(),
                ),
                offset,
                limit,
                sort: self.sort.get_untracked(),
                request_id: generation,
            };
            let page = backend::search(request).await.ok()?;
            if self.generation.get_untracked() != generation {
                return None;
            }
            if let Some(within_page) = page
                .items
                .iter()
                .position(|item| name_starts_with(item, initial))
            {
                return offset.checked_add(u32::try_from(within_page).ok()?);
            }
            let received = u32::try_from(page.items.len()).ok()?;
            if received == 0 {
                return None;
            }
            offset = offset.saturating_add(received);
        }
        None
    }

    pub fn refresh(self) {
        self.refresh_token
            .update(|value| *value = value.saturating_add(1));
    }

    pub fn refresh_incrementally(self) {
        self.preserve_view_on_refresh.set(true);
        self.refresh();
    }

    pub fn selection_request(self, ranges: Vec<SelectionRange>) -> SelectionRequest {
        SelectionRequest {
            query: compose_query(
                &self.query.get_untracked(),
                &self.excluded_folders.get_untracked(),
            ),
            sort: self.sort.get_untracked(),
            request_id: self.generation.get_untracked(),
            ranges,
        }
    }

    pub fn monitor<F>(self, visible_start: Memo<u32>, visible_end: Memo<u32>, on_new_search: F)
    where
        F: Fn(bool) + Copy + Send + Sync + 'static,
    {
        Effect::new(move |_| {
            let current_query = self.query.get();
            let excluded_folders = self.excluded_folders.get();
            let current_sort = self.sort.get();
            let _refresh = self.refresh_token.get();
            let preserve_view = self.preserve_view_on_refresh.get_untracked();
            self.preserve_view_on_refresh.set(false);
            let next_generation = self.begin_generation(preserve_view);
            on_new_search(preserve_view);

            if current_query.trim().is_empty() {
                self.loading.set(false);
                return;
            }

            self.loading.set(true);
            spawn_local(async move {
                TimeoutFuture::new(55).await;
                if self.generation.get_untracked() != next_generation {
                    return;
                }

                if !self
                    .register_debounced_backend_generation(next_generation)
                    .await
                {
                    return;
                }
                self.request_page(
                    compose_query(&current_query, &excluded_folders),
                    0,
                    current_sort,
                    next_generation,
                );
            });
        });

        Effect::new(move |_| {
            let start = visible_start.get();
            let end = visible_end.get();
            let current_query = self.query.get();
            let excluded_folders = self.excluded_folders.get();
            if current_query.trim().is_empty() || end == 0 {
                return;
            }
            let current_generation = self.generation.get();
            let current_sort = self.sort.get();
            let first_page = start / PAGE_SIZE;
            let last_page = end.saturating_sub(1) / PAGE_SIZE;
            let max_page = self.total.get().saturating_sub(1) / PAGE_SIZE;
            for page in first_page.saturating_sub(1)..=last_page.saturating_add(1).min(max_page) {
                self.request_page(
                    compose_query(&current_query, &excluded_folders),
                    page,
                    current_sort,
                    current_generation,
                );
            }
        });
    }

    async fn register_debounced_backend_generation(self, generation: u32) -> bool {
        if let Err(message) = backend::begin_generation(generation).await {
            self.error.set(Some(message));
        }
        self.generation.get_untracked() == generation
    }
}

fn evict_distant_pages(cache: &mut BTreeMap<u32, Vec<SearchResult>>, current_page: u32) {
    while cache.len() > PAGE_CACHE_LIMIT {
        let Some(distant_page) = cache
            .keys()
            .copied()
            .max_by_key(|page| page.abs_diff(current_page))
        else {
            break;
        };
        cache.remove(&distant_page);
    }
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

fn name_starts_with(item: &SearchResult, initial: char) -> bool {
    item.name
        .chars()
        .next()
        .is_some_and(|first| first.to_lowercase().eq(initial.to_lowercase()))
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use everything_core::SearchResult;

    use super::{evict_distant_pages, name_starts_with, PAGE_CACHE_LIMIT};

    #[test]
    fn page_cache_keeps_the_pages_nearest_to_the_latest_request() {
        let page_cache_limit =
            u32::try_from(PAGE_CACHE_LIMIT).expect("the page cache limit fits in u32");
        let mut cache = (0..=page_cache_limit)
            .map(|page| (page, Vec::<SearchResult>::new()))
            .collect::<BTreeMap<_, _>>();

        evict_distant_pages(&mut cache, 4);

        assert_eq!(cache.len(), PAGE_CACHE_LIMIT);
        assert!(cache.contains_key(&4));
        assert!(
            cache.contains_key(&0) ^ cache.contains_key(&page_cache_limit),
            "one of the two equally distant edge pages should be evicted"
        );
    }

    #[test]
    fn name_initial_matching_ignores_case() {
        let item = SearchResult {
            id: "1".into(),
            name: "File.txt".into(),
            parent_path: String::new(),
            full_path: "File.txt".into(),
            size: None,
            modified_unix: None,
            is_dir: false,
        };

        assert!(name_starts_with(&item, 'f'));
        assert!(!name_starts_with(&item, 'x'));
    }
}
