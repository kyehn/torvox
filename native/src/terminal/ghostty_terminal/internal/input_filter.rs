//! VT 输入预处理：剔除上游未实现的序列，并代答 Kitty 图形协议的能力探测。
//!
//! 背景（真机 logcat 证据）：
//! - `kitten icat` 启动时按协议探测可用传输介质（direct / 临时文件 / 共享内存），
//!   探测用的查询命令会到达终端。本环境满足不了其中两项：Android 没有 POSIX
//!   共享内存（上游在 `readSharedMemory` 里直接拒绝）；临时文件介质那条探测把
//!   **路径长度**当成了数据长度声明（`S`），而文件里只有 3 字节，上游按声明
//!   长度读必然读不满（`error.EndOfStream`）。上游把这类「客户端探测失败」记成
//!   `W/kitty_gfx failed to read image file` 与
//!   `W/kitty_gfx erroneous kitty graphics response`。
//! - kitty 收尾时发 `CSI * x`（DECSACE 复位），上游未实现 DECCARA/DECSACE，
//!   记成 `W/stream unimplemented CSI action`。
//!
//! 本层按同一协议自己回答探测（错误应答与上游逐字节一致），并丢弃 DECSACE：
//! 终端行为不变（客户端同样回退到 direct 介质），日志不再被上游的协议探测噪声淹没。
//!
//! 本层还标出「清屏」序列（ED2/ED3）的位置：清屏要撤下 Kitty 图像放置，
//! 而撤下必须发生在清屏之前——清屏若走「旧行先滚进回滚再清屏」的路径，
//! 可见放置会随旧行滚进回滚区，此后清屏不再撤下它们。撤下命令由调用方
//! 依据终端里的放置状态生成（见 `kitty::clear_delete_commands`）。
//!
//! 三处都只在「本块内完整出现」时处理：序列跨块拆分时原样透传，交给上游的
//! 增量解析器续接——宁可漏掉一次过滤，也不冒破坏字节流的风险。
use std::borrow::Cow;

use base64::Engine;
use base64::engine::general_purpose::STANDARD_NO_PAD_INDIFFERENT as BASE64;

/// 介质能力：决定本层能否代答某介质的能力探测。
#[derive(Clone, Copy, Debug)]
pub(crate) struct MediaCapabilities<'a> {
    /// 平台是否支持 POSIX 共享内存（Kitty `t=s` 介质）。
    pub(crate) shared_memory: bool,
    /// 终端侧配置的 kitty 临时文件目录（与子进程 `TMPDIR` 同源）。
    pub(crate) temp_dir: Option<&'a std::path::Path>,
}

impl<'a> MediaCapabilities<'a> {
    /// 本平台的能力。
    ///
    /// Android 与 Windows 无 POSIX 共享内存（上游在 `readSharedMemory` 里对这两者
    /// 直接拒绝），其余 Unix 平台按 POSIX 语义可用。
    pub(crate) fn platform(temp_dir: Option<&'a std::path::Path>) -> Self {
        Self {
            shared_memory: !cfg!(any(target_os = "android", target_os = "windows")),
            temp_dir,
        }
    }
}

/// 预处理结果：透传给上游 VT 解析器的字节与需要写回 PTY 的协议应答。
pub(crate) struct FilteredInput<'a> {
    /// 透传给上游的字节；无序列被处理时直接借用输入，不额外拷贝。
    pub(crate) payload: Cow<'a, [u8]>,
    /// 需要写回 PTY 的应答（Kitty 错误应答）。
    pub(crate) responses: Vec<Vec<u8>>,
    /// 本块里的清屏序列及其在 `payload` 中的位置。
    pub(crate) clears: Vec<Clear>,
}

/// 清屏序列的种类。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ClearKind {
    /// ED2：清活动区（kitty 扩展的 `22` 同属此类）。
    Display,
    /// ED3：清回滚区。
    Scrollback,
}

/// 清屏序列在透传载荷中的位置。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Clear {
    /// 序列起始下标（`payload` 内的字节偏移）。
    pub(crate) offset: usize,
    /// 清屏种类。
    pub(crate) kind: ClearKind,
}

