use std::sync::Arc;
use std::sync::atomic::Ordering;

use libghostty_vt::key::{self, Mods};
use libghostty_vt::mouse;
use libghostty_vt::render::{CellIterator, CursorVisualStyle, RenderState, RowIterator};
use libghostty_vt::style::{PaletteIndex, StyleColor};
use libghostty_vt::terminal::{Mode, ModeKind, Point, PointCoordinate};
use libghostty_vt::{Terminal, TerminalOptions};

use super::commands::{Command, RunConfig};
use super::keymap::map_android_key_code;
use super::types::*;
use flume::Sender;

/// Send a value over a channel, logging on failure.
fn try_send<T>(sender: &Sender<T>, value: T, context: &str) {
    if let Err(e) = sender.send(value) {
        log::error!("ghostty_terminal: {context}: channel send failed: {e}");
    }
}

// ── Free functions ──────────────────────────────────────────────

/// The 16 standard ANSI palette indices, in xterm order (normal colors
/// followed by bright variants). `libghostty_vt::Palette` only exposes named
/// `PaletteIndex` constants, so map theme colors onto them explicitly.
const ANSI_PALETTE_INDICES: [PaletteIndex; 16] = [
    PaletteIndex::BLACK,
    PaletteIndex::RED,
    PaletteIndex::GREEN,
    PaletteIndex::YELLOW,
    PaletteIndex::BLUE,
    PaletteIndex::MAGENTA,
    PaletteIndex::CYAN,
    PaletteIndex::WHITE,
    PaletteIndex::BRIGHT_BLACK,
    PaletteIndex::BRIGHT_RED,
    PaletteIndex::BRIGHT_GREEN,
    PaletteIndex::BRIGHT_YELLOW,
    PaletteIndex::BRIGHT_BLUE,
    PaletteIndex::BRIGHT_MAGENTA,
    PaletteIndex::BRIGHT_CYAN,
    PaletteIndex::BRIGHT_WHITE,
];

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

// ── impl GhosttyTerminal ──────────────────────────────────────
impl super::GhosttyTerminal {
    pub(crate) fn process_query(query: Command, terminal: &mut Terminal) {
        match query {
            Command::Rows(tx) => {
                if let Err(error) =
                    tx.send(terminal.rows().unwrap_or(DISCONNECTED_ROWS as u16) as u32)
                {
                    log::error!("ghostty_terminal: query channel send failed: {error}");
                }
            }
            Command::Cols(tx) => {
                if let Err(error) =
                    tx.send(terminal.cols().unwrap_or(DISCONNECTED_COLS as u16) as u32)
                {
                    log::error!("ghostty_terminal: query channel send failed: {error}");
                }
            }
            Command::CursorX(tx) => {
                if let Err(error) =
                    tx.send(terminal.cursor_x().unwrap_or(DISCONNECTED_CURSOR_X as u16) as u32)
                {
                    log::error!("ghostty_terminal: query channel send failed: {error}");
                }
            }
            Command::CursorY(tx) => {
                if let Err(error) =
                    tx.send(terminal.cursor_y().unwrap_or(DISCONNECTED_CURSOR_Y as u16) as u32)
                {
                    log::error!("ghostty_terminal: query channel send failed: {error}");
                }
            }
            Command::CursorVisible(tx) => {
                try_send(
                    &tx,
                    terminal.is_cursor_visible().unwrap_or(true),
                    "query channel send failed",
                );
            }
            Command::OriginMode(tx) => {
                if let Err(error) =
                    tx.send(terminal.mode(Mode::new(6, ModeKind::Dec)).unwrap_or(false))
                {
                    log::error!("ghostty_terminal: query channel send failed: {error}");
                }
            }
            Command::Autowrap(tx) => {
                if let Err(error) =
                    tx.send(terminal.mode(Mode::new(7, ModeKind::Dec)).unwrap_or(false))
                {
                    log::error!("ghostty_terminal: query channel send failed: {error}");
                }
            }
            Command::AltScreen(tx) => {
                let is_alt = terminal
                    .active_screen()
                    .is_ok_and(|s| s == libghostty_vt::screen::Screen::Alternate);
                try_send(&tx, is_alt, "ghostty_terminal: query channel send failed");
            }
            Command::Title(tx) => {
                try_send(
                    &tx,
                    terminal.title().unwrap_or("").to_string(),
                    "query channel send failed",
                );
            }
            Command::Cwd(tx) => {
                if let Err(error) =
                    tx.send(terminal.pwd().map(|p| p.to_string()).unwrap_or_default())
                {
                    log::error!("ghostty_terminal: query channel send failed: {error}");
                }
            }
            Command::ModeGet(num, kind, tx) => {
                let mode_kind = match kind {
                    0 => ModeKind::Dec,
                    _ => ModeKind::Ansi,
                };
                if let Err(error) =
                    tx.send(terminal.mode(Mode::new(num, mode_kind)).unwrap_or(false))
                {
                    log::error!("ghostty_terminal: query channel send failed: {error}");
                }
            }
            Command::ScrollbackLength(tx) => {
                let len = terminal.scrollback_rows().unwrap_or(0) as u32;
                log::debug!("ghostty_terminal: scrollback_rows query returned {len}");
                try_send(&tx, len, "ghostty_terminal: query channel send failed");
            }
            Command::ReadLineText { row, tx } => {
                try_send(
                    &tx,
                    Self::read_line_text_impl(terminal, row),
                    "query channel send failed",
                );
            }
            Command::ReadVisibleText(tx) => {
                let rows = terminal.rows().unwrap_or(24) as u32;
                let scrollback_rows = terminal.scrollback_rows().unwrap_or(0) as u32;
                let mut text = String::new();
                for row in 0..rows {
                    // read_line_text_impl expects an absolute row (history + viewport).
                    if let Some(line) = Self::read_line_text_impl(terminal, scrollback_rows + row) {
                        text.push_str(&line);
                        text.push('\n');
                    }
                }
                try_send(&tx, text, "ghostty_terminal: query channel send failed");
            }
            Command::SelectionText {
                start,
                end,
                rectangle,
                tx,
            } => {
                // Ghostty-native wrap-aware selection extraction (termux
                // TerminalBuffer.getSelectedText semantics): unwrap joins
                // soft-wrapped lines without '\n', trim drops trailing
                // whitespace, and the formatter maps grid columns to char
                // indices internally so CJK wide glyphs are never split.
                let text = Self::selection_text_impl(terminal, start, end, rectangle);
                try_send(&tx, text, "selection text response send failed");
            }
            Command::HyperlinkAt { row, col, tx } => {
                let url = Self::hyperlink_at_impl(terminal, row, col);
                try_send(&tx, url, "hyperlink_at response send failed");
            }
            _ => {}
        }
    }

