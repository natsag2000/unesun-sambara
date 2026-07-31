use cosmic_text::Color;
use serde::{Serialize, Deserialize};
use wasm_bindgen::prelude::*;

#[derive(Clone, Serialize, Deserialize)]
pub struct EditorSettings {
    pub appearance: AppearanceSettings,
    pub fonts: FontSettings,
    pub editor: EditorBehaviorSettings,
}

impl Default for EditorSettings {
    fn default() -> Self {
        Self {
            appearance: AppearanceSettings::default(),
            fonts: FontSettings::default(),
            editor: EditorBehaviorSettings::default(),
        }
    }
}

impl EditorSettings {
    pub fn to_json(&self) -> Result<String, JsValue> {
        serde_json::to_string(self)
            .map_err(|e| JsValue::from_str(&e.to_string()))
    }

    pub fn from_json(json: &str) -> Result<EditorSettings, JsValue> {
        serde_json::from_str(json)
            .map_err(|e| JsValue::from_str(&e.to_string()))
    }
}

#[derive(Clone, Serialize, Deserialize)]
pub struct AppearanceSettings {
    #[serde(with = "color_serde")]
    pub text_color: Color,
    #[serde(with = "color_serde")]
    pub background_color: Color,
    #[serde(with = "color_serde")]
    pub cursor_color: Color,
    #[serde(with = "color_serde")]
    pub selection_color: Color,
    #[serde(with = "color_serde")]
    pub gutter_background: Color,
    #[serde(with = "color_serde")]
    pub line_number_color: Color,
}

impl Default for AppearanceSettings {
    fn default() -> Self {
        Self {
            // Canonical defaults (see prompt/FUTURE_PLAN.md P0-01):
            // dark text on a light background for a paper-like editing surface.
            text_color: Color::rgb(0x0d, 0x0d, 0x0d),       // #0d0d0d
            background_color: Color::rgb(0xf5, 0xf5, 0xf5), // #f5f5f5
            cursor_color: Color::rgb(0x0d, 0x0d, 0x0d),     // matches text
            selection_color: Color::rgba(80, 120, 200, 128),
            gutter_background: Color::rgb(37, 37, 38),
            line_number_color: Color::rgb(133, 133, 133),
        }
    }
}

#[derive(Clone, Serialize, Deserialize)]
pub struct FontSettings {
    pub font_family: String,
    pub font_size: f32,
    pub line_height: f32,
}

impl Default for FontSettings {
    fn default() -> Self {
        Self {
            font_family: "Noto Sans Mongolian".to_string(),
            font_size: 43.0,
            line_height: 54.0,
        }
    }
}

#[derive(Clone, Serialize, Deserialize)]
pub struct EditorBehaviorSettings {
    pub orientation: String,
    /// Show a line-number gutter (P2-02). Left column in horizontal
    /// mode, top strip of column numbers in vertical mode. Off by
    /// default to keep the existing look for current users.
    #[serde(default)]
    pub show_line_numbers: bool,
    /// Wrap long lines to the buffer's visible width instead of letting
    /// them run past the edge (P2-04). On by default, matching the
    /// existing `buffer.set_size`-driven behavior prior to this option
    /// existing.
    #[serde(default = "default_word_wrap")]
    pub word_wrap: bool,
    /// Id of the active built-in theme (P4-01), or `"custom"` once the
    /// user has hand-edited a color after picking one. Purely
    /// informational for the JS theme dropdown - Rust never reads this
    /// to decide what colors to render; `appearance` is always the
    /// source of truth for actual rendering.
    #[serde(default = "default_theme_id")]
    pub theme_id: String,
}

fn default_word_wrap() -> bool {
    true
}

fn default_theme_id() -> String {
    "default-light".to_string()
}

impl Default for EditorBehaviorSettings {
    fn default() -> Self {
        Self {
            orientation: "vertical".to_string(),
            show_line_numbers: false,
            word_wrap: true,
            theme_id: default_theme_id(),
        }
    }
}

// Color serde module for cosmic_text::Color
//
// Alpha round-trips too (P4-01 exposes `selection_color` et al. in the
// UI, which surfaced a latent bug: this module used to always emit
// 6-digit RGB hex, silently dropping alpha - so `selection_color`'s
// default 128-alpha became fully opaque the moment settings were saved
// once, since the JS layer round-trips through `get_settings_json` /
// `set_settings_json` on every save even for fields it doesn't touch).
// Opaque colors (the common case) still serialize as 6 digits so
// existing `.uns` files / localStorage entries are visually unchanged;
// only colors with `a != 255` grow the extra 2 hex digits. Deserialize
// accepts either length, so old 6-digit data still loads fine (with the
// alpha it always implicitly had: fully opaque).
mod color_serde {
    use cosmic_text::Color;
    use serde::{Deserialize, Deserializer, Serializer};

