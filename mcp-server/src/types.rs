//! MCP protocol types — error, request/response, session info, commands.
//!
//! # Requirements
//! - FR-044 — MCP server protocol types

use flume::Sender;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use terminal_core::cell::Cell;
use thiserror::Error;

/// Errors produced by the MCP server.
#[derive(Error, Debug)]
pub enum McpError {
    #[error("invalid JSON-RPC request: {0}")]
    InvalidRequest(String),
    #[error("unknown method: {0}")]
    UnknownMethod(String),
    #[error("invalid parameters: {0}")]
    InvalidParams(String),
    #[error("session not found: {0}")]
    SessionNotFound(u32),
    #[error("internal error: {0}")]
    Internal(String),
}

impl McpError {
    pub fn to_json_rpc_error(&self, id: &Value) -> Value {
        let (code, message) = match self {
            Self::InvalidRequest(m) => (-32600, m.clone()),
            Self::UnknownMethod(m) => (-32601, m.clone()),
            Self::InvalidParams(m) => (-32602, m.clone()),
            Self::SessionNotFound(_) => (-32001, self.to_string()),
            Self::Internal(m) => (-32603, m.clone()),
        };
        json!({
            "jsonrpc": "2.0",
            "id": id,
            "error": { "code": code, "message": message },
        })
    }
}

/// A session handle exposed via MCP. Real session is owned by the GUI;
/// this is a snapshot/proxy view.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SessionInfo {
    pub id: u32,
    pub title: String,
    pub rows: u32,
    pub cols: u32,
    pub shell: String,
    pub pid: Option<u32>,
    pub is_exited: bool,
}

/// A grid snapshot for MCP consumers. Only the text content + attributes,
/// not the full Cell binary form.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GridCellData {
    pub row: u32,
    pub col: u32,
    pub codepoint: u32,
    pub fg_r: u8,
    pub fg_g: u8,
    pub fg_b: u8,
    pub bg_r: u8,
    pub bg_g: u8,
    pub bg_b: u8,
    pub bold: bool,
    pub italic: bool,
    pub underline: bool,
    pub reverse: bool,
    pub dim: bool,
    pub strikethrough: bool,
    pub blink: bool,
    pub hidden: bool,
}

/// A serialized snapshot of the terminal grid for MCP client consumption.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GridSnapshotData {
    pub rows: u32,
    pub cols: u32,
    pub cells: Vec<GridCellData>,
    pub cursor_row: u32,
    pub cursor_col: u32,
    pub cursor_visible: bool,
}

impl From<&Cell> for GridCellData {
    fn from(c: &Cell) -> Self {
        Self {
            row: 0,
            col: 0,
            codepoint: c.char as u32,
            fg_r: c.foreground.r,
            fg_g: c.foreground.g,
            fg_b: c.foreground.b,
            bg_r: c.background.r,
            bg_g: c.background.g,
            bg_b: c.background.b,
            bold: c.attrs.bold,
            italic: c.attrs.italic,
            underline: c.attrs.underline,
            reverse: c.attrs.reverse,
            dim: c.attrs.dim,
            strikethrough: c.attrs.strikethrough,
            blink: c.attrs.blink,
            hidden: c.attrs.hidden,
        }
    }
}

/// Request from a client to perform a session read.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum ReadRequest {
    Sessions,
    Grid {
        session_id: u32,
    },
    Scrollback {
        session_id: u32,
        max_lines: u32,
    },
    Cursor {
        session_id: u32,
    },
    Selection {
        session_id: u32,
    },
    Title {
        session_id: u32,
    },
    ScrollbackSearch {
        session_id: u32,
        pattern: String,
        max_matches: u32,
    },
    TerminalSize {
        session_id: u32,
    },
    ListDirectory {
        path: String,
    },
    ReadFile {
        path: String,
        max_lines: u32,
    },
    ReadClipboard,
}

/// Response to a read request.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum ReadResponse {
    Sessions(Vec<SessionInfo>),
    Grid(GridSnapshotData),
    Scrollback(Vec<String>),
    Cursor {
        row: u32,
        col: u32,
        visible: bool,
    },
    Selection(Option<String>),
    Title(String),
    SearchMatches(Vec<SearchMatch>),
    TerminalSize {
        rows: u32,
        cols: u32,
    },
    DirectoryEntries(Vec<DirEntry>),
    FileContent {
        lines: Vec<String>,
        total_lines: u32,
        truncated: bool,
    },
    ClipboardContent(String),
}