    pub(crate) fn run(config: RunConfig) {
        // Wrap the entire body in `catch_unwind` so that any unexpected FFI
        // panic (e.g. from Ghostty's C code) is logged instead of silently
        // killing the thread. `AssertUnwindSafe` is safe here because:
        // - Terminal is `!UnwindSafe` due to internal C pointers, but its
        //   Drop implementation will call `ghostty_terminal_free` on unwind.
        // - We always exit the thread after a panic, so no double-use occurs.
        let result =
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| Self::run_inner(config)));
        if let Err(panic) = result {
            log::error!("ghostty_terminal: VT thread panicked: {panic:?}");
        }
    }

    /// The inner run loop (separated so `catch_unwind` can call it).
    fn run_inner(config: RunConfig) {
        let Ok(mut terminal) = Terminal::new(TerminalOptions {
            cols: config.cols as u16,
            rows: config.rows as u16,
            max_scrollback: config.scrollback_lines as usize,
        }) else {
            log::error!("ghostty_terminal: Terminal::new failed — thread exiting");
            return;
        };
        // The C `ghostty_terminal_new` ABI consumes `Options.max_scrollback`
        // directly (ghostty a887df42, libghostty-vt 0.2.1): `Screen.init`
        // sets `no_scrollback = max_scrollback == 0`, so a non-zero value
        // enables scrollback (round-205: scrollback_rows query returned 0
        // when scrollback was disabled).

        // Initialize Kitty Graphics Protocol (KGP) support
        if let Err(error) = terminal.set_kitty_image_storage_limit(KGP_STORAGE_LIMIT) {
            log::error!("ghostty_terminal: set_kitty_image_storage_limit failed: {error}");
        }
        // PNG decoder is disabled because the upstream RustPngDecoder API has not
        // stabilized across libghostty-vt versions. KGP image storage still accepts
        // pre-decoded raw RGBA data from external PNG decoders.

        // Register PTY write-back callback for terminal responses
        // (DECRPM mode reports, DSR, DA, etc.)
        if let Err(error) = terminal.on_pty_write({
            let response_buffer = config.response_buffer.clone();
            move |_term, data| {
                if let Ok(mut guard) = response_buffer.lock() {
                    guard.push(data.to_vec());
                }
            }
        }) {
            log::error!("ghostty_terminal: on_pty_write callback registration failed: {error}");
        }

        let mut default_bg = Self::byte_color_to_float(config.background_color);
        let mut default_fg = Self::byte_color_to_float(config.foreground_color);

        // Reused per-keystroke encoder/event. Allocating these once per
        // terminal (instead of per keystroke) matches the reference
        // implementation and avoids losing per-encoder state between keys.
        // `set_options_from_terminal` still re-syncs encoder modes each key.
        let mut encoder = match key::Encoder::new() {
            Ok(enc) => Some(enc),
            Err(error) => {
                log::warn!(
                    "ghostty_terminal: key::Encoder::new() failed: {error} — keyboard protocol disabled"
                );
                None
            }
        };
        let mut event = match key::Event::new() {
            Ok(evt) => Some(evt),
            Err(error) => {
                log::warn!(
                    "ghostty_terminal: key::Event::new() failed: {error} — keyboard protocol disabled"
                );
                None
            }
        };

        // Reused per-mouse-event encoder/event, same lifetime pattern as the
        // key encoder above. `set_options_from_terminal` re-syncs tracking
        // mode and output format before each event (zelland pattern).
        let mut mouse_encoder = match mouse::Encoder::new() {
            Ok(enc) => Some(enc),
            Err(error) => {
                log::warn!(
                    "ghostty_terminal: mouse::Encoder::new() failed: {error} — mouse protocol disabled"
                );
                None
            }
        };
        let mut mouse_event = match mouse::Event::new() {
            Ok(evt) => Some(evt),
            Err(error) => {
                log::warn!(
                    "ghostty_terminal: mouse::Event::new() failed: {error} — mouse protocol disabled"
                );
                None
            }
        };

        // Initial theme: libghostty-vt is a pure VT layer and does not
        // process OSC 10/11/4 color escapes (the embedder owns them), so use
        // the native setters. Without this the terminal keeps the built-in
        // xterm palette and the theme colors never reach the grid.
        let _ = terminal.set_default_bg_color(Some(libghostty_vt::style::RgbColor {
            r: config.background_color[0],
            g: config.background_color[1],
            b: config.background_color[2],
        }));
        let _ = terminal.set_default_fg_color(Some(libghostty_vt::style::RgbColor {
            r: config.foreground_color[0],
            g: config.foreground_color[1],
            b: config.foreground_color[2],
        }));
        if let Ok(mut palette) = terminal.default_color_palette() {
            for (index, color) in ANSI_PALETTE_INDICES.iter().zip(config.ansi_colors.iter()) {
                palette.set(
                    *index,
                    libghostty_vt::style::RgbColor {
                        r: color[0],
                        g: color[1],
                        b: color[2],
                    },
                );
            }
            let _ = terminal.set_default_color_palette(Some(palette));
        }

        let query_receiver = config.query_receiver;

        // Cache the last built grid snapshot so we skip the expensive
        // per-cell ghostty FFI rebuild when neither the grid content nor the
        // scroll offset changed since the previous frame. The VT thread is
        // single-threaded and processes commands sequentially, so there is no
        // race between marking `grid_dirty` and rebuilding.
        let mut cached_snapshot: Option<Arc<GridSnapshot>> = None;
        let mut cached_scroll_offset: u32 = u32::MAX;
        // ── Auto-push CellData ──
        // Use a separate dirty flag to avoid coupling with the
        // legacy GridSnapshot grid_dirty tracker. Both flags are
        // set together on Write/Resize/SetTheme, but cleared
        // grid_dirty after TakeSnapshot.
        let mut grid_dirty = true;
        // zelland row-level dirty cache: rows that did not change are copied
        // from this cache instead of re-walking cells (build_cell_data).
        // Invalidated on resize below (row count changes).
        let mut row_cache: Vec<Vec<CellData>> = Vec::new();

        loop {
            // Wait for the next command from the bounded channel. Use a
            // timeout so we periodically check the query channel even when
            // no commands are pending (e.g., queries sent between writes).
            let command = match config
                .command_receiver
                .recv_timeout(std::time::Duration::from_millis(50))
            {
                Ok(command) => command,
                Err(flume::RecvTimeoutError::Timeout) => {
                    // No bounded commands pending — drain query channel so
                    // queries sent between commands don't wait indefinitely.
                    while let Ok(query) = query_receiver.try_recv() {
                        Self::process_query(query, &mut terminal);
                    }
                    // ── Auto-push CellData (also sent on each state change above) ──
                    #[allow(clippy::collapsible_if)]
                    if let Some(tx) = config.cell_data_tx.as_ref()
                        && let Some(data) =
                            Self::build_cell_data(&terminal, default_fg, default_bg, &mut row_cache)
                    {
                        let _ = tx.try_send(data);
                    }
                    continue;
                }
                Err(flume::RecvTimeoutError::Disconnected) => break,
            };
            // Process the bounded command first so state mutations (resize,
            // theme change, font change) take effect before queries check the
            // updated terminal state.
            match command {
                Command::Write(data) => {
                    terminal.vt_write(&data);
                    grid_dirty = true;
                    #[allow(clippy::collapsible_if)]
                    if let Some(tx) = config.cell_data_tx.as_ref()
                        && let Some(data) =
                            Self::build_cell_data(&terminal, default_fg, default_bg, &mut row_cache)
                    {
                        let _ = tx.try_send(data);
                    }
                }
                Command::FlushAck(tx) => {
                    try_send(&tx, (), "command channel send failed");
                }
                Command::SetTheme {
                    background,
                    foreground,
                    ansi,
                } => {
                    default_bg = Self::byte_color_to_float(background);
                    default_fg = Self::byte_color_to_float(foreground);
                    log::debug!(
                        "SetTheme: bg={:?} fg={:?} -> default_bg={:?} default_fg={:?}",
                        background,
                        foreground,
                        default_bg,
                        default_fg
                    );
                    // Use the native theme API instead of hand-written
                    // OSC 10/11/4 sequences: libghostty-vt is a pure VT
                    // layer and does not process OSC color escapes (the
                    // embedder owns them), so the OSC approach silently
                    // kept the built-in xterm palette. These setters store
                    // the default colors that `cell.fg_color()` /
                    // `cell.bg_color()` resolve against.
                    let _ = terminal.set_default_bg_color(Some(libghostty_vt::style::RgbColor {
                        r: background[0],
                        g: background[1],
                        b: background[2],
                    }));
                    let _ = terminal.set_default_fg_color(Some(libghostty_vt::style::RgbColor {
                        r: foreground[0],
                        g: foreground[1],
                        b: foreground[2],
                    }));
                    if let Ok(mut palette) = terminal.default_color_palette() {
                        for (index, color) in ANSI_PALETTE_INDICES.iter().zip(ansi.iter()) {
                            palette.set(
                                *index,
                                libghostty_vt::style::RgbColor {
                                    r: color[0],
                                    g: color[1],
                                    b: color[2],
                                },
                            );
                        }
                        let _ = terminal.set_default_color_palette(Some(palette));
                    }
                    grid_dirty = true;
                    #[allow(clippy::collapsible_if)]
                    if let Some(tx) = config.cell_data_tx.as_ref()
                        && let Some(data) =
                            Self::build_cell_data(&terminal, default_fg, default_bg, &mut row_cache)
                    {
                        let _ = tx.try_send(data);
                    }
                }
                Command::Resize { rows, cols } => {
                    // Ghostty's C API takes u16 dimensions; reject out-of-
                    // range values instead of silently truncating (a
                    // hostile Kotlin caller could pass >65535 and wrap).
                    let (Ok(cols), Ok(rows)) = (u16::try_from(cols), u16::try_from(rows)) else {
                        log::error!(
                            "ghostty_terminal: resize rejected — dimensions out of u16 range"
                        );
                        continue;
                    };
                    if let Err(error) =
                        terminal.resize(cols, rows, DEFAULT_CELL_WIDTH, DEFAULT_CELL_HEIGHT)
                    {
                        log::error!("ghostty_terminal: resize failed: {error}");
                    }
                    grid_dirty = true;
                    // zelland row-cache pattern: row count changed on resize,
                    // the row cache is stale and must be invalidated.
                    row_cache.clear();
                    // zelland row-cache pattern: row count changed on resize,
                    // the row cache is stale and must be invalidated.
                    row_cache.clear();
                    #[allow(clippy::collapsible_if)]
                    if let Some(tx) = config.cell_data_tx.as_ref()
                        && let Some(data) =
                            Self::build_cell_data(&terminal, default_fg, default_bg, &mut row_cache)
                    {
                        let _ = tx.try_send(data);
                    }
                }
                Command::ScrollViewport(delta) => {
                    // C ABI returns void; viewport failures surface as a
                    // no-op (grid unchanged) and the retry logic in
                    // setScrollOffset re-sends on the next offset change.
                    terminal.scroll_viewport(libghostty_vt::terminal::ScrollViewport::Delta(delta));
                    grid_dirty = true;
                    // Rebuild + repush CellData so the renderer draws the
                    // scrolled view immediately.
                    if let Some(tx) = config.cell_data_tx.as_ref()
                        && let Some(data) =
                            Self::build_cell_data(&terminal, default_fg, default_bg, &mut row_cache)
                    {
                        let _ = tx.try_send(data);
                    }
                }
                Command::TakeSnapshot { tx, scroll_offset } => {
                    let needs_rebuild = snapshot_needs_rebuild(
                        grid_dirty,
                        scroll_offset,
                        cached_scroll_offset,
                        cached_snapshot.is_some(),
                    );
                    let snapshot = if needs_rebuild {
                        config
                            .snapshot_rebuild_count
                            .fetch_add(1, Ordering::Relaxed);
                        let snap = Self::build_snapshot(
                            &terminal,
                            default_fg,
                            default_bg,
                            &config.ansi_colors,
                            scroll_offset,
                        );
                        let cached = Arc::new(snap);
                        cached_snapshot = Some(Arc::clone(&cached));
                        cached_scroll_offset = scroll_offset;
                        grid_dirty = false;
                        cached
                    } else {
                        // INVARIANT: when `needs_rebuild` is false, `cached_snapshot`
                        // is always `Some` (the third clause above guarantees it).
                        // Use fallback if invariant is violated (poison etc.).
                        cached_snapshot.as_ref().map(Arc::clone).unwrap_or_else(|| {
                            log::error!(
                                "ghostty_terminal: cached_snapshot missing — using fallback"
                            );
                            let fb_rows = terminal.rows().unwrap_or(24) as u32;
                            let fb_cols = terminal.cols().unwrap_or(80) as u32;
                            Arc::new(GridSnapshot::fallback(fb_rows, fb_cols))
                        })
                    };
                    try_send(
                        &tx,
                        snapshot,
                        "ghostty_terminal: command channel send failed",
                    );
                }
                Command::ScrollbackLength(tx) => {
                    try_send(
                        &tx,
                        terminal.scrollback_rows().unwrap_or(0) as u32,
                        "command channel send failed",
                    );
                }
                Command::ReadLineText { row, tx } => {
                    let text = Self::read_line_text_impl(&terminal, row);
                    try_send(&tx, text, "ghostty_terminal: command channel send failed");
                }
                Command::ReadVisibleText(tx) => {
                    let rows = terminal.rows().unwrap_or(24) as u32;
                    let scrollback_rows = terminal.scrollback_rows().unwrap_or(0) as u32;
                    let mut text = String::new();
                    for row in 0..rows {
                        // read_line_text_impl expects an absolute row (history + viewport).
                        if let Some(line) =
                            Self::read_line_text_impl(&terminal, scrollback_rows + row)
                        {
                            text.push_str(&line);
                            text.push('\n');
                        }
                    }
                    try_send(&tx, text, "ghostty_terminal: command channel send failed");
                }
                Command::SearchInScrollback { query, tx } => {
                    let result = Self::search_in_scrollback_impl(&terminal, &query);
                    try_send(&tx, result, "ghostty_terminal: command channel send failed");
                }
                Command::SearchInScrollbackAll {
                    query,
                    case_sensitive,
                    fuzzy,
                    tx,
                } => {
                    let results = Self::search_in_scrollback_all_impl(
                        &terminal,
                        &query,
                        case_sensitive,
                        fuzzy,
                    );
                    try_send(
                        &tx,
                        results,
                        "ghostty_terminal: command channel send failed",
                    );
                }
                Command::DumpGrid { tx } => {
                    let dumped = Self::build_dumped_grid(&terminal);
                    try_send(&tx, dumped, "ghostty_terminal: command channel send failed");
                }
                Command::TakeKittyGraphicsImage { id, tx } => {
                    let kitty_graphics_data = (|| -> Option<KittyGraphicsImageData> {
                        let graphics = terminal.kitty_graphics().ok()?;
                        let image = graphics.image(id)?;
                        let width = image.width().ok()?;
                        let height = image.height().ok()?;
                        let data = image.data().ok()?;
                        Some(KittyGraphicsImageData {
                            id,
                            width,
                            height,
                            data: data.to_vec(),
                        })
                    })();
                    try_send(
                        &tx,
                        kitty_graphics_data,
                        "ghostty_terminal: command channel send failed",
                    );
                }
                Command::KeyEncode {
                    key_code,
                    modifiers,
                    action,
                    unicode_char,
                    unshifted_char,
                    tx,
                } => {
                    let (encoder, event) = match (encoder.as_mut(), event.as_mut()) {
                        (Some(enc), Some(evt)) => (enc, evt),
                        _ => {
                            log::warn!(
                                "ghostty_terminal: key encoder/event unavailable — dropping key"
                            );
                            let _ = tx.send(Vec::new());
                            continue;
                        }
                    };

                    let ghostty_key = map_android_key_code(key_code);
                    let mods = Mods::from_bits_retain(modifiers);
                    let encoder_action = match action {
                        1 => key::Action::Release,
                        2 => key::Action::Repeat,
                        _ => key::Action::Press,
                    };

                    encoder.set_options_from_terminal(&terminal);
                    event.set_action(encoder_action);
                    event.set_key(ghostty_key);
                    event.set_consumed_mods(Mods::empty());
                    // Clear text state left over from the previous keystroke.
                    event.set_utf8(None::<&str>);
                    event.set_unshifted_codepoint('\0');

                    // Per libghostty-vt key/event.h:
                    // - `utf8` is the produced text WITHOUT Ctrl/Alt
                    //   transformations. C0 control characters
                    //   (U+0000..U+001F, U+007F) must NOT be passed; pass NULL
                    //   so the encoder uses the logical key instead.
                    // - `unshifted_codepoint` is the base key with NO modifiers.
                    // The Kotlin bridge supplies `unshifted_char`; when absent we
                    // fall back to `unicode_char` for both fields.
                    let is_c0 = unicode_char <= 0x1F || unicode_char == 0x7F;
                    if !is_c0 {
                        if let Some(character) = char::from_u32(unicode_char) {
                            let mut utf8_buf = [0u8; 4];
                            event.set_utf8(Some(character.encode_utf8(&mut utf8_buf)));
                        }
                        let unshifted_cp = char::from_u32(if unshifted_char > 0 {
                            unshifted_char
                        } else {
                            unicode_char
                        });
                        if let Some(cp) = unshifted_cp {
                            event.set_unshifted_codepoint(cp);
                        }
                        // RK2: when SHIFT only changed the printed character
                        // (e.g. Shift+; -> :), strip SHIFT so the Kitty
                        // keyboard protocol does not emit a spurious
                        // `\033[59;2u` for plain printable input. Requires the
                        // unshifted codepoint to detect the shift-only change.
                        let final_mods = if mods.contains(Mods::SHIFT)
                            && unshifted_char > 0
                            && unicode_char != unshifted_char
                        {
                            mods & !Mods::SHIFT
                        } else {
                            mods
                        };
                        event.set_mods(final_mods);
                    } else {
                        event.set_mods(mods);
                    }

                    let mut response = Vec::new();
                    if let Err(error) = encoder.encode_to_vec(event, &mut response) {
                        log::warn!("ghostty_terminal: encoder.encode_to_vec failed: {error}");
                    }
                    try_send(&tx, response, "key_encode response send failed");
                }
                Command::EncodeMouseEvent {
                    position,
                    action,
                    button,
                    cell_w,
                    cell_h,
                    tx,
                } => {
                    // Reference: zelland src-tauri/src/terminal.rs
                    // `encode_mouse_event` — uses the Ghostty mouse encoder
                    // with the renderer's live cell size, and drops the
                    // event when mouse reporting is off. The encoder takes
                    // options from the terminal (tracking mode + format) so
                    // SGR/X10/UTF-8 output follows the application's
                    // DECSET selection.
                    let (mouse_encoder, mouse_event) = match (
                        mouse_encoder.as_mut(),
                        mouse_event.as_mut(),
                    ) {
                        (Some(enc), Some(evt)) => (enc, evt),
                        _ => {
                            log::warn!(
                                "ghostty_terminal: mouse encoder/event unavailable — dropping mouse event"
                            );
                            let _ = tx.send(Vec::new());
                            continue;
                        }
                    };
                    mouse_encoder.set_options_from_terminal(&terminal);
                    let cols = terminal.cols().unwrap_or(80) as u32;
                    let rows = terminal.rows().unwrap_or(24) as u32;
                    let size = mouse::EncoderSize {
                        screen_width: cols.saturating_mul(cell_w.max(1.0) as u32),
                        screen_height: rows.saturating_mul(cell_h.max(1.0) as u32),
                        cell_width: cell_w.max(1.0) as u32,
                        cell_height: cell_h.max(1.0) as u32,
                        padding_top: 0,
                        padding_bottom: 0,
                        padding_right: 0,
                        padding_left: 0,
                    };
                    mouse_encoder.set_size(size);
                    mouse_event.set_position(mouse::Position {
                        x: position.0,
                        y: position.1,
                    });
                    mouse_event.set_action(match action {
                        1 => mouse::Action::Release,
                        2 => mouse::Action::Motion,
                        _ => mouse::Action::Press,
                    });
                    mouse_event.set_button(match button {
                        1 => Some(mouse::Button::Right),
                        2 => Some(mouse::Button::Middle),
                        3 => Some(mouse::Button::Four),
                        4 => Some(mouse::Button::Five),
                        _ => Some(mouse::Button::Left),
                    });
                    let mut response = Vec::new();
                    if let Err(error) = mouse_encoder.encode_to_vec(mouse_event, &mut response) {
                        log::warn!("ghostty_terminal: mouse encode failed: {error}");
                    }
                    try_send(&tx, response, "mouse encode response send failed");
                }
                // Query-only commands — never reach command_receiver in
                // normal operation (they go via query_receiver), but we
                // must still handle them for match exhaustiveness.
                Command::Rows(_)
                | Command::Cols(_)
                | Command::CursorX(_)
                | Command::CursorY(_)
                | Command::CursorVisible(_)
                | Command::OriginMode(_)
                | Command::Autowrap(_)
                | Command::AltScreen(_)
                | Command::Title(_)
                | Command::Cwd(_)
                | Command::ModeGet(..)
                | Command::SelectionText { .. }
                | Command::HyperlinkAt { .. } => {
                    log::warn!("ghostty_terminal: unexpected query on command channel, skipping");
                }
                Command::Terminate => break,
            }
            // After processing the bounded command, drain any pending queries
            // so they see the updated terminal state.
            while let Ok(query) = query_receiver.try_recv() {
                Self::process_query(query, &mut terminal);
            }
        }
    }

    pub(crate) fn recv_or_fallback<T: core::fmt::Debug>(
        rx: flume::Receiver<T>,
        fallback: T,
        method: &str,
    ) -> T {
        match rx.recv_timeout(std::time::Duration::from_millis(QUERY_TIMEOUT_MS)) {
            Ok(value) => value,
            Err(_) => {
                log::warn!(
                    "ghostty_terminal: {method} timed out — returning fallback: {fallback:?}"
                );
                fallback
            }
        }
    }
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

    /// Builds the full `CellData` grid for rendering, skipping clean rows.
    ///
    /// Reference: zelland src-tauri/src/renderer/mod.rs `draw_ghostty_state`
    /// (row-level dirty cache): the Ghostty render state tracks per-row
    /// dirty flags; rows that did not change since the last build are copied
    /// from `row_cache` instead of re-walking their cells (which costs N
    /// FFI calls and per-cell style/color resolution). The output is still
    /// the full flat `Vec<CellData>` (render side and JNI are unchanged).
    /// The cache is invalidated by the caller on resize (row count changes).
    pub(crate) fn build_cell_data(
        terminal: &Terminal,
        default_fg: [f32; 4],
        default_bg: [f32; 4],
        row_cache: &mut Vec<Vec<CellData>>,
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
            let row_idx = current_row as usize;
            let is_dirty = row.dirty().unwrap_or(true);
            // zelland row-cache pattern: clean rows are copied from the
            // cache instead of re-walking their cells (FFI per cell).
            if !is_dirty && let Some(cached) = row_cache.get(row_idx) {
                data.extend_from_slice(cached);
                current_row += 1;
                continue;
            }

            let mut row_data = Vec::with_capacity(cols as usize);
            let mut cell_iter_impl = match cell_iter.update(row) {
                Ok(ci) => ci,
                Err(_) => break,
            };

            let mut current_col = 0u32;
            // CellRun-style per-row style cache (termlib CellRun.kt):
            // consecutive cells sharing a style_id resolve their
            // style/fg/bg once; the flat CellData output is unchanged but
            // the per-cell FFI calls (style/fg_color/bg_color) are skipped
            // for the run.
            let mut cached_style_id: Option<libghostty_vt::style::Id> = None;
            let mut cached_fg = default_fg;
            let mut cached_bg = default_bg;
            let mut cached_flags = 0u32;

            while let Some(cell) = cell_iter_impl.next() {
                let raw = match cell.raw_cell() {
                    Ok(c) => c,
                    Err(_) => {
                        row_data.push(CellData {
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

                let style_id = raw.style_id().ok();
                let (_style, fg_color, bg_color, flags) =
                    if style_id.is_some() && style_id == cached_style_id {
                        // Same style run: reuse the cached resolved colors.
                        (None, cached_fg, cached_bg, cached_flags)
                    } else {
                        match cell.style() {
                            Ok(s) => {
                                let fg = match cell.fg_color() {
                                    Ok(Some(rgb)) => [
                                        rgb.r as f32 / 255.0,
                                        rgb.g as f32 / 255.0,
                                        rgb.b as f32 / 255.0,
                                        1.0,
                                    ],
                                    _ => default_fg,
                                };
                                let bg = match cell.bg_color() {
                                    Ok(Some(rgb)) => [
                                        rgb.r as f32 / 255.0,
                                        rgb.g as f32 / 255.0,
                                        rgb.b as f32 / 255.0,
                                        1.0,
                                    ],
                                    _ => default_bg,
                                };
                                let fl = Self::pack_style_flags(&s);
                                cached_style_id = style_id;
                                cached_fg = fg;
                                cached_bg = bg;
                                cached_flags = fl;
                                (Some(s), fg, bg, fl)
                            }
                            Err(_) => {
                                row_data.push(CellData {
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
                        }
                    };

                let codepoint = raw.codepoint().unwrap_or(0);

                // Skip spacer cells (SpacerTail, SpacerHead) that Ghostty
                // emits for wide characters. These have no content and would
                // advance `current_col` incorrectly, causing all subsequent
                // cells to shift right by one column.
                let width = match raw.wide() {
                    Ok(libghostty_vt::screen::CellWide::Wide) => 2,
                    Ok(
                        libghostty_vt::screen::CellWide::SpacerTail
                        | libghostty_vt::screen::CellWide::SpacerHead,
                    ) => {
                        // Spacer cells: do not produce a CellData entry.
                        // current_col stays unchanged — the wide cell already
                        // consumed both columns.
                        continue;
                    }
                    _ => 1,
                };

                let mut grapheme_extra = [0u32; 7];
                if let Ok(g) = cell.graphemes() {
                    for (i, &c) in g.iter().enumerate().skip(1).take(7) {
                        grapheme_extra[i - 1] = c as u32;
                    }
                }

                row_data.push(CellData {
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
            // Update the row cache for this row (zelland row-cache pattern).
            if row_idx >= row_cache.len() {
                row_cache.resize(row_idx + 1, Vec::new());
            }
            row_cache[row_idx] = row_data.clone();
            data.extend_from_slice(&row_data);
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

    /// Pack style attributes into a bitmask matching `cell.wgsl` shader layout:
    /// Bit 0=bold, 1=italic, 2=reverse, 3=underline,
    /// 5=strikethrough, 6=overline, 7=dim, 8=double_underline
    /// Bits 4,9+ reserved for future use (not read by current shader).
    fn pack_style_flags(style: &libghostty_vt::style::Style) -> u32 {
        let mut flags = 0u32;
        if style.bold {
            flags |= 1 << 0;
        }
        if style.italic {
            flags |= 1 << 1;
        }
        if style.inverse {
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
        if style.strikethrough {
            flags |= 1 << 5;
        }
        if style.overline {
            flags |= 1 << 6;
        }
        if style.faint {
            flags |= 1 << 7;
        }
        if style.underline == libghostty_vt::style::Underline::Double {
            flags |= 1 << 8;
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
        // NOTE (round-209): a scrolled snapshot returns an EMPTY fallback
        // grid — the CellData path does not expose scrollback content, and
        // `take_snapshot_with_scroll` is only exercised by tests. Any
        // future query/MCP caller passing a non-zero offset will get an
        // empty grid; implement history snapshots there if needed.
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

        let dirty = vec![true; (rows as usize) * (cols as usize)];

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
    pub(crate) fn read_line_text_impl(terminal: &Terminal, row: u32) -> Option<String> {
        let cols = terminal.cols().unwrap_or(80) as u32;
        let scrollback_rows = terminal.scrollback_rows().unwrap_or(0) as u32;
        let mut text = String::new();
        for col in 0..cols {
            let coord = PointCoordinate {
                x: col as u16,
                y: row,
            };
            let point = if row < scrollback_rows {
                terminal.grid_ref(Point::History(coord))
            } else {
                let viewport_row = row - scrollback_rows;
                let vp_coord = PointCoordinate {
                    x: col as u16,
                    y: viewport_row,
                };
                terminal.grid_ref(Point::Viewport(vp_coord))
            };
            if let Ok(point) = point
                && let Ok(cell) = point.cell()
            {
                let cp = cell.codepoint().unwrap_or(0);
                if cp != 0 {
                    if let Some(ch) = char::from_u32(cp) {
                        text.push(ch);
                    }
                } else {
                    text.push(' ');
                }
            }
        }
        let trimmed = text.trim_end().to_string();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed)
        }
    }

    /// Wrap-aware selection text extraction via Ghostty's native formatter.
    ///
    /// Reference: termux-app TerminalBuffer.getSelectedText (joinBackLines)
    /// plus TerminalRow.findStartOfColumn. The formatter's `unwrap` joins
    /// soft-wrapped lines without '\n' and `trim` removes trailing
    /// whitespace; grid columns map to char indices internally so CJK wide
    /// glyphs are never split (no column-to-char drift on surrogate pairs).
    ///
    /// Coordinates are screen-space rows (0..rows+scrollback are valid);
    /// scrollback rows are passed as negative offsets per ghostty Point
    /// semantics. Returns an empty string for an invalid selection.
    pub(crate) fn selection_text_impl(
        terminal: &Terminal,
        start: (u32, u32),
        end: (u32, u32),
        rectangle: bool,
    ) -> String {
        let cols = terminal.cols().unwrap_or(80) as u32;
        let scrollback_rows = terminal.scrollback_rows().unwrap_or(0) as u32;
        let start_col = (start.1).min(cols - 1);
        let end_col = (end.1).min(cols - 1);
        // Grid rows are absolute (0 = top of history; viewport starts at
        // scrollback_rows). Ghostty's Point::History expects y in history
        // space and Point::Viewport expects viewport-local y; resolve which
        // space each endpoint lives in, mirroring read_line_text_impl.
        let start_point = {
            let y = start.0;
            if y < scrollback_rows {
                Point::History(PointCoordinate {
                    x: start_col as u16,
                    y,
                })
            } else {
                Point::Viewport(PointCoordinate {
                    x: start_col as u16,
                    y: y - scrollback_rows,
                })
            }
        };
        let end_point = {
            let y = end.0;
            if y < scrollback_rows {
                Point::History(PointCoordinate {
                    x: end_col as u16,
                    y,
                })
            } else {
                Point::Viewport(PointCoordinate {
                    x: end_col as u16,
                    y: y - scrollback_rows,
                })
            }
        };
        let (Ok(start_gref), Ok(end_gref)) =
            (terminal.grid_ref(start_point), terminal.grid_ref(end_point))
        else {
            return String::new();
        };
        let selection = libghostty_vt::selection::Selection::new(start_gref, end_gref, rectangle);
        let mut formatter = match libghostty_vt::fmt::Formatter::new(
            terminal,
            libghostty_vt::fmt::FormatterOptions::new()
                .with_unwrap(true)
                .with_trim(true)
                .with_selection(&selection),
        ) {
            Ok(f) => f,
            Err(error) => {
                log::error!("ghostty_terminal: formatter new failed: {error}");
                return String::new();
            }
        };
        match formatter.format_alloc(None) {
            Ok(bytes) => String::from_utf8_lossy(&bytes).into_owned(),
            Err(error) => {
                log::error!("ghostty_terminal: formatter format failed: {error}");
                String::new()
            }
        }
    }

    /// Query the OSC 8 hyperlink URI at a grid cell (termux TerminalView
    /// openLinkAt equivalent; ghostty cell.has_hyperlink + hyperlink_uri).
    pub(crate) fn hyperlink_at_impl(terminal: &Terminal, row: u32, col: u32) -> Option<String> {
        let scrollback_rows = terminal.scrollback_rows().unwrap_or(0) as u32;
        let point = if row < scrollback_rows {
            Point::History(PointCoordinate {
                x: col as u16,
                y: row,
            })
        } else {
            Point::Viewport(PointCoordinate {
                x: col as u16,
                y: row - scrollback_rows,
            })
        };
        let grid_ref = terminal.grid_ref(point).ok()?;
        let cell = grid_ref.cell().ok()?;
        if !cell.has_hyperlink().unwrap_or(false) {
            return None;
        }
        let mut buf = [0u8; 4096];
        let len = grid_ref.hyperlink_uri(&mut buf).ok()?;
        if len == 0 {
            return None;
        }
        Some(String::from_utf8_lossy(&buf[..len]).into_owned())
    }

    pub(crate) fn search_in_scrollback_impl(
        terminal: &Terminal,
        query: &str,
    ) -> Option<(u32, u32)> {
        if query.is_empty() {
            return None;
        }
        let total = terminal.total_rows().unwrap_or(0) as u32;
        for row in 0..total {
            if let Some(line) = Self::read_line_text_impl(terminal, row)
                && let Some(col) = line.find(query)
            {
                return Some((row, col as u32));
            }
        }
        None
    }

    pub(crate) fn search_in_scrollback_all_impl(
        terminal: &Terminal,
        query: &str,
        case_sensitive: bool,
        fuzzy: bool,
    ) -> Vec<SearchMatch> {
        if query.is_empty() {
            return vec![];
        }
        let total = terminal.total_rows().unwrap_or(0) as u32;
        let mut results = Vec::new();
        let search_query = if case_sensitive {
            query.to_string()
        } else {
            query.to_lowercase()
        };
        for row in 0..total {
            if let Some(line) = Self::read_line_text_impl(terminal, row) {
                let search_line = if case_sensitive {
                    line.clone()
                } else {
                    line.to_lowercase()
                };
                if fuzzy {
                    // Window size is the query's char count, not byte
                    // length: a multi-byte query (CJK) otherwise forms
                    // windows that end mid-character and can never match.
                    let query_chars = search_query.chars().count();
                    let max_distance = std::cmp::max(1, query_chars / 3);
                    // Char-boundary byte offsets of the line; windows are
                    // sized by char count so slicing stays valid.
                    let boundaries: Vec<usize> = search_line
                        .char_indices()
                        .map(|(offset, _)| offset)
                        .collect();
                    if query_chars <= boundaries.len() {
                        // Sliding window: find all windows whose edit distance is within threshold.
                        // Return each match position so all results are highlighted, not just
                        // the nearest one (which would miss overlapping near-matches).
                        for (window_index, &start) in boundaries.iter().enumerate() {
                            let end = boundaries
                                .get(window_index + query_chars)
                                .copied()
                                .unwrap_or(search_line.len());
                            let window = &search_line[start..end];
                            let dist = Self::levenshtein_distance(&search_query, window);
                            if dist <= max_distance {
                                // Convert byte offsets to character columns:
                                // rendering highlights by CellData.col (char
                                // index), not byte offset — CJK/emoji rows
                                // would misalign otherwise.
                                let start_col = search_line[..start].chars().count() as u32;
                                let end_col = search_line[..end].chars().count() as u32;
                                results.push(SearchMatch {
                                    row,
                                    start_col,
                                    end_col,
                                });
                            }
                        }
                    }
                } else {
                    let mut start = 0;
                    while let Some(col) = search_line[start..].find(&search_query) {
                        let abs_col = start + col;
                        // Byte offset -> character column (see above).
                        let match_start_col = search_line[..abs_col].chars().count() as u32;
                        let match_end = abs_col + search_query.len();
                        let match_end_col = search_line[..match_end].chars().count() as u32;
                        results.push(SearchMatch {
                            row,
                            start_col: match_start_col,
                            end_col: match_end_col,
                        });
                        // Advance past this match to its end (always a char
                        // boundary), then step to the next boundary so
                        // overlapping matches are still found without
                        // slicing mid-character.
                        let mut next = abs_col + search_query.len();
                        if next < search_line.len() {
                            next += 1;
                            while next < search_line.len() && !search_line.is_char_boundary(next) {
                                next += 1;
                            }
                        }
                        start = next;
                    }
                }
            }
        }
        results
    }

    /// Compute the Levenshtein distance (edit distance) between two strings.
    /// Uses the classic dynamic programming approach with O(min(m,n)) memory.
    pub(crate) fn levenshtein_distance(a: &str, b: &str) -> usize {
        let a_chars: Vec<char> = a.chars().collect();
        let b_chars: Vec<char> = b.chars().collect();
        let m = a_chars.len();
        let n = b_chars.len();
        // Use the shorter string as the column vector for memory efficiency
        if m < n {
            return Self::levenshtein_distance(b, a);
        }
        let mut prev: Vec<usize> = (0..=n).collect();
        for i in 1..=m {
            let mut current = i;
            for j in 1..=n {
                let cost = (a_chars[i - 1] != b_chars[j - 1]) as usize;
                let next =
                    std::cmp::min(std::cmp::min(current + 1, prev[j] + 1), prev[j - 1] + cost);
                prev[j - 1] = current;
                current = next;
            }
            prev[n] = current;
        }
        prev[n]
    }
}
