use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use libghostty_vt::Terminal;
use libghostty_vt::key::{self, Mods};
use libghostty_vt::mouse;
use libghostty_vt::render::{CellIterator, RenderState, RowIterator};
use libghostty_vt::style::PaletteIndex;
use libghostty_vt::terminal::{Mode, ModeKind, Point, PointCoordinate};

use super::commands::{Command, Query, RunConfig};
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
    pub(crate) fn process_query(
        query: Query,
        terminal: &mut Terminal,
        alt_screen_active: &Arc<AtomicBool>,
        encoder: &mut Option<key::Encoder>,
        event: &mut Option<key::Event>,
        mouse_encoder: &mut Option<mouse::Encoder>,
        mouse_event: &mut Option<mouse::Event>,
    ) {
        match query {
            Query::Rows(tx) => {
                if let Err(error) =
                    tx.send(terminal.rows().unwrap_or(DISCONNECTED_ROWS as u16) as u32)
                {
                    log::error!("ghostty_terminal: query channel send failed: {error}");
                }
            }
            Query::Cols(tx) => {
                if let Err(error) =
                    tx.send(terminal.cols().unwrap_or(DISCONNECTED_COLS as u16) as u32)
                {
                    log::error!("ghostty_terminal: query channel send failed: {error}");
                }
            }
            Query::CursorX(tx) => {
                if let Err(error) =
                    tx.send(terminal.cursor_x().unwrap_or(DISCONNECTED_CURSOR_X as u16) as u32)
                {
                    log::error!("ghostty_terminal: query channel send failed: {error}");
                }
            }
            Query::CursorY(tx) => {
                if let Err(error) =
                    tx.send(terminal.cursor_y().unwrap_or(DISCONNECTED_CURSOR_Y as u16) as u32)
                {
                    log::error!("ghostty_terminal: query channel send failed: {error}");
                }
            }
            Query::CursorVisible(tx) => {
                try_send(
                    &tx,
                    terminal.is_cursor_visible().unwrap_or(true),
                    "query channel send failed",
                );
            }
            Query::Title(tx) => {
                try_send(
                    &tx,
                    terminal.title().unwrap_or("").to_string(),
                    "query channel send failed",
                );
            }
            Query::Cwd(tx) => {
                if let Err(error) =
                    tx.send(terminal.pwd().map(|p| p.to_string()).unwrap_or_default())
                {
                    log::error!("ghostty_terminal: query channel send failed: {error}");
                }
            }
            Query::ModeGet(num, kind, tx) => {
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
            Query::ScrollbackLength(tx) => {
                let len = terminal.scrollback_rows().unwrap_or(0) as u32;
                log::debug!("ghostty_terminal: scrollback_rows query returned {len}");
                try_send(&tx, len, "ghostty_terminal: query channel send failed");
            }
            Query::ReadLineText { row, tx } => {
                try_send(
                    &tx,
                    Self::read_line_text_impl(terminal, row),
                    "query channel send failed",
                );
            }
            Query::RenderCursor(tx) => {
                let visible_cursor = Self::build_cell_data(
                    terminal,
                    [1.0, 1.0, 1.0, 1.0],
                    [0.0, 0.0, 0.0, 1.0],
                    &mut Vec::new(),
                    alt_screen_active,
                )
                .and_then(|(_, cursor)| cursor.visible.then_some((cursor.row, cursor.col)));
                try_send(&tx, visible_cursor, "query channel send failed");
            }
            Query::ReadVisibleText(tx) => {
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
            Query::SelectionText {
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
            Query::HyperlinkAt { row, col, tx } => {
                let url = Self::hyperlink_at_impl(terminal, row, col);
                try_send(&tx, url, "hyperlink_at response send failed");
            }
            Query::SearchInScrollback { query, tx } => {
                let result = Self::search_in_scrollback_impl(terminal, &query);
                try_send(&tx, result, "ghostty_terminal: query channel send failed");
            }
            Query::SearchInScrollbackAll {
                query,
                case_sensitive,
                tx,
            } => {
                let results = Self::search_in_scrollback_all_impl(terminal, &query, case_sensitive);
                try_send(&tx, results, "ghostty_terminal: query channel send failed");
            }
            Query::DumpGrid { tx } => {
                let dumped = Self::build_dumped_grid(terminal);
                try_send(&tx, dumped, "ghostty_terminal: query channel send failed");
            }
            Query::TakeKittyGraphicsImage { id, tx } => {
                let kitty_graphics_data = (|| -> Option<KittyGraphicsImageData> {
                    let graphics = terminal.kitty_graphics().ok()?;
                    let image = graphics.image(id)?;
                    let (width, height, rgba) = Self::kitty_image_to_rgba(&image)?;
                    Some(KittyGraphicsImageData {
                        id,
                        width,
                        height,
                        data: rgba,
                    })
                })();
                try_send(
                    &tx,
                    kitty_graphics_data,
                    "ghostty_terminal: query channel send failed",
                );
            }
            Query::TakeKittyPlacements { tx } => {
                let placements = Self::collect_kitty_placements(terminal);
                try_send(
                    &tx,
                    placements,
                    "ghostty_terminal: query channel send failed",
                );
            }
            Query::KeyEncode {
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
                        return;
                    }
                };

                let ghostty_key = map_android_key_code(key_code);
                let mods = Mods::from_bits_retain(modifiers);
                let encoder_action = match action {
                    1 => key::Action::Release,
                    2 => key::Action::Repeat,
                    _ => key::Action::Press,
                };

                encoder.set_options_from_terminal(terminal);
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
                    // (e.g. Shift+; ->:), strip SHIFT so the Kitty
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
            Query::EncodeMouseEvent {
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
                        return;
                    }
                };
                mouse_encoder.set_options_from_terminal(terminal);
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
        let Ok(mut terminal) = Terminal::new(config.cols as u16, config.rows as u16) else {
            log::error!("ghostty_terminal: Terminal::new failed — thread exiting");
            return;
        };
        // Upstream (git master) no longer takes options in `Terminal::new`;
        // scrollback is configured with the `set_scrollback_max_lines` setter.
        // A non-zero value enables scrollback (scrollback_rows query returned
        // 0 when scrollback was disabled).
        if let Err(error) =
            terminal.set_scrollback_max_lines(Some(config.scrollback_lines as usize))
        {
            log::error!("ghostty_terminal: set_scrollback_max_lines failed: {error}");
        }

        // Initialize Kitty Graphics Protocol (KGP) support
        if let Err(error) = terminal.set_kitty_image_storage_limit(KGP_STORAGE_LIMIT) {
            log::error!("ghostty_terminal: set_kitty_image_storage_limit failed: {error}");
        }
        // PNG 载荷在进程内解码为 RGBA8（线程局部回调，须在 VT 线程注册；
        // 上游 RustPngDecoder 无公开构造器，自实现等价解码器）。
        if let Err(error) = libghostty_vt::kitty::graphics::set_png_decoder(Some(Box::new(
            KittyPngDecoder::default(),
        ))) {
            log::error!("ghostty_terminal: set_png_decoder failed: {error}");
        }

        // Register PTY write-back callback for terminal responses
        // (DECRPM mode reports, DSR, DA, etc.)
        if let Err(error) = terminal.on_pty_write({
            let response_buffer = config.response_buffer.clone();
            move |_terminal, data| {
                if let Ok(mut guard) = response_buffer.lock() {
                    guard.push(data.to_vec());
                }
            }
        }) {
            log::error!("ghostty_terminal: on_pty_write callback registration failed: {error}");
        }
        // OSC 7 工作目录与 OSC 52 剪贴板写入改走上游回调：序列直达 Ghostty，
        // 事件经通道推送、由调用方在 flush 后收割。本仓不再自建 OSC 解析器。
        // try_send 永不阻塞 VT 线程；OSC 52 读取请求（`?`）上游明确忽略，
        // 仍由 OutputProcessor 的最小扫描器拦截（FR-036）。
        if let Err(error) = terminal.on_pwd_changed({
            let cwd_tx = config.cwd_tx.clone();
            move |terminal| {
                if let Ok(pwd) = terminal.pwd() {
                    let _ = cwd_tx.try_send(pwd.to_string());
                }
            }
        }) {
            log::error!("ghostty_terminal: on_pwd_changed callback registration failed: {error}");
        }
        if let Err(error) = terminal.on_clipboard_write({
            let clipboard_tx = config.clipboard_tx.clone();
            move |_terminal, write| {
                // 选择器字母沿用 xterm 约定（Kotlin 侧原样透传）：c=剪贴板、p=主选区、s=次选区。
                let selection = match write.location() {
                    libghostty_vt::terminal::ClipboardLocation::Standard => "c",
                    libghostty_vt::terminal::ClipboardLocation::Primary => "p",
                    libghostty_vt::terminal::ClipboardLocation::Selection => "s",
                }
                .to_string();
                let mut fallback: Option<&[u8]> = None;
                let mut chosen: Option<&[u8]> = None;
                for content in write.contents() {
                    if fallback.is_none() {
                        fallback = Some(content.data);
                    }
                    if content.mime.starts_with("text/") {
                        chosen = Some(content.data);
                        break;
                    }
                }
                let text = String::from_utf8_lossy(chosen.or(fallback).unwrap_or(&[])).into_owned();
                let _ = clipboard_tx.try_send((selection, text));
                Ok(())
            }
        }) {
            log::error!(
                "ghostty_terminal: on_clipboard_write callback registration failed: {error}"
            );
        }
        // BEL 振铃走上游 on_bell 回调：VT 线程推送空消息，调用方在 flush 后收割。
        // try_send 永不阻塞 VT 线程；满则丢弃单次振铃（振铃是瞬时提示，可合并）。
        if let Err(error) = terminal.on_bell({
            let bell_tx = config.bell_tx.clone();
            move |_terminal| {
                let _ = bell_tx.try_send(());
            }
        }) {
            log::error!("ghostty_terminal: on_bell callback registration failed: {error}");
        }

        let mut default_background = Self::byte_color_to_float(config.background_color);
        let mut default_foreground = Self::byte_color_to_float(config.foreground_color);

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

        // Initial theme: libghostty-vt does not process OSC 10/11
        // default-color escapes (the embedder owns the defaults; OSC 4
        // palette overrides are processed upstream), so use
        // the native setters. Without this the terminal keeps the built-in
        // xterm palette and the theme colors never reach the grid.
        Self::apply_theme(
            &mut terminal,
            config.background_color,
            config.foreground_color,
            &config.ansi_colors,
        );

        // Clone (flume receivers are Arc-handles): keeps `config`
        // borrowable for the command loop below without partial moves.
        let query_receiver = config.query_receiver.clone();

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
        // Content dedup for the auto-push path: `push_cell_data` builds the
        // full grid on every loop iteration (including the 50ms idle
        // timeout), and the render thread treats every received batch as
        // `is_new_data` → a full repaint per frame. On an idle terminal
        // that kept the renderer busy at full rate (measured ~5 fps on the
        // SwiftShader emulator, burned CPU on real GPUs too: "20fps is
        // unacceptable"). Skip the send entirely when neither the cells nor
        // the cursor changed since the previous push.
        let mut last_cell_data_push: Option<(Vec<CellData>, CursorInfo)> = None;

        'vt_loop: loop {
            // Wait for the next command from the bounded channel. Use a
            // timeout so we periodically check the query channel even when
            // no commands are pending (e.g., queries sent between writes).
            let mut pending = match config
                .command_receiver
                .recv_timeout(std::time::Duration::from_millis(50))
            {
                Ok(command) => Some(command),
                Err(flume::RecvTimeoutError::Timeout) => {
                    // No bounded commands pending — drain query channel so
                    // queries sent between commands don't wait indefinitely.
                    while let Ok(query) = query_receiver.try_recv() {
                        Self::process_query(
                            query,
                            &mut terminal,
                            &config.alt_screen_active,
                            &mut encoder,
                            &mut event,
                            &mut mouse_encoder,
                            &mut mouse_event,
                        );
                    }
                    // ── Auto-push CellData (also sent on each state change below) ──
                    Self::refresh_cell_data(
                        &config,
                        &terminal,
                        default_foreground,
                        default_background,
                        &mut row_cache,
                        &mut last_cell_data_push,
                    );
                    continue;
                }
                Err(flume::RecvTimeoutError::Disconnected) => break,
            };
            // Process the ENTIRE command backlog with ONE cell-data build at
            // the end. poll_pty_output feeds up to MAX_CHUNKS_PER_FRAME Write
            // commands per render frame; rebuilding the full grid after every
            // single Write (~2444 Ghostty FFI calls per walk) throttled
            // sustained-output rendering to ~6 fps on SwiftShader because
            // N−1 of the N builds were overwritten before the renderer ever
            // saw them. Intermediate states are intentionally coalesced —
            // the renderer only ever consumes the newest state.
            //
            // `grid_dirty` is still raised INLINE by each mutating command
            // (not deferred to the end of the batch): a TakeSnapshot arriving
            // in the same batch must observe the writes that preceded it in
            // FIFO order and rebuild rather than serve a stale cache.
            let mut batch_dirty = false;
            let mut batch_acks: Vec<flume::Sender<()>> = Vec::new();
            while let Some(command) = pending.take() {
                match command {
                    Command::Write(data) => {
                        terminal.vt_write(&data);
                        grid_dirty = true;
                        batch_dirty = true;
                    }
                    Command::FlushAck(tx) => {
                        // Held until the end-of-batch build completes: the
                        // `flush()` contract is that when it returns, every
                        // command before it — INCLUDING the CellData push —
                        // has been processed. Tests call flush() and then
                        // receive_cell_data(); an early ack would race the
                        // build and hand them an empty channel.
                        batch_acks.push(tx);
                    }
                    Command::SetTheme {
                        background,
                        foreground,
                        ansi,
                    } => {
                        default_background = Self::byte_color_to_float(background);
                        default_foreground = Self::byte_color_to_float(foreground);
                        log::debug!(
                            "SetTheme: background={:?} foreground={:?} -> default_background={:?} default_foreground={:?}",
                            background,
                            foreground,
                            default_background,
                            default_foreground
                        );
                        // Use the native theme API instead of hand-written
                        // OSC 10/11 sequences: libghostty-vt does not process
                        // OSC default-color escapes (the embedder owns the
                        // defaults; OSC 4 overrides are processed upstream), so the OSC approach silently
                        // kept the built-in xterm palette. These setters store
                        // the default colors that upstream cell color queries
                        // resolve against.
                        Self::apply_theme(&mut terminal, background, foreground, &ansi);
                        grid_dirty = true;
                        batch_dirty = true;
                    }
                    Command::Resize { rows, cols } => {
                        // Ghostty's C API takes u16 dimensions; reject out-of-
                        // range values instead of silently truncating (a
                        // hostile Kotlin caller could pass >65535 and wrap).
                        let (Ok(cols), Ok(rows)) = (u16::try_from(cols), u16::try_from(rows))
                        else {
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
                        // zelland row-cache pattern: row count changed on resize,
                        // the row cache is stale and must be invalidated.
                        row_cache.clear();
                        grid_dirty = true;
                        batch_dirty = true;
                    }
                    Command::SetCellPixelSize {
                        cell_width,
                        cell_height,
                    } => {
                        // 同行列重调，仅更新单元格像素几何（Kitty 放置/鼠标映射用）。
                        // 网格内容不变，不失效行缓存、不置脏，避免字体变化引发全量重绘。
                        let (Ok(cols), Ok(rows)) = (terminal.cols(), terminal.rows()) else {
                            continue;
                        };
                        if cell_width == 0 || cell_height == 0 {
                            continue;
                        }
                        if let Err(error) = terminal.resize(cols, rows, cell_width, cell_height) {
                            log::error!("ghostty_terminal: cell resize failed: {error}");
                        }
                    }
                    Command::ScrollViewport(delta) => {
                        // C ABI returns void; viewport failures surface as a
                        // no-op (grid unchanged) and the retry logic in
                        // setScrollOffset re-sends on the next offset change.
                        terminal
                            .scroll_viewport(libghostty_vt::terminal::ScrollViewport::Delta(delta));
                        grid_dirty = true;
                        batch_dirty = true;
                    }
                    Command::Reset => {
                        // RIS 全重置：恢复初始状态并清空回滚；全部帧缓存失效。
                        // 显式清除终端持有的活动选区（幂等，不依赖上游副作用）。
                        terminal.reset();
                        let _ = terminal.set_selection(None);
                        row_cache.clear();
                        cached_snapshot = None;
                        cached_scroll_offset = u32::MAX;
                        last_cell_data_push = None;
                        grid_dirty = true;
                        batch_dirty = true;
                    }
                    Command::SetSelection {
                        start,
                        end,
                        rectangle,
                    } => {
                        // 终端持有化：选区经 set_selection 安装为终端状态
                        //（上游转为跟踪引用，随滚动/输出/重排跟随文本）。
                        // 选区变化改变每行反白，需失效行缓存并重推帧。
                        Self::install_selection_impl(&terminal, start, end, rectangle);
                        row_cache.clear();
                        last_cell_data_push = None;
                        grid_dirty = true;
                        batch_dirty = true;
                    }
                    Command::ClearSelection => {
                        if terminal.set_selection(None).is_err() {
                            log::warn!("ghostty_terminal: clear selection failed");
                        }
                        row_cache.clear();
                        last_cell_data_push = None;
                        grid_dirty = true;
                        batch_dirty = true;
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
                                default_foreground,
                                default_background,
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
                    Command::Terminate => break 'vt_loop,
                }
                pending = config.command_receiver.try_recv().ok();
            }
            // After processing the batch, drain any pending queries so they
            // see the fully-updated terminal state.
            while let Ok(query) = query_receiver.try_recv() {
                Self::process_query(
                    query,
                    &mut terminal,
                    &config.alt_screen_active,
                    &mut encoder,
                    &mut event,
                    &mut mouse_encoder,
                    &mut mouse_event,
                );
            }
            // ONE cell-data build for the whole batch — every state mutation
            // above has completed, so this reflects the final backlog state.
            // `grid_dirty` was raised inline by each mutating command and
            // stays set until a TakeSnapshot consumes it (snapshot
            // cache-invalidation semantics, unchanged from the per-command
            // version).
            if batch_dirty {
                Self::refresh_cell_data(
                    &config,
                    &terminal,
                    default_foreground,
                    default_background,
                    &mut row_cache,
                    &mut last_cell_data_push,
                );
            }
            // Flushers are released only after the build above, preserving
            // the original per-command ordering guarantee of `flush()`.
            for ack in batch_acks {
                try_send(&ack, (), "command channel send failed");
            }
        }
    }

    pub(crate) fn apply_style_to_snapshot(
        data: &mut CellSnapshot,
        style: &libghostty_vt::style::Style,
        terminal: &Terminal,
        default_foreground: [f32; 4],
        default_background: [f32; 4],
    ) {
        data.foreground = Self::resolve_style_color(terminal, &style.fg_color, default_foreground);
        data.background = Self::resolve_style_color(terminal, &style.bg_color, default_background);
        // SGR 58 下划线色：未设置时回退到解析后的前景（着色器旧 `deco = foreground` 语义）。
        data.underline_color =
            Self::resolve_style_color(terminal, &style.underline_color, data.foreground);
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
        let (_, fallback_background, fallback_foreground) = Self::catppuccin_mocha_palette();
        let default_foreground = terminal
            .default_fg_color()
            .ok()
            .flatten()
            .map(|color| Self::byte_color_to_float([color.r, color.g, color.b]))
            .unwrap_or_else(|| Self::byte_color_to_float(fallback_foreground));
        let default_background = terminal
            .default_bg_color()
            .ok()
            .flatten()
            .map(|color| Self::byte_color_to_float([color.r, color.g, color.b]))
            .unwrap_or_else(|| Self::byte_color_to_float(fallback_background));

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
                            &mut data,
                            &style,
                            terminal,
                            default_foreground,
                            default_background,
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
                            &mut data,
                            &style,
                            terminal,
                            default_foreground,
                            default_background,
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

    /// Push a blank (codepoint 0) cell into the row data. Used for cells
    /// whose raw ghostty data or style cannot be resolved.
    fn push_blank_cell(
        row_data: &mut Vec<CellData>,
        default_foreground: [f32; 4],
        default_background: [f32; 4],
        row: u32,
        col: u32,
    ) {
        row_data.push(CellData {
            codepoint: 0,
            width: 1,
            grapheme_extra: [0; 7],
            foreground: default_foreground,
            background: default_background,
            underline_color: default_foreground,
            flags: 0,
            row,
            col,
        });
    }

    /// Map a ghostty cursor visual style to the app-level cursor style.
    /// Hollow block renders as solid block for now (shape subdivision later).
    /// Unknown future upstream styles fall back to block: the cursor must
    /// always paint something (DESIGN 极端崩溃不适用于每帧可见元素）。
    fn cursor_style_from_snapshot(
        snapshot: &libghostty_vt::render::Snapshot<'_, '_>,
    ) -> CursorStyle {
        use libghostty_vt::render::CursorVisualStyle;
        match snapshot.cursor_visual_style() {
            Ok(CursorVisualStyle::Bar) => CursorStyle::Bar,
            Ok(CursorVisualStyle::Underline) => CursorStyle::Underline,
            _ => CursorStyle::Block,
        }
    }

    /// Resolve a cell color to `[r, g, b, 1.0]` floats, falling back to the
    /// cell default when the FFI returns an error or transparent color.
    fn cell_color(
        color: Result<Option<libghostty_vt::style::RgbColor>, libghostty_vt::error::Error>,
        default: [f32; 4],
    ) -> [f32; 4] {
        match color {
            Ok(Some(rgb)) => [
                rgb.r as f32 / 255.0,
                rgb.g as f32 / 255.0,
                rgb.b as f32 / 255.0,
                1.0,
            ],
            _ => default,
        }
    }

    /// Apply default background/foreground colors and the 16-color ANSI palette via the
    /// native theme API. libghostty-vt does not process OSC 10/11
    /// default-color escapes (the embedder owns the defaults; OSC 4 palette
    /// overrides are processed upstream), so the
    /// embedder must push theme colors directly; without this the terminal
    /// keeps the built-in xterm palette.
    fn apply_theme(
        terminal: &mut libghostty_vt::terminal::Terminal,
        background: [u8; 3],
        foreground: [u8; 3],
        ansi: &[[u8; 3]],
    ) {
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
    }

    /// Resolve a `StyleColor` against the terminal's EFFECTIVE 256-color
    /// palette (default colors plus OSC 4 overrides), falling back to
    /// `default` when unset or unreadable. Replaces the historic static
    /// 16-color table + xterm formula, which silently ignored OSC 4 recolors.
    pub(crate) fn resolve_style_color(
        terminal: &Terminal,
        color: &libghostty_vt::style::StyleColor,
        default: [f32; 4],
    ) -> [f32; 4] {
        match color {
            libghostty_vt::style::StyleColor::Rgb(c) => Self::byte_color_to_float([c.r, c.g, c.b]),
            libghostty_vt::style::StyleColor::Palette(idx) => terminal
                .color_palette()
                .map(|palette| {
                    let rgb = palette.get(*idx);
                    Self::byte_color_to_float([rgb.r, rgb.g, rgb.b])
                })
                .unwrap_or(default),
            _ => default,
        }
    }

    pub(crate) fn byte_color_to_float(color: [u8; 3]) -> [f32; 4] {
        [
            Self::byte_to_float(color[0]),
            Self::byte_to_float(color[1]),
            Self::byte_to_float(color[2]),
            1.0,
        ]
    }

    /// Rebuild CellData from current terminal state and push it to the render
    /// thread's channel (no-op when no consumer is attached). Single call
    /// site for the frame-emission block shared by Write/SetTheme/Resize/
    /// ScrollViewport.
    /// Push a fresh cell-data snapshot after the terminal grid changed.
    /// A thin wrapper over [`Self::push_cell_data`] that supplies the
    /// standard config/state plumbing shared by every command handler.
    fn refresh_cell_data(
        config: &RunConfig,
        terminal: &libghostty_vt::terminal::Terminal,
        default_foreground: [f32; 4],
        default_background: [f32; 4],
        row_cache: &mut Vec<Vec<CellData>>,
        last_push: &mut Option<(Vec<CellData>, CursorInfo)>,
    ) {
        Self::push_cell_data(
            config.cell_data_tx.as_ref(),
            &config.alt_screen_active,
            terminal,
            default_foreground,
            default_background,
            row_cache,
            last_push,
        );
    }

    fn push_cell_data(
        cell_data_tx: Option<&Sender<(Vec<CellData>, CursorInfo)>>,
        alt_screen_active: &Arc<AtomicBool>,
        terminal: &Terminal,
        default_foreground: [f32; 4],
        default_background: [f32; 4],
        row_cache: &mut Vec<Vec<CellData>>,
        last_push: &mut Option<(Vec<CellData>, CursorInfo)>,
    ) {
        if let Some(tx) = cell_data_tx
            && let Some(data) = Self::build_cell_data(
                terminal,
                default_foreground,
                default_background,
                row_cache,
                alt_screen_active,
            )
        {
            // Skip the send when nothing changed since the last push: the
            // loop timer (50ms idle wakeup) re-runs this path with no
            // terminal activity, and the render thread treats any received
            // batch as new data → a full repaint every frame. Comparing the
            // bytemuck Pod cells as raw bytes (plus the cursor) is ~150KB
            // memcmp on a 24x80 grid — microseconds, dwarfed by the
            // full-render cost it avoids.
            let unchanged = match last_push {
                Some((last_cells, last_cursor)) => {
                    *last_cursor == data.1
                        && bytemuck::cast_slice::<CellData, u8>(last_cells)
                            == bytemuck::cast_slice::<CellData, u8>(&data.0)
                }
                None => false,
            };
            if unchanged {
                return;
            }
            *last_push = Some(data.clone());
            let _ = tx.try_send(data);
        }
    }
}

/// PNG 解码器（上游 `DecodePng` 实现，等价于缺构造器的 RustPngDecoder）。
#[derive(Default)]
struct KittyPngDecoder {
    scratch: Vec<u8>,
}

impl libghostty_vt::kitty::graphics::DecodePng for KittyPngDecoder {
    fn decode_png<'alloc>(
        &mut self,
        alloc: &'alloc libghostty_vt::alloc::Allocator<'_>,
        data: &[u8],
    ) -> Option<libghostty_vt::kitty::graphics::DecodedImage<'alloc>> {
        use png::{ColorType, Decoder, Transformations};
        use std::io::Cursor;
        let mut decoder = Decoder::new(Cursor::new(data));
        // 与上游 RustPngDecoder 一致：ALPHA 把调色板展开为 RGBA 并保留透明，
        // STRIP_16 把 16 位截断为 8 位。png crate 不支持灰度转 RGB，
        // 剩余灰度/RGB 输出在下面手动展开为 RGBA8。
        decoder.set_transformations(Transformations::ALPHA | Transformations::STRIP_16);
        let mut reader = decoder.read_info().ok()?;
        let mut raw = vec![0u8; reader.output_buffer_size()?];
        let info = reader.next_frame(&mut raw).ok()?;
        let frame_bytes = raw.get(..info.buffer_size())?;
        self.scratch.clear();
        let rgba: &[u8] = match info.color_type {
            ColorType::Rgba => frame_bytes,
            ColorType::Rgb => {
                self.scratch.reserve(frame_bytes.len() / 3 * 4);
                for triple in frame_bytes.as_chunks::<3>().0 {
                    self.scratch
                        .extend_from_slice(&[triple[0], triple[1], triple[2], 255]);
                }
                &self.scratch
            }
            ColorType::Grayscale => {
                self.scratch.reserve(frame_bytes.len() * 4);
                for gray in frame_bytes {
                    self.scratch.extend_from_slice(&[*gray, *gray, *gray, 255]);
                }
                &self.scratch
            }
            ColorType::GrayscaleAlpha => {
                self.scratch.reserve(frame_bytes.len() / 2 * 4);
                for pair in frame_bytes.as_chunks::<2>().0 {
                    self.scratch
                        .extend_from_slice(&[pair[0], pair[0], pair[0], pair[1]]);
                }
                &self.scratch
            }
            _ => return None,
        };
        let mut bytes = libghostty_vt::alloc::Bytes::new_with_alloc(alloc, rgba.len()).ok()?;
        bytes.copy_from_slice(rgba);
        reader.finish().ok()?;
        Some(libghostty_vt::kitty::graphics::DecodedImage {
            width: info.width,
            height: info.height,
            data: bytes,
        })
    }
}

impl super::GhosttyTerminal {
    /// 上游 Kitty 图像载荷归一化为 RGBA8：Ok(None) 为分片传输待定返回 None；
    /// RGB 直扩 alpha，灰度按亮度展开，PNG 经线程解码器已为 RGBA（按长度兜底）。
    fn kitty_image_to_rgba(
        image: &libghostty_vt::kitty::graphics::Image<'_>,
    ) -> Option<(u32, u32, Vec<u8>)> {
        use libghostty_vt::kitty::graphics::ImageFormat;
        let width = image.width().ok()?;
        let height = image.height().ok()?;
        let bytes = image.data().ok()??;
        let pixel_count = width.checked_mul(height)? as usize;
        let rgba = match image.format().ok()? {
            ImageFormat::Rgba => {
                if bytes.len() != pixel_count.checked_mul(4)? {
                    return None;
                }
                bytes.to_vec()
            }
            ImageFormat::Rgb => {
                if bytes.len() != pixel_count.checked_mul(3)? {
                    return None;
                }
                let (triples, _) = bytes.as_chunks::<3>();
                let mut out = Vec::with_capacity(pixel_count * 4);
                for triple in triples {
                    out.extend_from_slice(&[triple[0], triple[1], triple[2], 255]);
                }
                out
            }
            ImageFormat::Gray => {
                if bytes.len() != pixel_count {
                    return None;
                }
                let mut out = Vec::with_capacity(pixel_count * 4);
                for gray in bytes {
                    out.extend_from_slice(&[*gray, *gray, *gray, 255]);
                }
                out
            }
            ImageFormat::GrayAlpha => {
                if bytes.len() != pixel_count.checked_mul(2)? {
                    return None;
                }
                let (pairs, _) = bytes.as_chunks::<2>();
                let mut out = Vec::with_capacity(pixel_count * 4);
                for pair in pairs {
                    out.extend_from_slice(&[pair[0], pair[0], pair[0], pair[1]]);
                }
                out
            }
            ImageFormat::Png => {
                // 解码器输出 RGBA；按长度兜底 RGB（防御上游行为漂移）。
                if bytes.len() == pixel_count.checked_mul(4)? {
                    bytes.to_vec()
                } else if bytes.len() == pixel_count.checked_mul(3)? {
                    let (triples, _) = bytes.as_chunks::<3>();
                    let mut out = Vec::with_capacity(pixel_count * 4);
                    for triple in triples {
                        out.extend_from_slice(&[triple[0], triple[1], triple[2], 255]);
                    }
                    out
                } else {
                    return None;
                }
            }
            _ => return None,
        };
        Some((width, height, rgba))
    }

    /// 采集视口内全部可见 Kitty 放置（跳过虚拟占位与屏外项，按 z 排序）。
    /// 虚拟放置（unicode 占位符）由字形管线渲染，不进图像通道。
    fn collect_kitty_placements(terminal: &Terminal) -> Vec<KittyPlacementFrame> {
        use libghostty_vt::kitty::graphics::PlacementIterator;
        let Ok(graphics) = terminal.kitty_graphics() else {
            return Vec::new();
        };
        let Ok(mut iterator) = PlacementIterator::new() else {
            return Vec::new();
        };
        let Ok(mut placements) = iterator.update(&graphics) else {
            return Vec::new();
        };
        let mut out = Vec::new();
        while let Some(placement) = placements.next() {
            if placement.is_virtual().unwrap_or(true) {
                continue;
            }
            let Ok(image_id) = placement.image_id() else {
                continue;
            };
            let Some(image) = graphics.image(image_id) else {
                continue;
            };
            let Some((image_width, image_height, image_rgba)) = Self::kitty_image_to_rgba(&image)
            else {
                continue;
            };
            let Ok(info) = placement.placement_render_info(&image, terminal) else {
                continue;
            };
            if !info.viewport_visible
                || info.pixel_width == 0
                || info.pixel_height == 0
                || info.source_width == 0
                || info.source_height == 0
            {
                continue;
            }
            out.push(KittyPlacementFrame {
                image_id,
                viewport_col: info.viewport_col,
                viewport_row: info.viewport_row,
                pixel_width: info.pixel_width,
                pixel_height: info.pixel_height,
                source_x: info.source_x,
                source_y: info.source_y,
                source_width: info.source_width,
                source_height: info.source_height,
                cell_offset_x: placement.x_offset().unwrap_or(0),
                cell_offset_y: placement.y_offset().unwrap_or(0),
                z: placement.z().unwrap_or(0),
                image_width,
                image_height,
                image_rgba,
            });
        }
        out.sort_by_key(|frame| frame.z);
        out
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
        default_foreground: [f32; 4],
        default_background: [f32; 4],
        row_cache: &mut Vec<Vec<CellData>>,
        alt_screen_active: &Arc<AtomicBool>,
    ) -> Option<(Vec<CellData>, CursorInfo)> {
        // Keep the lock-free alternate-screen mirror in sync on every frame
        // the VT thread emits. The Android input path reads this mirror
        // lock-free on every touch-scroll to decide whether to forward the
        // gesture to the remote (Haven research: altScreen wheel consumption).
        alt_screen_active.store(
            terminal
                .active_screen()
                .is_ok_and(|s| s == libghostty_vt::screen::Screen::Alternate),
            Ordering::Release,
        );
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
        // 终端持有选区激活期间禁用行缓存读取：跟踪选区随滚动/输出移动时，
        // 网格未变脏的行也可能改变反白归属，缓存会提供过期高亮。
        // （写入仍更新缓存；清除选区时整缓存失效。）
        let selection_active = terminal
            .selection()
            .map(|selected| selected.is_some())
            .unwrap_or(false);

        while let Some(row) = row_iter_impl.next() {
            let row_idx = current_row as usize;
            let is_dirty = row.dirty().unwrap_or(true);
            // 行级选区范围（无选区时为 None）：每行一次 FFI，避免逐单元格查询。
            let row_selection = if selection_active {
                row.selection().ok().flatten()
            } else {
                None
            };
            // zelland row-cache pattern: clean rows are copied from the
            // cache instead of re-walking their cells (FFI per cell).
            if !is_dirty
                && !selection_active
                && let Some(cached) = row_cache.get(row_idx)
            {
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
            // style/foreground/background once; the flat CellData output is unchanged but
            // the per-cell FFI calls (style/foreground/background) are skipped
            // for the run.
            let mut cached_style_id: Option<libghostty_vt::style::Id> = None;
            let mut cached_foreground = default_foreground;
            let mut cached_background = default_background;
            let mut cached_underline = default_foreground;
            let mut cached_flags = 0u32;

            while let Some(cell) = cell_iter_impl.next() {
                let raw = match cell.raw_cell() {
                    Ok(c) => c,
                    Err(_) => {
                        Self::push_blank_cell(
                            &mut row_data,
                            default_foreground,
                            default_background,
                            current_row,
                            current_col,
                        );
                        current_col += 1;
                        continue;
                    }
                };

                let style_id = raw.style_id().ok();
                let (_style, foreground, background, underline_color, flags) = if style_id.is_some()
                    && style_id == cached_style_id
                {
                    // Same style run: reuse the cached resolved colors.
                    (
                        None,
                        cached_foreground,
                        cached_background,
                        cached_underline,
                        cached_flags,
                    )
                } else {
                    match cell.style() {
                        Ok(s) => {
                            let foreground = Self::cell_color(cell.fg_color(), default_foreground);
                            let background = Self::cell_color(cell.bg_color(), default_background);
                            let underline =
                                Self::resolve_style_color(terminal, &s.underline_color, foreground);
                            let fl = Self::pack_style_flags(&s);
                            cached_style_id = style_id;
                            cached_foreground = foreground;
                            cached_background = background;
                            cached_underline = underline;
                            cached_flags = fl;
                            (Some(s), foreground, background, underline, fl)
                        }
                        Err(_) => {
                            row_data.push(CellData {
                                codepoint: 0,
                                width: 1,
                                grapheme_extra: [0; 7],
                                foreground: default_foreground,
                                background: default_background,
                                underline_color: default_foreground,
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

                // 终端持有选区的行内反白（经典反白：前景背景互换，与覆盖层旧语义一致；
                // 下划线色同步取反白后的前景，保证选中文本的下划线仍可见）。
                let (foreground, background, underline_color) =
                    if row_selection.as_ref().is_some_and(|range| {
                        let col = current_col as u16;
                        col >= range.start_x && col <= range.end_x
                    }) {
                        (background, foreground, background)
                    } else {
                        (foreground, background, underline_color)
                    };

                row_data.push(CellData {
                    codepoint,
                    width,
                    grapheme_extra,
                    foreground,
                    background,
                    underline_color,
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
        let cursor_style = Self::cursor_style_from_snapshot(&snapshot);
        // Cursor position must come from the render-state VIEWPORT
        // coordinates, not the active-screen query: `cursor_y()` returns
        // the row within the ACTIVE area, while CellData rows are viewport
        // rows (0..rows-1 of the currently scrolled viewport). Once the
        // user scrolls scrollback into view the two diverge and the cursor
        // would be matched against the wrong grid row — rendered as the
        // cursor "block" jumping down-right by the scroll amount (reported
        // on real devices as cursor offset of ~1 cell). `cursor_viewport()`
        // returns None when the cursor page is not in the visible viewport
        // (e.g. large scrolls); the cursor must then not be drawn at all.
        let (cursor_row, cursor_col, cursor_visible) = match snapshot.cursor_viewport() {
            Ok(Some(cv)) => (
                cv.y as u32,
                cv.x as u32,
                snapshot.cursor_visible().unwrap_or(true),
            ),
            Ok(None) | Err(_) => (0, 0, false),
        };
        Some((
            data,
            CursorInfo {
                row: cursor_row,
                col: cursor_col,
                visible: cursor_visible,
                style: cursor_style,
                scrollback_length: terminal.scrollback_rows().unwrap_or(0) as u32,
                kitty_generation: terminal
                    .kitty_graphics()
                    .ok()
                    .and_then(|graphics| graphics.generation().ok())
                    .unwrap_or(0),
            },
        ))
    }

    /// Pack style attributes into a bitmask matching `cell.wgsl` shader layout:
    /// Bit 0=bold, 1=italic, 2=reverse, 3=underline,
    /// 5=strikethrough, 6=overline, 7=dim, 8=double_underline
    /// Bits 4,9+ reserved for future use (not read by current shader).
    fn pack_style_flags(style: &libghostty_vt::style::Style) -> u32 {
        use crate::terminal::ghostty_terminal::cell_flags;
        let mut flags = 0u32;
        if style.bold {
            flags |= 1 << cell_flags::BOLD;
        }
        if style.italic {
            flags |= 1 << cell_flags::ITALIC;
        }
        if style.inverse {
            flags |= 1 << cell_flags::REVERSE;
        }
        if matches!(
            style.underline,
            libghostty_vt::style::Underline::Single
                | libghostty_vt::style::Underline::Double
                | libghostty_vt::style::Underline::Curly
                | libghostty_vt::style::Underline::Dashed
                | libghostty_vt::style::Underline::Dotted
        ) {
            flags |= 1 << cell_flags::UNDERLINE;
        }
        if style.strikethrough {
            flags |= 1 << cell_flags::STRIKETHROUGH;
        }
        if style.overline {
            flags |= 1 << cell_flags::OVERLINE;
        }
        if style.faint {
            flags |= 1 << cell_flags::FAINT;
        }
        if style.underline == libghostty_vt::style::Underline::Double {
            flags |= 1 << cell_flags::DOUBLE_UNDERLINE;
        }
        flags
    }

    pub(crate) fn build_snapshot(
        terminal: &Terminal,
        default_foreground: [f32; 4],
        default_background: [f32; 4],
        _palette: &[[u8; 3]; 16],
        scroll_offset: u32,
    ) -> GridSnapshot {
        // NOTE: a scrolled snapshot returns an EMPTY fallback
        // grid — the CellData path does not expose scrollback content, and
        // `take_snapshot_with_scroll` is only exercised by tests. Any
        // future query caller passing a non-zero offset will get an
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
                            foreground: default_foreground,
                            background: default_background,
                            ..CellSnapshot::default()
                        });
                        continue;
                    }
                };

                let style = match cell.style() {
                    Ok(s) => s,
                    Err(_) => {
                        cells.push(CellSnapshot {
                            foreground: default_foreground,
                            background: default_background,
                            ..CellSnapshot::default()
                        });
                        continue;
                    }
                };

                let codepoint = raw.codepoint().unwrap_or(0);

                // 快照路径保留占位：与 CellData 跳过 Spacer 不同，快照按网格索引逐格存放，宽字符占位格须保留 width=1 条目以对齐行列。仅测试行使。
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

                let foreground = Self::cell_color(cell.fg_color(), default_foreground);
                let background = Self::cell_color(cell.bg_color(), default_background);
                let underline_color =
                    Self::resolve_style_color(terminal, &style.underline_color, foreground);

                cells.push(CellSnapshot {
                    codepoint,
                    graphemes,
                    foreground,
                    background,
                    underline_color,
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
                    overline: style.overline,
                    double_underline: style.underline == libghostty_vt::style::Underline::Double,
                    width,
                });
            }
        }

        // Viewport-relative cursor coordinates (matches the cell rows above,
        // which are viewport rows): the active-screen `cursor_y` diverges
        // from the viewport once scrollback is scrolled into view.
        let (cursor_row, cursor_col, cursor_visible) = match snapshot.cursor_viewport() {
            Ok(Some(cv)) => (
                cv.y as u32,
                cv.x as u32,
                snapshot.cursor_visible().unwrap_or(true),
            ),
            Ok(None) | Err(_) => (0, 0, false),
        };

        let dirty = vec![true; (rows as usize) * (cols as usize)];

        let sync_active = terminal.mode(Mode::SYNC_OUTPUT).unwrap_or(false);

        GridSnapshot {
            rows,
            cols,
            cursor_row,
            cursor_col,
            cursor_visible,
            cursor_style: Self::cursor_style_from_snapshot(&snapshot),
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
    /// Coordinates are absolute grid rows (0 = top of history; the viewport
    /// starts at `scrollback_rows`). History rows resolve via Point::History
    /// and viewport rows via Point::Viewport (equivalent to Point::Screen
    /// with an absolute y). Returns an empty string for an invalid selection.
    pub(crate) fn selection_text_impl(
        terminal: &Terminal,
        start: (u32, u32),
        end: (u32, u32),
        rectangle: bool,
    ) -> String {
        let cols = terminal.cols().unwrap_or(80).max(1) as u32;
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

    /// 将视图坐标的选区安装为终端持有的活动选区（跟踪引用）。
    /// 与 selection_text_impl 同一坐标系：绝对网格行（0 = 回滚顶部）。
    /// 无效端点时静默忽略（调用方已标记脏并重推，保持帧一致）。
    pub(crate) fn install_selection_impl(
        terminal: &Terminal,
        start: (u32, u32),
        end: (u32, u32),
        rectangle: bool,
    ) {
        let cols = terminal.cols().unwrap_or(80).max(1) as u32;
        let scrollback_rows = terminal.scrollback_rows().unwrap_or(0) as u32;
        let resolve = |row: u32, col: u32| {
            let clamped = col.min(cols - 1) as u16;
            if row < scrollback_rows {
                Point::History(PointCoordinate { x: clamped, y: row })
            } else {
                Point::Viewport(PointCoordinate {
                    x: clamped,
                    y: row - scrollback_rows,
                })
            }
        };
        let (Ok(start_ref), Ok(end_ref)) = (
            terminal.grid_ref(resolve(start.0, start.1)),
            terminal.grid_ref(resolve(end.0, end.1)),
        ) else {
            return;
        };
        let selection = libghostty_vt::selection::Selection::new(start_ref, end_ref, rectangle);
        if terminal.set_selection(Some(&selection)).is_err() {
            log::warn!("ghostty_terminal: install selection failed");
        }
    }

    /// Query the OSC 8 hyperlink URI at a grid cell (termux TerminalView
    /// openLinkAt equivalent; ghostty cell.has_hyperlink + hyperlink_uri).
    pub(crate) fn hyperlink_at_impl(terminal: &Terminal, row: u32, col: u32) -> Option<String> {
        let cols = terminal.cols().unwrap_or(80) as u32;
        let total_rows = terminal.total_rows().unwrap_or(0) as u32;
        if col >= cols || row >= total_rows {
            return None;
        }
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
        // 超长 URI（>4KiB）按上游 OutOfSpace{required} 重试，避免截断长链接。
        let mut buf = [0u8; 4096];
        match grid_ref.hyperlink_uri(&mut buf) {
            Ok(0) => None,
            Ok(len) => Some(String::from_utf8_lossy(&buf[..len]).into_owned()),
            Err(libghostty_vt::error::Error::OutOfSpace { required }) => {
                let capped = required.min(64 * 1024);
                if capped == 0 {
                    return None;
                }
                let mut grown = vec![0u8; capped];
                let grown_len = grid_ref.hyperlink_uri(&mut grown).ok()?;
                if grown_len == 0 {
                    return None;
                }
                Some(String::from_utf8_lossy(&grown[..grown_len]).into_owned())
            }
            Err(_) => None,
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
                && let Some(byte_offset) = line.find(query)
            {
                // SearchMatch 列为字符列而非字节偏移，CJK 行须转换以免高亮错位。
                let column = line[..byte_offset].chars().count() as u32;
                return Some((row, column));
            }
        }
        None
    }

    pub(crate) fn search_in_scrollback_all_impl(
        terminal: &Terminal,
        query: &str,
        case_sensitive: bool,
    ) -> Vec<SearchMatch> {
        if query.is_empty() {
            return vec![];
        }
        let total = terminal.total_rows().unwrap_or(0) as u32;
        let mut results = Vec::new();
        let search_query = if case_sensitive {
            query.to_string()
        } else {
            // 大小写不敏感路径在小写空间匹配，无 Unicode 规范化：
            // 小写展开字符（如 U+0130）后方列可能漂移，组合/分解形式直接 miss。
            // 主流 ASCII/CJK 场景不受影响，复杂场景按规范不处理。
            query.to_lowercase()
        };
        for row in 0..total {
            if let Some(line) = Self::read_line_text_impl(terminal, row) {
                let search_line = if case_sensitive {
                    line.clone()
                } else {
                    line.to_lowercase()
                };
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
                    // Advance past this match (its end is always a char
                    // boundary): adjacent matches are still found,
                    // overlapping matches are not reported.
                    let mut next = abs_col + search_query.len();
                    while next < search_line.len() && !search_line.is_char_boundary(next) {
                        next += 1;
                    }
                    start = next;
                }
            }
        }
        results
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::terminal::ghostty_terminal::GhosttyTerminal;
    use libghostty_vt::style::{Style, StyleColor, Underline};

    #[test]
    fn byte_to_float_scales_255() {
        assert_eq!(GhosttyTerminal::byte_to_float(0), 0.0);
        assert_eq!(GhosttyTerminal::byte_to_float(255), 1.0);
        assert!((GhosttyTerminal::byte_to_float(128) - 128.0 / 255.0).abs() < 1e-6);
    }

    #[test]
    fn byte_color_to_float_includes_opaque_alpha() {
        let color = GhosttyTerminal::byte_color_to_float([13, 188, 121]);
        assert!((color[0] - 13.0 / 255.0).abs() < 1e-6);
        assert!((color[1] - 188.0 / 255.0).abs() < 1e-6);
        assert!((color[2] - 121.0 / 255.0).abs() < 1e-6);
        assert_eq!(color[3], 1.0, "alpha must be opaque");
    }

    /// OSC 4 调色板覆盖必须生效：有效调色板（含覆盖）而非静态表决定渲染色。
    #[test]
    fn osc4_palette_override_reaches_dumped_grid() {
        let mut terminal = GhosttyTerminal::new(5, 20, 100).expect("terminal");
        // 调色板索引 1 改为纯绿，再以红色（索引 1）写字：看到的必须是绿色。
        terminal.vt_write(b"\x1b]4;1;#00ff00\x07\x1b[31mX");
        terminal.flush();
        let dumped = terminal.dump_grid();
        let cell = &dumped.visible[0];
        assert_eq!(cell.codepoint, 'X' as u32);
        assert_eq!(
            cell.foreground,
            GhosttyTerminal::byte_color_to_float([0, 255, 0]),
            "OSC 4 override must win over the static palette"
        );
    }

    /// SGR 58 下划线色进入快照（未设置时回退前景）。
    #[test]
    fn sgr58_underline_color_reaches_dumped_grid() {
        let mut terminal = GhosttyTerminal::new(5, 20, 100).expect("terminal");
        terminal.vt_write(b"\x1b[4m\x1b[58;2;255;0;0mU");
        terminal.flush();
        let dumped = terminal.dump_grid();
        let cell = &dumped.visible[0];
        assert!(cell.underline, "SGR 4 must set underline");
        assert_eq!(
            cell.underline_color,
            GhosttyTerminal::byte_color_to_float([255, 0, 0]),
            "SGR 58 must set the underline color"
        );
    }

    #[test]
    fn underline_color_falls_back_to_foreground() {
        let mut terminal = GhosttyTerminal::new(5, 20, 100).expect("terminal");
        terminal.vt_write(b"\x1b[4m\x1b[31mV");
        terminal.flush();
        let dumped = terminal.dump_grid();
        let cell = &dumped.visible[0];
        assert!(cell.underline);
        assert_eq!(
            cell.underline_color, cell.foreground,
            "unset SGR 58 must fall back to the resolved foreground"
        );
    }

    /// SGR 53 上划线进入快照（与 SGR 58 用例对称的端到端覆盖）。
    #[test]
    fn sgr53_overline_reaches_dumped_grid() {
        let mut terminal = GhosttyTerminal::new(5, 20, 100).expect("terminal");
        terminal.vt_write(b"\x1b[53mO");
        terminal.flush();
        let dumped = terminal.dump_grid();
        let cell = &dumped.visible[0];
        assert!(cell.overline, "SGR 53 must set overline");
    }

    /// SGR 3 斜体进入快照：斜体缺失首先排除 VT 层，回应对“斜体无法显示”。
    #[test]
    fn sgr3_italic_reaches_dumped_grid() {
        let mut terminal = GhosttyTerminal::new(5, 20, 100).expect("terminal");
        terminal.vt_write(b"\x1b[3mI");
        terminal.flush();
        let dumped = terminal.dump_grid();
        let cell = &dumped.visible[0];
        assert_eq!(cell.codepoint, 'I' as u32);
        assert!(cell.italic, "SGR 3 must set italic");
    }

    /// SGR 31 红色前景进入快照：颜色缺失首先排除 VT 层，回应对“颜色只显示背景”。
    #[test]
    fn sgr31_red_foreground_reaches_dumped_grid() {
        let mut terminal = GhosttyTerminal::new(5, 20, 100).expect("terminal");
        terminal.vt_write(b"\x1b[31mR");
        terminal.flush();
        let dumped = terminal.dump_grid();
        let cell = &dumped.visible[0];
        assert_eq!(cell.codepoint, 'R' as u32);
        // 默认主题调色板索引 1 为 Catppuccin 红，非纯红：断言有效调色板色即证明链路。
        assert_eq!(
            cell.foreground,
            GhosttyTerminal::byte_color_to_float([243, 139, 168]),
            "SGR 31 must resolve to palette index 1"
        );
    }

    fn style_with_flags() -> Style {
        Style {
            bold: true,
            italic: true,
            faint: true,
            blink: false,
            inverse: true,
            invisible: false,
            strikethrough: true,
            overline: true,
            underline: Underline::Double,
            fg_color: StyleColor::None,
            bg_color: StyleColor::None,
            underline_color: StyleColor::None,
        }
    }

    #[test]
    fn pack_style_flags_sets_expected_bits() {
        let flags = GhosttyTerminal::pack_style_flags(&style_with_flags());
        assert_ne!(flags & (1 << cell_flags::BOLD), 0, "bold bit");
        assert_ne!(flags & (1 << cell_flags::ITALIC), 0, "italic bit");
        assert_ne!(flags & (1 << cell_flags::REVERSE), 0, "reverse bit");
        assert_ne!(flags & (1 << cell_flags::UNDERLINE), 0, "underline bit");
        assert_ne!(
            flags & (1 << cell_flags::DOUBLE_UNDERLINE),
            0,
            "double underline bit"
        );
        assert_ne!(
            flags & (1 << cell_flags::STRIKETHROUGH),
            0,
            "strikethrough bit"
        );
        assert_ne!(flags & (1 << cell_flags::OVERLINE), 0, "overline bit");
        assert_ne!(flags & (1 << cell_flags::FAINT), 0, "faint bit");
    }

    #[test]
    fn pack_style_flags_double_underline_sets_underline_too() {
        // Double underline must also set the plain underline bit so the
        // shader's highlight path treats the cell as underlined.
        let flags = GhosttyTerminal::pack_style_flags(&style_with_flags());
        let both = (1 << cell_flags::UNDERLINE) | (1 << cell_flags::DOUBLE_UNDERLINE);
        assert_eq!(flags & both, both);
    }

    #[test]
    fn pack_style_flags_default_style_is_zero() {
        let flags = GhosttyTerminal::pack_style_flags(&Style::default());
        assert_eq!(flags, 0);
    }

    #[test]
    fn pack_style_flags_plain_underline_sets_only_underline_bit() {
        let style = Style {
            underline: Underline::Single,
            ..Default::default()
        };
        let flags = GhosttyTerminal::pack_style_flags(&style);
        assert_eq!(flags, 1 << cell_flags::UNDERLINE);
    }
}
