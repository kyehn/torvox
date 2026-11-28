use super::{
    cancel_request, expire_requests_older_than, flood_shed_warn_throttled, register_request,
    take_request_slot, unanswered_clipboard_reads,
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
    let first_a = register_request(4101, "c".to_string());
    let second_a = register_request(4101, "c".to_string());
    let only_b = register_request(4102, "c".to_string());
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

/// 超期未答复的槽位必须被清掉：留着既占本会话的「未作答」额度（后续请求全被
/// 洪泛削减），又让请求表随远端反复请求无界增长。
#[test]
fn expired_read_requests_are_dropped_from_the_registry() {
    let request_id = register_request(4104, "c".to_string());
    assert_eq!(unanswered_clipboard_reads(4104), 1);
    expire_requests_older_than(std::time::Instant::now() + std::time::Duration::from_secs(1));
    assert_eq!(
        unanswered_clipboard_reads(4104),
        0,
        "超期槽位必须被清掉，否则未作答额度永不释放"
    );
    // 槽位消失后迟到的答复是无操作，不得复活请求。
    assert!(take_request_slot(4104, request_id).is_none());
}
