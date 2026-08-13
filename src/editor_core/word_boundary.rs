//! Word-boundary detection for the word suggestion popup (Phase P9,
//! `prompt/WORD_SUGGESTIONS_PLAN.md`).
//!
//! Pure, `cosmic_text`/`wasm_bindgen`-free functions operating on `&[char]`
//! + a character-index cursor position - same style and testing approach
//! as `src/editor_core/format_control.rs` (P7-03). Callers reuse the same
//! `chars`/`cursor_index` extraction `WasmEditor::current_line_chars_and_cursor`
//! already does for backspace/delete.
//!
//! Design notes (see the plan document's §4 for the full rationale):
//!
//! * Matches never cross a newline - `chars` is always one line's
//!   characters, same restriction `find_replace.rs`'s `whole_word` search
//!   already has.
//! * Mongolian format-control characters (U+180B/C/D/E/F, U+202F) are
//!   *never* word separators - confirmed in `format_control.rs`'s own doc
//!   comment that they only ever attach to one neighboring visible
//!   character. A word-boundary scan must skip over them, not stop at
//!   them.
//! * Digits are boundaries, not word characters (resolved scope decision -
//!   see `WORD_SUGGESTIONS_PLAN.md` §11.3). Only `char::is_alphabetic()`
//!   characters (which already covers both the Cyrillic and Mongolian
//!   Bichig Unicode blocks) or format-control characters count as "part
//!   of a word."

use crate::editor_core::format_control::is_format_control;

/// Which script a word "belongs to," for choosing which dictionary
/// index to query (see `Dictionary::suggest` in `src/dictionary/lookup.rs`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Script {
    Cyrillic,
    Mongolian,
    /// Alphabetic but neither of the above (e.g. plain Latin) - not
    /// currently backed by any suggestion index, but classified rather
    /// than silently folded into one of the other variants so callers
    /// can decide to show nothing.
    Other,
}

/// True if `c` should be treated as "part of a word" when scanning for
/// word boundaries: alphabetic (any script) or a Mongolian format-control
/// character attached to a neighboring letter.
fn is_word_char(c: char) -> bool {
    c.is_alphabetic() || is_format_control(c)
}

/// The word (if any) the cursor is currently positioned inside or at the
/// edge of, as a half-open `[start, end)` character-index range within
/// `chars`. `cursor_index` uses the same convention as
/// `format_control::backspace_plan`/`delete_plan`: the number of
/// characters before the cursor (`0..=chars.len()`).
///
/// Returns `None` if the cursor isn't adjacent to any word character at
/// all (e.g. sitting between two spaces, or at the very start/end of an
/// empty line).
pub fn current_word_bounds(chars: &[char], cursor_index: usize) -> Option<(usize, usize)> {
    let cursor_index = cursor_index.min(chars.len());

    // A cursor "touches" a word character if the character immediately
    // before OR immediately after it qualifies - covers both "cursor
    // inside/after a word" and "cursor right before a word" (e.g. right
    // after accepting a suggestion that left the cursor at the start of
    // trailing punctuation).
    let touches_before = cursor_index > 0 && is_word_char(chars[cursor_index - 1]);
    let touches_after = cursor_index < chars.len() && is_word_char(chars[cursor_index]);
    if !touches_before && !touches_after {
        return None;
    }

    let mut start = cursor_index;
    while start > 0 && is_word_char(chars[start - 1]) {
        start -= 1;
    }

    let mut end = cursor_index;
    while end < chars.len() && is_word_char(chars[end]) {
        end += 1;
    }

    Some((start, end))
}

/// Classify a word's script by its first alphabetic character (format
/// control characters are skipped when looking for that first
/// character, since they carry no script information of their own).
pub fn classify_script(word: &[char]) -> Option<Script> {
    let first_letter = word.iter().copied().find(|c| c.is_alphabetic())?;
    Some(classify_char(first_letter))
}

