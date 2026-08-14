# Word Suggestion Popup — Implementation Plan

Status: **WS-01 through WS-06, plus WS-09, implemented and shipped;
WS-08 partially done** (desktop MVP + canvas rendering + mobile
trigger wiring - see `DOC/IMPL/WORD_SUGGESTIONS_IMPL.md` for the
implementation log, `patch/word-suggestions.patch` /
`patch/word-suggestions-arrow-nav-fix.patch` /
`patch/word-suggestions-ws09.patch` /
`patch/word-suggestions-mobile-trigger.patch` for the patches). `WS-09`
(added after the original plan - see below) replaced the popup's
DOM/CSS text rendering with the same `cosmic-text` canvas pipeline the
main document uses, for guaranteed cross-browser Mongolian glyph
consistency. `WS-08` (mobile) is now partially done - the popup didn't
trigger at all on real mobile devices, now fixed; popup *positioning*
on a small/keyboard-covered viewport is still not addressed. `WS-07`
(phrase-aware suggestions) remains fully deferred, exactly as scoped in
§12 below. This is a standalone plan for a single
feature the user identified as the most important remaining one for
this editor, kept separate from `prompt/FUTURE_PLAN.md`'s backlog given
its size and priority (same reasoning `prompt/PLUGIN_API.md` is its own
document rather than a section of the backlog). `FUTURE_PLAN.md` has a
short cross-reference entry pointing here; this file is where the real
design detail lives.

## 1. Feature summary

While the user types — in either script the editor already accepts
character input in (Cyrillic, or Mongolian Bichig produced live via the
Latin→Mongolian input mode) — show a small popup anchored next to the
text cursor on the canvas, listing word completions/corrections drawn
from the editor's existing Cyrillic↔Mongolian dictionary. The user can
accept a suggestion with the keyboard (arrows to move, Tab/Enter to
accept, Escape to dismiss) or by clicking it with the mouse, at which
point it replaces the word currently being typed.

This is functionally "bring the existing Transliteration modal's lookup
inline, live, as you type, without opening a dialog" — confirmed with
the user as the target design: a **new, additional** feature that
reuses the same dictionary the modal (`Ctrl+T`) already uses; the modal
itself is not touched or replaced by this work.

## 2. Scope decisions confirmed with the user before this plan was written

1. **Trigger sources: both.** The popup must trigger after letters are
   typed regardless of which "mode" produced them — raw Cyrillic typed
   directly into the buffer, *or* Mongolian Bichig produced character-
   by-character through the existing Latin→Mongolian input mode
   (`config/latin.csv`). See §4 for how this is unified into one
   algorithm rather than two separate code paths.
