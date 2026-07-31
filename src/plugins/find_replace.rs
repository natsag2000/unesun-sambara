//! Find & replace (Phase P2-03).
//!
//! See `prompt/FUTURE_PLAN.md` P2-03 and `prompt/PLUGIN_API.md`.
//!
//! Architecture
//! ------------
//! * [`FindReplaceState`] holds the query, options, and the last computed
//!   match list. It lives on `EditorState::find_replace` rather than
//!   inside [`FindReplacePlugin`] itself: command handlers are bare `fn`
//!   pointers (see `editor_core::plugin`) with no access to the plugin
//!   instance, only to `PluginContext` (i.e. `&mut EditorState`). This is
//!   the "plugin data slot" pattern flagged as future work in
//!   `prompt/PLUGIN_API.md`.
//! * Matching itself always goes through the `regex` crate: a literal
//!   query is turned into a pattern via `regex::escape`, so "plain text"
//!   and "regex" search share one code path. Whole-word wraps the
//!   pattern in `\b...\b`; case sensitivity toggles
//!   `RegexBuilder::case_insensitive`.
//! * Replacements go through `Edit::delete_range` + `Edit::insert_at`
//!   (the same primitives `cosmic_text::Editor` uses internally), wrapped
//!   in `EditorState::begin_history_group` / `flush_history_group` so
//!   they participate in undo/redo (P2-01) as a single step - even
//!   "Replace All", which is one `Change` for the whole batch.
//! * Rendering the match highlight overlay needs pixel rectangles, which
//!   requires `buffer.layout_runs()` and therefore lives in
//!   `WasmEditor::render` (`src/lib.rs`), not here. This module only
//!   exposes the cursor ranges (`FindReplaceState::ranges`) that the
//!   renderer converts to rectangles.

use cosmic_text::{Buffer, Cursor, Edit, Selection};
use regex::RegexBuilder;
use serde::Deserialize;
use wasm_bindgen::JsValue;

use crate::editor_core::history::EditKind;
use crate::editor_core::plugin::{Command, Plugin, PluginContext};

/// A single match, expressed as byte offsets within `line`'s text (the
/// same convention `cosmic_text::Cursor::index` uses).
#[derive(Clone, Copy, Debug)]
pub struct MatchRange {
    pub line: usize,
    pub start: usize,
    pub end: usize,
}

pub struct FindReplaceState {
    pub query: String,
    pub replacement: String,
    pub case_sensitive: bool,
    pub whole_word: bool,
    pub use_regex: bool,
    pub matches: Vec<MatchRange>,
    pub current: Option<usize>,
    /// Set when `query` fails to compile as a regex (only reachable with
    /// `use_regex: true`, or a `whole_word` boundary that happens to be
    /// invalid, which cannot actually happen with `\b`, but kept generic).
    pub error: Option<String>,
}

impl FindReplaceState {
    pub fn new() -> Self {
        Self {
            query: String::new(),
            replacement: String::new(),
            case_sensitive: false,
            whole_word: false,
            use_regex: false,
            matches: Vec::new(),
            current: None,
            error: None,
        }
    }

    fn build_pattern(&self) -> String {
        let base = if self.use_regex {
            self.query.clone()
        } else {
            regex::escape(&self.query)
        };
        if self.whole_word {
            format!(r"\b(?:{base})\b")
        } else {
            base
        }
    }

    /// Recompute `matches` (and `current`) against the given buffer's
    /// current text. Matching is line-by-line: search never spans a
    /// newline, which matches how `.uns`/`.txt` documents are edited a
    /// line at a time in this editor.
    pub fn search(&mut self, buffer: &Buffer) {
        self.matches.clear();
        self.error = None;

        if self.query.is_empty() {
            self.current = None;
            return;
        }

        let regex = match RegexBuilder::new(&self.build_pattern())
            .case_insensitive(!self.case_sensitive)
            .build()
        {
            Ok(r) => r,
            Err(e) => {
                self.error = Some(e.to_string());
                self.current = None;
                return;
            }
        };

        for (line_i, line) in buffer.lines.iter().enumerate() {
            let text = line.text();
            for m in regex.find_iter(text) {
                // Zero-width matches (e.g. a bad-but-valid pattern like
                // `a*`) would otherwise loop forever in the UI stepping
                // through "matches" that highlight nothing.
                if m.start() == m.end() {
                    continue;
                }
                self.matches.push(MatchRange {
                    line: line_i,
                    start: m.start(),
                    end: m.end(),
                });
            }
        }

        self.current = if self.matches.is_empty() { None } else { Some(0) };
    }

