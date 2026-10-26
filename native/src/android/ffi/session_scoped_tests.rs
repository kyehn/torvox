use super::SessionScoped;

/// 切会话只丢 `current`，`drawn` 必须留一帧。
///
/// 判别力：渲染态是进程级唯一的，而高亮来自每会话的查询——`current` 是本会话的
/// 命中框，不清就把上一会话的命中框合成到新会话网格上。`drawn` 是「上一帧画进累加器
/// 的高亮行」，两者之差正是空闲重绘路径的门控（`needs_repaint` 里
/// `current != drawn`）：把它一并清掉，判据当场变假，空闲帧不再重绘，累加器里的旧
/// 高亮像素就留在屏幕上（实测：关闭搜索后反色块不消失，随后只有光标行在重绘）。
/// 留着的这份残留只活一帧：两条渲染路径都在成功后把 `drawn` 赋成此刻已空的
/// `current`，自净。
///
/// 曾有一条反向断言（「`drawn` 一并清，否则脏带判重读成高亮未变」）已被推翻：
/// `highlight_rows_to_mark` 恒取 `current ∪ drawn`，没有「读成未变」这条分支。
#[test]
fn entering_another_session_drops_current_but_keeps_drawn_for_one_frame() {
    let mut scoped = SessionScoped::new();
    scoped.session = 1;
    scoped.current = vec![10, 11];
    scoped.drawn = vec![10, 11];

    scoped.enter_session(2);
    assert!(scoped.current.is_empty(), "命中框不得带到另一会话");
    assert_eq!(
        scoped.drawn,
        vec![10, 11],
        "上一帧画过的高亮行要留一帧：清掉这些行就不再重绘，旧像素留在累加器里"
    );
    assert_ne!(
        scoped.current, scoped.drawn,
        "两者之差正是空闲路径重绘的门控条件，清成一致会让本帧静默跳过"
    );

    // 同一会话重复进入是每帧常态：不得清自己的高亮，也不得制造无谓重绘。
    scoped.session = 2;
    scoped.current = vec![20];
    scoped.drawn = vec![20];
    scoped.enter_session(2);
    assert_eq!(scoped.current, vec![20], "同一会话的高亮须保留");
    assert_eq!(scoped.drawn, vec![20]);
    assert_eq!(scoped.current, scoped.drawn, "同一会话内门控不得被点亮");
}
