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
//! 代答只在「能确定上游会给出同一条应答」时才做：控制数据的解析（见
//! [`parse_control_data`]）与判据顺序（见 [`kitty_probe`]）都照抄上游，
//! 其余一律交回上游处理。
//!
//! 本层还标出「清屏」序列（ED2/ED3）的位置：清屏要撤下 Kitty 图像放置，
//! 而撤下必须发生在清屏之前——清屏若走「旧行先滚进回滚再清屏」的路径，
//! 可见放置会随旧行滚进回滚区，此后清屏不再撤下它们。撤下命令由调用方
//! 依据终端里的放置状态生成（见 `kitty::clear_delete_commands`）。
//!
//! 两处（DECSACE 与清屏标记）都只在「本块内完整出现」时处理：CSI 跨块拆分时
//! 原样透传，交给上游的增量解析器续接——宁可漏掉一次过滤，也不冒破坏字节流的风险。
//!
//! Kitty APC 是例外：判定它要读整条控制数据，而探测被 PTY 读边界拆开时上游
//! 会为它记两条警告。故块尾未结束的 APC 前缀由 [`FilteredInput::carry`] 交给
//! 调用方扣留，下一块拼接后重新判定（上限 [`KITTY_CARRY_LIMIT`]，超限即是
//! 从不代答的 direct 大图，直接透传）。
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
    /// 块尾未完成的 Kitty APC 前缀：读线程 MUST 在下一块前拼回，再交给本层判定。
    ///
    /// 拼回去是判定能力的**前提**而不是优化：探测被 PTY 读边界拆开时，直接透传
    /// 会让上游拿到一条本层没见过的探测——文件打不开的那条由上游记
    /// `W/kitty_gfx failed to open image file` 与 `erroneous kitty graphics response`。
    pub(crate) carry: Cow<'a, [u8]>,
}

