//! 诊断日志的限频闸门。
//!
//! `logcat-hygiene` 规范要求逐帧诊断限频：两次输出之间的间隔不小于 500ms，
//! MUST NOT 以逐帧频率刷屏。渲染侧有四类慢帧诊断（帧内分段、渲染分段、单元构建、
//! 采样段），每类各持一个闸门——共用一个计数器会让先到的诊断吞掉后到的，
//! 规范明令禁止（「慢循环与慢帧各自限频」场景）。
//!
//! 终端侧单元帧丢弃告警、剪贴板读请求洪泛告警、超长选择器告警与事件队列溢出
//! 告警同样限频：各写一份时口径分叉，间隔下限的要求便失去保证，故一律走本模块。
//!
//! 闸门用进程内单调时钟（[`Instant`] 相对进程启动时刻），不依赖墙上时钟：
//! 墙上时钟被校准时可能前跳或回跳，前跳会把两次输出的真实间隔压到下限以下，
//! 回跳会把间隔算成零而让告警在时钟追上之前长期沉默。

use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Instant;

/// 诊断输出的最小间隔（毫秒）。与 `logcat-hygiene` 规范的 500ms 下限同档。
pub(crate) const SLOW_LOG_INTERVAL_MS: u64 = 500;

/// 进程内单调时钟原点。`Instant` 不可存成整数，取进程启动时刻作基准，
/// 之后一律用相对毫秒比较。
fn now_ms() -> u64 {
    static EPOCH: std::sync::OnceLock<Instant> = std::sync::OnceLock::new();
    EPOCH.get_or_init(Instant::now).elapsed().as_millis() as u64
}

/// 一类诊断的限频闸门。
///
/// 语义：首次调用放行；此后两次放行之间的间隔 MUST 不小于本闸门的窗口长度。
/// 并发调用只有一方能推进窗口起点，故同一时刻不会输出两行。
///
/// 窗口在构造时给定：逐帧诊断取 [`SLOW_LOG_INTERVAL_MS`]，窗口更长的调用点
/// （如帧丢弃告警）用 [`Self::with_interval`] 另给。
pub(crate) struct SlowLogThrottle {
    /// 上次放行的时刻（毫秒）。0 表示「尚未放行过」。
    last_logged_ms: AtomicU64,
    /// 两次放行之间的最小间隔（毫秒）。
    interval_ms: u64,
}

impl SlowLogThrottle {
    /// 取默认窗口（逐帧诊断用）。
    pub(crate) const fn new() -> Self {
        Self::with_interval(SLOW_LOG_INTERVAL_MS)
    }

    /// 指定窗口长度（毫秒）：同一类诊断需要比逐帧诊断更长的静默期时用它，
    /// 而非另写一份闸门。
    pub(crate) const fn with_interval(interval_ms: u64) -> Self {
        Self {
            last_logged_ms: AtomicU64::new(0),
            interval_ms,
        }
    }

    /// 清空窗口起点，回到「尚未放行过」：下一次调用必定放行。
    ///
    /// 仅测试构建存在：静态闸门是进程级的，同一测试二进制里别的用例可能先推进
    /// 过窗口起点，用例需要回到首条必定可见的初始态才敢断言放行次数。
    #[cfg(test)]
    pub(crate) fn reset(&self) {
        self.last_logged_ms.store(0, Ordering::Release);
    }

    /// 该次诊断是否允许输出；放行时同时推进窗口起点。
    pub(crate) fn should_log(&self) -> bool {
        self.should_log_at(now_ms())
    }

    /// 以给定时刻判定是否放行。
    ///
    /// 时刻由调用方给出，使测试无需靠睡眠推进时钟（睡眠会让用例变慢且不稳定）。
    pub(crate) fn should_log_at(&self, now_ms: u64) -> bool {
        let last = self.last_logged_ms.load(Ordering::Acquire);
        // 0 是「尚未放行过」的哨兵：首条必须可见。
        if last != 0 && now_ms.saturating_sub(last) < self.interval_ms {
            return false;
        }
        // CAS：并发调用只有一方能把窗口起点推进到当前时刻，
        // 否则同一帧的并发诊断会输出多行。
        self.last_logged_ms
            .compare_exchange(
                last,
                // 0 与「尚未放行过」同义，故实际起点至少取 1ms。
                now_ms.max(1),
                Ordering::AcqRel,
                Ordering::Acquire,
            )
            .is_ok()
    }
}

/// 带计数的限频闸门：在限频窗口内累计事件数，放行时把计数一并交出。
///
/// 逐帧诊断若逐条输出会把 logcat 刷成洪水；若只按闸门丢弃，窗口内发生了多少
/// 次就此不可见，排障时看不出「多久一次」。本类型两者兼得：每个窗口放行一次，
/// 窗口内的事件计数随该次输出一起交出。
pub(crate) struct CountedLogThrottle {
    throttle: SlowLogThrottle,
    count: u32,
}

impl CountedLogThrottle {
    pub(crate) const fn new() -> Self {
        Self {
            throttle: SlowLogThrottle::new(),
            count: 0,
        }
    }

    /// 记一次事件；闸门放行时返回窗口内累计的事件数，否则返回 `None`。
    pub(crate) fn record(&mut self) -> Option<u32> {
        self.record_at(now_ms())
    }

