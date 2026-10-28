use super::*;

/// 纯视口滚动必须走行平移快路径：只逐格抽取露出的边缘行，
/// 其余行整段复用上一帧缓存。没有这条断言时，平移逻辑退化成全量抽取
/// 仍能通过内容断言（真机表现为每次滚轮事件 58ms 卡顿）。
#[test]
fn scroll_viewport_reuses_shifted_rows() {
    let terminal_under_test = terminal_with_twenty_lines();
    // 清空首帧的全量计数，只统计滚动帧。
    terminal_under_test.take_cache_stats();

    assert!(
        terminal_under_test.scroll_viewport(-2),
        "scroll_viewport should accept delta"
    );
    terminal_under_test.flush();
    let scrolled = terminal_under_test
        .receive_cell_data()
        .expect("滚动后必须收到 CellData 帧");
    let scrolled_rows: std::collections::HashSet<u32> =
        scrolled.0.iter().map(|cell| cell.row).collect();
    assert_eq!(scrolled_rows.len(), 5, "滚动帧仍须覆盖 5 个视口行");

    let (reused, extracted) = terminal_under_test.take_cache_stats();
    assert_eq!(
        extracted, 2,
        "向上滚 2 行只应逐格抽取露出的 2 行，实际 {extracted} 行"
    );
    assert_eq!(
        reused, 3,
        "其余 3 行应整段复用平移后的缓存，实际 {reused} 行"
    );
}

/// 输出发生在滚动之后时，待平移量必须丢弃：否则平移帧会跳过上游脏标记，
/// 把滚动前的旧行当成新内容复用。计数是唯一判别力来源——内容断言在
/// 视口外的输出上看不出差别。
#[test]
fn write_after_scroll_does_not_reuse_shifted_rows() {
    let mut terminal_under_test = terminal_with_twenty_lines();
    terminal_under_test.take_cache_stats();

    assert!(
        terminal_under_test.scroll_viewport(-2),
        "scroll_viewport should accept delta"
    );
    terminal_under_test.vt_write(b"post-scroll output\n");
    terminal_under_test.flush();
    assert!(
        terminal_under_test.receive_cell_data().is_some(),
        "滚动后写入必须收到 CellData 帧"
    );

    let (_reused, extracted) = terminal_under_test.take_cache_stats();
    assert!(
        extracted > 2,
        "滚动后写入必须让脏行走逐行判定，而不是只抽露出的 2 行；实际 {extracted} 行"
    );
}

/// 已在底部时继续向下滚动：视口内容不变，但显式滚动是用户动作，
/// 必须强制推帧，否则行级滚动的即时重绘补救路径会被内容去重吞掉。
#[test]
fn scroll_viewport_with_unchanged_content_still_pushes_frame() {
    let mut terminal_under_test = GhosttyTerminal::new(5, 10, 100).expect("terminal");
    for line_number in 0..20 {
        terminal_under_test.vt_write(format!("line{line_number}\r\n").as_bytes());
    }
    terminal_under_test.flush();
    assert!(
        terminal_under_test.receive_cell_data().is_some(),
        "首帧必须到达"
    );
    assert!(
        terminal_under_test.scroll_viewport(2),
        "scroll_viewport 应接受增量"
    );
    terminal_under_test.flush();
    await_pushed_frame(&mut terminal_under_test, "内容不变的滚动后");
}

/// 同步更新（模式 2026）期间，VT 线程不得发布中间帧：BSU 与 ESU 分两次写入，
/// 保持期间收到的 CellData 必须为空，ESU 之后的帧才包含期间写入的内容。
#[test]
fn synchronized_output_defers_frame_until_release() {
    let mut terminal_under_test = GhosttyTerminal::new(5, 10, 100).expect("terminal");
    terminal_under_test.vt_write(b"\x1b[?2026h");
    terminal_under_test.flush();
    // 排空进入保持前的帧（若空闲 tick 已推过一帧）。
    let _ = terminal_under_test.receive_cell_data();
    terminal_under_test.vt_write(b"held");
    terminal_under_test.flush();
    assert!(
        terminal_under_test.receive_cell_data().is_none(),
        "BSU..ESU 之间不得发布中间帧"
    );
    terminal_under_test.vt_write(b"\x1b[?2026l");
    terminal_under_test.flush();
    let (cells, _) = terminal_under_test
        .receive_cell_data()
        .expect("ESU 必须发布最终帧");
    let text: String = cells
        .iter()
        .filter_map(|cell| char::from_u32(cell.codepoint))
        .collect();
    assert!(
        text.contains("held"),
        "最终帧必须包含保持期间写入的内容, got {text:?}"
    );
}

/// 引擎不会自行结束保持：程序发了 BSU 后崩溃（没有 ESU）时，宿主必须超时解锁，
/// 否则屏幕永久冻结。这里不发 ESU，仍应在超时后收到包含挂起内容的帧。
#[test]
fn synchronized_output_hold_times_out_without_release() {
    let mut terminal_under_test = GhosttyTerminal::new(5, 10, 100).expect("terminal");
    terminal_under_test.vt_write(b"\x1b[?2026hstuck");
    terminal_under_test.flush();
    // 排空进入保持前的帧；此后没有任何写入，唯一能推帧的途径就是超时解锁。
    let _ = terminal_under_test.receive_cell_data();
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(3);
    let mut frame = None;
    while frame.is_none() {
        frame = terminal_under_test.receive_cell_data();
        assert!(
            frame.is_some() || std::time::Instant::now() < deadline,
            "保持超时后 3s 内仍未强制发布帧"
        );
        if frame.is_none() {
            std::thread::sleep(std::time::Duration::from_millis(2));
        }
    }
    let (cells, _) = frame.expect("frame");
    let text: String = cells
        .iter()
        .filter_map(|cell| char::from_u32(cell.codepoint))
        .collect();
    assert!(
        text.contains("stuck"),
        "超时解锁发布的帧必须包含保持期间的内容, got {text:?}"
    );
}

