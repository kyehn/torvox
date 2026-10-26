use super::{SessionScoped, highlight_rows_to_mark, mark_overlay_dirty_rows};
use crate::render::cell_builder::SearchHighlight;

fn highlight(row: i32) -> SearchHighlight {
    SearchHighlight {
        row,
        start_col: 0,
        end_col_exclusive: 6,
        color: [0, 0, 0, 255],
    }
}

/// 换会话（`clearSearchHighlights` 走同一条 `enter_session` 分支）后，上一次画过
/// 高亮的行必须仍进脏掩码。
///
/// 判别力：`enter_session` 若把 `drawn` 一并清空，掩码上就没有第 2 行，脏带渲染
/// 跳过它，累加器里的旧高亮像素留在屏幕上——实测关闭搜索后反色块不消失，随后
/// 只有光标行在重绘。
#[test]
fn discarded_highlights_stay_dirty_for_one_frame() {
    let mut scoped = SessionScoped::<SearchHighlight>::new();
    scoped.enter_session(1);
    scoped.current.push(highlight(2));
    scoped.drawn = scoped.current.clone();

    scoped.enter_session(2);
    assert!(scoped.current.is_empty(), "换会话后不得再画上一会话的高亮");
    assert_eq!(scoped.drawn.len(), 1, "被丢弃的高亮行要留一帧用于标脏");

    let mut dirty_mask = [false; 5];
    mark_overlay_dirty_rows(
        &mut dirty_mask,
        5,
        highlight_rows_to_mark(&scoped.current, &scoped.drawn),
    );
    assert!(
        dirty_mask[2],
        "旧高亮所在行必须标脏，否则旧像素永远留在累加器里"
    );
    assert!(!dirty_mask[3], "未受高亮影响的行不该被顺带标脏");
}
