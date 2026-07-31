#![allow(clippy::duration_suboptimal_units)]

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
//!                    clipboard  notify  terminal_info  ... (8 tools)
//!                           │       │       │
//!                           ▼       ▼       ▼
//!                       McpState — JNI callbacks — Android host
//! ```
//!
//! # Threading model
//!
//! The MCP server runs in a **dedicated background thread** started by
//! [`start()`].  This thread owns its own tokio runtime and never blocks
//! the render thread or JNI callbacks.
//!
//! - [`start()`]: spawns a `std::thread`; creates a `tokio::runtime` inside it.
//! - [`stop()`]: deletes the Unix socket file and attempts a graceful
//!   join with 50ms timeout. If the thread doesn't exit, it's detached
//!   to avoid blocking the caller.
//! - [`run_stdio()`]: meant for standalone mode (Claude Code CLI).  Blocks
//!   the calling thread forever; must be called from outside the embedded
//!   JNI context (i.e., not on the main UI thread).
//!
//! # Concurrency
//!
//! MCP tool handlers (in the background thread) access shared state through:
//! - `McpState` (behind `once_lock!`) — session IDs, enabled flag, callbacks.
//! - JNI callbacks that push **events** into `EVENT_QUEUE` and wait on a
//!   oneshot channel for the Kotlin response.
//!
//! ## Important: MutexGuard must NOT cross `.await` boundaries
//!
//! All dialog/pick-file callbacks extract the closure from its `Mutex`,
//! drop the guard, **then** `.await` the oneshot receiver.  Holding a
//! MutexGuard across `.await` would create a `!Send` future (and block
//! the tokio worker).  The pattern is:
//!
//! ```ignore
//! let cb = state.0.on_show_dialog.lock().unwrap().take();
//! drop(guard);  // MutexGuard released before await
//! let result = rx.await;
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
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::Duration;

use crate::lock_util::lock_or_recover;
use tower_mcp::{
    CallToolResult, McpRouter, StdioTransport, Tool, ToolBuilder, UnixSocketTransport,
    schemars::JsonSchema,
};

// ── Settings ─────────────────────────────────────────────────────────────

static MCP_ENABLED: AtomicBool = AtomicBool::new(false);

/// Global MCP thread handle.
///
/// # Lock seam
/// `stop()` acquires the lock, takes the handle, then **drops the guard**
/// before calling `join()`. This prevents a deadlock if `start()` is called
/// concurrently while a prior thread is still joining.
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
/// Returns `(request_id, receiver)`; the receiver resolves when Kotlin
/// answers via `clipboardResult()`.
type CallbackGetStr = Box<dyn Fn() -> (u64, tokio::sync::oneshot::Receiver<String>) + Send>;
type CallbackDialog = Box<
    dyn Fn(
            u64,
            String,
            String,
            String,
            Vec<String>,
        ) -> (u64, tokio::sync::oneshot::Receiver<String>)
        + Send
        + Sync,
>;
type CallbackPickFile =
    Box<dyn Fn(u64, String, String) -> (u64, tokio::sync::oneshot::Receiver<String>) + Send + Sync>;
type CallbackSendSignal = Box<dyn Fn(u64, i32) -> String + Send + Sync>;

/// Thread-safe state shared between JNI bridge and MCP tools.
#[derive(Clone)]
pub struct McpState(Arc<McpStateInner>);

struct McpStateInner {
    on_notify: Mutex<Option<CallbackStr>>,
    on_toast: Mutex<Option<CallbackStr>>,
    on_open_url: Mutex<Option<CallbackStr>>,
    on_clipboard_get: Mutex<Option<CallbackGetStr>>,
    on_clipboard_set: Mutex<Option<CallbackStr>>,
    on_show_dialog: Mutex<Option<CallbackDialog>>,
    on_pick_file: Mutex<Option<CallbackPickFile>>,
    on_send_signal: Mutex<Option<CallbackSendSignal>>,
    terminal_rows: AtomicU32,
    terminal_cols: AtomicU32,
    active_session_id: AtomicU64,
}