/// 切会话后累加器里留着上一个会话的像素，而目标会话内容与上次推送逐字节相同
/// （终端本身毫无变化）——内容去重会吞掉「重推」请求，屏幕就停在上一个会话上。
#[test]
fn request_repaint_pushes_frame_with_unchanged_content() {
    let mut terminal_under_test = GhosttyTerminal::new(5, 10, 100).expect("terminal");
    terminal_under_test.vt_write(b"prompt$ ");
    terminal_under_test.flush();
    assert!(
        terminal_under_test.receive_cell_data().is_some(),
        "首帧必须到达"
    );
    terminal_under_test.request_repaint();
    terminal_under_test.flush();
    await_pushed_frame(&mut terminal_under_test, "内容不变时的重推请求");
}

/// 连续推帧后消费者必须取到最新一帧。
///
/// 帧通道有界时通道满会丢弃**新**帧：最新的画面反而丢失，终端停在旧视口
/// （真机表现：突发输出后画面停在若干帧之前）。每次写入后 flush 让每行单独
/// 成批、单独推帧，而消费者始终不取——通道必然被填满。
#[test]
fn newest_frame_reaches_consumer_after_a_burst_of_pushes() {
    let mut terminal_under_test = GhosttyTerminal::new(10, 40, 200).expect("terminal");
    for line_number in 0..60 {
        terminal_under_test.vt_write(format!("line{line_number:02}\r\n").as_bytes());
        terminal_under_test.flush();
    }
    let (cells, _) = terminal_under_test
        .receive_cell_data()
        .expect("突发推帧后必须收到 CellData 帧");
    let row_text = |row: u32| -> String {
        cells
            .iter()
            .filter(|cell| cell.row == row)
            .filter_map(|cell| {
                if cell.codepoint == 0 {
                    None
                } else {
                    char::from_u32(cell.codepoint)
                }
            })
            .collect()
    };
    // 10 行视口在 60 行输出后停在底部：末行是光标所在的空行，
    // 其上 9 行依次是最后 9 行输出。
    assert_eq!(row_text(0), "line51", "视口首行必须是滚动窗口的第一行");
    assert_eq!(row_text(8), "line59", "视口末行必须是最后写入的一行");
    assert_eq!(row_text(9), "", "光标行必须为空");
}

/// 视口滚动后，绝对网格行（0 = 回滚顶部，与 Kotlin 的
/// `scrollbackLength - scrollOffset + row` 同口径）查询必须仍命中该行的内容。
/// `dump_grid.visible` 直接取 `Point::Viewport`，即屏幕上真实显示的文本，
/// 作为地面真值。
#[test]
fn absolute_row_text_survives_viewport_scroll() {
    const ROWS: u32 = 5;
    const SCROLL_OFFSET: u32 = 3;
    let mut terminal_under_test = GhosttyTerminal::new(ROWS, 20, 100).expect("terminal");
    // vt_write 不做 LF→CRLF 展开，必须自带 CR，否则光标只下移不回列。
    for line_number in 0..20 {
        terminal_under_test.vt_write(format!("line{line_number}\r\n").as_bytes());
    }
    terminal_under_test.flush();
    let scrollback = terminal_under_test.scrollback_length();
    assert!(
        scrollback > SCROLL_OFFSET,
        "回滚须多于上滚量, got {scrollback}"
    );
    assert!(terminal_under_test.scroll_viewport(-(SCROLL_OFFSET as isize)));
    terminal_under_test.flush();

    let dumped = terminal_under_test.dump_grid();
    assert_eq!(dumped.rows, ROWS);
    let visible_line = |viewport_row: u32| -> String {
        let start = (viewport_row * dumped.cols) as usize;
        dumped.visible[start..start + dumped.cols as usize]
            .iter()
            .map(|cell| match char::from_u32(cell.codepoint) {
                Some('\0') | None => ' ',
                Some(character) => character,
            })
            .collect::<String>()
            .trim_end()
            .to_string()
    };
    assert_eq!(
        visible_line(0),
        format!("line{}", scrollback - SCROLL_OFFSET),
        "上滚后视口首行应是回滚中的某一行"
    );

    for viewport_row in 0..ROWS {
        let absolute_row = scrollback - SCROLL_OFFSET + viewport_row;
        assert_eq!(
            terminal_under_test.read_line_text(absolute_row).as_deref(),
            Some(visible_line(viewport_row).as_str()),
            "绝对行 {absolute_row}（视口第 {viewport_row} 行）在上滚 {SCROLL_OFFSET} 后读错行"
        );
    }
}

