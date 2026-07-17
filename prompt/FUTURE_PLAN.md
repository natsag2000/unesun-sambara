# UNS Editor — Future Features Backlog

This document supersedes the "Future Extensions (Deferred)" list at the end of `IMPLEMENTATION_SUMMARY.md`. It captures the next phase of work after the February 2026 refactor and reflects the state of the codebase as analyzed in this session: settings system, Mongolian format-control handling, `.txt`/`.uns` file I/O, PNG export, Latin-to-Mongolian input mode, touch gestures, and the Tailwind-based settings modal are all in place.

## How to use this plan

Each item has a stable ID of the form `P<phase>-<number>` (for example `P2-03`). IDs do not change when items are reordered or reworded; if the task is broken up, child items get suffixes (`P2-03a`, `P2-03b`). This lets you resume work after a context loss by looking up an ID.

Every item carries a process mark:

- `[ ]` pending — not started
- `[~]` in progress — someone is actively working on it
- `[x]` done — merged and verified
- `[!]` blocked — waiting on something; the description should say on what

Effort sizing:

- **S** — half a day or less
- **M** — one to two days
- **L** — three to five days
- **XL** — more than five days

Within a phase, items are roughly ordered by dependency: earlier items may be prerequisites for later ones. Phases themselves are ordered by recommended execution order, but that ordering is a suggestion — see the "Suggested order of execution" section at the end for a concrete path.

---

## Phase P0 — Foundation cleanup

Prerequisites that unblock cleaner feature work. These are not user-visible features; they are debts already present in the current code.

- **P0-01** `[ ]` **S** — Reconcile default settings between Rust and JavaScript
  - `AppearanceSettings::default()` in `src/config/settings.rs` uses `#73f73f` for text and `#0c0c0c` for background, and `FontSettings::default()` uses font size 73 / line height 93. The JS `reset()` in `index.html` uses `#c8c8c8`, `#2b2b2b`, 24, and 30. Pick a single source of truth (Rust) and expose a `reset_to_defaults()` WASM method that returns the JSON so JS never duplicates the values.

- **P0-02** `[ ]` **S** — Stop overwriting orientation when saving settings from the UI
  - `index.html` hardcodes `orientation: "vertical"` in the payload passed to `set_settings_json()`. This clobbers the user's horizontal toggle every time they touch the settings modal. Either read the current orientation from `get_settings_json()` before writing, or change the Rust side to merge partial settings instead of replacing them.

- **P0-03** `[ ]` **S** — Decide the fate of selection / gutter / line-number colors
  - These fields exist in `AppearanceSettings` but are never editable. The JS `save()` always re-sends hardcoded values. Either expose them in an "Advanced" subsection of the Color tab, or have `save()` merge on top of the existing JSON so unedited fields are preserved.

- **P0-04** `[ ]` **S** — Remove dead code and allow-dead warnings
  - `color_to_hex` and `hex_to_color` in `src/config/settings.rs` are marked `#[allow(dead_code)]`. Either fold them into the `color_serde` module (which currently inlines the same logic) or delete them.

---

## Phase P1 — Plugin / extension architecture

Goal: decouple features so new ones can register commands, keybindings, rendering decorations, and menu entries without editing `src/lib.rs`. This phase pays off starting in P2.

- **P1-01** `[ ]` **M** — Event bus in Rust
  - New module `src/editor_core/events.rs` defining `enum EditorEvent { TextChanged, CursorMoved, SelectionChanged, SettingsChanged, KeyPressed(KeyInfo), DocumentLoaded, DocumentSaved }`. Dispatch from `EditorState` and from `WasmEditor::handle_key_down` / `handle_mouse_down`. Subscribers are trait objects stored in `EditorState`.

- **P1-02** `[ ]` **M** — Plugin trait and registry
  - `src/editor_core/plugin.rs` exposing `trait Plugin { fn id() -> &'static str; fn on_event(&mut self, state: &mut EditorState, evt: &EditorEvent); fn commands(&self) -> &[Command]; }` and a `PluginRegistry` held by `EditorState`. Register built-in plugins in `EditorState::new`.

