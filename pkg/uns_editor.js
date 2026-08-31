/* @ts-self-types="./uns_editor.d.ts" */

export class WasmEditor {
    static __wrap(ptr) {
        ptr = ptr >>> 0;
        const obj = Object.create(WasmEditor.prototype);
        obj.__wbg_ptr = ptr;
        WasmEditorFinalization.register(obj, obj.__wbg_ptr, obj);
        return obj;
    }
    __destroy_into_raw() {
        const ptr = this.__wbg_ptr;
        this.__wbg_ptr = 0;
        WasmEditorFinalization.unregister(this);
        return ptr;
    }
    free() {
        const ptr = this.__destroy_into_raw();
        wasm.__wbg_wasmeditor_free(ptr, 0);
    }
    /**
     * Inserts the buffered NNBSP and the validated noun-case suffix after
     * the exact Bichig stem that triggered `get_word_suggestions_json`.
     * @param {string} text
     */
    accept_case_suffix(text) {
        const ptr0 = passStringToWasm0(text, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.wasmeditor_accept_case_suffix(this.__wbg_ptr, ptr0, len0);
        if (ret[1]) {
            throw takeFromExternrefTable0(ret[0]);
        }
    }
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
     * @param {string} text
     */
    accept_word_suggestion(text) {
        const ptr0 = passStringToWasm0(text, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.wasmeditor_accept_word_suggestion(this.__wbg_ptr, ptr0, len0);
        if (ret[1]) {
            throw takeFromExternrefTable0(ret[0]);
        }
    }
    /**
     * @returns {boolean}
     */
    can_redo() {
        const ret = wasm.wasmeditor_can_redo(this.__wbg_ptr);
        return ret !== 0;
    }
    /**
     * @returns {boolean}
     */
    can_undo() {
        const ret = wasm.wasmeditor_can_undo(this.__wbg_ptr);
        return ret !== 0;
    }
    delete_selection() {
        wasm.wasmeditor_delete_selection(this.__wbg_ptr);
    }
    /**
     * Clears the popup's state without changing the buffer - called on
     * Escape, on cursor-moving keys/clicks, and on document-changing
     * actions (tab switch, clear, open) per the trigger table in
     * `WORD_SUGGESTIONS_PLAN.md` §7.
     */
    dismiss_word_suggestions() {
        wasm.wasmeditor_dismiss_word_suggestions(this.__wbg_ptr);
    }
    /**
     * @returns {any}
     */
    get_cursor_position() {
        const ret = wasm.wasmeditor_get_cursor_position(this.__wbg_ptr);
        return ret;
    }
    /**
     * Returns the built-in default settings as JSON without mutating the
     * editor state. Useful for previewing defaults or seeding local UI.
     * @returns {string}
     */
    get_default_settings_json() {
        let deferred2_0;
        let deferred2_1;
        try {
            const ret = wasm.wasmeditor_get_default_settings_json(this.__wbg_ptr);
            var ptr1 = ret[0];
            var len1 = ret[1];
            if (ret[3]) {
                ptr1 = 0; len1 = 0;
                throw takeFromExternrefTable0(ret[2]);
            }
            deferred2_0 = ptr1;
            deferred2_1 = len1;
            return getStringFromWasm0(ptr1, len1);
        } finally {
            wasm.__wbindgen_free(deferred2_0, deferred2_1, 1);
        }
    }
    /**
     * @returns {any}
     */
    get_selected_text() {
        const ret = wasm.wasmeditor_get_selected_text(this.__wbg_ptr);
        return ret;
    }
    /**
     * @returns {string}
     */
    get_settings_json() {
        let deferred2_0;
        let deferred2_1;
        try {
            const ret = wasm.wasmeditor_get_settings_json(this.__wbg_ptr);
            var ptr1 = ret[0];
            var len1 = ret[1];
            if (ret[3]) {
                ptr1 = 0; len1 = 0;
                throw takeFromExternrefTable0(ret[2]);
            }
            deferred2_0 = ptr1;
            deferred2_1 = len1;
            return getStringFromWasm0(ptr1, len1);
        } finally {
            wasm.__wbindgen_free(deferred2_0, deferred2_1, 1);
        }
    }
    /**
     * @returns {string}
     */
    get_text() {
        let deferred1_0;
        let deferred1_1;
        try {
            const ret = wasm.wasmeditor_get_text(this.__wbg_ptr);
            deferred1_0 = ret[0];
            deferred1_1 = ret[1];
            return getStringFromWasm0(ret[0], ret[1]);
        } finally {
            wasm.__wbindgen_free(deferred1_0, deferred1_1, 1);
        }
    }
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
     * is disabled in settings, a dictionary is unavailable for ordinary
     * word completion, or the cursor isn't inside/adjacent to a word at
     * least `MIN_SUGGESTION_WORD_LEN` characters long - all "nothing to
     * show right now," not failure conditions. Bichig suffix requests do
     * not require the dictionary.
     * @param {boolean} suffix_requested
     * @returns {any}
     */
    get_word_suggestions_json(suffix_requested) {
        const ret = wasm.wasmeditor_get_word_suggestions_json(this.__wbg_ptr, suffix_requested);
        if (ret[2]) {
            throw takeFromExternrefTable0(ret[1]);
        }
        return takeFromExternrefTable0(ret[0]);
    }
    /**
     * Returns whether this keystroke actually changed the buffer text
     * (as opposed to just moving the cursor/selection, or doing
     * nothing). JS uses this to decide whether to refresh the word
     * suggestion popup or dismiss it - per the trigger table in
     * `WORD_SUGGESTIONS_PLAN.md` §7, pure cursor movement (arrow keys,
     * Home/End, PageUp/Down) should dismiss the popup (the word
     * context changed), not refresh it, since the user isn't actively
     * typing.
     * @param {KeyboardEvent} event
     * @returns {boolean}
     */
    handle_key_down(event) {
        const ret = wasm.wasmeditor_handle_key_down(this.__wbg_ptr, event);
        if (ret[2]) {
            throw takeFromExternrefTable0(ret[1]);
        }
        return ret[0] !== 0;
    }
    /**
     * @param {KeyboardEvent} event
     */
    handle_key_up(event) {
        wasm.wasmeditor_handle_key_up(this.__wbg_ptr, event);
    }
    /**
     * @param {MouseEvent} event
     */
    handle_mouse_down(event) {
        const ret = wasm.wasmeditor_handle_mouse_down(this.__wbg_ptr, event);
        if (ret[1]) {
            throw takeFromExternrefTable0(ret[0]);
        }
    }
    /**
     * @param {MouseEvent} event
     */
    handle_mouse_move(event) {
        const ret = wasm.wasmeditor_handle_mouse_move(this.__wbg_ptr, event);
        if (ret[1]) {
            throw takeFromExternrefTable0(ret[0]);
        }
    }
    /**
     * @param {MouseEvent} _event
     */
    handle_mouse_up(_event) {
        const ret = wasm.wasmeditor_handle_mouse_up(this.__wbg_ptr, _event);
        if (ret[1]) {
            throw takeFromExternrefTable0(ret[0]);
        }
    }
    /**
     * @param {TouchEvent} _event
     */
    handle_touch_end(_event) {
        const ret = wasm.wasmeditor_handle_touch_end(this.__wbg_ptr, _event);
        if (ret[1]) {
            throw takeFromExternrefTable0(ret[0]);
        }
    }
    /**
     * @param {TouchEvent} event
     */
    handle_touch_move(event) {
        const ret = wasm.wasmeditor_handle_touch_move(this.__wbg_ptr, event);
        if (ret[1]) {
            throw takeFromExternrefTable0(ret[0]);
        }
    }
    /**
     * @param {TouchEvent} event
     */
    handle_touch_start(event) {
        const ret = wasm.wasmeditor_handle_touch_start(this.__wbg_ptr, event);
        if (ret[1]) {
            throw takeFromExternrefTable0(ret[0]);
        }
    }
    /**
     * @param {WheelEvent} event
     */
    handle_wheel(event) {
        const ret = wasm.wasmeditor_handle_wheel(this.__wbg_ptr, event);
        if (ret[1]) {
            throw takeFromExternrefTable0(ret[0]);
        }
    }
    /**
     * @param {string} text
     */
    insert_text(text) {
        const ptr0 = passStringToWasm0(text, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        wasm.wasmeditor_insert_text(this.__wbg_ptr, ptr0, len0);
    }
    /**
     * Returns the list of registered commands as JSON:
     * `[{ "id": ..., "title": ..., "category": ...|null, "keybinding": ...|null }, ...]`
     * The order matches plugin registration order.
     * @returns {any}
     */
    list_commands() {
        const ret = wasm.wasmeditor_list_commands(this.__wbg_ptr);
        if (ret[2]) {
            throw takeFromExternrefTable0(ret[1]);
        }
        return takeFromExternrefTable0(ret[0]);
    }
    /**
     * Returns the default app-level keybindings (P6-01) as a JSON
     * array of `{ id, label, default }`. Does not mutate editor
     * state - the JS `KeybindingManager` layers localStorage overrides
     * on top and owns the actual `KeyboardEvent` matching; Rust is
     * only the source of truth for what ships out of the box. See
     * `src/config/keybindings.rs` for the full scope rationale (only
     * app-level shortcuts are covered, not low-level text-editing
     * keys like undo/redo).
     * @returns {string}
     */
    list_keybindings() {
        let deferred2_0;
        let deferred2_1;
        try {
            const ret = wasm.wasmeditor_list_keybindings(this.__wbg_ptr);
            var ptr1 = ret[0];
            var len1 = ret[1];
            if (ret[3]) {
                ptr1 = 0; len1 = 0;
                throw takeFromExternrefTable0(ret[2]);
            }
            deferred2_0 = ptr1;
            deferred2_1 = len1;
            return getStringFromWasm0(ptr1, len1);
        } finally {
            wasm.__wbindgen_free(deferred2_0, deferred2_1, 1);
        }
    }
    /**
     * Returns the built-in theme presets (P4-01) as a JSON array of
     * `{ id, name, appearance: { text_color, background_color, ... } }`.
     * Does not mutate editor state - the JS Color tab applies a theme by
     * merging the chosen entry's `appearance` into the current settings
     * and calling `set_settings_json`, the same "merge on save" pattern
     * used for every other settings field (see P0-02/P0-03).
     * @returns {string}
     */
    list_themes() {
        let deferred2_0;
        let deferred2_1;
        try {
            const ret = wasm.wasmeditor_list_themes(this.__wbg_ptr);
            var ptr1 = ret[0];
            var len1 = ret[1];
            if (ret[3]) {
                ptr1 = 0; len1 = 0;
                throw takeFromExternrefTable0(ret[2]);
            }
            deferred2_0 = ptr1;
            deferred2_1 = len1;
            return getStringFromWasm0(ptr1, len1);
        } finally {
            wasm.__wbindgen_free(deferred2_0, deferred2_1, 1);
        }
    }
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
     * @param {string} text
     */
    load_dictionary_text(text) {
        const ptr0 = passStringToWasm0(text, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        wasm.wasmeditor_load_dictionary_text(this.__wbg_ptr, ptr0, len0);
    }
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
     * @returns {any}
     */
    measure_suggestions_popup() {
        const ret = wasm.wasmeditor_measure_suggestions_popup(this.__wbg_ptr);
        if (ret[2]) {
            throw takeFromExternrefTable0(ret[1]);
        }
        return takeFromExternrefTable0(ret[0]);
    }
    /**
     * True if the JS `animate()` loop should actually call `render()`
     * this frame (P7-01). Two reasons to render: something the user
     * did (or a programmatic change) marked the frame dirty, or the
     * cursor's 500ms blink interval has elapsed - the latter check
     * mirrors the one `render()` itself does internally, so the
     * cursor keeps blinking at its usual rate even while otherwise
     * idle. Cheap to call every RAF tick: no pixel work, just a bool
     * and a float comparison.
     * @param {number} timestamp
     * @returns {boolean}
     */
    needs_render(timestamp) {
        const ret = wasm.wasmeditor_needs_render(this.__wbg_ptr, timestamp);
        return ret !== 0;
    }
    /**
     * @param {string} canvas_id
     * @returns {Promise<WasmEditor>}
     */
    static new(canvas_id) {
        const ptr0 = passStringToWasm0(canvas_id, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.wasmeditor_new(ptr0, len0);
        return ret;
    }
    /**
     * Redo the most recently undone change. Returns `true` if something
     * was redone.
     * @returns {boolean}
     */
    redo() {
        const ret = wasm.wasmeditor_redo(this.__wbg_ptr);
        return ret !== 0;
    }
    /**
     * @param {number} timestamp
     */
    render(timestamp) {
        const ret = wasm.wasmeditor_render(this.__wbg_ptr, timestamp);
        if (ret[1]) {
            throw takeFromExternrefTable0(ret[0]);
        }
    }
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
     * @param {string} canvas_id
     * @param {number} selected_index
     * @param {number} hover_index
     */
    render_suggestions_popup(canvas_id, selected_index, hover_index) {
        const ptr0 = passStringToWasm0(canvas_id, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.wasmeditor_render_suggestions_popup(this.__wbg_ptr, ptr0, len0, selected_index, hover_index);
        if (ret[1]) {
            throw takeFromExternrefTable0(ret[0]);
        }
    }
    /**
     * Resets settings to the built-in defaults, applies them, and returns the
     * resulting JSON so the JS layer can update its UI and localStorage
     * without duplicating the default values. See P0-01 in
     * `prompt/FUTURE_PLAN.md`.
     * @returns {string}
     */
    reset_to_defaults() {
        let deferred2_0;
        let deferred2_1;
        try {
            const ret = wasm.wasmeditor_reset_to_defaults(this.__wbg_ptr);
            var ptr1 = ret[0];
            var len1 = ret[1];
            if (ret[3]) {
                ptr1 = 0; len1 = 0;
                throw takeFromExternrefTable0(ret[2]);
            }
            deferred2_0 = ptr1;
            deferred2_1 = len1;
            return getStringFromWasm0(ptr1, len1);
        } finally {
            wasm.__wbindgen_free(deferred2_0, deferred2_1, 1);
        }
    }
    /**
     * Invoke a registered command by id. `args_json` is opaque to the
     * registry and forwarded to the handler untouched; pass `""` when
     * unused. Returns whatever the handler returns (typically
     * `JsValue::NULL`).
     *
     * Errors:
     * * `"Unknown command: <id>"` when no command matches.
     * * Any error the handler itself raises.
     * @param {string} id
     * @param {string} args_json
     * @returns {any}
     */
    run_command(id, args_json) {
        const ptr0 = passStringToWasm0(id, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ptr1 = passStringToWasm0(args_json, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len1 = WASM_VECTOR_LEN;
        const ret = wasm.wasmeditor_run_command(this.__wbg_ptr, ptr0, len0, ptr1, len1);
        if (ret[2]) {
            throw takeFromExternrefTable0(ret[1]);
        }
        return takeFromExternrefTable0(ret[0]);
    }
    /**
     * Move the cursor to a 1-based `(line, column)` position, the same
     * convention `get_cursor_position` returns. Used by auto-save draft
     * recovery (P2-05) to restore where the user was editing; clamps
     * silently to valid buffer bounds via cosmic-text's own cursor
     * handling rather than erroring on stale positions.
     * @param {number} line
     * @param {number} column
     */
    set_cursor_position(line, column) {
        wasm.wasmeditor_set_cursor_position(this.__wbg_ptr, line, column);
    }
    /**
     * @param {string} json
     */
    set_settings_json(json) {
        const ptr0 = passStringToWasm0(json, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.wasmeditor_set_settings_json(this.__wbg_ptr, ptr0, len0);
        if (ret[1]) {
            throw takeFromExternrefTable0(ret[0]);
        }
    }
    /**
     * @param {number} width
     * @param {number} height
     */
    set_size(width, height) {
        wasm.wasmeditor_set_size(this.__wbg_ptr, width, height);
    }
    /**
     * @param {string} text
     */
    set_text(text) {
        const ptr0 = passStringToWasm0(text, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        wasm.wasmeditor_set_text(this.__wbg_ptr, ptr0, len0);
    }
    toggle_vertical() {
        wasm.wasmeditor_toggle_vertical(this.__wbg_ptr);
    }
    /**
     * Sets up (or resizes) the offscreen translit renderer bound to the
     * given canvas element.
     * @param {string} canvas_id
     * @param {number} width
     * @param {number} height
     */
    translit_init_canvas(canvas_id, width, height) {
        const ptr0 = passStringToWasm0(canvas_id, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.wasmeditor_translit_init_canvas(this.__wbg_ptr, ptr0, len0, width, height);
        if (ret[1]) {
            throw takeFromExternrefTable0(ret[0]);
        }
    }
    /**
     * Returns true if the dictionary has been loaded.
     * @returns {boolean}
     */
    translit_is_loaded() {
        const ret = wasm.wasmeditor_translit_is_loaded(this.__wbg_ptr);
        return ret !== 0;
    }
    /**
     * Fetches and parses the dictionary TSV. Safe to call multiple times; the
     * actual network + parse work happens only on the first call. Subsequent
     * calls resolve immediately.
     * @param {string} url
     * @returns {Promise<void>}
     */
    translit_load_dictionary(url) {
        const ptr0 = passStringToWasm0(url, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.wasmeditor_translit_load_dictionary(this.__wbg_ptr, ptr0, len0);
        return ret;
    }
    /**
     * Looks up a Cyrillic word and returns
     * `{ found: bool, mongolian: string | null, variants: number }`.
     * @param {string} cyrillic
     * @returns {any}
     */
    translit_lookup(cyrillic) {
        const ptr0 = passStringToWasm0(cyrillic, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.wasmeditor_translit_lookup(this.__wbg_ptr, ptr0, len0);
        return ret;
    }
    /**
     * Renders the given Mongolian text vertically onto the translit canvas.
     * @param {string} canvas_id
     * @param {string} text
     */
    translit_render(canvas_id, text) {
        const ptr0 = passStringToWasm0(canvas_id, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ptr1 = passStringToWasm0(text, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len1 = WASM_VECTOR_LEN;
        const ret = wasm.wasmeditor_translit_render(this.__wbg_ptr, ptr0, len0, ptr1, len1);
        if (ret[1]) {
            throw takeFromExternrefTable0(ret[0]);
        }
    }
    /**
     * Undo the most recent change. Returns `true` if something was
     * undone. Bound to `Ctrl+Z` in `handle_key_down`; also exposed here
     * for the command palette / toolbar.
     * @returns {boolean}
     */
    undo() {
        const ret = wasm.wasmeditor_undo(this.__wbg_ptr);
        return ret !== 0;
    }
}
if (Symbol.dispose) WasmEditor.prototype[Symbol.dispose] = WasmEditor.prototype.free;

export function main() {
    wasm.main();
}

function __wbg_get_imports() {
    const import0 = {
        __proto__: null,
        __wbg___wbindgen_debug_string_0bc8482c6e3508ae: function(arg0, arg1) {
            const ret = debugString(arg1);
            const ptr1 = passStringToWasm0(ret, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
            const len1 = WASM_VECTOR_LEN;
            getDataViewMemory0().setInt32(arg0 + 4 * 1, len1, true);
            getDataViewMemory0().setInt32(arg0 + 4 * 0, ptr1, true);
        },
        __wbg___wbindgen_is_function_0095a73b8b156f76: function(arg0) {
            const ret = typeof(arg0) === 'function';
            return ret;
        },
        __wbg___wbindgen_is_undefined_9e4d92534c42d778: function(arg0) {
            const ret = arg0 === undefined;
            return ret;
        },
        __wbg___wbindgen_string_get_72fb696202c56729: function(arg0, arg1) {
            const obj = arg1;
            const ret = typeof(obj) === 'string' ? obj : undefined;
            var ptr1 = isLikeNone(ret) ? 0 : passStringToWasm0(ret, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
            var len1 = WASM_VECTOR_LEN;
            getDataViewMemory0().setInt32(arg0 + 4 * 1, len1, true);
            getDataViewMemory0().setInt32(arg0 + 4 * 0, ptr1, true);
        },
        __wbg___wbindgen_throw_be289d5034ed271b: function(arg0, arg1) {
            throw new Error(getStringFromWasm0(arg0, arg1));
        },
        __wbg__wbg_cb_unref_d9b87ff7982e3b21: function(arg0) {
            arg0._wbg_cb_unref();
        },
        __wbg_altKey_73c1173ba53073d5: function(arg0) {
            const ret = arg0.altKey;
            return ret;
        },
        __wbg_arrayBuffer_bb54076166006c39: function() { return handleError(function (arg0) {
            const ret = arg0.arrayBuffer();
            return ret;
        }, arguments); },
        __wbg_call_389efe28435a9388: function() { return handleError(function (arg0, arg1) {
            const ret = arg0.call(arg1);
            return ret;
        }, arguments); },
        __wbg_call_4708e0c13bdc8e95: function() { return handleError(function (arg0, arg1, arg2) {
            const ret = arg0.call(arg1, arg2);
            return ret;
        }, arguments); },
        __wbg_clientX_a3c5f4ff30e91264: function(arg0) {
            const ret = arg0.clientX;
            return ret;
        },
        __wbg_clientX_ed7d2827ca30c165: function(arg0) {
            const ret = arg0.clientX;
            return ret;
        },
        __wbg_clientY_79ab4711d0597b2c: function(arg0) {
            const ret = arg0.clientY;
            return ret;
        },
        __wbg_clientY_e28509acb9b4a42a: function(arg0) {
            const ret = arg0.clientY;
            return ret;
        },
        __wbg_ctrlKey_09a1b54d77dea92b: function(arg0) {
            const ret = arg0.ctrlKey;
            return ret;
        },
        __wbg_deltaX_f0ca9116db5f7bc1: function(arg0) {
            const ret = arg0.deltaX;
            return ret;
        },
        __wbg_deltaY_eb94120160ac821c: function(arg0) {
            const ret = arg0.deltaY;
            return ret;
        },
        __wbg_devicePixelRatio_5c458affc89fc209: function(arg0) {
            const ret = arg0.devicePixelRatio;
            return ret;
        },
        __wbg_document_ee35a3d3ae34ef6c: function(arg0) {
            const ret = arg0.document;
            return isLikeNone(ret) ? 0 : addToExternrefTable0(ret);
        },
        __wbg_error_7534b8e9a36f1ab4: function(arg0, arg1) {
            let deferred0_0;
            let deferred0_1;
            try {
                deferred0_0 = arg0;
                deferred0_1 = arg1;
                console.error(getStringFromWasm0(arg0, arg1));
            } finally {
                wasm.__wbindgen_free(deferred0_0, deferred0_1, 1);
            }
        },
        __wbg_fetch_4f06ca81d87798ba: function(arg0, arg1, arg2) {
            const ret = arg0.fetch(getStringFromWasm0(arg1, arg2));
            return ret;
        },
        __wbg_fillRect_d44afec47e3a3fab: function(arg0, arg1, arg2, arg3, arg4) {
            arg0.fillRect(arg1, arg2, arg3, arg4);
        },
        __wbg_fillText_4a931850b976cc62: function() { return handleError(function (arg0, arg1, arg2, arg3, arg4) {
            arg0.fillText(getStringFromWasm0(arg1, arg2), arg3, arg4);
        }, arguments); },
        __wbg_getBoundingClientRect_b5c8c34d07878818: function(arg0) {
            const ret = arg0.getBoundingClientRect();
            return ret;
        },
        __wbg_getContext_2a5764d48600bc43: function() { return handleError(function (arg0, arg1, arg2) {
            const ret = arg0.getContext(getStringFromWasm0(arg1, arg2));
            return isLikeNone(ret) ? 0 : addToExternrefTable0(ret);
        }, arguments); },
        __wbg_getElementById_e34377b79d7285f6: function(arg0, arg1, arg2) {
            const ret = arg0.getElementById(getStringFromWasm0(arg1, arg2));
            return isLikeNone(ret) ? 0 : addToExternrefTable0(ret);
        },
        __wbg_height_38750dc6de41ee75: function(arg0) {
            const ret = arg0.height;
            return ret;
        },
        __wbg_instanceof_CanvasRenderingContext2d_4bb052fd1c3d134d: function(arg0) {
            let result;
            try {
                result = arg0 instanceof CanvasRenderingContext2D;
            } catch (_) {
                result = false;
            }
            const ret = result;
            return ret;
        },
        __wbg_instanceof_HtmlCanvasElement_3f2f6e1edb1c9792: function(arg0) {
            let result;
            try {
                result = arg0 instanceof HTMLCanvasElement;
            } catch (_) {
                result = false;
            }
            const ret = result;
            return ret;
        },
        __wbg_instanceof_Response_ee1d54d79ae41977: function(arg0) {
            let result;
            try {
                result = arg0 instanceof Response;
            } catch (_) {
                result = false;
            }
            const ret = result;
            return ret;
        },
        __wbg_instanceof_Window_ed49b2db8df90359: function(arg0) {
            let result;
            try {
                result = arg0 instanceof Window;
            } catch (_) {
                result = false;
            }
            const ret = result;
            return ret;
        },
        __wbg_item_98b174cdde606b25: function(arg0, arg1) {
            const ret = arg0.item(arg1 >>> 0);
            return isLikeNone(ret) ? 0 : addToExternrefTable0(ret);
        },
        __wbg_key_d41e8e825e6bb0e9: function(arg0, arg1) {
            const ret = arg1.key;
            const ptr1 = passStringToWasm0(ret, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
            const len1 = WASM_VECTOR_LEN;
            getDataViewMemory0().setInt32(arg0 + 4 * 1, len1, true);
            getDataViewMemory0().setInt32(arg0 + 4 * 0, ptr1, true);
        },
        __wbg_left_3b7c3c1030d5ca7a: function(arg0) {
            const ret = arg0.left;
            return ret;
        },
        __wbg_length_32ed9a279acd054c: function(arg0) {
            const ret = arg0.length;
            return ret;
        },
        __wbg_log_6b5ca2e6124b2808: function(arg0) {
            console.log(arg0);
        },
        __wbg_metaKey_67113fb40365d736: function(arg0) {
            const ret = arg0.metaKey;
            return ret;
        },
        __wbg_new_361308b2356cecd0: function() {
            const ret = new Object();
            return ret;
        },
        __wbg_new_3eb36ae241fe6f44: function() {
            const ret = new Array();
            return ret;
        },
        __wbg_new_8a6f238a6ece86ea: function() {
            const ret = new Error();
            return ret;
        },
        __wbg_new_b5d9e2fb389fef91: function(arg0, arg1) {
            try {
                var state0 = {a: arg0, b: arg1};
                var cb0 = (arg0, arg1) => {
                    const a = state0.a;
                    state0.a = 0;
                    try {
                        return wasm_bindgen__convert__closures_____invoke__h23f0b2cb1aaf7965(a, state0.b, arg0, arg1);
                    } finally {
                        state0.a = a;
                    }
                };
                const ret = new Promise(cb0);
                return ret;
            } finally {
                state0.a = state0.b = 0;
            }
        },
        __wbg_new_dd2b680c8bf6ae29: function(arg0) {
            const ret = new Uint8Array(arg0);
            return ret;
        },
        __wbg_new_no_args_1c7c842f08d00ebb: function(arg0, arg1) {
            const ret = new Function(getStringFromWasm0(arg0, arg1));
            return ret;
        },
        __wbg_new_with_u8_clamped_array_and_sh_0c0b789ceb2eab31: function() { return handleError(function (arg0, arg1, arg2, arg3) {
            const ret = new ImageData(getClampedArrayU8FromWasm0(arg0, arg1), arg2 >>> 0, arg3 >>> 0);
            return ret;
        }, arguments); },
        __wbg_now_a3af9a2f4bbaa4d1: function() {
            const ret = Date.now();
            return ret;
        },
        __wbg_ok_87f537440a0acf85: function(arg0) {
            const ret = arg0.ok;
            return ret;
        },
        __wbg_preventDefault_cdcfcd7e301b9702: function(arg0) {
            arg0.preventDefault();
        },
        __wbg_prototypesetcall_bdcdcc5842e4d77d: function(arg0, arg1, arg2) {
            Uint8Array.prototype.set.call(getArrayU8FromWasm0(arg0, arg1), arg2);
        },
        __wbg_push_8ffdcb2063340ba5: function(arg0, arg1) {
            const ret = arg0.push(arg1);
            return ret;
        },
        __wbg_putImageData_78318465ad96c2c3: function() { return handleError(function (arg0, arg1, arg2, arg3) {
            arg0.putImageData(arg1, arg2, arg3);
        }, arguments); },
        __wbg_queueMicrotask_0aa0a927f78f5d98: function(arg0) {
            const ret = arg0.queueMicrotask;
            return ret;
        },
        __wbg_queueMicrotask_5bb536982f78a56f: function(arg0) {
            queueMicrotask(arg0);
        },
        __wbg_random_912284dbf636f269: function() {
            const ret = Math.random();
            return ret;
        },
        __wbg_resolve_002c4b7d9d8f6b64: function(arg0) {
            const ret = Promise.resolve(arg0);
            return ret;
        },
        __wbg_set_6cb8631f80447a67: function() { return handleError(function (arg0, arg1, arg2) {
            const ret = Reflect.set(arg0, arg1, arg2);
            return ret;
        }, arguments); },
        __wbg_set_fillStyle_783d3f7489475421: function(arg0, arg1, arg2) {
            arg0.fillStyle = getStringFromWasm0(arg1, arg2);
        },
        __wbg_set_font_575685c8f7e56957: function(arg0, arg1, arg2) {
            arg0.font = getStringFromWasm0(arg1, arg2);
        },
        __wbg_set_height_f21f985387070100: function(arg0, arg1) {
            arg0.height = arg1 >>> 0;
        },
        __wbg_set_textAlign_cdfa5b9f1c14f5c6: function(arg0, arg1, arg2) {
            arg0.textAlign = getStringFromWasm0(arg1, arg2);
        },
        __wbg_set_textBaseline_c7ec6538cc52b073: function(arg0, arg1, arg2) {
            arg0.textBaseline = getStringFromWasm0(arg1, arg2);
        },
        __wbg_set_width_d60bc4f2f20c56a4: function(arg0, arg1) {
            arg0.width = arg1 >>> 0;
        },
        __wbg_shiftKey_564be91ec842bcc4: function(arg0) {
            const ret = arg0.shiftKey;
            return ret;
        },
        __wbg_stack_0ed75d68575b0f3c: function(arg0, arg1) {
            const ret = arg1.stack;
            const ptr1 = passStringToWasm0(ret, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
            const len1 = WASM_VECTOR_LEN;
            getDataViewMemory0().setInt32(arg0 + 4 * 1, len1, true);
            getDataViewMemory0().setInt32(arg0 + 4 * 0, ptr1, true);
        },
        __wbg_static_accessor_GLOBAL_12837167ad935116: function() {
            const ret = typeof global === 'undefined' ? null : global;
            return isLikeNone(ret) ? 0 : addToExternrefTable0(ret);
        },
        __wbg_static_accessor_GLOBAL_THIS_e628e89ab3b1c95f: function() {
            const ret = typeof globalThis === 'undefined' ? null : globalThis;
            return isLikeNone(ret) ? 0 : addToExternrefTable0(ret);
        },
        __wbg_static_accessor_SELF_a621d3dfbb60d0ce: function() {
            const ret = typeof self === 'undefined' ? null : self;
            return isLikeNone(ret) ? 0 : addToExternrefTable0(ret);
        },
        __wbg_static_accessor_WINDOW_f8727f0cf888e0bd: function() {
            const ret = typeof window === 'undefined' ? null : window;
            return isLikeNone(ret) ? 0 : addToExternrefTable0(ret);
        },
        __wbg_status_89d7e803db911ee7: function(arg0) {
            const ret = arg0.status;
            return ret;
        },
        __wbg_target_521be630ab05b11e: function(arg0) {
            const ret = arg0.target;
            return isLikeNone(ret) ? 0 : addToExternrefTable0(ret);
        },
        __wbg_text_083b8727c990c8c0: function() { return handleError(function (arg0) {
            const ret = arg0.text();
            return ret;
        }, arguments); },
        __wbg_then_0d9fe2c7b1857d32: function(arg0, arg1, arg2) {
            const ret = arg0.then(arg1, arg2);
            return ret;
        },
        __wbg_then_b9e7b3b5f1a9e1b5: function(arg0, arg1) {
            const ret = arg0.then(arg1);
            return ret;
        },
        __wbg_timeStamp_ddfe1bf5c2346d68: function(arg0) {
            const ret = arg0.timeStamp;
            return ret;
        },
        __wbg_top_3d27ff6f468cf3fc: function(arg0) {
            const ret = arg0.top;
            return ret;
        },
        __wbg_touches_55ce167b42bcdf52: function(arg0) {
            const ret = arg0.touches;
            return ret;
        },
        __wbg_wasmeditor_new: function(arg0) {
            const ret = WasmEditor.__wrap(arg0);
            return ret;
        },
        __wbg_width_5f66bde2e810fbde: function(arg0) {
            const ret = arg0.width;
            return ret;
        },
        __wbindgen_cast_0000000000000001: function(arg0, arg1) {
            // Cast intrinsic for `Closure(Closure { dtor_idx: 294, function: Function { arguments: [Externref], shim_idx: 295, ret: Unit, inner_ret: Some(Unit) }, mutable: true }) -> Externref`.
            const ret = makeMutClosure(arg0, arg1, wasm.wasm_bindgen__closure__destroy__h9fe21e8b023d8040, wasm_bindgen__convert__closures_____invoke__h5ce99ad185dd8d06);
            return ret;
        },
        __wbindgen_cast_0000000000000002: function(arg0) {
            // Cast intrinsic for `F64 -> Externref`.
            const ret = arg0;
            return ret;
        },
        __wbindgen_cast_0000000000000003: function(arg0, arg1) {
            // Cast intrinsic for `Ref(String) -> Externref`.
            const ret = getStringFromWasm0(arg0, arg1);
            return ret;
        },
        __wbindgen_init_externref_table: function() {
            const table = wasm.__wbindgen_externrefs;
            const offset = table.grow(4);
            table.set(0, undefined);
            table.set(offset + 0, undefined);
            table.set(offset + 1, null);
            table.set(offset + 2, true);
            table.set(offset + 3, false);
        },
    };
    return {
        __proto__: null,
        "./uns_editor_bg.js": import0,
    };
}

function wasm_bindgen__convert__closures_____invoke__h5ce99ad185dd8d06(arg0, arg1, arg2) {
    wasm.wasm_bindgen__convert__closures_____invoke__h5ce99ad185dd8d06(arg0, arg1, arg2);
}

function wasm_bindgen__convert__closures_____invoke__h23f0b2cb1aaf7965(arg0, arg1, arg2, arg3) {
    wasm.wasm_bindgen__convert__closures_____invoke__h23f0b2cb1aaf7965(arg0, arg1, arg2, arg3);
}

const WasmEditorFinalization = (typeof FinalizationRegistry === 'undefined')
    ? { register: () => {}, unregister: () => {} }
    : new FinalizationRegistry(ptr => wasm.__wbg_wasmeditor_free(ptr >>> 0, 1));

function addToExternrefTable0(obj) {
    const idx = wasm.__externref_table_alloc();
    wasm.__wbindgen_externrefs.set(idx, obj);
    return idx;
}

const CLOSURE_DTORS = (typeof FinalizationRegistry === 'undefined')
    ? { register: () => {}, unregister: () => {} }
    : new FinalizationRegistry(state => state.dtor(state.a, state.b));

function debugString(val) {
    // primitive types
    const type = typeof val;
    if (type == 'number' || type == 'boolean' || val == null) {
        return  `${val}`;
    }
    if (type == 'string') {
        return `"${val}"`;
    }
    if (type == 'symbol') {
        const description = val.description;
        if (description == null) {
            return 'Symbol';
        } else {
            return `Symbol(${description})`;
        }
    }
    if (type == 'function') {
        const name = val.name;
        if (typeof name == 'string' && name.length > 0) {
            return `Function(${name})`;
        } else {
            return 'Function';
        }
    }
    // objects
    if (Array.isArray(val)) {
        const length = val.length;
        let debug = '[';
        if (length > 0) {
            debug += debugString(val[0]);
        }
        for(let i = 1; i < length; i++) {
            debug += ', ' + debugString(val[i]);
        }
        debug += ']';
        return debug;
    }
    // Test for built-in
    const builtInMatches = /\[object ([^\]]+)\]/.exec(toString.call(val));
    let className;
    if (builtInMatches && builtInMatches.length > 1) {
        className = builtInMatches[1];
    } else {
        // Failed to match the standard '[object ClassName]'
        return toString.call(val);
    }
    if (className == 'Object') {
        // we're a user defined class or Object
        // JSON.stringify avoids problems with cycles, and is generally much
        // easier than looping through ownProperties of `val`.
        try {
            return 'Object(' + JSON.stringify(val) + ')';
        } catch (_) {
            return 'Object';
        }
    }
    // errors
    if (val instanceof Error) {
        return `${val.name}: ${val.message}\n${val.stack}`;
    }
    // TODO we could test for more things here, like `Set`s and `Map`s.
    return className;
}

function getArrayU8FromWasm0(ptr, len) {
    ptr = ptr >>> 0;
    return getUint8ArrayMemory0().subarray(ptr / 1, ptr / 1 + len);
}

function getClampedArrayU8FromWasm0(ptr, len) {
    ptr = ptr >>> 0;
    return getUint8ClampedArrayMemory0().subarray(ptr / 1, ptr / 1 + len);
}

let cachedDataViewMemory0 = null;
function getDataViewMemory0() {
    if (cachedDataViewMemory0 === null || cachedDataViewMemory0.buffer.detached === true || (cachedDataViewMemory0.buffer.detached === undefined && cachedDataViewMemory0.buffer !== wasm.memory.buffer)) {
        cachedDataViewMemory0 = new DataView(wasm.memory.buffer);
    }
    return cachedDataViewMemory0;
}

function getStringFromWasm0(ptr, len) {
    ptr = ptr >>> 0;
    return decodeText(ptr, len);
}

let cachedUint8ArrayMemory0 = null;
function getUint8ArrayMemory0() {
    if (cachedUint8ArrayMemory0 === null || cachedUint8ArrayMemory0.byteLength === 0) {
        cachedUint8ArrayMemory0 = new Uint8Array(wasm.memory.buffer);
    }
    return cachedUint8ArrayMemory0;
}

let cachedUint8ClampedArrayMemory0 = null;
function getUint8ClampedArrayMemory0() {
    if (cachedUint8ClampedArrayMemory0 === null || cachedUint8ClampedArrayMemory0.byteLength === 0) {
        cachedUint8ClampedArrayMemory0 = new Uint8ClampedArray(wasm.memory.buffer);
    }
    return cachedUint8ClampedArrayMemory0;
}

function handleError(f, args) {
    try {
        return f.apply(this, args);
    } catch (e) {
        const idx = addToExternrefTable0(e);
        wasm.__wbindgen_exn_store(idx);
    }
}

function isLikeNone(x) {
    return x === undefined || x === null;
}

function makeMutClosure(arg0, arg1, dtor, f) {
    const state = { a: arg0, b: arg1, cnt: 1, dtor };
    const real = (...args) => {

        // First up with a closure we increment the internal reference
        // count. This ensures that the Rust closure environment won't
        // be deallocated while we're invoking it.
        state.cnt++;
        const a = state.a;
        state.a = 0;
        try {
            return f(a, state.b, ...args);
        } finally {
            state.a = a;
            real._wbg_cb_unref();
        }
    };
    real._wbg_cb_unref = () => {
        if (--state.cnt === 0) {
            state.dtor(state.a, state.b);
            state.a = 0;
            CLOSURE_DTORS.unregister(state);
        }
    };
    CLOSURE_DTORS.register(real, state, state);
    return real;
}

function passStringToWasm0(arg, malloc, realloc) {
    if (realloc === undefined) {
        const buf = cachedTextEncoder.encode(arg);
        const ptr = malloc(buf.length, 1) >>> 0;
        getUint8ArrayMemory0().subarray(ptr, ptr + buf.length).set(buf);
        WASM_VECTOR_LEN = buf.length;
        return ptr;
    }

    let len = arg.length;
    let ptr = malloc(len, 1) >>> 0;

    const mem = getUint8ArrayMemory0();

    let offset = 0;

    for (; offset < len; offset++) {
        const code = arg.charCodeAt(offset);
        if (code > 0x7F) break;
        mem[ptr + offset] = code;
    }
    if (offset !== len) {
        if (offset !== 0) {
            arg = arg.slice(offset);
        }
        ptr = realloc(ptr, len, len = offset + arg.length * 3, 1) >>> 0;
        const view = getUint8ArrayMemory0().subarray(ptr + offset, ptr + len);
        const ret = cachedTextEncoder.encodeInto(arg, view);

        offset += ret.written;
        ptr = realloc(ptr, len, offset, 1) >>> 0;
    }

    WASM_VECTOR_LEN = offset;
    return ptr;
}

function takeFromExternrefTable0(idx) {
    const value = wasm.__wbindgen_externrefs.get(idx);
    wasm.__externref_table_dealloc(idx);
    return value;
}

let cachedTextDecoder = new TextDecoder('utf-8', { ignoreBOM: true, fatal: true });
cachedTextDecoder.decode();
const MAX_SAFARI_DECODE_BYTES = 2146435072;
let numBytesDecoded = 0;
function decodeText(ptr, len) {
    numBytesDecoded += len;
    if (numBytesDecoded >= MAX_SAFARI_DECODE_BYTES) {
        cachedTextDecoder = new TextDecoder('utf-8', { ignoreBOM: true, fatal: true });
        cachedTextDecoder.decode();
        numBytesDecoded = len;
    }
    return cachedTextDecoder.decode(getUint8ArrayMemory0().subarray(ptr, ptr + len));
}

const cachedTextEncoder = new TextEncoder();

if (!('encodeInto' in cachedTextEncoder)) {
    cachedTextEncoder.encodeInto = function (arg, view) {
        const buf = cachedTextEncoder.encode(arg);
        view.set(buf);
        return {
            read: arg.length,
            written: buf.length
        };
    };
}

let WASM_VECTOR_LEN = 0;

let wasmModule, wasm;
function __wbg_finalize_init(instance, module) {
    wasm = instance.exports;
    wasmModule = module;
    cachedDataViewMemory0 = null;
    cachedUint8ArrayMemory0 = null;
    cachedUint8ClampedArrayMemory0 = null;
    wasm.__wbindgen_start();
    return wasm;
}

async function __wbg_load(module, imports) {
    if (typeof Response === 'function' && module instanceof Response) {
        if (typeof WebAssembly.instantiateStreaming === 'function') {
            try {
                return await WebAssembly.instantiateStreaming(module, imports);
            } catch (e) {
                const validResponse = module.ok && expectedResponseType(module.type);

                if (validResponse && module.headers.get('Content-Type') !== 'application/wasm') {
                    console.warn("`WebAssembly.instantiateStreaming` failed because your server does not serve Wasm with `application/wasm` MIME type. Falling back to `WebAssembly.instantiate` which is slower. Original error:\n", e);

                } else { throw e; }
            }
        }

        const bytes = await module.arrayBuffer();
        return await WebAssembly.instantiate(bytes, imports);
    } else {
        const instance = await WebAssembly.instantiate(module, imports);

        if (instance instanceof WebAssembly.Instance) {
            return { instance, module };
        } else {
            return instance;
        }
    }

    function expectedResponseType(type) {
        switch (type) {
            case 'basic': case 'cors': case 'default': return true;
        }
        return false;
    }
}

function initSync(module) {
    if (wasm !== undefined) return wasm;


    if (module !== undefined) {
        if (Object.getPrototypeOf(module) === Object.prototype) {
            ({module} = module)
        } else {
            console.warn('using deprecated parameters for `initSync()`; pass a single object instead')
        }
    }

    const imports = __wbg_get_imports();
    if (!(module instanceof WebAssembly.Module)) {
        module = new WebAssembly.Module(module);
    }
    const instance = new WebAssembly.Instance(module, imports);
    return __wbg_finalize_init(instance, module);
}

async function __wbg_init(module_or_path) {
    if (wasm !== undefined) return wasm;


    if (module_or_path !== undefined) {
        if (Object.getPrototypeOf(module_or_path) === Object.prototype) {
            ({module_or_path} = module_or_path)
        } else {
            console.warn('using deprecated parameters for the initialization function; pass a single object instead')
        }
    }

    if (module_or_path === undefined) {
        module_or_path = new URL('uns_editor_bg.wasm', import.meta.url);
    }
    const imports = __wbg_get_imports();

    if (typeof module_or_path === 'string' || (typeof Request === 'function' && module_or_path instanceof Request) || (typeof URL === 'function' && module_or_path instanceof URL)) {
        module_or_path = fetch(module_or_path);
    }

    const { instance, module } = await __wbg_load(await module_or_path, imports);

    return __wbg_finalize_init(instance, module);
}

export { initSync, __wbg_init as default };
