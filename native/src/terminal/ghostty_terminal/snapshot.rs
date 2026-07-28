use super::types::*;
use libghostty_vt::Terminal;
use libghostty_vt::render::{CellIterator, CursorVisualStyle, RenderState, RowIterator};
use libghostty_vt::style::{PaletteIndex, StyleColor};
use libghostty_vt::terminal::{Mode, Point, PointCoordinate};

/// Helper to create the three per-frame render iterators.
/// Returns `None` and logs on any creation failure.
fn create_render_iterators() -> Option<(
    RenderState<'static>,
    RowIterator<'static>,
    CellIterator<'static>,
)> {
    let render_state = match RenderState::new() {
        Ok(rs) => rs,
        Err(e) => {
            log::error!("create_render_iterators: RenderState::new() failed: {e}");
            return None;
        }
    };
    let row_iter = match RowIterator::new() {
        Ok(ri) => ri,
        Err(e) => {
            log::error!("create_render_iterators: RowIterator::new() failed: {e}");
            return None;
        }
    };
    let cell_iter = match CellIterator::new() {
        Ok(ci) => ci,
        Err(e) => {
            log::error!("create_render_iterators: CellIterator::new() failed: {e}");
            return None;
        }
    };
    Some((render_state, row_iter, cell_iter))
}

/// Decide whether the VT thread must rebuild the grid snapshot from the
/// terminal, as opposed to cloning the previously built (cached) snapshot.
///
/// Rebuild only when the grid content changed (`grid_dirty`, set by
/// `Command::Write` / `Resize` / `SetTheme`), the scroll offset changed, or
/// there is no cached snapshot yet. When none of these hold the grid content
/// is byte-for-byte identical to the cached snapshot, so reusing it cannot
/// yield a stale frame while skipping ~1920 per-cell ghostty FFI calls.
pub(crate) fn snapshot_needs_rebuild(
    grid_dirty: bool,
    scroll_offset: u32,
    cached_scroll_offset: u32,
    has_cache: bool,
) -> bool {
    grid_dirty || scroll_offset != cached_scroll_offset || !has_cache
}

impl super::GhosttyTerminal {
    pub(crate) fn apply_style_to_snapshot(
        data: &mut CellSnapshot,
        style: &libghostty_vt::style::Style,
        default_fg: [f32; 4],
        default_bg: [f32; 4],
        palette: &[[u8; 3]; 16],
    ) {
        match style.fg_color {
            StyleColor::Rgb(c) => {
                data.foreground = Self::byte_color_to_float([c.r, c.g, c.b]);
            }
            StyleColor::Palette(idx) => {
                data.foreground = Self::palette_index_to_float(idx, palette);
            }
            _ => {
                data.foreground = default_fg;
            }
        }
        match style.bg_color {
            StyleColor::Rgb(c) => {
                data.background = Self::byte_color_to_float([c.r, c.g, c.b]);
            }
            StyleColor::Palette(idx) => {
                data.background = Self::palette_index_to_float(idx, palette);
            }
            _ => {
                data.background = default_bg;
            }
        }
        data.bold = style.bold;
        data.dim = style.faint;
        data.italic = style.italic;
        data.strikethrough = style.strikethrough;
        data.overline = style.overline;
        data.blink = style.blink;
        data.hidden = style.invisible;
        data.underline = matches!(
            style.underline,
            libghostty_vt::style::Underline::Single
                | libghostty_vt::style::Underline::Double
                | libghostty_vt::style::Underline::Curly
                | libghostty_vt::style::Underline::Dashed
                | libghostty_vt::style::Underline::Dotted
        );
        data.double_underline = style.underline == libghostty_vt::style::Underline::Double;
        data.reverse = style.inverse;
    }

