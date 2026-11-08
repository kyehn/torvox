use super::{
    cancel_request, flood_shed_warn_throttled, register_request, unanswered_clipboard_reads,
};

/// 洪泛削减的告警必须按窗口输出，且把窗口内累计的削减条数一并交出。
#[test]
fn flood_shed_warning_reports_the_window_count() {
    let flood_shed_log = crate::slow_log::CountedLogThrottle::new();
    assert_eq!(
        flood_shed_warn_throttled(4201, 64, &flood_shed_log),
        Some(1),
        "首条必须可见，且带上窗口内累计的削减条数"
    );
    assert_eq!(
        flood_shed_warn_throttled(4201, 65, &flood_shed_log),
        None,
        "窗口内的后续告警不得逐条输出"
    );
    assert_eq!(
        flood_shed_log.pending_count(),
        1,
        "窗口内被压住的那条告警不得丢失计数"
    );
}

/// 未作答计数按会话隔离，且只计入驻留槽位。
#[test]
fn unanswered_reads_count_is_per_session() {
    let (first_a, _first_rx) = register_request(4101);
    let (second_a, _second_rx) = register_request(4101);
    let (only_b, _only_rx) = register_request(4102);
    assert_eq!(unanswered_clipboard_reads(4101), 2);
    assert_eq!(unanswered_clipboard_reads(4102), 1);
    assert_eq!(unanswered_clipboard_reads(4103), 0);
    cancel_request(4101, first_a);
    assert_eq!(unanswered_clipboard_reads(4101), 1);
    cancel_request(4101, second_a);
    cancel_request(4102, only_b);
    assert_eq!(unanswered_clipboard_reads(4101), 0);
    assert_eq!(unanswered_clipboard_reads(4102), 0);
}