/// 过滤一帧 VT 输入字节。
///
/// 逐字节扫描：`ESC [` 的 CSI 序列在中间字节为 `*` 且终字节为 `x`（DECSACE）时丢弃；
/// `ESC _ G` 的 Kitty APC 若是本层代答的能力探测则丢弃并回代错误应答；其余
/// 字符串序列（OSC / DCS / APC / PM / SOS）整段透传，内部不做任何扫描。
pub(crate) fn filter_input<'a>(
    data: &'a [u8],
    capabilities: MediaCapabilities<'_>,
) -> FilteredInput<'a> {
    if !data.contains(&0x1b) {
        return FilteredInput {
            payload: Cow::Borrowed(data),
            responses: Vec::new(),
            clears: Vec::new(),
        };
    }
    let mut payload: Vec<u8> = Vec::with_capacity(data.len());
    let mut responses: Vec<Vec<u8>> = Vec::new();
    let mut clears: Vec<Clear> = Vec::new();
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
                Some((end, kind)) => {
                    match kind {
                        CsiKind::Pass => {}
                        CsiKind::Decsace => {}
                        CsiKind::EraseDisplay => clears.push(Clear {
                            offset: payload.len(),
                            kind: ClearKind::Display,
                        }),
                        CsiKind::EraseScrollback => clears.push(Clear {
                            offset: payload.len(),
                            kind: ClearKind::Scrollback,
                        }),
                    }
                    if kind != CsiKind::Decsace {
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
        clears,
    }
}

/// CSI 序列的种类。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum CsiKind {
    /// 普通序列：原样透传。
    Pass,
    /// DECSACE（`CSI * x`）：上游未实现，丢弃。
    Decsace,
    /// ED2：清活动区。
    EraseDisplay,
    /// ED3：清回滚区。
    EraseScrollback,
}

/// 解析一帧内的 CSI 序列：`ESC [` + 参数字节 + 中间字节 + 终字节。
///
/// 返回（序列结束位置，序列种类）；序列不完整或不合法时返回 `None`，
/// 由调用方原样透传。
///
/// 字节划分与上游一致：数字与分号是参数，私有前缀（`?`）与 `0x20..=0x2F`
/// 是中间字节；中间字节之后再出现数字、子参数（`:`）的序列上游整条忽略。
fn parse_csi(data: &[u8], start: usize) -> Option<(usize, CsiKind)> {
    let mut index = start + 2;
    // 参数：分号隔开的若干个数字，值只关心「只有一个参数」的情形。
    let mut separators = 0usize;
    let mut has_param_bytes = false;
    let mut param_value = 0u32;
    // 中间字节：私有前缀与 `0x20..=0x2F`。
    let mut intermediate_count = 0usize;
    let mut only_star = true;
    let mut has_question = false;
    // 上游在中间字节之后只收中间字节，出现数字即整条忽略。
    let mut after_intermediate = false;
    while index < data.len() {
        let byte = data[index];
        match byte {
            0x30..=0x39 => {
                if after_intermediate {
                    return None;
                }
                if !has_param_bytes && separators == 0 {
                    param_value = 0;
                }
                // 饱和累加：参数来自终端输出，位数不受限。裸算在 debug 构建下溢出即
                // panic（debug 构建正是用户安装的形态），release 下绕成另一个值。
                // 饱和后参数停在 u32::MAX，既不匹配任何清屏参数（原样透传），也不会
                // 让整帧解析崩掉。
                param_value = param_value
                    .saturating_mul(10)
                    .saturating_add(u32::from(byte - b'0'));
                has_param_bytes = true;
            }
            0x3b => {
                if after_intermediate {
                    return None;
                }
                separators += 1;
                param_value = 0;
            }
            0x3a => return None,
            0x3c..=0x3f => {
                // 私有前缀：上游按中间字节收，且不打断参数。
                intermediate_count += 1;
                only_star = false;
                if byte == b'?' {
                    has_question = true;
                }
            }
            0x20..=0x2f => {
                intermediate_count += 1;
                if byte != b'*' {
                    only_star = false;
                }
                after_intermediate = true;
            }
            _ => break,
        }
        index += 1;
    }
    let final_byte = data.get(index).copied()?;
    if !matches!(final_byte, 0x40..=0x7e) {
        return None;
    }
    // DECSACE：`CSI <参数> * x`，中间字节只有 `*`（kitty 的 `CSI 2*x` 与
    // `CSI *x` 都落在这里）。
    if final_byte == b'x' && intermediate_count == 1 && only_star {
        return Some((index + 1, CsiKind::Decsace));
    }
    // 清屏：终字节 `J`，参数是擦除模式，中间字节只能是私有前缀 `?`
    // （`CSI ? 2 J` 是受保护的清屏，上游同样清活动区与图像放置）。
    let private_only = intermediate_count == 0 || (intermediate_count == 1 && has_question);
    if final_byte == b'J' && private_only && separators == 0 {
        let param_count = usize::from(has_param_bytes);
        if param_count == 1 {
            match param_value {
                2 | 22 => return Some((index + 1, CsiKind::EraseDisplay)),
                3 => return Some((index + 1, CsiKind::EraseScrollback)),
                _ => {}
            }
        }
    }
    Some((index + 1, CsiKind::Pass))
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
/// 只代答「本环境无法服务」的介质：直接介质（`t=d`）与能服务的文件介质交回上游
/// 处理；共享内存介质在本平台不可用时按上游的错误消息代答；文件介质打不开、
/// 不是普通文件、或按声明长度读不满时按上游同款的错误消息代答（判据见
/// [`file_medium_verdict`]）。
fn kitty_probe(content: &[u8], capabilities: MediaCapabilities<'_>) -> KittyProbe {
    let (control_bytes, payload_bytes) = match content.iter().position(|&byte| byte == b';') {
        Some(separator) => (&content[..separator], &content[separator + 1..]),
        None => (content, &content[content.len()..]),
    };
    let control = ControlData::parse(control_bytes);
    // 只有查询与传输两类命令会因介质不可用而失败；其余（显示、删除、动画等）
    // 本层不干预。`a` 的缺省值按上游语义是 `t`：kitty 规范里「查询可用传输介质」
    // 的示例本身就省略 `a`（`ESC_Gi=31,s=10,v=2,t=s;…`），按「无 `a` 即非探测」
    // 放行会让共享内存探测直达上游，由上游记一条 `erroneous kitty graphics response`。
    if !matches!(control.action, None | Some(b'q' | b't' | b'T')) {
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
            Some(path) => match file_medium_verdict(&path, &control, &capabilities) {
                FileVerdict::Pass => KittyProbe::Pass,
                FileVerdict::Reject => {
                    KittyProbe::Answer(control.error_response("EINVAL: invalid data"))
                }
            },
            None => KittyProbe::Answer(control.error_response("EINVAL: invalid data")),
        },
        _ => KittyProbe::Pass,
    }
}

