//! PTY 输出预处理：字节透传到 VT 引擎，仅拦截上游明确不支持的
//! OSC 52 读取请求（`ESC ] 52 ; <selection> ; ?`，上游直接忽略，FR-036）。
//!
//! OSC 7/8/52 写入一律直达 Ghostty：工作目录、剪贴板写入、超链接状态
//! 以 Ghostty 为单一来源（上游回调与查询 API），本仓不再自建 OSC 解析器。

use std::sync::atomic::{AtomicBool, Ordering};

#[derive(Debug, Default)]
pub struct OutputSnapshot {
    pub filtered: Vec<u8>,
    /// 本块内命中的 OSC 52 读取请求的选择器名（按序）。
    /// 单槽 last-wins 会让被挤掉的请求永不作答、远端挂起直到超时，
    /// 故保留全部；消费侧（会话队列 → 注册表上限）负责有界。
    pub clipboard_reads: Vec<String>,
}

pub struct OutputProcessor {
    scan: ReadScan,
    /// `new_output` 标志（见 docs/specification/REFERENCE.md）：[`Self::process`] 摄入
    /// 非空 PTY 块时置位，渲染线程经 [`Self::take_new_output`] 读取并清除。
    /// 与 `dirty` 标志相互独立：新输出可把视口复位到底部，而 dirty（选区/高亮/字号
    /// 变化）只能触发重绘，绝不滚动复位。
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

    pub fn take_new_output(&self) -> bool {
        self.new_output.swap(false, Ordering::AcqRel)
    }

