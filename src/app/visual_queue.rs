use leptos::prelude::*;
use leptos::task::spawn_local;
use std::cell::RefCell;
use std::collections::{HashMap, VecDeque};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use wasm_bindgen::{closure::Closure, JsCast, JsValue};
use wasm_bindgen_futures::JsFuture;

use crate::backend;
use crate::diagnostics;

const CACHE_MAX_BYTES: usize = 24 * 1024 * 1024;
const CACHE_MAX_ENTRIES: usize = 512;

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
struct ThumbnailKey {
    path: String,
    pixel_size: u32,
    file_size: Option<u64>,
    modified_unix: Option<i64>,
}

struct QueuedThumbnail {
    key: ThumbnailKey,
    source: RwSignal<Option<Option<String>>>,
    cancelled: Arc<AtomicBool>,
}

#[derive(Clone)]
pub(super) struct ThumbnailSubscription {
    pub source: RwSignal<Option<Option<String>>>,
    pub animate_reveal: bool,
    cancelled: Arc<AtomicBool>,
}

impl ThumbnailSubscription {
    pub fn cancel(&self) {
        self.cancelled.store(true, Ordering::Relaxed);
    }
}

struct ThumbnailCache {
    entries: HashMap<ThumbnailKey, Option<String>>,
    order: VecDeque<ThumbnailKey>,
    bytes: usize,
    max_bytes: usize,
    max_entries: usize,
}

impl ThumbnailCache {
    fn new(max_bytes: usize, max_entries: usize) -> Self {
        Self {
            entries: HashMap::new(),
            order: VecDeque::new(),
            bytes: 0,
            max_bytes,
            max_entries,
        }
    }

    fn get(&mut self, key: &ThumbnailKey) -> Option<Option<String>> {
        let source = self.entries.get(key).cloned();
        if source.is_some() {
            self.touch(key);
        }
        source
    }

    fn insert(&mut self, key: ThumbnailKey, source: Option<String>) {
        if self.entries.contains_key(&key) {
            self.touch(&key);
            return;
        }

        self.bytes = self
            .bytes
            .saturating_add(source.as_ref().map_or(0, String::len));
        self.entries.insert(key.clone(), source);
        self.order.push_back(key);
        self.evict_over_limit();
    }

    fn touch(&mut self, key: &ThumbnailKey) {
        if let Some(position) = self.order.iter().position(|candidate| candidate == key) {
            self.order.remove(position);
        }
        self.order.push_back(key.clone());
    }

    fn evict_over_limit(&mut self) {
        while self.bytes > self.max_bytes || self.order.len() > self.max_entries {
            let Some(oldest) = self.order.pop_front() else {
                break;
            };
            if let Some(removed) = self.entries.remove(&oldest) {
                self.bytes = self
                    .bytes
                    .saturating_sub(removed.as_ref().map_or(0, String::len));
            }
        }
    }
}

struct ThumbnailPipeline {
    pending: VecDeque<QueuedThumbnail>,
    running: bool,
    cache: ThumbnailCache,
}

impl Default for ThumbnailPipeline {
    fn default() -> Self {
        Self {
            pending: VecDeque::new(),
            running: false,
            cache: ThumbnailCache::new(CACHE_MAX_BYTES, CACHE_MAX_ENTRIES),
        }
    }
}

impl ThumbnailPipeline {
    fn enqueue(&mut self, request: QueuedThumbnail) -> bool {
        self.pending
            .retain(|pending| !pending.cancelled.load(Ordering::Relaxed));
        self.pending.push_back(request);
        if self.running {
            false
        } else {
            self.running = true;
            true
        }
    }

    fn next_request(&mut self) -> Option<QueuedThumbnail> {
        while let Some(request) = self.pending.pop_front() {
            if !request.cancelled.load(Ordering::Relaxed) {
                return Some(request);
            }
        }
        self.running = false;
        None
    }
}

thread_local! {
    static PIPELINE: RefCell<ThumbnailPipeline> = RefCell::new(ThumbnailPipeline::default());
}

fn cached_thumbnail(key: &ThumbnailKey) -> Option<Option<String>> {
    PIPELINE.with(|pipeline| pipeline.borrow_mut().cache.get(key))
}

fn cache_thumbnail(key: ThumbnailKey, source: Option<String>) {
    PIPELINE.with(|pipeline| pipeline.borrow_mut().cache.insert(key, source));
}

fn enqueue_thumbnail(request: QueuedThumbnail) {
    let start_worker = PIPELINE.with(|pipeline| pipeline.borrow_mut().enqueue(request));

    if start_worker {
        spawn_local(process_thumbnail_queue());
    }
}

async fn process_thumbnail_queue() {
    next_animation_frame().await;

    loop {
        let request = PIPELINE.with(|pipeline| pipeline.borrow_mut().next_request());
        let Some(request) = request else {
            return;
        };
        if let Some(cached) = cached_thumbnail(&request.key) {
            request.source.set(Some(cached));
            continue;
        }

        match backend::visual(&request.key.path, request.key.pixel_size).await {
            Ok(visual) => {
                cache_thumbnail(request.key, visual.clone());
                if !request.cancelled.load(Ordering::Relaxed) {
                    request.source.set(Some(visual));
                    next_animation_frame().await;
                }
            }
            Err(error) if !request.cancelled.load(Ordering::Relaxed) => {
                // Errors remain retryable; only a confirmed absence is cached.
                diagnostics::warn(&format!(
                    "Unable to load visual for '{}': {error}",
                    request.key.path
                ));
                request.source.set(Some(None));
            }
            Err(error) => diagnostics::warn(&format!(
                "Unable to load visual for cancelled request '{}': {error}",
                request.key.path
            )),
        }
    }
}

