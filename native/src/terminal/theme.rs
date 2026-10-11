//! 终端主题：构建期从 <https://github.com/alacritty/alacritty-theme> 读入 alacritty
//! TOML 源，运行时解析为渲染与 UI 共用的调色板。
//!
//! 仓库不留颜色副本：TOML 源由 `include_str!` 在编译期嵌入，字段按 alacritty
//! `config/color.rs` 的结构声明，缺失字段回退到上游同类型 `Default` 的值——
//! 回退取自上游而非本仓库另造算法。
//!
//! 主题标识在 [`SOURCES`] 各出现且只出现一次：它既是 alacritty 文件名，也是设置中
//! 存储的名字，故二者不会漂移；默认主题用下标引用而非重复名字面量。

use std::sync::LazyLock;

use serde::Deserialize;

// 主题目录：alacritty 主题仓库的浅克隆（由 nix shellHook 准备，见 `flake.nix`）。
//
// 路径须为字面量：`include_str!` 与 `concat!` 不接受标识符参数，故由
// [`theme_sources!`] 在调用处声明，目录在此只写一次。用绝对路径而非相对上跳，
// 因为 `include_str!` 的相对基准是本文件位置，仓库移位即失效。

/// 把主题清单展开为 `(源名, TOML 文本)` 表。
///
/// 宏让主题名在一处书写即可同时充当 `include_str!` 的路径片段与设置中存储的名字；
/// 默认主题用清单下标引用，因此不需要第二处名字面量。
macro_rules! theme_sources {
    (
        dir = $dir:literal;
        default_dark = $dark:literal, default_light = $light:literal;
        $($name:literal,)*
    ) => {
        /// 默认（夜间）主题在 [`SOURCES`] 中的下标，对应 DESIGN 的默认终端主题。
        const DEFAULT_DARK_INDEX: usize = $dark;
        /// 默认（日间）主题在 [`SOURCES`] 中的下标。
        const DEFAULT_LIGHT_INDEX: usize = $light;
        /// `(alacritty 文件名, 构建期嵌入的 TOML 文本)`。
        static SOURCES: &[(&str, &str)] = &[
            $(($name, include_str!(concat!($dir, $name, ".toml"))),)*
        ];
    };
}

// 主题清单：DESIGN「终端主题」节指定的 10 个，不多不少，标识原样保留。
theme_sources! {
    dir = "/tmp/alacritty-theme/themes/";
    default_dark = 2, default_light = 0;
    "catppuccin_latte",
    "gruvbox_light",
    "dracula_plus",
    "tomorrow",
    "tomorrow_night",
    "tokyo_night",
    "tokyo_night_light",
    "catppuccin_mocha",
    "gruvbox_dark",
    "monokai",
}

/// 颜色取值：十六字面量，或 alacritty 的两个特殊色名。
#[derive(Clone, Debug, Deserialize)]
#[serde(untagged)]
enum Color {
    /// `'#RRGGBB'`。
    Rgb(String),
    /// `CellForeground` / `CellBackground`——指单元格自身的前景/背景，非常量色。
    Symbolic(String),
}

/// 一套主题的全部颜色。
///
/// ANSI 十六色按 alacritty 槽位顺序展开：`colors.normal` 占 0..8、`colors.bright`
/// 占 8..16。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Palette {
    pub background: [u8; 3],
    pub foreground: [u8; 3],
    pub cursor: [u8; 3],
    pub selection_background: [u8; 3],
    pub ansi: [[u8; 3]; 16],
}

/// alacritty `Colors::primary`。
#[derive(Deserialize)]
struct Primary {
    background: Color,
    foreground: Color,
}

/// ANSI 十六色的一个色组（`colors.normal` 或 `colors.bright`）。
#[derive(Deserialize)]
struct AnsiGroup {
    black: Color,
    red: Color,
    green: Color,
    yellow: Color,
    blue: Color,
    magenta: Color,
    cyan: Color,
    white: Color,
}