    pub fn process(&mut self, data: &[u8]) -> OutputSnapshot {
        // PTY 摄入即置 `new_output`（旁路标志而非入队事件）。空块不算输出。
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
                } else if byte == 0x07 || byte == 0x1B {
                    self.drain(&mut snapshot.filtered);
                    if byte == 0x1B {
                        self.buf.push(byte);
                        self.state = ReadState::Esc;
                    } else {
                        snapshot.filtered.push(byte);
                    }
                } else if self.buf.len() >= MAX_SCAN_BYTES {
                    // 选择器名超上限：序列原样透传给上游，本仓不再应答。
                    // 必须出声——否则远端只看到「粘贴没反应」，无从定位。
                    log::warn!(
                        "osc52: selection name exceeds {MAX_SCAN_BYTES} bytes, read request passed through unanswered"
                    );
                    self.drain(&mut snapshot.filtered);
                    snapshot.filtered.push(byte);
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
            .filter_map(
                |(byte_index, &byte)| {
                    if byte == b';' { Some(byte_index) } else { None }
                },
            );
        if let (Some(first), Some(second)) = (semis.next(), semis.next()) {
            let selection = String::from_utf8_lossy(&self.buf[first + 1..second]).into_owned();
            snapshot.clipboard_reads.push(selection);
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
        let mut processor = OutputProcessor::new();
        // 摄入前空闲：标志须为清。
        assert!(!processor.take_new_output());
        // PTY 写入后标志为置。
        let _ = processor.process(b"echo hi\r\n");
        assert!(processor.take_new_output());
        // 单消费者读清：第二次读取为 false。
        assert!(!processor.take_new_output());
    }

    #[test]
    fn idle_keeps_new_output_flag_clear() {
        let mut processor = OutputProcessor::new();
        let _ = processor.process(b"first chunk");
        assert!(processor.take_new_output());
        // 此后无 PTY 写入：多次读取后标志仍为清。
        assert!(!processor.take_new_output());
        assert!(!processor.take_new_output());
    }

    #[test]
    fn empty_chunk_does_not_raise_new_output_flag() {
        let mut processor = OutputProcessor::new();
        let _ = processor.process(b"");
        assert!(!processor.take_new_output());
    }

    #[test]
    fn default_matches_new_idle_behavior() {
        let mut processor = OutputProcessor::default();
        assert!(!processor.take_new_output());
        let snapshot = processor.process(b"");
        assert!(snapshot.filtered.is_empty());
        assert!(!processor.take_new_output());
    }

    #[test]
    fn osc52_read_request_stripped_and_reported() {
        let mut processor = OutputProcessor::new();
        let snapshot = processor.process(b"\x1b]52;c;?\x07");
        assert_eq!(snapshot.clipboard_reads.as_slice(), ["c"]);
        assert!(
            snapshot.filtered.is_empty(),
            "read request must not reach the VT parser"
        );
    }

    #[test]
    fn osc52_read_request_st_terminator() {
        let mut processor = OutputProcessor::new();
        let snapshot = processor.process(b"\x1b]52;p;?\x1b\\");
        assert_eq!(snapshot.clipboard_reads.as_slice(), ["p"]);
        assert!(snapshot.filtered.is_empty());
    }

    /// 同块内多个读请求必须全部保留（按序），单槽 last-wins 会让
    /// 被挤掉的请求永不作答、远端挂起直到超时。
    #[test]
    fn osc52_multiple_read_requests_in_one_block_all_kept() {
        let mut processor = OutputProcessor::new();
        let snapshot = processor.process(b"\x1b]52;c;?\x07text\x1b]52;p;?\x07");
        assert_eq!(snapshot.clipboard_reads.as_slice(), ["c", "p"]);
        assert_eq!(snapshot.filtered, b"text");
    }

    #[test]
    fn osc52_read_request_split_across_chunks() {
        let mut processor = OutputProcessor::new();
        let snapshot = processor.process(b"\x1b]52;c;");
        assert!(snapshot.clipboard_reads.is_empty());
        assert!(snapshot.filtered.is_empty());
        let snapshot = processor.process(b"?\x07");
        assert_eq!(snapshot.clipboard_reads.as_slice(), ["c"]);
        assert!(snapshot.filtered.is_empty());
    }

    #[test]
    fn osc52_write_passes_through() {
        let mut processor = OutputProcessor::new();
        let input = b"\x1b]52;c;SGVsbG8=\x07";
        let snapshot = processor.process(input);
        assert!(snapshot.clipboard_reads.is_empty());
        assert_eq!(snapshot.filtered, input);
    }

    #[test]
    fn osc52_oversized_selection_passes_through_unanswered() {
        // 选择器名超上限：整段序列必须原样透传给上游（不吞字节），
        // 但本仓不产生读取请求——远端的粘贴得不到应答。
        let mut processor = OutputProcessor::new();
        let mut input = Vec::from(&b"\x1b]52;"[..]);
        input.extend(std::iter::repeat_n(b'x', MAX_SCAN_BYTES + 8));
        input.extend_from_slice(b";?\x07");
        let snapshot = processor.process(&input);
        assert!(snapshot.clipboard_reads.is_empty());
        assert_eq!(snapshot.filtered, input, "超限序列必须整段透传，不得丢字节");
    }

    #[test]
    fn osc7_and_osc8_pass_through() {
        // 工作目录与超链接直达 Ghostty（上游回调/查询处理），不再剥离。
        let mut processor = OutputProcessor::new();
        let input = b"\x1b]7;file:///home/user\x07ab\x1b]8;;https://example.com\x07cd";
        let snapshot = processor.process(input);
        assert!(snapshot.clipboard_reads.is_empty());
        assert_eq!(snapshot.filtered, input);
    }

    #[test]
    fn question_with_payload_is_not_a_read() {
        // `?` 后还有负载就不是读取请求，原样透传（上游忽略）。
        let mut processor = OutputProcessor::new();
        let input = b"\x1b]52;c;?abc\x07";
        let snapshot = processor.process(input);
        assert!(snapshot.clipboard_reads.is_empty());
        assert_eq!(snapshot.filtered, input);
    }

    #[test]
    fn short_form_without_selection_passes_through() {
        let mut processor = OutputProcessor::new();
        let input = b"\x1b]52;?\x07";
        let snapshot = processor.process(input);
        assert!(snapshot.clipboard_reads.is_empty());
        assert_eq!(snapshot.filtered, input);
    }

    #[test]
    fn mixed_text_and_read_request() {
        let mut processor = OutputProcessor::new();
        let snapshot = processor.process(b"before\x1b]52;c;?\x07after");
        assert_eq!(snapshot.filtered, b"beforeafter");
        assert_eq!(snapshot.clipboard_reads.as_slice(), ["c"]);
    }

    #[test]
    fn sgr_sequence_untouched() {
        // 非 OSC 转义（SGR/CSI）在 Esc 状态即吐出，不被吞字节。
        let mut processor = OutputProcessor::new();
        let input = b"\x1b[31mred\x1b[0m";
        let snapshot = processor.process(input);
        assert!(snapshot.clipboard_reads.is_empty());
        assert_eq!(snapshot.filtered, input);
    }

    proptest! {
        /// 无 ESC 的任意可打印文本必须原样透传且无读取事件（扫描器不误伤普通输出）。
        #[test]
        fn printable_text_passthrough_identity(text in "\\PC*") {
            let mut processor = OutputProcessor::new();
            let snapshot = processor.process(text.as_bytes());
            prop_assert_eq!(snapshot.filtered, text.as_bytes());
            prop_assert!(snapshot.clipboard_reads.is_empty());
        }
    }
}
