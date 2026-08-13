# Phase P9 — Word suggestion popup (implementation log)

Date: 2026-08-11
Reference: `prompt/FUTURE_PLAN.md` Phase P9; `prompt/WORD_SUGGESTIONS_PLAN.md`
(full design), items WS-01 through WS-06 (WS-07 phrase-suggestions and
WS-08 mobile/touch positioning explicitly deferred, per the plan's own
`(deferred/follow-up)` markers)
Patch: `patch/word-suggestions.patch` (initial implementation); see also
`patch/word-suggestions-arrow-nav-fix.patch` for a follow-up fix found
through further testing after the initial commit (§"Follow-up fix"
below).

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

## Follow-ups / known limits

- **WS-07 (phrase-aware suggestions) and WS-08 (mobile/touch
  positioning) remain deferred**, exactly as scoped in the plan's §12.
- **Numbered multi-column layout is vertical-mode only**, per explicit
  user confirmation — horizontal mode's suggestion list is unchanged
  from the original stacked single-column design.
- **Mobile hidden-input path's `keydown` listener** (Backspace/Enter
  only) doesn't call `wordSuggestions.refresh()` at all — a pre-existing
  gap from WS-04's original implementation, not something introduced
  by this session's bug fixes, and out of scope until WS-08 is
  scheduled.
- **No before/after performance measurement** for the `shape_as_needed()`
  call added to `get_word_suggestions_json()` — it's documented as a
  no-op when nothing changed (same guarantee `render()` already relies
  on every frame), but this sandbox has no way to profile real
  browser CPU/frame timings beyond functional assertions.

## Next phase

Per `prompt/FUTURE_PLAN.md`'s backlog, remaining after this: Phase P5
(`P5-01` recent files, `P5-03` `.uns` view state, `P5-04` PDF export),
Phase P8 stretch items (`P8-01` syntax highlighting, `P8-02` CRDT
collaboration), `P3-01`/`P3-02`/`P3-04` (format-control visibility
toggle, orientation-persistence regression test, input-mode status
pill), and this plan's own deferred `WS-07`/`WS-08`. None of these
block each other.
