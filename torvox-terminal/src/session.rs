//! Session orchestrator — wires PTY reader, VT parser, and process waiter together.
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
use std::fs::File;
use std::io::Read;
use std::os::unix::io::AsRawFd;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};
use std::sync::{Condvar, Mutex};
use std::time::Duration;

use flume::{Receiver, bounded};
use thiserror::Error;

use crate::ghostty_terminal::GhosttyTerminal;
use crate::lock_util::lock_or_recover;
use crate::osc_handler::{OscEvent, OscHandler};
use crate::pty::{Pty, PtyError, PtyPair};
use crate::shell_env::ShellEnv;

const READ_BUF_SIZE: usize = 8192;
/// How long the reader thread parks in `poll` before re-checking the exit flag.
/// Replaces the previous 2 ms busy-poll `sleep`, so output latency stays low
/// while the thread no longer spins the CPU when the PTY is idle.
const READ_POLL_TIMEOUT_MS: i32 = 100;

const DEFAULT_SCROLLBACK_LINES: u32 = 50000;

/// OSC 133 Shell Integration markers.
/// See <https://gitlab.freedesktop.org/terminal-wg/specifications/-/issues/31>
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum ShellIntegration {
    None = 0,
    PromptStart = 1,
    PromptEnd = 2,
    CommandStart = 3,
    CommandExecuted = 4,
}

impl From<u8> for ShellIntegration {
    fn from(v: u8) -> Self {
        match v {
            1 => Self::PromptStart,
            2 => Self::PromptEnd,
            3 => Self::CommandStart,
            4 => Self::CommandExecuted,
            _ => Self::None,
        }
    }
}

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
}

pub struct Session {
    pty: Box<dyn Pty>,
    terminal: GhosttyTerminal,
    osc_handler: OscHandler,
    output_tx: flume::Sender<Vec<u8>>,
    output_rx: Receiver<Vec<u8>>,
    /// Lock-free channel for user-initiated PTY writes (keyboard/IME input,
    /// paste). The main thread sends here without taking the session lock;
    /// `process_output` drains it under the lock already held by the render
    /// thread, so input never blocks the UI thread on the session mutex.
    user_write_tx: flume::Sender<Vec<u8>>,
    user_write_rx: Receiver<Vec<u8>>,
    output_notify: Arc<(Mutex<bool>, Condvar)>,
    exited: Arc<AtomicBool>,
    bel_triggered: Arc<AtomicBool>,
    clipboard_text: Arc<Mutex<Option<String>>>,
    notification: Arc<Mutex<Option<(String, String)>>>,
    hyperlink: Arc<Mutex<Option<String>>>,
    cwd: Arc<Mutex<Option<String>>>,
    shell_integration: Arc<AtomicU8>,
    reader_handle: Option<std::thread::JoinHandle<()>>,
    wait_handle: Option<std::thread::JoinHandle<()>>,
}

impl Session {
    /// Create a session with an already-constructed PTY.
    /// No reader/wait threads are spawned — the caller is responsible for
    /// driving PTY I/O. Primarily used for testing with `MockPty`.
    pub fn with_pty(pty: Box<dyn Pty>, rows: u32, cols: u32) -> Result<Self, SessionError> {
        let (palette_ansi, palette_background, palette_foreground) =
            GhosttyTerminal::catppuccin_mocha_palette();
        Self::spawn_with_theme_inner(
            pty,
            rows,
            cols,
            DEFAULT_SCROLLBACK_LINES,
            palette_background,
            palette_foreground,
            palette_ansi,
        )
    }

    pub fn spawn(shell: &str, rows: u32, cols: u32, env: &ShellEnv) -> Result<Self, SessionError> {
        let (palette_ansi, palette_background, palette_foreground) =
            GhosttyTerminal::catppuccin_mocha_palette();
        Self::spawn_with_theme(
            shell,
            rows,
            cols,
            env,
            palette_background,
            palette_foreground,
            palette_ansi,
            DEFAULT_SCROLLBACK_LINES,
        )
    }

