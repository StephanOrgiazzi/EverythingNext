use crate::{backend, diagnostics};
use leptos::prelude::*;
use leptos::task::spawn_local;
use std::cell::RefCell;
use std::collections::{HashMap, VecDeque};
use std::rc::Rc;
use wasm_bindgen::{closure::Closure, JsCast, JsValue};
use wasm_bindgen_futures::JsFuture;
use web_sys::HtmlImageElement;

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
struct ThumbnailKey {
    path: String,
    file_size: Option<u64>,
    modified_unix: Option<i64>,
}

type ThumbnailSource = ArcRwSignal<Option<Option<String>>>;

struct ThumbnailResource {
    source: ThumbnailSource,
    decoded_image: RefCell<Option<HtmlImageElement>>,
}

impl ThumbnailResource {
    fn new() -> Self {
        Self {
            source: ArcRwSignal::new(None),
            decoded_image: RefCell::new(None),
        }
    }

    async fn publish(&self, source: Option<String>) {
        if let Some(source) = source {
            match decode_image(&source).await {
                Ok(image) => {
                    self.decoded_image.replace(Some(image));
                }
                Err(error) => {
                    diagnostics::warn_js("Unable to pre-decode a thumbnail.", &error);
                }
            }
            self.source.set(Some(Some(source)));
        } else {
            self.source.set(Some(None));
        }
    }
}

type ThumbnailResourceRef = Rc<ThumbnailResource>;

#[derive(Clone)]
pub(super) struct ThumbnailSubscription {
    pub source: ThumbnailSource,
    pub animate_reveal: bool,
}

#[derive(Default)]
struct ThumbnailPipeline {
    pending: VecDeque<ThumbnailKey>,
    running: bool,
    resources: HashMap<ThumbnailKey, ThumbnailResourceRef>,
}

impl ThumbnailPipeline {
    fn subscribe(&mut self, key: ThumbnailKey) -> (ThumbnailSource, bool, bool) {
        if let Some(resource) = self.resources.get(&key) {
            return (
                resource.source.clone(),
                resource.source.get_untracked().is_some(),
                false,
            );
        }

        let resource = Rc::new(ThumbnailResource::new());
        let source = resource.source.clone();
        self.resources.insert(key.clone(), resource);
        self.pending.push_back(key);
        let start_worker = !self.running;
        self.running = true;
        (source, false, start_worker)
    }

    fn next_request(&mut self) -> Option<(ThumbnailKey, ThumbnailResourceRef)> {
        let Some(key) = self.pending.pop_front() else {
            self.running = false;
            return None;
        };
        let resource = self
            .resources
            .get(&key)
            .expect("a queued thumbnail always has a cached source")
            .clone();
        Some((key, resource))
    }
}

thread_local! {
    static PIPELINE: RefCell<ThumbnailPipeline> = RefCell::new(ThumbnailPipeline::default());
}

async fn process_thumbnail_queue() {
    next_animation_frame().await;

    loop {
        let request = PIPELINE.with(|pipeline| pipeline.borrow_mut().next_request());
        let Some((key, resource)) = request else {
            return;
        };

        match backend::visual(&key.path, true).await {
            Ok(visual) => resource.publish(visual).await,
            Err(error) => {
                diagnostics::warn(&format!(
                    "Unable to load visual for '{}': {error}",
                    key.path
                ));
                resource.source.set(Some(None));
            }
        }
        next_animation_frame().await;
    }
}

async fn decode_image(source: &str) -> Result<HtmlImageElement, JsValue> {
    let image = HtmlImageElement::new()?;
    image.set_decoding("sync");
    image.set_src(source);
    JsFuture::from(image.decode()).await?;
    Ok(image)
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
    file_size: Option<u64>,
    modified_unix: Option<i64>,
) -> ThumbnailSubscription {
    let key = ThumbnailKey {
        path,
        file_size,
        modified_unix,
    };
    let (source, loaded, start_worker) =
        PIPELINE.with(|pipeline| pipeline.borrow_mut().subscribe(key));
    if start_worker {
        spawn_local(process_thumbnail_queue());
    }
    ThumbnailSubscription {
        source,
        animate_reveal: !loaded,
    }
}

#[cfg(test)]
mod tests {
    use super::{ThumbnailKey, ThumbnailPipeline};
    use leptos::prelude::*;

    fn key(path: &str, modified_unix: i64) -> ThumbnailKey {
        ThumbnailKey {
            path: path.into(),
            file_size: Some(10),
            modified_unix: Some(modified_unix),
        }
    }

    #[test]
    fn cache_key_changes_with_the_file_version() {
        assert_ne!(key("track.mp3", 1), key("track.mp3", 2));
    }

    #[test]
    fn subscriptions_share_one_source_and_one_request() {
        Owner::new().with(|| {
            let mut pipeline = ThumbnailPipeline::default();
            let item = key("track.mp3", 1);
            let (first_source, first_loaded, start_worker) = pipeline.subscribe(item.clone());
            let (second_source, second_loaded, second_worker) = pipeline.subscribe(item);

            assert!(!first_loaded);
            assert!(start_worker);
            assert!(!second_loaded);
            assert!(!second_worker);
            assert_eq!(pipeline.pending.len(), 1);

            first_source.set(Some(Some("thumbnail".into())));
            assert_eq!(
                second_source.get_untracked(),
                Some(Some("thumbnail".into()))
            );
        });
    }

    #[test]
    fn loaded_sources_remain_cached_for_the_session() {
        Owner::new().with(|| {
            let mut pipeline = ThumbnailPipeline::default();
            let item = key("track.mp3", 1);
            let (source, _, _) = pipeline.subscribe(item.clone());
            source.set(Some(Some("thumbnail".into())));
            let (cached, loaded, start_worker) = pipeline.subscribe(item);

            assert!(loaded);
            assert!(!start_worker);
            assert_eq!(cached.get_untracked(), Some(Some("thumbnail".into())));
            assert_eq!(pipeline.resources.len(), 1);
            assert_eq!(pipeline.pending.len(), 1);
        });
    }
}
