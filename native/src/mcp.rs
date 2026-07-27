//! MCP server — Model Context Protocol (MCP) for AI agents.
//!
//! Implements the standard MCP protocol over two transports:
//!
//! - **HTTP** (via axum + Unix socket): embedded in the native process,
//!   used by tools like `curl` or any HTTP MCP client.
//! - **Stdio** (stdin/stdout): for Claude Code, Codex CLI, OpenCode, etc.
//!
//! ## Architecture
//!
//! ```text
//! AI Agent (Claude Code / Codex CLI / OpenCode)
//!   │
//!   ├── Stdio ──► StdioTransport ──┐
//!   │                              │
//!   └── HTTP ──► axum::Router ─────┤
//!                                  ▼
//!                          McpRouter (tower-mcp)
//!                           │       │       │
//!                    clipboard  notify  terminal_info  ... (7 tools)
//!                           │       │       │
//!                           ▼       ▼       ▼
//!                       McpState — JNI callbacks — Android host
//! ```
//!
//! ## Usage (JNI bridge)
//!
//! ```ignore
//! let state = mcp::McpState::new();
//! state.set_clipboard_get_handler(|| { /* JNI call */ });
//! mcp::set_enabled(true);
//! mcp::start_http(state); // background tokio runtime
//! ```
//!
//! ## Usage (Stdio — standalone binary)
//!
//! ```ignore
//! let state = mcp::McpState::new();
//! mcp::run_stdio(state).await?;
//! ```

use serde::Deserialize;
use serde_json::json;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use tower_mcp::{
    CallToolResult, McpRouter, StdioTransport, Tool, ToolBuilder, UnixSocketTransport,
    schemars::JsonSchema,
};

// ── Settings ─────────────────────────────────────────────────────────────

static MCP_ENABLED: AtomicBool = AtomicBool::new(false);
static MCP_THREAD: Mutex<Option<JoinHandle<()>>> = Mutex::new(None);

/// Enable or disable the MCP server.
pub fn set_enabled(enabled: bool) {
    MCP_ENABLED.store(enabled, Ordering::Release);
    if enabled {
        start();
    } else {
        stop();
    }
}

/// Whether the MCP server is enabled.
pub fn is_enabled() -> bool {
    MCP_ENABLED.load(Ordering::Acquire)
}

// ── Shared state ─────────────────────────────────────────────────────────

type CallbackStr = Box<dyn Fn(String) + Send>;
type CallbackGetStr = Box<dyn Fn() -> String + Send>;

/// Thread-safe state shared between JNI bridge and MCP tools.
#[derive(Clone)]
pub struct McpState(Arc<McpStateInner>);

struct McpStateInner {
    on_notify: Mutex<Option<CallbackStr>>,
    on_toast: Mutex<Option<CallbackStr>>,
    on_open_url: Mutex<Option<CallbackStr>>,
    on_clipboard_get: Mutex<Option<CallbackGetStr>>,
    on_clipboard_set: Mutex<Option<CallbackStr>>,
    terminal_rows: AtomicU32,
    terminal_cols: AtomicU32,
}

impl McpState {
    pub fn new() -> Self {
        Self(Arc::new(McpStateInner {
            on_notify: Mutex::new(None),
            on_toast: Mutex::new(None),
            on_open_url: Mutex::new(None),
            on_clipboard_get: Mutex::new(None),
            on_clipboard_set: Mutex::new(None),
            terminal_rows: AtomicU32::new(24),
            terminal_cols: AtomicU32::new(80),
        }))
    }

    pub fn set_terminal_dims(&self, rows: u32, cols: u32) {
        self.0.terminal_rows.store(rows, Ordering::Release);
        self.0.terminal_cols.store(cols, Ordering::Release);
    }

