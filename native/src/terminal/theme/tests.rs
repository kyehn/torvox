//! 主题解析测试。
//!
//! 测试内不书写任何主题标识：它们只在 [`SOURCES`] 出现一次，故此处一律经清单
//! 下标或 [`Palette::parse`] 的内联夹具断言。这样新增、改名或调整分组主题都
//! 不会牵连测试，也无需在此同步一份名字副本。

use super::*;

/// 最小主题：只给 `primary`，其余段全部缺失，用于钉住上游默认回退。
const MINIMAL: &str = r"
[colors.primary]
background = '#101020'
foreground = '#c0c0d0'
";

/// 完整主题：两个反色段各用其别名键，十六色两色组齐全。
const COMPLETE: &str = r"
[colors.primary]
background = '#101020'
foreground = '#c0c0d0'

[colors.cursor]
text = '#101020'
cursor = '#0a0a0b'

[colors.selection]
text = 'CellBackground'
background = '#1a1a2c'

[colors.normal]
black = '#010101'
red = '#020202'
green = '#030303'
yellow = '#040404'
blue = '#050505'
magenta = '#060606'
cyan = '#070707'
white = '#080808'

[colors.bright]
black = '#090909'
red = '#0a0a0a'
green = '#0b0b0b'
yellow = '#0c0c0c'
blue = '#0d0d0d'
magenta = '#0e0e0e'
cyan = '#0f0f0f'
white = '#101010'
";

fn parse(text: &str) -> Palette {
    Palette::parse("test", text).expect("内联夹具应可解析")
}

/// 缺段时按上游默认回退：反色底色取前景色，十六色取 alacritty 的
/// `NormalColors`/`BrightColors` 默认值。回退值取自上游而非本仓库另造。
#[test]
fn missing_sections_fall_back_to_alacritty_defaults() {
    let palette = parse(MINIMAL);
    assert_eq!(palette.background, [0x10, 0x10, 0x20]);
    assert_eq!(palette.foreground, [0xc0, 0xc0, 0xd0]);
    assert_eq!(palette.cursor, palette.foreground);
    assert_eq!(palette.selection_background, palette.foreground);
    assert_eq!(&palette.ansi[..8], &default_normal());
    assert_eq!(&palette.ansi[8..], &default_bright());
}

/// 反色段的两处键名都是上游别名，须都接受；十六色按 `normal` 在前、`bright` 在后
/// 的槽位顺序展开。
#[test]
fn aliases_and_ansi_slot_order_follow_the_source_layout() {
    let palette = parse(COMPLETE);
    assert_eq!(palette.cursor, [0x0a, 0x0a, 0x0b]);
    assert_eq!(palette.selection_background, [0x1a, 0x1a, 0x2c]);
    assert_eq!(palette.ansi[0], [0x01, 0x01, 0x01]);
    assert_eq!(palette.ansi[7], [0x08, 0x08, 0x08]);
    assert_eq!(palette.ansi[8], [0x09, 0x09, 0x09]);
    assert_eq!(palette.ansi[15], [0x10, 0x10, 0x10]);
}

/// `CellForeground` 取前景色；`CellBackground` 依赖单元格实际背景、解析期无从得知，
/// 故退回前景色而非误用别处的值。
#[test]
fn symbolic_colors_resolve_or_fall_back_to_foreground() {
    let symbolic = parse(
        r"
[colors.primary]
background = '#101020'
foreground = '#c0c0d0'

[colors.selection]
background = 'CellForeground'

[colors.cursor]
cursor = 'CellBackground'
",
    );
    assert_eq!(symbolic.selection_background, symbolic.foreground);
    assert_eq!(symbolic.cursor, symbolic.foreground);
}

/// 三字符缩写每位加倍，与 alacritty 的 `#RGB` 展开一致；位数非 6/3 则无法解析。
#[test]
fn hex_shorthand_expands_and_rejects_other_lengths() {
    assert_eq!(parse_hex("#abc"), Some([0xaa, 0xbb, 0xcc]));
    assert_eq!(parse_hex("#AABBCC"), Some([0xaa, 0xbb, 0xcc]));
    assert_eq!(parse_hex("#abcd"), None);
    assert_eq!(parse_hex("abc"), None);
}

/// 每个源都能解析出调色板，且标识与 alacritty 文件名逐字相同——标识同时是构建期
/// 路径片段与设置中落盘的键值，二者必须同源。
#[test]
fn every_source_parses_and_keeps_its_identifier() {
    let themes = all();
    assert_eq!(themes.len(), SOURCES.len());
    for theme in &themes {
        assert_eq!(
            theme.palette.background,
            Palette::parse(theme.name, source_of(theme.name))
                .expect("清单项应可解析")
                .background,
            "{} 的标识与清单不一致",
            theme.name,
        );
        assert!(theme.palette.ansi.iter().all(|color| *color != [0, 0, 0]));
    }
    let unique: std::collections::BTreeSet<&str> = themes.iter().map(|theme| theme.name).collect();
    assert_eq!(unique.len(), themes.len(), "主题标识重复：{unique:?}");
    assert!(by_name("no-such-theme").is_none());
}

/// 深底判定驱动「跟随系统」的日夜切换，两组不交错；默认主题落在各自主组。
#[test]
fn dark_and_light_groups_are_separated() {
    let themes = all();
    let first_light = themes.iter().position(|theme| !theme.dark).unwrap();
    assert!(
        themes[..first_light].iter().all(|theme| theme.dark)
            && themes[first_light..].iter().all(|theme| !theme.dark),
        "深底与日间主题在 all() 中交错"
    );
    // 默认主题以清单下标声明，故下标越界或错组都会在此暴露。
    assert!(by_name(default_dark_name()).expect("默认夜间主题").dark);
    assert!(!by_name(default_light_name()).expect("默认日间主题").dark);
    assert_eq!(default_dark_name(), SOURCES[DEFAULT_DARK_INDEX].0);
    assert_eq!(default_light_name(), SOURCES[DEFAULT_LIGHT_INDEX].0);
    assert_ne!(DEFAULT_DARK_INDEX, DEFAULT_LIGHT_INDEX);
}

/// 深浅阈值对两端主题都留有余量：贴近中灰的背景不会被误判。
#[test]
fn dark_threshold_separates_luminance_extremes() {
    assert!(is_dark([0x21, 0x21, 0x21]));
    assert!(!is_dark([0xfb, 0xf1, 0xc7]));
}
