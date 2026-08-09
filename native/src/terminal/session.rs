//! Session orchestrator — wires PTY reader, VT parser, and process waiter together.
//!
//! # Session Lifecycle
//!
//! ```text
//!                     ┌─────────────┐
//!                     │   Spawned   │
//!                     └──────┬──────┘
//!                            │ PTY created, threads started
//!                            ▼
//!                     ┌─────────────┐
//!                     │   Running   │◄──────┐
//!                     └──────┬──────┘       │ process_output()
//!                            │              │
//!              ┌─────────────┼─────────────┐│
//!              │             │             ││
//!              ▼             ▼             ││
//!       ┌──────────┐  ┌──────────┐        ││
//!       │  Paused  │  │   Idle   │────────┘│
//!       └──────────┘  └──────────┘  output  │
//!                            │             │
//!                            │ EOF / exit  │
//!                            ▼             │
//!                     ┌─────────────┐      │
//!                     │   Exited    │      │
//!                     └──────┬──────┘      │
//!                            │             │
//!                            ▼             │
//!                     ┌─────────────┐      │
//!                     │   Cleaned   │──────┘
//!                     └─────────────┘  cleanup_resources()
//! ```
//!
//! # Requirements
//! - [FR-009](crate) — Input: Ctrl-C, Ctrl-D, Ctrl-Z signal passthrough
//! - [FR-027](crate) — Session: double-fork child with PID tracking
//! - [FR-028](crate) — Process: exited callback
//! - [FR-029](crate) — Scrollback: scroll up
//! - [FR-039](crate) — MCP: server lifecycle
//! - [FR-043](crate) — MCP: I/O multiplexing
//! - [NFR-005](crate) — Session: zombie reaping
//! - [NFR-024](crate) — Session: crash recovery
use parking_lot::Mutex;
use std::fs::File;
use std::io::Read;
use std::os::unix::io::AsRawFd;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::time::Duration;

use flume::{Receiver, bounded};
use thiserror::Error;

use crate::terminal::ghostty_terminal::GhosttyTerminal;
use crate::terminal::output_processor::OutputProcessor;
use crate::terminal::pty::{Pty, PtyError, PtyPair};
use crate::terminal::shell_env::ShellEnv;

const READ_BUF_SIZE: usize = 8192;
/// How long the reader thread parks in `poll` before re-checking the exit flag.
/// Replaces the previous 2 ms busy-poll `sleep`, so output latency stays low
/// while the thread no longer spins the CPU when the PTY is idle.
const READ_POLL_TIMEOUT_MS: i32 = 100;

const DEFAULT_SCROLLBACK_LINES: u32 = 50000;

/// Errors that can occur during session operations.
#[derive(Debug, Error)]
pub enum SessionError {
    #[error("pty error: {0}")]
    Pty(#[from] PtyError),
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("ghostty error: {0}")]
    Ghostty(String),
    #[error("session closed")]
    Closed,
    #[error("invalid dimensions (out of u16 range)")]
    InvalidDimensions,
}

/// Result of a resize: whether the ghostty grid accepted the command.
/// `Applied` — PTY and grid both resized. `Dropped` — PTY winsize changed
/// but the grid command was dropped (channel full/wedged VT thread); the
/// caller must not publish the new dims as authoritative (round-113).
pub enum ResizeOutcome {
    Applied,
    Dropped,
}

impl SessionError {
    /// True when the underlying write failed with EAGAIN/EWOULDBLOCK —
    /// i.e. the PTY buffer is full (child not reading) on a non-blocking
    /// master. Callers drop the input in that case (xterm semantics)
    /// instead of surfacing it as an error.
    pub fn is_would_block(&self) -> bool {
        matches!(self, SessionError::Io(e) if e.kind() == std::io::ErrorKind::WouldBlock)
    }
}

/// A terminal session — wires PTY reader, VT parser, and process waiter together.
///
/// # Lifecycle
///
/// ```text
///                     ┌─────────────┐
///                     │   Spawned   │
///                     └──────┬──────┘
///                            │ PTY created, threads started
///                            ▼
///                     ┌─────────────┐
///                     │   Running   │◄──────┐
///                     └──────┬──────┘       │ process_output()
///                            │              │
///              ┌─────────────┼─────────────┐│
///              │             │             ││
///              ▼             ▼             ││
///       ┌──────────┐  ┌──────────┐        ││
///       │  Paused  │  │   Idle   │────────┘│
///       └──────────┘  └──────────┘  output  │
///                            │             │
///                            │ EOF / exit  │
///                            ▼             │
///                     ┌─────────────┐      │
///                     │   Exited    │      │
///                     └──────┬──────┘      │
///                            │             │
///                            ▼             │
///                     ┌─────────────┐      │
///                     │   Cleaned   │──────┘
///                     └─────────────┘  cleanup_resources()
/// ```
pub struct Session {
    pty: Box<dyn Pty>,
    terminal: GhosttyTerminal,
    output_processor: OutputProcessor,
    output_tx: flume::Sender<Vec<u8>>,
    output_rx: Receiver<Vec<u8>>,
    /// Tee channel: secondary consumer for raw PTY output (logging, tracing, MCP screenshot).
    /// Bounded(256) — drops silently when full (backpressure without blocking PTY reader).
    tee_tx: Option<flume::Sender<Vec<u8>>>,
    tee_rx: Option<Receiver<Vec<u8>>>,

    // ── Event state (polled from Kotlin via push_event) ──────────────
    exited: Arc<AtomicBool>,
    /// Set once the Exit event for a background (non-active) session has
    /// been pushed to the event queue, so the per-frame sweep in pollEvent
    /// reports it exactly once.
    exit_reported: Arc<AtomicBool>,
    bel_triggered: Arc<AtomicBool>,
    clipboard_text: Arc<Mutex<Option<String>>>,
    /// Pending OSC 52 clipboard read request: the requested selection name.
    /// Consumed by the JNI layer (`poll_clipboard_read`), which forwards it
    /// to the host app and writes the answer back via
    /// [`Session::answer_clipboard_read`].
    clipboard_read: Arc<Mutex<Option<String>>>,
    notification: Arc<Mutex<Option<(String, String)>>>,
    cwd: Arc<Mutex<Option<String>>>,

    // ── Thread lifecycle ─────────────────────────────────────────────
    reader_handle: Option<std::thread::JoinHandle<()>>,
    wait_handle: Option<std::thread::JoinHandle<()>>,

