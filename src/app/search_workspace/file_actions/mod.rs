mod dialogs;

pub(super) use dialogs::FileActionDialogs;

use super::results::ResultSelection;
use super::search::SearchResults;
use crate::backend;
use everything_core::{validate_windows_name, SearchResult};
use gloo_timers::future::TimeoutFuture;
use leptos::prelude::*;
use leptos::task::spawn_local;

const INDEX_SETTLE_DELAY_MS: u32 = 500;

#[derive(Clone)]
pub(super) struct PendingTrash {
    pub count: usize,
    pub snapshot_id: u64,
}

#[derive(Clone, Default)]
enum TrashWorkflow {
    #[default]
    Idle,
    Preparing,
    AwaitingConfirmation(PendingTrash),
    Deleting(PendingTrash),
}

#[derive(Clone, Copy)]
pub(super) struct FileOperations {
    pub error: RwSignal<Option<String>>,
    pub rename_target: RwSignal<Option<SearchResult>>,
    pub rename_value: RwSignal<String>,
    trash: RwSignal<TrashWorkflow>,
}

impl FileOperations {
    pub fn new() -> Self {
        Self {
            error: RwSignal::new(None),
            rename_target: RwSignal::new(None),
            rename_value: RwSignal::new(String::new()),
            trash: RwSignal::new(TrashWorkflow::Idle),
        }
    }

    pub fn trash_is_preparing(self) -> bool {
        matches!(self.trash.get(), TrashWorkflow::Preparing)
    }

    pub fn pending_trash(self) -> Option<PendingTrash> {
        match self.trash.get() {
            TrashWorkflow::AwaitingConfirmation(pending) | TrashWorkflow::Deleting(pending) => {
                Some(pending)
            }
            TrashWorkflow::Idle | TrashWorkflow::Preparing => None,
        }
    }

    pub fn trash_is_deleting(self) -> bool {
        matches!(self.trash.get(), TrashWorkflow::Deleting(_))
    }

    pub fn reset_for_new_search(self) {
        self.rename_target.set(None);
        let pending = match self.trash.get_untracked() {
            TrashWorkflow::AwaitingConfirmation(pending) | TrashWorkflow::Deleting(pending) => {
                pending
            }
            TrashWorkflow::Idle | TrashWorkflow::Preparing => return,
        };
        self.trash.set(TrashWorkflow::Idle);
        spawn_local(async move {
            if let Err(message) = backend::cancel_trash(pending.snapshot_id).await {
                self.error.set(Some(message));
            }
        });
    }

    pub fn open(self, path: String) {
        spawn_local(async move {
            if let Err(message) = backend::open(&path).await {
                self.error.set(Some(message));
            }
        });
    }

    pub fn reveal(self, path: String) {
        spawn_local(async move {
            if let Err(message) = backend::reveal(&path).await {
                self.error.set(Some(message));
            }
        });
    }

    pub fn copy(self, text: String) {
        spawn_local(async move {
            if let Err(message) = backend::copy_text(&text).await {
                self.error.set(Some(message));
            }
        });
    }

    pub fn copy_files(self, selection: ResultSelection, results: SearchResults) {
        let indices = selection.indices.get_untracked();
        if indices.is_empty() {
            return;
        }

        let request = results.selection_request(indices.ranges());
        spawn_local(async move {
            if let Err(message) = backend::copy_files(request).await {
                self.error.set(Some(message));
            }
        });
    }

    pub fn begin_rename(self, item: SearchResult) {
        self.rename_value.set(item.name.clone());
        self.rename_target.set(Some(item));
    }

    pub fn cancel_rename(self) {
        self.rename_target.set(None);
    }

    pub fn submit_rename(self, results: SearchResults) {
        let Some(item) = self.rename_target.get_untracked() else {
            return;
        };
        let new_name = self.rename_value.get_untracked();
        if new_name == item.name {
            self.cancel_rename();
            return;
        }
        if let Err(error) = validate_windows_name(&new_name) {
            self.error.set(Some(error.to_string()));
            return;
        }

        spawn_local(async move {
            match backend::rename(&item.full_path, &new_name).await {
                Ok(_) => {
                    self.cancel_rename();
                    results.refresh();
                }
                Err(message) => self.error.set(Some(message)),
            }
        });
    }

    pub fn begin_trash(self, selection: ResultSelection, results: SearchResults) {
        let indices = selection.indices.get_untracked();
        if indices.is_empty() || !matches!(self.trash.get_untracked(), TrashWorkflow::Idle) {
            return;
        }

        self.trash.set(TrashWorkflow::Preparing);
        let request = results.selection_request(indices.ranges());
        spawn_local(async move {
            match backend::prepare_trash(request).await {
                Ok(prepared) => self
                    .trash
                    .set(TrashWorkflow::AwaitingConfirmation(PendingTrash {
                        count: prepared.count,
                        snapshot_id: prepared.snapshot_id,
                    })),
                Err(message) => {
                    self.trash.set(TrashWorkflow::Idle);
                    self.error.set(Some(message));
                }
            }
        });
    }

    pub fn cancel_trash(self) {
        let TrashWorkflow::AwaitingConfirmation(pending) = self.trash.get_untracked() else {
            return;
        };
        self.trash.set(TrashWorkflow::Idle);
        spawn_local(async move {
            if let Err(message) = backend::cancel_trash(pending.snapshot_id).await {
                self.error.set(Some(message));
            }
        });
    }

    pub fn submit_trash(self, selection: ResultSelection, results: SearchResults) {
        let TrashWorkflow::AwaitingConfirmation(pending) = self.trash.get_untracked() else {
            return;
        };
        self.trash.set(TrashWorkflow::Deleting(pending.clone()));
        spawn_local(async move {
            match backend::execute_trash(pending.snapshot_id).await {
                Ok(outcome) => {
                    self.trash.set(TrashWorkflow::Idle);
                    results.suppress_deleted(outcome.deleted_paths);
                    selection.clear();
                    if !outcome.failures.is_empty() {
                        self.error.set(Some(format!(
                            "{} item(s) deleted, {} failure(s):\n{}",
                            outcome.deleted,
                            outcome.failures.len(),
                            outcome.failures.join("\n")
                        )));
                    }
                    TimeoutFuture::new(INDEX_SETTLE_DELAY_MS).await;
                    results.refresh_incrementally();
                }
                Err(message) => {
                    self.trash.set(TrashWorkflow::AwaitingConfirmation(pending));
                    self.error.set(Some(message));
                }
            }
        });
    }
}
