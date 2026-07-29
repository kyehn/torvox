//! Cell instance builder — converts terminal grid state into GPU instance data.
use crate::render::CellInstance;

use crate::terminal::CursorStyle;
use crate::terminal::SelectionMode;

use std::collections::HashMap;

/// Bit position of reverse video (SGR 7) in CellData.flags.
/// Must match the bit layout used by pack_style_flags() and shader/cell.wgsl.
const REVERSE_BIT: u8 = 2;

/// Cursor state passed to build_instances_from_cell_data() for cursor rendering.
#[derive(Debug, Clone, Copy, Default)]
pub struct CellCursor {
    pub row: u32,
    pub col: u32,
    pub visible: bool,
    pub style: CursorStyle,
    pub color: Option<[f32; 4]>,
}

/// A selected range of characters to highlight with a background color.
///
/// Supports Char/Word/Semantic (box), Line (full rows), and Block modes.
#[derive(Debug, Clone, Copy, Default)]
pub struct SelectionRange {
    pub start_row: i32,
    pub start_col: i32,
    pub end_row: i32,
    pub end_col: i32,
    pub active: bool,
    pub mode: SelectionMode,
    pub origin: Option<(i32, i32)>,
    pub is_empty: bool,
}

/// Returns ordered (lo_row, lo_col, hi_row, hi_col) so consuming code does
/// not need to worry about direction.
impl SelectionRange {
    pub fn contains(&self, row: u32, col: u32, _cols: u32) -> bool {
        if !self.active {
            return false;
        }
        let row = row as i32;
        let col = col as i32;
        let (lo_row, lo_col, hi_row, hi_col) = self.ordered();
        match self.mode {
            SelectionMode::Line => row >= lo_row && row <= hi_row,
            SelectionMode::Block => {
                row >= lo_row && row <= hi_row && col >= lo_col && col <= hi_col
            }
            SelectionMode::Char | SelectionMode::Word | SelectionMode::Semantic => {
                if row < lo_row || row > hi_row {
                    return false;
                }
                if lo_row == hi_row {
                    col >= lo_col && col <= hi_col
                } else if row == lo_row {
                    col >= lo_col
                } else if row == hi_row {
                    col <= hi_col
                } else {
                    true
                }
            }
        }
    }

    fn ordered(&self) -> (i32, i32, i32, i32) {
        if self.start_row < self.end_row
            || (self.start_row == self.end_row && self.start_col <= self.end_col)
        {
            (self.start_row, self.start_col, self.end_row, self.end_col)
        } else {
            (self.end_row, self.end_col, self.start_row, self.start_col)
        }
    }
}

#[derive(Debug, Clone, Copy)]
/// A single row range for search result highlighting.
pub struct SearchHighlight {
    pub row: i32,
    pub start_col: i32,
    pub end_col_exclusive: i32,
    pub color: [u8; 4],
}

/// Check whether one cell falls inside a search highlight.
pub(crate) fn cell_highlight<'a>(
    row: u32,
    col: u32,
    by_row: &'a HashMap<i32, Vec<&'a SearchHighlight>>,
) -> Option<&'a [u8; 4]> {
    let h_list = by_row.get(&(row as i32))?;
    let highlight = h_list
        .iter()
        .find(|h| (col as i32) >= h.start_col && (col as i32) < h.end_col_exclusive)?;
    Some(&highlight.color)
}

/// Blend a highlight RGBA into a float color.
pub(crate) fn blend_highlight(base: [f32; 4], hl_rgba: [u8; 4]) -> [f32; 4] {
    let alpha = hl_rgba[3] as f32 / 255.0;
    if alpha <= 0.0 {
        return base;
    }
    let hr = hl_rgba[0] as f32 / 255.0;
    let hg = hl_rgba[1] as f32 / 255.0;
    let hb = hl_rgba[2] as f32 / 255.0;
    [
        base[0] * (1.0 - alpha) + hr * alpha,
        base[1] * (1.0 - alpha) + hg * alpha,
        base[2] * (1.0 - alpha) + hb * alpha,
        1.0,
    ]
}

#[inline]
/// Apply a search-highlight RGBA to a cell foreground/background.
pub(crate) fn apply_search_highlight(fg: &mut [f32; 4], bg: &mut [f32; 4], hl: [u8; 4]) {
    if hl[3] >= 128 {
        std::mem::swap(fg, bg);
    }
    *bg = blend_highlight(*bg, hl);
}

