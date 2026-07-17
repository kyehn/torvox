use std::collections::HashMap;

use super::CellInstance;
use torvox_core::selection::SelectionMode;

pub struct CellInstanceConfig<'a> {
    pub atlas_width: f32,
    pub atlas_height: f32,
    pub projection_height: f32,
    pub selection: Option<SelectionRange>,
    pub selection_bg: Option<[f32; 4]>,
    pub search_highlights: &'a [SearchHighlight],
    pub cursor_color: Option<[f32; 4]>,
    pub cursor_style: torvox_core::cursor::CursorStyle,
    pub surface_bg: [f32; 4],
    pub dirty_rows: &'a [bool],
    pub render_scale: f32,
    pub cached_instances: &'a [CellInstance],
    pub cached_row_ends: &'a [usize],
}

#[derive(Debug, Clone, Copy, Default)]
pub struct SelectionRange {
    pub start_row: i32,
    pub start_col: i32,
    pub end_row: i32,
    pub end_col: i32,
    pub active: bool,
    pub mode: SelectionMode,
    pub origin: Option<(i32, i32)>,
}

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

#[derive(Debug, Clone)]
pub struct SearchHighlight {
    pub row: i32,
    pub start_col: i32,
    pub end_col_exclusive: i32,
    pub color: [u8; 4],
}

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

pub(crate) fn color_f32x4_eq(a: [f32; 4], b: [f32; 4]) -> bool {
    a[0].to_bits() == b[0].to_bits()
        && a[1].to_bits() == b[1].to_bits()
        && a[2].to_bits() == b[2].to_bits()
        && a[3].to_bits() == b[3].to_bits()
}

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
fn apply_search_highlight(fg: &mut [f32; 4], bg: &mut [f32; 4], hl: [u8; 4]) {
    if hl[3] >= 128 {
        std::mem::swap(fg, bg);
    }
    *bg = blend_highlight(*bg, hl);
}

pub fn build_cell_instances_from_snapshot(
    snapshot: &torvox_terminal::ghostty_terminal::GridSnapshot,
    font_pipeline: &mut crate::font::FontPipeline,
    config: CellInstanceConfig<'_>,
) -> Vec<CellInstance> {
    let mut instances = Vec::new();
    let mut _row_ends = Vec::new();
    build_cell_instances_into(
        snapshot,
        font_pipeline,
        config,
        &mut instances,
        &mut _row_ends,
    );
    instances
}

