use super::*;

/// 拖动手柄只移动被抓端点：锚点固定，另一端点跟随落点
///（对标 selectionDragMovesGrabbedEndpoint：先拖尾端点 0→8，再拖首端点 0→6）。
#[test]
fn selection_text_grabbed_endpoint_moves() {
    let mut terminal_under_test = terminal();
    terminal_under_test.vt_write(b"hello world");
    terminal_under_test.flush();
    let snap = terminal_under_test.take_snapshot();
    let row = snap.scrollback_length;
    let extended = terminal_under_test.selection_text((row, 0), (row, 8));
    assert_eq!(
        extended, "hello wor",
        "drag end handle must extend (got {extended:?})"
    );
    let shrunk = terminal_under_test.selection_text((row, 6), (row, 8));
    assert_eq!(
        shrunk, "wor",
        "drag start handle must shrink (got {shrunk:?})"
    );
}

/// 选中文本滚入回滚后同端点仍取回原文：端点为绝对行号，不随视口漂移
///（对标 selectionTracksTextIntoScrollback）。
#[test]
fn selection_text_tracks_scrolled_content() {
    let mut terminal_under_test = GhosttyTerminal::new(4, 20, 100).expect("terminal");
    terminal_under_test.vt_write(b"alpha\n");
    terminal_under_test.flush();
    let first = terminal_under_test.selection_text((0, 0), (0, 4));
    assert_eq!(first, "alpha", "baseline selection (got {first:?})");
    for filler in 0..8 {
        terminal_under_test.vt_write(format!("filler{filler}\n").as_bytes());
    }
    terminal_under_test.flush();
    assert!(
        terminal_under_test.scrollback_length() > 0,
        "content must have scrolled"
    );
    let tracked = terminal_under_test.selection_text((0, 0), (0, 4));
    assert_eq!(
        tracked, "alpha",
        "selection must track into scrollback (got {tracked:?})"
    );
}

/// OSC 2 会话标题必须可读（对标 titleChangeEventAndValue 的值断言；
/// 我方无标题事件通道且 DESIGN 未声明，只断言值本身）。
#[test]
fn osc2_title_readable() {
    let mut terminal_under_test = terminal();
    terminal_under_test.vt_write(b"\x1b]2;my title\x07");
    terminal_under_test.flush();
    assert_eq!(
        terminal_under_test.title(),
        "my title",
        "OSC 2 title must be readable (got {:?})",
        terminal_under_test.title()
    );
}

/// DECSCUSR 程序光标样式必须到达快照（对标 cursorStyleDefaultApplies；
/// 空章节 from Termux testSetCursorStyle 的正文）。
#[test]
fn decscusr_cursor_style_reaches_snapshot() {
    use crate::terminal::ghostty_terminal::CursorStyle;
    // （DECSCUSR 参数，期望快照样式）
    for (param, expected) in [
        (2u8, CursorStyle::Block),
        (3u8, CursorStyle::Underline),
        (5u8, CursorStyle::Bar),
        (0u8, CursorStyle::Block),
    ] {
        let mut styled = terminal();
        styled.vt_write(format!("\x1b[{param} q").as_bytes());
        styled.flush();
        let snapshot = styled.take_snapshot();
        assert_eq!(
            snapshot.cursor_style, expected,
            "DECSCUSR {param} must yield {expected:?}"
        );
    }
    // 程序覆盖后重置必须回到默认（对标 programCursorStyleOverridesDefaultUntilReset）。
    let mut reset = terminal();
    reset.vt_write(b"\x1b[5 q");
    reset.flush();
    assert_eq!(reset.take_snapshot().cursor_style, CursorStyle::Bar);
    reset.vt_write(b"\x1b[0 q");
    reset.flush();
    assert_eq!(
        reset.take_snapshot().cursor_style,
        CursorStyle::Block,
        "reset must restore default"
    );
}

/// 回滚上限透传：建会参数必须约束回滚深度（设备 instrumented 测试的 host 复刻）。
/// 上游按页粒度修剪（文档：实际值通常高于配置几十到一百行），故断言
/// 上限生效（远小于无约束时的全部保留），而非精确等于配置值。
#[test]
fn scrollback_cap_is_honored() {
    let mut terminal = GhosttyTerminal::new(24, 80, 10).expect("terminal");
    for index in 0..2000 {
        terminal.vt_write(format!("CAP_{index:04}\r\n").as_bytes());
    }
    terminal.flush();
    let depth = terminal.scrollback_length();
    assert!(
        depth < 2000,
        "回滚上限必须生效（2000 行输入不得全保留）, 实际={depth}",
    );
}

