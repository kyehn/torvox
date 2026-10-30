//! 终端主题：构建期从 <https://github.com/alacritty/alacritty-theme> 读入 alacritty
//! TOML 源，运行时解析为渲染与 UI 共用的调色板。
//!
//! 仓库不留颜色副本：TOML 源由 `include_str!` 在编译期嵌入，字段按 alacritty
//! `config/color.rs` 的结构声明。色值语法与深浅判定都取自上游 ghostty
//! （`RgbColor::parse` 与 `RgbColor::perceived_luminance`），故本仓不另造颜色算法，
//! 上游改动也不必在此同步。十六色与主色按上游结构**必填**：缺项即解析失败，
//! 不回退到任何手写默认色。
//!
//! 主题标识只在主题清单宏的调用处各出现一次：它既是 alacritty 文件名，也是设置中
//! 存储的名字，故二者不会漂移；默认主题用下标引用而非重复名字面量。

use std::sync::LazyLock;

use libghostty_vt::style::RgbColor;
use serde::{Deserialize, Deserializer, de::Error as _};

// 主题目录：alacritty 主题仓库的浅克隆（由 nix shellHook 准备，见 `flake.nix`）。
//
// 路径须为字面量：`include_str!` 与 `concat!` 不接受标识符参数，故由
// [`theme_sources!`] 在调用处声明，目录在此只写一次。用绝对路径而非相对上跳，
// 因为 `include_str!` 的相对基准是本文件位置，仓库移位即失效。

/// 把主题清单展开为 `(源名, TOML 文本)` 表。
///
/// 宏让主题名在一处书写即可同时充当 `include_str!` 的路径片段与设置中存储的名字；
/// 默认主题用清单下标引用，因此不需要第二处名字面量。展开结果为私有 `SOURCES`。
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

// 主题清单：DESIGN「终端主题」节指定的 13 个，不多不少，标识原样保留。
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
    "github_dark_high_contrast",
    "moonfly",
    "pastel_dark",
}

/// alacritty 的 `CellForeground`：取本主题前景色（反色段的合法特殊色名）。
const CELL_FOREGROUND: &str = "CellForeground";

/// 按上游 ghostty 颜色语法解析色值。失败信息带上出错文本：上游错误只有
/// `invalid value`，而 TOML 只给出行列，两者合起来才指得出是哪个色值坏了。
fn rgb<'de, D: Deserializer<'de>>(deserializer: D) -> Result<RgbColor, D::Error> {
    let text = String::deserialize(deserializer)?;
    RgbColor::parse(&text)
        .map_err(|_| D::Error::custom(format!("{text:?} 不是 ghostty 颜色语法的色值")))
}

/// 一套主题的全部颜色。
///
/// ANSI 十六色按 alacritty 槽位顺序展开：`colors.normal` 占 0..8、`colors.bright`
/// 占 8..16。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Palette {
    pub background: RgbColor,
    pub foreground: RgbColor,
    pub cursor: RgbColor,
    pub selection_background: RgbColor,
    pub ansi: [RgbColor; 16],
}

/// alacritty `Colors::primary`。
#[derive(Deserialize)]
struct Primary {
    #[serde(deserialize_with = "rgb")]
    background: RgbColor,
    #[serde(deserialize_with = "rgb")]
    foreground: RgbColor,
}

/// ANSI 十六色的一个色组（`colors.normal` 或 `colors.bright`）。
#[derive(Deserialize)]
struct AnsiGroup {
    #[serde(deserialize_with = "rgb")]
    black: RgbColor,
    #[serde(deserialize_with = "rgb")]
    red: RgbColor,
    #[serde(deserialize_with = "rgb")]
    green: RgbColor,
    #[serde(deserialize_with = "rgb")]
    yellow: RgbColor,
    #[serde(deserialize_with = "rgb")]
    blue: RgbColor,
    #[serde(deserialize_with = "rgb")]
    magenta: RgbColor,
    #[serde(deserialize_with = "rgb")]
    cyan: RgbColor,
    #[serde(deserialize_with = "rgb")]
    white: RgbColor,
}

impl AnsiGroup {
    /// 按 alacritty 的槽位顺序展开：字段顺序即 ANSI 序号（`normal` 在前、
    /// `bright` 在后占 8..16）。
    fn into_colors(self) -> [RgbColor; 8] {
        [
            self.black,
            self.red,
            self.green,
            self.yellow,
            self.blue,
            self.magenta,
            self.cyan,
            self.white,
        ]
    }
}

/// `colors.cursor` 与 `colors.selection` 的源结构：两者在 alacritty 中都是
/// `InvertedCellColors`，故共用同一形状。该类型的背景色字段在 `[colors.cursor]`
/// 段写作别名 `cursor`、在 `[colors.selection]` 段写作别名 `background`
/// （见上游 `InvertedCellColors` 的 `#[config(alias)]`），`#[serde(alias)]` 同时
/// 接受两者。其前景色字段（`text` / `foreground`）只影响选区文字，本应用以反色
/// 绘制选区、不消费该值，故不声明。色值不走上面的 `rgb`：该段允许写
/// `CellForeground`，且缺段时按上游 `InvertedCellColors::default` 取前景色。
#[derive(Deserialize)]
struct InvertedCellColors {
    #[serde(alias = "background", alias = "cursor")]
    inverted_background: String,
}

#[derive(Deserialize)]
struct Colors {
    primary: Primary,
    normal: AnsiGroup,
    bright: AnsiGroup,
    cursor: Option<InvertedCellColors>,
    selection: Option<InvertedCellColors>,
}

#[derive(Deserialize)]
struct ThemeFile {
    colors: Colors,
}

