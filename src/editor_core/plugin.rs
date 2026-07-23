//! Plugin trait and in-process registry.
//!
//! See `prompt/FUTURE_PLAN.md` (Phase P1-02) and `prompt/PLUGIN_API.md`.
//!
//! Scope
//! -----
//! * Plugins are compiled into the WASM binary. There is no external
//!   loading path yet (that would need a stable ABI and is deliberately
//!   deferred).
//! * Every plugin can register zero or more commands. Commands are
//!   invoked either from the command palette (`P1-03`) or by other
//!   code holding a `PluginRegistry`.
//! * Plugins may observe events by implementing `on_event`. The registry
//!   itself is the sole subscriber the `EditorState` needs ÔÇö it fans
//!   events out to plugins in registration order.
//!
//! Borrow strategy
//! ---------------
//! Command handlers receive a `PluginContext` that holds `&mut
//! EditorState`. During `run_command` we look up the command by id
//! (immutable borrow of the registry), pull out the function pointer,
//! release the registry borrow, then invoke the handler with `&mut
//! EditorState`. The registry's own plugin storage is never borrowed
//! mutably while a handler runs, so plugins cannot re-enter their own
//! command dispatch. This mirrors the way `EventBus` drains its
//! subscribers during dispatch (`events.rs`).
//!
//! As with `events.rs`, parts of the trait / registry surface are not
//! yet consumed (e.g. `Plugin::id`, `Plugin::name`, `DocumentSaved`
//! fields) but they are the documented API plugins will use in later
//! phases. The module-level `allow(dead_code)` acknowledges that.

#![allow(dead_code)]

use cosmic_text::Edit;
use wasm_bindgen::JsValue;

use crate::editor_core::editor_state::EditorState;
use crate::editor_core::events::EditorEvent;

/// A single invokable action exposed by a plugin.
///
/// `handler` is a bare `fn` pointer, not a closure, so that `Command`
/// stays `Copy`-compatible for lookup-then-invoke without holding a
/// borrow on the registry. If a future plugin needs closure captures,
/// the migration path is either to move state into `EditorState` (via a
/// plugin data slot) or to switch this to `Box<dyn Fn(...)>`.
#[derive(Clone)]
pub struct Command {
    /// Stable id, e.g. `"editor.toggleOrientation"`. Used by the command
    /// palette and by other plugins to invoke this command.
    pub id: &'static str,
    /// Human-readable label shown in the palette.
    pub title: &'static str,
    /// Optional grouping label (e.g. "Editor", "View", "File").
    pub category: Option<&'static str>,
    /// Display-only keybinding hint. Actual key routing still lives in
    /// `WasmEditor::handle_key_down` / `index.html`; this string is
    /// shown in the palette so users learn the shortcut.
    pub keybinding: Option<&'static str>,
    /// The command implementation. Receives a mutable context and a
    /// JSON string of arguments (may be empty).
    pub handler: fn(&mut PluginContext<'_>, &str) -> Result<JsValue, JsValue>,
}

/// Context handed to command handlers and to `Plugin::on_event`.
///
/// Currently exposes only `state`; over time we will add narrow helpers
/// (e.g. a `notify(&str)` for status-bar messages) rather than growing
/// the surface arbitrarily.
pub struct PluginContext<'a> {
    pub state: &'a mut EditorState,
}

impl<'a> PluginContext<'a> {
    pub fn new(state: &'a mut EditorState) -> Self {
        Self { state }
    }
}

/// The plugin trait.
///
/// Deliberately narrow: everything a plugin does today is either
/// contribute commands, or react to events. Rendering hooks, decoration
/// layers, and keybinding contributions are called out as future work
/// in `prompt/PLUGIN_API.md`.
pub trait Plugin {
    /// Stable identifier, e.g. `"core.commands"`. Used for logging and
    /// disambiguating command sources.
    fn id(&self) -> &'static str;

    /// Human-readable name (for debugging / an eventual plugin list UI).
    fn name(&self) -> &'static str {
        self.id()
    }

    /// Commands this plugin contributes. Called once at registration
    /// time; the result is cached on the registry.
    fn commands(&self) -> Vec<Command> {
        Vec::new()
    }

    /// React to an editor event. Default: no-op.
    fn on_event(&mut self, _ctx: &mut PluginContext<'_>, _event: &EditorEvent) {}
}

struct RegisteredPlugin {
    plugin: Box<dyn Plugin>,
    commands: Vec<Command>,
}

/// In-process registry of compiled-in plugins.
pub struct PluginRegistry {
    plugins: Vec<RegisteredPlugin>,
}

impl PluginRegistry {
    pub fn new() -> Self {
        Self {
            plugins: Vec::new(),
        }
    }

    /// Register a plugin. Commands are captured at registration time;
    /// changing a plugin's command list dynamically is not supported.
    pub fn register(&mut self, plugin: Box<dyn Plugin>) {
        let commands = plugin.commands();
        self.plugins.push(RegisteredPlugin { plugin, commands });
    }

