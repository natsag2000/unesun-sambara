use cosmic_text::{Editor, FontSystem, SwashCache, Buffer, Metrics, TextOrientation, Attrs, Shaping, fontdb, Edit};
use crate::config::settings::EditorSettings;
use crate::editor_core::events::EventBus;
use crate::editor_core::history::{EditKind, HistoryManager};
use crate::editor_core::plugin::{CoreCommandsPlugin, PluginRegistry};
use crate::plugins::find_replace::FindReplaceState;
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::JsFuture;
use web_sys::Response;

pub struct EditorState {
    pub font_system: FontSystem,
    pub cache: SwashCache,
    pub editor: Editor<'static>,
    pub settings: EditorSettings,
    pub cursor_visible: bool,
    pub last_render_time: f64,
    /// Event bus. `WasmEditor` dispatches into this after actions that
    /// mutate observable state; plugins subscribe by being registered on
    /// `plugins` below (the registry handles fan-out to plugins itself).
    pub events: EventBus,
    /// Compiled-in plugin registry. New plugins register in
    /// `EditorState::new` so they are available before the first render.
    pub plugins: PluginRegistry,
    /// Undo / redo history (P2-01). See `crate::editor_core::history`.
    pub history: HistoryManager,
    /// Find & replace state (P2-03). Lives here (rather than inside the
    /// `FindReplacePlugin` struct) because command handlers are bare `fn`
    /// pointers with no access to plugin instance state - see the note
    /// on "plugin data slots" in `prompt/PLUGIN_API.md`.
    pub find_replace: FindReplaceState,
}

impl EditorState {
    pub async fn new(settings: EditorSettings) -> Result<Self, JsValue> {
        // Create empty font database and font system (no system fonts in WASM)
        let db = fontdb::Database::new();
        let mut font_system = FontSystem::new_with_locale_and_db("en-US".to_string(), db);

        // Load all fonts explicitly (following rich-text example pattern)
        // Load base fonts first
        let noto_font = Self::load_font_file("fonts/NotoSans-Regular.ttf").await?;
        font_system.db_mut().load_font_data(noto_font);

        let mongolian_font = Self::load_font_file("fonts/NotoSansMongolian-Regular.ttf").await?;
        font_system.db_mut().load_font_data(mongolian_font);

        let baiti_font = Self::load_font_file("fonts/monbaiti.ttf").await?;
        font_system.db_mut().load_font_data(baiti_font);

        // Load emoji font (required for emoji support in WASM)
        // Note: Using NotoEmoji-Regular (monochrome) instead of NotoColorEmoji
        // because color emoji fonts have positioning issues in vertical mode
        // Note: Emoji spacing in vertical mode may be larger than ideal due to
        // lack of vertical metrics (vhea/vmtx tables) in most emoji fonts
        let emoji_font = Self::load_font_file("fonts/NotoEmoji-Regular.ttf?v=2").await?;
        font_system.db_mut().load_font_data(emoji_font);

        let cache = SwashCache::new();

        // Create buffer with settings
        let metrics = Metrics::new(settings.fonts.font_size, settings.fonts.line_height);
        let mut buffer = Buffer::new(&mut font_system, metrics);

        let orientation = if settings.editor.orientation == "vertical" {
            TextOrientation::VerticalLtr
        } else {
            TextOrientation::Horizontal
        };

        buffer.set_orientation(&mut font_system, orientation);

        let wrap = if settings.editor.word_wrap {
            cosmic_text::Wrap::WordOrGlyph
        } else {
            cosmic_text::Wrap::None
        };
        buffer.set_wrap(&mut font_system, wrap);

        // Load random greeting text
        let greeting_text = Self::load_random_greeting().await?;

        // Use the font family from settings so regular text uses the correct font,
        // but emoji will fallback to the emoji font
        let attrs = Attrs::new().family(cosmic_text::Family::Name(&settings.fonts.font_family));
        buffer.set_text(
            &mut font_system,
            &greeting_text,
            &attrs,
            Shaping::Advanced,
            None,
        );

        let editor = Editor::new(buffer);

        // Register built-in plugins. Any future built-ins go here; user
        // plugins can be pushed onto `state.plugins` later.
        let mut plugins = PluginRegistry::new();
        plugins.register(Box::new(CoreCommandsPlugin));
        plugins.register(Box::new(crate::plugins::find_replace::FindReplacePlugin));

        Ok(Self {
            font_system,
            cache,
            editor,
            settings,
            cursor_visible: true,
            last_render_time: 0.0,
            events: EventBus::new(),
            plugins,
            history: HistoryManager::new(),
            find_replace: FindReplaceState::new(),
        })
    }

