//! PTY 输出预处理：字节透传到 VT 引擎，仅拦截上游明确不支持的
//! OSC 52 读取请求（`ESC ] 52 ; <selection> ; ?`，上游直接忽略，FR-036）。
//!
//! OSC 7/8/52 写入一律直达 Ghostty：工作目录、剪贴板写入、超链接状态
//! 以 Ghostty 为单一来源（上游回调与查询 API），本仓不再自建 OSC 解析器。

use std::sync::atomic::{AtomicBool, Ordering};

/// Process() 返回的单块快照：透传字节与读取请求。
#[derive(Debug, Default)]
pub struct OutputSnapshot {
    /// 透传给 VT 引擎的字节（读取请求序列已剥离）。
    pub filtered: Vec<u8>,
    /// OSC 52 读取请求的选择器名，无请求为 None。
    /// 同一块内多个请求为 last-wins：上游忽略读取且应用侧读取极低频，不设队列。
    pub clipboard_read: Option<String>,
}

/// PTY 输出预处理器：透传 + 读取请求扫描 + new_output 标志。
pub struct OutputProcessor {
    scan: ReadScan,
    /// P1-1 `new_output` flag (dual-flag protocol, see
    /// docs/reference/dual-flag-protocol.md): set when a non-empty PTY
    /// chunk is ingested ([`Self::process`]), read-and-cleared by the
    /// render thread via [`Self::take_new_output`]. Independent from the
    /// P2-1 `dirty` flag: new output may reset the viewport to the
    /// bottom; dirty (selection/highlight/font-size changes) must only
    /// trigger a repaint, never a scroll reset.
    new_output: AtomicBool,
}

impl Default for OutputProcessor {
    fn default() -> Self {
        Self::new()
    }
}

impl OutputProcessor {
    pub fn new() -> Self {
        Self {
            scan: ReadScan::default(),
            new_output: AtomicBool::new(false),
        }
    }

    /// Take and clear the `new_output` flag (single-consumer read-clear;
    /// the render thread is the only reader). Returns true if any PTY
    /// output was ingested since the last take.
    pub fn take_new_output(&self) -> bool {
        self.new_output.swap(false, Ordering::AcqRel)
    }

    /// 处理一块 PTY 输出：全部字节透传，仅剥离 OSC 52 读取请求。
    pub fn process(&mut self, data: &[u8]) -> OutputSnapshot {
        // P1-1: PTY ingest → raise `new_output` (bypass flag, not a queued
        // event — see docs/reference/dual-flag-protocol.md). Empty chunks
        // carry no output and do not count.
        if !data.is_empty() {
            self.new_output.store(true, Ordering::Release);
        }
        let mut snapshot = OutputSnapshot::default();
        snapshot.filtered.reserve(data.len());
        for &byte in data {
            self.scan.step(byte, &mut snapshot);
        }
        snapshot
    }
}

/// OSC 52 读取请求扫描器：只识别 `ESC ] 5 2 ; <selection> ; ?`（BEL 或 ST
/// 结束），命中时吞掉整段序列并报告选择器；其余字节原样透传。
/// 跨 process() 调用保持状态，以覆盖分块到达的序列。
#[derive(Debug, Default)]
struct ReadScan {
    state: ReadState,
    /// 已暂存、尚未定性的字节：命中则丢弃，否则原样吐出。
    buf: Vec<u8>,
}

#[derive(Debug, Default, PartialEq, Eq)]
enum ReadState {
    #[default]
    Ground,
    Esc,
    EscBracket,
    Num5,
    Num2,
    Selection,
    Question,
    QuestionEnd,
    QuestionSt,
}

/// 扫描暂存上限：合法读取请求远小于此值，超限直接透传（防病态输入积压）。
const MAX_SCAN_BYTES: usize = 64;

