# Phase P6 (+ P3-03, rest of P7) — Keybindings, accessibility, Latin-map editor, performance (implementation log)

Date: 2026-08-04
Reference: `prompt/FUTURE_PLAN.md`, Phase P6 (P6-01..P6-03), P3-03, P7-01, P7-02, remainder of P7-03
Patch: `patch/Phase_6.patch`

## Scope

This session covered the plan's "suggested order of execution" step 7 in
full - another cross-phase bundle (P6, P3, P7), all done together per the
user's explicit choice ("all 7, in one session").

| ID    | Effort | Status | Summary                                                                              |
|-------|--------|--------|----------------------------------------------------------------------------------------|
| P7-03 | M      | done   | Format-control backspace/delete logic extracted into pure, unit-tested functions      |
| P7-02 | S      | done   | `wasm-opt` enabled (19.4% size reduction); `wasm-strip` step added to build scripts    |
| P6-02 | S      | done   | Visually-hidden `aria-live` regions mirroring status bar + cursor position             |
| P6-03 | S      | done   | Global `:focus-visible` outline rule covering every button/input/canvas in the app     |
| P7-01 | M      | done   | Dirty-flag-driven rendering: `animate()` skips `render()`/`updateCursorPosition()` when idle |
| P3-03 | M      | done   | Latin-to-Mongolian mapping editor: editable table, localStorage overrides              |
| P6-01 | M      | done   | Configurable app-level keybindings: data-driven dispatch, recordable in a settings tab |

Done in roughly this order (cheapest/lowest-risk first, building toward
the two larger items), verified against the real headless Chromium set
up in the P7-04 session - every JS-facing item below has a Playwright
test that was actually run, not just written.

## Design decisions

- **P7-03** extracted `is_format_control`/`is_format_after_visible`/
  `is_format_before_visible` plus the two decision trees from
  `handle_backspace`/`handle_delete` into `src/editor_core/format_control.rs`
  as pure functions operating on `&[char]` + a character-index cursor
  position, returning a `DeletePlan { backspaces, deletes }`. Every
  branch in the original hand-written code happened to execute as "all
  backspaces, then all deletes" (never interleaved), so `DeletePlan`
  bakes that fixed order in rather than modeling a more general action
  sequence - verified by re-deriving each branch from the original code
  one at a time rather than rewriting the logic from scratch, to avoid
  introducing a behavior change while "cleaning up." 18 new unit tests,
  including two explicit symmetry tests (backspacing a
  `[visible][format_after]` group from the end agrees with
  forward-deleting the same group from the start).
- **P7-02**: `wasm-opt = ["-Oz"]` (previously `false`) - wasm-pack
  downloads and runs binaryen's `wasm-opt` itself, no system install
  needed. Measured 2,805,343 -> 2,261,185 bytes (19.4% smaller) on this
  build. Additionally tested `wasm-strip` (WABT) on the same binary by
  downloading a portable release into this sandbox (no root available)
  - it saved only 235 bytes on top of `wasm-opt`'s output, confirming
  `wasm-opt -Oz` already strips nearly everything `wasm-strip` would.
  The build-script step added for it degrades gracefully (skips with a
  message) when `wasm-strip` isn't installed, since its marginal value
  is this low and it's a separate, less commonly pre-installed tool than
  `wasm-opt` (which wasm-pack manages itself).
- **P6-02**: two separate visually-hidden (`sr-only`) live regions
  (`#sr-status`, `#sr-cursor`) rather than one shared region, so a
  screen reader doesn't concatenate a status message and a cursor
  position into one confusing announcement. `updateCursorPosition()`
  already runs every animation frame (now gated by P7-01's dirty-flag
  check); mirroring its output into `#sr-cursor` on every call is safe
  because browsers don't fire an accessibility "content changed" event
  for a live region when `.textContent` is set to an unchanged value -
  no extra debouncing needed to avoid announcement spam during rapid
  cursor movement.
