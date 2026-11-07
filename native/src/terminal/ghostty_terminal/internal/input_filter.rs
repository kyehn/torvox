//! VT 输入预处理：剔除上游未实现的序列，并代答 Kitty 图形协议的能力探测。
//!
//! 背景（真机 logcat 证据）：
//! - `kitten icat` 启动时按协议探测可用传输介质（direct / 临时文件 / 共享内存），
//!   探测用的查询命令会到达终端。本环境满足不了其中两项：Android 没有 POSIX
//!   共享内存（上游在 `readSharedMemory` 里直接拒绝）；客户端的临时文件写在
//!   子进程沙箱的私有 `/dev/shm` 里，终端进程看不见（`error.FileNotFound`）。
//!   上游把这类「客户端探测失败」记成
//!   `W/kitty_gfx erroneous kitty graphics response`。
//! - kitty 收尾时发 `CSI * x`（DECSACE 复位），上游未实现 DECCARA/DECSACE，
//!   记成 `W/stream unimplemented CSI action`。
//!
//! 本层按同一协议自己回答探测（错误应答与上游逐字节一致），并丢弃 DECSACE：
//! 终端行为不变（客户端同样回退到 direct 介质），日志不再被上游的协议探测噪声淹没。
//!
//! 两处都只在「本块内完整出现」时处理：序列跨块拆分时原样透传，交给上游的
//! 增量解析器续接——宁可漏掉一次过滤，也不冒破坏字节流的风险。
use std::borrow::Cow;

use base64::Engine;
use base64::engine::general_purpose::STANDARD_NO_PAD_INDIFFERENT as BASE64;

/// 介质能力：决定本层能否代答某介质的能力探测。
#[derive(Clone, Copy, Debug)]
pub(crate) struct MediaCapabilities {
    /// 平台是否支持 POSIX 共享内存（Kitty `t=s` 介质）。
    pub(crate) shared_memory: bool,
}

impl MediaCapabilities {
    /// 本平台的能力。
    ///
    /// Android 与 Windows 无 POSIX 共享内存（上游在 `readSharedMemory` 里对这两者
    /// 直接拒绝），其余 Unix 平台按 POSIX 语义可用。
    pub(crate) fn platform() -> Self {
        Self {
            shared_memory: !cfg!(any(target_os = "android", target_os = "windows")),
        }
    }
}

/// 预处理结果：透传给上游 VT 解析器的字节与需要写回 PTY 的协议应答。
pub(crate) struct FilteredInput<'a> {
    /// 透传给上游的字节；无序列被处理时直接借用输入，不额外拷贝。
    pub(crate) payload: Cow<'a, [u8]>,
    /// 需要写回 PTY 的应答（Kitty 错误应答）。
    pub(crate) responses: Vec<Vec<u8>>,
}

/// 过滤一帧 VT 输入字节。
///
/// 逐字节扫描：`ESC [` 的 CSI 序列在中间字节为 `*` 且终字节为 `x`（DECSACE）时丢弃；
/// `ESC _ G` 的 Kitty APC 若是本层代答的能力探测则丢弃并回代错误应答；其余
/// 字符串序列（OSC / DCS / APC / PM / SOS）整段透传，内部不做任何扫描。
pub(crate) fn filter_input(data: &[u8], capabilities: MediaCapabilities) -> FilteredInput<'_> {
    if !data.contains(&0x1b) {
        return FilteredInput {
            payload: Cow::Borrowed(data),
            responses: Vec::new(),
        };
    }
    let mut payload: Vec<u8> = Vec::with_capacity(data.len());
    let mut responses: Vec<Vec<u8>> = Vec::new();
    let mut index = 0usize;
    while index < data.len() {
        let byte = data[index];
        if byte != 0x1b {
            payload.push(byte);
            index += 1;
            continue;
        }
        match data.get(index + 1).copied() {
            None => {
                payload.push(byte);
                index += 1;
            }
            Some(b'[') => match parse_csi(data, index) {
                Some((end, is_decsace)) => {
                    if !is_decsace {
                        payload.extend_from_slice(&data[index..end]);
                    }
                    index = end;
                }
                None => {
                    payload.extend_from_slice(&data[index..]);
                    index = data.len();
                }
            },
            Some(b']') => match find_string_end(data, index + 2, true) {
                Some(end) => {
                    payload.extend_from_slice(&data[index..end]);
                    index = end;
                }
                None => {
                    payload.extend_from_slice(&data[index..]);
                    index = data.len();
                }
            },
            Some(b'_') if data.get(index + 2).copied() == Some(b'G') => {
                match find_kitty_apc(data, index) {
                    Some((content, end)) => {
                        match kitty_probe(&data[content.0..content.1], capabilities) {
                            KittyProbe::Pass => payload.extend_from_slice(&data[index..end]),
                            KittyProbe::Answer(response) => {
                                if let Some(response) = response {
                                    responses.push(response);
                                }
                            }
                        }
                        index = end;
                    }
                    None => {
                        payload.extend_from_slice(&data[index..]);
                        index = data.len();
                    }
                }
            }
            Some(b'_' | b'P' | b'^' | b'X') => match find_string_end(data, index + 2, false) {
                Some(end) => {
                    payload.extend_from_slice(&data[index..end]);
                    index = end;
                }
                None => {
                    payload.extend_from_slice(&data[index..]);
                    index = data.len();
                }
            },
            Some(_) => {
                payload.extend_from_slice(&data[index..index + 2]);
                index += 2;
            }
        }
    }
    FilteredInput {
        payload: Cow::Owned(payload),
        responses,
    }
}

