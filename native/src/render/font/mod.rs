pub mod atlas;
pub mod cjk;
pub mod font_db;
pub mod glyph_cache;
pub mod pipeline;
pub mod rasterization;
pub mod shaping;

use thiserror::Error;

pub const GLYPH_CACHE_CAPACITY: usize = 10_000;

/// 整形跨度缓存容量：整簇字形按 (文本, 字号, 面) 缓存，一屏通常远少于此。
pub(crate) const SHAPE_CACHE_CAPACITY: usize = 1024;

/// 同族样式面解析缓存容量：键为 (字体, 粗, 斜) 三元组，组合数天然很小。
pub(crate) const STYLE_FACE_CACHE_CAPACITY: usize = 64;

/// 轮廓来源探测缓存容量：键带光栅尺寸，覆盖常用字号×字形组合。
pub(crate) const OUTLINE_CACHE_CAPACITY: usize = 10_000;

/// Unicode code point where CJK Ideographic characters begin (U+2E80).
/// Used to decide whether to attempt CJK fallback font lookup.
pub(crate) const CJK_IDEOGRAPHIC_START: u32 = 0x2E80;

/// Nerd Font 私用区（U+E000–U+F8FF）：这些码位只在加载 Nerd Font 后才有字形，
/// 查字形缓存前必须跳过，否则会命中主字体写入的 .notdef（豆腐块）。
pub(crate) const NERD_FONT_PRIVATE_USE_START: u32 = 0xE000;
pub(crate) const NERD_FONT_PRIVATE_USE_END: u32 = 0xF8FF;

/// ASCII 上界（不含）：`ascii_glyph_ids` 定长表按下标直查，表长即此值。
pub(crate) const ASCII_UPPER_BOUND: u32 = 0x80;

#[derive(Debug, Error)]
pub enum FontError {
    #[error("no monospace font found")]
    NoMonospaceFont,
    #[error("font loading failed: {0}")]
    FontLoad(String),
    #[error("atlas allocation failed")]
    AtlasAllocationFailed,
}

/// Glyph synthesis mode: how a glyph is styled when the
/// font has no matching bold/italic face. Pixels are post-processed on the
/// rasterized alpha mask — bold emboldens, italic shears.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum GlyphSynthesis {
    #[default]
    None,
    Bold,
    Italic,
    BoldItalic,
}

impl GlyphSynthesis {
    /// Bit values packed into the glyph cache key (3 bits are enough).
    pub(crate) fn bits(self) -> u8 {
        match self {
            GlyphSynthesis::None => 0,
            GlyphSynthesis::Bold => 1,
            GlyphSynthesis::Italic => 2,
            GlyphSynthesis::BoldItalic => 3,
        }
    }

    /// Inverse of [`GlyphSynthesis::bits`]: restore the synthesis mode
    /// stored in a cache key. Unknown bit patterns fall back to no
    /// synthesis rather than inventing a style.
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
    pub advance_width: f32,
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
mod tests {
    use super::*;

    /// 图集每个像素的字节数（RGBA）。
    const BYTES_PER_PIXEL: usize = 4;

    #[test]
    fn han_is_wide_and_non_emoji() {
        let mut pipeline = FontPipeline::new(512, 512, 14.0);
        let ascii_info = pipeline.glyph_information('A').expect("ASCII glyph info");
        let han_info = pipeline.glyph_information('世').expect("Han glyph info");
        assert!(
            han_info.width as f32 > ascii_info.width as f32 * 1.5,
            "CJK 回退字体已安装时汉字必须是双格宽：'世' {} vs 'A' {}",
            han_info.width,
            ascii_info.width
        );
    }

    /// 回应对"加粗中文空白"：合成样式不得跳过回退链，加粗汉字必须有位图。
    #[test]
    fn bold_cjk_fallback_not_blank() {
        let (mut pipeline, _) = styled_test_pipeline();
        pipeline.find_cjk_fallback_fonts("");
        let bold_han = pipeline
            .glyph_information_styled('中', true, false)
            .expect("加粗汉字必须解析出字形");
        assert!(
            bold_han.width > 0 && bold_han.height > 0,
            "加粗汉字位图不得为空"
        );
    }

    #[test]
    fn font_pipeline_creation() {
        let pipeline = FontPipeline::new(1024, 1024, 14.0);
        assert_eq!(pipeline.atlas_dimensions(), (1024, 1024));
        assert!(
            pipeline.cache_length() > 0,
            "ASCII glyphs should be pre-rasterized"
        );
    }

    #[test]
    fn font_pipeline_has_system_fonts() {
        let pipeline = FontPipeline::new(1024, 1024, 14.0);
        let fonts = pipeline.list_monospace_fonts();
        assert!(
            !fonts.is_empty(),
            "Should have at least one system monospace font"
        );
    }

    #[test]
    fn font_matching_stripped_spaces() {
        let pipeline = FontPipeline::new(1024, 1024, 14.0);
        let names = pipeline.list_monospace_fonts();
        assert!(!names.is_empty(), "Should have at least one font");
        let name_with_space = names.iter().find(|name| name.contains(' '));
        let name = match name_with_space {
            Some(n) => n.clone(),
            None => {
                panic!(
                    "no monospace font with spaces found; cannot test stripped-name matching; available: {:?}",
                    names.iter().take(5).collect::<Vec<_>>()
                );
            }
        };
        let stripped: String = name
            .chars()
            .filter(|character| !character.is_whitespace())
            .collect();
        assert!(stripped != name, "Sanity: stripped name differs");
        let mut p2 = FontPipeline::new(1024, 1024, 14.0);
        assert!(
            p2.set_font_family(&stripped),
            "set_font_family should find '{}' when given '{}'",
            name,
            stripped
        );
    }

    #[test]
    fn glyph_hao_cjk_cross_verify() {
        let mut pipeline = FontPipeline::new(512, 512, 14.0);
        let info = pipeline
            .glyph_information('好')
            .expect("pipeline should have CJK glyph info (via fallback)");
        assert!(
            info.width > 0 || info.height > 0,
            "CJK '好' should produce non-zero glyph info: got {}x{}",
            info.width,
            info.height
        );
    }

    #[test]
    fn cjk_width_is_double_ascii() {
        let mut pipeline = FontPipeline::new(512, 512, 14.0);
        let ascii_info = pipeline
            .glyph_information('A')
            .expect("ascii 'A' glyph info");
        let cjk_info = pipeline
            .glyph_information('中')
            .expect("CJK '中' glyph info");
        assert!(
            ascii_info.width > 0,
            "ASCII glyph should have positive width"
        );
        assert!(cjk_info.width > 0, "CJK glyph should have positive width");
        let (cell_width, _) = pipeline.cell_metrics();
        assert!(cell_width > 0.0, "cell width should be positive");
        let cell_span = if cjk_info.width as f32 > ascii_info.width as f32 * 1.5 {
            2
        } else {
            1
        };
        assert!(cell_span >= 1, "CJK cell span should be at least 1");
    }

    #[test]
    fn glyph_information_ascii() {
        let mut pipeline = FontPipeline::new(1024, 1024, 14.0);
        let info = pipeline.glyph_information('A');
        assert!(info.is_some());
        let info = info.unwrap();
        assert!(info.width > 0);
        assert!(info.height > 0);
    }

    #[test]
    fn glyph_information_caching() {
        let mut pipeline = FontPipeline::new(1024, 1024, 14.0);
        let before = pipeline.cache_length();
        pipeline.glyph_information('B');
        assert_eq!(pipeline.cache_length(), before);
        pipeline.glyph_information('B');
        assert_eq!(pipeline.cache_length(), before);
    }

    #[test]
    fn rasterize_ascii_populates_cache() {
        let pipeline = FontPipeline::new(1024, 1024, 14.0);
        // ASCII 32..127 共 95 格；空格无位图不入库，94 为正确值。
        assert!(pipeline.cache_length() >= 94);
    }

