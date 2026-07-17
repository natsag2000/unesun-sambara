# Phase P0 — Foundation cleanup (implementation log)

Date: 2026-07-17
Reference: `prompt/FUTURE_PLAN.md`, Phase P0 (items P0-01 through P0-04)
Patch: `patch/Phase_0.patch`

## Scope

All four P0 items completed:

| ID    | Effort | Status | Summary                                                                          |
|-------|--------|--------|----------------------------------------------------------------------------------|
| P0-01 | S      | done   | Rust becomes single source of truth for default settings; add `reset_to_defaults()` |
| P0-02 | S      | done   | Stop overwriting orientation on save                                             |
| P0-03 | S      | done   | Preserve selection / gutter / line-number colors on save                         |
| P0-04 | S      | done   | Remove dead helpers in `src/config/settings.rs`                                  |

## Decisions taken

- **Canonical defaults (P0-01).** Adopted a paper-like light theme:
  - `text_color = #0d0d0d`
  - `background_color = #f5f5f5`
  - `cursor_color = #0d0d0d` (matches text)
  - `font_size = 43`, `line_height = 54`
  - Hidden fields untouched: `selection_color`, `gutter_background`, `line_number_color`.
- **Handling of hidden colors (P0-03).** Chose "merge on save" over adding an
  Advanced subsection now. Cheaper fix; advanced exposure can happen in a
  later phase (P4 themes).
- **`EditorSettings::new`.** Removed alongside P0-04 as it was a zero-value
  wrapper around `default()` with no live call sites. Not in the original P0
  list, but consistent with its intent.

## Files changed

- `src/config/settings.rs`
  - New default values for `AppearanceSettings` and `FontSettings`.
  - Deleted `#[allow(dead_code)]` helpers `color_to_hex` and `hex_to_color`.
  - Deleted unused `EditorSettings::new`.
- `src/lib.rs`
  - Added `WasmEditor::reset_to_defaults()` returning the applied JSON.
  - Added `WasmEditor::get_default_settings_json()` (non-mutating helper).
- `index.html`
  - `SettingsModal.save()` now reads current settings via
    `get_settings_json()` and merges the UI-controlled fields on top,
    preserving orientation and the three hidden colors.
  - `SettingsModal.reset()` delegates to `editor.reset_to_defaults()` and
    stores the returned JSON directly, removing all hardcoded defaults from
    JS.

## Verification

- `cargo check --target wasm32-unknown-unknown` succeeds; only pre-existing
  warnings (deprecated `set_fill_style`, `Dictionary::len` unused).
- `wasm-pack build --target web --release` succeeds.
- Generated `pkg/uns_editor.d.ts` exposes `reset_to_defaults()` and
  `get_default_settings_json()`.

## Follow-ups / known limits

- Users with a pre-existing `uns-editor-settings` entry in localStorage will
  keep their old values on the next load. Only a fresh browser or an explicit
  "Reset" picks up the new light-theme defaults. A one-shot migration
  (invalidate cached settings by version bump) is out of scope for P0 and can
  be handled together with P5-03 (`.uns` v1.1) or P4-01 (themes).
- `pkg/` artifacts changed as a side-effect of the release build. They are
  intentionally excluded from `patch/Phase_0.patch` because `wasm-pack build`
  regenerates them.

## Next phase

Phase P1 (plugin / extension architecture) is next in the plan. The
recommended sequencing is under discussion; see the open question in the
current session log before starting.
