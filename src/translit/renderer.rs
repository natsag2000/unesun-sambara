use std::cell::RefCell;

use cosmic_text::{
    Attrs, Buffer, Color, Family, FontSystem, Metrics, Shaping, SwashCache, TextOrientation,
};
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use web_sys::{CanvasRenderingContext2d, HtmlCanvasElement, ImageData};

use crate::config::settings::EditorSettings;

const PADDING_Y: i32 = 10;

/// Offscreen renderer that draws Mongolian traditional script vertically into
/// a `<canvas>` element. Shares the main editor's `FontSystem` and
/// `SwashCache` so the output is visually identical.
pub struct TranslitRenderer {
    buffer: Buffer,
    width: u32,
    height: u32,
}

impl TranslitRenderer {
    /// Creates a new renderer with a vertical buffer sized to `width` x
    /// `height` (physical pixels).
    pub fn new(
        font_system: &mut FontSystem,
        settings: &EditorSettings,
        width: u32,
        height: u32,
    ) -> Self {
        let metrics = Metrics::new(settings.fonts.font_size, settings.fonts.line_height);
        let mut buffer = Buffer::new(font_system, metrics);
        buffer.set_size(font_system, Some(width as f32), Some(height as f32));
        buffer.set_orientation(font_system, TextOrientation::VerticalLtr);

        Self {
            buffer,
            width,
            height,
        }
    }

    /// Resize the underlying buffer. Called when the modal canvas is resized.
    pub fn resize(&mut self, font_system: &mut FontSystem, width: u32, height: u32) {
        self.width = width;
        self.height = height;
        self.buffer
            .set_size(font_system, Some(width as f32), Some(height as f32));
    }

    /// Re-apply metrics/orientation if the user changed font size / line
    /// height in the main settings while the dialog was open.
    pub fn apply_settings(&mut self, font_system: &mut FontSystem, settings: &EditorSettings) {
        let metrics = Metrics::new(settings.fonts.font_size, settings.fonts.line_height);
        self.buffer.set_metrics(font_system, metrics);
        // Keep vertical regardless of main-editor orientation.
        self.buffer
            .set_orientation(font_system, TextOrientation::VerticalLtr);
    }

    /// Sets the text to render, using the font family from the provided
    /// settings so Mongolian shaping matches the main editor.
    pub fn set_text(&mut self, font_system: &mut FontSystem, settings: &EditorSettings, text: &str) {
        let font_family = settings.fonts.font_family.clone();
        let attrs = Attrs::new().family(Family::Name(&font_family));
        self.buffer.set_text(
            font_system,
            text,
            &attrs,
            Shaping::Advanced,
            None,
        );
    }

    /// Renders the current buffer contents into the target canvas.
    /// Mirrors the pixel-blend loop used by `WasmEditor::render`, minus the
    /// cursor/selection logic.
    pub fn render(
        &mut self,
        font_system: &mut FontSystem,
        cache: &mut SwashCache,
        settings: &EditorSettings,
        canvas: &HtmlCanvasElement,
    ) -> Result<(), JsValue> {
        let context = canvas
            .get_context("2d")?
            .ok_or("No 2d context on translit canvas")?
            .dyn_into::<CanvasRenderingContext2d>()?;

        let width = self.width as usize;
        let height = self.height as usize;
        if width == 0 || height == 0 {
            return Ok(());
        }

        let bg = settings.appearance.background_color;
        let text_color = settings.appearance.text_color;

        // Clear via fillRect first (cheap, also paints any area we don't
        // touch with ImageData below).
        context.set_fill_style(
            &format!("rgb({}, {}, {})", bg.r(), bg.g(), bg.b()).into(),
        );
        context.fill_rect(0.0, 0.0, self.width as f64, self.height as f64);

        // Build a pixel buffer pre-filled with the background colour.
        let mut pixels = vec![0u8; width * height * 4];
        for pixel in pixels.chunks_exact_mut(4) {
            pixel[0] = bg.r();
            pixel[1] = bg.g();
            pixel[2] = bg.b();
            pixel[3] = 255;
        }

        self.buffer.shape_until_scroll(font_system, false);

        let width_u32 = self.width;
        let height_u32 = self.height;

        // First pass: discover the horizontal extent of the rendered glyphs
        // so we can offset them to be centered horizontally on the canvas.
        let bounds: RefCell<Option<(i32, i32)>> = RefCell::new(None);
        self.buffer.draw(
            font_system,
            cache,
            text_color,
            |x: i32, _y: i32, w: u32, _h: u32, _color: Color| {
                let mut b = bounds.borrow_mut();
                let xr = x + w as i32;
                match *b {
                    None => *b = Some((x, xr)),
                    Some((min_x, max_x)) => {
                        *b = Some((min_x.min(x), max_x.max(xr)));
                    }
                }
            },
        );

        let pad_x: i32 = match *bounds.borrow() {
            Some((min_x, max_x)) => {
                let content_w = (max_x - min_x).max(0);
                let canvas_w = width_u32 as i32;
                // Center: compute left offset so the glyph block sits in the
                // middle of the canvas. Subtract `min_x` so the left-most
                // glyph lands at the desired x.
                ((canvas_w - content_w) / 2) - min_x
            }
            None => 10, // fallback when nothing was drawn
        };

        // Second pass: actually paint pixels with the horizontal offset.
        self.buffer.draw(
            font_system,
            cache,
            text_color,
            |x: i32, y: i32, w: u32, h: u32, color: Color| {
                let x = x + pad_x;
                let y = y + PADDING_Y;

                if x + w as i32 <= 0
                    || x >= width_u32 as i32
                    || y + h as i32 <= 0
                    || y >= height_u32 as i32
                {
                    return;
                }

                let x_start = x.max(0) as u32;
                let y_start = y.max(0) as u32;
                let x_end = (x + w as i32).min(width_u32 as i32) as u32;
                let y_end = (y + h as i32).min(height_u32 as i32) as u32;

                for py in y_start..y_end {
                    for px in x_start..x_end {
                        let idx = ((py * width_u32 + px) * 4) as usize;
                        if idx + 3 < pixels.len() {
                            let src_alpha = color.a() as f32 / 255.0;
                            let dst_r = pixels[idx];
                            let dst_g = pixels[idx + 1];
                            let dst_b = pixels[idx + 2];

                            pixels[idx] = ((color.r() as f32 * src_alpha)
                                + (dst_r as f32 * (1.0 - src_alpha)))
                                as u8;
                            pixels[idx + 1] = ((color.g() as f32 * src_alpha)
                                + (dst_g as f32 * (1.0 - src_alpha)))
                                as u8;
                            pixels[idx + 2] = ((color.b() as f32 * src_alpha)
                                + (dst_b as f32 * (1.0 - src_alpha)))
                                as u8;
                            pixels[idx + 3] = 255;
                        }
                    }
                }
            },
        );

        let image_data = ImageData::new_with_u8_clamped_array_and_sh(
            wasm_bindgen::Clamped(&pixels),
            self.width,
            self.height,
        )?;
        context.put_image_data(&image_data, 0.0, 0.0)?;

        Ok(())
    }
}