2. **Accept via keyboard *and* mouse.** Arrow keys + Tab/Enter + Escape
   are the primary interaction (doesn't interrupt typing flow); a click
   on a suggestion is also wired up, since the popup is a normal DOM
   overlay (not canvas-drawn), so this costs little extra.
3. **Additive, not a replacement for the Transliteration modal.** Both
   features coexist; deprecating the modal in favor of this is
   explicitly out of scope for now (could be revisited in a later,
   separate decision once this ships and is used for a while).

## 3. Grounding facts (what's already there to build on)

Gathered by reading the actual code before writing this plan, not
assumed:

- **The dictionary is big and already exists.** `dist/dictionary/01_kv.tsv`
  is 7.8 MB, **101,877 lines** (101,869 data rows + 8 `#` header/metadata
  lines), two tab-separated columns: Cyrillic (`mn`) → traditional
  Mongolian Bichig (`mb`). Shipped in git as a 976 KB `.xz` and
  decompressed by `scripts/prepare-dictionary.js` during the build (`npm
  run prepare:dict`, wired into `build.sh`/`build.bat`).
- **Most entries are multi-word phrases, not single words.** Only
  **~25,019 of ~101,869** Cyrillic keys (~24.6%) contain no internal
  whitespace. Longest key is 462 characters. This matters a lot for
  §5's indexing design — phrase entries are much lower quality as
  "word completion" candidates and need to be filtered out of the core
  suggestion set, not just used as-is.
- **`src/dictionary/lookup.rs`'s `Dictionary` only does exact-match
  lookup** (`HashMap<String, Vec<String>>`, case-insensitive/trimmed
  key). No prefix search, no iteration exposed, no frequency/ranking
  data. This needs new capability, not just new call sites.
- **The dictionary is currently loaded lazily**, only when the
  Transliteration modal is first opened (`TranslitModal`'s
  `translit_load_dictionary` call in `index.html`). For this feature to
  work "no matter what," it needs to be available much earlier —
  see §8's loading strategy.
- **Cursor pixel position is available for free.** `cosmic_text`'s
  `Edit::cursor_position(&self) -> Option<(i32, i32)>` (already in the
  vendored `cosmic-text` crate, `src/edit/editor.rs:1053`) returns the
  cursor's top-left corner in the *same raw buffer-pixel space* every
  glyph coordinate in `WasmEditor::render()` starts in — i.e. it
  composes directly with the exact transform `render()` already applies
  everywhere else in `src/lib.rs`:
  `canvas_pixel = buffer_pixel − scroll_offset + content_padding()`.
  This method is not currently called anywhere in `src/lib.rs` — new
  code, but no new math to invent, just reusing the established
  formula (also used for the P2-03 find/replace highlight overlay and
  the P2-02 gutter labels).
- **No word-boundary-detection utility exists to reuse.** The only
  "word" concept in the codebase is `find_replace.rs`'s `whole_word`
  option, which just wraps a query in regex's own `\b`/`\b` and lets the
  `regex` crate's Unicode word-boundary tables do the work — it doesn't
  scan characters itself and only ever operates on one already-known
  query string, not "what word is the cursor currently inside." New
  word-boundary-scanning logic is needed (§4). The three character
  predicates in `src/editor_core/format_control.rs`
  (`is_format_control`, `is_format_after_visible`,
  `is_format_before_visible`) are directly reusable — confirmed that
  Mongolian format-control characters (U+180B/C/D/E/F, U+202F) are
  *never* word separators, only attachment modifiers glued to one
  neighboring visible character, so a word-boundary scanner must treat
  them as "part of the word," not a boundary.

## 4. Word-boundary detection algorithm

New pure function(s), same style/testing approach as
`src/editor_core/format_control.rs` (P7-03) — a dedicated module,
`src/editor_core/word_boundary.rs`, with no `cosmic_text` or
`wasm_bindgen` dependency so it's unit-testable in isolation:

```rust
/// The word (if any) the cursor is currently positioned inside or at
/// the end of, as a half-open `[start, end)` character-index range
/// within `chars` (the current line's characters — matches never cross
/// a newline, same restriction `find_replace.rs` already has).
pub fn current_word_bounds(chars: &[char], cursor_index: usize) -> Option<(usize, usize)>;
```

Rules (derived from §3's facts, not invented from scratch):
- A character is "part of a word" if it is alphabetic (Cyrillic *or*
  Mongolian Bichig block, `char::is_alphabetic()` already covers both)
  **or** a Mongolian format-control character (attach to the
  neighboring word, per §3's confirmation — skipped over, not treated
  as a boundary, when scanning either direction).
- Anything else (whitespace, ASCII/Mongolian punctuation, digits —
  digits deliberately excluded; see open question in §11) is a
  boundary.
- Scan backward from `cursor_index` while the preceding character is
  "part of a word"; scan forward while the following character is
  "part of a word." Returns `None` if the cursor isn't adjacent to any
  word characters at all (e.g. cursor sits between two spaces).
- Deliberately does **not** attempt to classify *which* script the word
  is in — that's a separate, later step (§5) once the bounds are known,
  keeping this function's contract simple and independently testable.

This function is called after every relevant keystroke (§7) with the
current line's characters (same `current_line_chars_and_cursor()`
helper `handle_backspace`/`handle_delete` already use in `src/lib.rs`,
which can be reused as-is) and the cursor's character-index position.

## 5. Dictionary indexing for live prefix search

New capability on `Dictionary` (`src/dictionary/lookup.rs`), additive —
existing `lookup()` (exact match, used by the Transliteration modal)
is untouched.

- **Two sorted indices, built once when the dictionary loads:**
  - `cyrillic_index: Vec<(String, usize)>` — `(lowercased Cyrillic key,
    index into a shared entries table)`, sorted lexicographically.
  - `mongolian_index: Vec<(String, usize)>` — same shape, but keyed by
    the traditional Bichig value with Mongolian format-control
    characters *stripped* for indexing/sorting purposes only (comparing
    strings containing invisible modifiers would sort inconsistently
    with user expectation; the original, unstripped text is still what
    gets inserted on accept).
  - Both indices are built **only from single-word entries** (no
    internal whitespace in either column) per §3's data-quality
    finding — phrase entries are excluded from the live-suggestion
    indices entirely in this first pass (see §11 for the deferred
    "phrase suggestions" idea).
  - A prefix query is a binary search for the range
    `[prefix, prefix_with_last_char_incremented)` — standard sorted-Vec
    prefix-range technique, `O(log n)` to find the range boundaries,
    `O(k)` to read out `k` matches. No trie: simpler to implement and
    verify, and a sorted `Vec` of ~25K short strings is small enough
    (a trie's usual advantage — avoiding re-comparing shared prefixes —
    doesn't matter much at this size).
- **Which index gets queried is decided by the word's script**, not by
  "which input mode was active" (per the confirmed requirement that the
  trigger is input-source-agnostic): the first alphabetic character in
  the word bounds from §4 is checked against the Cyrillic Unicode block
  (U+0400–U+04FF) vs. the Mongolian block (U+1800–U+18AF); query the
  matching index. A word that's a mix of both (shouldn't normally
  happen) uses whichever script its *first* character belongs to.
- **Ranking** (no frequency data exists in the source TSV — see §11):
  1. Exact case-insensitive match first, if present.
  2. Shorter completions before longer ones (a completion two
     characters longer than what's typed is a "closer" suggestion than
     one twenty characters longer).
  3. Alphabetical as a final tie-break.
  4. Capped at a fixed limit (proposed: 8) before returning to JS.

## 6. New Rust surface

- **`src/dictionary/lookup.rs`**: `Dictionary::build_indices(&mut self)`
  (called once after `from_tsv`/`load`), `Dictionary::suggest(&self,
  prefix: &str, script: Script) -> Vec<&str>` (`Script` a small `enum
  { Cyrillic, Mongolian }`).
- **`src/editor_core/word_boundary.rs`** (new module): the function
  from §4, plus a small `Script` classification helper, both `pub` and
  unit-tested independently of any WASM/cosmic-text machinery.
- **A new plugin, `src/plugins/word_suggestions.rs`**, following the
  established "plugin data slot" pattern (`FindReplaceState` on
  `EditorState` is the direct precedent): `SuggestionState { query:
  String, word_start: Option<Cursor>, word_end: Option<Cursor>,
  suggestions: Vec<String>, selected: usize }` lives on `EditorState`
  because command handlers are bare `fn` pointers with no access to a
  plugin instance (same reasoning documented in `PLUGIN_API.md` and
  `find_replace.rs`'s module doc comment).
- **New `WasmEditor` methods in `src/lib.rs`** (not routed through the
  generic `run_command` bridge, unlike most P1 plugin commands — this
  needs `&self.context`/`self.width`/`self.height`/`self.scroll_offset`
  for the pixel-position math, which command handlers can't reach since
  they only get `&mut EditorState`, not the whole `WasmEditor`):
  - `get_word_suggestions_json(&mut self) -> Result<String, JsValue>` —
    recomputes word bounds via §4, queries the dictionary via §5,
    computes the cursor's anchor position via §3's transform, and
    returns
    `{ hasSuggestions, suggestions: string[], anchorX: number, anchorY: number }`
    (anchor already in canvas-pixel space; JS divides by
    `devicePixelRatio` and adds the canvas's `getBoundingClientRect()`
    offset to get CSS position — the exact inverse of the DPR scaling
    `handle_mouse_down` et al. already do, just going the other
    direction).
  - `accept_word_suggestion(&mut self, text: &str) -> Result<(), JsValue>`
    — re-resolves current word bounds (don't trust bounds computed on a
    previous keystroke — the buffer may have changed), then
    `delete_range` + `insert_at` (same primitives `find_replace.rs`'s
    single-replace already uses), wrapped in
    `begin_history_group(EditKind::Paste, now)` /
    `flush_history_group()` so accepting a suggestion is one atomic
    undo step, same convention as every other programmatic multi-char
    edit in this codebase. Sets `self.dirty = true` (P7-01 integration)
    and dispatches `EditorEvent::TextChanged`.
  - `dismiss_word_suggestions(&mut self)` — clears `SuggestionState`
    (called on Escape, on cursor-moving keys/clicks, and after
    `accept_word_suggestion`).
  - `load_suggestion_dictionary(&mut self, url: String) -> Result<(),
    JsValue>` (async) — separate from `translit_load_dictionary` in
    name only if the two end up needing different lifecycles (see
    §8); may turn out to be the same call site reused, decide during
    implementation rather than guessing here.

## 7. Trigger points (where JS calls into the new Rust methods)

Every location that currently mutates buffer text needs a follow-up
call to `get_word_suggestions_json()` (to refresh/show/hide the popup)
or `dismiss_word_suggestions()` (cursor moved away from the word being
edited), mirroring how P7-01's `dirty` flag had to be threaded through
every mutating call site — same audit discipline, applied to a
different concern:

| Site (in `index.html`) | Action |
|---|---|
| Canvas `keydown` → `editor.handle_key_down(e)` returns with a text-changing effect | refresh suggestions |
| Canvas `keydown` → Latin-mapped character branch (`editor.insert_text(convertedChar)`) | refresh suggestions |
| Mobile hidden-input `input` event → Latin-mapped conversion branch | refresh suggestions |
| Arrow keys / click / touch that move the cursor without changing text | dismiss (word context changed) |
| `TabManager.switchTo()` / tab close / document clear / file open | dismiss (whole document changed) |
| Popup's own Escape handler | dismiss |
| Popup's own Tab/Enter/click accept handler | `accept_word_suggestion`, then refresh (the new word might itself want different suggestions — unlikely but cheap to check) or dismiss if the accepted text ends the word (trailing space typically follows an accept in normal typing flow) |

## 8. Dictionary loading strategy

Open decision, not resolved by this plan alone (flagged for the "confirm
scope" step when implementation actually starts — see §12):

- **Option A — eager, backgrounded.** Kick off the fetch+parse+index
  build right after the editor becomes ready (non-blocking — the app
  doesn't wait on it), so by the time a user has typed enough to form a
  word, the dictionary is very likely already loaded. Con: downloads
  7.8 MB for every visitor even if they never use suggestions or
  transliteration.
  - Also means `Ctrl+T`'s `TranslitModal.translit_load_dictionary` call
    becomes a no-op fast-path on subsequent opens (already true today
    for repeat opens — just changes when the *first* load happens).
- **Option B — lazy, first-trigger.** Only start loading on the first
  keystroke that would need suggestions (first Cyrillic or Mongolian
  word-character typed). Con: the very first suggestion opportunity in
  a session is delayed by a multi-hundred-millisecond parse+index (network
  fetch could be near-instant from HTTP cache after the first visit,
  but the parse+index step still costs CPU time every full page load
  regardless of caching).
- Recommendation to revisit at implementation time: **Option B**,
  matching the existing lazy-load convention (`AutoSave`, `TabManager`,
  `LatinMappingManager`, and the Translit modal's own dictionary load are
  all "do the expensive thing on first real need," not eagerly at
  startup) — but flagged as a real trade-off worth a deliberate decision,
  not an assumption.

## 9. UI design (`index.html`)

- New `#word-suggestions-popup` element, absolutely positioned inside
  `#canvas-container` (same containing block find-panel/dirty-dot etc.
  already use), `display: none` until there's something to show,
  position set via inline `style.left`/`style.top` computed from
  `get_word_suggestions_json()`'s `anchorX`/`anchorY` on every refresh —
  *not* a fixed-position bar like `#find-panel`, since it needs to
  track the cursor around the document as the user types and scrolls.
- New `WordSuggestionsPopup` class (module scope singleton, same shape
  as `TabManager`/`KeybindingManager`/`LatinMappingManager`):
  `refresh()` (calls the WASM method, shows/hides/repositions/re-renders
  the list), `next()`/`prev()` (move `selected` index, wraps), `accept()`
  (calls `accept_word_suggestion` with the currently-selected item's
  text, focuses back on the canvas), `dismiss()`.
- **Keyboard precedence**: when the popup is visible, the canvas
  `keydown` listener must intercept `ArrowUp`/`ArrowDown` (navigate the
  list, not move the text cursor), `Tab`/`Enter` (accept), and `Escape`
  (dismiss) *before* they reach `editor.handle_key_down(e)` — same
  pattern already used for `Ctrl+C`/`Ctrl+X`/`Ctrl+V` in that same
  listener (early return with `e.preventDefault()`), not a new
  mechanism. All other keys fall through to normal handling as today,
  which then triggers a suggestions refresh per §7.
- **Mouse**: each rendered suggestion `<li>` gets a `click` listener
  calling `accept()` with that item's text — plain DOM event handling,
  no canvas hit-testing needed since this is an HTML overlay, not
  canvas-drawn content.
- Minimum trigger length: don't show the popup until the current word
  is at least 2 characters (avoids a distracting popup after every
  single keystroke at the start of a word) — configurable constant, not
  a user-facing setting in the first pass.

## 10. Testing strategy

- **Rust unit tests** (`src/editor_core/word_boundary.rs`,
  `src/dictionary/lookup.rs`'s new indexing code): word-boundary
  detection across plain text, format-control-attached characters at
  the start/middle/end of a word, mixed-script edge cases, empty/
  boundary-only lines; prefix search correctness and ranking order
  against a small fixture dictionary (not the real 100K-entry file —
  same testing philosophy already used throughout this codebase, e.g.
  `find_replace.rs`'s tests build tiny in-memory buffers rather than
  loading real data).
- **Playwright** (`tests/e2e/word-suggestions.spec.js`, following the
  now-established pattern from `tests/e2e/latin-mapping.spec.js` /
  `keybindings.spec.js`): type a known Cyrillic prefix, confirm the
  popup appears with expected candidates, accept one via keyboard,
  confirm the buffer text updated correctly and the popup closed;
  repeat via mouse click; confirm Escape dismisses without changing the
  buffer; confirm arrow keys navigate the list instead of moving the
  text cursor while the popup is open. This will need real dictionary
  data available to the test server (already the case — `dist/dictionary/`
  is part of the served static files once `npm run prepare:dict` has
  run, which `playwright.config.js`'s `webServer` doesn't currently
  invoke automatically — needs adding to the CI workflow / local
  pre-test setup, flagged as a concrete task in §12's breakdown, not an
  afterthought).

## 11. Open questions / risks (resolutions added after implementation — see `DOC/IMPL/WORD_SUGGESTIONS_IMPL.md`)

1. **Phrase entries are 75% of the dictionary and excluded from v1.**
   — **Resolved: excluded, `WS-07` scoped as an explicit deferred
   follow-up phase**, not silently dropped. Still deferred as of this
   writing.
2. **No frequency/popularity data exists** for ranking beyond
   length+alphabetical. — **Not resolved (accepted as a known limit).**
   Shipped with length+alphabetical ranking; no corpus/frequency data
   was introduced (would be a real content dependency, distinct from a
   code change, and wasn't requested).
3. **Should digits count as "word characters"?** — **Resolved: no,
   digits are word boundaries**, confirmed with the user before coding
   (see the top of this file's former "confirmed scope" list, now
   folded into the impl log). `word_boundary.rs`'s `classify_script`
   treats a digit as ending the current word, same as whitespace.
4. **Mobile/touch and the `mobile-input` indirection layer.** —
   **Deferred as `WS-08`, unchanged.** The mobile hidden-input's
   `keydown` listener (Backspace/Enter) doesn't even call `refresh()`
   yet - a concrete gap to close whenever `WS-08` is scheduled.
5. **Interaction with Find & Replace / Command Palette.** — **Not
   explicitly addressed - known gap.** No code was added to dismiss the
   suggestion popup when Find & Replace or the Command Palette opens
   while it's showing; conversely, opening either while suggestions are
   up wasn't tested. Worth a deliberate "only one overlay owns keyboard
   focus" pass before `WS-07`/`WS-08`, not discovered as a bug in
   testing so far but not proven safe either.
6. **Per-tab scoping (P8-03 interaction).** — **Resolved.**
   `TabManager.loadActiveIntoEditor()` is the single centralized point
   that dismisses word suggestions on every tab-switching action
   (switchTo/closeTab/createTab all funnel through it), rather than
   scattering `dismiss()` calls across each call site individually.
7. **Should there be a settings toggle to disable this entirely?** —
   **Resolved: yes.** `word_suggestions_enabled` (default `true`),
   Editor tab in Settings - `WS-06`.

## 12. Proposed phased breakdown

Stable IDs (`WS-<n>`), for resuming after context loss — same
convention `FUTURE_PLAN.md` uses. Ordered by dependency; effort sizing
uses the same S/M/L/XL scale as `FUTURE_PLAN.md`.

- **WS-01** (S) `[x]` — `src/editor_core/word_boundary.rs`: word-bounds
  scanning + script classification, pure functions, unit tests.
- **WS-02** (M) `[x]` — `Dictionary` indexing: single-word filtering, two
  sorted prefix indices, `suggest()`, ranking, unit tests against a
  fixture dictionary.
- **WS-03** (M) `[x]` — `src/editor_core/suggestions.rs` (a plain struct,
  not a `Plugin` — see the impl log's design decisions) +
  `EditorState.suggestions` field; new `WasmEditor` methods from §6
  (`get_word_suggestions_json`, `accept_word_suggestion`,
  `dismiss_word_suggestions`, plus a synchronous `load_dictionary_text`
  not originally in this plan - see §8/impl log); dictionary loading
  wired per §8's decision (Option B, lazy).
- **WS-04** (M) `[x]` — `WordSuggestionsPopup` JS class + `#word-suggestions-popup`
  markup; wiring into the trigger table (§7); keyboard precedence
  changes in the canvas `keydown` listener. Extended beyond this plan's
  original text during implementation: orientation-aware rendering and
  positioning, motion-vs-typing refresh/dismiss distinction, and a
  numbered multi-column layout for vertical mode - see
  `DOC/IMPL/WORD_SUGGESTIONS_IMPL.md` for why each of these wasn't
  anticipated here.
- **WS-05** (S) `[x]` — Playwright tests per §10 (`tests/e2e/word-suggestions.spec.js`,
  7 tests). No CI/local test-server dictionary-availability fix was
  needed - the real `dist/dictionary/01_kv.tsv` was already present in
  the checkout.
- **WS-06** (S) `[x]` — Settings toggle (`word_suggestions_enabled`,
  Editor tab, confirmed always-on-by-default rather than gated behind
  §11.7's original open question, which was resolved directly with the
  user before coding started - see §2/the "confirmed scope" list this
  file's header used to carry).
- **WS-07** (M, deferred/follow-up) — Phrase-aware suggestions (§11.1).
  Still deferred.
- **WS-08** (M) `[~]` — **Partially done.** Trigger wiring through the
  `mobile-input` path (§11.4) fixed: the popup didn't appear on mobile
  *at all* (reported by the user testing on a real device) because
  virtual keyboards drive input through `#mobile-input`'s `input`
  event, which never called `refresh()`; the mobile `keydown` handler
  (Backspace/Enter, plus now the same popup-navigation keys the
  desktop canvas listener intercepts) and the popup canvas's
  touch-tap-to-accept support were fixed/added too. **Still not done:**
  popup *positioning* on a small screen with an on-screen keyboard
  covering much of the viewport (§11.4's original stated concern) -
  see `DOC/IMPL/WORD_SUGGESTIONS_IMPL.md`'s WS-08 section.
- **WS-09** (L) `[x]` — Canvas-rendered popup: replaced the DOM/CSS
  `writing-mode: vertical-lr; text-orientation: mixed` text rendering
  (a browser-native-text-shaping dependency, inconsistent across
  browsers for Mongolian) with the same `cosmic-text`+`harfrust`+`swash`
  pipeline the main document canvas uses, mirroring the existing
  `TranslitRenderer` pattern. Added after the original WS-01..WS-08
  breakdown, at the user's request once they noticed the DOM-based
  vertical-mode rendering's cross-browser risk. New pure
  `suggestion_popup_layout.rs` module (7 unit tests); new
  `measure_suggestions_popup()`/`render_suggestions_popup()` WASM
  methods; `#word-suggestions-popup`'s `<ul>`/`<li>` markup replaced
  with a `<canvas>`; JS-side hit-testing for click/hover against
  Rust-reported item bounds; `#sr-suggestions` accessibility mirror
  added (canvas has no semantic DOM content of its own). Also
  incidentally fixed the popup's text being smaller than the editor's
  real font size, since sizing now comes directly from
  `settings.fonts.font_size` in Rust rather than a fixed JS/CSS
  constant. See `DOC/IMPL/WORD_SUGGESTIONS_IMPL.md`'s WS-09 section for
  the full design/bug log.

## 13. Non-goals (explicitly out of scope for this plan)

- Replacing or removing the Transliteration modal (confirmed with the
  user, §2.3).
- Spell-checking / grammar suggestions unrelated to the existing
  dictionary (this is word *completion/correction from a known
  dictionary*, not a general spell-checker).
- Any new content acquisition (frequency data, a curated word list
  beyond what's already in `dist/dictionary/01_kv.tsv`) — everything in
  this plan is built from data already in the repository.

## 14. How to resume this plan after context loss

Same discipline as `FUTURE_PLAN.md`'s own "Resuming after a context
loss" section: this file *is* the resumable state for this feature.
Before writing code, follow `FUTURE_PLAN.md`'s "Per-phase workflow"
(§27 of that file) exactly as if `WS-01..WS-09` were phase items in the
main backlog — confirm scope (especially §8's loading strategy and
§11's seven open questions) with the user first, track subtasks with
the todo tool, verify locally, write an impl log at
`DOC/IMPL/WORD_SUGGESTIONS_IMPL.md` (or split per sub-batch, following
how multi-item batches were logged for the P4/P6 sessions), produce a
patch, and only then update `FUTURE_PLAN.md`'s cross-reference entry to
point at the finished impl log(s).