pub fn build_cell_instances_into(
    snapshot: &torvox_terminal::ghostty_terminal::GridSnapshot,
    font_pipeline: &mut crate::font::FontPipeline,
    config: CellInstanceConfig<'_>,
    instances: &mut Vec<CellInstance>,
    row_ends: &mut Vec<usize>,
) {
    let atlas_width = config.atlas_width;
    let atlas_height = config.atlas_height;
    let projection_height = config.projection_height * config.render_scale;
    let selection = config.selection;
    let selection_bg = config.selection_bg;
    let search_highlights = config.search_highlights;
    let cursor_color = config.cursor_color;
    let cursor_style = config.cursor_style;
    let surface_bg = config.surface_bg;
    let rows = snapshot.rows;
    let cols = snapshot.cols;
    let (mut cell_w, mut cell_h) = font_pipeline.cell_metrics();
    cell_w *= config.render_scale;
    cell_h *= config.render_scale;
    let ascent_pixels = font_pipeline.ascent_pixels();
    let raster_scale = font_pipeline.get_raster_scale();
    let expected = (snapshot.rows * snapshot.cols) as usize;
    if snapshot.cells.len() < expected {
        log::warn!(
            "build_cell_instances_into: snapshot cells too short ({} < {}), skipping render",
            snapshot.cells.len(),
            expected,
        );
        instances.clear();
        return;
    }

    instances.clear();
    let use_cache = !config.dirty_rows.is_empty()
        && config.dirty_rows.len() >= rows as usize
        && config.cached_row_ends.len() >= rows as usize
        && config.cached_instances.len() > config.cached_row_ends[rows as usize - 1];
    if use_cache {
        instances.reserve(config.cached_instances.len());
    } else {
        instances.reserve((rows * cols) as usize);
    }
    row_ends.clear();

    let cursor_row = snapshot.cursor_row;
    let cursor_col = snapshot.cursor_col;
    let cursor_visible = snapshot.cursor_visible;

    let mut glyph_found = 0u64;
    let mut glyph_not_found = 0u64;

    const CURSOR_BAR_WIDTH_RATIO: f32 = 0.15;
    const CURSOR_UNDERLINE_HEIGHT_RATIO: f32 = 0.15;
    const CURSOR_BLOCK_ALPHA: f32 = 0.7;
    const CURSOR_LINE_ALPHA: f32 = 0.9;

    let mut highlights_by_row: HashMap<i32, Vec<&SearchHighlight>> = HashMap::new();
    for h in search_highlights {
        highlights_by_row.entry(h.row).or_default().push(h);
    }

    for row in 0..rows {
        if projection_height > 0.0 && (row as f32 * cell_h) >= projection_height {
            break;
        }

        if use_cache && !config.dirty_rows[row as usize] {
            let ru = row as usize;
            let start = if ru == 0 {
                0_usize
            } else {
                config.cached_row_ends[ru - 1]
            };
            let end = config.cached_row_ends[ru];
            instances.extend_from_slice(&config.cached_instances[start..end]);
            row_ends.push(instances.len());
            continue;
        }
        let mut skip_cols = 0u32;
        for col in 0..cols {
            if skip_cols > 0 {
                skip_cols -= 1;
                continue;
            }

            let idx = (row * cols + col) as usize;
            let cell = &snapshot.cells[idx];
            let cell_span = cell.width.max(1) as f32;
            let is_cursor = cursor_visible && row == cursor_row && col == cursor_col;

            if cell.codepoint == 0 || cell.codepoint == 0x20 {
                let (mut fg, mut bg) = if cell.reverse {
                    (cell.background, cell.foreground)
                } else {
                    (cell.foreground, cell.background)
                };
                if selection.unwrap_or_default().contains(row, col, cols) {
                    if let Some(sbg) = selection_bg {
                        bg = sbg;
                    } else {
                        std::mem::swap(&mut fg, &mut bg);
                    }
                }
                if let Some(hl) = cell_highlight(row, col, &highlights_by_row) {
                    apply_search_highlight(&mut fg, &mut bg, *hl);
                }
                let has_special_state = is_cursor
                    || selection.unwrap_or_default().contains(row, col, cols)
                    || cell_highlight(row, col, &highlights_by_row).is_some()
                    || cell.reverse;
                if (cell.codepoint == 0 || cell.codepoint == 0x20)
                    && !has_special_state
                    && color_f32x4_eq(bg, surface_bg)
                {
                    if cell_span > 1.0 {
                        skip_cols = (cell_span as u32) - 1;
                    }
                    continue;
                }
                let base_x = col as f32 * cell_w;
                let base_y = row as f32 * cell_h;
                let (quad_size, quad_origin) = if is_cursor {
                    let raw_cursor_bg = cursor_color.unwrap_or([1.0, 1.0, 1.0, 1.0]);
                    let cursor_alpha = match cursor_style {
                        torvox_core::cursor::CursorStyle::Block => CURSOR_BLOCK_ALPHA,
                        _ => CURSOR_LINE_ALPHA,
                    };
                    let cursor_bg = [
                        raw_cursor_bg[0],
                        raw_cursor_bg[1],
                        raw_cursor_bg[2],
                        raw_cursor_bg[3] * cursor_alpha,
                    ];
                    fg = bg;
                    bg = cursor_bg;
                    match cursor_style {
                        torvox_core::cursor::CursorStyle::Block => {
                            ([cell_w, cell_h], [base_x, base_y])
                        }
                        torvox_core::cursor::CursorStyle::Bar => {
                            ([cell_w * CURSOR_BAR_WIDTH_RATIO, cell_h], [base_x, base_y])
                        }
                        torvox_core::cursor::CursorStyle::Underline => (
                            [cell_w, cell_h * CURSOR_UNDERLINE_HEIGHT_RATIO],
                            [
                                base_x,
                                base_y + cell_h - cell_h * CURSOR_UNDERLINE_HEIGHT_RATIO,
                            ],
                        ),
                    }
                } else {
                    ([cell_w, cell_h], [base_x, base_y])
                };
                instances.push(CellInstance {
                    quad_origin,
                    atlas_offset: [0.0; 2],
                    atlas_size: [0.0; 2],
                    fg_color: fg,
                    bg_color: bg,
                    quad_size,
                    flags: 0.0,
                    bearing: [0.0; 2],
                    glyph_advance_width: 0.0,
                });
                if cell_span > 1.0 {
                    skip_cols = (cell_span as u32) - 1;
                }
                continue;
            }

            let mut run_len = 1u32;
            if !is_cursor {
                let mut adv = col + cell_span as u32;
                while adv < cols {
                    let nidx = (row * cols + adv) as usize;
                    let next = &snapshot.cells[nidx];
                    let next_cursor = cursor_visible && row == cursor_row && adv == cursor_col;
                    if next.codepoint == 0 || next.codepoint == 0x20 {
                        break;
                    }
                    if next_cursor {
                        break;
                    }
                    let attrs_differ = !color_f32x4_eq(next.foreground, cell.foreground)
                        || !color_f32x4_eq(next.background, cell.background)
                        || next.bold != cell.bold
                        || next.dim != cell.dim
                        || next.italic != cell.italic
                        || next.underline != cell.underline
                        || next.double_underline != cell.double_underline
                        || next.reverse != cell.reverse
                        || next.strikethrough != cell.strikethrough
                        || next.overline != cell.overline
                        || next.uri.is_some() != cell.uri.is_some();
                    if attrs_differ {
                        break;
                    }
                    run_len += 1;
                    adv += next.width.max(1) as u32;
                }
            }

            if run_len > 1 {
                let mut run_text = String::with_capacity(run_len as usize * 4);
                let mut adv_col = col;
                let mut run_skip = 0u32;
                for _ in 0..run_len {
                    let cidx = (row * cols + adv_col) as usize;
                    let c = &snapshot.cells[cidx];
                    if let Some(ch) = char::from_u32(c.codepoint) {
                        run_text.push(ch);
                    }
                    for &cp in c.graphemes.iter().skip(1) {
                        if let Some(gch) = char::from_u32(cp) {
                            run_text.push(gch);
                        }
                    }
                    let span = c.width.max(1) as u32;
                    run_skip += span - 1;
                    adv_col += span;
                }

                let shaped = font_pipeline.shape_run(&run_text);
                let flags = (if cell.bold { 1.0 } else { 0.0 })
                    + (if cell.italic { 2.0 } else { 0.0 })
                    + (if cell.reverse { 4.0 } else { 0.0 })
                    + (if cell.underline { 8.0 } else { 0.0 })
                    + (if cell.uri.is_some() { 16.0 } else { 0.0 })
                    + (if cell.strikethrough { 32.0 } else { 0.0 })
                    + (if cell.overline { 64.0 } else { 0.0 })
                    + (if cell.dim { 128.0 } else { 0.0 })
                    + (if cell.double_underline { 256.0 } else { 0.0 });

                for sg in &shaped {
                    let gcol_f = sg.x / cell_w;
                    let gcol = (col as f32 + gcol_f).round() as u32;
                    let gspan = ((sg.w / cell_w).round() as u32).max(1);

                    let cell_idx = (row * cols + gcol) as usize;
                    let ref_cell = &snapshot.cells[cell_idx];
                    let g_cursor = cursor_visible && row == cursor_row && gcol == cursor_col;

                    let (mut gfg, mut gbg) = if ref_cell.reverse {
                        (ref_cell.background, ref_cell.foreground)
                    } else {
                        (ref_cell.foreground, ref_cell.background)
                    };
                    if selection.unwrap_or_default().contains(row, gcol, cols) {
                        if let Some(sbg) = selection_bg {
                            gbg = sbg;
                        } else {
                            std::mem::swap(&mut gfg, &mut gbg);
                        }
                    }
                    if let Some(hl) = cell_highlight(row, gcol, &highlights_by_row) {
                        apply_search_highlight(&mut gfg, &mut gbg, *hl);
                    }
                    let (gfg_scoped, gbg_scoped) = if g_cursor {
                        let raw_cursor_bg = cursor_color.unwrap_or([1.0, 1.0, 1.0, 1.0]);
                        let (cursor_alpha, gfg_override) = match cursor_style {
                            torvox_core::cursor::CursorStyle::Block => (CURSOR_BLOCK_ALPHA, gbg),
                            _ => (CURSOR_LINE_ALPHA, gfg),
                        };
                        let cursor_bg = [
                            raw_cursor_bg[0],
                            raw_cursor_bg[1],
                            raw_cursor_bg[2],
                            raw_cursor_bg[3] * cursor_alpha,
                        ];
                        (gfg_override, cursor_bg)
                    } else {
                        (gfg, gbg)
                    };

                    if let Some(info) =
                        font_pipeline.glyph_information_for_glyph(sg.font_id, sg.glyph_id)
                    {
                        glyph_found += 1;
                        let uv_x = info.atlas_x as f32 / atlas_width;
                        let uv_y = info.atlas_y as f32 / atlas_height;
                        let uv_w = info.width as f32 / atlas_width;
                        let uv_h = info.height as f32 / atlas_height;
                        let bearing_x = info.placement.left as f32 + sg.x_offset * raster_scale;
                        let glyph_h = info.height as f32 / raster_scale;
                        let raw_bearing_y =
                            ascent_pixels * raster_scale - info.placement.top as f32;
                        let bearing_y = if glyph_h > cell_h {
                            (cell_h - glyph_h) / 2.0 * raster_scale * raster_scale
                                + sg.y_offset * raster_scale
                        } else {
                            raw_bearing_y + sg.y_offset * raster_scale
                        };

                        instances.push(CellInstance {
                            quad_origin: [gcol as f32 * cell_w, row as f32 * cell_h],
                            atlas_offset: [uv_x, uv_y],
                            atlas_size: [uv_w, uv_h],
                            fg_color: gfg_scoped,
                            bg_color: gbg_scoped,
                            quad_size: [cell_w * gspan as f32, cell_h],
                            flags,
                            bearing: [bearing_x, bearing_y],
                            glyph_advance_width: info.advance_width,
                        });
                    } else {
                        glyph_not_found += 1;
                        instances.push(CellInstance {
                            quad_origin: [gcol as f32 * cell_w, row as f32 * cell_h],
                            atlas_offset: [0.0; 2],
                            atlas_size: [1.0 / atlas_width, 1.0 / atlas_height],
                            fg_color: gfg_scoped,
                            bg_color: gbg_scoped,
                            quad_size: [cell_w * gspan as f32, cell_h],
                            flags,
                            bearing: [0.0; 2],
                            glyph_advance_width: 0.0,
                        });
                    }
                }

                skip_cols = run_len - 1 + run_skip;
                continue;
            }

            let ch = char::from_u32(cell.codepoint).unwrap_or('\u{FFFD}');
            let flags = if cell.bold { 1.0 } else { 0.0 }
                + if cell.italic { 2.0 } else { 0.0 }
                + if cell.reverse { 4.0 } else { 0.0 }
                + if cell.underline { 8.0 } else { 0.0 }
                + if cell.uri.is_some() { 16.0 } else { 0.0 }
                + if cell.strikethrough { 32.0 } else { 0.0 }
                + if cell.overline { 64.0 } else { 0.0 }
                + if cell.dim { 128.0 } else { 0.0 }
                + if cell.double_underline { 256.0 } else { 0.0 };

            let (mut fg, mut bg) = if cell.reverse {
                (cell.background, cell.foreground)
            } else {
                (cell.foreground, cell.background)
            };

            if selection.unwrap_or_default().contains(row, col, cols) {
                if let Some(sbg) = selection_bg {
                    bg = sbg;
                } else {
                    std::mem::swap(&mut fg, &mut bg);
                }
            }
            if let Some(hl) = cell_highlight(row, col, &highlights_by_row) {
                apply_search_highlight(&mut fg, &mut bg, *hl);
            }

            let (fg, bg) = if is_cursor {
                let raw_cursor_bg = cursor_color.unwrap_or([1.0, 1.0, 1.0, 1.0]);
                let (cursor_alpha, fg_override) = match cursor_style {
                    torvox_core::cursor::CursorStyle::Block => (CURSOR_BLOCK_ALPHA, bg),
                    _ => (CURSOR_LINE_ALPHA, fg),
                };
                let cursor_bg = [
                    raw_cursor_bg[0],
                    raw_cursor_bg[1],
                    raw_cursor_bg[2],
                    raw_cursor_bg[3] * cursor_alpha,
                ];
                (fg_override, cursor_bg)
            } else {
                (fg, bg)
            };

            let base_x = col as f32 * cell_w;
            let base_y = row as f32 * cell_h;
            let (cursor_quad_size, cursor_quad_origin) = if is_cursor {
                match cursor_style {
                    torvox_core::cursor::CursorStyle::Block => {
                        ([cell_w * cell_span, cell_h], [base_x, base_y])
                    }
                    torvox_core::cursor::CursorStyle::Bar
                    | torvox_core::cursor::CursorStyle::Underline => {
                        ([cell_w * cell_span, cell_h], [base_x, base_y])
                    }
                }
            } else {
                ([cell_w * cell_span, cell_h], [base_x, base_y])
            };

            if let Some(info) = font_pipeline.glyph_information(ch) {
                glyph_found += 1;
                let uv_x = info.atlas_x as f32 / atlas_width;
                let uv_y = info.atlas_y as f32 / atlas_height;
                let uv_w = info.width as f32 / atlas_width;
                let uv_h = info.height as f32 / atlas_height;

                let bearing_x = info.placement.left as f32;
                let glyph_h = info.height as f32 / raster_scale;
                let raw_bearing_y = ascent_pixels * raster_scale - info.placement.top as f32;
                let bearing_y = if glyph_h > cell_h {
                    (cell_h - glyph_h) / 2.0
                } else {
                    raw_bearing_y
                };

                instances.push(CellInstance {
                    quad_origin: cursor_quad_origin,
                    atlas_offset: [uv_x, uv_y],
                    atlas_size: [uv_w, uv_h],
                    fg_color: fg,
                    bg_color: bg,
                    quad_size: cursor_quad_size,
                    flags,
                    bearing: [bearing_x, bearing_y],
                    glyph_advance_width: info.advance_width,
                });
                if cell_span > 1.0 {
                    skip_cols = (cell_span as u32) - 1;
                }
                for &cp in cell.graphemes.iter().skip(1) {
                    let Some(mark_ch) = char::from_u32(cp) else {
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
                        quad_origin: cursor_quad_origin,
                        atlas_offset: [uv_x, uv_y],
                        atlas_size: [uv_w, uv_h],
                        fg_color: fg,
                        bg_color: bg,
                        quad_size: cursor_quad_size,
                        flags,
                        bearing: [bearing_x, bearing_y],
                        glyph_advance_width: 0.0,
                    });
                }
            } else {
                glyph_not_found += 1;
                instances.push(CellInstance {
                    quad_origin: cursor_quad_origin,
                    atlas_offset: [0.0; 2],
                    atlas_size: [1.0 / atlas_width, 1.0 / atlas_height],
                    fg_color: fg,
                    bg_color: bg,
                    quad_size: cursor_quad_size,
                    flags,
                    bearing: [0.0; 2],
                    glyph_advance_width: 0.0,
                });
                if cell_span > 1.0 {
                    skip_cols = (cell_span as u32) - 1;
                }
                for &cp in cell.graphemes.iter().skip(1) {
                    let Some(mark_ch) = char::from_u32(cp) else {
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
                        quad_origin: cursor_quad_origin,
                        atlas_offset: [uv_x, uv_y],
                        atlas_size: [uv_w, uv_h],
                        fg_color: fg,
                        bg_color: bg,
                        quad_size: cursor_quad_size,
                        flags,
                        bearing: [bearing_x, bearing_y],
                        glyph_advance_width: 0.0,
                    });
                }
            }
        }
    }
    if glyph_found + glyph_not_found > 0 {
        log::debug!(
            "build_cell_instances: glyph_found={} glyph_not_found={} total={}",
            glyph_found,
            glyph_not_found,
            glyph_found + glyph_not_found
        );
    }
}

