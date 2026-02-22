/* tslint:disable */
/* eslint-disable */

export class WasmEditor {
    private constructor();
    free(): void;
    [Symbol.dispose](): void;
    get_cursor_position(): any;
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
    static new(canvas_id: string): Promise<WasmEditor>;
    render(timestamp: number): void;
    set_settings_json(json: string): void;
    set_size(width: number, height: number): void;
    set_text(text: string): void;
    toggle_vertical(): void;
}

export function main(): void;

export type InitInput = RequestInfo | URL | Response | BufferSource | WebAssembly.Module;

export interface InitOutput {
    readonly memory: WebAssembly.Memory;
    readonly __wbg_wasmeditor_free: (a: number, b: number) => void;
    readonly wasmeditor_get_cursor_position: (a: number) => any;
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
    readonly wasmeditor_new: (a: number, b: number) => any;
    readonly wasmeditor_render: (a: number, b: number) => [number, number];
    readonly wasmeditor_set_settings_json: (a: number, b: number, c: number) => [number, number];
    readonly wasmeditor_set_size: (a: number, b: number, c: number) => void;
    readonly wasmeditor_set_text: (a: number, b: number, c: number) => void;
    readonly wasmeditor_toggle_vertical: (a: number) => void;
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
