mod config;
mod dictionary;
mod editor_core;
mod plugins;
mod translit;

use cosmic_text::{
    Action, Color, Cursor, Motion, Selection, Edit,
};
use unicode_segmentation::UnicodeSegmentation;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use web_sys::{CanvasRenderingContext2d, HtmlCanvasElement, ImageData, KeyboardEvent, MouseEvent, WheelEvent, TouchEvent};

use dictionary::lookup::Dictionary;
use editor_core::editor_state::EditorState;
use editor_core::events::{EditorEvent, KeyInfo};
use editor_core::history::EditKind;
use editor_core::plugin::{PluginContext, PluginRegistry};
use config::settings::EditorSettings;
use translit::renderer::TranslitRenderer;

// Padding from top and left for vertical text to prevent cursor being cut off
const VERTICAL_TOP_PADDING: i32 = 10;
const VERTICAL_LEFT_PADDING: i32 = 10;

// P2-02: gutter / line-number strip sizing. In horizontal mode the gutter
// is a left column of row numbers; in vertical mode (columns run
// top-to-bottom) it becomes a top strip of column numbers instead.
const GUTTER_WIDTH: i32 = 44;
const GUTTER_HEIGHT: i32 = 26;

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
    dictionary: Option<Dictionary>,
    translit_renderer: Option<TranslitRenderer>,
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
            dictionary: None,
            translit_renderer: None,
        })
    }

    /// Fan an event out to every plugin registered on `state`.
    ///
    /// The registry lives inside `state`, so we cannot borrow both at
    /// the same time. Instead we swap the registry out with an empty
    /// placeholder, dispatch on the owned copy, and swap it back. This
    /// keeps dispatch cheap (moves, no clones) while satisfying the
    /// borrow checker.
    fn dispatch_event(&mut self, event: EditorEvent) {
        let mut registry = std::mem::replace(&mut self.state.plugins, PluginRegistry::new());
        registry.dispatch_event(&mut self.state, &event);
        // Also notify raw bus subscribers (non-plugin listeners).
        self.state.events.dispatch(&event);
        // Restore the registry. If dispatch replaced state.plugins with
        // a fresh registry (it cannot today, but the API allows it),
        // preserve whatever landed there by extending.
        if self.state.plugins.plugin_count() == 0 {
            self.state.plugins = registry;
        } else {
            // Extremely unlikely; keep the newer registry and drop the
            // one we pulled out. Documented so future maintainers know
            // this branch is intentional.
            let _ = registry;
        }
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

    /// Left/top offset (in device pixels) applied to all drawn content:
    /// glyphs, cursor, selection, and match highlights (P2-02, P2-03).
    /// Widens whichever side the gutter occupies when line numbers are
    /// shown, so the gutter never overlaps text. Mouse/touch hit-testing
    /// must subtract the same values (see `handle_mouse_down` et al.)
    /// for cursor placement to line up with what's drawn.
    fn content_padding(&self) -> (i32, i32) {
        if !self.state.settings.editor.show_line_numbers {
            return (VERTICAL_LEFT_PADDING, VERTICAL_TOP_PADDING);
        }
        if self.state.settings.editor.orientation == "vertical" {
            (VERTICAL_LEFT_PADDING, VERTICAL_TOP_PADDING + GUTTER_HEIGHT)
        } else {
            (VERTICAL_LEFT_PADDING + GUTTER_WIDTH, VERTICAL_TOP_PADDING)
        }
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
        let (left_padding, top_padding) = self.content_padding();

        // P2-03: blend find/replace match highlights *before* drawing
        // glyphs/selection/cursor, so text renders on top of the tint
        // (matching how cosmic-text layers selection under glyphs in
        // `Editor::draw`).
        if !self.state.find_replace.matches.is_empty() {
            let current_color = Color::rgba(255, 165, 0, 130); // amber, current match
            let other_color = Color::rgba(255, 255, 0, 70); // pale yellow, other matches
            let ranges: Vec<(Cursor, Cursor, bool)> = self.state.find_replace.ranges().collect();
            self.state.editor.with_buffer(|buffer| {
                for (start, end, is_current) in ranges {
                    let color = if is_current { current_color } else { other_color };
                    for (rx, ry, rw, rh) in highlight_rects(buffer, start, end) {
                        let x = rx - scroll_offset.0 + left_padding;
                        let y = ry - scroll_offset.1 + top_padding;
                        blend_rect(&mut pixels, width_u32, height_u32, x, y, rw, rh, color);
                    }
                }
            });
        }

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
                let x = x - scroll_offset.0 + left_padding;
                let y = y - scroll_offset.1 + top_padding;
                blend_rect(&mut pixels, width_u32, height_u32, x, y, w, h, color);
            },
        );

        // Put pixels to canvas
        let image_data = ImageData::new_with_u8_clamped_array_and_sh(
            wasm_bindgen::Clamped(&pixels),
            self.width,
            self.height,
        )?;
        self.context.put_image_data(&image_data, 0.0, 0.0)?;

        // P2-02: gutter / line numbers, drawn on top via the 2D context
        // (plain digits don't need cosmic-text shaping).
        if self.state.settings.editor.show_line_numbers {
            self.draw_gutter(scroll_offset, left_padding, top_padding)?;
        }

        Ok(())
    }

    /// Draws the line-number gutter (horizontal mode: left column;
    /// vertical mode: top strip of column numbers). Only the first
    /// visual row of a wrapped source line is labeled, matching common
    /// editor convention.
    fn draw_gutter(&mut self, scroll_offset: (i32, i32), left_padding: i32, top_padding: i32) -> Result<(), JsValue> {
        let is_vertical = self.state.settings.editor.orientation == "vertical";
        let gutter_bg = self.state.settings.appearance.gutter_background;
        let line_num_color = self.state.settings.appearance.line_number_color;
        let width = self.width as f64;
        let height = self.height as f64;

        self.context.set_fill_style(
            &format!("rgb({}, {}, {})", gutter_bg.r(), gutter_bg.g(), gutter_bg.b()).into(),
        );
        if is_vertical {
            self.context.fill_rect(0.0, 0.0, width, GUTTER_HEIGHT as f64);
        } else {
            self.context.fill_rect(0.0, 0.0, GUTTER_WIDTH as f64, height);
        }

        // Collect (position-along-line, label) pairs first so we don't
        // hold a `with_buffer` borrow while also calling into
        // `self.context` (both are fields of `self`).
        let mut labels: Vec<(f32, String)> = Vec::new();
        self.state.editor.with_buffer(|buffer| {
            let mut last_line_i: Option<usize> = None;
            for run in buffer.layout_runs() {
                if last_line_i == Some(run.line_i) {
                    continue;
                }
                last_line_i = Some(run.line_i);
                let pos = if is_vertical {
                    run.line_top
                } else {
                    run.line_top + run.line_height / 2.0
                };
                labels.push((pos, (run.line_i + 1).to_string()));
            }
        });

        self.context.set_fill_style(
            &format!("rgb({}, {}, {})", line_num_color.r(), line_num_color.g(), line_num_color.b()).into(),
        );
        self.context.set_font("12px monospace");
        self.context.set_text_baseline(if is_vertical { "top" } else { "middle" });
        self.context.set_text_align(if is_vertical { "center" } else { "right" });

        for (pos, label) in labels {
            if is_vertical {
                let x = pos - scroll_offset.0 as f32 + left_padding as f32;
                if x < -20.0 || x as f64 > width + 20.0 {
                    continue;
                }
                let _ = self.context.fill_text(&label, x as f64, 6.0);
            } else {
                let y = pos - scroll_offset.1 as f32 + top_padding as f32;
                if y < -20.0 || y as f64 > height + 20.0 {
                    continue;
                }
                let _ = self
                    .context
                    .fill_text(&label, (GUTTER_WIDTH - 6) as f64, y as f64);
            }
        }

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

        // Dispatch a KeyPressed event first so plugins observe raw
        // input (P1-01). Handlers can inspect modifiers; they cannot
        // yet suppress the default Rust behavior below (that hook is
        // future work).
        self.dispatch_event(EditorEvent::KeyPressed(KeyInfo {
            key: key.clone(),
            ctrl: event.ctrl_key(),
            shift: event.shift_key(),
            alt: event.alt_key(),
            meta: event.meta_key(),
        }));

        let now = event.time_stamp();

        // Undo / redo (P2-01). Handled here (rather than in JS) since it
        // is purely internal editor state, unlike copy/cut/paste which
        // need the browser clipboard API.
        if self.ctrl_pressed && !event.alt_key() && (key == "z" || key == "Z") {
            event.prevent_default();
            if self.shift_pressed {
                self.state.redo();
            } else {
                self.state.undo();
            }
            self.state.cursor_visible = true;
            self.state.last_render_time = now;
            self.dispatch_event(EditorEvent::TextChanged);
            return Ok(());
        }
        // Ctrl+Y as an alias for redo (common on Windows).
        if self.ctrl_pressed && !event.alt_key() && (key == "y" || key == "Y") {
            event.prevent_default();
            self.state.redo();
            self.state.cursor_visible = true;
            self.state.last_render_time = now;
            self.dispatch_event(EditorEvent::TextChanged);
            return Ok(());
        }

        // Categorize the branch that actually runs so we can dispatch a
        // single coarse event at the end (P1-01). More granular events
        // will follow in later phases as consumers need them.
        #[derive(Copy, Clone, PartialEq, Eq)]
        enum KeyEffect {
            None,
            Motion,
            TextChange,
        }
        let mut effect = KeyEffect::None;

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
                self.state.flush_history_group();
                handle_motion(&mut self.state, Motion::Left, self.shift_pressed);
                event.prevent_default();
                effect = KeyEffect::Motion;
            }
            "ArrowRight" => {
                self.state.flush_history_group();
                handle_motion(&mut self.state, Motion::Right, self.shift_pressed);
                event.prevent_default();
                effect = KeyEffect::Motion;
            }
            "ArrowUp" => {
                self.state.flush_history_group();
                handle_motion(&mut self.state, Motion::Up, self.shift_pressed);
                event.prevent_default();
                effect = KeyEffect::Motion;
            }
            "ArrowDown" => {
                self.state.flush_history_group();
                handle_motion(&mut self.state, Motion::Down, self.shift_pressed);
                event.prevent_default();
                effect = KeyEffect::Motion;
            }
            "Home" => {
                self.state.flush_history_group();
                handle_motion(&mut self.state, Motion::Home, self.shift_pressed);
                event.prevent_default();
                effect = KeyEffect::Motion;
            }
            "End" => {
                self.state.flush_history_group();
                handle_motion(&mut self.state, Motion::End, self.shift_pressed);
                event.prevent_default();
                effect = KeyEffect::Motion;
            }
            "PageUp" => {
                self.state.flush_history_group();
                handle_motion(&mut self.state, Motion::PageUp, self.shift_pressed);
                event.prevent_default();
                effect = KeyEffect::Motion;
            }
            "PageDown" => {
                self.state.flush_history_group();
                handle_motion(&mut self.state, Motion::PageDown, self.shift_pressed);
                event.prevent_default();
                effect = KeyEffect::Motion;
            }
            "Backspace" => {
                // One `begin_history_group` call per keystroke, even
                // though `handle_backspace` may synthesize several
                // `Action::Backspace`/`Action::Delete` calls for
                // Mongolian format-control sequences - they all land in
                // the same `Change`, i.e. the same undo step.
                self.state.begin_history_group(EditKind::Delete, now);
                self.handle_backspace();
                event.prevent_default();
                effect = KeyEffect::TextChange;
            }
            "Delete" => {
                self.state.begin_history_group(EditKind::Delete, now);
                self.handle_delete();
                event.prevent_default();
                effect = KeyEffect::TextChange;
            }
            "Enter" => {
                self.state.begin_history_group(EditKind::Newline, now);
                self.state.editor.action(&mut self.state.font_system, Action::Enter);
                event.prevent_default();
                effect = KeyEffect::TextChange;
            }
            "Tab" => {
                self.state.begin_history_group(EditKind::Indent, now);
                self.state.editor.action(&mut self.state.font_system, Action::Indent);
                event.prevent_default();
                effect = KeyEffect::TextChange;
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
                            self.state.begin_history_group(EditKind::Insert, now);
                            self.state.editor.action(&mut self.state.font_system, Action::Insert(first_char));
                            event.prevent_default();
                            effect = KeyEffect::TextChange;
                        }
                    }
                }
            }
        }

        // Reset cursor blink
        self.state.cursor_visible = true;
        self.state.last_render_time = event.time_stamp();

        // Fire the coarse editor event corresponding to the branch we
        // took. `TextChange` implies the cursor also moved but we send
        // only the strongest event to keep bus traffic sane.
        match effect {
            KeyEffect::None => {}
            KeyEffect::Motion => self.dispatch_event(EditorEvent::CursorMoved),
            KeyEffect::TextChange => self.dispatch_event(EditorEvent::TextChanged),
        }

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
        let (left_padding, top_padding) = self.content_padding();
        let x = x + self.scroll_offset.0 - left_padding;
        let y = y + self.scroll_offset.1 - top_padding;

        // A click relocates the cursor; any in-progress typing group
        // should not merge with edits made at the new location (P2-01).
        self.state.flush_history_group();

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

        // Notify plugins the cursor moved due to a click.
        self.dispatch_event(EditorEvent::CursorMoved);

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
        let (left_padding, top_padding) = self.content_padding();
        let x = x + self.scroll_offset.0 - left_padding;
        let y = y + self.scroll_offset.1 - top_padding;

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
            let (left_padding, top_padding) = self.content_padding();
            let x = x + self.scroll_offset.0 - left_padding;
            let y = y + self.scroll_offset.1 - top_padding;

            // A tap relocates the cursor; don't merge subsequent typing
            // into whatever group was in progress (P2-01).
            self.state.flush_history_group();

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
                let (left_padding, top_padding) = self.content_padding();
                let x = x + self.scroll_offset.0 - left_padding;
                let y = y + self.scroll_offset.1 - top_padding;

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
        // Loading a new document invalidates undo history (P2-01) and
        // any in-progress find/replace matches (P2-03) - both refer to
        // positions in the *old* text.
        self.state.discard_history();
        self.state.find_replace.clear();
        // `set_text` is used both for programmatic edits and to seed a
        // loaded document; the plan (P1-01) treats this as a
        // document-load event so plugins can re-scan the buffer.
        self.dispatch_event(EditorEvent::DocumentLoaded { name: None });
    }

    pub fn insert_text(&mut self, text: &str) {
        // Programmatic bulk insert (paste, Latin->Mongolian conversion,
        // transliteration insert, ...). Always its own atomic undo step
        // (P2-01), never coalesced with adjacent typing.
        self.state.flush_history_group();
        self.state.begin_history_group(EditKind::Paste, js_sys::Date::now());
        for ch in text.chars() {
            self.state.editor.action(&mut self.state.font_system, Action::Insert(ch));
        }
        self.state.flush_history_group();
        self.dispatch_event(EditorEvent::TextChanged);
    }

    pub fn get_selected_text(&self) -> JsValue {
        match self.state.editor.copy_selection() {
            Some(text) => JsValue::from_str(&text),
            None => JsValue::NULL,
        }
    }

    pub fn delete_selection(&mut self) {
        self.state.flush_history_group();
        self.state.begin_history_group(EditKind::Paste, js_sys::Date::now());
        self.state.editor.action(&mut self.state.font_system, Action::Backspace);
        self.state.flush_history_group();
        self.dispatch_event(EditorEvent::TextChanged);
    }

    // ==========================================================================
    // Undo / redo (P2-01)
    // ==========================================================================

    /// Undo the most recent change. Returns `true` if something was
    /// undone. Bound to `Ctrl+Z` in `handle_key_down`; also exposed here
    /// for the command palette / toolbar.
    #[wasm_bindgen]
    pub fn undo(&mut self) -> bool {
        let did = self.state.undo();
        if did {
            self.state.cursor_visible = true;
            self.dispatch_event(EditorEvent::TextChanged);
        }
        did
    }

    /// Redo the most recently undone change. Returns `true` if something
    /// was redone.
    #[wasm_bindgen]
    pub fn redo(&mut self) -> bool {
        let did = self.state.redo();
        if did {
            self.state.cursor_visible = true;
            self.dispatch_event(EditorEvent::TextChanged);
        }
        did
    }

    #[wasm_bindgen]
    pub fn can_undo(&self) -> bool {
        self.state.can_undo()
    }

    #[wasm_bindgen]
    pub fn can_redo(&self) -> bool {
        self.state.can_redo()
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

        self.dispatch_event(EditorEvent::OrientationToggled);
        self.dispatch_event(EditorEvent::SettingsChanged);
    }

    #[wasm_bindgen]
    pub fn get_settings_json(&self) -> Result<String, JsValue> {
        self.state.settings.to_json()
    }

    #[wasm_bindgen]
    pub fn set_settings_json(&mut self, json: &str) -> Result<(), JsValue> {
        let settings = EditorSettings::from_json(json)?;
        self.state.update_settings(settings);
        self.dispatch_event(EditorEvent::SettingsChanged);
        Ok(())
    }

    /// Resets settings to the built-in defaults, applies them, and returns the
    /// resulting JSON so the JS layer can update its UI and localStorage
    /// without duplicating the default values. See P0-01 in
    /// `prompt/FUTURE_PLAN.md`.
    #[wasm_bindgen]
    pub fn reset_to_defaults(&mut self) -> Result<String, JsValue> {
        let settings = EditorSettings::default();
        let json = settings.to_json()?;
        self.state.update_settings(settings);
        self.dispatch_event(EditorEvent::SettingsChanged);
        Ok(json)
    }

    /// Returns the built-in default settings as JSON without mutating the
    /// editor state. Useful for previewing defaults or seeding local UI.
    #[wasm_bindgen]
    pub fn get_default_settings_json(&self) -> Result<String, JsValue> {
        EditorSettings::default().to_json()
    }

    /// Returns the built-in theme presets (P4-01) as a JSON array of
    /// `{ id, name, appearance: { text_color, background_color, ... } }`.
    /// Does not mutate editor state - the JS Color tab applies a theme by
    /// merging the chosen entry's `appearance` into the current settings
    /// and calling `set_settings_json`, the same "merge on save" pattern
    /// used for every other settings field (see P0-02/P0-03).
    #[wasm_bindgen]
    pub fn list_themes(&self) -> Result<String, JsValue> {
        serde_json::to_string(&config::themes::built_in_themes())
            .map_err(|e| JsValue::from_str(&e.to_string()))
    }

    // ==========================================================================
    // Plugin system: command palette bindings (P1-03)
    // ==========================================================================

    /// Returns the list of registered commands as JSON:
    /// `[{ "id": ..., "title": ..., "category": ...|null, "keybinding": ...|null }, ...]`
    /// The order matches plugin registration order.
    #[wasm_bindgen]
    pub fn list_commands(&self) -> Result<JsValue, JsValue> {
        let arr = js_sys::Array::new();
        for cmd in self.state.plugins.commands() {
            let obj = js_sys::Object::new();
            js_sys::Reflect::set(&obj, &"id".into(), &JsValue::from_str(cmd.id))?;
            js_sys::Reflect::set(&obj, &"title".into(), &JsValue::from_str(cmd.title))?;
            let cat = match cmd.category {
                Some(c) => JsValue::from_str(c),
                None => JsValue::NULL,
            };
            js_sys::Reflect::set(&obj, &"category".into(), &cat)?;
            let kb = match cmd.keybinding {
                Some(k) => JsValue::from_str(k),
                None => JsValue::NULL,
            };
            js_sys::Reflect::set(&obj, &"keybinding".into(), &kb)?;
            arr.push(&obj);
        }
        Ok(arr.into())
    }

    /// Invoke a registered command by id. `args_json` is opaque to the
    /// registry and forwarded to the handler untouched; pass `""` when
    /// unused. Returns whatever the handler returns (typically
    /// `JsValue::NULL`).
    ///
    /// Errors:
    /// * `"Unknown command: <id>"` when no command matches.
    /// * Any error the handler itself raises.
    #[wasm_bindgen]
    pub fn run_command(&mut self, id: &str, args_json: &str) -> Result<JsValue, JsValue> {
        // Copy the handler pointer out so we can release the immutable
        // borrow on the registry before running the handler (which
        // wants a mutable borrow of `state`).
        let handler = match self.state.plugins.find_command(id) {
            Some(cmd) => cmd.handler,
            None => {
                return Err(JsValue::from_str(&format!("Unknown command: {}", id)));
            }
        };

        let result = {
            let mut ctx = PluginContext::new(&mut self.state);
            handler(&mut ctx, args_json)
        };

        // Commands may mutate anything; dispatch a coarse
        // SettingsChanged/TextChanged pair so plugins observing state
        // stay in sync. Cheaper than reflecting per-command intent for
        // now; can be refined once plugins declare their side effects.
        if result.is_ok() {
            self.dispatch_event(EditorEvent::SettingsChanged);
        }

        result
    }

    #[wasm_bindgen]
    pub fn get_cursor_position(&self) -> JsValue {
        let cursor = self.state.editor.cursor();
        let obj = js_sys::Object::new();
        js_sys::Reflect::set(&obj, &"line".into(), &(cursor.line + 1).into()).unwrap();
        js_sys::Reflect::set(&obj, &"column".into(), &(cursor.index + 1).into()).unwrap();
        obj.into()
    }

    /// Move the cursor to a 1-based `(line, column)` position, the same
    /// convention `get_cursor_position` returns. Used by auto-save draft
    /// recovery (P2-05) to restore where the user was editing; clamps
    /// silently to valid buffer bounds via cosmic-text's own cursor
    /// handling rather than erroring on stale positions.
    #[wasm_bindgen]
    pub fn set_cursor_position(&mut self, line: usize, column: usize) {
        let line = line.saturating_sub(1);
        let index = column.saturating_sub(1);
        self.state.editor.set_cursor(Cursor::new(line, index));
        self.state.cursor_visible = true;
    }

    // ================= Cyrillic -> Mongolian transliteration =================

    /// Fetches and parses the dictionary TSV. Safe to call multiple times; the
    /// actual network + parse work happens only on the first call. Subsequent
    /// calls resolve immediately.
    #[wasm_bindgen]
    pub async fn translit_load_dictionary(&mut self, url: String) -> Result<(), JsValue> {
        if self.dictionary.is_some() {
            return Ok(());
        }
        let dict = Dictionary::load(&url).await?;
        self.dictionary = Some(dict);
        Ok(())
    }

    /// Returns true if the dictionary has been loaded.
    #[wasm_bindgen]
    pub fn translit_is_loaded(&self) -> bool {
        self.dictionary.is_some()
    }

    /// Looks up a Cyrillic word and returns
    /// `{ found: bool, mongolian: string | null, variants: number }`.
    #[wasm_bindgen]
    pub fn translit_lookup(&self, cyrillic: &str) -> JsValue {
        let obj = js_sys::Object::new();
        let mut found = false;
        let mut mongolian: Option<&str> = None;
        let mut variants: u32 = 0;

        if let Some(dict) = &self.dictionary {
            if let Some(entries) = dict.lookup(cyrillic) {
                if !entries.is_empty() {
                    found = true;
                    mongolian = Some(entries[0].as_str());
                    variants = entries.len() as u32;
                }
            }
        }

        js_sys::Reflect::set(&obj, &"found".into(), &found.into()).unwrap();
        js_sys::Reflect::set(
            &obj,
            &"mongolian".into(),
            &match mongolian {
                Some(s) => JsValue::from_str(s),
                None => JsValue::NULL,
            },
        )
        .unwrap();
        js_sys::Reflect::set(&obj, &"variants".into(), &variants.into()).unwrap();
        obj.into()
    }

    /// Sets up (or resizes) the offscreen translit renderer bound to the
    /// given canvas element.
    #[wasm_bindgen]
    pub fn translit_init_canvas(
        &mut self,
        canvas_id: &str,
        width: u32,
        height: u32,
    ) -> Result<(), JsValue> {
        // Verify the canvas exists so JS gets a meaningful error early.
        let window = web_sys::window().ok_or("No window")?;
        let document = window.document().ok_or("No document")?;
        document
            .get_element_by_id(canvas_id)
            .ok_or_else(|| JsValue::from_str("Translit canvas not found"))?
            .dyn_into::<HtmlCanvasElement>()?;

        match &mut self.translit_renderer {
            Some(renderer) => {
                renderer.resize(&mut self.state.font_system, width, height);
                renderer.apply_settings(&mut self.state.font_system, &self.state.settings);
            }
            None => {
                self.translit_renderer = Some(TranslitRenderer::new(
                    &mut self.state.font_system,
                    &self.state.settings,
                    width,
                    height,
                ));
            }
        }
        Ok(())
    }

    /// Renders the given Mongolian text vertically onto the translit canvas.
    #[wasm_bindgen]
    pub fn translit_render(&mut self, canvas_id: &str, text: &str) -> Result<(), JsValue> {
        let window = web_sys::window().ok_or("No window")?;
        let document = window.document().ok_or("No document")?;
        let canvas = document
            .get_element_by_id(canvas_id)
            .ok_or_else(|| JsValue::from_str("Translit canvas not found"))?
            .dyn_into::<HtmlCanvasElement>()?;

        let renderer = self
            .translit_renderer
            .as_mut()
            .ok_or_else(|| JsValue::from_str("Translit renderer not initialized"))?;

        renderer.apply_settings(&mut self.state.font_system, &self.state.settings);
        renderer.set_text(&mut self.state.font_system, &self.state.settings, text);
        renderer.render(
            &mut self.state.font_system,
            &mut self.state.cache,
            &self.state.settings,
            &canvas,
        )
    }
}