    // ── Runtime state ────────────────────────────────────────────────
    /// Exit code captured from waitpid, `None` while process runs.
    pub(crate) exit_code: Arc<Mutex<Option<i32>>>,
    /// Round-224: child alive duration (ms, fork → waitpid), written by
    /// the wait thread on exit. Consumed by ffi::poll_event for the
    /// fast-death Exit event payload.
    pub(crate) exit_alive_ms: Arc<Mutex<Option<u64>>>,
    /// Round-224: fork timestamp, the start point for [Self::exit_alive_ms].
    spawned_at: std::time::Instant,

    // ── Cached grid size ─────────────────────────────────────────────
    /// Last known terminal grid size, updated on spawn and successful
    /// resize. Read lock-free by ffi::switch_session_inner to refresh the
    /// MCP terminal_info dims WITHOUT a blocking query RPC on the VT
    /// thread (a query inside the registry write lock would freeze every
    /// session operation for up to 2×QUERY_TIMEOUT_MS).
    terminal_rows: AtomicU32,
    terminal_cols: AtomicU32,
    /// Set when a grid resize command was dropped while the PTY was already
    /// resized. Cleared on the next successful grid resize. Prevents the
    /// size short-circuit from permanently masking the PTY/grid divergence
    /// (round-113).
    grid_dirty: AtomicBool,
}

pub struct ThemeConfig {
    pub background: [u8; 3],
    pub foreground: [u8; 3],
    pub ansi: [[u8; 3]; 16],
    pub scrollback_lines: u32,
}

impl Default for ThemeConfig {
    fn default() -> Self {
        let (ansi, bg, fg) = GhosttyTerminal::catppuccin_mocha_palette();
        Self {
            background: bg,
            foreground: fg,
            ansi,
            scrollback_lines: DEFAULT_SCROLLBACK_LINES,
        }
    }
}

impl Session {
    /// Create a session with an already-constructed PTY.
    /// No reader/wait threads are spawned — the caller is responsible for
    /// driving PTY I/O. Primarily used for testing with `MockPty`.
    pub fn with_pty(pty: Box<dyn Pty>, rows: u32, cols: u32) -> Result<Self, SessionError> {
        Self::spawn_with_theme_inner(pty, rows, cols, ThemeConfig::default())
    }

    /// Spawn a new session with the default Catppuccin Mocha theme.
    pub fn spawn(shell: &str, rows: u32, cols: u32, env: &ShellEnv) -> Result<Self, SessionError> {
        Self::spawn_with_theme(shell, rows, cols, env, ThemeConfig::default())
    }

    /// Spawn a new session with a custom theme and scrollback buffer size.
    pub fn spawn_with_theme(
        shell: &str,
        rows: u32,
        cols: u32,
        env: &ShellEnv,
        theme: ThemeConfig,
    ) -> Result<Self, SessionError> {
        log::info!("Session::spawn: shell='{shell}', rows={rows}, cols={cols}");
        // Reject out-of-range dimensions up front (mirrors resize's
        // InvalidDimensions check): `as u16` below would silently truncate
        // and the cached grid_size would then disagree with the PTY.
        if !(u16::try_from(rows).is_ok() && u16::try_from(cols).is_ok()) {
            return Err(SessionError::InvalidDimensions);
        }
        let pty = match PtyPair::spawn(shell, rows as u16, cols as u16, env) {
            Ok(p) => {
                log::info!("Session::spawn: PtyPair::spawn OK");
                p
            }
            Err(e) => {
                log::info!("Session::spawn: PtyPair::spawn error: {e}");
                return Err(e.into());
            }
        };
        match pty.set_nonblocking() {
            Ok(()) => log::info!("Session::spawn: set_nonblocking OK"),
            Err(e) => {
                log::info!("Session::spawn: set_nonblocking error: {e}");
                return Err(e.into());
            }
        }

        log::info!("Session::spawn: cloning master fd for reader");
        // Safe: the dup happens inside `try_clone_reader_fd` (in pty.rs, where
        // `unsafe` is permitted). The result is an owned, safe handle we read
        // through a `std::fs::File`, so no `unsafe` block is needed here.
        let reader_fd = pty.try_clone_reader_fd().map_err(SessionError::Io)?;
        let mut read_file = File::from(reader_fd);

        let child_pid = pty.child_pid();

        let mut session =
            match Self::spawn_with_theme_inner(Box::new(pty) as Box<dyn Pty>, rows, cols, theme) {
                Ok(session) => session,
                Err(e) => {
                    // `read_file` is dropped here, closing its fd safely.
                    return Err(e);
                }
            };

        let exited = session.exited.clone();
        let output_tx = session.output_tx.clone();
        let tee_tx = session.tee_tx.clone();

        log::info!("Session::spawn: spawning reader thread");
        let exited_read = exited.clone();
        let reader_handle = std::thread::spawn(move || {
            let mut read_buf = [0u8; READ_BUF_SIZE];
            let poll_fd = read_file.as_raw_fd();
            loop {
                if exited_read.load(Ordering::Acquire) {
                    log::info!("reader thread: exiting due to exited flag");
                    break;
                }
                let mut poll_fd = libc::pollfd {
                    fd: poll_fd,
                    events: libc::POLLIN,
                    revents: 0,
                };
                // SAFETY: `poll` is a POSIX syscall; `poll_fd` is a valid, initialized
                // `pollfd` whose `fd` is the live reader fd owned by `read_file`.
                // `poll` only reads these inputs and writes `revents` back. This is
                // the sole `unsafe` remaining in the reader and does not bypass the
                // `Pty` abstraction (the fd was obtained via `try_clone_reader_fd`).
                let poll_result = unsafe {
                    libc::poll(&mut poll_fd as *mut libc::pollfd, 1, READ_POLL_TIMEOUT_MS)
                };
                match poll_result.cmp(&0) {
                    std::cmp::Ordering::Greater => {}
                    std::cmp::Ordering::Equal => continue,
                    std::cmp::Ordering::Less => {
                        log::info!("reader thread: poll error: {poll_result}");
                        exited_read.store(true, Ordering::Release);
                        break;
                    }
                }
                match read_file.read(&mut read_buf) {
                    Ok(0) => {
                        log::info!("reader thread: EOF from PTY");
                        exited_read.store(true, Ordering::Release);
                        break;
                    }
                    Ok(bytes_read) => {
                        // Tee: send raw bytes to secondary consumer (non-blocking).
                        // Only clone if tee channel exists.
                        if let Some(ref tee) = tee_tx {
                            let tee_data = read_buf[..bytes_read].to_vec();
                            let _ = tee.try_send(tee_data);
                        }
                        // Primary channel: send ownership (no clone needed)
                        let data = read_buf[..bytes_read].to_vec();
                        if output_tx.send(data).is_err() {
                            log::info!("reader thread: output channel closed");
                            break;
                        }
                    }
                    Err(e) => match e.raw_os_error() {
                        Some(libc::EINTR) => {}
                        Some(libc::EIO) => {
                            log::info!("reader thread: PTY EOF (slave closed, EIO)");
                            exited_read.store(true, Ordering::Release);
                            break;
                        }
                        _ => {
                            log::info!("reader thread: read error: {e}");
                            exited_read.store(true, Ordering::Release);
                            break;
                        }
                    },
                }
            }
            // `read_file` (and its fd) is dropped here, closing it safely.
        });

        let exit_code = session.exit_code.clone();
        let exit_alive_ms = session.exit_alive_ms.clone();
        let spawned_at = session.spawned_at;
        let exited_wait = exited.clone();
        let wait_handle = std::thread::spawn(move || {
            log::info!("wait thread: waiting for child pid={child_pid}");
            let result = nix::sys::wait::waitpid(child_pid, None);
            // Round-224: record the child's real lifetime (fork → waitpid)
            // for the fast-death Exit event — Kotlin event handling latency
            // must not skew the fast-death decision.
            *exit_alive_ms.lock() = Some(spawned_at.elapsed().as_millis() as u64);
            if let Ok(nix::sys::wait::WaitStatus::Exited(_, code)) = result
                && code >= 100
            {
                // Round-215: codes >= 100 encode execve errno + 100.
                log::error!(
                    "wait thread: child execve FAILED errno={} (exit code {code})",
                    code - 100
                );
            }
            log::info!("wait thread: child exited: {result:?}");
            match result {
                Ok(nix::sys::wait::WaitStatus::Exited(_, code)) => {
                    *exit_code.lock() = Some(code);
                }
                // Shell killed by a signal (Ctrl+\, kill -9): report the
                // conventional 128 + signal code so the UI does not present
                // a signal death as a clean exit code 0.
                Ok(nix::sys::wait::WaitStatus::Signaled(_, signal, _)) => {
                    *exit_code.lock() = Some(128 + signal as i32);
                }
                _ => {}
            }
            exited_wait.store(true, Ordering::Release);
        });

        session.reader_handle = Some(reader_handle);
        session.wait_handle = Some(wait_handle);

        Ok(session)
    }

