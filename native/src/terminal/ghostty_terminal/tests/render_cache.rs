use super::*;

/// 帧通道满后仍须补发最终状态，不能停在已入队的旧帧。
#[test]
fn saturated_frame_channel_republishes_the_latest_state() {
    let mut terminal_under_test = terminal();
    for marker in ['A', 'B', 'C', 'D', 'E'] {
        terminal_under_test.vt_write(format!("\r{marker}").as_bytes());
        terminal_under_test.flush();
    }

    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(1);
    loop {
        if let Some((cells, _)) = terminal_under_test.receive_cell_data()
            && row_text_from_cells(&cells, 0).starts_with('E')
        {
            return;
        }
        assert!(
            std::time::Instant::now() < deadline,
            "通道饱和后必须补发最终状态"
        );
        std::thread::sleep(std::time::Duration::from_millis(2));
    }
}

/// OSC 8 hyperlink query (termux TerminalView.openLinkAt equivalent):
/// after writing an OSC 8 link, hyperlink_at returns the URI at the link
/// cells and None outside them.
#[test]
fn hyperlink_at_returns_uri_inside_link() {
    let mut terminal_under_test = terminal();
    terminal_under_test.vt_write(b"\x1b]8;;https://example.com\x1b\\Link\x1b]8;;\x1b\\");
    terminal_under_test.flush();
    let snap = terminal_under_test.take_snapshot();
    let row0 = snap.scrollback_length;
    // "Link" starts at col 0.
    let url = terminal_under_test
        .hyperlink_at(row0, 0)
        .expect("cell 0 has the link");
    assert_eq!(url, "https://example.com");
    // Last link cell still resolves; one past the link does not.
    assert!(
        terminal_under_test.hyperlink_at(row0, 3).is_some(),
        "col 3 is the last link char"
    );
    assert!(
        terminal_under_test.hyperlink_at(row0, 4).is_none(),
        "col 4 is past the link"
    );
}

/// 超长 URI 行为：实测上游约 2KB 以上即不建链接，本仓读路径
/// 只须保证不崩溃、返回 None（丢弃出声由日志承担）；上限内超长的
/// 重读分支是上游契约的防御性处理，随上游行为变化而生效。
/// 截断的链接指向错误地址，比没有链接更糟，故超限只丢不截。
#[test]
fn hyperlink_at_long_and_oversized_uris() {
    let mut terminal_under_test = terminal();
    // 1.5KiB：上游建链上限内的长 URI，必须完整返回。
    let long_uri = format!("https://example.com/{}", "a".repeat(1500));
    terminal_under_test.vt_write(format!("\x1b]8;;{long_uri}\x1b\\Link\x1b]8;;\x1b\\").as_bytes());
    terminal_under_test.flush();
    let snap = terminal_under_test.take_snapshot();
    let row0 = snap.scrollback_length;
    assert_eq!(
        terminal_under_test.hyperlink_at(row0, 0).as_deref(),
        Some(long_uri.as_str()),
        "within-cap URI must come back whole"
    );
    // 100KiB：上游不建链，直接丢弃（None）而非截断，不崩溃。
    // 先换行，使 "More" 落在绝对行 1，避免命中上一段遗留的链接。
    let huge_uri = format!("https://example.com/{}", "b".repeat(100 * 1024));
    terminal_under_test.vt_write(b"\r\n");
    terminal_under_test.vt_write(format!("\x1b]8;;{huge_uri}\x1b\\More\x1b]8;;\x1b\\").as_bytes());
    terminal_under_test.flush();
    assert!(
        terminal_under_test.hyperlink_at(1, 0).is_none(),
        "over-cap URI must be dropped, not truncated"
    );
}

/// Kitty 图形协议查询必须应答，且**回显被查询的图像 id**。
///
/// 客户端（如 yazi）用固定 id 探测能力：`ESC_Gi=278941603,s=1,v=1,a=q,t=d,f=24;AAAA`，
/// 并把应答解析成 `i=<id>` + `OK`/`ERROR`。应答若不带 id，客户端解析失败即判定
/// 「终端不支持 kitty 图像」，图片预览整体不显示。
#[test]
fn kitty_graphics_query_echoes_the_requested_image_id() {
    let mut terminal = terminal();
    terminal.pty_write(b"\x1b_Gi=278941603,s=1,v=1,a=q,t=d,f=24;AAAA\x1b\\");
    terminal.flush();
    let responses = terminal.drain_pty_write_responses();
    let response = responses
        .last()
        .map(|bytes| String::from_utf8_lossy(bytes).into_owned())
        .unwrap_or_default();
    assert!(
        response.contains("i=278941603"),
        "kitty 查询应答必须回显请求的图像 id，实际收到：{response:?}"
    );
    assert!(
        response.contains("OK"),
        "kitty 查询应答必须是 OK，实际收到：{response:?}"
    );
}

/// Kitty 图像协议像素回读（ghostty-android-terminal
/// kittyGraphicsPlacementAndPixelReadback 对等）：传输 1x1 红色 RGB 图像后，
/// take_kitty_graphics_image 返回 Some 且宽高为 1x1。
#[test]
fn kitty_graphics_transmit_returns_1x1_image() {
    let mut terminal_under_test = terminal();
    // 1x1 RGB 红色像素：base64("/wAA") = {0xff, 0x00, 0x00}。
    // 显式 i=1 指定图像 id（上游默认自动分配 id，不保证为 1）。
    // 注意：vt_write/pty_write 均为分片直透（上游跨调用重组，无 ST/SGR 提前闭合）；
    // 此处走 pty_write 以覆盖 LF→CRLF 文本路径。
    terminal_under_test.pty_write(b"\x1b_Ga=T,f=24,s=1,v=1,i=1;/wAA\x1b\\");
    terminal_under_test.flush();
    let image = terminal_under_test
        .take_kitty_graphics_image(1)
        .expect("image id 1 must exist after transmit");
    assert_eq!(image.width, 1);
    assert_eq!(image.height, 1);
    assert_eq!(
        image.data,
        vec![255, 0, 0, 255],
        "RGB 载荷归一化为不透明 RGBA"
    );
}

