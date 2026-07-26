use leptos::prelude::*;

use super::{ExcludedFoldersSetting, ExcludedFoldersState, ThemeSetting, ThemeState};

#[component]
#[allow(
    non_snake_case,
    reason = "Leptos components conventionally use PascalCase names"
)]
pub(in crate::app) fn SettingsDialog(
    open: RwSignal<bool>,
    theme: ThemeState,
    excluded_folders: ExcludedFoldersState,
) -> impl IntoView {
    view! {
        <Show when=move || open.get()>
            <div class="modal-backdrop" on:click=move |_| open.set(false)>
                <div
                    class="modal-card settings-modal"
                    role="dialog"
                    aria-modal="true"
                    aria-labelledby="settings-title"
                    on:click=move |event| event.stop_propagation()
                >
                    <h2 id="settings-title">"Settings"</h2>
                    <section class="settings-section" data-setting="theme" aria-labelledby="theme-setting-title">
                        <h3 id="theme-setting-title">"Theme"</h3>
                        <div class="settings-control" data-settings-control="theme">
                            <ThemeSetting state=theme />
                        </div>
                    </section>
                    <section class="settings-section" data-setting="excluded-folders" aria-labelledby="excluded-folders-setting-title">
                        <h3 id="excluded-folders-setting-title">"Excluded folders"</h3>
                        <div class="settings-control" data-settings-control="excluded-folders">
                            <ExcludedFoldersSetting state=excluded_folders />
                        </div>
                    </section>
                    <div class="modal-actions">
                        <button
                            type="button"
                            class="dialog-button"
                            autofocus
                            on:click=move |_| open.set(false)
                        >
                            "Close"
                        </button>
                    </div>
                </div>
            </div>
        </Show>
    }
}
