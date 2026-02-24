mod config;
mod editor_core;

use cosmic_text::{
    Action, Color, Motion, Selection, Edit,
};
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use web_sys::{CanvasRenderingContext2d, HtmlCanvasElement, ImageData, KeyboardEvent, MouseEvent, WheelEvent, TouchEvent};

use editor_core::editor_state::EditorState;
use config::settings::EditorSettings;

// Padding from top and left for vertical text to prevent cursor being cut off
const VERTICAL_TOP_PADDING: i32 = 10;
const VERTICAL_LEFT_PADDING: i32 = 10;

#[wasm_bindgen]
pub struct WasmEditor {
    canvas: HtmlCanvasElement,
    context: CanvasRenderingContext2d,
    state: EditorState,
    width: u32,
    height: u32,
    scroll_offset: (i32, i32),
    shift_pressed: bool,
    ctrl_pressed: bool,
    is_dragging: bool,
    last_touch_x: i32,
    last_touch_y: i32,
    touch_start_time: f64,
    is_touch_scrolling: bool,
}

#[wasm_bindgen]
impl WasmEditor {
    pub async fn new(canvas_id: &str) -> Result<WasmEditor, JsValue> {
        // Set panic hook for better error messages
        console_error_panic_hook::set_once();

        let window = web_sys::window().ok_or("No window")?;
        let document = window.document().ok_or("No document")?;
        let canvas = document
            .get_element_by_id(canvas_id)
            .ok_or("Canvas not found")?
            .dyn_into::<HtmlCanvasElement>()?;

        let context = canvas
            .get_context("2d")?
            .ok_or("No 2d context")?
            .dyn_into::<CanvasRenderingContext2d>()?;

        let width = canvas.width();
        let height = canvas.height();

        // Create editor state with default settings
        let settings = EditorSettings::default();
        let mut state = EditorState::new(settings).await?;

        // Set buffer size
        state.editor.with_buffer_mut(|buffer| {
            buffer.set_size(&mut state.font_system, Some(width as f32), Some(height as f32));
        });

        Ok(WasmEditor {
            canvas,
            context,
            state,
            width,
            height,
            scroll_offset: (0, 0),
            shift_pressed: false,
            ctrl_pressed: false,
            is_dragging: false,
            last_touch_x: 0,
            last_touch_y: 0,
            touch_start_time: 0.0,
            is_touch_scrolling: false,
        })
    }

    pub fn set_size(&mut self, width: u32, height: u32) {
        self.width = width;
        self.height = height;
        self.canvas.set_width(width);
        self.canvas.set_height(height);

        // Split borrows to avoid closure borrowing issues
        let font_system = &mut self.state.font_system;
        let editor = &mut self.state.editor;
        editor.with_buffer_mut(move |buffer| {
            buffer.set_size(font_system, Some(width as f32), Some(height as f32));
        });
    }

