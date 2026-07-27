use crate::{backend, diagnostics};
use gloo_timers::future::TimeoutFuture;
use leptos::prelude::*;
use leptos::task::spawn_local;
use std::cell::RefCell;
use std::collections::{HashMap, VecDeque};

use super::visual_queue::request_thumbnail;

const ICON_CACHE_MAX_ENTRIES: usize = 256;
const ICON_LOAD_ATTEMPTS: usize = 2;
const ICON_RETRY_DELAY_MS: u32 = 80;
const ICON_ERROR_RETRY_MS: f64 = 1_000.0;
const ICON_MISS_RETRY_MS: f64 = 15_000.0;

type IconSource = ArcRwSignal<Option<Option<String>>>;

struct IconCacheEntry {
    source: IconSource,
    loading: bool,
    generation: u64,
    retry_after_ms: Option<f64>,
}

struct IconSourceCache {
    entries: HashMap<String, IconCacheEntry>,
    order: VecDeque<String>,
    next_generation: u64,
}

impl IconSourceCache {
    fn new() -> Self {
        Self {
            entries: HashMap::new(),
            order: VecDeque::new(),
            next_generation: 0,
        }
    }

    fn next_generation(&mut self) -> u64 {
        self.next_generation = self.next_generation.wrapping_add(1);
        self.next_generation
    }

    fn get_or_insert(&mut self, key: String) -> (IconSource, bool, u64) {
        let now = js_sys::Date::now();
        if self.entries.contains_key(&key) {
            let retry_generation = self.next_generation();
            let (source, should_load, generation) = {
                let entry = self
                    .entries
                    .get_mut(&key)
                    .expect("an existing icon cache key remains present");
                let retry_due = entry
                    .retry_after_ms
                    .is_some_and(|retry_after| retry_after <= now);
                let should_load = !entry.loading
                    && entry.source.get_untracked() == Some(None)
                    && retry_due;
                if should_load {
                    entry.loading = true;
                    entry.generation = retry_generation;
                    entry.retry_after_ms = None;
                    entry.source.set(None);
                }
                (entry.source.clone(), should_load, entry.generation)
            };
            self.touch(&key);
            return (source, should_load, generation);
        }

        let generation = self.next_generation();
        let source = ArcRwSignal::new(None);
        self.entries.insert(
            key.clone(),
            IconCacheEntry {
                source: source.clone(),
                loading: true,
                generation,
                retry_after_ms: None,
            },
        );
        self.order.push_back(key);
        while self.order.len() > ICON_CACHE_MAX_ENTRIES {
            if let Some(oldest) = self.order.pop_front() {
                self.entries.remove(&oldest);
            }
        }
        (source, true, generation)
    }

    fn finish(&mut self, key: &str, generation: u64, retry_after_ms: Option<f64>) {
        let Some(entry) = self.entries.get_mut(key) else {
            return;
        };
        if entry.generation != generation {
            return;
        }
        entry.loading = false;
        entry.retry_after_ms = retry_after_ms;
    }

    fn touch(&mut self, key: &str) {
        if let Some(position) = self.order.iter().position(|candidate| candidate == key) {
            self.order.remove(position);
        }
        self.order.push_back(key.to_string());
    }
}

thread_local! {
    static ICON_SOURCES: RefCell<IconSourceCache> = RefCell::new(IconSourceCache::new());
}

fn icon_cache_key(path: &str, is_dir: bool) -> String {
    let normalized = path.replace('/', "\\").to_lowercase();
    if is_dir {
        return format!("path:{normalized}");
    }

    let name = normalized.rsplit('\\').next().unwrap_or(&normalized);
    let extension = name
        .rsplit_once('.')
        .map(|(_, extension)| extension)
        .filter(|extension| !extension.is_empty());
    match extension {
        Some("exe" | "lnk" | "url" | "ico" | "cur") | None => format!("path:{normalized}"),
        Some(extension) => format!("extension:{extension}"),
    }
}

fn icon_source(key: String) -> (IconSource, bool, u64) {
    ICON_SOURCES.with(|sources| sources.borrow_mut().get_or_insert(key))
}

fn finish_icon_load(key: &str, generation: u64, retry_after_ms: Option<f64>) {
    ICON_SOURCES.with(|sources| {
        sources
            .borrow_mut()
            .finish(key, generation, retry_after_ms);
    });
}