    pub fn serialize<S>(color: &Color, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let hex = if color.a() == 255 {
            format!("#{:02x}{:02x}{:02x}", color.r(), color.g(), color.b())
        } else {
            format!(
                "#{:02x}{:02x}{:02x}{:02x}",
                color.r(),
                color.g(),
                color.b(),
                color.a()
            )
        };
        serializer.serialize_str(&hex)
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Color, D::Error>
    where
        D: Deserializer<'de>,
    {
        let hex = String::deserialize(deserializer)?;
        let hex = hex.trim_start_matches('#');
        if hex.len() < 6 {
            return Err(serde::de::Error::custom(format!(
                "invalid color hex '{hex}': expected 6 or 8 hex digits"
            )));
        }
        let r = u8::from_str_radix(&hex[0..2], 16).map_err(serde::de::Error::custom)?;
        let g = u8::from_str_radix(&hex[2..4], 16).map_err(serde::de::Error::custom)?;
        let b = u8::from_str_radix(&hex[4..6], 16).map_err(serde::de::Error::custom)?;
        let a = if hex.len() >= 8 {
            u8::from_str_radix(&hex[6..8], 16).map_err(serde::de::Error::custom)?
        } else {
            255
        };
        Ok(Color::rgba(r, g, b, a))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn settings_round_trip_preserves_defaults() {
        let original = EditorSettings::default();
        let json = original.to_json().expect("serialize");
        let restored = EditorSettings::from_json(&json).expect("deserialize");
        assert_eq!(restored.appearance.text_color, original.appearance.text_color);
        assert_eq!(restored.fonts.font_size, original.fonts.font_size);
        assert_eq!(restored.editor.orientation, original.editor.orientation);
        assert_eq!(restored.editor.theme_id, original.editor.theme_id);
    }

    #[test]
    fn selection_color_alpha_survives_a_json_round_trip() {
        // Regression test: this used to silently become fully opaque
        // (see the `color_serde` module doc comment above) because the
        // old 6-digit-only serializer dropped alpha entirely.
        let original = AppearanceSettings::default();
        assert_eq!(original.selection_color.a(), 128);

        let json = serde_json::to_string(&original).expect("serialize");
        let restored: AppearanceSettings = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(restored.selection_color.a(), 128);
        assert_eq!(restored.selection_color, original.selection_color);
    }

    #[test]
    fn opaque_colors_serialize_as_six_hex_digits() {
        let json = serde_json::to_string(&AppearanceSettings::default()).expect("serialize");
        assert!(json.contains("\"text_color\":\"#0d0d0d\""));
    }

    #[test]
    fn six_digit_hex_still_deserializes_as_fully_opaque() {
        // Raw string delimiter needs two hashes since the JSON content
        // itself contains `"#`, which would otherwise terminate a
        // single-hash raw string early.
        let json = r##"{"text_color":"#112233","background_color":"#ffffff","cursor_color":"#000000","selection_color":"#ff0000","gutter_background":"#000000","line_number_color":"#ffffff"}"##;
        let appearance: AppearanceSettings = serde_json::from_str(json).expect("deserialize");
        assert_eq!(appearance.text_color, Color::rgb(0x11, 0x22, 0x33));
        assert_eq!(appearance.text_color.a(), 255);
    }

    #[test]
    fn old_settings_json_without_new_fields_still_deserializes() {
        // Simulates a `.uns` file / localStorage entry saved before
        // P2-02/P2-04/P4-01 introduced these fields.
        let json = r##"{
            "appearance": {"text_color":"#0d0d0d","background_color":"#f5f5f5","cursor_color":"#0d0d0d","selection_color":"#5078c8","gutter_background":"#252526","line_number_color":"#858585"},
            "fonts": {"font_family":"Noto Sans Mongolian","font_size":43.0,"line_height":54.0},
            "editor": {"orientation":"vertical"}
        }"##;
        let settings = EditorSettings::from_json(json).expect("deserialize");
        assert!(!settings.editor.show_line_numbers);
        assert!(settings.editor.word_wrap);
        assert_eq!(settings.editor.theme_id, "default-light");
    }
}
