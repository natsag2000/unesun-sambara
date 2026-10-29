# Phase P10 — Double-click word translation (implementation log)

Date: 2026-10-28
Reference: `prompt/WORD_TRANSLATION_PLAN.md` (full design), `prompt/FUTURE_PLAN.md` Phase P10
Branch: `feature/double-click-translation` (from local `dev`)
Patch: `patch/word-translation.patch`

## Scope

| ID    | Effort | Status | Summary |
|-------|--------|--------|---------|
| WT-01 | S      | done   | Bichig→Cyrillic reverse index + `reverse_lookup` / `reverse_lookup_word` (NNBSP stem fallback) |
| WT-02 | S      | done   | `WasmEditor::select_word_at` — whole-word selection under a client position |
| WT-03 | S      | done   | `WasmEditor::translate_word_json` |
| WT-04 | M      | done   | `WordTranslationPopup`, markup, desktop double-click + mobile double-tap, dismissal wiring |
| WT-05 | S      | done   | `word_translation_enabled` setting (default on) + Editor-tab checkbox |
| WT-06 | S      | done   | Playwright coverage (12 tests) |

Scope confirmed with the user before coding: popup only for Bichig words
(Cyrillic words are selected, no popup); display-only popup (Esc / click-away
closes); settings toggle plus mobile double-tap support.

## Design decisions

- **Selection uses the project's own word boundaries, not cosmic-text's
  `Selection::Word`.** cosmic-text segments with UAX #29, which doesn't treat
  Mongolian format-control characters (FVS/MVS/NNBSP) as word-internal.
  `select_word_at` hit-tests with `Buffer::hit`, runs
  `word_boundary::current_word_bounds` + `classify_script`, then sets
  `Selection::Normal(start)` with the cursor at the end. Consequence: a
  trailing FVS stays inside the selection, and an inflected
  `stem<NNBSP>suffix` is selected as one word.
- **Stem fallback for inflected words.** Because NNBSP is word-internal, the
  selected word is often `stem + suffix`, which never matches a headword.
  `reverse_lookup_word` retries with the part before the first NNBSP and the
  popup labels the result "Cyrillic (for the word stem)". Nothing smarter
  (no suffix analysis).
- **Reverse index keys are format-control-stripped**, same normalization as
  the Mongolian suggestion index, so FVS variants typed differently from the
  dictionary still match. Rows with whitespace in the Bichig column are
  skipped; Cyrillic phrases are kept (a single Bichig word may translate to a
  phrase). Ranking: shorter Cyrillic, then alphabetical; deduped; capped at 6
  (`TRANSLATION_LIMIT`).
- **`select_word_at` clears `is_dragging`.** The double-click's second
  `mousedown` goes through `handle_mouse_down`, which arms drag-selection;
  otherwise one pixel of mouse jitter would run `Action::Drag` and shrink the
  selection to a partial word.
- **Plain DOM tooltip, not the WS-09 canvas pipeline.** The content is
  Cyrillic, so there is no cross-browser shaping concern. Text is set via
  `textContent`.
- **Dictionary loading reuses `WordSuggestionsPopup.ensureDictionaryLoaded()`**
  (JS `fetch` + synchronous `load_dictionary_text`), so there is still a
  single fetch/instance and the async-borrow panic documented in the P9 log
  can't recur. A per-call token discards a load that finishes after the user
  has already dismissed/clicked elsewhere.
- **Double-click detection on desktop is `mousedown` with `e.detail === 2`**
  (runs after the normal single-click handling, so the cursor logic is
  unchanged). On touch, two single-finger `touchstart`s within 350 ms and
  30 px; emulated mouse events within 800 ms of a touch are ignored to avoid
  firing twice.
- **The document-level "mousedown outside" dismissal ignores
  `#editor-canvas`.** The canvas handler can open the popup during the same
  event (the await continuation runs between the canvas and document
  listeners); without the exclusion the document listener closed it
  immediately.
- **No `resize` dismissal.** Opening the mobile keyboard after a double-tap
  fires `resize`, which would have closed the popup instantly.
