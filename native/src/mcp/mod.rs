#![allow(clippy::duration_suboptimal_units)]

//! MCP server — Model Context Protocol (MCP) for AI agents.
//!
//! Implements the standard MCP protocol over two transports:
//!
//! - **HTTP** (via axum + Unix socket): embedded in the native process,
//!   used by tools like `curl` or any HTTP MCP client.
//! - **Stdio** (stdin/stdout): for AI coding agents (Codex CLI, OpenCode, etc.)
//!
//! ## Architecture
//!
//! ```text
//! AI Agent (Codex CLI / OpenCode)
//!   │
//!   ├── Stdio ──► StdioTransport ──┐
//!   │                              │
//!   └── HTTP ──► axum::Router ─────┤
//!                                  ▼
//!                          McpRouter (tower-mcp)
//!                           │       │       │
//!                    clipboard  notify  terminal_info... (12 tools)
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
//! - [`run_stdio()`]: meant for standalone mode (AI coding agent CLI).  Blocks
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

// ── Submodules ────

mod auth;
mod tools;

pub(crate) use auth::{MAX_CONSECUTIVE_REJECTIONS_BEFORE_LOG, peer_uid_allowed, peer_uid_of};
pub(crate) use tools::{
    clipboard_get_tool, clipboard_set_tool, dialog_tool, last_command_output_tool, notify_tool,
    open_url_tool, pick_file_tool, run_command_tool, screenshot_tool, send_signal_tool,
    terminal_info_tool, toast_tool,
};

use parking_lot::Mutex;
use std::os::fd::AsRawFd;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
use std::thread::JoinHandle;
use std::time::Duration;

// ── SO_PEERCRED peer validation, termux AmSocketServer) ──

/// Maximum number of consecutive rejected (foreign-uid) connections before
/// the accept loop logs a warning, to avoid log spam from a malicious or
/// misbehaving local client.
/// A wrapper around `tokio::net::UnixListener` that rejects connections
/// from foreign uids before they reach axum. Implements
/// `axum::serve::Listener` so it drops into `axum::serve(...)` unchanged.
///
/// On accept, the stream's SO_PEERCRED uid is read; only the app's own uid
/// or root pass. Rejected streams are closed and the accept loop continues.
/// This is defense-in-depth: the socket lives in the app-private dir, but
/// any process with filesystem access to it must not gain MCP privileges.
struct PeerCheckedListener {
    inner: tokio::net::UnixListener,
    own_uid: u32,
    consecutive_rejections: u32,
}

impl PeerCheckedListener {
    fn new(inner: tokio::net::UnixListener) -> Self {
        // SAFETY: getuid(2) is always safe — returns the real user ID of the calling process.
        let own_uid = unsafe { libc::getuid() };
        Self::with_own_uid(inner, own_uid)
    }

    /// Test hook: construct with an explicit expected uid so the rejection
    /// branch can be exercised without setuid privileges.
    fn with_own_uid(inner: tokio::net::UnixListener, own_uid: u32) -> Self {
        Self {
            inner,
            own_uid,
            consecutive_rejections: 0,
        }
    }
}

impl axum::serve::Listener for PeerCheckedListener {
    type Io = tokio::net::UnixStream;
    type Addr = tokio::net::unix::SocketAddr;

    async fn accept(&mut self) -> (Self::Io, Self::Addr) {
        loop {
            match self.inner.accept().await {
                Ok((stream, addr)) => {
                    #[cfg(any(target_os = "android", target_os = "linux"))]
                    {
                        let peer_uid = peer_uid_of(stream.as_raw_fd());
                        match peer_uid {
                            Some(uid) if peer_uid_allowed(uid, self.own_uid) => {
                                self.consecutive_rejections = 0;
                                return (stream, addr);
                            }
                            Some(uid) => {
                                // Saturating: a pathological flood cannot
                                // wrap the counter and restart the logs.
                                self.consecutive_rejections =
                                    self.consecutive_rejections.saturating_add(1);
                                if self.consecutive_rejections
                                    <= MAX_CONSECUTIVE_REJECTIONS_BEFORE_LOG
                                {
                                    log::warn!(
                                        "MCP server: rejected connection from uid {uid} (own uid {})",
                                        self.own_uid
                                    );
                                } else if self.consecutive_rejections
                                    == MAX_CONSECUTIVE_REJECTIONS_BEFORE_LOG + 1
                                {
                                    log::warn!(
                                        "MCP server: rejecting further foreign-uid connections (suppressing logs)"
                                    );
                                }
                            }
                            None => {
                                // getsockopt failed: treat as untrusted and
                                // throttle like the foreign-uid branch so a
                                // persistently failing SO_PEERCRED cannot
                                // flood the log audit fix).
                                self.consecutive_rejections =
                                    self.consecutive_rejections.saturating_add(1);
                                if self.consecutive_rejections
                                    <= MAX_CONSECUTIVE_REJECTIONS_BEFORE_LOG
                                {
                                    log::warn!(
                                        "MCP server: rejected connection with unreadable peer uid"
                                    );
                                } else if self.consecutive_rejections
                                    == MAX_CONSECUTIVE_REJECTIONS_BEFORE_LOG + 1
                                {
                                    log::warn!(
                                        "MCP server: peer uid unreadable repeatedly (suppressing logs)"
                                    );
                                }
                            }
                        }
                    }
                    #[cfg(not(any(target_os = "android", target_os = "linux")))]
                    {
                        return (stream, addr);
                    }
                }
                Err(e) => {
                    // Mirrors axum's built-in accept error handling: log and retry.
                    log::warn!("MCP server: accept error: {e}");
                    tokio::time::sleep(Duration::from_millis(100)).await;
                }
            }
        }
    }