/// 块尾扣留的上限：能力探测只有几十字节，超过这个长度仍未结束的 APC 必是
/// direct 介质的大图（本层从不代答 direct），扣留只会把大图推迟到下一块才显示。
const KITTY_CARRY_LIMIT: usize = 4096;

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
            carry: Cow::Borrowed(&[][..]),
        };
    }
    let mut payload: Vec<u8> = Vec::with_capacity(data.len());
    let mut responses: Vec<Vec<u8>> = Vec::new();
    let mut clears: Vec<Clear> = Vec::new();
    // 块尾扣留的字节：本块内没机会再判定的 APC 前缀。
    let mut carry: &'a [u8] = &[];
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
                // 块尾的裸 ESC 可能正是 APC 的开头，留到下一块再判定。
                carry = &data[index..];
                index = data.len();
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
                        if data.len() - index <= KITTY_CARRY_LIMIT {
                            carry = &data[index..];
                        } else {
                            payload.extend_from_slice(&data[index..]);
                        }
                        index = data.len();
                    }
                }
            }
            Some(b'_' | b'P' | b'^' | b'X') => match find_string_end(data, index + 2, false) {
                Some(end) => {
                    payload.extend_from_slice(&data[index..end]);
                    index = end;
                }
                // 只剩引入符（`ESC _` 等）时下一块可能正是 `_G` 的后半，一并扣留。
                None if data.len() - index <= 2 => {
                    carry = &data[index..];
                    index = data.len();
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
        carry: Cow::Borrowed(carry),
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
/// 判据与上游同序：上游先判图像 id 与图像编号互斥，再按动作分流；查询缺图像
/// id 时上游先报 id 缺失（与介质无关），而那条错误的应答没有 id 与编号，上游
/// 压根不编码它，传输才轮到介质。介质不可用时上游回 `EINVAL: unsupported
/// medium`（共享内存）或 `EINVAL: invalid data`（文件），本层按同款错误消息
/// 代答。
///
/// 只代答「本环境无法服务」的介质：直接介质（`t=d`）与能服务的文件介质交回上游
/// 处理；共享内存介质在本平台不可用时按上游的错误消息代答；文件介质打不开、
/// 不是普通文件、或按声明长度读不满时按上游同款的错误消息代答（判据见
/// [`file_medium_verdict`]）。载荷不是合法 base64 时上游在解析阶段就丢弃整条
/// 命令且不回应答，本层同样丢弃。
fn kitty_probe(content: &[u8], capabilities: MediaCapabilities<'_>) -> KittyProbe {
    let Some((control, payload_bytes)) = parse_control_data(content) else {
        // 上游判这条命令无效：它整条丢弃且不回应答。本层交回上游，
        // 行为与没有本层时一致。
        return KittyProbe::Pass;
    };
    // 只有查询与传输两类命令会因介质不可用而失败；其余（显示、删除、动画等）
    // 本层不干预。`a` 的缺省值按上游语义是 `t`：kitty 规范里「查询可用传输介质」
    // 的示例本身就省略 `a`（`ESC_Gi=31,s=10,v=2,t=s;…`），按「无 `a` 即非探测」
    // 放行会让共享内存探测直达上游，由上游记一条 `erroneous kitty graphics response`。
    if !matches!(control.action, None | Some(b'q' | b't' | b'T')) {
        return KittyProbe::Pass;
    }
    // 上游在动作分流之前先判 id 与编号互斥：两者都给时上游回互斥错误，
    // 根本走不到介质判据。
    if control.image_id > 0 && control.image_number > 0 {
        return KittyProbe::Answer(
            control.error_response("EINVAL: image ID and number are mutually exclusive"),
        );
    }
    // 查询要求图像 id，缺省时上游先报 id 缺失（与介质无关）。但那条错误的应答
    // 没有图像 id 与编号，上游压根不编码它——客户端收不到任何字节。
    if control.action == Some(b'q') && control.image_id == 0 {
        return KittyProbe::Answer(None);
    }
    match control.medium {
        Medium::Direct => KittyProbe::Pass,
        Medium::SharedMemory => {
            if capabilities.shared_memory {
                KittyProbe::Pass
            } else {
                KittyProbe::Answer(control.error_response("EINVAL: unsupported medium"))
            }
        }
        Medium::File | Medium::TempFile => match decode_path(payload_bytes) {
            // 载荷不是合法 base64：上游在解析阶段就判整条命令无效，既不执行也
            // 不回应答。本层同样丢弃，且不留上游那两条协议错误日志。
            None => KittyProbe::Answer(None),
            Some(path) => match file_medium_verdict(&path, &control, &capabilities) {
                FileVerdict::Pass => KittyProbe::Pass,
                FileVerdict::Reject => {
                    KittyProbe::Answer(control.error_response("EINVAL: invalid data"))
                }
            },
        },
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
    if control.medium == Medium::TempFile && !is_serviceable_temp_file(path, capabilities) {
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
/// Android 上 `/data/data` 指向 `/data/user/0`）。上游比的是**打开后文件句柄的
/// 规范化路径**，名字判据也看那条路径；本层照抄——命中时上游必然也命中，两边对
/// 同一条探测给出同一条错误应答；判断不了时交回上游，由上游给出它自己的那条错误。
fn is_serviceable_temp_file(path: &str, capabilities: &MediaCapabilities<'_>) -> bool {
    let Ok(canonical_path) = std::fs::canonicalize(path) else {
        return false;
    };
    let Some(canonical_path) = canonical_path.to_str() else {
        return false;
    };
    if !canonical_path.contains("tty-graphics-protocol") {
        return false;
    }
    if is_path_in_dir("/tmp", canonical_path) || is_path_in_dir("/dev/shm", canonical_path) {
        return true;
    }
    // macOS 上 `/tmp` 是 `/private/tmp` 的符号链接，规范化路径落在那里。
    if cfg!(target_os = "macos") && is_path_in_dir("/private/tmp", canonical_path) {
        return true;
    }
    let Some(temp_dir) = capabilities.temp_dir.and_then(|dir| dir.to_str()) else {
        return false;
    };
    if is_path_in_dir(temp_dir, canonical_path) {
        return true;
    }
    // 配置的临时目录本身可能是符号链接，再比一次它的规范化形式。
    let Ok(canonical_dir) = std::fs::canonicalize(temp_dir) else {
        return false;
    };
    canonical_dir
        .to_str()
        .is_some_and(|canonical_dir| is_path_in_dir(canonical_dir, canonical_path))
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

/// 传输介质（上游 `Transmission.Medium`）。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Medium {
    /// `d`：直接，载荷即图像数据。
    Direct,
    /// `f`：文件，载荷是 base64 编码的路径。
    File,
    /// `t`：临时文件，载荷是 base64 编码的路径。
    TempFile,
    /// `s`：共享内存，载荷是 base64 编码的段名。
    SharedMemory,
}

/// Kitty 控制数据里本层关心的键。
struct ControlData {
    /// `a`：动作（`q` 查询 / `t` 传输 / `T` 传输并显示）。缺省时上游按 `t` 处理。
    action: Option<u8>,
    /// `t`：传输介质（默认 `d` 直接）。
    medium: Medium,
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

/// 控制数据的解析状态（上游 `command.Parser` 的 `State`）。
#[derive(Clone, Copy)]
enum ParseState {
    /// 读键：读到 `=` 进值状态，读到 `;` 进载荷。
    Key,
    /// 忽略键：多字节键与键缓冲溢出都落在这里，此后的键值对全部被忽略。
    KeyIgnore,
    /// 读值：读到 `,` 回键状态，读到 `;` 进载荷。
    Value,
    /// 忽略值：值缓冲溢出落在这里；`,` 回忽略键状态，`;` 进载荷。
    ValueIgnore,
}

/// 上游控制数据缓冲的长度上限（11 字节：32 位整数加一位符号）。
const CONTROL_BUFFER_LIMIT: usize = 11;

/// 按上游 `command.Parser` 的口径解析 Kitty APC 段的内容。
///
/// 返回 `None` 表示上游会判这条命令无效：控制数据的结构或取值不合法时，上游
/// 整条丢弃且不回应答，本层 MUST NOT 代答——交回上游，行为与没有本层时一致。
///
/// 上游的解析口径（本层逐条照抄）：
/// - 键恰好一个字节才算数。键多于一个字节时上游转入「忽略态」，此后**所有**
///   键值对都被忽略（不只是那一对）：`a=q,bogus,t=s,i=6` 里键是 `bogus,t`
///   （上游在键状态里把 `,` 当普通字节收下），整条命令只剩 `a=q` 有效。
/// - 值恰好一个字节且不是数字时取其 ASCII 码；否则按十进制无符号整数解析，
///   解析失败即整条命令无效。
/// - 键或值超过 11 字节时上游的缓冲溢出，同样转入忽略态。
/// - `;` 落在键状态时上游丢弃这个键并开始收载荷；落在忽略态的键状态时被忽略，
///   载荷根本不开始，整条命令无效。
/// - 键在命令末尾没有收尾时整条命令无效；值在末尾收尾则照常解析。
///
/// 返回的载荷是 `;` 之后的部分（上游以 `;` 划分控制数据与载荷）。
fn parse_control_data(content: &[u8]) -> Option<(ControlData, &[u8])> {
    let mut control = ControlData {
        action: None,
        medium: Medium::Direct,
        image_id: 0,
        image_number: 0,
        placement_id: 0,
        quiet: 0,
        data_size: 0,
        offset: 0,
    };
    let mut state = ParseState::Key;
    let mut key = [0u8; CONTROL_BUFFER_LIMIT];
    let mut key_len = 0usize;
    let mut value = [0u8; CONTROL_BUFFER_LIMIT];
    let mut value_len = 0usize;
    let mut current_key = 0u8;
    let mut index = 0usize;
    while index < content.len() {
        let byte = content[index];
        index += 1;
        match state {
            ParseState::Key => match byte {
                b'=' => {
                    if key_len == 1 {
                        current_key = key[0];
                        state = ParseState::Value;
                    } else {
                        // 多字节键：整对被忽略，且此后的键值对都被忽略。
                        state = ParseState::ValueIgnore;
                    }
                    key_len = 0;
                }
                // `;` 落在键状态：上游丢弃这个键，余下是载荷。
                b';' => return Some((control, &content[index..])),
                _ => {
                    if key_len == CONTROL_BUFFER_LIMIT {
                        // 键缓冲溢出：上游转入忽略态。
                        state = ParseState::KeyIgnore;
                        key_len = 0;
                    } else {
                        key[key_len] = byte;
                        key_len += 1;
                    }
                }
            },
            ParseState::KeyIgnore => {
                if byte == b'=' {
                    state = ParseState::ValueIgnore;
                }
            }
            ParseState::Value => match byte {
                b',' => {
                    finish_value(&mut control, current_key, &value[..value_len])?;
                    value_len = 0;
                    state = ParseState::Key;
                }
                b';' => {
                    finish_value(&mut control, current_key, &value[..value_len])?;
                    return Some((control, &content[index..]));
                }
                _ => {
                    if value_len == CONTROL_BUFFER_LIMIT {
                        // 值缓冲溢出：整对被忽略，且此后的键值对都被忽略。
                        state = ParseState::ValueIgnore;
                        value_len = 0;
                    } else {
                        value[value_len] = byte;
                        value_len += 1;
                    }
                }
            },
            ParseState::ValueIgnore => match byte {
                b',' => state = ParseState::KeyIgnore,
                b';' => return Some((control, &content[index..])),
                _ => {}
            },
        }
    }
    match state {
        // 命令末尾仍停在键状态：上游判整条命令无效。
        ParseState::Key | ParseState::KeyIgnore => None,
        // 值在末尾收尾：上游照常解析它。
        ParseState::Value => {
            finish_value(&mut control, current_key, &value[..value_len])?;
            Some((control, &[]))
        }
        // 被忽略的那一对：上游照常收尾。
        ParseState::ValueIgnore => Some((control, &[])),
    }
}

/// 收尾一个键值对：按上游 `finishValue` 的口径取值并写入控制数据。
///
/// 取值失败即整条命令无效（上游在 `finishValue` 里把解析错误抛给调用方）。
fn finish_value(control: &mut ControlData, key: u8, value: &[u8]) -> Option<()> {
    let number = parse_value(value)?;
    match key {
        b'a' => control.action = Some(u8::try_from(number).ok()?),
        b't' => {
            control.medium = match u8::try_from(number).ok()? {
                b'd' => Medium::Direct,
                b'f' => Medium::File,
                b't' => Medium::TempFile,
                b's' => Medium::SharedMemory,
                _ => return None,
            }
        }
        b'i' => control.image_id = number,
        b'I' => control.image_number = number,
        b'p' => control.placement_id = number,
        b'q' => control.quiet = number,
        b'S' => control.data_size = number,
        b'O' => control.offset = number,
        _ => {}
    }
    Some(())
}

/// 按上游 `finishValue` 的口径解析一个控制值。
///
/// 上游的规则：恰好一个字节且不是数字时取其 ASCII 码；否则按十进制无符号整数
/// 解析，解析失败（空值、非数字、越界）即整条命令无效。
fn parse_value(value: &[u8]) -> Option<u32> {
    if value.len() == 1 && !value[0].is_ascii_digit() {
        return Some(u32::from(value[0]));
    }
    if value.is_empty() {
        return None;
    }
    let mut number = 0u32;
    for &byte in value {
        if !byte.is_ascii_digit() {
            return None;
        }
        number = number
            .checked_mul(10)?
            .checked_add(u32::from(byte - b'0'))?;
    }
    Some(number)
}

impl ControlData {
    /// 按上游的应答格式生成错误应答；静默规则与上游一致。
    fn error_response(&self, message: &str) -> Option<Vec<u8>> {
        // 静默等级 ≥2 时连失败应答也不回。上游把除 0、1 之外的值一律当
        // 「不回包」（`0 => .no, 1 => .ok, else => .failures`），故 `q=3` 与
        // `q=2` 同样静默——只认 `q=2` 会让本层比上游多回一个错误应答。
        if self.quiet >= 2 {
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

#[cfg(test)]
mod tests;