/// PNG 载荷经进程内解码器透出为 RGBA（1x1 红点 PNG，对标上游文档示例）。
#[test]
fn kitty_graphics_png_decodes_to_rgba() {
    let mut terminal = terminal();
    terminal.pty_write(
        b"\x1b_Ga=T,f=100,i=2;iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR4nGP4z8DwHwAFAAH/iZk9HQAAAABJRU5ErkJggg==\x1b\\",
    );
    terminal.flush();
    let image = terminal
        .take_kitty_graphics_image(2)
        .expect("PNG 图像解码后必须存在");
    assert_eq!((image.width, image.height), (1, 1));
    assert_eq!(image.data.len(), 4, "解码输出为 RGBA8");
}

/// PNG 调色板/16bit 灰度经解码器展开为 8bit RGBA（解码器 EXPAND/STRIP_16 路径回归）。
#[test]
fn kitty_graphics_png_palette_and_gray16_decode() {
    let mut terminal = terminal();
    terminal.pty_write(b"\x1b_Ga=T,f=100,i=11;iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAMAAAAoyzS7AAAAA1BMVEX/AAAZ4gk3AAAACklEQVR4nGNgAAAAAgABSK+kcQAAAABJRU5ErkJggg==\x1b\\");
    terminal.pty_write(b"\x1b_Ga=T,f=100,i=12;iVBORw0KGgoAAAANSUhEUgAAAAEAAAABEAAAAABq7kcWAAAAC0lEQVR4nGP4/x8AAwAB//wl3FEAAAAASUVORK5CYII=\x1b\\");
    terminal.flush();
    let palette = terminal
        .take_kitty_graphics_image(11)
        .expect("调色板 PNG 必须解码");
    assert_eq!((palette.width, palette.height), (1, 1));
    assert_eq!(palette.data, vec![255, 0, 0, 255]);
    let gray = terminal
        .take_kitty_graphics_image(12)
        .expect("16bit 灰度 PNG 必须解码");
    assert_eq!((gray.width, gray.height), (1, 1));
    assert_eq!(gray.data, vec![255, 255, 255, 255]);
}

/// Kitty 文件介质（`t=f`）：图像数据在文件里时必须能读出并显示。
///
/// 协议规定文件介质的载荷是 **base64 编码的文件路径**，`S` 是要从文件里读的
/// 字节数（缺省读整个文件）。`kitten icat` 等客户端用该介质传大图。
#[test]
fn kitty_graphics_file_medium_reads_the_image_file() {
    use base64::Engine;
    let dir = tempfile::tempdir().expect("创建临时目录");
    let path = dir.path().join("kitty-file");
    std::fs::write(&path, [0xffu8, 0x00, 0x00]).expect("写图像文件");
    let payload =
        base64::engine::general_purpose::STANDARD.encode(path.to_string_lossy().as_bytes());

    let mut terminal = terminal();
    terminal.pty_write(format!("\x1b_Ga=T,t=f,f=24,s=1,v=1,i=21;{payload}\x1b\\").as_bytes());
    terminal.flush();
    let image = terminal
        .take_kitty_graphics_image(21)
        .expect("文件介质传输后图像必须存在");
    assert_eq!((image.width, image.height), (1, 1));
    assert_eq!(
        image.data,
        vec![255, 0, 0, 255],
        "文件里的 RGB 载荷归一化为不透明 RGBA"
    );
}

/// Kitty 临时文件介质（`t=t`）：文件须位于终端的临时目录白名单内且名字含
/// `tty-graphics-protocol`，读取后由终端删除。
#[test]
fn kitty_graphics_temporary_file_medium_reads_the_image_file() {
    use base64::Engine;
    let file = tempfile::Builder::new()
        .prefix("tty-graphics-protocol-")
        .tempfile()
        .expect("创建临时文件");
    let path = file.path();
    std::fs::write(path, [0xffu8, 0x00, 0x00]).expect("写图像文件");
    let payload =
        base64::engine::general_purpose::STANDARD.encode(path.to_string_lossy().as_bytes());

    let mut terminal = terminal();
    terminal.pty_write(format!("\x1b_Ga=T,t=t,f=24,s=1,v=1,i=22;{payload}\x1b\\").as_bytes());
    terminal.flush();
    let image = terminal
        .take_kitty_graphics_image(22)
        .expect("临时文件介质传输后图像必须存在");
    assert_eq!((image.width, image.height), (1, 1));
    assert_eq!(
        image.data,
        vec![255, 0, 0, 255],
        "临时文件里的 RGB 载荷归一化为不透明 RGBA"
    );
}

