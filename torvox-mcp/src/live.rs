//! Real `SessionStore` backend built on `torvox-terminal`.
//!
//! This powers end-to-end MCP testing against a genuine PTY-backed shell, both
//! on the host and inside the Android emulator. It is gated behind the `live`
//! cargo feature so the default build of `torvox-mcp` stays dependency-light
//! (no `torvox-terminal` / ghostty) and CI stays fast.
//!
//! # Requirements
//! - FR-044 — Run an MCP server over a Unix domain socket with JSON-RPC 2.0
//! - FR-045 — Expose tools for listing sessions, reading grid state, scrollback, cursor, selected text
//! - FR-046 — Expose tools for writing to the PTY, sending signals, resizing terminal, clipboard

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use crate::{
    DirEntry, GridCellData, GridSnapshotData, ReadRequest, ReadResponse, SearchMatch, SessionInfo,
    SessionStore, SignalKind,
};
use torvox_terminal::ShellEnv;
use torvox_terminal::session::Session;

/// Maximum rows we walk when collecting scrollback text from the VT thread.
const SCROLLBACK_READ_LIMIT: u32 = 5000;

fn lock_or_recover<'a, T>(
    mutex: &'a std::sync::Mutex<T>,
    context: &str,
) -> std::sync::MutexGuard<'a, T> {
    match mutex.lock() {
        Ok(guard) => guard,
        Err(poisoned) => {
            log::warn!("{context}: mutex poisoned, recovered");
            poisoned.into_inner()
        }
    }
}

fn color_to_u8(value: f32) -> u8 {
    (value.clamp(0.0, 1.0) * 255.0).round() as u8
}

/// A live session store backed by real `torvox-terminal::Session` instances.
pub struct LiveShellStore {
    sessions: Arc<Mutex<HashMap<u32, Arc<Mutex<Session>>>>>,
    next_id: Arc<Mutex<u32>>,
    clipboard: Arc<Mutex<String>>,
    notification: Arc<Mutex<Option<(String, String)>>>,
    scroll_offset: Arc<Mutex<i32>>,
}

impl LiveShellStore {
    /// Create an empty live store with no sessions.
    pub fn new() -> Self {
        Self {
            sessions: Arc::new(Mutex::new(HashMap::new())),
            next_id: Arc::new(Mutex::new(1)),
            clipboard: Arc::new(Mutex::new(String::new())),
            notification: Arc::new(Mutex::new(None)),
            scroll_offset: Arc::new(Mutex::new(0)),
        }
    }

    /// Spawn a real shell session and register it, returning its session id.
    pub fn spawn_session(&self, shell: &str, rows: u32, cols: u32) -> u32 {
        let session = Session::spawn(shell, rows, cols, &ShellEnv::default())
            .expect("failed to spawn live shell session");
        let id = {
            let mut next = lock_or_recover(&self.next_id, "spawn_session::next_id");
            let id = *next;
            *next += 1;
            id
        };
        let arc = Arc::new(Mutex::new(session));
        // Pump the VT output into the terminal grid so read tools observe it.
        let pump = arc.clone();
        std::thread::spawn(move || {
            loop {
                let exited = {
                    let mut s = match pump.lock() {
                        Ok(s) => s,
                        Err(_) => break,
                    };
                    s.process_output();
                    s.is_exited()
                };
                if exited {
                    break;
                }
                std::thread::sleep(Duration::from_millis(10));
            }
        });
        lock_or_recover(&self.sessions, "spawn_session::sessions").insert(id, arc);
        id
    }

    fn lock_session(&self, session_id: u32) -> Result<Arc<Mutex<Session>>, String> {
        lock_or_recover(&self.sessions, "lock_session::sessions")
            .get(&session_id)
            .cloned()
            .ok_or_else(|| format!("session {session_id} not found"))
    }

    fn collect_lines(&self, session: &mut Session, max_lines: usize) -> Vec<String> {
        session.process_output();
        let term = session.terminal();
        let scrollback_len = term.scrollback_length();
        let rows = term.rows();
        let total = scrollback_len
            .saturating_add(rows)
            .min(SCROLLBACK_READ_LIMIT);
        let mut lines = Vec::new();
        for row in 0..total {
            if let Some(text) = term.read_line_text(row) {
                lines.push(text);
            }
        }
        let n = max_lines.min(lines.len());
        let start = lines.len() - n;
        lines[start..].to_vec()
    }
}

