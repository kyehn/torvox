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
    /// 主题背景色。
    pub background: RgbColor,
    /// 主题前景色。
    pub foreground: RgbColor,
    /// 光标色。
    pub cursor: RgbColor,
    /// 选区底色（与前景互换的选区反白由渲染层处理，此处是选区自身的底色）。
    pub selection_background: RgbColor,
    /// ANSI 十六色：`colors.normal` 占 0..8、`colors.bright` 占 8..16。
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

    /// 用 `overrides` 逐槽覆盖自身，未覆盖的槽保持原值。
    ///
    /// 光标色是唯一的**派生**槽：背景被覆盖而光标未指定时，光标按背景的感知亮度取
    /// 白色或黑色（上游 termux `TerminalColorScheme.setCursorColorForBackground` 的
    /// 口径）——主题自带的光标色是按主题背景算的，背景换掉后它可能与新背景同色而
    /// 看不见。背景未被覆盖时，主题的光标色原样保留。
    ///
    /// 选区底色没有对应键——`colors.properties` 没有它——原样保留，故覆盖后的选区色
    /// 仍与主题一致。
    #[must_use]
    pub fn with_overrides(&self, overrides: &PaletteOverride) -> Self {
        let background = overrides.background.unwrap_or(self.background);
        let cursor = overrides
            .cursor
            .unwrap_or_else(|| match overrides.background {
                Some(_) => cursor_for_background(background),
                None => self.cursor,
            });
        let mut ansi = self.ansi;
        for (index, color) in overrides.ansi.iter().enumerate() {
            if let Some(color) = color {
                ansi[index] = *color;
            }
        }
        Self {
            background,
            foreground: overrides.foreground.unwrap_or(self.foreground),
            cursor,
            selection_background: self.selection_background,
            ansi,
        }
    }

    /// 深底判定：与 [`Theme::dark`] 同口径（背景的 ghostty 感知亮度不超过 0.5）。
    ///
    /// 明暗上报取的是**合并后**的背景：深色主题被 `colors.properties` 覆盖成浅色
    /// 背景时，须向 PTY 上报日间模式。
    pub fn is_dark(&self) -> bool {
        self.background.perceived_luminance() <= 0.5
    }
}

/// 调色板覆盖的槽位数：背景、前景、光标，随后 16 个 ANSI 色。
///
/// 槽位顺序是跨语言契约，与 Kotlin `ColorsProperties.PALETTE_OVERRIDE_SLOTS` 对齐。
pub const PALETTE_OVERRIDE_SLOTS: usize = 19;

/// 覆盖通道的槽位下标（跨语言契约）。
pub const OVERRIDE_BACKGROUND: usize = 0;
/// 覆盖通道槽位：前景色。
pub const OVERRIDE_FOREGROUND: usize = 1;
/// 覆盖通道槽位：光标色。
pub const OVERRIDE_CURSOR: usize = 2;
/// 覆盖通道槽位：ANSI 十六色的首槽下标。
pub const OVERRIDE_ANSI_FIRST: usize = 3;

/// ANSI 槽数：覆盖通道的 16 个 ANSI 色。
const ANSI_SLOTS: usize = 16;

/// 一份调色板覆盖：每个槽位为 `None` 表示沿用主题原色。
///
/// 来源是 `~/.termux/colors.properties`：它是**主题的部分覆盖**而非完整主题，未写的
/// 槽保持主题值。选区底色没有对应键，故不在覆盖通道里。
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PaletteOverride {
    /// 背景色覆盖；`None` 表示沿用主题原色。
    pub background: Option<RgbColor>,
    /// 前景色覆盖；`None` 表示沿用主题原色。
    pub foreground: Option<RgbColor>,
    /// 光标色覆盖；`None` 表示沿用主题原色。
    pub cursor: Option<RgbColor>,
    /// ANSI 十六色槽 0..15 的覆盖值。
    pub ansi: [Option<RgbColor>; ANSI_SLOTS],
}

impl PaletteOverride {
    /// 从整数槽位数组构造覆盖：负值表示该槽不覆盖，沿用主题原色。
    ///
    /// 长度不符即报错：静默截断会让颜色整体错位一格，而错位表现为「某些颜色不对」，
    /// 无从定位。色值超出 `0xRRGGBB` 同样报错——把它当成颜色会用错的高位切出别的
    /// 颜色，而「颜色不对」比「调用失败」难查得多。
    pub fn from_slots(slots: &[i32]) -> Result<Self, String> {
        if slots.len() != PALETTE_OVERRIDE_SLOTS {
            return Err(format!(
                "期望 {PALETTE_OVERRIDE_SLOTS} 个覆盖槽位，实际 {} 个",
                slots.len()
            ));
        }
        let mut ansi = [None; ANSI_SLOTS];
        for (index, slot) in ansi.iter_mut().enumerate() {
            *slot = slot_color(slots, OVERRIDE_ANSI_FIRST + index)?;
        }
        Ok(Self {
            background: slot_color(slots, OVERRIDE_BACKGROUND)?,
            foreground: slot_color(slots, OVERRIDE_FOREGROUND)?,
            cursor: slot_color(slots, OVERRIDE_CURSOR)?,
            ansi,
        })
    }
}

/// 槽位值 → 颜色：负值表示不覆盖，沿用主题原色。
fn slot_color(slots: &[i32], index: usize) -> Result<Option<RgbColor>, String> {
    let value = slots[index];
    if value < 0 {
        return Ok(None);
    }
    if value > 0x00FF_FFFF {
        return Err(format!("覆盖槽位 {index} 的值 {value} 超出 0xRRGGBB 范围"));
    }
    Ok(Some(RgbColor {
        r: ((value >> 16) & 0xFF) as u8,
        g: ((value >> 8) & 0xFF) as u8,
        b: (value & 0xFF) as u8,
    }))
}

/// 上游 termux `TerminalColors.getPerceivedBrightnessOfColor` 的 HSP 感知亮度，值域 0..255。
///
/// 与 ghostty 的 [`RgbColor::perceived_luminance`] 不是同一个公式：光标自适应用的是
/// termux 的口径，故此处按上游公式另算，不借用上游那个。
fn perceived_brightness(color: RgbColor) -> f64 {
    let red = f64::from(color.r);
    let green = f64::from(color.g);
    let blue = f64::from(color.b);
    (0.241 * red * red + 0.691 * green * green + 0.068 * blue * blue).sqrt()
}

/// 背景被覆盖而光标未指定时的光标色：背景暗取白，背景亮取黑。
///
/// 阈值 130 是上游 `TerminalColorScheme.setCursorColorForBackground` 的分界。
fn cursor_for_background(background: RgbColor) -> RgbColor {
    if perceived_brightness(background) < 130.0 {
        RgbColor {
            r: 0xFF,
            g: 0xFF,
            b: 0xFF,
        }
    } else {
        RgbColor {
            r: 0x00,
            g: 0x00,
            b: 0x00,
        }
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
    /// 主题的调色板。
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
                dark: palette.is_dark(),
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