fn fallback_icon(is_dir: bool) -> AnyView {
    if is_dir {
        view! {
            <svg class="size-full" viewBox="0 0 20 20" aria-hidden="true">
                <path
                    fill="#F5B72E"
                    d="M1.5 4.25A1.25 1.25 0 0 1 2.75 3h4.1c.4 0 .78.19 1.02.51l.87 1.16h8.51A1.25 1.25 0 0 1 18.5 5.92v8.83A1.25 1.25 0 0 1 17.25 16H2.75A1.25 1.25 0 0 1 1.5 14.75V4.25Z"
                />
                <path
                    fill="#FFD45A"
                    d="M1.5 6.25h17v8.5A1.25 1.25 0 0 1 17.25 16H2.75A1.25 1.25 0 0 1 1.5 14.75v-8.5Z"
                />
            </svg>
        }
        .into_any()
    } else {
        view! {
            <svg
                class="size-[88%] text-[var(--muted)]"
                viewBox="0 0 20 20"
                fill="none"
                aria-hidden="true"
            >
                <path
                    d="M4.5 2.5h7l4 4v11h-11v-15Z"
                    stroke="currentColor"
                    stroke-width="1.4"
                    stroke-linejoin="round"
                />
                <path
                    d="M11.5 2.5v4h4"
                    stroke="currentColor"
                    stroke-width="1.4"
                    stroke-linejoin="round"
                />
            </svg>
        }
        .into_any()
    }
}

fn thumbnail_pixel_size(visual_size: u32) -> u32 {
    let pixel_ratio = web_sys::window()
        .map(|window| window.device_pixel_ratio())
        .unwrap_or(1.0);
    ((f64::from(visual_size) * pixel_ratio).ceil() as u32).clamp(32, 256)
}

#[component]
pub(crate) fn FileIcon(path: String, is_dir: bool) -> impl IntoView {
    let cache_key = icon_cache_key(&path, is_dir);
    let (source, should_load, generation) = icon_source(cache_key.clone());
    if should_load {
        let source_for_load = source.clone();
        let path = path.clone();
        spawn_local(async move {
            let mut last_error = None;
            for attempt in 0..ICON_LOAD_ATTEMPTS {
                match backend::visual(&path, 64, false).await {
                    Ok(Some(icon)) => {
                        source_for_load.set(Some(Some(icon)));
                        finish_icon_load(&cache_key, generation, None);
                        return;
                    }
                    Ok(None) => {}
                    Err(error) => last_error = Some(error),
                }

                if attempt + 1 < ICON_LOAD_ATTEMPTS {
                    TimeoutFuture::new(ICON_RETRY_DELAY_MS).await;
                }
            }

            let retry_delay = if let Some(error) = last_error {
                diagnostics::warn(&format!("Unable to load icon for '{path}': {error}"));
                ICON_ERROR_RETRY_MS
            } else {
                ICON_MISS_RETRY_MS
            };
            source_for_load.set(Some(None));
            finish_icon_load(
                &cache_key,
                generation,
                Some(js_sys::Date::now() + retry_delay),
            );
        });
    }

    view! {
        <span class="file-icon grid size-5 place-items-center [&>img]:size-5 [&>img]:object-contain">
            {move || match source.get().flatten() {
                Some(source) => view! {
                    <img src=source alt="" loading="eager" decoding="async" />
                }.into_any(),
                None => fallback_icon(is_dir),
            }}
        </span>
    }
}

#[component]
pub(crate) fn FileVisual(
    path: String,
    is_dir: bool,
    visual_size: u32,
    file_size: Option<u64>,
    modified_unix: Option<i64>,
    load: bool,
) -> impl IntoView {
    let subscription = load.then(|| {
        request_thumbnail(
            path.clone(),
            thumbnail_pixel_size(visual_size),
            file_size,
            modified_unix,
        )
    });

    if let Some(subscription) = subscription.as_ref() {
        let subscription = subscription.clone();
        on_cleanup(move || subscription.cancel());
    }

    if let Some(subscription) = subscription {
        let source = subscription.source;
        let fallback_source = source.clone();
        let animate_reveal = subscription.animate_reveal;
        view! {
            <span class="icon-result-visual thumbnail-stack grid size-[var(--view-icon-size)] shrink-0 place-items-center [&>*]:[grid-area:1/1]">
                {move || fallback_source.get().flatten().is_none().then(|| fallback_icon(is_dir))}
                {move || source.get().flatten().map(|source| view! {
                    <img
                        class="file-visual-image size-full object-contain"
                        class:thumbnail-reveal=animate_reveal
                        src=source
                        alt=""
                        loading="eager"
                        decoding="async"
                    />
                })}
            </span>
        }
        .into_any()
    } else {
        view! {
            <span class="icon-result-visual grid size-[var(--view-icon-size)] shrink-0 place-items-center">
                <FileIcon path=path is_dir />
            </span>
        }
        .into_any()
    }
}

#[cfg(test)]
mod tests {
    use super::icon_cache_key;

    #[test]
    fn shared_file_types_reuse_one_icon_source() {
        assert_eq!(
            icon_cache_key(r"C:\Music\first.mp3", false),
            icon_cache_key(r"D:\Other\second.MP3", false)
        );
    }

    #[test]
    fn unique_icons_and_folders_remain_path_specific() {
        assert_ne!(
            icon_cache_key(r"C:\Apps\first.exe", false),
            icon_cache_key(r"C:\Apps\second.exe", false)
        );
        assert_ne!(
            icon_cache_key(r"C:\First.folder", true),
            icon_cache_key(r"C:\Second.folder", true)
        );
    }
}