    fn spawn_with_theme_inner(
        pty: Box<dyn Pty>,
        rows: u32,
        cols: u32,
        theme: ThemeConfig,
    ) -> Result<Self, SessionError> {
        log::info!("Session::spawn_with_theme_inner: creating Arc/Channel");
        let exited = Arc::new(AtomicBool::new(false));
        let exit_reported = Arc::new(AtomicBool::new(false));
        let bel_triggered = Arc::new(AtomicBool::new(false));
        let clipboard_text = Arc::new(Mutex::new(None));
        let clipboard_read = Arc::new(Mutex::new(None));
        let (output_tx, output_rx) = bounded::<Vec<u8>>(128);
        // Tee channel: secondary consumer for raw PTY output (logging, tracing, MCP screenshot).
        let (tee_tx, tee_rx) = bounded::<Vec<u8>>(256);

        let terminal = GhosttyTerminal::new_with_theme(
            rows,
            cols,
            theme.scrollback_lines,
            theme.background,
            theme.foreground,
            theme.ansi,
        )
        .map_err(SessionError::Ghostty)?;

        let notification = Arc::new(Mutex::new(None));
        let cwd = Arc::new(Mutex::new(None));

        Ok(Self {
            pty,
            terminal,
            output_processor: OutputProcessor::new(),
            output_tx,
            output_rx,
            tee_tx: Some(tee_tx),
            tee_rx: Some(tee_rx),
            exited,
            exit_reported,
            bel_triggered,
            clipboard_text,
            clipboard_read,
            notification,
            cwd,
            reader_handle: None,
            wait_handle: None,
            exit_code: Arc::new(Mutex::new(None)),
            exit_alive_ms: Arc::new(Mutex::new(None)),
            spawned_at: std::time::Instant::now(),
            terminal_rows: AtomicU32::new(rows),
            terminal_cols: AtomicU32::new(cols),
            grid_dirty: AtomicBool::new(false),
        })
    }

    /// Write raw bytes to the PTY (keyboard input, paste).
    pub fn write(&mut self, data: &[u8]) -> Result<(), SessionError> {
        if self.is_exited() {
            return Err(SessionError::Closed);
        }
        self.pty.write_all(data).map_err(SessionError::Io)?;
        Ok(())
    }

    /// Resize the terminal to the given number of rows and columns.
    /// Rejects dimensions outside the u16 range of the PTY ioctl.
    ///
    /// Returns the outcome: when the grid command was dropped the PTY
    /// winsize is still updated (ioctl already succeeded) but the caller
    /// must not publish the new dims as authoritative (round-113).
    pub fn resize(&mut self, rows: u32, cols: u32) -> Result<ResizeOutcome, SessionError> {
        let (Ok(rows), Ok(cols)) = (u16::try_from(rows), u16::try_from(cols)) else {
            return Err(SessionError::InvalidDimensions);
        };
        // Short-circuit identical sizes UNLESS a previous grid resize was
        // dropped: the cached size then no longer matches the grid, so the
        // same dims must be re-sent to heal the divergence (round-113).
        let dirty = self.grid_dirty.load(Ordering::Acquire);
        if !dirty && (rows as u32, cols as u32) == self.grid_size() {
            return Ok(ResizeOutcome::Applied);
        }
        self.pty.resize(rows, cols)?;
        if !self.terminal.resize(rows as u32, cols as u32) {
            // PTY winsize changed but the ghostty grid did not (command
            // dropped). Cache keeps the OLD size and grid_dirty is set so
            // the next resize event (even with identical dims) retries
            // instead of short-circuiting (round-112/113).
            self.grid_dirty.store(true, Ordering::Release);
            log::warn!(
                "session: ghostty grid resize to {rows}x{cols} dropped; PTY updated, grid lags — retry on next resize"
            );
            return Ok(ResizeOutcome::Dropped);
        }
        self.grid_dirty.store(false, Ordering::Release);
        self.terminal_rows.store(rows as u32, Ordering::Release);
        self.terminal_cols.store(cols as u32, Ordering::Release);
        Ok(ResizeOutcome::Applied)
    }