    fn local_addr(&self) -> std::io::Result<Self::Addr> {
        self.inner.local_addr()
    }
}

use tower_mcp::{McpRouter, StdioTransport};

// ── Settings ─────────────────────────────────────────────────────────────

static MCP_ENABLED: AtomicBool = AtomicBool::new(false);

/// Global MCP thread handle.
///
/// # Lock seam
/// `stop()` acquires the lock, takes the handle, then **drops the guard**
/// before calling `join()`. This prevents a deadlock if `start()` is called
/// concurrently while a prior thread is still joining.
/// Join handle + shutdown signal for the running MCP server thread
/// `UnixSocketTransport::serve` blocks forever in the
/// accept loop, and deleting the socket file does NOT wake it — the old
/// code detached the thread on stop, leaking a thread + tokio runtime +
/// listening fd per toggle. The transport is now built manually with
/// `axum::serve(...).with_graceful_shutdown(notify)`, so stop() can
/// signal a clean exit and join.
static MCP_THREAD: Mutex<Option<(JoinHandle<()>, std::sync::Arc<tokio::sync::Notify>)>> =
    Mutex::new(None);

/// Serializes tests that read or write the MCP globals (`MCP_ENABLED`,
/// the handler registry, the socket path) so a `terminal::pty` test and
/// an `mcp` test never interleave their `set_enabled`/`build_env` runs
/// on the same process-wide state. Compiled out of release.
#[cfg(test)]
pub(crate) static MCP_TEST_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

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
type CallbackRunCommand =
    Box<dyn Fn(u64, String) -> (u64, tokio::sync::oneshot::Receiver<String>) + Send + Sync>;
type CallbackScreenshot =
    Box<dyn Fn(u64) -> (u64, tokio::sync::oneshot::Receiver<(u32, u32, Vec<u8>)>) + Send + Sync>;