/// 解析一帧内的 CSI 序列：`ESC [` + 参数字节 + 中间字节 + 终字节。
///
/// 返回（序列结束位置，是否为 DECSACE）；序列不完整或不合法时返回 `None`，
/// 由调用方原样透传。
fn parse_csi(data: &[u8], start: usize) -> Option<(usize, bool)> {
    let mut index = start + 2;
    while index < data.len() && matches!(data[index], 0x30..=0x3f) {
        index += 1;
    }
    let intermediates_start = index;
    while index < data.len() && matches!(data[index], 0x20..=0x2f) {
        index += 1;
    }
    let intermediates = &data[intermediates_start..index];
    let final_byte = data.get(index).copied()?;
    if !matches!(final_byte, 0x40..=0x7e) {
        return None;
    }
    // DECSACE：`CSI <参数> * x`，中间字节只有 `*`（kitty 的 `CSI 2*x` 与
    // `CSI *x` 都落在这里）。
    let is_decsace = final_byte == b'x' && intermediates == b"*";
    Some((index + 1, is_decsace))
}

/// 找到字符串序列（OSC / DCS / APC / PM / SOS）的结束位置。
///
/// `allow_bel` 为真时 BEL 也算结束（OSC 允许 BEL 终止，其余字符串序列只有 ST）。
fn find_string_end(data: &[u8], start: usize, allow_bel: bool) -> Option<usize> {
    let mut index = start;
    while index < data.len() {
        match data[index] {
            0x07 if allow_bel => return Some(index + 1),
            0x1b if data.get(index + 1).copied() == Some(0x5c) => return Some(index + 2),
            _ => index += 1,
        }
    }
    None
}

/// Kitty 图形协议的 APC 段：`ESC _ G <控制数据>;<载荷> ESC \`。
///
/// 返回（内容区间，整段结束位置）；不完整时返回 `None`。
fn find_kitty_apc(data: &[u8], start: usize) -> Option<((usize, usize), usize)> {
    let content_start = start + 3;
    let mut index = content_start;
    while index < data.len() {
        if data[index] == 0x1b && data.get(index + 1).copied() == Some(0x5c) {
            return Some(((content_start, index), index + 2));
        }
        index += 1;
    }
    None
}

/// Kitty 命令是否在本层代答。
enum KittyProbe {
    /// 不是代答范围：交回上游处理。
    Pass,
    /// 本层代答：`None` 表示按静默规则不回包。
    Answer(Option<Vec<u8>>),
}

/// 判定一帧 Kitty APC 是否由本层代答。
///
/// 只代答「本环境无法服务」的介质：直接介质（`t=d`）与能打开的文件介质交回上游
/// 处理；共享内存介质在本平台不可用时按上游的错误消息代答；文件介质打不开时按
/// 上游打开失败的错误消息代答（打开的判据与上游一致，都按进程工作目录解析）。
fn kitty_probe(content: &[u8], capabilities: MediaCapabilities) -> KittyProbe {
    let (control_bytes, payload_bytes) = match content.iter().position(|&byte| byte == b';') {
        Some(separator) => (&content[..separator], &content[separator + 1..]),
        None => (content, &content[content.len()..]),
    };
    let control = ControlData::parse(control_bytes);
    // 只有查询与传输两类命令会因介质不可用而失败；其余（显示、删除、动画等）
    // 本层不干预。
    if !matches!(control.action, Some(b'q') | Some(b't') | Some(b'T')) {
        return KittyProbe::Pass;
    }
    match control.medium {
        b'd' => KittyProbe::Pass,
        b's' => {
            if capabilities.shared_memory {
                KittyProbe::Pass
            } else {
                KittyProbe::Answer(control.error_response("EINVAL: unsupported medium"))
            }
        }
        b'f' | b't' => match decode_path(payload_bytes) {
            Some(path) if std::fs::File::open(&path).is_ok() => KittyProbe::Pass,
            _ => KittyProbe::Answer(control.error_response("EINVAL: invalid data")),
        },
        _ => KittyProbe::Pass,
    }
}