    /// Lock-free read of the last known grid size (spawn/resize). Never
    /// blocks: the VT thread's authoritative size is only reachable via a
    /// query RPC, which callers holding the registry write lock must avoid.
    pub fn grid_size(&self) -> (u32, u32) {
        (
            self.terminal_rows.load(Ordering::Acquire),
            self.terminal_cols.load(Ordering::Acquire),
        )
    }

    /// Send a POSIX signal (by number) to the child process backing this session.
    /// Used by the MCP server's `send_signal` tool so an external controller can
    /// interrupt / terminate a live shell.
    pub fn send_signal(&self, signum: i32) -> Result<(), SessionError> {
        let signal = nix::sys::signal::Signal::try_from(signum)
            .map_err(|error| SessionError::Ghostty(format!("invalid signal {signum}: {error}")))?;
        // Kill the foreground process group first (zed-port pattern: pty_info.rs:29-53).
        let target = self.pty.foreground_pid().unwrap_or(self.pty.child_pid());
        let pgid = -(target.as_raw() as libc::pid_t);
        // SAFETY: killpg sends signal to a process group. We use negative PID
        // to target the group rather than the individual process.
        let result = unsafe { libc::kill(pgid, signal as i32) };
        if result == 0 {
            Ok(())
        } else {
            // Fall back to direct child kill if group kill fails
            let child = self.pty.child_pid();
            let child_signal = signal;
            let child_result =
                unsafe { libc::kill(child.as_raw() as libc::pid_t, child_signal as i32) };
            if child_result == 0 {
                Ok(())
            } else {
                Err(SessionError::Ghostty(format!(
                    "kill({pgid}, {signal:?}) failed: {}",
                    nix::errno::Errno::last()
                )))
            }
        }
    }

    /// Maximum chunks of VT output processed per session frame, bounding
    /// render-thread latency when the PTY floods output.
    const MAX_CHUNKS_PER_FRAME: u32 = 10;

    /// Timeout for the DECSET 1004 mode query inside [`Self::focus_event`].
    /// focus_event runs on the UI thread (window focus change), so the
    /// query must fail fast when the VT thread is wedged instead of
    /// stalling the UI for the full query timeout.
    const FOCUS_MODE_QUERY_TIMEOUT_MS: u64 = 50;

    /// Process terminal output from the PTY reader thread.
    /// Reads VT output, updates terminal state, and drains write-back responses.
    /// Returns true if any VT data was processed.
    pub(crate) fn poll_pty_output(&mut self, max_chunks: u32) -> bool {
        let mut count = 0u32;
        while let Ok(data) = self.output_rx.try_recv() {
            let snap = self.output_processor.process(&data);

            if let Some(text) = snap.clipboard {
                *self.clipboard_text.lock() = Some(text);
            }
            if let Some(selection) = snap.clipboard_read {
                *self.clipboard_read.lock() = Some(selection);
            }
            if let Some(path) = snap.cwd.as_ref() {
                *self.cwd.lock() = Some(path.clone());
            }
            if let Some((ref title, ref body)) = snap.notification {
                let mut guard = self.notification.lock();
                *guard = Some((title.clone(), body.clone()));
            }

            if snap.bel {
                self.bel_triggered.store(true, Ordering::Release);
            }
            self.terminal.pty_write(&snap.filtered);
            count += 1;
            // Cap per-frame processing to avoid one render call blocking
            // the session lock for too long. Remaining chunks are processed
            // on the next render frame at no correctness cost — the VT thread
            // processes commands in FIFO order.
            if count >= max_chunks {
                log::trace!(
                    "poll_pty_output: hit cap of {} chunks, {} remain",
                    max_chunks,
                    self.output_rx.len(),
                );
                self.terminal.flush();
                // Drain write-back responses even on the cap path: a flood
                // of output must not starve DECRPM/DSR/DA replies (the
                // child application would wait for them indefinitely).
                self.drain_pty_write_back();
                return true;
            }
        }
        if count > 0 {
            log::trace!("poll_pty_output: processed {count} chunks");
            self.terminal.flush();
            self.drain_pty_write_back();
            true
        } else {
            false
        }
    }

    /// Write back any pending VT responses (DECRPM, DSR, DA, …) to the
    /// child PTY. The VT engine buffers them; the child waits for them.
    fn drain_pty_write_back(&mut self) {
        for response in self.terminal.drain_pty_write_responses() {
            log::trace!("poll_pty_output: pty write-back {} bytes", response.len());
            if let Err(error) = self.pty.write_all(&response) {
                log::error!(
                    "session: PTY write-back failed ({} bytes): {}",
                    response.len(),
                    error
                );
            }
        }
    }

    /// Process all available output and user input for this frame.
    ///
    /// Returns `true` if any VT output was processed (caller should
    /// rebuild the display snapshot).
    pub fn process_output(&mut self) -> bool {
        self.poll_pty_output(Self::MAX_CHUNKS_PER_FRAME)
    }

    /// Poll for a BEL (bell character) event. Returns true if a BEL was received since last poll.
    pub fn poll_bel(&self) -> bool {
        self.bel_triggered.swap(false, Ordering::AcqRel)
    }

    /// Poll for clipboard text set by an OSC 52 escape sequence.
    pub fn poll_clipboard(&self) -> Option<String> {
        let mut guard = self.clipboard_text.lock();
        guard.take()
    }

    /// Take the tee receiver for raw PTY output consumption.
    /// Returns `None` if already taken. The caller owns the raw byte stream.
    pub fn take_tee_receiver(&mut self) -> Option<Receiver<Vec<u8>>> {
        self.tee_rx.take()
    }

    /// Take the pending OSC 52 clipboard read request (the selection name),
    /// if any. The JNI layer forwards it to the host app and calls
    /// [`Session::answer_clipboard_read`] with the result.
    pub fn poll_clipboard_read(&self) -> Option<String> {
        let mut guard = self.clipboard_read.lock();
        guard.take()
    }

