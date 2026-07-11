//! Model Context Protocol (MCP) server for Torvox.
//!
//! Implements a JSON-RPC 2.0 server over Unix domain sockets, exposing
//! terminal session state to AI agents for inspection and structured interaction.
//!
//! ## Architecture
//!
//! ```text
//! AI Agent  <--stdio/JSON-RPC-->  torvox-mcp  <--Unix socket-->  torvox-gui-android
//! ```
//!
//! ## Wire protocol
//!
//! JSON-RPC 2.0 over newline-delimited JSON. Each line is a complete
//! request, response, or notification.
//!
//! ## Tools exposed
//!
//! - `list_sessions`: list all active terminal sessions
//! - `read_grid`: read current grid state of a session (rows × cols)
//! - `read_scrollback`: read last N lines of scrollback
//! - `read_cursor`: read cursor position and visibility
//! - `read_selection`: read selected text (if any)
//! - `read_title`: read session title (OSC 0/2)
//! - `send_input`: write text to PTY (requires write consent)
//! - `send_signal`: send signal to child process (SIGINT/SIGTERM/SIGHUP/SIGQUIT)

#![forbid(unsafe_code)]

use std::collections::BTreeMap;
use std::io::{BufRead, Write};
use std::path::Path;
use std::sync::Arc;

use flume::Sender;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use thiserror::Error;
use torvox_core::cell::Cell;

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

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SearchMatch {
    pub line_number: u32,
    pub text: String,
    pub start_col: u32,
    pub end_col: u32,
}

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

#[derive(Copy, Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum SignalKind {
    Interrupt,
    Terminate,
    Hangup,
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

use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};

