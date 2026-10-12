//! 主题解析测试。
//!
//! 测试内不书写任何主题标识：它们只在 [`SOURCES`] 出现一次，故此处一律经清单
//! 下标或 [`Palette::parse`] 的内联夹具断言。这样新增、改名或调整分组主题都
//! 不会牵连测试，也无需在此同步一份名字副本。

use super::*;

use libghostty_vt::style::RgbColor;

/// 只给主色与十六色、两个反色段全缺的主题：钉住缺段回退到前景色。
const INVERTED_SECTIONS_MISSING: &str = r"
[colors.primary]
background = '#101020'
foreground = '#c0c0d0'

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

/// 十六色缺失即解析失败：清单内各主题都写全两组十六色，回退到任何默认调色板
/// 都只会让损坏的源被悄悄渲染成另一种配色。
#[test]
fn missing_ansi_group_is_a_parse_error() {
    let only_primary = r"
[colors.primary]
background = '#101020'
foreground = '#c0c0d0'
";
    assert!(
        Palette::parse("test", only_primary).is_err(),
        "缺十六色段必须报错而非回退"
    );
}

/// 色值不是 ghostty 颜色语法时报错，错误信息含主题标识与出错的色值原文。
#[test]
fn malformed_color_names_the_offending_value() {
    let broken = INVERTED_SECTIONS_MISSING.replace("red = '#020202'", "red = '#02020'");
    let error = Palette::parse("broken", &broken).expect_err("残缺色值必须报错");
    assert!(
        error.contains("broken") && error.contains("#02020"),
        "错误信息须含主题标识与出错色值: {error}"
    );
}

/// 缺反色段时取前景色（同上游 `InvertedCellColors::default`）；`CellForeground`
/// 同义，`CellBackground` 依赖单元格实际背景、解析期无从得知，故报错。
#[test]
fn inverted_sections_default_to_foreground() {
    let missing = parse(INVERTED_SECTIONS_MISSING);
    assert_eq!(missing.cursor, missing.foreground);
    assert_eq!(missing.selection_background, missing.foreground);

    let symbolic = parse(&format!(
        "{INVERTED_SECTIONS_MISSING}
[colors.selection]
background = '{CELL_FOREGROUND}'
"
    ));
    assert_eq!(symbolic.selection_background, symbolic.foreground);

    let cell_background = format!(
        "{INVERTED_SECTIONS_MISSING}
[colors.cursor]
cursor = 'CellBackground'
"
    );
    assert!(
        Palette::parse("test", &cell_background).is_err(),
        "CellBackground 在解析期无从定值，必须报错"
    );
}

/// 反色段的两处键名都是上游别名，须都接受；十六色按 `normal` 在前、`bright` 在后
/// 的槽位顺序展开。
#[test]
fn aliases_and_ansi_slot_order_follow_the_source_layout() {
    let palette = parse(&format!(
        "{INVERTED_SECTIONS_MISSING}
[colors.cursor]
text = '#101020'
cursor = '#0a0a0b'

[colors.selection]
text = 'CellBackground'
background = '#1a1a2c'
"
    ));
    assert_eq!(
        palette.cursor,
        RgbColor {
            r: 0x0a,
            g: 0x0a,
            b: 0x0b
        }
    );
    assert_eq!(
        palette.selection_background,
        RgbColor {
            r: 0x1a,
            g: 0x1a,
            b: 0x2c
        }
    );
    assert_eq!(
        palette.ansi[0],
        RgbColor {
            r: 0x01,
            g: 0x01,
            b: 0x01
        }
    );
    assert_eq!(
        palette.ansi[7],
        RgbColor {
            r: 0x08,
            g: 0x08,
            b: 0x08
        }
    );
    assert_eq!(
        palette.ansi[8],
        RgbColor {
            r: 0x09,
            g: 0x09,
            b: 0x09
        }
    );
    assert_eq!(
        palette.ansi[15],
        RgbColor {
            r: 0x10,
            g: 0x10,
            b: 0x10
        }
    );
}

/// 三字符缩写由上游 ghostty 颜色语法展开，与 alacritty 的 `#RGB` 一致；
/// `#RRGGBB` 同样被接受。
#[test]
fn upstream_color_syntax_expands_hex_shorthand() {
    assert_eq!(
        RgbColor::parse("#abc").expect("缩写色值"),
        RgbColor {
            r: 0xaa,
            g: 0xbb,
            b: 0xcc
        }
    );
    assert_eq!(
        RgbColor::parse("#AABBCC").expect("六位色值"),
        RgbColor {
            r: 0xaa,
            g: 0xbb,
            b: 0xcc
        }
    );
}

/// 每个源都能解析出调色板，且标识与 alacritty 文件名逐字相同——标识同时是构建期
/// 路径片段与设置中落盘的键值，二者必须同源。
#[test]
fn every_source_parses_and_keeps_its_identifier() {
    let themes = all();
    assert_eq!(themes.len(), SOURCES.len());
    for theme in themes {
        assert_eq!(
            theme.palette.background,
            Palette::parse(theme.name, source_of(theme.name))
                .expect("清单项应可解析")
                .background,
            "{} 的标识与清单不一致",
            theme.name,
        );
        assert!(
            theme
                .palette
                .ansi
                .iter()
                .all(|color| *color != RgbColor::default())
        );
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
    // 默认主题以清单下标声明，故下标越界（在此 panic）或错组都会在此暴露。
    assert!(default_dark().dark);
    assert!(!by_name(default_light_name()).expect("默认日间主题").dark);
    assert_ne!(DEFAULT_DARK_INDEX, DEFAULT_LIGHT_INDEX);
}
