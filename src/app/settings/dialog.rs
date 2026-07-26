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
            <div class="modal-backdrop fixed inset-0 z-[200] grid place-items-center bg-black/35 p-6 backdrop-blur-[4px]" on:click=move |_| open.set(false)>
                <div
                    class="modal-card settings-modal grid max-h-[min(720px,calc(100vh-48px))] w-[min(620px,100%)] select-text gap-0 overflow-auto rounded-xl border border-[var(--border)] bg-[var(--surface-solid)] p-5 shadow-[var(--shadow)] [&>h2]:mb-2 [&>h2]:mt-0 [&>h2]:text-lg [&>h2]:font-semibold"
                    role="dialog"
                    aria-modal="true"
                    aria-labelledby="settings-title"
                    on:click=move |event| event.stop_propagation()
                >
                    <h2 id="settings-title">"Settings"</h2>
                    <section class="settings-section grid gap-[10px] border-b border-[var(--border-soft)] py-4 [&>h3]:m-0 [&>h3]:text-sm [&>h3]:font-semibold" data-setting="theme" aria-labelledby="theme-setting-title">
                        <h3 id="theme-setting-title">"Theme"</h3>
                        <div class="settings-control min-w-0" data-settings-control="theme">
                            <ThemeSetting state=theme />
                        </div>
                    </section>
                    <section class="settings-section grid gap-[10px] border-b border-[var(--border-soft)] py-4 last-of-type:border-b-0 [&>h3]:m-0 [&>h3]:text-sm [&>h3]:font-semibold" data-setting="excluded-folders" aria-labelledby="excluded-folders-setting-title">
                        <h3 id="excluded-folders-setting-title">"Excluded folders"</h3>
                        <div class="settings-control min-w-0" data-settings-control="excluded-folders">
                            <ExcludedFoldersSetting state=excluded_folders />
                        </div>
                    </section>
                    <div class="modal-actions mt-2 flex justify-end gap-2">
                        <button
                            type="button"
                            class="dialog-button h-[34px] min-w-[88px] rounded-[7px] border border-[var(--border)] bg-[var(--surface-2)] px-[14px] hover:bg-[var(--hover)] focus-visible:bg-[var(--hover)]"
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