/// 解出文件介质载荷里的路径：载荷是 base64 编码的路径字节。
///
/// kitty 用无填充的标准 base64 传输（`base64.RawStdEncoding`），彻底等：`BASE64`
/// 同时接受带填充与不带填充的输入。
fn decode_path(payload: &[u8]) -> Option<String> {
    let decoded = BASE64.decode(payload).ok()?;
    String::from_utf8(decoded).ok()
}

/// Kitty 控制数据里本层关心的键。
struct ControlData {
    /// `a`：动作（`q` 查询 / `t` 传输 / `T` 传输并显示）。
    action: Option<u8>,
    /// `t`：传输介质（默认 `d` 直接）。
    medium: u8,
    /// `i`：图像 id。
    image_id: u32,
    /// `I`：图像编号。
    image_number: u32,
    /// `p`：放置 id。
    placement_id: u32,
    /// `q`：静默（0 不静默 / 1 静默成功应答 / 2 连失败应答也不回）。
    quiet: u32,
}

impl ControlData {
    fn parse(bytes: &[u8]) -> Self {
        let mut control = Self {
            action: None,
            medium: b'd',
            image_id: 0,
            image_number: 0,
            placement_id: 0,
            quiet: 0,
        };
        for item in bytes.split(|&byte| byte == b',') {
            let Some(separator) = item.iter().position(|&byte| byte == b'=') else {
                continue;
            };
            if separator == 0 {
                continue;
            }
            let key = item[separator - 1];
            let value = &item[separator + 1..];
            match key {
                b'a' => control.action = value.first().copied(),
                b't' => control.medium = value.first().copied().unwrap_or(b'd'),
                b'i' => control.image_id = parse_number(value),
                b'I' => control.image_number = parse_number(value),
                b'p' => control.placement_id = parse_number(value),
                b'q' => control.quiet = parse_number(value),
                _ => {}
            }
        }
        control
    }

    /// 按上游的应答格式生成错误应答；静默规则与上游一致。
    fn error_response(&self, message: &str) -> Option<Vec<u8>> {
        // `q=2`：连失败应答也不回。
        if self.quiet == 2 {
            return None;
        }
        // 上游在没有图像 id 与图像编号时压根不编码应答。
        if self.image_id == 0 && self.image_number == 0 {
            return None;
        }
        let mut response = Vec::with_capacity(message.len() + 24);
        response.extend_from_slice(b"\x1b_G");
        let mut separated = false;
        if self.image_id > 0 {
            response.extend_from_slice(b"i=");
            response.extend_from_slice(self.image_id.to_string().as_bytes());
            separated = true;
        }
        if self.image_number > 0 {
            if separated {
                response.push(b',');
            }
            response.extend_from_slice(b"I=");
            response.extend_from_slice(self.image_number.to_string().as_bytes());
            separated = true;
        }
        if self.placement_id > 0 {
            if separated {
                response.push(b',');
            }
            response.extend_from_slice(b"p=");
            response.extend_from_slice(self.placement_id.to_string().as_bytes());
        }
        response.push(b';');
        response.extend_from_slice(message.as_bytes());
        response.extend_from_slice(b"\x1b\\");
        Some(response)
    }
}

/// 无符号数值解析；非数字或空值按 0 取（与上游「解析失败按 0」的口径一致）。
fn parse_number(value: &[u8]) -> u32 {
    let mut number = 0u32;
    for &byte in value {
        if !byte.is_ascii_digit() {
            return 0;
        }
        number = number
            .saturating_mul(10)
            .saturating_add(u32::from(byte - b'0'));
    }
    number
}

#[cfg(test)]
mod tests {
    use super::*;