/// 回滚区必须「旧行按顺序进入、新行留在底部」，不能只断言深度。
#[test]
fn scrollback_keeps_order_and_newest_line_stays_visible() {
    let mut terminal = GhosttyTerminal::new(24, 80, 5_000).expect("terminal");
    for index in 0..500u32 {
        terminal.vt_write(format!("SEQ_{index:04}\r\n").as_bytes());
    }
    terminal.flush();

    let depth = terminal.scrollback_length();
    assert!(depth >= 400, "回滚区应保留大部分输入行, 实际={depth}");

    // 回滚行必须按写入顺序单调递增。
    let mut previous = String::new();
    for row in 0..depth.min(400) {
        let text = terminal
            .read_line_text(row)
            .unwrap_or_else(|| panic!("回滚第 {row} 行必须可读"));
        let Some(marker) = text.trim().strip_prefix("SEQ_") else {
            panic!("回滚第 {row} 行缺少标记, 实际={text:?}");
        };
        if row > 0 {
            assert!(
                marker > previous.as_str(),
                "回滚行必须按写入顺序递增, 第 {row} 行={marker} 上一行={previous}",
            );
        }
        previous = marker.to_string();
    }

    // 最新一行必须出现在可视区（底部），而不是被推进回滚后丢失。
    let last_marker = format!("SEQ_{:04}", 499);
    assert!(
        terminal.read_visible_text().contains(&last_marker),
        "最新一行必须留在可视区底部, 可视区未包含 {last_marker}",
    );
}

/// 深缓冲不坍缩（对标 scrollbackHonorsConfiguredLineCount：上游 max_scrollback
/// 是字节预算而非行数，预算过小会被忽略致历史坍缩；本地建会参数透传后，
/// 2 万行配置喂 2.5 万行，历史必须达到万行量级而非几百行。页粒度修剪有
/// 百行级波动，不断言精确值）。
#[test]
fn scrollback_deep_buffer_does_not_collapse() {
    let mut terminal = GhosttyTerminal::new(24, 80, 20_000).expect("terminal");
    // 与 scroll_viewport_delta_scrolls_cell_data 同口径：vt_write 直透 LF，
    // 上游按 LF 语义换行推进 scrollback（\r\n 在该路径行为不同）。
    // 命令通道有界（1024）：大批量写入必须分批 flush，否则 try_send
    // 静默丢弃，scrollback 恒 0。
    for chunk in (0..25_000)
        .map(|index| format!("DEEP_{index:05}\n"))
        .collect::<Vec<_>>()
        .chunks(500)
    {
        for line in chunk {
            terminal.vt_write(line.as_bytes());
        }
        terminal.flush();
    }
    let depth = terminal.scrollback_length();
    assert!(
        depth >= 19_000,
        "深缓冲历史不得坍缩（2 万行配置至少保留万行量级）, 实际={depth}",
    );
}

/// SGR 7 反白必须到达渲染层（对标 inverseIsResolvedNatively；本仓反白在
/// cell_builder 做前景背景互换，VT 层只断言标志位到达）。
#[test]
fn inverse_reaches_cell_data_and_snapshot() {
    use crate::terminal::ghostty_terminal::cell_flags;
    let mut inverse = terminal();
    inverse.vt_write(b"\x1b[7mX");
    inverse.flush();
    let (cells, _) = inverse.receive_cell_data().expect("cell data");
    let marked = cells
        .iter()
        .find(|cell| cell.codepoint == 'X' as u32)
        .expect("X cell present");
    assert_eq!(
        (marked.flags >> cell_flags::REVERSE) & 1,
        1,
        "SGR 7 must set CellData REVERSE bit"
    );
    let snapshot = inverse.take_snapshot();
    let snap_cell = snapshot
        .cells
        .iter()
        .find(|cell| cell.codepoint == 'X' as u32)
        .expect("snapshot X present");
    assert!(snap_cell.reverse, "SGR 7 must set snapshot reverse flag");
}

