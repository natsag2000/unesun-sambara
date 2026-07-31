# Phase P2 — Core editing features (implementation log)

Date: 2026-07-30
Reference: `prompt/FUTURE_PLAN.md`, Phase P2 (items P2-01 through P2-05)
Patch: `patch/Phase_2.patch`

## Scope

All five P2 items completed in this session, in dependency order:

| ID    | Effort | Status | Summary                                                                                          |
|-------|--------|--------|---------------------------------------------------------------------------------------------------|
| P2-01 | M      | done   | Undo/redo built on cosmic-text's own `Change`/`ChangeItem` tracking; debounced typing groups       |
| P2-02 | M      | done   | Line-number gutter (left column horizontal, top strip vertical); `show_line_numbers` setting       |
| P2-04 | S      | done   | Word-wrap toggle wired to `Buffer::set_wrap`; `word_wrap` setting                                   |
| P2-03 | L      | done   | Find & Replace as a plugin (`src/plugins/find_replace.rs`); regex-backed; match highlight overlay  |
| P2-05 | M      | done   | Auto-save draft to `localStorage`; non-blocking recovery banner on startup                         |

Order followed the plan's own dependency note: undo landed first since later
features (replace, replace-all) needed to be undoable as atomic steps, and
the gutter's padding refactor was a prerequisite for both the gutter itself
and for the find/replace overlay sharing the same coordinate transform.

## Design decisions

- **Undo/redo reuses cosmic-text's built-in change tracking instead of
  full-text snapshots.** `cosmic_text::Editor` already accumulates a
  `Change` (list of `ChangeItem`s) between `Edit::start_change` /
  `Edit::finish_change`, and every `delete_range` / `insert_at` call
  (therefore every `Action::Insert`/`Backspace`/`Delete`/`Enter`/`Indent`)
  appends to it automatically. `HistoryManager`
  (`src/editor_core/history.rs`) only decides *when* to open/close that
  window; it stores no document text itself. `undo()` clones the popped
  `Change`, reverses the clone, and applies it via `Edit::apply_change`;
  `redo()` re-applies the original forward `Change`. This is O(edit size),
  not O(document size), and required no new fields on the buffer/editor.
- **Debounced typing groups fall out of the same start/finish window.**
  Consecutive keystrokes of the same `EditKind` within `DEBOUNCE_MS`
  (700ms) simply don't call `finish_change()` between them, so
  cosmic-text keeps appending `ChangeItem`s to one `Change` - i.e. one
  undo step for a whole burst of typing. A different `EditKind`, a
  pause, a cursor motion, or a click flushes (closes) the current group
  first. This also gives the Mongolian format-control-aware
  `handle_backspace`/`handle_delete` a single undo step "for free": the
  caller opens one group before calling either function, which may issue
  several synthesized `Action::Backspace`/`Delete` calls that all land in
  the same `Change` — satisfying the plan's explicit requirement without
  any special-casing in those functions.
- **Paste-like operations (`insert_text`, `delete_selection`, find/replace
  substitutions) are always their own atomic undo step**, never coalesced
  with adjacent typing or with each other. "Replace All" wraps its entire
  loop in a single `start_change`/`finish_change` pair, so undoing it
  reverts every replacement at once.
- **Gutter/line-numbers reuse the existing scroll+padding transform
  instead of a second rendering pass.** `WasmEditor::content_padding()`
  widens whichever side the gutter occupies (`GUTTER_WIDTH` on the left
  for horizontal mode, `GUTTER_HEIGHT` on top for vertical mode) and
  replaces the two previously-hardcoded `VERTICAL_LEFT_PADDING` /
  `VERTICAL_TOP_PADDING` constants everywhere they were used: `render`,
  and all four hit-testing sites (`handle_mouse_down`, `handle_mouse_move`,
  `handle_touch_start`, `handle_touch_move`). This keeps click-to-cursor
  mapping correct regardless of whether the gutter is showing. With
  `show_line_numbers: false` (the default), `content_padding()` returns
  the original constants unchanged, so existing behavior is byte-for-byte
  identical - this feature is purely additive and opt-in.
