use super::build_cursor;
use crate::terminal::ghostty_terminal::CursorInfo;
use crate::terminal::ghostty_terminal::CursorStyle;

/// 落在宽字符尾列的光标必须折算成该宽字符的头列。
///
/// 判别力：`build_cell_data` 对一个宽字符只发出一条头列记录（`width=2`，一条即
/// 整个字形），spacer 单元不产出记录。渲染侧按 `row`/`col` 相等匹配光标，故不折算
/// 时尾列上的光标匹配不到任何记录，那几帧终端看上去「重绘了但没有光标」——
/// 方块/竖线/下划线三种样式全中，因为匹配发生在样式分支之前。
#[test]
fn cursor_column_folds_onto_the_wide_characters_head() {
    let mut wide_tail = CursorInfo {
        row: 3,
        col: 7,
        at_wide_tail: true,
        visible: true,
        style: CursorStyle::Block,
        scrollback_length: 0,
        rows: 24,
        cols: 80,
        kitty_generation: 0,
        kitty_placement_generation: 0,
    };
    let mut render_state = super::render_state_mut();
    let render_state = render_state
        .as_mut()
        .expect("渲染状态在无 GPU 环境下仍应可用（无 surface 的渲染器不碰设备）");
    let cursor = build_cursor(render_state, &wide_tail);
    assert_eq!(cursor.col, 6, "尾列（第 7 格）折算到头列（第 6 格）");

    // 折算只认尾列标志：普通列与宽度为 1 的单元都不受影响。
    wide_tail.at_wide_tail = false;
    assert_eq!(build_cursor(render_state, &wide_tail).col, 7);
}