/// 组合重音必须以 grapheme 形式到达 VT 层（对标 graphemeClusterCombiningMark
/// 的文本部分：基码点 e 在主格，U+0301 进 grapheme_extra/快照 graphemes）。
#[test]
fn combining_mark_reaches_grapheme_channel() {
    let mut clustered = terminal();
    clustered.vt_write("e\u{301}x".as_bytes());
    clustered.flush();
    let (cells, _) = clustered.receive_cell_data().expect("cell data");
    let base = cells
        .iter()
        .find(|cell| cell.codepoint == 'e' as u32)
        .expect("base e present");
    assert_eq!(
        base.grapheme_extra[0], 0x301,
        "combining acute must ride grapheme_extra[0]"
    );
    let snapshot = clustered.take_snapshot();
    let snap_cell = snapshot
        .cells
        .iter()
        .find(|cell| cell.codepoint == 'e' as u32)
        .expect("snapshot e present");
    assert!(
        snap_cell.graphemes.contains(&0x301),
        "snapshot graphemes must contain U+0301"
    );
}

#[test]
fn vt_write_sanitize_keeps_high_bytes_drops_nul() {
    // NUL 必须剔除（mpv --vo=kitty 每帧追加一个游离 NUL）；0xF8–0xFF 必须原样保留
    // （Kitty m=1 直接 RGB 载荷里的合法数据，旧写法静默替换为空格）。
    assert_eq!(
        sanitize_vt_input(&[0x41, 0x00, 0xF8, 0xFF, 0x42]),
        vec![0x41, 0xF8, 0xFF, 0x42]
    );
    assert_eq!(sanitize_vt_input(b"ABC"), b"ABC");
    assert!(sanitize_vt_input(&[0x00, 0x00]).is_empty());
}

#[test]
fn deliver_terminate_reaches_a_full_queue_once_the_consumer_drains() {
    // 队列满是常态：VT 线程忙时命令堆积到上限，析构时的 Terminate 若只
    // try_send 一次就被丢弃，线程只能等信道彻底排空才退出，join 随即超时。
    //
    // 消费端刻意延迟后再排空：只有「满队列 + 有界等待」才能送达，
    // 单次 try_send 在此必失败——该用例因此能区分两种实现。
    let (cmd_tx, cmd_rx) = flume::bounded::<Command>(1);
    cmd_tx
        .send(Command::FlushAck(flume::bounded::<()>(1).0))
        .expect("queue not full");
    let consumer = std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_millis(20));
        let _ = cmd_rx.recv_timeout(std::time::Duration::from_secs(5));
    });
    assert!(
        super::super::deliver_terminate(&cmd_tx, std::time::Duration::from_secs(5)),
        "Terminate must reach the VT thread even when the queue was full at drop time"
    );
    consumer.join().expect("consumer thread panicked");
}

#[test]
fn deliver_terminate_gives_up_instead_of_blocking_on_a_stuck_consumer() {
    // 对端永不取：等待必须是有界的，否则卡死的 VT 线程会把析构一并卡住。
    let (cmd_tx, _cmd_rx) = flume::bounded::<Command>(1);
    cmd_tx
        .send(Command::FlushAck(flume::bounded::<()>(1).0))
        .expect("queue not full");
    let start = std::time::Instant::now();
    assert!(
        !super::super::deliver_terminate(&cmd_tx, super::super::TERMINATE_SEND_TIMEOUT),
        "a consumer that never drains must not make delivery succeed"
    );
    assert!(
        start.elapsed() < std::time::Duration::from_secs(1),
        "delivery waited {:?} — drop would block on a dead VT thread",
        start.elapsed()
    );
}

// ── 行级增量抽取（grid-extraction-incremental） ───────────────────────────
//
// 行级增量与全量重建产出逐字节相同，内容断言无法区分二者（增量正确但从未
// 命中时用例照样全绿）。故本组用例的判别力来自行级计数：断言本次抽取确有
// 行走复制路径，且复制结果与逐格抽取逐字节相同。计数按终端实例隔离
// （取自 `GhosttyTerminal` 而非进程全局）——`cargo test` 并行执行，
// 全局计数会被同进程其他终端的抽取淹没，「本帧复用 0 行」将不可判别。

