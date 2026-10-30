use super::*;

/// 从快照中取出指定行列的单元
#[test]
fn create_terminal_zero_scrollback() {
    let terminal_under_test = GhosttyTerminal::new(5, 10, 0).expect("terminal");
    assert_eq!(terminal_under_test.scrollback_length(), 0);
}

#[test]
fn read_line_text_returns_text() {
    let mut terminal_under_test = terminal();
    terminal_under_test.vt_write(b"\x1b[1;1HHello World");
    terminal_under_test.flush();
    let text = terminal_under_test.read_line_text(0);
    assert!(text.is_some());
    assert!(text.unwrap().contains("Hello"));
}

#[test]
fn read_line_text_empty_returns_none() {
    let terminal_under_test = terminal();
    let text = terminal_under_test.read_line_text(5);
    assert!(text.is_none());
}

/// 行文本的字符下标恒等于网格列（每列恰好一个字符），宽字符尾格是空格占位。
/// Kotlin 侧据此按列直取字符，不再用宽度表反推。
#[test]
fn read_line_text_index_equals_grid_column_for_wide_chars() {
    let mut terminal_under_test = terminal();
    terminal_under_test.vt_write("中文AB".as_bytes());
    terminal_under_test.flush();
    let text = terminal_under_test.read_line_text(0).expect("row has text");
    let columns: Vec<char> = text.chars().collect();
    assert_eq!(
        columns,
        vec!['中', ' ', '文', ' ', 'A', 'B'],
        "宽字符尾格占一列空格，字符下标即列号"
    );
}

/// 宽字符的单元宽度取自上游 VT 的宽度表，本仓按该宽度推进列号：
/// 双宽字符占两列，其后的单元列号须跳过这两列。
#[test]
fn cell_iterator_cjk_double_width() {
    let mut terminal_under_test = GhosttyTerminal::new(5, 10, 100).expect("terminal");
    terminal_under_test.vt_write("A中B".as_bytes());
    terminal_under_test.flush();
    let snap = terminal_under_test.take_snapshot();

    let a_cell = &snap.cells[0];
    assert_eq!(a_cell.codepoint, 'A' as u32, "Cell 0 should be 'A'");
    assert_eq!(a_cell.width, 1, "'A' should have width=1");

    let cjk_cell = &snap.cells[1];
    assert_eq!(cjk_cell.codepoint, 0x4E2D, "Cell 1 should be CJK '中'");
    assert_eq!(cjk_cell.width, 2, "CJK should have width=2");

    let b_cell = &snap.cells[3];
    assert_eq!(b_cell.codepoint, 'B' as u32, "Cell 3 should be 'B'");
    assert_eq!(b_cell.width, 1, "'B' should have width=1");
}

#[test]
fn wide_char_tail_cols_marks_only_trailing_halves() {
    let mut terminal_under_test = terminal();
    terminal_under_test.vt_write("中文AB".as_bytes());
    terminal_under_test.flush();
    // 「中」占列 0..1、「文」占列 2..3：两个尾格都被标出，窄字符与起始格不标。
    assert_eq!(terminal_under_test.wide_char_tail_cols(0), vec![1, 3]);
}

/// 纯 ASCII 行没有尾格（无吸附），行文本的列映射因而与字符下标一致。
#[test]
fn wide_char_tail_cols_is_empty_for_narrow_text() {
    let mut terminal_under_test = terminal();
    terminal_under_test.vt_write(b"hello");
    terminal_under_test.flush();
    assert!(terminal_under_test.wide_char_tail_cols(0).is_empty());
}

/// 软换行续行的行首是 `SpacerHead`：它不是尾格，不得被标出——其前一列属于折行前
/// 的上一行，把它当尾格会把选区跨过折行边界。
#[test]
fn wide_char_tail_cols_ignores_wrapped_row_head() {
    let mut terminal_under_test = GhosttyTerminal::new(5, 10, 100).expect("terminal");
    terminal_under_test.vt_write("0123456789中".as_bytes());
    terminal_under_test.flush();
    // 10 列宽：首行是「0123456789」，宽字符折到续行的第 0..1 列。
    let first_row_tails = terminal_under_test.wide_char_tail_cols(0);
    let continuation_row_tails = terminal_under_test.wide_char_tail_cols(1);
    assert!(first_row_tails.is_empty(), "首行全窄字符，不应有尾格");
    assert!(
        !continuation_row_tails.contains(&0),
        "续行行首不得被当作尾格（会把选区吸过折行边界）"
    );
    assert!(
        continuation_row_tails.contains(&1),
        "续行里的宽字符尾格仍须标出"
    );
}

#[test]
fn reset_clears_grid_and_scrollback() {
    let mut terminal = GhosttyTerminal::new(5, 20, 100).expect("terminal");
    terminal.vt_write(b"reset_marker_xyz\nsecond line");
    terminal.flush();
    assert!(terminal.read_visible_text().contains("reset_marker_xyz"));
    terminal.reset();
    terminal.flush();
    assert!(
        !terminal.read_visible_text().contains("reset_marker_xyz"),
        "RIS 重置必须清屏"
    );
    assert_eq!(terminal.scrollback_length(), 0);
}

