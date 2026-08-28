use std::sync::atomic::{AtomicBool, AtomicU64};
use std::sync::{Arc, Mutex};

use flume::{Receiver, Sender};

use super::types::*;

/// Commands sent from the caller to the VT thread, processed in order.
///
/// The channel is **bounded** so a wedged VT thread cannot grow memory
/// unboundedly; senders use `try_send` and fall back to a cached value.
///
/// Stateless queries belong in [`Query`] (the unbounded `query_tx`
/// channel, drained by the VT thread between commands) — the two channels
/// exist so action ordering and query latency are independent.
pub enum Command {
    /// Write raw bytes to the PTY (keyboard input, paste).
    Write(Vec<u8>),
    /// Request a flush acknowledgment from the render thread.
    FlushAck(Sender<()>),
    /// Update the terminal color theme.
    SetTheme {
        background: [u8; 3],
        foreground: [u8; 3],
        ansi: [[u8; 3]; 16],
    },
    Resize {
        rows: u32,
        cols: u32,
    },
    TakeSnapshot {
        tx: Sender<Arc<GridSnapshot>>,
        scroll_offset: u32,
    },
    /// Scroll the terminal viewport by a delta (up is negative). Backs
    /// the app's scrollback browsing: previously a Kotlin-side
    /// no-op — the CellData render path had no scroll support at all).
    ScrollViewport(isize),
    /// Graceful shutdown signal.
    Terminate,
}

/// Stateless queries answered by the VT thread's `process_query` handler.
///
/// Sent on the `query_tx` channel (bounded 256, `try_send` + fallback — a
/// burst of queries, e.g. a settings panel reading every mode, never blocks
/// the caller and never delays ordered actions on the Write/Resize command
/// channel). The VT thread drains this channel after every command and on
/// its idle timeout.
///
/// Ordering note: queries are drained between commands and on the idle
/// timeout, so a query may be answered before an earlier-issued action
/// (Write/Resize) is processed — the two channels have no cross-channel
/// ordering guarantee. Actions that must observe prior state mutations use
/// the command channel (e.g. [`crate::terminal::ghostty_terminal::Command::TakeSnapshot`]).
pub enum Query {
    Rows(Sender<u32>),
    Cols(Sender<u32>),
    CursorX(Sender<u32>),
    CursorY(Sender<u32>),
    CursorVisible(Sender<bool>),
    OriginMode(Sender<bool>),
    Autowrap(Sender<bool>),
    AltScreen(Sender<bool>),
    Title(Sender<String>),
    Cwd(Sender<String>),
    ModeGet(u16, u8, Sender<bool>),
    ScrollbackLength(Sender<u32>),
    ReadLineText {
        row: u32,
        tx: Sender<Option<String>>,
    },
    /// Cursor viewport (row, col) read through `build_cell_data` — the EXACT
    /// source the render thread consumes.  observability: lets the
    /// instrumentation layer assert the same coordinates the GPU draws.
    /// None when the cursor is hidden or the build fails.
    RenderCursor(Sender<Option<(u32, u32)>>),
    ReadVisibleText(Sender<String>),
    /// Extract selection text with Ghostty's native formatter: soft-wrapped
    /// lines are unwrapped (joined without '\n') and trailing whitespace is
    /// trimmed — the same wrap-aware semantics as termux-app's
    /// TerminalBuffer.getSelectedText (joinBackLines). Column endpoints are
    /// grid columns; the formatter maps columns to char indices internally
    /// (wide-char safe), matching TerminalRow.findStartOfColumn.
    SelectionText {
        /// Grid rows (absolute: scrollback rows are negative offsets in
        /// ghostty semantics — callers pass Point::Screen coordinates).
        start: (u32, u32),
        end: (u32, u32),
        rectangle: bool,
        tx: Sender<String>,
    },
    /// Query the OSC 8 hyperlink URI at a grid cell, if any (termux
    /// TerminalView openLinkAt equivalent).
    HyperlinkAt {
        row: u32,
        col: u32,
        tx: Sender<Option<String>>,
    },
    SearchInScrollback {
        query: String,
        tx: Sender<Option<(u32, u32)>>,
    },
    SearchInScrollbackAll {
        query: String,
        case_sensitive: bool,
        fuzzy: bool,
        tx: Sender<Vec<SearchMatch>>,
    },
    DumpGrid {
        tx: Sender<DumpedGrid>,
    },
    TakeKittyGraphicsImage {
        id: u32,
        tx: Sender<Option<KittyGraphicsImageData>>,
    },
    KeyEncode {
        key_code: u32,
        modifiers: u16,
        action: u8,
        unicode_char: u32,
        unshifted_char: u32,
        tx: Sender<Vec<u8>>,
    },
    /// Encode a mouse event into terminal escape sequences using the
    /// Ghostty mouse encoder (SGR/X10/UTF-8 per terminal state).
    /// `position` is in surface pixels; `cell_w`/`cell_h` are the live
    /// cell dimensions (pixels) so the encoder maps pixel→cell correctly.
    /// Returns an empty Vec when mouse reporting is disabled or encoding
    /// fails (zelland renderer/mod.rs pattern: mouse events are dropped
    /// when the application has not enabled a tracking mode).
    EncodeMouseEvent {
        position: (f32, f32),
        action: u8,
        button: u8,
        cell_w: f32,
        cell_h: f32,
        tx: Sender<Vec<u8>>,
    },
}

pub(crate) struct SnapshotCache {
    pub(crate) cached: Arc<GridSnapshot>,
    pub(crate) pending_rx: Option<Receiver<Arc<GridSnapshot>>>,
    pub(crate) initialized: bool,
}

pub(crate) struct RunConfig {
    pub(crate) command_receiver: Receiver<Command>,
    pub(crate) query_receiver: Receiver<Query>,
    pub(crate) rows: u32,
    pub(crate) cols: u32,
    pub(crate) scrollback_lines: u32,
    pub(crate) background_color: [u8; 3],
    pub(crate) foreground_color: [u8; 3],
    pub(crate) ansi_colors: [[u8; 3]; 16],
    pub(crate) response_buffer: Arc<Mutex<Vec<Vec<u8>>>>,
    pub(crate) snapshot_rebuild_count: Arc<AtomicU64>,
    /// Mirror of the alternate-screen state, updated lock-free by the VT
    /// thread on every `Query::AltScreen` so the input path can detect it
    /// without a blocking RPC.
    pub(crate) alt_screen_active: Arc<AtomicBool>,
    /// Optional channel for auto-pushing CellData after each frame update.
    /// When set, the ghostty thread will automatically build and send
    /// Vec<CellData> (via CellIterator) whenever the grid changes.
    /// This is the data path for the new thread-split architecture:
    ///   Session thread → Vec<CellData> → Render thread
    pub(crate) cell_data_tx: Option<flume::Sender<(Vec<CellData>, CursorInfo)>>,
}