#[cfg(test)]
pub struct FlatGrid {
    pub rows: u32,
    pub cols: u32,
    pub chars: Vec<char>,
    pub foreground: Vec<[f32; 4]>,
    pub background: Vec<[f32; 4]>,
    pub selected: Vec<bool>,
}

#[cfg(test)]
impl FlatGrid {
    pub fn new(rows: u32, cols: u32) -> Self {
        let len = (rows * cols) as usize;
        Self {
            rows,
            cols,
            chars: vec![' '; len],
            foreground: vec![[1.0, 1.0, 1.0, 1.0]; len],
            background: vec![[0.0, 0.0, 0.0, 1.0]; len],
            selected: vec![false; len],
        }
    }

    pub fn set_cell(
        &mut self,
        row: u32,
        col: u32,
        ch: char,
        foreground: [f32; 4],
        background: [f32; 4],
    ) {
        let idx = (row * self.cols + col) as usize;
        if idx < self.chars.len() {
            self.chars[idx] = ch;
            self.foreground[idx] = foreground;
            self.background[idx] = background;
        }
    }

    pub fn cell(&self, row: u32, col: u32) -> Option<(char, [f32; 4], [f32; 4])> {
        if row >= self.rows || col >= self.cols {
            return None;
        }
        let idx = (row * self.cols + col) as usize;
        if idx < self.chars.len() {
            Some((self.chars[idx], self.foreground[idx], self.background[idx]))
        } else {
            None
        }
    }
}

