pub mod atlas;
pub mod cjk;
pub mod font_db;
pub mod glyph_cache;
pub mod pipeline;
pub mod rasterization;
pub mod shaping;

pub const GLYPH_CACHE_CAPACITY: usize = 10_000;

/// 整形跨度缓存容量：整簇字形按 (文本, 字号, 面) 缓存，一屏通常远少于此。
pub(crate) const SHAPE_CACHE_CAPACITY: usize = 1024;

/// 同族样式面解析缓存容量：键为 (字体, 粗, 斜) 三元组，组合数天然很小。
pub(crate) const STYLE_FACE_CACHE_CAPACITY: usize = 64;

/// 样式面字形号缓存容量：键为 (面, 码位)，规模随**屏幕上出现过的不同字符**增长，
/// 与上一项的三元组不同源。一屏 40×20 就有 800 个不同码位，64 条会被立刻穿透，
/// 而每次穿透都要重入 `with_face_data` 并重建 charmap（字体解压）。
pub(crate) const STYLE_GLYPH_ID_CACHE_CAPACITY: usize = 4_096;

/// 轮廓来源探测缓存容量：键带光栅尺寸，覆盖常用字号×字形组合。
pub(crate) const OUTLINE_CACHE_CAPACITY: usize = 10_000;

/// 「无面能解析此码位」负缓存容量：设备不装的罕见汉字只以两种形态出现——整段输出
/// 或 `cat` 出来，数量有界，故容量取小值即可（它只在命中时省下一次全库扫描）。
pub(crate) const UNRESOLVABLE_CACHE_CAPACITY: usize = 512;

/// CJK 表意文字的起始码位（U+2E80），用于判定是否尝试 CJK 回退字体查询。
pub(crate) const CJK_IDEOGRAPHIC_START: u32 = 0x2E80;

/// Nerd Font 私用区（U+E000–U+F8FF）：这些码位只在加载 Nerd Font 后才有字形，
/// 查字形缓存前必须跳过，否则会命中主字体写入的 .notdef（豆腐块）。
pub(crate) const NERD_FONT_PRIVATE_USE_START: u32 = 0xE000;
pub(crate) const NERD_FONT_PRIVATE_USE_END: u32 = 0xF8FF;

/// ASCII 上界（不含）：`ascii_glyph_ids` 定长表按下标直查，表长即此值。
pub(crate) const ASCII_UPPER_BOUND: u32 = 0x80;

/// 字形合成模式：字体缺少匹配的粗/斜体字面时如何呈现字形。
/// 在光栅化后的 alpha 遮罩上后处理像素——粗体加粗，斜体剪切。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum GlyphSynthesis {
    #[default]
    None,
    Bold,
    Italic,
    BoldItalic,
}

impl GlyphSynthesis {
    /// 打包进字形缓存键的位值（3 位足够）。
    pub(crate) fn bits(self) -> u8 {
        match self {
            GlyphSynthesis::None => 0,
            GlyphSynthesis::Bold => 1,
            GlyphSynthesis::Italic => 2,
            GlyphSynthesis::BoldItalic => 3,
        }
    }

    /// [`GlyphSynthesis::bits`] 的逆运算：从缓存键还原合成模式。
    /// 未知位模式退回无合成，而不是臆造样式。
    pub(crate) fn from_bits(bits: u8) -> Self {
        match bits {
            1 => GlyphSynthesis::Bold,
            2 => GlyphSynthesis::Italic,
            3 => GlyphSynthesis::BoldItalic,
            _ => GlyphSynthesis::None,
        }
    }
}

/// 可变字体 `wght` 轴的目标取值（与 CSS `font-weight: 700` 同义）。
const VARIATION_WEIGHT_BOLD: f32 = 700.0;
/// 可变字体 `ital` 轴的目标取值（0 = upright，1 = italic）。
const VARIATION_ITALIC_ON: f32 = 1.0;

/// 由 `synthesis` 与字体自身声明的轴，产出要写入 swash 的轴设置。
///
/// fonts.xml 的 39 个 VF 字体（Roboto、NotoSans*-VF、MapleMono NF CN 等）把字重与
/// 倾斜表达为 `wght`/`ital` 轴而非独立文件，而 fontdb 对 VF 只登记默认实例，
/// 其余取值必须显式告知 swash，否则拿到的永远是 Regular 轮廓。
/// 字体没声明该轴时返回空，调用方退回轮廓级加粗/剪切。
///
/// 取值一律 clamp 到字体声明的轴范围：部分 VF 只声明 `wght 100..400`，
/// 越界会经 `avar` 映射到非预期位置。
pub(super) fn variation_settings(
    font_ref: &swash::FontRef<'_>,
    synthesis: GlyphSynthesis,
) -> Vec<swash::Setting<f32>> {
    let mut settings = Vec::new();
    for (requested, tag_bytes, wanted) in [
        (
            matches!(synthesis, GlyphSynthesis::Bold | GlyphSynthesis::BoldItalic),
            b"wght",
            VARIATION_WEIGHT_BOLD,
        ),
        (
            matches!(
                synthesis,
                GlyphSynthesis::Italic | GlyphSynthesis::BoldItalic
            ),
            b"ital",
            VARIATION_ITALIC_ON,
        ),
    ] {
        if !requested {
            continue;
        }
        let tag = swash::tag_from_bytes(tag_bytes);
        // 字体没声明该轴（非 VF）时留空，调用方退回轮廓级加粗/剪切。
        let Some(axis) = font_ref.variations().find(|axis| axis.tag() == tag) else {
            continue;
        };
        settings.push(swash::Setting {
            tag,
            value: wanted.clamp(axis.min_value(), axis.max_value()),
        });
    }
    settings
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct GlyphKey {
    pub font_id: fontdb::ID,
    pub glyph_id: u16,
    /// 光栅尺寸的精确位模式：不同子像素尺寸必须命中不同缓存条目
    /// （此前 `as u16` 截断使 36.75px 与 36.22px 共用同一键，
    /// 缩放后取到错误尺寸位图，表现为模糊/字形不对）。
    pub raster_size_bits: u32,
    /// 样式：光栅化时的合成模式（0 = none）。可变字体走轴设置时同样按此值
    /// 区分缓存条目——同一字形号在不同 `wght`/`ital` 下轮廓不同。
    pub synthesis: u8,
}

/// 由光栅尺寸派生缓存键分量：精确区分，不截断。
pub(crate) fn raster_size_key(raster_size: f32) -> u32 {
    raster_size.to_bits()
}

#[derive(Debug, Clone)]
pub struct GlyphInfo {
    pub atlas_x: i32,
    pub atlas_y: i32,
    pub width: i32,
    pub height: i32,
    pub placement: swash::zeno::Placement,
    pub allocation_id: Option<guillotiere::AllocId>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ShapedGlyphInfo {
    pub glyph_id: u16,
    pub font_id: fontdb::ID,
    pub x: f32,
    pub w: f32,
    pub x_offset: f32,
    pub y_offset: f32,
}

pub(crate) use atlas::ATLAS_BYTES_PER_PIXEL;
#[cfg(target_os = "android")]
pub use font_db::{add_extra_font_path, set_current_locale, set_extra_font_paths};
pub use pipeline::FontPipeline;
pub(crate) use pipeline::OverlayQuad;

#[cfg(test)]
mod tests;