    // ======================================================================
    // Undo / redo (P2-01)
    // ======================================================================
    //
    // cosmic-text's `Editor` accumulates a `Change` (a list of
    // `ChangeItem`s) while a change is "in progress" (`Edit::start_change`
    // .. `Edit::finish_change`). `begin_history_group` / `flush_history_group`
    // decide when that window opens and closes; `HistoryManager` decides
    // whether a given keystroke continues the current window (coalesced
    // typing) or starts a new one. See `crate::editor_core::history` for
    // the full rationale.

    /// Call before performing a mutating action of `kind` at time `now`
    /// (typically `event.time_stamp()`). Flushes the previous group first
    /// if this keystroke starts a new one, then ensures a change is being
    /// collected so the upcoming `Action::*` / `insert_at` / `delete_range`
    /// calls are recorded.
    pub fn begin_history_group(&mut self, kind: EditKind, now: f64) {
        if self.history.is_new_group(kind, now) {
            self.flush_history_group();
            self.history.begin_group(kind, now);
        } else {
            self.history.touch(now);
        }
        self.editor.start_change();
    }

    /// End whatever change is currently being collected (if any) and
    /// commit it to the undo stack. Safe to call even if no change is in
    /// progress. Call this before any action that should *not* be
    /// coalesced with subsequent typing (cursor motion, mouse clicks,
    /// undo/redo itself, loading a new document, ...).
    pub fn flush_history_group(&mut self) {
        if let Some(change) = self.editor.finish_change() {
            self.history.commit(change);
        }
        self.history.end_group();
    }

    /// Undo the most recent change. Returns `true` if something was
    /// undone.
    pub fn undo(&mut self) -> bool {
        self.flush_history_group();
        let Some(change) = self.history.pop_undo() else {
            return false;
        };
        let mut reversed = change.clone();
        reversed.reverse();
        self.editor.apply_change(&reversed);
        self.history.push_redo(change);
        true
    }

    /// Redo the most recently undone change. Returns `true` if something
    /// was redone.
    pub fn redo(&mut self) -> bool {
        self.flush_history_group();
        let Some(change) = self.history.pop_redo() else {
            return false;
        };
        self.editor.apply_change(&change);
        self.history.push_undo(change);
        true
    }

    pub fn can_undo(&self) -> bool {
        self.history.can_undo()
    }

    pub fn can_redo(&self) -> bool {
        self.history.can_redo()
    }

    /// Discard all undo/redo history. Called when a brand new document
    /// replaces the buffer contents (`WasmEditor::set_text`) - undoing
    /// "past" a freshly loaded document has no sensible meaning.
    pub fn discard_history(&mut self) {
        // Drop any change collected against the *old* text; applying it
        // later would operate on stale line indices.
        let _ = self.editor.finish_change();
        self.history.clear();
    }

    // ======================================================================
    // Find & replace (P2-03)
    // ======================================================================

    /// Re-run the current find query against the buffer's live text.
    /// Call after any text mutation that might invalidate previously
    /// computed match offsets (typing, replace, undo/redo, document
    /// load).
    pub fn refresh_find_matches(&mut self) {
        let find_replace = &mut self.find_replace;
        self.editor.with_buffer(|buffer| find_replace.search(buffer));
    }

