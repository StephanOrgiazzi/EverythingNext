use crate::{backend, diagnostics};
use leptos::prelude::*;
use leptos::task::spawn_local;
use std::cell::RefCell;
use std::collections::HashMap;

use super::visual_queue::request_thumbnail;

type IconSource = ArcRwSignal<Option<Option<String>>>;

thread_local! {
    static ICON_SOURCES: RefCell<HashMap<String, IconSource>> = RefCell::new(HashMap::new());
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

fn icon_source(key: &str) -> (IconSource, bool) {
    ICON_SOURCES.with(|sources| {
        let mut sources = sources.borrow_mut();
        if let Some(source) = sources.get(key) {
            return (source.clone(), false);
        }

        let source = ArcRwSignal::new(None);
        sources.insert(key.to_string(), source.clone());
        (source, true)
    })
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

#[component]
pub(crate) fn FileIcon(path: String, is_dir: bool) -> impl IntoView {
    let cache_key = icon_cache_key(&path, is_dir);
    let (source, should_load) = icon_source(&cache_key);
    if should_load {
        let source_for_load = source.clone();
        let path = path.clone();
        spawn_local(async move {
            match backend::visual(&path, false).await {
                Ok(icon) => source_for_load.set(Some(icon)),
                Err(error) => {
                    diagnostics::warn(&format!("Unable to load icon for '{path}': {error}"));
                    source_for_load.set(Some(None));
                }
            }
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
    file_size: Option<u64>,
    modified_unix: Option<i64>,
    load: bool,
) -> impl IntoView {
    let subscription = load.then(|| request_thumbnail(path.clone(), file_size, modified_unix));

    if let Some(subscription) = subscription {
        let source = subscription.source;
        let animate_reveal = subscription.animate_reveal;
        view! {
            <span class="icon-result-visual thumbnail-stack grid size-[var(--view-icon-size)] shrink-0 place-items-center [&>*]:[grid-area:1/1]">
                {move || match source.get().flatten() {
                    Some(source) => view! {
                        <img
                            class="file-visual-image size-full object-contain"
                            class:thumbnail-reveal=animate_reveal
                            src=source
                            alt=""
                            loading="eager"
                            decoding=if animate_reveal { "async" } else { "sync" }
                        />
                    }.into_any(),
                    None => fallback_icon(is_dir),
                }}
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
