//! Word suggestion state (Phase P9, `prompt/WORD_SUGGESTIONS_PLAN.md`).
//!
//! Plain state on `EditorState` - same shape as `HistoryManager`, *not*
//! a `Plugin` (deliberate deviation from the plan document's initial
//! sketch, recorded in the impl log): there's no natural command-palette
//! action here beyond what the JS popup's dedicated keybindings/mouse
//! clicks already cover directly through `WasmEditor` methods, so the
//! P1 plugin substrate (built for contributing commands/reacting to
//! events) doesn't buy anything for this feature.
//!
//! Holds only the *current* word bounds + candidate list. Which
//! suggestion is highlighted is tracked entirely in JS
//! (`WordSuggestionsPopup.selected` in `index.html`), the same way
//! `CommandPalette.selectedIndex` already works - there's no need for
//! Rust to also track it, since `accept_word_suggestion` takes the
//! chosen text directly rather than reading a "currently selected"
//! index back out of this struct.
//!
//! The dictionary itself lives on `WasmEditor` (like
//! `translit_renderer`), not here, since computing suggestions needs
//! this state *and* the dictionary *and* (for pixel positioning) canvas
//! dimensions only `WasmEditor` has - see
//! `WasmEditor::get_word_suggestions_json` in `src/lib.rs` for where
//! those come together.

use cosmic_text::Cursor;

pub struct SuggestionState {
    /// Bounds of the word suggestions are currently computed for, so
    /// `WasmEditor::accept_word_suggestion` knows what to replace.
    pub word_start: Option<Cursor>,
    pub word_end: Option<Cursor>,
    /// Start of the currently suggested noun-case segment, used when a
    /// short reflexive-possessive form replaces that case suffix.
    pub suffix_start: Option<Cursor>,
    pub suggestions: Vec<String>,
}

impl SuggestionState {
    pub fn new() -> Self {
        Self {
            word_start: None,
            word_end: None,
            suffix_start: None,
            suggestions: Vec::new(),
        }
    }

    pub fn clear(&mut self) {
        self.word_start = None;
        self.word_end = None;
        self.suffix_start = None;
        self.suggestions.clear();
    }
}

impl Default for SuggestionState {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clear_resets_everything() {
        let mut state = SuggestionState::new();
        state.suggestions = vec!["x".into()];
        state.word_start = Some(Cursor::new(0, 0));
        state.word_end = Some(Cursor::new(0, 1));
        state.suffix_start = Some(Cursor::new(0, 1));
        state.clear();
        assert!(state.suggestions.is_empty());
        assert!(state.word_start.is_none());
        assert!(state.word_end.is_none());
        assert!(state.suffix_start.is_none());
    }
}