- **Gutter numbers are drawn via the 2D canvas context, not cosmic-text
  shaping.** Plain ASCII digits don't need swash/font shaping; `draw_gutter`
  runs after `put_image_data` and uses `fill_text`. Only the first visual
  row of a wrapped source line is labeled, matching common editor
  convention.
- **Find/replace match highlighting required generalizing cosmic-text's
  selection-highlight algorithm.** `cosmic_text::Editor::draw` computes
  selection rectangles with a per-glyph, per-grapheme loop that is not
  exposed as a reusable primitive (the public `LayoutRun::highlight`
  helper only handles the horizontal-mode coordinate space, not vertical).
  `highlight_rects` in `src/lib.rs` is a from-scratch port of that
  algorithm generalized from "the current selection" to an arbitrary
  `Cursor` range, so it works for both orientations and can be reused for
  any future range-based overlay (e.g. a future "highlight all format
  control chars" decoration from P3-01). Rectangles are blended into the
  pixel buffer *before* `Editor::draw` runs, so glyphs render on top of
  the highlight tint - matching how cosmic-text itself layers selection
  under glyphs.
- **Find & Replace state lives on `EditorState`, not inside the plugin
  struct.** Command handlers in the P1 plugin substrate are bare `fn`
  pointers with no access to `&mut Self` on the plugin instance (see
  `prompt/PLUGIN_API.md`'s note on this limitation). `FindReplaceState`
  is therefore a plain field (`EditorState::find_replace`), following the
  same "plugin data slot" pattern already used for `settings` and
  `history`. This also means `WasmEditor::render` can read
  `state.find_replace.matches` directly with no new WASM bridging method.
- **Search always goes through the `regex` crate**, even for "plain text,
  case-insensitive" queries: a literal query is escaped via `regex::escape`
  first, so "plain" and "regex" mode share one matching path (whole-word
  wraps the pattern in `\b...\b`; case sensitivity toggles
  `RegexBuilder::case_insensitive`). This avoids maintaining two matchers
  and made whole-word trivial.
- **No new WASM methods for find/replace** - `editor.find.setQuery` /
  `.next` / `.prev` / `.replaceCurrent` / `.replaceAll` / `.clear` are
  ordinary plugin commands invoked via the existing
  `run_command(id, args_json)` bridge from P1-03. The JS panel never talks
  to a bespoke API surface; it looks and feels like any other command.
- **Auto-save polls rather than hooking a change event.** There is no
  "text changed" JS-visible event today (P1's `EditorEvent::TextChanged`
  is Rust-internal, fanned out to plugins, not exposed to JS). Rather than
  add new plumbing, `AutoSave` polls `editor.get_text()` every 3 seconds
  and only writes to `localStorage` when the text actually differs from
  the last poll - this reproduces the "debounced, every 2-5s after the
  last edit" behavior from the plan without needing a new event bridge.
- **The draft is cleared on every explicit save or file open, not on a
  timestamp comparison.** The plan says "if a newer draft exists relative
  to any last-opened file, offer recovery" - rather than tracking a
  per-file last-modified timestamp (browsers don't expose reliable mtimes
  for downloaded/re-opened files), `FileManager.saveAsTxt` / `saveAsUns` /
  `loadTxtFile` / `loadUnsFile` all call `autoSaveManager.clearDraft()` on
  success. A draft surviving to the next page load therefore always means
  the previous session ended without an explicit save - exactly the
  "recover unsaved work" case, with no timestamp arithmetic needed.
- **The find/replace panel and the auto-save banner do not animate.**
  Consistent with every other dialog in this codebase (settings, save,
  translit, command palette), both use a plain `hidden`/`flex` class
  toggle. A CSS transform-based slide animation was considered but
  rejected: keeping focusable inputs permanently in the DOM (required for
  a transform transition, since `display:none` can't animate) creates a
  minor tab-order accessibility gap for no functional benefit, and would
  be the only animated surface in an otherwise non-animated UI.

## Files changed

- **New:** `src/editor_core/history.rs` (253 lines)
  - `EditKind` (coalescing classification), `HistoryManager` (undo/redo
    stacks over `cosmic_text::Change`, capped at 200 entries via
    `VecDeque`, debounce/grouping decisions).
  - 7 unit tests covering grouping, debounce timeout, paste-is-atomic,
    commit/redo-clearing, and the history cap.
- **New:** `src/plugins/mod.rs` (9 lines) - new home for feature plugins,
  distinct from the plugin *substrate* in `editor_core::plugin`.
- **New:** `src/plugins/find_replace.rs` (487 lines)
  - `MatchRange`, `FindReplaceState` (query/options/matches/current,
    `search`/`advance`/`select_from`/`cursor_range`/`ranges`/`summary_json`).
  - `FindReplacePlugin` contributing six commands: `editor.find.setQuery`,
    `.next`, `.prev`, `.replaceCurrent`, `.replaceAll`, `.clear`.
  - 6 unit tests covering case sensitivity, whole word, regex mode,
    invalid-regex error handling, and match-index wraparound.
- Modified: `src/editor_core/mod.rs` - publish `history`.
- Modified: `src/editor_core/editor_state.rs` (+128)
  - New fields: `history: HistoryManager`, `find_replace: FindReplaceState`.
  - New methods: `begin_history_group`, `flush_history_group`, `undo`,
    `redo`, `can_undo`, `can_redo`, `discard_history`,
    `refresh_find_matches`.
  - `update_settings` / `new` now apply `word_wrap` to the buffer
    (`Buffer::set_wrap`).
  - Registers `FindReplacePlugin` alongside `CoreCommandsPlugin`.
- Modified: `src/config/settings.rs` (+17) - `EditorBehaviorSettings` gains
  `show_line_numbers: bool` (default `false`) and `word_wrap: bool`
  (default `true`), both `#[serde(default = ...)]` so older saved/`.uns`
  settings JSON without these fields still deserializes.
- Modified: `src/lib.rs` (+437/-54... see diff)
  - `content_padding()`, `GUTTER_WIDTH`/`GUTTER_HEIGHT` constants,
    `draw_gutter()`.
  - `blend_rect()` and `highlight_rects()` free functions (shared pixel
    blending; generalized selection-highlight algorithm).
  - `handle_key_down`: `Ctrl+Z`/`Ctrl+Shift+Z`/`Ctrl+Y` interception;
    `EditKind`-tagged `begin_history_group` calls on
    Backspace/Delete/Enter/Tab/char-insert; `flush_history_group` on all
    motion keys.
  - `handle_mouse_down` / `handle_touch_start`: flush history group before
    relocating the cursor; all four mouse/touch handlers use
    `content_padding()` instead of the old constants.
  - `set_text`: discards undo history and find/replace state (new
    document). `insert_text` / `delete_selection`: wrapped as atomic
    "Paste" history groups.
  - New WASM methods: `undo()`, `redo()`, `can_undo()`, `can_redo()`,
    `set_cursor_position(line, column)`.
- Modified: `Cargo.toml` - new dependencies `regex = "1"` and
  `unicode-segmentation = "1"`.
- Modified: `index.html` (+530)
  - Settings modal: new "Editor" tab (`show_line_numbers`, `word_wrap`
    checkboxes), wired into `SettingsModal.loadSettings`/`.save`.
  - New `#find-panel` (find/replace UI: query, replace, case/word/regex
    toggles, match counter, next/prev/replace/replace-all/close) and the
    `FindReplacePanel` class, talking exclusively through
    `editor.run_command`. Keybindings `Ctrl+F`, `Ctrl+H`, `F3`,
    `Shift+F3`, `Escape` (global handler defers to the panel's own
    keydown handler via `stopPropagation` to avoid double-handling).
  - New `#autosave-banner` and the `AutoSave` class (poll-based draft
    save, `readDraft`/`clearDraft`); wired into `main()` (recovery
    banner + Restore/Dismiss) and into `FileManager`'s four save/open
    success paths.
  - Cache-busting query params bumped (`uns_editor.js?v=6`,
    `uns_editor_bg.wasm?v=6`, `output.css?v=10`).
- Modified: `dist/output.css` - rebuilt via `npm run build:css` to include
  the new utility classes used above (`bg-amber-500`,
  `accent-editor-accent`, etc.).

## Verification

- `cargo test --lib` - 17 tests pass (7 new in `history`, 6 new in
  `plugins::find_replace`, 4 pre-existing from P1).
- `cargo check --target wasm32-unknown-unknown` - only the 5 pre-existing
  warnings (deprecated `set_fill_style` ×3, `Dictionary::len` unused,
  `cosmic-text`'s own upstream warnings). No new warnings.
- `wasm-pack build --target web --release` - succeeds.
  `pkg/uns_editor.d.ts` exposes `undo()`, `redo()`, `can_undo()`,
  `can_redo()`, `set_cursor_position(line, column)`, alongside the
  already-present `list_commands()`/`run_command()` that the find/replace
  panel and undo/redo palette entries use.
- `node --check` against the extracted `<script type="module">` body -
  no syntax errors.
- `npm run build:css` - rebuilt `dist/output.css`; confirmed new classes
  (e.g. `bg-amber-500`) present in the output.
- Manual code-path review (no browser available in this environment):
  - Default settings (`show_line_numbers: false`, `word_wrap: true`)
    reproduce the exact padding/wrap behavior that existed before this
    phase - verified by reading `content_padding()` and the
    `update_settings`/`new` wrap wiring, not just testing the new branch.
  - Traced `Ctrl+F` / `Ctrl+H` / `F3` event flow through both the
    canvas-level and document-level `keydown` listeners to confirm no
    double-handling and that the browser's native find bar is still
    suppressed via `preventDefault`.

## Follow-ups / known limits

- **No manual in-browser QA.** This sandbox has no display/browser, so
  verification stopped at static analysis + Rust test/build success +
  JS syntax checking. The next session with a real browser available
  should smoke-test: undo/redo across format-control backspace/delete,
  gutter numbering in both orientations while scrolling, find/replace in
  vertical mode (the highlight-rectangle vertical-mode math is the
  highest-risk untested path), and the auto-save recovery banner after a
  hard reload.
- **`Ctrl+F`/`Ctrl+H` browser shortcut conflicts.** Same caveat as
  `Ctrl+Shift+P` from Phase P1: some browser/OS combinations reserve these
  (e.g. Cmd+F on some in-app webviews). `preventDefault()` covers the
  common case; a configurable-keybindings layer (P6-01) would let a user
  route around a stubborn conflict.
- **Find/replace search never crosses a line boundary.** Matching is
  line-by-line (`FindReplaceState::search` iterates `buffer.lines`
  independently), so a regex like `foo\nbar` will never match even though
  the buffer contains it. This mirrors how the editor already treats
  lines as the atomic unit elsewhere (e.g. the format-control backspace
  logic) and was an explicit scope cut, not an oversight.
- **Auto-save is poll-based, not event-based.** A real "document changed"
  event (already scaffolded as `EditorEvent::TextChanged` in the Rust
  event bus, just not exposed to JS) would let auto-save react
  immediately after a debounce window instead of on a fixed 3s cadence.
  Low priority: the visible behavior difference is at most one poll
  interval of latency.
- **No settings UI for auto-save.** The plan didn't call for a toggle, so
  none was added; it is always on. If a future session wants an opt-out,
  it's a small addition (a checkbox gating `AutoSave.start()`).
- **Gutter width is fixed** (`GUTTER_WIDTH = 44px`), not sized to the
  actual digit count of the largest line number. Fine for realistic
  document lengths (up to 4 digits); very large documents would need a
  dynamic width.
- **`highlight_rects` is a hand-ported duplicate of cosmic-text's
  selection algorithm**, not a shared upstream primitive. Any future
  upstream change to that algorithm (e.g. a cosmic-text version bump)
  would need this function re-synced by hand. Flagged here so it isn't
  forgotten; not worth abstracting further until a third caller needs it.

## Next phase

Phase P4 (theming), P8-04 (zoom shortcut), and P5-02 (dirty indicator) are
next per the plan's suggested order - UX polish, all independent of each
other and of anything added in P2. `EditorState::history` and
`find_replace` are stable extension points (both already have the "data
lives on `EditorState`, plugin contributes commands" shape) that later
phases can follow, e.g. P3-01's format-control visibility overlay is a
natural `highlight_rects`/`blend_rect` consumer.