    /// Fan an event out to every plugin, in registration order. Called
    /// from `WasmEditor` after mutating actions.
    pub fn dispatch_event(&mut self, state: &mut EditorState, event: &EditorEvent) {
        for entry in self.plugins.iter_mut() {
            let mut ctx = PluginContext::new(state);
            entry.plugin.on_event(&mut ctx, event);
        }
    }

    /// Iterate every registered command. Order is stable: earlier
    /// plugins first, then per-plugin registration order.
    pub fn commands(&self) -> impl Iterator<Item = &Command> {
        self.plugins.iter().flat_map(|p| p.commands.iter())
    }

    /// Look up a command by id. Returns the handler pointer plus the
    /// command metadata (borrowed) ÔÇö the caller can then release the
    /// registry borrow and invoke the handler.
    pub fn find_command(&self, id: &str) -> Option<&Command> {
        self.commands().find(|c| c.id == id)
    }

    /// Number of plugins registered. Used by the WASM entry point to
    /// detect whether dispatch replaced the registry (see
    /// `WasmEditor::dispatch_event`); also handy in tests.
    pub fn plugin_count(&self) -> usize {
        self.plugins.len()
    }
}

impl Default for PluginRegistry {
    fn default() -> Self {
        Self::new()
    }
}

// ==========================================================================
// Built-in plugin: core commands
// ==========================================================================

/// Contributes commands that mirror the existing hardcoded keybindings
/// and toolbar actions. Registering this at startup makes the command
/// palette useful out of the box even before any user-facing plugins
/// are added.
pub struct CoreCommandsPlugin;

impl Plugin for CoreCommandsPlugin {
    fn id(&self) -> &'static str {
        "core.commands"
    }

    fn name(&self) -> &'static str {
        "Core commands"
    }

    fn commands(&self) -> Vec<Command> {
        vec![
            Command {
                id: "editor.toggleOrientation",
                title: "Editor: Toggle Vertical / Horizontal Orientation",
                category: Some("Editor"),
                keybinding: Some("Ctrl+Alt+V"),
                handler: cmd_toggle_orientation,
            },
            Command {
                id: "editor.resetSettings",
                title: "Editor: Reset Settings to Defaults",
                category: Some("Editor"),
                keybinding: None,
                handler: cmd_reset_settings,
            },
            Command {
                id: "editor.clearSelection",
                title: "Editor: Clear Selection",
                category: Some("Editor"),
                keybinding: Some("Esc"),
                handler: cmd_clear_selection,
            },
        ]
    }
}

fn cmd_toggle_orientation(
    ctx: &mut PluginContext<'_>,
    _args: &str,
) -> Result<JsValue, JsValue> {
    use cosmic_text::TextOrientation;

    let new_orientation = if ctx.state.settings.editor.orientation == "vertical" {
        "horizontal"
    } else {
        "vertical"
    };
    ctx.state.settings.editor.orientation = new_orientation.to_string();

    let orientation = if new_orientation == "vertical" {
        TextOrientation::VerticalLtr
    } else {
        TextOrientation::Horizontal
    };

    let font_system = &mut ctx.state.font_system;
    ctx.state.editor.with_buffer_mut(move |buffer| {
        for line in buffer.lines.iter_mut() {
            line.reset_shaping();
        }
        buffer.set_orientation(font_system, orientation);
    });

    Ok(JsValue::from_str(new_orientation))
}

fn cmd_reset_settings(
    ctx: &mut PluginContext<'_>,
    _args: &str,
) -> Result<JsValue, JsValue> {
    use crate::config::settings::EditorSettings;
    let settings = EditorSettings::default();
    let json = settings.to_json()?;
    ctx.state.update_settings(settings);
    Ok(JsValue::from_str(&json))
}

fn cmd_clear_selection(
    ctx: &mut PluginContext<'_>,
    _args: &str,
) -> Result<JsValue, JsValue> {
    use cosmic_text::Selection;
    if ctx.state.editor.selection() != Selection::None {
        ctx.state.editor.set_selection(Selection::None);
    }
    Ok(JsValue::NULL)
}

#[cfg(test)]
mod tests {
    use super::*;

    struct CountingPlugin {
        seen: usize,
    }
    impl Plugin for CountingPlugin {
        fn id(&self) -> &'static str {
            "test.counting"
        }
        fn commands(&self) -> Vec<Command> {
            vec![Command {
                id: "test.noop",
                title: "Test: no-op",
                category: None,
                keybinding: None,
                handler: |_, _| Ok(JsValue::NULL),
            }]
        }
        fn on_event(&mut self, _ctx: &mut PluginContext<'_>, _event: &EditorEvent) {
            self.seen += 1;
        }
    }

    #[test]
    fn registry_captures_commands_at_registration_time() {
        let mut reg = PluginRegistry::new();
        reg.register(Box::new(CountingPlugin { seen: 0 }));
        assert_eq!(reg.plugin_count(), 1);
        assert!(reg.find_command("test.noop").is_some());
        assert!(reg.find_command("missing").is_none());
    }
}