#[test]
fn row_extraction_reuses_unchanged_rows_across_frames() {
    let mut terminal_under_test = terminal();
    // 写满多行，使首帧每行都有内容（全部判脏，建立缓存）。
    for row in 0..20 {
        terminal_under_test.vt_write(format!("\x1b[{};1Hline{row}", row + 1).as_bytes());
    }
    terminal_under_test.flush();
    let _ = next_cell_frame(&terminal_under_test);

    let (_, extracted_before) = terminal_under_test.take_cache_stats();
    assert!(
        extracted_before > 0,
        "首帧必须逐格抽取建立缓存（缓存不存在时无行可复用）"
    );

    // 只改第 0 行，其余不动。
    terminal_under_test.vt_write(b"\x1b[1;1HX");
    terminal_under_test.flush();
    let cells = next_cell_frame(&terminal_under_test);
    let (reused_after, extracted_after) = terminal_under_test.take_cache_stats();

    assert!(
        reused_after > 0,
        "只改一行时其余行 MUST 走复制路径，实测复用 {reused_after} 行、逐格 {extracted_after} 行"
    );
    assert!(
        extracted_after < reused_after,
        "逐格行数({extracted_after}) MUST 少于复制行数({reused_after})，\
         否则行级判定没有生效（等价于全量重建）"
    );
    // 复制的那一行内容必须仍是旧值——行号 1 未被本次写入触及。
    assert_eq!(
        row_text_from_cells(&cells, 1),
        "line1",
        "未被改动的行经复制路径后内容必须与写入时一致"
    );
    assert_eq!(
        row_text_from_cells(&cells, 0),
        "Xine0",
        "被改动的行必须走逐格抽取并反映新内容"
    );
}

#[test]
fn row_extraction_result_matches_full_rebuild_byte_for_byte() {
    let mut terminal_under_test = terminal();
    for row in 0..12 {
        terminal_under_test.vt_write(format!("\x1b[{};1Hcontent{row}", row + 1).as_bytes());
    }
    terminal_under_test.flush();
    let _ = next_cell_frame(&terminal_under_test);

    // 改若干行（含宽字符行与颜色行，覆盖样式与宽度两条易错路径）。
    terminal_under_test.vt_write(b"\x1b[1;1HAA\x1b[31mred");
    terminal_under_test.vt_write(b"\x1b[5;1H\x1b[32mgreen\x1b[0m");
    terminal_under_test.vt_write("\x1b[7;1H中文宽字符".as_bytes());
    terminal_under_test.flush();
    let incremental = next_cell_frame(&terminal_under_test);
    let (reused, extracted) = terminal_under_test.take_cache_stats();
    assert!(reused > 0, "本帧 MUST 命中行复制路径才有对照意义");

    // 强制全量重建：`reset` 清空网格与缓存，再重放同一段输出。
    let mut fresh = terminal();
    for row in 0..12 {
        fresh.vt_write(format!("\x1b[{};1Hcontent{row}", row + 1).as_bytes());
    }
    fresh.flush();
    let _ = next_cell_frame(&fresh);
    fresh.vt_write(b"\x1b[1;1HAA\x1b[31mred");
    fresh.vt_write(b"\x1b[5;1H\x1b[32mgreen\x1b[0m");
    fresh.vt_write("\x1b[7;1H中文宽字符".as_bytes());
    fresh.flush();
    let full = next_cell_frame(&fresh);

    // 逐字节比较：`CellData` 是 bytemuck POD，按原始字节比最快且最严。
    assert_eq!(
        bytemuck::cast_slice::<CellData, u8>(&incremental),
        bytemuck::cast_slice::<CellData, u8>(&full),
        "行级增量的 CellData MUST 与全量重建逐字节相同（复用 {reused} 行 / 逐格 {extracted} 行）"
    );
}