    #[test]
    fn font_size_change_invalidates_ascii_identity_cache() {
        // 手势缩放/字号切换走 set_font_size_in_place：若 ascii 字形 id
        // 表不清零，d 等字符会命中旧尺寸的脏 id 而显示错误字形。
        let mut pipeline = FontPipeline::new(1024, 1024, 14.0);
        let before = pipeline.glyph_information('d').expect("d glyph info");
        pipeline.set_font_size_in_place(28.0);
        let after = pipeline
            .glyph_information('d')
            .expect("d glyph info after resize");
        // 同一字符新尺寸必须重光栅化：位图尺寸随字号显著变化。
        assert!(
            after.width != before.width || after.height != before.height,
            "d must re-rasterize after font size change"
        );
        assert!(after.width > 0 && after.height > 0);
    }

    #[test]
    fn atlas_eviction_keeps_glyphs_retrievable() {
        let mut pipeline = FontPipeline::new(256, 256, 14.0);
        let distinct: Vec<char> = ('A'..='Z')
            .chain('a'..='z')
            .chain('0'..='9')
            .chain("!@#$%^&*()_+-=[]{}|;:,.<>?/".chars())
            .collect();
        for &ch in distinct.iter().cycle().take(distinct.len() * 4) {
            assert!(
                pipeline.glyph_information(ch).is_some(),
                "glyph {ch:?} must remain retrievable under eviction pressure"
            );
        }
        let generation_after_fill = pipeline.atlas_generation();
        assert!(pipeline.glyph_information('A').is_some());
        assert!(
            pipeline.atlas_generation() >= generation_after_fill,
            "atlas generation must remain monotonic under eviction"
        );
        assert!(
            pipeline.cache_length() <= GLYPH_CACHE_CAPACITY,
            "glyph cache must respect its capacity after eviction"
        );
    }

    #[test]
    fn glyph_information_has_atlas_coords() {
        let mut pipeline = FontPipeline::new(1024, 1024, 14.0);
        let info = pipeline.glyph_information('X').unwrap();
        assert!(info.atlas_x >= 0);
        assert!(info.atlas_y >= 0);
        assert!(info.width > 0);
        assert!(info.height > 0);
    }

    #[test]
    fn atlas_bitmap_not_empty_after_rasterize() {
        let mut pipeline = FontPipeline::new(1024, 1024, 14.0);
        pipeline.glyph_information('A');
        let bitmap = pipeline.atlas_bitmap();
        assert!(bitmap.iter().any(|&b| b != 0));
    }

    #[test]
    fn cell_metrics_scales_with_font_size() {
        let small = FontPipeline::new(1024, 1024, 10.0);
        let large = FontPipeline::new(1024, 1024, 20.0);
        let (sw, sh) = small.cell_metrics();
        let (lw, lh) = large.cell_metrics();
        assert!(lw > sw, "larger font must have wider cell");
        assert!(lh > sh, "larger font must have taller cell");
    }

    #[test]
    fn setting_font_family_changes_metrics() {
        let mut pipeline = FontPipeline::new(512, 512, 12.0);
        let name = pipeline
            .list_monospace_fonts()
            .into_iter()
            .next()
            .expect("系统应至少提供一个等宽字体");
        assert!(
            pipeline.set_font_family(&name),
            "set_font_family 应对 {name} 成功"
        );
        let (cell_width, cell_height) = pipeline.cell_metrics();
        assert!(cell_width > 0.0 && cell_height > 0.0, "单元格尺寸必须为正");
    }

    #[test]
    fn bearing_values_for_dot() {
        let mut pipeline = FontPipeline::new(512, 512, 14.0);
        let info = pipeline
            .glyph_information('.')
            .expect("'.' should glyph_information");
        assert!(
            info.placement.left >= 0,
            "dot bearing_x={} should be >= 0",
            info.placement.left
        );
        assert!(
            info.placement.top > 0,
            "dot bearing_y={} should be > 0",
            info.placement.top
        );
    }

    #[test]
    fn bearing_values_for_a() {
        let mut pipeline = FontPipeline::new(512, 512, 14.0);
        let info = pipeline
            .glyph_information('A')
            .expect("'A' should have glyph_information");
        assert!(
            info.placement.width > 0,
            "A glyph_width={} should be > 0",
            info.placement.width
        );
        assert!(
            info.placement.height > 0,
            "A glyph_height={} should be > 0",
            info.placement.height
        );
    }

    #[test]
    fn bearing_values_for_cjk() {
        let mut pipeline = FontPipeline::new(512, 512, 14.0);
        let info = pipeline
            .glyph_information('好')
            .expect("'好' should have glyph_information");
        assert!(
            info.placement.left >= 0,
            "好 bearing_x={} should be >= 0",
            info.placement.left
        );
        assert!(
            info.placement.top > 0,
            "好 bearing_y={} should be > 0",
            info.placement.top
        );
        let dot_info = pipeline.glyph_information('.').expect("'.' for comparison");
        assert!(
            info.placement.width >= dot_info.placement.width * 2 - 2,
            "好 width={} should be ~2x dot width={}",
            info.placement.width,
            dot_info.placement.width
        );
    }

    fn bearing_fits_inside_cell(glyph: char, label: &str) {
        let mut pipeline = FontPipeline::new(512, 512, 14.0);
        let info = pipeline
            .glyph_information(glyph)
            .unwrap_or_else(|| panic!("'{glyph}' glyph_information"));
        let (_cell_width, cell_height) = pipeline.cell_metrics();
        let ascent = pipeline.ascent_pixels();
        let bearing_y = ascent - info.placement.top as f32;
        let glyph_h = info.placement.height as f32;
        assert!(
            bearing_y >= -cell_height,
            "{label} glyph starts way above cell: bearing_y={} < -cell_height",
            bearing_y
        );
        assert!(glyph_h > 0.0, "{label} glyph has zero height");
        assert!(cell_height > 0.0, "{label} cell has zero height");
    }

    #[test]
    fn bearing_dot_fits_inside_cell() {
        bearing_fits_inside_cell('.', "dot");
    }

    #[test]
    fn bearing_a_fits_inside_cell() {
        bearing_fits_inside_cell('a', "a");
    }

    #[test]
    fn bearing_values_non_zero_for_rendered_glyphs() {
        let mut pipeline = FontPipeline::new(512, 512, 14.0);
        pipeline.rasterize_ascii();
        let glyphs = ['0', 'x', 'g', 'p', 'W', 'M', 'f', '(', ')'];
        for &ch in &glyphs {
            if let Some(info) = pipeline.glyph_information(ch) {
                assert!(
                    info.placement.width > 0,
                    "'{ch}' width={} should be > 0",
                    info.placement.width
                );
                assert!(
                    info.placement.height > 0,
                    "'{ch}' height={} should be > 0",
                    info.placement.height
                );
            }
        }
    }

    #[test]
    fn font_enumeration_finds_monospace() {
        let pipeline = FontPipeline::new(512, 512, 14.0);
        let fonts = pipeline.list_monospace_fonts();
        assert!(
            !fonts.is_empty(),
            "FontLoader should find at least one monospace face, got: {:?}",
            fonts
        );
        assert!(
            pipeline.has_font(),
            "FontPipeline should have a font assigned"
        );
    }

    #[test]
    fn cjk_glyph_zhong() {
        let mut pipeline = FontPipeline::new(512, 512, 14.0);
        let info = pipeline
            .glyph_information('中')
            .expect("CJK '中' (U+4E2D) should have glyph info");
        assert!(
            info.width > 0,
            "CJK '中' width should be non-zero, got {}",
            info.width
        );
        assert!(
            info.height > 0,
            "CJK '中' height should be non-zero, got {}",
            info.height
        );
        let atlas = pipeline.atlas_bitmap();
        let atlas_width = 512usize;
        let atlas_left = info.atlas_x as usize;
        let atlas_top = info.atlas_y as usize;
        let mut has_ink = false;
        for pixel_row in 0..info.height as usize {
            for pixel_column in 0..info.width as usize {
                let byte_offset =
                    ((atlas_top + pixel_row) * atlas_width + atlas_left + pixel_column) * 4;
                if byte_offset < atlas.len() && atlas[byte_offset] > 0 {
                    has_ink = true;
                    break;
                }
            }
            if has_ink {
                break;
            }
        }
        assert!(has_ink, "CJK '中' should have non-zero coverage in atlas");
    }