    pub(crate) fn build_dumped_grid(terminal: &Terminal) -> DumpedGrid {
        let rows = terminal.rows().unwrap_or(24) as u32;
        let cols = terminal.cols().unwrap_or(80) as u32;
        let scrollback_rows = terminal.scrollback_rows().unwrap_or(0) as u32;
        let palette = Self::catppuccin_mocha_palette().0;

        let mut visible = Vec::with_capacity((rows * cols) as usize);
        for row in 0..rows {
            for col in 0..cols {
                let coord = PointCoordinate {
                    x: col as u16,
                    y: row,
                };
                let mut data = CellSnapshot::default();
                if let Ok(point) = terminal.grid_ref(Point::Viewport(coord)) {
                    if let Ok(cell) = point.cell() {
                        data.codepoint = cell.codepoint().unwrap_or(0);
                    }
                    if let Ok(style) = point.style() {
                        Self::apply_style_to_snapshot(
                            &mut data, &style, [0.0; 4], [0.0; 4], &palette,
                        );
                    }
                }
                visible.push(data);
            }
        }

        let mut scrollback = Vec::with_capacity(scrollback_rows as usize);
        for i in 0..scrollback_rows {
            let mut row_cells = Vec::with_capacity(cols as usize);
            for col in 0..cols {
                let coord = PointCoordinate {
                    x: col as u16,
                    y: i,
                };
                let mut data = CellSnapshot::default();
                if let Ok(point) = terminal.grid_ref(Point::History(coord)) {
                    if let Ok(cell) = point.cell() {
                        data.codepoint = cell.codepoint().unwrap_or(0);
                    }
                    if let Ok(style) = point.style() {
                        Self::apply_style_to_snapshot(
                            &mut data, &style, [0.0; 4], [0.0; 4], &palette,
                        );
                    }
                }
                row_cells.push(data);
            }
            scrollback.push(row_cells);
        }

        DumpedGrid {
            rows,
            cols,
            visible,
            scrollback,
        }
    }

    pub(crate) fn byte_to_float(value: u8) -> f32 {
        value as f32 / 255.0
    }

    pub(crate) fn byte_color_to_float(color: [u8; 3]) -> [f32; 4] {
        [
            Self::byte_to_float(color[0]),
            Self::byte_to_float(color[1]),
            Self::byte_to_float(color[2]),
            1.0,
        ]
    }

    pub(crate) fn palette_index_to_float(idx: PaletteIndex, palette: &[[u8; 3]; 16]) -> [f32; 4] {
        let index = idx.0 as usize;
        if index < 16 {
            let [red, green, blue] = palette[index];
            Self::byte_color_to_float([red, green, blue])
        } else {
            // Extended 256-color palette (indices 16-231: 6x6x6 cube, 232-255: grayscale)
            let (red, green, blue) = if index < 232 {
                let offset = index - 16;
                let red_index = offset / 36;
                let green_index = (offset % 36) / 6;
                let blue_index = offset % 6;
                let expand = |value: u8| -> u8 { if value == 0 { 0 } else { value * 40 + 55 } };
                (
                    expand(red_index as u8),
                    expand(green_index as u8),
                    expand(blue_index as u8),
                )
            } else {
                let gray = (index - 232) * 10 + 8;
                (gray as u8, gray as u8, gray as u8)
            };
            Self::byte_color_to_float([red, green, blue])
        }
    }

    pub(crate) fn build_cell_data(
        terminal: &Terminal,
        default_fg: [f32; 4],
        default_bg: [f32; 4],
    ) -> Option<(Vec<CellData>, CursorInfo)> {
        let rows = terminal.rows().unwrap_or(24) as u32;
        let cols = terminal.cols().unwrap_or(80) as u32;
        let size = (rows * cols) as usize;

        let (mut render_state, mut row_iter, mut cell_iter) = create_render_iterators()?;

        let snapshot = match render_state.update(terminal) {
            Ok(s) => s,
            Err(e) => {
                log::error!("build_cell_data: render_state.update failed: {e}");
                return None;
            }
        };

        let mut row_iter_impl = match row_iter.update(&snapshot) {
            Ok(ri) => ri,
            Err(e) => {
                log::error!("build_cell_data: row_iter.update failed: {e}");
                return None;
            }
        };

        let mut data = Vec::with_capacity(size);
        let mut current_row = 0u32;

        while let Some(row) = row_iter_impl.next() {
            let mut cell_iter_impl = match cell_iter.update(row) {
                Ok(ci) => ci,
                Err(_) => break,
            };

            let mut current_col = 0u32;

            while let Some(cell) = cell_iter_impl.next() {
                let raw = match cell.raw_cell() {
                    Ok(c) => c,
                    Err(_) => {
                        data.push(CellData {
                            codepoint: 0,
                            width: 1,
                            grapheme_extra: [0; 7],
                            fg_color: default_fg,
                            bg_color: default_bg,
                            flags: 0,
                            row: current_row,
                            col: current_col,
                        });
                        current_col += 1;
                        continue;
                    }
                };

                let style = match cell.style() {
                    Ok(s) => s,
                    Err(_) => {
                        data.push(CellData {
                            codepoint: 0,
                            width: 1,
                            grapheme_extra: [0; 7],
                            fg_color: default_fg,
                            bg_color: default_bg,
                            flags: 0,
                            row: current_row,
                            col: current_col,
                        });
                        current_col += 1;
                        continue;
                    }
                };

                let codepoint = raw.codepoint().unwrap_or(0);
                let width = match raw.wide() {
                    Ok(libghostty_vt::screen::CellWide::Wide) => 2,
                    _ => 1,
                };

                let mut grapheme_extra = [0u32; 7];
                if let Ok(g) = cell.graphemes() {
                    for (i, &c) in g.iter().enumerate().skip(1).take(7) {
                        grapheme_extra[i - 1] = c as u32;
                    }
                }

                let fg_color = match cell.fg_color() {
                    Ok(Some(rgb)) => [
                        rgb.r as f32 / 255.0,
                        rgb.g as f32 / 255.0,
                        rgb.b as f32 / 255.0,
                        1.0,
                    ],
                    _ => default_fg,
                };
                let bg_color = match cell.bg_color() {
                    Ok(Some(rgb)) => [
                        rgb.r as f32 / 255.0,
                        rgb.g as f32 / 255.0,
                        rgb.b as f32 / 255.0,
                        1.0,
                    ],
                    _ => default_bg,
                };

                let flags = Self::pack_style_flags(&style);

                data.push(CellData {
                    codepoint,
                    width,
                    grapheme_extra,
                    fg_color,
                    bg_color,
                    flags,
                    row: current_row,
                    col: current_col,
                });
                current_col += width;
            }
            current_row += 1;
        }
        let cursor_style = snapshot
            .cursor_visual_style()
            .ok()
            .map(|cvs| match cvs {
                CursorVisualStyle::Bar => CursorStyle::Bar,
                CursorVisualStyle::Block | CursorVisualStyle::BlockHollow => CursorStyle::Block,
                CursorVisualStyle::Underline => CursorStyle::Underline,
                _ => CursorStyle::default(),
            })
            .unwrap_or_default();
        Some((
            data,
            CursorInfo {
                row: terminal.cursor_y().unwrap_or(0) as u32,
                col: terminal.cursor_x().unwrap_or(0) as u32,
                visible: terminal.is_cursor_visible().unwrap_or(true),
                style: cursor_style,
            },
        ))
    }

