use super::*;

/// 无反显/高亮时，普通单元保持自身颜色。
#[test]
fn plain_cell_keeps_colors() {
    let cells = vec![cell_data(
        0,
        0,
        'A',
        [1.0, 0.0, 0.0, 1.0],
        [0.0, 0.0, 1.0, 1.0],
        0,
    )];
    let instances = build(&cells, CellCursor::default(), &[]);
    assert_eq!(instances.len(), 1);
    assert_eq!(instances[0].foreground, [1.0, 0.0, 0.0, 1.0]);
    assert_eq!(instances[0].background, [0.0, 0.0, 1.0, 1.0]);
    assert_eq!(instances[0].flags, 0.0);
}

/// SGR 7 反显交换前景与背景。
#[test]
fn reverse_swaps_foreground_background() {
    let cells = vec![cell_data(
        0,
        0,
        'A',
        [1.0, 0.0, 0.0, 1.0],
        [0.0, 0.0, 1.0, 1.0],
        1 << cell_flags::REVERSE,
    )];
    let instances = build(&cells, CellCursor::default(), &[]);
    assert_eq!(instances[0].foreground, [0.0, 0.0, 1.0, 1.0]);
    assert_eq!(instances[0].background, [1.0, 0.0, 0.0, 1.0]);
}

/// SGR 1/3 bold+italic 标志传到 GPU 实例（shader 样式位）。
#[test]
fn bold_italic_flags_reach_instance() {
    let flags = (1 << cell_flags::BOLD) | (1 << cell_flags::ITALIC);
    let cells = vec![cell_data(
        0,
        0,
        'I',
        [1.0, 1.0, 1.0, 1.0],
        [0.0, 0.0, 0.0, 1.0],
        flags,
    )];
    let instances = build(&cells, CellCursor::default(), &[]);
    assert_eq!(
        instances[0].flags, flags as f32,
        "bold+italic flags must reach the instance"
    );
    assert!(
        instances[0].atlas_size[0] > 0.0 && instances[0].atlas_size[1] > 0.0,
        "styled cell must carry a real atlas quad, not a blank fallback"
    );
}

/// SGR 8 隐藏单元只保留背景，不生成字形或文字装饰。
#[test]
fn concealed_cell_emits_background_without_glyph() {
    let cells = vec![
        cell_data(0, 0, 'A', [1.0; 4], [0.0; 4], 0),
        cell_data(
            0,
            1,
            'H',
            [1.0; 4],
            [0.25, 0.5, 0.75, 1.0],
            (1 << 9) | (1 << cell_flags::UNDERLINE),
        ),
    ];
    let instances = build(&cells, CellCursor::default(), &[]);
    let concealed = instances
        .iter()
        .find(|instance| instance.quad_origin[0] > 0.0)
        .expect("concealed cell must keep its background quad");
    assert_eq!(concealed.atlas_size, [0.0; 2]);
    assert_eq!(concealed.background, [0.25, 0.5, 0.75, 1.0]);
    assert_eq!(concealed.flags, 0.0, "hidden text must emit no decorations");
}

/// SGR 58 下划线颜色传到 GPU 实例，供 shader 装饰遍使用。
#[test]
fn underline_color_reaches_instance() {
    let mut decorated = cell_data(
        0,
        0,
        'U',
        [1.0, 1.0, 1.0, 1.0],
        [0.0, 0.0, 0.0, 1.0],
        1 << cell_flags::UNDERLINE,
    );
    decorated.underline_color = [1.0, 0.0, 0.0, 1.0];
    let instances = build(&[decorated], CellCursor::default(), &[]);
    assert_eq!(
        instances[0].underline_color,
        [1.0, 0.0, 0.0, 1.0],
        "SGR 58 color must reach the instance deco channel"
    );
}

