//! Kitty 图形协议客户端模拟器：复刻 kitty 上游写出的字节流。
//!
//! 依据上游源码：
//! - `kittens/icat/detect.go`：能力探测——direct / tempfile / sharedmem 三条查询；
//! - `kittens/icat/transmit.go`：`kitten icat` 的传输——光标定位后发 `a=T`；
//! - `tools/tui/graphics/command.go`：控制数据键序与分块规则。
//!
//! 模拟器只产出字节流，不含断言；断言在 `kitty_graphics.rs`。
//!
//! kitty 未装进本仓的运行环境，客户端行为只能靠这份模拟器复现：
//! `kitten icat test.png` 的真实字节流（光标定位 + `a=T` + 载荷）是本仓
//! 唯一能稳定复现「图像显示与清除」缺陷的输入。
//!
//! 上游对非 PNG 载荷会在「zlib 压缩后更小」时启用 `o=z`；模拟器不带压缩器，
//! 故只覆盖压缩不会更小的情形（PNG 介质上游同样不压缩）。

use std::path::Path;

use base64::Engine;
use base64::engine::general_purpose::STANDARD_NO_PAD;

/// 数据格式：RGB（上游 `GRT_format_rgb`）。
pub(super) const FORMAT_RGB: u32 = 24;

/// 数据格式：PNG（上游 `GRT_format_png`）。
pub(super) const FORMAT_PNG: u32 = 100;

/// 上游的分块长度：base64 编码后的字符数。
const CHUNK_LENGTH: usize = 128 * 1024;

/// 传输介质（上游 `GRT_transmission_*`）。
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) enum Medium {
    /// 直接传输：载荷就是图像数据。
    #[default]
    Direct,
    /// 文件传输：载荷是文件路径。
    File,
    /// 临时文件传输：载荷是临时文件路径。
    TempFile,
    /// 共享内存传输：载荷是共享内存名。
    SharedMemory,
}

impl Medium {
    /// 控制数据里的介质字母；直接介质是缺省值，上游不写。
    fn letter(self) -> Option<char> {
        match self {
            Medium::Direct => None,
            Medium::File => Some('f'),
            Medium::TempFile => Some('t'),
            Medium::SharedMemory => Some('s'),
        }
    }
}

/// kitty 图形命令：字段与键序按上游 `serialize_non_default_fields`。
///
/// 上游按固定顺序写控制数据（`a,q,f,t,o,m,C,U,d,N,s,v,S,O,x,y,w,h,X,Y,c,r,i,I,p,z`），
/// 缺省键不写。模拟器复刻该顺序，使测试驱动的字节流与真实 kitty 一致。
#[derive(Clone, Debug, Default)]
struct GraphicsCommand {
    /// `a`：动作（`q` 查询、`T` 传输并显示）。
    action: char,
    /// `q`：静默级别（0 不静默、1 只回失败、2 全静默）。
    quiet: u8,
    /// `f`：数据格式。
    format: u32,
    /// `t`：传输介质。
    medium: Medium,
    /// `m=1`：后续还有分块。
    more: bool,
    /// `s`：数据宽（像素）。
    data_width: u32,
    /// `v`：数据高（像素）。
    data_height: u32,
    /// `S`：数据字节数。
    data_size: u32,
    /// `X`：单元内横向偏移。
    x_offset: u32,
    /// `i`：图像 id。
    image_id: u32,
}

impl GraphicsCommand {
    /// 控制数据：按上游键序拼接，缺省键不写。
    fn control_data(&self) -> String {
        let mut items: Vec<String> = Vec::new();
        if self.action != '\0' {
            items.push(format!("a={}", self.action));
        }
        if self.quiet != 0 {
            items.push(format!("q={}", self.quiet));
        }
        if self.format != 0 {
            items.push(format!("f={}", self.format));
        }
        if let Some(letter) = self.medium.letter() {
            items.push(format!("t={letter}"));
        }
        if self.more {
            items.push("m=1".to_string());
        }
        if self.data_width != 0 {
            items.push(format!("s={}", self.data_width));
        }
        if self.data_height != 0 {
            items.push(format!("v={}", self.data_height));
        }
        if self.data_size != 0 {
            items.push(format!("S={}", self.data_size));
        }
        if self.x_offset != 0 {
            items.push(format!("X={}", self.x_offset));
        }
        if self.image_id != 0 {
            items.push(format!("i={}", self.image_id));
        }
        items.join(",")
    }
}

