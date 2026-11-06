use super::wake::take_notification_event;

use crate::event::{Event, EventQueue};

/// 通知事件构造：断言对象是「取用与推进的先后」，负载内容无关。
fn notification(body: &str) -> Event {
    Event::DesktopNotification {
        session_id: 1,
        title: String::new(),
        body: body.to_string(),
    }
}

/// 取到已入队的通知时不推进会话。
///
/// 判别力：先推进后取时，排空方每取一条通知就推进一次全部会话（推进要取会话锁
/// 并解析 VT 输出，与渲染线程争锁）——通知洪泛时一次收割被放大成上百次会话推进。
#[test]
fn queued_notification_is_taken_without_pumping() {
    let queue = EventQueue::new();
    queue.push(notification("queued"));
    let mut pump_count = 0;
    let event = take_notification_event(&queue, || pump_count += 1);
    assert_eq!(event, Some(notification("queued")));
    assert_eq!(pump_count, 0, "取到已入队的通知时不得推进会话");
}

/// 队列为空时推进会话并收割：推进后取到刚收割到的通知。
#[test]
fn empty_queue_pumps_sessions_before_taking() {
    let queue = EventQueue::new();
    let mut pump_count = 0;
    let event = take_notification_event(&queue, || {
        pump_count += 1;
        queue.push(notification("harvested"));
    });
    assert_eq!(pump_count, 1, "队列为空时必须推进一次会话");
    assert_eq!(event, Some(notification("harvested")));
}

/// 推进后仍无通知时返回空：宿主据此结束本轮排空。
#[test]
fn empty_queue_after_pumping_reports_no_notification() {
    let queue = EventQueue::new();
    let mut pump_count = 0;
    let event = take_notification_event(&queue, || pump_count += 1);
    assert_eq!(event, None);
    assert_eq!(pump_count, 1);
}