    fn filter(data: &[u8]) -> (Vec<u8>, Vec<Vec<u8>>) {
        let filtered = filter_input(
            data,
            MediaCapabilities {
                shared_memory: false,
            },
        );
        (filtered.payload.into_owned(), filtered.responses)
    }

    #[test]
    fn decape_reset_is_removed() {
        let (payload, responses) = filter(b"before\x1b[*xafter");
        assert_eq!(payload, b"beforeafter", "DECSACE 复位不得到达上游");
        assert!(responses.is_empty());
    }

    #[test]
    fn decape_region_select_is_removed() {
        let (payload, _) = filter(b"\x1b[2*xcontent");
        assert_eq!(payload, b"content", "DECSACE 区域选择同样未实现，必须丢弃");
    }

    #[test]
    fn other_csi_sequences_are_preserved() {
        let (payload, responses) = filter(b"\x1b[31mred\x1b[0m\x1b[2J\x1b[?25l");
        assert_eq!(payload, b"\x1b[31mred\x1b[0m\x1b[2J\x1b[?25l");
        assert!(responses.is_empty());
    }

    #[test]
    fn csi_with_other_intermediate_is_not_decape() {
        // `CSI 1 $ r` 是 DECCARA（上游同样未实现，但那不是本次处理范围，
        // 不得把它当 DECSACE 误吞——它仍要到达上游）。
        let (payload, _) = filter(b"\x1b[1;2;3;4;5$rX");
        assert_eq!(payload, b"\x1b[1;2;3;4;5$rX");
    }

    #[test]
    fn decape_inside_osc_string_is_preserved() {
        // OSC 字符串内容里含 DECSACE 字节形时不得误伤：字符串整段透传。
        let (payload, _) = filter(b"\x1b]0;title \x1b[*x still title\x07after");
        assert_eq!(payload, b"\x1b]0;title \x1b[*x still title\x07after");
    }

    #[test]
    fn incomplete_csi_at_chunk_end_is_passed_through() {
        let (payload, _) = filter(b"text\x1b[2*");
        assert_eq!(payload, b"text\x1b[2*");
    }

    #[test]
    fn incomplete_kitty_apc_at_chunk_end_is_passed_through() {
        let (payload, responses) = filter(b"\x1b_Ga=q,i=3,t=s;AAAA");
        assert_eq!(payload, b"\x1b_Ga=q,i=3,t=s;AAAA");
        assert!(
            responses.is_empty(),
            "不完整 APC 未到达本层判定，应答不得凭空生成"
        );
    }

    #[test]
    fn direct_medium_probe_is_passed_to_upstream() {
        let (payload, responses) = filter(b"\x1b_Ga=q,i=1,t=d,f=24,s=1,v=1;MTIz\x1b\\");
        assert_eq!(payload, b"\x1b_Ga=q,i=1,t=d,f=24,s=1,v=1;MTIz\x1b\\");
        assert!(responses.is_empty());
    }

    #[test]
    fn shared_memory_probe_is_answered_by_this_layer() {
        let (payload, responses) = filter(b"\x1b_Ga=q,i=3,t=s;AAAA\x1b\\");
        assert!(payload.is_empty(), "本层代答的探测命令不得到达上游");
        assert_eq!(
            responses,
            [b"\x1b_Gi=3;EINVAL: unsupported medium\x1b\\".to_vec()],
            "应答必须与上游格式逐字节一致，否则客户端回退失败"
        );
    }

    #[test]
    fn shared_memory_probe_is_answered_when_shared_memory_is_available() {
        let filtered = filter_input(
            b"\x1b_Ga=q,i=3,t=s;AAAA\x1b\\",
            MediaCapabilities {
                shared_memory: true,
            },
        );
        assert_eq!(filtered.payload.as_ref(), b"\x1b_Ga=q,i=3,t=s;AAAA\x1b\\");
        assert!(filtered.responses.is_empty());
    }

    #[test]
    fn file_probe_with_missing_file_is_answered_by_this_layer() {
        // 载荷是 base64 编码的路径("/definitely/not/here.png")。
        let (payload, responses) =
            filter(b"\x1b_Ga=q,i=2,t=f;/ZGVmaW5pdGVseS9ub3QvaGVyZS5wbmc=\x1b\\");
        assert!(payload.is_empty());
        assert_eq!(
            responses,
            [b"\x1b_Gi=2;EINVAL: invalid data\x1b\\".to_vec()]
        );
    }

