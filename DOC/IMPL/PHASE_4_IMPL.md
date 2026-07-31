# Phase P4 (+ P8-04, P5-02) — Theming, zoom, dirty indicator (implementation log)

Date: 2026-07-31
Reference: `prompt/FUTURE_PLAN.md`, Phase P4 (P4-01..P4-03), P8-04, P5-02
Patch: `patch/Phase_4.patch`

## Scope

This session covered the plan's "suggested order of execution" step 4 in
full, which bundles items from three different numbered phases (P4, P8,
P5) rather than one phase end-to-end. Everything below shipped together.

| ID    | Effort | Status | Summary                                                                                  |
|-------|--------|--------|-------------------------------------------------------------------------------------------|
| P4-01 | M      | done   | 5 built-in theme presets, dropdown in Color tab, "Custom" on manual edit                  |
| P4-02 | S      | done   | Import/export theme JSON from the Color tab                                              |
| P4-03 | S      | done   | `prefers-color-scheme` detection on first-ever load (no saved settings yet)               |
| P8-04 | S      | done   | `Ctrl+=`/`Ctrl+-` zoom, proportionally scaling line height                                |
| P5-02 | M      | done   | Dirty (unsaved-changes) dot, `beforeunload` warning, confirm on Open/Clear                |

Plus one pre-existing latent bug fixed along the way: `selection_color`'s
alpha channel silently became fully opaque on every settings save (see
"Design decisions").

## Design decisions

- **Themes cover colors only, not fonts.** Per the scope decision made
  before starting: switching a theme never resizes or reflows text, only
  recolors it. This keeps theme switching instant and side-effect-free
  on cursor position / scroll / undo history.
- **`"default-light"` is defined to exactly equal
  `AppearanceSettings::default()`.** Verified by a unit test
  (`themes::tests::default_light_matches_appearance_default`). Existing
  users who have never touched the Color tab see zero visual change and
  the theme dropdown correctly shows "Default Light" pre-selected.
- **`EditorBehaviorSettings.theme_id` is purely cosmetic state for the JS
  dropdown.** Rust never reads it to decide what to render -
  `appearance` alone is the rendering source of truth, same principle as
  P0-01's "Rust is the single source of truth for defaults." This avoids
  a second code path that could drift from what's actually on screen.
- **No new WASM methods beyond `list_themes()`.** Applying a theme,
  importing, and exporting are all plain JS operations against the
  existing `get_settings_json()`/`set_settings_json()` pair - the same
  "merge on save" pattern from P0-02/P0-03. `list_themes()` itself is a
  read-only snapshot (doesn't mutate `WasmEditor` state), consistent with
  `get_default_settings_json()`.
- **Fixed a latent alpha-dropping bug in `color_serde` while exposing the
  four previously-hidden color fields.** The old serializer always wrote
  6-digit RGB hex, discarding alpha entirely. Because the JS settings
  modal round-trips through `get_settings_json()` → merge →
  `set_settings_json()` on *every* save (even for fields it doesn't
  touch), `selection_color`'s default 128-alpha silently became fully
  opaque the first time a user ever opened Settings and clicked Save -
  before this session, that was invisible because nothing let a user see
  or care about `selection_color`. Now that P4-01 puts it in the
  "Advanced colors" UI, the bug would have been immediately visible (a
  selection highlight that's supposed to be translucent turning opaque
  after any save). Fixed via optional 8-digit hex (`#rrggbbaa`), fully
  backward compatible: 6-digit values still parse as before (opaque),
  and only non-opaque colors grow the extra 2 digits on serialize - so
  existing `.uns` files and localStorage entries are byte-for-byte
  unchanged for every color that's already opaque. Covered by 4 new
  tests in `src/config/settings.rs`.
- **`<input type="color">` can't hold alpha, so the JS layer tracks each
  advanced field's alpha suffix separately** (`SettingsModal.colorAlpha`,
  keyed by field name) and re-appends it on save. A theme or an imported
  JSON file updates the tracked alpha to whatever that source specifies;
  a manual edit via the picker keeps whatever alpha was already tracked
  (i.e. picking a new hue for the selection color doesn't accidentally
  make it opaque).