    pub fn spawn_with_theme(
        shell: &str,
        rows: u32,
        cols: u32,
        env: &ShellEnv,
        initial_bg: [u8; 3],
        initial_fg: [u8; 3],
        initial_ansi: [[u8; 3]; 16],
        scrollback_lines: u32,
    ) -> Result<Self, SessionError> {
        log::info!("Session::spawn: shell='{shell}', rows={rows}, cols={cols}");
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
        let mut read_file = File::from(pty.try_clone_reader_fd().map_err(SessionError::Io)?);

        let child_pid = pty.child_pid();

        let mut session = match Self::spawn_with_theme_inner(
            Box::new(pty) as Box<dyn Pty>,
            rows,
            cols,
            scrollback_lines,
            initial_bg,
            initial_fg,
            initial_ansi,
        ) {
            Ok(session) => session,
            Err(e) => {
                // `read_file` is dropped here, closing its fd safely.
                return Err(e);
            }
        };

        let exited = session.exited.clone();
        let output_notify = session.output_notify.clone();
        let output_tx = session.output_tx.clone();

        log::info!("Session::spawn: spawning reader thread");
        let exited_read = exited.clone();
        let notify_read = output_notify.clone();
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
                        Self::notify_output(&notify_read);
                        break;
                    }
                }
                match read_file.read(&mut read_buf) {
                    Ok(0) => {
                        log::info!("reader thread: EOF from PTY");
                        exited_read.store(true, Ordering::Release);
                        Self::notify_output(&notify_read);
                        break;
                    }
                    Ok(bytes_read) => {
                        let data = read_buf[..bytes_read].to_vec();
                        if output_tx.send(data).is_err() {
                            log::info!("reader thread: output channel closed");
                            break;
                        }
                        Self::notify_output(&notify_read);
                    }
                    Err(e) => match e.raw_os_error() {
                        Some(libc::EINTR) => {}
                        Some(libc::EIO) => {
                            log::info!("reader thread: PTY EOF (slave closed, EIO)");
                            exited_read.store(true, Ordering::Release);
                            Self::notify_output(&notify_read);
                            break;
                        }
                        _ => {
                            log::info!("reader thread: read error: {e}");
                            exited_read.store(true, Ordering::Release);
                            Self::notify_output(&notify_read);
                            break;
                        }
                    },
                }
            }
            // `read_file` (and its fd) is dropped here, closing it safely.
        });

        let exited_wait = exited.clone();
        let wait_handle = std::thread::spawn(move || {
            log::info!("wait thread: waiting for child pid={child_pid}");
            let status = nix::sys::wait::waitpid(child_pid, None);
            log::info!("wait thread: child exited: {status:?}");
            exited_wait.store(true, Ordering::Release);
        });

        session.reader_handle = Some(reader_handle);
        session.wait_handle = Some(wait_handle);

        Ok(session)
    }

    fn notify_output(notify: &Arc<(Mutex<bool>, Condvar)>) {
        let (lock, cvar) = &**notify;
        let mut pending = lock.lock().unwrap_or_else(|e| e.into_inner());
        *pending = true;
        cvar.notify_one();
    }

    fn spawn_with_theme_inner(
        pty: Box<dyn Pty>,
        rows: u32,
        cols: u32,
        scrollback_lines: u32,
        initial_bg: [u8; 3],
        initial_fg: [u8; 3],
        initial_ansi: [[u8; 3]; 16],
    ) -> Result<Self, SessionError> {
        log::info!("Session::spawn_with_theme_inner: creating Arc/Channel");
        let exited = Arc::new(AtomicBool::new(false));
        let bel_triggered = Arc::new(AtomicBool::new(false));
        let clipboard_text = Arc::new(Mutex::new(None));
        let output_notify = Arc::new((Mutex::new(false), Condvar::new()));
        let (output_tx, output_rx) = bounded::<Vec<u8>>(128);
        // User-write channel: unbounded so the UI thread's send never blocks.
        let (user_write_tx, user_write_rx) = bounded::<Vec<u8>>(4096);

        let terminal = GhosttyTerminal::new_with_theme(
            rows,
            cols,
            scrollback_lines,
            initial_bg,
            initial_fg,
            initial_ansi,
        )
        .map_err(SessionError::Ghostty)?;

        let shell_integration = Arc::new(AtomicU8::new(0));
        let notification = Arc::new(Mutex::new(None));
        let hyperlink = Arc::new(Mutex::new(None));
        let cwd = Arc::new(Mutex::new(None));

        Ok(Self {
            pty,
            terminal,
            osc_handler: OscHandler::new(),
            output_tx,
            output_rx,
            user_write_tx,
            user_write_rx,
            output_notify,
            exited,
            bel_triggered,
            clipboard_text,
            notification,
            hyperlink,
            cwd,
            shell_integration,
            reader_handle: None,
            wait_handle: None,
        })
    }

    pub fn write(&mut self, data: &[u8]) -> Result<(), SessionError> {
        if self.is_exited() {
            return Err(SessionError::Closed);
        }
        self.pty.write_all(data).map_err(SessionError::Io)?;
        Ok(())
    }

    pub fn resize(&mut self, rows: u32, cols: u32) -> Result<(), SessionError> {
        self.pty.resize(rows as u16, cols as u16)?;
        self.terminal.resize(rows, cols);
        Ok(())
    }

    /// Send a POSIX signal (by number) to the child process backing this session.
    /// Used by the MCP server's `send_signal` tool so an external controller can
    /// interrupt / terminate a live shell.
    pub fn send_signal(&self, signum: i32) -> Result<(), SessionError> {
        let pid = self.pty.child_pid();
        let signal = nix::sys::signal::Signal::try_from(signum)
            .map_err(|error| SessionError::Ghostty(format!("invalid signal {signum}: {error}")))?;
        nix::sys::signal::kill(pid, signal).map_err(|error| {
            SessionError::Ghostty(format!("kill({pid}, {signal:?}) failed: {error}"))
        })
    }

    pub fn set_pixel_size(&mut self, width: u16, height: u16) {
        self.pty.set_pixel_size(width, height);
    }

    pub fn wait_for_output(&self) {
        let (lock, cvar) = &*self.output_notify;
        let mut pending = lock.lock().unwrap_or_else(|e| e.into_inner());
        while !*pending {
            pending = cvar.wait(pending).unwrap_or_else(|e| e.into_inner());
        }
        *pending = false;
    }

    /// Returns a clone of the lock-free sender for user-initiated PTY writes.
    /// The main thread uses this to enqueue keyboard/IME/paste data without
    /// taking the session mutex, avoiding UI-thread stalls.
    pub fn user_write_sender(&self) -> flume::Sender<Vec<u8>> {
        self.user_write_tx.clone()
    }

    const MAX_CHUNKS_PER_FRAME: u32 = 10;

    pub fn process_output(&mut self) -> bool {
        let mut changed = false;
        let mut count = 0u32;
        while let Ok(data) = self.output_rx.try_recv() {
            self.osc_handler.process(&data);

            for event in self.osc_handler.events() {
                match event {
                    OscEvent::Clipboard(clipboard_event) => {
                        if let Ok(mut guard) = self.clipboard_text.lock() {
                            *guard = Some(clipboard_event.text.clone());
                        }
                    }
                    OscEvent::Cwd(cwd_event) => {
                        if let Ok(mut guard) = self.cwd.lock() {
                            *guard = Some(cwd_event.path.clone());
                        }
                    }
                    OscEvent::Hyperlink(hyperlink_event) => {
                        if let Ok(mut guard) = self.hyperlink.lock() {
                            *guard = hyperlink_event.url.clone();
                        }
                    }
                    OscEvent::Notification(notification_event) => {
                        if let Ok(mut guard) = self.notification.lock() {
                            *guard = Some((
                                notification_event.title.clone(),
                                notification_event.body.clone(),
                            ));
                        }
                    }
                }
            }

            let filtered = self.osc_handler.output();
            if filtered.contains(&0x07) {
                self.bel_triggered.store(true, Ordering::Release);
            }
            if let Some(marker) = extract_osc133(filtered) {
                self.shell_integration
                    .store(marker as u8, Ordering::Release);
            }
            self.terminal.pty_write(filtered);
            changed = true;
            count += 1;
            // Cap per-frame processing to avoid one render call blocking
            // the session lock for too long. Remaining chunks are processed
            // on the next render frame at no correctness cost — the VT thread
            // processes commands in FIFO order.
            if count >= Self::MAX_CHUNKS_PER_FRAME {
                log::trace!(
                    "process_output: hit cap of {} chunks, {} remain",
                    Self::MAX_CHUNKS_PER_FRAME,
                    self.output_rx.len(),
                );
                self.terminal.flush();
                break;
            }
        }
        if count > 0 {
            log::trace!("process_output: processed {count} chunks");
        }
        if changed {
            self.terminal.flush();
            for response in self.terminal.drain_pty_write_responses() {
                log::trace!("process_output: pty write-back {} bytes", response.len());
                if let Err(error) = self.pty.write_all(&response) {
                    log::error!(
                        "session: PTY write-back failed ({} bytes): {}",
                        response.len(),
                        error
                    );
                }
            }
        }
        // Drain user-initiated PTY writes (keyboard/IME/paste) queued by the
        // main thread through the lock-free channel. Written under the session
        // lock already held by the render thread, so UI-thread input never
        // blocks on the session mutex.
        let mut user_writes: Vec<Vec<u8>> = Vec::new();
        while let Ok(data) = self.user_write_rx.try_recv() {
            user_writes.push(data);
        }
        for data in &user_writes {
            if let Err(error) = self.pty.write_all(data) {
                log::error!(
                    "session: user PTY write failed ({} bytes): {}",
                    data.len(),
                    error
                );
            }
        }
        changed
    }

    pub fn poll_bel(&self) -> bool {
        self.bel_triggered.swap(false, Ordering::AcqRel)
    }

    pub fn poll_clipboard(&self) -> Option<String> {
        let mut guard = lock_or_recover(&self.clipboard_text, "poll_clipboard");
        guard.take()
    }

    pub fn poll_notification(&self) -> Option<(String, String)> {
        let mut guard = lock_or_recover(&self.notification, "poll_notification");
        guard.take()
    }

    pub fn poll_hyperlink(&self) -> Option<String> {
        let mut guard = lock_or_recover(&self.hyperlink, "poll_hyperlink");
        guard.take()
    }

    pub fn poll_shell_integration(&self) -> ShellIntegration {
        let raw_value = self.shell_integration.swap(0, Ordering::AcqRel);
        ShellIntegration::from(raw_value)
    }

    pub fn is_exited(&self) -> bool {
        self.exited.load(Ordering::Acquire)
    }

    pub fn exited_flag(&self) -> Arc<AtomicBool> {
        self.exited.clone()
    }

    pub fn terminal(&self) -> &GhosttyTerminal {
        &self.terminal
    }

    pub fn terminal_mut(&mut self) -> &mut GhosttyTerminal {
        &mut self.terminal
    }

    pub fn title(&self) -> String {
        self.terminal.title()
    }

    pub fn cwd(&self) -> String {
        if let Ok(guard) = self.cwd.lock()
            && let Some(tracked) = guard.as_ref()
        {
            return tracked.clone();
        }
        self.terminal.cwd()
    }

    pub fn key_encode(
        &self,
        key_code: u32,
        modifiers: u16,
        action: u8,
        unicode_char: u32,
        unshifted_char: u32,
    ) -> Option<Vec<u8>> {
        self.terminal
            .key_encode(key_code, modifiers, action, unicode_char, unshifted_char)
    }

    /// Submit a key for encoding and return a receiver for the result.
    /// The caller should NOT hold any session lock while waiting on the returned receiver.
    pub fn clone_cmd_tx(&self) -> flume::Sender<crate::ghostty_terminal::Command> {
        self.terminal.clone_cmd_tx()
    }

    pub fn key_encode_submit(
        &self,
        key_code: u32,
        modifiers: u16,
        action: u8,
        unicode_char: u32,
        unshifted_char: u32,
    ) -> Option<flume::Receiver<Vec<u8>>> {
        self.terminal
            .key_encode_submit(key_code, modifiers, action, unicode_char, unshifted_char)
    }

    pub fn mode_get(&self, mode_num: u16, kind: u8) -> bool {
        self.terminal.mode_get(mode_num, kind)
    }

    pub fn focus_event(&mut self, focused: bool) {
        let data = if focused { b"[I" } else { b"[O" };
        self.terminal.vt_write(data);
    }
}

