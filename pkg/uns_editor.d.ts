/* tslint:disable */
/* eslint-disable */

export class WasmEditor {
    private constructor();
    free(): void;
    [Symbol.dispose](): void;
    can_redo(): boolean;
    can_undo(): boolean;
    delete_selection(): void;
    get_cursor_position(): any;
    /**
     * Returns the built-in default settings as JSON without mutating the
     * editor state. Useful for previewing defaults or seeding local UI.
     */
    get_default_settings_json(): string;
    get_selected_text(): any;
    get_settings_json(): string;
    get_text(): string;
    handle_key_down(event: KeyboardEvent): void;
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
     * Returns the built-in theme presets (P4-01) as a JSON array of
     * `{ id, name, appearance: { text_color, background_color, ... } }`.
     * Does not mutate editor state - the JS Color tab applies a theme by
     * merging the chosen entry's `appearance` into the current settings
     * and calling `set_settings_json`, the same "merge on save" pattern
     * used for every other settings field (see P0-02/P0-03).
     */
    list_themes(): string;
    static new(canvas_id: string): Promise<WasmEditor>;
    /**
     * Redo the most recently undone change. Returns `true` if something
     * was redone.
     */
    redo(): boolean;
    render(timestamp: number): void;
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
    readonly wasmeditor_can_redo: (a: number) => number;
    readonly wasmeditor_can_undo: (a: number) => number;
    readonly wasmeditor_delete_selection: (a: number) => void;
    readonly wasmeditor_get_cursor_position: (a: number) => any;
    readonly wasmeditor_get_default_settings_json: (a: number) => [number, number, number, number];
    readonly wasmeditor_get_selected_text: (a: number) => any;
    readonly wasmeditor_get_settings_json: (a: number) => [number, number, number, number];
    readonly wasmeditor_get_text: (a: number) => [number, number];
    readonly wasmeditor_handle_key_down: (a: number, b: any) => [number, number];
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
    readonly wasmeditor_list_themes: (a: number) => [number, number, number, number];
    readonly wasmeditor_new: (a: number, b: number) => any;
    readonly wasmeditor_redo: (a: number) => number;
    readonly wasmeditor_render: (a: number, b: number) => [number, number];
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