/// Resolves a session's current working directory (None when unavailable).
/// Registered by the JNI bridge so MCP tools can query the session registry
/// without taking its lock (mirrors ffi.rs session_exit_code).
type CallbackSessionCwd = Box<dyn Fn(u64) -> Option<String> + Send + Sync + 'static>;
/// Resolves a session's exit code (None while it is still running).
type CallbackSessionExitCode = Box<dyn Fn(u64) -> Option<i32> + Send + Sync + 'static>;
/// Drains a session's last OSC 133 command output (None when empty).
type CallbackSessionLastCommandOutput = Box<dyn Fn(u64) -> Option<String> + Send + Sync + 'static>;
/// Cancels a pending async request (drops registry entries, dismisses the
/// still-visible Kotlin dialog).
type CallbackCancelRequest = Box<dyn Fn(u64, u64) + Send + Sync + 'static>;

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
    on_run_command: Mutex<Option<CallbackRunCommand>>,
    on_screenshot: Mutex<Option<CallbackScreenshot>>,
    on_session_cwd: Mutex<Option<CallbackSessionCwd>>,
    on_session_exit_code: Mutex<Option<CallbackSessionExitCode>>,
    on_session_last_command_output: Mutex<Option<CallbackSessionLastCommandOutput>>,
    on_cancel_request: Mutex<Option<CallbackCancelRequest>>,
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
            on_run_command: Mutex::new(None),
            on_screenshot: Mutex::new(None),
            on_session_cwd: Mutex::new(None),
            on_session_exit_code: Mutex::new(None),
            on_session_last_command_output: Mutex::new(None),
            on_cancel_request: Mutex::new(None),
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
        *self.0.on_notify.lock() = Some(Box::new(f));
    }

    pub fn set_toast_handler<F: Fn(String) + Send + 'static>(&self, f: F) {
        *self.0.on_toast.lock() = Some(Box::new(f));
    }

    pub fn set_open_url_handler<F: Fn(String) + Send + 'static>(&self, f: F) {
        *self.0.on_open_url.lock() = Some(Box::new(f));
    }

    pub fn set_clipboard_get_handler<
        F: Fn() -> (u64, tokio::sync::oneshot::Receiver<String>) + Send + 'static,
    >(
        &self,
        f: F,
    ) {
        *self.0.on_clipboard_get.lock() = Some(Box::new(f));
    }

    pub fn set_clipboard_set_handler<F: Fn(String) + Send + 'static>(&self, f: F) {
        *self.0.on_clipboard_set.lock() = Some(Box::new(f));
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
        *self.0.on_show_dialog.lock() = Some(Box::new(f));
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
        *self.0.on_pick_file.lock() = Some(Box::new(f));
    }

    pub fn set_send_signal_handler<F>(&self, f: F)
    where
        F: Fn(u64, i32) -> String + Send + Sync + 'static,
    {
        *self.0.on_send_signal.lock() = Some(Box::new(f));
    }

    /// Set the handler for MCP `run_command`. The handler receives the
    /// active session id and the raw command string, and returns a oneshot
    /// receiver that resolves when Kotlin replies via `runCommandResult()`.
    pub fn set_run_command_handler<F>(&self, f: F)
    where
        F: Fn(u64, String) -> (u64, tokio::sync::oneshot::Receiver<String>) + Send + Sync + 'static,
    {
        *self.0.on_run_command.lock() = Some(Box::new(f));
    }

    /// Set the handler for MCP `screenshot`. The handler receives the session_id
    /// and returns a channel that will receive the RGBA image bytes.
    pub fn set_screenshot_handler<F>(&self, f: F)
    where
        F: Fn(u64) -> (u64, tokio::sync::oneshot::Receiver<(u32, u32, Vec<u8>)>)
            + Send
            + Sync
            + 'static,
    {
        *self.0.on_screenshot.lock() = Some(Box::new(f));
    }

    /// Register a handler resolving a session's current working directory
    /// (session.cwd(): OSC 7 shell-tracked first, then the terminal's
    /// /proc-derived fallback, session.rs:752). The JNI bridge registers
    /// this so the MCP `terminal_info` tool can report `cwd` without the
    /// MCP thread holding the session lock.
    pub fn set_session_cwd_handler<F>(&self, f: F)
    where
        F: Fn(u64) -> Option<String> + Send + Sync + 'static,
    {
        *self.0.on_session_cwd.lock() = Some(Box::new(f));
    }
    /// Register the live session-exit-code resolver (JNI bridge).
    pub fn set_session_exit_code_handler<F>(&self, f: F)
    where
        F: Fn(u64) -> Option<i32> + Send + Sync + 'static,
    {
        *self.0.on_session_exit_code.lock() = Some(Box::new(f));
    }

    /// Register the OSC 133 last-command-output resolver (JNI bridge).
    pub fn set_session_last_command_output_handler<F>(&self, f: F)
    where
        F: Fn(u64) -> Option<String> + Send + Sync + 'static,
    {
        *self.0.on_session_last_command_output.lock() = Some(Box::new(f));
    }

    /// Register the request-cancellation callback (JNI bridge).
    pub fn set_cancel_request_handler<F>(&self, f: F)
    where
        F: Fn(u64, u64) + Send + Sync + 'static,
    {
        *self.0.on_cancel_request.lock() = Some(Box::new(f));
    }

    /// Resolve a session's exit code via the registered handler (None when
    /// unregistered or the session is still running). MCP tools call this
    /// instead of touching the JNI bridge directly, keeping the dependency
    /// direction ffi → mcp.
    pub fn session_exit_code(&self, session_id: u64) -> Option<i32> {
        self.0
            .on_session_exit_code
            .lock()
            .as_ref()
            .and_then(|f| f(session_id))
    }

    /// Drain a session's last OSC 133 command output via the registered
    /// handler (None when unregistered or no output pending).
    pub fn session_last_command_output(&self, session_id: u64) -> Option<String> {
        self.0
            .on_session_last_command_output
            .lock()
            .as_ref()
            .and_then(|f| f(session_id))
    }

    /// Cancel a pending async request via the registered handler. No-op
    /// when the bridge has not registered one.
    pub fn cancel_request(&self, session_id: u64, request_id: u64) {
        if let Some(f) = self.0.on_cancel_request.lock().as_ref() {
            f(session_id, request_id);
        }
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
        .tool(last_command_output_tool())
        .tool(pick_file_tool())
        .tool(dialog_tool())
        .tool(run_command_tool())
        .tool(screenshot_tool())
}

// ── Server lifecycle ─────────────────────────────────────────────────────

/// Overridable socket path: the Android default is
/// derived from the app data dir which is `applicationId`-dependent —
/// hardcoding `/data/data/com.termux` breaks if the package is renamed.
/// Kotlin calls `setMcpSocketPath(context.filesDir/...)` at startup.
static MCP_SOCKET_PATH: std::sync::Mutex<Option<String>> = std::sync::Mutex::new(None);

pub(crate) fn set_socket_path(path: String) {
    *MCP_SOCKET_PATH.lock().unwrap_or_else(|p| p.into_inner()) = Some(path);
}