    /// Move `current` to the next (`forward = true`) or previous match,
    /// wrapping around. Returns the new index, or `None` if there are no
    /// matches.
    pub fn advance(&mut self, forward: bool) -> Option<usize> {
        if self.matches.is_empty() {
            self.current = None;
            return None;
        }
        let len = self.matches.len();
        let next = match self.current {
            None => {
                if forward {
                    0
                } else {
                    len - 1
                }
            }
            Some(i) => {
                if forward {
                    (i + 1) % len
                } else {
                    (i + len - 1) % len
                }
            }
        };
        self.current = Some(next);
        Some(next)
    }

    /// After a text mutation shifts match positions, point `current` at
    /// the first match at or after `(line, index)` (typically the
    /// editor's cursor right after a replace), falling back to the first
    /// match, or `None` if there are no matches left.
    pub fn select_from(&mut self, line: usize, index: usize) {
        if self.matches.is_empty() {
            self.current = None;
            return;
        }
        self.current = Some(
            self.matches
                .iter()
                .position(|m| (m.line, m.start) >= (line, index))
                .unwrap_or(0),
        );
    }

    pub fn clear(&mut self) {
        self.query.clear();
        self.replacement.clear();
        self.matches.clear();
        self.current = None;
        self.error = None;
    }

    /// Cursor range for match `idx`, for both text mutation (replace) and
    /// rendering (highlight overlay).
    pub fn cursor_range(&self, idx: usize) -> Option<(Cursor, Cursor)> {
        self.matches
            .get(idx)
            .map(|m| (Cursor::new(m.line, m.start), Cursor::new(m.line, m.end)))
    }

    /// Cursor ranges for every current match, alongside whether each one
    /// is the "current" (actively selected) match. Used by
    /// `WasmEditor::render` to draw the highlight overlay.
    pub fn ranges(&self) -> impl Iterator<Item = (Cursor, Cursor, bool)> + '_ {
        self.matches.iter().enumerate().map(move |(i, m)| {
            (
                Cursor::new(m.line, m.start),
                Cursor::new(m.line, m.end),
                self.current == Some(i),
            )
        })
    }

    pub fn summary_json(&self) -> String {
        #[derive(serde::Serialize)]
        struct Summary {
            count: usize,
            current: Option<usize>,
            error: Option<String>,
        }
        serde_json::to_string(&Summary {
            count: self.matches.len(),
            current: self.current,
            error: self.error.clone(),
        })
        .unwrap_or_else(|_| "{\"count\":0,\"current\":null,\"error\":null}".to_string())
    }
}

impl Default for FindReplaceState {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Deserialize)]
struct SetQueryArgs {
    query: String,
    #[serde(default)]
    replacement: String,
    #[serde(default)]
    case_sensitive: bool,
    #[serde(default)]
    whole_word: bool,
    #[serde(default)]
    use_regex: bool,
}

pub struct FindReplacePlugin;