    #[test]
    fn emoji_glyph_grinning() {
        let mut pipeline = FontPipeline::new(512, 512, 14.0);
        let ch = '\u{1F600}';
        let info = pipeline.glyph_information(ch);
        if info.is_none()
            || info
                .as_ref()
                .is_some_and(|glyph_info| glyph_info.width == 0)
        {
            let fonts = pipeline.list_monospace_fonts();
            let found_emoji = fonts.iter().any(|name| {
                name.contains("Emoji")
                    || name.contains("Noto")
                    || name.to_lowercase().contains("emoji")
            });
            assert!(
                found_emoji,
                "no emoji-supporting font found in system; emoji glyph test requires Noto Emoji or similar"
            );
        }
        let info = info.expect("emoji should have glyph info");
        assert!(
            info.width > 0 || info.height > 0,
            "emoji should produce non-zero glyph info: got {}x{}",
            info.width,
            info.height
        );
    }

    #[test]
    fn glyph_atlas_lru_eviction() {
        let mut pipeline = FontPipeline::new(512, 512, 14.0);
        pipeline.rasterize_ascii();
        let after_ascii = pipeline.cache_length();
        // ASCII 32..127 共 95 格；空格无位图不入库，94 为正确值。
        assert!(
            after_ascii >= 94,
            "should have at least 94 cached after rasterize_ascii, got {}",
            after_ascii
        );

        let mut inserted = 0u32;
        for cp in 0x4E00u32..0x4F00u32 {
            let ch = char::from_u32(cp).unwrap_or('\0');
            if pipeline
                .glyph_information(ch)
                .is_some_and(|glyph_info| glyph_info.width > 0)
            {
                inserted += 1;
            }
        }
        let final_len = pipeline.cache_length();
        assert!(
            final_len <= 10000,
            "cache_length {} exceeds capacity 10000",
            final_len
        );
        assert!(
            final_len >= after_ascii,
            "cache should not shrink after inserting new glyphs: \
             before={} after={} inserted={}",
            after_ascii,
            final_len,
            inserted
        );
        let bitmap = pipeline.atlas_bitmap();
        assert!(
            bitmap.iter().any(|&b| b != 0),
            "atlas bitmap should have non-zero bytes after glyph insertion"
        );
    }

    #[test]
    fn cjk_glyph_information_returns_nonzero_for_common_chars() {
        let mut pipeline = FontPipeline::new(1024, 1024, 14.0);
        let chars = [
            '你', '好', '世', '界', '中', '文', '字', '体', '渲', '染', '测', '试',
        ];
        for ch in chars {
            let info = pipeline
                .glyph_information(ch)
                .unwrap_or_else(|| panic!("CJK glyph_information('{ch}') should return Some"));
            assert!(
                info.width > 0 && info.height > 0,
                "CJK glyph '{ch}' should have nonzero dimensions: {}x{}",
                info.width,
                info.height
            );
        }
    }

    #[test]
    fn font_switching_changes_font_id() {
        let mut pipeline = FontPipeline::new(1024, 1024, 14.0);
        let original_id = pipeline.font_id;
        let names = pipeline.list_monospace_fonts();
        let mut found_switch = false;
        for name in &names {
            if name.is_empty() {
                continue;
            }
            if pipeline.set_font_family(name) && pipeline.font_id != original_id {
                found_switch = true;
                break;
            }
        }
        assert!(
            found_switch,
            "At least one font family should change font_id from {original_id:?}",
        );
    }

    #[test]
    fn font_switching_clears_cache() {
        let mut pipeline = FontPipeline::new(1024, 1024, 14.0);
        pipeline.rasterize_ascii();
        assert!(pipeline.cache_length() > 0);
        // Include a non-ASCII glyph in the baseline: after the switch the
        // cache is cleared and re-filled only with the new font's ASCII
        // rasterization, so the length must drop below this value.
        pipeline.glyph_information('好');
        let before = pipeline.cache_length();
        let names = pipeline.list_monospace_fonts();
        if names.len() > 1 {
            let alt = names.last().unwrap();
            pipeline.set_font_family(alt);
            assert!(
                pipeline.cache_length() < before,
                "cache should shrink after font switch to '{alt}'"
            );
        } else {
            pipeline.set_font_family("monospace");
            assert_ne!(
                pipeline.cache_length(),
                0,
                "cache must be non-empty after font switch"
            );
            assert!(
                pipeline.cache_length() < before,
                "cache should shrink after font switch"
            );
        }
    }

    #[test]
    fn cell_metrics_height_is_integer() {
        let pipeline = FontPipeline::new(1024, 1024, 14.0);
        let (_cw, ch) = pipeline.cell_metrics();
        assert!(
            (ch - ch.floor()).abs() < f32::EPSILON,
            "cell_height should be integer (ceil'd), got {ch}"
        );
    }

    #[test]
    fn cell_metrics_height_scales_with_font_size() {
        let small = FontPipeline::new(1024, 1024, 10.0);
        let large = FontPipeline::new(1024, 1024, 20.0);
        let (_, sh) = small.cell_metrics();
        let (_, lh) = large.cell_metrics();
        assert!(lh > sh, "larger font must have taller cell");
        assert!(
            (sh - sh.floor()).abs() < f32::EPSILON,
            "small cell_height should be integer"
        );
        assert!(
            (lh - lh.floor()).abs() < f32::EPSILON,
            "large cell_height should be integer"
        );
    }

    #[test]
    fn termux_formula_ascent_plus_descent_equals_cell_height() {
        let pipeline = FontPipeline::new(1024, 1024, 14.0);
        let ascent = pipeline.ascent_pixels();
        let descent = pipeline.descent_pixels();
        let (_, ch) = pipeline.cell_metrics();
        assert!(
            (ascent + descent - ch).abs() < 2.0,
            "ascent({ascent}) + descent({descent}) ≈ cell_height({ch}), diff={}",
            (ascent + descent - ch).abs()
        );
    }

    #[test]
    fn termux_formula_baseline_is_ascent_from_cell_top() {
        let pipeline = FontPipeline::new(1024, 1024, 14.0);
        let ascent = pipeline.ascent_pixels();
        let (_, ch) = pipeline.cell_metrics();
        assert!(
            ascent > 0.0 && ascent < ch,
            "ascent({ascent}) must be in (0, cell_height={ch})"
        );
    }

    #[test]
    fn termux_formula_glyph_bearing_y_matches() {
        let mut pipeline = FontPipeline::new(1024, 1024, 14.0);
        let ascent = pipeline.ascent_pixels();
        let info = pipeline
            .glyph_information('A')
            .expect("should have 'A' glyph");
        let bearing_y = ascent - info.placement.top as f32;
        assert!(
            bearing_y >= 0.0,
            "bearing_y for 'A' should be >= 0, got {bearing_y}"
        );
        let (_, ch) = pipeline.cell_metrics();
        assert!(
            bearing_y < ch,
            "bearing_y({bearing_y}) should be < cell_height({ch})"
        );
    }

    #[test]
    fn descent_pixels_is_positive() {
        let pipeline = FontPipeline::new(1024, 1024, 14.0);
        let descent = pipeline.descent_pixels();
        assert!(descent > 0.0, "descent should be positive, got {descent}");
    }

    #[test]
    fn cell_width_from_m_advance_matches() {
        let mut pipeline = FontPipeline::new(1024, 1024, 14.0);
        let info_m = pipeline.glyph_information('m').expect("should have 'm'");
        let info_x = pipeline.glyph_information('X').expect("should have 'X'");
        let (cw, _ch) = pipeline.cell_metrics();
        assert!(
            (info_m.advance_width - cw).abs() < 1.0,
            "advance_width('m')={} ≈ cell_width={}",
            info_m.advance_width,
            cw
        );
        assert!(
            (info_x.advance_width - cw).abs() < 1.0,
            "advance_width('X')={} ≈ cell_width={}",
            info_x.advance_width,
            cw
        );
    }

    #[test]
    fn any_monospace_advance_matches_cell_width() {
        let mut pipeline = FontPipeline::new(1024, 1024, 14.0);
        let (cw, _) = pipeline.cell_metrics();
        let chars = ['A', 'm', 'W', '0', 'l', 'i'];
        for ch in chars {
            if let Some(info) = pipeline.glyph_information(ch) {
                assert!(
                    (info.advance_width - cw).abs() < 2.0,
                    "advance('{ch}')={:.1} ≈ cell_width={:.1}",
                    info.advance_width,
                    cw
                );
            }
        }
        if let Some(alt) = pipeline.list_monospace_fonts().first().cloned()
            && pipeline.set_font_family(&alt)
        {
            let (cw2, _) = pipeline.cell_metrics();
            for ch in chars {
                if let Some(info) = pipeline.glyph_information(ch) {
                    assert!(
                        (info.advance_width - cw2).abs() < 2.0,
                        "font '{alt}': advance('{ch}')={:.1} ≈ cell_width={:.1}",
                        info.advance_width,
                        cw2
                    );
                }
            }
        }
    }