/// Scrollback browsing: the cursor must be reported in VIEWPORT
/// coordinates (or hidden when the cursor page is scrolled out of the
/// viewport), never in active-screen coordinates. The old code used
/// `cursor_y()` (active-area row) against viewport-relative CellData
/// rows, drawing the cursor on the wrong grid row after any scroll —
/// the reported cursor "block" offset ~1 cell down/right.
#[test]
fn cursor_viewport_coordinates_track_scrollback_scroll() {
    let mut terminal_under_test = GhosttyTerminal::new(5, 10, 100).expect("terminal");
    for line_number in 0..20 {
        terminal_under_test.vt_write(format!("line {line_number}\n").as_bytes());
    }
    terminal_under_test.flush();

    // Cursor sits on the active bottom row (row 4), col 0.
    let (_, cursor0) = terminal_under_test.receive_cell_data().expect("cell data");
    assert!(cursor0.visible, "cursor visible at scroll offset 0");
    assert_eq!(cursor0.row, 4, "cursor on active bottom row");

    // Scroll up 1: viewport now shows scrollback rows 14..18; the
    // cursor page (active row 19) is out of view → the cursor must be
    // hidden, not drawn on scrollback row 4.
    assert!(
        terminal_under_test.scroll_viewport(-1),
        "scroll_viewport(-1)"
    );
    let mut hidden = None;
    for _ in 0..50 {
        if let Some((_, cursor)) = terminal_under_test.receive_cell_data() {
            hidden = Some(cursor);
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(2));
    }
    match hidden {
        Some(cursor) => {
            assert!(!cursor.visible, "cursor hidden when scrolled out of view")
        }
        None => panic!("no cell data after scroll"),
    }

    // Scroll back to the bottom: the cursor reappears on row 4.
    assert!(
        terminal_under_test.scroll_viewport(1),
        "scroll_viewport(+1)"
    );
    let mut restored = None;
    for _ in 0..50 {
        if let Some((_, cursor)) = terminal_under_test.receive_cell_data() {
            restored = Some(cursor);
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(2));
    }
    match restored {
        Some(cursor) => {
            assert!(cursor.visible, "cursor visible again at offset 0");
            assert_eq!(cursor.row, 4, "cursor back on the active bottom row");
        }
        None => panic!("no cell data after scroll back"),
    }
}

#[test]
fn terminal_is_alive_after_creation() {
    let terminal_under_test = small_terminal();
    assert!(terminal_under_test.is_alive());
}

#[test]
fn terminal_is_alive_after_vt_write() {
    let mut terminal_under_test = small_terminal();
    terminal_under_test.vt_write(b"Hello, world!");
    assert!(terminal_under_test.is_alive());
}

#[test]
fn terminal_is_alive_after_flush() {
    let mut terminal_under_test = small_terminal();
    terminal_under_test.vt_write(b"ABC");
    terminal_under_test.flush();
    assert!(terminal_under_test.is_alive());
}

/// 经公开 receive_cell_data() 流断言逐行输出一致性：写入第 1 行不得扰动第 0 行。
#[test]
fn cell_data_rows_stay_consistent_across_writes() {
    let mut terminal_under_test = terminal();
    terminal_under_test.vt_write(b"hello");
    terminal_under_test.flush();
    let first = terminal_under_test
        .receive_cell_data()
        .expect("first cell data");
    let (first_cells, _) = first;
    let cols = 80usize;
    let row0_first: Vec<u32> = first_cells[..cols]
        .iter()
        .map(|cell| cell.codepoint)
        .collect();

    // 空闲去重下静默 VT 线程不再推送相同内容：一致性由下方第三快照
    // （新输入后的确定性重建）验证，此处不做定时等待。

    // New input on row 1 must not disturb row 0's cached content.
    terminal_under_test.vt_write(b"\nworld");
    terminal_under_test.flush();
    let third = terminal_under_test
        .receive_cell_data()
        .expect("third cell data");
    let (third_cells, _) = third;
    let row0_third: Vec<u32> = third_cells[..cols]
        .iter()
        .map(|cell| cell.codepoint)
        .collect();
    let row1_third: Vec<u32> = third_cells[cols..cols * 2]
        .iter()
        .map(|cell| cell.codepoint)
        .collect();
    assert_eq!(row0_third, row0_first, "row 0 unchanged after row-1 write");
    // vt_write treats LF as a bare line feed (no CR), so "world" lands at
    // col 5 on row 1, right after the LF.
    assert_eq!(row1_third[5], 'w' as u32, "row 1 col 5 is 'w'");
}

/// 调整尺寸后推送的单元数据必须反映新网格维度，而非旧尺寸的行。
#[test]
fn cell_data_follows_resized_grid() {
    let mut terminal_under_test = terminal();
    terminal_under_test.vt_write(b"top");
    terminal_under_test.flush();
    // 确定性轮询直到工作线程产出对应尺寸数据，杜绝固定时长等待的 flaky。
    let start = Instant::now();
    let before_cells = loop {
        if let Some((cells, _)) = terminal_under_test.receive_cell_data()
            && cells.len() == 24 * 80
        {
            break cells;
        }
        assert!(
            start.elapsed() < std::time::Duration::from_secs(5),
            "调整前单元数据未就绪"
        );
        std::thread::sleep(std::time::Duration::from_millis(10));
    };
    assert_eq!(before_cells.len(), 24 * 80);

    assert!(terminal_under_test.resize(10, 40), "resize to 10x40");
    let start = Instant::now();
    let after_cells = loop {
        if let Some((cells, _)) = terminal_under_test.receive_cell_data()
            && cells.len() == 10 * 40
        {
            break cells;
        }
        assert!(
            start.elapsed() < std::time::Duration::from_secs(5),
            "调整后单元数据未就绪"
        );
        std::thread::sleep(std::time::Duration::from_millis(10));
    };
    assert_eq!(after_cells.len(), 10 * 40, "调整后仍推送旧尺寸的行");
}

