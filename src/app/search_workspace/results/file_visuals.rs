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

fn icon_source(path: &str, is_dir: bool) -> (IconSource, bool) {
    let key = icon_cache_key(path, is_dir);
    ICON_SOURCES.with(|sources| {
        let mut sources = sources.borrow_mut();
        if let Some(source) = sources.get(&key) {
            return (source.clone(), false);
        }

        let source = ArcRwSignal::new(None);
        sources.insert(key, source.clone());
        (source, true)
    })
}

#[component]
pub(crate) fn FileIcon(path: String, is_dir: bool) -> impl IntoView {
    let (source, should_load) = icon_source(&path, is_dir);
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
            {move || source.get().flatten().map(|source| view! {
                <img src=source alt="" loading="lazy" decoding="async" />
            })}
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
        let fallback_source = source.clone();
        let animate_reveal = subscription.animate_reveal;
        let fallback_path = path.clone();
        view! {
            <span class="icon-result-visual thumbnail-stack grid size-[var(--view-icon-size)] shrink-0 place-items-center [&>*]:[grid-area:1/1]">
                {move || matches!(fallback_source.get(), None | Some(None)).then(|| view! {
                    <FileIcon path=fallback_path.clone() is_dir />
                })}
                {move || source.get().flatten().map(|source| view! {
                    <img
                        class="file-visual-image size-full object-contain"
                        class:thumbnail-reveal=animate_reveal
                        src=source
                        alt=""
                        loading="eager"
                        decoding=if animate_reveal { "async" } else { "sync" }
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
