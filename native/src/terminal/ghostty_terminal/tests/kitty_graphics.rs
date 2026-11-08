//! Kitty 图形协议的端到端用例：用上游客户端模拟器复现 `kitten icat` 的字节流。
//!
//! 判据取自上游规则而非本仓实现：客户端算好放置位置与像素尺寸后交给终端，
//! 终端按上游口径放置图像，并在 ED2 清除时撤下放置。

use super::kitty_simulator::{
    FORMAT_PNG, GraphicsResponse, IcatTransmission, Medium, Screen, detect_support_probes,
    icat_transmit, parse_responses,
};

use super::*;

/// 终端列数（与 `terminal()` 一致）。
const COLUMNS: u32 = 80;

/// 单元格宽（像素）。
const CELL_WIDTH: u32 = 8;

/// 单元格高（像素）。
const CELL_HEIGHT: u32 = 16;

/// 图像画布宽（像素）：占 13 列，余 4 像素落在末列内。
const CANVAS_WIDTH: u32 = 100;

/// 图像画布高（像素）：占 4 行。
const CANVAS_HEIGHT: u32 = 50;

/// `kitten icat test.png` 的输入：临时目录里的 PNG 文件与对应的字节流。
///
/// 客户端按 `--fit=width` 把图缩到屏幕宽以内（100 < 80 列 × 8 像素），PNG 原样
/// 传输（`f=100`）；终端认得文件介质时走文件介质（`t=f`），载荷是 PNG 文件的
/// 绝对路径。临时目录随返回值一起交出，文件在终端读完前不消失。
fn icat_png_input() -> (tempfile::TempDir, Vec<u8>) {
    let directory = tempfile::tempdir().expect("创建临时目录");
    let path = directory.path().join("test.png");
    std::fs::write(&path, solid_png(CANVAS_WIDTH, CANVAS_HEIGHT)).expect("写图像文件");
    let payload = path.to_string_lossy().into_owned();
    let stream = icat_transmit(&IcatTransmission {
        screen: Screen {
            columns: COLUMNS,
            cell_width: CELL_WIDTH,
        },
        canvas_width: CANVAS_WIDTH,
        canvas_height: CANVAS_HEIGHT,
        image_id: 0,
        format: FORMAT_PNG,
        medium: Medium::File,
        payload: payload.as_bytes(),
    });
    (directory, stream)
}

/// `kitten icat test.png` 的字节流必须让终端按客户端算好的位置与尺寸放置图像。
///
/// 判别力：客户端把 100×50 像素的图按屏幕宽居中，图占 13 列，于是从第 33 列
/// 开始（`(80 - 13) / 2`）；图宽不是单元格宽的整数倍，客户端按上游口径给出
/// 单元内横向偏移 2 像素（`(8 - 4) / 2`）。终端若自行按原图定位或忽略单元内
/// 偏移，图像就会贴到列首或与客户端算的位置差半格。
#[test]
fn kitten_icat_places_the_image_where_the_client_positioned_it() {
    let mut terminal = terminal();
    terminal.set_cell_pixel_size(CELL_WIDTH, CELL_HEIGHT);
    let (_directory, stream) = icat_png_input();
    terminal.pty_write(&stream);
    terminal.flush();

    let placements = terminal.take_kitty_placements();
    assert_eq!(placements.len(), 1, "客户端字节流必须产生一个可见放置");
    let placement = &placements[0];
    assert_eq!(
        (placement.viewport_col, placement.viewport_row),
        (33, 0),
        "放置左缘必须是客户端居中后的第 33 列、首行"
    );
    assert_eq!(
        (placement.pixel_width, placement.pixel_height),
        (CANVAS_WIDTH, CANVAS_HEIGHT),
        "未指定 c/r 时显示尺寸就是客户端给的像素尺寸"
    );
    assert_eq!(
        placement.cell_offset_x, 2,
        "单元内横向偏移必须是客户端算出的 2 像素"
    );
    assert_eq!(
        (placement.source_width, placement.source_height),
        (CANVAS_WIDTH, CANVAS_HEIGHT),
        "取源矩形是整幅原图"
    );
    assert_eq!(
        (placement.image_width, placement.image_height),
        (CANVAS_WIDTH, CANVAS_HEIGHT),
        "解码后的图像尺寸与客户端声明一致"
    );
}

/// `kitten icat test.png` 之后输入 `clear`：图像必须在**第一次**清除时撤下。
///
/// 回归背景：用户报障「`kitten icat test.png` 后输入 clear，文本被清除但图像
/// 依旧显示且变大盖住屏幕，再敲一次 clear 才清掉」。本用例走客户端真实字节流
/// （含光标定位与换行），断言第一次 ED2 之后放置集合即为空。
#[test]
fn kitten_icat_clear_removes_the_placement_on_first_clear() {
    let mut terminal = terminal();
    terminal.set_cell_pixel_size(CELL_WIDTH, CELL_HEIGHT);
    let (_directory, stream) = icat_png_input();
    terminal.pty_write(&stream);
    terminal.flush();
    assert_eq!(
        terminal.take_kitty_placements().len(),
        1,
        "清除前必须确有可见放置，否则本用例不覆盖撤除路径"
    );

    terminal.pty_write(b"\x1b[H\x1b[2J");
    terminal.flush();
    assert!(
        terminal.take_kitty_placements().is_empty(),
        "第一次 clear 必须撤下 kitty 图像"
    );
}

