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
    pub fn new() -> Self {
        Self::default()
    }

    pub fn to_json(&self) -> Result<String, JsValue> {
        serde_json::to_string(self)
            .map_err(|e| JsValue::from_str(&e.to_string()))
    }

    pub fn from_json(json: &str) -> Result<EditorSettings, JsValue> {
        serde_json::from_str(json)
            .map_err(|e| JsValue::from_str(&e.to_string()))
    }
}

// Helper functions (not currently used but available for future use)
#[allow(dead_code)]
fn color_to_hex(color: &Color) -> String {
    format!("#{:02x}{:02x}{:02x}", color.r(), color.g(), color.b())
}

#[allow(dead_code)]
fn hex_to_color(hex: &str) -> Color {
    let hex = hex.trim_start_matches('#');
    let r = u8::from_str_radix(&hex[0..2], 16).unwrap_or(0);
    let g = u8::from_str_radix(&hex[2..4], 16).unwrap_or(0);
    let b = u8::from_str_radix(&hex[4..6], 16).unwrap_or(0);
    Color::rgb(r, g, b)
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
            text_color: Color::rgb(115, 247, 63),
            background_color: Color::rgb(12, 12, 12),
            cursor_color: Color::rgb(255, 255, 255),
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
            font_size: 73.0,
            line_height: 93.0,
        }
    }
}

#[derive(Clone, Serialize, Deserialize)]
pub struct EditorBehaviorSettings {
    pub orientation: String,
}

impl Default for EditorBehaviorSettings {
    fn default() -> Self {
        Self {
            orientation: "vertical".to_string(),
        }
    }
}

// Color serde module for cosmic_text::Color
mod color_serde {
    use cosmic_text::Color;
    use serde::{Deserialize, Deserializer, Serializer};

    pub fn serialize<S>(color: &Color, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let hex = format!("#{:02x}{:02x}{:02x}", color.r(), color.g(), color.b());
        serializer.serialize_str(&hex)
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Color, D::Error>
    where
        D: Deserializer<'de>,
    {
        let hex = String::deserialize(deserializer)?;
        let hex = hex.trim_start_matches('#');
        let r = u8::from_str_radix(&hex[0..2], 16).map_err(serde::de::Error::custom)?;
        let g = u8::from_str_radix(&hex[2..4], 16).map_err(serde::de::Error::custom)?;
        let b = u8::from_str_radix(&hex[4..6], 16).map_err(serde::de::Error::custom)?;
        Ok(Color::rgb(r, g, b))
    }
}