/// Convert pre-built `CellData` slices into GPU instance data.
///
/// Takes selection and search highlight parameters so the renderer can
/// apply visual feedback (selection background swap, search highlight
/// overlay) without requiring a full GridSnapshot.
///
/// Returns `None` if conversion fails (font atlas unavailable, etc.).
#[allow(clippy::too_many_arguments)]
pub fn build_instances_from_cell_data(
    cell_data: &[crate::terminal::ghostty_terminal::CellData],
    _rows: u32,
    cols: u32,
    cursor: CellCursor,
    font_pipeline: &mut crate::render::font::FontPipeline,
    atlas_width: f32,
    atlas_height: f32,
    selection: Option<SelectionRange>,
    selection_bg: Option<[f32; 4]>,
    search_highlights: &[SearchHighlight],
) -> Option<Vec<CellInstance>> {
    let (cell_w, cell_h) = font_pipeline.cell_metrics();
    let ascent_pixels = font_pipeline.ascent_pixels();
    let raster_scale = font_pipeline.get_raster_scale();
    let mut instances = Vec::with_capacity(cell_data.len());

    let selection = selection.filter(|s| !s.is_empty);
    let mut highlights_by_row: HashMap<i32, Vec<&SearchHighlight>> = HashMap::new();
    for h in search_highlights {
        highlights_by_row.entry(h.row).or_default().push(h);
    }

    for cd in cell_data {
        // Validate codepoint before conversion — invalid values should be
        // caught in debug builds so terminal-content bugs don't hide.
        debug_assert!(
            char::from_u32(cd.codepoint).is_some(),
            "cell_builder: invalid codepoint: {}",
            cd.codepoint
        );
        let ch = char::from_u32(cd.codepoint).unwrap_or(' ');
        let cell_span = cd.width.max(1) as f32;
        let quad_origin = [cd.col as f32 * cell_w, cd.row as f32 * cell_h];
        let mut fg_color = cd.fg_color;
        let mut bg_color = cd.bg_color;
        // SGR 7 reverse video: swap foreground and background colors
        // Check reverse attribute (bit 2 in new layout matching old path's
        // `cell.reverse` bit position used by pack_style_flags → shader).
        if (cd.flags >> REVERSE_BIT) & 1 == 1 {
            std::mem::swap(&mut fg_color, &mut bg_color);
        }

        // Selection highlight: swap fg/bg or apply selection_bg color
        if selection.unwrap_or_default().contains(cd.row, cd.col, cols) {
            if let Some(sbg) = selection_bg {
                bg_color = sbg;
            } else {
                std::mem::swap(&mut fg_color, &mut bg_color);
            }
        }
        // Search highlight overlay (applied on top of selection)
        if let Some(hl) = cell_highlight(cd.row, cd.col, &highlights_by_row) {
            apply_search_highlight(&mut fg_color, &mut bg_color, *hl);
        }
        let is_cursor = cursor.visible && cd.row == cursor.row && cd.col == cursor.col;
        let effective_fg = fg_color;
        let mut effective_bg = bg_color;
        let mut cursor_marker: Option<([f32; 2], [f32; 2], [f32; 4])> = None;
        // Default quad size (used for Block cursor and empty cells)
        let quad_size = [cell_w * cell_span, cell_h];

        if is_cursor {
            let cursor_color = cursor.color.unwrap_or([1.0, 1.0, 1.0, 1.0]);
            match cursor.style {
                CursorStyle::Block | CursorStyle::Default => {
                    // Block cursor: keep text readable by using the original
                    // foreground; only the background is replaced by cursor color
                    // (semi-transparent overlay).
                    effective_bg = [
                        cursor_color[0],
                        cursor_color[1],
                        cursor_color[2],
                        cursor_color[3] * 0.7,
                    ];
                }
                CursorStyle::Bar => {
                    // Bar cursor: emit a thin vertical bar as a background quad,
                    // then render glyph full-size with original colors on top.
                    cursor_marker = Some((
                        quad_origin,
                        [cell_w * 0.15, cell_h],
                        [
                            cursor_color[0],
                            cursor_color[1],
                            cursor_color[2],
                            cursor_color[3] * 0.9,
                        ],
                    ));
                }
                CursorStyle::Underline => {
                    // Underline cursor: emit a thin horizontal bar at bottom,
                    // then render glyph full-size with original colors on top.
                    cursor_marker = Some((
                        [quad_origin[0], quad_origin[1] + cell_h * 0.85],
                        [cell_w * cell_span, cell_h * 0.15],
                        [
                            cursor_color[0],
                            cursor_color[1],
                            cursor_color[2],
                            cursor_color[3] * 0.9,
                        ],
                    ));
                }
            }
        }

        // Full-size glyph quad dimensions (so combining marks etc. aren't clipped
        // by Bar/Underline cursor marker size).
        let glyph_quad_size = [cell_w * cell_span, cell_h];
        let glyph_quad_origin = [cd.col as f32 * cell_w, cd.row as f32 * cell_h];

        if ch == ' ' || ch == '\0' || cd.codepoint == 0 {
            // Empty cell: emit cursor marker (if any) but NOT the background
            // quad — cursor marker itself serves as the background.
            if let Some((qo, qs, bg)) = cursor_marker.take() {
                instances.push(CellInstance {
                    quad_origin: qo,
                    atlas_offset: [0.0; 2],
                    atlas_size: [0.0; 2],
                    fg_color: effective_fg,
                    bg_color: bg,
                    quad_size: qs,
                    flags: cd.flags as f32,
                    bearing: [0.0; 2],
                    glyph_advance_width: 0.0,
                });
            } else {
                instances.push(CellInstance {
                    quad_origin,
                    atlas_offset: [0.0; 2],
                    atlas_size: [0.0; 2],
                    fg_color: effective_fg,
                    bg_color: effective_bg,
                    quad_size,
                    flags: cd.flags as f32,
                    bearing: [0.0; 2],
                    glyph_advance_width: 0.0,
                });
            }
            continue;
        }

        // Non-empty cell: emit glyph quad first, then cursor marker
        // (if any; for Bar/Underline) on top so the thin bar/underline
        // is visible over the glyph.
        // Primary glyph
        if let Some(info) = font_pipeline.glyph_information(ch) {
            let uv_x = info.atlas_x as f32 / atlas_width;
            let uv_y = info.atlas_y as f32 / atlas_height;
            let uv_w = info.width as f32 / atlas_width;
            let uv_h = info.height as f32 / atlas_height;
            let bearing_x = info.placement.left as f32;
            let glyph_h = info.height as f32 / raster_scale;
            let raw_bearing_y = ascent_pixels * raster_scale - info.placement.top as f32;
            let bearing_y = if glyph_h > cell_h {
                (cell_h - glyph_h) / 2.0 * raster_scale
            } else {
                raw_bearing_y
            };

            instances.push(CellInstance {
                quad_origin: glyph_quad_origin,
                atlas_offset: [uv_x, uv_y],
                atlas_size: [uv_w, uv_h],
                fg_color: effective_fg,
                bg_color: effective_bg,
                quad_size: glyph_quad_size,
                flags: cd.flags as f32,
                bearing: [bearing_x, bearing_y],
                glyph_advance_width: info.advance_width,
            });

            // Grapheme continuation codepoints (combining marks, emoji ZWJ, etc.)
            // Rendered as overlay instances on top of the base glyph.
            for cp in &cd.grapheme_extra {
                if *cp == 0 {
                    continue;
                }
                let Some(mark_ch) = char::from_u32(*cp) else {
                    continue;
                };
                let Some(info) = font_pipeline.glyph_information(mark_ch) else {
                    continue;
                };
                let uv_x = info.atlas_x as f32 / atlas_width;
                let uv_y = info.atlas_y as f32 / atlas_height;
                let uv_w = info.width as f32 / atlas_width;
                let uv_h = info.height as f32 / atlas_height;
                let bearing_x = info.placement.left as f32;
                let glyph_h = info.height as f32 / raster_scale;
                let raw_bearing_y = ascent_pixels * raster_scale - info.placement.top as f32;
                let bearing_y = if glyph_h > cell_h {
                    (cell_h - glyph_h) / 2.0 * raster_scale
                } else {
                    raw_bearing_y
                };

                instances.push(CellInstance {
                    quad_origin: glyph_quad_origin,
                    atlas_offset: [uv_x, uv_y],
                    atlas_size: [uv_w, uv_h],
                    fg_color: effective_fg,
                    bg_color: effective_bg,
                    quad_size: glyph_quad_size,
                    flags: cd.flags as f32,
                    bearing: [bearing_x, bearing_y],
                    glyph_advance_width: 0.0,
                });
            }
        } else {
            // Glyph not found in atlas – push a blank background quad so
            // the cell background (including selection/highlight) is visible.
            // The glyph will appear once font atlas is rebuilt.
            instances.push(CellInstance {
                quad_origin,
                atlas_offset: [0.0; 2],
                atlas_size: [0.0; 2],
                fg_color: effective_fg,
                bg_color: effective_bg,
                quad_size,
                flags: cd.flags as f32,
                bearing: [0.0; 2],
                glyph_advance_width: 0.0,
            });
        }

        // Cursor marker (for Bar/Underline styles), drawn on top of glyph.
        // Use flags=0 so cell style flags (dim, underline, strikethrough)
        // from the cell aren't applied to the cursor marker.
        if let Some((qo, qs, bg)) = cursor_marker.take() {
            instances.push(CellInstance {
                quad_origin: qo,
                atlas_offset: [0.0; 2],
                atlas_size: [0.0; 2],
                fg_color: effective_fg,
                bg_color: bg,
                quad_size: qs,
                flags: 0.0,
                bearing: [0.0; 2],
                glyph_advance_width: 0.0,
            });
        }
    }
    Some(instances)
}