    /// Pack style attributes into a bitmask.
    /// Bit 0=bold, 1=dim, 2=italic, 3=underline, 4=reverse,
    /// 5=strikethrough, 6=blink, 7=hidden, 8=overline, 9=double_underline
    fn pack_style_flags(style: &libghostty_vt::style::Style) -> u32 {
        let mut flags = 0u32;
        if style.bold {
            flags |= 1 << 0;
        }
        if style.faint {
            flags |= 1 << 1;
        }
        if style.italic {
            flags |= 1 << 2;
        }
        if matches!(
            style.underline,
            libghostty_vt::style::Underline::Single
                | libghostty_vt::style::Underline::Double
                | libghostty_vt::style::Underline::Curly
                | libghostty_vt::style::Underline::Dashed
                | libghostty_vt::style::Underline::Dotted
        ) {
            flags |= 1 << 3;
        }
        if style.inverse {
            flags |= 1 << 4;
        }
        if style.strikethrough {
            flags |= 1 << 5;
        }
        if style.blink {
            flags |= 1 << 6;
        }
        if style.invisible {
            flags |= 1 << 7;
        }
        if style.overline {
            flags |= 1 << 8;
        }
        if style.underline == libghostty_vt::style::Underline::Double {
            flags |= 1 << 9;
        }
        flags
    }