/// 按上游 `WriteWithPayloadTo` 写一条命令。
///
/// 上游规则：载荷为空时只写控制数据；载荷 base64 后按 128KiB 分块，首块带全部
/// 控制数据，后续块只带 `a`/`q`/`m`（末块的 `m=0` 是缺省值，不写）。
fn write_command(command: &GraphicsCommand, payload: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    if payload.is_empty() {
        out.extend_from_slice(b"\x1b_G");
        out.extend_from_slice(command.control_data().as_bytes());
        out.extend_from_slice(b"\x1b\\");
        return out;
    }
    let encoded = STANDARD_NO_PAD.encode(payload);
    let chunks: Vec<&[u8]> = encoded.as_bytes().chunks(CHUNK_LENGTH).collect();
    for (index, chunk) in chunks.iter().enumerate() {
        let mut chunk_command = if index == 0 {
            command.clone()
        } else {
            GraphicsCommand {
                action: command.action,
                quiet: command.quiet,
                ..GraphicsCommand::default()
            }
        };
        chunk_command.more = index + 1 < chunks.len();
        out.extend_from_slice(b"\x1b_G");
        out.extend_from_slice(chunk_command.control_data().as_bytes());
        out.push(b';');
        out.extend_from_slice(chunk);
        out.extend_from_slice(b"\x1b\\");
    }
    out
}

/// 屏幕几何：kitty 按列数与单元格宽推算放置位置与单元内偏移。
#[derive(Clone, Copy, Debug)]
pub(super) struct Screen {
    /// 终端列数。
    pub(super) columns: u32,
    /// 单元格宽（像素）。
    pub(super) cell_width: u32,
}

/// `kitten icat` 的一次图像传输：客户端算好的画布尺寸、格式与介质载荷。
pub(super) struct IcatTransmission<'a> {
    /// 屏幕几何。
    pub(super) screen: Screen,
    /// 缩放后的画布宽（像素）。
    pub(super) canvas_width: u32,
    /// 缩放后的画布高（像素）。
    pub(super) canvas_height: u32,
    /// 图像 id（`--image-id` 指定；0 表示客户端不指定，由终端分配）。
    pub(super) image_id: u32,
    /// 数据格式（`f`）。
    pub(super) format: u32,
    /// 传输介质。
    pub(super) medium: Medium,
    /// 介质载荷：直接介质是图像字节，其余介质是路径或共享内存名。
    pub(super) payload: &'a [u8],
}

/// `kitten icat` 的图像传输（上游 `kittens/icat/transmit.go`）。
///
/// 复刻要点：先回车（`\r`），再按居中对齐把光标右移 `move_x_by` 列，然后发
/// `a=T` 命令（`q=2` 全静默），最后换行。图像宽高是**缩放后**的尺寸——上游默认
/// 按屏幕宽缩放（`--fit=width`），缩放过的图像以原始像素传输。
pub(super) fn icat_transmit(transmission: &IcatTransmission<'_>) -> Vec<u8> {
    let cell_width = transmission.screen.cell_width.max(1);
    let width_cells = transmission.canvas_width.div_ceil(cell_width);
    let move_x_by = transmission.screen.columns.saturating_sub(width_cells) / 2;
    let extra_pixels = transmission.canvas_width % cell_width;
    let cell_x_offset = if extra_pixels == 0 {
        0
    } else {
        (cell_width - extra_pixels) / 2
    };
    let command = GraphicsCommand {
        action: 'T',
        quiet: 2,
        format: transmission.format,
        medium: transmission.medium,
        data_width: transmission.canvas_width,
        data_height: transmission.canvas_height,
        // 上游只给临时文件与共享内存介质写 `S`（各自的载荷字节数）。
        data_size: match transmission.medium {
            Medium::TempFile | Medium::SharedMemory => transmission.payload.len() as u32,
            Medium::Direct | Medium::File => 0,
        },
        x_offset: cell_x_offset,
        image_id: transmission.image_id,
        ..GraphicsCommand::default()
    };
    let mut out = vec![b'\r'];
    if move_x_by > 0 {
        out.extend_from_slice(format!("\x1b[{move_x_by}C").as_bytes());
    }
    out.extend_from_slice(&write_command(&command, transmission.payload));
    out.push(b'\n');
    out
}