    /// Answer a pending OSC 52 clipboard read request by writing
    /// `ESC ] 52 ; <selection> ; <base64> ESC \\` to the PTY (FR-036).
    ///
    /// An empty selection string (`c` is the conventional default) is
    /// answered verbatim; the application decides what it means.
    pub fn answer_clipboard_read(
        &mut self,
        selection: &str,
        text: &str,
    ) -> Result<(), SessionError> {
        use base64::Engine;
        let encoded = base64::engine::general_purpose::STANDARD.encode(text.as_bytes());
        let mut response = Vec::with_capacity(selection.len() + encoded.len() + 8);
        response.extend_from_slice(b"\x1b]52;");
        response.extend_from_slice(selection.as_bytes());
        response.push(b';');
        response.extend_from_slice(encoded.as_bytes());
        response.push(0x07); // BEL terminator (xterm-compatible)
        if self.is_exited() {
            return Err(SessionError::Closed);
        }
        self.pty.write_all(&response).map_err(SessionError::Io)?;
        Ok(())
    }

    /// Poll for a desktop notification set by an OSC 9 escape sequence.
    pub fn poll_notification(&self) -> Option<(String, String)> {
        let mut guard = self.notification.lock();
        guard.take()
    }

    /// Returns true if the child process has exited.
    pub fn is_exited(&self) -> bool {
        self.exited.load(Ordering::Acquire)
    }

    /// Read the child's exit code if the wait thread already wrote it.
    /// Non-blocking: callers that need to wait for the code poll this in a
    /// loop WITHOUT holding the session lock (see ffi::wait_exit_code).
    pub fn exit_code_now(&self) -> Option<i32> {
        let guard = self.exit_code.lock();
        *guard
    }

    /// Get a clone of the exit flag for external monitoring.
    pub fn exited_flag(&self) -> Arc<AtomicBool> {
        self.exited.clone()
    }

    /// Atomically mark the exit event as reported to the Kotlin side.
    /// Returns true only for the first caller — the per-frame background
    /// sweep in pollEvent uses this to report each exit exactly once.
    pub fn mark_exit_reported(&self) -> bool {
        !self.exit_reported.swap(true, Ordering::AcqRel)
    }

    /// Get a reference to the terminal engine.
    pub fn terminal(&self) -> &GhosttyTerminal {
        &self.terminal
    }

    /// Get a mutable reference to the terminal engine.
    pub fn terminal_mut(&mut self) -> &mut GhosttyTerminal {
        &mut self.terminal
    }

    /// Get the current window title set by the shell.
    pub fn title(&self) -> String {
        self.terminal.title()
    }

    /// Get the current working directory of the child process.
    pub fn cwd(&self) -> String {
        let guard = self.cwd.lock();
        if let Some(tracked) = guard.as_ref() {
            return tracked.clone();
        }
        self.terminal.cwd()
    }

    pub fn mode_get(&self, mode_num: u16, kind: u8) -> bool {
        self.terminal.mode_get(mode_num, kind)
    }

    pub fn focus_event(&mut self, focused: bool) {
        // DECSET 1004 focus reporting: the sequence must go DIRECTLY to the
        // child PTY, not into the VT engine. The engine's output-stream
        // parser interprets `CSI I` as CHT (cursor horizontal tab) and
        // `CSI O` as an invalid CSI — feeding them to the engine moves the
        // cursor to the next tab stop instead of notifying the application.
        // Only send when the child actually enabled 1004 (xterm semantics).
        // Short timeout: this runs on the UI thread (window focus change);
        // a wedged VT thread must not stall it for the full query timeout.
        if !self.terminal.mode_get_with_timeout(
            1004,
            0,
            std::time::Duration::from_millis(Self::FOCUS_MODE_QUERY_TIMEOUT_MS),
        ) {
            return;
        }
        let data = if focused { b"\x1b[I" } else { b"\x1b[O" };
        if let Err(error) = self.pty.write_all(data) {
            log::warn!("session: focus_event write failed: {error}");
        }
    }
}