    pub fn set_notify_handler<F: Fn(String) + Send + 'static>(&self, f: F) {
        *self.0.on_notify.lock().unwrap() = Some(Box::new(f));
    }

    pub fn set_toast_handler<F: Fn(String) + Send + 'static>(&self, f: F) {
        *self.0.on_toast.lock().unwrap() = Some(Box::new(f));
    }

    pub fn set_open_url_handler<F: Fn(String) + Send + 'static>(&self, f: F) {
        *self.0.on_open_url.lock().unwrap() = Some(Box::new(f));
    }

    pub fn set_clipboard_get_handler<F: Fn() -> String + Send + 'static>(&self, f: F) {
        *self.0.on_clipboard_get.lock().unwrap() = Some(Box::new(f));
    }

    pub fn set_clipboard_set_handler<F: Fn(String) + Send + 'static>(&self, f: F) {
        *self.0.on_clipboard_set.lock().unwrap() = Some(Box::new(f));
    }
}

impl Default for McpState {
    fn default() -> Self {
        Self::new()
    }
}

static GLOBAL_STATE: std::sync::LazyLock<McpState> = std::sync::LazyLock::new(McpState::new);

/// Get the global MCP state (for registering callbacks from JNI).
pub(crate) fn global_state() -> &'static McpState {
    &GLOBAL_STATE
}

// ── Input types for tools ────────────────────────────────────────────────

/// Input for clipboard_set.
#[derive(JsonSchema, Deserialize)]
struct ClipboardSetInput {
    text: String,
}

/// Input for notify.
#[derive(JsonSchema, Deserialize)]
struct NotifyInput {
    #[serde(default)]
    title: String,
    #[serde(default)]
    body: String,
}

/// Input for toast.
#[derive(JsonSchema, Deserialize)]
struct ToastInput {
    text: String,
}

/// Input for open_url.
#[derive(JsonSchema, Deserialize)]
struct OpenUrlInput {
    url: String,
}

// ── Tool definitions ─────────────────────────────────────────────────────

fn terminal_info_tool() -> Tool {
    ToolBuilder::new("terminal_info")
        .title("Get terminal info")
        .description("Get terminal dimensions (rows, columns) and version info")
        .no_params_handler(|| async move {
            let state = global_state();
            let rows = state.0.terminal_rows.load(Ordering::Acquire);
            let cols = state.0.terminal_cols.load(Ordering::Acquire);
            let info = json!({
                "rows": rows,
                "columns": cols,
                "version": env!("CARGO_PKG_VERSION"),
            });
            Ok(CallToolResult::text(info.to_string()))
        })
        .build()
}

fn clipboard_get_tool() -> Tool {
    ToolBuilder::new("clipboard_get")
        .title("Get clipboard content")
        .description("Read the current system clipboard text")
        .no_params_handler(|| async move {
            let state = global_state();
            let guard = state.0.on_clipboard_get.lock().unwrap();
            match guard.as_ref() {
                Some(f) => Ok(CallToolResult::text(f())),
                None => Ok(CallToolResult::error(
                    "Clipboard not available on this platform",
                )),
            }
        })
        .build()
}

fn clipboard_set_tool() -> Tool {
    ToolBuilder::new("clipboard_set")
        .title("Set clipboard content")
        .description("Write text to the system clipboard")
        .handler(|input: ClipboardSetInput| async move {
            let state = global_state();
            let guard = state.0.on_clipboard_set.lock().unwrap();
            match guard.as_ref() {
                Some(f) => {
                    f(input.text);
                    Ok(CallToolResult::text("Clipboard updated"))
                }
                None => Ok(CallToolResult::error("Clipboard not available")),
            }
        })
        .build()
}

fn notify_tool() -> Tool {
    ToolBuilder::new("notify")
        .title("Send notification")
        .description("Show a system notification with title and body")
        .handler(|input: NotifyInput| async move {
            let state = global_state();
            let guard = state.0.on_notify.lock().unwrap();
            match guard.as_ref() {
                Some(f) => {
                    let msg = if input.body.is_empty() {
                        input.title
                    } else {
                        format!("{}\n{}", input.title, input.body)
                    };
                    f(msg);
                    Ok(CallToolResult::text("Notification sent"))
                }
                None => Ok(CallToolResult::error("Notifications not available")),
            }
        })
        .build()
}

fn toast_tool() -> Tool {
    ToolBuilder::new("toast")
        .title("Show toast")
        .description("Show a brief toast message on screen")
        .handler(|input: ToastInput| async move {
            let state = global_state();
            let guard = state.0.on_toast.lock().unwrap();
            match guard.as_ref() {
                Some(f) => {
                    f(input.text);
                    Ok(CallToolResult::text("Toast shown"))
                }
                None => Ok(CallToolResult::error("Toast not available")),
            }
        })
        .build()
}

