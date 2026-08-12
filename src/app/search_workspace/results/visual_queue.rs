use crate::{backend, diagnostics};
use everything_core::MAX_CONCURRENT_THUMBNAIL_LOADS;
use leptos::prelude::*;
use leptos::task::spawn_local;
use std::cell::{Cell, RefCell};
use std::collections::{HashMap, VecDeque};
use std::rc::Rc;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};

const CACHE_MAX_BYTES: usize = 24 * 1024 * 1024;
const CACHE_MAX_ENTRIES: usize = 512;

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
struct ThumbnailKey {
    path: String,
    pixel_size: u32,
    file_size: Option<u64>,
    modified_unix: Option<i64>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) enum VisualState {
    Loading,
    Missing,
    Ready(String),
}

impl From<Option<String>> for VisualState {
    fn from(source: Option<String>) -> Self {
        source.map_or(Self::Missing, Self::Ready)
    }
}

type ThumbnailSource = ArcRwSignal<VisualState>;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ResourceState {
    Pending,
    Running,
    Cancelled,
}

struct ThumbnailResource {
    id: u64,
    source: ThumbnailSource,
    subscribers: Cell<usize>,
    state: Cell<ResourceState>,
}

impl ThumbnailResource {
    fn new(id: u64) -> Self {
        Self {
            id,
            source: ArcRwSignal::new(VisualState::Loading),
            subscribers: Cell::new(1),
            state: Cell::new(ResourceState::Pending),
        }
    }
}

type ThumbnailResourceRef = Rc<ThumbnailResource>;

struct QueuedThumbnail {
    key: ThumbnailKey,
    resource: ThumbnailResourceRef,
}

#[derive(Clone)]
pub(super) struct ThumbnailSubscription {
    pub source: ThumbnailSource,
    pub animate_reveal: bool,
    key: Option<ThumbnailKey>,
    resource_id: Option<u64>,
    cancelled: Arc<AtomicBool>,
}

impl ThumbnailSubscription {
    pub fn cancel(&self) {
        if self.cancelled.swap(true, Ordering::Relaxed) {
            return;
        }
        let (Some(key), Some(resource_id)) = (&self.key, self.resource_id) else {
            return;
        };
        PIPELINE.with(|pipeline| pipeline.borrow_mut().unsubscribe(key, resource_id));
    }
}

#[derive(Clone)]
struct CachedThumbnail {
    state: VisualState,
    bytes: usize,
}

struct ThumbnailCache {
    entries: HashMap<ThumbnailKey, CachedThumbnail>,
    order: VecDeque<ThumbnailKey>,
    bytes: usize,
}

impl ThumbnailCache {
    fn new() -> Self {
        Self {
            entries: HashMap::new(),
            order: VecDeque::new(),
            bytes: 0,
        }
    }

    fn get(&mut self, key: &ThumbnailKey) -> Option<VisualState> {
        let state = self.entries.get(key).map(|entry| entry.state.clone());
        if state.is_some() {
            touch(&mut self.order, key);
        }
        state
    }

    fn insert(&mut self, key: ThumbnailKey, source: Option<String>) {
        if self.entries.contains_key(&key) {
            touch(&mut self.order, &key);
            return;
        }

        let bytes = source.as_ref().map_or(0, String::len);
        self.entries.insert(
            key.clone(),
            CachedThumbnail {
                state: source.into(),
                bytes,
            },
        );
        self.order.push_back(key);
        self.bytes = self.bytes.saturating_add(bytes);

        while self.bytes > CACHE_MAX_BYTES || self.order.len() > CACHE_MAX_ENTRIES {
            let Some(oldest) = self.order.pop_front() else {
                break;
            };
            if let Some(removed) = self.entries.remove(&oldest) {
                self.bytes = self.bytes.saturating_sub(removed.bytes);
            }
        }
    }
}

struct ThumbnailPipeline {
    pending: VecDeque<QueuedThumbnail>,
    running_workers: usize,
    resources: HashMap<ThumbnailKey, ThumbnailResourceRef>,
    cache: ThumbnailCache,
    next_resource_id: u64,
}