impl Plugin for FindReplacePlugin {
    fn id(&self) -> &'static str {
        "core.findReplace"
    }

    fn name(&self) -> &'static str {
        "Find & Replace"
    }

    fn commands(&self) -> Vec<Command> {
        vec![
            Command {
                id: "editor.find.setQuery",
                title: "Find: Set Query / Options",
                category: Some("Find"),
                keybinding: Some("Ctrl+F"),
                handler: cmd_set_query,
            },
            Command {
                id: "editor.find.next",
                title: "Find: Next Match",
                category: Some("Find"),
                keybinding: Some("F3"),
                handler: cmd_find_next,
            },
            Command {
                id: "editor.find.prev",
                title: "Find: Previous Match",
                category: Some("Find"),
                keybinding: Some("Shift+F3"),
                handler: cmd_find_prev,
            },
            Command {
                id: "editor.find.replaceCurrent",
                title: "Find: Replace Current Match",
                category: Some("Find"),
                keybinding: Some("Ctrl+H"),
                handler: cmd_replace_current,
            },
            Command {
                id: "editor.find.replaceAll",
                title: "Find: Replace All",
                category: Some("Find"),
                keybinding: None,
                handler: cmd_replace_all,
            },
            Command {
                id: "editor.find.clear",
                title: "Find: Close / Clear",
                category: Some("Find"),
                keybinding: Some("Esc"),
                handler: cmd_clear,
            },
        ]
    }
}

fn cmd_set_query(ctx: &mut PluginContext<'_>, args: &str) -> Result<JsValue, JsValue> {
    let parsed: SetQueryArgs =
        serde_json::from_str(args).map_err(|e| JsValue::from_str(&e.to_string()))?;

    ctx.state.find_replace.query = parsed.query;
    ctx.state.find_replace.replacement = parsed.replacement;
    ctx.state.find_replace.case_sensitive = parsed.case_sensitive;
    ctx.state.find_replace.whole_word = parsed.whole_word;
    ctx.state.find_replace.use_regex = parsed.use_regex;
    ctx.state.refresh_find_matches();

    // Jump straight to (and select) the first match so the user sees
    // feedback as they type, mirroring browser find-in-page.
    if let Some((start, end)) = ctx.state.find_replace.cursor_range(0) {
        if ctx.state.find_replace.current == Some(0) {
            ctx.state.editor.set_selection(Selection::Normal(start));
            ctx.state.editor.set_cursor(end);
        }
    }

    Ok(JsValue::from_str(&ctx.state.find_replace.summary_json()))
}

fn cmd_find_next(ctx: &mut PluginContext<'_>, _args: &str) -> Result<JsValue, JsValue> {
    ctx.state.find_replace.advance(true);
    select_current_match(ctx);
    Ok(JsValue::from_str(&ctx.state.find_replace.summary_json()))
}

fn cmd_find_prev(ctx: &mut PluginContext<'_>, _args: &str) -> Result<JsValue, JsValue> {
    ctx.state.find_replace.advance(false);
    select_current_match(ctx);
    Ok(JsValue::from_str(&ctx.state.find_replace.summary_json()))
}

fn select_current_match(ctx: &mut PluginContext<'_>) {
    let Some(idx) = ctx.state.find_replace.current else {
        return;
    };
    if let Some((start, end)) = ctx.state.find_replace.cursor_range(idx) {
        ctx.state.editor.set_selection(Selection::Normal(start));
        ctx.state.editor.set_cursor(end);
    }
}

fn cmd_replace_current(ctx: &mut PluginContext<'_>, _args: &str) -> Result<JsValue, JsValue> {
    let now = js_sys::Date::now();

    let Some(idx) = ctx.state.find_replace.current else {
        return Ok(JsValue::from_str(&ctx.state.find_replace.summary_json()));
    };
    let Some((start, end)) = ctx.state.find_replace.cursor_range(idx) else {
        return Ok(JsValue::from_str(&ctx.state.find_replace.summary_json()));
    };
    let replacement = ctx.state.find_replace.replacement.clone();

    ctx.state.begin_history_group(EditKind::Paste, now);
    ctx.state.editor.delete_range(start, end);
    let new_cursor = ctx.state.editor.insert_at(start, &replacement, None);
    ctx.state.flush_history_group();

    ctx.state.editor.set_cursor(new_cursor);
    ctx.state.refresh_find_matches();
    ctx.state
        .find_replace
        .select_from(new_cursor.line, new_cursor.index);
    select_current_match(ctx);

    Ok(JsValue::from_str(&ctx.state.find_replace.summary_json()))
}

