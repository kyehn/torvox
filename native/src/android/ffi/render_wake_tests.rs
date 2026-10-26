use std::sync::Arc;
use std::time::{Duration, Instant};

use super::RenderWake;

/// 代数语义：期间已通知过，带旧代数进场必须立即返回（不丢唤醒）。
#[test]
fn wait_returns_at_once_when_notified_before_entry() {
    let wake = RenderWake::default();
    assert_eq!(wake.wait(0, Duration::ZERO), 0);
    wake.notify();
    let start = Instant::now();
    assert_eq!(wake.wait(0, Duration::from_secs(5)), 1);
    assert!(
        start.elapsed() < Duration::from_millis(100),
        "已有通知时不应真的等待：{:?}",
        start.elapsed()
    );
}

/// 无通知时等满超时并返回同一代数——调用方据此照常落一帧，不 busy-spin。
#[test]
fn wait_times_out_and_keeps_generation() {
    let wake = RenderWake::default();
    let start = Instant::now();
    // 初始代数为 0：带 0 进场且无人通知 → 等满超时后仍返回 0。
    assert_eq!(wake.wait(0, Duration::from_millis(50)), 0);
    assert!(
        start.elapsed() >= Duration::from_millis(40),
        "应至少等满大部分超时：{:?}",
        start.elapsed()
    );
}

/// 跨线程通知：等待中的线程必须被立刻叫醒（PTY 读线程→渲染线程的真实形态）。
#[test]
fn notify_wakes_another_thread() {
    let wake = Arc::new(RenderWake::default());
    let waiter = {
        let wake = Arc::clone(&wake);
        std::thread::spawn(move || wake.wait(0, Duration::from_secs(5)))
    };
    std::thread::sleep(Duration::from_millis(50));
    wake.notify();
    assert_eq!(waiter.join().expect("waiter panicked"), 1);
}