- **P6-03**: one global CSS rule
  (`button, a, input, select, [tabindex] { &:focus-visible { outline: ... } }`)
  instead of editing every button's Tailwind class list individually -
  this app has dozens of buttons across the toolbar and five modals, all
  previously relying on `:hover` alone. `:focus-visible` (not `:focus`)
  so the ring only appears for keyboard/programmatic focus, matching how
  `.toolbar-btn:active` already gives pointer interactions distinct
  feedback. Discovered along the way that `#editor-canvas` is made
  focusable dynamically in JS (`canvas.setAttribute("tabindex", "0")`,
  not in the static HTML) - the `[tabindex]` selector picks it up for
  free.
- **P7-01**: a single `dirty: bool` field on `WasmEditor`, set to `true`
  at the top of every mutating public method (17 call sites - `set_size`,
  all seven `handle_*` input methods, `set_text`, `insert_text`,
  `delete_selection`, `undo`/`redo` (conditionally, only when something
  was actually undone/redone), `toggle_vertical`, `set_settings_json`,
  `reset_to_defaults`, `run_command`, `set_cursor_position`), cleared at
  the end of `render()`. New `needs_render(timestamp)` WASM method
  returns `self.dirty || (timestamp - self.state.last_render_time > 500.0)`
  - the second half of that OR keeps the cursor blinking at its usual
  rate even while otherwise idle, mirroring the exact threshold
  `render()` itself already used internally. `animate()` in `index.html`
  now checks `needs_render()` before calling both `render()` (rebuilds a
  full `width*height*4` pixel buffer + `put_image_data`) and
  `updateCursorPosition()` (calls `get_text()`, O(document length)) -
  both previously ran unconditionally on every `requestAnimationFrame`
  tick, ~60 times/second, even fully idle. Chose "mark dirty
  unconditionally, err toward an occasional unnecessary render" over
  precisely enumerating each method's exact visual effect, since a
  missed spot would be a real (if minor) staleness bug and the cost of
  a false positive is just one extra cheap frame. Deliberately did
  *not* implement the more surgical "cursor blink only repaints a small
  region" half of the plan's P7-01 description - that would need
  `render()` to track and reuse the cursor's previous bounding box
  separately from the full glyph-drawing pass, a deeper change for
  modest additional gain given blink-only frames are already just
  2/second, not a hot path; the dominant win (skipping ~58 of every 60
  idle frames entirely) is what's implemented. Verified with a
  dedicated Playwright test that polls `needs_render()` until it
  observes `false` (can't assert a single point in time - it races the
  live blink interval - so the test asserts "the gate can close at all,"
  not its exact timing).
- **P3-03**: `LatinMappingManager` (JS, module-scope, same shape as
  `TabManager`/`KeybindingManager`) holds `base` (parsed once from
  `config/latin.csv`, never mutated - the "factory default" a row's
  reset button reverts to), `overrides` (from localStorage, one key per
  overridden mapping), and `effective` (`{ ...base, ...overrides }`,
  what the canvas keydown handler and the iOS mobile-input handler
  actually look characters up in). Discovered two duplicate CSV rows
  (backtick and apostrophe each listed twice, mapping to the same
  character both times - harmless, since `Object.keys` on the parsed
  `base` object naturally dedupes) and the pre-existing special-case
  comma handling (an empty-key row with a third CSV column) while
  porting the parsing logic; preserved both exactly rather than
  "fixing" something that wasn't actually broken. The editable table
  only lets you change a *value* (the Mongolian output), not add/remove
  *keys* - changing which physical keys exist wasn't asked for and
  would need UI for typing a raw key combo, similar complexity to
  P6-01's binding recorder.