fn cmd_replace_all(ctx: &mut PluginContext<'_>, _args: &str) -> Result<JsValue, JsValue> {
    let now = js_sys::Date::now();

    // Make sure we are replacing against up-to-date matches.
    ctx.state.refresh_find_matches();
    let matches = ctx.state.find_replace.matches.clone();
    if matches.is_empty() {
        return Ok(JsValue::from_str("{\"replaced\":0}"));
    }
    let replacement = ctx.state.find_replace.replacement.clone();

    ctx.state.begin_history_group(EditKind::Paste, now);
    // Process from the last match to the first: earlier byte offsets on
    // a line stay valid while later ones on the same line are replaced
    // first. `matches` is produced in ascending (line, start) order, so
    // reversing it yields exactly this order.
    for m in matches.iter().rev() {
        let start = Cursor::new(m.line, m.start);
        let end = Cursor::new(m.line, m.end);
        ctx.state.editor.delete_range(start, end);
        ctx.state.editor.insert_at(start, &replacement, None);
    }
    ctx.state.flush_history_group();

    let count = matches.len();
    ctx.state.refresh_find_matches();

    Ok(JsValue::from_str(&format!(
        "{{\"replaced\":{count},\"count\":{},\"current\":null,\"error\":null}}",
        ctx.state.find_replace.matches.len()
    )))
}

fn cmd_clear(ctx: &mut PluginContext<'_>, _args: &str) -> Result<JsValue, JsValue> {
    ctx.state.find_replace.clear();
    Ok(JsValue::NULL)
}

#[cfg(test)]
mod tests {
    use super::*;
    use cosmic_text::{Attrs, FontSystem, Metrics, Shaping};

    fn make_buffer(text: &str) -> (FontSystem, Buffer) {
        let mut font_system = FontSystem::new();
        let metrics = Metrics::new(16.0, 20.0);
        let mut buffer = Buffer::new(&mut font_system, metrics);
        buffer.set_text(&mut font_system, text, &Attrs::new(), Shaping::Basic, None);
        (font_system, buffer)
    }

    #[test]
    fn plain_search_is_case_insensitive_by_default() {
        let (_fs, buffer) = make_buffer("Hello world\nhello again");
        let mut state = FindReplaceState::new();
        state.query = "hello".to_string();
        state.search(&buffer);
        assert_eq!(state.matches.len(), 2);
        assert_eq!(state.current, Some(0));
    }

    #[test]
    fn case_sensitive_narrows_matches() {
        let (_fs, buffer) = make_buffer("Hello world\nhello again");
        let mut state = FindReplaceState::new();
        state.query = "hello".to_string();
        state.case_sensitive = true;
        state.search(&buffer);
        assert_eq!(state.matches.len(), 1);
        assert_eq!(state.matches[0].line, 1);
    }

    #[test]
    fn whole_word_excludes_partial_matches() {
        let (_fs, buffer) = make_buffer("cat catalog concatenate");
        let mut state = FindReplaceState::new();
        state.query = "cat".to_string();
        state.whole_word = true;
        state.search(&buffer);
        assert_eq!(state.matches.len(), 1);
    }

    #[test]
    fn regex_mode_matches_pattern() {
        let (_fs, buffer) = make_buffer("foo1 foo22 bar3");
        let mut state = FindReplaceState::new();
        state.query = r"foo\d+".to_string();
        state.use_regex = true;
        state.search(&buffer);
        assert_eq!(state.matches.len(), 2);
    }

    #[test]
    fn invalid_regex_reports_error_without_panicking() {
        let (_fs, buffer) = make_buffer("anything");
        let mut state = FindReplaceState::new();
        state.query = "(unclosed".to_string();
        state.use_regex = true;
        state.search(&buffer);
        assert!(state.matches.is_empty());
        assert!(state.error.is_some());
    }

    #[test]
    fn advance_wraps_around() {
        let mut state = FindReplaceState::new();
        state.matches = vec![
            MatchRange { line: 0, start: 0, end: 1 },
            MatchRange { line: 0, start: 2, end: 3 },
        ];
        state.current = Some(1);
        assert_eq!(state.advance(true), Some(0));
        assert_eq!(state.advance(false), Some(1));
    }
}