fn classify_char(c: char) -> Script {
    let cp = c as u32;
    if (0x0400..=0x04FF).contains(&cp) {
        Script::Cyrillic
    } else if (0x1800..=0x18AF).contains(&cp) {
        Script::Mongolian
    } else {
        Script::Other
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn chars_of(s: &str) -> Vec<char> {
        s.chars().collect()
    }

    #[test]
    fn cursor_inside_a_plain_word_returns_its_full_bounds() {
        let chars = chars_of("hello world");
        // Cursor after "hel|lo world" (index 3).
        assert_eq!(current_word_bounds(&chars, 3), Some((0, 5)));
    }

    #[test]
    fn cursor_at_end_of_word_returns_its_bounds() {
        let chars = chars_of("hello world");
        assert_eq!(current_word_bounds(&chars, 5), Some((0, 5)));
    }

    #[test]
    fn cursor_at_start_of_second_word_returns_its_bounds() {
        let chars = chars_of("hello world");
        // Cursor right before "world" (index 6).
        assert_eq!(current_word_bounds(&chars, 6), Some((6, 11)));
    }

    #[test]
    fn cursor_between_two_spaces_returns_none() {
        let chars = chars_of("a  b");
        assert_eq!(current_word_bounds(&chars, 2), None);
    }

    #[test]
    fn empty_line_returns_none() {
        let chars: Vec<char> = vec![];
        assert_eq!(current_word_bounds(&chars, 0), None);
    }

    #[test]
    fn digits_are_boundaries_not_word_characters() {
        let chars = chars_of("a1b");
        // Cursor after "a1|b" - the digit splits "a" and "b" into
        // separate one-character words rather than one "a1b" word.
        assert_eq!(current_word_bounds(&chars, 2), Some((2, 3)));
        assert_eq!(current_word_bounds(&chars, 1), Some((0, 1)));
    }

    #[test]
    fn format_control_characters_are_part_of_the_word_not_boundaries() {
        // "a" + U+180B (format_after) + "b", cursor at the very end.
        let chars = chars_of("a\u{180B}b");
        assert_eq!(current_word_bounds(&chars, 3), Some((0, 3)));
        // Cursor between the format control and "b" (index 2) - still
        // inside the same word, not a boundary.
        assert_eq!(current_word_bounds(&chars, 2), Some((0, 3)));
    }

    #[test]
    fn cyrillic_word_is_detected_like_any_other_alphabetic_word() {
        let chars = chars_of("привет мир");
        assert_eq!(current_word_bounds(&chars, 3), Some((0, 6)));
    }

    #[test]
    fn mongolian_bichig_word_is_detected_like_any_other_alphabetic_word() {
        let chars = chars_of("\u{1820}\u{1821}\u{1822}");
        assert_eq!(current_word_bounds(&chars, 2), Some((0, 3)));
    }

    #[test]
    fn classify_script_identifies_cyrillic() {
        let word = chars_of("привет");
        assert_eq!(classify_script(&word), Some(Script::Cyrillic));
    }

    #[test]
    fn classify_script_identifies_mongolian() {
        let word = chars_of("\u{1820}\u{1821}");
        assert_eq!(classify_script(&word), Some(Script::Mongolian));
    }

    #[test]
    fn classify_script_skips_leading_format_control_characters() {
        // A word starting with a format-before control (rare in
        // practice for a *word start*, but the classifier should not
        // crash or misclassify - it should look past it).
        let word = chars_of("\u{202F}\u{0430}");
        assert_eq!(classify_script(&word), Some(Script::Cyrillic));
    }

    #[test]
    fn classify_script_of_empty_or_all_format_control_word_is_none() {
        let word = chars_of("\u{180B}\u{180C}");
        assert_eq!(classify_script(&word), None);
    }

    #[test]
    fn classify_script_identifies_other_scripts_as_other() {
        let word = chars_of("hello");
        assert_eq!(classify_script(&word), Some(Script::Other));
    }
}