impl Default for ThumbnailPipeline {
    fn default() -> Self {
        Self {
            pending: VecDeque::new(),
            running_workers: 0,
            resources: HashMap::new(),
            cache: ThumbnailCache::new(),
            next_resource_id: 1,
        }
    }
}

impl ThumbnailPipeline {
    fn subscribe(&mut self, key: ThumbnailKey) -> (ThumbnailSubscription, usize) {
        if let Some(cached) = self.cache.get(&key) {
            return (
                ThumbnailSubscription {
                    source: ArcRwSignal::new(cached),
                    animate_reveal: false,
                    key: None,
                    resource_id: None,
                    cancelled: Arc::new(AtomicBool::new(false)),
                },
                0,
            );
        }

        if let Some(resource) = self.resources.get(&key) {
            resource
                .subscribers
                .set(resource.subscribers.get().saturating_add(1));
            return (
                ThumbnailSubscription {
                    source: resource.source.clone(),
                    animate_reveal: resource.source.get_untracked() == VisualState::Loading,
                    key: Some(key),
                    resource_id: Some(resource.id),
                    cancelled: Arc::new(AtomicBool::new(false)),
                },
                0,
            );
        }

        let resource_id = self.next_resource_id;
        self.next_resource_id = self.next_resource_id.wrapping_add(1);
        let resource = Rc::new(ThumbnailResource::new(resource_id));
        self.resources.insert(key.clone(), resource.clone());
        self.pending.push_back(QueuedThumbnail {
            key: key.clone(),
            resource: resource.clone(),
        });
        let workers = self.workers_to_start();
        (
            ThumbnailSubscription {
                source: resource.source.clone(),
                animate_reveal: true,
                key: Some(key),
                resource_id: Some(resource_id),
                cancelled: Arc::new(AtomicBool::new(false)),
            },
            workers,
        )
    }

    fn unsubscribe(&mut self, key: &ThumbnailKey, resource_id: u64) {
        let Some(current) = self.resources.get(key) else {
            return;
        };
        if current.id != resource_id {
            return;
        }

        let remaining = current.subscribers.get().saturating_sub(1);
        current.subscribers.set(remaining);
        if remaining == 0 && current.state.get() == ResourceState::Pending {
            current.state.set(ResourceState::Cancelled);
            self.resources.remove(key);
        }
    }

    fn workers_to_start(&mut self) -> usize {
        if self.running_workers >= MAX_CONCURRENT_THUMBNAIL_LOADS {
            return 0;
        }
        self.running_workers += 1;
        1
    }

    fn next_request(&mut self) -> Option<QueuedThumbnail> {
        while let Some(request) = self.pending.pop_front() {
            if request.resource.state.get() != ResourceState::Pending
                || request.resource.subscribers.get() == 0
            {
                continue;
            }
            request.resource.state.set(ResourceState::Running);
            return Some(request);
        }
        None
    }

    fn complete(&mut self, request: &QueuedThumbnail, source: Option<String>) {
        self.cache.insert(request.key.clone(), source.clone());
        if request.resource.subscribers.get() > 0 {
            request.resource.source.set(source.into());
        }
        self.remove_resource(&request.key, &request.resource);
    }

    fn fail(&mut self, request: &QueuedThumbnail) {
        if request.resource.subscribers.get() > 0 {
            request.resource.source.set(VisualState::Missing);
        }
        self.remove_resource(&request.key, &request.resource);
    }

    fn remove_resource(&mut self, key: &ThumbnailKey, resource: &ThumbnailResourceRef) {
        let should_remove = self
            .resources
            .get(key)
            .is_some_and(|current| Rc::ptr_eq(current, resource));
        if should_remove {
            self.resources.remove(key);
        }
    }

    fn worker_finished(&mut self) {
        self.running_workers = self.running_workers.saturating_sub(1);
    }
}

fn touch(order: &mut VecDeque<ThumbnailKey>, key: &ThumbnailKey) {
    if let Some(position) = order.iter().position(|candidate| candidate == key) {
        order.remove(position);
    }
    order.push_back(key.clone());
}

