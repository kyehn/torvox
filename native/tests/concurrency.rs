//! `EventQueue` 的并发不变量。
//!
//! 两个用例都在 shuttle 的调度器下穷举线程交错，因此本文件**必须是独立测试
//! 目标**：shuttle 的 `init_panic_hook` 用 `Once` 安装全局 panic 钩子，而钩子
//! 内部先调 `ExecutionState::failing_task()`，该调用在非 shuttle 线程上会自己
//! panic，于是同一进程内其余测试的真实断言信息会被 `Tried to get ExecutionState`
//! 顶掉。放进独立目标后钩子的作用域限于本进程，失败信息不再互相掩盖。

use std::sync::Arc;

use native::event::{Event, EventQueue};

/// 并发 push/pop：每个被压入的事件恰好被取出一次，不丢、不重、不死锁
/// （shuttle 穷举交错）。
#[test]
fn event_queue_concurrent_push_pop() {
    shuttle::check_random(
        || {
            let queue = Arc::new(EventQueue::new());
            const N: u64 = 32;
            const THREADS: usize = 4;

            let mut handles = Vec::new();
            for thread_index in 0..THREADS {
                let queue = Arc::clone(&queue);
                handles.push(shuttle::thread::spawn(move || {
                    for item in 0..N {
                        queue.push(Event::Clipboard {
                            session_id: thread_index as u64 * N + item,
                            text: String::new(),
                        });
                    }
                }));
            }

            let mut popped = std::collections::HashSet::new();
            let mut attempts = 0;
            while popped.len() < N as usize * THREADS && attempts < 10_000 {
                if let Some(event) = queue.pop() {
                    match event {
                        Event::Clipboard { session_id, .. } => {
                            assert!(popped.insert(session_id), "duplicate event {session_id}");
                        }
                        other => panic!("unexpected event {other:?}"),
                    }
                }
                shuttle::thread::yield_now();
                attempts += 1;
            }
            for handle in handles {
                handle.join().unwrap();
            }
            while let Some(event) = queue.pop() {
                match event {
                    Event::Clipboard { session_id, .. } => {
                        assert!(popped.insert(session_id), "duplicate event {session_id}");
                    }
                    other => panic!("unexpected event {other:?}"),
                }
            }
            assert_eq!(popped.len(), N as usize * THREADS, "events lost");
        },
        64,
    );
}

/// 队列溢出时 Exit 事件必须存活：Kotlin 依赖 Exit 回收会话（native 在压入时即置
/// `native exit_reported`，不会重发），挤掉一个就会泄漏会话。
#[test]
fn event_queue_exit_survives_overflow() {
    shuttle::check_random(
        || {
            let queue = Arc::new(EventQueue::new());
            const THREADS: usize = 4;

            let mut handles = Vec::new();
            for thread_index in 0..THREADS {
                let queue = Arc::clone(&queue);
                handles.push(shuttle::thread::spawn(move || {
                    for item in 0..50u64 {
                        queue.push(Event::Clipboard {
                            session_id: thread_index as u64 * 100 + item,
                            text: String::new(),
                        });
                    }
                    // 每个线程一个 Exit，最后压入，必须存活。
                    queue.push(Event::Exit {
                        session_id: thread_index as u64,
                        code: Some(0),
                    });
                }));
            }
            for handle in handles {
                handle.join().unwrap();
            }

            let mut exits = 0;
            let mut clipboards = 0;
            while let Some(event) = queue.pop() {
                match event {
                    Event::Exit { .. } => exits += 1,
                    Event::Clipboard { .. } => clipboards += 1,
                    other => panic!("unexpected event {other:?}"),
                }
            }
            assert_eq!(exits, THREADS, "an Exit event was evicted");
            assert!(clipboards > 0);
        },
        64,
    );
}
