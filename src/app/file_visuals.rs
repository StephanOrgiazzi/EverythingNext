use leptos::prelude::*;
use leptos::task::spawn_local;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};

use super::visual_queue::{next_animation_frame, request_thumbnail};
use crate::backend;
use crate::diagnostics;

#[component]
pub(super) fn FileIcon(path: String) -> impl IntoView {
    let source = RwSignal::new(None::<String>);
    let cancelled = Arc::new(AtomicBool::new(false));
    let cancelled_on_cleanup = cancelled.clone();
    on_cleanup(move || cancelled_on_cleanup.store(true, Ordering::Relaxed));

    Effect::new(move |_| {
        let path = path.clone();
        let cancelled = cancelled.clone();
        spawn_local(async move {
            next_animation_frame().await;
            if cancelled.load(Ordering::Relaxed) {
                return;
            }
            match backend::icon(&path).await {
                Ok(icon) if !cancelled.load(Ordering::Relaxed) => source.set(icon),
                Ok(_) => {}
                Err(error) => {
                    diagnostics::warn(&format!("Unable to load icon for '{path}': {error}"))
                }
            }
        });
    });

    view! {
        <span class="file-icon">
            {move || source.get().map(|source| view! {
                <img src=source alt="" loading="lazy" decoding="async" />
            })}
        </span>
    }
}

#[component]
pub(super) fn FileVisual(
    path: String,
    visual_size: u32,
    file_size: Option<u64>,
    modified_unix: Option<i64>,
    load: bool,
) -> impl IntoView {
    let subscription = load.then(|| {
        let pixel_ratio = web_sys::window()
            .map(|window| window.device_pixel_ratio())
            .unwrap_or(1.0);
        let pixel_size = ((visual_size as f64 * pixel_ratio).ceil() as u32).clamp(32, 256);
        request_thumbnail(path.clone(), pixel_size, file_size, modified_unix)
    });

    if let Some(subscription) = subscription.as_ref() {
        let subscription = subscription.clone();
        on_cleanup(move || subscription.cancel());
    }

    if let Some(subscription) = subscription {
        let source = subscription.source;
        let animate_reveal = subscription.animate_reveal;
        view! {
            <span class="icon-result-visual thumbnail-stack">
                <FileIcon path=path />
                {move || source.get().flatten().map(|source| view! {
                    <img
                        class="file-visual-image"
                        class:thumbnail-reveal=animate_reveal
                        src=source
                        alt=""
                        loading="lazy"
                        decoding="async"
                    />
                })}
            </span>
        }
        .into_any()
    } else {
        view! {
            <span class="icon-result-visual">
                <span class="file-visual-placeholder" aria-hidden="true"></span>
            </span>
        }
        .into_any()
    }
}
