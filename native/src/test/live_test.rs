use std::io::{BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::Arc;
use std::time::{Duration, Instant};

use base64::Engine;
use serde::{Deserialize, Serialize};

use crate::render::cpu_frame::{CpuFrame, TextItem};

// ---------------------------------------------------------------------------
// Serializable wrappers (avoids requiring Serialize/Deserialize on CpuFrame
// types — those live in production code outside this test module).
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SerTextItem {
    text: String,
    row: u32,
    col: u32,
    width: u32,
    height: u32,
}

impl From<&TextItem> for SerTextItem {
    fn from(item: &TextItem) -> Self {
        Self {
            text: item.text.clone(),
            row: item.row,
            col: item.col,
            width: item.width,
            height: item.height,
        }
    }
}

// ---------------------------------------------------------------------------
// Test commands
// ---------------------------------------------------------------------------

/// Commands that can be sent to the LiveTest server.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "cmd")]
pub enum TestCommand {
    /// Get visible text from the terminal frame.
    GetText {},
    /// Get cursor position and visibility.
    GetCursor {},
    /// Wait for specific text to appear in the terminal.
    WaitForText { text: String, timeout_ms: u64 },
    /// Wait for cursor to reach a specific position.
    WaitForCursor { row: u32, col: u32, timeout_ms: u64 },
    /// Take a screenshot (returns base64-encoded PNG).
    Screenshot {},
    /// Get the current terminal dimensions.
    GetSize {},
    /// Wait for a fixed duration.
    Wait { ms: u64 },
    /// Ping to check server is alive.
    Ping {},
}

// ---------------------------------------------------------------------------
// Test responses
// ---------------------------------------------------------------------------

/// Response from the LiveTest server.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "status")]
pub enum TestResponse {
    Ok {},
    Text {
        items: Vec<SerTextItem>,
    },
    Cursor {
        row: u32,
        col: u32,
        visible: bool,
    },
    Size {
        rows: u32,
        cols: u32,
    },
    Screenshot {
        png_base64: String,
        width: u32,
        height: u32,
    },
    Pong {},
    Error {
        message: String,
    },
}

// ---------------------------------------------------------------------------
// Provider callbacks
// ---------------------------------------------------------------------------

/// Callback type for frame provider.
pub type FrameProvider = Arc<dyn Fn() -> CpuFrame + Send + Sync>;

/// Callback type for cursor provider.
pub type CursorProvider = Arc<dyn Fn() -> (u32, u32, bool) + Send + Sync>;

/// Callback type for terminal size provider.
pub type SizeProvider = Arc<dyn Fn() -> (u32, u32) + Send + Sync>;

// ---------------------------------------------------------------------------
// Server
// ---------------------------------------------------------------------------

/// A LiveTest server that accepts TCP connections and processes test commands.
pub struct LiveTestServer {
    listener: TcpListener,
    frame_provider: FrameProvider,
    cursor_provider: CursorProvider,
    size_provider: SizeProvider,
    running: Arc<parking_lot::Mutex<bool>>,
}

impl LiveTestServer {
    /// Create a new LiveTest server bound to the given port.
    pub fn new(
        port: u16,
        frame_provider: FrameProvider,
        cursor_provider: CursorProvider,
        size_provider: SizeProvider,
    ) -> std::io::Result<Self> {
        let addr = format!("127.0.0.1:{port}");
        let listener = TcpListener::bind(addr)?;
        listener.set_nonblocking(true)?;

        Ok(Self {
            listener,
            frame_provider,
            cursor_provider,
            size_provider,
            running: Arc::new(parking_lot::Mutex::new(true)),
        })
    }

    /// Start accepting connections (blocking loop with poll interval).
    pub fn serve(&self) -> std::io::Result<()> {
        self.listener.set_nonblocking(true)?;
        while *self.running.lock() {
            match self.listener.accept() {
                Ok((stream, _addr)) => {
                    self.handle_connection(stream);
                }
                Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    // No pending connection — sleep briefly then re-check `running`.
                    std::thread::sleep(Duration::from_millis(10));
                    continue;
                }
                Err(e) => {
                    eprintln!("LiveTest: accept error: {e}");
                    break;
                }
            }
        }
        Ok(())
    }

    /// Stop the server.
    pub fn stop(&self) {
        *self.running.lock() = false;
    }

    /// Check if server is running.
    pub fn is_running(&self) -> bool {
        *self.running.lock()
    }

    fn handle_connection(&self, mut stream: TcpStream) {
        let reader_stream = stream.try_clone().unwrap();
        reader_stream
            .set_read_timeout(Some(Duration::from_millis(500)))
            .ok();
        let mut reader = BufReader::new(reader_stream);
        let mut line = String::new();

        loop {
            line.clear();
            match reader.read_line(&mut line) {
                Ok(0) => break, // Connection closed
                Ok(_) => {
                    let trimmed = line.trim();
                    if trimmed.is_empty() {
                        continue;
                    }

                    let response = self.handle_command(trimmed);
                    let response_json = serde_json::to_string(&response).unwrap_or_else(|e| {
                        serde_json::to_string(&TestResponse::Error {
                            message: format!("Serialization error: {e}"),
                        })
                        .unwrap()
                    });

                    let _ = writeln!(stream, "{response_json}");
                    let _ = stream.flush();
                }
                Err(e)
                    if e.kind() == std::io::ErrorKind::WouldBlock
                        || e.kind() == std::io::ErrorKind::TimedOut =>
                {
                    // Read timeout — re-check running flag.
                    if !*self.running.lock() {
                        break;
                    }
                    continue;
                }
                Err(e) => {
                    eprintln!("LiveTest: read error: {e}");
                    break;
                }
            }
        }
    }

    fn handle_command(&self, line: &str) -> TestResponse {
        let cmd: TestCommand = match serde_json::from_str(line) {
            Ok(cmd) => cmd,
            Err(e) => {
                return TestResponse::Error {
                    message: format!("Invalid command JSON: {e}"),
                };
            }
        };

        match cmd {
            TestCommand::Ping {} => TestResponse::Pong {},

            TestCommand::GetText {} => {
                let frame = (self.frame_provider)();
                let items = frame.extract_text_items();
                let ser_items: Vec<SerTextItem> = items.iter().map(SerTextItem::from).collect();
                TestResponse::Text { items: ser_items }
            }

            TestCommand::GetCursor {} => {
                let (row, col, visible) = (self.cursor_provider)();
                TestResponse::Cursor { row, col, visible }
            }

            TestCommand::WaitForText { text, timeout_ms } => {
                let deadline = Instant::now() + Duration::from_millis(timeout_ms);
                loop {
                    let frame = (self.frame_provider)();
                    let hits = frame.find_text(&text);
                    if !hits.is_empty() {
                        return TestResponse::Ok {};
                    }
                    if Instant::now() >= deadline {
                        return TestResponse::Error {
                            message: format!("Timeout waiting for text: '{text}'"),
                        };
                    }
                    std::thread::sleep(Duration::from_millis(50));
                }
            }

            TestCommand::WaitForCursor {
                row,
                col,
                timeout_ms,
            } => {
                let deadline = Instant::now() + Duration::from_millis(timeout_ms);
                loop {
                    let (r, c, _) = (self.cursor_provider)();
                    if r == row && c == col {
                        return TestResponse::Ok {};
                    }
                    if Instant::now() >= deadline {
                        return TestResponse::Error {
                            message: format!(
                                "Timeout waiting for cursor at ({row}, {col}), current at ({r}, {c})"
                            ),
                        };
                    }
                    std::thread::sleep(Duration::from_millis(50));
                }
            }

            TestCommand::Screenshot {} => {
                let frame = (self.frame_provider)();
                // Render frame to a simple PNG via the image crate (dev-dep).
                let png_bytes = encode_frame_png(&frame, 8, 16);
                let png_base64 = base64::engine::general_purpose::STANDARD.encode(&png_bytes);
                TestResponse::Screenshot {
                    png_base64,
                    width: frame.width * 8,
                    height: frame.height * 16,
                }
            }

            TestCommand::GetSize {} => {
                let (rows, cols) = (self.size_provider)();
                TestResponse::Size { rows, cols }
            }

            TestCommand::Wait { ms } => {
                std::thread::sleep(Duration::from_millis(ms));
                TestResponse::Ok {}
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Screenshot helper (CPU rasterisation → PNG via the `image` crate)
// ---------------------------------------------------------------------------

/// Encode a `CpuFrame` as PNG bytes using the `image` crate.
///
/// `cell_w` / `cell_h` are the pixel dimensions of each terminal cell.
fn encode_frame_png(frame: &CpuFrame, cell_w: u32, cell_h: u32) -> Vec<u8> {
    let img_w = frame.width * cell_w;
    let img_h = frame.height * cell_h;
    let mut img = image::RgbaImage::new(img_w, img_h);

    for row in 0..frame.height {
        for col in 0..frame.width {
            if let Some(cell) = frame.cell_at(row, col) {
                // Fill the cell background.
                let x0 = col * cell_w;
                let y0 = row * cell_h;
                for dy in 0..cell_h {
                    for dx in 0..cell_w {
                        img.put_pixel(x0 + dx, y0 + dy, image::Rgba(cell.bg));
                    }
                }
                // Draw a simple placeholder glyph (filled square) for non-space chars.
                if cell.codepoint != ' ' && cell.codepoint != '\0' {
                    let fg = image::Rgba(cell.fg);
                    let pad = cell_w / 4;
                    for dy in pad..(cell_h - pad) {
                        for dx in pad..(cell_w - pad) {
                            img.put_pixel(x0 + dx, y0 + dy, fg);
                        }
                    }
                }
            }
        }
    }

    let mut buf = std::io::Cursor::new(Vec::new());
    img.write_to(&mut buf, image::ImageFormat::Png)
        .expect("PNG encoding failed");
    buf.into_inner()
}

// ---------------------------------------------------------------------------
// Client
// ---------------------------------------------------------------------------

/// A test client for connecting to a LiveTest server.
pub struct LiveTestClient {
    stream: TcpStream,
}

impl LiveTestClient {
    pub fn connect(port: u16) -> std::io::Result<Self> {
        let stream = TcpStream::connect(format!("127.0.0.1:{port}"))?;
        stream.set_read_timeout(Some(Duration::from_secs(30)))?;
        stream.set_write_timeout(Some(Duration::from_secs(5)))?;
        Ok(Self { stream })
    }

    pub fn send_command(&mut self, cmd: &TestCommand) -> std::io::Result<TestResponse> {
        let json = serde_json::to_string(cmd)?;
        writeln!(self.stream, "{json}")?;
        self.stream.flush()?;

        let mut reader = BufReader::new(self.stream.try_clone()?);
        let mut response_line = String::new();
        reader.read_line(&mut response_line)?;

        let response: TestResponse = serde_json::from_str(response_line.trim())
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;

        Ok(response)
    }

    pub fn ping(&mut self) -> std::io::Result<bool> {
        match self.send_command(&TestCommand::Ping {})? {
            TestResponse::Pong {} => Ok(true),
            _ => Ok(false),
        }
    }

    pub fn get_text(&mut self) -> std::io::Result<Vec<TextItem>> {
        match self.send_command(&TestCommand::GetText {})? {
            TestResponse::Text { items } => Ok(items
                .into_iter()
                .map(|s| TextItem {
                    text: s.text,
                    row: s.row,
                    col: s.col,
                    width: s.width,
                    height: s.height,
                })
                .collect()),
            TestResponse::Error { message } => {
                Err(std::io::Error::new(std::io::ErrorKind::Other, message))
            }
            _ => Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "unexpected response type",
            )),
        }
    }

    pub fn wait_for_text(&mut self, text: &str, timeout_ms: u64) -> std::io::Result<()> {
        match self.send_command(&TestCommand::WaitForText {
            text: text.to_string(),
            timeout_ms,
        })? {
            TestResponse::Ok {} => Ok(()),
            TestResponse::Error { message } => {
                Err(std::io::Error::new(std::io::ErrorKind::TimedOut, message))
            }
            _ => Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "unexpected response type",
            )),
        }
    }

    pub fn wait_for_cursor(&mut self, row: u32, col: u32, timeout_ms: u64) -> std::io::Result<()> {
        match self.send_command(&TestCommand::WaitForCursor {
            row,
            col,
            timeout_ms,
        })? {
            TestResponse::Ok {} => Ok(()),
            TestResponse::Error { message } => {
                Err(std::io::Error::new(std::io::ErrorKind::TimedOut, message))
            }
            _ => Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "unexpected response type",
            )),
        }
    }

    pub fn get_size(&mut self) -> std::io::Result<(u32, u32)> {
        match self.send_command(&TestCommand::GetSize {})? {
            TestResponse::Size { rows, cols } => Ok((rows, cols)),
            TestResponse::Error { message } => {
                Err(std::io::Error::new(std::io::ErrorKind::Other, message))
            }
            _ => Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "unexpected response type",
            )),
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::render::cpu_frame::CpuFrame;
    use crate::terminal::ghostty_terminal::{CellData, CursorInfo, CursorStyle};
    use std::sync::{Arc, Mutex};
    use std::thread;

    fn make_simple_frame(text: &str) -> CpuFrame {
        let cols = text.len() as u32;
        let cells: Vec<CellData> = text
            .chars()
            .enumerate()
            .map(|(i, c)| CellData {
                codepoint: c as u32,
                width: 1,
                grapheme_extra: [0; 7],
                fg_color: [1.0, 1.0, 1.0, 1.0],
                bg_color: [0.0, 0.0, 0.0, 1.0],
                flags: 0,
                row: 0,
                col: i as u32,
            })
            .collect();
        let cursor = CursorInfo {
            row: 0,
            col: 0,
            visible: true,
            style: CursorStyle::Block,
            scrollback_length: 0,
        };
        CpuFrame::from_cell_data(&cells, 1, cols, &cursor)
    }

    fn start_test_server(
        port: u16,
        frame_text: Arc<Mutex<String>>,
    ) -> (LiveTestServer, Arc<parking_lot::Mutex<bool>>) {
        let frame_text_clone = frame_text.clone();
        let frame_provider: FrameProvider = Arc::new(move || {
            let text = frame_text_clone.lock().unwrap().clone();
            make_simple_frame(&text)
        });

        let cursor_provider: CursorProvider = Arc::new(|| (0, 0, true));
        let size_provider: SizeProvider = Arc::new(|| (1, 80));

        let server =
            LiveTestServer::new(port, frame_provider, cursor_provider, size_provider).unwrap();
        let running = server.running.clone();
        (server, running)
    }

    #[test]
    fn server_ping_pong() {
        let frame_text = Arc::new(Mutex::new("hello".to_string()));
        let (server, running) = start_test_server(19876, frame_text);

        let server_handle = thread::spawn(move || {
            let _ = server.serve();
        });

        thread::sleep(Duration::from_millis(100));

        let mut client = LiveTestClient::connect(19876).unwrap();
        assert!(client.ping().unwrap());

        *running.lock() = false;
        server_handle.join().ok();
    }

    #[test]
    fn client_get_text() {
        let frame_text = Arc::new(Mutex::new("Hi There".to_string()));
        let (server, running) = start_test_server(19877, frame_text);

        let server_handle = thread::spawn(move || {
            let _ = server.serve();
        });

        thread::sleep(Duration::from_millis(100));

        let mut client = LiveTestClient::connect(19877).unwrap();
        let items = client.get_text().unwrap();
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].text, "Hi There");

        *running.lock() = false;
        server_handle.join().ok();
    }

    #[test]
    fn client_wait_for_text() {
        let frame_text = Arc::new(Mutex::new("loading".to_string()));
        let (server, running) = start_test_server(19878, frame_text.clone());

        let server_handle = thread::spawn(move || {
            let _ = server.serve();
        });

        thread::sleep(Duration::from_millis(100));

        // Change text after a short delay.
        let ft = frame_text;
        thread::spawn(move || {
            thread::sleep(Duration::from_millis(100));
            *ft.lock().unwrap() = "done!".to_string();
        });

        let mut client = LiveTestClient::connect(19878).unwrap();
        let result = client.wait_for_text("done!", 2000);
        assert!(result.is_ok());

        *running.lock() = false;
        server_handle.join().ok();
    }

    #[test]
    fn server_get_cursor() {
        let frame_text = Arc::new(Mutex::new("test".to_string()));
        let (server, running) = start_test_server(19879, frame_text);

        let server_handle = thread::spawn(move || {
            let _ = server.serve();
        });

        thread::sleep(Duration::from_millis(100));

        let mut client = LiveTestClient::connect(19879).unwrap();
        match client.send_command(&TestCommand::GetCursor {}).unwrap() {
            TestResponse::Cursor { row, col, visible } => {
                assert_eq!(row, 0);
                assert_eq!(col, 0);
                assert!(visible);
            }
            other => panic!("Expected Cursor response, got {other:?}"),
        }

        *running.lock() = false;
        server_handle.join().ok();
    }

    #[test]
    fn server_get_size() {
        let frame_text = Arc::new(Mutex::new("test".to_string()));
        let (server, running) = start_test_server(19880, frame_text);

        let server_handle = thread::spawn(move || {
            let _ = server.serve();
        });

        thread::sleep(Duration::from_millis(100));

        let mut client = LiveTestClient::connect(19880).unwrap();
        let (rows, cols) = client.get_size().unwrap();
        assert_eq!(rows, 1);
        assert_eq!(cols, 80);

        *running.lock() = false;
        server_handle.join().ok();
    }

    #[test]
    fn invalid_json_returns_error() {
        let frame_text = Arc::new(Mutex::new("test".to_string()));
        let (server, running) = start_test_server(19881, frame_text);

        let server_handle = thread::spawn(move || {
            let _ = server.serve();
        });

        thread::sleep(Duration::from_millis(100));

        let mut stream = TcpStream::connect("127.0.0.1:19881").unwrap();
        writeln!(stream, "not json").unwrap();
        stream.flush().unwrap();

        let mut reader = BufReader::new(stream);
        let mut line = String::new();
        reader.read_line(&mut line).unwrap();

        let response: TestResponse = serde_json::from_str(line.trim()).unwrap();
        match response {
            TestResponse::Error { message } => {
                assert!(message.contains("Invalid command JSON"));
            }
            other => panic!("Expected Error, got {other:?}"),
        }

        *running.lock() = false;
        server_handle.join().ok();
    }

    #[test]
    fn wait_for_text_timeout() {
        let frame_text = Arc::new(Mutex::new("never".to_string()));
        let (server, running) = start_test_server(19882, frame_text);

        let server_handle = thread::spawn(move || {
            let _ = server.serve();
        });

        thread::sleep(Duration::from_millis(100));

        let mut client = LiveTestClient::connect(19882).unwrap();
        let result = client.wait_for_text("not_here", 200);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Timeout"));

        *running.lock() = false;
        server_handle.join().ok();
    }
}