- **P1-03** `[ ]` **M** — Command palette (Rust + JS)
  - Keybinding `Ctrl+Shift+P`. New WASM methods `list_commands() -> JsValue` and `run_command(id: &str, args_json: &str)`. JS shows a Tailwind-styled modal with fuzzy search over command titles and ids.

- **P1-04** `[ ]` **S** — Plugin API documentation
  - Write `prompt/PLUGIN_API.md` with: event catalog, command/keybinding registration example, and a minimal hello-world plugin.

---

## Phase P2 — Core editing features

The highest-impact user-visible additions.

- **P2-01** `[ ]` **M** — Undo / Redo
  - History manager on `EditorState` that snapshots on each logical action; typing is debounced into single history entries. Expose `undo()` and `redo()` WASM methods. Keybindings `Ctrl+Z` and `Ctrl+Shift+Z`. The format-control-aware backspace and delete must produce a single undo step per user keystroke, not one per synthesized action.

- **P2-02** `[ ]` **M** — Line numbers and gutter
  - Re-introduce `show_line_numbers: bool` on `EditorBehaviorSettings`. Render a gutter strip in `render()` using `appearance.gutter_background` and `appearance.line_number_color`. In vertical mode the gutter becomes a top strip with row indices. Add the toggle to the Font tab (or a new Editor tab) in the settings modal.

- **P2-03** `[ ]` **L** — Find and Replace
  - New plugin `src/plugins/find_replace.rs`. Slide-down Tailwind panel at the top of the canvas area. Options: case sensitivity, whole word, regex, find next/prev, replace, replace all. Matches are drawn as an overlay of filled rectangles on top of the cosmic-text output. Keybindings `Ctrl+F`, `Ctrl+H`, `F3`, `Shift+F3`.

- **P2-04** `[ ]` **S** — Word-wrap toggle
  - Setting plus a status-bar button. `buffer.set_size` already drives this; the work is wiring the flag through.

- **P2-05** `[ ]` **M** — Auto-save draft to localStorage
  - Debounced (every 2-5 s after the last edit). Stored under a separate key containing `{ text, settings, cursor, timestamp }`. On startup, if a newer draft exists relative to any last-opened file, offer recovery through a non-blocking banner.

---

## Phase P3 — Mongolian-specific features

Extend the already-deep Mongolian support.

- **P3-01** `[ ]` **M** — Format-control character visibility toggle
  - Optional overlay that renders U+180B-U+180F and U+202F as faint glyph badges (debug mode). Implemented as a decoration plugin so the main buffer is untouched.

- **P3-02** `[ ]` **S** — Verify and document orientation persistence in `.uns`
  - `.uns` already embeds the full settings object, so orientation should survive a round trip. Add a regression test greeting file in each orientation and make sure `FileManager.loadUnsFile` applies it.

- **P3-03** `[ ]` **M** — Latin-to-Mongolian mapping editor
  - Today `config/latin.csv` is static. Add a new "Input" tab in the settings modal that shows the mapping as an editable table. Overrides are stored in localStorage and take precedence over the CSV.

- **P3-04** `[ ]` **S** — Persistent input-mode indicator in status bar
  - Currently the input mode (Latin-to-Mongolian on/off) is only flashed via `updateStatus`. Add a dedicated pill on the right side showing the mode and the keybinding hint.

---

## Phase P4 — Theming and appearance

- **P4-01** `[ ]` **M** — Theme presets
  - New `src/config/themes.rs` with built-in themes: Default Dark, Default Light, High Contrast, Solarized Dark, Mongolian Parchment. Theme dropdown at the top of the Color tab. Selecting a preset populates all color fields; subsequent edits create an implicit "Custom" theme.

- **P4-02** `[ ]` **S** — Import and export theme JSON
  - Buttons in the Color tab that round-trip theme objects as downloadable `.json` files (reuse the `downloadBlob` helper from `FileManager`).

- **P4-03** `[ ]` **S** — Detect `prefers-color-scheme` on first load
  - If no saved settings exist, pick Default Dark or Default Light based on `matchMedia('(prefers-color-scheme: dark)')`.

---

## Phase P5 — File and document enhancements

