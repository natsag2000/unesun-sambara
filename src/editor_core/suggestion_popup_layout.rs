//! Pure arithmetic for arranging the word-suggestion popup's items into
//! either a horizontal row of columns (vertical editor mode - see
//! `WORD_SUGGESTIONS_PLAN.md`'s WS-09) or a vertical stack of rows
//! (horizontal editor mode), given each item's already-*measured* pixel
//! dimensions.
//!
//! Deliberately has no dependency on `cosmic-text`, WASM, or any canvas
//! - the actual text shaping/measurement (which *does* need a real
//! `FontSystem`) lives in `src/lib.rs`'s `measure_suggestions_popup`/
//! `render_suggestions_popup`, which call into this module only after
//! they already have real glyph-measured widths/heights to arrange.
//! Same separation of concerns as `word_boundary.rs`/`format_control.rs`
//! - keep the parts that don't need a browser/fonts unit-testable
//! without either.
//!
//! All units here are in the same "device pixel" space
//! `get_word_suggestions_json`'s `anchorX`/`anchorY`/`lineAdvance`
//! already use (i.e. whatever `settings.fonts.font_size` is directly
//! in, with no DPR division - that only happens in JS at the very end).

/// One suggestion's already-measured pieces:
/// - `number_width`/`number_height`: the "1".."8" badge (vertical mode
///   only - pass `0.0`/`0.0` for horizontal mode, where there is no
///   number badge, matching the existing vertical-mode-only numbering
///   feature).
/// - `word_width`/`word_height`: the suggestion word itself.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct ItemMetrics {
    pub number_width: f32,
    pub number_height: f32,
    pub word_width: f32,
    pub word_height: f32,
}

/// An axis-aligned pixel rectangle, top-left origin (matching canvas
/// pixel-space convention used throughout this codebase).
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

impl Rect {
    // Not called from production code today - hit-testing happens in
    // JS against the `itemBounds` this module's callers hand back
    // across the WASM boundary (see `WasmEditor::measure_suggestions_popup`),
    // not from Rust. Kept public + tested as part of this module's
    // small API surface in case a future Rust-side hit-test method
    // wants it (e.g. to avoid a JS round trip on every `mousemove`).
    #[allow(dead_code)]
    pub fn contains(&self, x: f32, y: f32) -> bool {
        x >= self.x && x < self.x + self.w && y >= self.y && y < self.y + self.h
    }
}

/// Where to draw one suggestion's pieces, plus its full hit-testable
/// cell (`bounds`) - deliberately stretched to the popup's full
/// cross-axis extent (full height in vertical mode, full width in
/// horizontal mode) rather than exactly hugging the content, so click
/// targets are comfortably sized (like a table cell), not just the
/// glyphs' own tight bounding box.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct ItemLayout {
    pub bounds: Rect,
    pub number: Rect,
    pub word: Rect,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct PopupLayout {
    pub width: f32,
    pub height: f32,
    pub items: Vec<ItemLayout>,
}

/// Padding/gap constants, factored out as parameters (rather than
/// hardcoded) so callers can tune them without touching the layout
/// arithmetic itself.
#[derive(Debug, Clone, Copy)]
pub struct LayoutConfig {
    pub cell_padding_x: f32,
    pub cell_padding_y: f32,
    /// Vertical gap between a column's number badge and its word
    /// (vertical mode only).
    pub number_word_gap: f32,
    /// Caps a single row's width in horizontal mode (mirrors the
    /// existing `max-width: 20rem`-equivalent truncation) - words
    /// wider than this are clipped when drawn, not ellipsized (a
    /// deliberate v1 simplification, see the WS-09 impl log).
    pub max_row_width: f32,
}