/// Ghostty formatter selection extraction: a soft-wrapped long line must be
/// joined without '\n' (termux TerminalBuffer.getSelectedText joinBackLines
/// semantics). Write a line longer than 80 cols then select across the wrap.
#[test]
fn selection_text_unwraps_soft_wrapped_lines() {
    let mut terminal_under_test = terminal(); // 24x80
    // 90 chars: exceeds the 80-col width -> soft wrap onto row 2.
    let long = "a".repeat(90);
    terminal_under_test.vt_write(long.as_bytes());
    terminal_under_test.flush();
    let snap = terminal_under_test.take_snapshot();
    let scrollback = snap.scrollback_length;
    // The text starts at viewport row 0 (grid row = scrollback_rows).
    let row0 = scrollback;
    let text = terminal_under_test.selection_text((row0, 0), (row0 + 1, 9));
    assert_eq!(
        text.len(),
        90,
        "soft-wrapped selection must be joined without newline (len={})",
        text.len()
    );
    assert!(
        !text.contains('\n'),
        "no newline inside a soft-wrapped selection"
    );
}

/// 宽字符（CJK）列映射：选区若含宽字符字形，MUST NOT 把它拆开，且抽取出的
/// 文本必须与可见内容一致（等价于 TerminalRow.findStartOfColumn）。
#[test]
fn selection_text_wide_char_columns() {
    let mut terminal_under_test = terminal();
    terminal_under_test.vt_write("中".as_bytes()); // wide char at cols 0-1
    terminal_under_test.vt_write(b"ab");
    terminal_under_test.flush();
    let snap = terminal_under_test.take_snapshot();
    let row0 = snap.scrollback_length;
    let text = terminal_under_test.selection_text((row0, 0), (row0, 3));
    assert_eq!(
        text, "中ab",
        "wide char must round-trip exactly (got {text:?})"
    );
}

/// 空格选字：点空格选中该格（对标 selectWordOnBlankCellSelectsThatCell）。
/// 上游格式化器把纯空白选区 trim 为空串：不断言文本，只断言选区反白
/// 烘焙到了该格（选区存在且可见）。
#[test]
fn selection_text_blank_cell_selects_itself() {
    let mut terminal_under_test = terminal();
    terminal_under_test.vt_write(b"a b");
    terminal_under_test.flush();
    let snap = terminal_under_test.take_snapshot();
    let row0 = snap.scrollback_length;
    terminal_under_test.set_selection((row0, 1), (row0, 1));
    let picked_foreground = viewport_cell_foreground(&mut terminal_under_test, 1);
    assert_eq!(
        picked_foreground,
        default_background(),
        "blank cell under selection must be inverted"
    );
    terminal_under_test.clear_selection();
    terminal_under_test.flush();
}

/// 选择清除往返：装选区烘焙反白，清除后恢复基线（对标 selectionClearRemovesSelection）。
#[test]
fn selection_clear_restores_baseline_colors() {
    let mut terminal_under_test = terminal();
    terminal_under_test.vt_write(b"hello");
    terminal_under_test.flush();
    let snap = terminal_under_test.take_snapshot();
    let row0 = snap.scrollback_length;
    terminal_under_test.set_selection((row0, 0), (row0, 4));
    let picked_foreground = viewport_cell_foreground(&mut terminal_under_test, 0);
    assert_eq!(picked_foreground, default_background());
    terminal_under_test.clear_selection();
    let restored_foreground = viewport_cell_foreground(&mut terminal_under_test, 0);
    assert_eq!(
        restored_foreground,
        default_foreground(),
        "clear must restore baseline foreground"
    );
}

/// 终端持有选区反白（spec 文本选择：选区存于终端，跟踪引用）：
/// 对标上游 selectWordHighlightsAndExtractsText：上游原生 select_word
/// 派生选区并经格式化器提取文本。本仓单测经“解析出词边界→
/// 用 install 链路安装→格式化器提取”验证同一语义（本仓 selection
/// 接口即 install+format；上游 select_word 另经 select_word_at 查询接入）。
#[test]
fn select_word_via_boundary_resolve_extracts_hello() {
    let mut terminal_under_test = terminal();
    terminal_under_test.vt_write(b"hello world");
    terminal_under_test.flush();
    let snap = terminal_under_test.take_snapshot();
    let row0 = snap.scrollback_length;
    // 词边界解析：col 1（hello 内）→ 词起止 (0, 5)。
    let (word_start, word_end) = resolve_word_bounds(&terminal_under_test, row0, 1);
    assert_eq!(
        (word_start, word_end),
        (0, 5),
        "col 1 must resolve to hello"
    );
    terminal_under_test.set_selection((row0, word_start), (row0, word_end - 1));
    terminal_under_test.flush();
    assert_eq!(
        terminal_under_test.selection_text((row0, 0), (row0, 4)),
        "hello"
    );
}