- **Ctrl/Cmd+C and bare modifier keys don't dismiss**, so the user can copy
  the freshly selected word.

## Files changed

- Modified: `src/dictionary/lookup.rs` (+144) — `reverse_map`, `reverse_lookup`,
  `reverse_lookup_word`, `TRANSLATION_LIMIT`; 7 new unit tests.
- Modified: `src/config/settings.rs` (+27) — `word_translation_enabled`
  (serde default `true`, so old saved settings / `.uns` files load); 1 new test.
- Modified: `src/lib.rs` (+140) — `select_word_at`, `translate_word_json`.
- Modified: `index.html` (+283/-2):
  - `#word-translation-popup`, `#sr-translation` markup; `#word-translation-enabled`
    checkbox (Editor tab) wired into the settings modal load/save.
  - `WordTranslationPopup` class; `wordTranslation` singleton.
  - `handleWordDoubleClick`, mouse (`detail === 2`) and touch double-tap detection
    in `handlePointerDown`.
  - Dismissal: canvas `keydown`, wheel, mobile `input`/`keydown`, document-level
    Escape and outside-mousedown, `TabManager.loadActiveIntoEditor`, Clear,
    version-load.
  - Test hooks `getTranslationState()` / `getSelectedText()`.
  - Cache-busting bumped to `?v=13` for `uns_editor.js` / `uns_editor_bg.wasm`.
- **New:** `tests/e2e/word-translation.spec.js` (11 tests),
  `tests/e2e/word-translation-mobile.spec.js` (1 test).
- **New:** `prompt/WORD_TRANSLATION_PLAN.md`, this log.
- Modified: `prompt/FUTURE_PLAN.md` — Phase P10 entry.

`dist/output.css` was not touched: the markup only uses Tailwind classes that
already exist in the built CSS, plus inline styles for padding/list reset.

## Verification

- `cargo test --lib` — **96 passed** (88 before this branch; 8 new here:
  7 in `dictionary::lookup`, 1 in `config::settings`).
- `cargo check --target wasm32-unknown-unknown` — clean, no warnings.
- `wasm-pack build --target web --release` — succeeds; `select_word_at` /
  `translate_word_json` present in `pkg/uns_editor.d.ts`.
- `npx playwright test` — **35/35 passed** (12 new + 23 pre-existing, re-run
  unchanged because this touches the shared canvas pointer/keydown handlers).
  New tests cover: whole-word selection + translation; selecting only the
  clicked word among neighbours; homographs shortest-first (`ᠣᠲᠠᠴᠢ` →
  оточ, асклепий); trailing FVS kept in the selection and still matching
  (`армен`); NNBSP stem fallback; not-found message; Cyrillic word selected
  without popup; Escape / typing / click-away dismissal; popup stays inside
  the editor area; horizontal orientation; settings toggle (selection still
  works, popup doesn't); mobile single tap vs. double-tap (Pixel 5 preset).
- Visual check via Playwright screenshots (vertical and horizontal): word
  highlighted, popup beside/below it with the Cyrillic candidates.
- Limitation, as in earlier phases: only headless Chromium is available here;
  real-device touch behaviour (double-tap timing, keyboard interaction) is
  emulated, not observed.

## Follow-ups / known limits

- Inflected forms resolve only through the "part before first NNBSP" fallback;
  no real suffix analysis, so stems altered by a suffix won't match.
- Homographs have no frequency ranking.
- Popup is read-only (copy/insert deliberately out of scope).
- Cyrillic→Bichig on double-click not implemented (would need canvas
  rendering, see WS-09's pattern).
- Triple-click (select line) not handled.
- `crates/uns-dioxus/assets/editor/index.html` is an untracked copy of the
  editor page used by the Dioxus spike; it was **not** updated and will need
  the same `index.html` changes when that spike is next synced.

## Merge note

Review on `feature/double-click-translation`, then merge into `dev`
(`git checkout dev && git merge --no-ff feature/double-click-translation`).