/// 文件读取失败必须以**同一个**应答回报：文件不存在与文件短于客户端声明的
/// 字节数，答答应逐字节相同。
///
/// kitty 协议要求：文件读取失败（不存在、不可读、非普通文件、位于敏感目录、
/// 短于客户端声明）必须以同一个错误应答回报，不得让远端从应答差异里推断本地
/// 文件。kitty 客户端的能力探测把 `S` 写成文件路径的长度，必然触发短文件这条
/// 路径——终端按协议拒绝，不得替客户端改写声明值（那会让远端探知文件大小）。
#[test]
fn kitty_graphics_short_file_and_missing_file_answer_identically() {
    use base64::Engine;
    let dir = tempfile::tempdir().expect("创建临时目录");
    let path = dir.path().join("kitty-short");
    std::fs::write(&path, [0xffu8, 0x00, 0x00]).expect("写图像文件");
    let short_payload =
        base64::engine::general_purpose::STANDARD.encode(path.to_string_lossy().as_bytes());
    let missing_path = dir.path().join("kitty-absent");
    let missing_payload =
        base64::engine::general_purpose::STANDARD.encode(missing_path.to_string_lossy().as_bytes());

    let mut terminal = terminal();
    // S=60：文件只有 3 字节，短于客户端声明。
    terminal.pty_write(
        format!("\x1b_Ga=q,t=f,f=24,s=1,v=1,i=31,S=60;{short_payload}\x1b\\").as_bytes(),
    );
    terminal.flush();
    let short_response = terminal.drain_pty_write_responses();
    terminal
        .pty_write(format!("\x1b_Ga=q,t=f,f=24,s=1,v=1,i=32;{missing_payload}\x1b\\").as_bytes());
    terminal.flush();
    let missing_response = terminal.drain_pty_write_responses();
    std::fs::remove_file(&path).ok();

    let short_answer = short_response
        .last()
        .map(|bytes| String::from_utf8_lossy(bytes).into_owned())
        .unwrap_or_default();
    let missing_answer = missing_response
        .last()
        .map(|bytes| String::from_utf8_lossy(bytes).into_owned())
        .unwrap_or_default();
    assert!(
        short_answer.contains("EINVAL"),
        "短于声明字节数的文件必须回错误应答，实际收到：{short_answer:?}"
    );
    // 应答只允许在图像 id 上不同，错误内容必须逐字节相同。
    let error_part = |answer: &str| {
        answer
            .split_once(';')
            .map(|(_, error)| error)
            .unwrap_or(answer)
            .to_owned()
    };
    assert_eq!(
        error_part(&short_answer),
        error_part(&missing_answer),
        "文件读取失败的应答不得因失败原因不同而不同"
    );
    assert!(
        !short_answer.contains(&path.to_string_lossy().to_string()),
        "失败应答不得回显文件路径，实际收到：{short_answer:?}"
    );
}

/// Kitty 文件介质的 `S`/`O` 键语义：`S` 是**精确**读取的字节数，`O` 是起始偏移。
///
/// 协议规定 `S` 不是上限而是精确字节数（文件短于 `S` 即按读取失败处理），`O` 让
/// 终端从文件内的偏移处开始读。这里从偏移 1 读 3 字节，得到的是文件里的后三个
/// 字节，而不是文件开头的三个字节。
#[test]
fn kitty_graphics_file_medium_honors_size_and_offset() {
    use base64::Engine;
    let dir = tempfile::tempdir().expect("创建临时目录");
    let path = dir.path().join("kitty-offset");
    std::fs::write(&path, [0x11u8, 0x22, 0x33, 0x44]).expect("写图像文件");
    let payload =
        base64::engine::general_purpose::STANDARD.encode(path.to_string_lossy().as_bytes());

    let mut terminal = terminal();
    terminal
        .pty_write(format!("\x1b_Ga=T,t=f,f=24,s=1,v=1,i=23,S=3,O=1;{payload}\x1b\\").as_bytes());
    terminal.flush();
    let image = terminal
        .take_kitty_graphics_image(23)
        .expect("按偏移与精确字节数读取后图像必须存在");
    assert_eq!((image.width, image.height), (1, 1));
    assert_eq!(
        image.data,
        vec![0x22, 0x33, 0x44, 0xff],
        "O=1 起读 3 字节得到的是文件里的后三个字节"
    );
    std::fs::remove_file(&path).ok();
}

/// Kitty 放置采集：传输并显示 1x1 图像后，可见放置恰为 1 个且几何非零。
#[test]
fn kitty_placements_collects_visible_display() {
    let mut terminal = terminal();
    terminal.set_cell_pixel_size(8, 16);
    terminal.pty_write(b"\x1b_Ga=T,f=24,s=1,v=1,i=7;/wAA\x1b\\");
    terminal.flush();
    let placements = terminal.take_kitty_placements();
    assert_eq!(placements.len(), 1, "显示中的图像必须有可见放置");
    let placement = &placements[0];
    assert_eq!(placement.image_id, 7);
    assert!(placement.pixel_width > 0 && placement.pixel_height > 0);
    assert_eq!(placement.image_rgba.as_ref(), [255, 0, 0, 255]);
}

/// 新输出把行推入回滚时，普通放置的视口行号必须随之上移；渲染线程的
/// kitty 缓存键含回滚长度，正是依赖这个变化触发重查。
#[test]
fn kitty_placement_moves_up_when_output_scrolls() {
    let mut terminal = terminal();
    terminal.set_cell_pixel_size(8, 16);
    terminal.pty_write(b"\x1b[2;3H\x1b_Ga=T,f=24,s=1,v=1,i=13;/wAA\x1b\\");
    terminal.flush();
    let before = terminal
        .take_kitty_placements()
        .into_iter()
        .find(|placement| placement.image_id == 13)
        .expect("显示中的图像必须有可见放置");
    terminal.pty_write(b"\x1b[24;1H\n");
    terminal.flush();
    let after = terminal
        .take_kitty_placements()
        .into_iter()
        .find(|placement| placement.image_id == 13)
        .expect("上移一行后图像仍应可见");
    assert_eq!(
        after.viewport_row,
        before.viewport_row - 1,
        "回滚一行后放置必须上移一行"
    );
}