impl Default for LiveShellStore {
    fn default() -> Self {
        Self::new()
    }
}

impl SessionStore for LiveShellStore {
    fn read(&self, req: ReadRequest) -> Result<ReadResponse, String> {
        match req {
            ReadRequest::Sessions => {
                let sessions = lock_or_recover(&self.sessions, "read::sessions");
                let infos: Vec<SessionInfo> = sessions
                    .iter()
                    .map(|(id, arc)| {
                        let s = lock_or_recover(arc, "read::sessions::item");
                        SessionInfo {
                            id: *id,
                            title: s.title(),
                            rows: s.terminal().rows(),
                            cols: s.terminal().cols(),
                            shell: String::new(),
                            pid: None,
                            is_exited: s.is_exited(),
                        }
                    })
                    .collect();
                Ok(ReadResponse::Sessions(infos))
            }
            ReadRequest::Grid { session_id } => {
                let arc = self.lock_session(session_id)?;
                let mut s = lock_or_recover(&arc, "read::grid");
                s.process_output();
                let snap = s.terminal().take_snapshot();
                let cols = snap.cols.max(1);
                let cells: Vec<GridCellData> = snap
                    .cells
                    .iter()
                    .enumerate()
                    .map(|(idx, cell)| {
                        let row = (idx as u32 / cols) as u32;
                        let col = (idx as u32 % cols) as u32;
                        GridCellData {
                            row,
                            col,
                            codepoint: cell.codepoint,
                            fg_r: color_to_u8(cell.foreground[0]),
                            fg_g: color_to_u8(cell.foreground[1]),
                            fg_b: color_to_u8(cell.foreground[2]),
                            bg_r: color_to_u8(cell.background[0]),
                            bg_g: color_to_u8(cell.background[1]),
                            bg_b: color_to_u8(cell.background[2]),
                            bold: cell.bold,
                            italic: cell.italic,
                            underline: cell.underline,
                            reverse: cell.reverse,
                            dim: cell.dim,
                            strikethrough: cell.strikethrough,
                            blink: cell.blink,
                            hidden: cell.hidden,
                        }
                    })
                    .collect();
                Ok(ReadResponse::Grid(GridSnapshotData {
                    rows: snap.rows,
                    cols: snap.cols,
                    cells,
                    cursor_row: snap.cursor_row,
                    cursor_col: snap.cursor_col,
                    cursor_visible: snap.cursor_visible,
                }))
            }
            ReadRequest::Scrollback {
                session_id,
                max_lines,
            } => {
                let arc = self.lock_session(session_id)?;
                let mut s = lock_or_recover(&arc, "read::scrollback");
                let lines = self.collect_lines(&mut s, max_lines as usize);
                Ok(ReadResponse::Scrollback(lines))
            }
            ReadRequest::Cursor { session_id } => {
                let arc = self.lock_session(session_id)?;
                let mut s = lock_or_recover(&arc, "read::cursor");
                s.process_output();
                let term = s.terminal();
                Ok(ReadResponse::Cursor {
                    row: term.cursor_y(),
                    col: term.cursor_x(),
                    visible: term.cursor_visible(),
                })
            }
            ReadRequest::Selection { session_id } => {
                let _ = session_id;
                Ok(ReadResponse::Selection(None))
            }
            ReadRequest::Title { session_id } => {
                let arc = self.lock_session(session_id)?;
                let s = lock_or_recover(&arc, "read::title");
                Ok(ReadResponse::Title(s.title()))
            }
            ReadRequest::ScrollbackSearch {
                session_id,
                pattern,
                max_matches,
            } => {
                let arc = self.lock_session(session_id)?;
                let mut s = lock_or_recover(&arc, "read::scrollback_search");
                let lines = self.collect_lines(&mut s, SCROLLBACK_READ_LIMIT as usize);
                let mut matches = Vec::new();
                for (line_number, line) in lines.iter().enumerate() {
                    if let Some(start) = line.find(&pattern) {
                        let end = (start + pattern.len()) as u32;
                        matches.push(SearchMatch {
                            line_number: line_number as u32,
                            text: line.clone(),
                            start_col: start as u32,
                            end_col: end,
                        });
                        if matches.len() >= max_matches as usize {
                            break;
                        }
                    }
                }
                Ok(ReadResponse::SearchMatches(matches))
            }
            ReadRequest::TerminalSize { session_id } => {
                let arc = self.lock_session(session_id)?;
                let s = lock_or_recover(&arc, "read::terminal_size");
                let term = s.terminal();
                Ok(ReadResponse::TerminalSize {
                    rows: term.rows(),
                    cols: term.cols(),
                })
            }
            ReadRequest::ListDirectory { path } => {
                let mut entries = Vec::new();
                for entry in
                    std::fs::read_dir(&path).map_err(|e| format!("read_dir {path}: {e}"))?
                {
                    let entry = entry.map_err(|e| format!("dir entry: {e}"))?;
                    let meta = entry.metadata().map_err(|e| format!("metadata: {e}"))?;
                    entries.push(DirEntry {
                        name: entry.file_name().to_string_lossy().into_owned(),
                        is_dir: meta.is_dir(),
                        size: if meta.is_file() {
                            Some(meta.len())
                        } else {
                            None
                        },
                        modified: meta
                            .modified()
                            .ok()
                            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                            .map(|d| d.as_secs().to_string()),
                    });
                }
                Ok(ReadResponse::DirectoryEntries(entries))
            }
            ReadRequest::ReadFile { path, max_lines } => {
                let content =
                    std::fs::read_to_string(&path).map_err(|e| format!("read {path}: {e}"))?;
                let all_lines: Vec<&str> = content.lines().collect();
                let total = all_lines.len();
                let take = (max_lines as usize).min(total);
                let start = total - take;
                let lines: Vec<String> = all_lines[start..].iter().map(|s| s.to_string()).collect();
                Ok(ReadResponse::FileContent {
                    lines,
                    total_lines: total as u32,
                    truncated: take < total,
                })
            }
            ReadRequest::ReadClipboard => Ok(ReadResponse::ClipboardContent(
                lock_or_recover(&self.clipboard, "read::clipboard").clone(),
            )),
        }
    }

