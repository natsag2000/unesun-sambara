//! Mongolian format-control-aware backspace/delete decision logic
//! (Phase P7-03).
//!
//! Mongolian text uses a handful of invisible format-control characters
//! (free variation selectors, vowel separator, narrow no-break space)
//! that should be deleted *together with* the visible character they
//! attach to, rather than one keystroke at a time like an ordinary
//! character. There are two attachment patterns:
//!
//! * `[visible][format_after]` - U+180B, U+180C, U+180D, U+180F attach
//!   to the character *before* them.
//! * `[format_before][visible]` - U+180E, U+202F attach to the
//!   character *after* them.
//!
//! This module is pure decision logic extracted from
//! `WasmEditor::handle_backspace`/`handle_delete` in `src/lib.rs`: given
//! the current line's characters and the cursor's character-index
//! position, decide *how many* synthesized `Action::Backspace` /
//! `Action::Delete` calls a single keystroke should turn into. It has no
//! dependency on `cosmic_text` or `wasm_bindgen`, so it's exercised here
//! with plain `#[test]`s instead of needing a `WasmEditor` fixture.
//!
//! Every plan in this module executes as *all backspaces, then all
//! deletes* - every branch in the original hand-written code happened to
//! follow that order already (never interleaved), so `DeletePlan` bakes
//! it in rather than modeling a more general action sequence.

/// How many `Action::Backspace` / `Action::Delete` calls to issue for a
/// single Backspace or Delete keystroke, applied in that order
/// (backspaces first, then deletes).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct DeletePlan {
    pub backspaces: usize,
    pub deletes: usize,
}

impl DeletePlan {
    const fn backspace(n: usize) -> Self {
        Self { backspaces: n, deletes: 0 }
    }

    const fn delete(n: usize) -> Self {
        Self { backspaces: 0, deletes: n }
    }
}

/// Any Mongolian format-control character this module cares about.
pub fn is_format_control(c: char) -> bool {
    matches!(c, '\u{180B}' | '\u{180C}' | '\u{180D}' | '\u{180E}' | '\u{180F}' | '\u{202F}')
}

/// Attaches to the character *before* it: `[visible][format]`.
pub fn is_format_after_visible(c: char) -> bool {
    matches!(c, '\u{180B}' | '\u{180C}' | '\u{180D}' | '\u{180F}')
}

/// Attaches to the character *after* it: `[format][visible]`.
pub fn is_format_before_visible(c: char) -> bool {
    matches!(c, '\u{180E}' | '\u{202F}')
}

/// Decide the delete plan for a Backspace keystroke.
///
/// `cursor_index` is the number of characters before the cursor within
/// `chars` (so `0` is the start of the line and `chars.len()` is the
/// end) - the same convention `WasmEditor::handle_backspace` already
/// used internally.
pub fn backspace_plan(chars: &[char], cursor_index: usize) -> DeletePlan {
    if cursor_index == 0 || cursor_index > chars.len() {
        return DeletePlan::backspace(1);
    }

    let char_before = chars[cursor_index - 1];

    // Pattern 1: cursor sits right after a run of `format_after`
    // controls - delete the whole run, plus the visible character
    // before it if there is one.
    if is_format_after_visible(char_before) {
        let mut format_count = 1;
        let mut pos = cursor_index - 1;
        while pos > 0 && is_format_after_visible(chars[pos - 1]) {
            format_count += 1;
            pos -= 1;
        }
        return if pos > 0 && !is_format_control(chars[pos - 1]) {
            DeletePlan::backspace(format_count + 1)
        } else {
            DeletePlan::backspace(1)
        };
    }

    // Pattern 2: cursor sits right after a visible character - check
    // whether it's preceded by a run of `format_before` controls that
    // should be deleted along with it.
    if !is_format_control(char_before) {
        let mut format_count = 0;
        let mut pos = cursor_index - 1;
        while pos > 0 && is_format_before_visible(chars[pos - 1]) {
            format_count += 1;
            pos -= 1;
        }
        return if format_count > 0 {
            DeletePlan::backspace(format_count + 1)
        } else {
            DeletePlan::backspace(1)
        };
    }

    // Cursor sits right after a `format_before` run (and that run isn't
    // itself preceded by a visible character we'd catch above) - if
    // there's a visible character immediately *after* the cursor, treat
    // the whole [format_before...][visible] group as one unit: back up
    // over the formats, then delete forward into the visible character.
    if is_format_before_visible(char_before) {
        let mut format_count = 1;
        let mut pos = cursor_index - 1;
        while pos > 0 && is_format_before_visible(chars[pos - 1]) {
            format_count += 1;
            pos -= 1;
        }
        return if cursor_index < chars.len() && !is_format_control(chars[cursor_index]) {
            DeletePlan { backspaces: format_count, deletes: 1 }
        } else {
            DeletePlan::backspace(1)
        };
    }

    DeletePlan::backspace(1)
}