    pub fn render(&mut self, timestamp: f64) -> Result<(), JsValue> {
        // Handle cursor blinking
        if timestamp - self.state.last_render_time > 500.0 {
            self.state.cursor_visible = !self.state.cursor_visible;
            self.state.last_render_time = timestamp;
        }

        let width = self.width as usize;
        let height = self.height as usize;

        // Clear with background color from settings
        let bg = &self.state.settings.appearance.background_color;
        self.context.set_fill_style(&format!("rgb({}, {}, {})", bg.r(), bg.g(), bg.b()).into());
        self.context.fill_rect(0.0, 0.0, width as f64, height as f64);

        let mut pixels = vec![0u8; width * height * 4];

        // Fill background
        for pixel in pixels.chunks_exact_mut(4) {
            pixel[0] = bg.r();
            pixel[1] = bg.g();
            pixel[2] = bg.b();
            pixel[3] = 255;
        }

        self.state.editor.shape_as_needed(&mut self.state.font_system, false);

        let text_color = self.state.settings.appearance.text_color;
        let cursor_color = if self.state.cursor_visible {
            self.state.settings.appearance.cursor_color
        } else {
            Color::rgba(0, 0, 0, 0)
        };
        let selection_color = self.state.settings.appearance.selection_color;
        let selected_text_color = Color::rgb(255, 255, 255);

        // Extract values to avoid borrowing self in closure
        let scroll_offset = self.scroll_offset;
        let width_u32 = self.width;
        let height_u32 = self.height;

        // Render text (existing logic but with colors from settings)
        self.state.editor.draw(
            &mut self.state.font_system,
            &mut self.state.cache,
            text_color,
            cursor_color,
            selection_color,
            selected_text_color,
            |x: i32, y: i32, w: u32, h: u32, color: Color| {
                // Apply scroll offset and padding for vertical text
                let x = x - scroll_offset.0 + VERTICAL_LEFT_PADDING;
                let y = y - scroll_offset.1 + VERTICAL_TOP_PADDING;

                // Clip to canvas bounds
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

                // Render rectangle
                for py in y_start..y_end {
                    for px in x_start..x_end {
                        let idx = ((py * width_u32 + px) * 4) as usize;
                        if idx + 3 < pixels.len() {
                            // Alpha blending
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

        // Put pixels to canvas
        let image_data = ImageData::new_with_u8_clamped_array_and_sh(
            wasm_bindgen::Clamped(&pixels),
            self.width,
            self.height,
        )?;
        self.context.put_image_data(&image_data, 0.0, 0.0)?;

        Ok(())
    }

    // Check if a character is a Mongolian format control character
    fn is_format_control(c: char) -> bool {
        matches!(c, '\u{180B}' | '\u{180C}' | '\u{180D}' | '\u{180E}' | '\u{180F}' | '\u{202F}')
    }

    // Check if format control comes AFTER visible char: [visible][format]
    // U+180B, U+180C, U+180D, U+180F
    fn is_format_after_visible(c: char) -> bool {
        matches!(c, '\u{180B}' | '\u{180C}' | '\u{180D}' | '\u{180F}')
    }

    // Check if format control comes BEFORE visible char: [format][visible]
    // U+180E, U+202F
    fn is_format_before_visible(c: char) -> bool {
        matches!(c, '\u{180E}' | '\u{202F}')
    }

    // Handle backspace with format control character awareness
    // Two patterns:
    // 1. [visible][format_after] - U+180B, U+180C, U+180D, U+180F
    // 2. [format_before][visible] - U+180E, U+202F
    fn handle_backspace(&mut self) {
        let cursor = self.state.editor.cursor();
        let cursor_index = cursor.index;
        let cursor_line = cursor.line;

        // Get current line text - cursor.index is relative to this line
        let line_text = self.state.editor.with_buffer(|buffer| {
            buffer.lines.get(cursor_line).map(|line| line.text().to_string())
        });

        let line_text = match line_text {
            Some(text) => text,
            None => {
                // Fallback to normal backspace
                self.state.editor.action(&mut self.state.font_system, Action::Backspace);
                return;
            }
        };

        // Convert byte index to character index
        let char_index = line_text[..cursor_index.min(line_text.len())].chars().count();

        let chars: Vec<char> = line_text.chars().collect();

        if char_index == 0 || char_index > chars.len() {
            // At start or invalid position, use normal backspace
            self.state.editor.action(&mut self.state.font_system, Action::Backspace);
            return;
        }

        let cursor_index = char_index;
        let char_before = chars[cursor_index - 1];

        // Pattern 1: [visible][format_after] - cursor after format
        if Self::is_format_after_visible(char_before) {
            // Count all consecutive format_after controls
            let mut format_count = 1;
            let mut pos = cursor_index - 1;
            while pos > 0 && Self::is_format_after_visible(chars[pos - 1]) {
                format_count += 1;
                pos -= 1;
            }

            // Check if there's a visible char before the formats
            if pos > 0 && !Self::is_format_control(chars[pos - 1]) {
                // Delete formats + visible char before them
                for _ in 0..(format_count + 1) {
                    self.state.editor.action(&mut self.state.font_system, Action::Backspace);
                }
            } else {
                // No visible char before, just delete the format
                self.state.editor.action(&mut self.state.font_system, Action::Backspace);
            }
            return;
        }

        // Pattern 2: [format_before][visible] - cursor after visible
        if !Self::is_format_control(char_before) {
            // Check if there are format_before controls before this visible char
            let mut format_count = 0;
            let mut pos = cursor_index - 1;
            while pos > 0 && Self::is_format_before_visible(chars[pos - 1]) {
                format_count += 1;
                pos -= 1;
            }

            if format_count > 0 {
                // Delete visible char + format_before controls before it
                for _ in 0..(format_count + 1) {
                    self.state.editor.action(&mut self.state.font_system, Action::Backspace);
                }
            } else {
                // No format controls, normal delete
                self.state.editor.action(&mut self.state.font_system, Action::Backspace);
            }
            return;
        }

        // Cursor is after format_before control - could be between [format_before] and [visible]
        if Self::is_format_before_visible(char_before) {
            let mut format_count = 1;
            let mut pos = cursor_index - 1;
            while pos > 0 && Self::is_format_before_visible(chars[pos - 1]) {
                format_count += 1;
                pos -= 1;
            }

            // Check if there's a visible char after cursor
            if cursor_index < chars.len() && !Self::is_format_control(chars[cursor_index]) {
                // Delete format_before controls + visible char after
                for _ in 0..format_count {
                    self.state.editor.action(&mut self.state.font_system, Action::Backspace);
                }
                self.state.editor.action(&mut self.state.font_system, Action::Delete);
            } else {
                // No visible char after, just delete the format
                self.state.editor.action(&mut self.state.font_system, Action::Backspace);
            }
            return;
        }

        // Default: normal backspace
        self.state.editor.action(&mut self.state.font_system, Action::Backspace);
    }

    // Handle delete with format control character awareness
    // Two patterns:
    // 1. [visible][format_after] - U+180B, U+180C, U+180D, U+180F
    // 2. [format_before][visible] - U+180E, U+202F
    fn handle_delete(&mut self) {
        let cursor = self.state.editor.cursor();
        let cursor_index = cursor.index;
        let cursor_line = cursor.line;

        // Get current line text - cursor.index is relative to this line
        let line_text = self.state.editor.with_buffer(|buffer| {
            buffer.lines.get(cursor_line).map(|line| line.text().to_string())
        });

        let line_text = match line_text {
            Some(text) => text,
            None => {
                // Fallback to normal delete
                self.state.editor.action(&mut self.state.font_system, Action::Delete);
                return;
            }
        };

        // Convert byte index to character index
        let char_index = line_text[..cursor_index.min(line_text.len())].chars().count();

        let chars: Vec<char> = line_text.chars().collect();

        if char_index >= chars.len() {
            // At end or invalid position, use normal delete
            self.state.editor.action(&mut self.state.font_system, Action::Delete);
            return;
        }

        let cursor_index = char_index;
        let char_at = chars[cursor_index];

        // Pattern 1: [visible][format_after] - cursor before visible
        if !Self::is_format_control(char_at) {
            // Count format_after controls after this visible char
            let mut format_count = 0;
            let mut pos = cursor_index + 1;
            while pos < chars.len() && Self::is_format_after_visible(chars[pos]) {
                format_count += 1;
                pos += 1;
            }

            if format_count > 0 {
                // Delete visible char + format_after controls after it
                for _ in 0..(format_count + 1) {
                    self.state.editor.action(&mut self.state.font_system, Action::Delete);
                }
            } else {
                // No format controls, normal delete
                self.state.editor.action(&mut self.state.font_system, Action::Delete);
            }
            return;
        }

        // Pattern 2: [format_before][visible] - cursor before format_before
        if Self::is_format_before_visible(char_at) {
            // Count all consecutive format_before controls
            let mut format_count = 1;
            let mut pos = cursor_index + 1;
            while pos < chars.len() && Self::is_format_before_visible(chars[pos]) {
                format_count += 1;
                pos += 1;
            }

            // Check if there's a visible char after the formats
            if pos < chars.len() && !Self::is_format_control(chars[pos]) {
                // Delete format_before controls + visible char after them
                for _ in 0..(format_count + 1) {
                    self.state.editor.action(&mut self.state.font_system, Action::Delete);
                }
            } else {
                // No visible char after, just delete the format
                self.state.editor.action(&mut self.state.font_system, Action::Delete);
            }
            return;
        }

        // Cursor is before format_after - could be between [visible] and [format_after]
        if Self::is_format_after_visible(char_at) {
            let mut format_count = 1;
            let mut pos = cursor_index + 1;
            while pos < chars.len() && Self::is_format_after_visible(chars[pos]) {
                format_count += 1;
                pos += 1;
            }

            // Check if there's a visible char before cursor
            if cursor_index > 0 && !Self::is_format_control(chars[cursor_index - 1]) {
                // Delete visible char before + format_after controls
                self.state.editor.action(&mut self.state.font_system, Action::Backspace);
                for _ in 0..format_count {
                    self.state.editor.action(&mut self.state.font_system, Action::Delete);
                }
            } else {
                // No visible char before, just delete the format
                self.state.editor.action(&mut self.state.font_system, Action::Delete);
            }
            return;
        }

        // Default: normal delete
        self.state.editor.action(&mut self.state.font_system, Action::Delete);
    }

    pub fn handle_key_down(&mut self, event: KeyboardEvent) -> Result<(), JsValue> {
        self.shift_pressed = event.shift_key();
        self.ctrl_pressed = event.ctrl_key() || event.meta_key();

        let key = event.key();

        // Handle motion with shift for selection
        let handle_motion = |state: &mut EditorState, motion: Motion, shift: bool| {
            if shift {
                if state.editor.selection() == Selection::None {
                    let cursor = state.editor.cursor();
                    state.editor.set_selection(Selection::Normal(cursor));
                }
            } else {
                if state.editor.selection() != Selection::None {
                    state.editor.set_selection(Selection::None);
                }
            }
            state.editor.action(&mut state.font_system, Action::Motion(motion));
        };

        match key.as_str() {
            "ArrowLeft" => {
                handle_motion(&mut self.state, Motion::Left, self.shift_pressed);
                event.prevent_default();
            }
            "ArrowRight" => {
                handle_motion(&mut self.state, Motion::Right, self.shift_pressed);
                event.prevent_default();
            }
            "ArrowUp" => {
                handle_motion(&mut self.state, Motion::Up, self.shift_pressed);
                event.prevent_default();
            }
            "ArrowDown" => {
                handle_motion(&mut self.state, Motion::Down, self.shift_pressed);
                event.prevent_default();
            }
            "Home" => {
                handle_motion(&mut self.state, Motion::Home, self.shift_pressed);
                event.prevent_default();
            }
            "End" => {
                handle_motion(&mut self.state, Motion::End, self.shift_pressed);
                event.prevent_default();
            }
            "PageUp" => {
                handle_motion(&mut self.state, Motion::PageUp, self.shift_pressed);
                event.prevent_default();
            }
            "PageDown" => {
                handle_motion(&mut self.state, Motion::PageDown, self.shift_pressed);
                event.prevent_default();
            }
            "Backspace" => {
                self.handle_backspace();
                event.prevent_default();
            }
            "Delete" => {
                self.handle_delete();
                event.prevent_default();
            }
            "Enter" => {
                self.state.editor.action(&mut self.state.font_system, Action::Enter);
                event.prevent_default();
            }
            "Tab" => {
                self.state.editor.action(&mut self.state.font_system, Action::Indent);
                event.prevent_default();
            }
            _ => {
                // Allow Ctrl+V (paste), Ctrl+C (copy), Ctrl+X (cut) to work natively
                if self.ctrl_pressed && (key == "v" || key == "c" || key == "x") {
                    // Don't prevent default - let browser handle clipboard
                    return Ok(());
                }

                // Handle regular character input
                // Accept any key that is exactly one Unicode character (supports international keyboards)
                if !self.ctrl_pressed && !key.is_empty() {
                    let mut chars = key.chars();
                    if let Some(first_char) = chars.next() {
                        // Only insert if it's exactly one character (not "Shift", "Control", etc.)
                        if chars.next().is_none() {
                            self.state.editor.action(&mut self.state.font_system, Action::Insert(first_char));
                            event.prevent_default();
                        }
                    }
                }
            }
        }

        // Reset cursor blink
        self.state.cursor_visible = true;
        self.state.last_render_time = event.time_stamp();

        Ok(())
    }

    pub fn handle_key_up(&mut self, event: KeyboardEvent) {
        self.shift_pressed = event.shift_key();
        self.ctrl_pressed = event.ctrl_key() || event.meta_key();
    }

    pub fn handle_mouse_down(&mut self, event: MouseEvent) -> Result<(), JsValue> {
        // Get mouse position relative to canvas
        let target = event
            .target()
            .ok_or("No event target")?
            .dyn_into::<web_sys::HtmlCanvasElement>()?;

        let rect = target.get_bounding_client_rect();
        let dpr = web_sys::window()
            .ok_or("No window")?
            .device_pixel_ratio();

        let x = ((event.client_x() as f64 - rect.left()) * dpr) as i32;
        let y = ((event.client_y() as f64 - rect.top()) * dpr) as i32;

        // Apply scroll offset and remove padding offsets for hit testing
        let x = x + self.scroll_offset.0 - VERTICAL_LEFT_PADDING;
        let y = y + self.scroll_offset.1 - VERTICAL_TOP_PADDING;

        // Use cosmic-text's hit testing with Click action
        self.state.editor.action(&mut self.state.font_system, Action::Click {
            x,
            y,
        });

        // Start dragging for selection
        self.is_dragging = true;

        // Reset cursor blink to show cursor is active
        self.state.cursor_visible = true;
        self.state.last_render_time = event.time_stamp();

        Ok(())
    }

    pub fn handle_mouse_move(&mut self, event: MouseEvent) -> Result<(), JsValue> {
        // Don't handle mouse move if touch scrolling
        if !self.is_dragging || self.is_touch_scrolling {
            return Ok(());
        }

        // Get mouse position relative to canvas
        let target = event
            .target()
            .ok_or("No event target")?
            .dyn_into::<web_sys::HtmlCanvasElement>()?;

        let rect = target.get_bounding_client_rect();
        let dpr = web_sys::window()
            .ok_or("No window")?
            .device_pixel_ratio();

        let x = ((event.client_x() as f64 - rect.left()) * dpr) as i32;
        let y = ((event.client_y() as f64 - rect.top()) * dpr) as i32;

        // Apply scroll offset and remove padding offsets for hit testing
        let x = x + self.scroll_offset.0 - VERTICAL_LEFT_PADDING;
        let y = y + self.scroll_offset.1 - VERTICAL_TOP_PADDING;

        // Drag to extend selection
        self.state.editor.action(&mut self.state.font_system, Action::Drag {
            x,
            y,
        });

        Ok(())
    }

    pub fn handle_mouse_up(&mut self, _event: MouseEvent) -> Result<(), JsValue> {
        self.is_dragging = false;
        self.is_touch_scrolling = false;
        Ok(())
    }

    pub fn handle_wheel(&mut self, event: WheelEvent) -> Result<(), JsValue> {
        // Get scroll delta
        let delta_x = event.delta_x();
        let delta_y = event.delta_y();

        // Apply scroll (inverted for natural scrolling)
        self.scroll_offset.0 += delta_x as i32;
        self.scroll_offset.1 += delta_y as i32;

        // Clamp scroll to prevent scrolling too far
        self.scroll_offset.0 = self.scroll_offset.0.max(0);
        self.scroll_offset.1 = self.scroll_offset.1.max(0);

        event.prevent_default();
        Ok(())
    }

    pub fn handle_touch_start(&mut self, event: TouchEvent) -> Result<(), JsValue> {
        if let Some(touch) = event.touches().item(0) {
            self.last_touch_x = touch.client_x();
            self.last_touch_y = touch.client_y();
            self.touch_start_time = event.time_stamp();
            self.is_touch_scrolling = false;

            // Also handle as click for cursor positioning
            let target = event
                .target()
                .ok_or("No event target")?
                .dyn_into::<web_sys::HtmlCanvasElement>()?;

            let rect = target.get_bounding_client_rect();
            let dpr = web_sys::window()
                .ok_or("No window")?
                .device_pixel_ratio();

            let x = ((touch.client_x() as f64 - rect.left()) * dpr) as i32;
            let y = ((touch.client_y() as f64 - rect.top()) * dpr) as i32;

            // Apply scroll offset and remove padding offsets for hit testing
            let x = x + self.scroll_offset.0 - VERTICAL_LEFT_PADDING;
            let y = y + self.scroll_offset.1 - VERTICAL_TOP_PADDING;

            // Position cursor
            self.state.editor.action(&mut self.state.font_system, Action::Click {
                x,
                y,
            });

            self.is_dragging = true;
            self.state.cursor_visible = true;
        }
        Ok(())
    }

    pub fn handle_touch_move(&mut self, event: TouchEvent) -> Result<(), JsValue> {
        if let Some(touch) = event.touches().item(0) {
            let current_x = touch.client_x();
            let current_y = touch.client_y();
            let delta_x = (current_x - self.last_touch_x).abs();
            let delta_y = (current_y - self.last_touch_y).abs();

            // Determine gesture type on first significant movement based on VELOCITY
            if !self.is_touch_scrolling && (delta_x > 10 || delta_y > 10) {
                let elapsed_time = event.time_stamp() - self.touch_start_time;
                let distance = ((delta_x * delta_x + delta_y * delta_y) as f64).sqrt();

                // Calculate velocity in pixels per millisecond
                let velocity = if elapsed_time > 0.0 {
                    distance / elapsed_time
                } else {
                    0.0
                };

                // Fast movement (> 0.5 px/ms) = scroll
                // Slow movement (< 0.5 px/ms) = text selection
                if velocity > 0.5 {
                    self.is_touch_scrolling = true;
                }
            }

            if self.is_touch_scrolling {
                // Apply scroll in the appropriate direction based on orientation
                let is_vertical = self.state.settings.editor.orientation == "vertical";

                if is_vertical {
                    // Horizontal scroll for vertical text
                    let scroll_delta = self.last_touch_x - current_x;
                    self.scroll_offset.0 += scroll_delta;
                    self.scroll_offset.0 = self.scroll_offset.0.max(0);
                } else {
                    // Vertical scroll for horizontal text
                    let scroll_delta = self.last_touch_y - current_y;
                    self.scroll_offset.1 += scroll_delta;
                    self.scroll_offset.1 = self.scroll_offset.1.max(0);
                }

                self.last_touch_x = current_x;
                self.last_touch_y = current_y;
            } else if self.is_dragging {
                // Handle as drag for text selection
                let target = event
                    .target()
                    .ok_or("No event target")?
                    .dyn_into::<web_sys::HtmlCanvasElement>()?;

                let rect = target.get_bounding_client_rect();
                let dpr = web_sys::window()
                    .ok_or("No window")?
                    .device_pixel_ratio();

                let x = ((touch.client_x() as f64 - rect.left()) * dpr) as i32;
                let y = ((touch.client_y() as f64 - rect.top()) * dpr) as i32;

                // Apply scroll offset and remove padding offsets for hit testing
                let x = x + self.scroll_offset.0 - VERTICAL_LEFT_PADDING;
                let y = y + self.scroll_offset.1 - VERTICAL_TOP_PADDING;

                // Drag to extend selection
                self.state.editor.action(&mut self.state.font_system, Action::Drag {
                    x,
                    y,
                });
            }
        }
        Ok(())
    }

    pub fn handle_touch_end(&mut self, _event: TouchEvent) -> Result<(), JsValue> {
        self.is_touch_scrolling = false;
        self.is_dragging = false;
        Ok(())
    }

    pub fn get_text(&self) -> String {
        let mut text = String::new();
        self.state.editor.with_buffer(|buffer| {
            for line in buffer.lines.iter() {
                text.push_str(line.text());
                text.push('\n');
            }
        });
        text
    }

    pub fn set_text(&mut self, text: &str) {
        use cosmic_text::{Attrs, Shaping, Family};
        let font_system = &mut self.state.font_system;
        let font_family = self.state.settings.fonts.font_family.clone();
        self.state.editor.with_buffer_mut(move |buffer| {
            // Use the font family from settings to ensure proper font fallback
            let attrs = Attrs::new().family(Family::Name(&font_family));
            buffer.set_text(
                font_system,
                text,
                &attrs,
                Shaping::Advanced,
                None,
            );
        });
    }

    pub fn insert_text(&mut self, text: &str) {
        // Insert text at current cursor position
        for ch in text.chars() {
            self.state.editor.action(&mut self.state.font_system, Action::Insert(ch));
        }
    }

    pub fn get_selected_text(&self) -> JsValue {
        match self.state.editor.copy_selection() {
            Some(text) => JsValue::from_str(&text),
            None => JsValue::NULL,
        }
    }

    pub fn delete_selection(&mut self) {
        self.state.editor.action(&mut self.state.font_system, Action::Backspace);
    }

    pub fn toggle_vertical(&mut self) {
        use cosmic_text::TextOrientation;

        // Update settings
        let new_orientation = if self.state.settings.editor.orientation == "vertical" {
            "horizontal"
        } else {
            "vertical"
        };
        self.state.settings.editor.orientation = new_orientation.to_string();

        // Apply to buffer
        let font_system = &mut self.state.font_system;
        let editor = &mut self.state.editor;
        let orientation = if new_orientation == "vertical" {
            TextOrientation::VerticalLtr
        } else {
            TextOrientation::Horizontal
        };

        editor.with_buffer_mut(move |buffer| {
            // Reset shaping for all lines to force re-shaping with new orientation
            for line in buffer.lines.iter_mut() {
                line.reset_shaping();
            }

            buffer.set_orientation(font_system, orientation);
        });
    }

    #[wasm_bindgen]
    pub fn get_settings_json(&self) -> Result<String, JsValue> {
        self.state.settings.to_json()
    }

    #[wasm_bindgen]
    pub fn set_settings_json(&mut self, json: &str) -> Result<(), JsValue> {
        let settings = EditorSettings::from_json(json)?;
        self.state.update_settings(settings);
        Ok(())
    }

    #[wasm_bindgen]
    pub fn get_cursor_position(&self) -> JsValue {
        let cursor = self.state.editor.cursor();
        let obj = js_sys::Object::new();
        js_sys::Reflect::set(&obj, &"line".into(), &(cursor.line + 1).into()).unwrap();
        js_sys::Reflect::set(&obj, &"column".into(), &(cursor.index + 1).into()).unwrap();
        obj.into()
    }
}

#[wasm_bindgen(start)]
pub fn main() -> Result<(), JsValue> {
    Ok(())
}
