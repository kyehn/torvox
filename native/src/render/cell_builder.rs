//! Cell instance builder — converts terminal grid state into GPU instance data.
//!
//! # Requirements
//! - FR-050 — surface lifecycle: cell instances rebuilt on resize/surface recreation
use crate::render::CellInstance;

use crate::terminal::CursorStyle;
use crate::terminal::SelectionMode;
use crate::terminal::ghostty_terminal::cell_flags;

use foldhash::fast::RandomState;
use std::collections::HashMap;

/// Cursor state passed to build_instances_from_cell_data() for cursor rendering.
#[derive(Debug, Clone, Copy, Default)]
pub struct CellCursor {
    pub row: u32,
    pub col: u32,
    pub visible: bool,
    pub style: CursorStyle,
    pub color: Option<[f32; 4]>,
}

/// Configuration for a cell-instance build pass.
///
/// Shared by the full ([`build_instances_from_cell_data`]) and incremental
/// ([`build_instances_cached`]) builders. Bulk data stays as separate
/// arguments: the cell buffer, the mutable font pipeline, and the output
/// instance buffer.
#[derive(Debug, Clone, Copy)]
pub struct CellInstanceConfig<'a> {
    pub rows: u32,
    pub cols: u32,
    pub grid_cell_w: f32,
    pub grid_cell_h: f32,
    pub cursor: CellCursor,
    pub atlas_width: f32,
    pub atlas_height: f32,
    pub selection: Option<SelectionRange>,
    pub search_highlights: &'a [SearchHighlight],
}

/// A selected range of characters to highlight with a background color.
///
/// Supports Char/Word/Semantic (box), Line (full rows), and Block modes.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
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
    by_row: &'a HashMap<i32, Vec<&'a SearchHighlight>, RandomState>,
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

/// Row-level instance cache for incremental rendering (FR-013 / NFR-010).
///
/// Mirrors the test-only reference in `old_path::build_cell_instances_into`
/// (row_ends + per-row instance slices): after a build, `row_ends[r]` is the
/// exclusive end index of row `r`'s instances in `instances`. Clean rows can
/// then be copied from the previous frame instead of re-walking their cells
/// through the font atlas (NFR-010: only repaint dirty rows).
#[derive(Debug)]
pub struct CachedInstances {
    row_ends: Vec<usize>,
    instances: Vec<CellInstance>,
    rows: u32,
    cols: u32,
    /// Whether a build has ever populated this cache. A freshly created
    /// cache (e.g. right after a resize) is dimensionally "compatible"
    /// with the new grid but holds NO row data: serving "clean" rows from
    /// it would copy 0 instances and drop rows regression).
    built: bool,
}

impl CachedInstances {
    pub fn new(rows: u32, cols: u32) -> Self {
        Self {
            row_ends: vec![0; rows as usize],
            instances: Vec::new(),
            rows,
            cols,
            built: false,
        }
    }

    /// The full instance list of the last build (row-major, per `row_ends`).
    pub fn instances(&self) -> &[CellInstance] {
        &self.instances
    }

    /// Whether the cache still matches the current grid size (a resize
    /// invalidates the row layout and forces a full rebuild) AND holds row
    /// data from a previous build. An empty, never-built cache must not be
    /// trusted for incremental serving.
    pub fn is_compatible(&self, rows: u32, cols: u32) -> bool {
        self.built && self.rows == rows && self.cols == cols && self.row_ends.len() == rows as usize
    }

    /// `[start, end)` instance slice belonging to `row`.
    pub(crate) fn row_slice(&self, row: usize) -> (usize, usize) {
        let start = if row == 0 { 0 } else { self.row_ends[row - 1] };
        (start, self.row_ends[row])
    }

    /// Replace the cache contents after a build.
    fn update(&mut self, rows: u32, cols: u32, instances: &[CellInstance], row_ends: Vec<usize>) {
        self.rows = rows;
        self.cols = cols;
        self.row_ends = row_ends;
        self.instances.clear();
        self.instances.extend_from_slice(instances);
        self.built = true;
    }
}

/// Partition a flat, row-major `cell_data` slice into per-row ranges using
/// the `CellData.row` field. Row lengths vary because wide/spacer cells are
/// elided by the terminal; rows without cells map to empty ranges. Returns
/// `None` when cells exist beyond `rows` (or the input is not row-major) —
/// stale/mismatched data that must not be trusted for incremental builds.
pub(crate) fn build_row_ranges(
    cell_data: &[crate::terminal::ghostty_terminal::CellData],
    rows: u32,
) -> Option<Vec<std::ops::Range<usize>>> {
    let mut ranges: Vec<std::ops::Range<usize>> = (0..rows as usize).map(|_| 0..0).collect();
    let mut current = 0usize;
    for (r, range) in ranges.iter_mut().enumerate() {
        let start = current;
        while current < cell_data.len() && cell_data[current].row as usize == r {
            current += 1;
        }
        *range = start..current;
    }
    if current < cell_data.len() {
        return None;
    }
    Some(ranges)
}