/// 对标上游 selectWordOnBlankCellSelectsThatCell：空格格解析为空长度
/// 词边界并经 install 链路反白烘焙（文本断言见
/// selection_text_blank_cell_selects_itself）。
#[test]
fn select_word_blank_cell_resolves_empty_bounds() {
    let mut terminal_under_test = terminal();
    terminal_under_test.vt_write(b"a b");
    terminal_under_test.flush();
    let snap = terminal_under_test.take_snapshot();
    let row0 = snap.scrollback_length;
    let (word_start, word_end) = resolve_word_bounds(&terminal_under_test, row0, 1);
    assert_eq!(
        (word_start, word_end),
        (1, 2),
        "blank cell must resolve to its own cell"
    );
}

/// 对标上游 selectLineSelectsWholeLine：整行解析出起止并经
/// install+format 提取整行文本。
#[test]
fn select_line_via_full_row_extracts_whole_line() {
    let mut terminal_under_test = terminal();
    terminal_under_test.vt_write(b"hello world");
    terminal_under_test.flush();
    let snap = terminal_under_test.take_snapshot();
    let row0 = snap.scrollback_length;
    let line = terminal_under_test.read_line_text(row0).expect("line text");
    assert_eq!(line, "hello world");
    let end_col = (line.chars().count() as u32).saturating_sub(1);
    terminal_under_test.set_selection((row0, 0), (row0, end_col));
    terminal_under_test.flush();
    assert_eq!(
        terminal_under_test.selection_text((row0, 0), (row0, end_col)),
        "hello world"
    );
}

/// 对标上游 selectAllCoversScrollback：首行滚入历史后，跨全缓冲
/// 的 install+format 仍覆盖首尾行。
#[test]
fn select_all_via_range_covers_scrollback() {
    let mut terminal_under_test = GhosttyTerminal::new(5, 20, 100).expect("terminal");
    terminal_under_test.vt_write(b"alpha\n");
    for index in 0..8 {
        terminal_under_test.vt_write(format!("filler{index}\n").as_bytes());
    }
    terminal_under_test.flush();
    let total = terminal_under_test.take_snapshot();
    let last_row = total.rows + total.scrollback_length - 1;
    let text = terminal_under_test.selection_text((0, 0), (last_row, 19));
    assert!(
        text.starts_with("alpha"),
        "must include scrolled-off first line"
    );
    assert!(text.contains("filler7"), "must include the latest line");
}

/// 空白格没有词：长按空白只出仅粘贴菜单，Kotlin 侧据此分菜单，故此处钉死该语义。
/// 若上游对空白格也派生出一个"词"，Kotlin 的空白判定会整体失效（空白格弹出
/// 带复制的完整菜单）。
#[test]
fn select_word_at_yields_nothing_on_blank_cell() {
    let mut terminal_under_test = terminal();
    terminal_under_test.vt_write(b"ab");
    terminal_under_test.flush();
    let row0 = terminal_under_test.take_snapshot().scrollback_length;
    // 第 8 列在 80 列网格上必为空白：行内既无文字也无尾格。
    assert!(
        terminal_under_test.select_word_at(row0, 8).is_none(),
        "空白格不得派生出词界"
    );
}

/// 上游词选接入（design 决策 1）：select_word_at 派生词界限、取序、反解为
/// 绝对坐标并安装；返回界限提取的文本恰为该词，渲染反白证明选区已装回。
#[test]
fn select_word_at_derives_installs_and_returns_bounds() {
    let mut terminal_under_test = terminal();
    terminal_under_test.vt_write(b"git status");
    terminal_under_test.flush();
    let snap = terminal_under_test.take_snapshot();
    let row0 = snap.scrollback_length;
    let ((start_row, start_col), (end_row, end_col)) = terminal_under_test
        .select_word_at(row0, 1)
        .expect("word bounds under cell");
    assert_eq!((start_row, start_col), (row0, 0), "word starts at col 0");
    assert_eq!((end_row, end_col), (row0, 2), "git spans cols 0..=2");
    assert_eq!(
        terminal_under_test.selection_text((start_row, start_col), (end_row, end_col)),
        "git",
        "returned bounds must extract exactly the word"
    );
    // 安装断言：派生快照经 to_ordered 装回终端后，该格渲染必须反白。
    let picked_foreground = viewport_cell_foreground(&mut terminal_under_test, 1);
    assert_eq!(
        picked_foreground,
        default_background(),
        "derived selection must be installed (cell inverted)"
    );
    terminal_under_test.clear_selection();
    terminal_under_test.flush();
}

/// 上游全选接入（design 决策 2 钉住）：两行内容之后，select_all 界限落在
/// 最后一行的最后内容列，不含尾部空行与空列。
#[test]
fn select_all_bounds_exclude_trailing_blank_rows_and_columns() {
    let mut terminal_under_test = GhosttyTerminal::new(5, 20, 100).expect("terminal");
    terminal_under_test.vt_write(b"alpha\r\nbravo");
    terminal_under_test.flush();
    let snap = terminal_under_test.take_snapshot();
    let row0 = snap.scrollback_length;
    let bounds = terminal_under_test.select_all().expect("select_all bounds");
    assert_eq!(
        bounds,
        ((row0, 0), (row0 + 1, 4)),
        "bounds must exclude trailing blank rows and columns (got {bounds:?})"
    );
    let text = terminal_under_test.selection_text(bounds.0, bounds.1);
    assert!(
        text.contains("alpha") && text.contains("bravo"),
        "pinned bounds must cover both content lines (got {text:?})"
    );
}

