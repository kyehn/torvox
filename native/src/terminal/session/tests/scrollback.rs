use super::super::*;
use crate::terminal::mock_pty::MockPty;
use crate::terminal::pty::Pty;

/// 起一个 24x80 的会话（回滚行数按传入值），并写入足以越过默认回滚上限的行数。
///
/// 写入的行数须越过「活动区 + 默认回滚行数」，否则回滚区还没到上限，断言不出
/// 上限是否生效。
fn session_with_scrollback_lines(lines: u32) -> Session {
    let (pty, _handle) = MockPty::new(24, 80);
    let mut session = Session::spawn_with_pty(
        Box::new(pty) as Box<dyn Pty>,
        24,
        80,
        None,
        Some(std::env::temp_dir()),
        lines,
    )
    .expect("spawn failed");

    let mut payload = Vec::new();
    for index in 0..DEFAULT_SCROLLBACK_LINES + 24 + 200 {
        payload.extend_from_slice(format!("line-{index}\n").as_bytes());
    }
    session.inject_output_for_test(&payload);
    // 投递可能被 VT 命令通道容量挡回（留在 pending 里），故排几轮直到送进去。
    for _ in 0..4 {
        session.process_output();
    }
    // 30s 与同类排空用例（`batch_latency`）同口径：整套测试并行跑时，5s 会把
    // CPU 调度延迟误判成 VT 线程卡死。
    assert!(
        session
            .terminal()
            .flush_with_timeout(std::time::Duration::from_secs(30)),
        "VT 线程须在期限内排空"
    );
    session
}

/// 越界的回滚行数必须回退默认。
///
/// 上游的字节预算已解除（行数是回滚区唯一的约束），故行数一旦越界放行，回滚区
/// 可一路长到把进程内存吃光。越界回退默认而非钳到边界，与上游
/// `getDefaultIfNotInRange` 同口径。
#[test]
fn out_of_range_scrollback_lines_falls_back_to_the_default() {
    let session = session_with_scrollback_lines(MAX_SCROLLBACK_LINES + 1);
    let scrollback = session.terminal().scrollback_length();
    assert!(
        scrollback > 0,
        "前置：回滚区须确有内容，实测 {scrollback} 行"
    );
    assert!(
        scrollback <= DEFAULT_SCROLLBACK_LINES,
        "越界回滚行数必须回退默认（{DEFAULT_SCROLLBACK_LINES} 行），实测 {scrollback} 行"
    );
}

/// 区间内的回滚行数必须原样生效：收窄不得把合法的配置也折成默认值。
#[test]
fn in_range_scrollback_lines_is_honored() {
    let session = session_with_scrollback_lines(MAX_SCROLLBACK_LINES);
    let scrollback = session.terminal().scrollback_length();
    assert!(
        scrollback > DEFAULT_SCROLLBACK_LINES,
        "区间内的回滚行数必须按传入值生效，实测 {scrollback} 行"
    );
}