#[test]
fn reset_clears_selection() {
    let mut terminal = GhosttyTerminal::new(5, 20, 100).expect("terminal");
    terminal.vt_write(b"hello");
    terminal.flush();
    let snap = terminal.take_snapshot();
    let row = snap.scrollback_length;
    terminal.set_selection((row, 0), (row, 4));
    terminal.flush();
    terminal.reset();
    terminal.flush();
    let (cells, _) = terminal.receive_cell_data().expect("cell data after reset");
    let picked = cells
        .iter()
        .find(|cell| cell.row == 0 && cell.col == 0)
        .expect("row 0 col 0 present");
    let theme_foreground = default_foreground();
    assert_eq!(
        picked.foreground, theme_foreground,
        "重置后选区反白必须清除"
    );
}

#[test]
fn search_finds_match_and_keeps_terminal_usable() {
    let mut terminal_under_test = GhosttyTerminal::new(3, 80, 100).expect("terminal");
    terminal_under_test.vt_write(b"search_target_here\n");
    terminal_under_test.flush();
    for line_number in 0..5 {
        terminal_under_test.vt_write(format!("filler {line_number}\n").as_bytes());
    }
    terminal_under_test.flush();
    // 搜索能定位到已滚入历史的 needle，且终端保持可用。
    assert!(
        terminal_under_test.search_set_needle("search_target") > 0,
        "search_target must be found in scrollback"
    );
    terminal_under_test.vt_write(b"AfterSearch");
    terminal_under_test.flush();
    let snap = terminal_under_test.take_snapshot();
    assert!(
        snap.cells.iter().any(|cell| cell.codepoint == 'A' as u32),
        "terminal should remain functional after scrollback search"
    );
    assert_invariants(&snap);
}

/// 软换行的逻辑行只算一次，且命中按其真实所在的物理行与列返回：
/// 逐物理行拼接会产生 N 份重复命中，并把续接段的命中记到行首那一行。
#[test]
fn search_reports_each_wrapped_line_once_with_physical_rows() {
    const COLS: u32 = 10;
    let mut terminal_under_test = GhosttyTerminal::new(5, COLS, 100).expect("terminal");
    // 26 字符在 10 列网格上软换行为 3 个物理行：abcdefghij / klmnopqrst / uvwxyz
    terminal_under_test.vt_write(b"abcdefghijklmnopqrstuvwxyz\r\n");
    terminal_under_test.flush();
    assert_eq!(
        terminal_under_test.read_line_text(0).as_deref(),
        Some("abcdefghij")
    );
    assert_eq!(
        terminal_under_test.read_line_text(1).as_deref(),
        Some("klmnopqrst")
    );

    assert_eq!(
        search_viewport_matches(&terminal_under_test, "klmnopqrst"),
        vec![SearchMatch {
            row: 1,
            start_col: 0,
            end_col: 10,
        }],
        "单个物理行内的命中不得按逻辑行重复上报"
    );

    // 跨软换行的命中按物理行拆成两段，列区间落在各自行内。
    assert_eq!(
        search_viewport_matches(&terminal_under_test, "ijklm"),
        vec![
            SearchMatch {
                row: 0,
                start_col: 8,
                end_col: 10,
            },
            SearchMatch {
                row: 1,
                start_col: 0,
                end_col: 3,
            },
        ],
        "跨软换行的命中必须按各物理行拆分"
    );
}

#[test]
fn search_empty_needle_returns_nothing() {
    let terminal_under_test = terminal();
    assert_eq!(terminal_under_test.search_set_needle(""), 0);
    assert!(search_viewport_matches(&terminal_under_test, "").is_empty());
}

#[test]
fn dump_grid_dimensions_match() {
    let terminal_under_test = terminal();
    // 查询经工作线程超时回退空值，新终端繁忙时单次查询可能命中回退；
    // 确定性轮询直到就绪，杜绝 flaky。
    let start = Instant::now();
    let dumped = loop {
        let dumped = terminal_under_test.dump_grid();
        if dumped.rows == 24 && dumped.cols == 80 {
            break dumped;
        }
        assert!(
            start.elapsed() < std::time::Duration::from_secs(5),
            "dump_grid 未就绪：rows={} cols={}",
            dumped.rows,
            dumped.cols
        );
        std::thread::sleep(std::time::Duration::from_millis(10));
    };
    assert_eq!(dumped.visible.len(), (24 * 80) as usize);
    let _snap = terminal_under_test.take_snapshot();
    assert_invariants(&_snap);
}

/// 滚出屏幕的单元必须保留其显式样式，回滚区复制与高亮依赖这一点。
#[test]
fn scrollback_retains_explicit_cell_style() {
    let mut terminal_under_test = terminal();
    terminal_under_test.vt_write(b"\x1b[38;2;255;0;0mred\x1b[0m\r\n");
    for index in 0..26 {
        terminal_under_test.vt_write(format!("line{index}\r\n").as_bytes());
    }
    terminal_under_test.flush();

    let start = Instant::now();
    let dumped = loop {
        let dumped = terminal_under_test.dump_grid();
        if !dumped.scrollback.is_empty() {
            break dumped;
        }
        assert!(
            start.elapsed() < READY_POLL_TIMEOUT,
            "回滚区未在超时内产生内容"
        );
        std::thread::sleep(READY_POLL_INTERVAL);
    };

    let first = dumped.scrollback.first().expect("回滚区首行");
    let red = first
        .iter()
        .find(|cell| cell.codepoint == 'r' as u32)
        .expect("回滚区首行应含写入的字符");
    assert_eq!(red.foreground, [1.0, 0.0, 0.0, 1.0]);
}

