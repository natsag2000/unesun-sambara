//! Built-in color themes (Phase P4-01).
//!
//! Themes only cover [`AppearanceSettings`] (colors) - per the P4 scope
//! decision recorded in `prompt/FUTURE_PLAN.md`, fonts are left alone so
//! switching themes never resizes or reflows text, only recolors it.
//!
//! `Theme` is serialized wholesale via `list_themes()` in `src/lib.rs`;
//! the JS `SettingsModal` populates the Color tab's theme dropdown from
//! that list and applies a theme by merging `appearance` into the
//! current settings, the same "merge on save" pattern used everywhere
//! else fields are updated piecemeal (see P0-02/P0-03).
//!
//! `EditorBehaviorSettings::theme_id` records which of these (or
//! `"custom"`) is active, purely so the dropdown can restore its
//! selection after a reload - Rust never reads it to decide what to
//! render; `appearance` alone is the rendering source of truth.

use crate::config::settings::AppearanceSettings;
use cosmic_text::Color;
use serde::Serialize;

#[derive(Serialize)]
pub struct Theme {
    pub id: &'static str,
    pub name: &'static str,
    pub appearance: AppearanceSettings,
}

/// The built-in theme list, in display order. `"default-light"` is
/// defined to exactly equal `AppearanceSettings::default()` (see P0-01),
/// so it is a no-op for anyone who has never touched the Color tab.
pub fn built_in_themes() -> Vec<Theme> {
    vec![
        Theme {
            id: "default-light",
            name: "Default Light",
            appearance: AppearanceSettings::default(),
        },
        Theme {
            id: "default-dark",
            name: "Default Dark",
            appearance: AppearanceSettings {
                text_color: Color::rgb(0xd4, 0xd4, 0xd4),
                background_color: Color::rgb(0x1e, 0x1e, 0x1e),
                cursor_color: Color::rgb(0xd4, 0xd4, 0xd4),
                selection_color: Color::rgba(38, 79, 120, 180),
                gutter_background: Color::rgb(0x25, 0x25, 0x26),
                line_number_color: Color::rgb(0x85, 0x85, 0x85),
            },
        },
        Theme {
            id: "high-contrast",
            name: "High Contrast",
            appearance: AppearanceSettings {
                text_color: Color::rgb(0xff, 0xff, 0xff),
                background_color: Color::rgb(0x00, 0x00, 0x00),
                cursor_color: Color::rgb(0xff, 0xff, 0x00),
                selection_color: Color::rgba(255, 255, 0, 130),
                gutter_background: Color::rgb(0x00, 0x00, 0x00),
                line_number_color: Color::rgb(0xff, 0xff, 0xff),
            },
        },
        Theme {
            id: "solarized-dark",
            name: "Solarized Dark",
            appearance: AppearanceSettings {
                text_color: Color::rgb(0x83, 0x94, 0x96),
                background_color: Color::rgb(0x00, 0x2b, 0x36),
                cursor_color: Color::rgb(0x93, 0xa1, 0xa1),
                selection_color: Color::rgba(0x07, 0x36, 0x42, 220),
                gutter_background: Color::rgb(0x07, 0x36, 0x42),
                line_number_color: Color::rgb(0x58, 0x6e, 0x75),
            },
        },
        Theme {
            id: "mongolian-parchment",
            name: "Mongolian Parchment",
            appearance: AppearanceSettings {
                text_color: Color::rgb(0x4a, 0x33, 0x1e),
                background_color: Color::rgb(0xf1, 0xe4, 0xc6),
                cursor_color: Color::rgb(0x7a, 0x1f, 0x1f),
                selection_color: Color::rgba(0xc9, 0xa8, 0x66, 150),
                gutter_background: Color::rgb(0xe3, 0xd2, 0xa8),
                line_number_color: Color::rgb(0x8a, 0x6d, 0x45),
            },
        },
    ]
}

/// Not currently called from `WasmEditor` (the JS layer already has the
/// full list via `list_themes()` and looks up by id client-side), but
/// kept as the obvious Rust-side lookup API - e.g. useful if a future
/// command (P1 palette) wants to apply a theme by id server-side.
#[allow(dead_code)]
pub fn theme_by_id(id: &str) -> Option<Theme> {
    built_in_themes().into_iter().find(|t| t.id == id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_light_matches_appearance_default() {
        let themes = built_in_themes();
        let default_light = themes.iter().find(|t| t.id == "default-light").unwrap();
        let default_appearance = AppearanceSettings::default();
        assert_eq!(default_light.appearance.text_color, default_appearance.text_color);
        assert_eq!(
            default_light.appearance.background_color,
            default_appearance.background_color
        );
    }

    #[test]
    fn theme_ids_are_unique() {
        let themes = built_in_themes();
        let mut ids: Vec<&str> = themes.iter().map(|t| t.id).collect();
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(ids.len(), themes.len());
    }

    #[test]
    fn theme_by_id_finds_known_theme_and_rejects_unknown() {
        assert!(theme_by_id("high-contrast").is_some());
        assert!(theme_by_id("nonexistent").is_none());
    }

    /// Exercises the exact contract `WasmEditor::list_themes()` promises
    /// the JS `SettingsModal` (see `index.html`'s `loadThemes()`): a JSON
    /// array of `{ id, name, appearance: { <6 hex color fields> } }`.
    #[test]
    fn serializes_to_the_shape_the_js_settings_modal_expects() {
        let json = serde_json::to_string(&built_in_themes()).expect("serialize");
        let value: serde_json::Value = serde_json::from_str(&json).expect("valid json");
        let array = value.as_array().expect("top-level array");
        assert_eq!(array.len(), 5);

        for entry in array {
            assert!(entry["id"].is_string());
            assert!(entry["name"].is_string());
            let appearance = &entry["appearance"];
            for field in [
                "text_color",
                "background_color",
                "cursor_color",
                "selection_color",
                "gutter_background",
                "line_number_color",
            ] {
                let hex = appearance[field].as_str().unwrap_or_else(|| {
                    panic!("missing appearance.{field} in theme {entry}")
                });
                assert!(
                    hex.starts_with('#') && (hex.len() == 7 || hex.len() == 9),
                    "unexpected color hex format: {hex}"
                );
            }
        }
    }
}