#[cfg(test)]
pub fn build_cell_instances_from_flat(
    flat: &FlatGrid,
    font_pipeline: &mut crate::font::FontPipeline,
    atlas_width: f32,
    atlas_height: f32,
) -> Vec<CellInstance> {
    let (cell_w, cell_h) = font_pipeline.cell_metrics();
    let ascent_pixels = font_pipeline.ascent_pixels();
    let mut instances = Vec::with_capacity((flat.rows * flat.cols) as usize);

    for row in 0..flat.rows {
        for col in 0..flat.cols {
            if let Some((ch, fg, bg)) = flat.cell(row, col) {
                let idx = (row * flat.cols + col) as usize;
                let (fg, bg) = if flat.selected.get(idx).copied().unwrap_or(false) {
                    (bg, fg)
                } else {
                    (fg, bg)
                };
                if ch == ' ' {
                    instances.push(CellInstance {
                        quad_origin: [col as f32 * cell_w, row as f32 * cell_h],
                        atlas_offset: [0.0, 0.0],
                        atlas_size: [0.0, 0.0],
                        fg_color: [0.0, 0.0, 0.0, 0.0],
                        bg_color: bg,
                        quad_size: [cell_w, cell_h],
                        flags: 0.0,
                        bearing: [0.0; 2],
                        glyph_advance_width: 0.0,
                    });
                } else if let Some(info) = font_pipeline.glyph_information(ch) {
                    let uv_x = info.atlas_x as f32 / atlas_width;
                    let uv_y = info.atlas_y as f32 / atlas_height;
                    let uv_w = info.width as f32 / atlas_width;
                    let uv_h = info.height as f32 / atlas_height;

                    let bearing_x = info.placement.left as f32;
                    let bearing_y = ascent_pixels - info.placement.top as f32;

                    instances.push(CellInstance {
                        quad_origin: [col as f32 * cell_w, row as f32 * cell_h],
                        atlas_offset: [uv_x, uv_y],
                        atlas_size: [uv_w, uv_h],
                        fg_color: fg,
                        bg_color: bg,
                        quad_size: [cell_w, cell_h],
                        flags: 0.0,
                        bearing: [bearing_x, bearing_y],
                        glyph_advance_width: info.advance_width,
                    });
                }
            }
        }
    }
    instances
}
