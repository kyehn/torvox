use std::sync::atomic::Ordering;

use libghostty_vt::key::{self, Mods};
use libghostty_vt::screen::GridRef;
use libghostty_vt::style::{PaletteIndex, StyleColor};
use libghostty_vt::terminal::{Mode, ModeKind, Point, PointCoordinate};
use libghostty_vt::{Terminal, TerminalOptions};

use super::commands::{Command, RunConfig};
use super::keymap::map_android_key_code;
use super::types::*;

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
    pub(crate) fn osc_sequence(command: u8, r: u8, g: u8, b: u8) -> Vec<u8> {
        format!("\x1b]{};rgb:{:02x}/{:02x}/{:02x}\x1b\\", command, r, g, b).into_bytes()
    }

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
                if let Err(error) = tx.send(terminal.is_cursor_visible().unwrap_or(true)) {
                    log::error!("ghostty_terminal: query channel send failed: {error}");
                }
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
                if let Err(error) = tx.send(is_alt) {
                    log::error!("ghostty_terminal: query channel send failed: {error}");
                }
            }
            Command::Title(tx) => {
                if let Err(error) = tx.send(terminal.title().unwrap_or("").to_string()) {
                    log::error!("ghostty_terminal: query channel send failed: {error}");
                }
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
                if let Err(error) = tx.send(len) {
                    log::error!("ghostty_terminal: query channel send failed: {error}");
                }
            }
            Command::ReadLineText { row, tx } => {
                if let Err(error) = tx.send(Self::read_line_text_impl(terminal, row)) {
                    log::error!("ghostty_terminal: query channel send failed: {error}");
                }
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
                if let Err(error) = tx.send(text) {
                    log::error!("ghostty_terminal: query channel send failed: {error}");
                }
            }
            _ => {}
        }
    }

    pub(crate) fn run(config: RunConfig) {
        let Ok(mut terminal) = Terminal::new(TerminalOptions {
            cols: config.cols as u16,
            rows: config.rows as u16,
            max_scrollback: config.scrollback_lines as usize,
        }) else {
            log::error!("ghostty_terminal: Terminal::new failed — thread exiting");
            return;
        };

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

        terminal.vt_write(&Self::osc_sequence(
            11,
            config.background_color[0],
            config.background_color[1],
            config.background_color[2],
        ));
        terminal.vt_write(&Self::osc_sequence(
            10,
            config.foreground_color[0],
            config.foreground_color[1],
            config.foreground_color[2],
        ));

        let query_receiver = config.query_receiver;

        // Cache the last built grid snapshot so we skip the expensive
        // per-cell ghostty FFI rebuild when neither the grid content nor the
        // scroll offset changed since the previous frame. The VT thread is
        // single-threaded and processes commands sequentially, so there is no
        // race between marking `grid_dirty` and rebuilding.
        let mut cached_snapshot: Option<GridSnapshot> = None;
        let mut cached_scroll_offset: u32 = u32::MAX;
        let mut grid_dirty = true;
        loop {
            // Wait for the next command from the bounded channel. Use a
            // timeout so we periodically check the query channel even when
            // no commands are pending (e.g., queries sent between writes).
            let command = match config
                .command_receiver
                .recv_timeout(std::time::Duration::from_millis(50))
            {
                Ok(cmd) => cmd,
                Err(flume::RecvTimeoutError::Timeout) => {
                    // No bounded commands pending — drain query channel so
                    // queries sent between commands don't wait indefinitely.
                    while let Ok(query) = query_receiver.try_recv() {
                        Self::process_query(query, &mut terminal);
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
                }
                Command::FlushAck(tx) => {
                    if let Err(error) = tx.send(()) {
                        log::error!("ghostty_terminal: command channel send failed: {error}");
                    }
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
                    terminal.vt_write(&Self::osc_sequence(
                        11,
                        background[0],
                        background[1],
                        background[2],
                    ));
                    terminal.vt_write(&Self::osc_sequence(
                        10,
                        foreground[0],
                        foreground[1],
                        foreground[2],
                    ));
                    for (i, color) in ansi.iter().enumerate() {
                        let osc4 = format!(
                            "\x1b]4;{};rgb:{:02x}/{:02x}/{:02x}\x1b\\",
                            i, color[0], color[1], color[2]
                        );
                        terminal.vt_write(osc4.as_bytes());
                    }
                    grid_dirty = true;
                }
                Command::Resize { rows, cols } => {
                    if let Err(error) = terminal.resize(
                        cols as u16,
                        rows as u16,
                        DEFAULT_CELL_WIDTH,
                        DEFAULT_CELL_HEIGHT,
                    ) {
                        log::error!("ghostty_terminal: resize failed: {error}");
                    }
                    grid_dirty = true;
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
                        cached_snapshot = Some(snap.clone());
                        cached_scroll_offset = scroll_offset;
                        grid_dirty = false;
                        snap
                    } else {
                        // INVARIANT: when `needs_rebuild` is false, `cached_snapshot`
                        // is always `Some` (the third clause above guarantees it).
                        cached_snapshot
                            .clone()
                            .expect("cached_snapshot present when not rebuilding")
                    };
                    if let Err(error) = tx.send(snapshot) {
                        log::error!("ghostty_terminal: command channel send failed: {error}");
                    }
                }
                Command::ScrollbackLength(tx) => {
                    if let Err(error) = tx.send(terminal.scrollback_rows().unwrap_or(0) as u32) {
                        log::error!("ghostty_terminal: command channel send failed: {error}");
                    }
                }
                Command::ReadLineText { row, tx } => {
                    let text = Self::read_line_text_impl(&terminal, row);
                    if let Err(error) = tx.send(text) {
                        log::error!("ghostty_terminal: command channel send failed: {error}");
                    }
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
                    if let Err(error) = tx.send(text) {
                        log::error!("ghostty_terminal: command channel send failed: {error}");
                    }
                }
                Command::SearchInScrollback { query, tx } => {
                    let result = Self::search_in_scrollback_impl(&terminal, &query);
                    if let Err(error) = tx.send(result) {
                        log::error!("ghostty_terminal: command channel send failed: {error}");
                    }
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
                    if let Err(error) = tx.send(results) {
                        log::error!("ghostty_terminal: command channel send failed: {error}");
                    }
                }
                Command::Rows(tx) => {
                    if let Err(error) = tx.send(terminal.rows().unwrap_or(24) as u32) {
                        log::error!("ghostty_terminal: command channel send failed: {error}");
                    }
                }
                Command::Cols(tx) => {
                    if let Err(error) = tx.send(terminal.cols().unwrap_or(80) as u32) {
                        log::error!("ghostty_terminal: command channel send failed: {error}");
                    }
                }
                Command::CursorX(tx) => {
                    if let Err(error) = tx.send(terminal.cursor_x().unwrap_or(0) as u32) {
                        log::error!("ghostty_terminal: command channel send failed: {error}");
                    }
                }
                Command::CursorY(tx) => {
                    if let Err(error) = tx.send(terminal.cursor_y().unwrap_or(0) as u32) {
                        log::error!("ghostty_terminal: command channel send failed: {error}");
                    }
                }
                Command::CursorVisible(tx) => {
                    if let Err(error) = tx.send(terminal.is_cursor_visible().unwrap_or(true)) {
                        log::error!("ghostty_terminal: command channel send failed: {error}");
                    }
                }
                Command::OriginMode(tx) => {
                    if let Err(error) =
                        tx.send(terminal.mode(Mode::new(6, ModeKind::Dec)).unwrap_or(false))
                    {
                        log::error!("ghostty_terminal: command channel send failed: {error}");
                    }
                }
                Command::Autowrap(tx) => {
                    if let Err(error) =
                        tx.send(terminal.mode(Mode::new(7, ModeKind::Dec)).unwrap_or(false))
                    {
                        log::error!("ghostty_terminal: command channel send failed: {error}");
                    }
                }
                Command::AltScreen(tx) => {
                    let is_alt = terminal
                        .active_screen()
                        .is_ok_and(|s| s == libghostty_vt::screen::Screen::Alternate);
                    if let Err(error) = tx.send(is_alt) {
                        log::error!("ghostty_terminal: command channel send failed: {error}");
                    }
                }
                Command::Cwd(tx) => {
                    if let Err(error) = tx.send(
                        terminal
                            .pwd()
                            .map(|path| path.to_string())
                            .unwrap_or_default(),
                    ) {
                        log::error!("ghostty_terminal: command channel send failed: {error}");
                    }
                }
                Command::ModeGet(num, kind, tx) => {
                    let mode_kind = match kind {
                        0 => libghostty_vt::terminal::ModeKind::Dec,
                        _ => libghostty_vt::terminal::ModeKind::Ansi,
                    };
                    if let Err(error) = tx.send(
                        terminal
                            .mode(libghostty_vt::terminal::Mode::new(num, mode_kind))
                            .unwrap_or(false),
                    ) {
                        log::error!("ghostty_terminal: command channel send failed: {error}");
                    }
                }
                Command::Title(tx) => {
                    if let Err(error) = tx.send(terminal.title().unwrap_or("").to_string()) {
                        log::error!("ghostty_terminal: command channel send failed: {error}");
                    }
                }
                Command::DumpGrid { tx } => {
                    let dumped = Self::build_dumped_grid(&terminal);
                    if let Err(error) = tx.send(dumped) {
                        log::error!("ghostty_terminal: command channel send failed: {error}");
                    }
                }
                Command::TakeKgpImage { id, tx } => {
                    let kgp_data = (|| -> Option<KgpImageData> {
                        let graphics = terminal.kitty_graphics().ok()?;
                        let image = graphics.image(id)?;
                        let width = image.width().ok()?;
                        let height = image.height().ok()?;
                        let data = image.data().ok()?;
                        Some(KgpImageData {
                            id,
                            width,
                            height,
                            data: data.to_vec(),
                        })
                    })();
                    if let Err(error) = tx.send(kgp_data) {
                        log::error!("ghostty_terminal: command channel send failed: {error}");
                    }
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
                    if let Err(error) = tx.send(response) {
                        log::warn!("ghostty_terminal: key_encode response send failed: {error}");
                    }
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

    pub(crate) fn read_semantic_content(point: &libghostty_vt::screen::GridRef) -> SemanticContent {
        match point.cell().and_then(|c| c.semantic_content()) {
            Ok(libghostty_vt::screen::CellSemanticContent::Input) => SemanticContent::Input,
            Ok(libghostty_vt::screen::CellSemanticContent::Prompt) => SemanticContent::Prompt,
            _ => SemanticContent::Output,
        }
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

    pub(crate) fn build_snapshot(
        terminal: &Terminal,
        default_fg: [f32; 4],
        default_bg: [f32; 4],
        palette: &[[u8; 3]; 16],
        scroll_offset: u32,
    ) -> GridSnapshot {
        let rows = terminal.rows().unwrap_or(24) as u32;
        let cols = terminal.cols().unwrap_or(80) as u32;
        let size = (rows * cols) as usize;
        let scrollback_rows = terminal.scrollback_rows().unwrap_or(0) as u32;

        let history_rows = scroll_offset.min(scrollback_rows).min(rows);
        let viewport_rows = rows - history_rows;

        let mut cells = Vec::with_capacity(size);

        let default_data = || -> CellSnapshot {
            CellSnapshot {
                codepoint: 0,
                graphemes: Vec::new(),
                foreground: default_fg,
                background: default_bg,
                bold: false,
                dim: false,
                italic: false,
                underline: false,
                reverse: false,
                strikethrough: false,
                blink: false,
                hidden: false,
                uri: None,
                semantic: SemanticContent::Output,
                overline: false,
                double_underline: false,
                width: 1,
            }
        };

        // Fill from scrollback history for scrolled-up portion
        for row in 0..history_rows {
            let history_row = scrollback_rows - scroll_offset + row;
            for col in 0..cols {
                let coord = PointCoordinate {
                    x: col as u16,
                    y: history_row,
                };
                let mut data = default_data();
                if let Ok(grid_ref) = terminal.grid_ref(Point::History(coord)) {
                    Self::read_cell_into_snapshot(
                        &grid_ref, &mut data, default_fg, default_bg, palette,
                    );
                }
                cells.push(data);
            }
        }

        // Fill from viewport for remaining bottom rows
        for row in 0..viewport_rows {
            for col in 0..cols {
                let coord = PointCoordinate {
                    x: col as u16,
                    y: row,
                };
                let mut data = default_data();
                if let Ok(grid_ref) = terminal.grid_ref(Point::Viewport(coord)) {
                    Self::read_cell_into_snapshot(
                        &grid_ref, &mut data, default_fg, default_bg, palette,
                    );
                }
                cells.push(data);
            }
        }

        let cursor_visible = if scroll_offset > 0 {
            false
        } else {
            terminal.is_cursor_visible().unwrap_or(true)
        };
        let cursor_row = terminal.cursor_y().unwrap_or(0) as u32;
        let cursor_col = terminal.cursor_x().unwrap_or(0) as u32;

        let dirty = vec![true; rows as usize];

        let kgp_placements = Self::collect_kgp_placements(terminal);
        let sync_active = terminal.mode(Mode::SYNC_OUTPUT).unwrap_or(false);

        GridSnapshot {
            rows,
            cols,
            cursor_row,
            cursor_col,
            cursor_visible,
            cursor_style: Default::default(),
            cells,
            dirty,
            kgp_placements,
            title: terminal.title().unwrap_or_default().to_string(),
            scrollback_length: terminal.scrollback_rows().unwrap_or(0) as u32,
            sync_active,
        }
    }

    pub(crate) fn read_cell_into_snapshot(
        grid_ref: &GridRef<'_>,
        data: &mut CellSnapshot,
        default_fg: [f32; 4],
        default_bg: [f32; 4],
        palette: &[[u8; 3]; 16],
    ) {
        if let Ok(cell) = grid_ref.cell() {
            data.codepoint = cell.codepoint().unwrap_or(0);
            let mut buf = [char::default(); MAX_GRAPHEME_CLUSTERS];
            if let Ok(n) = grid_ref.graphemes(&mut buf) {
                data.graphemes = buf[..n].iter().map(|&c| c as u32).collect();
            }
            if let Some(ch) = char::from_u32(data.codepoint)
                && torvox_core::unicode::is_wide(ch)
            {
                data.width = 2;
            }
        }
        if let Ok(style) = grid_ref.style() {
            Self::apply_style_to_snapshot(data, &style, default_fg, default_bg, palette);
        }
        data.semantic = Self::read_semantic_content(grid_ref);
    }

    pub(crate) fn collect_kgp_placements(terminal: &libghostty_vt::Terminal) -> Vec<KgpPlacement> {
        use libghostty_vt::kitty::graphics::PlacementIterator;

        let Ok(graphics) = terminal.kitty_graphics() else {
            log::warn!("ghostty_terminal: kitty_graphics() failed — no KGP placements");
            return Vec::new();
        };
        let Ok(mut iter) = PlacementIterator::new() else {
            log::warn!("ghostty_terminal: PlacementIterator::new() failed");
            return Vec::new();
        };
        let Ok(iteration) = iter.update(&graphics) else {
            log::warn!("ghostty_terminal: PlacementIterator::update() failed");
            return Vec::new();
        };

        let mut placements = Vec::new();
        let mut seen = std::collections::HashSet::new();
        let mut it = iteration;
        while let Some(place) = it.next() {
            let Ok(image_id) = place.image_id() else {
                continue;
            };
            let Ok(placement_id) = place.placement_id() else {
                continue;
            };
            if !seen.insert((image_id, placement_id)) {
                continue;
            }

            let Some(image) = graphics.image(image_id) else {
                continue;
            };
            if let Ok(Some(pos)) = place.viewport_pos(&image, terminal) {
                placements.push(KgpPlacement {
                    image_id,
                    placement_id,
                    row: pos.row,
                    col: pos.col,
                    z: 0,
                });
            }
        }
        placements
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
                    let max_distance = std::cmp::max(1, search_query.len() / 3);
                    if search_query.len() <= search_line.len() {
                        let end = search_line.len() - search_query.len();
                        // Sliding window: find all windows whose edit distance is within threshold.
                        // Return each match position so all results are highlighted, not just
                        // the nearest one (which would miss overlapping near-matches).
                        for start in 0..=end {
                            let window = &search_line[start..start + search_query.len()];
                            let dist = Self::levenshtein_distance(&search_query, window);
                            if dist <= max_distance {
                                results.push(SearchMatch {
                                    row,
                                    start_col: start as u32,
                                    end_col: (start + search_query.len()) as u32,
                                });
                            }
                        }
                    }
                } else {
                    let mut start = 0;
                    while let Some(col) = search_line[start..].find(&search_query) {
                        let abs_col = start + col;
                        results.push(SearchMatch {
                            row,
                            start_col: abs_col as u32,
                            end_col: (abs_col + search_query.len()) as u32,
                        });
                        start = abs_col + 1;
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
                let cost = if a_chars[i - 1] == b_chars[j - 1] {
                    0
                } else {
                    1
                };
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
