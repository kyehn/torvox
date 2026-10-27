use super::font::build_search_highlights;
use crate::render::cell_builder::{SEARCH_HIGHLIGHT_CURRENT_ALPHA, SEARCH_HIGHLIGHT_OTHER_ALPHA};
use crate::terminal::ghostty_terminal::SearchMatch;
use crate::terminal::theme::Palette;
use libghostty_vt::style::RgbColor;

fn palette() -> Palette {
    Palette {
        background: RgbColor { r: 0, g: 0, b: 0 },
        foreground: RgbColor {
            r: 0xEE,
            g: 0xEE,
            b: 0xEE,
        },
        cursor: RgbColor { r: 0, g: 0, b: 0 },
        selection_background: RgbColor {
            r: 0x33,
            g: 0x44,
            b: 0x55,
        },
        ansi: [RgbColor { r: 0, g: 0, b: 0 }; 16],
    }
}

fn matched(row: u32, start_col: u32, end_col: u32) -> SearchMatch {
    SearchMatch {
        row,
        start_col,
        end_col,
    }
}

/// 当前命中与其余命中的高亮 MUST 不同。
///
/// 用户报障：「文本搜索匹配和当前匹配高亮应该不同」。判别力：两者同色时，
/// 「当前在哪」无法从画面上分辨——即便当前项被额外标亮，两者的 alpha
/// 或颜色一旦取齐，画面就退化成「一片高亮」。
#[test]
fn current_match_is_distinguishable_from_other_matches() {
    let current = matched(3, 2, 8);
    let other = matched(5, 0, 4);
    let highlights = build_search_highlights(
        &[other.clone(), current.clone()],
        std::slice::from_ref(&current),
        0,
        20,
        &palette(),
    );
    assert_eq!(2, highlights.len());

    let current_hl = highlights
        .iter()
        .find(|hl| hl.row == 3)
        .expect("当前命中的高亮必须存在");
    let other_hl = highlights
        .iter()
        .find(|hl| hl.row == 5)
        .expect("其他命中的高亮必须存在");

    assert_ne!(
        current_hl.color, other_hl.color,
        "当前命中与其余命中的高亮颜色必须不同"
    );
    assert_eq!(
        SEARCH_HIGHLIGHT_CURRENT_ALPHA, current_hl.color[3],
        "当前命中 MUST 完全不透明"
    );
    assert_eq!(
        SEARCH_HIGHLIGHT_OTHER_ALPHA, other_hl.color[3],
        "其余命中 MUST 用较淡的 alpha"
    );
    assert_eq!(
        [0xEE, 0xEE, 0xEE],
        current_hl.color[..3],
        "当前命中 MUST 取主题前景色"
    );
    assert_eq!(
        [0x33, 0x44, 0x55],
        other_hl.color[..3],
        "其余命中 MUST 取主题选区色"
    );
}

/// 视口外的命中 MUST 两侧都裁掉。
///
/// 上游 `viewport_matches` 按页读取，会连带返回视口外的命中；只裁负行时，
/// 视口下方的命中会进入高亮列表，偏移换算一旦有半行误差就画到无关行上。
#[test]
fn matches_outside_viewport_are_clipped_on_both_sides() {
    let matches = [
        matched(1, 0, 3),  // 视口上方
        matched(4, 0, 3),  // 视口内
        matched(25, 0, 3), // 视口下方
    ];
    let highlights = build_search_highlights(&matches, &[], 3, 20, &palette());
    assert_eq!(1, highlights.len(), "只有视口内的命中可以写入高亮");
    assert_eq!(1, highlights[0].row, "折算后的行号是绝对行减去视口顶行");
}

/// 没有选中命中时，全部命中都按「其余命中」着色。
///
/// 判别力：若无选中态就被当作当前命中着色，用户点开搜索框却看不到任何
/// 「当前项」，正是「高亮成一片」的另一种形态。
#[test]
fn without_selection_all_matches_use_other_color() {
    let matches = [matched(1, 0, 3), matched(2, 0, 3)];
    let highlights = build_search_highlights(&matches, &[], 0, 20, &palette());
    assert_eq!(2, highlights.len());
    for hl in &highlights {
        assert_eq!(
            SEARCH_HIGHLIGHT_OTHER_ALPHA, hl.color[3],
            "无选中命中时不得有「当前项」着色"
        );
    }
}

/// 跨软换行的选中命中：每个物理行都 MUST 按「当前命中」着色。
///
/// 判别力：上游把命中拆成逐行区间，选中态若只记首行，续行会落回「其余命中」
/// 的淡色，同一处命中在画面上被劈成两种颜色。
#[test]
fn soft_wrapped_selection_colors_every_physical_row() {
    let first_row = matched(4, 70, 80);
    let second_row = matched(5, 0, 6);
    let other = matched(9, 1, 3);
    let highlights = build_search_highlights(
        &[other, first_row.clone(), second_row.clone()],
        &[first_row, second_row],
        0,
        20,
        &palette(),
    );
    for hl in &highlights {
        let expected = if hl.row == 4 || hl.row == 5 {
            SEARCH_HIGHLIGHT_CURRENT_ALPHA
        } else {
            SEARCH_HIGHLIGHT_OTHER_ALPHA
        };
        assert_eq!(expected, hl.color[3], "行 {} 的高亮归属有误", hl.row);
    }
}
