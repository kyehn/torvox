//! 中立事件类型与线程安全事件队列。事件从终端引擎流向 JNI 桥接：会话推入
//! [`EventQueue`]，Kotlin `pollEvent()` 按 FIFO 消费。

use parking_lot::Mutex;
use std::collections::VecDeque;
use std::time::Instant;

/// 缓冲区满时丢弃最旧事件的上限；避免事件突发超过 UI 消费速度时内存无界增长。
const MAX_QUEUED_EVENTS: usize = 1024;

/// 溢出告警的最小间隔（限频）。
const OVERFLOW_WARN_INTERVAL: std::time::Duration = std::time::Duration::from_secs(1);

/// Rust 发往 Kotlin UI 层的事件。触发来源：`Clipboard` = OSC 52 写入，
/// `ClipboardRead` = OSC 52 读取请求，`Bell` = BEL，`Exit` = `session.is_exited()`；
/// 均由 `session` 产生（`Exit` 亦可经 `pollEvent` 上报）。
///
/// 跨 JNI 边界前统一序列化为 JSON，用内部标签（`#[serde(tag = "event")]`）供 Kotlin 匹配。
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(tag = "event", rename_all = "snake_case")]
pub enum Event {
    /// 剪贴板写入内容：OSC 52 set 交由 Kotlin 经 `setPrimaryClip` 写入系统剪贴板。
    Clipboard { session_id: u64, text: String },
    /// 终端收到 BEL（0x07）：上游 `on_bell` 回调经有界通道上报，
    /// 会话锁存后由 `pollEvent` 逐帧上报一次（单帧多响合并为一，
    /// 与对标实现的事件位置位/清零语义等价）。
    Bell { session_id: u64 },
    /// 子进程已退出。
    Exit {
        session_id: u64,
        code: i32,
        /// 子进程实际存活时长（毫秒，fork 到 waitpid），原生侧测量不受 Kotlin 事件
        /// 处理延迟影响，仅作诊断载荷。
        alive_ms: u64,
    },
    /// OSC 52 剪贴板读取请求（`ESC ] 52 ; c ; ?`）：宿主读系统剪贴板后经
    /// `clipboardResult()` JNI 应答，Rust 写回 PTY。携带请求的 selection 名。
    ClipboardRead {
        session_id: u64,
        request_id: u64,
        selection: String,
    },
}

/// Rust 与 Kotlin 共享的线程安全事件队列，FIFO 消费。
///
/// 锁顺序：若需同时持有 JNI `SESSION_REGISTRY` 与本队列，必须先锁 `SESSION_REGISTRY`
/// 再锁队列，反序会死锁。
pub struct EventQueue {
    inner: Mutex<VecDeque<Event>>,
    /// 上次记录溢出告警的时刻（限频）。
    last_overflow_warn: Mutex<Option<Instant>>,
}

impl Default for EventQueue {
    fn default() -> Self {
        Self::new()
    }
}

impl EventQueue {
    /// 创建空事件队列。
    pub const fn new() -> Self {
        Self {
            inner: Mutex::new(VecDeque::new()),
            last_overflow_warn: Mutex::new(None),
        }
    }

    /// 将事件推入队尾（FIFO）。
    ///
    /// 队列满时绝不丢弃 `Exit`，而是淘汰最旧的非 `Exit` 事件：原生侧的
    /// `exit_reported` 标志在推入时即置位且不会重发，丢失 `Exit` 会永久泄漏会话。
    ///
    /// 例外：满队列中只有 `Exit` 时改为丢弃新事件（淘汰任何 `Exit` 同样会遗弃会话）。
    pub fn push(&self, event: Event) {
        // parking_lot Mutex 无中毒，故无需恢复分支。
        let mut guard = self.inner.lock();
        if guard.len() >= MAX_QUEUED_EVENTS {
            self.warn_overflow_once();
            let evict_idx = guard
                .iter()
                .position(|existing_event| !matches!(existing_event, Event::Exit { .. }));
            match evict_idx {
                Some(idx) => {
                    guard.remove(idx);
                }
                None => {
                    // 队列只有 Exit：淘汰任一都会遗弃其会话（标志已置位），
                    // 故丢弃新事件。
                    return;
                }
            }
        }
        guard.push_back(event);
    }

    /// 队列溢出告警每秒至多一次。
    fn warn_overflow_once(&self) {
        let now = Instant::now();
        let mut last = self.last_overflow_warn.lock();
        if last.is_none_or(|t| now.duration_since(t) >= OVERFLOW_WARN_INTERVAL) {
            log::warn!("EventQueue: dropping oldest event (queue full at {MAX_QUEUED_EVENTS})");
            *last = Some(now);
        }
    }