/// JSON-RPC 2.0 request envelope.
#[derive(Clone, Debug, Deserialize)]
pub struct JsonRpcRequest {
    /// JSON-RPC protocol version string.
    pub jsonrpc: String,
    /// Name of the method to invoke on the server.
    pub method: String,
    /// Parameters for the method call.
    #[serde(default)]
    pub params: Value,
    /// Request identifier used to correlate responses.
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

/// Prompt-matching input queue for AI agents (inspired by Haven #161).
///
/// Watches scrollback for a configurable prompt pattern and injects
/// queued text when the pattern appears. Useful for driving interactive
/// REPLs, scripts with prompts, or automated testing.
#[derive(Clone)]
pub struct InputQueue {
    entries: Arc<Mutex<HashMap<String, QueuedEntry>>>,
}

#[derive(Clone)]
struct QueuedEntry {
    /// Unique identifier for this queued entry.
    entry_id: String,
    /// Session ID to monitor for the prompt pattern.
    session_id: u32,
    /// Text to inject when the prompt pattern matches.
    text: String,
    /// Key sequence to send after the text (e.g., Enter).
    submit_key: String,
    /// Regex pattern to watch for in scrollback output.
    prompt_pattern: String,
    /// Instant after which this entry expires.
    deadline: Instant,
}

impl Default for InputQueue {
    fn default() -> Self {
        Self::new()
    }
}

impl InputQueue {
    pub fn new() -> Self {
        Self {
            entries: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub fn enqueue(
        &self,
        session_id: u32,
        text: String,
        submit_key: String,
        prompt_pattern: String,
        timeout_seconds: u32,
    ) -> String {
        let entry_id = format!(
            "q-{}-{}",
            session_id,
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis()
        );
        let entry = QueuedEntry {
            entry_id: entry_id.clone(),
            session_id,
            text,
            submit_key,
            prompt_pattern,
            deadline: Instant::now() + Duration::from_secs(timeout_seconds.into()),
        };
        self.entries
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .insert(entry_id.clone(), entry);
        entry_id
    }

    pub fn cancel(&self, entry_id: &str) -> bool {
        self.entries
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .remove(entry_id)
            .is_some()
    }

    pub fn pending(&self) -> Vec<Value> {
        self.entries
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .values()
            .map(|entry| {
                json!({
                    "entry_id": entry.entry_id,
                    "session_id": entry.session_id,
                    "text_preview": if entry.text.len() > 40 { format!("{}…", &entry.text[..37]) } else { entry.text.clone() },
                    "prompt_pattern": entry.prompt_pattern,
                    "seconds_remaining": entry.deadline.saturating_duration_since(Instant::now()).as_secs(),
                })
            })
            .collect()
    }

    pub fn check_and_deliver(&self, store: &Arc<dyn SessionStore>, write_consent: bool) {
        if !write_consent {
            return;
        }
        let now = Instant::now();
        let mut to_remove = Vec::new();

        for (entry_id, entry) in self
            .entries
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .iter()
        {
            if now > entry.deadline {
                to_remove.push(entry_id.clone());
                continue;
            }

            let scrollback = match store.read_scrollback_tail(entry.session_id, 20) {
                Ok(lines) => lines.join("\n"),
                Err(_) => continue,
            };

            if scrollback.contains(&entry.prompt_pattern) {
                let data = format!("{}{}", entry.text, entry.submit_key);
                if let Err(error) = store.write(entry.session_id, data.into_bytes()) {
                    log::error!(
                        "mcp: failed to write shell entry for session {}: {}",
                        entry.session_id,
                        error
                    );
                }
                to_remove.push(entry_id.clone());
            }
        }

        let mut entries = self.entries.lock().unwrap_or_else(|e| e.into_inner());
        for entry_id in to_remove {
            entries.remove(&entry_id);
        }
    }
}

/// MCP server that handles JSON-RPC 2.0 requests.
pub struct McpServer {
    store: Arc<dyn SessionStore>,
    write_consent: bool,
    input_queue: InputQueue,
}

impl McpServer {
    pub fn new(store: Arc<dyn SessionStore>) -> Self {
        Self {
            store,
            write_consent: false,
            input_queue: InputQueue::new(),
        }
    }

    /// Enable write permission (called by user via --mcp-allow-write flag).
    pub fn with_write_consent(mut self) -> Self {
        self.write_consent = true;
        self
    }

    /// Handle a single JSON-RPC request and produce a response.
    pub fn handle(&self, req: &JsonRpcRequest) -> Result<Value, McpError> {
        match req.method.as_str() {
            "initialize" => Ok(Self::handle_initialize()),
            "tools/list" => Ok(Self::list_tools()),
            "tools/call" => self.handle_tool_call(&req.params, &req.id),
            "ping" => Ok(json!({})),
            "notifications/initialized" => Ok(json!({})),
            _ => Err(McpError::UnknownMethod(req.method.clone())),
        }
    }

    fn handle_initialize() -> Value {
        json!({
            "protocolVersion": "2024-11-05",
            "serverInfo": {
                "name": "torvox-mcp",
                "version": env!("CARGO_PKG_VERSION"),
            },
            "capabilities": {
                "tools": {}
            },
        })
    }

    fn list_tools() -> Value {
        let tools = vec![
            (
                "list_sessions",
                "List all active terminal sessions",
                empty_schema(),
            ),
            (
                "read_grid",
                "Read current grid state of a session (rows × cols cells)",
                schema_required(&["session_id"]),
            ),
            (
                "read_scrollback",
                "Read last N lines of scrollback",
                schema_required(&["session_id", "max_lines"]),
            ),
            (
                "read_cursor",
                "Read cursor position (row, col) and visibility",
                schema_required(&["session_id"]),
            ),
            (
                "read_selection",
                "Read currently selected text (if any)",
                schema_required(&["session_id"]),
            ),
            (
                "read_title",
                "Read the session title (from OSC 0/2)",
                schema_required(&["session_id"]),
            ),
            (
                "send_input",
                "Write text to PTY (requires --mcp-allow-write consent)",
                schema_required(&["session_id", "data"]),
            ),
            (
                "send_signal",
                "Send signal to child process (SIGINT/SIGTERM/SIGHUP/SIGQUIT)",
                schema_required(&["session_id", "signal"]),
            ),
            (
                "get_app_info",
                "Get app version and capabilities",
                empty_schema(),
            ),
            (
                "scrollback_search",
                "Search scrollback buffer for a text pattern (regex)",
                schema_required(&["session_id", "pattern", "max_matches"]),
            ),
            (
                "set_terminal_size",
                "Resize terminal to specified rows and cols (requires write consent)",
                schema_required(&["session_id", "rows", "cols"]),
            ),
            (
                "list_directory",
                "List files in a local directory",
                schema_required(&["path"]),
            ),
            (
                "read_file",
                "Read contents of a local file (text, limited lines)",
                schema_required(&["path", "max_lines"]),
            ),
            (
                "read_clipboard",
                "Read current clipboard content",
                empty_schema(),
            ),
            (
                "write_clipboard",
                "Write text to clipboard (requires write consent)",
                schema_required(&["text"]),
            ),
            (
                "raise_notification",
                "Show an Android notification (requires write consent)",
                schema_required(&["title", "body"]),
            ),
            (
                "scroll_terminal",
                "Scroll terminal viewport by N lines (negative=up, positive=down). Returns new scroll offset",
                schema_required(&["session_id", "lines"]),
            ),
            (
                "feed_terminal_output",
                "Inject text into terminal as if the child process produced it (requires write consent)",
                schema_required(&["session_id", "text"]),
            ),
            (
                "queue_terminal_input",
                "Queue text to be typed when a prompt pattern appears in scrollback (AI agent automation)",
                schema_required(&["session_id", "text", "prompt_pattern", "timeout_seconds"]),
            ),
            (
                "list_queued_inputs",
                "List all pending queued inputs",
                empty_schema(),
            ),
            (
                "cancel_queued_input",
                "Cancel a pending queued input by its entry_id",
                schema_required(&["entry_id"]),
            ),
        ];
        let tools_json: Vec<Value> = tools
            .into_iter()
            .map(|(name, desc, schema)| {
                json!({
                    "name": name,
                    "description": desc,
                    "inputSchema": schema,
                })
            })
            .collect();
        json!({ "tools": tools_json })
    }

    fn handle_tool_call(&self, params: &Value, _id: &Value) -> Result<Value, McpError> {
        let name = params
            .get("name")
            .and_then(|v| v.as_str())
            .ok_or_else(|| McpError::InvalidParams("missing 'name'".into()))?;
        let args = params.get("arguments").cloned().unwrap_or(json!({}));

        match name {
            "list_sessions" => {
                let resp = self
                    .store
                    .read(ReadRequest::Sessions)
                    .map_err(McpError::Internal)?;
                Ok(json!({ "content": [{ "type": "json", "data": resp }] }))
            }
            "read_grid" => {
                let session_id = args
                    .get("session_id")
                    .and_then(|v| v.as_u64())
                    .ok_or_else(|| McpError::InvalidParams("session_id required".into()))?
                    as u32;
                let resp = self
                    .store
                    .read(ReadRequest::Grid { session_id })
                    .map_err(McpError::Internal)?;
                Ok(json!({ "content": [{ "type": "json", "data": resp }] }))
            }
            "read_scrollback" => {
                let session_id = args
                    .get("session_id")
                    .and_then(|v| v.as_u64())
                    .ok_or_else(|| McpError::InvalidParams("session_id required".into()))?
                    as u32;
                let max = args
                    .get("max_lines")
                    .and_then(|v| v.as_u64())
                    .ok_or_else(|| McpError::InvalidParams("max_lines required".into()))?
                    as u32;
                let resp = self
                    .store
                    .read(ReadRequest::Scrollback {
                        session_id,
                        max_lines: max,
                    })
                    .map_err(McpError::Internal)?;
                Ok(json!({ "content": [{ "type": "json", "data": resp }] }))
            }
            "read_cursor" => {
                let session_id = args
                    .get("session_id")
                    .and_then(|v| v.as_u64())
                    .ok_or_else(|| McpError::InvalidParams("session_id required".into()))?
                    as u32;
                let resp = self
                    .store
                    .read(ReadRequest::Cursor { session_id })
                    .map_err(McpError::Internal)?;
                Ok(json!({ "content": [{ "type": "json", "data": resp }] }))
            }
            "read_selection" => {
                let session_id = args
                    .get("session_id")
                    .and_then(|v| v.as_u64())
                    .ok_or_else(|| McpError::InvalidParams("session_id required".into()))?
                    as u32;
                let resp = self
                    .store
                    .read(ReadRequest::Selection { session_id })
                    .map_err(McpError::Internal)?;
                Ok(json!({ "content": [{ "type": "json", "data": resp }] }))
            }
            "read_title" => {
                let session_id = args
                    .get("session_id")
                    .and_then(|v| v.as_u64())
                    .ok_or_else(|| McpError::InvalidParams("session_id required".into()))?
                    as u32;
                let resp = self
                    .store
                    .read(ReadRequest::Title { session_id })
                    .map_err(McpError::Internal)?;
                Ok(json!({ "content": [{ "type": "json", "data": resp }] }))
            }
            "send_input" => {
                if !self.write_consent {
                    return Err(McpError::InvalidParams(
                        "send_input requires --mcp-allow-write consent flag".into(),
                    ));
                }
                let session_id = args
                    .get("session_id")
                    .and_then(|v| v.as_u64())
                    .ok_or_else(|| McpError::InvalidParams("session_id required".into()))?
                    as u32;
                let data = args
                    .get("data")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| McpError::InvalidParams("data required".into()))?
                    .as_bytes()
                    .to_vec();
                self.store
                    .write(session_id, data)
                    .map_err(McpError::Internal)?;
                Ok(json!({ "content": [{ "type": "text", "text": "wrote to PTY" }] }))
            }
            "send_signal" => {
                if !self.write_consent {
                    return Err(McpError::InvalidParams(
                        "send_signal requires --mcp-allow-write consent flag".into(),
                    ));
                }
                let session_id = args
                    .get("session_id")
                    .and_then(|v| v.as_u64())
                    .ok_or_else(|| McpError::InvalidParams("session_id required".into()))?
                    as u32;
                let signal_string = args
                    .get("signal")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| McpError::InvalidParams("signal required".into()))?;
                let signal_kind = match signal_string {
                    "SIGINT" | "INT" | "interrupt" => SignalKind::Interrupt,
                    "SIGTERM" | "TERM" | "terminate" => SignalKind::Terminate,
                    "SIGHUP" | "HUP" | "hangup" => SignalKind::Hangup,
                    "SIGQUIT" | "QUIT" | "quit" => SignalKind::Quit,
                    _ => {
                        return Err(McpError::InvalidParams(format!(
                            "unknown signal: {signal_string}"
                        )));
                    }
                };
                self.store
                    .signal(session_id, signal_kind)
                    .map_err(McpError::Internal)?;
                Ok(json!({ "content": [{ "type": "text", "text": "sent signal" }] }))
            }
            "get_app_info" => Ok(json!({
                "content": [{
                    "type": "json",
                    "data": {
                        "name": "torvox",
                        "version": env!("CARGO_PKG_VERSION"),
                        "protocol": "2024-11-05",
                        "capabilities": [
                            "list_sessions", "read_grid", "read_scrollback",
                            "read_cursor", "read_selection", "read_title",
                            "send_input", "send_signal", "get_app_info",
                            "scrollback_search", "set_terminal_size",
                            "list_directory", "read_file",
                            "read_clipboard", "write_clipboard",
                            "raise_notification", "scroll_terminal",
                            "feed_terminal_output"
                        ],
                        "rendering": "gpu-wgpu",
                        "font_system": "cosmic-text",
                        "platform": "android"
                    }
                }]
            })),
            "scrollback_search" => {
                let session_id = args
                    .get("session_id")
                    .and_then(|v| v.as_u64())
                    .ok_or_else(|| McpError::InvalidParams("session_id required".into()))?
                    as u32;
                let pattern = args
                    .get("pattern")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| McpError::InvalidParams("pattern required".into()))?
                    .to_string();
                let max = args
                    .get("max_matches")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(50) as u32;
                let resp = self
                    .store
                    .read(ReadRequest::ScrollbackSearch {
                        session_id,
                        pattern,
                        max_matches: max,
                    })
                    .map_err(McpError::Internal)?;
                Ok(json!({ "content": [{ "type": "json", "data": resp }] }))
            }
            "set_terminal_size" => {
                if !self.write_consent {
                    return Err(McpError::InvalidParams(
                        "set_terminal_size requires --mcp-allow-write consent flag".into(),
                    ));
                }
                let session_id = args
                    .get("session_id")
                    .and_then(|v| v.as_u64())
                    .ok_or_else(|| McpError::InvalidParams("session_id required".into()))?
                    as u32;
                let rows = args
                    .get("rows")
                    .and_then(|v| v.as_u64())
                    .ok_or_else(|| McpError::InvalidParams("rows required".into()))?
                    as u32;
                let cols = args
                    .get("cols")
                    .and_then(|v| v.as_u64())
                    .ok_or_else(|| McpError::InvalidParams("cols required".into()))?
                    as u32;
                self.store
                    .set_terminal_size(session_id, rows, cols)
                    .map_err(McpError::Internal)?;
                Ok(
                    json!({ "content": [{ "type": "text", "text": format!("resized to {rows}x{cols}") }] }),
                )
            }
            "list_directory" => {
                let path = args
                    .get("path")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| McpError::InvalidParams("path required".into()))?
                    .to_string();
                let resp = self
                    .store
                    .read(ReadRequest::ListDirectory { path })
                    .map_err(McpError::Internal)?;
                Ok(json!({ "content": [{ "type": "json", "data": resp }] }))
            }
            "read_file" => {
                let path = args
                    .get("path")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| McpError::InvalidParams("path required".into()))?
                    .to_string();
                let max = args
                    .get("max_lines")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(1000) as u32;
                let resp = self
                    .store
                    .read(ReadRequest::ReadFile {
                        path,
                        max_lines: max,
                    })
                    .map_err(McpError::Internal)?;
                Ok(json!({ "content": [{ "type": "json", "data": resp }] }))
            }
            "read_clipboard" => {
                let content = self.store.read_clipboard().map_err(McpError::Internal)?;
                Ok(json!({ "content": [{ "type": "text", "text": content }] }))
            }
            "write_clipboard" => {
                if !self.write_consent {
                    return Err(McpError::InvalidParams(
                        "write_clipboard requires --mcp-allow-write consent flag".into(),
                    ));
                }
                let text = args
                    .get("text")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| McpError::InvalidParams("text required".into()))?
                    .to_string();
                self.store
                    .write_clipboard(&text)
                    .map_err(McpError::Internal)?;
                Ok(json!({ "content": [{ "type": "text", "text": "clipboard updated" }] }))
            }
            "raise_notification" => {
                if !self.write_consent {
                    return Err(McpError::InvalidParams(
                        "raise_notification requires --mcp-allow-write consent flag".into(),
                    ));
                }
                let title = args
                    .get("title")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| McpError::InvalidParams("title required".into()))?
                    .to_string();
                let body = args
                    .get("body")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| McpError::InvalidParams("body required".into()))?
                    .to_string();
                self.store
                    .raise_notification(&title, &body)
                    .map_err(McpError::Internal)?;
                Ok(json!({ "content": [{ "type": "text", "text": "notification sent" }] }))
            }
            "scroll_terminal" => {
                let session_id = args
                    .get("session_id")
                    .and_then(|v| v.as_u64())
                    .ok_or_else(|| McpError::InvalidParams("session_id required".into()))?
                    as u32;
                let lines = args
                    .get("lines")
                    .and_then(|v| v.as_i64())
                    .ok_or_else(|| McpError::InvalidParams("lines required".into()))?
                    as i32;
                let offset = self
                    .store
                    .scroll_terminal(session_id, lines)
                    .map_err(McpError::Internal)?;
                Ok(json!({
                    "content": [{ "type": "json", "data": { "scroll_offset": offset } }]
                }))
            }
            "feed_terminal_output" => {
                if !self.write_consent {
                    return Err(McpError::InvalidParams(
                        "feed_terminal_output requires --mcp-allow-write consent flag".into(),
                    ));
                }
                let session_id = args
                    .get("session_id")
                    .and_then(|v| v.as_u64())
                    .ok_or_else(|| McpError::InvalidParams("session_id required".into()))?
                    as u32;
                let text = args
                    .get("text")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| McpError::InvalidParams("text required".into()))?;
                self.store
                    .feed_terminal_output(session_id, text)
                    .map_err(McpError::Internal)?;
                Ok(json!({ "content": [{ "type": "text", "text": "output fed to terminal" }] }))
            }
            "queue_terminal_input" => {
                if !self.write_consent {
                    return Err(McpError::InvalidParams(
                        "queue_terminal_input requires --mcp-allow-write consent flag".into(),
                    ));
                }
                let session_id = args
                    .get("session_id")
                    .and_then(|v| v.as_u64())
                    .ok_or_else(|| McpError::InvalidParams("session_id required".into()))?
                    as u32;
                let text = args
                    .get("text")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| McpError::InvalidParams("text required".into()))?;
                let prompt_pattern = args
                    .get("prompt_pattern")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| McpError::InvalidParams("prompt_pattern required".into()))?;
                let timeout_seconds = args
                    .get("timeout_seconds")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(60) as u32;
                let submit_key = args
                    .get("submit_key")
                    .and_then(|v| v.as_str())
                    .unwrap_or("\r");
                let entry_id = self.input_queue.enqueue(
                    session_id,
                    text.to_string(),
                    submit_key.to_string(),
                    prompt_pattern.to_string(),
                    timeout_seconds,
                );
                Ok(json!({
                    "content": [{
                        "type": "text",
                        "text": format!("Queued input {entry_id}: will type when pattern \"{prompt_pattern}\" appears in session {session_id}")
                    }]
                }))
            }
            "list_queued_inputs" => {
                let entries = self.input_queue.pending();
                Ok(json!({
                    "content": [{ "type": "json", "data": entries }]
                }))
            }
            "cancel_queued_input" => {
                let entry_id = args
                    .get("entry_id")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| McpError::InvalidParams("entry_id required".into()))?;
                let removed = self.input_queue.cancel(entry_id);
                Ok(json!({
                    "content": [{
                        "type": "text",
                        "text": if removed { format!("Cancelled {entry_id}") } else { format!("No entry found with id {entry_id}") }
                    }]
                }))
            }
            _ => Err(McpError::UnknownMethod(name.into())),
        }
    }
}

