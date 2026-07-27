//! OSC sequence interceptor that strips handled OSC sequences from terminal
//! output before it reaches the VT emulator. Inspired by Haven's OscHandler.
//!
//! # Requirements
//! - [FR-033](crate) — Scrollback: terminal content
//! - [FR-034](crate) — Clipboard: set/get bridge
//! - [FR-035](crate) — Clipboard: OSC 52 set
//! - [FR-036](crate) — Clipboard: OSC 52 get
//! - [FR-037](crate) — Filesystem: cwd reporting
//! - [FR-038](crate) — Filesystem: readable working directory
//! - [FR-040](crate) — Notification: iTerm2 OSC 9
//! - [FR-041](crate) — Notification: OSC 777
//! - [NFR-023](crate) — Clipboard: UTF-8 safe, protocol-agnostic
//!
//! Handled OSC types (stripped from output and emitted as [`OscEvent`]s):
///    7 — current working directory (`OscEvent::Cwd`)
///    8 — hyperlinks (open/close) (`OscEvent::Hyperlink`)
///    9 — notifications (iTerm2-style) (`OscEvent::Notification`)
///   52 — clipboard set (base64 decode) (`OscEvent::Clipboard`)
///  777 — notifications (rxvt-unicode-style) (`OscEvent::Notification`)
///
/// Unrecognised OSC numbers (0, 1, 4, 10, etc.) pass through unchanged so
/// the terminal emulator can handle them (e.g. OSC 0 sets the title).
///
/// Handles partial sequences across buffer boundaries. Invalid sequences
/// flush accumulated bytes to output. Payload capped at [MAX_PAYLOAD_BYTES].
const MAX_PAYLOAD_BYTES: usize = 1_048_576;

const MAX_SEQUENCE_OVERHEAD: usize = 64;

/// OSC numbers we handle (strip from output).
const HANDLED_OSC: &[u32] = &[
    OSC_CWD,
    OSC_HYPERLINK,
    OSC_NOTIFICATION_ITERM2,
    OSC_CLIPBOARD,
    OSC_NOTIFICATION_RXVT,
];

const OSC_CLIPBOARD: u32 = 52;
const OSC_CWD: u32 = 7;
const OSC_HYPERLINK: u32 = 8;
const OSC_NOTIFICATION_ITERM2: u32 = 9;
const OSC_NOTIFICATION_RXVT: u32 = 777;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum OscState {
    Ground,
    Esc,
    OscBracket,
    OscNumber,
    Payload,
    PassThrough,
    StEsc,
    PtStEsc,
}

/// Decoded OSC 52 clipboard event.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClipboardEvent {
    pub selection: String,
    pub text: String,
}

/// Decoded OSC 7 CWD event.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CwdEvent {
    pub path: String,
}

/// Decoded OSC 8 hyperlink event.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HyperlinkEvent {
    pub url: Option<String>,
}

/// Decoded OSC 9/777 notification event.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NotificationEvent {
    pub title: String,
    pub body: String,
}

/// Events decoded by the OSC handler.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OscEvent {
    Clipboard(ClipboardEvent),
    Cwd(CwdEvent),
    Hyperlink(HyperlinkEvent),
    Notification(NotificationEvent),
}

/// OSC sequence interceptor that processes terminal output bytes and strips
/// handled OSC sequences. Produces a filtered byte stream and decoded events.
///
/// Uses reusable internal buffers to avoid per-call allocations. Callers
/// borrow the filtered output via [`OscHandler::output`] after [`OscHandler::process`].
#[derive(Debug)]
pub struct OscHandler {
    /// Current state machine state.
    state: OscState,
    /// OSC number being parsed (e.g., 7, 8, 52).
    osc_number: u32,
    /// Accumulated sequence bytes.
    seq_buf: Vec<u8>,
    /// Payload accumulator for the current sequence.
    payload_buf: Vec<u8>,
    /// Output buffer (bytes that pass through unhandled).
    output_buf: Vec<u8>,
    /// Decoded events from handled OSC sequences.
    events_buf: Vec<OscEvent>,
}