    /// 弹出队首最旧事件（FIFO）。
    pub fn pop(&self) -> Option<Event> {
        self.inner.lock().pop_front()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn push_pop_fifo_order() {
        let q = EventQueue::new();
        q.push(Event::Clipboard {
            session_id: 1,
            text: String::new(),
        });
        q.push(Event::Clipboard {
            session_id: 2,
            text: String::new(),
        });
        q.push(Event::Clipboard {
            session_id: 3,
            text: String::new(),
        });
        assert_eq!(
            q.pop(),
            Some(Event::Clipboard {
                session_id: 1,
                text: String::new()
            })
        );
        assert_eq!(
            q.pop(),
            Some(Event::Clipboard {
                session_id: 2,
                text: String::new()
            })
        );
        assert_eq!(
            q.pop(),
            Some(Event::Clipboard {
                session_id: 3,
                text: String::new()
            })
        );
        assert_eq!(q.pop(), None);
    }

    #[test]
    fn pop_empty_returns_none() {
        let q = EventQueue::new();
        assert_eq!(q.pop(), None);
    }

    #[test]
    fn multiple_events_interleaved() {
        let q = EventQueue::new();
        q.push(Event::Clipboard {
            session_id: 1,
            text: String::new(),
        });
        q.push(Event::Exit {
            session_id: 2,
            code: 0,
            alive_ms: 10,
        });
        q.push(Event::Clipboard {
            session_id: 3,
            text: String::new(),
        });
        assert_eq!(
            q.pop(),
            Some(Event::Clipboard {
                session_id: 1,
                text: String::new()
            })
        );
        q.push(Event::Clipboard {
            session_id: 4,
            text: "hello".into(),
        });
        assert_eq!(
            q.pop(),
            Some(Event::Exit {
                session_id: 2,
                code: 0,
                alive_ms: 10
            })
        );
        assert_eq!(
            q.pop(),
            Some(Event::Clipboard {
                session_id: 3,
                text: String::new()
            })
        );
        assert_eq!(
            q.pop(),
            Some(Event::Clipboard {
                session_id: 4,
                text: "hello".into(),
            })
        );
        assert_eq!(q.pop(), None);
    }

    #[test]
    fn push_drops_oldest_when_full() {
        let q = EventQueue::new();
        for i in 0..(MAX_QUEUED_EVENTS + 8) {
            q.push(Event::Clipboard {
                session_id: i as u64,
                text: String::new(),
            });
        }
        // 最旧事件须被丢弃，最新事件保留。
        assert_eq!(
            q.pop(),
            Some(Event::Clipboard {
                session_id: 8,
                text: String::new()
            })
        );
        assert_eq!(
            q.pop(),
            Some(Event::Clipboard {
                session_id: 9,
                text: String::new()
            })
        );
    }

    #[test]
    fn push_never_evicts_exit_events() {
        let q = EventQueue::new();
        // 先填满 Exit，再推入放不下的非 Exit 事件：Exit 必须全部存活
        //（exit_reported 标志已在原生侧置位且不会重发），改为丢弃新事件。
        for i in 0..MAX_QUEUED_EVENTS {
            q.push(Event::Exit {
                session_id: i as u64,
                code: 0,
                alive_ms: 10,
            });
        }
        q.push(Event::Clipboard {
            session_id: 999,
            text: String::new(),
        });
        q.push(Event::Clipboard {
            session_id: 1000,
            text: String::new(),
        });
        // 全部 Exit 存活，其余事件被丢弃。
        let mut exits = 0;
        while let Some(event) = q.pop() {
            assert!(
                matches!(event, Event::Exit { .. }),
                "non-Exit evicted: {event:?}"
            );
            exits += 1;
        }
        assert_eq!(exits, MAX_QUEUED_EVENTS);
    }

    #[test]
    fn push_evicts_oldest_non_exit_when_mixed() {
        let q = EventQueue::new();
        // 先填事件，末尾补一个 Exit 至满。
        for i in 0..(MAX_QUEUED_EVENTS - 1) {
            q.push(Event::Clipboard {
                session_id: i as u64,
                text: String::new(),
            });
        }
        q.push(Event::Exit {
            session_id: 42,
            code: 7,
            alive_ms: 10,
        });
        // 队列已满；新事件应淘汰最旧的（session 0），而非 Exit。
        q.push(Event::Clipboard {
            session_id: 1000,
            text: String::new(),
        });
        let popped = (0..MAX_QUEUED_EVENTS)
            .filter_map(|_| q.pop())
            .collect::<Vec<_>>();
        assert_eq!(
            popped[0],
            Event::Clipboard {
                session_id: 1,
                text: String::new()
            }
        );
        assert!(popped.contains(&Event::Exit {
            session_id: 42,
            code: 7,
            alive_ms: 10
        }));
        assert_eq!(
            popped[MAX_QUEUED_EVENTS - 1],
            Event::Clipboard {
                session_id: 1000,
                text: String::new()
            }
        );
    }

    #[test]
    fn push_pop_default_works() {
        let q: EventQueue = Default::default();
        q.push(Event::Exit {
            session_id: 1,
            code: 0,
            alive_ms: 10,
        });
        assert_eq!(
            q.pop(),
            Some(Event::Exit {
                session_id: 1,
                code: 0,
                alive_ms: 10
            })
        );
        assert_eq!(q.pop(), None);
    }
}

#[cfg(test)]
mod bell_tests {
    use super::*;

    #[test]
    fn bell_serializes_with_snake_case_discriminator() {
        // Kotlin PollEvent.Bell 解码的契约：discriminator 必须为 "bell"。
        let json = serde_json::to_string(&Event::Bell { session_id: 7 }).expect("bell serializes");
        assert_eq!(json, r#"{"event":"bell","session_id":7}"#);
    }
}
