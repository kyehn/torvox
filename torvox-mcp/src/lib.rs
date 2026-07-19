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

pub mod input_queue;
pub mod serve;
pub mod server;
pub mod types;

pub use input_queue::InputQueue;
pub use serve::{serve_tcp, serve_unix};
pub use server::McpServer;
pub use types::*;

/// Real `SessionStore` backend built on `torvox-terminal` (feature `live`).
#[cfg(feature = "live")]
pub mod live;

/// Functional in-memory `SessionStore` backend for testing / demos (feature `mock`).
#[cfg(feature = "mock")]
pub mod mock;

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
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
        let store = std::sync::Arc::new(MockStore::new());
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
        let store = std::sync::Arc::new(MockStore::new());
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
        let store = std::sync::Arc::new(MockStore::new());
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
        let store = std::sync::Arc::new(MockStore::new());
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
            let store = std::sync::Arc::new(MockStore::new());
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
        let store = std::sync::Arc::new(MockStore::new());
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
        let store = std::sync::Arc::new(MockStore::new());
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
        let store = std::sync::Arc::new(MockStore::new());
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
        let store = std::sync::Arc::new(MockStore::new());
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
        let store = std::sync::Arc::new(MockStore::new());
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
        let store = std::sync::Arc::new(MockStore::new());
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
        let store = std::sync::Arc::new(MockStore::new());
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
        let store = std::sync::Arc::new(MockStore::new());
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
        let store = std::sync::Arc::new(MockStore::new());
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
        let store = std::sync::Arc::new(MockStore::new());
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
        let store = std::sync::Arc::new(MockStore::new());
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
        let store = std::sync::Arc::new(MockStore::new());
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
        assert!(preview.ends_with('\u{2026}'));
    }

    #[test]
    fn input_queue_check_and_deliver_no_consent() {
        let q = InputQueue::new();
        q.enqueue(1, "test".into(), "\r".into(), "$ ".into(), 60);
        let store: std::sync::Arc<dyn SessionStore> = std::sync::Arc::new(MockStore::new());
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
        let store = std::sync::Arc::new(MockStore::new());
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
        let store = std::sync::Arc::new(MockStore::new());
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
        let store = std::sync::Arc::new(MockStore::new());
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
        let store = std::sync::Arc::new(MockStore::new());
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
        let store = std::sync::Arc::new(MockStore::new());
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
        let store = std::sync::Arc::new(MockStore::new());
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
        let store = std::sync::Arc::new(MockStore::new());
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
        let store = std::sync::Arc::new(MockStore::new());
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
        let store = std::sync::Arc::new(MockStore::new());
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
        let store = std::sync::Arc::new(MockStore::new());
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
        let store = std::sync::Arc::new(MockStore::new());
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
        let store = std::sync::Arc::new(MockStore::new());
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
        let store = std::sync::Arc::new(MockStore::new());
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
        let store = std::sync::Arc::new(MockStore::new());
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
        let store = std::sync::Arc::new(MockStore::new());
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
        let store = std::sync::Arc::new(MockStore::new());
        let server = McpServer::new(store);
        let req = JsonRpcRequest {
            jsonrpc: "2.0".into(),
            method: "tools/call".into(),
            params: json!({ "name": "read_clipboard", "arguments": {} }),
            id: json!(1),
        };
        let result = server.handle(&req).unwrap();
        assert_eq!(result["content"][0]["data"]["Clipboard"], "mock clipboard");
    }

    #[test]
    fn handle_read_file_tool() {
        let store = std::sync::Arc::new(MockStore::new());
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