/// `colors.cursor` 与 `colors.selection` 的源结构：两者在 alacritty 中都是
/// `InvertedCellColors`，故共用同一形状。该类型的背景色字段在 `[colors.cursor]`
/// 段写作别名 `cursor`、在 `[colors.selection]` 段写作 `background`
/// （见上游 `InvertedCellColors` 的 `#[config(alias)]`），`#[serde(alias)]` 同时
/// 接受两者。其前景色字段（`text` / `foreground`）只影响选区文字，本应用以反色
/// 绘制选区、不消费该值，故不声明。
#[derive(Deserialize)]
struct InvertedCellColors {
    #[serde(alias = "background", alias = "cursor")]
    inverted_background: Color,
}

#[derive(Deserialize)]
struct Colors {
    primary: Primary,
    normal: Option<AnsiGroup>,
    bright: Option<AnsiGroup>,
    cursor: Option<InvertedCellColors>,
    selection: Option<InvertedCellColors>,
}

#[derive(Deserialize)]
struct ThemeFile {
    colors: Colors,
}

impl Palette {
    /// 解析一套 alacritty 主题源。
    ///
    /// 缺段时按上游默认回退，与 alacritty 加载同一份配置的结果一致：
    /// `cursor` / `selection` 的底色缺省为单元格前景色
    /// （`InvertedCellColors::default` 的 `background: CellForeground`），
    /// 十六色缺项回退到 `NormalColors` / `BrightColors` 的 `Default`。
    fn parse(name: &str, text: &str) -> Result<Self, String> {
        let Colors {
            primary,
            normal,
            bright,
            cursor,
            selection,
        } = toml::from_str::<ThemeFile>(text)
            .map_err(|error| format!("{name}: {error}"))?
            .colors;

        let background = resolve(&primary.background, None)
            .ok_or_else(|| format!("{name}: colors.primary.background"))?;
        let foreground = resolve(&primary.foreground, None)
            .ok_or_else(|| format!("{name}: colors.primary.foreground"))?;

        let mut ansi = [[0u8; 3]; 16];
        ansi[..8].copy_from_slice(
            &normal.map_or_else(default_normal, |group| expand(&group, foreground)),
        );
        ansi[8..].copy_from_slice(
            &bright.map_or_else(default_bright, |group| expand(&group, foreground)),
        );

        Ok(Self {
            background,
            foreground,
            cursor: inverted_background(cursor, foreground),
            selection_background: inverted_background(selection, foreground),
            ansi,
        })
    }
}

/// alacritty `NormalColors::default`。
fn default_normal() -> [[u8; 3]; 8] {
    [
        [0x18, 0x18, 0x18],
        [0xac, 0x42, 0x42],
        [0x90, 0xa9, 0x59],
        [0xf4, 0xbf, 0x75],
        [0x6a, 0x9f, 0xb5],
        [0xaa, 0x75, 0x9f],
        [0x75, 0xb5, 0xaa],
        [0xd8, 0xd8, 0xd8],
    ]
}

/// alacritty `BrightColors::default`。
fn default_bright() -> [[u8; 3]; 8] {
    [
        [0x6b, 0x6b, 0x6b],
        [0xc5, 0x55, 0x55],
        [0xaa, 0xc4, 0x74],
        [0xfe, 0xca, 0x88],
        [0x82, 0xb8, 0xc8],
        [0xc2, 0x8c, 0xb8],
        [0x93, 0xd3, 0xc3],
        [0xf8, 0xf8, 0xf8],
    ]
}

/// 字面量取十六进制；`CellForeground` 取给定前景色。
///
/// `CellBackground` 与未知色名在此返回 `None`：它们需要单元格实际背景才能定值，
/// 而解析期只有一个静态默认值可依，故交由调用方按上游语义回退。
fn resolve(color: &Color, foreground: Option<[u8; 3]>) -> Option<[u8; 3]> {
    match color {
        Color::Rgb(text) => parse_hex(text),
        Color::Symbolic(name) if name == "CellForeground" => foreground,
        Color::Symbolic(_) => None,
    }
}