- **"Custom" is purely a UI state transition, not a distinct color set.**
  Any manual edit to any of the six color fields (via `markCustom()`,
  called from every color `input` listener except while
  `applyThemeColors` is running) flips the dropdown to "Custom" and
  `activeThemeId`. There's no dedicated "custom theme" object - the six
  live color fields already *are* the custom theme.
- **Theme export always exports the live fields, not a named preset's
  canonical values.** Exporting while "Default Dark" is selected but a
  color has been hand-tweaked exports the tweaked version, matching
  user expectation ("export what I see").
- **Zoom scales line-height proportionally, not to a fixed 1.25× ratio.**
  The settings modal's own font-size slider hard-resets line-height to
  `round(size * 1.25)` on every change; the zoom shortcut instead
  preserves whatever ratio the user already had
  (`newLineHeight = round(oldLineHeight * newSize / oldSize)`), since a
  keyboard shortcut a user reaches for repeatedly during a session
  shouldn't silently overwrite a manually-tuned line-height on every
  press.
- **The dirty indicator's "clean" baseline is the last explicit
  save/open/clear, not a content hash.** The plan's text says "hash of
  the last saved content," but a plain string comparison
  (`text !== baselineText`) is simpler, has no collision risk, and costs
  the same as hashing would (both require reading the full text once per
  poll) at this app's scale. `DirtyTracker` polls every 1s - more
  responsive than `AutoSave`'s 3s cadence, appropriate since this one
  drives a UI element the user is actively looking at rather than a
  background safety net.
- **Restoring an auto-save draft does *not* mark the document clean.**
  Recovered unsaved work is still unsaved work; the dirty dot stays on
  (or turns on) until the user explicitly saves it, prompting exactly
  the action that data deserves.
- **Confirm-before-discard covers Open (button, `Ctrl+O`, drag-and-drop)
  and Clear, per the resolved scope decision, but not the logo's "load
  version info" easter egg**, which also replaces the buffer via
  `set_text`. That path was judged out of scope (not "Open" or "Clear" in
  the ordinary sense) rather than an oversight - flagged in Follow-ups
  below in case that's the wrong call.
- **`beforeunload` and the in-app confirms are independent guards.** A
  browser tab close/refresh only ever gets the native `beforeunload`
  prompt (no custom confirm text is possible there); in-app Open/Clear
  get an app-controlled `confirm()` with editor-specific wording. Both
  read the same `dirtyTracker.isDirty` flag so they can never disagree
  about whether there are unsaved changes.

## Files changed

- **New:** `src/config/themes.rs` (162 lines)
  - `Theme` struct, `built_in_themes()` (5 presets: Default Light,
    Default Dark, High Contrast, Solarized Dark, Mongolian Parchment),
    `theme_by_id()`.
  - 4 unit tests, including one asserting the exact JSON shape
    `list_themes()` promises the JS layer.
- Modified: `src/config/mod.rs` - publish `themes`.
- Modified: `src/config/settings.rs` (+110)
  - `EditorBehaviorSettings.theme_id: String` (default `"default-light"`,
    `#[serde(default = ...)]` for old settings JSON).
  - `color_serde`: alpha-preserving 6-or-8-digit hex round trip (the bug
    fix above).
  - 5 new unit tests (settings round-trip, alpha round-trip, 6-digit
    opacity default, old-JSON-without-new-fields compatibility).
