//! Integration tests for `torvox-mcp` exercising the full public surface
//! (server + transport) against a faithful in-memory `SessionStore`.
//!
//! These tests do not use the limited `MockStore` from the unit tests; they
//! drive every tool through `McpServer::handle` and through a real Unix
//! domain socket served by `serve_unix`.

use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::UnixStream;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde_json::{Value, json};
use torvox_mcp::{
    DirEntry, GridCellData, GridSnapshotData, InputQueue, JsonRpcRequest, McpError, McpServer,
    ReadRequest, ReadResponse, SearchMatch, SessionInfo, SessionStore, SignalKind, serve_unix,
};

/// A fully-populated in-memory session store used to verify tool behavior.
struct FakeStore {
    sessions: Vec<SessionInfo>,
    grid: GridSnapshotData,
    scrollback: Vec<String>,
    cursor: (u32, u32, bool),
    selection: Option<String>,
    title: String,
    search: Vec<SearchMatch>,
    terminal_size: (u32, u32),
    dir_entries: Vec<DirEntry>,
    file: (Vec<String>, u32, bool),
    clipboard: Mutex<String>,
    written: Arc<Mutex<Vec<Vec<u8>>>>,
    signals: Arc<Mutex<Vec<SignalKind>>>,
    sizes: Arc<Mutex<Vec<(u32, u32, u32)>>>,
    scrolls: Arc<Mutex<Vec<i32>>>,
    feeds: Arc<Mutex<Vec<String>>>,
    notifications: Arc<Mutex<Vec<(String, String)>>>,
    scroll_offset: Arc<Mutex<i32>>,
}

impl Default for FakeStore {
    fn default() -> Self {
        let cells = vec![
            GridCellData {
                row: 0,
                col: 0,
                codepoint: 'H' as u32,
                fg_r: 200,
                fg_g: 200,
                fg_b: 200,
                bg_r: 0,
                bg_g: 0,
                bg_b: 0,
                bold: true,
                italic: false,
                underline: false,
                reverse: false,
                dim: false,
                strikethrough: false,
                blink: false,
                hidden: false,
            },
            GridCellData {
                row: 0,
                col: 1,
                codepoint: 'i' as u32,
                fg_r: 200,
                fg_g: 200,
                fg_b: 200,
                bg_r: 0,
                bg_g: 0,
                bg_b: 0,
                bold: false,
                italic: false,
                underline: false,
                reverse: false,
                dim: false,
                strikethrough: false,
                blink: false,
                hidden: false,
            },
        ];
        Self {
            sessions: vec![SessionInfo {
                id: 1,
                title: "zsh".into(),
                rows: 24,
                cols: 80,
                shell: "/bin/zsh".into(),
                pid: Some(4242),
                is_exited: false,
            }],
            grid: GridSnapshotData {
                rows: 1,
                cols: 2,
                cells,
                cursor_row: 0,
                cursor_col: 2,
                cursor_visible: true,
            },
            scrollback: vec![
                "$ echo hello".into(),
                "hello".into(),
                "$ ls".into(),
                "Cargo.toml".into(),
            ],
            cursor: (3, 7, true),
            selection: Some("hello".into()),
            title: "my-session".into(),
            search: vec![SearchMatch {
                line_number: 1,
                text: "hello".into(),
                start_col: 0,
                end_col: 5,
            }],
            terminal_size: (24, 80),
            dir_entries: vec![
                DirEntry {
                    name: ".".into(),
                    is_dir: true,
                    size: None,
                    modified: None,
                },
                DirEntry {
                    name: "src".into(),
                    is_dir: true,
                    size: None,
                    modified: None,
                },
                DirEntry {
                    name: "README.md".into(),
                    is_dir: false,
                    size: Some(1234),
                    modified: Some("2024-01-01".into()),
                },
            ],
            file: (vec!["line one".into(), "line two".into()], 2, false),
            clipboard: Mutex::new("clipboard-contents".into()),
            written: Arc::new(Mutex::new(Vec::new())),
            signals: Arc::new(Mutex::new(Vec::new())),
            sizes: Arc::new(Mutex::new(Vec::new())),
            scrolls: Arc::new(Mutex::new(Vec::new())),
            feeds: Arc::new(Mutex::new(Vec::new())),
            notifications: Arc::new(Mutex::new(Vec::new())),
            scroll_offset: Arc::new(Mutex::new(0)),
        }
    }
}