/// Kitty 虚拟放置（Unicode 占位符）：fg 颜色编码图像 id，underline 颜色编码
/// 放置 id，占位符后的组合记号编码图像行列；锚点 = 单元网格位置 − 图像行列。
#[test]
fn kitty_virtual_placement_uses_placeholder_diacritics() {
    let mut terminal = terminal();
    terminal.set_cell_pixel_size(8, 16);
    terminal.pty_write(b"\x1b_Ga=T,f=24,s=1,v=1,i=9,U=1,p=1,z=2,X=3,Y=4;/wAA\x1b\\");
    terminal.pty_write(
        "\x1b[2;3H\x1b[38;2;0;0;9m\x1b[58;2;0;0;1m\u{10EEEE}\u{0305}\u{0305}".as_bytes(),
    );
    terminal.flush();
    let placements = terminal.take_kitty_placements();
    assert_eq!(placements.len(), 1, "虚拟放置必须进入图像通道");
    let placement = &placements[0];
    assert_eq!(placement.image_id, 9);
    assert_eq!((placement.viewport_row, placement.viewport_col), (1, 2));
    assert_eq!((placement.pixel_width, placement.pixel_height), (1, 1));
    assert_eq!(
        (
            placement.z,
            placement.cell_offset_x,
            placement.cell_offset_y
        ),
        (2, 3, 4)
    );
    assert_eq!(placement.image_rgba.as_ref(), [255, 0, 0, 255]);
}

/// 占位符未设置 underline（放置 id 缺省为 0）时，回退到该图像的第一个虚拟放置，
/// 与上游行为一致：客户端漏编码放置 id 不应让整幅图消失。
#[test]
fn kitty_virtual_placement_without_underline_falls_back_to_first_placement() {
    let mut terminal = terminal();
    terminal.set_cell_pixel_size(8, 16);
    terminal.pty_write(b"\x1b_Ga=T,f=24,s=1,v=1,i=10,U=1,p=5;/wAA\x1b\\");
    terminal.pty_write("\x1b[2;3H\x1b[38;2;0;0;10m\u{10EEEE}\u{0305}\u{0305}".as_bytes());
    terminal.flush();
    let placements = terminal.take_kitty_placements();
    assert_eq!(placements.len(), 1, "缺省放置 id 必须回退到首个虚拟放置");
    assert_eq!(placements[0].image_id, 10);
    assert_eq!(
        (placements[0].viewport_row, placements[0].viewport_col),
        (1, 2)
    );
}

/// Kitty 载荷杂散 NUL 不得丢图（对标上游 kittyStrayNulInPayloadStillStores：
/// mpv --vo=kitty 在末块追加 NUL，ECMA-48 忽略控制字符，不得污染 base64）。
#[test]
fn kitty_graphics_stray_nul_still_stores() {
    let mut terminal = terminal();
    terminal.pty_write(b"\x1b_Ga=T,f=24,s=1,v=1,i=8;/wAA\x00\x1b\\");
    terminal.flush();
    let image = terminal
        .take_kitty_graphics_image(8)
        .expect("杂散 NUL 不得丢弃整图");
    assert_eq!((image.width, image.height), (1, 1));
    assert_eq!(image.data, vec![255, 0, 0, 255]);
}

/// 无图像时 Kitty 查询必须为空（对标 kittyGraphicsAbsentWhenNoImages：
/// 未传输返回空放置，未知 id 取图返回 None）。
#[test]
fn kitty_graphics_absent_when_no_images() {
    let terminal = terminal();
    assert!(
        terminal.take_kitty_placements().is_empty(),
        "no images must yield no placements"
    );
    assert_eq!(
        terminal
            .take_kitty_graphics_image(123)
            .map(|image| image.width),
        None,
        "unknown image id must yield None"
    );
}

// ── : cursor/row coordinate consistency (D1 deterministic leg) ──

/// shell 回显路径（提示文本 + 已键入字符，无换行）上报的光标 MUST 落在最后
/// 打印行的文本之后一列——即 CellData 各行所占据的同一行。此处一旦不符，
/// 光标会渲染在文本下方一行 ("输入指针出现在当前文本的正下",
/// emulator evidence). Mirrors the exact production path: VT loop auto-push →
/// build_cell_data → (cells, CursorInfo).
#[test]
fn cursor_matches_last_printed_row_after_echo_print() {
    let mut terminal_under_test = terminal();
    terminal_under_test.vt_write(b"$ ");
    terminal_under_test.vt_write(b"abc");
    terminal_under_test.flush();

    let (cells, cursor) = terminal_under_test
        .receive_cell_data()
        .expect("VT loop must auto-push cell data after writes");

    // Last row that has any non-blank cell = the row the text visibly sits on.
    let last_text_row = cells
        .iter()
        .filter(|cell| cell.codepoint != 0)
        .map(|cell| cell.row)
        .max()
        .expect("printed cells must exist");
    let last_text_col = cells
        .iter()
        .filter(|cell| cell.codepoint != 0 && cell.row == last_text_row)
        .map(|cell| cell.col)
        .max()
        .expect("printed cells must exist on the text row");

    assert_eq!(
        cursor.row, last_text_row,
        "cursor row must equal the last printed row (echo path)"
    );
    assert_eq!(
        cursor.col,
        last_text_col + 1,
        "cursor col must be right after the last printed cell"
    );
    assert!(cursor.visible, "cursor must be visible after print");
}