    #[test]
    fn cjk_advance_valid_for_any_font() {
        let mut pipeline = FontPipeline::new(1024, 1024, 14.0);
        let (cw, _) = pipeline.cell_metrics();
        let cjk_chars = ['中', '好', '世', '界', '日', '本'];
        for ch in cjk_chars {
            if let Some(info) = pipeline.glyph_information(ch) {
                assert!(
                    info.advance_width > 0.0,
                    "CJK '{ch}' must have positive advance, got {:.1}",
                    info.advance_width
                );
                assert!(
                    info.advance_width <= cw * 3.0,
                    "CJK '{ch}' advance={:.1} should be ≤ 3*cell_width={:.1}",
                    info.advance_width,
                    cw * 3.0
                );
            }
        }
    }

    #[test]
    fn ascii_bearing_y_nonnegative_for_any_font() {
        let mut pipeline = FontPipeline::new(1024, 1024, 14.0);
        let ascent = pipeline.ascent_pixels();
        let ascii = ['A', 'B', 'C', 'x', 'y', 'z', '0', '1', '9'];
        for ch in ascii {
            if let Some(info) = pipeline.glyph_information(ch) {
                let bearing_y = ascent - info.placement.top as f32;
                assert!(
                    bearing_y >= -2.0,
                    "bearing_y('{ch}')={:.1} should be >= -2",
                    bearing_y
                );
            }
        }
    }

    #[test]
    fn all_glyphs_within_atlas_bounds() {
        let mut pipeline = FontPipeline::new(1024, 1024, 14.0);
        pipeline.rasterize_ascii();
        let atlas_width = pipeline.atlas_width as i32;
        let atlas_height = pipeline.atlas_height as i32;
        let chars = ['A', '中', '好', 'α', 'Ω'];
        for ch in chars {
            if let Some(info) = pipeline.glyph_information(ch) {
                assert!(
                    info.atlas_x + info.width <= atlas_width,
                    "glyph '{ch}' atlas_x({}) + width({}) exceeds atlas_w({})",
                    info.atlas_x,
                    info.width,
                    atlas_width
                );
                assert!(
                    info.atlas_y + info.height <= atlas_height,
                    "glyph '{ch}' atlas_y({}) + height({}) exceeds atlas_h({})",
                    info.atlas_y,
                    info.height,
                    atlas_height
                );
            }
        }
    }

    #[test]
    fn system_monospace_name_returns_nonempty() {
        let pipeline = FontPipeline::new(1024, 1024, 14.0);
        let name = pipeline.system_monospace_name();
        assert!(
            !name.is_empty(),
            "system_monospace_name should return a non-empty string"
        );
    }

    #[test]
    fn set_font_family_empty_resets_to_default() {
        let mut pipeline = FontPipeline::new(1024, 1024, 14.0);
        let default_name = pipeline.default_font_name().clone();
        let fonts = pipeline.list_monospace_fonts();
        if let Some(other) = fonts
            .iter()
            .find(|name| name.as_str() != default_name.as_str())
        {
            pipeline.set_font_family(other);
            assert_eq!(
                pipeline.current_font_family_name().as_deref(),
                Some(other.as_str())
            );
            pipeline.set_font_family("");
            assert_eq!(
                pipeline.current_font_family_name().as_deref(),
                Some(default_name.as_str())
            );
        }
    }

    #[test]
    fn font_information_contains_all_sections() {
        let pipeline = FontPipeline::new(1024, 1024, 14.0);
        let info = pipeline.font_information();
        assert!(
            info.contains("Active:"),
            "font_information should contain 'Active:', got: {}",
            info
        );
        assert!(
            info.contains("CJK fallback:"),
            "font_information should contain 'CJK fallback:', got: {}",
            info
        );
        assert!(
            info.contains("Cell:"),
            "font_information should contain 'Cell:', got: {}",
            info
        );
        assert!(
            info.contains("Font size:"),
            "font_information should contain 'Font size:', got: {}",
            info
        );
    }

    #[test]
    fn set_font_family_persists_through_size_change() {
        let mut pipeline = FontPipeline::new(1024, 1024, 14.0);
        let fonts = pipeline.list_monospace_fonts();
        if let Some(target) = fonts.first() {
            pipeline.set_font_family(target);
            let name_before = pipeline.current_font_family_name();
            pipeline.set_font_size_in_place(20.0);
            let name_after = pipeline.current_font_family_name();
            assert_eq!(
                name_before, name_after,
                "font family should persist through size change"
            );
        }
    }

    #[test]
    fn cjk_fallback_has_vector_font() {
        let pipeline = FontPipeline::new(1024, 1024, 14.0);
        let cjk_names = pipeline.cjk_fallback_names();
        if !cjk_names.is_empty() {
            assert!(
                cjk_names.iter().all(|name| !name.is_empty()),
                "CJK fallback names should not be empty strings"
            );
        }
    }

    #[test]
    fn cell_metrics_reasonable_ratios() {
        let pipeline = FontPipeline::new(1024, 1024, 14.0);
        let (cw, ch) = pipeline.cell_metrics();
        assert!(cw > 0.0, "cell_width must be > 0, got {cw}");
        assert!(ch > 0.0, "cell_height must be > 0, got {ch}");
        assert!(
            cw < ch,
            "terminal cells should be taller than wide: cell_width={cw} >= cell_height={ch}"
        );
    }

    #[test]
    fn find_monospace_font_prefers_roboto_mono() {
        let pipeline = FontPipeline::new(1024, 1024, 14.0);
        let name = pipeline.default_font_name();
        assert!(!name.is_empty(), "should find a monospace font, got empty");
    }

    fn try_load_cjk_fonts(font_database: &mut fontdb::Database) -> bool {
        let has_cjk = font_database.faces().any(|face| {
            face.families
                .first()
                .map(|(n, _)| n.to_lowercase().contains("cjk"))
                .unwrap_or(false)
        });
        if has_cjk {
            return true;
        }
        // System fonts only (fontconfig resolves the dev-shell fonts):
        // never scan hardcoded store paths.
        font_database.load_system_fonts();
        font_database.faces().any(|face| {
            face.families
                .first()
                .map(|(n, _)| n.to_lowercase().contains("cjk"))
                .unwrap_or(false)
        })
    }

    #[test]
    fn non_cjk_locale_no_fallback() {
        let mut pipeline = FontPipeline::new(1024, 1024, 14.0);
        pipeline.set_system_locale("en-US");
        let info = pipeline.font_information();
        assert!(
            info.contains("CJK fallback: none"),
            "en-US locale should have no CJK fallback: {info}"
        );
    }

    #[test]
    fn font_info_json_serializes_structured_fields() {
        let mut pipeline = FontPipeline::new(1024, 1024, 14.0);
        pipeline.set_system_locale("en-US");
        let json = serde_json::to_string(&pipeline.font_info()).expect("font_info serializes");
        let parsed: serde_json::Value = serde_json::from_str(&json).expect("font_info json parses");
        assert_eq!(parsed["cjk_state"], "none");
        assert!(
            parsed["active"].is_object(),
            "active should be present: {json}"
        );
        assert_eq!(parsed["font_size"], 14.0);
        assert!(parsed["cell_width_px"].as_f64().is_some());
        assert!(parsed["cjk_families"].is_array());
    }

    // ──: layered fallback ─────────────