- **P6-01**: scope decision made and confirmed with the user before
  writing any code - only *app-level* actions are configurable (command
  palette, find/replace, zoom, tabs, save/open/export, transliteration,
  input-mode toggle - 16 actions total). Undo/redo, copy/cut/paste, and
  character typing stay hardcoded in `WasmEditor::handle_key_down`,
  since making those configurable would mean either duplicating Rust's
  key-matching in JS or routing every keystroke through the combo parser
  before it can be typed - not worth the risk for this pass. Rust
  (`src/config/keybindings.rs`) owns the default combo per action
  (source of truth, same principle as `themes.rs`); JS
  (`KeybindingManager`) owns the actual `KeyboardEvent` matching and the
  localStorage override layer - Rust never parses a combo string. The
  entire global `document.addEventListener("keydown", ...)` shortcut
  block (previously ~140 lines of one hardcoded `if` per shortcut) was
  refactored into a data-driven loop over a `keyActions` array, each
  entry `{ id, run, guard? }`; `guard` covers the two find-navigation
  actions, which (matching pre-refactor behavior) only fire while the
  find panel is open. Recording a new shortcut attaches a one-shot
  `keydown` listener directly to the "record" button (not `document`)
  and calls `stopPropagation()`, so a key pressed while recording can
  never also reach the global dispatcher or trigger a browser default.
  Conflict detection (two actions bound to the same combo) is a
  `confirm()` warning, not a hard block - ties resolve by array order in
  the dispatch loop, which is a reasonable, simple resolution kept
  intentionally simple rather than auto-clearing the other binding.

## Files changed

- **New:** `src/editor_core/format_control.rs` (337 lines) - pure
  backspace/delete decision logic + 18 unit tests (P7-03).
- **New:** `src/config/keybindings.rs` (164 lines) - `KeyBinding`
  struct, `default_keybindings()` (16 actions), 3 unit tests (P6-01).
- Modified: `src/editor_core/mod.rs` / `src/config/mod.rs` - publish the
  two new modules.
- Modified: `src/lib.rs` (net -326 due to the format_control extraction
  removing far more than the dirty-flag/keybindings additions add back)
  - `handle_backspace`/`handle_delete` reduced to thin wrappers calling
    `format_control::backspace_plan`/`delete_plan` +
    a new shared `apply_delete_plan`/`current_line_chars_and_cursor`
    pair (P7-03).
  - New `dirty: bool` field, `needs_render()` method, `self.dirty = true`
    at 17 mutating call sites, `self.dirty = false` at the end of
    `render()` (P7-01).
  - New `list_keybindings()` WASM method (P6-01).
- Modified: `Cargo.toml` - `wasm-opt = ["-Oz"]` (P7-02).
- Modified: `build.sh` / `build.bat` - `wasm-strip` step (graceful
  no-op if not installed) + final-size report (P7-02).
- Modified: `index.html` (+885/-... see diff)
  - `#sr-status` / `#sr-cursor` live regions;
    `updateStatus()`/`updateCursorPosition()` mirror into them (P6-02).
  - Global `:focus-visible` CSS rule (P6-03).
  - `animate()` gated by `editor.needs_render(timestamp)` (P7-01).
  - New `LatinMappingManager` class; "Input" settings tab (editable
    table, per-row reset, "reset all"); every `latinToMongolianMap[...]`
    read site repointed at `latinMapping.effective[...]` (P3-03).
  - New `KeybindingManager` class; "Keys" settings tab (recordable
    per-action shortcut, per-row reset, "reset all"); the global
    keydown listener's ~140-line `if`-chain replaced with a
    `keyActions` data-driven dispatch loop (P6-01).
  - `window.__unsTestHooks` gained `needsRender` (P7-01 test seam).
  - Cache-busting query params bumped (`uns_editor.js?v=9`,
    `uns_editor_bg.wasm?v=9`, `output.css?v=13`).
- Modified: `dist/output.css` - rebuilt via `npm run build:css`.
- **New:** `tests/e2e/rendering.spec.js`, `tests/e2e/latin-mapping.spec.js`,
  `tests/e2e/keybindings.spec.js` - one Playwright test each for P7-01,
  P3-03, and P6-01 respectively (see Verification below).

## Verification

- `cargo test --lib` - 47 tests pass (18 new in `format_control`, 3 new
  in `keybindings`; 26 pre-existing from P0-P4).
- `cargo check --target wasm32-unknown-unknown` - only the same 5
  pre-existing warnings; no new ones.
