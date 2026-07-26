use crate::app::search_workspace::search::SearchResults;
use everything_core::{IndexSelection, SearchResult};
use leptos::prelude::*;

#[derive(Clone, Copy)]
pub(crate) enum FocusMove {
    Relative(i32),
    Absolute(u32),
}

#[derive(Clone, Copy)]
pub(crate) struct SelectionModifiers {
    pub extend: bool,
    pub preserve: bool,
}

#[derive(Clone, Copy)]
pub(crate) struct ResultSelection {
    pub indices: RwSignal<IndexSelection>,
    pub focused_index: RwSignal<Option<u32>>,
    pub anchor: RwSignal<Option<u32>>,
}

impl ResultSelection {
    pub fn new() -> Self {
        Self {
            indices: RwSignal::new(IndexSelection::default()),
            focused_index: RwSignal::new(None),
            anchor: RwSignal::new(None),
        }
    }

    pub fn clear(self) {
        self.indices.set(IndexSelection::default());
        self.focused_index.set(None);
        self.anchor.set(None);
    }

    pub fn clear_indices(self) {
        self.indices.set(IndexSelection::default());
        self.anchor.set(None);
    }

    pub fn select_all(self, total: u32) {
        self.indices.update(|selection| selection.select_all(total));
        if total > 0 && self.focused_index.get_untracked().is_none() {
            self.focused_index.set(Some(0));
            self.anchor.set(Some(0));
        }
    }

    pub fn toggle_focused(self) {
        if let Some(index) = self.focused_index.get_untracked() {
            self.indices.update(|selection| selection.toggle(index));
            self.anchor.set(Some(index));
        }
    }

    pub fn select_row(self, index: u32, modifiers: SelectionModifiers) {
        self.focused_index.set(Some(index));
        if modifiers.extend {
            let anchor = self.anchor.get_untracked().unwrap_or(index);
            self.indices.update(|selection| {
                if modifiers.preserve {
                    selection.add_range(anchor, index);
                } else {
                    selection.select_range(anchor, index);
                }
            });
        } else if modifiers.preserve {
            self.indices.update(|selection| selection.toggle(index));
            self.anchor.set(Some(index));
        } else {
            self.indices
                .update(|selection| selection.select_only(index));
            self.anchor.set(Some(index));
        }
    }

    pub fn focus(
        self,
        movement: FocusMove,
        modifiers: SelectionModifiers,
        total: u32,
    ) -> Option<u32> {
        if total == 0 {
            return None;
        }

        let next = match movement {
            FocusMove::Absolute(index) => index.min(total - 1),
            FocusMove::Relative(delta) => match self.focused_index.get_untracked() {
                Some(current) => (current as i64 + delta as i64).clamp(0, total as i64 - 1) as u32,
                None if delta < 0 => total - 1,
                None => 0,
            },
        };

        self.focused_index.set(Some(next));
        if modifiers.extend {
            let anchor = self.anchor.get_untracked().unwrap_or(next);
            self.indices.update(|selection| {
                if modifiers.preserve {
                    selection.add_range(anchor, next);
                } else {
                    selection.select_range(anchor, next);
                }
            });
        } else if !modifiers.preserve {
            self.anchor.set(Some(next));
            self.indices.update(|selection| selection.select_only(next));
        }
        Some(next)
    }

    pub fn focused_item(self, results: SearchResults) -> Option<SearchResult> {
        self.focused_index
            .get_untracked()
            .and_then(|index| results.item_at(index))
    }

    pub fn select_context_item(self, index: u32) {
        self.indices.update(|selection| {
            if !selection.contains(index) {
                selection.select_only(index);
            }
        });
        self.focused_index.set(Some(index));
        self.anchor.set(Some(index));
    }
}