/// kitty 的能力探测（上游 `kittens/icat/detect.go`）。
///
/// 三条查询依次问 direct / tempfile / sharedmem：图像 id 从 1 递增，格式 RGB、
/// 宽高各 1，载荷是介质对应的路径（直接介质是 3 字节样例数据）。上游把 `S`
/// 写成载荷长度——临时文件介质下这个长度是路径长度，与文件实际字节数无关。
pub(super) fn detect_support_probes(temp_file: &Path, shared_memory_name: &str) -> Vec<Vec<u8>> {
    let probes = [
        (Medium::Direct, b"123".to_vec()),
        (
            Medium::TempFile,
            temp_file.to_string_lossy().as_bytes().to_vec(),
        ),
        (Medium::SharedMemory, shared_memory_name.as_bytes().to_vec()),
    ];
    probes
        .iter()
        .enumerate()
        .map(|(index, (medium, payload))| {
            let command = GraphicsCommand {
                action: 'q',
                format: FORMAT_RGB,
                medium: *medium,
                data_width: 1,
                data_height: 1,
                data_size: payload.len() as u32,
                image_id: index as u32 + 1,
                ..GraphicsCommand::default()
            };
            write_command(&command, payload)
        })
        .collect()
}

/// 终端的图形协议应答。
#[derive(Debug, PartialEq, Eq)]
pub(super) struct GraphicsResponse {
    /// 被应答的图像 id（上游对无 id 的查询不回包）。
    pub(super) image_id: u32,
    /// 应答消息：成功为 `OK`，失败为 `EINVAL: ...` 等。
    pub(super) message: String,
}

/// 解析终端写回 PTY 的图形协议应答。
pub(super) fn parse_responses(bytes: &[u8]) -> Vec<GraphicsResponse> {
    let mut responses = Vec::new();
    let mut index = 0;
    while index + 3 <= bytes.len() {
        if bytes[index..index + 3] != *b"\x1b_G" {
            index += 1;
            continue;
        }
        let Some(end) = find_string_end(bytes, index + 3) else {
            break;
        };
        let content = &bytes[index + 3..end];
        let (control, message) = match content.iter().position(|&byte| byte == b';') {
            Some(separator) => (&content[..separator], &content[separator + 1..]),
            None => (content, content),
        };
        responses.push(GraphicsResponse {
            image_id: parse_image_id(control),
            message: String::from_utf8_lossy(message).into_owned(),
        });
        index = end + 2;
    }
    responses
}

/// 应答控制数据里的图像 id。
fn parse_image_id(control: &[u8]) -> u32 {
    let mut image_id = 0;
    for item in control.split(|&byte| byte == b',') {
        let Some(separator) = item.iter().position(|&byte| byte == b'=') else {
            continue;
        };
        if separator != 1 || item[0] != b'i' {
            continue;
        }
        image_id = std::str::from_utf8(&item[separator + 1..])
            .ok()
            .and_then(|text| text.parse().ok())
            .unwrap_or(0);
    }
    image_id
}

/// 找到 ST（`ESC \`）的位置。
fn find_string_end(bytes: &[u8], start: usize) -> Option<usize> {
    let mut index = start;
    while index + 1 < bytes.len() {
        if bytes[index] == 0x1b && bytes[index + 1] == 0x5c {
            return Some(index);
        }
        index += 1;
    }
    None
}