impl ReadScan {
    fn step(&mut self, byte: u8, snapshot: &mut OutputSnapshot) {
        match self.state {
            ReadState::Ground => {
                if byte == 0x1B {
                    self.buf.push(byte);
                    self.state = ReadState::Esc;
                } else {
                    snapshot.filtered.push(byte);
                }
            }
            ReadState::Esc => {
                self.buf.push(byte);
                if byte == b']' {
                    self.state = ReadState::EscBracket;
                } else {
                    // 不是 OSC：暂存（含本字节）整体吐出。
                    self.drain(&mut snapshot.filtered);
                }
            }
            ReadState::EscBracket => {
                self.buf.push(byte);
                if byte == b'5' {
                    self.state = ReadState::Num5;
                } else {
                    self.drain(&mut snapshot.filtered);
                }
            }
            ReadState::Num5 => {
                self.buf.push(byte);
                if byte == b'2' {
                    self.state = ReadState::Num2;
                } else {
                    self.drain(&mut snapshot.filtered);
                }
            }
            ReadState::Num2 => {
                self.buf.push(byte);
                if byte == b';' {
                    self.state = ReadState::Selection;
                } else {
                    self.drain(&mut snapshot.filtered);
                }
            }
            ReadState::Selection => {
                if byte == b';' {
                    self.buf.push(byte);
                    self.state = ReadState::Question;
                } else if byte == 0x07 || byte == 0x1B || self.buf.len() >= MAX_SCAN_BYTES {
                    // BEL/ESC 说明这不是读取请求（读取请求形如 `52;<sel>;?`）：
                    // 吐出暂存，本字节按 Ground 语义处理（ESC 开启新扫描）。
                    self.drain(&mut snapshot.filtered);
                    if byte == 0x1B {
                        self.buf.push(byte);
                        self.state = ReadState::Esc;
                    } else {
                        snapshot.filtered.push(byte);
                    }
                } else {
                    self.buf.push(byte);
                }
            }
            ReadState::Question => {
                // `?` 在此出现才是读取请求标志；其他字节说明是写入负载。
                self.buf.push(byte);
                if byte == b'?' {
                    self.state = ReadState::QuestionEnd;
                } else {
                    self.drain(&mut snapshot.filtered);
                }
            }
            ReadState::QuestionEnd => {
                if byte == 0x07 {
                    self.emit(snapshot);
                } else if byte == 0x1B {
                    self.buf.push(byte);
                    self.state = ReadState::QuestionSt;
                } else {
                    // `?` 后还有负载（如 `??`）：不是读取请求，原样透传。
                    self.buf.push(byte);
                    self.drain(&mut snapshot.filtered);
                }
            }
            ReadState::QuestionSt => {
                if byte == b'\\' {
                    self.emit(snapshot);
                } else {
                    self.drain(&mut snapshot.filtered);
                    if byte == 0x1B {
                        self.buf.push(byte);
                        self.state = ReadState::Esc;
                    } else {
                        snapshot.filtered.push(byte);
                    }
                }
            }
        }
    }

    /// 吐出全部暂存字节并回 Ground。
    fn drain(&mut self, out: &mut Vec<u8>) {
        out.extend_from_slice(&self.buf);
        self.buf.clear();
        self.state = ReadState::Ground;
    }