impl SessionStore for FakeStore {
    fn read(&self, req: ReadRequest) -> Result<ReadResponse, String> {
        match req {
            ReadRequest::Sessions => Ok(ReadResponse::Sessions(self.sessions.clone())),
            ReadRequest::Grid { session_id } => {
                if session_id == 1 {
                    Ok(ReadResponse::Grid(self.grid.clone()))
                } else {
                    Err(format!("session {session_id} not found"))
                }
            }
            ReadRequest::Scrollback {
                session_id,
                max_lines,
            } => {
                if session_id == 1 {
                    let n = (max_lines as usize).min(self.scrollback.len());
                    let start = self.scrollback.len() - n;
                    Ok(ReadResponse::Scrollback(self.scrollback[start..].to_vec()))
                } else {
                    Err(format!("session {session_id} not found"))
                }
            }
            ReadRequest::Cursor { session_id } => {
                if session_id == 1 {
                    Ok(ReadResponse::Cursor {
                        row: self.cursor.0,
                        col: self.cursor.1,
                        visible: self.cursor.2,
                    })
                } else {
                    Err(format!("session {session_id} not found"))
                }
            }
            ReadRequest::Selection { session_id } => {
                if session_id == 1 {
                    Ok(ReadResponse::Selection(self.selection.clone()))
                } else {
                    Err(format!("session {session_id} not found"))
                }
            }
            ReadRequest::Title { session_id } => {
                if session_id == 1 {
                    Ok(ReadResponse::Title(self.title.clone()))
                } else {
                    Err(format!("session {session_id} not found"))
                }
            }
            ReadRequest::ScrollbackSearch {
                session_id,
                pattern,
                max_matches,
            } => {
                if session_id == 1 {
                    let mut matches = self.search.clone();
                    if !pattern.is_empty() && pattern != "hello" {
                        matches.clear();
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
                    Ok(ReadResponse::TerminalSize {
                        rows: self.terminal_size.0,
                        cols: self.terminal_size.1,
                    })
                } else {
                    Err(format!("session {session_id} not found"))
                }
            }
            ReadRequest::ListDirectory { path: _ } => {
                Ok(ReadResponse::DirectoryEntries(self.dir_entries.clone()))
            }
            ReadRequest::ReadFile {
                path: _,
                max_lines: _,
            } => Ok(ReadResponse::FileContent {
                lines: self.file.0.clone(),
                total_lines: self.file.1,
                truncated: self.file.2,
            }),
            ReadRequest::ReadClipboard => Ok(ReadResponse::ClipboardContent(
                self.clipboard.lock().unwrap().clone(),
            )),
        }
    }

    fn write(&self, session_id: u32, data: Vec<u8>) -> Result<(), String> {
        if session_id == 1 {
            self.written.lock().unwrap().push(data);
            Ok(())
        } else {
            Err(format!("session {session_id} not found"))
        }
    }

    fn signal(&self, session_id: u32, signal_kind: SignalKind) -> Result<(), String> {
        if session_id == 1 {
            self.signals.lock().unwrap().push(signal_kind);
            Ok(())
        } else {
            Err(format!("session {session_id} not found"))
        }
    }

    fn set_terminal_size(&self, session_id: u32, rows: u32, cols: u32) -> Result<(), String> {
        if session_id == 1 {
            self.sizes.lock().unwrap().push((session_id, rows, cols));
            Ok(())
        } else {
            Err(format!("session {session_id} not found"))
        }
    }

    fn write_clipboard(&self, text: &str) -> Result<(), String> {
        *self.clipboard.lock().unwrap() = text.to_string();
        Ok(())
    }

    fn read_clipboard(&self) -> Result<String, String> {
        Ok(self.clipboard.lock().unwrap().clone())
    }

    fn raise_notification(&self, title: &str, body: &str) -> Result<(), String> {
        self.notifications
            .lock()
            .unwrap()
            .push((title.to_string(), body.to_string()));
        Ok(())
    }

    fn scroll_terminal(&self, session_id: u32, lines: i32) -> Result<i32, String> {
        if session_id == 1 {
            let mut offset = self.scroll_offset.lock().unwrap();
            *offset += lines;
            Ok(*offset)
        } else {
            Err(format!("session {session_id} not found"))
        }
    }

    fn feed_terminal_output(&self, session_id: u32, text: &str) -> Result<(), String> {
        if session_id == 1 {
            self.feeds.lock().unwrap().push(text.to_string());
            Ok(())
        } else {
            Err(format!("session {session_id} not found"))
        }
    }

    fn read_scrollback_tail(
        &self,
        session_id: u32,
        max_lines: usize,
    ) -> Result<Vec<String>, String> {
        if session_id == 1 {
            let n = max_lines.min(self.scrollback.len());
            let start = self.scrollback.len() - n;
            Ok(self.scrollback[start..].to_vec())
        } else {
            Err(format!("session {session_id} not found"))
        }
    }
}

fn call(server: &McpServer, method: &str, name: Option<&str>, arguments: Value) -> Value {
    let mut params = json!({});
    if let Some(name) = name {
        params["name"] = json!(name);
    }
    if !arguments.is_null() {
        params["arguments"] = arguments;
    }
    let req = JsonRpcRequest {
        jsonrpc: "2.0".into(),
        method: method.into(),
        params,
        id: json!(1),
    };
    server.handle(&req).unwrap()
}

// ---- happy-path tool coverage ----

#[test]
fn read_grid_returns_cells() {
    let store = Arc::new(FakeStore::default());
    let server = McpServer::new(store);
    let result = call(
        &server,
        "tools/call",
        Some("read_grid"),
        json!({"session_id": 1}),
    );
    let grid = &result["content"][0]["data"]["Grid"];
    assert_eq!(grid["rows"], 1);
    assert_eq!(grid["cols"], 2);
    let cells = grid["cells"].as_array().unwrap();
    assert_eq!(cells.len(), 2);
    assert_eq!(cells[0]["codepoint"], 'H' as u32);
    assert_eq!(cells[0]["bold"], true);
}

#[test]
fn read_scrollback_returns_tail() {
    let store = Arc::new(FakeStore::default());
    let server = McpServer::new(store);
    let result = call(
        &server,
        "tools/call",
        Some("read_scrollback"),
        json!({"session_id": 1, "max_lines": 2}),
    );
    let lines = result["content"][0]["data"]["Scrollback"]
        .as_array()
        .unwrap();
    assert_eq!(lines.len(), 2);
    assert_eq!(lines[1], "Cargo.toml");
}

#[test]
fn read_cursor_returns_position() {
    let store = Arc::new(FakeStore::default());
    let server = McpServer::new(store);
    let result = call(
        &server,
        "tools/call",
        Some("read_cursor"),
        json!({"session_id": 1}),
    );
    let c = &result["content"][0]["data"]["Cursor"];
    assert_eq!(c["row"], 3);
    assert_eq!(c["col"], 7);
    assert_eq!(c["visible"], true);
}

#[test]
fn read_selection_returns_text() {
    let store = Arc::new(FakeStore::default());
    let server = McpServer::new(store);
    let result = call(
        &server,
        "tools/call",
        Some("read_selection"),
        json!({"session_id": 1}),
    );
    assert_eq!(result["content"][0]["data"]["Selection"], "hello");
}

#[test]
fn read_title_returns_title() {
    let store = Arc::new(FakeStore::default());
    let server = McpServer::new(store);
    let result = call(
        &server,
        "tools/call",
        Some("read_title"),
        json!({"session_id": 1}),
    );
    assert_eq!(result["content"][0]["data"]["Title"], "my-session");
}

#[test]
fn scrollback_search_returns_matches() {
    let store = Arc::new(FakeStore::default());
    let server = McpServer::new(store);
    let result = call(
        &server,
        "tools/call",
        Some("scrollback_search"),
        json!({"session_id": 1, "pattern": "hello", "max_matches": 10}),
    );
    let m = &result["content"][0]["data"]["SearchMatches"];
    assert_eq!(m.as_array().unwrap().len(), 1);
    assert_eq!(m[0]["text"], "hello");
}

#[test]
fn scrollback_search_empty_when_no_match() {
    let store = Arc::new(FakeStore::default());
    let server = McpServer::new(store);
    let result = call(
        &server,
        "tools/call",
        Some("scrollback_search"),
        json!({"session_id": 1, "pattern": "nomatch", "max_matches": 10}),
    );
    let m = &result["content"][0]["data"]["SearchMatches"];
    assert!(m.as_array().unwrap().is_empty());
}

#[test]
fn terminal_size_is_not_a_tool() {
    let server = McpServer::new(Arc::new(FakeStore::default()));
    // `terminal_size` is a ReadRequest variant but is intentionally not exposed
    // as an MCP tool; it must be rejected as an unknown tool.
    let result = server.handle(&JsonRpcRequest {
        jsonrpc: "2.0".into(),
        method: "tools/call".into(),
        params: json!({"name": "terminal_size", "arguments": {"session_id": 1}}),
        id: json!(1),
    });
    assert!(matches!(result, Err(McpError::UnknownMethod(_))));
}

#[test]
fn list_directory_returns_entries() {
    let store = Arc::new(FakeStore::default());
    let server = McpServer::new(store);
    let result = call(
        &server,
        "tools/call",
        Some("list_directory"),
        json!({"path": "/tmp"}),
    );
    let entries = result["content"][0]["data"]["DirectoryEntries"]
        .as_array()
        .unwrap();
    assert_eq!(entries.len(), 3);
    assert_eq!(entries[2]["name"], "README.md");
    assert_eq!(entries[2]["is_dir"], false);
}

#[test]
fn read_file_returns_lines() {
    let store = Arc::new(FakeStore::default());
    let server = McpServer::new(store);
    let result = call(
        &server,
        "tools/call",
        Some("read_file"),
        json!({"path": "/tmp/x", "max_lines": 100}),
    );
    let fc = &result["content"][0]["data"]["FileContent"];
    assert_eq!(fc["total_lines"], 2);
    assert_eq!(fc["lines"][0], "line one");
}

#[test]
fn read_clipboard_returns_content() {
    let store = Arc::new(FakeStore::default());
    let server = McpServer::new(store);
    let result = call(&server, "tools/call", Some("read_clipboard"), json!({}));
    assert_eq!(
        result["content"][0]["data"]["Clipboard"],
        "clipboard-contents"
    );
}

#[test]
fn scroll_terminal_returns_offset() {
    let store = Arc::new(FakeStore::default());
    let server = McpServer::new(store);
    let result = call(
        &server,
        "tools/call",
        Some("scroll_terminal"),
        json!({"session_id": 1, "lines": 5}),
    );
    assert_eq!(result["content"][0]["data"]["scroll_offset"], 5);
}

// ---- write tools + consent ----

#[test]
fn send_input_writes_to_store() {
    let store = Arc::new(FakeStore::default());
    let server = McpServer::new(store.clone()).with_write_consent();
    let result = call(
        &server,
        "tools/call",
        Some("send_input"),
        json!({"session_id": 1, "data": "ls -la"}),
    );
    assert_eq!(result["content"][0]["text"], "wrote to PTY");
    assert_eq!(store.written.lock().unwrap().len(), 1);
    assert_eq!(store.written.lock().unwrap()[0], b"ls -la");
}

#[test]
fn send_signal_records_signal() {
    let store = Arc::new(FakeStore::default());
    let server = McpServer::new(store.clone()).with_write_consent();
    let result = call(
        &server,
        "tools/call",
        Some("send_signal"),
        json!({"session_id": 1, "signal": "SIGINT"}),
    );
    assert_eq!(result["content"][0]["text"], "sent signal");
    assert_eq!(*store.signals.lock().unwrap(), vec![SignalKind::Interrupt]);
}

#[test]
fn set_terminal_size_records_size() {
    let store = Arc::new(FakeStore::default());
    let server = McpServer::new(store.clone()).with_write_consent();
    let result = call(
        &server,
        "tools/call",
        Some("set_terminal_size"),
        json!({"session_id": 1, "rows": 40, "cols": 120}),
    );
    assert!(
        result["content"][0]["text"]
            .as_str()
            .unwrap()
            .contains("40x120")
    );
    assert_eq!(*store.sizes.lock().unwrap(), vec![(1, 40, 120)]);
}

#[test]
fn write_clipboard_updates_store() {
    let store = Arc::new(FakeStore::default());
    let server = McpServer::new(store.clone()).with_write_consent();
    let result = call(
        &server,
        "tools/call",
        Some("write_clipboard"),
        json!({"text": "new-clip"}),
    );
    assert_eq!(result["content"][0]["text"], "clipboard updated");
    assert_eq!(store.read_clipboard().unwrap(), "new-clip");
}

#[test]
fn raise_notification_records_notification() {
    let store = Arc::new(FakeStore::default());
    let server = McpServer::new(store.clone()).with_write_consent();
    let result = call(
        &server,
        "tools/call",
        Some("raise_notification"),
        json!({"title": "T", "body": "B"}),
    );
    assert_eq!(result["content"][0]["text"], "notification sent");
    assert_eq!(
        *store.notifications.lock().unwrap(),
        vec![("T".to_string(), "B".to_string())]
    );
}

#[test]
fn feed_terminal_output_records_text() {
    let store = Arc::new(FakeStore::default());
    let server = McpServer::new(store.clone()).with_write_consent();
    let result = call(
        &server,
        "tools/call",
        Some("feed_terminal_output"),
        json!({"session_id": 1, "text": "injected"}),
    );
    assert_eq!(result["content"][0]["text"], "output fed to terminal");
    assert_eq!(*store.feeds.lock().unwrap(), vec!["injected".to_string()]);
}

#[test]
fn write_tools_rejected_without_consent() {
    let server = McpServer::new(Arc::new(FakeStore::default()));
    for tool in [
        "send_input",
        "send_signal",
        "set_terminal_size",
        "write_clipboard",
        "raise_notification",
        "feed_terminal_output",
        "queue_terminal_input",
    ] {
        let args = match tool {
            "send_input" => json!({"session_id": 1, "data": "x"}),
            "send_signal" => json!({"session_id": 1, "signal": "SIGINT"}),
            "set_terminal_size" => json!({"session_id": 1, "rows": 1, "cols": 1}),
            "write_clipboard" => json!({"text": "x"}),
            "raise_notification" => json!({"title": "t", "body": "b"}),
            "feed_terminal_output" => json!({"session_id": 1, "text": "x"}),
            "queue_terminal_input" => json!({
                "session_id": 1, "text": "x", "prompt_pattern": "$ ", "timeout_seconds": 1
            }),
            _ => json!({}),
        };
        let result = server.handle(&JsonRpcRequest {
            jsonrpc: "2.0".into(),
            method: "tools/call".into(),
            params: json!({"name": tool, "arguments": args}),
            id: json!(1),
        });
        assert!(
            matches!(result, Err(McpError::InvalidParams(_))),
            "{tool} should require consent"
        );
    }
}

// ---- error / protocol paths ----

#[test]
fn jsonrpc_version_must_be_2_0() {
    let server = McpServer::new(Arc::new(FakeStore::default()));
    let result = server.handle(&JsonRpcRequest {
        jsonrpc: "1.0".into(),
        method: "ping".into(),
        params: json!({}),
        id: json!(1),
    });
    assert!(matches!(result, Err(McpError::InvalidRequest(_))));
}

#[test]
fn unknown_method_returns_error() {
    let server = McpServer::new(Arc::new(FakeStore::default()));
    let result = server.handle(&JsonRpcRequest {
        jsonrpc: "2.0".into(),
        method: "bogus".into(),
        params: json!({}),
        id: json!(1),
    });
    assert!(matches!(result, Err(McpError::UnknownMethod(_))));
}

#[test]
fn unknown_tool_returns_error() {
    let server = McpServer::new(Arc::new(FakeStore::default()));
    let result = server.handle(&JsonRpcRequest {
        jsonrpc: "2.0".into(),
        method: "tools/call".into(),
        params: json!({"name": "no_such_tool", "arguments": {}}),
        id: json!(1),
    });
    assert!(matches!(result, Err(McpError::UnknownMethod(_))));
}

#[test]
fn missing_session_id_is_invalid_params() {
    let server = McpServer::new(Arc::new(FakeStore::default()));
    let result = server.handle(&JsonRpcRequest {
        jsonrpc: "2.0".into(),
        method: "tools/call".into(),
        params: json!({"name": "read_grid", "arguments": {}}),
        id: json!(1),
    });
    assert!(matches!(result, Err(McpError::InvalidParams(_))));
}

#[test]
fn unknown_signal_is_invalid_params() {
    let server = McpServer::new(Arc::new(FakeStore::default())).with_write_consent();
    let result = server.handle(&JsonRpcRequest {
        jsonrpc: "2.0".into(),
        method: "tools/call".into(),
        params: json!({"name": "send_signal", "arguments": {"session_id": 1, "signal": "SIGKILL"}}),
        id: json!(1),
    });
    assert!(matches!(result, Err(McpError::InvalidParams(_))));
}

#[test]
fn session_not_found_propagates_error() {
    let store = Arc::new(FakeStore::default());
    let server = McpServer::new(store);
    let result = server.handle(&JsonRpcRequest {
        jsonrpc: "2.0".into(),
        method: "tools/call".into(),
        params: json!({"name": "read_grid", "arguments": {"session_id": 99}}),
        id: json!(1),
    });
    // store returns Err -> Internal
    assert!(matches!(result, Err(McpError::Internal(_))));
}

#[test]
fn initialize_returns_protocol_version() {
    let server = McpServer::new(Arc::new(FakeStore::default()));
    let result = server.handle(&JsonRpcRequest {
        jsonrpc: "2.0".into(),
        method: "initialize".into(),
        params: json!({}),
        id: json!(1),
    });
    assert_eq!(result.unwrap()["protocolVersion"], "2024-11-05");
}

#[test]
fn tools_list_count_is_21() {
    let server = McpServer::new(Arc::new(FakeStore::default()));
    let result = server.handle(&JsonRpcRequest {
        jsonrpc: "2.0".into(),
        method: "tools/list".into(),
        params: json!({}),
        id: json!(1),
    });
    let tools = result.unwrap()["tools"].as_array().unwrap().to_vec();
    assert_eq!(tools.len(), 21);
}

// ---- input queue delivery with real scrollback ----

#[test]
fn input_queue_delivers_on_pattern_match() {
    let store = Arc::new(FakeStore::default());
    let dyn_store: Arc<dyn SessionStore> = store.clone();
    let queue = InputQueue::new();
    queue.enqueue(1, "whoami".into(), "\r".into(), "Cargo.toml".into(), 5);
    // scrollback tail ends with "Cargo.toml" -> pattern matches
    queue.check_and_deliver(&dyn_store, true);
    assert!(
        store
            .written
            .lock()
            .unwrap()
            .iter()
            .any(|w| w == b"whoami\r")
    );
    assert!(queue.pending().is_empty());
}

#[test]
fn input_queue_expires_without_match() {
    let store = Arc::new(FakeStore::default());
    let dyn_store: Arc<dyn SessionStore> = store.clone();
    let queue = InputQueue::new();
    queue.enqueue(
        1,
        "ls".into(),
        "\r".into(),
        "PROMPT_THAT_NEVER_APPEARS".into(),
        5,
    );
    queue.check_and_deliver(&dyn_store, true);
    // no match and not expired -> still pending
    assert_eq!(queue.pending().len(), 1);
    assert!(store.written.lock().unwrap().is_empty());
}

// ---- unix socket round trip ----

fn spawn_server(
    store: Arc<FakeStore>,
    write_consent: bool,
) -> (std::thread::JoinHandle<()>, std::path::PathBuf) {
    use std::sync::atomic::{AtomicU64, Ordering};
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let dir = std::env::temp_dir();
    let n = COUNTER.fetch_add(1, Ordering::SeqCst);
    let socket = dir.join(format!("torvox-mcp-test-{}-{}.sock", std::process::id(), n));
    let _ = std::fs::remove_file(&socket);
    let handle = {
        let socket = socket.clone();
        std::thread::spawn(move || {
            let _ = serve_unix(&socket, store as Arc<dyn SessionStore>, write_consent);
        })
    };
    // give the listener a moment to bind
    std::thread::sleep(Duration::from_millis(100));
    (handle, socket)
}

fn socket_request(socket: &std::path::Path, request: Value) -> Option<Value> {
    let mut stream = UnixStream::connect(socket).unwrap();
    stream
        .set_read_timeout(Some(Duration::from_millis(500)))
        .unwrap();
    let mut buf = String::new();
    let mut reader = BufReader::new(stream.try_clone().unwrap());
    writeln!(stream, "{request}").unwrap();
    stream.flush().unwrap();
    match reader.read_line(&mut buf) {
        Ok(0) | Err(_) => None,
        Ok(_) => serde_json::from_str(buf.trim()).ok(),
    }
}

#[test]
fn unix_socket_list_sessions_round_trip() {
    let (_handle, socket) = spawn_server(Arc::new(FakeStore::default()), false);
    let req = json!({
        "jsonrpc": "2.0",
        "method": "tools/call",
        "params": {"name": "list_sessions", "arguments": {}},
        "id": 1
    });
    let resp = socket_request(&socket, req).expect("expected a response");
    assert_eq!(resp["jsonrpc"], "2.0");
    assert_eq!(resp["id"], 1);
    let sessions = resp["result"]["content"][0]["data"]["Sessions"]
        .as_array()
        .unwrap();
    assert_eq!(sessions.len(), 1);
    assert_eq!(sessions[0]["id"], 1);
    let _ = std::fs::remove_file(&socket);
}

#[test]
fn unix_socket_read_grid_round_trip() {
    let (_handle, socket) = spawn_server(Arc::new(FakeStore::default()), false);
    let req = json!({
        "jsonrpc": "2.0",
        "method": "tools/call",
        "params": {"name": "read_grid", "arguments": {"session_id": 1}},
        "id": 2
    });
    let resp = socket_request(&socket, req).expect("expected a response");
    assert_eq!(resp["id"], 2);
    assert_eq!(resp["result"]["content"][0]["data"]["Grid"]["rows"], 1);
    let _ = std::fs::remove_file(&socket);
}

#[test]
fn unix_socket_parse_error_returns_32700() {
    let (_handle, socket) = spawn_server(Arc::new(FakeStore::default()), false);
    let mut stream = UnixStream::connect(&socket).unwrap();
    stream
        .set_read_timeout(Some(Duration::from_millis(500)))
        .unwrap();
    let mut reader = BufReader::new(stream.try_clone().unwrap());
    writeln!(stream, "this is not json").unwrap();
    stream.flush().unwrap();
    let mut buf = String::new();
    let resp = match reader.read_line(&mut buf) {
        Ok(n) if n > 0 => serde_json::from_str::<Value>(buf.trim()).unwrap(),
        _ => panic!("expected parse error response"),
    };
    assert_eq!(resp["error"]["code"], -32700);
    let _ = std::fs::remove_file(&socket);
}

#[test]
fn unix_socket_notification_gets_no_response() {
    let (_handle, socket) = spawn_server(Arc::new(FakeStore::default()), false);
    let mut stream = UnixStream::connect(&socket).unwrap();
    stream
        .set_read_timeout(Some(Duration::from_millis(400)))
        .unwrap();
    let mut reader = BufReader::new(stream.try_clone().unwrap());
    // A notification has no "id" -> must not be answered.
    writeln!(
        stream,
        "{{\"jsonrpc\":\"2.0\",\"method\":\"notifications/initialized\"}}"
    )
    .unwrap();
    stream.flush().unwrap();
    let mut buf = String::new();
    let result = reader.read_line(&mut buf);
    assert!(
        result.is_err() || result.unwrap() == 0,
        "notification must not be answered"
    );
    let _ = std::fs::remove_file(&socket);
}

// ---- input queue delivery through MCP server dispatch ----

#[test]
fn input_queue_delivery_through_server() {
    let store: Arc<FakeStore> = Arc::new(FakeStore::default());
    let store_for_observation = store.clone();
    let server = McpServer::new(store as Arc<dyn SessionStore>).with_write_consent();
    // Queue input that will match "Cargo.toml" in the scrollback.
    let result = server.handle(&JsonRpcRequest {
        jsonrpc: "2.0".into(),
        method: "tools/call".into(),
        params: json!({
            "name": "queue_terminal_input",
            "arguments": {
                "session_id": 1,
                "text": "whoami",
                "prompt_pattern": "Cargo.toml",
                "timeout_seconds": 5
            }
        }),
        id: json!(1),
    });
    assert!(result.is_ok());
    // Now trigger check_and_deliver via a harmless tool call.
    // The scrollback already contains "Cargo.toml", so check_and_deliver
    // should match and write "whoami\r" to the store.
    let result = server.handle(&JsonRpcRequest {
        jsonrpc: "2.0".into(),
        method: "tools/call".into(),
        params: json!({ "name": "list_sessions", "arguments": {} }),
        id: json!(2),
    });
    assert!(result.is_ok());
    assert!(
        store_for_observation
            .written
            .lock()
            .unwrap()
            .iter()
            .any(|w| w == b"whoami\r"),
        "check_and_deliver should have written queued input"
    );
}

// ---- stress / concurrent TCP requests ----

#[test]
fn concurrent_tcp_requests() {
    let store: Arc<FakeStore> = Arc::new(FakeStore::default());
    let socket_path = {
        let dir = std::env::temp_dir();
        let n = std::process::id();
        dir.join(format!("torvox-mcp-concurrent-{n}.sock"))
    };
    let _ = std::fs::remove_file(&socket_path);
    let server_store = store.clone();
    let serve_socket = socket_path.clone();
    let handle = std::thread::spawn(move || {
        let _ = serve_unix(&serve_socket, server_store as Arc<dyn SessionStore>, false);
    });
    std::thread::sleep(Duration::from_millis(200));

    let mut threads = Vec::new();
    for i in 0..10u32 {
        let sock = socket_path.clone();
        threads.push(std::thread::spawn(move || {
            match UnixStream::connect(&sock) {
                Ok(mut stream) => {
                    stream.set_read_timeout(Some(Duration::from_secs(2))).ok();
                    let mut reader = BufReader::new(stream.try_clone().unwrap());
                    let req = json!({
                        "jsonrpc": "2.0",
                        "method": "tools/call",
                        "params": {"name": "list_sessions", "arguments": {}},
                        "id": i
                    });
                    let mut buf = String::new();
                    writeln!(stream, "{req}").ok();
                    stream.flush().ok();
                    match reader.read_line(&mut buf) {
                        Ok(n) if n > 0 => {
                            let resp: Value = serde_json::from_str(buf.trim()).unwrap_or_default();
                            Some((i, resp))
                        }
                        _ => None,
                    }
                }
                Err(e) => {
                    eprintln!("concurrent test thread {i} connect error: {e}");
                    None
                }
            }
        }));
    }

    let mut results: Vec<(u32, Value)> = Vec::new();
    for t in threads {
        if let Some(Some((i, resp))) = t.join().ok() {
            results.push((i, resp));
        }
    }
    assert_eq!(
        results.len(),
        10,
        "all 10 concurrent requests should succeed"
    );
    for (i, resp) in &results {
        assert_eq!(resp["id"], *i, "request {i} should preserve id");
        let empty = vec![];
        let sessions = resp["result"]["content"][0]["data"]["Sessions"]
            .as_array()
            .unwrap_or(&empty);
        assert!(!sessions.is_empty(), "request {i} should return sessions");
    }

    drop(handle);
    let _ = std::fs::remove_file(&socket_path);
}

// ---- large payload through TCP ----

#[test]
fn large_payload_through_tcp() {
    let store: Arc<FakeStore> = Arc::new(FakeStore::default());
    let store_clone = store.clone();
    let socket_path = {
        let dir = std::env::temp_dir();
        let n = std::process::id();
        dir.join(format!("torvox-mcp-large-{n}.sock"))
    };
    let _ = std::fs::remove_file(&socket_path);
    let serve_socket = socket_path.clone();
    let handle = std::thread::spawn(move || {
        let _ = serve_unix(&serve_socket, store as Arc<dyn SessionStore>, true);
    });
    std::thread::sleep(Duration::from_millis(200));

    let large_text = "A".repeat(10_000);
    let req = json!({
        "jsonrpc": "2.0",
        "method": "tools/call",
        "params": {"name": "send_input", "arguments": {"session_id": 1, "data": large_text}},
        "id": 1
    });
    match UnixStream::connect(&socket_path) {
        Ok(mut stream) => {
            stream.set_read_timeout(Some(Duration::from_secs(5))).ok();
            let mut reader = BufReader::new(stream.try_clone().unwrap());
            let mut buf = String::new();
            writeln!(stream, "{req}").unwrap();
            stream.flush().unwrap();
            let n = reader.read_line(&mut buf).unwrap();
            assert!(n > 0, "should receive response for large payload");
            let resp: Value = serde_json::from_str(buf.trim()).unwrap();
            assert!(
                resp["result"]["content"][0]["text"]
                    .as_str()
                    .unwrap_or("")
                    .contains("wrote to PTY")
            );
        }
        Err(e) => panic!("connect error: {e}"),
    }

    let written = store_clone.written.lock().unwrap();
    assert!(
        !written.is_empty(),
        "store should have received written data"
    );
    let total: usize = written.iter().map(|w| w.len()).sum();
    assert!(
        total >= 10_000,
        "expected at least 10k bytes written, got {total}"
    );

    drop(handle);
    let _ = std::fs::remove_file(&socket_path);
}

// ---- unicode/internationalization ----

#[test]
fn unicode_input_through_server() {
    let store: Arc<FakeStore> = Arc::new(FakeStore::default());
    let store_clone = store.clone();
    let server = McpServer::new(store as Arc<dyn SessionStore>).with_write_consent();

    // Test CJK characters
    let result = server.handle(&JsonRpcRequest {
        jsonrpc: "2.0".into(),
        method: "tools/call".into(),
        params: json!({
            "name": "send_input",
            "arguments": {"session_id": 1, "data": "echo 你好世界\n"}
        }),
        id: json!(1),
    });
    assert!(result.is_ok());

    // Test emoji
    let result = server.handle(&JsonRpcRequest {
        jsonrpc: "2.0".into(),
        method: "tools/call".into(),
        params: json!({
            "name": "send_input",
            "arguments": {"session_id": 1, "data": "echo 🚀🔥\n"}
        }),
        id: json!(2),
    });
    assert!(result.is_ok());

    // Test mixed script
    let result = server.handle(&JsonRpcRequest {
        jsonrpc: "2.0".into(),
        method: "tools/call".into(),
        params: json!({
            "name": "send_input",
            "arguments": {"session_id": 1, "data": "echo こんにちは 123 µ©\n"}
        }),
        id: json!(3),
    });
    assert!(result.is_ok());

    // Test right-to-left
    let result = server.handle(&JsonRpcRequest {
        jsonrpc: "2.0".into(),
        method: "tools/call".into(),
        params: json!({
            "name": "send_input",
            "arguments": {"session_id": 1, "data": "echo السلام عليكم\n"}
        }),
        id: json!(4),
    });
    assert!(result.is_ok());

    // Verify all inputs were recorded in the store
    let written = store_clone.written.lock().unwrap();
    assert_eq!(written.len(), 4, "should have 4 write records");
    let all_text: String = written.iter().map(|w| String::from_utf8_lossy(w)).collect();
    assert!(all_text.contains("你好世界"), "CJK should be preserved");
    assert!(all_text.contains("🚀🔥"), "emoji should be preserved");
    assert!(
        all_text.contains("こんにちは"),
        "Japanese should be preserved"
    );
    assert!(all_text.contains("السلام"), "Arabic should be preserved");
}

// ---- client disconnect without panic ----

#[test]
fn client_disconnect_no_panic() {
    let store: Arc<FakeStore> = Arc::new(FakeStore::default());
    let socket_path = {
        let dir = std::env::temp_dir();
        let n = std::process::id();
        dir.join(format!("torvox-mcp-disconnect-{n}.sock"))
    };
    let _ = std::fs::remove_file(&socket_path);
    let serve_socket = socket_path.clone();
    let handle = std::thread::spawn(move || {
        let _ = serve_unix(&serve_socket, store as Arc<dyn SessionStore>, false);
    });
    std::thread::sleep(Duration::from_millis(200));

    // Connect and immediately drop without writing anything
    {
        let stream = UnixStream::connect(&socket_path).unwrap();
        // Set a short timeout so the test doesn't hang if something goes wrong
        stream
            .set_read_timeout(Some(Duration::from_millis(100)))
            .ok();
        // Drop without any I/O
    }
    // Give the server a moment to process the disconnect
    std::thread::sleep(Duration::from_millis(50));

    // Server should still be running and accepting connections
    let mut stream2 = UnixStream::connect(&socket_path).unwrap();
    stream2
        .set_read_timeout(Some(Duration::from_secs(1)))
        .unwrap();
    let mut reader = BufReader::new(stream2.try_clone().unwrap());
    writeln!(
        stream2,
        r#"{{"jsonrpc":"2.0","method":"tools/list","params":{{}},"id":1}}"#
    )
    .unwrap();
    stream2.flush().unwrap();
    let mut buf = String::new();
    let n = reader
        .read_line(&mut buf)
        .expect("should get response after reconnect");
    assert!(
        n > 0,
        "server should accept new connection after client disconnect"
    );
    let resp: Value = serde_json::from_str(buf.trim()).unwrap();
    assert!(
        resp.get("result").is_some(),
        "server should respond normally"
    );

    drop(handle);
    let _ = std::fs::remove_file(&socket_path);
}

// ---- write round-trip through TCP ----

#[test]
fn write_round_trip_through_tcp() {
    let store: Arc<FakeStore> = Arc::new(FakeStore::default());
    let store_clone = store.clone();
    let socket_path = {
        let dir = std::env::temp_dir();
        let n = std::process::id();
        dir.join(format!("torvox-mcp-writer-{n}.sock"))
    };
    let _ = std::fs::remove_file(&socket_path);
    let serve_socket = socket_path.clone();
    let handle = std::thread::spawn(move || {
        let _ = serve_unix(&serve_socket, store as Arc<dyn SessionStore>, true);
    });
    std::thread::sleep(Duration::from_millis(200));

    // Send input
    let mut stream = UnixStream::connect(&socket_path).unwrap();
    stream.set_read_timeout(Some(Duration::from_secs(2))).ok();
    let mut reader = BufReader::new(stream.try_clone().unwrap());
    writeln!(
        stream,
        r#"{{"jsonrpc":"2.0","method":"tools/call","params":{{"name":"send_input","arguments":{{"session_id":1,"data":"echo hello\n"}}}},"id":1}}"#
    )
    .unwrap();
    stream.flush().unwrap();
    let mut buf = String::new();
    reader.read_line(&mut buf).unwrap();
    let resp: Value = serde_json::from_str(buf.trim()).unwrap();
    assert!(
        resp["result"]["content"][0]["text"]
            .as_str()
            .unwrap_or("")
            .contains("wrote to PTY")
    );

    // Verify data reached store
    let written = store_clone.written.lock().unwrap();
    assert!(
        !written.is_empty(),
        "data should have been written to store"
    );

    drop(handle);
    let _ = std::fs::remove_file(&socket_path);
}

// ---- property tests: serde round trips ----

#[cfg(test)]
mod property {
    use super::*;
    use quickcheck_macros::quickcheck;

