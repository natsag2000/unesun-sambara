//! Undo / redo history.
//!
//! See `prompt/FUTURE_PLAN.md` (Phase P2-01).
//!
//! Design
//! ------
//! cosmic-text's `Editor` already ships a change-tracking mechanism
//! (`Edit::start_change` / `Edit::finish_change` / `Edit::apply_change`,
//! backed by `cosmic_text::Change` / `ChangeItem`). Every call to
//! `delete_range` / `insert_at` (and therefore every `Action::Insert`,
//! `Action::Backspace`, `Action::Delete`, `Action::Enter`, `Action::Indent`,
//! ...) appends a `ChangeItem` to the in-progress `Change` *if one has been
//! started*. That gives us exact, minimal-diff undo entries for free -
//! there is no need to snapshot the whole document text.
//!
//! `HistoryManager` only decides *when* a change should be started, ended,
//! and committed to the undo stack:
//!
//! * Consecutive keystrokes of the same [`EditKind`] within
//!   [`DEBOUNCE_MS`] of each other are coalesced into a single `Change`
//!   (one undo step for a whole burst of typing), by simply *not* ending
//!   the in-progress change between them - cosmic-text keeps appending
//!   `ChangeItem`s to it.
//! * A different `EditKind`, a pause longer than the debounce window, a
//!   cursor motion, a click, or an explicit flush point (undo/redo,
//!   document load, ...) ends the current change and pushes it onto the
//!   undo stack (if it is non-empty).
//! * This also gives the format-control-aware backspace/delete handlers
//!   (`WasmEditor::handle_backspace` / `handle_delete`) a single undo step
//!   per user keystroke "for free": the caller starts one change before
//!   calling the handler, which may perform several synthesized
//!   `Action::Backspace` / `Action::Delete` calls that all land in the
//!   same `Change`.

use std::collections::VecDeque;

use cosmic_text::Change;

/// Maximum number of grouped edits kept per stack. Old entries are
/// dropped once the cap is exceeded; this bounds memory for very long
/// editing sessions instead of growing unbounded.
const MAX_HISTORY: usize = 200;

/// Consecutive edits of the same kind within this many milliseconds are
/// coalesced into a single undo step.
pub const DEBOUNCE_MS: f64 = 700.0;

/// Coarse classification of what a mutating action *is*, used only to
/// decide whether two consecutive edits belong in the same undo step.
/// This is intentionally coarser than `EditorEvent` - it never leaves
/// this module.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum EditKind {
    Insert,
    Delete,
    Newline,
    Indent,
    /// Programmatic bulk insert (paste, transliteration, IME conversion).
    /// Always its own undo step; never coalesced with adjacent typing or
    /// with another paste.
    Paste,
}

pub struct HistoryManager {
    undo_stack: VecDeque<Change>,
    redo_stack: VecDeque<Change>,
    pending_kind: Option<EditKind>,
    pending_time: f64,
}

impl HistoryManager {
    pub fn new() -> Self {
        Self {
            undo_stack: VecDeque::new(),
            redo_stack: VecDeque::new(),
            pending_kind: None,
            pending_time: 0.0,
        }
    }

    /// True if starting an edit of `kind` at time `now` should begin a
    /// new undo group (i.e. the caller should flush/commit whatever
    /// change is currently in progress first).
    pub fn is_new_group(&self, kind: EditKind, now: f64) -> bool {
        match self.pending_kind {
            None => true,
            Some(prev) => {
                prev != kind
                    || kind == EditKind::Paste
                    || (now - self.pending_time).abs() > DEBOUNCE_MS
            }
        }
    }

    /// Mark that a group of `kind` is now in progress, starting at `now`.
    pub fn begin_group(&mut self, kind: EditKind, now: f64) {
        self.pending_kind = Some(kind);
        self.pending_time = now;
    }

    /// Extend the current group's debounce window (call on every
    /// keystroke that continues the group).
    pub fn touch(&mut self, now: f64) {
        self.pending_time = now;
    }

    /// No group is in progress any more (called after a flush).
    pub fn end_group(&mut self) {
        self.pending_kind = None;
    }

