use leptos::prelude::*;

fn svg(path: &'static str) -> AnyView {
    view! { <svg viewBox="0 0 24 24" aria-hidden="true"><path d=path></path></svg> }.into_any()
}

pub(super) fn search() -> AnyView {
    svg("M10.8 4.2a6.6 6.6 0 1 0 4.08 11.79l4.32 4.32 1.41-1.42-4.32-4.31A6.6 6.6 0 0 0 10.8 4.2Zm0 2a4.6 4.6 0 1 1 0 9.2 4.6 4.6 0 0 1 0-9.2Z")
}

pub(super) fn open() -> AnyView {
    svg("M5 4h6v2H6v12h12v-5h2v7H4V4h1Zm8-1h8v8h-2V6.41l-8.3 8.3-1.4-1.42L17.58 5H13V3Z")
}

pub(super) fn folder_open() -> AnyView {
    svg("M3 5h7l2 2h9v3h-2V9h-7.8l-2-2H5v10.2L7.2 11H22l-3.4 9H3V5Zm4.2 8L5.4 18h11.8l1.9-5H7.2Z")
}

pub(super) fn trash() -> AnyView {
    svg("M8 4V2h8v2h5v2H3V4h5Zm-2 4h12l-1 14H7L6 8Zm3 2 .6 10h1L10 10H9Zm5 0-.6 10h1L15 10h-1Z")
}

pub(super) fn home() -> AnyView {
    svg("m12 3 9 8h-3v10h-5v-6h-2v6H6V11H3l9-8Zm0 2.7L7.5 9.7V19H9v-6h6v6h1.5V9.7L12 5.7Z")
}

pub(super) fn clock() -> AnyView {
    svg("M12 2a10 10 0 1 0 0 20 10 10 0 0 0 0-20Zm0 2a8 8 0 1 1 0 16 8 8 0 0 1 0-16Zm-1 3v6l5 3 1-1.7-4-2.3V7h-2Z")
}

pub(super) fn document() -> AnyView {
    svg("M6 2h8l5 5v15H6V2Zm2 2v16h9V8h-4V4H8Zm7 .4V6h1.6L15 4.4ZM10 11h5v2h-5v-2Zm0 4h5v2h-5v-2Z")
}

pub(super) fn image() -> AnyView {
    svg("M4 3h16a2 2 0 0 1 2 2v14a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2Zm0 2v11l4-4 3 3 3-3 6 6V5H4Zm3 2a2 2 0 1 1 0 4 2 2 0 0 1 0-4Z")
}

pub(super) fn video() -> AnyView {
    svg("M4 5h12a2 2 0 0 1 2 2v2l4-2v10l-4-2v2a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2V7a2 2 0 0 1 2-2Zm0 2v10h12V7H4Zm14 4.2v1.6l2 .9V10.3l-2 .9Z")
}

pub(super) fn audio() -> AnyView {
    svg("M12 3v10.55A4 4 0 1 0 14 17V7h5V3h-7Zm-4 12a2 2 0 1 1 0 4 2 2 0 0 1 0-4Zm6-10h3v2h-3V5Z")
}

pub(super) fn archive() -> AnyView {
    svg("M4 3h16v5h-1v13H5V8H4V3Zm2 2v1h12V5H6Zm1 3v11h10V8H7Zm3 3h4v2h-4v-2Z")
}

pub(super) fn copy() -> AnyView {
    svg("M8 7h12v15H8V7Zm2 2v11h8V9h-8ZM4 2h12v3h-2V4H6v11h1v2H4V2Z")
}

pub(super) fn edit() -> AnyView {
    svg("m16.7 3.3 4 4L9 19H5v-4L16.7 3.3Zm0 2.8L7 15.8V17h1.2l9.7-9.7-1.2-1.2ZM4 21h16v2H4v-2Z")
}

pub(super) fn warning() -> AnyView {
    svg("M12 2 1 21h22L12 2Zm0 4 7.5 13h-15L12 6Zm-1 4v5h2v-5h-2Zm0 7v2h2v-2h-2Z")
}

pub(super) fn empty() -> AnyView {
    svg("M4 4h16v16H4V4Zm2 2v12h12V6H6Zm2 3h8v2H8V9Zm0 4h5v2H8v-2Z")
}

pub(super) fn minimize() -> AnyView {
    svg("M5 12h14v1H5v-1Z")
}

pub(super) fn maximize() -> AnyView {
    svg("M5 5h14v14H5V5Zm1.5 1.5v11h11v-11h-11Z")
}

pub(super) fn close() -> AnyView {
    svg("m6.7 5.3 5.3 5.3 5.3-5.3 1.4 1.4-5.3 5.3 5.3 5.3-1.4 1.4-5.3-5.3-5.3 5.3-1.4-1.4 5.3-5.3-5.3-5.3 1.4-1.4Z")
}