/// 文件介质本层的判定结果。
enum FileVerdict {
    /// 本层不代答，交回上游处理。
    Pass,
    /// 本层代答，回上游同款的 `EINVAL: invalid data`。
    Reject,
}

/// 文件介质能否由本环境服务。
///
/// 上游的判据（`readFile`）依次是：文件必须能打开；路径不得是 `/proc`、`/sys`、
/// `/dev`（`/dev/shm` 除外）；临时文件介质还要求路径在临时目录内且名字含
/// `tty-graphics-protocol`；文件必须是普通文件；最后按 `S` 从文件里读**恰好**
/// 这么多字节，读不满即 `InvalidData`（`EINVAL: invalid data`）。
///
/// 本层只在**能确定上游会给出同一条错误**时才代答。临时文件介质的两条额外判据
/// 在上游是更具体的错误（「not in temp dir」/「not named correctly」），故本层先
/// 确认路径满足那两条；确认不了就交回上游，由上游给出它自己的那条错误。
///
/// 代答的直接动因是日志卫生：客户端的能力探测故意写一个 3 字节的临时文件，却把
/// **路径长度**当成了数据长度声明（上游按声明长度读，必然读不满），上游为此先记
/// 一条 `failed to read image file` 警告再回错误应答——那是协议探测噪声，不是缺陷。
fn file_medium_verdict(
    path: &str,
    control: &ControlData,
    capabilities: &MediaCapabilities<'_>,
) -> FileVerdict {
    // 打不开：上游在判临时文件之前就回 `InvalidData`。
    let Ok(metadata) = std::fs::metadata(path) else {
        return FileVerdict::Reject;
    };
    // 临时文件介质：上游的两条判据过了才读文件。本层只做确认式判断——确认满足时
    // 上游必然也满足（判据口径与上游一致，见 `is_serviceable_temp_file`）；判断
    // 不了时交回上游，让上游给出它自己的那条错误。
    if control.medium == b't' && !is_serviceable_temp_file(path, capabilities) {
        return FileVerdict::Pass;
    }
    // 不是普通文件：上游回 `InvalidData`。
    if !metadata.is_file() {
        return FileVerdict::Reject;
    }
    // `S` 缺省时上游把整个文件读完，此时文件多长都能服务。
    if control.data_size > 0 {
        // `O`（偏移）是上游读文件的起点，故需要的长度是偏移加数据长度。
        let required = u64::from(control.data_size) + u64::from(control.offset);
        if metadata.len() < required {
            return FileVerdict::Reject;
        }
    }
    FileVerdict::Pass
}

