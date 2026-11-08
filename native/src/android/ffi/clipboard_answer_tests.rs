use super::{
    clipboard_answer_missing_level, log_clipboard_answer_missing, render_paused_flag,
    set_render_paused_flag,
};

/// 宿主暂停渲染时不派发应答，超时是必然的正常退化：不得记成告警。
///
/// 切后台、打开设置页、Surface 销毁都会暂停渲染，此时远端的剪贴板读请求必然
/// 无人应答。记成 warn 级即把正常退化说成异常（`logcat-hygiene` 的正常路径纪律）。
#[test]
fn a_paused_host_makes_a_missing_answer_a_normal_degradation() {
    assert_eq!(
        clipboard_answer_missing_level(true),
        log::Level::Debug,
        "宿主暂停渲染时应答缺失属正常退化"
    );
}

/// 仍在渲染却没应答：宿主漏了这条读请求，是真实异常。
#[test]
fn an_active_host_makes_a_missing_answer_an_anomaly() {
    assert_eq!(
        clipboard_answer_missing_level(false),
        log::Level::Warn,
        "宿主仍在渲染时应答缺失是真异常"
    );
}

/// 应答缺失按窗口汇总：首条立即交出窗口计数，窗口内的缺失攒着下次一并交出。
///
/// 逐条输出会刷屏：宿主暂停渲染时每条读请求都在 2s 后超时，一个窗口内可达数十次。
/// 但限频吞掉的是条数，不是次数——窗口内的次数不得丢失。
#[test]
fn missing_answers_are_aggregated_per_window() {
    let missing_log = crate::slow_log::CountedLogThrottle::new();

    log_clipboard_answer_missing("timed out", false, &missing_log);
    assert_eq!(
        missing_log.pending_count(),
        0,
        "首条缺失须立即交出窗口计数，不得被限频吞掉"
    );

    log_clipboard_answer_missing("timed out", false, &missing_log);
    log_clipboard_answer_missing("channel disconnected", false, &missing_log);
    assert_eq!(
        missing_log.pending_count(),
        2,
        "窗口内的缺失须攒着下次一并交出，不得丢弃"
    );
}

/// 渲染暂停标志的原子镜像必须跟随宿主的暂停/恢复。
///
/// 镜像存在的原因是渲染状态锁被整帧 GPU 工作持有：应答线程在旁路线程上等宿主
/// 应答，不能去抢那把锁，只能读这份无锁镜像。
#[test]
fn the_render_pause_mirror_follows_the_host() {
    set_render_paused_flag(true);
    assert!(render_paused_flag(), "宿主暂停后镜像须置位");
    set_render_paused_flag(false);
    assert!(!render_paused_flag(), "宿主恢复后镜像须复位");
}
