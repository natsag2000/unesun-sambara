//! Configurable keybindings (Phase P6-01).
//!
//! Scope decision made before implementing this: only *app-level*
//! shortcuts are configurable here (command palette, find/replace,
//! zoom, tabs, save/open/export, transliteration, input-mode toggle).
//! Low-level text-editing keys - undo/redo, copy/cut/paste, character
//! typing, arrow-key motion - stay hardcoded in
//! `WasmEditor::handle_key_down`. Those are handled inside Rust for
//! reasons that would make them awkward to reconfigure safely (undo/
//! redo interacts with history-group timing; typing must handle
//! arbitrary Unicode input, not a fixed combo) and reconfiguring them
//! risks colliding with what a JS-side app shortcut has been
//! reassigned to. This module - and the "Keybindings" settings tab it
//! backs - only ever sees the app-level action list below.
//!
//! Rust owns the *default* combo for each action (source of truth,
//! same principle as `themes.rs`); the JS `KeybindingManager` layers
//! localStorage overrides on top and does the actual `KeyboardEvent`
//! matching (see `index.html`) - Rust never parses a combo string
//! itself, it only produces the defaults.

use serde::Serialize;

#[derive(Serialize)]
pub struct KeyBinding {
    /// Stable id, namespaced like plugin commands (`editor.foo`) even
    /// though these aren't currently wired through the P1 command
    /// registry - both are "stable action id" spaces and keeping the
    /// same convention avoids collisions if they're ever unified.
    pub id: &'static str,
    /// Human-readable label for the Keybindings settings tab.
    pub label: &'static str,
    /// Default combo, e.g. `"Ctrl+Shift+P"`. `"Ctrl"` means
    /// ctrlKey-or-metaKey (see `KeybindingManager.matches` in
    /// `index.html`), matching how every other Ctrl-based shortcut in
    /// this app already treats macOS's Cmd key as equivalent.
    pub default: &'static str,
}

/// The full configurable action list, in the order they appear in the
/// Keybindings settings tab.
pub fn default_keybindings() -> Vec<KeyBinding> {
    vec![
        KeyBinding {
            id: "editor.commandPalette",
            label: "Command Palette",
            default: "Ctrl+Shift+P",
        },
        KeyBinding {
            id: "editor.find",
            label: "Find",
            default: "Ctrl+F",
        },
        KeyBinding {
            id: "editor.findReplace",
            label: "Find and Replace",
            default: "Ctrl+H",
        },
        KeyBinding {
            id: "editor.findNext",
            label: "Find Next",
            default: "F3",
        },
        KeyBinding {
            id: "editor.findPrev",
            label: "Find Previous",
            default: "Shift+F3",
        },
        KeyBinding {
            id: "editor.zoomIn",
            label: "Zoom In",
            default: "Ctrl+=",
        },
        KeyBinding {
            id: "editor.zoomOut",
            label: "Zoom Out",
            default: "Ctrl+-",
        },
        KeyBinding {
            id: "editor.newTab",
            label: "New Tab",
            default: "Ctrl+N",
        },
        KeyBinding {
            id: "editor.closeTab",
            label: "Close Tab",
            default: "Ctrl+W",
        },
        KeyBinding {
            id: "editor.cycleTabNext",
            label: "Next Tab",
            default: "Ctrl+Tab",
        },
        KeyBinding {
            id: "editor.cycleTabPrev",
            label: "Previous Tab",
            default: "Ctrl+Shift+Tab",
        },
        KeyBinding {
            id: "editor.save",
            label: "Save",
            default: "Ctrl+S",
        },
        KeyBinding {
            id: "editor.open",
            label: "Open File",
            default: "Ctrl+O",
        },
        KeyBinding {
            id: "editor.export",
            label: "Export as PNG",
            default: "Ctrl+E",
        },
        KeyBinding {
            id: "editor.transliterate",
            label: "Transliterate",
            default: "Ctrl+T",
        },
        KeyBinding {
            id: "editor.toggleInputMode",
            label: "Toggle Input Mode",
            default: "Ctrl+Alt+I",
        },
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn action_ids_are_unique() {
        let bindings = default_keybindings();
        let mut ids: Vec<&str> = bindings.iter().map(|b| b.id).collect();
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(ids.len(), bindings.len());
    }

    #[test]
    fn default_combos_are_unique() {
        // Not a hard technical requirement (the JS dispatcher resolves
        // ties by registration order), but the *defaults* shipped
        // out of the box should never collide with each other.
        let bindings = default_keybindings();
        let mut combos: Vec<&str> = bindings.iter().map(|b| b.default).collect();
        combos.sort_unstable();
        combos.dedup();
        assert_eq!(combos.len(), bindings.len());
    }

    #[test]
    fn serializes_to_the_shape_the_js_settings_modal_expects() {
        let json = serde_json::to_string(&default_keybindings()).expect("serialize");
        let value: serde_json::Value = serde_json::from_str(&json).expect("valid json");
        let array = value.as_array().expect("top-level array");
        assert!(!array.is_empty());
        for entry in array {
            assert!(entry["id"].is_string());
            assert!(entry["label"].is_string());
            assert!(entry["default"].is_string());
        }
    }
}
