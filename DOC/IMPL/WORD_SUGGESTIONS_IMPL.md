# Phase P9 — Word suggestion popup (implementation log)

Date: 2026-08-11
Reference: `prompt/FUTURE_PLAN.md` Phase P9; `prompt/WORD_SUGGESTIONS_PLAN.md`
(full design), items WS-01 through WS-06 (WS-07 phrase-suggestions and
WS-08 mobile/touch positioning explicitly deferred, per the plan's own
`(deferred/follow-up)` markers)
Patch: `patch/word-suggestions.patch` (initial implementation); see also
`patch/word-suggestions-arrow-nav-fix.patch`, `patch/word-suggestions-ws09.patch`,
and `patch/word-suggestions-mobile-trigger.patch` for follow-up work
found through further testing after the initial commit (see their
respective sections below).

## Scope

Full desktop MVP for the word suggestion popup, confirmed with the user
before coding:

| ID    | Effort | Status | Summary |
|-------|--------|--------|---------|
| WS-01 | S      | done   | `word_boundary.rs` — word-bounds scanning + script classification, pure functions |
| WS-02 | M      | done   | `Dictionary` prefix indexing (Cyrillic + Mongolian), ranked `suggest()` |
| WS-03 | M      | done   | `SuggestionState` + `WasmEditor` methods (`get_word_suggestions_json`, `accept_word_suggestion`, `dismiss_word_suggestions`, `load_dictionary_text`) |
| WS-04 | M      | done   | `WordSuggestionsPopup` JS class, markup, trigger wiring, keyboard precedence |
| WS-05 | S      | done   | `tests/e2e/word-suggestions.spec.js` — 7 Playwright tests |
| WS-06 | S      | done   | Settings toggle (`word_suggestions_enabled`, Editor tab) |
| WS-07 | M      | deferred | Phrase-aware suggestions — explicitly deferred per the plan |
| WS-08 | M      | deferred | Mobile/touch popup positioning — explicitly deferred per the plan |

Scope confirmed with the user before implementation: trigger on both
Cyrillic and Latin-mode-produced Mongolian input, after any letter
typed; accept via keyboard (arrows/Tab/Enter/Esc, later extended to
number keys — see below) *and* mouse click; additive, does not touch
the Transliteration modal; lazy dictionary loading; digits are word
boundaries, not word characters; settings toggle required.

Four rounds of user testing after the initial implementation surfaced
real bugs beyond the original plan's scope — each is called out below
since they materially changed the design from what was first shipped.

## Design decisions

- **`SuggestionState` is a plain `EditorState` field, not a `Plugin`**
  (`src/editor_core/suggestions.rs`) — unlike `find_replace`, there's no
  natural command-palette action here beyond the three direct
  `WasmEditor` methods, so the extra indirection of a `Plugin` impl
  wasn't worth it.
- **`Dictionary::suggest()` always returns Mongolian Bichig strings**
  regardless of the query script (`src/dictionary/lookup.rs`) — a
  Cyrillic query looks up the Mongolian translation via
  `cyrillic_index`; a Mongolian-script query completes against
  `mongolian_index` (built from the dictionary's own Bichig column used
  as a word list). Both indices exclude phrase entries (~75% of the
  101,869-row dictionary), keeping only the ~25,019 single-word rows.
  Ranking: exact match first, then shorter key length, then
  alphabetical; deduped; capped at `SUGGESTION_LIMIT` (8), scanning
  capped at `MAX_PREFIX_SCAN` (500 raw matches) for performance.
- **Dictionary loading is synchronous from the Rust side.** The
  Transliteration modal's existing `translit_load_dictionary` is `async`
  and holds a `&mut self` borrow across its `.await` — safe there
  because the modal traps focus away from the canvas during its own
  load. Word suggestions are triggered by canvas typing, so a `keyup`
  for the very keystroke that triggered the load can fire *while that
  borrow is still held* — this actually panics wasm-bindgen
  ("recursive use of an object detected"), confirmed during manual
  testing, not just a theoretical risk. Fix: a new synchronous
  `WasmEditor::load_dictionary_text(&mut self, text: &str)` with no
  `.await` in Rust; JS does the `fetch()` + `.text()` in plain
  JavaScript (no WASM object involved during the async gap) and hands
  the already-resolved string to the synchronous method.
