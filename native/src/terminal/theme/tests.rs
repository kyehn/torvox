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

/// 覆盖通道的槽位顺序：背景、前景、光标，随后 16 个 ANSI 色。
///
/// `None` 的槽位必须原样保留。只写背景色的 `colors.properties` 是最常见的一种，
/// 若实现成「未给的槽位清零」，用户改个背景色会连带丢掉光标色与全部十六色。
#[test]
fn overrides_replace_only_the_slots_that_are_given() {
    let base = parse(&format!(
        "{INVERTED_SECTIONS_MISSING}
[colors.cursor]
text = '#101020'
cursor = '#0a0a0b'

[colors.selection]
text = 'CellBackground'
background = '#1a1a2c'
"
    ));
    let override_color = RgbColor {
        r: 0xAB,
        g: 0xCD,
        b: 0xEF,
    };

    let unchanged = base.with_overrides(&[None; OVERRIDE_SLOTS]);
    assert_eq!(unchanged, base);

    let mut overrides = [None; OVERRIDE_SLOTS];
    overrides[0] = Some(override_color);
    let background_only = base.with_overrides(&overrides);
    assert_eq!(background_only.background, override_color);
    assert_eq!(background_only.foreground, base.foreground);
    assert_eq!(background_only.cursor, base.cursor);
    assert_eq!(background_only.ansi, base.ansi);
    // 选区色没有对应槽位，任何覆盖都不该动它。
    assert_eq!(
        background_only.selection_background,
        base.selection_background
    );
}