    fn write(&self, session_id: u32, data: Vec<u8>) -> Result<(), String> {
        let arc = self.lock_session(session_id)?;
        let mut s = lock_or_recover(&arc, "write");
        s.write(&data).map_err(|e| e.to_string())
    }

    fn signal(&self, session_id: u32, signal_kind: SignalKind) -> Result<(), String> {
        let signum: i32 = match signal_kind {
            SignalKind::Hangup => 1,
            SignalKind::Interrupt => 2,
            SignalKind::Quit => 3,
            SignalKind::Terminate => 15,
        };
        let arc = self.lock_session(session_id)?;
        let s = lock_or_recover(&arc, "signal");
        s.send_signal(signum).map_err(|e| e.to_string())
    }

    fn set_terminal_size(&self, session_id: u32, rows: u32, cols: u32) -> Result<(), String> {
        let arc = self.lock_session(session_id)?;
        let mut s = lock_or_recover(&arc, "set_terminal_size");
        s.resize(rows, cols).map_err(|e| e.to_string())
    }

    fn write_clipboard(&self, text: &str) -> Result<(), String> {
        *lock_or_recover(&self.clipboard, "write_clipboard") = text.to_string();
        Ok(())
    }

    fn read_clipboard(&self) -> Result<String, String> {
        Ok(lock_or_recover(&self.clipboard, "read_clipboard").clone())
    }

    fn raise_notification(&self, title: &str, body: &str) -> Result<(), String> {
        *lock_or_recover(&self.notification, "raise_notification") =
            Some((title.to_string(), body.to_string()));
        Ok(())
    }

    fn scroll_terminal(&self, _session_id: u32, lines: i32) -> Result<i32, String> {
        let mut offset = lock_or_recover(&self.scroll_offset, "scroll_terminal");
        *offset += lines;
        Ok(*offset)
    }

    fn feed_terminal_output(&self, session_id: u32, text: &str) -> Result<(), String> {
        self.write(session_id, text.as_bytes().to_vec())
    }

    fn read_scrollback_tail(
        &self,
        session_id: u32,
        max_lines: usize,
    ) -> Result<Vec<String>, String> {
        let arc = self.lock_session(session_id)?;
        let mut s = lock_or_recover(&arc, "read_scrollback_tail");
        let lines = self.collect_lines(&mut s, max_lines);
        Ok(lines)
    }
}