    #[quickcheck]
    fn session_info_roundtrip(id: u32, rows: u32, cols: u32, is_exited: bool) -> bool {
        let s = SessionInfo {
            id,
            title: "t".into(),
            rows,
            cols,
            shell: "s".into(),
            pid: Some(1),
            is_exited,
        };
        let json = serde_json::to_string(&s).unwrap();
        let back: SessionInfo = serde_json::from_str(&json).unwrap();
        back.id == id && back.rows == rows && back.cols == cols && back.is_exited == is_exited
    }

    #[quickcheck]
    fn grid_cell_roundtrip(codepoint: u32, fg: u8, bg: u8, bold: bool) -> bool {
        let c = GridCellData {
            row: 0,
            col: 0,
            codepoint,
            fg_r: fg,
            fg_g: fg,
            fg_b: fg,
            bg_r: bg,
            bg_g: bg,
            bg_b: bg,
            bold,
            italic: false,
            underline: false,
            reverse: false,
            dim: false,
            strikethrough: false,
            blink: false,
            hidden: false,
        };
        let json = serde_json::to_string(&c).unwrap();
        let back: GridCellData = serde_json::from_str(&json).unwrap();
        back.codepoint == codepoint && back.fg_r == fg && back.bold == bold
    }

    #[quickcheck]
    fn search_match_roundtrip(line: u32, start: u32, end: u32) -> bool {
        let m = SearchMatch {
            line_number: line,
            text: "x".into(),
            start_col: start,
            end_col: end,
        };
        let json = serde_json::to_string(&m).unwrap();
        let back: SearchMatch = serde_json::from_str(&json).unwrap();
        back.line_number == line && back.start_col == start && back.end_col == end
    }

    #[quickcheck]
    fn dir_entry_roundtrip(is_dir: bool, size: u64) -> bool {
        let e = DirEntry {
            name: "f".into(),
            is_dir,
            size: Some(size),
            modified: None,
        };
        let json = serde_json::to_string(&e).unwrap();
        let back: DirEntry = serde_json::from_str(&json).unwrap();
        back.is_dir == is_dir && back.size == Some(size)
    }
}