#[test]
fn row_cache_invalidated_on_selection_install() {
    let mut terminal_under_test = terminal();
    for row in 0..10 {
        terminal_under_test.vt_write(format!("\x1b[{};1Hrow{row}", row + 1).as_bytes());
    }
    terminal_under_test.flush();
    let _ = next_cell_frame(&terminal_under_test);

    // 计数基线清零：stats 是「自上次 take 以来的累计」，不先清零就会混进上面
    // 那批帧的复用数（与 row_cache_invalidated_on_resize 同一条理由）。
    let _ = terminal_under_test.take_cache_stats();

    // 安装选区：反白被烘焙进 CellData 颜色，缓存行颜色逐行过时。
    terminal_under_test.set_selection((1, 0), (1, 3));
    terminal_under_test.flush();
    // 必须轮询到「该行已反白」的那一帧，而不是取安装后的第一帧：通道里可能还压着
    // 安装之前排队的旧帧（`flush` 逐条命令各推一帧），负载高时它更容易被先取到，
    // 于是断言落在旧帧上——与被测行为无关的偶发失败。
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    let reused = loop {
        let cells = next_cell_frame(&terminal_under_test);
        let stats = terminal_under_test.take_cache_stats();
        let selected = cells
            .iter()
            .find(|cell| cell.row == 1 && cell.col == 0)
            .expect("选区内单元必须存在");
        let unselected = cells
            .iter()
            .find(|cell| cell.row == 2 && cell.col == 0)
            .expect("选区外单元必须存在");
        if selected.background != unselected.background {
            break stats.0;
        }
        assert!(
            std::time::Instant::now() < deadline,
            "安装选区后 5s 内未出现该行已反白的帧"
        );
        std::thread::sleep(std::time::Duration::from_millis(2));
    };
    assert_eq!(
        reused, 0,
        "选区安装 MUST 使行缓存整体失效，本次不得有复制行"
    );
}

#[test]
fn row_cache_invalidated_on_resize() {
    let mut terminal_under_test = terminal();
    for row in 0..20 {
        terminal_under_test.vt_write(format!("\x1b[{};1Hrow{row}", row + 1).as_bytes());
    }
    terminal_under_test.flush();
    let _ = next_cell_frame(&terminal_under_test);

    // 计数基线清零：stats 是「自上次 take 以来的累计」，而上面的轮询可能转过
    // 多轮。不先清零，断言就混进了 resize 之前那批帧的复用数——轮询转的轮数取决于
    // 机器负载，于是同一个实现有时过有时不过（实测负载下偶发失败一次）。
    let _ = terminal_under_test.take_cache_stats();

    // 改行数后行区间整体错位，缓存不可复用。
    assert!(terminal_under_test.resize(12, 80), "resize 应被接受");
    terminal_under_test.flush();
    // 必须轮询到「行数已随 resize 收敛」的那一帧，而不是取 resize 后的第一帧：
    // 通道里可能还压着 resize 之前排队的旧帧（`flush` 逐条命令各推一帧），负载
    // 高时它更容易被先取到，于是断言落在旧帧的最大行上——与被测行为无关的偶发失败。
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    let reused = loop {
        let cells = next_cell_frame(&terminal_under_test);
        let stats = terminal_under_test.take_cache_stats();
        let max_row = cells.iter().map(|cell| cell.row).max();
        if max_row.is_some_and(|max| max < 12) {
            break stats.0;
        }
        assert!(
            std::time::Instant::now() < deadline,
            "resize 后 5s 内未出现新尺寸的帧，最后一帧最大行 {max_row:?}"
        );
        std::thread::sleep(std::time::Duration::from_millis(2));
    };
    assert_eq!(reused, 0, "尺寸变化 MUST 使行缓存整体失效");
}

#[test]
fn row_extraction_handles_wide_char_row_growth() {
    // 宽字符使该行单元数少于其他行：行区间必须由 `build_row_ranges` 依
    // `CellData.row` 划分，若按 cols 反推会把错位的内容复制到别的行。
    let mut terminal_under_test = terminal();
    for row in 0..6 {
        terminal_under_test.vt_write(format!("\x1b[{};1Hplain{row}", row + 1).as_bytes());
    }
    terminal_under_test.flush();
    let _ = next_cell_frame(&terminal_under_test);

    // 第 2 行改为整行宽字符：其单元数从 cols 降到 cols/2。
    terminal_under_test.vt_write(format!("\x1b[2;1H{}", "宽".repeat(20)).as_bytes());
    terminal_under_test.flush();
    let cells = next_cell_frame(&terminal_under_test);
    let (reused, _) = terminal_under_test.take_cache_stats();
    assert!(reused > 0, "本帧 MUST 命中行复制路径");

    // 逐行核对：复制来的行不得被宽字符行的短行内容污染。
    for row in 0..6u32 {
        let expected = if row == 1 {
            "宽".repeat(20)
        } else {
            format!("plain{row}")
        };
        assert_eq!(
            row_text_from_cells(&cells, row),
            expected,
            "第 {row} 行内容必须正确（宽字符行缩短后行区间不得错位）"
        );
    }
}