fn empty_schema() -> Value {
    json!({
        "type": "object",
        "properties": {},
        "additionalProperties": false,
    })
}

fn schema_required(required: &[&str]) -> Value {
    let mut properties = BTreeMap::new();
    for r in required {
        match *r {
            "session_id" => {
                properties.insert(
                    "session_id".to_string(),
                    json!({ "type": "integer", "minimum": 0 }),
                );
            }
            "max_lines" => {
                properties.insert(
                    "max_lines".to_string(),
                    json!({ "type": "integer", "minimum": 1, "maximum": 100_000 }),
                );
            }
            "data" => {
                properties.insert("data".to_string(), json!({ "type": "string" }));
            }
            "signal" => {
                properties.insert(
                    "signal".to_string(),
                    json!({
                        "type": "string",
                        "enum": ["SIGINT", "SIGTERM", "SIGHUP", "SIGQUIT"]
                    }),
                );
            }
            "pattern" => {
                properties.insert(
                    "pattern".to_string(),
                    json!({ "type": "string", "description": "Text or regex pattern to search for" }),
                );
            }
            "max_matches" => {
                properties.insert(
                    "max_matches".to_string(),
                    json!({ "type": "integer", "minimum": 1, "maximum": 1000 }),
                );
            }
            "rows" => {
                properties.insert(
                    "rows".to_string(),
                    json!({ "type": "integer", "minimum": 1, "maximum": 1000 }),
                );
            }
            "cols" => {
                properties.insert(
                    "cols".to_string(),
                    json!({ "type": "integer", "minimum": 1, "maximum": 1000 }),
                );
            }
            "path" => {
                properties.insert(
                    "path".to_string(),
                    json!({ "type": "string", "description": "File or directory path" }),
                );
            }
            "text" => {
                properties.insert("text".to_string(), json!({ "type": "string" }));
            }
            "title" => {
                properties.insert(
                    "title".to_string(),
                    json!({ "type": "string", "description": "Notification title" }),
                );
            }
            "body" => {
                properties.insert(
                    "body".to_string(),
                    json!({ "type": "string", "description": "Notification body text" }),
                );
            }
            "lines" => {
                properties.insert(
                    "lines".to_string(),
                    json!({ "type": "integer", "description": "Number of lines to scroll (negative=up, positive=down)" }),
                );
            }
            _ => {}
        }
    }
    let mut reqs: Vec<String> = required.iter().map(|s| s.to_string()).collect();
    reqs.sort();
    json!({
        "type": "object",
        "properties": properties,
        "required": reqs,
        "additionalProperties": false,
    })
}

