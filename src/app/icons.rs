use leptos::prelude::*;

fn native(glyph: &'static str) -> AnyView {
    view! {
        <span
            class="native-icon inline-grid size-[18px] shrink-0 place-items-center font-['Segoe_Fluent_Icons','Segoe_MDL2_Assets'] text-base font-normal not-italic leading-none"
            aria-hidden="true"
        >
            {glyph}
        </span>
    }
    .into_any()
}

pub(super) fn search() -> AnyView {
    native("\u{E721}")
}

pub(super) fn open() -> AnyView {
    native("\u{E8A7}")
}

pub(super) fn folder_open() -> AnyView {
    native("\u{E838}")
}

pub(super) fn trash() -> AnyView {
    native("\u{E74D}")
}

pub(super) fn home() -> AnyView {
    native("\u{E80F}")
}

pub(super) fn clock() -> AnyView {
    native("\u{E823}")
}

pub(super) fn document() -> AnyView {
    native("\u{E8A5}")
}

pub(super) fn image() -> AnyView {
    native("\u{E8B9}")
}

pub(super) fn video() -> AnyView {
    native("\u{E714}")
}

pub(super) fn audio() -> AnyView {
    native("\u{E8D6}")
}

pub(super) fn archive() -> AnyView {
    native("\u{F012}")
}

pub(super) fn settings() -> AnyView {
    native("\u{E713}")
}

pub(super) fn copy() -> AnyView {
    native("\u{E8C8}")
}

pub(super) fn edit() -> AnyView {
    native("\u{E8AC}")
}

pub(super) fn warning() -> AnyView {
    native("\u{E7BA}")
}

pub(super) fn empty() -> AnyView {
    native("\u{E8A5}")
}

pub(super) fn minimize() -> AnyView {
    native("\u{E921}")
}

pub(super) fn maximize() -> AnyView {
    native("\u{E922}")
}

pub(super) fn close() -> AnyView {
    native("\u{E8BB}")
}