    /// 命中读取请求：解析选择器并报告，吞掉整段序列。
    fn emit(&mut self, snapshot: &mut OutputSnapshot) {
        // buf 形如 ESC ] 5 2 ; <sel> ; ? [ESC]：取两个 `;` 之间的选择器。
        let mut semis = self
            .buf
            .iter()
            .enumerate()
            .filter_map(|(i, &b)| if b == b';' { Some(i) } else { None });
        if let (Some(first), Some(second)) = (semis.next(), semis.next()) {
            let selection = String::from_utf8_lossy(&self.buf[first + 1..second]).into_owned();
            snapshot.clipboard_read = Some(selection);
        }
        self.buf.clear();
        self.state = ReadState::Ground;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    #[test]
    fn pty_write_raises_new_output_flag() {
        let mut proc = OutputProcessor::new();
        // Idle before any ingest: flag must be clear.
        assert!(!proc.take_new_output());
        // PTY write → flag true.
        let _ = proc.process(b"echo hi\r\n");
        assert!(proc.take_new_output());
        // Single-consumer read-clear: a second take sees false.
        assert!(!proc.take_new_output());
    }

    #[test]
    fn idle_keeps_new_output_flag_clear() {
        let mut proc = OutputProcessor::new();
        let _ = proc.process(b"first chunk");
        assert!(proc.take_new_output());
        // No further PTY writes: the flag stays clear across takes.
        assert!(!proc.take_new_output());
        assert!(!proc.take_new_output());
    }

    #[test]
    fn empty_chunk_does_not_raise_new_output_flag() {
        let mut proc = OutputProcessor::new();
        let _ = proc.process(b"");
        assert!(!proc.take_new_output());
    }

    #[test]
    fn default_matches_new_idle_behavior() {
        let mut proc = OutputProcessor::default();
        assert!(!proc.take_new_output());
        let snapshot = proc.process(b"");
        assert!(snapshot.filtered.is_empty());
        assert!(!proc.take_new_output());
    }

    #[test]
    fn osc52_read_request_stripped_and_reported() {
        let mut proc = OutputProcessor::new();
        let snap = proc.process(b"\x1b]52;c;?\x07");
        assert_eq!(snap.clipboard_read.as_deref(), Some("c"));
        assert!(
            snap.filtered.is_empty(),
            "read request must not reach the VT parser"
        );
    }

    #[test]
    fn osc52_read_request_st_terminator() {
        let mut proc = OutputProcessor::new();
        let snap = proc.process(b"\x1b]52;p;?\x1b\\");
        assert_eq!(snap.clipboard_read.as_deref(), Some("p"));
        assert!(snap.filtered.is_empty());
    }

    #[test]
    fn osc52_read_request_split_across_chunks() {
        let mut proc = OutputProcessor::new();
        let snap = proc.process(b"\x1b]52;c;");
        assert!(snap.clipboard_read.is_none());
        assert!(snap.filtered.is_empty());
        let snap = proc.process(b"?\x07");
        assert_eq!(snap.clipboard_read.as_deref(), Some("c"));
        assert!(snap.filtered.is_empty());
    }

    #[test]
    fn osc52_write_passes_through() {
        let mut proc = OutputProcessor::new();
        let input = b"\x1b]52;c;SGVsbG8=\x07";
        let snap = proc.process(input);
        assert!(snap.clipboard_read.is_none());
        assert_eq!(snap.filtered, input);
    }

    #[test]
    fn osc7_and_osc8_pass_through() {
        // 工作目录与超链接直达 Ghostty（上游回调/查询处理），不再剥离。
        let mut proc = OutputProcessor::new();
        let input = b"\x1b]7;file:///home/user\x07ab\x1b]8;;https://example.com\x07cd";
        let snap = proc.process(input);
        assert!(snap.clipboard_read.is_none());
        assert_eq!(snap.filtered, input);
    }

    #[test]
    fn question_with_payload_is_not_a_read() {
        // `?` 后还有负载就不是读取请求，原样透传（上游忽略）。
        let mut proc = OutputProcessor::new();
        let input = b"\x1b]52;c;?abc\x07";
        let snap = proc.process(input);
        assert!(snap.clipboard_read.is_none());
        assert_eq!(snap.filtered, input);
    }

    #[test]
    fn short_form_without_selection_passes_through() {
        let mut proc = OutputProcessor::new();
        let input = b"\x1b]52;?\x07";
        let snap = proc.process(input);
        assert!(snap.clipboard_read.is_none());
        assert_eq!(snap.filtered, input);
    }

    #[test]
    fn mixed_text_and_read_request() {
        let mut proc = OutputProcessor::new();
        let snap = proc.process(b"before\x1b]52;c;?\x07after");
        assert_eq!(snap.filtered, b"beforeafter");
        assert_eq!(snap.clipboard_read.as_deref(), Some("c"));
    }

    #[test]
    fn sgr_sequence_untouched() {
        // 非 OSC 转义（SGR/CSI）在 Esc 状态即吐出，不被吞字节。
        let mut proc = OutputProcessor::new();
        let input = b"\x1b[31mred\x1b[0m";
        let snap = proc.process(input);
        assert!(snap.clipboard_read.is_none());
        assert_eq!(snap.filtered, input);
    }

    proptest! {
        /// 无 ESC 的任意可打印文本必须原样透传且无读取事件（扫描器不误伤普通输出）。
        #[test]
        fn printable_text_passthrough_identity(text in "\\PC*") {
            let mut proc = OutputProcessor::new();
            let snap = proc.process(text.as_bytes());
            prop_assert_eq!(snap.filtered, text.as_bytes());
            prop_assert!(snap.clipboard_read.is_none());
        }
    }
}