    /// Commit a finished change to the undo stack. No-op for empty
    /// changes (nothing was actually mutated). Always clears the redo
    /// stack - a fresh edit invalidates any previously undone redo
    /// history.
    pub fn commit(&mut self, change: Change) {
        if change.items.is_empty() {
            return;
        }
        Self::push_capped(&mut self.undo_stack, change);
        self.redo_stack.clear();
    }

    pub fn can_undo(&self) -> bool {
        !self.undo_stack.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.redo_stack.is_empty()
    }

    pub fn pop_undo(&mut self) -> Option<Change> {
        self.undo_stack.pop_back()
    }

    pub fn pop_redo(&mut self) -> Option<Change> {
        self.redo_stack.pop_back()
    }

    pub fn push_undo(&mut self, change: Change) {
        Self::push_capped(&mut self.undo_stack, change);
    }

    pub fn push_redo(&mut self, change: Change) {
        Self::push_capped(&mut self.redo_stack, change);
    }

    /// Drop all history. Called when a brand new document is loaded
    /// (`set_text`) since undoing "past" the loaded text makes no sense.
    pub fn clear(&mut self) {
        self.undo_stack.clear();
        self.redo_stack.clear();
        self.pending_kind = None;
    }

    fn push_capped(stack: &mut VecDeque<Change>, change: Change) {
        stack.push_back(change);
        if stack.len() > MAX_HISTORY {
            stack.pop_front();
        }
    }
}

impl Default for HistoryManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cosmic_text::{ChangeItem, Cursor};

    fn dummy_change(text: &str) -> Change {
        Change {
            items: vec![ChangeItem {
                start: Cursor::new(0, 0),
                end: Cursor::new(0, text.len()),
                text: text.to_string(),
                insert: true,
            }],
        }
    }

    #[test]
    fn same_kind_within_debounce_is_not_a_new_group() {
        let mut h = HistoryManager::new();
        assert!(h.is_new_group(EditKind::Insert, 0.0));
        h.begin_group(EditKind::Insert, 0.0);
        assert!(!h.is_new_group(EditKind::Insert, 100.0));
    }

    #[test]
    fn different_kind_is_a_new_group() {
        let mut h = HistoryManager::new();
        h.begin_group(EditKind::Insert, 0.0);
        assert!(h.is_new_group(EditKind::Delete, 100.0));
    }

    #[test]
    fn timeout_forces_a_new_group() {
        let mut h = HistoryManager::new();
        h.begin_group(EditKind::Insert, 0.0);
        assert!(h.is_new_group(EditKind::Insert, DEBOUNCE_MS + 1.0));
    }

    #[test]
    fn paste_is_always_a_new_group() {
        let mut h = HistoryManager::new();
        h.begin_group(EditKind::Paste, 0.0);
        assert!(h.is_new_group(EditKind::Paste, 1.0));
    }

    #[test]
    fn commit_pushes_and_clears_redo() {
        let mut h = HistoryManager::new();
        h.push_redo(dummy_change("x"));
        assert!(h.can_redo());
        h.commit(dummy_change("hello"));
        assert!(h.can_undo());
        assert!(!h.can_redo());
    }

    #[test]
    fn commit_of_empty_change_is_noop() {
        let mut h = HistoryManager::new();
        h.commit(Change::default());
        assert!(!h.can_undo());
    }

    #[test]
    fn undo_redo_round_trip() {
        let mut h = HistoryManager::new();
        h.commit(dummy_change("hello"));
        let c = h.pop_undo().expect("undo entry");
        h.push_redo(c);
        assert!(h.can_redo());
        assert!(!h.can_undo());
        let c = h.pop_redo().expect("redo entry");
        h.push_undo(c);
        assert!(h.can_undo());
    }

    #[test]
    fn history_cap_drops_oldest() {
        let mut h = HistoryManager::new();
        for i in 0..(MAX_HISTORY + 10) {
            h.commit(dummy_change(&format!("edit-{i}")));
        }
        assert_eq!(h.undo_stack.len(), MAX_HISTORY);
    }
}