/// Run a JSON-RPC server over a Unix socket at `socket_path`.
#[cfg(unix)]
pub fn serve_unix(
    socket_path: &Path,
    store: Arc<dyn SessionStore>,
    write_consent: bool,
) -> std::io::Result<()> {
    use std::os::unix::net::UnixListener;

    if let Some(parent) = socket_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    if let Err(error) = std::fs::remove_file(socket_path) {
        log::warn!("mcp: failed to remove existing socket at {socket_path:?}: {error}");
    }
    let listener = UnixListener::bind(socket_path)?;

    let mut server = McpServer::new(store as Arc<dyn SessionStore>);
    if write_consent {
        server = server.with_write_consent();
    }
    let server = Arc::new(server);

    for stream in listener.incoming() {
        match stream {
            Ok(initial_stream) => {
                let server = Arc::clone(&server);
                std::thread::spawn(move || {
                    let mut socket = initial_stream;
                    let reader = match socket.try_clone() {
                        Ok(cloned) => std::io::BufReader::new(cloned),
                        Err(_) => return,
                    };
                    for line in reader.lines().map_while(Result::ok) {
                        let line = line.trim().to_string();
                        if line.is_empty() {
                            continue;
                        }
                        let response = match serde_json::from_str::<JsonRpcRequest>(&line) {
                            Ok(req) => match server.handle(&req) {
                                Ok(result) => json!({
                                    "jsonrpc": "2.0",
                                    "id": req.id,
                                    "result": result,
                                }),
                                Err(e) => e.to_json_rpc_error(&req.id),
                            },
                            Err(e) => json!({
                                "jsonrpc": "2.0",
                                "id": Value::Null,
                                "error": {
                                    "code": -32700,
                                    "message": format!("parse error: {e}"),
                                },
                            }),
                        };
                        if let Err(e) = writeln!(socket, "{response}") {
                            log::error!("mcp: failed to write JSON-RPC response: {e}");
                        }
                        if let Err(e) = socket.flush() {
                            log::error!("mcp: failed to flush socket: {e}");
                        }
                    }
                });
            }
            Err(e) => {
                log::error!("mcp: accept failed: {e}");
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    struct MockStore {
        sessions: Mutex<Vec<SessionInfo>>,
    }

    impl MockStore {
        fn new() -> Self {
            Self {
                sessions: Mutex::new(vec![SessionInfo {
                    id: 1,
                    title: "test".into(),
                    rows: 24,
                    cols: 80,
                    shell: "/bin/sh".into(),
                    pid: Some(1234),
                    is_exited: false,
                }]),
            }
        }
    }

    impl SessionStore for MockStore {
        fn read(&self, req: ReadRequest) -> Result<ReadResponse, String> {
            match req {
                ReadRequest::Sessions => Ok(ReadResponse::Sessions(
                    self.sessions.lock().unwrap().clone(),
                )),
                ReadRequest::Cursor { session_id } => {
                    if session_id == 1 {
                        Ok(ReadResponse::Cursor {
                            row: 5,
                            col: 10,
                            visible: true,
                        })
                    } else {
                        Err(format!("session {session_id} not found"))
                    }
                }
                ReadRequest::ListDirectory { path } => {
                    let mut entries = Vec::new();
                    if path == "/tmp" || path.starts_with('/') {
                        entries.push(DirEntry {
                            name: ".".into(),
                            is_dir: true,
                            size: None,
                            modified: None,
                        });
                        entries.push(DirEntry {
                            name: "..".into(),
                            is_dir: true,
                            size: None,
                            modified: None,
                        });
                        entries.push(DirEntry {
                            name: "test.txt".into(),
                            is_dir: false,
                            size: Some(100),
                            modified: None,
                        });
                    }
                    Ok(ReadResponse::DirectoryEntries(entries))
                }
                ReadRequest::ReadFile {
                    path: _,
                    max_lines: _,
                } => Ok(ReadResponse::FileContent {
                    lines: vec!["line 1".into(), "line 2".into()],
                    total_lines: 2,
                    truncated: false,
                }),
                ReadRequest::ReadClipboard => {
                    Ok(ReadResponse::ClipboardContent("mock clipboard".into()))
                }
                ReadRequest::ScrollbackSearch {
                    session_id,
                    pattern,
                    max_matches,
                } => {
                    if session_id == 1 {
                        let mut matches = Vec::new();
                        if pattern == "test" {
                            matches.push(SearchMatch {
                                line_number: 0,
                                text: "test line".into(),
                                start_col: 0,
                                end_col: 4,
                            });
                        }
                        if matches.len() > max_matches as usize {
                            matches.truncate(max_matches as usize);
                        }
                        Ok(ReadResponse::SearchMatches(matches))
                    } else {
                        Err(format!("session {session_id} not found"))
                    }
                }
                ReadRequest::TerminalSize { session_id } => {
                    if session_id == 1 {
                        Ok(ReadResponse::TerminalSize { rows: 24, cols: 80 })
                    } else {
                        Err(format!("session {session_id} not found"))
                    }
                }
                _ => Err("not implemented in mock".into()),
            }
        }
        fn write(&self, _: u32, _: Vec<u8>) -> Result<(), String> {
            Ok(())
        }
        fn signal(&self, _: u32, _: SignalKind) -> Result<(), String> {
            Ok(())
        }
        fn set_terminal_size(&self, _: u32, _rows: u32, _cols: u32) -> Result<(), String> {
            Ok(())
        }
        fn write_clipboard(&self, _: &str) -> Result<(), String> {
            Ok(())
        }
        fn read_clipboard(&self) -> Result<String, String> {
            Ok("mock clipboard".into())
        }
        fn raise_notification(&self, _: &str, _: &str) -> Result<(), String> {
            Ok(())
        }
        fn scroll_terminal(&self, _: u32, _lines: i32) -> Result<i32, String> {
            Ok(0)
        }
        fn feed_terminal_output(&self, _: u32, _: &str) -> Result<(), String> {
            Ok(())
        }
    }

    #[test]
    fn handle_initialize() {
        let store = Arc::new(MockStore::new());
        let server = McpServer::new(store);
        let req = JsonRpcRequest {
            jsonrpc: "2.0".into(),
            method: "initialize".into(),
            params: json!({}),
            id: json!(1),
        };
        let result = server.handle(&req).unwrap();
        assert_eq!(result["serverInfo"]["name"], "torvox-mcp");
    }

    #[test]
    fn list_tools() {
        let store = Arc::new(MockStore::new());
        let server = McpServer::new(store);
        let req = JsonRpcRequest {
            jsonrpc: "2.0".into(),
            method: "tools/list".into(),
            params: json!({}),
            id: json!(1),
        };
        let result = server.handle(&req).unwrap();
        let tools = result["tools"].as_array().unwrap();
        assert!(tools.iter().any(|t| t["name"] == "list_sessions"));
        assert!(tools.iter().any(|t| t["name"] == "read_grid"));
        assert!(tools.iter().any(|t| t["name"] == "send_input"));
    }

    #[test]
    fn unknown_method() {
        let store = Arc::new(MockStore::new());
        let server = McpServer::new(store);
        let req = JsonRpcRequest {
            jsonrpc: "2.0".into(),
            method: "nonexistent".into(),
            params: json!({}),
            id: json!(1),
        };
        let result = server.handle(&req);
        assert!(matches!(result, Err(McpError::UnknownMethod(_))));
    }

    #[test]
    fn list_sessions_tool() {
        let store = Arc::new(MockStore::new());
        let server = McpServer::new(store);
        let req = JsonRpcRequest {
            jsonrpc: "2.0".into(),
            method: "tools/call".into(),
            params: json!({
                "name": "list_sessions",
                "arguments": {}
            }),
            id: json!(1),
        };
        let result = server.handle(&req).unwrap();
        let content = &result["content"][0]["data"]["Sessions"];
        let arr = content.as_array().unwrap();
        assert_eq!(arr.len(), 1);
        assert_eq!(arr[0]["id"], 1);
    }

    #[test]
    fn write_consent() {
        for (with_consent, expect_ok) in [(false, false), (true, true)] {
            let store = Arc::new(MockStore::new());
            let server = if with_consent {
                McpServer::new(store).with_write_consent()
            } else {
                McpServer::new(store)
            };
            let req = JsonRpcRequest {
                jsonrpc: "2.0".into(),
                method: "tools/call".into(),
                params: json!({
                    "name": "send_input",
                    "arguments": { "session_id": 1, "data": "ls" }
                }),
                id: json!(1),
            };
            let result = server.handle(&req);
            if expect_ok {
                assert_eq!(result.unwrap()["content"][0]["text"], "wrote to PTY");
            } else {
                assert!(matches!(result, Err(McpError::InvalidParams(_))));
            }
        }
    }

    #[test]
    fn signal_validation() {
        let store = Arc::new(MockStore::new());
        let server = McpServer::new(store).with_write_consent();
        let req = JsonRpcRequest {
            jsonrpc: "2.0".into(),
            method: "tools/call".into(),
            params: json!({
                "name": "send_signal",
                "arguments": { "session_id": 1, "signal": "INVALID" }
            }),
            id: json!(1),
        };
        let result = server.handle(&req);
        assert!(matches!(result, Err(McpError::InvalidParams(_))));
    }

    #[test]
    fn error_envelope_serializes() {
        let e = McpError::SessionNotFound(42);
        let env = e.to_json_rpc_error(&json!(1));
        assert_eq!(env["error"]["code"], -32001);
        assert!(env["error"]["message"].as_str().unwrap().contains("42"));
    }

    #[test]
    fn cell_conversion() {
        use torvox_core::cell::Cell;
        let c = Cell::default();
        let d: GridCellData = (&c).into();
        assert_eq!(d.codepoint, c.char as u32);
    }

    #[test]
    fn session_info_roundtrip() {
        let s = SessionInfo {
            id: 7,
            title: "zsh".into(),
            rows: 30,
            cols: 100,
            shell: "/bin/zsh".into(),
            pid: Some(999),
            is_exited: false,
        };
        let json = serde_json::to_string(&s).unwrap();
        let back: SessionInfo = serde_json::from_str(&json).unwrap();
        assert_eq!(back.id, 7);
        assert_eq!(back.cols, 100);
    }

    #[quickcheck_macros::quickcheck]
    fn session_info_serde_id(session_id: u32) -> bool {
        let s = SessionInfo {
            id: session_id,
            title: String::new(),
            rows: 24,
            cols: 80,
            shell: String::new(),
            pid: None,
            is_exited: false,
        };
        let json = serde_json::to_string(&s).unwrap();
        let back: SessionInfo = serde_json::from_str(&json).unwrap();
        back.id == session_id
    }

    #[test]
    fn get_app_info_tool() {
        let store = Arc::new(MockStore::new());
        let server = McpServer::new(store);
        let req = JsonRpcRequest {
            jsonrpc: "2.0".into(),
            method: "tools/call".into(),
            params: json!({
                "name": "get_app_info",
                "arguments": {}
            }),
            id: json!(1),
        };
        let result = server.handle(&req).unwrap();
        let info = &result["content"][0]["data"];
        assert_eq!(info["name"], "torvox");
        assert_eq!(info["rendering"], "gpu-wgpu");
        let caps = info["capabilities"].as_array().unwrap();
        assert!(caps.contains(&json!("get_app_info")));
        assert!(caps.contains(&json!("scrollback_search")));
        assert!(caps.contains(&json!("list_directory")));
    }

    #[test]
    fn list_tools_count() {
        let store = Arc::new(MockStore::new());
        let server = McpServer::new(store);
        let req = JsonRpcRequest {
            jsonrpc: "2.0".into(),
            method: "tools/list".into(),
            params: json!({}),
            id: json!(1),
        };
        let result = server.handle(&req).unwrap();
        let tools = result["tools"].as_array().unwrap();
        assert_eq!(tools.len(), 21, "expected 21 tools, got {}", tools.len());
        assert!(tools.iter().any(|t| t["name"] == "get_app_info"));
        assert!(tools.iter().any(|t| t["name"] == "scrollback_search"));
        assert!(tools.iter().any(|t| t["name"] == "set_terminal_size"));
        assert!(tools.iter().any(|t| t["name"] == "queue_terminal_input"));
        assert!(tools.iter().any(|t| t["name"] == "list_queued_inputs"));
        assert!(tools.iter().any(|t| t["name"] == "cancel_queued_input"));
        assert!(tools.iter().any(|t| t["name"] == "list_directory"));
        assert!(tools.iter().any(|t| t["name"] == "read_file"));
        assert!(tools.iter().any(|t| t["name"] == "read_clipboard"));
        assert!(tools.iter().any(|t| t["name"] == "write_clipboard"));
        assert!(tools.iter().any(|t| t["name"] == "raise_notification"));
    }

    #[test]
    fn set_terminal_size_requires_consent() {
        let store = Arc::new(MockStore::new());
        let server = McpServer::new(store);
        let req = JsonRpcRequest {
            jsonrpc: "2.0".into(),
            method: "tools/call".into(),
            params: json!({
                "name": "set_terminal_size",
                "arguments": { "session_id": 1, "rows": 50, "cols": 120 }
            }),
            id: json!(1),
        };
        let result = server.handle(&req);
        assert!(matches!(result, Err(McpError::InvalidParams(_))));
    }

    #[test]
    fn write_clipboard_requires_consent() {
        let store = Arc::new(MockStore::new());
        let server = McpServer::new(store);
        let req = JsonRpcRequest {
            jsonrpc: "2.0".into(),
            method: "tools/call".into(),
            params: json!({
                "name": "write_clipboard",
                "arguments": { "text": "hello" }
            }),
            id: json!(1),
        };
        let result = server.handle(&req);
        assert!(matches!(result, Err(McpError::InvalidParams(_))));
    }

    #[test]
    fn read_clipboard_no_consent_needed() {
        let store = Arc::new(MockStore::new());
        let server = McpServer::new(store);
        let req = JsonRpcRequest {
            jsonrpc: "2.0".into(),
            method: "tools/call".into(),
            params: json!({
                "name": "read_clipboard",
                "arguments": {}
            }),
            id: json!(1),
        };
        let result = server.handle(&req);
        assert!(result.is_ok());
    }

    #[test]
    fn list_directory_tool() {
        let store = Arc::new(MockStore::new());
        let server = McpServer::new(store);
        let req = JsonRpcRequest {
            jsonrpc: "2.0".into(),
            method: "tools/call".into(),
            params: json!({
                "name": "list_directory",
                "arguments": { "path": "/nonexistent" }
            }),
            id: json!(1),
        };
        let result = server.handle(&req);
        assert!(result.is_ok());
    }

    #[test]
    fn search_match_serde() {
        let m = SearchMatch {
            line_number: 5,
            text: "test line".into(),
            start_col: 0,
            end_col: 9,
        };
        let json = serde_json::to_string(&m).unwrap();
        let back: SearchMatch = serde_json::from_str(&json).unwrap();
        assert_eq!(back.line_number, 5);
        assert_eq!(back.start_col, 0);
    }

    #[test]
    fn dir_entry_serde() {
        let e = DirEntry {
            name: "file.txt".into(),
            is_dir: false,
            size: Some(1024),
            modified: Some("2024-01-01".into()),
        };
        let json = serde_json::to_string(&e).unwrap();
        let back: DirEntry = serde_json::from_str(&json).unwrap();
        assert_eq!(back.name, "file.txt");
        assert!(!back.is_dir);
    }

    #[test]
    fn scroll_terminal_tool() {
        let store = Arc::new(MockStore::new());
        let server = McpServer::new(store).with_write_consent();
        let req = JsonRpcRequest {
            jsonrpc: "2.0".into(),
            method: "tools/call".into(),
            params: json!({
                "name": "scroll_terminal",
                "arguments": { "session_id": 1, "lines": -5 }
            }),
            id: json!(1),
        };
        let result = server.handle(&req).unwrap();
        assert_eq!(result["content"][0]["data"]["scroll_offset"], 0);
    }

    #[test]
    fn scroll_terminal_no_consent_needed() {
        let store = Arc::new(MockStore::new());
        let server = McpServer::new(store);
        let req = JsonRpcRequest {
            jsonrpc: "2.0".into(),
            method: "tools/call".into(),
            params: json!({
                "name": "scroll_terminal",
                "arguments": { "session_id": 1, "lines": 3 }
            }),
            id: json!(1),
        };
        let result = server.handle(&req).unwrap();
        assert_eq!(result["content"][0]["data"]["scroll_offset"], 0);
    }

    #[test]
    fn feed_terminal_output_tool() {
        let store = Arc::new(MockStore::new());
        let server = McpServer::new(store).with_write_consent();
        let req = JsonRpcRequest {
            jsonrpc: "2.0".into(),
            method: "tools/call".into(),
            params: json!({
                "name": "feed_terminal_output",
                "arguments": { "session_id": 1, "text": "hello world\n" }
            }),
            id: json!(1),
        };
        let result = server.handle(&req).unwrap();
        assert_eq!(result["content"][0]["text"], "output fed to terminal");
    }

    #[test]
    fn feed_terminal_output_requires_write_consent() {
        let store = Arc::new(MockStore::new());
        let server = McpServer::new(store);
        let req = JsonRpcRequest {
            jsonrpc: "2.0".into(),
            method: "tools/call".into(),
            params: json!({
                "name": "feed_terminal_output",
                "arguments": { "session_id": 1, "text": "test" }
            }),
            id: json!(1),
        };
        let result = server.handle(&req);
        assert!(matches!(result, Err(McpError::InvalidParams(_))));
    }

    #[test]
    fn new_tools_in_list() {
        let store = Arc::new(MockStore::new());
        let server = McpServer::new(store);
        let req = JsonRpcRequest {
            jsonrpc: "2.0".into(),
            method: "tools/list".into(),
            params: json!({}),
            id: json!(1),
        };
        let result = server.handle(&req).unwrap();
        let tools = result["tools"].as_array().unwrap();
        assert!(tools.iter().any(|t| t["name"] == "scroll_terminal"));
        assert!(tools.iter().any(|t| t["name"] == "feed_terminal_output"));
        assert!(tools.iter().any(|t| t["name"] == "queue_terminal_input"));
        assert!(tools.iter().any(|t| t["name"] == "list_queued_inputs"));
        assert!(tools.iter().any(|t| t["name"] == "cancel_queued_input"));
        assert_eq!(tools.len(), 21);
    }

    #[test]
    fn input_queue_new_is_empty() {
        let q = InputQueue::new();
        assert!(q.pending().is_empty());
    }

    #[test]
    fn input_queue_enqueue_and_pending() {
        let q = InputQueue::new();
        let id = q.enqueue(1, "ls".into(), "\r".into(), "$ ".into(), 60);
        assert!(!id.is_empty());
        let pending = q.pending();
        assert_eq!(pending.len(), 1);
        assert_eq!(pending[0]["session_id"], 1);
        assert_eq!(pending[0]["text_preview"], "ls");
        assert_eq!(pending[0]["prompt_pattern"], "$ ");
    }

    #[test]
    fn input_queue_cancel_existing() {
        let q = InputQueue::new();
        let id = q.enqueue(1, "test".into(), "\r".into(), "> ".into(), 60);
        assert!(q.cancel(&id));
        assert!(q.pending().is_empty());
    }

    #[test]
    fn input_queue_cancel_nonexistent() {
        let q = InputQueue::new();
        assert!(!q.cancel("nonexistent-id"));
    }

    #[test]
    fn input_queue_multiple_entries() {
        let q = InputQueue::new();
        q.enqueue(1, "cmd1".into(), "\r".into(), "$ ".into(), 60);
        q.enqueue(2, "cmd2".into(), "\r".into(), "> ".into(), 60);
        assert_eq!(q.pending().len(), 2);
    }

    #[test]
    fn input_queue_cancel_one_of_many() {
        let q = InputQueue::new();
        let id1 = q.enqueue(1, "cmd1".into(), "\r".into(), "$ ".into(), 60);
        q.enqueue(2, "cmd2".into(), "\r".into(), "> ".into(), 60);
        assert!(q.cancel(&id1));
        assert_eq!(q.pending().len(), 1);
        assert_eq!(q.pending()[0]["session_id"], 2);
    }

    #[test]
    fn input_queue_text_preview_truncation() {
        let q = InputQueue::new();
        let long_text = "a".repeat(60);
        q.enqueue(1, long_text, "\r".into(), "$ ".into(), 60);
        let pending = q.pending();
        let preview = pending[0]["text_preview"].as_str().unwrap();
        assert!(preview.len() <= 40);
        assert!(preview.ends_with('…'));
    }

    #[test]
    fn input_queue_check_and_deliver_no_consent() {
        let q = InputQueue::new();
        q.enqueue(1, "test".into(), "\r".into(), "$ ".into(), 60);
        let store: Arc<dyn SessionStore> = Arc::new(MockStore::new());
        q.check_and_deliver(&store, false);
        assert_eq!(q.pending().len(), 1);
    }

    #[test]
    fn mcp_error_invalid_request_code() {
        let e = McpError::InvalidRequest("bad".into());
        let env = e.to_json_rpc_error(&json!(1));
        assert_eq!(env["error"]["code"], -32600);
        assert_eq!(env["error"]["message"], "bad");
    }

    #[test]
    fn mcp_error_unknown_method_code() {
        let e = McpError::UnknownMethod("foo".into());
        let env = e.to_json_rpc_error(&json!(2));
        assert_eq!(env["error"]["code"], -32601);
        assert_eq!(env["error"]["message"], "foo");
    }

    #[test]
    fn mcp_error_invalid_params_code() {
        let e = McpError::InvalidParams("x".into());
        let env = e.to_json_rpc_error(&json!(3));
        assert_eq!(env["error"]["code"], -32602);
    }

    #[test]
    fn mcp_error_internal_code() {
        let e = McpError::Internal("crash".into());
        let env = e.to_json_rpc_error(&json!(4));
        assert_eq!(env["error"]["code"], -32603);
        assert_eq!(env["error"]["message"], "crash");
    }

    #[test]
    fn mcp_error_session_not_found_code() {
        let e = McpError::SessionNotFound(99);
        let env = e.to_json_rpc_error(&json!(5));
        assert_eq!(env["error"]["code"], -32001);
        assert!(env["error"]["message"].as_str().unwrap().contains("99"));
    }

    #[test]
    fn mcp_error_jsonrpc_envelope_structure() {
        let e = McpError::Internal("test".into());
        let env = e.to_json_rpc_error(&json!("abc"));
        assert_eq!(env["jsonrpc"], "2.0");
        assert_eq!(env["id"], "abc");
        assert!(env["error"]["code"].is_number());
        assert!(env["error"]["message"].is_string());
    }

    #[test]
    fn handle_ping() {
        let store = Arc::new(MockStore::new());
        let server = McpServer::new(store);
        let req = JsonRpcRequest {
            jsonrpc: "2.0".into(),
            method: "ping".into(),
            params: json!({}),
            id: json!(1),
        };
        let result = server.handle(&req).unwrap();
        assert_eq!(result, json!({}));
    }

    #[test]
    fn handle_unknown_method_error() {
        let store = Arc::new(MockStore::new());
        let server = McpServer::new(store);
        let req = JsonRpcRequest {
            jsonrpc: "2.0".into(),
            method: "fake_method".into(),
            params: json!({}),
            id: json!(1),
        };
        let err = server.handle(&req).unwrap_err();
        let env = err.to_json_rpc_error(&json!(1));
        assert_eq!(env["error"]["code"], -32601);
    }

    #[test]
    fn handle_tool_call_missing_name() {
        let store = Arc::new(MockStore::new());
        let server = McpServer::new(store);
        let req = JsonRpcRequest {
            jsonrpc: "2.0".into(),
            method: "tools/call".into(),
            params: json!({ "arguments": {} }),
            id: json!(1),
        };
        let result = server.handle(&req);
        assert!(matches!(result, Err(McpError::InvalidParams(_))));
    }

    #[test]
    fn handle_tool_call_unknown_tool() {
        let store = Arc::new(MockStore::new());
        let server = McpServer::new(store);
        let req = JsonRpcRequest {
            jsonrpc: "2.0".into(),
            method: "tools/call".into(),
            params: json!({ "name": "nonexistent_tool", "arguments": {} }),
            id: json!(1),
        };
        let result = server.handle(&req);
        assert!(matches!(result, Err(McpError::UnknownMethod(_))));
    }

    #[test]
    fn handle_read_grid_missing_session_id() {
        let store = Arc::new(MockStore::new());
        let server = McpServer::new(store);
        let req = JsonRpcRequest {
            jsonrpc: "2.0".into(),
            method: "tools/call".into(),
            params: json!({ "name": "read_grid", "arguments": {} }),
            id: json!(1),
        };
        let result = server.handle(&req);
        assert!(matches!(result, Err(McpError::InvalidParams(_))));
    }

    #[test]
    fn handle_read_scrollback_missing_params() {
        let store = Arc::new(MockStore::new());
        let server = McpServer::new(store);
        let req = JsonRpcRequest {
            jsonrpc: "2.0".into(),
            method: "tools/call".into(),
            params: json!({ "name": "read_scrollback", "arguments": { "session_id": 1 } }),
            id: json!(1),
        };
        let result = server.handle(&req);
        assert!(matches!(result, Err(McpError::InvalidParams(_))));
    }

    #[test]
    fn handle_send_signal_invalid_signal() {
        let store = Arc::new(MockStore::new());
        let server = McpServer::new(store).with_write_consent();
        let req = JsonRpcRequest {
            jsonrpc: "2.0".into(),
            method: "tools/call".into(),
            params: json!({ "name": "send_signal", "arguments": { "session_id": 1, "signal": "SIGKILL" } }),
            id: json!(1),
        };
        let result = server.handle(&req);
        assert!(matches!(result, Err(McpError::InvalidParams(_))));
    }

    #[test]
    fn handle_scrollback_search_tool() {
        let store = Arc::new(MockStore::new());
        let server = McpServer::new(store);
        let req = JsonRpcRequest {
            jsonrpc: "2.0".into(),
            method: "tools/call".into(),
            params: json!({
                "name": "scrollback_search",
                "arguments": { "session_id": 1, "pattern": "test", "max_matches": 10 }
            }),
            id: json!(1),
        };
        let result = server.handle(&req).unwrap();
        let matches = &result["content"][0]["data"]["SearchMatches"];
        let arr = matches.as_array().unwrap();
        assert_eq!(arr.len(), 1);
        assert_eq!(arr[0]["text"], "test line");
    }

    #[test]
    fn handle_set_terminal_size_tool() {
        let store = Arc::new(MockStore::new());
        let server = McpServer::new(store).with_write_consent();
        let req = JsonRpcRequest {
            jsonrpc: "2.0".into(),
            method: "tools/call".into(),
            params: json!({
                "name": "set_terminal_size",
                "arguments": { "session_id": 1, "rows": 50, "cols": 120 }
            }),
            id: json!(1),
        };
        let result = server.handle(&req).unwrap();
        assert!(
            result["content"][0]["text"]
                .as_str()
                .unwrap()
                .contains("50x120")
        );
    }

    #[test]
    fn handle_queue_terminal_input_tool() {
        let store = Arc::new(MockStore::new());
        let server = McpServer::new(store).with_write_consent();
        let req = JsonRpcRequest {
            jsonrpc: "2.0".into(),
            method: "tools/call".into(),
            params: json!({
                "name": "queue_terminal_input",
                "arguments": {
                    "session_id": 1,
                    "text": "echo hello",
                    "prompt_pattern": "$ ",
                    "timeout_seconds": 30
                }
            }),
            id: json!(1),
        };
        let result = server.handle(&req).unwrap();
        let text = result["content"][0]["text"].as_str().unwrap();
        assert!(text.contains("Queued input"));
        assert!(text.contains("$ "));
    }

    #[test]
    fn handle_list_queued_inputs_tool() {
        let store = Arc::new(MockStore::new());
        let server = McpServer::new(store).with_write_consent();
        let req_queue = JsonRpcRequest {
            jsonrpc: "2.0".into(),
            method: "tools/call".into(),
            params: json!({
                "name": "queue_terminal_input",
                "arguments": {
                    "session_id": 1,
                    "text": "test",
                    "prompt_pattern": "> ",
                    "timeout_seconds": 60
                }
            }),
            id: json!(1),
        };
        server.handle(&req_queue).unwrap();
        let req_list = JsonRpcRequest {
            jsonrpc: "2.0".into(),
            method: "tools/call".into(),
            params: json!({ "name": "list_queued_inputs", "arguments": {} }),
            id: json!(2),
        };
        let result = server.handle(&req_list).unwrap();
        let entries = result["content"][0]["data"].as_array().unwrap();
        assert_eq!(entries.len(), 1);
    }

    #[test]
    fn handle_cancel_queued_input_tool() {
        let store = Arc::new(MockStore::new());
        let server = McpServer::new(store).with_write_consent();
        let req_queue = JsonRpcRequest {
            jsonrpc: "2.0".into(),
            method: "tools/call".into(),
            params: json!({
                "name": "queue_terminal_input",
                "arguments": {
                    "session_id": 1,
                    "text": "test",
                    "prompt_pattern": "$ ",
                    "timeout_seconds": 60
                }
            }),
            id: json!(1),
        };
        let result = server.handle(&req_queue).unwrap();
        let text = result["content"][0]["text"].as_str().unwrap();
        let entry_id = text
            .split_whitespace()
            .nth(2)
            .unwrap()
            .trim_end_matches(':');
        let req_cancel = JsonRpcRequest {
            jsonrpc: "2.0".into(),
            method: "tools/call".into(),
            params: json!({
                "name": "cancel_queued_input",
                "arguments": { "entry_id": entry_id }
            }),
            id: json!(2),
        };
        let result = server.handle(&req_cancel).unwrap();
        assert!(
            result["content"][0]["text"]
                .as_str()
                .unwrap()
                .contains("Cancelled")
        );
    }

    #[test]
    fn handle_cancel_queued_input_missing_entry_id() {
        let store = Arc::new(MockStore::new());
        let server = McpServer::new(store);
        let req = JsonRpcRequest {
            jsonrpc: "2.0".into(),
            method: "tools/call".into(),
            params: json!({ "name": "cancel_queued_input", "arguments": {} }),
            id: json!(1),
        };
        let result = server.handle(&req);
        assert!(matches!(result, Err(McpError::InvalidParams(_))));
    }

    #[test]
    fn handle_raise_notification_requires_consent() {
        let store = Arc::new(MockStore::new());
        let server = McpServer::new(store);
        let req = JsonRpcRequest {
            jsonrpc: "2.0".into(),
            method: "tools/call".into(),
            params: json!({
                "name": "raise_notification",
                "arguments": { "title": "Hi", "body": "World" }
            }),
            id: json!(1),
        };
        assert!(matches!(
            server.handle(&req),
            Err(McpError::InvalidParams(_))
        ));
    }

    #[test]
    fn handle_write_clipboard_tool() {
        let store = Arc::new(MockStore::new());
        let server = McpServer::new(store).with_write_consent();
        let req = JsonRpcRequest {
            jsonrpc: "2.0".into(),
            method: "tools/call".into(),
            params: json!({
                "name": "write_clipboard",
                "arguments": { "text": "hello" }
            }),
            id: json!(1),
        };
        let result = server.handle(&req).unwrap();
        assert_eq!(result["content"][0]["text"], "clipboard updated");
    }

    #[test]
    fn handle_read_clipboard_tool() {
        let store = Arc::new(MockStore::new());
        let server = McpServer::new(store);
        let req = JsonRpcRequest {
            jsonrpc: "2.0".into(),
            method: "tools/call".into(),
            params: json!({ "name": "read_clipboard", "arguments": {} }),
            id: json!(1),
        };
        let result = server.handle(&req).unwrap();
        assert_eq!(result["content"][0]["text"], "mock clipboard");
    }

    #[test]
    fn handle_read_file_tool() {
        let store = Arc::new(MockStore::new());
        let server = McpServer::new(store);
        let req = JsonRpcRequest {
            jsonrpc: "2.0".into(),
            method: "tools/call".into(),
            params: json!({
                "name": "read_file",
                "arguments": { "path": "/tmp/test", "max_lines": 100 }
            }),
            id: json!(1),
        };
        let result = server.handle(&req).unwrap();
        let data = &result["content"][0]["data"]["FileContent"];
        assert_eq!(data["total_lines"], 2);
    }
}
