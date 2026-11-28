//! 诊断日志的限频闸门与分级口径。
//!
//! `logcat-hygiene` 规范要求逐帧诊断限频：两次输出之间的间隔不小于 500ms，
//! MUST NOT 以逐帧频率刷屏。渲染侧有四类慢帧诊断（帧内分段、渲染分段、单元构建、
//! 采样段），每类各持一个闸门——共用一个计数器会让先到的诊断吞掉后到的，
//! 规范明令禁止（「慢循环与慢帧各自限频」场景）。
//!
//! 终端侧单元帧丢弃告警、剪贴板读请求洪泛告警、超长选择器告警与事件队列溢出
//! 告警同样限频：各写一份时口径分叉，间隔下限的要求便失去保证，故一律走本模块。
//!
//! 慢帧诊断的级别分档（达到阈值记警告，否则记信息）也放在本模块：帧内分段与渲染
//! 分段两处慢帧诊断各写一份分档时，阈值一旦调整就会漏改一处，故只留一处口径。
//!
//! 闸门用进程内单调时钟（[`Instant`] 相对进程启动时刻），不依赖墙上时钟：
//! 墙上时钟被校准时可能前跳或回跳，前跳会把两次输出的真实间隔压到下限以下，
//! 回跳会把间隔算成零而让告警在时钟追上之前长期沉默。

use governor::{
    Quota, RateLimiter,
    clock::Clock,
    middleware::NoOpMiddleware,
    nanos::Nanos,
    state::{InMemoryState, NotKeyed},
};
use parking_lot::{Mutex, MutexGuard};
use std::num::NonZeroU32;
use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};
use std::sync::{Arc, OnceLock};
use std::time::{Duration, Instant};

/// 诊断输出的最小间隔（毫秒）。与 `logcat-hygiene` 规范的 500ms 下限同档。
pub(crate) const SLOW_LOG_INTERVAL_MS: u64 = 500;

/// 慢帧诊断记警告的阈值（毫秒）。
///
/// 30ms 是「算慢帧」的门槛，60ms 是「严重慢帧」的门槛：达到后者才记警告，否则
/// 记信息——逐帧都可能出现的诊断若一律记警告，真正的异常反而被淹没。
pub(crate) const SLOW_FRAME_WARN_THRESHOLD_MS: u64 = 60;

/// 慢帧诊断该记什么级别。
///
/// 纯函数：级别判定与日志输出分离，用例无需安装 logger 即可断言分档。
pub(crate) fn slow_frame_log_level(total: Duration) -> log::Level {
    if total >= Duration::from_millis(SLOW_FRAME_WARN_THRESHOLD_MS) {
        log::Level::Warn
    } else {
        log::Level::Info
    }
}

/// 进程内单调时钟原点。`Instant` 不可存成整数，取进程启动时刻作基准，
/// 之后一律用相对毫秒比较。
fn now_ms() -> u64 {
    static EPOCH: OnceLock<Instant> = OnceLock::new();
    EPOCH.get_or_init(Instant::now).elapsed().as_millis() as u64
}

/// governor 的闸门：进程内状态 + 可钉住时刻的单调时钟。
type Gate = RateLimiter<NotKeyed, InMemoryState, LogClock, NoOpMiddleware<Nanos>>;

/// 供 governor 使用的时钟：时刻由调用方写入原子量，与闸门共享同一份。
/// 测试写入给定时刻即可推进窗口，无需睡眠；生产由 [`now_ms`] 写入。
#[derive(Clone, Default)]
struct LogClock {
    nanos: Arc<AtomicU64>,
}

impl Clock for LogClock {
    type Instant = Nanos;

    fn now(&self) -> Nanos {
        Nanos::from(self.nanos.load(Ordering::Acquire))
    }
}