- `wasm-pack build --target web --release` / `./build.sh` end-to-end -
  succeeds; `pkg/uns_editor.d.ts` exposes `needs_render()` and
  `list_keybindings()`. Final WASM size 2.2M (down from 2.7M before
  P7-02, see design decisions for the exact before/after and the
  `wasm-strip` experiment).
- **`npx playwright test` - 6/6 passed** against the real headless
  Chromium set up in the P7-04 session:
  - `rendering.spec.js` (new): `needs_render()` observed to return
    `false` while idle.
  - `latin-mapping.spec.js` (new): override "z" -> "Q" in the Input tab,
    confirmed live typing conversion actually uses it, confirmed it
    survives a reload, reset back to default.
  - `keybindings.spec.js` (new): re-recorded "New Tab" from `Ctrl+N` to
    `Ctrl+Shift+K`, confirmed the *old* combo stopped working and the
    *new* one triggers the action, reset back to default. (Caught and
    fixed a test-only bug along the way: `toHaveClass(/border-editor-accent/)`
    false-positived against the row's own `hover:border-editor-accent`
    utility class - fixed by checking `classList.contains(...)` via
    `evaluate` instead of a substring-matching regex.)
  - `smoke.spec.js`, `tabs.spec.js` (×2, pre-existing) - re-run
    unchanged to confirm no regression from the keydown-dispatch
    refactor (P6-01) or the animate()-loop gating (P7-01), both of which
    touch code every other test's typing/shortcuts depend on.
- `node --check` against the extracted `<script type="module">` body,
  re-run after every JS edit batch - no syntax errors.
- `npm run build:css` - rebuilt `dist/output.css`.

## Follow-ups / known limits

- **Keybinding conflicts are a warning, not prevented.** Recording a
  combo already used by another action shows a `confirm()` but doesn't
  auto-clear the other binding; if accepted, whichever action appears
  first in the `keyActions` array wins ties. Acceptable for a first
  pass; a "swap" or "auto-clear the other one" flow would be a natural
  follow-up if this turns out to confuse anyone in practice.
- **`Ctrl+W`/`Ctrl+Tab` remain best-effort** regardless of what they're
  reconfigured to - this is a browser-level restriction (Chrome
  specifically blocks pages from overriding its own tab-close/switch
  shortcuts) that P6-01 cannot work around, only carry forward from
  P8-03.
- **The Latin mapping table doesn't let you add or remove keys**, only
  change a key's mapped value. Not requested, and doing so would need a
  "type a new key combo" UI with similar complexity to P6-01's shortcut
  recorder.
- **P7-01's cursor-blink still repaints the whole canvas**, not just the
  cursor's bounding box - see design decisions above for why that
  narrower optimization was deliberately deferred.
- **No before/after performance *measurement*** (e.g. actual CPU/frame
  timings) for P7-01 beyond confirming the gating logic itself works -
  this sandbox has no way to profile a real browser's paint/CPU usage,
  only to run functional assertions against it.
- **`wasm-strip` isn't installed in CI** (`.github/workflows/e2e.yml`
  from P7-04) or documented as a prerequisite anywhere beyond the build
  scripts' own graceful-skip message - low priority given the measured
  ~235-byte marginal benefit.

## Next phase

Per `prompt/FUTURE_PLAN.md`'s original suggested order (now that the two
user-reprioritized items, P7-04 and P8-03, and this batch are all done):
remaining backlog is Phase P5 (`P5-01` recent files, `P5-03` `.uns` view
state, `P5-04` PDF export), Phase P8 stretch items (`P8-01` syntax
highlighting, `P8-02` CRDT collaboration, `P8-03` is done), and `P3-01`/
`P3-02`/`P3-04` (format-control visibility toggle, orientation-persistence
regression test, input-mode status pill). None of these block each
other; `EditorState`'s "plugin data slot" shape
(`settings`/`history`/`find_replace`) and the `TabManager`/
`LatinMappingManager`/`KeybindingManager` JS module-scope-singleton
pattern are both now well-established for whichever comes next to
follow.