#[test]
fn dump_grid_visible_populated() {
    let mut terminal_under_test = terminal();
    terminal_under_test.vt_write(b"hello");
    terminal_under_test.flush();
    let dumped = terminal_under_test.dump_grid();
    let has_h_character = dumped
        .visible
        .iter()
        .any(|cell| cell.codepoint == 'h' as u32);
    assert!(
        has_h_character,
        "dump_grid visible: 'h' from 'hello' should be present"
    );
    let _snap = terminal_under_test.take_snapshot();
    assert_invariants(&_snap);
}

#[test]
fn dump_grid_scrollback_populated_after_scroll() {
    let mut terminal_under_test = GhosttyTerminal::new(3, 10, 100).expect("terminal");
    for line_number in 0..10 {
        terminal_under_test.vt_write(format!("line{line_number}\n").as_bytes());
    }
    terminal_under_test.flush();
    let dumped = terminal_under_test.dump_grid();
    assert!(
        !dumped.scrollback.is_empty(),
        "scrollback should contain scrolled-off lines"
    );
    let has_line0 = dumped
        .scrollback
        .iter()
        .any(|row| row.iter().any(|cell| cell.codepoint == 'l' as u32));
    assert!(has_line0, "scrollback: should contain 'l' from line0");
    let _snap = terminal_under_test.take_snapshot();
    assert_invariants(&_snap);
}

/// `read_all_text` 取代 `getTerminalText` 的 `dump_grid` 路径：文本必须逐行等价。
///
/// `dump_grid` 丢弃 codepoint 0（宽字符尾格不计字符）而 `read_line_text_impl` 以空格
/// 占位，两者在行尾 `trim_end` 后同形；行内宽字符尾格处 `dump_grid` 少一个字符，
/// 故此处的等价性以「行尾去空白后的行文本」为准。
#[test]
fn read_all_text_matches_dump_grid_lines() {
    let mut terminal_under_test = GhosttyTerminal::new(6, 20, 100).expect("terminal");
    for line_number in 0..20 {
        terminal_under_test.vt_write(format!("行{line_number}tail\n").as_bytes());
    }
    terminal_under_test.flush();
    let dumped = terminal_under_test.dump_grid();
    let from_grid: Vec<String> = dumped
        .scrollback
        .iter()
        .map(|row| {
            row.iter()
                .filter_map(|cell| char::from_u32(cell.codepoint).filter(|&c| c != '\0'))
                .collect::<String>()
                .trim_end()
                .to_string()
        })
        .chain((0..dumped.rows as usize).map(|row| {
            let start = row * dumped.cols as usize;
            let end = (start + dumped.cols as usize).min(dumped.visible.len());
            dumped.visible[start..end]
                .iter()
                .filter_map(|cell| char::from_u32(cell.codepoint).filter(|&c| c != '\0'))
                .collect::<String>()
                .trim_end()
                .to_string()
        }))
        .collect();
    let from_text: Vec<String> = terminal_under_test
        .read_all_text()
        .lines()
        .map(str::to_string)
        .collect();
    assert_eq!(
        from_text.len(),
        from_grid.len(),
        "行数必须一致（read_all_text={} dump_grid={}）",
        from_text.len(),
        from_grid.len()
    );
    assert_eq!(from_text, from_grid, "逐行文本必须一致");
    assert!(
        from_text.iter().any(|line| line.contains("行0tail")),
        "回滚内容必须可读：{from_text:?}"
    );
}