/// 一类诊断的限频闸门。
///
/// 语义：首次调用放行；此后两次放行之间的间隔 MUST 不小于本闸门的窗口长度。
/// 闸门用 governor 的 GCRA，`allow_burst(1)` 使突发容忍为零，两次放行严格相隔
/// 一个窗口；闸门自身的原子状态保证并发调用只有一方通过。
///
/// 窗口在构造时给定：逐帧诊断取 [`SLOW_LOG_INTERVAL_MS`]，窗口更长的调用点
/// （如帧丢弃告警）用 [`Self::with_interval`] 另给。
pub(crate) struct SlowLogThrottle {
    interval_ms: u64,
    clock: OnceLock<LogClock>,
    gate: Mutex<Option<Gate>>,
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
            interval_ms,
            clock: OnceLock::new(),
            gate: Mutex::new(None),
        }
    }

    /// 清空窗口起点，回到「尚未放行过」：下一次调用必定放行。
    ///
    /// 仅测试构建存在：静态闸门是进程级的，同一测试二进制里别的用例可能先推进
    /// 过窗口起点，用例需要回到首条必定可见的初始态才敢断言放行次数。
    #[cfg(test)]
    pub(crate) fn reset(&self) {
        self.clock().nanos.store(0, Ordering::Release);
        *self.gate.lock() = None;
    }

    fn clock(&self) -> &LogClock {
        self.clock.get_or_init(LogClock::default)
    }

    fn gate(&self) -> MutexGuard<'_, Option<Gate>> {
        let mut gate = self.gate.lock();
        if gate.is_none() {
            let quota = Quota::with_period(Duration::from_millis(self.interval_ms))
                .expect("interval must be nonzero")
                .allow_burst(NonZeroU32::MIN);
            *gate = Some(RateLimiter::direct_with_clock(quota, self.clock().clone()));
        }
        gate
    }

    /// 该次诊断是否允许输出；放行时同时推进窗口起点。
    pub(crate) fn should_log(&self) -> bool {
        self.should_log_at(now_ms())
    }

    /// 以给定时刻判定是否放行。
    ///
    /// 时刻由调用方给出，使测试无需靠睡眠推进时钟（睡眠会让用例变慢且不稳定）。
    pub(crate) fn should_log_at(&self, now_ms: u64) -> bool {
        self.clock()
            .nanos
            .store(now_ms.saturating_mul(1_000_000).max(1), Ordering::Release);
        self.gate()
            .as_ref()
            .expect("gate initialized")
            .check()
            .is_ok()
    }
}

/// 带计数的限频闸门：在限频窗口内累计事件数，放行时把计数一并交出。
///
/// 逐帧诊断若逐条输出会把 logcat 刷成洪水；若只按闸门丢弃，窗口内发生了多少
/// 次就此不可见，排障时看不出「多久一次」。本类型两者兼得：每个窗口放行一次，
/// 窗口内的事件计数随该次输出一起交出。
///
/// 计数用原子量而非普通整数：同一闸门既被独占 `&mut` 的调用点使用（如会话的
/// 剪贴板丢弃计数），也被只持 `&self` 的调用点使用（如终端命令入队失败的告警），
/// 后者来自渲染线程与输入线程，共享同一把闸门。
pub(crate) struct CountedLogThrottle {
    throttle: SlowLogThrottle,
    count: AtomicU32,
}

impl CountedLogThrottle {
    pub(crate) const fn new() -> Self {
        Self {
            throttle: SlowLogThrottle::new(),
            count: AtomicU32::new(0),
        }
    }

    /// 指定窗口长度（毫秒）：同一类诊断需要比默认窗口更长的静默期时用它，
    /// 而非另写一份闸门。
    pub(crate) const fn with_interval(interval_ms: u64) -> Self {
        Self {
            throttle: SlowLogThrottle::with_interval(interval_ms),
            count: AtomicU32::new(0),
        }
    }

    /// 记一次事件；闸门放行时返回窗口内累计的事件数，否则返回 `None`。
    pub(crate) fn record(&self) -> Option<u32> {
        self.record_at(now_ms())
    }

    /// 以给定时刻记一次事件（测试用，免去睡眠）。
    pub(crate) fn record_at(&self, now_ms: u64) -> Option<u32> {
        self.record_events_at(1, now_ms)
    }

    /// 一批记 `events` 次事件；窗内计数照常累加，放行时一并交出。
    ///
    /// 成批丢弃（队列溢出一次丢掉多条）若逐条走 [`Self::record`]，每条都要判定
    /// 一次窗口、且被压住的那几条会白跑一遍时钟；批量口径与逐条口径在「窗口内
    /// 累计了多少次」上完全一致。`events` 为 0 时不推进窗口、不产生输出。
    pub(crate) fn record_events(&self, events: u32) -> Option<u32> {
        self.record_events_at(events, now_ms())
    }

    /// [`Self::record_events`] 的定时版（测试用）。
    pub(crate) fn record_events_at(&self, events: u32, now_ms: u64) -> Option<u32> {
        if events == 0 {
            return None;
        }
        self.count
            .try_update(Ordering::AcqRel, Ordering::Acquire, |count| {
                Some(count.saturating_add(events))
            })
            .ok();
        if !self.throttle.should_log_at(now_ms) {
            return None;
        }
        Some(self.count.swap(0, Ordering::AcqRel))
    }

    /// 窗口内已累计但尚未交出的事件数（测试用）。
    #[cfg(test)]
    pub(crate) fn pending_count(&self) -> u32 {
        self.count.load(Ordering::Acquire)
    }
}

