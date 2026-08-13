mod config;
mod dictionary;
mod editor_core;
mod plugins;
mod translit;

use cosmic_text::{
    Action, Attrs, Buffer, Color, Cursor, Family, FontSystem, Metrics, Motion, Selection,
    Shaping, SwashCache, TextOrientation, Edit,
};
use unicode_segmentation::UnicodeSegmentation;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use web_sys::{CanvasRenderingContext2d, HtmlCanvasElement, ImageData, KeyboardEvent, MouseEvent, WheelEvent, TouchEvent};

use dictionary::lookup::Dictionary;
use editor_core::editor_state::EditorState;
use editor_core::events::{EditorEvent, KeyInfo};
use editor_core::format_control;
use editor_core::history::EditKind;
use editor_core::plugin::{PluginContext, PluginRegistry};
use editor_core::suggestion_popup_layout::{self, ItemMetrics, LayoutConfig, PopupLayout};
use editor_core::word_boundary::{self, Script};
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

// Phase P9: don't show the word suggestion popup until the word being
// typed is at least this many characters - avoids a distracting popup
// after every single keystroke at the very start of a word (see
// `prompt/WORD_SUGGESTIONS_PLAN.md` §9).
const MIN_SUGGESTION_WORD_LEN: usize = 2;

// WS-09: word-suggestion popup layout constants, in the same
// device-pixel space `settings.fonts.font_size` and
// `get_word_suggestions_json`'s `anchorX`/`anchorY`/`lineAdvance`
// already use (no DPR division happens until JS converts to CSS
// pixels at the very end - see `suggestion_popup_layout`'s module doc
// comment). Chosen to visually approximate the `px-2 py-2 gap-1`
// Tailwind spacing the previous DOM-based popup used.
const SUGGESTION_CELL_PADDING_X: f32 = 10.0;
const SUGGESTION_CELL_PADDING_Y: f32 = 10.0;
const SUGGESTION_NUMBER_WORD_GAP: f32 = 6.0;
const SUGGESTION_MAX_ROW_WIDTH: f32 = 320.0;
// The number badge's line height, as a fixed multiple of its own font
// size (computed per-call from the editor's real font size - see
// `render_suggestions_popup`'s note on why the number stays visually
// secondary rather than matching the word's size 1:1).
const SUGGESTION_NUMBER_LINE_HEIGHT_RATIO: f32 = 1.2;

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
    /// WS-09: the last layout `measure_suggestions_popup()` computed,
    /// consumed (not recomputed) by the very next
    /// `render_suggestions_popup()` call - see that method's doc
    /// comment for why measuring twice would be both wasteful and a
    /// consistency risk (JS sizes the canvas from the *measured*
    /// layout; drawing must use those exact same rectangles, not a
    /// freshly recomputed one that could drift by a pixel due to
    /// float rounding).
    suggestion_popup_cache: Option<SuggestionPopupCache>,
    /// P7-01: set by every method that changes what the main canvas
    /// should look like; cleared at the end of `render()`. Consulted by
    /// `needs_render()`, which the JS `animate()` loop calls every RAF
    /// tick to decide whether to actually call `render()` this frame -
    /// previously `render()` (which rebuilds a full `width*height*4`
    /// pixel buffer and calls `put_image_data`) ran unconditionally on
    /// every tick, ~60 times a second, even while the document was
    /// completely idle.
    dirty: bool,
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
            suggestion_popup_cache: None,
            dirty: true,
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
        self.dirty = true;
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

    /// True if the JS `animate()` loop should actually call `render()`
    /// this frame (P7-01). Two reasons to render: something the user
    /// did (or a programmatic change) marked the frame dirty, or the
    /// cursor's 500ms blink interval has elapsed - the latter check
    /// mirrors the one `render()` itself does internally, so the
    /// cursor keeps blinking at its usual rate even while otherwise
    /// idle. Cheap to call every RAF tick: no pixel work, just a bool
    /// and a float comparison.
    #[wasm_bindgen]
    pub fn needs_render(&self, timestamp: f64) -> bool {
        self.dirty || (timestamp - self.state.last_render_time > 500.0)
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
        self.context.set_fill_style_str(&format!("rgb({}, {}, {})", bg.r(), bg.g(), bg.b()));
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

        // P7-01: this frame is now up to date; the next call to
        // `needs_render()` should return `false` until something
        // changes again (or the next blink is due).
        self.dirty = false;

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

        self.context.set_fill_style_str(
            &format!("rgb({}, {}, {})", gutter_bg.r(), gutter_bg.g(), gutter_bg.b()),
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

        self.context.set_fill_style_str(
            &format!("rgb({}, {}, {})", line_num_color.r(), line_num_color.g(), line_num_color.b()),
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

    /// Run a [`format_control::DeletePlan`] against the live editor:
    /// all backspaces, then all deletes (see the module doc comment on
    /// why that fixed order covers every plan this module produces).
    fn apply_delete_plan(&mut self, plan: format_control::DeletePlan) {
        for _ in 0..plan.backspaces {
            self.state.editor.action(&mut self.state.font_system, Action::Backspace);
        }
        for _ in 0..plan.deletes {
            self.state.editor.action(&mut self.state.font_system, Action::Delete);
        }
    }

    /// Get the current line's characters and the cursor's
    /// character-index position within it (as opposed to
    /// `cursor.index`, which is a *byte* offset) - the shared setup
    /// `handle_backspace`/`handle_delete` both need before consulting
    /// `format_control::backspace_plan`/`delete_plan`.
    fn current_line_chars_and_cursor(&self) -> Option<(Vec<char>, usize)> {
        let cursor = self.state.editor.cursor();
        let cursor_byte_index = cursor.index;
        let cursor_line = cursor.line;

        let line_text = self.state.editor.with_buffer(|buffer| {
            buffer.lines.get(cursor_line).map(|line| line.text().to_string())
        })?;

        let char_index = line_text[..cursor_byte_index.min(line_text.len())]
            .chars()
            .count();
        let chars: Vec<char> = line_text.chars().collect();
        Some((chars, char_index))
    }

    // Handle backspace with Mongolian format-control-character
    // awareness. Decision logic lives in `editor_core::format_control`
    // (extracted and unit-tested in P7-03); this just wires the plan it
    // produces up to the live cosmic-text `Editor`.
    fn handle_backspace(&mut self) {
        let plan = match self.current_line_chars_and_cursor() {
            Some((chars, cursor_index)) => format_control::backspace_plan(&chars, cursor_index),
            None => format_control::DeletePlan { backspaces: 1, deletes: 0 },
        };
        self.apply_delete_plan(plan);
    }

    // Handle delete with Mongolian format-control-character awareness.
    // See `handle_backspace` above.
    fn handle_delete(&mut self) {
        let plan = match self.current_line_chars_and_cursor() {
            Some((chars, cursor_index)) => format_control::delete_plan(&chars, cursor_index),
            None => format_control::DeletePlan { backspaces: 0, deletes: 1 },
        };
        self.apply_delete_plan(plan);
    }

    /// Returns whether this keystroke actually changed the buffer text
    /// (as opposed to just moving the cursor/selection, or doing
    /// nothing). JS uses this to decide whether to refresh the word
    /// suggestion popup or dismiss it - per the trigger table in
    /// `WORD_SUGGESTIONS_PLAN.md` §7, pure cursor movement (arrow keys,
    /// Home/End, PageUp/Down) should dismiss the popup (the word
    /// context changed), not refresh it, since the user isn't actively
    /// typing.
    pub fn handle_key_down(&mut self, event: KeyboardEvent) -> Result<bool, JsValue> {
        // P7-01: set unconditionally rather than per-branch - every
        // branch below either changes the cursor/selection/text, or is
        // a no-op (e.g. Ctrl+C) cheap enough that an extra unnecessary
        // render isn't worth the risk of missing a spot.
        self.dirty = true;
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
            return Ok(true);
        }
        // Ctrl+Y as an alias for redo (common on Windows).
        if self.ctrl_pressed && !event.alt_key() && (key == "y" || key == "Y") {
            event.prevent_default();
            self.state.redo();
            self.state.cursor_visible = true;
            self.state.last_render_time = now;
            self.dispatch_event(EditorEvent::TextChanged);
            return Ok(true);
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
                    return Ok(false);
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

        Ok(effect == KeyEffect::TextChange)
    }

    pub fn handle_key_up(&mut self, event: KeyboardEvent) {
        self.shift_pressed = event.shift_key();
        self.ctrl_pressed = event.ctrl_key() || event.meta_key();
    }

    pub fn handle_mouse_down(&mut self, event: MouseEvent) -> Result<(), JsValue> {
        self.dirty = true; // P7-01
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
        self.dirty = true; // P7-01: only past this point does drag actually extend the selection.

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
        self.dirty = true; // P7-01
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
        self.dirty = true; // P7-01
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
        self.dirty = true; // P7-01
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
        self.dirty = true; // P7-01
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
        // Loading a new document invalidates undo history (P2-01), any
        // in-progress find/replace matches (P2-03), and any word
        // suggestions (P9) - all three refer to positions in the *old*
        // text.
        self.state.discard_history();
        self.state.find_replace.clear();
        self.state.suggestions.clear();
        // `set_text` is used both for programmatic edits and to seed a
        // loaded document; the plan (P1-01) treats this as a
        // document-load event so plugins can re-scan the buffer.
        self.dispatch_event(EditorEvent::DocumentLoaded { name: None });
    }

    pub fn insert_text(&mut self, text: &str) {
        self.dirty = true; // P7-01
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
        self.dirty = true; // P7-01
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
            self.dirty = true; // P7-01
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
            self.dirty = true; // P7-01
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
        self.dirty = true; // P7-01
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
        self.dirty = true; // P7-01
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
        self.dirty = true; // P7-01
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

    /// Returns the default app-level keybindings (P6-01) as a JSON
    /// array of `{ id, label, default }`. Does not mutate editor
    /// state - the JS `KeybindingManager` layers localStorage overrides
    /// on top and owns the actual `KeyboardEvent` matching; Rust is
    /// only the source of truth for what ships out of the box. See
    /// `src/config/keybindings.rs` for the full scope rationale (only
    /// app-level shortcuts are covered, not low-level text-editing
    /// keys like undo/redo).
    #[wasm_bindgen]
    pub fn list_keybindings(&self) -> Result<String, JsValue> {
        serde_json::to_string(&config::keybindings::default_keybindings())
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
        // P7-01: commands can mutate practically anything (text,
        // settings, selection - see the plugin API), so mark dirty
        // unconditionally rather than trying to enumerate every
        // built-in and future command's visual effect.
        self.dirty = true;
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
        self.dirty = true; // P7-01
        let line = line.saturating_sub(1);
        let index = column.saturating_sub(1);
        self.state.editor.set_cursor(Cursor::new(line, index));
        self.state.cursor_visible = true;
    }

    // ================= Word suggestion popup (Phase P9) =================
    //
    // Shares `self.dictionary` with the Transliteration modal below -
    // whichever feature the user reaches for first triggers the lazy
    // load (`translit_load_dictionary`); there's only ever one
    // dictionary instance, not a separate one per feature. See
    // `prompt/WORD_SUGGESTIONS_PLAN.md` §6/§8.

    /// Builds the `{ hasSuggestions: false, ... }` result shared by
    /// every early-return branch of `get_word_suggestions_json` below.
    fn no_word_suggestions() -> Result<JsValue, JsValue> {
        let obj = js_sys::Object::new();
        js_sys::Reflect::set(&obj, &"hasSuggestions".into(), &false.into())?;
        js_sys::Reflect::set(&obj, &"suggestions".into(), &js_sys::Array::new())?;
        js_sys::Reflect::set(&obj, &"anchorX".into(), &0.into())?;
        js_sys::Reflect::set(&obj, &"anchorY".into(), &0.into())?;
        js_sys::Reflect::set(&obj, &"lineAdvance".into(), &0.into())?;
        Ok(obj.into())
    }

    /// Recomputes the word suggestion popup's state for the current
    /// cursor position. Returns
    /// `{ hasSuggestions, suggestions: string[], anchorX, anchorY, lineAdvance }` -
    /// `anchorX`/`anchorY` are the cursor's position in the same
    /// canvas-pixel space `render()` draws into (`buffer_pixel -
    /// scroll_offset + content_padding`, cosmic-text's own
    /// `cursor_position()` composing directly with the same transform
    /// used everywhere else in this file). JS converts that to CSS
    /// position via `devicePixelRatio` and the canvas's
    /// `getBoundingClientRect()` - the exact inverse of what
    /// `handle_mouse_down` et al. already do the other direction.
    /// `lineAdvance` is the line/column spacing at the cursor, also in
    /// canvas-pixel space - JS uses it to offset the popup a full
    /// line/column clear of the cursor rather than guessing a fixed
    /// pixel gap (which in vertical mode landed inside the *next*
    /// column's territory instead of a clean gap beside the current
    /// one).
    ///
    /// Returns `hasSuggestions: false` (never an error) if the feature
    /// is disabled in settings, the dictionary hasn't loaded yet, or
    /// the cursor isn't inside/adjacent to a word at least
    /// `MIN_SUGGESTION_WORD_LEN` characters long - all "nothing to show
    /// right now," not failure conditions.
    #[wasm_bindgen]
    pub fn get_word_suggestions_json(&mut self) -> Result<JsValue, JsValue> {
        self.state.suggestions.clear();

        if !self.state.settings.editor.word_suggestions_enabled {
            return Self::no_word_suggestions();
        }
        let Some(dictionary) = &self.dictionary else {
            return Self::no_word_suggestions();
        };

        // This is called synchronously right after `handle_key_down`
        // inserts a character - *before* the next `render()` call (the
        // only other place that shapes the buffer) has a chance to run
        // on the following animation frame. Without this,
        // `cursor_position()` below reads stale layout runs from
        // whatever was last shaped (the state *before* this keystroke,
        // or even the empty buffer if this is the very first edit), so
        // the popup's anchor position would silently fall back to a
        // stale/default value instead of tracking the cursor - visible
        // as the popup appearing "stuck" in the same spot no matter how
        // much more is typed. `shape_as_needed` is the same call
        // `render()` makes and is a no-op if nothing changed, so this
        // is safe to call unconditionally here.
        self.state.editor.shape_as_needed(&mut self.state.font_system, false);

        let cursor = self.state.editor.cursor();
        let cursor_line = cursor.line;
        let cursor_byte_index = cursor.index;

        let line_text = self.state.editor.with_buffer(|buffer| {
            buffer
                .lines
                .get(cursor_line)
                .map(|line| line.text().to_string())
        });
        let Some(line_text) = line_text else {
            return Self::no_word_suggestions();
        };

        let chars: Vec<char> = line_text.chars().collect();
        let char_index = line_text[..cursor_byte_index.min(line_text.len())]
            .chars()
            .count();

        let Some((start_char, end_char)) = word_boundary::current_word_bounds(&chars, char_index)
        else {
            return Self::no_word_suggestions();
        };
        if end_char - start_char < MIN_SUGGESTION_WORD_LEN {
            return Self::no_word_suggestions();
        }

        let word_chars = &chars[start_char..end_char];
        let Some(script) = word_boundary::classify_script(word_chars) else {
            return Self::no_word_suggestions();
        };
        if script == Script::Other {
            return Self::no_word_suggestions();
        }

        let word: String = word_chars.iter().collect();
        let suggestions = dictionary.suggest(&word, script);
        if suggestions.is_empty() {
            return Self::no_word_suggestions();
        }

        // Character-index bounds -> byte-index Cursor positions, for
        // accept_word_suggestion's delete_range/insert_at below.
        let byte_index_of = |char_idx: usize| -> usize {
            line_text
                .char_indices()
                .nth(char_idx)
                .map(|(b, _)| b)
                .unwrap_or(line_text.len())
        };
        self.state.suggestions.word_start = Some(Cursor::new(cursor_line, byte_index_of(start_char)));
        self.state.suggestions.word_end = Some(Cursor::new(cursor_line, byte_index_of(end_char)));
        self.state.suggestions.suggestions = suggestions;

        let (buffer_x, buffer_y) = self.state.editor.cursor_position().unwrap_or((0, 0));
        let (left_padding, top_padding) = self.content_padding();
        let anchor_x = buffer_x - self.scroll_offset.0 + left_padding;
        let anchor_y = buffer_y - self.scroll_offset.1 + top_padding;

        // The distance from one line to the next in horizontal mode -
        // or, equally, from one *column* to the next in vertical mode,
        // since cosmic-text's vertical layout reuses the same
        // `line_height` metric as the spacing between successive
        // columns (see `draw_gutter`'s use of the same value for
        // column-label placement). JS uses this to offset the popup a
        // full column/line clear of the cursor's own line - a fixed
        // guess here previously landed inside the *next* column's
        // territory in vertical mode instead of a clean gap beside the
        // current one (see git history for the visual bug this fixed).
        let line_advance = self
            .state
            .editor
            .with_buffer(|buffer| {
                buffer
                    .layout_runs()
                    .find(|run| run.line_i == cursor_line)
                    .map(|run| run.line_height)
            })
            .unwrap_or(24.0);

        let arr = js_sys::Array::new();
        for s in &self.state.suggestions.suggestions {
            arr.push(&JsValue::from_str(s));
        }
        let obj = js_sys::Object::new();
        js_sys::Reflect::set(&obj, &"hasSuggestions".into(), &true.into())?;
        js_sys::Reflect::set(&obj, &"suggestions".into(), &arr)?;
        js_sys::Reflect::set(&obj, &"anchorX".into(), &(anchor_x as f64).into())?;
        js_sys::Reflect::set(&obj, &"anchorY".into(), &(anchor_y as f64).into())?;
        js_sys::Reflect::set(&obj, &"lineAdvance".into(), &(line_advance as f64).into())?;
        Ok(obj.into())
    }

    /// Accepts a suggestion, replacing the word it was computed for
    /// with `text`. Re-resolves nothing beyond what
    /// `get_word_suggestions_json` already stored (`word_start`/
    /// `word_end`) - if the buffer changed since then in a way that
    /// invalidates those positions, cosmic-text's own range handling
    /// degrades gracefully rather than panicking, and the popup is
    /// always dismissed by the JS layer's own trigger logic before the
    /// buffer could change again out from under it (see
    /// `WORD_SUGGESTIONS_PLAN.md` §7's trigger table). One atomic undo
    /// step, same convention as every other programmatic multi-char
    /// edit in this codebase (P2-01, P2-03).
    #[wasm_bindgen]
    pub fn accept_word_suggestion(&mut self, text: &str) -> Result<(), JsValue> {
        self.dirty = true; // P7-01
        let (Some(start), Some(end)) =
            (self.state.suggestions.word_start, self.state.suggestions.word_end)
        else {
            return Ok(());
        };

        self.state.begin_history_group(EditKind::Paste, js_sys::Date::now());
        self.state.editor.delete_range(start, end);
        let new_cursor = self.state.editor.insert_at(start, text, None);
        self.state.flush_history_group();

        self.state.editor.set_cursor(new_cursor);
        self.state.suggestions.clear();
        self.dispatch_event(EditorEvent::TextChanged);
        Ok(())
    }

    /// Clears the popup's state without changing the buffer - called on
    /// Escape, on cursor-moving keys/clicks, and on document-changing
    /// actions (tab switch, clear, open) per the trigger table in
    /// `WORD_SUGGESTIONS_PLAN.md` §7.
    #[wasm_bindgen]
    pub fn dismiss_word_suggestions(&mut self) {
        self.state.suggestions.clear();
    }

    /// WS-09: measures the popup's current suggestion list (whatever
    /// `get_word_suggestions_json` last stored in
    /// `self.state.suggestions.suggestions`) using real `cosmic-text`
    /// shaping - the same shaping/rasterization pipeline the main
    /// document canvas uses, so the popup's Mongolian glyphs are
    /// visually identical to the document regardless of which browser
    /// this runs in, rather than depending on the browser's own
    /// (inconsistent, sometimes absent) support for shaping Mongolian
    /// text under CSS `writing-mode`/`text-orientation`.
    ///
    /// Returns `{ width, height, itemBounds: [{x,y,w,h}, ...] }` in the
    /// same device-pixel space as `get_word_suggestions_json`'s
    /// `anchorX`/`anchorY`/`lineAdvance`. JS resizes the popup's
    /// `<canvas>` to `width`/`height` (after its own DPR conversion)
    /// and uses `itemBounds` for click/hover hit-testing - `bounds` in
    /// each entry is stretched to the popup's full cross-axis extent
    /// (see `suggestion_popup_layout`'s doc comments), so click/hover
    /// targets are comfortably sized, not just the glyphs' own tight
    /// box.
    ///
    /// Deliberately does *not* draw anything - `render_suggestions_popup`
    /// (called right after, once JS has resized the canvas to this
    /// method's reported size) consumes the layout this call caches on
    /// `self.suggestion_popup_cache` rather than recomputing it, so the
    /// two calls can never disagree about where anything is.
    #[wasm_bindgen]
    pub fn measure_suggestions_popup(&mut self) -> Result<JsValue, JsValue> {
        let suggestions = self.state.suggestions.suggestions.clone();
        if suggestions.is_empty() {
            self.suggestion_popup_cache = None;
            let obj = js_sys::Object::new();
            js_sys::Reflect::set(&obj, &"width".into(), &0.into())?;
            js_sys::Reflect::set(&obj, &"height".into(), &0.into())?;
            js_sys::Reflect::set(&obj, &"itemBounds".into(), &js_sys::Array::new())?;
            return Ok(obj.into());
        }

        let is_vertical = self.state.settings.editor.orientation == "vertical";
        let word_font_size = self.state.settings.fonts.font_size;
        let word_line_height = self.state.settings.fonts.line_height;
        let number_font_size = suggestion_number_font_size(word_font_size);
        let number_line_height = number_font_size * SUGGESTION_NUMBER_LINE_HEIGHT_RATIO;
        let word_orientation = if is_vertical {
            TextOrientation::VerticalLtr
        } else {
            TextOrientation::Horizontal
        };
        let font_family = self.state.settings.fonts.font_family.clone();

        let font_system = &mut self.state.font_system;
        let cache = &mut self.state.cache;

        let mut metrics_list: Vec<ItemMetrics> = Vec::with_capacity(suggestions.len());
        let mut measured: Vec<(MeasuredText, MeasuredText)> = Vec::with_capacity(suggestions.len());

        for (i, word) in suggestions.iter().enumerate() {
            let number_measured = if is_vertical {
                measure_suggestion_text(
                    font_system,
                    cache,
                    &font_family,
                    &(i + 1).to_string(),
                    number_font_size,
                    number_line_height,
                    TextOrientation::Horizontal,
                )
            } else {
                MeasuredText::default()
            };
            let word_measured = measure_suggestion_text(
                font_system,
                cache,
                &font_family,
                word,
                word_font_size,
                word_line_height,
                word_orientation,
            );

            metrics_list.push(ItemMetrics {
                number_width: number_measured.width,
                number_height: number_measured.height,
                word_width: word_measured.width,
                word_height: word_measured.height,
            });
            measured.push((number_measured, word_measured));
        }

        let config = LayoutConfig {
            cell_padding_x: SUGGESTION_CELL_PADDING_X,
            cell_padding_y: SUGGESTION_CELL_PADDING_Y,
            number_word_gap: SUGGESTION_NUMBER_WORD_GAP,
            max_row_width: SUGGESTION_MAX_ROW_WIDTH,
        };
        let layout: PopupLayout = if is_vertical {
            suggestion_popup_layout::layout_vertical_columns(&metrics_list, &config)
        } else {
            suggestion_popup_layout::layout_horizontal_rows(&metrics_list, &config)
        };

        let mut cached_items = Vec::with_capacity(layout.items.len());
        let bounds_arr = js_sys::Array::new();
        for (item_layout, (number_measured, word_measured)) in layout.items.iter().zip(measured.iter()) {
            // The buffer's own natural glyph origin (`min_x`/`min_y`,
            // usually near but not exactly `0,0` - e.g. left side-bearing)
            // needs to be cancelled out so the glyph's drawn top-left
            // lands exactly at the layout's target rect, not offset by
            // whatever the shaper's own internal origin happened to be.
            let number_offset = (
                item_layout.number.x - number_measured.min_x as f32,
                item_layout.number.y - number_measured.min_y as f32,
            );
            let word_offset = (
                item_layout.word.x - word_measured.min_x as f32,
                item_layout.word.y - word_measured.min_y as f32,
            );
            cached_items.push(CachedSuggestionItem {
                bounds: item_layout.bounds,
                number_offset,
                word_offset,
            });

            let bounds_obj = js_sys::Object::new();
            js_sys::Reflect::set(&bounds_obj, &"x".into(), &(item_layout.bounds.x as f64).into())?;
            js_sys::Reflect::set(&bounds_obj, &"y".into(), &(item_layout.bounds.y as f64).into())?;
            js_sys::Reflect::set(&bounds_obj, &"w".into(), &(item_layout.bounds.w as f64).into())?;
            js_sys::Reflect::set(&bounds_obj, &"h".into(), &(item_layout.bounds.h as f64).into())?;
            bounds_arr.push(&bounds_obj);
        }

        self.suggestion_popup_cache = Some(SuggestionPopupCache {
            items: cached_items,
        });

        let obj = js_sys::Object::new();
        js_sys::Reflect::set(&obj, &"width".into(), &(layout.width as f64).into())?;
        js_sys::Reflect::set(&obj, &"height".into(), &(layout.height as f64).into())?;
        js_sys::Reflect::set(&obj, &"itemBounds".into(), &bounds_arr)?;
        Ok(obj.into())
    }

    /// WS-09: draws the popup's current suggestion list into a
    /// JS-supplied `<canvas>` (already resized by JS to whatever
    /// `measure_suggestions_popup` most recently reported), reusing
    /// that call's cached layout rather than recomputing it. `-1` for
    /// `selected_index`/`hover_index` means "none" (wasm-bindgen has no
    /// convenient `Option<usize>` across the JS boundary).
    ///
    /// Bails out (does nothing, not an error) if there's no cached
    /// layout, or if the cached layout's item count no longer matches
    /// the current suggestion list - the latter should never actually
    /// happen since JS always calls `measure_suggestions_popup`
    /// immediately before this, but a stale/mismatched draw would be a
    /// worse failure mode than silently skipping a frame.
    #[wasm_bindgen]
    pub fn render_suggestions_popup(
        &mut self,
        canvas_id: &str,
        selected_index: i32,
        hover_index: i32,
    ) -> Result<(), JsValue> {
        // Deliberately `.clone()`, not `.take()` - selection/hover
        // changes call this again *without* a fresh
        // `measure_suggestions_popup()` call in between (no need to
        // re-measure just because the highlighted item changed), so
        // the cache must survive a successful render, not just a
        // zero-size bail-out.
        let Some(cache) = self.suggestion_popup_cache.clone() else {
            return Ok(());
        };
        let suggestions = self.state.suggestions.suggestions.clone();
        if suggestions.len() != cache.items.len() {
            return Ok(());
        }

        let document = web_sys::window()
            .ok_or("no window")?
            .document()
            .ok_or("no document")?;
        let canvas = document
            .get_element_by_id(canvas_id)
            .ok_or("suggestions canvas not found")?
            .dyn_into::<HtmlCanvasElement>()?;
        let context = canvas
            .get_context("2d")?
            .ok_or("no 2d context on suggestions canvas")?
            .dyn_into::<CanvasRenderingContext2d>()?;

        let width = canvas.width();
        let height = canvas.height();
        if width == 0 || height == 0 {
            return Ok(());
        }

        let is_vertical = self.state.settings.editor.orientation == "vertical";
        let word_font_size = self.state.settings.fonts.font_size;
        let word_line_height = self.state.settings.fonts.line_height;
        let number_font_size = suggestion_number_font_size(word_font_size);
        let number_line_height = number_font_size * SUGGESTION_NUMBER_LINE_HEIGHT_RATIO;
        let word_orientation = if is_vertical {
            TextOrientation::VerticalLtr
        } else {
            TextOrientation::Horizontal
        };
        let font_family = self.state.settings.fonts.font_family.clone();

        // Fixed UI-chrome colors matching this popup's previous
        // Tailwind classes (`bg-editor-header`, `bg-editor-accent`,
        // `hover:bg-editor-border`, `text-editor-text`,
        // `text-editor-text-dim`, `tailwind.config.js`) - this app's
        // fixed dark UI chrome (toolbar, modals, popups) is
        // independent of the *document's* own appearance theme
        // (`settings.appearance`, which only affects canvas content).
        const POPUP_BG: Color = Color::rgb(37, 37, 38);
        const SELECTED_BG: Color = Color::rgb(14, 99, 156);
        const HOVER_BG: Color = Color::rgb(62, 62, 66);
        const TEXT_COLOR: Color = Color::rgb(204, 204, 204);
        const TEXT_COLOR_DIM: Color = Color::rgb(133, 133, 133);
        const SELECTED_TEXT_COLOR: Color = Color::rgb(255, 255, 255);

        let mut pixels = vec![0u8; width as usize * height as usize * 4];
        for pixel in pixels.chunks_exact_mut(4) {
            pixel[0] = POPUP_BG.r();
            pixel[1] = POPUP_BG.g();
            pixel[2] = POPUP_BG.b();
            pixel[3] = 255;
        }

        // Highlight rect(s) first, under the glyphs - selection wins
        // visually over hover if they land on the same item, matching
        // the old CSS (the selected item's own background always beat
        // `:hover`).
        for (i, item) in cache.items.iter().enumerate() {
            let highlight = if i as i32 == selected_index {
                Some(SELECTED_BG)
            } else if i as i32 == hover_index {
                Some(HOVER_BG)
            } else {
                None
            };
            if let Some(color) = highlight {
                blend_rect(
                    &mut pixels,
                    width,
                    height,
                    item.bounds.x.round() as i32,
                    item.bounds.y.round() as i32,
                    item.bounds.w.round() as u32,
                    item.bounds.h.round() as u32,
                    color,
                );
            }
        }

        let font_system = &mut self.state.font_system;
        let cache_swash = &mut self.state.cache;

        for (i, word) in suggestions.iter().enumerate() {
            let item = &cache.items[i];
            let is_selected = i as i32 == selected_index;
            let (number_color, word_color) = if is_selected {
                (SELECTED_TEXT_COLOR, SELECTED_TEXT_COLOR)
            } else {
                (TEXT_COLOR_DIM, TEXT_COLOR)
            };

            if is_vertical {
                draw_suggestion_text(
                    font_system,
                    cache_swash,
                    &font_family,
                    &(i + 1).to_string(),
                    number_font_size,
                    number_line_height,
                    TextOrientation::Horizontal,
                    item.number_offset,
                    number_color,
                    &mut pixels,
                    width,
                    height,
                );
            }
            draw_suggestion_text(
                font_system,
                cache_swash,
                &font_family,
                word,
                word_font_size,
                word_line_height,
                word_orientation,
                item.word_offset,
                word_color,
                &mut pixels,
                width,
                height,
            );
        }

        let image_data = ImageData::new_with_u8_clamped_array_and_sh(
            wasm_bindgen::Clamped(&pixels),
            width,
            height,
        )?;
        context.put_image_data(&image_data, 0.0, 0.0)?;

        Ok(())
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
        web_sys::console::log_1(
            &format!("Transliteration dictionary loaded: {} entries", dict.len()).into(),
        );
        self.dictionary = Some(dict);
        Ok(())
    }

    /// Synchronous alternative to `translit_load_dictionary` above, for
    /// the word suggestion popup (Phase P9). Takes already-fetched TSV
    /// text and parses it in one atomic call - no `.await` inside Rust,
    /// so no cross-await borrow of `self` for anything else running on
    /// the same JS event loop to collide with.
    ///
    /// This distinction matters in practice, not just in theory: an
    /// async Rust method holds its `&mut self` borrow for the entire
    /// span between `.await` points, for as long as wasm-bindgen keeps
    /// that generated JS `Promise` unresolved. `translit_load_dictionary`
    /// gets away with that because the Transliteration modal traps
    /// keyboard focus on its own input while loading - the canvas's own
    /// `keydown`/`keyup`/mouse listeners and the `render()` loop can't
    /// fire during that window. The word suggestion popup's dictionary
    /// load is triggered *by* canvas typing, the one context where that
    /// assumption doesn't hold - a `keyup` for the very keystroke that
    /// triggered the load can (and did, during manual testing) fire
    /// while the load's `await` is still pending, tripping
    /// wasm-bindgen's "recursive use of an object" panic. Fetching the
    /// text in JS (a plain `fetch()`, no WASM object involved) and
    /// handing the already-resolved string to this synchronous method
    /// avoids the hazard entirely - see `WordSuggestionsPopup.ensureDictionaryLoaded`
    /// in `index.html`.
    #[wasm_bindgen]
    pub fn load_dictionary_text(&mut self, text: &str) {
        if self.dictionary.is_some() {
            return;
        }
        let dict = Dictionary::from_tsv(text);
        web_sys::console::log_1(
            &format!("Word suggestion dictionary loaded: {} entries", dict.len()).into(),
        );
        self.dictionary = Some(dict);
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

// ================= WS-09: word-suggestion popup canvas rendering =================
//
// Free functions (not `WasmEditor` methods) so they can be called with
// disjoint `&mut self.state.font_system` / `&mut self.state.cache`
// borrows from `measure_suggestions_popup`/`render_suggestions_popup`
// without fighting the borrow checker over a second `&mut self`.

/// A short piece of text's measured pixel extent, from one dry
/// `Buffer::draw` pass (draws nothing to any canvas - the callback just
/// accumulates bounds and is thrown away). `min_x`/`min_y` are the
/// shaper's own natural glyph origin (rarely exactly `0,0` - e.g. left
/// side-bearing) and are needed later to compute the offset that lands
/// the glyph's drawn top-left exactly on a target layout rect.
#[derive(Debug, Clone, Copy, Default)]
struct MeasuredText {
    width: f32,
    height: f32,
    min_x: i32,
    min_y: i32,
}

/// One suggestion's cached draw geometry from the last
/// `measure_suggestions_popup` call - see that method's doc comment for
/// why this is cached rather than recomputed by `render_suggestions_popup`.
#[derive(Debug, Clone, Copy)]
struct CachedSuggestionItem {
    bounds: suggestion_popup_layout::Rect,
    number_offset: (f32, f32),
    word_offset: (f32, f32),
}

#[derive(Debug, Clone)]
struct SuggestionPopupCache {
    items: Vec<CachedSuggestionItem>,
}

/// The number badge's own font size, proportional to but capped well
/// below the word's real size - a `43px`-tall digit would dominate the
/// tiny label it's attached to. Matches the same clamp the DOM-based
/// popup used before this rewrite.
fn suggestion_number_font_size(word_font_size: f32) -> f32 {
    (word_font_size * 0.35).clamp(14.0, 20.0)
}

/// Runs one dry `Buffer::draw` pass over `text` to measure its pixel
/// extent, without touching any canvas. Returns a zero-sized
/// `MeasuredText` for an empty string (used for the number badge in
/// horizontal mode, where there is no number).
fn measure_suggestion_text(
    font_system: &mut FontSystem,
    cache: &mut SwashCache,
    font_family: &str,
    text: &str,
    font_size: f32,
    line_height: f32,
    orientation: TextOrientation,
) -> MeasuredText {
    if text.is_empty() {
        return MeasuredText::default();
    }

    let metrics = Metrics::new(font_size, line_height);
    let mut buffer = Buffer::new(font_system, metrics);
    // No wrap constraint - these are always short single words/digits,
    // never a phrase (see `Dictionary::suggest`'s doc comment), so
    // there's nothing to wrap and no reason to guess a width ahead of
    // knowing the real one.
    buffer.set_size(font_system, None, None);
    buffer.set_orientation(font_system, orientation);
    let attrs = Attrs::new().family(Family::Name(font_family));
    buffer.set_text(font_system, text, &attrs, Shaping::Advanced, None);
    buffer.shape_until_scroll(font_system, false);

    let mut min_x = i32::MAX;
    let mut max_x = i32::MIN;
    let mut min_y = i32::MAX;
    let mut max_y = i32::MIN;
    buffer.draw(
        font_system,
        cache,
        Color::rgb(0, 0, 0),
        |x: i32, y: i32, w: u32, h: u32, _color: Color| {
            min_x = min_x.min(x);
            max_x = max_x.max(x + w as i32);
            min_y = min_y.min(y);
            max_y = max_y.max(y + h as i32);
        },
    );

    if max_x < min_x || max_y < min_y {
        // Nothing drawn (e.g. a font with no glyph for this text).
        return MeasuredText::default();
    }

    MeasuredText {
        width: (max_x - min_x) as f32,
        height: (max_y - min_y) as f32,
        min_x,
        min_y,
    }
}

/// Shapes and draws `text` into `pixels`, shifted by `offset` (computed
/// by `measure_suggestions_popup` to cancel out the shaper's own
/// natural glyph origin - see `MeasuredText`'s doc comment) so it lands
/// exactly on its cached target rect.
fn draw_suggestion_text(
    font_system: &mut FontSystem,
    cache: &mut SwashCache,
    font_family: &str,
    text: &str,
    font_size: f32,
    line_height: f32,
    orientation: TextOrientation,
    offset: (f32, f32),
    color: Color,
    pixels: &mut [u8],
    width: u32,
    height: u32,
) {
    if text.is_empty() {
        return;
    }

    let metrics = Metrics::new(font_size, line_height);
    let mut buffer = Buffer::new(font_system, metrics);
    buffer.set_size(font_system, None, None);
    buffer.set_orientation(font_system, orientation);
    let attrs = Attrs::new().family(Family::Name(font_family));
    buffer.set_text(font_system, text, &attrs, Shaping::Advanced, None);
    buffer.shape_until_scroll(font_system, false);

    let (offset_x, offset_y) = offset;
    let offset_x = offset_x.round() as i32;
    let offset_y = offset_y.round() as i32;
    buffer.draw(
        font_system,
        cache,
        color,
        |x: i32, y: i32, w: u32, h: u32, glyph_color: Color| {
            blend_rect(pixels, width, height, x + offset_x, y + offset_y, w, h, glyph_color);
        },
    );
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