- **`TabManager.loadActiveIntoEditor()` is the single centralized point**
  for dismissing suggestions on any tab-switching action
  (switchTo/closeTab/createTab all funnel through it), rather than
  scattering `dismiss()` calls across every call site.

### Bugs found during user testing (beyond the original plan)

The plan covered the feature's happy path; testing against the real
vertical-Mongolian rendering mode surfaced four real bugs the plan
didn't anticipate, each fixed in turn:

1. **Suggestions rendered as ordinary horizontal HTML text even in
   vertical mode.** Fixed by making `WordSuggestionsPopup.render()`
   orientation-aware: in vertical mode, each suggestion's word gets
   `writing-mode: vertical-lr; text-orientation: mixed`.
   `text-orientation: mixed` (not `sideways`) matters specifically:
   Mongolian's *natural* form is already vertical, so `mixed` (which
   honors Unicode's per-script vertical-orientation property) draws it
   upright/correctly, while `sideways` would incorrectly force-rotate
   it as if it were Latin — confirmed by a side-by-side CSS comparison
   before picking one.
2. **Popup position tracking was fundamentally broken in vertical
   mode**, through three separate layers of the same underlying issue:
   - The anchor position didn't offset in the right axis at all
     initially (fixed to check `settings.editor.orientation` and offset
     the correct axis — X in vertical mode, Y in horizontal).
   - A fixed pixel offset guess (`24`) happened to roughly equal a full
     column width, so "beside the cursor" always landed exactly at the
     start of the *next* column — unavoidably overlaying whatever
     content already existed there, reading as "shows on the second
     column." Fixed with a small fixed gap (6px) instead of a full
     `lineAdvance`, so the popup hugs the cursor's own column/glyph.
   - Most significantly: `get_word_suggestions_json()` is called
     synchronously right after `handle_key_down` inserts a character —
     *before* the next animation frame's `render()` call (the only
     other place that shaped the buffer) gets a chance to run. It was
     therefore reading stale/unshaped layout data from
     `cursor_position()`, silently falling back to a default position.
     This made the popup appear frozen in the same spot no matter how
     much more was typed. Confirmed via a **native (non-WASM) Rust
     reproduction** against the patched `cosmic-text` crate directly
     (much faster to iterate on than the browser) that `cursor_position()`
     genuinely requires `shape_as_needed()` to have run since the last
     edit. Fixed by calling `self.state.editor.shape_as_needed(...)` —
     the same call `render()` makes, and a no-op if nothing changed — at
     the top of `get_word_suggestions_json()`.
3. **The popup reappeared on every cursor movement (arrow keys), not
   just while typing.** The canvas `keydown` handler called
   `wordSuggestions.refresh()` unconditionally after every
   `editor.handle_key_down(e)`, including pure motion keys — directly
   contradicting the plan's own §7 trigger table ("Arrow keys / click /
   touch that move the cursor without changing text → dismiss"). Fixed
   by changing `handle_key_down`'s return type from `Result<(), JsValue>`
   to `Result<bool, JsValue>`, reusing an *already-existing* internal
   `KeyEffect` enum (`None`/`Motion`/`TextChange`) that was tracked for
   event-bus dispatch but never exposed to JS. JS now calls `refresh()`
   only when the keystroke actually changed text, and `dismiss()`
   otherwise.
4. **Requested layout change (not a bug, but a real design revision):**
   the user asked for each suggestion to get its own column (arranged
   side-by-side) rather than all suggestions stacking in one column,
   numbered so a suggestion can be picked by its ordinal. Implemented
   as a vertical-mode-only change (confirmed with the user — horizontal
   mode keeps its original stacked single-column list, since "column"
   is specifically a vertical-mode concept): `<ul>` becomes `flex-row`;
   each `<li>` contains a small horizontal number badge (not rotated,
   so it reads as a normal digit) above the vertically-written word.
   Also added: pressing the corresponding number key (1-8) accepts that
   suggestion directly, falling through to ordinary typing if the digit
   is out of range for however many suggestions are currently showing
   (e.g. pressing "9" with only 3 suggestions inserts "9" literally).

