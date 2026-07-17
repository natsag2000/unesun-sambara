# UNS Editor Plugin API

Status: **experimental**, Phase P1 of `prompt/FUTURE_PLAN.md`.

The plugin API is compiled-in only. Plugins are Rust code that link into
the WASM binary. There is no dynamic loading (JS/WASM/Wasm-Component)
yet — that is deliberately deferred until the compiled-in surface has
been shaken out by real features in P2 and beyond.

## What a plugin can do

| Capability                                      | Available in P1 | Comes later     |
|-------------------------------------------------|:---------------:|:---------------:|
| Register commands (invokable by id)             |       yes       |                 |
| Advertise a display keybinding hint             |       yes       |                 |
| React to editor events                          |       yes       |                 |
| Show up in the command palette (`Ctrl+Shift+P`) |       yes       |                 |
| Own persistent settings                         |                 | P4 / P6         |
| Register keybindings that actually route input  |                 | P6-01           |
| Contribute rendering decorations                |                 | P3-01 (spike)   |
| Load at runtime from JS                         |                 | not planned     |

## Modules

* `src/editor_core/events.rs` — the event bus and `EditorEvent` enum.
* `src/editor_core/plugin.rs` — the `Plugin` trait, `PluginRegistry`,
  `PluginContext`, and the built-in `CoreCommandsPlugin`.
* `src/lib.rs` — WASM entry points `list_commands()` and
  `run_command(id, args_json)`.
* `index.html` — the `CommandPalette` class (Tailwind modal, fuzzy
  matcher, `Ctrl+Shift+P` binding).

## Event catalog

`EditorEvent` variants dispatched today:

| Variant                              | Payload                          | When it fires                                                    |
|--------------------------------------|----------------------------------|------------------------------------------------------------------|
| `TextChanged`                        | —                                | After any keystroke that mutated text; `insert_text`; `delete_selection`. |
| `CursorMoved`                        | —                                | Motion keys; mouse click; drag.                                  |
| `SelectionChanged`                   | —                                | Reserved (not yet dispatched — consumers should still handle it). |
| `SettingsChanged`                    | —                                | `set_settings_json`; `reset_to_defaults`; `toggle_vertical`; any `run_command`. |
| `KeyPressed(KeyInfo)`                | `{ key, ctrl, shift, alt, meta }`| Fired at the top of `handle_key_down`, before Rust acts.         |
| `DocumentLoaded { name }`            | optional filename                | `set_text` currently emits this with `name: None`.               |
| `DocumentSaved { name }`             | optional filename                | Reserved (not yet dispatched — save flow lives in JS today).     |
| `OrientationToggled`                 | —                                | `toggle_vertical`.                                               |

Every event is dispatched **after** the underlying state has been
mutated. Plugins can therefore read `state.editor` and
`state.settings` and see the post-mutation values.

The event enum is **additive**: new variants may be added over time;
plugins must include a `_ => {}` arm in their `match` blocks.

## Writing a plugin

Every plugin implements the `Plugin` trait, then registers itself with
the `EditorState::plugins` registry. The built-in
`CoreCommandsPlugin` in `src/editor_core/plugin.rs` is a working
reference.

### Minimal hello-world

```rust
// src/plugins/hello.rs
use wasm_bindgen::JsValue;

use crate::editor_core::events::EditorEvent;
use crate::editor_core::plugin::{Command, Plugin, PluginContext};

pub struct HelloPlugin;

impl Plugin for HelloPlugin {
    fn id(&self) -> &'static str {
        "hello"
    }

    fn commands(&self) -> Vec<Command> {
        vec![Command {
            id: "hello.greet",
            title: "Hello: Greet the user",
            category: Some("Hello"),
            keybinding: None,
            handler: |_ctx, _args| {
                web_sys::console::log_1(&"Hello from a plugin!".into());
                Ok(JsValue::from_str("greeted"))
            },
        }]
    }

    fn on_event(&mut self, _ctx: &mut PluginContext<'_>, event: &EditorEvent) {
        if let EditorEvent::TextChanged = event {
            web_sys::console::log_1(&"Text changed".into());
        }
    }
}
```