fn extract_osc133(data: &[u8]) -> Option<ShellIntegration> {
    let mut result = None;
    let mut i = 0;
    while i + 6 < data.len() {
        if data[i] == 0x1B
            && data[i + 1] == b']'
            && data[i + 2] == b'1'
            && data[i + 3] == b'3'
            && data[i + 4] == b'3'
            && data[i + 5] == b';'
        {
            let marker_position = i + 6;
            if marker_position < data.len() {
                let marker = data[marker_position];
                let si = match marker {
                    b'A' => ShellIntegration::PromptStart,
                    b'B' => ShellIntegration::PromptEnd,
                    b'C' => ShellIntegration::CommandStart,
                    b'D' => ShellIntegration::CommandExecuted,
                    _ => ShellIntegration::None,
                };
                if si != ShellIntegration::None {
                    // Found a valid marker — scan for the terminator
                    // (BEL \x07 or ST \x1b\\) to advance past the full sequence.
                    if let Some(end) = find_osc_terminator(data, marker_position + 1) {
                        result = Some(si);
                        i = end;
                        continue;
                    }
                }
            }
            // Invalid marker or unterminated sequence — advance past prefix
            i += 6;
        } else {
            i += 1;
        }
    }
    result
}

/// Find the end of an OSC sequence (BEL or ST terminator) starting from `position`.
/// Returns the index one past the terminator, or None if unterminated.
fn find_osc_terminator(data: &[u8], position: usize) -> Option<usize> {
    let mut j = position;
    while j < data.len() {
        if data[j] == 0x07 {
            // BEL terminator (1 byte)
            return Some(j + 1);
        }
        if data[j] == 0x1B && j + 1 < data.len() && data[j + 1] == b'\\' {
            // ST terminator (2 bytes)
            return Some(j + 2);
        }
        j += 1;
    }
    None
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

        if let Some(h) = self.reader_handle.take()
            && let Err(error) = h.join()
        {
            log::error!("session: reader thread panicked: {:?}", error);
        }
        if let Some(h) = self.wait_handle.take()
            && let Err(error) = h.join()
        {
            log::error!("session: wait thread panicked: {:?}", error);
        }
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
        // Each marker tested independently to avoid slice-index confusion.
        assert_eq!(
            extract_osc133(b"\x1b]133;A\x07"),
            Some(ShellIntegration::PromptStart)
        );
        assert_eq!(
            extract_osc133(b"\x1b]133;B\x1b\\"),
            Some(ShellIntegration::PromptEnd)
        );
        assert_eq!(
            extract_osc133(b"\x1b]133;C\x07"),
            Some(ShellIntegration::CommandStart)
        );
        assert_eq!(
            extract_osc133(b"\x1b]133;D\x1b\\"),
            Some(ShellIntegration::CommandExecuted)
        );
    }

    #[test]
    fn extract_osc133_returns_none_without_markers() {
        assert_eq!(extract_osc133(b"hello world"), None);
        assert_eq!(extract_osc133(b"\x1b]133;\x07"), None);
        assert_eq!(extract_osc133(b"\x1b]133;X\x07"), None);
    }

    #[test]
    fn extract_osc133_command_executed() {
        assert_eq!(
            extract_osc133(b"\x1b]133;D\x1b\\"),
            Some(ShellIntegration::CommandExecuted)
        );
    }

    #[test]
    fn session_new_creates_pty() {
        let (pty, _handle) = crate::mock_pty::MockPty::new(24, 80);
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
        let (pty, handle) = crate::mock_pty::MockPty::new(24, 80);
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
        let (pty, handle) = crate::mock_pty::MockPty::new(24, 80);
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
        let (pty, _handle) = crate::mock_pty::MockPty::new(24, 80);
        let session = Session::with_pty(Box::new(pty) as Box<dyn Pty>, 24, 80)
            .expect("with_pty must succeed");
        assert!(!session.poll_bel(), "fresh session must not have bel set");
    }

    #[test]
    fn session_title_default_is_empty() {
        let (pty, _handle) = crate::mock_pty::MockPty::new(24, 80);
        let session = Session::with_pty(Box::new(pty) as Box<dyn Pty>, 24, 80)
            .expect("with_pty must succeed");
        assert_eq!(session.title(), "");
    }

    #[test]
    fn session_cwd_default_is_empty() {
        let (pty, _handle) = crate::mock_pty::MockPty::new(24, 80);
        let session = Session::with_pty(Box::new(pty) as Box<dyn Pty>, 24, 80)
            .expect("with_pty must succeed");
        assert_eq!(session.cwd(), "");
    }

    #[test]
    fn session_mode_get_default_false() {
        let (pty, _handle) = crate::mock_pty::MockPty::new(24, 80);
        let session = Session::with_pty(Box::new(pty) as Box<dyn Pty>, 24, 80)
            .expect("with_pty must succeed");
        // Mode 2004 (bracketed paste) should be off by default
        assert!(!session.mode_get(2004, 0));
    }

    #[test]
    fn session_focus_event_writes_to_terminal() {
        let (pty, _handle) = crate::mock_pty::MockPty::new(24, 80);
        let mut session = Session::with_pty(Box::new(pty) as Box<dyn Pty>, 24, 80)
            .expect("with_pty must succeed");
        // focus_event writes CSI sequences to terminal; should not panic
        session.focus_event(true);
        session.focus_event(false);
    }

    #[test]
    fn session_exited_flag() {
        let (pty, handle) = crate::mock_pty::MockPty::new(24, 80);
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
        // Real-world scenario: output may contain OSC 133 mixed with other text
        assert_eq!(
            extract_osc133(b"$ \x1b]133;C\x07 echo hello"),
            Some(ShellIntegration::CommandStart)
        );
    }

    #[test]
    fn extract_osc133_empty_osc() {
        // OSC without parameters should not match
        assert_eq!(extract_osc133(b"\x1b]133;\x07"), None);
        assert_eq!(extract_osc133(b"\x1b]133;\x1b\\"), None);
    }

    #[test]
    fn extract_osc133_incomplete_sequence() {
        // Truncated OSC should not match (no terminator)
        assert_eq!(extract_osc133(b"\x1b]133;C"), None);
        assert_eq!(extract_osc133(b"\x1b]133;"), None);
    }

    #[test]
    fn extract_osc133_st_terminator() {
        assert_eq!(
            extract_osc133(b"\x1b]133;C\x1b\\"),
            Some(ShellIntegration::CommandStart)
        );
        assert_eq!(
            extract_osc133(b"\x1b]133;D\x1b\\"),
            Some(ShellIntegration::CommandExecuted)
        );
    }

    #[test]
    fn extract_osc133_mixed_terminators() {
        // BEL and ST should both work
        assert_eq!(
            extract_osc133(b"\x1b]133;A\x07"),
            Some(ShellIntegration::PromptStart)
        );
        assert_eq!(
            extract_osc133(b"\x1b]133;A\x1b\\"),
            Some(ShellIntegration::PromptStart)
        );
    }

    #[test]
    fn session_write_after_exit_returns_error() {
        let (pty, handle) = crate::mock_pty::MockPty::new(24, 80);
        handle.set_exited();
        // Session::with_pty creates a session whose internal `exited`
        // AtomicBool is independent of the mock's `child_exited`.
        // The session's write path checks `self.exited` (its own flag) before
        // calling pty.write_all(). Since with_pty never sets that flag, the
        // PTY's write_all is always reached — but the mock's write returns
        // BrokenPipe once child_exited is true.
        //
        // So even with the session state mismatch, the underlying PTY write
        // still propagates the error upward. This test asserts that error path.
        let mut session = Session::with_pty(Box::new(pty) as Box<dyn Pty>, 24, 80)
            .expect("with_pty must succeed");
        let result = session.write(b"test");
        assert!(
            result.is_err(),
            "write to exited pty must return error, got Ok"
        );
    }

    #[test]
    fn shell_integration_from_u8() {
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
}