impl Palette {
    /// 解析一套 alacritty 主题源。
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
        let foreground = primary.foreground;
        let mut ansi = [RgbColor::default(); 16];
        ansi[..8].copy_from_slice(&normal.into_colors());
        ansi[8..].copy_from_slice(&bright.into_colors());
        Ok(Self {
            background: primary.background,
            foreground,
            cursor: inverted_background(cursor, foreground)?,
            selection_background: inverted_background(selection, foreground)?,
            ansi,
        })
    }
}

/// `InvertedCellColors` 的底色。缺段回退到前景色（同上游默认值）；
/// `CellForeground` 取前景色，其余按上游颜色语法解析。
fn inverted_background(
    inverted: Option<InvertedCellColors>,
    foreground: RgbColor,
) -> Result<RgbColor, String> {
    let Some(InvertedCellColors {
        inverted_background: text,
    }) = inverted
    else {
        return Ok(foreground);
    };
    if text == CELL_FOREGROUND {
        return Ok(foreground);
    }
    RgbColor::parse(&text).map_err(|error| format!("{text}: {error}"))
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

/// 解析与排序只算一次，之后跨线程只读共享。
///
/// 排序在初始化时完成，故 [`all`] 返回切片而非每次调用的新 `Vec`。
static THEMES: LazyLock<Vec<Theme>> = LazyLock::new(|| {
    let mut themes: Vec<Theme> = SOURCES
        .iter()
        .map(|(name, text)| {
            let palette = Palette::parse(name, text)
                .unwrap_or_else(|error| panic!("alacritty 主题解析失败: {error}"));
            Theme {
                name,
                // 上游文档：背景亮度超过 0.5 即视为浅色，故此处取其补。
                dark: palette.background.perceived_luminance() <= 0.5,
                palette,
            }
        })
        .collect();
    themes.sort_by_key(|theme| !theme.dark);
    themes
});

/// 全部主题，深底在前、日间在后。
///
/// 顺序即设置界面的展示顺序：DESIGN 要求跟随系统时日夜两组各自可选，把同组
/// 主题相邻摆放免去调用方再排一次。
pub fn all() -> &'static [Theme] {
    &THEMES
}

/// 默认（夜间）主题，对应 DESIGN 指定的默认终端主题。
///
/// 会话初态、渲染首帧清除色与设置默认值都取这里，故不存在第二份默认主题声明。
pub fn default_dark() -> &'static Theme {
    by_name(SOURCES[DEFAULT_DARK_INDEX].0).expect("默认主题须在清单中")
}

/// 默认（日间）主题标识。
pub fn default_light_name() -> &'static str {
    SOURCES[DEFAULT_LIGHT_INDEX].0
}

/// 默认（日间）主题。
///
/// 与 [`default_dark`] 对称：跟随系统日间的会话初态取这里，避免调用方
/// 「先取名字再 `by_name`」时各自重复处理查不到的分支。
pub fn default_light() -> &'static Theme {
    by_name(default_light_name()).expect("默认日间主题须在清单中")
}

/// 主题目录一次查询的跨语言打包字节（小端）。
///
/// 取代「逐个标识取调色板」的往返：设置页一次显示全部主题，逐个取要做
/// `1 + N` 次同步 JNI，而目录在构建期即固定、进程内不变。
///
/// ```text
/// u16  主题数，与 [`all`] 同序
/// 每条：
///   u8  标志：bit0 默认深色；bit1 默认浅色
///   u8  名称字节数
///   名称 UTF-8
///   54 字节颜色：背景、前景各 3，十六色 48
/// ```
///
/// 光标色与选区色不入包：终端页的光标与选区由渲染器按同一份调色板绘制，
/// 设置页预览只用背景、前景与十六色，码进包里只会留下无人读取的字段。
pub fn pack_catalog() -> Vec<u8> {
    const COLOR_BYTES: usize = 3;
    /// 背景、前景与十六色，即 Kotlin 侧真正消费的全部颜色。
    const ENTRY_COLOR_BYTES: usize = 18 * COLOR_BYTES;
    // 深底/日间分组由 [THEMES] 的排序表达，不占标志位：打包出去无人读。
    const FLAG_DEFAULT_DARK: u8 = 1 << 0;
    const FLAG_DEFAULT_LIGHT: u8 = 1 << 1;

    let default_dark_name = default_dark().name;
    let default_light_name = default_light_name();
    let mut packed = Vec::with_capacity(2 + THEMES.len() * (2 + ENTRY_COLOR_BYTES));
    packed.extend_from_slice(&(THEMES.len() as u16).to_le_bytes());
    for theme in THEMES.iter() {
        let mut flags = 0;
        if theme.name == default_dark_name {
            flags |= FLAG_DEFAULT_DARK;
        }
        if theme.name == default_light_name {
            flags |= FLAG_DEFAULT_LIGHT;
        }
        packed.push(flags);
        let name = theme.name.as_bytes();
        packed.push(name.len() as u8);
        packed.extend_from_slice(name);
        for color in [theme.palette.background, theme.palette.foreground]
            .into_iter()
            .chain(theme.palette.ansi)
        {
            packed.extend_from_slice(&[color.r, color.g, color.b]);
        }
    }
    packed
}

/// 按主题标识取主题。未命中返回 `None`：存着无法解析的名字即设置数据损坏，
/// 由调用方清除该键（DESIGN「设置数据错误 → 清除设置数据」），不做静默替换。
pub fn by_name(name: &str) -> Option<&'static Theme> {
    THEMES.iter().find(|theme| theme.name == name)
}

#[cfg(test)]
mod tests;