    #[test]
    fn file_probe_with_existing_file_is_passed_to_upstream() {
        let path = std::env::temp_dir().join("torvox-input-filter-probe.txt");
        std::fs::write(&path, b"probe").expect("create probe file");
        let encoded = BASE64.encode(path.to_str().expect("temp dir is UTF-8"));
        let sequence = format!("\x1b_Ga=q,i=4,t=t;{encoded}\x1b\\");
        let (payload, responses) = filter(sequence.as_bytes());
        let _ = std::fs::remove_file(&path);
        assert_eq!(payload, sequence.as_bytes());
        assert!(responses.is_empty());
    }

    #[test]
    fn probe_without_image_id_is_dropped_without_response() {
        // 上游对无 id 的查询报 `EINVAL: image ID required` 且压根不回包：
        // 本层同样吞掉命令且不回包，否则客户端会卡在等应答上。
        let (payload, responses) = filter(b"\x1b_Ga=q,t=s;AAAA\x1b\\");
        assert!(payload.is_empty());
        assert!(responses.is_empty());
    }

    #[test]
    fn probe_with_silent_failures_is_dropped_without_response() {
        let (payload, responses) = filter(b"\x1b_Ga=q,i=7,t=s,q=2;AAAA\x1b\\");
        assert!(payload.is_empty());
        assert!(responses.is_empty(), "`q=2` 时上游也不回失败应答");
    }

    #[test]
    fn probe_with_silent_success_still_answers_failures() {
        let (payload, responses) = filter(b"\x1b_Ga=q,i=8,t=s,q=1;AAAA\x1b\\");
        assert!(payload.is_empty());
        assert_eq!(
            responses,
            [b"\x1b_Gi=8;EINVAL: unsupported medium\x1b\\".to_vec()],
            "`q=1` 只静默成功应答，失败仍要回"
        );
    }

    #[test]
    fn transmit_with_missing_file_is_answered_by_this_layer() {
        // 实际传输（非探测）同样可能因介质不可用而失败，须一并代答。
        let (payload, responses) =
            filter(b"\x1b_Ga=T,i=9,t=f;/ZGVmaW5pdGVseS9ub3QvaGVyZS5wbmc=\x1b\\");
        assert!(payload.is_empty());
        assert_eq!(
            responses,
            [b"\x1b_Gi=9;EINVAL: invalid data\x1b\\".to_vec()]
        );
    }

    #[test]
    fn non_query_kitty_commands_are_passed_through() {
        let (payload, responses) = filter(b"\x1b_Ga=p,i=3;OK\x1b\\");
        assert_eq!(payload, b"\x1b_Ga=p,i=3;OK\x1b\\");
        assert!(responses.is_empty());
    }

    #[test]
    fn kitty_apc_followed_by_text_is_split_cleanly() {
        let (payload, responses) = filter(b"\x1b_Ga=q,i=3,t=s;AAAA\x1b\\visible");
        assert_eq!(payload, b"visible");
        assert_eq!(responses.len(), 1);
    }

    #[test]
    fn response_includes_image_number_and_placement_id() {
        let (payload, responses) = filter(b"\x1b_Ga=q,I=13,p=7,t=s;AAAA\x1b\\");
        assert!(payload.is_empty());
        assert_eq!(
            responses,
            [b"\x1b_GI=13,p=7;EINVAL: unsupported medium\x1b\\".to_vec()]
        );
    }

    #[test]
    fn plain_text_without_escape_is_borrowed() {
        let input = b"plain output";
        let filtered = filter_input(
            input,
            MediaCapabilities {
                shared_memory: false,
            },
        );
        assert!(matches!(filtered.payload, Cow::Borrowed(_)));
        assert!(filtered.responses.is_empty());
    }

    #[test]
    fn control_data_with_unknown_keys_is_ignored() {
        let (payload, responses) = filter(b"\x1b_Ga=q,e=T,i=5,t=s,U=1,C=1;AAAA\x1b\\");
        assert!(payload.is_empty());
        assert_eq!(
            responses,
            [b"\x1b_Gi=5;EINVAL: unsupported medium\x1b\\".to_vec()]
        );
    }

    #[test]
    fn key_without_equals_is_ignored() {
        let (payload, responses) = filter(b"\x1b_Ga=q,bogus,t=s,i=6;AAAA\x1b\\");
        assert!(payload.is_empty());
        assert_eq!(
            responses,
            [b"\x1b_Gi=6;EINVAL: unsupported medium\x1b\\".to_vec()]
        );
    }
}
