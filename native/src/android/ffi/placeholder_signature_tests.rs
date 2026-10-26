use crate::terminal::ghostty_terminal::CellData;
use crate::terminal::ghostty_terminal::unicode_placement::PLACEHOLDER;
use crate::terminal::ghostty_terminal::{placeholder_layout_signature, placeholder_row_signatures};

fn placeholder(row: u32, col: u32) -> CellData {
    let mut cell: CellData = bytemuck::Zeroable::zeroed();
    cell.codepoint = PLACEHOLDER;
    cell.row = row;
    cell.col = col;
    cell
}

fn signature(cells: &[CellData], rows: u32, viewport_top_row: u64) -> u64 {
    placeholder_layout_signature(&placeholder_row_signatures(cells, rows), viewport_top_row)
}

/// 签名必须随占位符位置变化：同一幅图被 TUI 重绘到新行时，
/// 渲染线程要重查放置，否则图像钉在旧位置。
#[test]
fn signature_tracks_placeholder_movement() {
    let at_origin = [placeholder(2, 3)];
    assert_ne!(
        signature(&at_origin, 8, 0),
        signature(&[placeholder(4, 3)], 8, 0)
    );
    assert_ne!(
        signature(&at_origin, 8, 0),
        signature(&[placeholder(2, 5)], 8, 0)
    );
    assert_eq!(
        signature(&at_origin, 8, 0),
        signature(&[placeholder(2, 3)], 8, 0)
    );
}

/// 纯滚动（绝对行不变、视口行平移）不得改变签名，否则每次滚动都全量重扫。
/// 回看历史时视口顶行与视口行按相反方向变化，两者之和保持不变。
#[test]
fn signature_is_stable_across_pure_scroll() {
    assert_eq!(
        signature(&[placeholder(2, 3)], 8, 102),
        signature(&[placeholder(3, 3)], 8, 101)
    );
}

/// 纯文本（无占位符）必须返回稳定的 0，避免每帧误判为布局变化。
#[test]
fn no_placeholders_is_stable() {
    let mut text: CellData = bytemuck::Zeroable::zeroed();
    text.codepoint = u32::from('a');
    text.row = 1;
    text.col = 1;
    assert_eq!(signature(&[text], 8, 0), 0);
    assert_eq!(signature(&[], 8, 0), 0);
}