    pub fn update_settings(&mut self, settings: EditorSettings) {
        // Update metrics if font size or line height changed
        if self.settings.fonts.font_size != settings.fonts.font_size ||
           self.settings.fonts.line_height != settings.fonts.line_height {
            let metrics = Metrics::new(settings.fonts.font_size, settings.fonts.line_height);
            self.editor.with_buffer_mut(|buffer| {
                buffer.set_metrics(&mut self.font_system, metrics);
            });
        }

        // Update font family if changed
        if self.settings.fonts.font_family != settings.fonts.font_family {
            let font_system = &mut self.font_system;
            let font_family = settings.fonts.font_family.clone();

            // Get current text content and buffer size
            let mut text = String::new();
            let (buffer_width, buffer_height) = self.editor.with_buffer(|buffer| {
                for line in buffer.lines.iter() {
                    text.push_str(line.text());
                    text.push('\n');
                }
                // Get current buffer dimensions
                (buffer.size().0, buffer.size().1)
            });

            // Remove trailing newline if present
            if text.ends_with('\n') {
                text.pop();
            }

            // Set text again with new font family
            self.editor.with_buffer_mut(|buffer| {
                let new_attrs = Attrs::new().family(cosmic_text::Family::Name(&font_family));
                buffer.set_text(
                    font_system,
                    &text,
                    &new_attrs,
                    Shaping::Advanced,
                    None,
                );
                // Restore buffer size to maintain text wrapping
                buffer.set_size(font_system, buffer_width, buffer_height);
            });
        }

        // Update orientation if changed
        if self.settings.editor.orientation != settings.editor.orientation {
            let orientation = if settings.editor.orientation == "vertical" {
                TextOrientation::VerticalLtr
            } else {
                TextOrientation::Horizontal
            };
            self.editor.with_buffer_mut(|buffer| {
                // Reset shaping for all lines to force re-shaping with new orientation
                for line in buffer.lines.iter_mut() {
                    line.reset_shaping();
                }
                buffer.set_orientation(&mut self.font_system, orientation);
            });
        }

        // Update word wrap if changed (P2-04).
        if self.settings.editor.word_wrap != settings.editor.word_wrap {
            let wrap = if settings.editor.word_wrap {
                cosmic_text::Wrap::WordOrGlyph
            } else {
                cosmic_text::Wrap::None
            };
            let font_system = &mut self.font_system;
            self.editor.with_buffer_mut(move |buffer| {
                buffer.set_wrap(font_system, wrap);
            });
        }

        self.settings = settings;
    }

    async fn load_font_file(path: &str) -> Result<Vec<u8>, JsValue> {
        let window = web_sys::window().ok_or("No window")?;

        // Fetch the font file
        let resp_value = JsFuture::from(window.fetch_with_str(path)).await?;
        let resp: Response = resp_value.dyn_into()?;

        // Get the response as array buffer
        let array_buffer = JsFuture::from(resp.array_buffer()?).await?;
        let uint8_array = js_sys::Uint8Array::new(&array_buffer);

        // Convert to Vec<u8>
        let mut font_data = vec![0u8; uint8_array.length() as usize];
        uint8_array.copy_to(&mut font_data);

        Ok(font_data)
    }

    async fn load_text_file(path: &str) -> Result<String, JsValue> {
        let window = web_sys::window().ok_or("No window")?;

        // Fetch the text file
        let resp_value = JsFuture::from(window.fetch_with_str(path)).await?;
        let resp: Response = resp_value.dyn_into()?;

        // Get the response as text
        let text_promise = resp.text()?;
        let text_value = JsFuture::from(text_promise).await?;
        let text = text_value.as_string().ok_or("Failed to convert to string")?;

        Ok(text)
    }

    async fn load_random_greeting() -> Result<String, JsValue> {
        // Load manifest file to get list of available greeting files
        let manifest_json = match Self::load_text_file("texts/manifest.json").await {
            Ok(json) => json,
            Err(_) => {
                // If manifest doesn't exist, return default message
                return Ok("Welcome to WASM Text Editor!\n\nStart typing!".to_string());
            }
        };

        // Parse the JSON manifest
        let manifest: serde_json::Value = match serde_json::from_str(&manifest_json) {
            Ok(m) => m,
            Err(_) => {
                return Ok("Welcome to WASM Text Editor!\n\nStart typing!".to_string());
            }
        };

        // Get the greetings array
        let greetings = match manifest.get("greetings").and_then(|v| v.as_array()) {
            Some(arr) => arr,
            None => {
                return Ok("Welcome to WASM Text Editor!\n\nStart typing!".to_string());
            }
        };

        // If no greetings found, return default
        if greetings.is_empty() {
            return Ok("Welcome to WASM Text Editor!\n\nStart typing!".to_string());
        }

        // Randomly select one greeting file
        let random = js_sys::Math::random();
        let selected_index = (random * greetings.len() as f64).floor() as usize;

        // Get the filename
        let filename = match greetings[selected_index].as_str() {
            Some(name) => name,
            None => {
                return Ok("Welcome to WASM Text Editor!\n\nStart typing!".to_string());
            }
        };

        // Load the selected greeting file
        let path = format!("texts/{}", filename);
        Self::load_text_file(&path).await
    }
}