/// CUP-positioned cursor (the path CursorGeometryQuantifiedTest parks with)
/// must agree with the echo path — one coordinate contract for both.
#[test]
fn cursor_matches_cell_rows_after_cup_positioning() {
    let mut terminal_under_test = terminal();
    terminal_under_test.vt_write(b"\x1b[3;5H"); // CUP to row 2, col 4 (1-based)
    terminal_under_test.flush();

    let (cells, cursor) = terminal_under_test
        .receive_cell_data()
        .expect("VT loop must auto-push cell data after CUP");

    assert_eq!(cursor.row, 2, "CUP row must be 0-based viewport row 2");
    assert_eq!(cursor.col, 4, "CUP col must be 0-based viewport col 4");
    // The cell grid must have NO text (CUP only moves the cursor) — the
    // cursor must therefore sit on an empty row, not below any text.
    let text_rows: Vec<u32> = cells
        .iter()
        .filter(|cell| cell.codepoint != 0)
        .map(|cell| cell.row)
        .collect();
    assert!(
        text_rows.is_empty(),
        "CUP alone must not print cells, got rows {text_rows:?}"
    );
}

/// CursorInfo 必须携带产出本帧时的真实网格行列，与同批 CellData 同源。
/// resize 入队时会话侧原子缓存提前发布新尺寸，而 VT 尚未应用；渲染线程若读缓存，
/// 收缩帧会被 `build_row_ranges` 判空丢弃（IME 弹出/旋转必现）。
/// 断言默认网格下推送的 CursorInfo 与单元数一致。
#[test]
fn cursor_info_carries_producing_frame_grid_dimensions() {
    let mut terminal_under_test = terminal();
    terminal_under_test.vt_write(b"AB");
    terminal_under_test.flush();

    let (cells, cursor) = terminal_under_test
        .receive_cell_data()
        .expect("VT loop must auto-push cell data after writes");

    assert_eq!((cursor.rows, cursor.cols), (24, 80));
    assert_eq!(
        cells.len() as u32,
        cursor.rows * cursor.cols,
        "cells and CursorInfo grid must come from the same frame"
    );
}

/// resize 变体：resize 应用后推送的帧必须携带新网格，
/// 而不是入队瞬间缓存的尺寸与 VT 旧网格的混搭。
#[test]
fn cursor_info_carries_resized_grid_dimensions() {
    let mut terminal_under_test = terminal();
    terminal_under_test.resize(10, 40);
    terminal_under_test.flush();
    terminal_under_test.vt_write(b"XY");
    terminal_under_test.flush();

    let (cells, cursor) = terminal_under_test
        .receive_cell_data()
        .expect("VT loop must auto-push cell data after resize");

    assert_eq!((cursor.rows, cursor.cols), (10, 40));
    assert_eq!(
        cells.len() as u32,
        cursor.rows * cursor.cols,
        "cells and CursorInfo grid must come from the same frame"
    );
}

/// SGR31 红色必须到达渲染 CellData 的前景（设备渲染通路的精确复刻）。
/// 背景：设备像素验收曾报 SGR 无红色，后证实为测试 harness 自废武功
///（全局暂停与直接呈现互斥）；本测试在 host 复刻设备渲染输入
///（receive_cell_data），锁定颜色通路。
#[test]
fn sgr31_red_reaches_cell_data_foreground() {
    // 1) 默认配置：基线（默认主题）。
    let mut plain = terminal();
    plain.vt_write(b"\x1b[31mRED\x1b[0m");
    plain.flush();
    let (cells, _) = plain.receive_cell_data().expect("cell data");
    assert_sgr31_red_foreground(&cells, "默认配置 SGR31");

    // 2) 默认主题配置：复刻设备 apply_theme 后的精确状态。
    let default_theme = crate::terminal::theme::default_dark();
    let mut dracula = GhosttyTerminal::new_with_theme(24, 80, 1000, default_theme)
        .expect("default theme terminal");
    dracula.vt_write(b"\x1b[31mRED\x1b[0m");
    dracula.flush();
    let (cells, _) = dracula.receive_cell_data().expect("cell data");
    assert_sgr31_red_foreground(&cells, "Dracula 配置 SGR31");
}

/// FFI 默认主题下 SGR31 必须到达 CellData 前景。
/// 背景：设备像素验收曾报 SGR 无显色，后证实为 harness 问题；
/// 复刻其精确主题与几何，锁定颜色通路。
/// 回归钉：取光标查询不得触发整网格抽取。
///
/// `Query::RenderCursor` 只要 (row, col) 与可见性，却曾走 `build_cell_data`：
/// 逐行抽完整张网格、把结果写进跨帧行缓存并 `snapshot.clean()` 标为已消费。
/// 为两个整数做这件事，在大网格上是数百次跨语言调用，还让「取光标」与「渲染」
/// 共用同一份可变抽取状态。
///
/// 判别力：`take_cache_stats()` 是抽取路径自己记的账。先渲染若干帧让行缓存进入
/// 可复用态并确认账上确有复用（同时确认该账只在抽取路径上记），再清零、取一次
/// 光标——账上仍须为零；若它偷偷抽了网格，复用计数必然非零。
#[test]
fn render_cursor_query_does_not_extract_cells() {
    let mut terminal_under_test = terminal();
    terminal_under_test.vt_write(b"line one\r\nline two\r\nline three\r\n");
    terminal_under_test.flush();
    let _ = terminal_under_test.receive_cell_data().expect("warm frame");
    // 只改末行：前两行在缓存里可复用，于是本次构建必然记下非零复用——
    // 这同时确认该账确实只在抽取路径上记，且在此状态下复用可观测。
    terminal_under_test.vt_write(b"\x1b[3;1Hchanged\r\n");
    terminal_under_test.flush();
    let _ = terminal_under_test
        .receive_cell_data()
        .expect("second frame");
    let (reused_before, _) = terminal_under_test.take_cache_stats();
    assert!(
        reused_before > 0,
        "前置条件：只改末行时前两行应复用（实得 {reused_before}）"
    );

    terminal_under_test.take_cache_stats();
    let _ = terminal_under_test.render_cursor();

    let (reused_after, extracted_after) = terminal_under_test.take_cache_stats();
    assert_eq!(
        (reused_after, extracted_after),
        (0, 0),
        "取光标只该更新快照，不得触发任何网格抽取"
    );
}