impl OscHandler {
    pub fn new() -> Self {
        Self {
            state: OscState::Ground,
            osc_number: 0,
            seq_buf: Vec::with_capacity(64),
            payload_buf: Vec::with_capacity(1024),
            output_buf: Vec::with_capacity(4096),
            events_buf: Vec::new(),
        }
    }

    /// Process a chunk of terminal output. Handled OSC sequences are consumed;
    /// everything else is written to the internal output buffer. After calling
    /// this method, read the filtered bytes from [`OscHandler::output`] and decoded events
    /// from [`OscHandler::events`].
    pub fn process(&mut self, input: &[u8]) {
        let needed = input.len() + MAX_SEQUENCE_OVERHEAD;
        if self.output_buf.len() < needed {
            self.output_buf.resize(needed, 0);
        }
        self.output_buf.clear();
        self.events_buf.clear();

        for &byte in input {
            let value = byte as u32;
            match self.state {
                OscState::Ground => {
                    if value == 0x1B {
                        self.state = OscState::Esc;
                        self.seq_buf.clear();
                        self.seq_buf.push(byte);
                    } else {
                        self.output_buf.push(byte);
                    }
                }

                OscState::Esc => {
                    self.seq_buf.push(byte);
                    if value == b']' as u32 {
                        self.state = OscState::OscBracket;
                        self.osc_number = 0;
                    } else {
                        self.flush_seq_buf();
                    }
                }

                OscState::OscBracket => {
                    self.seq_buf.push(byte);
                    if byte.is_ascii_digit() {
                        self.osc_number = value - b'0' as u32;
                        self.state = OscState::OscNumber;
                    } else {
                        self.flush_seq_buf();
                    }
                }

                OscState::OscNumber => {
                    self.seq_buf.push(byte);
                    if byte.is_ascii_digit() {
                        self.osc_number = self.osc_number * 10 + (value - b'0' as u32);
                    } else if value == b';' as u32 {
                        if HANDLED_OSC.contains(&self.osc_number) {
                            self.seq_buf.clear();
                            self.payload_buf.clear();
                            self.state = OscState::Payload;
                        } else {
                            self.flush_seq_buf();
                            self.state = OscState::PassThrough;
                        }
                    } else {
                        self.flush_seq_buf();
                    }
                }

                OscState::Payload => match value {
                    0x07 => {
                        if let Some(event) = self.dispatch_osc() {
                            self.events_buf.push(event);
                        }
                        self.state = OscState::Ground;
                    }
                    0x1B => {
                        self.state = OscState::StEsc;
                    }
                    _ => {
                        if self.payload_buf.len() < MAX_PAYLOAD_BYTES {
                            self.payload_buf.push(byte);
                        }
                    }
                },

                OscState::StEsc => {
                    if value == b'\\' as u32 {
                        if let Some(event) = self.dispatch_osc() {
                            self.events_buf.push(event);
                        }
                        self.state = OscState::Ground;
                    } else {
                        self.ensure_output_capacity(self.seq_buf.len() + self.payload_buf.len());
                        self.flush_all();
                        if value == 0x1B {
                            self.state = OscState::Esc;
                            self.seq_buf.clear();
                            self.seq_buf.push(byte);
                        } else {
                            self.output_buf.push(byte);
                        }
                    }
                }

                OscState::PassThrough => match value {
                    0x07 => {
                        self.output_buf.push(byte);
                        self.state = OscState::Ground;
                    }
                    0x1B => {
                        self.state = OscState::PtStEsc;
                    }
                    _ => {
                        self.output_buf.push(byte);
                    }
                },

                OscState::PtStEsc => {
                    if value == b'\\' as u32 {
                        self.output_buf.push(0x1B);
                        self.output_buf.push(byte);
                        self.state = OscState::Ground;
                    } else {
                        self.output_buf.push(0x1B);
                        if value == 0x1B {
                            // Another ESC - stay in PtStEsc
                        } else {
                            self.output_buf.push(byte);
                            self.state = OscState::PassThrough;
                        }
                    }
                }
            }
        }
    }

