# P8-03 — Multiple documents / tab strip (implementation log)

Date: 2026-07-31
Reference: `prompt/FUTURE_PLAN.md`, P8-03
Patch: `patch/Phase_8_03.patch`

## Scope

| ID    | Effort | Status | Summary                                                                        |
|-------|--------|--------|---------------------------------------------------------------------------------|
| P8-03 | M      | done   | Multi-document tabs: JS-level `TabManager` layered on one shared `WasmEditor`, with full multi-tab session persistence |

Done ahead of the plan's suggested order, at the user's explicit
request, immediately after P7-04 (so this feature could be built and
verified against a real headless browser from the start - see
`DOC/IMPL/PHASE_7_04_IMPL.md`).

## Design decisions

All of the following were confirmed with the user before writing any
code (see the "Open questions" resolutions in `prompt/FUTURE_PLAN.md`):

- **JS-level tabs on one shared `WasmEditor`, not one `WasmEditor` per
  tab, and not per-tab state in Rust.** Only the *active* tab's
  text/cursor actually live in the WASM buffer at any moment.
  `TabManager.switchTo()` snapshots the outgoing tab's live state
  (`get_text()`/`get_cursor_position()`) into its own JS record, then
  loads the incoming tab's record into the buffer
  (`set_text()`/`set_cursor_position()`). This avoids a much larger,
  riskier refactor of `lib.rs` (which assumes one document throughout
  P2/P4/P5) so soon after that work shipped.
- **Accepted trade-off: undo/redo and find/replace state reset on every
  tab switch.** `set_text()` already calls `discard_history()` and
  `find_replace.clear()` in Rust (added in P2-01/P2-03 for the "opening
  a file resets undo" case) - switching tabs now goes through the exact
  same path, so this isn't new Rust behavior, just a new *trigger* for
  existing behavior. Explicitly not "fixed" by giving each tab its own
  `Editor`/`HistoryManager`/`FindReplaceState` in Rust, which was the
  heavyweight alternative considered and rejected up front.
- **Settings/theme are global, not per-tab.** Matches how most tabbed
  editors behave (one font/theme preference, many documents) and keeps
  `SettingsModal` completely unaware that tabs exist - it was not
  touched at all in this phase.
- **Open File (toolbar, `Ctrl+O`, drag-and-drop) creates a new tab
  instead of replacing the active one's content.** This is the
  headline behavior change from before P8-03 existed. A direct
  consequence: the confirm-before-discard guards added in P5-02 for
  `FileManager.openFile()` and `handleDroppedFile()` were **removed** -
  they no longer discard anything, so a confirmation asking "continue
  anyway?" would be actively wrong (nothing to lose). The guard on
  **Clear Document** was kept, since Clear still empties the *active*
  tab's content in place.
- **Every open tab is continuously persisted (not just the active
  one), and restoration on reload is silent - no banner.** This
  supersedes Phase P2-05's `AutoSave` class (single "draft" +
  Restore/Dismiss banner) and Phase P5-02's `DirtyTracker` class
  (single baseline/dirty flag) - both are now `TabManager`
  responsibilities, per-tab. The reasoning: once *every* tab's content
  is safely in localStorage on every poll tick, there's no longer a
  meaningful distinction between "the normal state of the app" and "an
  old draft to maybe recover" - the tabs simply *are* what was open,
  the same way a real desktop app's session restore doesn't ask
  permission either.
- **A tab's `isDirty` still means "differs from the last explicit
  save/open," not "differs from what's in localStorage."**
  localStorage persistence is a safety net against losing work to a
  crash or an accidentally closed tab, not a substitute for actually
  exporting a `.txt`/`.uns` file - the dirty dot and `beforeunload`
  warning stay meaningful even though the content itself won't
  actually be lost on a reload.
