use super::render::should_repaint_idle;

/// 空闲重绘门控的判定表。
///
/// 六个变化信号任一为真即需重绘；滚动命令在途时一律不重绘——屏幕内容尚未
/// 反映本次滚动，此刻重绘只能用旧视口配新余量，画出的内容比实际视口少滚了
/// 整行数（滑动慢一拍），且那是一次多余的全量提交。
///
/// 判别力：去掉「滚动在途」这一位，第三个断言立刻判红。
#[test]
fn idle_repaint_gate_decision_table() {
    // 无变化：不必重绘。
    assert!(
        !should_repaint_idle(false, false, false, false, false, false, false),
        "无变化时不得重绘"
    );
    // 五个变化信号各自独立触发重绘。
    assert!(
        should_repaint_idle(true, false, false, false, false, false, false),
        "搜索高亮变化必须重绘"
    );
    assert!(
        should_repaint_idle(false, true, false, false, false, false, false),
        "亚行余量变化必须重绘"
    );
    assert!(
        should_repaint_idle(false, false, true, false, false, false, false),
        "内容脏标志置位必须重绘"
    );
    assert!(
        should_repaint_idle(false, false, false, true, false, false, false),
        "累加器失效必须重绘"
    );
    assert!(
        should_repaint_idle(false, false, false, false, true, false, false),
        "整屏重绘请求必须重绘"
    );
    assert!(
        should_repaint_idle(false, false, false, false, false, true, false),
        "光标闪烁相位翻转必须重绘"
    );
    // 滚动在途：无论其余信号如何都不重绘。
    assert!(
        !should_repaint_idle(true, true, true, true, true, true, true),
        "滚动命令在途期间不得用旧视口重绘"
    );
}

/// 闪烁相位按时基与间隔算出，与「上一帧何时画的」无关。
///
/// 判别力：把 `% 2` 去掉（恒为真）时后三条断言立刻判红——表现为光标只亮不灭。
#[test]
fn blink_phase_alternates_with_the_configured_rate() {
    use super::cursor_blink_phase_on;
    // 间隔 0 = 不闪：任何时刻都亮。
    assert!(cursor_blink_phase_on(0, 0));
    assert!(cursor_blink_phase_on(9_999, 0));
    // 间隔 500ms：前半周期亮、后半周期灭，如此循环。
    assert!(cursor_blink_phase_on(0, 500), "周期起点必须亮");
    assert!(cursor_blink_phase_on(499, 500), "前半周期保持亮");
    assert!(!cursor_blink_phase_on(500, 500), "后半周期必须灭");
    assert!(!cursor_blink_phase_on(999, 500), "后半周期保持灭");
    assert!(cursor_blink_phase_on(1_000, 500), "下一周期重新亮起");
}