### Registering

At the moment the registry is populated in `EditorState::new`
(`src/editor_core/editor_state.rs`):

```rust
let mut plugins = PluginRegistry::new();
plugins.register(Box::new(CoreCommandsPlugin));
// plugins.register(Box::new(HelloPlugin));  // Add your plugin here.
```

Once a plugin's commands are registered they immediately appear in
`WasmEditor::list_commands()` and can be invoked from the palette or
directly via `editor.run_command("hello.greet", "")`.

### Command handler contract

```rust
pub fn handler(ctx: &mut PluginContext<'_>, args_json: &str)
    -> Result<JsValue, JsValue>;
```

* `ctx.state` is a mutable borrow of `EditorState`. All editor
  mutations must go through this handle.
* `args_json` is an opaque payload the caller supplied. The palette
  passes `""`. Handlers that need arguments should parse this with
  `serde_json`.
* Return `Ok(JsValue::NULL)` when there is nothing to return. Errors
  become `JsError`s at the WASM boundary.
* Handlers must not panic; the WASM boundary aborts the entire editor
  on panic.
* Handlers may mutate settings, buffer text, cursor, and selection
  freely. After a command runs, `WasmEditor::run_command` dispatches a
  coarse `SettingsChanged` event so other plugins can re-read state.
  Finer-grained per-command events are future work.

## Event handler contract

```rust
fn on_event(&mut self, ctx: &mut PluginContext<'_>, event: &EditorEvent);
```

* Called synchronously from the dispatch site. Keep it fast.
* The event bus does not carry the mutation payload; consult
  `ctx.state.editor` / `ctx.state.settings` for post-mutation values.
* You **cannot** subscribe more handlers from inside `on_event`; the
  bus is exclusively borrowed for the duration of the dispatch. Route
  new registrations through a command or through `EditorState::new`.
* Ordering: plugins are visited in registration order.
  `CoreCommandsPlugin` is always first.

## Command palette (JS side)

The palette lives in `index.html`. It:

1. On first open, calls `editor.list_commands()` once and caches the
   list on the `CommandPalette` instance.
2. Filters the list on every keystroke using a subsequence fuzzy match.
3. Runs the highlighted item on `Enter` via `editor.run_command(id, "")`.

If a plugin adds commands after startup (not currently possible, but
allowed by the API shape), the palette will pick them up on its next
open only if you also invalidate the cache — call
`commandPalette.commands = null` before opening.

## Known limits and open questions

* **No keybinding routing.** `Command::keybinding` is a display hint
  only. Actual key handling still lives in
  `WasmEditor::handle_key_down` (Rust) and `document.addEventListener("keydown", …)`
  (JS). P6-01 will introduce a real key-map.
* **Coarse events.** `handle_key_down` emits one event per keystroke
  based on which branch ran (`Motion` -> `CursorMoved`, text-mutating
  branch -> `TextChanged`). Per-command effect events are future work.
* **Plugin storage.** Plugins own their internal state, but there is
  no per-plugin persistence yet. If you need to remember state across
  reloads, marshal to JS through a command and use `localStorage` from
  JS today.
* **No plugin manifest / lifecycle.** No `on_load` / `on_unload`
  hooks; construct any state in the plugin's own constructor.
* **Reentrancy.** Commands may call other commands via
  `WasmEditor::run_command`, but a command handler cannot recursively
  call `run_command` because the plugin registry is not borrowed
  during handler execution. Handlers that need to invoke other
  commands should look them up on `ctx.state.plugins` and call the
  handler function pointer directly — this is not yet a public
  helper.

## Versioning

The plugin API is not versioned yet. Once P2 lands and the surface
proves out, we'll pick a semver-like discipline: variants are additive,
required trait methods only grow with a compatibility default.

## References

* Plan: `prompt/FUTURE_PLAN.md`, Phase P1.
* Phase log: `DOC/IMPL/PHASE_1_IMPL.md`.
* Patch: `patch/Phase_1.patch`.
