# Phase P1 — Plugin / extension architecture (implementation log)

Date: 2026-07-17
Reference: `prompt/FUTURE_PLAN.md`, Phase P1 (items P1-01 through P1-04)
Patch: `patch/Phase_1.patch`
API docs: `prompt/PLUGIN_API.md`

## Scope

All four P1 items completed:

| ID    | Effort | Status | Summary                                                                              |
|-------|--------|--------|--------------------------------------------------------------------------------------|
| P1-01 | M      | done   | `EventBus` in `src/editor_core/events.rs`; dispatched from `WasmEditor` action sites |
| P1-02 | M      | done   | `Plugin` trait, `PluginRegistry`, `PluginContext`, built-in `CoreCommandsPlugin`     |
| P1-03 | M      | done   | `list_commands()` / `run_command()` WASM methods; Tailwind palette; `Ctrl+Shift+P`   |
| P1-04 | S      | done   | `prompt/PLUGIN_API.md` with event catalog, hello-world example, known limits         |

## Design decisions

- **Compiled-in plugins only.** No dynamic loading path this phase.
  External JS-facing plugin ABI is intentionally deferred until at
  least one non-core plugin exists in-tree.
- **Two dispatch surfaces.** `EventBus` (subscriber-based, generic) and
  `PluginRegistry::dispatch_event` (plugin-focused). `WasmEditor` fans
  events out to both. Splitting them keeps the bus reusable for
  non-plugin listeners (e.g. future diagnostics) without dragging
  every subscriber through the plugin trait.
- **Function-pointer command handlers.** `Command::handler` is a plain
  `fn`, not `Box<dyn Fn>`. This makes lookup-then-invoke work without
  holding a borrow on the registry through the handler call, and
  keeps `Command` trivially `Clone`. Closures with captured state
  would require moving state into `EditorState` or switching to
  boxed dyn — noted as future work in `PLUGIN_API.md`.
- **Coarse event granularity.** `handle_key_down` classifies which
  branch ran (`None`, `Motion`, `TextChange`) via a local enum, then
  fires one `CursorMoved` or `TextChanged` per keystroke. Per-command
  effect events are deferred until a plugin actually needs them; the
  plan's `SelectionChanged` and `DocumentSaved` variants are shipped
  in the enum but not yet dispatched.
- **Reentrancy.** `EventBus::dispatch` moves the subscriber vec out
  during dispatch; subscribers do not receive a bus handle, so
  registering more subscribers from `on_event` is not reachable
  through the current type system. Similarly, `WasmEditor::dispatch_event`
  swaps the plugin registry out with an empty placeholder while
  dispatching so `PluginRegistry::dispatch_event` can hold `&mut
  EditorState` without conflicting borrows.

## Files changed

- **New:** `src/editor_core/events.rs`
  - `EditorEvent` enum (8 variants), `KeyInfo`, `EventSubscriber`,
    `FnSubscriber`, `EventBus`.
  - Two unit tests: dispatch order + empty-bus no-op.
- **New:** `src/editor_core/plugin.rs`
  - `Plugin` trait, `Command` struct, `PluginContext`,
    `PluginRegistry`, `CoreCommandsPlugin`.
  - Three built-in commands: `editor.toggleOrientation`,
    `editor.resetSettings`, `editor.clearSelection`.
  - One unit test: registry captures commands at registration time.
- **New:** `prompt/PLUGIN_API.md`
  - Event catalog, capability matrix, hello-world plugin example,
    handler contracts, known limits, versioning stance.
- Modified: `src/editor_core/mod.rs`
  - Publish the new `events` and `plugin` modules.
- Modified: `src/editor_core/editor_state.rs`
  - Add `events: EventBus` and `plugins: PluginRegistry` fields.
  - Register `CoreCommandsPlugin` in `EditorState::new`.
- Modified: `src/lib.rs`
  - Import `EditorEvent`, `KeyInfo`, `PluginContext`, `PluginRegistry`.
  - New `WasmEditor::dispatch_event` helper: swap the registry out,
    call `PluginRegistry::dispatch_event`, also drive the raw
    `EventBus`, then restore.
  - Dispatch calls inserted at `handle_key_down`, `handle_mouse_down`,
    `set_text`, `insert_text`, `delete_selection`,
    `toggle_vertical`, `set_settings_json`, `reset_to_defaults`.
  - New WASM methods: `list_commands()` and `run_command(id, args_json)`.
- Modified: `index.html`
  - New `#palette-modal` (Tailwind, matching `#settings-modal` /
    `#save-modal` styling).
  - New `CommandPalette` class with fuzzy match (subsequence, streak
    scoring), arrow-key navigation, click / Enter to run.
  - `Ctrl+Shift+P` binding added to the global keydown listener.
  - `#palette-modal` added to the mobile-input focus-refuse list.

## Verification

- `cargo check --target wasm32-unknown-unknown` — only 3 pre-existing
  warnings (deprecated `set_fill_style` in two places, `Dictionary::len`
  unused). No warnings introduced by P1 code.
- `cargo test --lib` — 3 tests pass (2 from `events`, 1 from `plugin`).
- `wasm-pack build --target web --release` — succeeds.
- Generated `pkg/uns_editor.d.ts` exposes `list_commands()` and
  `run_command(id: string, args_json: string)`.
- Manual smoke test path (browser): press `Ctrl+Shift+P`, palette
  appears; typing "toggle" filters to the orientation toggle; Enter
  toggles orientation; palette closes; focus returns to canvas.

## Follow-ups / known limits

- **`Ctrl+Shift+P` and Chrome.** Some browsers reserve `Ctrl+Shift+P`
  for private-window shortcuts. `e.preventDefault()` in the listener
  handles this in most cases but macOS Firefox may still capture it —
  noted in `PLUGIN_API.md`. If it becomes a real friction point, we
  can add `F1` as an alias in P6-01 when the keybinding layer lands.
- **Palette cache invalidation.** `CommandPalette.commands` is loaded
  lazily and never refreshed. Fine while all plugins are registered
  in `EditorState::new`. When a future phase introduces dynamic plugin
  registration, the palette will need an explicit refresh path.
- **`SelectionChanged` and `DocumentSaved` are unused.** They are
  present in the enum for API completeness. Consumers should still
  handle them (with a `_ => {}` arm) so future dispatch doesn't break
  them.
- **Event granularity vs. cost.** Every text mutation dispatches
  through the plugin registry and the raw bus. With one built-in
  plugin and zero raw subscribers today, cost is negligible. Once
  more plugins land, a per-event-topic subscription index (as noted in
  `events.rs`) will be worth doing.

## Next phase

Phase P2 (core editing features) is next. `P2-01 Undo/Redo` is the
first item and benefits from the event bus (auto-snapshot on
`TextChanged`), so building it on the P1 substrate should be
straightforward.