/// alpha >= 128 的搜索高亮先交换前景/背景，再混合背景。
#[test]
fn search_highlight_swaps_and_blends() {
    let cells = vec![cell_data(
        2,
        3,
        'D',
        [1.0, 0.0, 0.0, 1.0],
        [0.0, 0.0, 1.0, 1.0],
        0,
    )];
    let hl = SearchHighlight {
        row: 2,
        start_col: 3,
        end_col_exclusive: 4,
        color: [0xFF, 0xFF, 0x00, 0xFF], // 不透明黄
    };
    let instances = build(&cells, CellCursor::default(), &[hl]);
    // alpha >= 128 → 交换前景/背景，随后 background = blend(background, yellow, alpha=1) = yellow。
    assert_eq!(
        instances[0].foreground,
        [0.0, 0.0, 1.0, 1.0],
        "highlight swaps foreground→background"
    );
    assert_eq!(
        instances[0].background,
        [1.0, 1.0, 0.0, 1.0],
        "blend with opaque yellow"
    );
}

/// alpha 低于 128 的高亮只混合、不交换。
#[test]
fn search_highlight_blends_without_swap() {
    let cells = vec![cell_data(
        0,
        0,
        'E',
        [0.0, 0.0, 0.0, 1.0],
        [1.0, 1.0, 1.0, 1.0],
        0,
    )];
    let hl = SearchHighlight {
        row: 0,
        start_col: 0,
        end_col_exclusive: 1,
        color: [0xFF, 0x00, 0x00, 0x7F], // alpha ~0.5 的红（低于 128 交换阈值）
    };
    let instances = build(&cells, CellCursor::default(), &[hl]);
    assert_eq!(
        instances[0].foreground,
        [0.0, 0.0, 0.0, 1.0],
        "foreground unchanged below alpha 128"
    );
    // background = white * (1-a) + red * a，其中 a = 0x7F/255
    let alpha = GhosttyTerminal::byte_to_float(0x7F);
    let expected = 1.0 - alpha;
    assert!(
        (instances[0].background[0] - 1.0).abs() < 1e-5,
        "red channel keeps base white"
    );
    assert!((instances[0].background[1] - expected).abs() < 1e-5);
    assert!((instances[0].background[2] - expected).abs() < 1e-5);
}

/// 方块光标整格反色（termux `invertCursorTextColor`）：
/// 背景取不透明光标色，文字取原格背景色，二者不得同色。
#[test]
fn block_cursor_paints_background_keeps_foreground() {
    let cells = vec![cell_data(
        5,
        5,
        'F',
        [1.0, 0.0, 0.0, 1.0],
        [0.0, 0.0, 1.0, 1.0],
        0,
    )];
    let cursor = CellCursor {
        row: 5,
        col: 5,
        visible: true,
        style: CursorStyle::Block,
        color: [0.0, 1.0, 0.0, 1.0],
    };
    let instances = build(&cells, cursor, &[]);
    assert_eq!(
        instances[0].foreground,
        [0.0, 0.0, 1.0, 1.0],
        "block cursor text takes the cell background color (inverted)"
    );
    assert_eq!(
        instances[0].background,
        [0.0, 1.0, 0.0, 1.0],
        "block cursor background is the opaque cursor color"
    );
}

/// 方块光标恒为整格矩形（spec cursor-rendering「方块光标按整格几何」）：
/// 不随格内字形变高变矮，也不越过行界。
#[test]
fn block_cursor_quad_is_the_whole_cell() {
    const FG: [f32; 4] = [1.0, 1.0, 1.0, 1.0];
    const BG: [f32; 4] = [0.0, 0.0, 0.0, 1.0];

    let cell_width = 1024.0 / 80.0;
    let cell_height = 1024.0 / 24.0;
    let cursor = CellCursor {
        row: 0,
        col: 0,
        visible: true,
        style: CursorStyle::Block,
        color: [0.0, 1.0, 0.0, 1.0],
    };
    let cases = [
        ("空白", cell_data(0, 0, ' ', FG, BG, 0), 1u32),
        ("码点 0", cell_data(0, 0, '\0', FG, BG, 0), 1),
        ("升部", cell_data(0, 0, 'b', FG, BG, 0), 1),
        ("降部", cell_data(0, 0, 'g', FG, BG, 0), 1),
        ("宽字符", cell_data(0, 0, '中', FG, BG, 0), 2),
    ];
    for (name, mut cell, width) in cases {
        cell.width = width;
        let instances = build(&[cell], cursor, &[]);
        assert_eq!(
            instances[0].quad_origin,
            [0.0, 0.0],
            "{name}: 光标盒顶边即单元顶边"
        );
        assert_eq!(
            instances[0].quad_size,
            [cell_width * width as f32, cell_height],
            "{name}: 光标盒必须等于整格"
        );
    }
}