/// Bytewise comparison of two per-row cell slices. `CellData` is a POD
/// bytemuck struct, so this is valid and faster than a field-by-field diff.
fn rows_equal(
    old: &[crate::terminal::ghostty_terminal::CellData],
    new: &[crate::terminal::ghostty_terminal::CellData],
) -> bool {
    let a: &[u8] = bytemuck::cast_slice(old);
    let b: &[u8] = bytemuck::cast_slice(new);
    a == b
}

/// Derive a per-row dirty mask by diffing two consecutive CellData
/// snapshots. Row boundaries come from `build_row_ranges`; on degenerate
/// input (row ranges unbuildable) everything is conservatively dirty.
pub(crate) fn diff_dirty_rows(
    old: &[crate::terminal::ghostty_terminal::CellData],
    new: &[crate::terminal::ghostty_terminal::CellData],
    rows: u32,
) -> Vec<bool> {
    let mut dirty = vec![false; rows as usize];
    let (Some(old_ranges), Some(new_ranges)) =
        (build_row_ranges(old, rows), build_row_ranges(new, rows))
    else {
        dirty.fill(true);
        return dirty;
    };
    for r in 0..rows as usize {
        let o = &old[old_ranges[r].clone()];
        let n = &new[new_ranges[r].clone()];
        if !rows_equal(o, n) {
            dirty[r] = true;
        }
    }
    dirty
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
    config: CellInstanceConfig<'_>,
    font_pipeline: &mut crate::render::font::FontPipeline,
    instances: &mut Vec<CellInstance>,
) -> Option<()> {
    build_row_instances_into(cell_data, config, font_pipeline, None, None, instances)
}

/// Incremental variant of [`build_instances_from_cell_data`] (row-level
/// dirty caching, FR-013 / NFR-010): only rows flagged in `dirty_rows` are
/// rebuilt; clean rows are copied verbatim from `cache`. `cache` is updated
/// in place so the next frame can reuse it. A dirty mask shorter than
/// `rows`, or a cache incompatible with the grid size, degrades to a full
/// rebuild of every row.
pub fn build_instances_cached(
    cell_data: &[crate::terminal::ghostty_terminal::CellData],
    config: CellInstanceConfig<'_>,
    font_pipeline: &mut crate::render::font::FontPipeline,
    dirty_rows: &[bool],
    cache: &mut CachedInstances,
    instances: &mut Vec<CellInstance>,
) -> Option<()> {
    build_row_instances_into(
        cell_data,
        config,
        font_pipeline,
        Some(dirty_rows),
        Some(cache),
        instances,
    )
}