    pub fn output(&self) -> &[u8] {
        &self.output_buf
    }

    pub fn events(&self) -> &[OscEvent] {
        &self.events_buf
    }

    fn ensure_output_capacity(&mut self, additional: usize) {
        let needed = self.output_buf.len() + additional;
        if self.output_buf.capacity() < needed {
            self.output_buf.resize(needed, 0);
        }
    }

    fn flush_seq_buf(&mut self) {
        self.output_buf.extend_from_slice(&self.seq_buf);
        self.seq_buf.clear();
        self.state = OscState::Ground;
    }

    fn flush_all(&mut self) {
        self.output_buf.extend_from_slice(&self.seq_buf);
        self.seq_buf.clear();
        self.output_buf.extend_from_slice(&self.payload_buf);
        self.payload_buf.clear();
        self.state = OscState::Ground;
    }

    fn dispatch_osc(&mut self) -> Option<OscEvent> {
        let payload = String::from_utf8_lossy(&self.payload_buf).to_string();
        self.payload_buf.clear();
        self.seq_buf.clear();

        match self.osc_number {
            OSC_CLIPBOARD => self.dispatch_osc52(&payload),
            OSC_CWD => self.dispatch_osc7(&payload),
            OSC_HYPERLINK => self.dispatch_osc8(&payload),
            OSC_NOTIFICATION_ITERM2 => self.dispatch_osc9(&payload),
            OSC_NOTIFICATION_RXVT => self.dispatch_osc777(&payload),
            _ => None,
        }
    }

    fn dispatch_osc52(&self, payload: &str) -> Option<OscEvent> {
        let semi = payload.find(';')?;
        let base64_data = &payload[semi + 1..];
        if base64_data.is_empty() {
            return None;
        }
        use base64::Engine;
        let decoded = base64::engine::general_purpose::STANDARD
            .decode(base64_data.as_bytes())
            .ok()?;
        let text = String::from_utf8_lossy(&decoded).to_string();
        let selection = payload[..semi].to_string();
        Some(OscEvent::Clipboard(ClipboardEvent { selection, text }))
    }

    fn dispatch_osc7(&self, payload: &str) -> Option<OscEvent> {
        if payload.is_empty() {
            return None;
        }
        Some(OscEvent::Cwd(CwdEvent {
            path: payload.to_string(),
        }))
    }

    fn dispatch_osc8(&self, payload: &str) -> Option<OscEvent> {
        let semi = payload.find(';')?;
        let url = &payload[semi + 1..];
        let url_opt = if url.is_empty() {
            None
        } else {
            Some(url.to_string())
        };
        Some(OscEvent::Hyperlink(HyperlinkEvent { url: url_opt }))
    }

    fn dispatch_osc9(&self, payload: &str) -> Option<OscEvent> {
        if payload.is_empty() {
            return None;
        }
        // OSC 9 format: \x1b]9;body\x07 or \x1b]9;title;body\x07
        let (title, body) = if let Some(semi) = payload.find(';') {
            let t = &payload[..semi];
            let b = &payload[semi + 1..];
            (t.to_string(), b.to_string())
        } else {
            (String::new(), payload.to_string())
        };
        Some(OscEvent::Notification(NotificationEvent { title, body }))
    }

    fn dispatch_osc777(&self, payload: &str) -> Option<OscEvent> {
        let parts: Vec<&str> = payload.splitn(3, ';').collect();
        if parts.len() < 3 || parts[0] != "notify" {
            return None;
        }
        Some(OscEvent::Notification(NotificationEvent {
            title: parts[1].to_string(),
            body: parts[2].to_string(),
        }))
    }
}