- Modified: `src/lib.rs` (+12) - new `list_themes()` WASM method.
- Modified: `index.html` (+575/-31... see diff)
  - Color tab: theme `<select>` populated from `list_themes()`,
    "Advanced colors" `<details>` section (cursor/selection/gutter
    background/line number pickers), theme export/import buttons + hidden
    file input.
  - `SettingsModal`: `loadThemes()`, `applyTheme()`/`applyThemeColors()`,
    `markCustom()`, `exportTheme()`/`importTheme()`, `collectAppearance()`
    (alpha-aware); `loadSettings()`/`save()` updated to read/write all six
    color fields plus `theme_id`.
  - `main()`: `prefers-color-scheme` check on first-ever load (P4-03);
    `Ctrl+=`/`Ctrl+-` global keybinding calling new `adjustFontSize()`
    helper (P8-04); `DirtyTracker` instantiation + `beforeunload`
    listener (P5-02).
  - New `DirtyTracker` class (poll-based, mirrors `AutoSave`'s shape).
  - New `#dirty-indicator` badge dot next to the logo; confirm-before-
    discard guards in `FileManager.openFile()`, `handleDroppedFile()`,
    and the `clear-btn` handler; `markClean()` wired into every
    save/open/clear success path alongside the existing
    `autoSaveManager.clearDraft()` calls.
  - Cache-busting query params bumped (`uns_editor.js?v=7`,
    `uns_editor_bg.wasm?v=7`, `output.css?v=11`).
- Modified: `dist/output.css` - rebuilt via `npm run build:css`.

## Verification

- `cargo test --lib` - 26 tests pass (9 new: 4 in `themes`, 5 in
  `settings`; 17 pre-existing from P0-P2).
- `cargo check --target wasm32-unknown-unknown` - only the same 5
  pre-existing warnings; no new ones.
- `wasm-pack build --target web --release` - succeeds.
  `pkg/uns_editor.d.ts` exposes `list_themes(): string` alongside the
  existing settings/theme-adjacent methods.
- `node --check` against the extracted `<script type="module">` body,
  re-run after every JS edit batch - no syntax errors.
- `npm run build:css` - rebuilt `dist/output.css`; spot-checked new
  classes (`bg-amber-400`, `text-editor-text-dim`, etc.) present in the
  minified output.
- Manual code-path review (no browser in this environment, same
  limitation as Phase P2):
  - Traced `applyThemeColors`'s `applyingTheme` guard against
    `markCustom()` and confirmed setting `.value` programmatically does
    not fire the `input` events those listeners key off of - the guard
    is defensive, not load-bearing, but kept for clarity/future-proofing.
  - Traced the alpha round trip end-to-end: Rust default
    (`rgba(80,120,200,128)`) → `list_themes()`/`get_settings_json()`
    JSON (`"#5078c880"`) → `to6HexColor`/`alphaSuffixOf` split → picker
    display (`#5078c8`) + tracked alpha (`"80"`) → `collectAppearance()`
    reassembly (`"#5078c880"`) → `set_settings_json` → same `Color` value
    - confirmed by the Rust-side round-trip test plus this manual trace.

## Follow-ups / known limits

- **No manual in-browser QA**, same caveat as Phase P2. Highest-risk
  untested paths this time: the "Advanced colors" `<details>` UI
  rendering/interaction, the theme dropdown's visual state across
  save/reset/reload, and the dirty dot's timing relative to the
  auto-save banner on a fresh reload with a pending draft.
- **The logo's "load version info" action is not guarded by the dirty
  check**, even though it discards the current buffer the same way
  Clear does. Left out of scope per the resolved decision (only
  Open/Clear were asked for); worth revisiting if it trips someone up in
  practice.
- **Theme presets' exact color values are a first pass**, not
  extensively color-contrast-audited (WCAG AA/AAA). High Contrast in
  particular deserves an accessibility pass if it's meant to be relied
  on rather than illustrative.
- **Import validation is shallow.** `importTheme` only checks that the
  six `appearance` fields are present and are strings - it does not
  validate they're well-formed hex colors before handing them to
  `set_settings_json` (which will surface a Rust-side deserialize error
  if they're not, just not with a friendly message pointing at the
  specific bad field).
- **Dirty tracking re-reads the full document text every second.** Fine
  at this app's realistic document sizes (same assumption `AutoSave`
  already makes at a 3s cadence); would need rethinking if "very large
  document" ever becomes a real use case (see P7-01's dirty-flag-driven
  rendering note for the analogous concern on the render side).

## Next phase

Per the plan's suggested order, next up is **Phase P6 (keybindings and
accessibility)** and **P3-03 (Latin-map editor)**, then **Phase P7
(performance and quality)** - all "power-user and quality" per the
plan's own framing, and independent of anything added here. `EditorState`
now has three plugin-adjacent "data slots" following the same shape
(`settings`, `history`, `find_replace`); P6-01's configurable keybindings
would be a natural fourth if it's modeled the same way (a plain field +
commands that read/write it) rather than introducing a new pattern.