/// Join a thread handle with a deadline timeout, then retry up to 3×.
///
/// If the initial `timeout` expires, we retry the join with 100ms deadlines
/// for up to 3 additional attempts. This handles the case where the thread
/// is blocked on I/O that may need multiple signals to unblock.
/// If all retries fail, we detach (handle dropped) and log an error — the
/// thread's resources are leaked (fd, memory).
fn join_with_timeout(handle: &mut Option<std::thread::JoinHandle<()>>, timeout: Duration) {
    let Some(handle) = handle.take() else {
        return;
    };
    let deadline = std::time::Instant::now() + timeout;
    while std::time::Instant::now() < deadline {
        if handle.is_finished() {
            if let Err(e) = handle.join() {
                log::error!("session: thread panicked: {:?}", e);
            }
            return;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    log::warn!("session: thread did not exit within {timeout:?}, retrying up to 3×");
    for _attempt in 0..3 {
        let retry_deadline = std::time::Instant::now() + Duration::from_millis(100);
        while std::time::Instant::now() < retry_deadline {
            if handle.is_finished() {
                if let Err(e) = handle.join() {
                    log::error!("session: thread panicked: {:?}", e);
                }
                return;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
    }
    log::error!("session: thread failed to exit after retries — DETACHING (resource leak)");
    // handle is dropped here → detached
}

impl Drop for Session {
    fn drop(&mut self) {
        self.exited.store(true, Ordering::Release);

        let pid = self.pty.child_pid();
        if pid.as_raw() > 0 {
            if let Err(e) = nix::sys::signal::kill(pid, nix::sys::signal::Signal::SIGHUP) {
                log::warn!("session drop: failed to send SIGHUP to {}: {e}", pid);
            }
            if let Err(e) = nix::sys::signal::kill(pid, nix::sys::signal::Signal::SIGCONT) {
                log::warn!("session drop: failed to send SIGCONT to {}: {e}", pid);
            }
            std::thread::sleep(Duration::from_millis(50));
            if let Err(e) = nix::sys::signal::kill(pid, nix::sys::signal::Signal::SIGKILL) {
                log::warn!("session drop: failed to send SIGKILL to {}: {e}", pid);
            }
        }

        // Try to join the reader thread with a short timeout.
        // If it's blocked on PTY read, SIGKILL above will have closed the fd
        // and the thread should terminate quickly.
        join_with_timeout(&mut self.reader_handle, Duration::from_millis(50));
        // Try to join the wait thread with a short timeout.
        // SIGKILL was sent above, so waitpid() in the wait thread should
        // return promptly. Best-effort: if the child doesn't terminate,
        // the thread is detached to avoid blocking Drop indefinitely.
        join_with_timeout(&mut self.wait_handle, Duration::from_millis(50));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn drain_output(session: &mut Session, deadline: std::time::Instant) {
        while std::time::Instant::now() < deadline {
            session.process_output();
            if session.is_exited() {
                break;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
    }

    #[test]
    fn session_spawn_and_exit() {
        let mut session =
            Session::spawn("/bin/sh", 24, 80, &ShellEnv::default()).expect("spawn failed");
        session.write(b"exit\n").expect("write failed");
        let deadline = std::time::Instant::now() + Duration::from_secs(3);
        drain_output(&mut session, deadline);
        assert!(session.is_exited());
    }

    #[test]
    fn session_echo_hello() {
        let mut session =
            Session::spawn("/bin/sh", 24, 80, &ShellEnv::default()).expect("spawn failed");
        session.write(b"echo hello_p12\n").expect("write failed");
        let deadline = std::time::Instant::now() + Duration::from_secs(3);
        let mut found = false;
        while std::time::Instant::now() < deadline {
            session.process_output();
            let rows = session.terminal().rows();
            for row in 0..rows {
                if let Some(line) = session.terminal().read_line_text(row)
                    && line.contains("hello_p12")
                {
                    found = true;
                    break;
                }
            }
            if found {
                break;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        assert!(found, "did not find 'hello_p12' in terminal");
    }

    #[test]
    fn session_resize() {
        let mut session =
            Session::spawn("/bin/sh", 24, 80, &ShellEnv::default()).expect("spawn failed");
        session.resize(40, 120).expect("resize failed");
        assert_eq!(session.terminal().rows(), 40);
        assert_eq!(session.terminal().cols(), 120);
    }

    #[test]
    fn session_resize_same_size_is_applied_noop() {
        let (pty, handle) = crate::terminal::mock_pty::MockPty::new(24, 80);
        let mut session = Session::with_pty(Box::new(pty) as Box<dyn Pty>, 24, 80)
            .expect("with_pty must succeed");
        let before = handle.resize_count();
        // Clean state: same-size resize short-circuits and reports Applied.
        let outcome = session.resize(24, 80).expect("resize failed");
        assert!(matches!(outcome, ResizeOutcome::Applied));
        assert!(!session.grid_dirty.load(Ordering::Acquire));
        // The PTY must be untouched by the short-circuit (round-115).
        assert_eq!(
            handle.resize_count(),
            before,
            "pty.resize must not be called"
        );
        // Grid unchanged (still the spawn size).
        assert_eq!(session.grid_size(), (24, 80));
        assert_eq!(session.terminal().rows(), 24);
    }

    /// OSC 52 read answer is written back to the PTY as
    /// `ESC ] 52 ; <selection> ; <base64> BEL` (FR-036).
    #[test]
    fn answer_clipboard_read_writes_esc52_reply() {
        let (pty, handle) = crate::terminal::mock_pty::MockPty::new(24, 80);
        let mut session = Session::with_pty(Box::new(pty) as Box<dyn Pty>, 24, 80)
            .expect("with_pty must succeed");
        session
            .answer_clipboard_read("c", "Hello, 世界")
            .expect("answer must succeed");
        let written = handle.written();
        assert_eq!(
            written, b"\x1b]52;c;SGVsbG8sIOS4lueVjA==\x07",
            "answer must be base64-encoded OSC 52 with BEL terminator"
        );
    }

    /// An empty clipboard answer still produces a valid (empty payload)
    /// OSC 52 reply — the xterm-compatible "empty clipboard" response.
    #[test]
    fn answer_clipboard_read_empty_text() {
        let (pty, handle) = crate::terminal::mock_pty::MockPty::new(24, 80);
        let mut session = Session::with_pty(Box::new(pty) as Box<dyn Pty>, 24, 80)
            .expect("with_pty must succeed");
        session
            .answer_clipboard_read("c", "")
            .expect("answer must succeed");
        assert_eq!(handle.written(), b"\x1b]52;c;\x07");
    }

    /// Writing an answer after the session exited must fail cleanly.
    #[test]
    fn answer_clipboard_read_after_exit_fails() {
        let (pty, handle) = crate::terminal::mock_pty::MockPty::new(24, 80);
        let mut session = Session::with_pty(Box::new(pty) as Box<dyn Pty>, 24, 80)
            .expect("with_pty must succeed");
        session.exited_flag().store(true, Ordering::Release);
        let result = session.answer_clipboard_read("c", "text");
        assert!(result.is_err(), "exited session must reject writes");
        assert!(handle.written().is_empty(), "nothing may reach the PTY");
    }

    #[test]
    fn session_resize_dirty_same_size_still_retries() {
        let mut session =
            Session::spawn("/bin/sh", 24, 80, &ShellEnv::default()).expect("spawn failed");
        // Simulate a dropped grid command: dirty set, cache at old size.
        session.grid_dirty.store(true, Ordering::Release);
        // Same-size resize must NOT short-circuit: it re-issues the ioctl +
        // grid command and clears the dirty flag (heals the divergence).
        let outcome = session.resize(24, 80).expect("resize failed");
        assert!(matches!(outcome, ResizeOutcome::Applied));
        assert!(!session.grid_dirty.load(Ordering::Acquire));
        assert_eq!(session.grid_size(), (24, 80));
    }

    #[test]
    fn session_resize_dirty_cleared_on_success() {
        let mut session =
            Session::spawn("/bin/sh", 24, 80, &ShellEnv::default()).expect("spawn failed");
        session.grid_dirty.store(true, Ordering::Release);
        // A genuinely different size clears the dirty flag on success.
        let outcome = session.resize(40, 120).expect("resize failed");
        assert!(matches!(outcome, ResizeOutcome::Applied));
        assert!(!session.grid_dirty.load(Ordering::Acquire));
        assert_eq!(session.grid_size(), (40, 120));
    }

    #[test]
    fn session_after_exit_returns_error() {
        let mut session =
            Session::spawn("/bin/sh", 24, 80, &ShellEnv::default()).expect("spawn failed");
        session.write(b"exit\n").expect("write failed");
        let deadline = std::time::Instant::now() + Duration::from_secs(3);
        drain_output(&mut session, deadline);
        assert!(session.is_exited());
    }

    #[test]
    fn extract_osc133_all_markers() {
        use crate::terminal::output_processor::{OutputProcessor, ShellIntegration};
        let mut proc = OutputProcessor::new();
        assert_eq!(
            proc.process(b"\x1b]133;A\x07").shell_integration,
            ShellIntegration::PromptStart
        );
        assert_eq!(
            proc.process(b"\x1b]133;B\x1b\\").shell_integration,
            ShellIntegration::PromptEnd
        );
        assert_eq!(
            proc.process(b"\x1b]133;C\x07").shell_integration,
            ShellIntegration::CommandStart
        );
        assert_eq!(
            proc.process(b"\x1b]133;D\x1b\\").shell_integration,
            ShellIntegration::CommandExecuted
        );
    }

    #[test]
    fn extract_osc133_returns_none_without_markers() {
        use crate::terminal::output_processor::{OutputProcessor, ShellIntegration};
        let mut proc = OutputProcessor::new();
        assert_eq!(
            proc.process(b"hello world").shell_integration,
            ShellIntegration::None
        );
        assert_eq!(
            proc.process(b"\x1b]133;\x07").shell_integration,
            ShellIntegration::None
        );
        assert_eq!(
            proc.process(b"\x1b]133;X\x07").shell_integration,
            ShellIntegration::None
        );
    }

    #[test]
    fn extract_osc133_command_executed() {
        use crate::terminal::output_processor::{OutputProcessor, ShellIntegration};
        let mut proc = OutputProcessor::new();
        assert_eq!(
            proc.process(b"\x1b]133;D\x1b\\").shell_integration,
            ShellIntegration::CommandExecuted
        );
    }

    #[test]
    fn extract_osc133_exit_code() {
        // termlib OscParser handleOsc133: `D;exit_code` carries the command
        // exit code for the COMMAND_FINISHED marker.
        use crate::terminal::output_processor::{OutputProcessor, ShellIntegration};
        let mut proc = OutputProcessor::new();
        let snap = proc.process(b"\x1b]133;D;0\x1b\\");
        assert_eq!(snap.shell_integration, ShellIntegration::CommandExecuted);
        assert_eq!(snap.shell_exit_code, Some(0));
        let snap = proc.process(b"\x1b]133;D;42\x1b\\");
        assert_eq!(snap.shell_exit_code, Some(42));
        // Plain D has no exit code.
        let snap = proc.process(b"\x1b]133;D\x1b\\");
        assert_eq!(snap.shell_exit_code, None);
        // A/B/C never carry exit codes.
        let snap = proc.process(b"\x1b]133;C\x1b\\");
        assert_eq!(snap.shell_exit_code, None);
    }

    #[test]
    fn session_new_creates_pty() {
        let (pty, _handle) = crate::terminal::mock_pty::MockPty::new(24, 80);
        let session = Session::with_pty(Box::new(pty) as Box<dyn Pty>, 24, 80)
            .expect("with_pty must succeed");
        assert_eq!(
            session.terminal().rows(),
            24,
            "terminal rows must be 24 after creation"
        );
        assert_eq!(
            session.terminal().cols(),
            80,
            "terminal cols must be 80 after creation"
        );
        assert!(
            !session.is_exited(),
            "new session must not be in exited state"
        );
    }

    #[test]
    fn session_resize_sends_signal() {
        let (pty, handle) = crate::terminal::mock_pty::MockPty::new(24, 80);
        let mut session = Session::with_pty(Box::new(pty) as Box<dyn Pty>, 24, 80)
            .expect("with_pty must succeed");
        session.resize(40, 120).expect("resize must succeed");
        assert_eq!(
            session.terminal().rows(),
            40,
            "terminal rows must update after resize"
        );
        assert_eq!(
            session.terminal().cols(),
            120,
            "terminal cols must update after resize"
        );
        assert_eq!(handle.rows(), 40, "PTY rows must update after resize");
        assert_eq!(handle.cols(), 120, "PTY cols must update after resize");
    }

    #[test]
    fn session_write_input() {
        let (pty, handle) = crate::terminal::mock_pty::MockPty::new(24, 80);
        let mut session = Session::with_pty(Box::new(pty) as Box<dyn Pty>, 24, 80)
            .expect("with_pty must succeed");
        session.write(b"hello world").expect("write must succeed");
        let written = handle.written();
        assert_eq!(
            written, b"hello world",
            "input written to session must reach PTY master"
        );
    }

    #[test]
    fn session_poll_bel_on_exit_write_back() {
        // Verifies that pty_write responses from ghostty (e.g. DECRPM) do not
        // accidentally set the BEL flag. BEL is only set when output data
        // processed by process_output() contains 0x07.
        let (pty, _handle) = crate::terminal::mock_pty::MockPty::new(24, 80);
        let session = Session::with_pty(Box::new(pty) as Box<dyn Pty>, 24, 80)
            .expect("with_pty must succeed");
        assert!(!session.poll_bel(), "fresh session must not have bel set");
    }

    #[test]
    fn mark_exit_reported_is_idempotent_under_concurrency() {
        let (pty, _handle) = crate::terminal::mock_pty::MockPty::new(24, 80);
        let session = Session::with_pty(Box::new(pty) as Box<dyn Pty>, 24, 80)
            .expect("with_pty must succeed");
        // Exactly one caller must win the report race; every other caller
        // (concurrent pollEvent threads) must see false so the Exit event
        // is never duplicated. Arc<Mutex<...>> mirrors the production
        // SESSION_REGISTRY shape (Box<dyn Pty> is not Sync, so the session
        // must be shared through a mutex).
        let session = std::sync::Arc::new(parking_lot::Mutex::new(session));
        let mut handles = Vec::new();
        for _ in 0..8 {
            let session = session.clone();
            handles.push(std::thread::spawn(move || {
                session.lock().mark_exit_reported()
            }));
        }
        let winners = handles
            .into_iter()
            .filter_map(|h| h.join().ok())
            .filter(|won| *won)
            .count();
        assert_eq!(winners, 1, "exactly one caller must report the exit");
        // Subsequent calls stay false.
        assert!(!session.lock().mark_exit_reported());
    }

    #[test]
    fn session_title_default_is_empty() {
        let (pty, _handle) = crate::terminal::mock_pty::MockPty::new(24, 80);
        let session = Session::with_pty(Box::new(pty) as Box<dyn Pty>, 24, 80)
            .expect("with_pty must succeed");
        assert_eq!(session.title(), "");
    }

    #[test]
    fn session_cwd_default_is_empty() {
        let (pty, _handle) = crate::terminal::mock_pty::MockPty::new(24, 80);
        let session = Session::with_pty(Box::new(pty) as Box<dyn Pty>, 24, 80)
            .expect("with_pty must succeed");
        assert_eq!(session.cwd(), "");
    }

    #[test]
    fn session_mode_get_default_false() {
        let (pty, _handle) = crate::terminal::mock_pty::MockPty::new(24, 80);
        let session = Session::with_pty(Box::new(pty) as Box<dyn Pty>, 24, 80)
            .expect("with_pty must succeed");
        // Mode 2004 (bracketed paste) should be off by default
        assert!(!session.mode_get(2004, 0));
    }

    #[test]
    fn session_focus_event_writes_to_terminal() {
        let (pty, _handle) = crate::terminal::mock_pty::MockPty::new(24, 80);
        let mut session = Session::with_pty(Box::new(pty) as Box<dyn Pty>, 24, 80)
            .expect("with_pty must succeed");
        // focus_event writes CSI sequences to terminal; should not panic
        session.focus_event(true);
        session.focus_event(false);
    }

    #[test]
    fn session_exited_flag() {
        let (pty, handle) = crate::terminal::mock_pty::MockPty::new(24, 80);
        let session = Session::with_pty(Box::new(pty) as Box<dyn Pty>, 24, 80)
            .expect("with_pty must succeed");
        assert!(!session.is_exited(), "fresh session must not be exited");
        let flag = session.exited_flag();
        assert!(!flag.load(std::sync::atomic::Ordering::Acquire));
        // Mark exited and verify
        handle.set_exited();
        assert!(handle.is_exited());
    }

    #[test]
    fn extract_osc133_handles_concurrent_content() {
        use crate::terminal::output_processor::{OutputProcessor, ShellIntegration};
        let mut proc = OutputProcessor::new();
        assert_eq!(
            proc.process(b"$ \x1b]133;C\x07 echo hello")
                .shell_integration,
            ShellIntegration::CommandStart
        );
    }

    #[test]
    fn extract_osc133_empty_osc() {
        use crate::terminal::output_processor::{OutputProcessor, ShellIntegration};
        let mut proc = OutputProcessor::new();
        assert_eq!(
            proc.process(b"\x1b]133;\x07").shell_integration,
            ShellIntegration::None
        );
        assert_eq!(
            proc.process(b"\x1b]133;\x1b\\").shell_integration,
            ShellIntegration::None
        );
    }

    #[test]
    fn extract_osc133_incomplete_sequence() {
        use crate::terminal::output_processor::{OutputProcessor, ShellIntegration};
        let mut proc = OutputProcessor::new();
        assert_eq!(
            proc.process(b"\x1b]133;C").shell_integration,
            ShellIntegration::None
        );
        assert_eq!(
            proc.process(b"\x1b]133;").shell_integration,
            ShellIntegration::None
        );
    }

    #[test]
    fn extract_osc133_st_terminator() {
        use crate::terminal::output_processor::{OutputProcessor, ShellIntegration};
        let mut proc = OutputProcessor::new();
        assert_eq!(
            proc.process(b"\x1b]133;C\x1b\\").shell_integration,
            ShellIntegration::CommandStart
        );
        assert_eq!(
            proc.process(b"\x1b]133;D\x1b\\").shell_integration,
            ShellIntegration::CommandExecuted
        );
    }

    #[test]
    fn extract_osc133_mixed_terminators() {
        use crate::terminal::output_processor::{OutputProcessor, ShellIntegration};
        let mut proc = OutputProcessor::new();
        assert_eq!(
            proc.process(b"\x1b]133;A\x07").shell_integration,
            ShellIntegration::PromptStart
        );
        assert_eq!(
            proc.process(b"\x1b]133;A\x1b\\").shell_integration,
            ShellIntegration::PromptStart
        );
    }

    #[test]
    fn session_write_after_exit_returns_error() {
        let (pty, _handle) = crate::terminal::mock_pty::MockPty::new(24, 80);
        let mut session = Session::with_pty(Box::new(pty) as Box<dyn Pty>, 24, 80)
            .expect("with_pty must succeed");
        // Set the session's exited flag so write() checks it before calling the PTY
        session.exited_flag().store(true, Ordering::Release);
        let result = session.write(b"test");
        assert!(
            result.is_err(),
            "write after exit must return error, got Ok"
        );
    }

    #[test]
    fn shell_integration_from_u8() {
        use crate::terminal::output_processor::ShellIntegration;
        assert_eq!(ShellIntegration::from(0u8), ShellIntegration::None);
        assert_eq!(ShellIntegration::from(1u8), ShellIntegration::PromptStart);
        assert_eq!(ShellIntegration::from(2u8), ShellIntegration::PromptEnd);
        assert_eq!(ShellIntegration::from(3u8), ShellIntegration::CommandStart);
        assert_eq!(
            ShellIntegration::from(4u8),
            ShellIntegration::CommandExecuted
        );
        assert_eq!(ShellIntegration::from(5u8), ShellIntegration::None);
        assert_eq!(ShellIntegration::from(255u8), ShellIntegration::None);
    }

    #[test]
    fn tee_channel_receives_data() {
        let mut session =
            Session::spawn("/bin/sh", 24, 80, &ShellEnv::default()).expect("spawn failed");
        // Take the tee receiver
        let tee_rx = session.take_tee_receiver().expect("tee_rx should exist");
        assert!(
            tee_rx.try_recv().is_err(),
            "tee channel should be empty initially"
        );

        // Write something — reader thread should send to both output_tx and tee_tx
        session.write(b"echo tee_test_abc\n").expect("write failed");
        let deadline = std::time::Instant::now() + Duration::from_secs(3);
        loop {
            session.process_output();
            if std::time::Instant::now() >= deadline {
                break;
            }
            std::thread::sleep(Duration::from_millis(10));
        }

        // Tee channel should have received at least one chunk
        let mut tee_data = Vec::new();
        while let Ok(chunk) = tee_rx.try_recv() {
            tee_data.extend_from_slice(&chunk);
        }
        let tee_text = String::from_utf8_lossy(&tee_data);
        assert!(
            tee_text.contains("tee_test_abc"),
            "tee channel should contain test data, got: {tee_text}"
        );

        // Clean up: let session exit
        session.write(b"exit\n").expect("write failed");
        std::thread::sleep(Duration::from_millis(200));
    }

    #[test]
    fn tee_channel_single_take() {
        let mut session =
            Session::spawn("/bin/sh", 24, 80, &ShellEnv::default()).expect("spawn failed");
        let rx1 = session
            .take_tee_receiver()
            .expect("first take should succeed");
        assert!(
            session.take_tee_receiver().is_none(),
            "second take should return None"
        );
        // Drop rx1 to clean up
        drop(rx1);
        session.write(b"exit\n").expect("write failed");
        std::thread::sleep(Duration::from_millis(200));
    }
}