/// 十九个槽位逐个落到各自的位置，一个不错位。
///
/// 错位是这一路最容易出、又最难定位的缺陷：把光标色塞进 `ansi[0]` 不会报错，
/// 只会表现为「某些颜色不对」。故每个槽位给一个不同色值，逐个比对。
#[test]
fn every_override_slot_lands_in_its_own_field() {
    let base = parse(INVERTED_SECTIONS_MISSING);
    let overrides: [Option<RgbColor>; OVERRIDE_SLOTS] = std::array::from_fn(|slot| {
        Some(RgbColor {
            r: slot as u8,
            g: (slot * 2) as u8,
            b: (slot * 3) as u8,
        })
    });

    let palette = base.with_overrides(&overrides);
    let expect = |slot: usize| RgbColor {
        r: slot as u8,
        g: (slot * 2) as u8,
        b: (slot * 3) as u8,
    };
    assert_eq!(palette.background, expect(0));
    assert_eq!(palette.foreground, expect(1));
    assert_eq!(palette.cursor, expect(2));
    for slot in 0..16 {
        assert_eq!(palette.ansi[slot], expect(3 + slot), "ANSI 槽 {slot} 错位");
    }
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

/// 深底标记与首屏分组必须由背景亮度决定，且清单按深底在前排好。
///
/// 深底标记决定「跟随系统」在日夜两组间怎么选，而唯一的判据是上游文档给的那条
/// 亮度阈值（见 `Theme::dark` 与 `THEMES` 的排序）。此前这里只断言「标识与清单同源」，
/// 那等于把 `SOURCES` 和 `THEMES` 各解析一遍互相比对——两者同源，恒成立。
#[test]
fn dark_flag_and_leading_order_follow_background_luminance() {
    let themes = all();
    assert_eq!(themes.len(), SOURCES.len());
    for theme in themes {
        assert_eq!(
            theme.dark,
            theme.palette.background.perceived_luminance() <= 0.5,
            "{} 的深底标记与其背景亮度不符",
            theme.name,
        );
    }
    // 深底在前：任一深底主题的序号都不得晚于任一日间主题。
    let last_dark = themes.iter().rposition(|theme| theme.dark);
    let first_light = themes.iter().position(|theme| !theme.dark);
    if let (Some(last_dark), Some(first_light)) = (last_dark, first_light) {
        assert!(
            last_dark < first_light,
            "深底主题未排在日间主题之前：{last_dark} vs {first_light}"
        );
    }
    let unique: std::collections::BTreeSet<&str> = themes.iter().map(|theme| theme.name).collect();
    assert_eq!(unique.len(), themes.len(), "主题标识重复：{unique:?}");
    assert!(by_name("no-such-theme").is_none());
}

/// 清单规模须等于 DESIGN「终端主题」节点名的主题数。
///
/// 外部标准（DESIGN 点名 13 个）在此落成一条数：清单曾只有 10 个
/// （缺 `github_dark_high_contrast`、`moonfly`、`pastel_dark`），设置页因此少三项且
/// 无人察觉——名称不写进测试（见本模块文档），故规模是唯一能钉住这条外部约定的量。
#[test]
fn catalog_size_matches_the_design_mandated_set() {
    const DESIGN_MANDATED_THEME_COUNT: usize = 13;
    assert_eq!(
        SOURCES.len(),
        DESIGN_MANDATED_THEME_COUNT,
        "主题清单与 DESIGN 点名的主题数不符：{:?}",
        SOURCES.iter().map(|(name, _)| *name).collect::<Vec<_>>(),
    );
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

/// 目录打包：逐字段按布局读回，与 [`super::pack_catalog`] 的文档一致。
///
/// 判据不只是「能解开」：条数、顺序、名称、深底与默认标记、颜色三元组各自与
/// [`all`] / [`default_dark`] / [`default_light_name`] 对得上——打包与真源一旦
/// 漂移，设置页就会按另一套顺序或另一个默认主题渲染。
#[test]
fn catalog_packs_every_theme_in_source_order() {
    const ENTRY_COLOR_BYTES: usize = 54;

    struct Entry {
        flags: u8,
        name: String,
        colors: Vec<u8>,
    }

    fn parse(packed: &[u8]) -> Vec<Entry> {
        let mut offset = 0usize;
        let count = u16::from_le_bytes(packed[offset..offset + 2].try_into().unwrap());
        offset += 2;
        let mut entries = Vec::with_capacity(count as usize);
        for _ in 0..count {
            let flags = packed[offset];
            let name_length = packed[offset + 1] as usize;
            offset += 2;
            let name = String::from_utf8(packed[offset..offset + name_length].to_vec()).unwrap();
            offset += name_length;
            let colors = packed[offset..offset + ENTRY_COLOR_BYTES].to_vec();
            offset += ENTRY_COLOR_BYTES;
            entries.push(Entry {
                flags,
                name,
                colors,
            });
        }
        assert_eq!(offset, packed.len(), "打包长度须与逐条读取的偏移一致");
        entries
    }

    fn color_at(colors: &[u8], index: usize) -> RgbColor {
        let base = index * 3;
        RgbColor {
            r: colors[base],
            g: colors[base + 1],
            b: colors[base + 2],
        }
    }

    let entries = parse(&super::pack_catalog());
    let themes = all();
    assert_eq!(entries.len(), themes.len());
    let default_dark_name = default_dark().name;
    let default_light_name = default_light_name();
    for (entry, theme) in entries.iter().zip(themes) {
        assert_eq!(entry.name, theme.name);
        assert_eq!(
            entry.flags & 1 != 0,
            theme.name == default_dark_name,
            "{} 的默认深色标记与真源不符",
            theme.name
        );
        assert_eq!(
            entry.flags & 2 != 0,
            theme.name == default_light_name,
            "{} 的默认浅色标记与真源不符",
            theme.name
        );
        assert_eq!(color_at(&entry.colors, 0), theme.palette.background);
        assert_eq!(color_at(&entry.colors, 1), theme.palette.foreground);
        for (index, ansi) in theme.palette.ansi.iter().enumerate() {
            assert_eq!(color_at(&entry.colors, 2 + index), *ansi, "{}", theme.name);
        }
    }
    assert_eq!(
        entries.iter().filter(|entry| entry.flags & 1 != 0).count(),
        1,
        "默认深色主题须恰有一个"
    );
    assert_eq!(
        entries.iter().filter(|entry| entry.flags & 2 != 0).count(),
        1,
        "默认浅色主题须恰有一个"
    );
}

/// 调色板必须与上游主题源里的**十六进制字面量**逐字段相等。
///
/// 期望值刻意不来自 [`Palette::parse`]：期望与被测同源时，任何解析错误都会被一并
/// 搬进期望，断言恒成立（`dark_flag_and_leading_order_follow_background_luminance`
/// 的注释已记过同一坑）。这里用一段独立的按段扫描从嵌入的 TOML 源里读出背景、前景
/// 与十六色，再与解析结果比对——槽位顺序、段错读或配色被改写都会立刻判红。
/// `CellForeground` 一类别名不在扫描器职责内，那正是解析器要自行处理的部分。
#[test]
fn palette_matches_the_hex_literals_in_the_theme_sources() {
    for &(name, text) in SOURCES {
        let palette = Palette::parse(name, text).expect("清单内主题应可解析");
        let (background, foreground, ansi) = literal_colors(name, text);
        assert_eq!(palette.background, background, "{name}: 背景与源字面量不符");
        assert_eq!(palette.foreground, foreground, "{name}: 前景与源字面量不符");
        assert_eq!(palette.ansi, ansi, "{name}: 十六色与源字面量不符");
    }
}

/// 从主题源文本扫出 `(背景, 前景, 十六色)`，全部按 `#RRGGBB` 字面量读。
///
/// 只认 `[colors.primary]` / `[colors.normal]` / `[colors.bright]` 三段的
/// `键 = <引号>#RRGGBB` 首处字面量；整行注释（以 `#` 起）与行尾注释自然略过。
/// 段名或键名对不上即 panic——源结构变了必须判红，不得静默跳过。
fn literal_colors(name: &str, source: &str) -> (RgbColor, RgbColor, [RgbColor; 16]) {
    const ANSI_KEYS: [&str; 8] = [
        "black", "red", "green", "yellow", "blue", "magenta", "cyan", "white",
    ];

    fn hex(value: &str) -> Option<RgbColor> {
        let digits = value.get(value.find('#')? + 1..)?.get(..6)?;
        Some(RgbColor {
            r: u8::from_str_radix(digits.get(..2)?, 16).ok()?,
            g: u8::from_str_radix(digits.get(2..4)?, 16).ok()?,
            b: u8::from_str_radix(digits.get(4..6)?, 16).ok()?,
        })
    }

    let mut section = String::new();
    let mut primary: [Option<RgbColor>; 2] = [None; 2];
    let mut ansi: [Option<RgbColor>; 16] = [None; 16];
    for line in source.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if let Some(heading) = line
            .strip_prefix('[')
            .and_then(|rest| rest.strip_suffix(']'))
        {
            section.clear();
            section.push_str(heading);
            continue;
        }
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        let key = key.trim();
        match section.as_str() {
            "colors.primary" if key == "background" => primary[0] = hex(value),
            "colors.primary" if key == "foreground" => primary[1] = hex(value),
            "colors.normal" | "colors.bright" => {
                let index = key;
                if let Some(slot) = ANSI_KEYS.iter().position(|candidate| *candidate == index) {
                    ansi[slot + if section == "colors.normal" { 0 } else { 8 }] = hex(value);
                }
            }
            _ => {}
        }
    }
    let missing = |what: &str| panic!("{name}: 源文本里找不到 {what} 的 #RRGGBB 字面量");
    (
        primary[0].unwrap_or_else(|| missing("背景")),
        primary[1].unwrap_or_else(|| missing("前景")),
        std::array::from_fn(|slot| {
            ansi[slot].unwrap_or_else(|| missing(&format!("十六色第 {slot} 槽")))
        }),
    )
}