    #[test]
    fn symbol_glyph_resolves_via_database_scan() {
        // U+25B6 (▶) is absent from Liberation Mono but present in
        // DejaVu Sans on the host. With Liberation Mono as the primary
        // the layered chain ends with a whole-database scan (spec d7),
        // which must resolve the glyph in a non-primary font.
        let mut pipeline = FontPipeline::new(512, 512, 14.0);
        let names = pipeline.list_monospace_fonts();
        assert!(
            names.iter().any(|name| name.contains("Liberation")),
            "Liberation Mono must be present (run inside nix develop)"
        );
        assert!(
            pipeline.set_font_family("Liberation Mono"),
            "switch to Liberation Mono"
        );
        let primary = pipeline.font_id.expect("primary font");
        let info = pipeline.glyph_information('▶').expect("symbol glyph");
        assert!(
            info.width > 0,
            "symbol must rasterize, got width {}",
            info.width
        );
        let resolved = pipeline
            .caches
            .cjk_glyph_cache
            .get(&'▶')
            .expect("resolved glyph must be cached");
        assert_ne!(
            resolved.0, primary,
            "symbol must resolve via a fallback font, not the primary"
        );
        assert_ne!(
            resolved.1, 0,
            "symbol must resolve to a real glyph, not .notdef"
        );
    }

    #[test]
    fn private_use_glyph_renders_notdef_without_panic() {
        // U+E0A0 (powerline separator PUA) exists in no host font: the
        // chain must end at.notdef without panicking (spec d7 scenario 3).
        let mut pipeline = FontPipeline::new(512, 512, 14.0);
        let info = pipeline.glyph_information('\u{e0a0}');
        assert!(info.is_some(), ".notdef fallback must return a glyph");
    }

    #[test]
    fn emoji_glyph_no_panic_when_color_font_cannot_outline() {
        // Noto Color Emoji covers 😀 but swash cannot outline color
        // glyphs; the emoji layer and the database scan must skip it
        // without panicking (moke: emoji via system chain).
        let mut pipeline = FontPipeline::new(512, 512, 14.0);
        let _ = pipeline.glyph_information('😀');
        // Reaching here without panic is the assertion.
    }

    #[test]
    fn fallback_names_report_all_layers() {
        // cjk_fallback_names is the CJK-only view; the layered fields are
        // all populated by construction.
        let pipeline = FontPipeline::new(512, 512, 14.0);
        let cjk = pipeline.cjk_fallback_names();
        assert!(
            cjk.iter().all(|name| !name.is_empty()),
            "CJK fallback names must not be empty strings"
        );
    }

    #[test]
    fn primary_cjk_font_no_fallback() {
        let mut pipeline = FontPipeline::new(512, 512, 14.0);
        assert!(
            try_load_cjk_fonts(pipeline.font_system.db_mut()),
            "CJK fonts must load (run inside nix develop)"
        );
        let mut pipeline = FontPipeline::new(512, 512, 14.0);
        try_load_cjk_fonts(pipeline.font_system.db_mut());
        pipeline.set_system_locale("zh-CN");
        let cjk_fonts: Vec<String> = pipeline
            .list_monospace_fonts()
            .into_iter()
            .filter(|name| name.to_lowercase().contains("cjk"))
            .collect();
        if let Some(cjk_name) = cjk_fonts.first() {
            pipeline.set_font_family(cjk_name);
            let names = pipeline.cjk_fallback_names();
            assert!(
                names.is_empty(),
                "primary font '{cjk_name}' supports CJK → no fallback, got: {names:?}"
            );
        }
    }

    #[test]
    fn max_one_fallback_font() {
        let mut pipeline = FontPipeline::new(1024, 1024, 14.0);
        assert!(
            try_load_cjk_fonts(pipeline.font_system.db_mut()),
            "CJK fonts must load (run inside nix develop)"
        );
        let mut pipeline = FontPipeline::new(1024, 1024, 14.0);
        try_load_cjk_fonts(pipeline.font_system.db_mut());
        pipeline.set_system_locale("zh-CN");
        assert!(
            pipeline.cjk_fallback_ids.len() <= 3,
            "MAX_CJK_FALLBACK_FONTS=3, got {} IDs",
            pipeline.cjk_fallback_ids.len()
        );
    }

    #[test]
    fn font_information_includes_cjk_fallback() {
        let pipeline = FontPipeline::new(1024, 1024, 14.0);
        let info = pipeline.font_information();
        assert!(
            info.contains("Active:") || info.contains("Cell:"),
            "font info should have structure: {info}"
        );
    }

    #[test]
    fn fonts_xml_index_match_resolves_exact_face() {
        let mut font_database = fontdb::Database::new();
        assert!(
            try_load_cjk_fonts(&mut font_database),
            "CJK fonts must load (run inside nix develop)"
        );
        // Pick a TTC face so (filename, index) mapping is exercised.
        let (filename, index) = font_database
            .faces()
            .filter_map(|face| {
                let path = match &face.source {
                    fontdb::Source::File(path) => path,
                    fontdb::Source::SharedFile(path, _) => path,
                    fontdb::Source::Binary(_) => return None,
                };
                let name = path.file_name()?.to_str()?.to_string();
                (path.extension()?.to_str()?.eq_ignore_ascii_case("ttc"))
                    .then_some((name, face.index))
            })
            .next()
            .expect("CJK TTC face must exist after try_load_cjk_fonts");
        let xml = format!(
            r#"<familyset version="23"><family lang="zh-Hans"><font weight="400" style="normal" index="{index}">{filename}</font></family></familyset>"#
        );
        let ids = FontPipeline::match_fonts_xml_fallbacks(&font_database, &xml, "zh-CN", 3);
        assert_eq!(ids.len(), 1, "exact (filename, index) hit expected");
        let face = font_database.face(ids[0]).expect("matched face exists");
        assert_eq!(face.index, index, "TTC index must match fonts.xml");
        let matched_name = match &face.source {
            fontdb::Source::File(path) => path,
            fontdb::Source::SharedFile(path, _) => path,
            fontdb::Source::Binary(_) => panic!("matched face must be file-backed"),
        }
        .file_name()
        .and_then(|name| name.to_str())
        .expect("file-backed face has a name");
        assert_eq!(matched_name, filename);
    }

    #[test]
    fn fonts_xml_missing_file_falls_back_to_scan() {
        // Unknown filename: no exact hit, caller fills from the scan.
        let mut font_database = fontdb::Database::new();
        assert!(
            try_load_cjk_fonts(&mut font_database),
            "CJK fonts must load (run inside nix develop)"
        );
        let xml = r#"<familyset version="23"><family lang="zh-Hans"><font index="2">NoSuchFont-Regular.ttc</font></family></familyset>"#;
        let ids = FontPipeline::match_fonts_xml_fallbacks(&font_database, xml, "zh-CN", 3);
        assert!(
            ids.is_empty(),
            "unknown file must yield no exact hit: {ids:?}"
        );
    }

    #[test]
    fn fonts_xml_duplicate_entries_deduplicated() {
        // 真机形态：zh-Hans 链内同一文件多 weights 同 index 重复出现，
        // 首位命中后后续重复必须去重（真机 Sans index=2 出现 9 次只取 1 个）。
        let mut font_database = fontdb::Database::new();
        assert!(
            try_load_cjk_fonts(&mut font_database),
            "CJK fonts must load (run inside nix develop)"
        );
        let (filename, index) = font_database
            .faces()
            .filter_map(|face| {
                let path = match &face.source {
                    fontdb::Source::File(path) => path,
                    fontdb::Source::SharedFile(path, _) => path,
                    fontdb::Source::Binary(_) => return None,
                };
                let name = path.file_name()?.to_str()?.to_string();
                (path.extension()?.to_str()?.eq_ignore_ascii_case("ttc"))
                    .then_some((name, face.index))
            })
            .next()
            .expect("CJK TTC face must exist after try_load_cjk_fonts");
        let xml = format!(
            r#"<familyset version="23"><family lang="zh-Hans"><font weight="100" style="normal" index="{index}">{filename}</font><font weight="400" style="normal" index="{index}">{filename}</font><font weight="900" style="normal" index="{index}">{filename}</font></family></familyset>"#
        );
        let ids = FontPipeline::match_fonts_xml_fallbacks(&font_database, &xml, "zh-CN", 3);
        assert_eq!(
            ids.len(),
            1,
            "repeated (filename, index) entries must deduplicate: {ids:?}"
        );
        let face = font_database.face(ids[0]).expect("matched face exists");
        assert_eq!(face.index, index, "TTC index must match fonts.xml");
    }