    /// 以给定时刻记账（测试用，免去睡眠）。
    pub(crate) fn record_at(&mut self, now_ms: u64) -> Option<u32> {
        self.count += 1;
        if !self.throttle.should_log_at(now_ms) {
            return None;
        }
        let count = self.count;
        self.count = 0;
        Some(count)
    }
}

#[cfg(test)]
mod tests {
    use super::{CountedLogThrottle, SLOW_LOG_INTERVAL_MS, SlowLogThrottle};

    /// 首条必须可见：诊断的第一条往往就是最关键的那条（首次出现的慢帧）。
    #[test]
    fn first_diagnostic_is_always_logged() {
        let throttle = SlowLogThrottle::new();
        assert!(throttle.should_log_at(0));
    }

    /// 窗口内的后续诊断必须被压住——这正是「逐帧刷屏」的形态。
    #[test]
    fn diagnostics_inside_the_window_are_suppressed() {
        let throttle = SlowLogThrottle::new();
        assert!(throttle.should_log_at(1_000));
        for now in [1_001, 1_100, 1_499] {
            assert!(
                !throttle.should_log_at(now),
                "now={now} 落在窗口内，必须压住"
            );
        }
    }

    /// 窗口走完即放行：诊断要能反映「持续慢」的形状，而不是只报第一条。
    #[test]
    fn diagnostics_after_the_window_are_logged_again() {
        let throttle = SlowLogThrottle::new();
        assert!(throttle.should_log_at(1_000));
        assert!(!throttle.should_log_at(1_000 + SLOW_LOG_INTERVAL_MS - 1));
        assert!(throttle.should_log_at(1_000 + SLOW_LOG_INTERVAL_MS));
    }

    /// 时钟回跳不得放行：回跳会算出负间隔，若按「差值 < 窗口」判定会被永久放行。
    #[test]
    fn clock_going_backwards_does_not_unlock_the_window() {
        let throttle = SlowLogThrottle::new();
        assert!(throttle.should_log_at(10_000));
        assert!(!throttle.should_log_at(9_000));
        assert!(!throttle.should_log_at(5_000));
    }

    /// 自定义窗口必须生效：窗口更长的调用点靠它拉长静默期，而非另写一份闸门
    /// ——默认窗口被无视即意味着调用点的配置根本没生效。
    #[test]
    fn custom_interval_replaces_the_default_window() {
        const CUSTOM_INTERVAL_MS: u64 = 5_000;
        let throttle = SlowLogThrottle::with_interval(CUSTOM_INTERVAL_MS);
        assert!(throttle.should_log_at(1_000));
        // 默认窗口早已走完，但自定义窗口未走完：必须压住。
        assert!(!throttle.should_log_at(1_000 + SLOW_LOG_INTERVAL_MS));
        assert!(!throttle.should_log_at(1_000 + CUSTOM_INTERVAL_MS - 1));
        assert!(throttle.should_log_at(1_000 + CUSTOM_INTERVAL_MS));
    }

    /// 清空窗口起点后首条必定可见：静态闸门被别的用例推进过时，用例需要回到
    /// 「尚未放行过」的初始态才敢断言放行次数。
    #[test]
    fn reset_restores_the_initial_state() {
        let throttle = SlowLogThrottle::with_interval(5_000);
        assert!(throttle.should_log_at(1_000));
        assert!(!throttle.should_log_at(1_100));
        throttle.reset();
        assert!(throttle.should_log_at(1_100));
    }

    /// 首条必须可见：汇总行的第一条同样承载「诊断刚发生」这一事实。
    #[test]
    fn counted_throttle_reports_the_first_event() {
        let mut throttle = CountedLogThrottle::new();
        assert_eq!(throttle.record_at(1_000), Some(1));
    }

    /// 窗口内的事件被压住，但计数不得丢失：限频吞掉的是条数，不是次数。
    #[test]
    fn counted_throttle_keeps_the_suppressed_count() {
        let mut throttle = CountedLogThrottle::new();
        assert_eq!(throttle.record_at(1_000), Some(1));
        for now in [1_100, 1_200, 1_300] {
            assert_eq!(throttle.record_at(now), None, "now={now} 落在窗口内");
        }
        assert_eq!(
            throttle.record_at(1_000 + SLOW_LOG_INTERVAL_MS),
            Some(4),
            "窗口内累计 4 次（首条 + 3 次压住）必须原样交出"
        );
    }

    /// 交出计数后重新计数：下一窗口不得把上一窗口的次数算进来。
    #[test]
    fn counted_throttle_resets_the_count_after_handover() {
        let mut throttle = CountedLogThrottle::new();
        assert_eq!(throttle.record_at(1_000), Some(1));
        assert_eq!(throttle.record_at(1_100), None);
        assert_eq!(throttle.record_at(1_000 + SLOW_LOG_INTERVAL_MS), Some(2));
        assert_eq!(throttle.record_at(1_100 + SLOW_LOG_INTERVAL_MS), None);
        assert_eq!(
            throttle.record_at(1_000 + 2 * SLOW_LOG_INTERVAL_MS),
            Some(2),
            "新窗口只统计本窗口的事件"
        );
    }
}
