# Double-click Word Translation — Implementation Plan

Status: **WT-01 through WT-06 implemented** on branch
`feature/double-click-translation` (see `DOC/IMPL/WORD_TRANSLATION_IMPL.md`
for the implementation log and `patch/word-translation.patch`). Standalone
plan, same relationship to `prompt/FUTURE_PLAN.md` as
`prompt/WORD_SUGGESTIONS_PLAN.md`: this file holds the design,
`FUTURE_PLAN.md` carries a short cross-reference (Phase P10).

## 1. Feature summary

Double-clicking (desktop) or double-tapping (mobile) a word in the editor
selects the **whole word**. If it is a Mongolian Bichig word, a small popup
next to it shows the **Cyrillic translation(s)** from the editor's existing
Cyrillic↔Bichig dictionary. Complements the word-suggestion popup
(Cyrillic→Bichig while typing) and the Transliteration modal (`Ctrl+T`) with
the opposite direction: Bichig→Cyrillic, on demand, for text already in the
document.

## 2. Scope decisions (confirmed with the user)

1. **Bichig words only get a popup.** Double-clicking a Cyrillic (or other)
   word still selects it, but shows nothing.
2. **Display only.** Esc / clicking away / typing closes it. No copy or
   insert action in this pass.
3. **Settings toggle + mobile double-tap.** `word_translation_enabled`
   (default `true`) in the Editor tab; double-tap supported on touch.
   The toggle gates only the popup — double-click word *selection* is
   unconditional.
4. Work happens on a feature branch (`feature/double-click-translation`,
   from local `dev`) so it can be reviewed and merged into `dev`.

## 3. Grounding facts

- `handle_mouse_down` only sends `Action::Click`; no double-click handling
  existed anywhere (Rust or JS).
- cosmic-text has `Action::DoubleClick`/`Selection::Word`, but it uses
  UAX #29 word segmentation, which knows nothing about Mongolian
  format-control characters. The project already has
  `word_boundary::current_word_bounds` (P9) that treats FVS/MVS/NNBSP as
  word-internal — selection is built on that instead.
- `word_boundary` treats NNBSP (U+202F) as word-internal, so
  `stem<NNBSP>suffix` is one "word". Dictionary headwords are base forms, so
  a whole-word lookup of an inflected form misses → stem fallback (§4).
- `Dictionary` only had the forward map and prefix indices; Bichig→Cyrillic
  needs a new reverse index.
- The dictionary is already lazily loaded by
  `WordSuggestionsPopup.ensureDictionaryLoaded()` (synchronous
  `load_dictionary_text`; the async `translit_load_dictionary` has the
  wasm-bindgen re-entrancy hazard documented in the P9 log). Reused as-is.
- The result is Cyrillic: plain DOM text is fine (no shaping risk), so the
  WS-09 cosmic-text canvas pipeline is not needed.

## 4. Design

**Reverse index** (`src/dictionary/lookup.rs`): `reverse_map:
HashMap<normalized Bichig, Vec<Cyrillic>>` built in `build_indices()`.
Keys are stripped of format-control characters (existing
`normalize_mongolian`), so FVS variants match. Rows whose Bichig side has
whitespace are skipped; Cyrillic phrases are kept. `reverse_lookup()`
ranks shorter Cyrillic first, then alphabetical, deduplicated, capped at 6.
`reverse_lookup_word()` adds the NNBSP stem fallback and reports whether the
stem was used.

**Selection** (`WasmEditor::select_word_at(client_x, client_y)`): converts
client coordinates with the same DPR/scroll/padding math as
`handle_mouse_down`, hit-tests, runs `current_word_bounds` +
`classify_script`, sets `Selection::Normal(start)` + cursor at end, clears
`is_dragging` (so jitter after the second mousedown can't shrink the
selection), flushes the undo group, dispatches `CursorMoved` /
`SelectionChanged`. Returns `{ selected, word, script, translationEnabled,
anchorX, anchorY, lineAdvance }` (anchor = selection end, canvas-pixel space,
like `get_word_suggestions_json`).

**Translation** (`WasmEditor::translate_word_json(word)`): returns
`{ loaded, candidates, usedStem }`.

**UI** (`index.html`): `WordTranslationPopup` class + `#word-translation-popup`
DOM tooltip + `#sr-translation` live region. Desktop detects the double
click via `mousedown` with `e.detail === 2`; mobile via two single-finger
`touchstart`s within 350 ms / 30 px (emulated mouse events after a touch are
ignored for 800 ms to avoid double-firing). Placement: below the selection
end in horizontal mode, one column over in vertical mode (so the selected
word stays visible), clamped to the editor area. An in-flight dictionary load
carries a token so a stale load can't pop the tooltip up after the user moved
on. States: candidates / "No Cyrillic translation found" / "Dictionary
unavailable"; label notes when only the stem matched.

**Dismissal**: Escape, any non-modifier key (Ctrl/Cmd+C keeps it open so the
selected word can be copied), canvas click/tap, wheel scroll, mousedown
outside the editor/popup, tab switch / Clear / version-load, mobile
`input`/`keydown`.

## 5. Work items

Stable IDs, same convention as the other plans.

- **WT-01** (S) `[x]` — Reverse index + `reverse_lookup`/`reverse_lookup_word`
  in `src/dictionary/lookup.rs`, unit tests.
- **WT-02** (S) `[x]` — `select_word_at` in `src/lib.rs`.
- **WT-03** (S) `[x]` — `translate_word_json` in `src/lib.rs`.
- **WT-04** (M) `[x]` — `WordTranslationPopup`, markup, desktop double-click
  and mobile double-tap wiring, dismissal sites, test hooks.
- **WT-05** (S) `[x]` — `word_translation_enabled` setting + Editor-tab
  checkbox + serde round-trip test.
- **WT-06** (S) `[x]` — Playwright: `tests/e2e/word-translation.spec.js`
  (11 tests) and `tests/e2e/word-translation-mobile.spec.js` (1 test).

## 6. Non-goals / known limits

- No Cyrillic→Bichig popup (declined in scoping; would need canvas rendering).
- No copy/insert action from the popup.
- No suffix-aware analysis beyond "retry with the part before the first
  NNBSP"; a stem with a *spelling-changing* suffix, or a headword that is
  itself missing from the dictionary, shows "No Cyrillic translation found".
- Homographs are ranked by Cyrillic length/alphabet only — there is no
  frequency data.
- Triple-click is not handled (still just a cursor placement).

## 7. How to resume

Read `DOC/IMPL/WORD_TRANSLATION_IMPL.md` (design decisions + follow-ups),
then `FUTURE_PLAN.md`'s per-phase workflow. Branch:
`feature/double-click-translation`.