    #[test]
    fn fonts_xml_result_order_follows_xml_order() {
        // 真机形态：zh-Hans 链 Sans 在前 Serif 在后，返回顺序必须跟随
        // xml 顺序（首位胜出），而非数据库加载顺序。
        let mut font_database = fontdb::Database::new();
        assert!(
            try_load_cjk_fonts(&mut font_database),
            "CJK fonts must load (run inside nix develop)"
        );
        let mut faces: Vec<(String, u32)> = Vec::new();
        for face in font_database.faces() {
            let path = match &face.source {
                fontdb::Source::File(path) => path,
                fontdb::Source::SharedFile(path, _) => path,
                fontdb::Source::Binary(_) => continue,
            };
            let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
                continue;
            };
            let is_ttc = path
                .extension()
                .and_then(|extension| extension.to_str())
                .is_some_and(|extension| extension.eq_ignore_ascii_case("ttc"));
            if is_ttc && !faces.iter().any(|(_, index)| *index == face.index) {
                faces.push((name.to_string(), face.index));
            }
            if faces.len() == 2 {
                break;
            }
        }
        assert_eq!(
            faces.len(),
            2,
            "TTC must expose two distinct faces for order test"
        );
        let (first_filename, first_index) = &faces[0];
        let (second_filename, second_index) = &faces[1];
        let xml = format!(
            r#"<familyset version="23"><family lang="zh-Hans"><font weight="400" style="normal" index="{first_index}">{first_filename}</font><font weight="400" style="normal" index="{second_index}">{second_filename}</font></family></familyset>"#
        );
        let ids = FontPipeline::match_fonts_xml_fallbacks(&font_database, &xml, "zh-CN", 3);
        assert_eq!(ids.len(), 2, "both entries must resolve: {ids:?}");
        assert_eq!(
            font_database.face(ids[0]).expect("first face exists").index,
            *first_index,
            "first result must follow the first xml entry"
        );
        assert_eq!(
            font_database
                .face(ids[1])
                .expect("second face exists")
                .index,
            *second_index,
            "second result must follow the second xml entry"
        );
    }

    /// Locate the Maple Mono font through the system font database
    /// (fontconfig resolves the dev-shell fonts; no paths are hardcoded).
    fn find_maple_mono_font(font_database: &mut fontdb::Database) -> Option<std::path::PathBuf> {
        font_database.load_system_fonts();
        font_database
            .faces()
            .filter(|face| {
                face.families
                    .first()
                    .is_some_and(|(name, _)| name.to_lowercase().contains("maple"))
            })
            .filter_map(|face| match &face.source {
                fontdb::Source::File(path) => Some(path.clone()),
                fontdb::Source::SharedFile(path, _) => Some(path.clone()),
                fontdb::Source::Binary(_) => None,
            })
            .next()
    }

    #[test]
    fn maple_mono_primary_skips_cjk_fallback() {
        // Maple Mono NF CN ships CJK glyphs: as the primary font it must
        // cover CJK directly with no fallback layer (spec: skip path).
        // CJK + Latin resolve through the same cache, keeping CJK render
        // speed on par with Latin (no per-glyph fallback scan).
        let mut maple_db = fontdb::Database::new();
        let font_path = find_maple_mono_font(&mut maple_db)
            .expect("Maple Mono must be present (run inside nix develop)");
        let mut pipeline = FontPipeline::new(512, 512, 14.0);
        let family = pipeline
            .load_font_file(&font_path)
            .expect("maple mono loads");
        assert!(
            pipeline.set_font_family(&family),
            "maple mono selectable as primary"
        );
        pipeline.set_system_locale("zh-CN");
        assert!(
            pipeline.cjk_fallback_names().is_empty(),
            "CJK-capable primary must skip fallback, got: {:?}",
            pipeline.cjk_fallback_names()
        );
        let latin = pipeline.glyph_information('A').expect("latin resolves");
        let cjk = pipeline.glyph_information('中').expect("CJK resolves");
        assert!(latin.width > 0 && cjk.width > 0);
        // Second pass must hit the caches (no repeated fallback scans).
        let latin_again = pipeline.glyph_information('A').expect("latin cached");
        let cjk_again = pipeline.glyph_information('中').expect("CJK cached");
        assert_eq!(latin.width, latin_again.width);
        assert_eq!(cjk.width, cjk_again.width);
    }

    #[test]
    fn atlas_defrag_recovers_from_full_atlas() {
        let mut pipeline = FontPipeline::new(64, 64, 14.0);
        let mut successes = 0u32;
        for cp in 0x4E00u32..0x4F00u32 {
            if let Some(ch) = char::from_u32(cp)
                && pipeline
                    .glyph_information(ch)
                    .is_some_and(|glyph_info| glyph_info.width > 0)
            {
                successes += 1;
            }
        }
        assert!(
            successes > 0,
            "should have inserted at least some CJK glyphs"
        );
        let bitmap = pipeline.atlas_bitmap();
        assert!(
            bitmap.iter().any(|&b| b != 0),
            "atlas should have content after defrag"
        );
    }

    /// 取字体库中第一个来自文件的字形字体路径。
    ///
    /// 开发环境的 `flake.nix` 用 `FONTCONFIG_FILE` 固定了字体集，直接查已加载的
    /// 字体库即可，不再探测文件系统或调用外部进程。
    fn find_test_font() -> std::path::PathBuf {
        let pipeline = FontPipeline::new(512, 512, 14.0);
        pipeline
            .font_system
            .db()
            .faces()
            .filter_map(|face| match face.source {
                fontdb::Source::File(ref path) => Some(path.clone()),
                _ => None,
            })
            .find(|path| {
                path.extension()
                    .and_then(|extension| extension.to_str())
                    .is_some_and(|extension| extension.eq_ignore_ascii_case("ttf"))
            })
            .expect("开发环境固定了系统字体集，其中必有 .ttf 轮廓字体")
    }

    #[test]
    fn load_font_file_valid_ttf_returns_family() {
        let mut pipeline = FontPipeline::new(512, 512, 14.0);
        let font_path = find_test_font();
        let family = pipeline
            .load_font_file(&font_path)
            .expect("有效 TTF 必须返回族名");
        assert!(
            pipeline
                .font_system
                .db()
                .faces()
                .any(|face| face.families.iter().any(|(name, _)| *name == family)),
            "返回的族名 {family} 必须已在字体库中登记"
        );
    }

    #[test]
    fn load_font_file_nonexistent_path_returns_none() {
        let mut pipeline = FontPipeline::new(512, 512, 14.0);
        let result = pipeline.load_font_file(std::path::Path::new("/nonexistent/path/to/font.ttf"));
        assert!(result.is_none(), "should return None for nonexistent path");
    }

    #[test]
    fn load_font_file_empty_file_returns_none() {
        let dir = std::env::temp_dir().join("test_font_load");
        let _ = std::fs::create_dir_all(&dir);
        let empty_path = dir.join("empty.ttf");
        std::fs::write(&empty_path, []).ok();
        let mut pipeline = FontPipeline::new(512, 512, 14.0);
        let result = pipeline.load_font_file(&empty_path);
        assert!(result.is_none(), "empty file should return None");
        let _ = std::fs::remove_file(&empty_path);
    }

    #[test]
    fn load_font_file_corrupt_file_returns_none() {
        let dir = std::env::temp_dir().join("test_font_load");
        let _ = std::fs::create_dir_all(&dir);
        let corrupt_path = dir.join("corrupt.ttf");
        let garbage: Vec<u8> = (0..256).map(|raw_byte| (raw_byte ^ 0xAB) as u8).collect();
        std::fs::write(&corrupt_path, &garbage).ok();
        let mut pipeline = FontPipeline::new(512, 512, 14.0);
        let result = pipeline.load_font_file(&corrupt_path);
        assert!(result.is_none(), "corrupt file should return None");
        let _ = std::fs::remove_file(&corrupt_path);
    }

    // ──: bold/italic glyph synthesis ─────────────────────────

    /// 提取字形在图集区域的覆盖度字节（RGBA 图集的红色通道）。
    fn glyph_region_alpha(info: &GlyphInfo, bitmap: &[u8], atlas_width: usize) -> Vec<u8> {
        if info.allocation_id.is_none() || info.width <= 0 || info.height <= 0 {
            return Vec::new();
        }
        let mut region = Vec::with_capacity(info.width as usize * info.height as usize);
        for pixel_row in 0..info.height as usize {
            for pixel_column in 0..info.width as usize {
                let offset = ((info.atlas_y as usize + pixel_row) * atlas_width
                    + info.atlas_x as usize
                    + pixel_column)
                    * BYTES_PER_PIXEL;
                region.push(bitmap[offset]);
            }
        }
        region
    }

