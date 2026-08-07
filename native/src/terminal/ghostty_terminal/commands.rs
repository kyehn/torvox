use std::sync::atomic::AtomicU64;
use std::sync::{Arc, Mutex};

use flume::{Receiver, Sender};

use super::types::*;

/// Commands sent from the VT parser thread to the render thread.
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
    /// the app's scrollback browsing (round-205: previously a Kotlin-side
    /// no-op — the CellData render path had no scroll support at all).
    ScrollViewport(isize),
    ScrollbackLength(Sender<u32>),
    ReadLineText {
        row: u32,
        tx: Sender<Option<String>>,
    },
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
    /// Graceful shutdown signal.
    Terminate,
}

pub(crate) struct SnapshotCache {
    pub(crate) cached: Arc<GridSnapshot>,
    pub(crate) pending_rx: Option<Receiver<Arc<GridSnapshot>>>,
    pub(crate) initialized: bool,
}

pub(crate) struct RunConfig {
    pub(crate) command_receiver: Receiver<Command>,
    pub(crate) query_receiver: Receiver<Command>,
    pub(crate) rows: u32,
    pub(crate) cols: u32,
    pub(crate) scrollback_lines: u32,
    pub(crate) background_color: [u8; 3],
    pub(crate) foreground_color: [u8; 3],
    pub(crate) ansi_colors: [[u8; 3]; 16],
    pub(crate) response_buffer: Arc<Mutex<Vec<Vec<u8>>>>,
    pub(crate) snapshot_rebuild_count: Arc<AtomicU64>,
    /// Optional channel for auto-pushing CellData after each frame update.
    /// When set, the ghostty thread will automatically build and send
    /// Vec<CellData> (via CellIterator) whenever the grid changes.
    /// This is the data path for the new thread-split architecture:
    ///   Session thread → Vec<CellData> → Render thread
    pub(crate) cell_data_tx: Option<flume::Sender<(Vec<CellData>, CursorInfo)>>,
}