## Files changed

- **New:** `src/editor_core/word_boundary.rs` (206 lines) — WS-01.
  `current_word_bounds`, `Script` enum, `classify_script`; digits are
  word boundaries, format-control characters are word-internal. 14
  unit tests.
- **New:** `src/editor_core/suggestions.rs` (73 lines) — WS-03.
  `SuggestionState { word_start, word_end, suggestions }`, plain struct
  (not a `Plugin` — see design decisions). 1 unit test.
- Modified: `src/dictionary/lookup.rs` (+265) — WS-02. `Dictionary`
  extended with `entries` / `cyrillic_index` / `mongolian_index`
  (sorted-`Vec` prefix search) and `suggest(prefix, script)`. Existing
  `lookup()` (exact match, used by the Transliteration modal) untouched.
  10 new unit tests.
- Modified: `src/config/settings.rs` (+12) — WS-06.
  `EditorBehaviorSettings.word_suggestions_enabled: bool`, default `true`.
- Modified: `src/editor_core/editor_state.rs`, `src/editor_core/mod.rs`
  (+8) — wire `SuggestionState` into `EditorState`, publish the new
  `word_boundary`/`suggestions` modules.
- Modified: `src/lib.rs` (+270) — WS-03/WS-04 bug fixes:
  - `get_word_suggestions_json()`, `accept_word_suggestion()`,
    `dismiss_word_suggestions()`, `load_dictionary_text()`,
    `no_word_suggestions()` helper, `MIN_SUGGESTION_WORD_LEN` const.
  - `get_word_suggestions_json()` also returns `lineAdvance` (the real
    line/column spacing at the cursor) instead of JS guessing a fixed
    pixel offset, and calls `shape_as_needed()` before reading
    `cursor_position()` (bug #2 above).
  - `handle_key_down`'s return type changed from `Result<(), JsValue>`
    to `Result<bool, JsValue>` (bug #3 above) — `true` iff the
    `KeyEffect` for that keystroke was `TextChange`.
  - `set_text` now also clears `suggestions` state.
- Modified: `index.html` (+428) — WS-04/WS-06 + all four bug fixes:
  - `#word-suggestions-popup` markup; `#word-suggestions-enabled`
    settings checkbox (Editor tab).
  - `WordSuggestionsPopup` class (module-scope singleton, mirrors
    `TabManager`/`KeybindingManager`): `ensureDictionaryLoaded()`
    (shared in-flight promise so concurrent `refresh()` calls during
    the fetch all eventually see the loaded state, rather than only the
    call that happened to trigger it), `refresh()`, `render()`
    (orientation-aware — numbered flex-row columns in vertical mode,
    original stacked list in horizontal mode), `position()`
    (orientation-aware anchor offset), `show()`/`hide()`, `next()`/
    `prev()`, `acceptIndex()`/`acceptByNumber()`/`accept()`, `dismiss()`.
  - Canvas `keydown` listener: popup-open key interception (Escape,
    ArrowUp/Down, Tab/Enter, digit keys 1-9) before the existing
    Ctrl+C/X/V/Latin-conversion branches; `refresh()`-vs-`dismiss()`
    decision based on `handle_key_down`'s new boolean return; also
    wired after Ctrl+X/paste/Latin-mapped-character insertion.
  - `TabManager.loadActiveIntoEditor()`, `clear-btn`, logo-btn
    version-load handler: dismiss word suggestions.
  - Cache-busting bumped: `uns_editor.js?v=11`, `uns_editor_bg.wasm?v=11`
    (rebuilt several times across the bug-fix rounds above; CSS
    unchanged at `?v=13`, no new Tailwind classes needed — all new
    styling is inline/JS-applied for the orientation-conditional cases).
- **New:** `tests/e2e/word-suggestions.spec.js` (226 lines) — WS-05. 7
  Playwright tests (see Verification).
- **New:** `prompt/WORD_SUGGESTIONS_PLAN.md` — the design document
  itself (written and reviewed before implementation started).
- Modified: `prompt/FUTURE_PLAN.md` — Phase P9 marked done, this impl
  log and patch cross-referenced.

## Verification

- `cargo test --lib` — **72 passed**, 0 failed (14 new in
  `word_boundary`, 10 new in `dictionary::lookup`, 1 new in
  `suggestions`; rest pre-existing).
- `cargo check --target wasm32-unknown-unknown` — clean, no warnings.
- `wasm-pack build --release --target web` — succeeds; exports confirmed
  (`get_word_suggestions_json`, `accept_word_suggestion`,
  `dismiss_word_suggestions`, `load_dictionary_text`).
- A throwaway native (non-WASM) Rust program linking the same
  `cosmic-text` crate as a path dependency was used to isolate and
  confirm the `shape_as_needed()` staleness bug (item 2 above) much
  faster than iterating through the browser — not part of this
  project's source tree, deleted after use.
- **`npx playwright test` — 13/13 passed** against the real headless
  Chromium set up in the P7-04 session:
  - `word-suggestions.spec.js` (new, 7 tests): exact-suggestion
    accept-via-Enter using the real dictionary prefix "аалз" (exactly
    one single-word match after phrase filtering, verified against the
    real 101,877-line TSV); Escape-dismiss; arrow-nav + mouse-click
    using "аа" (58 matches, capped to 8, exercising the ranking/cap
    logic); number-key (`"3"`) accepts the exact 3rd suggestion;
    out-of-range number key (`"9"`) falls through to literal insertion;
    settings-toggle disables the popup; tab-switch dismisses it.
  - `smoke.spec.js`, `tabs.spec.js` (×2), `keybindings.spec.js`,
    `latin-mapping.spec.js`, `rendering.spec.js` (all pre-existing) —
    re-run unchanged to confirm no regression, since this feature
    touches the canvas `keydown` handler every one of them exercises.
  - Playwright-specific gotcha discovered and worked around:
    `page.keyboard.type()` doesn't dispatch real `keydown` events for
    Cyrillic characters on a `<canvas>` (no US-keyboard-layout mapping
    exists for them — Playwright silently falls back to a CDP
    `Input.insertText`-style call, which has nothing to insert into
    since the canvas isn't a native editable element). Worked around
    with a `typeCyrillic()` test helper that dispatches synthetic
    `KeyboardEvent`s directly. This is a test-tooling limitation, not
    an application bug.

## Follow-up fix (2026-08-11, after the initial commit)

Patch: `patch/word-suggestions-arrow-nav-fix.patch`

The numbered multi-column layout (bug #4 above) introduced a fifth
regression, found through further user testing after the feature was
already committed: `ArrowLeft`/`ArrowRight` — the keys a user would
naturally reach for to navigate a *horizontal row* of columns — still
fell through to ordinary cursor motion, which (per bug #3's fix)
immediately dismisses the popup. `ArrowUp`/`ArrowDown` still worked
correctly the whole time, but the redesign made them feel like the
"wrong" axis, and the user summarized this as "only accessible by
number now" - closing the popup on the first arrow key they actually
tried was indistinguishable, from their side, from "arrow navigation is
broken."

Fixed by intercepting `ArrowRight`/`ArrowLeft` (mapped to `next()`/
`prev()`, matching Mongolian's own left-to-right column order) in the
canvas `keydown` handler specifically when `wordSuggestions.isVerticalMode()`
is true — i.e. only when the popup is actually laid out as columns.
Horizontal mode's `ArrowLeft`/`ArrowRight` are untouched (that layout is
still the original single-column stacked list, where Up/Down remains
the only navigation axis). `ArrowUp`/`ArrowDown` continue to work
unconditionally in both orientations, as they always did.

Added a new Playwright test (`Left/Right arrows navigate the numbered
columns in vertical mode without dismissing`) exercising exactly this:
ArrowRight twice, ArrowLeft once, then Enter, asserting the popup
never disappears mid-navigation and the final accepted word matches
whichever column ended up highlighted. Full suite re-run: 14/14 passed
(8 in `word-suggestions.spec.js`, up from 7). `cargo test --lib`: 72/72
(no Rust touched — this was a pure JS fix).

## WS-09: canvas-rendered popup (2026-08-13)

Patch: `patch/word-suggestions-ws09.patch`

### Motivation

The numbered-column popup (bug #4 in the "Bugs found during user
testing" section above) rendered each suggestion's Mongolian word as
ordinary DOM text under `writing-mode: vertical-lr; text-orientation:
mixed`. The user raised a real architectural concern: this asks the
*browser's own* text engine to shape and rotate Mongolian glyphs,
while the main document canvas shapes 100% of its text itself (via the
project's vendored, patched `cosmic-text` fork, which uses `harfrust` -
a pure-Rust HarfBuzz port - for shaping and `swash` for rasterization,
with the browser used only to `put_image_data` the resulting pixels).
Different browsers have inconsistent (sometimes absent) support for
shaping Mongolian text under CSS vertical writing modes, so the popup
could visually mismatch the very document it's suggesting completions
for. Also separately fixed by this same change: the popup's text had
been noticeably smaller than the editor's actual font size (Tailwind
`text-sm`/`text-xs`, ~14px/12px, versus this editor's real default of
43px) - WS-09 fixes both at once, since the new Rust-side rendering
reads `settings.fonts.font_size` directly rather than JS guessing a
fixed CSS size.

### Design

Confirmed with the user before implementation: render the *whole*
popup (numbers, words, and the selected/hover highlight) via canvas,
not just the word glyphs; keep mouse hover highlighting (added
`mousemove` hit-testing + a cheap highlight-only redraw, rather than
dropping hover for simplicity); add a screen-reader text mirror (canvas
has no semantic DOM content of its own); track as a new backlog item
(WS-09) rather than folding it in untracked.

Two-call flow per suggestion-list change, mirroring - and directly
modeled on - the existing `TranslitRenderer`/`translit_render` pattern
(`src/translit/renderer.rs`, `WasmEditor::translit_render`) that
already proved this exact "small standalone `cosmic_text::Buffer`,
shared `FontSystem`/`SwashCache`, blit via `ImageData`" pipeline works
for rendering arbitrary Mongolian text outside the main document
buffer:

1. **`measure_suggestions_popup()`** - for each suggestion, builds a
   temporary number-`Buffer` (vertical mode only, horizontal
   orientation, small font) and word-`Buffer` (the editor's real
   orientation/font size), shapes each, and runs one *dry*
   `Buffer::draw` pass per buffer (draws nothing anywhere - the
   callback just accumulates a bounding box) to measure its exact
   pixel width/height - the same "dry pass" trick
   `TranslitRenderer::render` already used once for centering a single
   block of text, generalized here to N items. Feeds those measured
   sizes into a new pure, unit-tested layout module (see below) to get
   each item's target rectangle, caches the result (rectangles +
   per-item glyph-origin-cancelling draw offsets - see "Why cache the
   layout" below) on `WasmEditor`, and returns
   `{ width, height, itemBounds }` to JS.
2. JS resizes `#word-suggestions-canvas` to that size (DPR-aware, same
   pattern as the main/translit canvases) and calls
   **`render_suggestions_popup(canvas_id, selected_index, hover_index)`**,
   which reuses the cached layout (does *not* re-measure) to draw the
   highlight rect for whichever index is selected/hovered, then each
   item's number + word, blitting the result via `ImageData` exactly
   like `WasmEditor::render`/`TranslitRenderer::render` already do.

New pure module `src/editor_core/suggestion_popup_layout.rs` - the
arithmetic for arranging N pre-measured `(number_w, number_h, word_w,
word_h)` tuples into either a horizontal row of columns (vertical
editor mode) or a vertical stack of rows (horizontal editor mode),
with no dependency on `cosmic-text`/WASM/a browser at all, mirroring
this codebase's established pattern of keeping arithmetic testable
without a browser or real fonts (`word_boundary.rs`,
`format_control.rs`). 7 unit tests (empty list, single-item padding,
multi-item packing/shared max-height-or-width, horizontal-mode
word-width clipping, `Rect::contains`'s half-open bounds).

**Why cache the layout instead of recomputing on every draw**: JS must
know the popup's pixel size *before* it can resize the `<canvas>`
element, so a single combined "measure and draw" call can't work -
JS needs the size back first. Caching (rather than having
`render_suggestions_popup` redo its own measurement) also guarantees
the two calls can never visually disagree by a stray pixel from float
rounding, and lets keyboard/mouse-driven highlight changes (arrow
navigation, hover) call `render_suggestions_popup` again *without*
re-measuring at all - only the actual suggestion list changing
(`measure_suggestions_popup`) needs the full remeasure pass.

**Bug found and fixed during implementation**: the first version of
`render_suggestions_popup` used `self.suggestion_popup_cache.take()`
(consuming the cache) without ever restoring it after a successful
draw. This meant the *first* render after a `measure_suggestions_popup()`
call worked, but any subsequent `render_suggestions_popup` call for
the same list (i.e. every arrow-key press, number-key highlight
change, or mouse hover) silently found no cache and did nothing -
visually, the popup appeared to never move its highlight past the
first selection. Confirmed via direct canvas pixel-color inspection in
a Playwright script (checking the actual RGBA bytes at known column
positions) before finding the root cause. Fixed by `.clone()`-ing the
cache instead of `.take()`-ing it. This also caught a second, related
mistake: after fixing the Rust source, `wasm-pack build` wasn't
re-run before testing in the browser, so the *old* buggy `.wasm` was
still being served - a reminder (already known from earlier sessions,
re-learned here) that `cargo check`/`cargo test` alone never rebuild
the actual WASM artifact the browser loads.

**Colors**: the popup's background/highlight/text colors are hardcoded
Rust `Color` constants matching this app's fixed dark UI-chrome
Tailwind classes (`bg-editor-header` `#252526`, `bg-editor-accent`
`#0e639c`, `hover:bg-editor-border` `#3e3e42`, `text-editor-text`
`#cccccc`, `text-editor-text-dim` `#858585`, from `tailwind.config.js`)
rather than reading `settings.appearance` - that setting only affects
the *document's* own theme colors (light/dark content themes), not the
app's surrounding UI chrome (toolbar, modals, popups), which stays a
fixed dark theme regardless, matching how the popup already looked
before this rewrite.

**Accessibility**: `#sr-suggestions`, a new visually-hidden
`aria-live="polite"` region (same pattern as `#sr-status`/`#sr-cursor`
from P6-02), mirrors "Suggestion N of M selected: `<word>`. Press a
number key, arrow keys, or Enter to choose; Escape to dismiss." -
updated on every `render()`/navigation call, cleared on dismiss.

**Test hook**: added `window.__unsTestHooks.getSuggestionsState()`
(exposing `WordSuggestionsPopup`'s `suggestions`/`selected`/
`itemBounds`/`isOpen()` state) since the popup's content is no longer
DOM text Playwright can query directly - same "expose what the app
itself already computes, not a new capability" principle the rest of
`__unsTestHooks` follows.

### Files changed

- **New:** `src/editor_core/suggestion_popup_layout.rs` (pure layout
  arithmetic, 7 unit tests).
- Modified: `src/editor_core/mod.rs` (publish the new module).
- Modified: `src/lib.rs`:
  - New `suggestion_popup_cache: Option<SuggestionPopupCache>` field on
    `WasmEditor`.
  - New WASM methods `measure_suggestions_popup()` /
    `render_suggestions_popup(canvas_id, selected_index, hover_index)`.
  - New free functions (not `WasmEditor` methods, to avoid fighting the
    borrow checker over disjoint `&mut self.state.font_system` /
    `&mut self.state.cache` borrows): `measure_suggestion_text`,
    `draw_suggestion_text`, `suggestion_number_font_size`.
  - New layout constants (`SUGGESTION_CELL_PADDING_X/Y`,
    `SUGGESTION_NUMBER_WORD_GAP`, `SUGGESTION_MAX_ROW_WIDTH`,
    `SUGGESTION_NUMBER_LINE_HEIGHT_RATIO`).
- Modified: `index.html`:
  - `#word-suggestions-popup` markup: `<ul>` → `<div>` wrapping a new
    `<canvas id="word-suggestions-canvas">`; new `#sr-suggestions`
    live region.
  - `WordSuggestionsPopup`: constructor sets up canvas
    `mousedown`/`mousemove`/`mouseleave` listeners and new
    `itemBounds`/`hoverIndex` fields; `render()` rewritten to
    measure→resize→draw instead of building `<li>` DOM nodes; new
    `redraw()` (highlight-only, no remeasure) used by `next()`/
    `prev()`/hover; new `hitTest()`/`setupCanvasPointerEvents()`; new
    `updateAccessibilityMirror()`; removed `editorFontSizeCssPx()`
    (superseded - Rust now reads the real font size directly).
  - `window.__unsTestHooks.getSuggestionsState()` test hook.
  - Cache-busting bumped to `?v=12` (rebuilt several times across the
    bug-fix round above).
- Modified: `tests/e2e/word-suggestions.spec.js` - every test rewritten
  to use `getSuggestionsState()` instead of querying
  `#word-suggestions-popup`'s (now nonexistent) `<li>` children; the
  mouse-click test computes click coordinates from `itemBounds`
  converted back to CSS-pixel page coordinates (the inverse of
  `position()`'s own conversion); one new test for the accessibility
  mirror.

### Verification

- `cargo test --lib` - **79 passed** (7 new in
  `suggestion_popup_layout`, rest pre-existing).
- `cargo check --target wasm32-unknown-unknown` - clean, no warnings.
- `wasm-pack build --release --target web` - succeeds; exports
  confirmed (`measure_suggestions_popup`, `render_suggestions_popup`).
- Visual verification via Playwright screenshots: vertical mode's
  numbered columns (genuine `cosmic-text`-shaped Mongolian glyphs, not
  CSS), arrow-key navigation moving the highlight, mouse hover showing
  a distinct highlight color alongside keyboard selection, click
  accepting the correct item, horizontal mode's stacked rows, number
  keys accepting in both orientations.
- Rough performance check (not a rigorous benchmark, just a sanity
  check given the added shaping passes per keystroke): ~25ms for a
  full measure+render cycle over 8 suggestions, ~25ms per
  highlight-only `redraw()` call (10 consecutive `ArrowRight` presses
  in ~250ms) - both comfortably under the ~100ms "feels instant"
  threshold for interactive latency.
- **`npx playwright test` - 15/15 passed** (9 in
  `word-suggestions.spec.js`, up from 8; all pre-existing suite tests
  re-run unchanged to confirm no regression).
- **Explicit limitation, stated honestly (again)**: this sandbox has
  headless Chromium only. This change's entire purpose is guaranteeing
  identical rendering regardless of browser by bypassing browser text
  shaping entirely - a design guarantee inherent to the architecture
  (no browser-native Mongolian-shaping API is called anywhere in the
  new code, confirmable by inspection), not something this sandbox's
  Chromium-only test suite can empirically demonstrate cross-browser.

## WS-08 (partial): mobile trigger wiring (2026-08-14)

Patch: `patch/word-suggestions-mobile-trigger.patch`

The popup never appeared at all on mobile devices, reported by the
user testing on a real phone. Root cause: mobile virtual keyboards
drive text entry through `#mobile-input`'s native `input` event, not
`keydown` (unlike a physical keyboard) - and that `input` handler
(`index.html`, both the Latin-conversion and plain-insert branches)
never called `wordSuggestions.refresh()` at all. This was already
flagged as a known gap earlier in this log ("Mobile hidden-input
path's `keydown` listener... doesn't call `refresh()`") but the
*severity* wasn't clear until actually tested on a device - it wasn't
just Backspace/Enter missing a refresh, the entire feature was
unreachable on mobile since ordinary typing never triggered it in the
first place.

Fixed:

- `#mobile-input`'s `input` handler now calls `wordSuggestions.refresh()`
  after every `insert_text()` call (both the Latin-mapped and
  plain-Cyrillic branches) - this alone is what makes the popup appear
  on mobile at all.
- `#mobile-input`'s `keydown` handler (previously only Backspace/Enter,
  with no `refresh()`/`dismiss()` call either) now: (a) intercepts the
  same popup-navigation keys the desktop canvas listener does
  (Escape/arrows/Tab/Enter-accept/number-accept) - relevant for a
  physical/Bluetooth keyboard connected to a phone or tablet, which
  *does* send real `keydown` events with usable `.key` values even
  though the device is "mobile"; (b) for Backspace/Enter specifically,
  uses `handle_key_down`'s boolean return the same way the desktop path
  does, to `refresh()` or `dismiss()` accordingly instead of doing
  neither.
- `WordSuggestionsPopup`'s canvas (`#word-suggestions-canvas`) gained a
  `touchstart` listener alongside its existing `mousedown` one, mirroring
  why the *main* editor canvas already registers both
  (`handlePointerDown`'s comment: mobile browsers don't reliably fire a
  synthetic `mousedown` for every tap). `hitTest()` now accepts either a
  `MouseEvent` or a `TouchEvent`, reading `.touches[0]`/`.changedTouches[0]`
  for the latter.

New `tests/e2e/word-suggestions-mobile.spec.js` (3 tests), using
Playwright's `devices["Pixel 5"]` preset via `test.use()` so the app's
own `isMobileDevice` user-agent sniff genuinely takes the mobile
branch - deliberately an Android preset, not an iPhone one, since
Playwright's iOS presets default to the `webkit` browser engine, which
isn't installed in this sandbox (only Chromium is, per the P7-04
session) - Android presets default to `chromium`. Tests: typing via
the virtual-keyboard `input` path shows the correct suggestion;
tapping the popup canvas accepts it; Backspace refreshes (not leaves
stale) the suggestion list. All 18 tests (15 existing + 3 new) pass;
`cargo test --lib` unaffected (79/79, no Rust touched - pure JS fix).

Labeled "partial" (not a full close-out of WS-08) because popup
*positioning* on a small screen with an on-screen keyboard covering
much of the viewport (§11.4's original concern) is not addressed here
- only the "does it trigger and can you interact with it at all" gap,
which was the more fundamental problem the user actually hit.

## Follow-ups / known limits

- **WS-07 (phrase-aware suggestions) remain deferred**, exactly as
  scoped in the plan's §12. WS-08 is now partially addressed (trigger
  wiring - see above); popup positioning on small/keyboard-covered
  viewports is still not addressed.
- **Numbered multi-column layout is vertical-mode only**, per explicit
  user confirmation — horizontal mode's suggestion list is unchanged
  from the original stacked single-column design.
- **No before/after performance measurement** for the `shape_as_needed()`
  call added to `get_word_suggestions_json()` — it's documented as a
  no-op when nothing changed (same guarantee `render()` already relies
  on every frame), but this sandbox has no way to profile real
  browser CPU/frame timings beyond functional assertions.
- **WS-09's colors are hardcoded**, not read from any theme/settings
  object - correct today (the popup's UI chrome has never followed the
  document's own appearance theme), but would need revisiting if this
  app ever grows a "theme the UI chrome itself" feature.
- **WS-09's performance check is a rough sanity check, not a rigorous
  benchmark** - no baseline/before-after comparison exists, and this
  sandbox cannot profile real browser CPU/paint timings beyond
  functional latency assertions.

## Next phase

Per `prompt/FUTURE_PLAN.md`'s backlog, remaining after this: Phase P5
(`P5-01` recent files, `P5-03` `.uns` view state, `P5-04` PDF export),
Phase P8 stretch items (`P8-01` syntax highlighting, `P8-02` CRDT
collaboration), `P3-01`/`P3-02`/`P3-04` (format-control visibility
toggle, orientation-persistence regression test, input-mode status
pill), and this plan's own deferred `WS-07`/`WS-08`. None of these
block each other.