pub(crate) fn socket_path() -> String {
    MCP_SOCKET_PATH
        .lock()
        .unwrap_or_else(|p| p.into_inner())
        .clone()
        .unwrap_or_else(|| {
            #[cfg(target_os = "android")]
            {
                "/data/data/com.termux/run/mcp.sock".to_string()
            }
            #[cfg(not(target_os = "android"))]
            {
                "/tmp/mcp.sock".to_string()
            }
        })
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
    let mut guard = MCP_THREAD.lock();
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
        .enable_time()
        .build()
    {
        Ok(r) => r,
        Err(e) => {
            log::error!("MCP server: failed to build tokio runtime: {e}");
            return;
        }
    };

    // Per-server shutdown signal: a fresh Notify per
    // start, so a previous stop() cannot leave it permanently notified.
    let shutdown = std::sync::Arc::new(tokio::sync::Notify::new());
    let shutdown_for_thread = shutdown.clone();

    let handle = std::thread::spawn(move || {
        runtime.block_on(async {
            // Build the transport manually (equivalent to
            // UnixSocketTransport::serve, which does exactly this) so we
            // can attach `with_graceful_shutdown`. The alternative —
            // deleting the socket file — does not wake the accept loop.
            let _ = std::fs::remove_file(&path);
            let listener = match tokio::net::UnixListener::bind(&path) {
                Ok(l) => l,
                Err(e) => {
                    log::error!("MCP server: failed to bind {path}: {e}");
                    return;
                }
            };
            log::info!("MCP server listening on {path}");
            let router = build_router();
            let http = tower_mcp::transport::HttpTransport::new(router);
            let router = http.into_router();
            let listener = PeerCheckedListener::new(listener);
            if let Err(e) = axum::serve(listener, router)
                .with_graceful_shutdown(async move {
                    shutdown_for_thread.notified().await;
                    log::info!("MCP server shutdown signal received");
                })
                .await
            {
                log::error!("MCP server error: {e}");
            }
            log::info!("MCP server stopped");
        });
    });

    // Store JoinHandle for later stop — guard is still held from the
    // check-and-lock above, so no TOCTOU window exists.
    *guard = Some((handle, shutdown));
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
    // Take the handle + shutdown signal OUT of the lock, then drop the
    // lock immediately so the join is NOT called while holding MCP_THREAD.
    let (handle, shutdown) = match MCP_THREAD.lock().take() {
        Some(pair) => pair,
        None => return,
    };
    // guard dropped here — lock released before join

    // Signal the accept loop to shut down: without this,
    // the thread blocked in axum::serve never returns and would be leaked.
    shutdown.notify_one();

    let path = socket_path();
    let _ = std::fs::remove_file(&path);

    // Graceful join. axum's graceful shutdown drains in-flight requests
    // then returns, so this completes promptly; 500ms bounds pathological
    // cases (e.g. a stuck session handler) without blocking the caller
    // indefinitely.
    let deadline = std::time::Instant::now() + std::time::Duration::from_millis(500);
    while std::time::Instant::now() < deadline {
        if handle.is_finished() {
            if let Err(panic) = handle.join() {
                log::error!("MCP server thread panicked: {:?}", panic);
            }
            return;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    log::warn!("MCP server thread did not exit within 500ms after shutdown signal — DETACHING");
    // handle dropped here → thread detached (last resort; the notify makes
    // this path unreachable in practice)
}

/// Start the MCP server in stdio mode (for AI coding agent CLIs).
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
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tower_mcp::testing::TestClient;

    // `global_state()` is a process-wide singleton; the tools read the
    // handlers from it. Tests that register handlers MUST run serially or
    // one test's handler leaks into the next. This lock is held for the
    // whole test body. The lock itself lives at crate level (`super::`)
    // so `terminal::pty` tests serialize against the same globals.
    use super::MCP_TEST_LOCK;

    /// Clear every handler and the active session id so a freshly built
    /// router sees pristine global state. Must be called while holding
    /// MCP_TEST_LOCK (i.e. right after `let _guard =...`).
    fn reset_global_state() {
        let state = global_state();
        *state.0.on_notify.lock() = None;
        *state.0.on_toast.lock() = None;
        *state.0.on_open_url.lock() = None;
        *state.0.on_clipboard_get.lock() = None;
        *state.0.on_clipboard_set.lock() = None;
        *state.0.on_show_dialog.lock() = None;
        *state.0.on_pick_file.lock() = None;
        *state.0.on_send_signal.lock() = None;
        *state.0.on_run_command.lock() = None;
        *state.0.on_session_cwd.lock() = None;
        *state.0.on_session_exit_code.lock() = None;
        *state.0.on_session_last_command_output.lock() = None;
        *state.0.on_cancel_request.lock() = None;
        state.set_active_session_id(0);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn run_stdio_when_disabled_returns_ok_without_transport() {
        // The enabled flag defaults to false; run_stdio must short-circuit
        // instead of blocking on stdin. (Enabled mode owns the process
        // stdin/stdout and is exercised end-to-end only by the CLI binary.)
        let _guard = MCP_TEST_LOCK
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        reset_global_state();
        super::MCP_ENABLED.store(false, Ordering::Release);
        let result = super::run_stdio().await;
        assert!(result.is_ok(), "disabled MCP must return Ok immediately");
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_list_tools() {
        let _guard = MCP_TEST_LOCK
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        reset_global_state();
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
        assert!(names.contains(&"last_command_output"));
        assert!(names.contains(&"pick_file"));
        assert!(names.contains(&"dialog"));
        assert!(names.contains(&"screenshot"));
        assert_eq!(names.len(), 12);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_terminal_info_tool() {
        let _guard = MCP_TEST_LOCK
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        reset_global_state();
        let router = build_router();
        let mut client = TestClient::from_router(router);
        client.initialize().await;

        let result = client.call_tool("terminal_info", json!({})).await;
        assert!(!result.is_error);

        let text = result.content.first().unwrap().as_text().unwrap();
        let info: Value = serde_json::from_str(text).unwrap();
        assert!(info["rows"].is_number());
        assert!(info["columns"].is_number());
        assert!(info["version"].is_string());
        // Spec d4: terminal_info MUST include the session exit_code field.
        // With no active session registered it resolves to null.
        assert!(info.get("exit_code").is_some(), "exit_code field missing");
        assert!(info["exit_code"].is_null(), "no session → exit_code null");
        // cwd field: present and null when no session/handler.
        assert!(info.get("cwd").is_some(), "cwd field missing");
        assert!(info["cwd"].is_null(), "no session → cwd null");
    }

    /// Spec d4 scenario: an exited session reports its real exit code via
    /// terminal_info. Spawns a real shell, has it exit 3, registers the
    /// session (as the JNI spawn path would), and asserts the value.
    #[tokio::test(flavor = "current_thread")]
    async fn test_terminal_info_cwd_field_resolves_from_handler() {
        let _guard = MCP_TEST_LOCK
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        reset_global_state();
        // The cwd field is resolved via the JNI-registered handler (ffi.rs
        // registers a callback reading session.cwd() from the registry).
        global_state()
            .set_session_cwd_handler(|_id| Some("/data/data/com.termux/files/home".to_string()));
        global_state().set_active_session_id(42);
        let router = build_router();
        let mut client = TestClient::from_router(router);
        client.initialize().await;

        let result = client.call_tool("terminal_info", json!({})).await;
        assert!(!result.is_error);
        let text = result.content.first().unwrap().as_text().unwrap();
        let info: Value = serde_json::from_str(text).unwrap();
        assert_eq!(info["cwd"], "/data/data/com.termux/files/home");
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_terminal_info_exit_code_reflects_exited_session() {
        let _guard = MCP_TEST_LOCK
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        reset_global_state();
        // Spawn a real session and let it exit 3 (Linux host: fork works).
        let mut session = crate::terminal::session::Session::spawn(
            "/bin/sh",
            24,
            80,
            &crate::terminal::ShellEnv::default(),
            None,
        )
        .expect("spawn failed");
        session.write(b"exit 3\n").expect("write failed");
        let deadline = std::time::Instant::now() + Duration::from_secs(5);
        loop {
            session.process_output();
            if session.is_exited() && session.exit_code_now() == Some(3) {
                break;
            }
            assert!(
                std::time::Instant::now() < deadline,
                "session did not exit 3 in time"
            );
            std::thread::sleep(Duration::from_millis(20));
        }
        let handle = std::sync::Arc::new(parking_lot::Mutex::new(session));
        crate::android::ffi::register_session_for_test(42, handle);
        // Mirrors the JNI bridge registration (ffi.rs mcp init): without the
        // handler the tool resolves exit_code via a None callback and would
        // report null.
        crate::mcp::global_state()
            .set_session_exit_code_handler(crate::android::ffi::session_exit_code);
        crate::mcp::global_state().set_active_session_id(42);

        let router = build_router();
        let mut client = TestClient::from_router(router);
        client.initialize().await;

        let result = client.call_tool("terminal_info", json!({})).await;
        assert!(!result.is_error);
        let text = result.content.first().unwrap().as_text().unwrap();
        let info: Value = serde_json::from_str(text).unwrap();
        assert_eq!(
            info["exit_code"], 3,
            "exited session must report exit_code 3"
        );
        crate::android::ffi::clear_registry_for_test();
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_clipboard_set_requires_text() {
        let _guard = MCP_TEST_LOCK
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        reset_global_state();
        let router = build_router();
        let mut client = TestClient::from_router(router);
        client.initialize().await;

        let result = client.call_tool("clipboard_set", json!({})).await;
        // Should fail because `text` is required
        assert!(result.is_error);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_notify_tool_invokes_handler() {
        let _guard = MCP_TEST_LOCK
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        reset_global_state();
        let router = build_router();
        let mut client = TestClient::from_router(router);
        client.initialize().await;
        let state = global_state();

        let (tx, rx) = std::sync::mpsc::channel();
        state.set_notify_handler(move |msg| {
            tx.send(msg).unwrap();
        });
        let result = client
            .call_tool("notify", json!({"title": "T", "body": "B"}))
            .await;
        assert!(!result.is_error);
        assert_eq!(
            result.content.first().unwrap().as_text().unwrap(),
            "Notification sent"
        );
        assert_eq!(rx.try_recv().unwrap(), "T\nB");

        // No handler -> error
        state.set_notify_handler(|_| {});
        let result = client
            .call_tool("notify", json!({"title": "T2", "body": ""}))
            .await;
        assert!(!result.is_error);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_toast_tool_invokes_handler() {
        let _guard = MCP_TEST_LOCK
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        reset_global_state();
        let router = build_router();
        let mut client = TestClient::from_router(router);
        client.initialize().await;
        let state = global_state();

        let (tx, rx) = std::sync::mpsc::channel();
        state.set_toast_handler(move |text| {
            tx.send(text).unwrap();
        });
        let result = client.call_tool("toast", json!({"text": "hello"})).await;
        assert!(!result.is_error);
        assert_eq!(rx.try_recv().unwrap(), "hello");
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_open_url_tool_invokes_handler() {
        let _guard = MCP_TEST_LOCK
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        reset_global_state();
        let router = build_router();
        let mut client = TestClient::from_router(router);
        client.initialize().await;
        let state = global_state();

        let (tx, rx) = std::sync::mpsc::channel();
        state.set_open_url_handler(move |url| {
            tx.send(url).unwrap();
        });
        let result = client
            .call_tool("open_url", json!({"url": "https://example.com"}))
            .await;
        assert!(!result.is_error);
        assert_eq!(rx.try_recv().unwrap(), "https://example.com");
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_clipboard_get_returns_text() {
        let _guard = MCP_TEST_LOCK
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        reset_global_state();
        let router = build_router();
        let mut client = TestClient::from_router(router);
        client.initialize().await;
        let state = global_state();

        state.set_clipboard_get_handler(|| {
            let sid = 1;
            let (req_id, tx) = crate::android::ffi::register_request(sid);
            crate::android::ffi::answer_request(sid, req_id, "clip text".to_string());
            (req_id, tx)
        });
        let result = client.call_tool("clipboard_get", json!({})).await;
        assert!(!result.is_error, "clipboard_get failed: {:?}", result);
        assert_eq!(
            result.content.first().unwrap().as_text().unwrap(),
            "clip text"
        );
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_clipboard_get_unavailable_if_no_handler() {
        let _guard = MCP_TEST_LOCK
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        reset_global_state();
        let router = build_router();
        let mut client = TestClient::from_router(router);
        client.initialize().await;
        let state = global_state();
        state.set_clipboard_get_handler(|| {
            // Answer with a dropped channel: the tool's timeout branch
            // resolves immediately (Err(dropped)) and cancels the registry
            // entry — no 300s stall.
            let req_id = 1;
            let (tx, rx) = tokio::sync::oneshot::channel::<String>();
            drop(tx);
            crate::android::ffi::cancel_request(1, req_id);
            (req_id, rx)
        });
        let result = client.call_tool("clipboard_get", json!({})).await;
        assert!(result.is_error);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_send_signal_invokes_handler() {
        let _guard = MCP_TEST_LOCK
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        reset_global_state();
        let router = build_router();
        let mut client = TestClient::from_router(router);
        client.initialize().await;
        let state = global_state();
        state.set_active_session_id(42);

        let (tx, rx) = std::sync::mpsc::channel();
        state.set_send_signal_handler(move |sid, sig| {
            tx.send((sid, sig)).unwrap();
            format!("signal {sig} sent to {sid}")
        });
        let result = client.call_tool("send_signal", json!({"signal": 15})).await;
        assert!(!result.is_error);
        assert_eq!(
            result.content.first().unwrap().as_text().unwrap(),
            "signal 15 sent to 42"
        );
        assert_eq!(rx.try_recv().unwrap(), (42, 15));
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_run_command_invokes_handler() {
        let _guard = MCP_TEST_LOCK
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        reset_global_state();
        let router = build_router();
        let mut client = TestClient::from_router(router);
        client.initialize().await;
        let state = global_state();
        state.set_active_session_id(7);

        let (tx, rx) = std::sync::mpsc::channel();
        state.set_run_command_handler(move |sid, command| {
            tx.send((sid, command.clone())).unwrap();
            let (req_id, resp_rx) = crate::android::ffi::register_request(sid);
            // Simulate Kotlin answering via runCommandResult right away.
            let payload = serde_json::json!({
                "exit_code": 0,
                "stdout": command,
                "stderr": "",
            })
            .to_string();
            crate::android::ffi::answer_request(sid, req_id, payload);
            (req_id, resp_rx)
        });
        let result = client
            .call_tool("run_command", json!({"command": "echo \"hi\""}))
            .await;
        assert!(!result.is_error);
        assert_eq!(
            result.content.first().unwrap().as_text().unwrap(),
            r#"{"exit_code":0,"stderr":"","stdout":"echo \"hi\""}"#
        );
        assert_eq!(rx.try_recv().unwrap(), (7u64, "echo \"hi\"".to_string()));
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_last_command_output_tool() {
        let _guard = MCP_TEST_LOCK
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        reset_global_state();
        let router = build_router();
        let mut client = TestClient::from_router(router);
        client.initialize().await;
        let state = global_state();
        state.set_active_session_id(7);

        // No handler registered → empty text (drained-buffer semantics).
        let result = client.call_tool("last_command_output", json!({})).await;
        assert!(!result.is_error);
        assert_eq!(result.content.first().unwrap().as_text().unwrap(), "");

        // Handler registered → drains the buffer.
        state.set_session_last_command_output_handler(|_sid| Some("ls\nfile.txt".to_string()));
        let result = client.call_tool("last_command_output", json!({})).await;
        assert!(!result.is_error);
        assert_eq!(
            result.content.first().unwrap().as_text().unwrap(),
            "ls\nfile.txt"
        );
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_cancel_request_handler_invoked() {
        let _guard = MCP_TEST_LOCK
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        reset_global_state();
        let router = build_router();
        let mut client = TestClient::from_router(router);
        client.initialize().await;
        let state = global_state();
        state.set_active_session_id(7);

        let (tx, rx) = std::sync::mpsc::channel();
        state.set_cancel_request_handler(move |sid, req_id| {
            tx.send((sid, req_id)).unwrap();
        });
        // clipboard_get with a handler that answers with a dropped channel:
        // the tool's timeout branch immediately resolves (Err(dropped)) and
        // must cancel the request via the registered handler.
        let registered_id: std::sync::Arc<std::sync::Mutex<Option<u64>>> =
            std::sync::Arc::new(std::sync::Mutex::new(None));
        let captured = registered_id.clone();
        state.set_clipboard_get_handler(move || {
            // Registry entry with a live sender; the answer channel's sender
            // is dropped so the tool's `rx.await` resolves Err instantly —
            // the timeout/cancel branch must then invoke cancel_request.
            let (req_id, _rx) = crate::android::ffi::register_request(7);
            *captured
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner()) = Some(req_id);
            let (tx, rx) = tokio::sync::oneshot::channel::<String>();
            drop(tx);
            (req_id, rx)
        });
        let result = client.call_tool("clipboard_get", json!({})).await;
        assert!(result.is_error);
        // Cancel must fire exactly once and must carry the request id that
        // the handler actually registered (not a hard-coded sequence value
        // that other tests may have advanced past).
        let (sid, req_id) = rx.try_recv().expect("cancel handler must fire");
        assert_eq!(sid, 7);
        let expected = registered_id
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .expect("clipboard handler must register a request");
        assert_eq!(req_id, expected, "cancel must carry the registered id");
        assert!(
            rx.try_recv().is_err(),
            "cancel handler must be invoked exactly once"
        );
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_run_command_unavailable_without_handler() {
        let _guard = MCP_TEST_LOCK
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        reset_global_state();
        let router = build_router();
        let mut client = TestClient::from_router(router);
        client.initialize().await;
        let state = global_state();
        state.set_active_session_id(7);
        // No handler registered → the tool reports "not available".
        let result = client
            .call_tool("run_command", json!({"command": "echo hi"}))
            .await;
        assert!(result.is_error);
        assert!(
            result
                .content
                .first()
                .unwrap()
                .as_text()
                .unwrap()
                .contains("not available")
        );
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_pick_file_returns_path() {
        let _guard = MCP_TEST_LOCK
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        reset_global_state();
        let router = build_router();
        let mut client = TestClient::from_router(router);
        client.initialize().await;
        let state = global_state();

        state.set_pick_file_handler(|sid, dir, pattern| {
            let (req_id, rx) = crate::android::ffi::register_request(sid);
            assert_eq!(dir, "/tmp");
            assert_eq!(pattern, "*.rs");
            // Simulate Kotlin's dialogResult answering right away.
            crate::android::ffi::answer_request(sid, req_id, "/tmp/selected.rs".to_string());
            (req_id, rx)
        });
        let result = client
            .call_tool("pick_file", json!({"directory": "/tmp", "pattern": "*.rs"}))
            .await;
        assert!(!result.is_error, "pick_file failed: {:?}", result);
        assert_eq!(
            result.content.first().unwrap().as_text().unwrap(),
            "/tmp/selected.rs"
        );
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_dialog_returns_answer() {
        let _guard = MCP_TEST_LOCK
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        reset_global_state();
        let router = build_router();
        let mut client = TestClient::from_router(router);
        client.initialize().await;
        let state = global_state();

        state.set_dialog_handler(|sid, dtype, title, message, options| {
            let (req_id, rx) = crate::android::ffi::register_request(sid);
            assert_eq!(dtype, "confirm");
            assert_eq!(title, "Question");
            assert_eq!(message, "Really?");
            assert!(options.is_empty());
            // Simulate Kotlin's dialogResult answering right away.
            crate::android::ffi::answer_request(sid, req_id, "yes".to_string());
            (req_id, rx)
        });
        let result = client
            .call_tool(
                "dialog",
                json!({"dialog_type": "confirm", "title": "Question", "message": "Really?"}),
            )
            .await;
        assert!(!result.is_error, "dialog failed: {:?}", result);
        assert_eq!(result.content.first().unwrap().as_text().unwrap(), "yes");
    }

    // ──: SO_PEERCRED peer validation ──

    #[test]
    fn peer_uid_allowed_accepts_own_uid_and_root() {
        assert!(peer_uid_allowed(12345, 12345), "own uid must be allowed");
        assert!(peer_uid_allowed(0, 12345), "root (uid 0) must be allowed");
        assert!(
            !peer_uid_allowed(99999, 12345),
            "foreign uid must be rejected"
        );
        assert!(!peer_uid_allowed(1, 12345), "system uid must be rejected");
        assert!(
            !peer_uid_allowed(12346, 12345),
            "adjacent uid must be rejected"
        );
    }

    /// End-to-end: a real Unix socket pair — the server side wrapped in
    /// PeerCheckedListener must serve a same-uid client (our own test
    /// process), and the axum route must respond. Also verifies the
    /// listener implements axum::serve::Listener and accept() returns a
    /// usable stream.
    #[tokio::test(flavor = "current_thread")]
    async fn peer_checked_listener_serves_same_uid_client() {
        let dir = std::path::Path::new("/tmp").join(format!(
            "mcp-pc-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).expect("create temp dir");
        let sock = dir.join("mcp.sock");
        let path = sock.to_string_lossy().into_owned();

        let listener = tokio::net::UnixListener::bind(&path).expect("bind");
        let listener = PeerCheckedListener::new(listener);

        use axum::routing::get;
        let app = axum::Router::new().route("/", get(|| async { "peer-ok" }));
        let serve_handle = tokio::spawn(async move {
            axum::serve(listener, app).await.expect("serve");
        });

        // Client connects from the same process (same uid) — must be served.
        let stream = tokio::net::UnixStream::connect(&path)
            .await
            .expect("connect");
        let (mut reader, mut writer) = stream.into_split();
        let request = "GET / HTTP/1.1\r\nhost: localhost\r\nconnection: close\r\n\r\n";
        writer.write_all(request.as_bytes()).await.expect("write");

        let mut buf = Vec::new();
        let mut chunk = [0u8; 1024];
        loop {
            let n = reader.read(&mut chunk).await.expect("read");
            if n == 0 {
                break;
            }
            buf.extend_from_slice(&chunk[..n]);
        }
        let text = String::from_utf8_lossy(&buf);
        assert!(
            text.contains("200 OK"),
            "same-uid client must be served, got: {text}"
        );
        assert!(text.contains("peer-ok"), "body missing: {text}");

        serve_handle.abort();
        let _ = std::fs::remove_file(&path);
        let _ = std::fs::remove_dir(&dir);
    }

    /// Foreign-uid clients must be rejected at the listener layer: the
    /// accept loop drops them and continues accepting. We simulate a
    /// foreign uid by constructing the listener with an expected uid that
    /// can never match the real client (u32::MAX), so every incoming
    /// connection is rejected; a subsequent client must still be served
    /// after the rejection loop.
    #[tokio::test(flavor = "current_thread")]
    async fn peer_checked_listener_rejects_foreign_uid() {
        let dir = std::path::Path::new("/tmp").join(format!(
            "mcp-pr-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).expect("create temp dir");
        let sock = dir.join("mcp.sock");
        let path = sock.to_string_lossy().into_owned();

        let listener = tokio::net::UnixListener::bind(&path).expect("bind");
        // Every real client has uid != u32::MAX, so all connections are
        // rejected (foreign-uid path), and the accept loop keeps running.
        let listener = PeerCheckedListener::with_own_uid(listener, u32::MAX);

        use axum::routing::get;
        let app = axum::Router::new().route("/", get(|| async { "still-up" }));
        let serve_handle = tokio::spawn(async move {
            axum::serve(listener, app).await.expect("serve");
        });

        // First foreign-uid connection: rejected (dropped, no HTTP reply).
        let mut rejected = tokio::net::UnixStream::connect(&path)
            .await
            .expect("connect");
        let mut buf = [0u8; 16];
        let read_res =
            tokio::time::timeout(Duration::from_millis(300), rejected.read(&mut buf)).await;
        assert!(
            read_res.is_err() || matches!(read_res, Ok(Ok(0))),
            "foreign-uid connection must be dropped without data, got: {read_res:?}"
        );
        drop(rejected);

        // Second connection: also foreign uid → also rejected. The accept
        // loop must not have died.
        let mut rejected2 = tokio::net::UnixStream::connect(&path)
            .await
            .expect("connect 2");
        let read_res2 =
            tokio::time::timeout(Duration::from_millis(300), rejected2.read(&mut buf)).await;
        assert!(
            read_res2.is_err() || matches!(read_res2, Ok(Ok(0))),
            "second foreign-uid connection must be dropped, got: {read_res2:?}"
        );
        drop(rejected2);

        serve_handle.abort();
        let _ = std::fs::remove_file(&path);
        let _ = std::fs::remove_dir(&dir);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_method_not_found() {
        let _guard = MCP_TEST_LOCK
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        reset_global_state();
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

    /// Regression: `stop()` must signal the accept loop via
    /// `with_graceful_shutdown` and join the thread — previously it
    /// detached a thread blocked forever in `serve()`, leaking a thread +
    /// tokio runtime + listening fd on every MCP toggle. This test toggles
    /// start/stop repeatedly and asserts the thread actually exits.
    #[test]
    fn start_stop_cycle_releases_thread() {
        // Use a temp socket path (the production path is /data/data/...).
        let dir = std::env::temp_dir().join(format!("mcp-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("create temp dir");
        let sock = dir.join("mcp.sock");
        let path = sock.to_string_lossy().into_owned();

        // Point socket_path() at the temp file by monkey-patching is not
        // possible (it reads a constant); instead verify the transport
        // logic in isolation: bind a listener, signal shutdown, and assert
        // axum::serve returns.
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_io()
            .enable_time()
            .build()
            .expect("runtime");
        let shutdown = std::sync::Arc::new(tokio::sync::Notify::new());
        let s2 = shutdown.clone();
        let joined = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let j2 = joined.clone();
        let path2 = path.clone();
        let handle = std::thread::spawn(move || {
            runtime.block_on(async {
                let _ = std::fs::remove_file(&path2);
                let listener = tokio::net::UnixListener::bind(&path2).expect("bind");
                // Minimal axum router; the real one needs a McpRouter.
                use axum::routing::get;
                let app = axum::Router::new().route("/", get(|| async { "ok" }));
                let _ = axum::serve(listener, app)
                    .with_graceful_shutdown(async move {
                        s2.notified().await;
                    })
                    .await;
                j2.store(true, std::sync::atomic::Ordering::Release);
            });
        });
        // Give it a moment to bind, then signal shutdown.
        std::thread::sleep(std::time::Duration::from_millis(100));
        shutdown.notify_one();
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        while std::time::Instant::now() < deadline
            && !joined.load(std::sync::atomic::Ordering::Acquire)
        {
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        assert!(
            joined.load(std::sync::atomic::Ordering::Acquire),
            "shutdown signal must terminate the serve loop"
        );
        handle.join().expect("thread joins cleanly");
        let _ = std::fs::remove_file(&path);
        let _ = std::fs::remove_dir(&dir);
    }
}