/// 路径是否满足上游对临时文件的两条判据（在临时目录下 + 名字含标记）。
///
/// 上游的「在临时目录下」口径是 `isPathInTempDir`：`/tmp`、`/dev/shm`、配置的
/// 临时目录、以及配置目录的规范化形式，任一命中即可（配置目录可能是符号链接：
/// Android 上 `/data/data` 指向 `/data/user/0`）。本层照抄该口径——命中时上游
/// 必然也命中，两边对同一条探测给出同一条错误应答；判断不了时交回上游，由上游
/// 给出它那条更具体的错误。
fn is_serviceable_temp_file(path: &str, capabilities: &MediaCapabilities<'_>) -> bool {
    if !path.contains("tty-graphics-protocol") {
        return false;
    }
    if is_path_in_dir("/tmp", path) || is_path_in_dir("/dev/shm", path) {
        return true;
    }
    let Some(temp_dir) = capabilities.temp_dir.and_then(|dir| dir.to_str()) else {
        return false;
    };
    is_path_in_dir(temp_dir, path) || canonical_dir_contains(temp_dir, path)
}

/// 配置目录规范化后是否仍覆盖该路径（配置目录本身可能是符号链接）。
fn canonical_dir_contains(dir: &str, path: &str) -> bool {
    let (Ok(canonical_dir), Ok(canonical_path)) =
        (std::fs::canonicalize(dir), std::fs::canonicalize(path))
    else {
        return false;
    };
    match (canonical_dir.to_str(), canonical_path.to_str()) {
        (Some(canonical_dir), Some(canonical_path)) => {
            is_path_in_dir(canonical_dir, canonical_path)
        }
        _ => false,
    }
}