impl Default for OscHandler {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn passthrough_plain_text() {
        let mut handler = OscHandler::new();
        handler.process(b"hello world");
        assert_eq!(handler.output(), b"hello world");
        assert!(handler.events().is_empty());
    }

    #[test]
    fn strip_osc52_clipboard() {
        let mut handler = OscHandler::new();
        handler.process(b"\x1b]52;c;SGVsbG8=\x07");
        assert!(handler.output().is_empty());
        assert_eq!(handler.events().len(), 1);
        match &handler.events()[0] {
            OscEvent::Clipboard(ce) => assert_eq!(ce.text, "Hello"),
            _ => panic!("expected clipboard event"),
        }
    }

    #[test]
    fn strip_osc7_cwd() {
        let mut handler = OscHandler::new();
        handler.process(b"\x1b]7;file:///home/user\x07");
        assert!(handler.output().is_empty());
        assert_eq!(handler.events().len(), 1);
        match &handler.events()[0] {
            OscEvent::Cwd(cwd) => assert_eq!(cwd.path, "file:///home/user"),
            _ => panic!("expected cwd event"),
        }
    }

    #[test]
    fn strip_osc8_hyperlink_open() {
        let mut handler = OscHandler::new();
        handler.process(b"\x1b]8;id=link1;https://example.com\x07");
        assert!(handler.output().is_empty());
        assert_eq!(handler.events().len(), 1);
        match &handler.events()[0] {
            OscEvent::Hyperlink(h) => assert_eq!(h.url.as_deref(), Some("https://example.com")),
            _ => panic!("expected hyperlink event"),
        }
    }

    #[test]
    fn strip_osc8_hyperlink_close() {
        let mut handler = OscHandler::new();
        handler.process(b"\x1b]8;;\x07");
        assert!(handler.output().is_empty());
        assert_eq!(handler.events().len(), 1);
        match &handler.events()[0] {
            OscEvent::Hyperlink(h) => assert_eq!(h.url, None),
            _ => panic!("expected hyperlink close event"),
        }
    }

    #[test]
    fn passthrough_osc0_title() {
        let mut handler = OscHandler::new();
        handler.process(b"\x1b]0;My Terminal\x07");
        assert_eq!(handler.output(), b"\x1b]0;My Terminal\x07");
    }

    #[test]
    fn strip_osc9_notification() {
        let mut handler = OscHandler::new();
        handler.process(b"\x1b]9;Test notification\x07");
        assert!(handler.output().is_empty());
        assert_eq!(handler.events().len(), 1);
        match &handler.events()[0] {
            OscEvent::Notification(n) => assert_eq!(n.body, "Test notification"),
            _ => panic!("expected notification event"),
        }
    }

    #[test]
    fn strip_osc777_notification() {
        let mut handler = OscHandler::new();
        handler.process(b"\x1b]777;notify;Title;Body text\x07");
        assert!(handler.output().is_empty());
        assert_eq!(handler.events().len(), 1);
        match &handler.events()[0] {
            OscEvent::Notification(n) => {
                assert_eq!(n.title, "Title");
                assert_eq!(n.body, "Body text");
            }
            _ => panic!("expected notification event"),
        }
    }

    #[test]
    fn st_terminator() {
        let mut handler = OscHandler::new();
        handler.process(b"\x1b]52;c;SGVsbG8=\x1b\\");
        assert!(handler.output().is_empty());
        assert_eq!(handler.events().len(), 1);
    }

    #[test]
    fn partial_sequence_across_chunks() {
        let mut handler = OscHandler::new();
        handler.process(b"\x1b]52;c;");
        assert!(handler.output().is_empty());
        handler.process(b"SGVsbG8=\x07");
        assert!(handler.output().is_empty());
        assert_eq!(handler.events().len(), 1);
        match &handler.events()[0] {
            OscEvent::Clipboard(ce) => assert_eq!(ce.text, "Hello"),
            _ => panic!("expected clipboard event"),
        }
    }