/// 2.9 的效果测量：冷启动全量抽取与稳态增量抽取的单帧耗时对照。
///
/// 对照只用两条真实路径，不靠命令投递构造：新建终端的**首次**抽取没有缓存
/// 可用，必走全量；同一终端随后每次只改一行，未变行走复制。两者的唯一差异
/// 就是行缓存是否可用。
///
/// 只断言功能性事实（增量组确实命中复制路径），性能数字只打印不做阈值断言——
/// 并行执行下墙钟噪声大到足以让任何阈值失真，而 `TESTING.md` 要求测试可复现
/// 且无不稳定项。测量值按本机实测记录在变更文档里。
#[test]
fn bench_row_extraction_full_versus_incremental() {
    const FRAMES: u32 = 60;

    let mut terminal_under_test = terminal();
    for row in 0..23 {
        terminal_under_test.vt_write(format!("\x1b[{row};1Hseed row {row}").as_bytes());
    }
    // 冷启动：建立缓存的那一帧必为全量。
    let full_start = Instant::now();
    terminal_under_test.flush();
    black_box(next_cell_frame(&terminal_under_test));
    let full_millis = full_start.elapsed().as_secs_f64() * 1000.0;
    let (_, full_extracted) = terminal_under_test.take_cache_stats();

    // 稳态：每帧只改一行，其余行走复制。写入逐帧不同，故不会被内容去重吞掉。
    let incremental_start = Instant::now();
    let mut reused_total = 0u64;
    for frame in 0..FRAMES {
        terminal_under_test.vt_write(format!("\x1b[23;1H{frame:02}").as_bytes());
        terminal_under_test.flush();
        black_box(next_cell_frame(&terminal_under_test));
        reused_total += terminal_under_test.take_cache_stats().0;
    }
    let incremental_millis = incremental_start.elapsed().as_secs_f64() * 1000.0 / FRAMES as f64;

    println!(
        "首帧全量 {:.3}ms（逐格 {full_extracted} 行），稳态增量 {:.3}ms/帧（{FRAMES} 帧共复用 {reused_total} 行）",
        full_millis, incremental_millis
    );

    assert!(
        full_extracted > 0,
        "冷启动首帧 MUST 逐格抽取全部行（此时无缓存可复用）"
    );
    assert!(
        reused_total > 0,
        "稳态每帧 MUST 命中行复制路径，否则测的是全量而非增量"
    );
}

/// 光标落在宽字符尾列时须被认到该宽字符的头列记录上，否则整帧不画光标。
///
/// 上游为此专门给了 `CursorViewport::at_wide_tail`——此前本仓读快照时把它丢了，只留
/// `(row, col)`。而 `build_cell_data` 对一个宽字符只发出一条头列记录（`width=2`，
/// 一条即整个字形），于是尾列上的光标列号匹配不到任何记录：那几帧终端看上去「重绘了
/// 但没有光标」。三种光标样式（方块/竖线/下划线）全中，因为匹配发生在样式分支之前。
#[test]
fn cursor_on_a_wide_characters_tail_column_is_reported() {
    let mut terminal_under_test = GhosttyTerminal::new(2, 10, 100).expect("terminal");
    // 「中」占第 0、1 两格；`CSI 2 G` 把光标移到它的尾列（第 1 格）。
    terminal_under_test.vt_write("中\x1b[2G".as_bytes());
    terminal_under_test.flush();

    let (cells, cursor) = terminal_under_test
        .receive_cell_data()
        .expect("写完须产出单元数据");
    assert!(cursor.visible, "光标可见");
    assert_eq!(cursor.col, 1, "VT 真值：光标停在宽字符的第二格");
    assert!(cursor.at_wide_tail, "上游须把这个位置标成宽字符尾列");

    // 渲染侧按列匹配头列记录，故那一格必须存在（宽字符只发一条 `width=2` 的记录）。
    let wide = cells
        .iter()
        .find(|cell| cell.row == cursor.row && cell.col == 0)
        .expect("宽字符的头列记录");
    assert_eq!(wide.width, 2, "头列记录覆盖两格");

    // 关键判别力：光标所在行的尾列上**没有**任何记录（spacer 不产出记录），故光标列号
    // 若不折算成锚列就匹配不到任何记录，那一帧终端看上去「重绘了但没有光标」。
    let tail_column_records = cells
        .iter()
        .filter(|cell| cell.row == cursor.row && cell.col == cursor.col)
        .count();
    assert_eq!(
        tail_column_records, 0,
        "尾列不得有自己的记录：断言语料前提，否则本用例对折算与否没有判别力"
    );
}