/// 对标上游 selectionTracksTextIntoScrollback：选区安装后文本继续
/// 滚动，同一绝对坐标的 selection_text 仍提取原文本（跟踪引用语义
/// 经 install 链路保持）。
#[test]
fn selection_text_survives_scrolled_output() {
    let mut terminal_under_test = GhosttyTerminal::new(5, 20, 100).expect("terminal");
    terminal_under_test.vt_write(b"alpha\n");
    terminal_under_test.flush();
    let snap = terminal_under_test.take_snapshot();
    let row0 = snap.scrollback_length;
    terminal_under_test.set_selection((row0, 0), (row0, 4));
    terminal_under_test.flush();
    assert_eq!(
        terminal_under_test.selection_text((row0, 0), (row0, 4)),
        "alpha"
    );
    for index in 0..8 {
        terminal_under_test.vt_write(format!("filler{index}\n").as_bytes());
    }
    terminal_under_test.flush();
    assert_eq!(
        terminal_under_test.selection_text((row0, 0), (row0, 4)),
        "alpha",
        "tracked selection must follow text into scrollback"
    );
}

/// 对标上游 pasteEncodingHonorsBracketedMode：粘贴编码走
/// libghostty_vt::paste Native：非括号模式换行转回车，括号模式
/// 包裹 \x1b[200~/201~。
#[test]
fn paste_encoding_plain_and_bracketed() {
    let plain = encode_paste_text("ab\ncd", false);
    assert_eq!(plain, b"ab\rcd");
    let bracketed = encode_paste_text("ab", true);
    assert_eq!(bracketed, b"\x1b[200~ab\x1b[201~");
}

/// 粘贴编码端到端：bracketed 标志取活体 2004，非括号换行转回车，
/// 括号包裹且换行保留（对标上游 `paste` 口径）。
#[test]
fn encode_paste_follows_live_bracketed_mode() {
    let mut terminal_under_test = terminal();
    let plain = terminal_under_test
        .encode_paste(b"a\nb")
        .expect("paste encode");
    assert_eq!(plain, b"a\rb");
    terminal_under_test.vt_write(b"\x1b[?2004h");
    terminal_under_test.flush();
    let bracketed = terminal_under_test
        .encode_paste(b"a\nb")
        .expect("paste encode");
    assert_eq!(bracketed, b"\x1b[200~a\nb\x1b[201~");
    let single = terminal_under_test
        .encode_paste(b"hello")
        .expect("paste encode");
    assert_eq!(single, b"\x1b[200~hello\x1b[201~");
}

/// 控制字节中化为空格，终止序列不可逃逸；多字节文本逐字节保留。
#[test]
fn encode_paste_sanitizes_control_bytes() {
    let mut terminal_under_test = terminal();
    let sanitized = terminal_under_test
        .encode_paste(b"a\x00b\x1bc\x7fd")
        .expect("paste encode");
    assert_eq!(sanitized, b"a b c d");
    terminal_under_test.vt_write(b"\x1b[?2004h");
    terminal_under_test.flush();
    let bracketed = terminal_under_test
        .encode_paste(b"a\x1b[201~b")
        .expect("paste encode");
    assert_eq!(bracketed, b"\x1b[200~a [201~b\x1b[201~");
    let chinese = terminal_under_test
        .encode_paste("中文😀".as_bytes())
        .expect("paste encode");
    assert_eq!(
        chinese,
        b"\x1b[200~\xe4\xb8\xad\xe6\x96\x87\xf0\x9f\x98\x80\x1b[201~"
    );
}

/// 对标上游 searchFindsMatchAcrossScrollbackAndReveals：首行滚入历史后搜索仍能命中它；
/// 上游按 `Scroll::None` 选中不移动视口，宿主滚到命中后高亮才可见。
#[test]
fn search_select_leaves_scrolling_to_host() {
    let mut terminal_under_test = GhosttyTerminal::new(5, 20, 100).expect("terminal");
    terminal_under_test.vt_write(b"needle\n");
    for index in 0..10 {
        terminal_under_test.vt_write(format!("filler{index}\n").as_bytes());
    }
    terminal_under_test.flush();
    let viewport_top_text = |terminal: &GhosttyTerminal| {
        let grid = terminal.dump_grid();
        grid.visible
            .iter()
            .take(grid.cols as usize)
            .filter(|cell| cell.codepoint != 0)
            .filter_map(|cell| char::from_u32(cell.codepoint))
            .collect::<String>()
    };
    let before = viewport_top_text(&terminal_under_test);
    assert_ne!(before, "needle");
    assert_eq!(terminal_under_test.search_set_needle("needle"), 1);
    let selected = terminal_under_test
        .search_select(true)
        .expect("history hit is selectable");
    assert_eq!(selected.index, 0, "newest hit is index 0");
    assert_eq!(selected.matched.start_col, 0, "needle starts at col 0");
    assert_eq!(
        terminal_under_test.read_line_text(selected.matched.row),
        Some("needle".to_string()),
        "hit line must be the needle line"
    );
    // 上游不移动视口：命中仍在回滚区，视口顶行仍是 filler。
    assert_eq!(
        viewport_top_text(&terminal_under_test),
        before,
        "Scroll::None 选中不得移动视口"
    );
    // 宿主滚到命中（视图侧居中）后，命中才进入视口。
    assert!(terminal_under_test.scroll_viewport(-10));
    terminal_under_test.flush();
    assert_eq!(viewport_top_text(&terminal_under_test), "needle");
}