impl McpState {
    pub fn new() -> Self {
        Self(Arc::new(McpStateInner {
            on_notify: Mutex::new(None),
            on_toast: Mutex::new(None),
            on_open_url: Mutex::new(None),
            on_clipboard_get: Mutex::new(None),
            on_clipboard_set: Mutex::new(None),
            on_show_dialog: Mutex::new(None),
            on_pick_file: Mutex::new(None),
            on_send_signal: Mutex::new(None),
            terminal_rows: AtomicU32::new(24),
            terminal_cols: AtomicU32::new(80),
            active_session_id: AtomicU64::new(0),
        }))
    }

    /// Update the terminal dimensions exposed via the `terminal_info` MCP
    /// tool. NOTE: this is a global UI reference value (updated on init and
    /// resize), NOT per-session state — after a session is destroyed it
    /// intentionally keeps the last known dimensions until the next
    /// init/resize overwrites them. Only the ACTIVE session's init/resize
    /// updates the value, and switchSession refreshes it from the newly
    /// active session (ffi.rs guards all three call sites).
    pub fn set_terminal_dims(&self, rows: u32, cols: u32) {
        self.0.terminal_rows.store(rows, Ordering::Release);
        self.0.terminal_cols.store(cols, Ordering::Release);
    }

    /// Update the active session ID (called from JNI bridge on switch).
    pub fn set_active_session_id(&self, id: u64) {
        self.0.active_session_id.store(id, Ordering::Release);
    }