#[cfg(test)]
mod tests {
    use super::{
        CountedLogThrottle, SLOW_FRAME_WARN_THRESHOLD_MS, SLOW_LOG_INTERVAL_MS, SlowLogThrottle,
        slow_frame_log_level,
    };
    use std::time::Duration;

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
        let throttle = CountedLogThrottle::new();
        assert_eq!(throttle.record_at(1_000), Some(1));
    }

    /// 窗口内的事件被压住，但计数不得丢失：限频吞掉的是条数，不是次数。
    #[test]
    fn counted_throttle_keeps_the_suppressed_count() {
        let throttle = CountedLogThrottle::new();
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

    /// 成批记事件：一批与逐条在窗口计数上同口径，且一批只判定一次窗口。
    #[test]
    fn batch_record_counts_every_event_in_one_window() {
        let throttle = CountedLogThrottle::new();
        assert_eq!(throttle.record_events_at(5, 1_000), Some(5));
        assert_eq!(throttle.record_events_at(7, 1_100), None);
        assert_eq!(
            throttle.record_events_at(1, 1_000 + SLOW_LOG_INTERVAL_MS),
            Some(8),
            "窗口内共 5 + 7 + 1 次，被压住的两批必须计入"
        );
    }

    /// 一批零次事件不推进窗口、不产生输出：否则「本帧没有丢弃」也会被记成一次。
    #[test]
    fn empty_batch_does_not_open_the_window() {
        let throttle = CountedLogThrottle::new();
        assert_eq!(throttle.record_events_at(0, 1_000), None);
        assert_eq!(
            throttle.record_events_at(1, 1_100),
            Some(1),
            "零事件不得占用窗口起点"
        );
    }

    /// 交出计数后重新计数：下一窗口不得把上一窗口的次数算进来。
    #[test]
    fn counted_throttle_resets_the_count_after_handover() {
        let throttle = CountedLogThrottle::new();
        assert_eq!(throttle.record_at(1_000), Some(1));
        assert_eq!(throttle.record_at(1_100), None);
        assert_eq!(throttle.record_at(1_000 + SLOW_LOG_INTERVAL_MS), Some(2));
        assert_eq!(throttle.record_at(1_100 + SLOW_LOG_INTERVAL_MS), None);
        assert_eq!(
            throttle.record_at(1_000 + 2 * SLOW_LOG_INTERVAL_MS),
            Some(2)
        );
    }

    /// 慢帧诊断的分档：达到 60ms 记警告，否则记信息。两处慢帧诊断共用此口径，
    /// 阈值调整只需改一处。
    #[test]
    fn slow_frame_level_separates_warning_from_info() {
        assert_eq!(
            slow_frame_log_level(Duration::from_millis(SLOW_FRAME_WARN_THRESHOLD_MS)),
            log::Level::Warn,
            "达到阈值必须记警告"
        );
        assert_eq!(
            slow_frame_log_level(Duration::from_millis(SLOW_FRAME_WARN_THRESHOLD_MS - 1)),
            log::Level::Info,
            "未达阈值必须记信息"
        );
    }

    /// 共享闸门（只持 `&self`）在多线程下不得丢计数：命令入队失败来自渲染线程与
    /// 输入线程，两者共用同一把闸门。交出的计数之和必须等于记录的事件总数。
    #[test]
    fn counted_throttle_keeps_the_total_across_threads() {
        use std::sync::Arc;
        use std::sync::atomic::{AtomicU32, Ordering};

        const THREADS: u32 = 8;
        const EVENTS_PER_THREAD: u32 = 1_000;

        let throttle = Arc::new(CountedLogThrottle::new());
        let handed_over = Arc::new(AtomicU32::new(0));
        let mut handles = Vec::new();
        for _ in 0..THREADS {
            let throttle = Arc::clone(&throttle);
            let handed_over = Arc::clone(&handed_over);
            handles.push(std::thread::spawn(move || {
                let mut local = 0;
                for _ in 0..EVENTS_PER_THREAD {
                    if let Some(count) = throttle.record_events_at(1, 1_000) {
                        local += count;
                    }
                }
                handed_over.fetch_add(local, Ordering::AcqRel);
            }));
        }
        for handle in handles {
            handle.join().expect("计数线程");
        }
        assert_eq!(
            handed_over.load(Ordering::Acquire) + throttle.pending_count(),
            THREADS * EVENTS_PER_THREAD,
            "限频吞掉的是条数，不是次数：已交出与待交出的计数之和必须等于事件总数"
        );
    }
}