/// 回归护栏：回滚填满时单次 `read_all_text` 必须留在查询超时之内。
///
/// `read_all_text` 逐格走 ghostty FFI；回滚填到上限（2000 行 × 80 列）时一次重建
/// 曾逼近 `QUERY_TIMEOUT_MS`，调用方读到的却是超时后的空值——外部表现为「标记不落格」。
/// 网格不变时 VT 线程直接复用缓存，故重复查询必须是微秒级。
#[test]
fn read_all_text_answers_repeated_queries_within_query_timeout() {
    use super::super::types::QUERY_TIMEOUT_MS;
    const COLUMNS: u32 = 80;
    const ROWS: u32 = 24;
    const MAX_SCROLLBACK: u32 = 500;
    let mut terminal_under_test =
        GhosttyTerminal::new(ROWS, COLUMNS, MAX_SCROLLBACK).expect("terminal");
    for line_number in 0..MAX_SCROLLBACK + ROWS {
        terminal_under_test.vt_write(format!("line{line_number}\n").as_bytes());
    }
    terminal_under_test.flush();
    // ghostty 会自行裁剪回滚，故只断言「已大幅填满」而非精确上限。
    //
    // 与文件内其余护栏同口径按期限轮询：系统过载（本机全量套件并行、深缓冲
    // 用例同时段在跑）时 VT 线程可能整体错过一个 `QUERY_TIMEOUT_MS` 窗口，
    // 一次性读到兜底值 0（CI 尚未复现本现象）；轮询把「一次读不到」与
    // 「回滚确实没填满」区分开，配合查询端的重试兜底双保险。
    let deadline = Instant::now() + std::time::Duration::from_secs(5);
    let mut filled = terminal_under_test.scrollback_length();
    while filled <= MAX_SCROLLBACK / 2 && Instant::now() < deadline {
        std::thread::sleep(std::time::Duration::from_millis(10));
        filled = terminal_under_test.scrollback_length();
    }
    assert!(
        filled > MAX_SCROLLBACK / 2,
        "回滚必须大幅填满，护栏才覆盖最坏情况（实际={filled}）"
    );

    let first = terminal_under_test.read_all_text();
    assert!(
        first.contains("line0"),
        "首次查询必须拿到内容而非超时空值（长度={}）",
        first.len()
    );

    // 网格未变：内容必须逐轮一致（缓存复用而非陈旧重建）。
    for round in 0..5 {
        assert_eq!(
            terminal_under_test.read_all_text(),
            first,
            "第 {round} 轮内容必须与首轮一致"
        );
    }
    // 缓存命中时单轮必须留在查询期限内。VT 线程收到查询即应答（不再等空闲节拍），
    // 越过 500ms 说明查询排不上 VT 线程——回滚填满时的全量重建正是这条路。
    let started = Instant::now();
    let cached = terminal_under_test.read_all_text();
    let elapsed = started.elapsed();
    assert_eq!(cached, first, "缓存命中时内容必须一致");
    assert!(
        elapsed < std::time::Duration::from_millis(QUERY_TIMEOUT_MS),
        "缓存命中的单轮耗时 {elapsed:?}，越过查询期限 {QUERY_TIMEOUT_MS}ms"
    );

    // 网格一变缓存必须失效，否则调用方会读到陈旧内容。
    terminal_under_test.vt_write(b"fresh-marker\n");
    terminal_under_test.flush();
    // 网格一变缓存必须失效，否则调用方会读到陈旧内容。
    //
    // 这里是失效后的第一趟**未缓存**重建：逐行走 ghostty FFI，机器被压满时
    // VT 线程可能错过单次 `QUERY_TIMEOUT_MS`，`query_on` 便按契约返回空串兜底
    // ——但重建结果已经落进缓存，下一次查询必然命中（实测未压满约 100ms，
    // 远低于期限）。故按期限重试，把「一次超时」与「缓存永不失效」区分开：
    // 后者重试到期限也读不到新内容。
    let deadline = Instant::now() + std::time::Duration::from_secs(5);
    let mut text = String::new();
    while !text.contains("fresh-marker") && Instant::now() < deadline {
        text = terminal_under_test.read_all_text();
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    assert!(
        text.contains("fresh-marker"),
        "网格变动后缓存必须失效（重试到期限仍未读到新内容，长度={}）",
        text.len()
    );
}

/// 空闲 VT 线程必须即时应答查询：查询延迟不得被空闲节拍串行化。
///
/// VT 线程原本只在空闲节拍（`VT_IDLE_TICK_MS`）到点时排空查询通道，无命令在途时
/// 一次查询要等满整个节拍。渲染线程的 kitty 放置查询走的正是这条路：每等满一个
/// 节拍就丢掉一帧（真机实测 ~54ms 的慢帧即此）。
#[test]
fn idle_vt_thread_answers_queries_without_waiting_for_tick() {
    use super::super::types::VT_IDLE_TICK_MS;
    const QUERY_ROUNDS: usize = 8;
    let terminal_under_test = terminal();
    // 让 VT 线程进入空闲等待：节拍自此刻起算。
    std::thread::sleep(std::time::Duration::from_millis(20));
    let started = Instant::now();
    for _ in 0..QUERY_ROUNDS {
        let _title = terminal_under_test.title();
    }
    let elapsed = started.elapsed();
    // 一个节拍只够服务一轮查询：被节拍串行化的实现至少要等 7 个节拍（~350ms）。
    assert!(
        elapsed < std::time::Duration::from_millis(2 * VT_IDLE_TICK_MS),
        "{QUERY_ROUNDS} 次查询总耗时 {elapsed:?}，必须小于两个空闲节拍（{}ms）",
        2 * VT_IDLE_TICK_MS
    );
}

// ── DECSET/DECRST ──────────────────────────────────────────────────────

/// EncodeMouseEvent with no tracking mode enabled must return empty (the
/// application never asked for mouse reporting) — zelland renderer/mod.rs
/// drops mouse events when `get_mouse_mode()` is false.
#[test]
fn encode_mouse_event_gated_off_without_tracking_mode() {
    let terminal_under_test = terminal();
    let encoded = terminal_under_test.encode_mouse_event((50.0, 60.0), 0, 0, 0, 10.0, 20.0);
    let encoded = encoded.expect("encode_mouse_event should return Some");
    assert!(
        encoded.is_empty(),
        "mouse event must be dropped when tracking is off (got {encoded:?})"
    );
}

/// DECSET 1000 + SGR format (DECSET 1006) → left-click press at pixel
/// (35,45) with 10x20 cells must produce a valid SGR mouse sequence for
/// cell (3,2). Matches ghostty's standard SGR encoding.
#[test]
fn encode_mouse_event_sgr_press() {
    let mut terminal_under_test = terminal();
    terminal_under_test.vt_write(b"\x1b[?1000h\x1b[?1006h");
    terminal_under_test.flush();
    let encoded = terminal_under_test
        .encode_mouse_event((35.0, 45.0), 0, 0, 0, 10.0, 20.0)
        .expect("encode_mouse_event should return Some");
    // SGR: ESC [ < Cb ; Cx ; Cy M — Cb is the 0-based button (0 = left
    // press; X10's +32 offset does NOT apply to SGR mode).
    // Cell (3,2) → Cx=3+1=4, Cy=2+1=3.
    assert_eq!(
        encoded, b"\x1b[<0;4;3M",
        "SGR left-press at cell (3,2) must match ghostty's standard encoding"
    );
}

/// Shift/Ctrl 点击必须与普通左键在编码结果上可区分（vim/tmux 依此扩展选区、
/// 粘贴选择）。无修饰时 Cb=0，Shift 为 +4，Ctrl 为 +16（xterm SGR 口径）。
#[test]
fn encode_mouse_event_carries_modifiers() {
    let mut terminal_under_test = terminal();
    terminal_under_test.vt_write(b"\x1b[?1000h\x1b[?1006h");
    terminal_under_test.flush();
    let plain = terminal_under_test
        .encode_mouse_event((35.0, 45.0), 0, 0, Mods::empty().bits(), 10.0, 20.0)
        .expect("plain left-press encode");
    let shift = terminal_under_test
        .encode_mouse_event((35.0, 45.0), 0, 0, Mods::SHIFT.bits(), 10.0, 20.0)
        .expect("shift left-press encode");
    let ctrl = terminal_under_test
        .encode_mouse_event((35.0, 45.0), 0, 0, Mods::CTRL.bits(), 10.0, 20.0)
        .expect("ctrl left-press encode");
    assert_eq!(plain, b"\x1b[<0;4;3M");
    assert_ne!(
        plain, shift,
        "Shift+左键必须与普通左键编码不同（否则远端无法扩展选区）"
    );
    assert_ne!(
        plain, ctrl,
        "Ctrl+左键必须与普通左键编码不同（否则远端无法粘贴选择）"
    );
    assert_ne!(shift, ctrl, "Shift 与 Ctrl 组合不得互相混淆");
}

/// 一次调用编码 `repeat` 档滚轮：输出必须是单档序列的整数倍，且档位按下→释放成对。
///
/// 触摸拖动一个事件常滚过十余行，逐档 `encode_mouse_event` 会让 UI 线程等十余次
/// VT 应答——这正是「滚动卡顿」的直接来源。批量接口必须让一次往返产出全部档位。
#[test]
fn encode_mouse_wheel_batches_every_notch() {
    let mut terminal_under_test = terminal();
    terminal_under_test.vt_write(b"\x1b[?1000h\x1b[?1006h");
    terminal_under_test.flush();
    let single = terminal_under_test
        .encode_mouse_wheel((10.0, 10.0), 3, 0, 10.0, 20.0, 1)
        .expect("wheel encode");
    assert!(!single.is_empty(), "tracking on: one notch must encode");
    for repeat in [2u32, 5, 12] {
        let batched = terminal_under_test
            .encode_mouse_wheel((10.0, 10.0), 3, 0, 10.0, 20.0, repeat)
            .expect("wheel batch encode");
        assert_eq!(
            batched.len(),
            single.len() * repeat as usize,
            "repeat={repeat} 必须等于单档序列的整数倍"
        );
        assert_eq!(
            batched,
            single.repeat(repeat as usize),
            "repeat={repeat} 必须逐档重复同一 press+release 对"
        );
    }
    // 上报关闭时整批丢弃：调用方据此回落到本地回滚。
    let plain = terminal();
    plain.flush();
    let dropped = plain
        .encode_mouse_wheel((10.0, 10.0), 3, 0, 10.0, 20.0, 8)
        .expect("no-tracking encode still returns Some");
    assert!(dropped.is_empty(), "tracking off:整批必须丢弃（不写 PTY）");
}

/// Wheel-up with DECSET 1000 + SGR → button 4 (wheel-up = button 64+4-32).
/// The Ghostty encoder emits button 4 for wheel-up; SGR adds 32 for press.
#[test]
fn encode_mouse_event_wheel() {
    let mut terminal_under_test = terminal();
    terminal_under_test.vt_write(b"\x1b[?1000h\x1b[?1006h");
    terminal_under_test.flush();
    let up = terminal_under_test
        .encode_mouse_event((10.0, 10.0), 0, 3, 0, 10.0, 20.0)
        .expect("wheel-up encode");
    assert!(
        up.len() >= 6 && up.starts_with(b"\x1b[<"),
        "wheel-up must produce an SGR sequence (got {up:?})"
    );
    let down = terminal_under_test
        .encode_mouse_event((10.0, 10.0), 0, 4, 0, 10.0, 20.0)
        .expect("wheel-down encode");
    assert!(
        down.len() >= 6 && down.starts_with(b"\x1b[<"),
        "wheel-down must produce an SGR sequence (got {down:?})"
    );
}

/// Negative pixel coordinates must be handled gracefully — no panic,
/// no underflow, no out-of-range.  The Ghostty encoder may drop the
/// event (empty Vec) when coordinates are negative, which is correct
/// behavior — the application should clamp before calling this.
#[test]
fn encode_mouse_event_bounds_negative_clamp() {
    let mut terminal_under_test = terminal();
    terminal_under_test.vt_write(b"\x1b[?1000h\x1b[?1006h");
    terminal_under_test.flush();
    let result = terminal_under_test.encode_mouse_event((-5.0, -10.0), 0, 0, 0, 10.0, 20.0);
    // The encoder returns Some(empty) or Some(sgr) — it must not panic.
    assert!(
        result.is_some(),
        "negative coords must return Some (not None = tracking off)"
    );
    // Ghostty encoder drops events for negative coordinates.
    // This is correct — the Kotlin caller must clamp before calling.
    let encoded = result.unwrap();
    // No crash is the primary assertion. If it produces output, it
    // must be valid SGR.
    if !encoded.is_empty() {
        assert!(
            encoded.starts_with(b"\x1b[<"),
            "if output is produced, must be SGR (got {encoded:?})"
        );
    }
}

/// Oversized pixel coordinates (beyond the grid) must be handled
/// gracefully — no panic, no overflow.
#[test]
fn encode_mouse_event_bounds_oversized_clamp() {
    let mut terminal_under_test = terminal(); // default 24 rows × 80 cols
    terminal_under_test.vt_write(b"\x1b[?1000h\x1b[?1006h");
    terminal_under_test.flush();
    // Position far beyond the grid: 9999x9999 with 10x20 cells.
    let result = terminal_under_test.encode_mouse_event((9999.0, 9999.0), 0, 0, 0, 10.0, 20.0);
    assert!(result.is_some(), "oversized coords must return Some");
    let encoded = result.unwrap();
    if !encoded.is_empty() {
        let text = String::from_utf8_lossy(&encoded);
        assert!(
            text.starts_with("\x1b[<"),
            "if output is produced, must be SGR (got {text})"
        );
        // Parse and verify no overflow — coordinates must be finite integers.
        let inside = text.trim_start_matches("\x1b[<");
        let parts: Vec<&str> = inside.trim_end_matches('M').split(';').collect();
        assert_eq!(parts.len(), 3, "SGR must have 3 parts: {text}");
        let col: u32 = parts[1].parse().expect("col must be numeric");
        let row: u32 = parts[2].parse().expect("row must be numeric");
        assert!(col < 1000, "col must be reasonable, got {col}");
        assert!(row < 1000, "row must be reasonable, got {row}");
    }
}

/// 完整的按下 → 拖动 → 释放序列：三个 x 递增的事件都须产出合法 SGR，
/// 且动作字节必须变化（0=按下、2=移动、1=释放）。
#[test]
fn encode_mouse_event_drag_sequence() {
    let mut terminal_under_test = terminal();
    terminal_under_test.vt_write(b"\x1b[?1000h\x1b[?1006h"); // button tracking + SGR
    terminal_under_test.vt_write(b"\x1b[?1002h"); // button-event tracking (motion reports)
    terminal_under_test.flush();
    let cell_width = 10.0;
    let cell_height = 20.0;

    // Press at (10, 20)
    let press = terminal_under_test
        .encode_mouse_event((10.0, 20.0), 0, 0, 0, cell_width, cell_height)
        .expect("press encode");
    assert!(
        press.starts_with(b"\x1b[<"),
        "press must produce SGR (got {press:?})"
    );
    assert!(
        press.ends_with(b"M"),
        "press must end with M (got {press:?})"
    );

    // Drag (motion) at (30, 20) — action=2
    let drag = terminal_under_test
        .encode_mouse_event((30.0, 20.0), 2, 0, 0, cell_width, cell_height)
        .expect("drag encode");
    assert!(
        drag.starts_with(b"\x1b[<"),
        "drag must produce SGR (got {drag:?})"
    );

    // Release at (50, 20) — action=1
    let release = terminal_under_test
        .encode_mouse_event((50.0, 20.0), 1, 0, 0, cell_width, cell_height)
        .expect("release encode");
    assert!(
        release.starts_with(b"\x1b[<"),
        "release must produce SGR (got {release:?})"
    );

    // Column should increase across the sequence (10→30→50 px = 1→3→5 cell).
    let parse_col = |seq: &[u8]| -> u32 {
        let text = String::from_utf8_lossy(seq);
        let inside = text.trim_start_matches("\x1b[<");
        inside
            .trim_end_matches('M')
            .split(';')
            .nth(1)
            .unwrap()
            .parse()
            .unwrap()
    };
    let col_press = parse_col(&press);
    let col_drag = parse_col(&drag);
    let col_release = parse_col(&release);
    assert!(
        col_press < col_drag && col_drag < col_release,
        "columns must increase across drag: press={col_press}, drag={col_drag}, release={col_release}"
    );
}

/// 滑出视口的拖动必须上报：上游 mouse_encode 只在有键按下时报告
/// 视口外 motion（1003 全事件跟踪），标志缺省 false 会静默丢弃，
/// 手指滑出终端边缘的拖动在远端看来直接中断。
#[test]
fn encode_mouse_event_out_of_viewport_drag_is_reported() {
    let mut terminal_under_test = terminal();
    terminal_under_test.vt_write(b"\x1b[?1000h\x1b[?1006h");
    terminal_under_test.vt_write(b"\x1b[?1003h"); // any-event tracking
    terminal_under_test.flush();
    let drag = terminal_under_test
        .encode_mouse_event((-5.0, -10.0), 2, 0, 0, 10.0, 20.0)
        .expect("drag encode");
    assert!(
        drag.starts_with(b"\x1b[<"),
        "视口外拖动必须上报 SGR（got {drag:?}）"
    );
}

// ── OSC split-buffer tests ──────────────────────────────────────────────

/// OSC 0 title — split across two writes (true reassembly: the second half
/// must be consumed by the OSC, not rendered as text).
#[test]
fn osc_title_split_buffer() {
    let mut terminal_under_test = terminal();
    // 依次写入 OSC 0 序列的第一段与第二段
    terminal_under_test.vt_write(b"\x1b]0;My ");
    terminal_under_test.flush();
    terminal_under_test.vt_write(b"QQQ\x07");
    terminal_under_test.flush();
    let _snap = terminal_under_test.take_snapshot();
    // 设置标题后终端不得崩溃，文本仍须可写
    terminal_under_test.vt_write(b"AfterTitle");
    terminal_under_test.flush();
    let snap2 = terminal_under_test.take_snapshot();
    let found = snap2.cells.iter().any(|cell| cell.codepoint == 'A' as u32);
    assert!(found, "OSC split: text after split title should render");
    let leaked = snap2.cells.iter().any(|cell| cell.codepoint == 'Q' as u32);
    assert!(
        !leaked,
        "OSC split: title second half must be consumed, not rendered"
    );
    assert_invariants(&snap2);
}

/// OSC 52 clipboard — sent across split buffer (true reassembly: the payload
/// must reach the clipboard callback, not the grid).
#[test]
fn osc_clipboard_split_buffer() {
    let mut terminal_under_test = terminal();
    // OSC 52 sequence: first part sets clipboard selection, second provides data.
    terminal_under_test.vt_write(b"\x1b]52;c;");
    terminal_under_test.flush();
    terminal_under_test.vt_write(b"SGVsbG8=\x07");
    terminal_under_test.flush();
    // 分片必须由上游重组：剪贴板事件内容为解码后文本。
    let event = terminal_under_test.poll_clipboard_event();
    assert_eq!(
        event,
        Some(("c".to_string(), "Hello".to_string())),
        "OSC 52 split: payload must reassemble into clipboard event"
    );
    // 终端不得崩溃，文本仍须可写
    terminal_under_test.vt_write(b"PostClip");
    terminal_under_test.flush();
    let snap = terminal_under_test.take_snapshot();
    let found = snap.cells.iter().any(|cell| cell.codepoint == 'P' as u32);
    assert!(found, "OSC 52 split: post-clipboard text should render");
    assert_invariants(&snap);
}

/// OSC 52 clipboard via pty_write (the production output path:
/// Session::poll_pty_output feeds PTY bytes through pty_write, NOT
/// vt_write). LF→CRLF conversion must not corrupt the sequence.
#[test]
fn osc_clipboard_via_pty_write_path() {
    let mut terminal_under_test = terminal();
    terminal_under_test.pty_write(b"\x1b]52;c;SGVsbG8=\x07");
    terminal_under_test.flush();
    let event = terminal_under_test.poll_clipboard_event();
    assert_eq!(
        event,
        Some(("c".to_string(), "Hello".to_string())),
        "OSC 52 via pty_write must reach the clipboard callback"
    );
}

/// OSC 52 以 ST（`ESC \\`）终止时剪贴板事件 MUST 同样到达。
///
/// 判别力：只覆盖 BEL 结尾时，ST 结尾的序列（部分客户端如此发）会被静默丢弃，
/// 用户侧表现是「复制到剪贴板没反应」。
#[test]
fn osc_clipboard_st_terminated() {
    let mut terminal_under_test = terminal();
    terminal_under_test.vt_write(b"\x1b]52;c;SGVsbG8=\x1b\\");
    terminal_under_test.flush();
    assert_eq!(
        terminal_under_test.poll_clipboard_event(),
        Some(("c".to_string(), "Hello".to_string())),
        "OSC 52 以 ST 终止时剪贴板事件必须到达"
    );
}

/// 与剪贴板无关的 OSC MUST NOT 产生剪贴板事件。
///
/// 判别力：侧扫描器若把任意 OSC 都当剪贴板处理，标题/调色板之类的序列会污染
/// 用户剪贴板。
#[test]
fn unrelated_osc_produces_no_clipboard_event() {
    let mut terminal_under_test = terminal();
    terminal_under_test.vt_write(b"\x1b]0;unrelated\x07");
    terminal_under_test.flush();
    assert_eq!(
        terminal_under_test.poll_clipboard_event(),
        None,
        "标题等无关 OSC 不得触发剪贴板事件"
    );
}

/// 非法 base64 的 OSC 52 MUST NOT 产生剪贴板事件，终端仍须可用。
///
/// 判别力：解码失败若被当作空写入或半截写入上报，剪贴板会被垃圾内容覆盖。
#[test]
fn osc_clipboard_invalid_base64_is_ignored() {
    let mut terminal_under_test = terminal();
    terminal_under_test.vt_write(b"\x1b]52;c;!!!not-base64!!!\x07");
    terminal_under_test.flush();
    assert_eq!(
        terminal_under_test.poll_clipboard_event(),
        None,
        "非法 base64 不得进入剪贴板"
    );
    terminal_under_test.vt_write(b"After");
    terminal_under_test.flush();
    let snap = terminal_under_test.take_snapshot();
    assert!(
        snap.cells.iter().any(|cell| cell.codepoint == 'A' as u32),
        "非法 OSC 52 之后的文本仍须渲染"
    );
    assert_invariants(&snap);
}

/// OSC color reset — sent across split buffer.
#[test]
fn osc_color_reset_split_buffer() {
    let mut terminal_under_test = terminal();
    terminal_under_test.vt_write(b"\x1b]104;");
    terminal_under_test.flush();
    terminal_under_test.vt_write(b"\x07");
    terminal_under_test.flush();
    terminal_under_test.vt_write(b"ColorReset");
    terminal_under_test.flush();
    let snap = terminal_under_test.take_snapshot();
    let found = snap.cells.iter().any(|cell| cell.codepoint == 'C' as u32);
    assert!(found, "OSC 104 split: text after color reset should render");
    assert_invariants(&snap);
}

/// OSC sequence terminated after partial first block — crash test
#[test]
fn osc_aborted_after_partial_feed() {
    let mut terminal_under_test = terminal();
    // 先写残缺的 OSC 序列，再用 BEL 终止它
    terminal_under_test.vt_write(b"H\x1b]0;Partial\x07");
    terminal_under_test.flush();
    // 随后正常写入，不得被 OSC 吞掉
    terminal_under_test.vt_write(b"Normal");
    terminal_under_test.flush();
    let snap = terminal_under_test.take_snapshot();
    let outer = snap.cells.iter().any(|cell| cell.codepoint == 'H' as u32);
    let normal = snap.cells.iter().any(|cell| cell.codepoint == 'N' as u32);
    assert!(outer, "aborted OSC: H should be visible before OSC");
    assert!(normal, "aborted OSC: Normal should be visible");
    assert_invariants(&snap);
}

/// Oversized OSC 52 payload — no crash
#[test]
fn osc_large_clipboard_payload_terminal_survives() {
    let mut terminal_under_test = terminal();
    let large = vec![b'A'; 1024 * 4]; // 4KB base64
    let mut seq = Vec::from(b"\x1b]52;c;");
    seq.extend_from_slice(&large);
    seq.push(b'\x07');
    terminal_under_test.vt_write(&seq);
    terminal_under_test.flush();
    terminal_under_test.vt_write(b"OK");
    terminal_under_test.flush();
    let snap = terminal_under_test.take_snapshot();
    let ok = snap.cells.iter().any(|cell| cell.codepoint == 'O' as u32);
    assert!(ok, "OSC large payload: OK should render");
    assert_invariants(&snap);
}

/// Extremely long 8KB OSC string — no crash
#[test]
fn osc_extremely_long_8kb_string() {
    let mut terminal_under_test = terminal();
    let mut seq = Vec::from(b"\x1b]0;");
    seq.extend(std::iter::repeat_n(b'x', 8000));
    seq.push(b'\x07');
    terminal_under_test.vt_write(&seq);
    terminal_under_test.flush();
    terminal_under_test.vt_write(b"LongDone");
    terminal_under_test.flush();
    let snap = terminal_under_test.take_snapshot();
    let found = snap.cells.iter().any(|cell| cell.codepoint == 'L' as u32);
    assert!(found, "OSC 8KB: LongDone should render");
    assert_invariants(&snap);
}

// ── Resize + CJK + SGR ─────────────────────────────────────────────────

// ── Scroll Region (DECSTBM + Origin Mode) ──────────────────────────────

// ── DECSC/DECRC cursor save/restore ─────────────────────────────────────

// ── SGR 24-bit color ────────────────────────────────────────────────────

// ── UTF-8 edge cases ────────────────────────────────────────────────────

// ── Wide char scroll tests (scroll preserves wide char attributes) ─────

// ── OSC color setting ──────────────────────────────────────────────────

// ── Resize stress ──────────────────────────────────────────────────────