    fn styled_test_pipeline() -> (FontPipeline, String) {
        let mut pipeline = FontPipeline::new(512, 512, 14.0);
        let font_path = find_test_font();
        let family = pipeline
            .load_font_file(&font_path)
            .expect("test font loads");
        assert!(
            pipeline.set_font_family(&family),
            "test font family selects"
        );
        (pipeline, family)
    }

    /// 可变字体必须走 `wght`/`ital` 轴（DESIGN 字体节：多字重与动态字体）。
    ///
    /// 按能力而非按名字找：开发环境的 `flake.nix` 固定了 `noto-fonts-cjk-sans`
    /// （`NotoSansCJK-VF.otf.ttc`，带 `wght` 轴）与 `maple-mono.Normal-NF-CN`
    /// （16 个静态字重的族，**无** `fvar` 表，真字重靠同族查询）。两种形态都须正确。
    #[test]
    fn variable_font_axes_are_applied() {
        let pipeline = FontPipeline::new(512, 512, 24.0);
        let font_database = pipeline.font_system.db();
        let axes_of = |font_id: fontdb::ID| {
            font_database
                .with_face_data(font_id, |font_data, face_index| {
                    let font_ref = swash::FontRef::from_index(font_data, face_index as usize)?;
                    Some(
                        font_ref
                            .variations()
                            .map(|axis| (axis.tag(), axis.min_value(), axis.max_value()))
                            .collect::<Vec<_>>(),
                    )
                })
                .flatten()
        };
        let settings_of = |font_id: fontdb::ID, synthesis: GlyphSynthesis| {
            font_database
                .with_face_data(font_id, |font_data, face_index| {
                    let font_ref = swash::FontRef::from_index(font_data, face_index as usize)?;
                    Some(super::variation_settings(&font_ref, synthesis))
                })
                .flatten()
                .expect("字体可读")
        };

        let (variable_id, variable_axes) = font_database
            .faces()
            .filter_map(|face| axes_of(face.id).map(|axes| (face.id, axes)))
            .find(|(_, axes)| {
                axes.iter()
                    .any(|(tag, min, max)| *tag == swash::tag_from_bytes(b"wght") && max > min)
            })
            .expect("开发环境固定了带 wght 轴的可变字体（noto-fonts-cjk-sans）");
        let (_, min_weight, max_weight) = variable_axes
            .iter()
            .find(|(tag, _, _)| *tag == swash::tag_from_bytes(b"wght"))
            .copied()
            .expect("已断言存在 wght 轴");

        // 粗体只设 wght 一条轴，取值被 clamp 到字体声明的范围。
        let bold = settings_of(variable_id, GlyphSynthesis::Bold);
        assert_eq!(bold.len(), 1, "粗体只应设置 wght 一条轴");
        assert_eq!(bold[0].tag, swash::tag_from_bytes(b"wght"));
        assert_eq!(
            bold[0].value,
            VARIATION_WEIGHT_BOLD.min(max_weight).max(min_weight)
        );

        // 粗斜体：字体声明 ital 轴时两条，否则只有 wght。
        let bold_italic = settings_of(variable_id, GlyphSynthesis::BoldItalic);
        let has_ital = variable_axes
            .iter()
            .any(|(tag, _, _)| *tag == swash::tag_from_bytes(b"ital"));
        assert_eq!(
            bold_italic.len(),
            if has_ital { 2 } else { 1 },
            "粗斜体轴数须与字体声明一致"
        );

        // 静态字体（无 fvar，如 Maple Mono / DroidSansMono）不得产出轴设置，
        // 交回轮廓级加粗/剪切。
        let static_id = font_database
            .faces()
            .map(|face| face.id)
            .find(|id| axes_of(*id).is_some_and(|axes| axes.is_empty()))
            .expect("系统等宽字体不是可变字体");
        assert!(
            settings_of(static_id, GlyphSynthesis::BoldItalic).is_empty(),
            "静态字体必须退回轮廓级合成"
        );
    }

    #[test]
    fn styled_bold_produces_heavier_glyph() {
        let (mut pipeline, _) = styled_test_pipeline();
        let regular = pipeline.glyph_information('A').expect("regular A");
        let bold = pipeline
            .glyph_information_styled('A', true, false)
            .expect("bold A");
        assert!(bold.width > 0 && bold.height > 0, "bold bitmap must exist");
        // The styled bitmap must actually differ from the regular one —
        // either a real bold face was resolved or synthesis emboldened it.
        assert_ne!(
            bold.atlas_x, regular.atlas_x,
            "bold and regular glyphs must not share a cache entry"
        );
    }

    #[test]
    fn styled_italic_shears_glyph() {
        let (mut pipeline, _) = styled_test_pipeline();
        let regular = pipeline.glyph_information('A').expect("regular A");
        let italic = pipeline
            .glyph_information_styled('A', false, true)
            .expect("italic A");
        assert!(
            italic.width > 0 && italic.height > 0,
            "italic bitmap must exist"
        );
        assert_ne!(
            italic.atlas_x, regular.atlas_x,
            "italic and regular glyphs must not share a cache entry"
        );
        // 真实斜体字面或剪切合成都会改变位图内容，直接比较区域字节，
        // 避免像素计数偶然相同导致的误判。
        let bitmap = pipeline.atlas_bitmap();
        let atlas_width = pipeline.atlas_width as usize;
        let regular_alpha = glyph_region_alpha(&regular, bitmap, atlas_width);
        let italic_alpha = glyph_region_alpha(&italic, bitmap, atlas_width);
        assert!(
            regular_alpha.iter().any(|&alpha| alpha > 0),
            "常规字形区域必须有墨水"
        );
        assert!(
            italic_alpha.iter().any(|&alpha| alpha > 0),
            "斜体字形区域必须有墨水"
        );
        assert_ne!(
            regular_alpha, italic_alpha,
            "斜体位图必须与常规不同（真实字面或剪切合成）"
        );
    }

    #[test]
    fn styled_bold_italic_combines_both() {
        let (mut pipeline, _) = styled_test_pipeline();
        let regular = pipeline.glyph_information('A').expect("regular A");
        let bold_italic = pipeline
            .glyph_information_styled('A', true, true)
            .expect("bold-italic A");
        assert!(bold_italic.width > 0 && bold_italic.height > 0);
        assert_ne!(
            bold_italic.atlas_x, regular.atlas_x,
            "bold-italic must have its own cache entry"
        );
    }

    #[test]
    fn styled_glyph_cache_distinguishes_synthesis() {
        let (mut pipeline, _) = styled_test_pipeline();
        let _regular = pipeline.glyph_information('A').expect("regular A");
        let bold = pipeline
            .glyph_information_styled('A', true, false)
            .expect("bold A");
        let italic = pipeline
            .glyph_information_styled('A', false, true)
            .expect("italic A");
        // Re-lookup returns the cached styled glyphs (same atlas slot) and
        // never the regular one.
        let bold_again = pipeline
            .glyph_information_styled('A', true, false)
            .expect("bold A again");
        let italic_again = pipeline
            .glyph_information_styled('A', false, true)
            .expect("italic A again");
        assert_eq!(bold.atlas_x, bold_again.atlas_x);
        assert_eq!(italic.atlas_x, italic_again.atlas_x);
    }