- **New tabs are always clean at creation.** `createTab()` sets
  `savedBaseline` equal to the initial `text` - true both for a blank
  "+" tab (empty is trivially "saved") and for a freshly opened file
  (its on-disk content *is* what's "saved").
- **The very first tab on a first-ever visit is seeded from whatever
  the WASM editor already loaded (the random greeting), not created
  via the normal `createTab()`/`set_text()` path.**
  `seedFromCurrentEditorState()` avoids an redundant `set_text()` round
  trip through Rust (which would also pointlessly call
  `discard_history()` on a document that has no history yet).
- **Keybindings: `Ctrl+N` (new tab) is reliable; `Ctrl+W` (close) and
  `Ctrl+Tab`/`Ctrl+Shift+Tab` (cycle) are best-effort only.** Chrome
  specifically refuses to let a page override closing its own browser
  tab or switching browser tabs, regardless of `preventDefault()` - this
  is a known, long-standing browser restriction, not a bug in this
  implementation. The × button and clicking a tab remain the reliable
  interaction for close/switch; the keybindings are a bonus for
  whichever browsers do allow it.
- **The tab strip always rebuilds its DOM wholesale on `render()`**
  rather than diffing. Simpler, and tab counts are small enough (a
  handful in realistic use) that this is not a performance concern -
  consistent with `CommandPalette`'s list rendering and
  `FindReplacePanel`'s match count, which do the same thing.

## Files changed

- Modified: `index.html` (+422/-251 net, but effectively a full rewrite
  of the affected classes)
  - **Removed:** `AutoSave` class, `DirtyTracker` class, the
    `#autosave-banner` markup and its Restore/Dismiss wiring in
    `main()`.
  - **New:** `TabManager` class (~300 lines) - see design decisions
    above for behavior; `#tab-strip` markup (rendered entirely by the
    class, no static tab markup).
  - `FileManager.loadTxtFile`/`loadUnsFile`: create a new tab instead of
    calling `editor.set_text()` directly; `.uns` cursor restoration now
    applies to the new tab specifically.
  - `FileManager.saveAsTxt`/`saveAsUns`: call
    `tabManager.markActiveClean(filename)` (renames the tab to match
    the saved file) instead of the old
    `autoSaveManager.clearDraft()`/`dirtyTracker.markClean()` pair.
  - `FileManager.openFile()`/`handleDroppedFile()`: confirm-before-
    discard guards removed (see design decisions).
  - `clear-btn` handler: guard now reads `tabManager.isActiveDirty()`
    instead of the retired `dirtyTracker.isDirty`.
  - `main()`: `TabManager` construction, restore-or-seed on startup,
    `beforeunload` now reads `tabManager.anyDirty()`.
  - New keybindings: `Ctrl+N` (new tab), `Ctrl+W` (close active tab,
    best-effort), `Ctrl+Tab`/`Ctrl+Shift+Tab` (cycle tabs, best-effort).
  - `#dirty-indicator`'s doc comment and `title` updated to describe its
    new meaning (aggregate "any tab dirty," not "the document is
    dirty" - the per-tab dot in the strip itself covers the specific
    case now).
- Modified: `tests/e2e/smoke.spec.js` - reload/content-recovery
  assertions updated from "banner appears, click Restore" to "content
  is just already there" (see P7-04's follow-up note, now resolved).
- **New:** `tests/e2e/tabs.spec.js` - two tests: (1) create a second
  tab, confirm it's independent of the first, switch back, close the
  first (dirty - must prompt), confirm the second survives; (2) `Ctrl+N`
  opens a new tab.
- Modified: `dist/output.css` - rebuilt via `npm run build:css` for the
  tab strip's utility classes.

No Rust files were touched - this phase is entirely a JS/HTML feature
on top of existing WASM exports (`get_text`, `set_text`,
`get_cursor_position`, `set_cursor_position`, `get_settings_json`,
`set_settings_json`), confirmed via `git diff --stat -- src/` showing no
changes.

## Verification

- **Ran both new/updated Playwright specs for real** against the
  headless Chromium installed for P7-04:
  `npx playwright test` → `3 passed (5.1s)` across
  `smoke.spec.js` (updated) and `tabs.spec.js` (new: tab
  create/switch/close-with-confirm, and `Ctrl+N`).
- `cargo test --lib` re-run to confirm the (expected) zero Rust impact:
  26 tests pass, unchanged from before this phase.
- `node --check` against the extracted `<script type="module">` body
  after every edit batch - no syntax errors.
- `npm run build:css` - rebuilt `dist/output.css`.
- Manually traced the "close the last remaining tab" edge case: closing
  the only open tab immediately opens a fresh blank one rather than
  leaving zero tabs (verified in code, not directly asserted by a test
  - the two tabs tests always leave ≥1 tab open at the end).

## Follow-ups / known limits

- **The logo's "load version info" button still bypasses `TabManager`
  entirely** (calls `editor.set_text()` directly, same gap already
  flagged in `DOC/IMPL/PHASE_4_IMPL.md`'s follow-ups for P5-02). It
  still works correctly at the buffer level and the active tab's dirty
  state catches up within one poll tick (≤2s) or on the next tab
  switch/close, but there's a brief window where the tab strip doesn't
  yet reflect that the content changed. Low priority, same reasoning as
  before: not "Open" or "Clear" in the ordinary sense.
- **The Find & Replace panel's own input/checkboxes don't reset when
  switching tabs**, even though the underlying Rust `find_replace` state
  does (via `set_text`'s existing `find_replace.clear()`). If a user has
  the panel open with a query typed, switches tabs, and hits Enter/F3,
  they'll see "0/0" until they retype the query (which would generally
  need to happen anyway, since it's now a different document) - a minor
  UX rough edge, not a correctness bug, and out of this phase's
  requested scope.
- **No tab reordering (drag-to-reorder) or right-click context menu.**
  Not requested; tabs open in creation order and can only be closed, not
  rearranged.
- **No per-tab file-format memory.** Saving via the Save modal always
  asks txt-vs-uns regardless of how a tab was opened; a tab opened from
  a `.uns` file doesn't "remember" to offer `.uns` by default on its
  next save. Not requested, and the existing Save modal flow was left
  entirely alone.
- **No test for the localStorage persistence format surviving a schema
  change** (e.g. what happens if `uns-editor-tabs-v1`'s shape changes in
  a future phase). `restoreFromStorage()` defensively coerces every
  field's type and falls back to sane defaults per-field, and returns
  `false` (triggering the seed-fresh path) if the top-level shape is
  unrecognizable, but this defensive coding is unit-untested - only
  exercised implicitly by the happy-path Playwright tests.

## Next phase

Per `prompt/FUTURE_PLAN.md`'s updated suggested order: **Phase P6
(keybindings and accessibility)**, **P3-03 (Latin-map editor)**, and the
remainder of **Phase P7** (P7-01, P7-02, and the `handle_backspace`/
`handle_delete` extraction half of P7-03). All independent of tabs.
Should P8-03 ever need Rust-side per-tab state after all (e.g. if the
"undo resets on switch" trade-off turns out to bother users in
practice), `EditorState` already has the right shape to grow into it -
`settings`/`history`/`find_replace` are exactly the fields that would
need to move behind a `Vec<Document>` indirection.