thread_local! {
    static PIPELINE: RefCell<ThumbnailPipeline> = RefCell::new(ThumbnailPipeline::default());
}

async fn process_thumbnail_queue() {
    loop {
        let request = PIPELINE.with(|pipeline| pipeline.borrow_mut().next_request());
        let Some(request) = request else {
            PIPELINE.with(|pipeline| pipeline.borrow_mut().worker_finished());
            return;
        };

        match backend::visual(&request.key.path, request.key.pixel_size, true).await {
            Ok(source) => {
                PIPELINE.with(|pipeline| pipeline.borrow_mut().complete(&request, source));
            }
            Err(error) => {
                diagnostics::warn(&format!(
                    "Unable to load visual for '{}': {error}",
                    request.key.path
                ));
                PIPELINE.with(|pipeline| pipeline.borrow_mut().fail(&request));
            }
        }
    }
}

pub(super) fn request_thumbnail(
    path: String,
    pixel_size: u32,
    file_size: Option<u64>,
    modified_unix: Option<i64>,
) -> ThumbnailSubscription {
    let key = ThumbnailKey {
        path,
        pixel_size,
        file_size,
        modified_unix,
    };
    let (subscription, workers) = PIPELINE.with(|pipeline| pipeline.borrow_mut().subscribe(key));
    for _ in 0..workers {
        spawn_local(process_thumbnail_queue());
    }
    subscription
}

#[cfg(test)]
mod tests {
    use super::{ThumbnailKey, ThumbnailPipeline, VisualState};
    use everything_core::MAX_CONCURRENT_THUMBNAIL_LOADS;
    use leptos::prelude::*;

    fn key(path: &str, pixel_size: u32, modified_unix: i64) -> ThumbnailKey {
        ThumbnailKey {
            path: path.into(),
            pixel_size,
            file_size: Some(10),
            modified_unix: Some(modified_unix),
        }
    }

    #[test]
    fn cache_key_changes_with_size_and_file_version() {
        assert_ne!(key("track.mp3", 64, 1), key("track.mp3", 128, 1));
        assert_ne!(key("track.mp3", 64, 1), key("track.mp3", 64, 2));
    }

    #[test]
    fn subscriptions_share_one_pending_request() {
        Owner::new().with(|| {
            let mut pipeline = ThumbnailPipeline::default();
            let item = key("track.mp3", 96, 1);
            let (first, workers) = pipeline.subscribe(item.clone());
            let (second, second_workers) = pipeline.subscribe(item);

            assert_eq!(workers, 1);
            assert_eq!(second_workers, 0);
            assert_eq!(pipeline.pending.len(), 1);
            assert_eq!(pipeline.resources.len(), 1);
            first.source.set(VisualState::Ready("thumbnail".into()));
            assert_eq!(
                second.source.get_untracked(),
                VisualState::Ready("thumbnail".into())
            );
        });
    }

    #[test]
    fn cancelled_pending_requests_are_skipped() {
        Owner::new().with(|| {
            let mut pipeline = ThumbnailPipeline::default();
            let item = key("obsolete.mp3", 96, 1);
            let (subscription, _) = pipeline.subscribe(item.clone());
            let resource_id = subscription.resource_id.expect("pending resource");
            pipeline.unsubscribe(&item, resource_id);

            assert!(pipeline.next_request().is_none());
            assert!(pipeline.resources.is_empty());
        });
    }

    #[test]
    fn worker_count_is_bounded() {
        Owner::new().with(|| {
            let mut pipeline = ThumbnailPipeline::default();
            let mut started = 0;
            for index in 0..10 {
                let (_, workers) = pipeline.subscribe(key(&format!("{index}.png"), 96, 1));
                if index < MAX_CONCURRENT_THUMBNAIL_LOADS {
                    assert_eq!(workers, 1);
                } else {
                    assert_eq!(workers, 0);
                }
                started += workers;
            }
            assert_eq!(started, MAX_CONCURRENT_THUMBNAIL_LOADS);
            assert_eq!(pipeline.running_workers, MAX_CONCURRENT_THUMBNAIL_LOADS);
        });
    }
}