/// 空单元格（空格）只发背景四边形，绝不发字形实例。
#[test]
fn empty_cell_emits_background_quad() {
    let cells = vec![cell_data(
        0,
        0,
        ' ',
        [0.9, 0.9, 0.9, 1.0],
        [0.1, 0.1, 0.1, 1.0],
        0,
    )];
    let instances = build(&cells, CellCursor::default(), &[]);
    assert_eq!(instances.len(), 1);
    assert_eq!(
        instances[0].atlas_size, [0.0; 2],
        "no glyph UVs for a space"
    );
    assert_eq!(instances[0].background, [0.1, 0.1, 0.1, 1.0]);
}

/// 竖线光标在空单元格必须是左侧细竖条，而非整格方块。
#[test]
fn bar_cursor_on_empty_cell_is_thin_vertical() {
    let cursor = CellCursor {
        row: 0,
        col: 5,
        visible: true,
        style: crate::terminal::ghostty_terminal::CursorStyle::Bar,
        color: [1.0, 1.0, 1.0, 1.0],
    };
    let cells = vec![cell_data(0, 5, '\0', [1.0; 4], [0.0; 4], 0)];
    let instances = build(&cells, cursor, &[]);
    assert_eq!(instances.len(), 1, "empty bar cursor emits one quad");
    let cell_width = 1024.0 / 80.0;
    let cell_height = 1024.0 / 24.0;
    let width = instances[0].quad_size[0];
    let height = instances[0].quad_size[1];
    let origin_y = instances[0].quad_origin[1];
    assert!(
        width <= cell_width * 0.5,
        "bar width {width} must stay thin within cell {cell_width}"
    );
    assert!(
        width >= CURSOR_MARKER_MINIMUM_THICKNESS - 0.01,
        "bar width {width} must stay visible"
    );
    assert!(
        height >= CURSOR_MARKER_MINIMUM_THICKNESS - 0.01,
        "bar height {height} must stay visible"
    );
    assert!(
        origin_y >= 0.0 && origin_y + height <= cell_height + 0.5,
        "bar spans y [{origin_y}, {}] but row 0 ends at {cell_height}",
        origin_y + height
    );
    assert!(
        (instances[0].quad_origin[0] - 5.0 * cell_width).abs() <= 0.5,
        "bar must sit at the cell left edge"
    );
}
/// 下划线光标在空单元格必须是底部细横条，而非整格方块。
#[test]
fn underline_cursor_on_empty_cell_is_thin_horizontal() {
    let cursor = CellCursor {
        row: 0,
        col: 7,
        visible: true,
        style: crate::terminal::ghostty_terminal::CursorStyle::Underline,
        color: [1.0, 1.0, 1.0, 1.0],
    };
    let cells = vec![cell_data(0, 7, '\0', [1.0; 4], [0.0; 4], 0)];
    let instances = build(&cells, cursor, &[]);
    assert_eq!(instances.len(), 1, "empty underline cursor emits one quad");
    let cell_width = 1024.0 / 80.0;
    let cell_height = 1024.0 / 24.0;
    let width = instances[0].quad_size[0];
    let height = instances[0].quad_size[1];
    assert!(
        (width - cell_width).abs() <= 0.5,
        "underline width {width} must span the cell {cell_width}"
    );
    assert!(
        height <= cell_height * 0.5,
        "underline height {height} must stay thin within cell {cell_height}"
    );
    let bottom = instances[0].quad_origin[1] + height;
    assert!(
        bottom <= cell_height + 0.5,
        "underline bottom {bottom} must stay inside row 0 ({cell_height})"
    );
}
/// 非默认底色（选区/SGR 48）或带装饰的空白格 + 细光标：标记之外必须另铺
/// 整格四边形，否则全量帧的 Clear 会把格内底色与下划线抹掉、只留细条。
#[test]
fn thin_cursor_on_tinted_blank_cell_keeps_cell_background() {
    let cursor = CellCursor {
        row: 0,
        col: 3,
        visible: true,
        style: crate::terminal::ghostty_terminal::CursorStyle::Bar,
        color: [1.0, 1.0, 1.0, 1.0],
    };
    let tint = [0.2, 0.4, 0.6, 1.0];
    let flags = 1 << cell_flags::UNDERLINE;
    let cells = vec![cell_data(0, 3, ' ', [1.0; 4], tint, flags)];
    let instances = build(&cells, cursor, &[]);
    assert_eq!(
        instances.len(),
        2,
        "tinted blank cell needs a full-cell quad plus the bar marker"
    );
    let cell_width = 1024.0 / 80.0;
    let cell_height = 1024.0 / 24.0;
    assert_eq!(instances[0].background, tint, "cell tint must survive");
    assert_eq!(instances[0].quad_size, [cell_width, cell_height]);
    assert_eq!(instances[0].flags, flags as f32, "decoration must survive");
    assert!(
        instances[1].quad_size[0] < cell_width * 0.5,
        "second quad is the thin bar marker"
    );
}