fn open_url_tool() -> Tool {
    ToolBuilder::new("open_url")
        .title("Open URL")
        .description("Open a URL in the default browser")
        .handler(|input: OpenUrlInput| async move {
            let state = global_state();
            let guard = state.0.on_open_url.lock().unwrap();
            match guard.as_ref() {
                Some(f) => {
                    f(input.url);
                    Ok(CallToolResult::text("URL opened"))
                }
                None => Ok(CallToolResult::error("URL opening not available")),
            }
        })
        .build()
}

fn pick_file_tool() -> Tool {
    #[derive(Deserialize, JsonSchema)]
    struct PickFileInput {
        /// Starting directory
        #[serde(default)]
        directory: String,
        /// File filter pattern (e.g. "*.txt")
        #[serde(default)]
        pattern: String,
    }

    ToolBuilder::new("pick_file")
        .title("Pick file")
        .description("Open a system file picker dialog")
        .handler(|input: PickFileInput| async move {
            #[cfg(target_os = "android")]
            {
                let (request_id, rx) = crate::android::ffi::register_request(0);
                crate::android::ffi::push_event(crate::android::ffi::Event::PickFile {
                    session_id: 0,
                    request_id,
                    starting_path: input.directory,
                    filter: input.pattern,
                });
                match tokio::time::timeout(std::time::Duration::from_secs(300), rx).await {
                    Ok(Ok(path)) => {
                        if path.is_empty() {
                            Ok(CallToolResult::error("No file selected"))
                        } else {
                            Ok(CallToolResult::text(path))
                        }
                    }
                    _ => Ok(CallToolResult::error("File picker cancelled or timed out")),
                }
            }
            #[cfg(not(target_os = "android"))]
            {
                let _ = (input.directory, input.pattern);
                Ok(CallToolResult::error(
                    "File picker not yet implemented on this platform",
                ))
            }
        })
        .build()
}

fn dialog_tool() -> Tool {
    #[derive(Deserialize, JsonSchema)]
    struct DialogInput {
        /// "confirm", "input", or "select"
        dialog_type: String,
        title: String,
        message: String,
        /// Options for "select" type (ignored for confirm/input)
        #[serde(default)]
        options: Vec<String>,
    }

    ToolBuilder::new("dialog")
        .title("Show dialog")
        .description("Prompt the user with a dialog (confirm, input, or select)")
        .handler(|input: DialogInput| async move {
            #[cfg(target_os = "android")]
            {
                let (request_id, rx) = crate::android::ffi::register_request(0);
                crate::android::ffi::push_event(crate::android::ffi::Event::ShowDialog {
                    session_id: 0,
                    request_id,
                    dialog_type: input.dialog_type,
                    title: input.title,
                    message: input.message,
                });
                match tokio::time::timeout(std::time::Duration::from_secs(300), rx).await {
                    Ok(Ok(result)) => Ok(CallToolResult::text(result)),
                    _ => Ok(CallToolResult::error("Dialog cancelled or timed out")),
                }
            }
            #[cfg(not(target_os = "android"))]
            {
                let _ = (input.dialog_type, input.title, input.message, input.options);
                Ok(CallToolResult::error(
                    "Dialog not supported on this platform",
                ))
            }
        })
        .build()
}

// ── Router construction ──────────────────────────────────────────────────

fn build_router() -> McpRouter {
    McpRouter::new()
        .server_info("terminal", env!("CARGO_PKG_VERSION"))
        .tool(terminal_info_tool())
        .tool(clipboard_get_tool())
        .tool(clipboard_set_tool())
        .tool(notify_tool())
        .tool(toast_tool())
        .tool(open_url_tool())
        .tool(pick_file_tool())
        .tool(dialog_tool())
}

// ── Server lifecycle ─────────────────────────────────────────────────────

fn socket_path() -> String {
    #[cfg(target_os = "android")]
    {
        "/data/data/com.termux/run/mcp.sock".to_string()
    }
    #[cfg(not(target_os = "android"))]
    {
        "/tmp/mcp.sock".to_string()
    }
}