/// 未选中任何命中时视口命中集也必须可用：UI 在用户按「下一个」之前就要画高亮，
/// 高亮不能依赖选中态（选中只决定哪一个用强调色）。
#[test]
fn search_viewport_highlights_without_selection() {
    let mut terminal_under_test = GhosttyTerminal::new(5, 20, 100).expect("terminal");
    terminal_under_test.vt_write(b"alpha needle beta\n");
    terminal_under_test.vt_write(b"gamma\n");
    terminal_under_test.flush();
    assert_eq!(terminal_under_test.search_set_needle("needle"), 1);
    let highlights = terminal_under_test.search_viewport_highlights();
    assert!(highlights.selected.is_empty(), "未导航时不应有选中命中");
    assert_eq!(highlights.matches.len(), 1, "未选中时也要给出视口命中");
    assert_eq!(highlights.matches[0].start_col, 6);
    assert_eq!(highlights.matches[0].end_col, 12);
}

/// 对标上游 searchStepWraps：命中下标 0 为最新，`select_next` 朝更旧内容走，
/// 走到最旧后回绕到最新。
#[test]
fn search_select_walks_from_newest_toward_older_and_wraps() {
    let mut terminal_under_test = GhosttyTerminal::new(5, 20, 100).expect("terminal");
    for _ in 0..3 {
        terminal_under_test.vt_write(b"match\n");
    }
    terminal_under_test.flush();
    assert_eq!(terminal_under_test.search_set_needle("match"), 3);
    let newest = terminal_under_test
        .search_select(true)
        .expect("first select");
    let older = terminal_under_test
        .search_select(true)
        .expect("second select");
    assert_eq!((newest.index, older.index), (0, 1), "0 is the newest hit");
    assert!(
        older.matched.row < newest.matched.row,
        "select_next moves toward older content (newest row {}, older row {})",
        newest.matched.row,
        older.matched.row
    );
    let oldest = terminal_under_test
        .search_select(true)
        .expect("third select");
    assert_eq!(oldest.index, 2, "last hit before wrapping");
    let wrapped = terminal_under_test
        .search_select(true)
        .expect("wraps to newest");
    assert_eq!(wrapped.index, 0, "select_next wraps past the oldest hit");
}

/// 对标上游 searchStepRescansAfterOutput：needle 不变时，输出写入后的下一次
/// 导航必须看到新命中。上游 `select_*` 自带追赶（feed），本仓不另建重扫路径。
#[test]
fn search_select_sees_output_written_after_needle() {
    let mut terminal_under_test = GhosttyTerminal::new(5, 20, 100).expect("terminal");
    terminal_under_test.vt_write(b"needle one\n");
    terminal_under_test.flush();
    assert_eq!(terminal_under_test.search_set_needle("needle"), 1);
    let first = terminal_under_test.search_select(true).expect("first hit");
    assert_eq!(first.total, 1);

    terminal_under_test.vt_write(b"needle two\n");
    terminal_under_test.flush();

    let second = terminal_under_test.search_select(true).expect("second hit");
    assert_eq!(second.total, 2, "导航必须看到 needle 之后写入的新命中");
    assert_eq!(
        terminal_under_test
            .search_viewport_highlights()
            .matches
            .len(),
        2,
        "两行命中都在视口内时高亮必须齐全"
    );
}

/// 空 needle 必须让上游搜索回到 idle：总数、选中态与视口命中一并清空，
/// 关闭搜索后原生侧不保留旧匹配。
#[test]
fn search_empty_needle_clears_matches_and_selection() {
    let mut terminal_under_test = GhosttyTerminal::new(5, 20, 100).expect("terminal");
    terminal_under_test.vt_write(b"needle\n");
    terminal_under_test.flush();
    assert_eq!(terminal_under_test.search_set_needle("needle"), 1);
    assert!(terminal_under_test.search_select(true).is_some());

    assert_eq!(terminal_under_test.search_set_needle(""), 0);
    let highlights = terminal_under_test.search_viewport_highlights();
    assert!(highlights.matches.is_empty(), "清空后不得残留视口命中");
    assert!(highlights.selected.is_empty(), "清空后不得残留选中命中");
    assert!(terminal_under_test.search_select(true).is_none());
}

/// 对标上游 searchNoMatchesClearsSelection：无命中时总数为 0、视口命中为空，
/// 且导航返回 None（UI 据此清空选中态）。
#[test]
fn search_no_matches_clears_selection() {
    let mut terminal_under_test = terminal();
    terminal_under_test.vt_write(b"hello world");
    terminal_under_test.flush();
    assert_eq!(terminal_under_test.search_set_needle("zzz"), 0);
    assert!(search_viewport_matches(&terminal_under_test, "").is_empty());
    assert!(terminal_under_test.search_select(true).is_none());
    assert!(terminal_under_test.search_select(false).is_none());
}