    #[test]
    fn mixed_text_and_osc() {
        let mut handler = OscHandler::new();
        handler.process(b"before\x1b]52;c;SGVsbG8=\x07after");
        assert_eq!(handler.output(), b"beforeafter");
        assert_eq!(handler.events().len(), 1);
    }

    #[test]
    fn payload_too_large() {
        let mut handler = OscHandler::new();
        let mut input = b"\x1b]52;c;".to_vec();
        input.extend(std::iter::repeat_n(b'A', MAX_PAYLOAD_BYTES + 1));
        input.extend_from_slice(b"\x07");
        handler.process(&input);
        assert!(
            handler.output().is_empty() || !handler.output().contains(&b'A'),
            "oversized payload should be stripped from output"
        );
        assert!(handler.events().is_empty());
    }

    #[test]
    fn reuse_handler_multiple_chunks() {
        let mut handler = OscHandler::new();
        handler.process(b"first\x1b]52;c;Zmlyc3Q=\x07");
        assert_eq!(handler.output(), b"first");
        assert_eq!(handler.events().len(), 1);

        handler.process(b"second\x1b]52;c;dGVzdA==\x07");
        assert_eq!(handler.output(), b"second");
        assert_eq!(handler.events().len(), 1);
        match &handler.events()[0] {
            OscEvent::Clipboard(ce) => assert_eq!(ce.text, "test"),
            _ => panic!("expected clipboard event"),
        }
    }

    #[test]
    fn bel_in_handled_osc_not_in_output() {
        let mut handler = OscHandler::new();
        handler.process(b"\x1b]52;c;dGVzdA==\x07");
        let output = handler.output();
        assert!(
            !output.contains(&0x07),
            "BEL used as OSC terminator must not appear in output"
        );
    }

    #[test]
    fn standalone_bel_in_output() {
        let mut handler = OscHandler::new();
        handler.process(b"hello\x07world");
        let output = handler.output();
        assert!(
            output.contains(&0x07),
            "standalone BEL must be passed through in output"
        );
    }

    #[test]
    fn bel_in_unrecognized_osc_passes_through() {
        let mut handler = OscHandler::new();
        handler.process(b"\x1b]4;1;rgb:00/00/00\x07");
        let output = handler.output();
        assert!(
            output.contains(&0x07),
            "BEL in unrecognized OSC must pass through to VT engine"
        );
    }

    // ── R1: OSC 7 (cwd) is a handled OSC ───────────────────────────

    /// OSC 7 must be registered in `HANDLED_OSC` so the handler strips
    /// it and emits `OscEvent::Cwd` (R1 fix). If `OSC_CWD` were
    /// missing from the handled set, OSC 7 would be forwarded to Ghostty
    /// and `OscEvent::Cwd` would never be produced.
    #[test]
    fn handled_osc_includes_cwd() {
        assert_eq!(OSC_CWD, 7, "OSC_CWD constant must be 7");
        assert!(
            HANDLED_OSC.contains(&OSC_CWD),
            "OSC 7 (cwd) must be in HANDLED_OSC so it is intercepted"
        );
    }

    /// `OscHandler::process` for an OSC 7 sequence must emit
    /// `OscEvent::Cwd` carrying the path payload (R1 fix).
    #[test]
    fn osc7_process_emits_cwd_event_with_path() {
        let mut handler = OscHandler::new();
        handler.process(b"\x1b]7;file:///home/user/project\x07");
        assert!(
            handler.output().is_empty(),
            "OSC 7 body must be stripped from output"
        );
        assert_eq!(handler.events().len(), 1, "exactly one event expected");
        match &handler.events()[0] {
            OscEvent::Cwd(cwd) => {
                assert_eq!(cwd.path, "file:///home/user/project");
            }
            other => panic!("expected OscEvent::Cwd, got {other:?}"),
        }
    }
}
