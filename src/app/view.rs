use leptos::prelude::*;

use super::search_workspace::SearchWorkspace;

#[component]
#[allow(
    non_snake_case,
    reason = "Leptos components conventionally use PascalCase names"
)]
pub fn App() -> impl IntoView {
    view! { <SearchWorkspace /> }
}
