//! MCP server — handles JSON-RPC 2.0 requests and tool dispatch.

use std::collections::BTreeMap;
use std::sync::Arc;

use serde_json::{json, Value};

use crate::input_queue::InputQueue;
use crate::types::{McpError, ReadRequest, SessionStore, SignalKind};

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
    pub fn handle(&self, req: &crate::types::JsonRpcRequest) -> Result<Value, McpError> {
        if req.jsonrpc != "2.0" {
            return Err(McpError::InvalidRequest(format!(
                "jsonrpc must be \"2.0\", got {:?}",
                req.jsonrpc
            )));
        }
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

        let result = self.dispatch_tool_call(name, &args);
        self.input_queue
            .check_and_deliver(&self.store, self.write_consent);
        result
    }

    fn dispatch_tool_call(&self, name: &str, args: &Value) -> Result<Value, McpError> {
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
                            "feed_terminal_output",
                            "queue_terminal_input",
                            "list_queued_inputs",
                            "cancel_queued_input"
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
                Ok(json!({ "content": [{ "type": "json", "data": { "Clipboard": content } }] }))
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

pub(crate) fn empty_schema() -> Value {
    json!({
        "type": "object",
        "properties": {},
        "additionalProperties": false,
    })
}

pub(crate) fn schema_required(required: &[&str]) -> Value {
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