pub(super) async fn next_animation_frame() {
    let promise = js_sys::Promise::new(&mut |resolve, _reject| {
        let Some(window) = web_sys::window() else {
            if let Err(error) = resolve.call0(&JsValue::NULL) {
                diagnostics::warn_js("Unable to resolve the animation-frame fallback.", &error);
            }
            return;
        };
        let frame_resolve = resolve.clone();
        let callback = Closure::once(move |_timestamp: f64| {
            if let Err(error) = frame_resolve.call0(&JsValue::NULL) {
                diagnostics::warn_js("Unable to resolve the animation frame.", &error);
            }
        });
        match window.request_animation_frame(callback.as_ref().unchecked_ref()) {
            Ok(_) => callback.forget(),
            Err(error) => {
                diagnostics::warn_js("Unable to schedule an animation frame.", &error);
                if let Err(error) = resolve.call0(&JsValue::NULL) {
                    diagnostics::warn_js("Unable to resolve the animation-frame fallback.", &error);
                }
            }
        }
    });
    if let Err(error) = JsFuture::from(promise).await {
        diagnostics::warn_js("Animation-frame promise was rejected.", &error);
    }
}

pub(super) fn request_thumbnail(
    path: String,
    pixel_size: u32,
    file_size: Option<u64>,
    modified_unix: Option<i64>,
) -> ThumbnailSubscription {
    let cancelled = Arc::new(AtomicBool::new(false));
    let key = ThumbnailKey {
        path,
        pixel_size,
        file_size,
        modified_unix,
    };
    let cached = cached_thumbnail(&key);
    let source = RwSignal::new(cached.clone());
    if cached.is_none() {
        enqueue_thumbnail(QueuedThumbnail {
            key,
            source,
            cancelled: cancelled.clone(),
        });
    }
    ThumbnailSubscription {
        source,
        animate_reveal: cached.is_none(),
        cancelled,
    }
}

#[cfg(test)]
mod tests {
    use super::{QueuedThumbnail, ThumbnailCache, ThumbnailKey, ThumbnailPipeline};
    use leptos::prelude::*;
    use std::sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    };

    fn key(path: &str, modified_unix: i64) -> ThumbnailKey {
        ThumbnailKey {
            path: path.into(),
            pixel_size: 96,
            file_size: Some(10),
            modified_unix: Some(modified_unix),
        }
    }

    fn request(path: &str) -> (QueuedThumbnail, Arc<AtomicBool>) {
        let cancelled = Arc::new(AtomicBool::new(false));
        (
            QueuedThumbnail {
                key: key(path, 1),
                source: RwSignal::new(None),
                cancelled: cancelled.clone(),
            },
            cancelled,
        )
    }

    #[test]
    fn cache_key_changes_with_the_file_version() {
        assert_ne!(key("track.mp3", 1), key("track.mp3", 2));
    }

    #[test]
    fn cache_hit_promotes_the_entry_before_eviction() {
        let mut cache = ThumbnailCache::new(1_000, 2);
        let first = key("first.mp3", 1);
        let second = key("second.mp3", 1);
        let third = key("third.mp3", 1);
        cache.insert(first.clone(), Some("one".into()));
        cache.insert(second.clone(), Some("two".into()));

        assert_eq!(cache.get(&first), Some(Some("one".into())));
        cache.insert(third.clone(), Some("three".into()));

        assert!(cache.get(&second).is_none());
        assert!(cache.get(&first).is_some());
        assert!(cache.get(&third).is_some());
    }

    #[test]
    fn cache_evicts_until_its_byte_limit_is_respected() {
        let mut cache = ThumbnailCache::new(5, 10);
        let first = key("first.mp3", 1);
        let second = key("second.mp3", 1);
        cache.insert(first.clone(), Some("123".into()));
        cache.insert(second.clone(), Some("456".into()));

        assert!(cache.get(&first).is_none());
        assert_eq!(cache.get(&second), Some(Some("456".into())));
        assert_eq!(cache.bytes, 3);
    }

    #[test]
    fn confirmed_missing_thumbnails_are_bounded_by_entry_count() {
        let mut cache = ThumbnailCache::new(1_000, 1);
        let first = key("first.unknown", 1);
        let second = key("second.unknown", 1);
        cache.insert(first.clone(), None);
        cache.insert(second.clone(), None);

        assert!(cache.get(&first).is_none());
        assert_eq!(cache.get(&second), Some(None));
        assert_eq!(cache.entries.len(), 1);
    }

    #[test]
    fn queue_preserves_fifo_order_for_live_requests() {
        Owner::new().with(|| {
            let mut pipeline = ThumbnailPipeline::default();
            let (first, _) = request("first.mp3");
            let (second, _) = request("second.mp3");

            assert!(pipeline.enqueue(first));
            assert!(!pipeline.enqueue(second));
            assert_eq!(
                pipeline.next_request().map(|request| request.key.path),
                Some("first.mp3".into())
            );
            assert_eq!(
                pipeline.next_request().map(|request| request.key.path),
                Some("second.mp3".into())
            );
        });
    }

    #[test]
    fn enqueue_eagerly_discards_cancelled_requests() {
        Owner::new().with(|| {
            let mut pipeline = ThumbnailPipeline::default();
            let (obsolete, cancelled) = request("obsolete.mp3");
            assert!(pipeline.enqueue(obsolete));
            cancelled.store(true, Ordering::Relaxed);

            let (current, _) = request("current.mp3");
            assert!(!pipeline.enqueue(current));

            assert_eq!(pipeline.pending.len(), 1);
            assert_eq!(
                pipeline.next_request().map(|request| request.key.path),
                Some("current.mp3".into())
            );
        });
    }
}