/// Shared implementation behind the full and incremental builders.
///
/// Builds quad instances for every grid row. When `dirty_rows` and `cache`
/// are supplied and coherent, clean rows copy their cached instances
/// instead of re-walking their cells through the font atlas.
fn build_row_instances_into(
    cell_data: &[crate::terminal::ghostty_terminal::CellData],
    config: CellInstanceConfig<'_>,
    font_pipeline: &mut crate::render::font::FontPipeline,
    dirty_rows: Option<&[bool]>,
    mut cache: Option<&mut CachedInstances>,
    instances: &mut Vec<CellInstance>,
) -> Option<()> {
    let CellInstanceConfig {
        rows,
        cols,
        grid_cell_w,
        grid_cell_h,
        cursor,
        atlas_width,
        atlas_height,
        selection,
        search_highlights,
    } = config;
    // Quad geometry uses GRID cell dimensions (surface/rows, surface/cols),
    // not font metrics: font cell height (~20px) is smaller than the grid
    // row height (~92px on a 2209px surface / 24 rows), so quads sized by
    // font metrics left visible gaps between rows. The shader positions
    // glyphs inside the grid quad via bearing + ascent (glyph_h > cell_h
    // is then never true, so glyphs use the raw bearing path).
    let (cell_w, cell_h) = (grid_cell_w, grid_cell_h);
    log::info!(
        "cell_builder: grid {rows}x{cols} cell {cell_w:.1}x{cell_h:.1} cells={}",
        cell_data.len()
    );
    let _ = (rows, cols); // used by callers for projection; quad grid covers all
    let ascent_pixels = font_pipeline.ascent_pixels();
    let raster_scale = font_pipeline.get_raster_scale();
    // Cross-frame buffer reuse: the caller (Renderer) owns the Vec and
    // clears it here, avoiding a ~100KB allocation per frame at 60fps
    // (~6MB/s allocation traffic).
    instances.clear();
    instances.reserve(cell_data.len());

    let selection = selection.filter(|s| !s.is_empty);
    // foldhash (0.2, already in the dependency tree) instead of the std
    // SipHash13 default: this map is rebuilt and queried every frame
    // (~1920 hashes/frame @60fps); foldhash is ~5-10x faster on i32 keys.
    // skip the per-frame HashMap rebuild when there are
    // no highlights (the common case) — ~1920 hashes/frame saved.
    let mut highlights_by_row: HashMap<i32, Vec<&SearchHighlight>, RandomState> =
        HashMap::with_hasher(RandomState::default());
    if !search_highlights.is_empty() {
        for h in search_highlights {
            highlights_by_row.entry(h.row).or_default().push(h);
        }
    }

    // Partition into per-row ranges once; used for both the incremental
    // dirty-row decision and the per-row iteration below.
    let row_ranges = build_row_ranges(cell_data, rows)?;
    let incremental = dirty_rows.is_some_and(|d| d.len() >= rows as usize)
        && cache.as_ref().is_some_and(|c| c.is_compatible(rows, cols));

    let mut row_ends: Vec<usize> = Vec::with_capacity(rows as usize);
    for (row, range) in row_ranges.iter().enumerate() {
        if incremental && !dirty_rows.unwrap()[row] {
            // Clean row: reuse the instances built last frame (NFR-010).
            let (cs, ce) = cache.as_ref().unwrap().row_slice(row);
            instances.extend_from_slice(&cache.as_ref().unwrap().instances()[cs..ce]);
        } else {
            append_row_instances(
                cell_w,
                cell_h,
                ascent_pixels,
                raster_scale,
                atlas_width,
                atlas_height,
                cursor,
                selection,
                &highlights_by_row,
                cols,
                font_pipeline,
                instances,
                &cell_data[range.clone()],
            );
        }
        row_ends.push(instances.len());
    }
    if let Some(c) = cache.as_mut() {
        c.update(rows, cols, &instances[..], row_ends);
    }
    Some(())
}