/// 锁外查询通道与方法版同结果——JNI 侧只将会话锁删减到取通道，
/// 查询语义必须零漂移。
#[test]
fn search_via_bare_channel_matches_method() {
    let mut terminal_under_test = terminal();
    terminal_under_test.vt_write(b"needle in haystack needle");
    terminal_under_test.flush();
    let via_method = search_viewport_matches(&terminal_under_test, "needle");
    let via_channel = {
        let query_tx = terminal_under_test.query_channel();
        crate::terminal::ghostty_terminal::GhosttyTerminal::search_set_needle_on(
            &query_tx, "needle",
        );
        crate::terminal::ghostty_terminal::GhosttyTerminal::search_viewport_highlights_on(&query_tx)
            .matches
    };
    assert_eq!(via_method.len(), 2);
    assert_eq!(via_method, via_channel);
}

/// 安装选区后 VT 线程把行级选区反白烘焙进 CellData（前景背景互换），
/// 清除后恢复。该测试断言本仓的安装—烘焙链路，不复述上游选区语义。
#[test]
fn terminal_owned_selection_inverts_cell_data() {
    let mut terminal_under_test = terminal(); // 24x80
    terminal_under_test.vt_write(b"hello");
    terminal_under_test.flush();
    let snap = terminal_under_test.take_snapshot();
    let row0 = snap.scrollback_length;
    // 基线：未选中时前景为主题前景色。
    let (_, _) = terminal_under_test
        .receive_cell_data()
        .expect("baseline cell data");
    terminal_under_test.set_selection((row0, 0), (row0, 4));
    terminal_under_test.flush();
    let (selected, _) = terminal_under_test
        .receive_cell_data()
        .expect("selected cell data");
    let picked = selected
        .iter()
        .find(|cell| cell.row == 0 && cell.col == 0)
        .expect("row 0 col 0 present");
    // 默认主题的前景/背景；选中后互换。
    let theme_foreground = default_foreground();
    let theme_background = default_background();
    assert_eq!(
        picked.foreground, theme_background,
        "selected foreground must be theme background"
    );
    assert_eq!(
        picked.background, theme_foreground,
        "selected background must be theme foreground"
    );
    // 选区外单元格不受影响。
    let outside = selected
        .iter()
        .find(|cell| cell.row == 0 && cell.col == 10)
        .expect("row 0 col 10 present");
    assert_eq!(outside.foreground, theme_foreground);
    assert_eq!(outside.background, theme_background);
    // 清除后恢复基线。
    terminal_under_test.clear_selection();
    terminal_under_test.flush();
    let (cleared, _) = terminal_under_test
        .receive_cell_data()
        .expect("cleared cell data");
    let restored = cleared
        .iter()
        .find(|cell| cell.row == 0 && cell.col == 0)
        .expect("row 0 col 0 present");
    assert_eq!(restored.foreground, theme_foreground);
    assert_eq!(restored.background, theme_background);
}

/// 滚动请求序号必须随帧回传：渲染侧据此识别「早于在途滚动命令」的陈旧帧。
///
/// 判别力：序号不回传时渲染侧无法区分「VT 已处理滚动命令的帧」与「命令在途
/// 期间推送的旧视口帧」，后者一旦呈现就是旧视口配新亚行余量——内容先上跳
/// 半行、待新视口到达再下跳整行（上下滑动晃动）。
#[test]
fn scroll_request_id_travels_with_frame() {
    let mut terminal_under_test = GhosttyTerminal::new(5, 10, 100).expect("terminal");
    terminal_under_test.vt_write(b"line0\r\nline1\r\nline2\r\n");
    terminal_under_test.flush();
    let (_, cursor_before) = terminal_under_test.receive_cell_data().expect("cell data");
    assert_eq!(
        cursor_before.scroll_request_id, 0,
        "未下发滚动命令时帧不得携带请求序号"
    );

    assert!(
        terminal_under_test.scroll_viewport(-1),
        "scroll_viewport 应接受增量"
    );
    let request_id = terminal_under_test.latest_scroll_request_id();
    assert!(request_id > 0, "已送达的滚动命令必须分配请求序号");

    // VT 处理命令后推送的帧必须携带该序号。
    let mut stamped = None;
    for _ in 0..500 {
        if let Some((_, cursor)) = terminal_under_test.receive_cell_data()
            && cursor.scroll_request_id == request_id
        {
            stamped = Some(cursor);
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(2));
    }
    let stamped = stamped.expect("滚动命令处理后必须推出携带请求序号的帧");
    assert_eq!(
        stamped.scroll_request_id, request_id,
        "帧必须携带 VT 已处理的滚动请求序号"
    );
}

/// 滚动请求序号必须进程级单调递增、跨会话不复用。
///
/// 判别力：序号若按会话各自计数，会话重启后的新请求序号可能不大于重启前的帧
/// 携带的序号，渲染侧会把新帧误判成「早于在途命令」而永久不呈现（画面冻结）。
#[test]
fn scroll_request_id_is_process_wide_and_never_reused() {
    let first_terminal = GhosttyTerminal::new(5, 10, 100).expect("terminal");
    let second_terminal = GhosttyTerminal::new(5, 10, 100).expect("terminal");

    assert!(first_terminal.scroll_viewport(-1), "首个滚动应被接受");
    let first_id = first_terminal.latest_scroll_request_id();
    assert!(second_terminal.scroll_viewport(-1), "第二个滚动应被接受");
    let second_id = second_terminal.latest_scroll_request_id();

    assert!(
        second_id > first_id,
        "序号必须进程级递增、跨会话不复用：{second_id} 应大于 {first_id}"
    );
}