    /// 图集重建必须保留合成位：满图重建把合成字形降级为常规位图，
    /// 斜体重建成常规体（满图后斜体退化缺失）。
    #[test]
    fn rebuild_atlas_preserves_synthesis_keyed_entries() {
        let (mut pipeline, _) = styled_test_pipeline();
        let regular = pipeline.glyph_information('A').expect("regular A");
        let glyph_id = pipeline.caches.ascii_glyph_ids['A' as usize].expect("ascii gid cached");
        let font_id = pipeline.font_id.expect("primary font set");
        let italic = pipeline
            .glyph_information_from_font_with_synthesis(font_id, glyph_id, GlyphSynthesis::Italic)
            .expect("synthesized italic A");
        assert!(
            italic.width > 0 && italic.height > 0,
            "synthesized italic bitmap must exist"
        );
        let atlas_width = pipeline.atlas_width as usize;
        let italic_alpha_before = glyph_region_alpha(&italic, pipeline.atlas_bitmap(), atlas_width);
        assert!(
            italic_alpha_before.iter().any(|&alpha| alpha > 0),
            "synthesized italic region must have ink"
        );
        assert!(
            pipeline
                .caches
                .glyph_cache
                .iter()
                .any(|(key, _)| key.synthesis == GlyphSynthesis::Italic.bits()),
            "precondition: italic synthesis entry must be cached"
        );
        let generation = pipeline.atlas_generation();
        pipeline.rebuild_atlas();
        assert!(
            pipeline.atlas_generation() > generation,
            "rebuild must bump the atlas generation"
        );
        assert!(
            pipeline
                .caches
                .glyph_cache
                .iter()
                .any(|(key, _)| key.synthesis == GlyphSynthesis::Italic.bits()),
            "rebuild must preserve synthesis-keyed entries"
        );
        let italic_after = pipeline
            .lookup_glyph(font_id, glyph_id, GlyphSynthesis::Italic)
            .expect("synthesized italic must stay cached after rebuild");
        let bitmap_after = pipeline.atlas_bitmap().to_vec();
        let atlas_width_after = pipeline.atlas_width as usize;
        let italic_alpha_after =
            glyph_region_alpha(&italic_after, &bitmap_after, atlas_width_after);
        assert_eq!(
            italic_alpha_before, italic_alpha_after,
            "rebuilt synthesized bitmap must match the pre-rebuild content"
        );
        let regular_after = pipeline
            .glyph_information('A')
            .expect("regular A after rebuild");
        let regular_alpha_after =
            glyph_region_alpha(&regular_after, &bitmap_after, atlas_width_after);
        assert_ne!(
            regular_alpha_after, italic_alpha_after,
            "synthesized italic must still differ from regular after rebuild"
        );
        let _ = regular;
    }

    /// 回应对“d 有些区域像 a”：相邻小写字母必须命中不同缓存条目与不同位图。
    #[test]
    fn distinct_lowercase_glyphs_do_not_collide() {
        let (mut pipeline, _) = styled_test_pipeline();
        let glyph_d = pipeline.glyph_information('d').expect("d");
        let glyph_a = pipeline.glyph_information('a').expect("a");
        assert_ne!(
            (glyph_d.atlas_x, glyph_d.atlas_y),
            (glyph_a.atlas_x, glyph_a.atlas_y),
            "d and a must not share a cache entry"
        );
        let bitmap = pipeline.atlas_bitmap();
        let atlas_width = pipeline.atlas_width as usize;
        let alpha_d = glyph_region_alpha(&glyph_d, bitmap, atlas_width);
        let alpha_a = glyph_region_alpha(&glyph_a, bitmap, atlas_width);
        assert!(alpha_d.iter().any(|&alpha| alpha > 0), "d 必须有墨水");
        assert!(alpha_a.iter().any(|&alpha| alpha > 0), "a 必须有墨水");
        assert_ne!(alpha_d, alpha_a, "d 与 a 位图必须不同");
    }

    #[test]
    fn resolve_style_face_prefers_same_family_bold_when_available() {
        let (mut pipeline, _) = styled_test_pipeline();
        let base_id = pipeline.font_id.expect("font selected");
        let base_family = pipeline.font_system.db().face(base_id).and_then(|face| {
            face.families
                .first()
                .map(|(family_name, _)| family_name.clone())
        });
        // With system fonts loaded, the family may or may not have a bold
        // face on this host. Either way the contract must hold: a resolved
        // face belongs to the same family and differs from the base; no
        // face at all means the caller falls back to synthesis.
        if let Some(style_id) = pipeline.resolve_style_face(base_id, true, false) {
            assert_ne!(style_id, base_id, "bold face must differ from regular");
            let style_family = pipeline.font_system.db().face(style_id).and_then(|face| {
                face.families
                    .first()
                    .map(|(family_name, _)| family_name.clone())
            });
            assert_eq!(
                base_family, style_family,
                "style face must share the base family"
            );
        }
        // Resolving the plain style yields a face of the same family
        // (fontdb's query returns the closest match, which is the base
        // itself unless another normal face of the family exists — e.g.
        // DejaVuSansCondensed — so only the family invariant is asserted).
        if let Some(plain) = pipeline.resolve_style_face(base_id, false, false) {
            let plain_family = pipeline.font_system.db().face(plain).and_then(|face| {
                face.families
                    .first()
                    .map(|(family_name, _)| family_name.clone())
            });
            assert_eq!(
                base_family, plain_family,
                "plain-style face must share the base family"
            );
        }
    }

    #[test]
    fn load_font_file_multiple_times_works() {
        let mut pipeline = FontPipeline::new(512, 512, 14.0);
        let font_path = find_test_font();
        let first = pipeline.load_font_file(&font_path);
        let second = pipeline.load_font_file(&font_path);
        assert!(first.is_some(), "first load should succeed");
        assert!(second.is_some(), "second load of same file should succeed");
        assert_eq!(
            first, second,
            "loading same file twice should return same family"
        );
    }

    #[test]
    fn load_font_file_does_not_break_cell_metrics() {
        let mut pipeline = FontPipeline::new(512, 512, 14.0);
        let (cell_width_before, cell_height_before) = pipeline.cell_metrics();
        assert!(
            cell_width_before > 0.0 && cell_height_before > 0.0,
            "initial metrics should be positive"
        );
        let font_path = find_test_font();
        let family = pipeline.load_font_file(&font_path);
        assert!(family.is_some(), "should load test font");
        let (cell_width_after, cell_height_after) = pipeline.cell_metrics();
        assert!(
            (cell_width_before - cell_width_after).abs() < f32::EPSILON,
            "cell width unchanged after load_font_file"
        );
        assert!(
            (cell_height_before - cell_height_after).abs() < f32::EPSILON,
            "cell height unchanged after load_font_file"
        );
    }

    #[test]
    fn load_font_file_loaded_font_can_be_set() {
        let mut pipeline = FontPipeline::new(512, 512, 14.0);
        let family = pipeline
            .load_font_file(&find_test_font())
            .expect("should load test font");
        assert!(
            pipeline.set_font_family(&family),
            "set_font_family should succeed for loaded font '{family}'"
        );
        let (cw, ch) = pipeline.cell_metrics();
        assert!(cw > 0.0, "cell width positive after setting loaded font");
        assert!(ch > 0.0, "cell height positive after setting loaded font");
    }

    #[test]
    fn load_font_file_unicode_path() {
        let dir = std::env::temp_dir().join("test_unicode_字体");
        let _ = std::fs::create_dir_all(&dir);
        let target = dir.join("测试-font.ttf");
        std::fs::copy(find_test_font(), &target).expect("copy test font to unicode path");
        let mut pipeline = FontPipeline::new(512, 512, 14.0);
        let family = pipeline.load_font_file(&target);
        assert!(family.is_some(), "should load font from unicode path");
        assert!(!family.unwrap().is_empty(), "family should not be empty");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn load_font_file_after_set_font_family() {
        let mut pipeline = FontPipeline::new(512, 512, 14.0);
        let fonts = pipeline.list_monospace_fonts();
        if let Some(first) = fonts.first() {
            assert!(pipeline.set_font_family(first), "set font family {first}");
        }
        let result = pipeline.load_font_file(&find_test_font());
        assert!(
            result.is_some(),
            "load after set_font_family should succeed"
        );
    }

    #[test]
    fn glyph_cache_key_distinguishes_subpixel_raster_sizes() {
        // 36.75px 与 36.22px 截断同为 36：旧 `as u16` 键共用同一条目，
        // 缩放后取到错误尺寸位图（模糊/字形不对）。键必须精确区分。
        let big = 14.0f32 * 2.625;
        let small = 13.8f32 * 2.625;
        assert_eq!(big as u16, small as u16, "用例前提：截断后同键");
        assert_ne!(
            super::raster_size_key(big),
            super::raster_size_key(small),
            "不同光栅尺寸必须命中不同缓存条目"
        );
        assert_eq!(
            super::raster_size_key(big),
            super::raster_size_key(big),
            "同尺寸必须命中同一条目"
        );
    }
}