/// 回归钉：取光标查询不得用写死的默认色污染跨帧行缓存。
///
/// 污染成立需要三件事同时发生（第一版用例只满足了两件，故还原成写死色仍全绿）：
/// 1. 该行在取光标那一刻**仍是脏的**——写完不 `flush()`，让写入留在队列里；
/// 2. 写入与查询落在**同一批**——批处理里 `drain_queries` 先于
///    `refresh_cell_data`，二者共用同一个 `extractors`；
/// 3. `build_cell_data` 用的是**主题默认色**——写死色会让此刻判脏的行以那组颜色
///    存入 `row_cache`，又被 `snapshot.clean()` 标为已消费，下一步渲染看到
///    「全部干净」便直接复用，整行错色直到它再次变化。
#[test]
fn render_cursor_query_does_not_poison_the_row_cache_with_literal_colors() {
    let theme = crate::terminal::theme::by_name("tokyo_night").expect("tokyo_night theme");
    let foreground = GhosttyTerminal::color_to_float(theme.palette.foreground);
    let background = GhosttyTerminal::color_to_float(theme.palette.background);
    // 先坐实前提：这个主题确实不同于写死色，否则本例无判别力。
    assert_ne!(
        (background[0], background[1], background[2]),
        (0.0, 0.0, 0.0),
        "本例要求主题背景非纯黑，否则写死色与真实色无法区分"
    );
    assert_ne!(
        (foreground[0], foreground[1], foreground[2]),
        (1.0, 1.0, 1.0),
        "本例要求主题前景非纯白，否则写死色与真实色无法区分"
    );

    let mut terminal_under_test =
        GhosttyTerminal::new_with_theme(24, 80, 1000, theme).expect("theme terminal");
    // 先出一帧，让缓存与快照进入「已消费」状态。
    terminal_under_test.vt_write(b"seed\r\n");
    terminal_under_test.flush();
    let _ = terminal_under_test
        .receive_cell_data()
        .expect("seed cell data");

    // 写入刻意不 flush：命令留在队列里，与随后的取光标查询同批处理，
    // 且此刻该行仍是脏的——这正是会被写死色重新提取并存入缓存的那一行。
    terminal_under_test.vt_write(b"POISONED\r\n");
    let _ = terminal_under_test.render_cursor();
    // 随后这一帧走 refresh_cell_data：该行已被标记为已消费，故**复用**缓存里那份
    // （可能已被写死色污染的）单元，而不是重新提取。这就是缺陷的可观测结果。
    terminal_under_test.flush();

    let (cells, _) = terminal_under_test.receive_cell_data().expect("cell data");
    let cell = cells
        .iter()
        .find(|cell| cell.codepoint == 'P' as u32)
        .expect("P cell present after cursor query");
    assert_color(
        cell.background,
        background,
        "取光标查询后单元格背景须仍是主题默认背景",
    );
    assert_color(
        cell.foreground,
        foreground,
        "取光标查询后单元格前景须仍是主题默认前景",
    );
}

#[test]
fn sgr31_default_theme_reaches_foreground() {
    let mut terminal_under_test =
        GhosttyTerminal::new_with_theme(44, 48, 1000, crate::terminal::theme::default_dark())
            .expect("default terminal");
    terminal_under_test.vt_write(b"\x1b[31mRED\x1b[0m");
    terminal_under_test.flush();
    let (cells, _) = terminal_under_test.receive_cell_data().expect("cell data");
    let red_cell = cells
        .iter()
        .find(|cell| cell.codepoint == 'R' as u32)
        .expect("R cell present");
    assert_color(
        red_cell.foreground,
        default_ansi(1),
        "SGR31 前景须取 ANSI 槽位 1",
    );
}

/// 同一终端连收三包同文本不同色时，三行必须各就其位各带其色。
/// 背景：设备上三包同文本（EEEEEEEE）蓝/红/绿轮换时恒只有两行上屏，
/// 第三包系统性缺席；先在 host 判定是构建层去重还是设备推送时序。
#[test]
fn sgr_same_text_tricolor_rows_all_present() {
    let mut terminal =
        GhosttyTerminal::new_with_theme(24, 80, 1000, crate::terminal::theme::default_dark())
            .expect("default terminal");
    for code in [34u8, 31, 32] {
        terminal.vt_write(format!("\x1b[{code}mEEEEEEEE\x1b[0m\r\n").as_bytes());
    }
    terminal.flush();
    let (cells, _) = terminal.receive_cell_data().expect("cell data");
    // 三行 E 首字符：行0蓝、行1红、行2绿（默认主题槽位色）。
    for (row, ansi_index) in [(0, 4), (1, 1), (2, 2)] {
        let cell = cells
            .iter()
            .find(|cell| cell.row == row && cell.col == 0)
            .unwrap_or_else(|| panic!("行{row}首格缺席"));
        assert_eq!(
            cell.codepoint, 'E' as u32,
            "行{row}首格须为 E，实际 {}",
            cell.codepoint
        );
        assert_color(cell.foreground, default_ansi(ansi_index), "行首前景");
    }
}