/// A single search result from terminal scrollback.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SearchMatch {
    pub line_number: u32,
    pub text: String,
    pub start_col: u32,
    pub end_col: u32,
}

/// A file or directory entry from the filesystem.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DirEntry {
    pub name: String,
    pub is_dir: bool,
    pub size: Option<u64>,
    pub modified: Option<String>,
}

/// Commands sent from the MCP server to the GUI's session manager.
#[derive(Debug)]
pub enum McpCommand {
    Read(ReadRequest, Sender<Result<ReadResponse, String>>),
    Write {
        session_id: u32,
        data: Vec<u8>,
        reply: Sender<Result<(), String>>,
    },
    Signal {
        session_id: u32,
        signal: SignalKind,
        reply: Sender<Result<(), String>>,
    },
    SetTerminalSize {
        session_id: u32,
        rows: u32,
        cols: u32,
        reply: Sender<Result<(), String>>,
    },
    WriteClipboard {
        text: String,
        reply: Sender<Result<(), String>>,
    },
    RaiseNotification {
        title: String,
        body: String,
        reply: Sender<Result<(), String>>,
    },
}

/// Unix signal types that can be sent to a session's child process.
#[derive(Copy, Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum SignalKind {
    /// SIGINT — interrupt (Ctrl-C).
    Interrupt,
    /// SIGTERM — terminate (graceful shutdown).
    Terminate,
    /// SIGHUP — hangup (terminal closed).
    Hangup,
    /// SIGQUIT — quit (core dump).
    Quit,
}

/// Server-side abstraction over session storage. The real implementation
/// (in the GUI) owns the actual sessions; this is a trait for testability.
pub trait SessionStore: Send + Sync {
    fn read(&self, req: ReadRequest) -> Result<ReadResponse, String>;
    fn write(&self, session_id: u32, data: Vec<u8>) -> Result<(), String>;
    fn signal(&self, session_id: u32, signal_kind: SignalKind) -> Result<(), String>;
    fn set_terminal_size(&self, _session_id: u32, _rows: u32, _cols: u32) -> Result<(), String> {
        Err("set_terminal_size not supported".into())
    }
    fn write_clipboard(&self, _text: &str) -> Result<(), String> {
        Err("write_clipboard not supported".into())
    }
    fn read_clipboard(&self) -> Result<String, String> {
        Err("read_clipboard not supported".into())
    }
    fn raise_notification(&self, _title: &str, _body: &str) -> Result<(), String> {
        Err("raise_notification not supported".into())
    }
    fn scroll_terminal(&self, _session_id: u32, _lines: i32) -> Result<i32, String> {
        Err("scroll_terminal not supported".into())
    }
    fn feed_terminal_output(&self, _session_id: u32, _text: &str) -> Result<(), String> {
        Err("feed_terminal_output not supported".into())
    }
    fn read_scrollback_tail(
        &self,
        _session_id: u32,
        _max_lines: usize,
    ) -> Result<Vec<String>, String> {
        Err("read_scrollback_tail not supported".into())
    }
}

/// JSON-RPC 2.0 request envelope.
#[derive(Clone, Debug, Deserialize)]
pub struct JsonRpcRequest {
    /// JSON-RPC protocol version string. Must be "2.0".
    pub jsonrpc: String,
    /// Name of the method to invoke on the server.
    pub method: String,
    /// Parameters for the method call.
    #[serde(default)]
    pub params: Value,
    /// Request identifier used to correlate responses.
    ///
    /// Absent (deserializes to `Value::Null`) for notifications, which must
    /// not elicit a response per JSON-RPC 2.0.
    #[serde(default)]
    pub id: Value,
}

/// JSON-RPC 2.0 response envelope.
#[derive(Clone, Debug, Serialize)]
pub struct JsonRpcResponse {
    /// JSON-RPC protocol version string.
    pub jsonrpc: &'static str,
    /// Request identifier matching the originating request.
    pub id: Value,
    /// Result of the method invocation.
    pub result: Value,
}