/// Decide the delete plan for a Delete (forward-delete) keystroke. Same
/// `cursor_index` convention as [`backspace_plan`].
pub fn delete_plan(chars: &[char], cursor_index: usize) -> DeletePlan {
    if cursor_index >= chars.len() {
        return DeletePlan::delete(1);
    }

    let char_at = chars[cursor_index];

    // Pattern 1: the character at the cursor is visible - check whether
    // it's followed by a run of `format_after` controls that should be
    // deleted along with it.
    if !is_format_control(char_at) {
        let mut format_count = 0;
        let mut pos = cursor_index + 1;
        while pos < chars.len() && is_format_after_visible(chars[pos]) {
            format_count += 1;
            pos += 1;
        }
        return if format_count > 0 {
            DeletePlan::delete(format_count + 1)
        } else {
            DeletePlan::delete(1)
        };
    }

    // Pattern 2: cursor sits right before a run of `format_before`
    // controls - delete the whole run, plus the visible character after
    // it if there is one.
    if is_format_before_visible(char_at) {
        let mut format_count = 1;
        let mut pos = cursor_index + 1;
        while pos < chars.len() && is_format_before_visible(chars[pos]) {
            format_count += 1;
            pos += 1;
        }
        return if pos < chars.len() && !is_format_control(chars[pos]) {
            DeletePlan::delete(format_count + 1)
        } else {
            DeletePlan::delete(1)
        };
    }

    // Cursor sits right before a `format_after` run (and that run isn't
    // itself followed by a visible character we'd catch above) - if
    // there's a visible character immediately *before* the cursor, treat
    // the whole [visible][format_after...] group as one unit: delete the
    // visible character behind the cursor, then forward-delete the
    // formats.
    if is_format_after_visible(char_at) {
        let mut format_count = 1;
        let mut pos = cursor_index + 1;
        while pos < chars.len() && is_format_after_visible(chars[pos]) {
            format_count += 1;
            pos += 1;
        }
        return if cursor_index > 0 && !is_format_control(chars[cursor_index - 1]) {
            DeletePlan { backspaces: 1, deletes: format_count }
        } else {
            DeletePlan::delete(1)
        };
    }

    DeletePlan::delete(1)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn chars_of(s: &str) -> Vec<char> {
        s.chars().collect()
    }

    // ---- backspace_plan ----

    #[test]
    fn backspace_at_start_of_line_is_a_plain_backspace() {
        let chars = chars_of("hello");
        assert_eq!(backspace_plan(&chars, 0), DeletePlan::backspace(1));
    }

    #[test]
    fn backspace_out_of_bounds_falls_back_to_plain_backspace() {
        let chars = chars_of("hi");
        assert_eq!(backspace_plan(&chars, 99), DeletePlan::backspace(1));
    }

    #[test]
    fn backspace_after_plain_character_deletes_just_it() {
        let chars = chars_of("hello");
        // Cursor after "hell|o" (index 4).
        assert_eq!(backspace_plan(&chars, 4), DeletePlan::backspace(1));
    }

    #[test]
    fn backspace_after_single_format_after_control_deletes_both() {
        // "a" + U+180B, cursor at end (index 2).
        let chars = chars_of("a\u{180B}");
        assert_eq!(backspace_plan(&chars, 2), DeletePlan::backspace(2));
    }

    #[test]
    fn backspace_after_stacked_format_after_controls_deletes_the_whole_run() {
        // "a" + three stacked format_after controls, cursor at end.
        let chars = chars_of("a\u{180B}\u{180C}\u{180D}");
        assert_eq!(backspace_plan(&chars, 4), DeletePlan::backspace(4));
    }

    #[test]
    fn backspace_format_after_with_no_preceding_visible_char_deletes_only_the_control() {
        // Format-after control at the very start of the line (no
        // visible character before it to attach to).
        let chars = chars_of("\u{180B}x");
        assert_eq!(backspace_plan(&chars, 1), DeletePlan::backspace(1));
    }

    #[test]
    fn backspace_after_visible_preceded_by_format_before_run_deletes_both() {
        // U+202F + "a", cursor after "a" (index 2).
        let chars = chars_of("\u{202F}a");
        assert_eq!(backspace_plan(&chars, 2), DeletePlan::backspace(2));
    }

    #[test]
    fn backspace_between_format_before_run_and_following_visible_char() {
        // U+202F + "a", cursor between them (index 1): back up over the
        // format, then delete forward into "a" - one backspace, one
        // delete.
        let chars = chars_of("\u{202F}a");
        assert_eq!(backspace_plan(&chars, 1), DeletePlan { backspaces: 1, deletes: 1 });
    }

    #[test]
    fn backspace_format_before_run_at_end_of_line_deletes_only_the_control() {
        // Format-before control with nothing after it on the line.
        let chars = chars_of("x\u{202F}");
        assert_eq!(backspace_plan(&chars, 2), DeletePlan::backspace(1));
    }

    // ---- delete_plan ----

    #[test]
    fn delete_at_end_of_line_is_a_plain_delete() {
        let chars = chars_of("hello");
        assert_eq!(delete_plan(&chars, 5), DeletePlan::delete(1));
    }

    #[test]
    fn delete_plain_character_deletes_just_it() {
        let chars = chars_of("hello");
        assert_eq!(delete_plan(&chars, 0), DeletePlan::delete(1));
    }

    #[test]
    fn delete_visible_followed_by_format_after_run_deletes_both() {
        let chars = chars_of("a\u{180B}\u{180C}");
        assert_eq!(delete_plan(&chars, 0), DeletePlan::delete(3));
    }

    #[test]
    fn delete_format_before_run_followed_by_visible_deletes_both() {
        let chars = chars_of("\u{202F}a");
        assert_eq!(delete_plan(&chars, 0), DeletePlan::delete(2));
    }

    #[test]
    fn delete_format_before_run_at_end_of_line_deletes_only_the_control() {
        let chars = chars_of("x\u{202F}");
        assert_eq!(delete_plan(&chars, 1), DeletePlan::delete(1));
    }

    #[test]
    fn delete_between_visible_and_following_format_after_run() {
        // "a" + U+180B, cursor between them (index 1): delete "a"
        // backward, then forward-delete the format - one backspace, one
        // delete.
        let chars = chars_of("a\u{180B}");
        assert_eq!(delete_plan(&chars, 1), DeletePlan { backspaces: 1, deletes: 1 });
    }

    #[test]
    fn delete_format_after_run_with_no_preceding_visible_char() {
        let chars = chars_of("\u{180B}x");
        assert_eq!(delete_plan(&chars, 0), DeletePlan::delete(1));
    }

    // ---- symmetry: backspace_plan and delete_plan agree from opposite
    // sides of the same [visible][format_after] / [format_before][visible]
    // groups ----

    #[test]
    fn backspace_and_delete_agree_on_a_visible_format_after_group() {
        let chars = chars_of("a\u{180B}");
        // Deleting from the end (backspace) removes the whole group...
        assert_eq!(backspace_plan(&chars, 2), DeletePlan::backspace(2));
        // ...and so does forward-deleting from the start.
        assert_eq!(delete_plan(&chars, 0), DeletePlan::delete(2));
    }

    #[test]
    fn backspace_and_delete_agree_on_a_format_before_visible_group() {
        let chars = chars_of("\u{202F}a");
        assert_eq!(backspace_plan(&chars, 2), DeletePlan::backspace(2));
        assert_eq!(delete_plan(&chars, 0), DeletePlan::delete(2));
    }
}