/// Start the MCP server in Unix socket mode.
///
/// Spawns a tokio runtime in a background thread. The server listens on
/// the configured socket path and serves the standard MCP protocol.
/// AI agents connect via `.mcp.json` with `"type": "unix"` and
/// `"path": "/tmp/mcp.sock"`.
pub fn start() {
    if !is_enabled() {
        log::info!("MCP server is disabled via settings");
        return;
    }
    // Don't start if already running
    if let Ok(guard) = MCP_THREAD.lock()
        && guard.is_some()
    {
        log::info!("MCP server already running");
        return;
    }

    let path = socket_path();

    // Remove stale socket file
    let _ = std::fs::remove_file(&path);

    // Ensure parent directory exists
    if let Some(parent) = std::path::Path::new(&path).parent() {
        let _ = std::fs::create_dir_all(parent);
    }

    log::info!("MCP server starting on Unix socket: {path}");

    let handle = std::thread::spawn(move || {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_io()
            .build()
            .expect("failed to build tokio runtime for MCP");

        rt.block_on(async {
            let router = build_router();
            if let Err(e) = UnixSocketTransport::new(router).serve(&path).await {
                log::error!("MCP server error: {e}");
            }
            // Clean up socket on shutdown
            let _ = std::fs::remove_file(&path);
        });
    });

    // Store JoinHandle for later stop
    if let Ok(mut guard) = MCP_THREAD.lock() {
        *guard = Some(handle);
    }
}

/// Stop the MCP server.
pub fn stop() {
    if let Ok(mut guard) = MCP_THREAD.lock()
        && guard.take().is_some()
    {
        log::info!("MCP server stopped");
    }
}

/// Start the MCP server in stdio mode (for Claude Code / Codex CLI).
///
/// Reads JSON-RPC from stdin, writes to stdout. Call this from a
/// CLI subcommand (`terminal-mcp`) to support `"command": "terminal-mcp"`
/// in `.mcp.json`.
pub async fn run_stdio() -> Result<(), tower_mcp::Error> {
    if !is_enabled() {
        log::warn!("MCP server is disabled");
        return Ok(());
    }
    let router = build_router();
    let mut transport = StdioTransport::new(router);
    transport.run().await
}

// ── Tests ────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{Value, json};
    use tower_mcp::testing::TestClient;

    #[tokio::test]
    async fn test_list_tools() {
        let router = build_router();
        let mut client = TestClient::from_router(router);
        client.initialize().await;

        let tools = client.list_tools().await;
        let names: Vec<&str> = tools
            .iter()
            .filter_map(|v| v.get("name").and_then(|n| n.as_str()))
            .collect();

        assert!(names.contains(&"terminal_info"));
        assert!(names.contains(&"clipboard_get"));
        assert!(names.contains(&"clipboard_set"));
        assert!(names.contains(&"notify"));
        assert!(names.contains(&"toast"));
        assert!(names.contains(&"open_url"));
        assert!(names.contains(&"pick_file"));
        assert!(names.contains(&"dialog"));
        assert_eq!(names.len(), 8);
    }

    #[tokio::test]
    async fn test_terminal_info_tool() {
        let router = build_router();
        let mut client = TestClient::from_router(router);
        client.initialize().await;

        let result = client.call_tool("terminal_info", json!({})).await;
        assert!(!result.is_error);

        let text = result.content.first().unwrap().as_text().unwrap();
        let info: Value = serde_json::from_str(text).unwrap();
        assert!(info["rows"].is_number());
        assert!(info["columns"].is_number());
    }

    #[tokio::test]
    async fn test_clipboard_set_requires_text() {
        let router = build_router();
        let mut client = TestClient::from_router(router);
        client.initialize().await;

        let result = client.call_tool("clipboard_set", json!({})).await;
        // Should fail because `text` is required
        assert!(result.is_error);
    }

    #[tokio::test]
    async fn test_method_not_found() {
        let router = build_router();
        let mut client = TestClient::from_router(router);
        client.initialize().await;

        let result = client
            .call_tool_expect_error("nonexistent", json!({}))
            .await;
        assert!(
            result.is_object() || result.as_str().is_some(),
            "nonexistent tool should return an error response: {result}"
        );
    }
}
