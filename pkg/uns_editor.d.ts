/* tslint:disable */
/* eslint-disable */

export class WasmEditor {
    private constructor();
    free(): void;
    [Symbol.dispose](): void;
    /**
     * Accepts a suggestion, replacing the word it was computed for
     * with `text`. Re-resolves nothing beyond what
     * `get_word_suggestions_json` already stored (`word_start`/
     * `word_end`) - if the buffer changed since then in a way that
     * invalidates those positions, cosmic-text's own range handling
     * degrades gracefully rather than panicking, and the popup is
     * always dismissed by the JS layer's own trigger logic before the
     * buffer could change again out from under it (see
     * `WORD_SUGGESTIONS_PLAN.md` §7's trigger table). One atomic undo
     * step, same convention as every other programmatic multi-char
     * edit in this codebase (P2-01, P2-03).
     */
    accept_word_suggestion(text: string): void;
    can_redo(): boolean;
    can_undo(): boolean;
    delete_selection(): void;
    /**
     * Clears the popup's state without changing the buffer - called on
     * Escape, on cursor-moving keys/clicks, and on document-changing
     * actions (tab switch, clear, open) per the trigger table in
     * `WORD_SUGGESTIONS_PLAN.md` §7.
     */
    dismiss_word_suggestions(): void;
    get_cursor_position(): any;
    /**
     * Returns the built-in default settings as JSON without mutating the
     * editor state. Useful for previewing defaults or seeding local UI.
     */
    get_default_settings_json(): string;
    get_selected_text(): any;
    get_settings_json(): string;
    get_text(): string;
    /**
     * Recomputes the word suggestion popup's state for the current
     * cursor position. Returns
     * `{ hasSuggestions, suggestions: string[], anchorX, anchorY, lineAdvance }` -
     * `anchorX`/`anchorY` are the cursor's position in the same
     * canvas-pixel space `render()` draws into (`buffer_pixel -
     * scroll_offset + content_padding`, cosmic-text's own
     * `cursor_position()` composing directly with the same transform
     * used everywhere else in this file). JS converts that to CSS
     * position via `devicePixelRatio` and the canvas's
     * `getBoundingClientRect()` - the exact inverse of what
     * `handle_mouse_down` et al. already do the other direction.
     * `lineAdvance` is the line/column spacing at the cursor, also in
     * canvas-pixel space - JS uses it to offset the popup a full
     * line/column clear of the cursor rather than guessing a fixed
     * pixel gap (which in vertical mode landed inside the *next*
     * column's territory instead of a clean gap beside the current
     * one).
     *
     * Returns `hasSuggestions: false` (never an error) if the feature
     * is disabled in settings, the dictionary hasn't loaded yet, or
     * the cursor isn't inside/adjacent to a word at least
     * `MIN_SUGGESTION_WORD_LEN` characters long - all "nothing to show
     * right now," not failure conditions.
     */
    get_word_suggestions_json(): any;
    /**
     * Returns whether this keystroke actually changed the buffer text
     * (as opposed to just moving the cursor/selection, or doing
     * nothing). JS uses this to decide whether to refresh the word
     * suggestion popup or dismiss it - per the trigger table in
     * `WORD_SUGGESTIONS_PLAN.md` §7, pure cursor movement (arrow keys,
     * Home/End, PageUp/Down) should dismiss the popup (the word
     * context changed), not refresh it, since the user isn't actively
     * typing.
     */
    handle_key_down(event: KeyboardEvent): boolean;
    handle_key_up(event: KeyboardEvent): void;
    handle_mouse_down(event: MouseEvent): void;
    handle_mouse_move(event: MouseEvent): void;
    handle_mouse_up(_event: MouseEvent): void;
    handle_touch_end(_event: TouchEvent): void;
    handle_touch_move(event: TouchEvent): void;
    handle_touch_start(event: TouchEvent): void;
    handle_wheel(event: WheelEvent): void;
    insert_text(text: string): void;
    /**
     * Returns the list of registered commands as JSON:
     * `[{ "id": ..., "title": ..., "category": ...|null, "keybinding": ...|null }, ...]`
     * The order matches plugin registration order.
     */
    list_commands(): any;
    /**
     * Returns the default app-level keybindings (P6-01) as a JSON
     * array of `{ id, label, default }`. Does not mutate editor
     * state - the JS `KeybindingManager` layers localStorage overrides
     * on top and owns the actual `KeyboardEvent` matching; Rust is
     * only the source of truth for what ships out of the box. See
     * `src/config/keybindings.rs` for the full scope rationale (only
     * app-level shortcuts are covered, not low-level text-editing
     * keys like undo/redo).
     */
    list_keybindings(): string;
    /**
     * Returns the built-in theme presets (P4-01) as a JSON array of
     * `{ id, name, appearance: { text_color, background_color, ... } }`.
     * Does not mutate editor state - the JS Color tab applies a theme by
     * merging the chosen entry's `appearance` into the current settings
     * and calling `set_settings_json`, the same "merge on save" pattern
     * used for every other settings field (see P0-02/P0-03).
     */
    list_themes(): string;
    /**
     * Synchronous alternative to `translit_load_dictionary` above, for
     * the word suggestion popup (Phase P9). Takes already-fetched TSV
     * text and parses it in one atomic call - no `.await` inside Rust,
     * so no cross-await borrow of `self` for anything else running on
     * the same JS event loop to collide with.
     *
     * This distinction matters in practice, not just in theory: an
     * async Rust method holds its `&mut self` borrow for the entire
     * span between `.await` points, for as long as wasm-bindgen keeps
     * that generated JS `Promise` unresolved. `translit_load_dictionary`
     * gets away with that because the Transliteration modal traps
     * keyboard focus on its own input while loading - the canvas's own
     * `keydown`/`keyup`/mouse listeners and the `render()` loop can't
     * fire during that window. The word suggestion popup's dictionary
     * load is triggered *by* canvas typing, the one context where that
     * assumption doesn't hold - a `keyup` for the very keystroke that
     * triggered the load can (and did, during manual testing) fire
     * while the load's `await` is still pending, tripping
     * wasm-bindgen's "recursive use of an object" panic. Fetching the
     * text in JS (a plain `fetch()`, no WASM object involved) and
     * handing the already-resolved string to this synchronous method
     * avoids the hazard entirely - see `WordSuggestionsPopup.ensureDictionaryLoaded`
     * in `index.html`.
     */
    load_dictionary_text(text: string): void;
    /**
     * WS-09: measures the popup's current suggestion list (whatever
     * `get_word_suggestions_json` last stored in
     * `self.state.suggestions.suggestions`) using real `cosmic-text`
     * shaping - the same shaping/rasterization pipeline the main
     * document canvas uses, so the popup's Mongolian glyphs are
     * visually identical to the document regardless of which browser
     * this runs in, rather than depending on the browser's own
     * (inconsistent, sometimes absent) support for shaping Mongolian
     * text under CSS `writing-mode`/`text-orientation`.
     *
     * Returns `{ width, height, itemBounds: [{x,y,w,h}, ...] }` in the
     * same device-pixel space as `get_word_suggestions_json`'s
     * `anchorX`/`anchorY`/`lineAdvance`. JS resizes the popup's
     * `<canvas>` to `width`/`height` (after its own DPR conversion)
     * and uses `itemBounds` for click/hover hit-testing - `bounds` in
     * each entry is stretched to the popup's full cross-axis extent
     * (see `suggestion_popup_layout`'s doc comments), so click/hover
     * targets are comfortably sized, not just the glyphs' own tight
     * box.
     *
     * Deliberately does *not* draw anything - `render_suggestions_popup`
     * (called right after, once JS has resized the canvas to this
     * method's reported size) consumes the layout this call caches on
     * `self.suggestion_popup_cache` rather than recomputing it, so the
     * two calls can never disagree about where anything is.
     */
    measure_suggestions_popup(): any;
    /**
     * True if the JS `animate()` loop should actually call `render()`
     * this frame (P7-01). Two reasons to render: something the user
     * did (or a programmatic change) marked the frame dirty, or the
     * cursor's 500ms blink interval has elapsed - the latter check
     * mirrors the one `render()` itself does internally, so the
     * cursor keeps blinking at its usual rate even while otherwise
     * idle. Cheap to call every RAF tick: no pixel work, just a bool
     * and a float comparison.
     */
    needs_render(timestamp: number): boolean;
    static new(canvas_id: string): Promise<WasmEditor>;
    /**
     * Redo the most recently undone change. Returns `true` if something
     * was redone.
     */
    redo(): boolean;
    render(timestamp: number): void;
    /**
     * WS-09: draws the popup's current suggestion list into a
     * JS-supplied `<canvas>` (already resized by JS to whatever
     * `measure_suggestions_popup` most recently reported), reusing
     * that call's cached layout rather than recomputing it. `-1` for
     * `selected_index`/`hover_index` means "none" (wasm-bindgen has no
     * convenient `Option<usize>` across the JS boundary).
     *
     * Bails out (does nothing, not an error) if there's no cached
     * layout, or if the cached layout's item count no longer matches
     * the current suggestion list - the latter should never actually
     * happen since JS always calls `measure_suggestions_popup`
     * immediately before this, but a stale/mismatched draw would be a
     * worse failure mode than silently skipping a frame.
     */
    render_suggestions_popup(canvas_id: string, selected_index: number, hover_index: number): void;
    /**
     * Resets settings to the built-in defaults, applies them, and returns the
     * resulting JSON so the JS layer can update its UI and localStorage
     * without duplicating the default values. See P0-01 in
     * `prompt/FUTURE_PLAN.md`.
     */
    reset_to_defaults(): string;
    /**
     * Invoke a registered command by id. `args_json` is opaque to the
     * registry and forwarded to the handler untouched; pass `""` when
     * unused. Returns whatever the handler returns (typically
     * `JsValue::NULL`).
     *
     * Errors:
     * * `"Unknown command: <id>"` when no command matches.
     * * Any error the handler itself raises.
     */
    run_command(id: string, args_json: string): any;
    /**
     * Move the cursor to a 1-based `(line, column)` position, the same
     * convention `get_cursor_position` returns. Used by auto-save draft
     * recovery (P2-05) to restore where the user was editing; clamps
     * silently to valid buffer bounds via cosmic-text's own cursor
     * handling rather than erroring on stale positions.
     */
    set_cursor_position(line: number, column: number): void;
    set_settings_json(json: string): void;
    set_size(width: number, height: number): void;
    set_text(text: string): void;
    toggle_vertical(): void;
    /**
     * Sets up (or resizes) the offscreen translit renderer bound to the
     * given canvas element.
     */
    translit_init_canvas(canvas_id: string, width: number, height: number): void;
    /**
     * Returns true if the dictionary has been loaded.
     */
    translit_is_loaded(): boolean;
    /**
     * Fetches and parses the dictionary TSV. Safe to call multiple times; the
     * actual network + parse work happens only on the first call. Subsequent
     * calls resolve immediately.
     */
    translit_load_dictionary(url: string): Promise<void>;
    /**
     * Looks up a Cyrillic word and returns
     * `{ found: bool, mongolian: string | null, variants: number }`.
     */
    translit_lookup(cyrillic: string): any;
    /**
     * Renders the given Mongolian text vertically onto the translit canvas.
     */
    translit_render(canvas_id: string, text: string): void;
    /**
     * Undo the most recent change. Returns `true` if something was
     * undone. Bound to `Ctrl+Z` in `handle_key_down`; also exposed here
     * for the command palette / toolbar.
     */
    undo(): boolean;
}