/// 默认主题下 SGR31/32/34 三色必须各自到达 CellData 前景。
/// 背景：设备像素验收红绿显色而蓝计数为 0；先在 host 判定蓝是映射问题
/// 还是设备侧问题（计数/呈现），再定跟进方向。
#[test]
fn sgr_tricolor_default_theme_reaches_foreground() {
    // （码，标记字符，期望 ANSI 槽位）
    let cases: [(u8, u8, usize); 3] = [(31, b'R', 1), (32, b'G', 2), (34, b'B', 4)];
    for (code, marker, ansi_index) in cases {
        let mut terminal =
            GhosttyTerminal::new_with_theme(24, 80, 1000, crate::terminal::theme::default_dark())
                .expect("default terminal");
        terminal.vt_write(format!("\x1b[{code}m{}\x1b[0m", marker as char).as_bytes());
        terminal.flush();
        let (cells, _) = terminal.receive_cell_data().expect("cell data");
        let marked = marked_cell(&cells, marker);
        assert_color(
            marked.foreground,
            default_ansi(ansi_index),
            &format!("SGR{code} 前景"),
        );
    }
}

/// 256 色 SGR（`38;5;n` / `48;5;n`）必须落到 xterm 标准色：6×6×6 色立方与
/// 24 级灰阶由上游调色板生成，本仓不得另造公式。取色立方顶点、灰阶首格与
/// 一个中间绿做外部基准，证明 16..255 槽位真的进了 CellData。
#[test]
fn palette_256_cube_and_grayscale_match_xterm() {
    let mut terminal =
        GhosttyTerminal::new_with_theme(24, 80, 1000, crate::terminal::theme::default_dark())
            .expect("default terminal");
    terminal.vt_write(b"\x1b[38;5;196mA\x1b[38;5;21mB\x1b[38;5;231mC\x1b[38;5;232mD\x1b[48;5;46mE");
    terminal.flush();
    let (cells, _) = terminal.receive_cell_data().expect("cell data");
    let cell = |marker: char| {
        cells
            .iter()
            .find(|cell| cell.codepoint == marker as u32)
            .unwrap_or_else(|| panic!("{marker} 单元格缺席"))
    };
    let rgb = |color: [f32; 4]| {
        [
            (color[0] * 255.0).round() as u8,
            (color[1] * 255.0).round() as u8,
            (color[2] * 255.0).round() as u8,
        ]
    };
    // xterm：196=(255,0,0)，21=(0,0,255)，231=(255,255,255)，
    // 232=灰阶首格 8，46=(0,255,0)。
    for (marker, expected) in [
        ('A', [255, 0, 0]),
        ('B', [0, 0, 255]),
        ('C', [255, 255, 255]),
        ('D', [8, 8, 8]),
    ] {
        assert_eq!(rgb(cell(marker).foreground), expected, "{marker} 前景");
    }
    assert_eq!(rgb(cell('E').background), [0, 255, 0], "E 背景");
}

/// SGR 3/4/9（斜体/下划线/删除线）必须到达快照层与 CellData 标志位。
/// 背景：用户报告斜体等文本缺失无法显示；颜色有 VT 端到端覆盖
///（sgr_tricolor_mocha_reaches_foreground），样式属性只有 pack_style_flags
/// 单元测试（Style 结构体→bits），无 VT 解析端到端。
#[test]
fn sgr_style_attributes_reach_snapshot_and_cell_data() {
    use crate::terminal::ghostty_terminal::cell_flags;
    // （SGR 码，标记字符）
    for (code, marker) in [(3u8, b'I'), (4u8, b'U'), (9u8, b'S')] {
        let flag_bit = match code {
            3 => cell_flags::ITALIC,
            4 => cell_flags::UNDERLINE,
            _ => cell_flags::STRIKETHROUGH,
        };
        let mut styled = terminal();
        styled.vt_write(format!("\x1b[{code}m{}\x1b[0m", marker as char).as_bytes());
        styled.flush();
        let (cells, _) = styled.receive_cell_data().expect("cell data");
        let marked = marked_cell(&cells, marker);
        assert_eq!(
            (marked.flags >> flag_bit) & 1,
            1,
            "SGR{code} must set CellData flag bit {flag_bit}"
        );
        let snapshot = styled.take_snapshot();
        let snap_cell = snapshot
            .cells
            .iter()
            .find(|cell| cell.codepoint == marker as u32)
            .expect("marked snapshot cell present");
        let snapshot_set = match code {
            3 => snap_cell.italic,
            4 => snap_cell.underline,
            _ => snap_cell.strikethrough,
        };
        assert!(snapshot_set, "SGR{code} must set snapshot style attribute");
    }
}

/// SGR 8 隐藏状态必须同时到达快照与生产 CellData。
#[test]
fn sgr8_conceal_reaches_snapshot_and_cell_data() {
    use crate::terminal::ghostty_terminal::cell_flags;
    let mut terminal_under_test = terminal();
    terminal_under_test.vt_write(b"\x1b[8mH");
    terminal_under_test.flush();
    let (cells, _) = terminal_under_test.receive_cell_data().expect("cell data");
    let marked = marked_cell(&cells, b'H');
    assert_eq!(
        (marked.flags >> cell_flags::CONCEAL) & 1,
        1,
        "SGR 8 must set the CellData conceal bit"
    );
    let snapshot = terminal_under_test.take_snapshot();
    assert!(
        snapshot
            .cells
            .iter()
            .find(|cell| cell.codepoint == 'H' as u32)
            .expect("H snapshot cell present")
            .hidden,
        "SGR 8 must set the snapshot hidden flag"
    );
}