fn parse_hex(text: &str) -> Option<[u8; 3]> {
    let digits = text.strip_prefix('#')?;
    match digits.len() {
        6 => {
            let channel = |pair: &str| u8::from_str_radix(pair, 16).ok();
            Some([
                channel(&digits[0..2])?,
                channel(&digits[2..4])?,
                channel(&digits[4..6])?,
            ])
        }
        3 => {
            let channel = |index: usize| {
                let nibble = u8::from_str_radix(&digits[index..index + 1], 16).ok()?;
                Some(nibble << 4 | nibble)
            };
            Some([channel(0)?, channel(1)?, channel(2)?])
        }
        _ => None,
    }
}

/// 色组按 alacritty 的 ANSI 槽位顺序展开；单项不可解析时退回前景色。
fn expand(group: &AnsiGroup, foreground: [u8; 3]) -> [[u8; 3]; 8] {
    let color = |value: &Color| resolve(value, Some(foreground)).unwrap_or(foreground);
    [
        color(&group.black),
        color(&group.red),
        color(&group.green),
        color(&group.yellow),
        color(&group.blue),
        color(&group.magenta),
        color(&group.cyan),
        color(&group.white),
    ]
}

/// `InvertedCellColors` 的底色；缺段回退到前景色（同上游默认值）。
fn inverted_background(inverted: Option<InvertedCellColors>, foreground: [u8; 3]) -> [u8; 3] {
    inverted
        .and_then(|inverted| resolve(&inverted.inverted_background, Some(foreground)))
        .unwrap_or(foreground)
}

/// 一个主题的标识与调色板。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Theme {
    /// 主题标识，即 alacritty 源文件名，也是设置中落盘的键值。
    pub name: &'static str,
    pub palette: Palette,
    /// 深底主题，供「跟随系统」在日夜两组间切换（DESIGN「终端主题」节）。
    pub dark: bool,
}

/// 解析结果只算一次，之后跨线程只读共享。
static THEMES: LazyLock<Vec<Theme>> = LazyLock::new(|| {
    SOURCES
        .iter()
        .map(|(name, text)| {
            let palette = Palette::parse(name, text)
                .unwrap_or_else(|error| panic!("alacritty 主题解析失败: {error}"));
            Theme {
                name,
                dark: is_dark(palette.background),
                palette,
            }
        })
        .collect()
});

/// 相对亮度（Rec. 601 加权）。低于中灰即视为深底。
///
/// 以中灰为界而非更精细的感知模型，是因为清单内各主题背景色与 0.5 相距都远：
/// 最亮的一侧在 0.8 以上、最暗的一侧在 0.15 以下，阈值挪动不改变任何判定。
fn is_dark(color: [u8; 3]) -> bool {
    let [red, green, blue] = color.map(|channel| f32::from(channel) / 255.0);
    0.299 * red + 0.587 * green + 0.114 * blue < 0.5
}

/// 全部主题，深底在前、日间在后。
///
/// 顺序即设置界面的展示顺序：DESIGN 要求跟随系统时日夜两组各自可选，把同组
/// 主题相邻摆放免去调用方再排一次。
pub fn all() -> Vec<&'static Theme> {
    let mut themes: Vec<&'static Theme> = THEMES.iter().collect();
    themes.sort_by_key(|theme| !theme.dark);
    themes
}

fn default(index: usize) -> &'static Theme {
    &THEMES[index]
}

/// 标识对应的 alacritty 源文本。标识即文件名，故取源只需按标识查 [`SOURCES`]。
#[cfg(test)]
fn source_of(name: &str) -> &'static str {
    SOURCES
        .iter()
        .find_map(|(source_name, text)| (*source_name == name).then_some(*text))
        .unwrap_or_else(|| panic!("清单中没有标识 {name}"))
}

/// 默认（夜间）主题标识，对应 DESIGN 指定的默认终端主题。
pub fn default_dark_name() -> &'static str {
    default(DEFAULT_DARK_INDEX).name
}

/// 默认（日间）主题标识。
pub fn default_light_name() -> &'static str {
    default(DEFAULT_LIGHT_INDEX).name
}

/// 按主题标识取主题。未命中返回 `None`：存着无法解析的名字即设置数据损坏，
/// 由调用方清除该键（DESIGN「设置数据错误 → 清除设置数据」），不做静默替换。
pub fn by_name(name: &str) -> Option<&'static Theme> {
    THEMES.iter().find(|theme| theme.name == name)
}

#[cfg(test)]
mod tests;