#[test]
fn selection_highlight_covers_blank_row_inside_range() {
    let mut terminal_under_test = terminal();
    terminal_under_test.vt_write(b"\x1b[1;1HAAA");
    terminal_under_test.vt_write(b"\x1b[3;1HBBB");
    terminal_under_test.flush();
    let _ = next_cell_frame(&terminal_under_test);

    terminal_under_test.set_selection((0, 0), (2, 5));
    terminal_under_test.flush();
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    loop {
        let cells = next_cell_frame(&terminal_under_test);
        let first = cells
            .iter()
            .find(|cell| cell.row == 0 && cell.col == 0)
            .expect("单元格");
        let blank = cells
            .iter()
            .find(|cell| cell.row == 1 && cell.col == 0)
            .expect("单元格");
        let blank_mid = cells
            .iter()
            .find(|cell| cell.row == 1 && cell.col == 3)
            .expect("单元格");
        let outside = cells
            .iter()
            .find(|cell| cell.row == 5 && cell.col == 0)
            .expect("单元格");
        if first.background != outside.background {
            assert_eq!(
                blank.background, first.background,
                "选区内的空行必须反白：否则选中区域在空行处断开，表现为「不同行间存在空隙」"
            );
            assert_eq!(
                blank_mid.background, first.background,
                "空行的反白要铺满选中列，不能只在行首几格"
            );
            break;
        }
        assert!(std::time::Instant::now() < deadline, "5s 内未见反白帧");
        std::thread::sleep(std::time::Duration::from_millis(2));
    }
}

/// ED2（`clear`）必须**首次**即撤下 Kitty 放置，不得等第二次 clear。
///
/// 用户报障：「kitten icat 显示图像后 clear，文本被清除但图像依旧显示，
/// 第二次 clear 才消失」。判别力放在 VT 侧不变式上：ED2 之后放置集合必须
/// 立刻为空——若 VT 仍留着放置，渲染侧无从知道该擦除哪块像素，只能靠
/// 「整帧重绘」兜底，而那正是「第二次 clear 才消失」的成因。
///
/// 图像经直接放置通道（`a=T`，即 kitten icat 走的那条）送入，故用同一路径
/// 构造：VT 侧放置与 Unicode 占位符两条通道共用 ED2 撤除语义。
///
/// 两条通道的**存储代数**行为不同（此处只断言共有的放置代数）：直接放置把图像数据
/// 与放置一起送入，撤下时图像条目一并删除，存储生成戳随之变化；虚拟放置只撤放置，
/// 图像仍留在存储里，存储生成戳不变（见 `ed2_removes_virtual_kitty_placement_on_first_clear`）。
/// 渲染缓存因此 MUST 以放置生成戳为撤除信号。
#[test]
fn ed2_removes_kitty_placements_on_first_clear() {
    let mut terminal = terminal();
    terminal.set_cell_pixel_size(8, 16);
    terminal.pty_write(b"\x1b_Ga=T,f=24,s=1,v=1,i=7;/wAA\x1b\\");
    terminal.flush();
    let (_, before) = terminal.receive_cell_data().expect("cell data after image");
    assert_eq!(
        1,
        terminal.take_kitty_placements().len(),
        "直接放置通道送入的图像必须产生一个放置，否则本用例不覆盖撤除路径"
    );

    terminal.pty_write(b"\x1b[2J");
    terminal.flush();
    let (_, after) = terminal.receive_cell_data().expect("cell data after ED2");
    assert_ne!(
        before.kitty_placement_generation, after.kitty_placement_generation,
        "ED2 必须推进放置生成戳：渲染线程靠它知道缓存里的放置已失效"
    );
    assert!(
        terminal.take_kitty_placements().is_empty(),
        "首次 ED2 后放置集合必须为空（图像不得残留到第二次 clear）"
    );
}

