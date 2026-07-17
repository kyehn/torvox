use std::sync::atomic::AtomicU64;
use std::sync::{Arc, Mutex};

use flume::{Receiver, Sender};

use super::types::*;

pub enum Command {
    Write(Vec<u8>),
    FlushAck(Sender<()>),
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
        tx: Sender<GridSnapshot>,
        scroll_offset: u32,
    },
    ScrollbackLength(Sender<u32>),
    ReadLineText {
        row: u32,
        tx: Sender<Option<String>>,
    },
    ReadVisibleText(Sender<String>),
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
    TakeKgpImage {
        id: u32,
        tx: Sender<Option<KgpImageData>>,
    },
    KeyEncode {
        key_code: u32,
        modifiers: u16,
        action: u8,
        unicode_char: u32,
        unshifted_char: u32,
        tx: Sender<Vec<u8>>,
    },
    Terminate,
}

pub(crate) struct SnapshotCache {
    pub(crate) cached: GridSnapshot,
    pub(crate) pending_rx: Option<Receiver<GridSnapshot>>,
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
}