export function main(): void;

export type InitInput = RequestInfo | URL | Response | BufferSource | WebAssembly.Module;

export interface InitOutput {
    readonly memory: WebAssembly.Memory;
    readonly __wbg_wasmeditor_free: (a: number, b: number) => void;
    readonly wasmeditor_accept_word_suggestion: (a: number, b: number, c: number) => [number, number];
    readonly wasmeditor_can_redo: (a: number) => number;
    readonly wasmeditor_can_undo: (a: number) => number;
    readonly wasmeditor_delete_selection: (a: number) => void;
    readonly wasmeditor_dismiss_word_suggestions: (a: number) => void;
    readonly wasmeditor_get_cursor_position: (a: number) => any;
    readonly wasmeditor_get_default_settings_json: (a: number) => [number, number, number, number];
    readonly wasmeditor_get_selected_text: (a: number) => any;
    readonly wasmeditor_get_settings_json: (a: number) => [number, number, number, number];
    readonly wasmeditor_get_text: (a: number) => [number, number];
    readonly wasmeditor_get_word_suggestions_json: (a: number) => [number, number, number];
    readonly wasmeditor_handle_key_down: (a: number, b: any) => [number, number, number];
    readonly wasmeditor_handle_key_up: (a: number, b: any) => void;
    readonly wasmeditor_handle_mouse_down: (a: number, b: any) => [number, number];
    readonly wasmeditor_handle_mouse_move: (a: number, b: any) => [number, number];
    readonly wasmeditor_handle_mouse_up: (a: number, b: any) => [number, number];
    readonly wasmeditor_handle_touch_end: (a: number, b: any) => [number, number];
    readonly wasmeditor_handle_touch_move: (a: number, b: any) => [number, number];
    readonly wasmeditor_handle_touch_start: (a: number, b: any) => [number, number];
    readonly wasmeditor_handle_wheel: (a: number, b: any) => [number, number];
    readonly wasmeditor_insert_text: (a: number, b: number, c: number) => void;
    readonly wasmeditor_list_commands: (a: number) => [number, number, number];
    readonly wasmeditor_list_keybindings: (a: number) => [number, number, number, number];
    readonly wasmeditor_list_themes: (a: number) => [number, number, number, number];
    readonly wasmeditor_load_dictionary_text: (a: number, b: number, c: number) => void;
    readonly wasmeditor_measure_suggestions_popup: (a: number) => [number, number, number];
    readonly wasmeditor_needs_render: (a: number, b: number) => number;
    readonly wasmeditor_new: (a: number, b: number) => any;
    readonly wasmeditor_redo: (a: number) => number;
    readonly wasmeditor_render: (a: number, b: number) => [number, number];
    readonly wasmeditor_render_suggestions_popup: (a: number, b: number, c: number, d: number, e: number) => [number, number];
    readonly wasmeditor_reset_to_defaults: (a: number) => [number, number, number, number];
    readonly wasmeditor_run_command: (a: number, b: number, c: number, d: number, e: number) => [number, number, number];
    readonly wasmeditor_set_cursor_position: (a: number, b: number, c: number) => void;
    readonly wasmeditor_set_settings_json: (a: number, b: number, c: number) => [number, number];
    readonly wasmeditor_set_size: (a: number, b: number, c: number) => void;
    readonly wasmeditor_set_text: (a: number, b: number, c: number) => void;
    readonly wasmeditor_toggle_vertical: (a: number) => void;
    readonly wasmeditor_translit_init_canvas: (a: number, b: number, c: number, d: number, e: number) => [number, number];
    readonly wasmeditor_translit_is_loaded: (a: number) => number;
    readonly wasmeditor_translit_load_dictionary: (a: number, b: number, c: number) => any;
    readonly wasmeditor_translit_lookup: (a: number, b: number, c: number) => any;
    readonly wasmeditor_translit_render: (a: number, b: number, c: number, d: number, e: number) => [number, number];
    readonly wasmeditor_undo: (a: number) => number;
    readonly main: () => void;
    readonly wasm_bindgen__closure__destroy__h9fe21e8b023d8040: (a: number, b: number) => void;
    readonly wasm_bindgen__convert__closures_____invoke__h23f0b2cb1aaf7965: (a: number, b: number, c: any, d: any) => void;
    readonly wasm_bindgen__convert__closures_____invoke__h5ce99ad185dd8d06: (a: number, b: number, c: any) => void;
    readonly __wbindgen_malloc: (a: number, b: number) => number;
    readonly __wbindgen_realloc: (a: number, b: number, c: number, d: number) => number;
    readonly __wbindgen_exn_store: (a: number) => void;
    readonly __externref_table_alloc: () => number;
    readonly __wbindgen_externrefs: WebAssembly.Table;
    readonly __wbindgen_free: (a: number, b: number, c: number) => void;
    readonly __externref_table_dealloc: (a: number) => void;
    readonly __wbindgen_start: () => void;
}

export type SyncInitInput = BufferSource | WebAssembly.Module;

/**
 * Instantiates the given `module`, which can either be bytes or
 * a precompiled `WebAssembly.Module`.
 *
 * @param {{ module: SyncInitInput }} module - Passing `SyncInitInput` directly is deprecated.
 *
 * @returns {InitOutput}
 */
export function initSync(module: { module: SyncInitInput } | SyncInitInput): InitOutput;

/**
 * If `module_or_path` is {RequestInfo} or {URL}, makes a request and
 * for everything else, calls `WebAssembly.instantiate` directly.
 *
 * @param {{ module_or_path: InitInput | Promise<InitInput> }} module_or_path - Passing `InitInput` directly is deprecated.
 *
 * @returns {Promise<InitOutput>}
 */
export default function __wbg_init (module_or_path?: { module_or_path: InitInput | Promise<InitInput> } | InitInput | Promise<InitInput>): Promise<InitOutput>;
