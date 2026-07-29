use std::sync::Arc;
use std::sync::atomic::Ordering;

use libghostty_vt::key::{self, Mods};
use libghostty_vt::terminal::{Mode, ModeKind};
use libghostty_vt::{Terminal, TerminalOptions};

use super::commands::{Command, RunConfig};
use super::keymap::map_android_key_code;
use super::types::*;

use super::snapshot;

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
        let mut cached_snapshot: Option<Arc<GridSnapshot>> = None;
        let mut cached_scroll_offset: u32 = u32::MAX;
        // ── Auto-push CellData ──
        // Use a separate dirty flag to avoid coupling with the
        // legacy GridSnapshot grid_dirty tracker. Both flags are
        // set together on Write/Resize/SetTheme, but cleared
        // grid_dirty after TakeSnapshot.
        let mut grid_dirty = true;

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
                    if let Some(tx) = config.cell_data_tx.as_ref() {
                        if let Some(data) = Self::build_cell_data(&terminal, default_fg, default_bg)
                        {
                            let _ = tx.try_send(data);
                        }
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
                    if let Some(tx) = config.cell_data_tx.as_ref() {
                        if let Some(data) = Self::build_cell_data(&terminal, default_fg, default_bg)
                        {
                            let _ = tx.try_send(data);
                        }
                    }
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
                    #[allow(clippy::collapsible_if)]
                    if let Some(tx) = config.cell_data_tx.as_ref() {
                        if let Some(data) = Self::build_cell_data(&terminal, default_fg, default_bg)
                        {
                            let _ = tx.try_send(data);
                        }
                    }
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
                    #[allow(clippy::collapsible_if)]
                    if let Some(tx) = config.cell_data_tx.as_ref() {
                        if let Some(data) = Self::build_cell_data(&terminal, default_fg, default_bg)
                        {
                            let _ = tx.try_send(data);
                        }
                    }
                }
                Command::TakeSnapshot { tx, scroll_offset } => {
                    let needs_rebuild = snapshot::snapshot_needs_rebuild(
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
                Command::DumpGrid { tx } => {
                    let dumped = Self::build_dumped_grid(&terminal);
                    if let Err(error) = tx.send(dumped) {
                        log::error!("ghostty_terminal: command channel send failed: {error}");
                    }
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
                    if let Err(error) = tx.send(kitty_graphics_data) {
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
                Command::SaveSession { tx } => {
                    Self::handle_save_session(&terminal, tx);
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
                | Command::ModeGet(..) => {
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
}