/// SGR 5 闪烁必须到达渲染层且不污染下划线位（对标上游
/// sgrBlinkAndUnderlineStyleAreDisjoint：两字段曾共用一位，下划线 cell
/// 闪烁、闪烁 cell 画下划线）。
#[test]
fn blink_reaches_cell_data_without_underline_pollution() {
    use crate::terminal::ghostty_terminal::cell_flags;
    // 下划线不闪。
    let mut underlined = terminal();
    underlined.vt_write(b"\x1b[4mA");
    underlined.flush();
    let (underlined_cells, _) = underlined.receive_cell_data().expect("cell data");
    let underlined_cell = underlined_cells
        .iter()
        .find(|cell| cell.codepoint == 'A' as u32)
        .expect("A present");
    assert_eq!((underlined_cell.flags >> cell_flags::UNDERLINE) & 1, 1);
    assert_eq!(
        (underlined_cell.flags >> cell_flags::BLINK) & 1,
        0,
        "underline must not set blink"
    );
    // 闪烁不带下划线。
    let mut blinking = terminal();
    blinking.vt_write(b"\x1b[5mB");
    blinking.flush();
    let (blinking_cells, _) = blinking.receive_cell_data().expect("cell data");
    let blinking_cell = blinking_cells
        .iter()
        .find(|cell| cell.codepoint == 'B' as u32)
        .expect("B present");
    assert_eq!((blinking_cell.flags >> cell_flags::BLINK) & 1, 1);
    assert_eq!(
        (blinking_cell.flags >> cell_flags::UNDERLINE) & 1,
        0,
        "blink must not set underline"
    );
    let snapshot = blinking.take_snapshot();
    let snap_cell = snapshot
        .cells
        .iter()
        .find(|cell| cell.codepoint == 'B' as u32)
        .expect("snapshot B present");
    assert!(snap_cell.blink, "SGR 5 must set snapshot blink");
    // 两者叠加各自完整。
    let mut both = terminal();
    both.vt_write(b"\x1b[4;5mC");
    both.flush();
    let (both_cells, _) = both.receive_cell_data().expect("cell data");
    let both_cell = both_cells
        .iter()
        .find(|cell| cell.codepoint == 'C' as u32)
        .expect("C present");
    assert_eq!((both_cell.flags >> cell_flags::BLINK) & 1, 1);
    assert_eq!((both_cell.flags >> cell_flags::UNDERLINE) & 1, 1);
}

/// SGR 4:x 下划线形状必须到达渲染层（对标 sgrUnderlineStyles：单/双/卷/点/虚
/// 均置下划线位，仅双线另置双下划线位）。
#[test]
fn underline_styles_reach_cell_data_and_snapshot() {
    use crate::terminal::ghostty_terminal::cell_flags;
    for (sequence, marker, double) in [
        ("4:1", b'A', false),
        ("4:2", b'B', true),
        ("4:3", b'C', false),
        ("4:4", b'D', false),
        ("4:5", b'E', false),
    ] {
        let mut styled = terminal();
        styled.vt_write(format!("\x1b[{sequence}m{}", marker as char).as_bytes());
        styled.flush();
        let (cells, _) = styled.receive_cell_data().expect("cell data");
        let marked = marked_cell(&cells, marker);
        assert_eq!(
            (marked.flags >> cell_flags::UNDERLINE) & 1,
            1,
            "SGR {sequence} must set UNDERLINE bit"
        );
        assert_eq!(
            (marked.flags >> cell_flags::DOUBLE_UNDERLINE) & 1,
            u32::from(double),
            "SGR {sequence} double bit must be {double}"
        );
        let snapshot = styled.take_snapshot();
        let snap_cell = snapshot
            .cells
            .iter()
            .find(|cell| cell.codepoint == marker as u32)
            .expect("marked snapshot cell present");
        assert!(
            snap_cell.underline,
            "SGR {sequence} must set snapshot underline"
        );
        assert_eq!(
            snap_cell.double_underline, double,
            "SGR {sequence} snapshot double must be {double}"
        );
    }
}

/// 选择端点翻转必须返回同范围文本：拖动手柄越过锚点时调用方按
/// 触摸顺序传递端点（start > end），不得返回空串或 panic
///（对标 selectionDragAcrossAnchorFlips 的端点重排语义）。
#[test]
fn selection_text_flipped_endpoints() {
    let mut terminal_under_test = terminal();
    terminal_under_test.vt_write(b"hello world");
    terminal_under_test.flush();
    let snap = terminal_under_test.take_snapshot();
    let row0 = snap.scrollback_length;
    let forward = terminal_under_test.selection_text((row0, 0), (row0, 4));
    assert_eq!(
        forward, "hello",
        "forward selection baseline (got {forward:?})"
    );
    let flipped = terminal_under_test.selection_text((row0, 4), (row0, 0));
    assert_eq!(
        flipped, "hello",
        "flipped endpoints must yield the same range (got {flipped:?})"
    );
}

/// 切换主题必须重着色**已有**单元，而不只是新写入的单元。
#[test]
fn theme_change_recolors_existing_cells() {
    let mut terminal_under_test = terminal();
    terminal_under_test.vt_write(b"hello");
    terminal_under_test.flush();
    let (before, _) = terminal_under_test
        .receive_cell_data()
        .expect("first frame");
    let before_bg = before[0].background;

    terminal_under_test.set_theme(
        crate::terminal::theme::by_name(crate::terminal::theme::default_light_name())
            .expect("default light"),
    );
    terminal_under_test.flush();
    let (after, _) = terminal_under_test
        .receive_cell_data()
        .expect("frame after theme change");
    assert_ne!(
        after[0].background, before_bg,
        "existing cells must be recoloured by SetTheme"
    );
}
