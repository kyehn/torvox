//! 命令批的有界性：持续输出下帧与查询都不得被饿死。
//!
//! 批尾是 VT 线程唯一的帧发布点与查询应答点。批若无界（只等命令通道排空），
//! 持续输出下命令通道恒非空：屏幕停在旧帧（剩余输出不可见），查询逐个超时。

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use super::*;

/// 把命令通道灌到满：通道恒非空正是无界批会卡死的场景。
///
/// 用 `try_send` 而非 `vt_write`：后者在通道满时静默丢弃，灌不满。
fn fill_command_backlog(terminal_under_test: &GhosttyTerminal) -> usize {
    let payload = b"x\r\n".to_vec();
    let mut queued = 0;
    while terminal_under_test
        .cmd_tx
        .try_send(Command::Write(payload.clone()))
        .is_ok()
    {
        queued += 1;
    }
    queued
}

/// 批条数有界：持续输出下批不得把整条积压一次吞掉。
///
/// 判别力：批尾是唯一的帧发布点与查询应答点，批无界时整条积压被一次吞掉，
/// 帧发布与查询应答都被推迟到通道排空为止——屏幕停在旧帧（剩余输出不可见）。
#[test]
fn command_batch_stays_bounded_under_backlog() {
    let terminal_under_test = GhosttyTerminal::new(24, 80, 1000).expect("terminal");
    let queued = fill_command_backlog(&terminal_under_test);
    assert!(queued > 0, "命令通道须能灌入积压");

    let deadline = Instant::now() + Duration::from_secs(10);
    while !terminal_under_test.cmd_tx.is_empty() && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(1));
    }
    assert_eq!(
        terminal_under_test.cmd_tx.len(),
        0,
        "命令积压须在期限内排空"
    );
    // flush 的应答在批尾释放：返回时整条积压都已处理完，批计数也已完整。
    terminal_under_test.flush();

    let (batch_count, max_batch_size) = terminal_under_test.take_batch_stats();
    assert!(
        max_batch_size <= super::super::internal::MAX_COMMANDS_PER_BATCH,
        "批条数须有界：实测最大 {max_batch_size}，上界 {}",
        super::super::internal::MAX_COMMANDS_PER_BATCH
    );
    // 积压被切成有界的若干批，而不是一次吞掉：帧发布与查询应答得以插在批间。
    assert!(
        batch_count * super::super::internal::MAX_COMMANDS_PER_BATCH >= queued as u64,
        "积压须被切成有界的若干批：{queued} 条命令只跑了 {batch_count} 批"
    );
}

/// 持续输出下查询仍被及时应答：不得等命令通道排空才应答。
///
/// 判别力：查询在批尾应答，批无界时批尾要等整条积压处理完——查询被推迟到
/// 超时（`QUERY_TIMEOUT_MS`）后回退到断连值，kitty 放置查询与标题查询因此逐个超时。
#[test]
fn query_is_answered_while_output_keeps_flowing() {
    // 行数取 7：断连回退值是 24，两者可区分，超时即被断言捕获。
    let terminal_under_test = GhosttyTerminal::new(7, 13, 100).expect("terminal");
    let stop = Arc::new(AtomicBool::new(false));
    let producer = std::thread::spawn({
        let cmd_tx = terminal_under_test.cmd_tx.clone();
        let stop = Arc::clone(&stop);
        move || {
            let payload = b"x\r\n".to_vec();
            while !stop.load(Ordering::Relaxed) {
                if cmd_tx.try_send(Command::Write(payload.clone())).is_err() {
                    std::thread::sleep(Duration::from_millis(1));
                }
            }
        }
    });

    let rows = terminal_under_test.rows();
    stop.store(true, Ordering::Relaxed);
    producer.join().expect("producer");

    assert_eq!(
        rows, 7,
        "持续输出下查询须拿到真实网格行数（超时回退值是 24）"
    );
}