- **P5-01** `[ ]` **M** — Recent files list
  - Store the last 10 opened filenames (plus a short content hash) in localStorage. Surface as a submenu or recent-files section under the Open button.

- **P5-02** `[ ]` **M** — Dirty indicator and unsaved-changes warning
  - Track the hash of the last saved content. When the current content differs, show a dot next to the title and warn on `beforeunload`.

- **P5-03** `[ ]` **S** — `.uns` v1.1 — add view state
  - Additive schema change: embed `view.scrollOffset` and `view.selection` under the existing `content` section. Keep the major version at 1 so older files still open.

- **P5-04** `[ ]` **L** — Export as PDF
  - Either a browser-print-CSS path or a canvas-to-PDF library. Vertical Mongolian mode is the interesting correctness case.

---

## Phase P6 — Keybindings and accessibility

- **P6-01** `[ ]` **M** — Configurable keybindings
  - `src/config/keybindings.rs` mapping action ids to key combos. New "Keybindings" tab in the settings modal or edit via the command palette from P1-03. Persist to localStorage.

- **P6-02** `[ ]` **S** — Screen-reader announcements
  - Visually hidden `aria-live="polite"` region mirroring the status bar messages and cursor updates.

- **P6-03** `[ ]` **S** — Focus-indicator audit
  - Current toolbar buttons rely heavily on `:hover`. Add Tailwind `focus-visible` rings meeting WCAG contrast guidelines.

---

## Phase P7 — Performance and quality

- **P7-01** `[ ]` **M** — Dirty-flag-driven rendering
  - The animation loop currently re-renders every `requestAnimationFrame` tick. Refactor to render only when a dirty flag is set (text, cursor, selection, scroll, or settings changed). Cursor blink only repaints a small region.

- **P7-02** `[ ]` **S** — WASM size pass
  - `wasm-opt` is disabled in `Cargo.toml`. Enable it, add a `wasm-strip`/`--strip-all` step to `build.sh` and `build.bat`, and record before/after sizes.

- **P7-03** `[ ]` **M** — Rust unit tests
  - Extract the format-control decision logic from `handle_backspace` and `handle_delete` into pure functions and cover them with `#[cfg(test)]` tests. Settings serialization round-trip tests.

- **P7-04** `[ ]` **S** — Playwright smoke test
  - End-to-end: load page, type text, toggle orientation, open settings, save, reload, assert content and settings survived. Run headless in CI.

---

## Phase P8 — Stretch and deferred

Items inherited from the original `IMPLEMENTATION_SUMMARY.md` deferred list, plus a couple of new stretches. IDs are stable so they are not lost.

- **P8-01** `[ ]` **L** — Syntax highlighting plugin
  - Useful for code modes or TODO-style markers. Good demo of the plugin system from P1.

- **P8-02** `[ ]` **XL** — Collaborative editing via CRDT
  - Research spike first; likely needs a significant rework of the editing model.

- **P8-03** `[ ]` **M** — Multiple documents / tab strip

- **P8-04** `[ ]` **S** — Zoom shortcut
  - `Ctrl+=` and `Ctrl+-` bump font size up and down. Very cheap; noticeable UX gain.

---

## Suggested order of execution

1. **Phase P0** foundation cleanup (1-2 days total). Small, unblocks everything else.
2. **Phase P1** plugin architecture. P2-01 and P2-03 are cleaner as plugins.
3. **P2-01 undo**, **P2-02 line numbers**, **P2-03 find and replace**, **P2-05 auto-save**. Highest user impact.
4. **Phase P4 themes**, **P8-04 zoom**, **P5-02 dirty indicator**. UX polish.
5. **Phase P6 keybindings**, **P3-03 Latin-map editor**, **Phase P7** performance and tests. Power-user and quality.

## Open questions (to be resolved before execution)

These are the decisions that shape the first milestone. Record the resolution here when made.

1. **Plugin architecture before quick wins, or after?** Recommendation: P0 then P1 then features, for a cleaner long-term architecture.
2. **Scope of the first milestone.** Options considered: P0 only; P0 plus undo and line numbers; P0 plus themes; P0 plus find/replace; all of P0 and P2.
3. **Backlog adjustments.** Any items to add, drop, or regroup before the plan is locked in.
