# Transliteration Feature — Implementation Plan

## Overview

Add a toolbar button that opens a Cyrillic → Traditional Mongolian transliteration dialog. The user types a Cyrillic word into a horizontal input field; live lookups against a bundled dictionary (`01_kv.tsv.xz`) render matching Mongolian script vertically on a second canvas that shares the main editor's `FontSystem` and `SwashCache` so the result is visually identical to the main editor. Enter inserts the matched word into the main editor at the cursor; Escape clears the input or closes the dialog when empty.

## Phase 1: Dictionary pipeline

### 1.1 Build-time decompression
- `scripts/prepare-dictionary.js` (Node) decompresses `01_kv.tsv.xz` using `xz-decompress` and writes `dist/dictionary/01_kv.tsv`.
- New npm script `prepare:dict` invoked from `build.sh` and `build.bat` before `build:css`.
- Generated file is gitignored; `.xz` stays the source of truth.

### 1.2 Format
- TSV: two tab-separated columns per line — `cyrillic<TAB>mongolian`.
- Blank lines and comments (`#`) ignored. Keys are lower-cased and NFC-normalized on load.

### 1.3 Runtime load (Rust/WASM)
- New module `src/dictionary/mod.rs` + `src/dictionary/lookup.rs`.
- `Dictionary { map: HashMap<String, Vec<String>> }`.
- Loaded lazily on first dialog open.

## Phase 2: Shared rendering

### 2.1 Share FontSystem / SwashCache
- `WasmEditor` owns `FontSystem` via `EditorState`. Translit renderer lives on `WasmEditor` so it reuses the same font system through split-borrow patterns (same style as `set_size`).

### 2.2 TranslitRenderer
- `src/translit/renderer.rs` holds a `Buffer` with `TextOrientation::VerticalLtr`.
- `render_to_canvas` shapes the text, paints pixels, and writes to a target canvas via a `CanvasRenderingContext2d`.

### 2.3 Visual consistency
- Pulls font family / size / line height from current `EditorSettings::fonts`.
- Pulls text and background colors from current `EditorSettings::appearance`.

## Phase 3: UI — modal dialog

### 3.1 Toolbar button
- New button `#translit-btn` with Material icon `translate`. Added to desktop toolbar and mobile menu. Shortcut `Ctrl+T`.

### 3.2 Modal structure
- `#translit-modal` following the Tailwind pattern of `#settings-modal`.
- Body: horizontal text input, status row, vertical preview canvas.
- Footer: Copy, Insert into editor, Cancel.

### 3.3 Interaction
- Debounced (120 ms) live lookup on each keystroke.
- Enter: insert found Mongolian into main editor at cursor, clear input, keep dialog open.
- Escape: clear input first; Escape on empty input closes dialog.
- Click outside: close.
- Copy: writes current Mongolian result to clipboard.
- Insert: same as Enter.

### 3.4 Mobile
- Modal responsive tweaks match existing modals (95vw/95vh, 16px input to prevent iOS zoom).
- Preview canvas resizes on modal open and viewport change.

## Phase 4: Integration wiring (JS)

- `TranslitModal` class in `index.html` mirroring `SettingsModal` / `FileManager`.
- Lazy dictionary load on first open with "Loading dictionary…" status.
- Canvas DPI-aware sizing via `editor.translit_resize`.
- Global `Ctrl+T` registered in existing keydown listener.

## Phase 5: Build & testing

### 5.1 Build scripts
- `build.sh` and `build.bat`:
  1. `npm run prepare:dict`
  2. `npm run build:css`
  3. `wasm-pack build --target web --release`

### 5.2 Testing checklist
- Dictionary loads on first open; subsequent opens are instant.
- Known Cyrillic words render correctly in vertical Mongolian.
- Unknown words show "Not found".
- Enter inserts, clears input, keeps dialog open.
- Escape clears or closes.
- Copy writes to clipboard.
- Font/color changes in main settings flow through to the dialog.
- Mobile keyboard behaves correctly.

### 5.3 Edge cases
- Empty input: canvas cleared, status "Ready".
- Whitespace: trimmed before lookup.
- Multiple variants: first rendered; variant count shown in status.

## Architecture changes

```
src/
  config/
    settings.rs
  editor_core/
    editor_state.rs
  dictionary/
    mod.rs
    lookup.rs
  translit/
    mod.rs
    renderer.rs
  lib.rs                 (+ translit WASM methods)

scripts/
  prepare-dictionary.js  (new)

dist/
  dictionary/
    01_kv.tsv            (generated, gitignored)

index.html               (+ translit button, modal, TranslitModal class)
```

## Dependencies

- Rust: no new crates.
- JavaScript: `xz-decompress` (dev dependency).

## Success metrics

- Dialog opens in < 300 ms (after first dictionary load).
- Lookup + render latency < 50 ms.
- Dictionary heap footprint < 10 MB.
- Mongolian rendering pixel-identical to main editor.
- No regressions in existing features.

## Out-of-scope for v1

- Editing the dictionary in-app.
- Showing all variants simultaneously.
- Reverse (Mongolian → Cyrillic) lookup.
- Prefix / fuzzy matching.

## Follow-ups (to add to FUTURE_PLAN.md after landing)

- **P9-01** — Prefix/fuzzy suggestions (trie).
- **P9-02** — Variant picker when multiple Mongolian forms exist.
- **P9-03** — Reverse lookup mode.
- **P9-04** — User-editable dictionary overrides in localStorage.
