use super::FileOperations;
use crate::app::search_workspace::results::ResultSelection;
use crate::app::search_workspace::search::SearchResults;
use leptos::prelude::*;
use wasm_bindgen::JsCast;
use web_sys::{HtmlInputElement, KeyboardEvent};

#[component]
#[allow(
    non_snake_case,
    reason = "Leptos components conventionally use PascalCase names"
)]
pub(crate) fn FileActionDialogs(
    files: FileOperations,
    selection: ResultSelection,
    results: SearchResults,
) -> impl IntoView {
    let rename_target = files.rename_target;
    let rename_value = files.rename_value;

    view! {
        <Show when=move || rename_target.get().is_some()>
            {move || rename_target.get().map(|item| view! {
                <div class="modal-backdrop fixed inset-0 z-[200] grid place-items-center bg-black/35 p-6 backdrop-blur-[4px]" on:click=move |_| files.cancel_rename()>
                    <div class="modal-card grid w-[min(440px,100%)] select-text gap-3 rounded-xl border border-[var(--border)] bg-[var(--surface-solid)] p-5 shadow-[var(--shadow)] [&>h2]:m-0 [&>h2]:text-lg [&>h2]:font-semibold [&>p]:m-0 [&>p]:leading-[1.45] [&>p]:text-[var(--muted)]" role="dialog" aria-modal="true" aria-label="Rename" on:click=move |event| event.stop_propagation()>
                        <h2>"Rename"</h2>
                        <p class="modal-description m-0 overflow-hidden text-ellipsis whitespace-nowrap text-xs text-[var(--muted)]">{item.full_path.clone()}</p>
                        <input
                            class="modal-input h-9 w-full select-text rounded-[7px] border border-[var(--border)] bg-[var(--surface-2)] px-[10px] outline-none focus:border-[var(--border)] focus:shadow-none"
                            type="text"
                            prop:value=move || rename_value.get()
                            on:input=move |event| {
                                if let Some(input) = event.target().and_then(|target| target.dyn_into::<HtmlInputElement>().ok()) {
                                    rename_value.set(input.value());
                                }
                            }
                            on:keydown=move |event: KeyboardEvent| {
                                match event.key().as_str() {
                                    "Enter" => {
                                        event.prevent_default();
                                        files.submit_rename(results);
                                    }
                                    "Escape" => {
                                        event.prevent_default();
                                        files.cancel_rename();
                                    }
                                    _ => {}
                                }
                            }
                            autofocus
                        />
                        <div class="modal-actions mt-1 flex justify-end gap-2">
                            <button class="dialog-button h-[34px] min-w-[88px] rounded-[7px] border border-[var(--border)] bg-[var(--surface-2)] px-[14px] hover:bg-[var(--hover)] focus-visible:bg-[var(--hover)]" on:click=move |_| files.cancel_rename()>"Cancel"</button>
                            <button class="dialog-button primary h-[34px] min-w-[88px] rounded-[7px] border border-[var(--accent)] bg-[var(--accent)] px-[14px] text-white focus-visible:brightness-[1.12]" on:click=move |_| files.submit_rename(results)>"Rename"</button>
                        </div>
                    </div>
                </div>
            })}
        </Show>

        <Show when=move || files.trash_is_preparing()>
            <div class="modal-backdrop fixed inset-0 z-[200] grid place-items-center bg-black/35 p-6 backdrop-blur-[4px]">
                <div class="modal-card grid w-[min(440px,100%)] select-text gap-3 rounded-xl border border-[var(--border)] bg-[var(--surface-solid)] p-5 shadow-[var(--shadow)] [&>h2]:m-0 [&>h2]:text-lg [&>h2]:font-semibold [&>p]:m-0 [&>p]:leading-[1.45] [&>p]:text-[var(--muted)]" role="status" aria-live="polite">
                    <h2>"Preparing deletion…"</h2>
                    <p>"Everything Modern is capturing an immutable list of the selected files."</p>
                </div>
            </div>
        </Show>

        <Show when=move || files.pending_trash().is_some()>
            {move || files.pending_trash().map(|pending| view! {
                <div class="modal-backdrop fixed inset-0 z-[200] grid place-items-center bg-black/35 p-6 backdrop-blur-[4px]">
                    <div class="modal-card grid w-[min(440px,100%)] select-text gap-3 rounded-xl border border-[var(--border)] bg-[var(--surface-solid)] p-5 shadow-[var(--shadow)] [&>h2]:m-0 [&>h2]:text-lg [&>h2]:font-semibold [&>p]:m-0 [&>p]:leading-[1.45] [&>p]:text-[var(--muted)]" role="alertdialog" aria-modal="true" aria-label="Confirmation de suppression" on:click=move |event| event.stop_propagation()>
                        <h2>"Move to Recycle Bin?"</h2>
                        <p>{format!("{} item(s) will be moved to the Recycle Bin.", pending.count)}</p>
                        <div class="modal-actions mt-1 flex justify-end gap-2">
                            <button class="dialog-button h-[34px] min-w-[88px] rounded-[7px] border border-[var(--border)] bg-[var(--surface-2)] px-[14px] hover:bg-[var(--hover)] focus-visible:bg-[var(--hover)]" disabled=move || files.trash_is_deleting() on:click=move |_| files.cancel_trash()>"Cancel"</button>
                            <button class="dialog-button danger h-[34px] min-w-[88px] rounded-[7px] border border-[#c42b1c] bg-[#c42b1c] px-[14px] text-white focus-visible:brightness-[1.12]" disabled=move || files.trash_is_deleting() on:click=move |_| files.submit_trash(selection, results)>
                                {move || if files.trash_is_deleting() { "Deleting…" } else { "Move to Recycle Bin" }}
                            </button>
                        </div>
                    </div>
                </div>
            })}
        </Show>
    }
}