/// 虚拟放置（`icat` 常走的「先传图、再放图」两段式）的 ED2 撤除。
///
/// 判别力：ED2 撤下虚拟放置时**不**推进图像存储生成戳（实测不变式：存储键只随
/// 图像内容增删而变，放置的增删与移动走放置生成戳）；旧缓存规则只看存储戳，
/// 于是既不会醒来也不会重查，图像要等第二次 clear 才消失。放置生成戳
/// 必须独立变化，渲染侧才能第一次就撤下。
///
/// 注：上游对虚拟放置的撤除**不**推进存储生成戳，故此处同时锁住两条不变式；
/// 渲染缓存对「存储戳不变、放置戳变化」的判定由
/// `native/src/android/ffi.rs` 的纯函数单测覆盖。
#[test]
fn ed2_removes_virtual_kitty_placement_on_first_clear() {
    let mut terminal = terminal();
    terminal.set_cell_pixel_size(8, 16);
    terminal.pty_write(b"\x1b_Ga=T,U=1,f=24,s=1,v=1,i=7,c=1,r=1,q=2;/wAA\x1b\\");
    terminal.pty_write("\x1b[38;5;7m\u{10EEEE}\u{0305}\u{0305}\x1b[39m".as_bytes());
    terminal.flush();
    let (_, before) = terminal
        .receive_cell_data()
        .expect("cell data after virtual placement");
    assert_eq!(
        1,
        terminal.take_kitty_placements().len(),
        "虚拟放置通道必须产生一个可见放置，否则本用例不覆盖撤除路径"
    );
    assert_ne!(
        before.kitty_placement_generation, 0,
        "虚拟放置必须推进放置生成戳，否则渲染侧无法感知它"
    );

    terminal.pty_write(b"\x1b[2J");
    terminal.flush();
    let (_, after) = terminal.receive_cell_data().expect("cell data after ED2");
    assert_eq!(
        before.kitty_generation, after.kitty_generation,
        "撤下虚拟放置不改变图像存储：存储生成戳只随图像内容的增删而变"
    );
    assert_ne!(
        before.kitty_placement_generation, after.kitty_placement_generation,
        "放置生成戳必须变化，渲染线程才能醒来重查"
    );
    assert!(
        terminal.take_kitty_placements().is_empty(),
        "首次 ED2 后虚拟放置必须为空"
    );
}

/// 提示符行上的 ED2 走上游 scrollClear：回滚长度与放置生成戳同时变化。
///
/// 判别力：上游 `eraseDisplay(.complete)` 在主屏且最后一行非空内容是提示符时，
/// 先把该行以上的旧行滚进回滚（scrollClear），再撤下放置。视口因此平移，
/// 而放置集合已经变了——渲染侧若把这种平移当成「用户滚动」复用旧帧，
/// 撤下的图像就留在屏幕上，要等下一次重查才消失（用户实测「clear 一下不清，
/// 两下才清」）。此处锁住 VT 侧不变式：回滚长度增长且放置为空。
#[test]
fn ed2_at_prompt_scrolls_and_removes_kitty_placements() {
    let mut terminal = terminal();
    terminal.set_cell_pixel_size(8, 16);
    // OSC 133 A/B 标记提示符行：ED2 在此行走 scrollClear 路径。
    terminal.pty_write(b"\x1b]133;A\x1b\\$ \x1b]133;B\x1b\\");
    terminal.pty_write(b"\x1b_Ga=T,f=24,s=1,v=1,i=7;/wAA\x1b\\");
    terminal.flush();
    let (_, before) = terminal.receive_cell_data().expect("cell data after image");
    assert_eq!(
        1,
        terminal.take_kitty_placements().len(),
        "直接放置通道送入的图像必须产生一个放置，否则本用例不覆盖撤除路径"
    );

    terminal.pty_write(b"\x1b[2J");
    terminal.flush();
    let (_, after) = terminal.receive_cell_data().expect("cell data after ED2");
    assert!(
        after.scrollback_length > before.scrollback_length,
        "提示符行上的 ED2 必须走 scrollClear：旧行滚进回滚，回滚长度增长"
    );
    assert_ne!(
        before.kitty_placement_generation, after.kitty_placement_generation,
        "ED2 必须推进放置生成戳"
    );
    assert!(
        terminal.take_kitty_placements().is_empty(),
        "首次 ED2 后放置集合必须为空"
    );
}
