//! Functional in-memory `SessionStore` backend for testing and demos.
//!
//! Unlike `NoOpStore` (which rejects everything), `MockStore` actually records
//! input into a scrollback buffer and answers reads, so the full MCP protocol
//! and Unix-socket transport can be exercised end-to-end — including on the
//! Android emulator, where the PTY-based `live` backend cannot yet be built.
//!
//! # Requirements
//! - FR-044 — Run an MCP server over a Unix domain socket with JSON-RPC 2.0
//! - FR-045 — Expose tools for listing sessions, reading grid state, scrollback, cursor, selected text
//! - FR-046 — Expose tools for writing to the PTY, sending signals, resizing terminal, clipboard

use std::sync::Mutex;

use crate::{
    DirEntry, GridCellData, GridSnapshotData, ReadRequest, ReadResponse, SearchMatch, SessionInfo,
    SessionStore, SignalKind,
};

const MOCK_SESSION_ID: u32 = 1;
const MOCK_ROWS: u32 = 24;
const MOCK_COLS: u32 = 80;

/// An in-memory terminal session used to drive the MCP server without a GUI
/// or a real PTY.
pub struct MockStore {
    scrollback: Mutex<Vec<String>>,
    clipboard: Mutex<String>,
    notification: Mutex<Option<(String, String)>>,
    scroll_offset: Mutex<i32>,
}

impl Default for MockStore {
    fn default() -> Self {
        Self::new()
    }
}

impl MockStore {
    pub fn new() -> Self {
        Self {
            scrollback: Mutex::new(vec!["$ ".to_string()]),
            clipboard: Mutex::new(String::new()),
            notification: Mutex::new(None),
            scroll_offset: Mutex::new(0),
        }
    }

    fn push_lines(&self, data: &[u8]) {
        let text = String::from_utf8_lossy(data);
        let mut sb = self.scrollback.lock().unwrap();
        for line in text.split('\n') {
            let line = line.trim_end_matches('\r');
            // Simulate a shell echoing the typed command followed by a fresh
            // prompt line, so read_scrollback round-trips the typed text.
            sb.push(line.to_string());
            sb.push("$ ".to_string());
        }
        const MAX: usize = 2000;
        if sb.len() > MAX {
            let drop = sb.len() - MAX;
            sb.drain(0..drop);
        }
    }

    fn tail(&self, max_lines: usize) -> Vec<String> {
        let sb = self.scrollback.lock().unwrap();
        let n = max_lines.min(sb.len());
        sb[sb.len() - n..].to_vec()
    }
}

impl SessionStore for MockStore {
    fn read(&self, req: ReadRequest) -> Result<ReadResponse, String> {
        match req {
            ReadRequest::Sessions => Ok(ReadResponse::Sessions(vec![SessionInfo {
                id: MOCK_SESSION_ID,
                title: "mock".to_string(),
                rows: MOCK_ROWS,
                cols: MOCK_COLS,
                shell: String::new(),
                pid: None,
                is_exited: false,
            }])),
            ReadRequest::Grid { session_id: _ } => {
                let lines = self.tail(MOCK_ROWS as usize);
                let cols = MOCK_COLS as usize;
                let mut cells = Vec::new();
                for (row, line) in lines.iter().enumerate() {
                    for (col, ch) in line.chars().take(cols).enumerate() {
                        cells.push(GridCellData {
                            row: row as u32,
                            col: col as u32,
                            codepoint: ch as u32,
                            fg_r: 220,
                            fg_g: 220,
                            fg_b: 220,
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
                        });
                    }
                }
                Ok(ReadResponse::Grid(GridSnapshotData {
                    rows: MOCK_ROWS,
                    cols: MOCK_COLS,
                    cells,
                    cursor_row: lines.len() as u32,
                    cursor_col: 0,
                    cursor_visible: true,
                }))
            }
            ReadRequest::Scrollback {
                session_id: _,
                max_lines,
            } => Ok(ReadResponse::Scrollback(self.tail(max_lines as usize))),
            ReadRequest::Cursor { session_id: _ } => Ok(ReadResponse::Cursor {
                row: 0,
                col: 0,
                visible: true,
            }),
            ReadRequest::Selection { session_id: _ } => Ok(ReadResponse::Selection(None)),
            ReadRequest::Title { session_id: _ } => Ok(ReadResponse::Title("mock".to_string())),
            ReadRequest::ScrollbackSearch {
                session_id: _,
                pattern,
                max_matches,
            } => {
                let sb = self.scrollback.lock().unwrap();
                let mut matches = Vec::new();
                for (line_number, line) in sb.iter().enumerate() {
                    if let Some(start) = line.find(&pattern) {
                        matches.push(SearchMatch {
                            line_number: line_number as u32,
                            text: line.clone(),
                            start_col: start as u32,
                            end_col: (start + pattern.len()) as u32,
                        });
                        if matches.len() >= max_matches as usize {
                            break;
                        }
                    }
                }
                Ok(ReadResponse::SearchMatches(matches))
            }
            ReadRequest::TerminalSize { session_id: _ } => Ok(ReadResponse::TerminalSize {
                rows: MOCK_ROWS,
                cols: MOCK_COLS,
            }),
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
                self.clipboard.lock().unwrap().clone(),
            )),
        }
    }

    fn write(&self, _session_id: u32, data: Vec<u8>) -> Result<(), String> {
        self.push_lines(&data);
        Ok(())
    }

    fn signal(&self, _session_id: u32, _signal_kind: SignalKind) -> Result<(), String> {
        Ok(())
    }

    fn set_terminal_size(&self, _session_id: u32, _rows: u32, _cols: u32) -> Result<(), String> {
        Ok(())
    }

    fn write_clipboard(&self, text: &str) -> Result<(), String> {
        *self.clipboard.lock().unwrap() = text.to_string();
        Ok(())
    }

    fn read_clipboard(&self) -> Result<String, String> {
        Ok(self.clipboard.lock().unwrap().clone())
    }

    fn raise_notification(&self, title: &str, body: &str) -> Result<(), String> {
        *self.notification.lock().unwrap() = Some((title.to_string(), body.to_string()));
        Ok(())
    }

    fn scroll_terminal(&self, _session_id: u32, lines: i32) -> Result<i32, String> {
        let mut offset = self.scroll_offset.lock().unwrap();
        *offset += lines;
        Ok(*offset)
    }

    fn feed_terminal_output(&self, _session_id: u32, text: &str) -> Result<(), String> {
        self.push_lines(text.as_bytes());
        Ok(())
    }

    fn read_scrollback_tail(
        &self,
        _session_id: u32,
        max_lines: usize,
    ) -> Result<Vec<String>, String> {
        Ok(self.tail(max_lines))
    }
}