/// Build instances for one grid row. Shared by the full and incremental
/// builders so the cell-level logic stays identical in both paths.
#[allow(clippy::too_many_arguments)]
fn append_row_instances(
    cell_w: f32,
    cell_h: f32,
    ascent_pixels: f32,
    raster_scale: f32,
    atlas_width: f32,
    atlas_height: f32,
    cursor: CellCursor,
    selection: Option<SelectionRange>,
    highlights_by_row: &HashMap<i32, Vec<&SearchHighlight>, RandomState>,
    cols: u32,
    font_pipeline: &mut crate::render::font::FontPipeline,
    instances: &mut Vec<CellInstance>,
    cell_row: &[crate::terminal::ghostty_terminal::CellData],
) {
    for cd in cell_row {
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
        // Matches termux TerminalRenderer.java:182-187 (selection &
        // reverseVideo fold into the same fg/bg swap) and Ghostty's
        // renderer inverse-video handling.
        if (cd.flags >> cell_flags::REVERSE) & 1 == 1 {
            std::mem::swap(&mut fg_color, &mut bg_color);
        }

        // Selection highlight: classic terminal inverse video — the selected
        // cell shows the background color as its text color and the
        // foreground color as its background (fg<->bg swap), so the text
        // visibly inverts instead of just getting a background tint
        //, reported as "text does not change color when
        // selected"). selection_bg is deliberately not applied here: on a
        // dark theme it would keep the text dark on a dark highlight and
        // break readability; the swap uses the terminal's own fg/bg which
        // are already theme-derived.
        if selection.unwrap_or_default().contains(cd.row, cd.col, cols) {
            std::mem::swap(&mut fg_color, &mut bg_color);
        }
        // Search highlight overlay (applied on top of selection)
        if let Some(hl) = cell_highlight(cd.row, cd.col, highlights_by_row) {
            apply_search_highlight(&mut fg_color, &mut bg_color, *hl);
        }
        let is_cursor = cursor.visible && cd.row == cursor.row && cd.col == cursor.col;
        let effective_fg = fg_color;
        let mut effective_bg = bg_color;
        let mut cursor_marker: Option<([f32; 2], [f32; 2], [f32; 4])> = None;
        // Default quad size (used for Block cursor and empty cells)
        let quad_size = [cell_w * cell_span, cell_h];
        // Block cursor height tracks the glyph (ascent+descent in
        // physical pixels), not the full grid cell — a cell-high block at
        // 420dpi looks like a giant filled rectangle around a ~66px glyph in
        // a 79px cell.

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
                let mut origin = quad_origin;
                let mut size = quad_size;
                // Block cursor on an empty cell: height = ascent+descent
                // (physical), top aligned with where a glyph would sit —
                // the glyph top edge in the cell is at
                // (ascent - glyph_top) × raster_scale; for an empty cell we
                // approximate with the ascent bearing of the default glyph.
                if is_cursor && matches!(cursor.style, CursorStyle::Block | CursorStyle::Default) {
                    let cursor_h =
                        ((ascent_pixels + font_pipeline.descent_pixels()) * raster_scale).max(1.0);
                    // Top of the em box: ascent × raster_scale. This matches
                    // where the glyph top lands (glyphs are drawn from the
                    // ascent line down), keeping cursor and text aligned.
                    origin[1] += ascent_pixels * raster_scale;
                    size[1] = cursor_h;
                }
                instances.push(CellInstance {
                    quad_origin: origin,
                    atlas_offset: [0.0; 2],
                    atlas_size: [0.0; 2],
                    fg_color: effective_fg,
                    bg_color: effective_bg,
                    quad_size: size,
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
        // Primary glyph — styled when the cell carries bold/italic flags
        // same-family styled face preferred, else synthesis).
        let cell_bold = (cd.flags >> cell_flags::BOLD) & 1 == 1;
        let cell_italic = (cd.flags >> cell_flags::ITALIC) & 1 == 1;
        if let Some(info) = font_pipeline.glyph_information_styled(ch, cell_bold, cell_italic) {
            let uv_x = info.atlas_x as f32 / atlas_width;
            let uv_y = info.atlas_y as f32 / atlas_height;
            let uv_w = info.width as f32 / atlas_width;
            let uv_h = info.height as f32 / atlas_height;
            let bearing_x = info.placement.left as f32;
            // info.height is the rasterized bitmap height in PHYSICAL pixels
            // (already × raster_scale). Compare against the physical grid
            // cell height; the centering fallback is then in physical
            // pixels too: units must not mix).
            let glyph_h_px = info.height as f32;
            let raw_bearing_y = ascent_pixels * raster_scale - info.placement.top as f32;
            let bearing_y = if glyph_h_px > cell_h {
                (cell_h - glyph_h_px) / 2.0
            } else {
                raw_bearing_y
            };
            let mut origin = glyph_quad_origin;
            let mut size = glyph_quad_size;
            // Block cursor quad tracks the glyph bitmap: same
            // height AND top edge as the glyph. Centering the cursor in the
            // cell misaligned it with the text because the glyph sits on the
            // font baseline, not at the cell center (reported as "input
            // pointer not vertically aligned with the text"). The glyph's
            // top edge inside the cell is exactly raw_bearing_y, so the
            // cursor quad starts there and keeps the glyph's bearing.
            if is_cursor && matches!(cursor.style, CursorStyle::Block | CursorStyle::Default) {
                let cursor_h = glyph_h_px.max(1.0);
                origin[1] += raw_bearing_y;
                size[1] = cursor_h;
            }

            instances.push(CellInstance {
                quad_origin: origin,
                atlas_offset: [uv_x, uv_y],
                atlas_size: [uv_w, uv_h],
                fg_color: effective_fg,
                bg_color: effective_bg,
                quad_size: size,
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
}

// ── Tests ────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::terminal::ghostty_terminal::CellData;

    fn cell_data(row: u32, col: u32, ch: char, fg: [f32; 4], bg: [f32; 4], flags: u32) -> CellData {
        CellData {
            codepoint: ch as u32,
            width: 1,
            grapheme_extra: [0; 7],
            fg_color: fg,
            bg_color: bg,
            flags,
            row,
            col,
        }
    }

    fn build(
        cells: &[CellData],
        cursor: CellCursor,
        selection: Option<SelectionRange>,
        highlights: &[SearchHighlight],
    ) -> Vec<CellInstance> {
        let mut font_pipeline = crate::render::font::FontPipeline::new(1024, 1024, 14.0);
        let mut instances = Vec::new();
        let result = build_instances_from_cell_data(
            cells,
            CellInstanceConfig {
                rows: 24,
                cols: 80,
                grid_cell_w: 1024.0 / 80.0,
                grid_cell_h: 1024.0 / 24.0,
                cursor,
                atlas_width: 1024.0,
                atlas_height: 1024.0,
                selection,
                search_highlights: highlights,
            },
            &mut font_pipeline,
            &mut instances,
        );
        assert!(
            result.is_some(),
            "build should succeed with a font pipeline"
        );
        instances
    }

    /// Without reverse/selection/highlight, a plain cell keeps its colors.
    #[test]
    fn plain_cell_keeps_colors() {
        let cells = vec![cell_data(
            0,
            0,
            'A',
            [1.0, 0.0, 0.0, 1.0],
            [0.0, 0.0, 1.0, 1.0],
            0,
        )];
        let instances = build(&cells, CellCursor::default(), None, &[]);
        assert_eq!(instances.len(), 1);
        assert_eq!(instances[0].fg_color, [1.0, 0.0, 0.0, 1.0]);
        assert_eq!(instances[0].bg_color, [0.0, 0.0, 1.0, 1.0]);
        assert_eq!(instances[0].flags, 0.0);
    }

    /// SGR 7 reverse swaps fg and bg.
    #[test]
    fn reverse_swaps_fg_bg() {
        let cells = vec![cell_data(
            0,
            0,
            'A',
            [1.0, 0.0, 0.0, 1.0],
            [0.0, 0.0, 1.0, 1.0],
            1 << cell_flags::REVERSE,
        )];
        let instances = build(&cells, CellCursor::default(), None, &[]);
        assert_eq!(instances[0].fg_color, [0.0, 0.0, 1.0, 1.0]);
        assert_eq!(instances[0].bg_color, [1.0, 0.0, 0.0, 1.0]);
    }

    /// Selection without an explicit bg swaps fg/bg.
    #[test]
    fn selection_swaps_fg_bg() {
        let cells = vec![cell_data(
            1,
            1,
            'B',
            [1.0, 0.0, 0.0, 1.0],
            [0.0, 1.0, 0.0, 1.0],
            0,
        )];
        let selection = SelectionRange {
            start_row: 1,
            start_col: 1,
            end_row: 1,
            end_col: 1,
            active: true,
            mode: SelectionMode::Char,
            ..Default::default()
        };
        let instances = build(&cells, CellCursor::default(), Some(selection), &[]);
        assert_eq!(
            instances[0].fg_color,
            [0.0, 1.0, 0.0, 1.0],
            "selection swaps fg→bg"
        );
        assert_eq!(instances[0].bg_color, [1.0, 0.0, 0.0, 1.0]);
    }

    /// Selection with an explicit bg color still uses classic inverse video
    /// the explicit selection bg no longer overrides the swap —
    /// the terminal's own fg/bg are theme-derived, so the swap keeps the
    /// selected text readable on both light and dark themes.
    #[test]
    fn selection_bg_does_not_override_inverse_video() {
        let cells = vec![cell_data(
            0,
            0,
            'C',
            [1.0, 1.0, 1.0, 1.0],
            [0.1, 0.1, 0.1, 1.0],
            0,
        )];
        let selection = SelectionRange {
            start_row: 0,
            start_col: 0,
            end_row: 0,
            end_col: 0,
            active: true,
            mode: SelectionMode::Char,
            ..Default::default()
        };
        let instances = build(&cells, CellCursor::default(), Some(selection), &[]);
        // Inverse video: fg<->bg swapped (selection_bg arg ignored).
        assert_eq!(instances[0].bg_color, [1.0, 1.0, 1.0, 1.0]);
        assert_eq!(instances[0].fg_color, [0.1, 0.1, 0.1, 1.0]);
    }

    /// Search highlight with alpha >= 128 swaps fg/bg then blends bg.
    #[test]
    fn search_highlight_swaps_and_blends() {
        let cells = vec![cell_data(
            2,
            3,
            'D',
            [1.0, 0.0, 0.0, 1.0],
            [0.0, 0.0, 1.0, 1.0],
            0,
        )];
        let hl = SearchHighlight {
            row: 2,
            start_col: 3,
            end_col_exclusive: 4,
            color: [0xFF, 0xFF, 0x00, 0xFF], // opaque yellow
        };
        let instances = build(&cells, CellCursor::default(), None, &[hl]);
        // Alpha >= 128 → swap fg/bg, then bg = blend(bg, yellow, alpha=1) = yellow.
        assert_eq!(
            instances[0].fg_color,
            [0.0, 0.0, 1.0, 1.0],
            "highlight swaps fg→bg"
        );
        assert_eq!(
            instances[0].bg_color,
            [1.0, 1.0, 0.0, 1.0],
            "blend with opaque yellow"
        );
    }

    /// Highlight alpha below 128 blends without swapping.
    #[test]
    fn search_highlight_blends_without_swap() {
        let cells = vec![cell_data(
            0,
            0,
            'E',
            [0.0, 0.0, 0.0, 1.0],
            [1.0, 1.0, 1.0, 1.0],
            0,
        )];
        let hl = SearchHighlight {
            row: 0,
            start_col: 0,
            end_col_exclusive: 1,
            color: [0xFF, 0x00, 0x00, 0x7F], // alpha ~0.5 red (below the 128 swap threshold)
        };
        let instances = build(&cells, CellCursor::default(), None, &[hl]);
        assert_eq!(
            instances[0].fg_color,
            [0.0, 0.0, 0.0, 1.0],
            "fg unchanged below alpha 128"
        );
        // bg = white * (1-a) + red * a with a = 0x7F/255
        let alpha = 0x7F as f32 / 255.0;
        let expected = 1.0 - alpha;
        assert!(
            (instances[0].bg_color[0] - 1.0).abs() < 1e-5,
            "red channel keeps base white"
        );
        assert!((instances[0].bg_color[1] - expected).abs() < 1e-5);
        assert!((instances[0].bg_color[2] - expected).abs() < 1e-5);
    }

    /// Block cursor replaces bg with cursor color (semi-transparent) and
    /// keeps the original fg so the glyph stays readable.
    #[test]
    fn block_cursor_paints_bg_keeps_fg() {
        let cells = vec![cell_data(
            5,
            5,
            'F',
            [1.0, 0.0, 0.0, 1.0],
            [0.0, 0.0, 1.0, 1.0],
            0,
        )];
        let cursor = CellCursor {
            row: 5,
            col: 5,
            visible: true,
            style: CursorStyle::Block,
            color: Some([0.0, 1.0, 0.0, 1.0]),
        };
        let instances = build(&cells, cursor, None, &[]);
        assert_eq!(
            instances[0].fg_color,
            [1.0, 0.0, 0.0, 1.0],
            "block cursor keeps fg"
        );
        assert_eq!(
            instances[0].bg_color,
            [0.0, 1.0, 0.0, 0.7],
            "bg = cursor color * 0.7"
        );
    }

    /// Bar cursor emits the glyph instance plus a thin vertical marker with
    /// flags=0 (cell style flags must not leak into the marker).
    #[test]
    fn bar_cursor_emits_marker_with_zero_flags() {
        let cells = vec![cell_data(
            3,
            3,
            'G',
            [1.0, 1.0, 1.0, 1.0],
            [0.0, 0.0, 0.0, 1.0],
            0,
        )];
        let cursor = CellCursor {
            row: 3,
            col: 3,
            visible: true,
            style: CursorStyle::Bar,
            color: Some([1.0, 0.0, 0.0, 1.0]),
        };
        let instances = build(&cells, cursor, None, &[]);
        assert!(
            instances.len() >= 2,
            "glyph + bar marker expected, got {}",
            instances.len()
        );
        // The marker instance has flags 0 and a thin quad width.
        let marker = instances.last().unwrap();
        assert_eq!(
            marker.flags, 0.0,
            "marker must not inherit cell style flags"
        );
        let glyph = &instances[0];
        assert!(
            marker.quad_size[0] < glyph.quad_size[0],
            "bar marker ({}) must be thinner than the glyph quad ({})",
            marker.quad_size[0],
            glyph.quad_size[0],
        );
        assert_eq!(marker.fg_color, [1.0, 1.0, 1.0, 1.0]);
    }

    /// Empty cells (space) emit a background quad (or cursor marker), never
    /// a glyph instance.
    #[test]
    fn empty_cell_emits_background_quad() {
        let cells = vec![cell_data(
            0,
            0,
            ' ',
            [0.9, 0.9, 0.9, 1.0],
            [0.1, 0.1, 0.1, 1.0],
            0,
        )];
        let instances = build(&cells, CellCursor::default(), None, &[]);
        assert_eq!(instances.len(), 1);
        assert_eq!(
            instances[0].atlas_size, [0.0; 2],
            "no glyph UVs for a space"
        );
        assert_eq!(instances[0].bg_color, [0.1, 0.1, 0.1, 1.0]);
    }

    /// SelectionRange.contains covers Char, Line and Block modes.
    #[test]
    fn selection_range_contains_modes() {
        let char_sel = SelectionRange {
            start_row: 1,
            start_col: 2,
            end_row: 2,
            end_col: 3,
            active: true,
            mode: SelectionMode::Char,
            ..Default::default()
        };
        assert!(char_sel.contains(1, 2, 80));
        assert!(char_sel.contains(1, 5, 80), "middle row covers all cols");
        assert!(char_sel.contains(2, 3, 80));
        assert!(!char_sel.contains(0, 0, 80));
        assert!(!char_sel.contains(2, 4, 80));

        let line_sel = SelectionRange {
            start_row: 0,
            start_col: 0,
            end_row: 3,
            end_col: 0,
            active: true,
            mode: SelectionMode::Line,
            ..Default::default()
        };
        assert!(
            line_sel.contains(2, 99, 80),
            "Line mode covers the whole row"
        );
        assert!(!line_sel.contains(4, 0, 80));

        let block_sel = SelectionRange {
            start_row: 1,
            start_col: 1,
            end_row: 2,
            end_col: 2,
            active: true,
            mode: SelectionMode::Block,
            ..Default::default()
        };
        assert!(block_sel.contains(1, 1, 80));
        assert!(block_sel.contains(2, 2, 80));
        assert!(
            !block_sel.contains(1, 3, 80),
            "Block mode bounds the column"
        );
    }

    /// Field-by-field comparison (CellInstance does not derive PartialEq).
    fn instances_equal(a: &[CellInstance], b: &[CellInstance]) -> bool {
        a.len() == b.len()
            && a.iter().zip(b).all(|(x, y)| {
                x.quad_origin == y.quad_origin
                    && x.atlas_offset == y.atlas_offset
                    && x.atlas_size == y.atlas_size
                    && x.fg_color == y.fg_color
                    && x.bg_color == y.bg_color
                    && x.quad_size == y.quad_size
                    && x.flags == y.flags
                    && x.bearing == y.bearing
                    && x.glyph_advance_width == y.glyph_advance_width
            })
    }

    /// Incremental path (build_instances_cached): a frame that only dirties
    /// row 2 must produce exactly the instances of a full rebuild, with the
    /// clean rows served from the cache (NFR-010: only repaint dirty rows).
    #[test]
    fn cached_incremental_rebuild_matches_full_build() {
        let mk = |row: u32, ch: char| {
            cell_data(row, 0, ch, [1.0, 1.0, 1.0, 1.0], [0.0, 0.0, 0.0, 1.0], 0)
        };
        let cells: Vec<CellData> = (0..24).map(|r| mk(r, 'a')).collect();
        let cursor = CellCursor {
            row: 0,
            col: 0,
            visible: false,
            style: CursorStyle::Block,
            color: None,
        };

        // Frame 1: full dirty pass seeds the cache.
        let mut font_pipeline = crate::render::font::FontPipeline::new(1024, 1024, 14.0);
        let mut instances = Vec::new();
        let mut cache = CachedInstances::new(24, 80);
        let all_dirty = vec![true; 24];
        let ok = build_instances_cached(
            &cells,
            CellInstanceConfig {
                rows: 24,
                cols: 80,
                grid_cell_w: 1024.0 / 80.0,
                grid_cell_h: 1024.0 / 24.0,
                cursor,
                atlas_width: 1024.0,
                atlas_height: 1024.0,
                selection: None,
                search_highlights: &[],
            },
            &mut font_pipeline,
            &all_dirty,
            &mut cache,
            &mut instances,
        );
        assert!(ok.is_some(), "initial full build should succeed");
        let full_frame1 = build(&cells, cursor, None, &[]);
        assert!(
            instances_equal(&instances, &full_frame1),
            "initial build equals a full build"
        );

        // Frame 2: only row 2 changes; all other rows must be served from
        // the cache and the result must still equal a full rebuild.
        let mut cells2 = cells.clone();
        cells2[2] = mk(2, 'z');
        let mut dirty = vec![false; 24];
        dirty[2] = true;
        instances.clear();
        let ok = build_instances_cached(
            &cells2,
            CellInstanceConfig {
                rows: 24,
                cols: 80,
                grid_cell_w: 1024.0 / 80.0,
                grid_cell_h: 1024.0 / 24.0,
                cursor,
                atlas_width: 1024.0,
                atlas_height: 1024.0,
                selection: None,
                search_highlights: &[],
            },
            &mut font_pipeline,
            &dirty,
            &mut cache,
            &mut instances,
        );
        assert!(ok.is_some(), "incremental build should succeed");
        let full_frame2 = build(&cells2, cursor, None, &[]);
        assert!(
            instances_equal(&instances, &full_frame2),
            "incremental result must match a full rebuild"
        );
        // The cache now holds frame 2's instances for the next frame.
        assert!(
            instances_equal(cache.instances(), &instances),
            "cache must be refreshed with the latest instances"
        );
    }

    /// Degenerate incremental inputs (stale cache grid size or a dirty mask
    /// shorter than the grid) must fall back to a full rebuild instead of
    /// serving stale rows.
    #[test]
    fn cached_degraded_input_falls_back_to_full_rebuild() {
        let cells: Vec<CellData> = (0..24)
            .map(|r| cell_data(r, 0, 'x', [1.0; 4], [0.0, 0.0, 0.0, 1.0], 0))
            .collect();
        let cursor = CellCursor {
            row: 0,
            col: 0,
            visible: false,
            style: CursorStyle::Block,
            color: None,
        };
        let mut font_pipeline = crate::render::font::FontPipeline::new(1024, 1024, 14.0);
        let full = build(&cells, cursor, None, &[]);

        // Stale cache: built for a 12-row grid while the grid has 24 rows.
        let mut cache = CachedInstances::new(12, 80);
        let mut instances = Vec::new();
        let ok = build_instances_cached(
            &cells,
            CellInstanceConfig {
                rows: 24,
                cols: 80,
                grid_cell_w: 1024.0 / 80.0,
                grid_cell_h: 1024.0 / 24.0,
                cursor,
                atlas_width: 1024.0,
                atlas_height: 1024.0,
                selection: None,
                search_highlights: &[],
            },
            &mut font_pipeline,
            &vec![true; 24],
            &mut cache,
            &mut instances,
        );
        assert!(ok.is_some());
        assert!(
            instances_equal(&instances, &full),
            "stale cache must force a full rebuild"
        );

        // Dirty mask shorter than the grid (only 10 rows).
        let mut cache2 = CachedInstances::new(24, 80);
        instances.clear();
        let ok = build_instances_cached(
            &cells,
            CellInstanceConfig {
                rows: 24,
                cols: 80,
                grid_cell_w: 1024.0 / 80.0,
                grid_cell_h: 1024.0 / 24.0,
                cursor,
                atlas_width: 1024.0,
                atlas_height: 1024.0,
                selection: None,
                search_highlights: &[],
            },
            &mut font_pipeline,
            &[true; 10],
            &mut cache2,
            &mut instances,
        );
        assert!(ok.is_some());
        assert!(
            instances_equal(&instances, &full),
            "a too-short dirty mask must force a full rebuild"
        );
    }

    /// regression: a cache that no longer matches the grid
    /// (e.g. a cols-only resize keeps rows identical) combined with a
    /// PARTIAL dirty mask must still produce a full rebuild. A freshly
    /// re-created empty cache looks "compatible" (same rows/cols fields),
    /// so serving "clean" rows from it would copy 0 instances and drop
    /// those rows from the frame. The caller (pass.rs) is responsible for
    /// substituting an all-true mask when it rebuilds the cache; this test
    /// pins the degenerate combination so it can never silently pass.
    #[test]
    fn stale_cache_with_partial_mask_must_not_drop_rows() {
        let cells: Vec<CellData> = (0..24)
            .map(|r| cell_data(r, 0, 'x', [1.0; 4], [0.0, 0.0, 0.0, 1.0], 0))
            .collect();
        let cursor = CellCursor {
            row: 0,
            col: 0,
            visible: false,
            style: CursorStyle::Block,
            color: None,
        };
        let mut font_pipeline = crate::render::font::FontPipeline::new(1024, 1024, 14.0);
        let full = build(&cells, cursor, None, &[]);

        // Simulate the resize frame: cache was built for 24x40 but the grid
        // is now 24x80 (cols-only change). A re-created empty cache for
        // 24x80 is "compatible" by dimensions, yet holds no row data.
        let mut cache = CachedInstances::new(24, 80);
        let mut instances = Vec::new();
        // Partial mask: only row 2 is dirty (as produced by the cell diff
        // path when only one row's bytes changed).
        let mut mask = vec![false; 24];
        mask[2] = true;
        let ok = build_instances_cached(
            &cells,
            CellInstanceConfig {
                rows: 24,
                cols: 80,
                grid_cell_w: 1024.0 / 80.0,
                grid_cell_h: 1024.0 / 24.0,
                cursor,
                atlas_width: 1024.0,
                atlas_height: 1024.0,
                selection: None,
                search_highlights: &[],
            },
            &mut font_pipeline,
            &mask,
            &mut cache,
            &mut instances,
        );
        assert!(ok.is_some());
        // Every row must be present: an empty cache serving "clean" rows
        // would produce 0 instances for the 23 clean rows.
        assert_eq!(
            instances.len(),
            full.len(),
            "partial mask on an empty cache must not drop rows (got {} vs {})",
            instances.len(),
            full.len()
        );
        assert!(
            instances_equal(&instances, &full),
            "result must equal a full rebuild on cache mismatch"
        );
    }
}
