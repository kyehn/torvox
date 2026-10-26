use super::{cancel_request, register_request, unanswered_clipboard_reads};

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