/// Arranges `items` as a horizontal row of columns, each with a number
/// badge stacked above its word - the vertical-editor-mode layout.
/// Columns are packed left-to-right in `items`' order (matching
/// Mongolian's own left-to-right column progression); each column's
/// own width fits its content exactly, but every column's `bounds.h`
/// is stretched to the tallest column's height.
pub fn layout_vertical_columns(items: &[ItemMetrics], config: &LayoutConfig) -> PopupLayout {
    if items.is_empty() {
        return PopupLayout::default();
    }

    let mut x_cursor = 0.0f32;
    let mut max_height = 0.0f32;
    let mut layouts: Vec<ItemLayout> = Vec::with_capacity(items.len());

    for item in items {
        let content_width = item.number_width.max(item.word_width);
        let cell_width = content_width + 2.0 * config.cell_padding_x;
        let cell_height = item.number_height
            + config.number_word_gap
            + item.word_height
            + 2.0 * config.cell_padding_y;

        let number = Rect {
            x: x_cursor + (cell_width - item.number_width) / 2.0,
            y: config.cell_padding_y,
            w: item.number_width,
            h: item.number_height,
        };
        let word = Rect {
            x: x_cursor + (cell_width - item.word_width) / 2.0,
            y: config.cell_padding_y + item.number_height + config.number_word_gap,
            w: item.word_width,
            h: item.word_height,
        };
        let bounds = Rect {
            x: x_cursor,
            y: 0.0,
            w: cell_width,
            h: cell_height,
        };

        layouts.push(ItemLayout { bounds, number, word });
        x_cursor += cell_width;
        max_height = max_height.max(cell_height);
    }

    // Stretch every column's clickable cell to the tallest column's
    // height, so a short word's column is just as easy to click
    // anywhere in its vertical extent as a tall one's.
    for layout in &mut layouts {
        layout.bounds.h = max_height;
    }

    PopupLayout {
        width: x_cursor,
        height: max_height,
        items: layouts,
    }
}