/// 能力探测的三条查询都要有应答，且应答回显各自的图像 id。
///
/// 客户端（`kitten icat`）按上游 `DetectSupport` 依次问 direct / tempfile /
/// sharedmem 三个介质，等应答决定用哪种介质传图；少一条应答客户端就等到探测
/// 超时（默认 10 秒）才继续，表现为「图像迟迟不出现」。应答必须带 `i=<id>`，
/// 否则客户端认不出是哪条查询的应答。
#[test]
fn kitty_capability_probes_are_answered_with_their_image_ids() {
    let mut terminal = terminal();
    let temp_file = tempfile::Builder::new()
        .prefix("tty-graphics-protocol-")
        .tempfile()
        .expect("创建探测用临时文件");
    std::fs::write(temp_file.path(), [1u8, 2, 3]).expect("写探测用临时文件");
    let probes = detect_support_probes(temp_file.path(), "/icat-shm");
    assert_eq!(probes.len(), 3, "上游按三种介质各发一条查询");
    for probe in probes {
        terminal.pty_write(&probe);
    }
    terminal.flush();

    let responses: Vec<GraphicsResponse> = terminal
        .drain_pty_write_responses()
        .iter()
        .flat_map(|bytes| parse_responses(bytes))
        .collect();
    let answered: Vec<(u32, &str)> = responses
        .iter()
        .map(|response| (response.image_id, response.message.as_str()))
        .collect();
    assert_eq!(
        answered,
        [
            (1, "OK"),
            (2, "EINVAL: invalid data"),
            (3, "EINVAL: invalid data"),
        ],
        "三条查询的应答必须按上游错误语义逐条给出"
    );
}

/// 构造一张纯色 PNG（RGBA）：扫描行用 stored 块的 zlib 流逐字节照抄。
///
/// 夹具要的是「一张能被上游 PNG 解码器解出指定尺寸的图」，不是压缩率，故用
/// stored 块（不压缩）拼 zlib 流，省掉压缩器依赖。
fn solid_png(width: u32, height: u32) -> Vec<u8> {
    let mut row = Vec::with_capacity(1 + 4 * width as usize);
    row.push(0);
    for _ in 0..width {
        row.extend_from_slice(&[0xff, 0x00, 0x00, 0xff]);
    }
    let mut scanlines = Vec::with_capacity(row.len() * height as usize);
    for _ in 0..height {
        scanlines.extend_from_slice(&row);
    }
    let mut header = Vec::new();
    header.extend_from_slice(&width.to_be_bytes());
    header.extend_from_slice(&height.to_be_bytes());
    header.extend_from_slice(&[8, 6, 0, 0, 0]);
    let mut png = Vec::new();
    png.extend_from_slice(&[0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]);
    push_png_chunk(&mut png, b"IHDR", &header);
    push_png_chunk(&mut png, b"IDAT", &zlib_stored(&scanlines));
    push_png_chunk(&mut png, b"IEND", &[]);
    png
}

/// PNG 数据块：长度 + 类型 + 数据 + CRC32。
fn push_png_chunk(png: &mut Vec<u8>, kind: &[u8; 4], data: &[u8]) {
    png.extend_from_slice(&(data.len() as u32).to_be_bytes());
    png.extend_from_slice(kind);
    png.extend_from_slice(data);
    let mut checksum_input = Vec::with_capacity(4 + data.len());
    checksum_input.extend_from_slice(kind);
    checksum_input.extend_from_slice(data);
    png.extend_from_slice(&crc32(&checksum_input).to_be_bytes());
}

/// CRC-32（PNG 块校验用的多项式 0xedb88320）。
fn crc32(data: &[u8]) -> u32 {
    let mut checksum = 0xffff_ffffu32;
    for &byte in data {
        checksum ^= u32::from(byte);
        for _ in 0..8 {
            let mask = if checksum & 1 == 0 { 0 } else { 0xedb8_8320 };
            checksum = (checksum >> 1) ^ mask;
        }
    }
    !checksum
}

/// zlib 流（stored 块）：产出合法的 zlib 数据，避免在测试里再引入压缩器依赖。
///
/// 载荷逐段照抄原文，单块上限 65535 字节，尾部是 adler32 校验和。
fn zlib_stored(data: &[u8]) -> Vec<u8> {
    let mut out = vec![0x78, 0x01];
    let mut remaining = data;
    loop {
        let block_length = remaining.len().min(u16::MAX as usize);
        let is_last = block_length == remaining.len();
        out.push(u8::from(is_last));
        out.extend_from_slice(&(block_length as u16).to_le_bytes());
        out.extend_from_slice(&(!(block_length as u16)).to_le_bytes());
        out.extend_from_slice(&remaining[..block_length]);
        remaining = &remaining[block_length..];
        if is_last {
            break;
        }
    }
    out.extend_from_slice(&adler32(data).to_be_bytes());
    out
}

/// adler32 校验和（zlib 流尾部）。
fn adler32(data: &[u8]) -> u32 {
    let mut low = 1u32;
    let mut high = 0u32;
    for &byte in data {
        low = (low + u32::from(byte)) % 65521;
        high = (high + low) % 65521;
    }
    (high << 16) | low
}