/// Alpha-blend a filled rectangle into an RGBA8 pixel buffer, clipped to
/// its bounds. Shared by glyph rendering, the find/replace highlight
/// overlay (P2-03), and (indirectly, via the same call convention) any
/// future decoration layer that wants to paint into the same buffer
/// `WasmEditor::render` builds each frame.
fn blend_rect(
    pixels: &mut [u8],
    width_u32: u32,
    height_u32: u32,
    x: i32,
    y: i32,
    w: u32,
    h: u32,
    color: Color,
) {
    if x + w as i32 <= 0 || x >= width_u32 as i32 || y + h as i32 <= 0 || y >= height_u32 as i32 {
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

                pixels[idx] =
                    ((color.r() as f32 * src_alpha) + (dst_r as f32 * (1.0 - src_alpha))) as u8;
                pixels[idx + 1] =
                    ((color.g() as f32 * src_alpha) + (dst_g as f32 * (1.0 - src_alpha))) as u8;
                pixels[idx + 2] =
                    ((color.b() as f32 * src_alpha) + (dst_b as f32 * (1.0 - src_alpha))) as u8;
                pixels[idx + 3] = 255;
            }
        }
    }
}

/// Compute pixel rectangles (in un-scrolled, un-padded buffer space - the
/// same space the `render` draw callback receives *before* it applies
/// `scroll_offset` / content padding) that visually cover the cursor
/// range `start..end`.
///
/// This mirrors the per-glyph selection-highlight algorithm in
/// `cosmic_text::Editor::draw` (see `cosmic-text/src/edit/editor.rs`),
/// generalized from "the current selection" to an arbitrary range so it
/// can be reused for find/replace match highlights (P2-03). Kept as a
/// free function (rather than upstreamed into cosmic-text) since it is
/// specific to this overlay's needs.
fn highlight_rects(
    buffer: &cosmic_text::Buffer,
    start: Cursor,
    end: Cursor,
) -> Vec<(i32, i32, u32, u32)> {
    let (start, end) = if start <= end { (start, end) } else { (end, start) };
    let mut rects = Vec::new();

    for run in buffer.layout_runs() {
        let line_i = run.line_i;
        if line_i < start.line || line_i > end.line {
            continue;
        }

        let line_top = run.line_top;
        let line_height = run.line_height;
        let is_vertical = run.orientation.is_vertical();

        // Vertical mode: the highlighted band's cross-axis (x) position
        // and width are fixed for the whole run (the column), only the
        // along-axis (y) extent varies per glyph below.
        let (band_x, band_w) = if is_vertical {
            let x_offset = run.glyphs.first().map_or(0.0, |g| g.x);
            let glyph_width = run.max_ascent + run.max_descent;
            let centering = (line_height - glyph_width) / 2.0;
            (line_top + x_offset + centering, glyph_width)
        } else {
            (0.0, line_height)
        };

        let mut range_opt: Option<(i32, i32)> = None;
        let flush = |range_opt: &mut Option<(i32, i32)>, rects: &mut Vec<(i32, i32, u32, u32)>| {
            if let Some((min, max)) = range_opt.take() {
                let extent = (max - min).max(0) as u32;
                if extent > 0 {
                    if is_vertical {
                        rects.push((band_x as i32, min, band_w as u32, extent));
                    } else {
                        rects.push((min, line_top as i32, extent, line_height as u32));
                    }
                }
            }
        };

        for glyph in run.glyphs {
            let cluster = &run.text[glyph.start..glyph.end];
            let total = cluster.grapheme_indices(true).count().max(1);
            let mut c_pos = if is_vertical { glyph.y } else { glyph.x };
            let c_w = glyph.w / total as f32;
            for (i, c) in cluster.grapheme_indices(true) {
                let c_start = glyph.start + i;
                let c_end = glyph.start + i + c.len();
                if (start.line != line_i || c_end > start.index)
                    && (end.line != line_i || c_start < end.index)
                {
                    range_opt = match range_opt.take() {
                        Some((min, max)) => Some((
                            min.min(c_pos as i32),
                            max.max((c_pos + c_w) as i32),
                        )),
                        None => Some((c_pos as i32, (c_pos + c_w) as i32)),
                    };
                } else {
                    flush(&mut range_opt, &mut rects);
                }
                c_pos += c_w;
            }
        }
        flush(&mut range_opt, &mut rects);
    }

    rects
}

#[wasm_bindgen(start)]
pub fn main() -> Result<(), JsValue> {
    Ok(())
}