    pub(crate) fn build_snapshot(
        terminal: &Terminal,
        default_fg: [f32; 4],
        default_bg: [f32; 4],
        _palette: &[[u8; 3]; 16],
        scroll_offset: u32,
    ) -> GridSnapshot {
        // Fallback path: when scrolled into history, use legacy per-cell
        // grid_ref() approach. (RenderState doesn't expose scrollback.)
        if scroll_offset > 0 {
            return GridSnapshot::fallback(
                terminal.rows().unwrap_or(24) as u32,
                terminal.cols().unwrap_or(80) as u32,
            );
        }
        let rows = terminal.rows().unwrap_or(24) as u32;
        let cols = terminal.cols().unwrap_or(80) as u32;
        let size = (rows * cols) as usize;
        let mut cells = Vec::with_capacity(size);

        // Local RenderState+iterators — created per-call to avoid lifetime
        // issues with the invariant-param Terminal type.
        let (mut render_state, mut row_iter, mut cell_iter) = match create_render_iterators() {
            Some(v) => v,
            None => return GridSnapshot::fallback(rows, cols),
        };

        let snapshot = match render_state.update(terminal) {
            Ok(s) => s,
            Err(e) => {
                log::error!("build_snapshot: render_state.update failed: {e}");
                return GridSnapshot::fallback(rows, cols);
            }
        };

        let mut row_iter_impl = match row_iter.update(&snapshot) {
            Ok(ri) => ri,
            Err(e) => {
                log::error!("build_snapshot: row_iter.update failed: {e}");
                return GridSnapshot::fallback(rows, cols);
            }
        };

        // ── CellIterator loop ──
        // Iterate over all visible rows via RowIterator, then all cells
        // per row via CellIterator. This replaces per-cell grid_ref.
        while let Some(row) = row_iter_impl.next() {
            let mut cell_iter_impl = match cell_iter.update(row) {
                Ok(ci) => ci,
                Err(_) => break,
            };

            while let Some(cell) = cell_iter_impl.next() {
                let raw = match cell.raw_cell() {
                    Ok(c) => c,
                    Err(_) => {
                        cells.push(CellSnapshot {
                            foreground: default_fg,
                            background: default_bg,
                            ..CellSnapshot::default()
                        });
                        continue;
                    }
                };

                let style = match cell.style() {
                    Ok(s) => s,
                    Err(_) => {
                        cells.push(CellSnapshot {
                            foreground: default_fg,
                            background: default_bg,
                            ..CellSnapshot::default()
                        });
                        continue;
                    }
                };

                let codepoint = raw.codepoint().unwrap_or(0);
                let width = match raw.wide() {
                    Ok(libghostty_vt::screen::CellWide::Wide) => 2,
                    _ => 1,
                };

                let graphemes: Vec<u32> = match cell.graphemes() {
                    Ok(g) if g.len() <= MAX_GRAPHEME_CLUSTERS => {
                        g.iter().map(|&c| c as u32).collect()
                    }
                    Ok(g) => g
                        .iter()
                        .take(MAX_GRAPHEME_CLUSTERS)
                        .map(|&c| c as u32)
                        .collect(),
                    Err(_) => vec![codepoint],
                };

                let foreground = match cell.fg_color() {
                    Ok(Some(rgb)) => [
                        rgb.r as f32 / 255.0,
                        rgb.g as f32 / 255.0,
                        rgb.b as f32 / 255.0,
                        1.0,
                    ],
                    _ => default_fg,
                };
                let background = match cell.bg_color() {
                    Ok(Some(rgb)) => [
                        rgb.r as f32 / 255.0,
                        rgb.g as f32 / 255.0,
                        rgb.b as f32 / 255.0,
                        1.0,
                    ],
                    _ => default_bg,
                };

                let semantic = match raw.semantic_content() {
                    Ok(libghostty_vt::screen::CellSemanticContent::Input) => SemanticContent::Input,
                    Ok(libghostty_vt::screen::CellSemanticContent::Prompt) => {
                        SemanticContent::Prompt
                    }
                    _ => SemanticContent::Output,
                };

                cells.push(CellSnapshot {
                    codepoint,
                    graphemes,
                    foreground,
                    background,
                    bold: style.bold,
                    dim: style.faint,
                    italic: style.italic,
                    underline: matches!(
                        style.underline,
                        libghostty_vt::style::Underline::Single
                            | libghostty_vt::style::Underline::Double
                            | libghostty_vt::style::Underline::Curly
                            | libghostty_vt::style::Underline::Dashed
                            | libghostty_vt::style::Underline::Dotted
                    ),
                    reverse: style.inverse,
                    strikethrough: style.strikethrough,
                    blink: style.blink,
                    hidden: style.invisible,
                    uri: None,
                    semantic,
                    overline: style.overline,
                    double_underline: style.underline == libghostty_vt::style::Underline::Double,
                    width,
                });
            }
        }

        let cursor_visible = terminal.is_cursor_visible().unwrap_or(true);
        let cursor_row = terminal.cursor_y().unwrap_or(0) as u32;
        let cursor_col = terminal.cursor_x().unwrap_or(0) as u32;

        let dirty = vec![true; rows as usize];

        let sync_active = terminal.mode(Mode::SYNC_OUTPUT).unwrap_or(false);

        GridSnapshot {
            rows,
            cols,
            cursor_row,
            cursor_col,
            cursor_visible,
            cursor_style: snapshot
                .cursor_visual_style()
                .ok()
                .map(|cvs| match cvs {
                    CursorVisualStyle::Bar => CursorStyle::Bar,
                    CursorVisualStyle::Block | CursorVisualStyle::BlockHollow => CursorStyle::Block,
                    CursorVisualStyle::Underline => CursorStyle::Underline,
                    _ => CursorStyle::default(),
                })
                .unwrap_or_default(),
            cells,
            dirty,

            title: terminal.title().unwrap_or_default().to_string(),
            scrollback_length: terminal.scrollback_rows().unwrap_or(0) as u32,
            sync_active,
        }
    }
}