/// 路径是否位于目录之下（口径与上游 `isPathInDir` 一致）。
fn is_path_in_dir(dir: &str, path: &str) -> bool {
    if dir.is_empty() || !path.starts_with(dir) {
        return false;
    }
    if path.len() == dir.len() || dir.ends_with('/') {
        return true;
    }
    path.as_bytes().get(dir.len()) == Some(&b'/')
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
    /// `a`：动作（`q` 查询 / `t` 传输 / `T` 传输并显示）。缺省时上游按 `t` 处理。
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
    /// `S`：数据长度（字节）。文件介质下上游按它从文件里读恰好这么多字节。
    data_size: u32,
    /// `O`：文件内的起始偏移（字节）。
    offset: u32,
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
            data_size: 0,
            offset: 0,
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
                b'S' => control.data_size = parse_number(value),
                b'O' => control.offset = parse_number(value),
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
                temp_dir: None,
            },
        );
        (filtered.payload.into_owned(), filtered.responses)
    }

    /// 过滤一帧并取清屏序列：返回（载荷，清屏列表）。
    fn filter_clears(data: &[u8]) -> (Vec<u8>, Vec<Clear>) {
        let filtered = filter_input(
            data,
            MediaCapabilities {
                shared_memory: false,
                temp_dir: None,
            },
        );
        (filtered.payload.into_owned(), filtered.clears)
    }

    #[test]
    fn erase_display_is_reported_with_its_offset() {
        let (payload, clears) = filter_clears(b"text\x1b[H\x1b[2Jmore");
        assert_eq!(payload, b"text\x1b[H\x1b[2Jmore", "清屏序列原样透传");
        assert_eq!(
            clears,
            [Clear {
                offset: 7,
                kind: ClearKind::Display,
            }],
            "ED2 必须被标出，且位置指向序列起点"
        );
    }

    #[test]
    fn erase_scrollback_is_reported() {
        let (_, clears) = filter_clears(b"\x1b[3J");
        assert_eq!(
            clears,
            [Clear {
                offset: 0,
                kind: ClearKind::Scrollback,
            }],
            "ED3 必须被标出"
        );
    }

    #[test]
    fn kitty_scroll_complete_erase_counts_as_erase_display() {
        let (_, clears) = filter_clears(b"\x1b[22J");
        assert_eq!(
            clears,
            [Clear {
                offset: 0,
                kind: ClearKind::Display,
            }],
            "kitty 扩展的 22 与 ED2 同样清活动区"
        );
    }

    #[test]
    fn protected_erase_display_is_reported() {
        let (_, clears) = filter_clears(b"\x1b[?2J");
        assert_eq!(
            clears,
            [Clear {
                offset: 0,
                kind: ClearKind::Display,
            }],
            "受保护的清屏同样清活动区与图像放置"
        );
    }

    #[test]
    fn partial_erase_modes_are_not_reported() {
        // ED0（清到行尾）与 ED1（清到行首）不动整屏，上游也不撤下图像放置。
        let (_, clears) = filter_clears(b"\x1b[0J\x1b[1J");
        assert!(clears.is_empty(), "局部清屏不得触发撤下");
    }

    #[test]
    fn erase_with_space_before_final_is_not_reported() {
        // `CSI 2 SP J` 的中间字节不是私有前缀，上游整条忽略。
        let (_, clears) = filter_clears(b"\x1b[2 J");
        assert!(clears.is_empty(), "上游忽略的清屏不得触发撤下");
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

    /// 超长数字参数不得让整帧解析崩掉：参数位数不受限，裸算在 debug 构建下溢出即
    /// panic（debug 构建正是用户安装的形态）。饱和后参数不再是任何清屏参数，
    /// 序列原样透传。
    #[test]
    fn oversized_csi_parameter_does_not_overflow() {
        let (payload, clears) = filter_clears(b"text\x1b[99999999999999999999Jtail");
        assert_eq!(
            payload, b"text\x1b[99999999999999999999Jtail",
            "超长参数既不得崩掉也不得被当成清屏"
        );
        assert!(
            clears.is_empty(),
            "超长参数不匹配任何清屏参数，不得误报清屏"
        );
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

    /// kitty 规范里「查询可用传输介质」的示例省略 `a`，而上游按 `t` 处理缺省动作：
    /// 这类探测必须与本层已覆盖的显式 `a=t` 同等代答，否则上游会为它记一条
    /// `erroneous kitty graphics response`（真机 logcat 的唯一来源）。
    #[test]
    fn shared_memory_probe_without_action_is_answered_by_this_layer() {
        let (payload, responses) = filter(b"\x1b_Gi=31,s=1,v=1,t=s;AAAA\x1b\\");
        assert!(payload.is_empty(), "本层代答的探测命令不得到达上游");
        assert_eq!(
            responses,
            [b"\x1b_Gi=31;EINVAL: unsupported medium\x1b\\".to_vec()],
            "缺省动作按上游语义就是 transmit，应答必须与显式 a=t 一致"
        );
    }

    /// 缺省动作 + 能服务的介质仍须交回上游：那是真实传输，拦下图像就传不过来。
    #[test]
    fn shared_memory_probe_without_action_is_passed_when_shared_memory_is_available() {
        let filtered = filter_input(
            b"\x1b_Gi=31,s=1,v=1,t=s;AAAA\x1b\\",
            MediaCapabilities {
                shared_memory: true,
                temp_dir: None,
            },
        );
        assert_eq!(
            filtered.payload.as_ref(),
            b"\x1b_Gi=31,s=1,v=1,t=s;AAAA\x1b\\"
        );
        assert!(filtered.responses.is_empty());
    }

    #[test]
    fn shared_memory_probe_is_answered_when_shared_memory_is_available() {
        let filtered = filter_input(
            b"\x1b_Ga=q,i=3,t=s;AAAA\x1b\\",
            MediaCapabilities {
                shared_memory: true,
                temp_dir: None,
            },
        );
        assert_eq!(filtered.payload.as_ref(), b"\x1b_Ga=q,i=3,t=s;AAAA\x1b\\");
        assert!(filtered.responses.is_empty());
    }

    /// 文件介质探测同样可能省略 `a`：缺省动作 + 打不开的路径必须由本层代答。
    #[test]
    fn file_probe_without_action_is_answered_by_this_layer() {
        let (payload, responses) = filter(b"\x1b_Gi=2,t=f;/ZGVmaW5pdGVseS9ub3QvaGVyZS5wbmc=\x1b\\");
        assert!(payload.is_empty());
        assert_eq!(
            responses,
            [b"\x1b_Gi=2;EINVAL: invalid data\x1b\\".to_vec()]
        );
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
        let path = std::env::temp_dir().join("input-filter-probe.txt");
        std::fs::write(&path, b"probe").expect("create probe file");
        let encoded = BASE64.encode(path.to_str().expect("temp dir is UTF-8"));
        let sequence = format!("\x1b_Ga=q,i=4,t=t;{encoded}\x1b\\");
        let (payload, responses) = filter(sequence.as_bytes());
        let _ = std::fs::remove_file(&path);
        assert_eq!(payload, sequence.as_bytes());
        assert!(responses.is_empty());
    }

    /// 客户端的能力探测把路径长度当成了数据长度：文件只有 3 字节，上游按声明
    /// 长度读必然读不满，先记一条警告再回错误应答。本层按上游的错误语义代答，
    /// 终端行为不变而日志不再有探测噪声。
    #[test]
    fn file_probe_shorter_than_declared_length_is_answered_by_this_layer() {
        let path = std::env::temp_dir().join("input-filter-short-probe.bin");
        std::fs::write(&path, [1u8, 2, 3]).expect("create probe file");
        let encoded = BASE64.encode(path.to_str().expect("temp dir is UTF-8"));
        let sequence = format!("\x1b_Ga=q,i=5,t=f,S=64;{encoded}\x1b\\");
        let (payload, responses) = filter(sequence.as_bytes());
        let _ = std::fs::remove_file(&path);
        assert!(payload.is_empty(), "本层代答的探测命令不得到达上游");
        assert_eq!(
            responses,
            [b"\x1b_Gi=5;EINVAL: invalid data\x1b\\".to_vec()],
            "读不满时上游回的就是这条错误应答"
        );
    }

    /// 声明长度与文件等长时必须交回上游：真实传输正是这条路，拦下它图像就传不过来。
    #[test]
    fn file_probe_matching_the_declared_length_is_passed_to_upstream() {
        let path = std::env::temp_dir().join("input-filter-exact-probe.bin");
        std::fs::write(&path, [1u8, 2, 3, 4, 5]).expect("create probe file");
        let encoded = BASE64.encode(path.to_str().expect("temp dir is UTF-8"));
        let sequence = format!("\x1b_Ga=q,i=6,t=f,S=5;{encoded}\x1b\\");
        let (payload, responses) = filter(sequence.as_bytes());
        let _ = std::fs::remove_file(&path);
        assert_eq!(payload, sequence.as_bytes());
        assert!(responses.is_empty());
    }

    /// 偏移也要算进所需长度：上游先跳到偏移再读声明长度，文件不够长就读不满。
    #[test]
    fn file_probe_with_offset_needs_the_offset_room() {
        let path = std::env::temp_dir().join("input-filter-offset-probe.bin");
        std::fs::write(&path, [1u8, 2, 3, 4, 5, 6]).expect("create probe file");
        let encoded = BASE64.encode(path.to_str().expect("temp dir is UTF-8"));
        let sequence = format!("\x1b_Ga=q,i=7,t=f,O=3,S=3;{encoded}\x1b\\");
        let (payload, responses) = filter(sequence.as_bytes());
        let _ = std::fs::remove_file(&path);
        assert_eq!(payload, sequence.as_bytes(), "偏移加长度正好等于文件长度");
        assert!(responses.is_empty());
    }

    /// 偏移加长度超出文件长度时读不满，与没有偏移时同一条错误。
    #[test]
    fn file_probe_with_offset_past_the_end_is_answered_by_this_layer() {
        let path = std::env::temp_dir().join("input-filter-offset-short-probe.bin");
        std::fs::write(&path, [1u8, 2, 3, 4, 5]).expect("create probe file");
        let encoded = BASE64.encode(path.to_str().expect("temp dir is UTF-8"));
        let sequence = format!("\x1b_Ga=q,i=8,t=f,O=3,S=3;{encoded}\x1b\\");
        let (payload, responses) = filter(sequence.as_bytes());
        let _ = std::fs::remove_file(&path);
        assert!(payload.is_empty());
        assert_eq!(
            responses,
            [b"\x1b_Gi=8;EINVAL: invalid data\x1b\\".to_vec()]
        );
    }

    /// 临时文件介质：路径在配置的临时目录里、名字带标记时，本层才敢代答——
    /// 上游对临时文件另有两判据，不满足时它给的是更具体的错误。
    #[test]
    fn temp_file_probe_inside_the_configured_temp_dir_is_answered_by_this_layer() {
        let path = outside_system_temp_dir("kitty-tty-graphics-protocol-inside");
        std::fs::write(&path, [1u8, 2, 3]).expect("create probe file");
        let encoded = BASE64.encode(path.to_str().expect("probe path is UTF-8"));
        let sequence = format!("\x1b_Ga=q,i=9,t=t,S=64;{encoded}\x1b\\");
        let filtered = filter_input(
            sequence.as_bytes(),
            MediaCapabilities {
                shared_memory: false,
                temp_dir: Some(path.parent().expect("probe file has a parent")),
            },
        );
        let _ = std::fs::remove_file(&path);
        assert!(filtered.payload.is_empty());
        assert_eq!(
            filtered.responses,
            [b"\x1b_Gi=9;EINVAL: invalid data\x1b\\".to_vec()]
        );
    }

    /// 临时文件介质：路径不在任何临时目录里时交回上游，由上游按自己的判据
    /// 给出「not in temp dir」那一条——本层不代答它给不出的错误。
    #[test]
    fn temp_file_probe_outside_every_temp_dir_is_left_to_upstream() {
        let path = outside_system_temp_dir("kitty-tty-graphics-protocol-outside");
        std::fs::write(&path, [1u8, 2, 3]).expect("create probe file");
        let encoded = BASE64.encode(path.to_str().expect("probe path is UTF-8"));
        let sequence = format!("\x1b_Ga=q,i=10,t=t,S=64;{encoded}\x1b\\");
        let other_directory = path
            .parent()
            .expect("probe file has a parent")
            .join("another-dir");
        let filtered = filter_input(
            sequence.as_bytes(),
            MediaCapabilities {
                shared_memory: false,
                temp_dir: Some(&other_directory),
            },
        );
        let _ = std::fs::remove_file(&path);
        assert_eq!(filtered.payload, sequence.as_bytes());
        assert!(filtered.responses.is_empty());
    }

    /// 数据长度是 `S`，`s` 是像素宽：把 `s` 当长度会拦下真实传输（宽度常常大于
    /// 文件字节数），也会放过上轮的探测噪声（宽度恒为 1，永远「读得满」）。
    #[test]
    fn file_probe_pixel_width_is_not_treated_as_the_data_size() {
        let path = std::env::temp_dir().join("input-filter-width-probe.bin");
        std::fs::write(&path, [1u8, 2, 3, 4, 5]).expect("create probe file");
        let encoded = BASE64.encode(path.to_str().expect("temp dir is UTF-8"));
        let sequence = format!("\x1b_Ga=q,i=11,t=f,s=100;{encoded}\x1b\\");
        let (payload, responses) = filter(sequence.as_bytes());
        let _ = std::fs::remove_file(&path);
        assert_eq!(
            payload,
            sequence.as_bytes(),
            "没有 `S` 时上游把整个文件读完，本层不得拦下"
        );
        assert!(responses.is_empty());
    }

    /// 不在 `/tmp`（也不在 `/dev/shm`）下的探测文件路径：上游的临时目录判据把
    /// 这两个目录无条件算作临时目录，测试要覆盖「配置目录之外的临时文件」，
    /// 就得把文件放在这两处之外。Cargo 的构建输出目录既存在又不在它们之下。
    fn outside_system_temp_dir(name: &str) -> std::path::PathBuf {
        std::path::Path::new(env!("OUT_DIR")).join(name)
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
                temp_dir: None,
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
