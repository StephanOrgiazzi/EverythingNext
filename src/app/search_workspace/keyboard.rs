use super::file_actions::FileOperations;
use super::results::{
    event_target_is_interactive, FocusMove, ResultContextMenu, ResultSelection, ResultViewport,
    SelectionModifiers,
};
use super::search::SearchResults;
use crate::diagnostics;
use leptos::prelude::*;
use web_sys::KeyboardEvent;

#[derive(Clone, Copy)]
pub(super) struct KeyboardContext {
    pub settings_open: RwSignal<bool>,
    pub search_ref: NodeRef<leptos::html::Input>,
    pub selection: ResultSelection,
    pub results: SearchResults,
    pub viewport: ResultViewport,
    pub files: FileOperations,
    pub menu: ResultContextMenu,
}

impl KeyboardContext {
    pub fn handle_keydown(self, event: KeyboardEvent) {
        let key = event.key();
        if key == "Escape" && self.settings_open.get_untracked() {
            event.prevent_default();
            self.settings_open.set(false);
            return;
        }

        if event.ctrl_key() && key.eq_ignore_ascii_case("l") {
            event.prevent_default();
            if let Some(input) = self.search_ref.get() {
                if let Err(error) = input.focus() {
                    diagnostics::warn_js("Unable to focus the search field.", &error);
                }
                input.select();
            }
            return;
        }

        if event_target_is_interactive(&event) {
            return;
        }

        if event.ctrl_key() && key.eq_ignore_ascii_case("a") {
            event.prevent_default();
            self.selection
                .select_all(self.results.total.get_untracked());
            return;
        }

        if let Some(delta) = self.viewport.navigation_delta(&key) {
            event.prevent_default();
            self.move_selection_focus(FocusMove::Relative(delta), &event);
            return;
        }

        match key.as_str() {
            "PageDown" => {
                event.prevent_default();
                self.move_selection_focus(FocusMove::Relative(self.viewport.page_step()), &event);
            }
            "PageUp" => {
                event.prevent_default();
                self.move_selection_focus(FocusMove::Relative(-self.viewport.page_step()), &event);
            }
            "Home" => {
                event.prevent_default();
                self.move_selection_focus(FocusMove::Absolute(0), &event);
            }
            "End" => {
                event.prevent_default();
                let last = self.results.total.get_untracked().saturating_sub(1);
                self.move_selection_focus(FocusMove::Absolute(last), &event);
            }
            " " if event.ctrl_key() => {
                event.prevent_default();
                self.selection.toggle_focused();
            }
            "Enter" => {
                event.prevent_default();
                if let Some(item) = self.selection.focused_item(self.results) {
                    self.files.open(item.full_path);
                }
            }
            "Delete" => {
                event.prevent_default();
                self.files.begin_trash(self.selection, self.results);
            }
            "F2" => {
                event.prevent_default();
                if let Some(item) = self.selection.focused_item(self.results) {
                    self.files.begin_rename(item);
                }
            }
            "ContextMenu" => {
                event.prevent_default();
                self.menu
                    .open_at_focused_row(self.results, self.selection, self.viewport);
            }
            "F10" if event.shift_key() => {
                event.prevent_default();
                self.menu
                    .open_at_focused_row(self.results, self.selection, self.viewport);
            }
            "Escape" => {
                if self.menu.state.get_untracked().is_some() {
                    self.menu.close();
                } else {
                    self.selection.clear_indices();
                }
            }
            _ => {}
        }
    }

    fn move_selection_focus(self, movement: FocusMove, event: &KeyboardEvent) {
        let modifiers = SelectionModifiers {
            extend: event.shift_key(),
            preserve: event.ctrl_key(),
        };
        if let Some(index) =
            self.selection
                .focus(movement, modifiers, self.results.total.get_untracked())
        {
            self.viewport.scroll_row_into_view(index);
        }
    }
}