/// Arranges `items` as a vertical stack of rows, each just the word
/// (no number badge) - the horizontal-editor-mode layout, matching the
/// original single-column stacked-list design.
pub fn layout_horizontal_rows(items: &[ItemMetrics], config: &LayoutConfig) -> PopupLayout {
    if items.is_empty() {
        return PopupLayout::default();
    }

    let mut y_cursor = 0.0f32;
    let mut max_width = 0.0f32;
    let mut layouts: Vec<ItemLayout> = Vec::with_capacity(items.len());

    let available_width = (config.max_row_width - 2.0 * config.cell_padding_x).max(0.0);

    for item in items {
        let clipped_word_width = item.word_width.min(available_width);
        let row_width = clipped_word_width + 2.0 * config.cell_padding_x;
        let row_height = item.word_height + 2.0 * config.cell_padding_y;

        let word = Rect {
            x: config.cell_padding_x,
            y: y_cursor + config.cell_padding_y,
            w: clipped_word_width,
            h: item.word_height,
        };
        let bounds = Rect {
            x: 0.0,
            y: y_cursor,
            w: row_width,
            h: row_height,
        };

        layouts.push(ItemLayout {
            bounds,
            number: Rect::default(),
            word,
        });
        y_cursor += row_height;
        max_width = max_width.max(row_width);
    }

    // Stretch every row's clickable cell to the widest row's width, so
    // a short word's row is just as easy to click across its full
    // horizontal extent as a long one's.
    for layout in &mut layouts {
        layout.bounds.w = max_width;
    }

    PopupLayout {
        width: max_width,
        height: y_cursor,
        items: layouts,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn config() -> LayoutConfig {
        LayoutConfig {
            cell_padding_x: 8.0,
            cell_padding_y: 8.0,
            number_word_gap: 4.0,
            max_row_width: 200.0,
        }
    }

    #[test]
    fn vertical_layout_of_empty_list_is_zero_sized() {
        let layout = layout_vertical_columns(&[], &config());
        assert_eq!(layout.width, 0.0);
        assert_eq!(layout.height, 0.0);
        assert!(layout.items.is_empty());
    }

    #[test]
    fn horizontal_layout_of_empty_list_is_zero_sized() {
        let layout = layout_horizontal_rows(&[], &config());
        assert_eq!(layout.width, 0.0);
        assert_eq!(layout.height, 0.0);
        assert!(layout.items.is_empty());
    }

    #[test]
    fn single_vertical_column_is_centered_and_padded() {
        let items = [ItemMetrics {
            number_width: 6.0,
            number_height: 10.0,
            word_width: 20.0,
            word_height: 40.0,
        }];
        let cfg = config();
        let layout = layout_vertical_columns(&items, &cfg);

        // width = max(6, 20) + 2*8 = 36; height = 10 + 4 + 40 + 2*8 = 70
        assert_eq!(layout.width, 36.0);
        assert_eq!(layout.height, 70.0);
        assert_eq!(layout.items.len(), 1);

        let item = layout.items[0];
        assert_eq!(item.bounds, Rect { x: 0.0, y: 0.0, w: 36.0, h: 70.0 });
        // number centered within the 20px-wide content column (widest
        // of number/word), i.e. x = 8 + (20-6)/2 = 15
        assert_eq!(item.number.x, 15.0);
        assert_eq!(item.number.y, 8.0);
        // word starts right after the padding (it's the widest, so no
        // extra centering offset)
        assert_eq!(item.word.x, 8.0);
        assert_eq!(item.word.y, 8.0 + 10.0 + 4.0);
    }

    #[test]
    fn multiple_vertical_columns_pack_left_to_right_and_share_max_height() {
        let items = [
            ItemMetrics { number_width: 6.0, number_height: 10.0, word_width: 10.0, word_height: 20.0 },
            ItemMetrics { number_width: 6.0, number_height: 10.0, word_width: 10.0, word_height: 60.0 },
        ];
        let cfg = config();
        let layout = layout_vertical_columns(&items, &cfg);

        // Both columns: width = max(6,10) + 16 = 26 each -> total 52.
        assert_eq!(layout.width, 52.0);
        // Column 0 content height = 10+4+20+16 = 50; column 1 = 10+4+60+16 = 90.
        // Popup height is the taller one.
        assert_eq!(layout.height, 90.0);

        assert_eq!(layout.items[0].bounds.x, 0.0);
        assert_eq!(layout.items[1].bounds.x, 26.0);
        // Both columns' clickable cells stretch to the shared max height,
        // even though column 0's own content is shorter.
        assert_eq!(layout.items[0].bounds.h, 90.0);
        assert_eq!(layout.items[1].bounds.h, 90.0);
    }

    #[test]
    fn horizontal_rows_stack_top_to_bottom_and_share_max_width() {
        let items = [
            ItemMetrics { number_width: 0.0, number_height: 0.0, word_width: 20.0, word_height: 15.0 },
            ItemMetrics { number_width: 0.0, number_height: 0.0, word_width: 60.0, word_height: 15.0 },
        ];
        let cfg = config();
        let layout = layout_horizontal_rows(&items, &cfg);

        // Row 0 width = 20+16=36; row 1 width = 60+16=76 -> popup width is the wider one.
        assert_eq!(layout.width, 76.0);
        // Row heights: 15+16=31 each -> total 62.
        assert_eq!(layout.height, 62.0);

        assert_eq!(layout.items[0].bounds.y, 0.0);
        assert_eq!(layout.items[1].bounds.y, 31.0);
        // Both rows' clickable cells stretch to the shared max width.
        assert_eq!(layout.items[0].bounds.w, 76.0);
        assert_eq!(layout.items[1].bounds.w, 76.0);
    }

    #[test]
    fn horizontal_row_clips_a_word_wider_than_max_row_width() {
        let items = [ItemMetrics {
            number_width: 0.0,
            number_height: 0.0,
            word_width: 1000.0,
            word_height: 15.0,
        }];
        let cfg = config(); // max_row_width = 200
        let layout = layout_horizontal_rows(&items, &cfg);

        // available width = 200 - 16 = 184, clipped from 1000.
        assert_eq!(layout.items[0].word.w, 184.0);
        assert_eq!(layout.width, 200.0);
    }

    #[test]
    fn rect_contains_checks_half_open_bounds() {
        let r = Rect { x: 10.0, y: 10.0, w: 5.0, h: 5.0 };
        assert!(r.contains(10.0, 10.0));
        assert!(r.contains(14.9, 14.9));
        assert!(!r.contains(15.0, 15.0)); // half-open: exclusive on the far edge
        assert!(!r.contains(9.9, 10.0));
    }
}