    pub fn set_notify_handler<F: Fn(String) + Send + 'static>(&self, f: F) {
        *lock_or_recover(&self.0.on_notify, "mcp: set_notify") = Some(Box::new(f));
    }

    pub fn set_toast_handler<F: Fn(String) + Send + 'static>(&self, f: F) {
        *lock_or_recover(&self.0.on_toast, "mcp: set_toast") = Some(Box::new(f));
    }

    pub fn set_open_url_handler<F: Fn(String) + Send + 'static>(&self, f: F) {
        *lock_or_recover(&self.0.on_open_url, "mcp: set_open_url") = Some(Box::new(f));
    }

    pub fn set_clipboard_get_handler<
        F: Fn() -> (u64, tokio::sync::oneshot::Receiver<String>) + Send + 'static,
    >(
        &self,
        f: F,
    ) {
        *lock_or_recover(&self.0.on_clipboard_get, "mcp: set_clipboard_get") = Some(Box::new(f));
    }

    pub fn set_clipboard_set_handler<F: Fn(String) + Send + 'static>(&self, f: F) {
        *lock_or_recover(&self.0.on_clipboard_set, "mcp: set_clipboard_set") = Some(Box::new(f));
    }

    /// Set a handler for showing dialogs. Called from the MCP `dialog` tool.
    /// The handler returns a oneshot receiver that resolves when the user responds.
    /// `options` contains selectable choices for "select" type dialogs.
    pub fn set_dialog_handler<F>(&self, f: F)
    where
        F: Fn(
                u64,
                String,
                String,
                String,
                Vec<String>,
            ) -> (u64, tokio::sync::oneshot::Receiver<String>)
            + Send
            + Sync
            + 'static,
    {
        *lock_or_recover(&self.0.on_show_dialog, "mcp: set_dialog") = Some(Box::new(f));
    }

    /// Set a handler for file picking. Called from the MCP `pick_file` tool.
    /// The handler returns a oneshot receiver that resolves when the user picks a file.
    pub fn set_pick_file_handler<F>(&self, f: F)
    where
        F: Fn(u64, String, String) -> (u64, tokio::sync::oneshot::Receiver<String>)
            + Send
            + Sync
            + 'static,
    {
        *lock_or_recover(&self.0.on_pick_file, "mcp: set_pick_file") = Some(Box::new(f));
    }

    pub fn set_send_signal_handler<F>(&self, f: F)
    where
        F: Fn(u64, i32) -> String + Send + Sync + 'static,
    {
        *lock_or_recover(&self.0.on_send_signal, "mcp: set_send_signal") = Some(Box::new(f));
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

/// Input for send_signal.
#[derive(JsonSchema, Deserialize)]
struct SendSignalInput {
    signal: i32,
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
            let (session_id, pending) = {
                let state = global_state();
                let guard =
                    lock_or_recover(&state.0.on_clipboard_get, "mcp: clipboard_get_handler");
                let session_id = state.0.active_session_id.load(Ordering::Acquire);
                (session_id, guard.as_ref().map(|f| f()))
            }; // guard dropped before await
            match pending {
                Some((request_id, rx)) => {
                    match tokio::time::timeout(Duration::from_secs(300), rx).await {
                        // Empty text is a legitimate result (empty clipboard),
                        // not a failure. Only a dropped sender (session closed)
                        // or the timeout is reported as an error.
                        Ok(Ok(text)) => Ok(CallToolResult::text(text)),
                        _ => {
                            crate::android::ffi::cancel_request(session_id, request_id);
                            Ok(CallToolResult::error(
                                "Clipboard read cancelled or timed out",
                            ))
                        }
                    }
                }
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
            let guard = lock_or_recover(&state.0.on_clipboard_set, "mcp: clipboard_set_handler");
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
            let guard = lock_or_recover(&state.0.on_notify, "mcp: notify_handler");
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
            let guard = lock_or_recover(&state.0.on_toast, "mcp: toast_handler");
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
            let guard = lock_or_recover(&state.0.on_open_url, "mcp: open_url_handler");
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

fn send_signal_tool() -> Tool {
    ToolBuilder::new("send_signal")
        .title("Send signal to terminal")
        .description("Send a POSIX signal (by number) to the foreground process in the active terminal session. Common signals: 2 (SIGINT), 3 (SIGQUIT), 9 (SIGKILL), 15 (SIGTERM), 20 (SIGTSTP).")
        // Round-98 note: the handler synchronously takes the session
        // registry read lock + the session lock on the MCP worker thread
        // (no await inside). It blocks up to a pollEvent frame or a 50ms
        // focus-event query while the UI thread holds the session lock —
        // brief and deadlock-free (lock order is the global one), but it
        // DOES briefly block the MCP worker, unlike the pure snapshot
        // reads of the other tools.
        .handler(|input: SendSignalInput| async move {
            let state = global_state();
            let guard = lock_or_recover(&state.0.on_send_signal, "mcp: send_signal_handler");
            match guard.as_ref() {
                Some(f) => {
                    let session_id = state.0.active_session_id.load(Ordering::Acquire);
                    let result = f(session_id, input.signal);
                    Ok(CallToolResult::text(result))
                }
                None => Ok(CallToolResult::error("send_signal not available")),
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
            let (session_id, rx) = {
                let state = global_state();
                let guard = lock_or_recover(&state.0.on_pick_file, "mcp: pick_file_handler");
                let session_id = state.0.active_session_id.load(Ordering::Acquire);
                (
                    session_id,
                    guard
                        .as_ref()
                        .map(|callback| callback(session_id, input.directory, input.pattern)),
                )
            }; // guard + state drop before await
            match rx {
                Some((request_id, rx)) => {
                    match tokio::time::timeout(Duration::from_secs(300), rx).await {
                        Ok(Ok(path)) if !path.is_empty() => Ok(CallToolResult::text(path)),
                        _ => {
                            // Drop the pending registry entry so a
                            // never-answered picker cannot leak one Sender
                            // per call. The request_id is only known here,
                            // after the callback registered it.
                            crate::android::ffi::cancel_request(session_id, request_id);
                            Ok(CallToolResult::error("File picker cancelled or timed out"))
                        }
                    }
                }
                None => Ok(CallToolResult::error("File picker not available")),
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
            let (session_id, rx) = {
                let state = global_state();
                let guard = lock_or_recover(&state.0.on_show_dialog, "mcp: dialog_handler");
                let session_id = state.0.active_session_id.load(Ordering::Acquire);
                (
                    session_id,
                    guard.as_ref().map(|callback| {
                        callback(
                            session_id,
                            input.dialog_type,
                            input.title,
                            input.message,
                            input.options,
                        )
                    }),
                )
            }; // guard + state drop before await
            match rx {
                Some((request_id, rx)) => {
                    match tokio::time::timeout(Duration::from_secs(300), rx).await {
                        Ok(Ok(result)) => Ok(CallToolResult::text(result)),
                        _ => {
                            crate::android::ffi::cancel_request(session_id, request_id);
                            Ok(CallToolResult::error("Dialog cancelled or timed out"))
                        }
                    }
                }
                None => Ok(CallToolResult::error("Dialog not available")),
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
        .tool(send_signal_tool())
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
    // Check both is_enabled AND MCP_THREAD under a single lock acquisition
    // to prevent TOCTOU between the two checks.
    let mut guard = match MCP_THREAD.lock() {
        Ok(g) => g,
        Err(poisoned) => {
            log::error!("MCP_THREAD lock poisoned in start()");
            poisoned.into_inner()
        }
    };
    if guard.is_some() {
        log::info!("MCP server already running");
        return;
    }
    // Re-check enabled under lock to catch set_enabled(false) just before us
    if !is_enabled() {
        log::info!("MCP server is disabled via settings");
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

    // Build the runtime BEFORE spawning the thread: if it fails, nothing
    // was registered in MCP_THREAD, so set_enabled(true) can retry later
    // (a failed runtime left behind would make the server permanently
    // "already running" with a dead handle).
    let runtime = match tokio::runtime::Builder::new_current_thread()
        .enable_io()
        .build()
    {
        Ok(r) => r,
        Err(e) => {
            log::error!("MCP server: failed to build tokio runtime: {e}");
            return;
        }
    };

    let handle = std::thread::spawn(move || {
        runtime.block_on(async {
            let router = build_router();
            if let Err(e) = UnixSocketTransport::new(router).serve(&path).await {
                log::error!("MCP server error: {e}");
            }
            // NOTE: deliberately NOT removing the socket file here. stop()
            // removes it while the server is known to be shutting down; a
            // detached thread removing it later could unlink a NEW server's
            // socket bound at the same path after a quick restart. A stale
            // file left by an abnormal exit is harmless — start() removes
            // it before binding.
        });
    });

    // Store JoinHandle for later stop — guard is still held from the
    // check-and-lock above, so no TOCTOU window exists.
    *guard = Some(handle);
}

/// Stop the MCP server.
///
/// Deletes the Unix socket file and attempts a graceful join of the server
/// thread with a 50ms timeout. If the thread doesn't exit in time, it is
/// detached to avoid blocking the calling thread (JNI main thread) indefinitely.
///
/// The deleted socket file allows a new server to bind to the same path
/// immediately. The detached thread will clean up on its own when accept()
/// eventually returns or the process exits.
pub fn stop() {
    // Take the handle OUT of the lock, then drop the lock immediately
    // so the timeout loop is NOT called while holding MCP_THREAD.
    let handle = match MCP_THREAD.lock() {
        Ok(mut guard) => guard.take(),
        Err(poisoned) => {
            log::error!("MCP_THREAD lock poisoned in stop()");
            poisoned.into_inner().take()
        }
    };
    // guard dropped here — lock released before join/sleep

    let Some(handle) = handle else {
        return;
    };

    let path = socket_path();
    let _ = std::fs::remove_file(&path);

    // Try graceful join with 50ms timeout.
    // is_finished() + join() is safe: join() after is_finished() is
    // guaranteed immediate.
    let deadline = std::time::Instant::now() + std::time::Duration::from_millis(50);
    while std::time::Instant::now() < deadline {
        if handle.is_finished() {
            if let Err(panic) = handle.join() {
                log::error!("MCP server thread panicked: {:?}", panic);
            }
            log::info!("MCP server stopped");
            return;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    log::warn!(
        "MCP server thread did not exit within 50ms — DETACHING (will exit \
         when accept returns)"
    );
    // handle dropped here → thread detached
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
        assert!(names.contains(&"send_signal"));
        assert!(names.contains(&"pick_file"));
        assert!(names.contains(&"dialog"));
        assert_eq!(names.len(), 9);
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
        match &result {
            serde_json::Value::Object(obj) => {
                let msg = obj.get("message").and_then(|v| v.as_str()).unwrap_or("");
                assert!(
                    msg.contains("not found"),
                    "nonexistent tool should return a not-found error, got: {result}"
                );
            }
            other => {
                panic!("nonexistent tool should return a JSON-RPC error object, got: {other}");
            }
        }
    }
}
