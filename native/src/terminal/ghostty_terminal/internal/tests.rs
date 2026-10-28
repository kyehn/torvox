use super::*;

use crate::terminal::ghostty_terminal::GhosttyTerminal;
use libghostty_vt::style::{Style, StyleColor, Underline};

#[test]
pub(crate) fn byte_to_float_scales_255() {
    assert_eq!(GhosttyTerminal::byte_to_float(0), 0.0);
    assert_eq!(GhosttyTerminal::byte_to_float(255), 1.0);
    assert!((GhosttyTerminal::byte_to_float(128) - 128.0 / 255.0).abs() < 1e-6);
}

#[test]
pub(crate) fn byte_color_to_float_includes_opaque_alpha() {
    let color = GhosttyTerminal::color_to_float(libghostty_vt::style::RgbColor {
        r: 13,
        g: 188,
        b: 121,
    });
    assert!((color[0] - 13.0 / 255.0).abs() < 1e-6);
    assert!((color[1] - 188.0 / 255.0).abs() < 1e-6);
    assert!((color[2] - 121.0 / 255.0).abs() < 1e-6);
    assert_eq!(color[3], 1.0, "alpha must be opaque");
}

/// OSC 4 调色板覆盖必须生效：有效调色板（含覆盖）而非静态表决定渲染色。
#[test]
pub(crate) fn osc4_palette_override_reaches_dumped_grid() {
    let mut terminal = GhosttyTerminal::new(5, 20, 100).expect("terminal");
    // 调色板索引 1 改为纯绿，再以红色（索引 1）写字：看到的必须是绿色。
    terminal.vt_write(b"\x1b]4;1;#00ff00\x07\x1b[31mX");
    terminal.flush();
    let dumped = terminal.dump_grid();
    let cell = &dumped.visible[0];
    assert_eq!(cell.codepoint, 'X' as u32);
    assert_eq!(
        cell.foreground,
        GhosttyTerminal::color_to_float(libghostty_vt::style::RgbColor { r: 0, g: 255, b: 0 }),
        "OSC 4 override must win over the static palette"
    );
}

/// SGR 58 下划线色进入快照（未设置时回退前景）。
#[test]
pub(crate) fn sgr58_underline_color_reaches_dumped_grid() {
    let mut terminal = GhosttyTerminal::new(5, 20, 100).expect("terminal");
    terminal.vt_write(b"\x1b[4m\x1b[58;2;255;0;0mU");
    terminal.flush();
    let dumped = terminal.dump_grid();
    let cell = &dumped.visible[0];
    assert!(cell.underline, "SGR 4 must set underline");
    assert_eq!(
        cell.underline_color,
        GhosttyTerminal::color_to_float(libghostty_vt::style::RgbColor { r: 255, g: 0, b: 0 }),
        "SGR 58 must set the underline color"
    );
}

#[test]
pub(crate) fn underline_color_falls_back_to_foreground() {
    let mut terminal = GhosttyTerminal::new(5, 20, 100).expect("terminal");
    terminal.vt_write(b"\x1b[4m\x1b[31mV");
    terminal.flush();
    let dumped = terminal.dump_grid();
    let cell = &dumped.visible[0];
    assert!(cell.underline);
    assert_eq!(
        cell.underline_color, cell.foreground,
        "unset SGR 58 must fall back to the resolved foreground"
    );
}

/// SGR 53 上划线进入快照（与 SGR 58 用例对称的端到端覆盖）。
#[test]
pub(crate) fn sgr53_overline_reaches_dumped_grid() {
    let mut terminal = GhosttyTerminal::new(5, 20, 100).expect("terminal");
    terminal.vt_write(b"\x1b[53mO");
    terminal.flush();
    let dumped = terminal.dump_grid();
    let cell = &dumped.visible[0];
    assert!(cell.overline, "SGR 53 must set overline");
}

/// SGR 3 斜体进入快照：斜体缺失首先排除 VT 层，回应对“斜体无法显示”。
#[test]
pub(crate) fn sgr3_italic_reaches_dumped_grid() {
    let mut terminal = GhosttyTerminal::new(5, 20, 100).expect("terminal");
    terminal.vt_write(b"\x1b[3mI");
    terminal.flush();
    let dumped = terminal.dump_grid();
    let cell = &dumped.visible[0];
    assert_eq!(cell.codepoint, 'I' as u32);
    assert!(cell.italic, "SGR 3 must set italic");
}

