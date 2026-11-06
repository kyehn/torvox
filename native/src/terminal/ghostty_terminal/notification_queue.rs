//! 桌面通知队列：VT 线程写入，收割方取走。

use std::collections::VecDeque;

use parking_lot::Mutex;

use super::types::NOTIFICATION_QUEUE_CAPACITY;
use crate::slow_log::SlowLogThrottle;

/// 通知队列满时的告警间隔（毫秒）。通知可被程序洪泛，逐条告警会淹没 logcat。
const OVERFLOW_WARN_INTERVAL_MS: u64 = 1_000;

/// 桌面通知队列：有界，满时丢最旧的一条。
///
/// 与剪贴板通道（flume，满时丢最新）刻意不同：通知是「刚刚发生的事」，丢最新的
/// 那条会让用户错过关键通知（如「构建完成」）；而通知没有协议级应答，丢旧的只是
/// 少看一条历史。故通知丢旧、剪贴板丢新。
pub(crate) struct NotificationQueue {
    entries: Mutex<VecDeque<(String, String)>>,
    /// 溢出告警限频：洪泛时逐条告警会把 logcat 刷满，真正的错误反而看不见。
    overflow_warn: SlowLogThrottle,
}

impl NotificationQueue {
    pub(crate) fn new() -> Self {
        Self {
            entries: Mutex::new(VecDeque::new()),
            overflow_warn: SlowLogThrottle::with_interval(OVERFLOW_WARN_INTERVAL_MS),
        }
    }

    /// 推入一条通知；队列满时丢最旧的一条并告警。
    pub(crate) fn push(&self, notification: (String, String)) {
        let mut entries = self.entries.lock();
        if entries.len() >= NOTIFICATION_QUEUE_CAPACITY {
            entries.pop_front();
            if self.overflow_warn.should_log() {
                log::warn!(
                    "desktop notification queue full: dropped the oldest notification \
                     (capacity {NOTIFICATION_QUEUE_CAPACITY})"
                );
            }
        }
        entries.push_back(notification);
    }

    /// 取走最旧的一条通知；无通知时为 `None`。
    pub(crate) fn pop(&self) -> Option<(String, String)> {
        self.entries.lock().pop_front()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 队列满时丢最旧：通知是「刚刚发生的事」，丢最新的那条会让用户错过关键通知。
    #[test]
    fn push_beyond_capacity_drops_the_oldest() {
        let queue = NotificationQueue::new();
        for index in 0..NOTIFICATION_QUEUE_CAPACITY {
            queue.push((String::new(), index.to_string()));
        }
        queue.push((String::new(), "newest".to_string()));
        assert_eq!(queue.pop(), Some((String::new(), "1".to_string())));
    }

    /// 先进先出：收割方按发生顺序取走通知。
    #[test]
    fn pop_returns_the_oldest_first() {
        let queue = NotificationQueue::new();
        queue.push((String::new(), "first".to_string()));
        queue.push((String::new(), "second".to_string()));
        assert_eq!(queue.pop(), Some((String::new(), "first".to_string())));
        assert_eq!(queue.pop(), Some((String::new(), "second".to_string())));
        assert_eq!(queue.pop(), None);
    }
}