/// 空白网格不再为每格发射背景四边形，实例数只随有效字符增长。
#[test]
fn blank_default_cells_do_not_emit_instances() {
    let mut cells = vec![cell_data(0, 0, 'A', [1.0; 4], [0.0; 4], 0)];
    for col in 1..80 {
        cells.push(cell_data(0, col, '\0', [1.0; 4], [0.0; 4], 0));
    }
    let instances = build(&cells, CellCursor::default(), &[]);
    assert_eq!(
        instances.len(),
        1,
        "only the glyph cell should emit an instance"
    );
}

/// 空白格只有与清除色一致且无光标/装饰时才可省略背景四边形。
#[test]
fn blank_cell_quad_skipped_only_when_clear_color_and_undecorated() {
    let background = [0.1, 0.2, 0.3, 1.0];
    assert!(!blank_cell_needs_quad(0, false, background, background));
    assert!(blank_cell_needs_quad(0, true, background, background));
    assert!(blank_cell_needs_quad(
        1 << cell_flags::UNDERLINE,
        false,
        background,
        background
    ));
    assert!(blank_cell_needs_quad(
        0,
        false,
        [0.1, 0.2, 0.4, 1.0],
        background
    ));
}

/// 非空单元格竖线光标：字形保持原文色并追加一枚标记。
#[test]
fn bar_cursor_on_glyph_cell_emits_glyph_plus_marker() {
    let cursor = CellCursor {
        row: 0,
        col: 0,
        visible: true,
        style: crate::terminal::ghostty_terminal::CursorStyle::Bar,
        color: [1.0, 1.0, 1.0, 1.0],
    };
    let cells = vec![cell_data(
        0,
        0,
        'A',
        [1.0, 0.0, 0.0, 1.0],
        [0.0, 0.0, 0.0, 1.0],
        0,
    )];
    let instances = build(&cells, cursor, &[]);
    assert_eq!(
        instances.len(),
        2,
        "bar cursor on a glyph emits the glyph plus one marker"
    );
    assert_eq!(
        instances[0].foreground,
        [1.0, 0.0, 0.0, 1.0],
        "glyph keeps its own foreground under a bar cursor"
    );
    assert_eq!(
        instances[0].background,
        [0.0, 0.0, 0.0, 1.0],
        "glyph keeps its own background under a bar cursor"
    );
    assert_eq!(
        instances[1].atlas_size,
        [0.0, 0.0],
        "marker is a solid color quad without glyph content"
    );
    assert!(
        instances[1].quad_size[0] <= instances[0].quad_size[0] * 0.5,
        "marker must stay thin relative to the glyph quad"
    );
}