/// SGR 31 红色前景进入快照：颜色缺失首先排除 VT 层，回应对“颜色只显示背景”。
#[test]
pub(crate) fn sgr31_red_foreground_reaches_dumped_grid() {
    let mut terminal = GhosttyTerminal::new(5, 20, 100).expect("terminal");
    terminal.vt_write(b"\x1b[31mR");
    terminal.flush();
    let dumped = terminal.dump_grid();
    let cell = &dumped.visible[0];
    assert_eq!(cell.codepoint, 'R' as u32);
    // 默认主题调色板索引 1，非纯红：断言有效调色板色即证明链路。
    // 色值取自默认调色板单源，不手写。
    assert_eq!(
        cell.foreground,
        GhosttyTerminal::color_to_float(crate::terminal::theme::default_dark().palette.ansi[1]),
        "SGR 31 must resolve to palette index 1"
    );
}

pub(crate) fn style_with_flags() -> Style {
    Style {
        bold: true,
        italic: true,
        faint: true,
        blink: false,
        inverse: true,
        invisible: false,
        strikethrough: true,
        overline: true,
        underline: Underline::Double,
        fg_color: StyleColor::None,
        bg_color: StyleColor::None,
        underline_color: StyleColor::None,
    }
}

#[test]
pub(crate) fn pack_style_flags_sets_expected_bits() {
    let flags = GhosttyTerminal::pack_style_flags(&style_with_flags());
    assert_ne!(flags & (1 << cell_flags::BOLD), 0, "bold bit");
    assert_ne!(flags & (1 << cell_flags::ITALIC), 0, "italic bit");
    assert_ne!(flags & (1 << cell_flags::REVERSE), 0, "reverse bit");
    assert_ne!(flags & (1 << cell_flags::UNDERLINE), 0, "underline bit");
    assert_ne!(
        flags & (1 << cell_flags::DOUBLE_UNDERLINE),
        0,
        "double underline bit"
    );
    assert_ne!(
        flags & (1 << cell_flags::STRIKETHROUGH),
        0,
        "strikethrough bit"
    );
    assert_ne!(flags & (1 << cell_flags::OVERLINE), 0, "overline bit");
    assert_ne!(flags & (1 << cell_flags::FAINT), 0, "faint bit");
    assert_eq!(
        flags & (1 << cell_flags::CONCEAL),
        0,
        "conceal bit stays clear for a visible style"
    );
    let concealed = Style {
        invisible: true,
        ..Default::default()
    };
    assert_eq!(
        GhosttyTerminal::pack_style_flags(&concealed),
        1 << cell_flags::CONCEAL
    );
}

#[test]
pub(crate) fn pack_style_flags_double_underline_sets_underline_too() {
    // 双下划线必须同时置上普通下划线位，着色器的高亮路径才会
    // 把该单元当作带下划线处理。
    let flags = GhosttyTerminal::pack_style_flags(&style_with_flags());
    let both = (1 << cell_flags::UNDERLINE) | (1 << cell_flags::DOUBLE_UNDERLINE);
    assert_eq!(flags & both, both);
}

#[test]
pub(crate) fn pack_style_flags_default_style_is_zero() {
    let flags = GhosttyTerminal::pack_style_flags(&Style::default());
    assert_eq!(flags, 0);
}

#[test]
pub(crate) fn pack_style_flags_plain_underline_sets_only_underline_bit() {
    let style = Style {
        underline: Underline::Single,
        ..Default::default()
    };
    let flags = GhosttyTerminal::pack_style_flags(&style);
    assert_eq!(flags, 1 << cell_flags::UNDERLINE);
}

#[test]
pub(crate) fn pack_style_flags_curly_dashed_dotted_keep_underline_without_double() {
    // 对标上游下划线样式回归：Curly/Dashed/Dotted 必须保留下划线，
    // 且不得误置双下划线位。
    for underline in [Underline::Curly, Underline::Dashed, Underline::Dotted] {
        let style = Style {
            underline,
            ..Default::default()
        };
        let flags = GhosttyTerminal::pack_style_flags(&style);
        assert_eq!(
            flags & (1 << cell_flags::UNDERLINE),
            1 << cell_flags::UNDERLINE,
            "underline bit must survive"
        );
        assert_eq!(
            flags & (1 << cell_flags::DOUBLE_UNDERLINE),
            0,
            "double bit must stay clear"
        );
    }
}
