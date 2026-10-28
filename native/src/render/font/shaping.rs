//! 文本整形：接入 cosmic-text 以支持单码位整形与复杂文种叠加（按格渲染，不跨单元连字）。
use super::{FontPipeline, ShapedGlyphInfo, glyph_cache::ShapeKey};
use unicode_script::{Script, UnicodeScript};

/// 需要挂 CJK 回退族的文字：判定走 UAX #24 `Script` 属性而非硬编码码点区间，
/// 随 `unicode-script` 的 Unicode 版本更新自动跟进（如扩展 G/H 区）。
/// 兼容/半角符号区（U+3000..U+303F CJK 符号标点、U+FF00..U+FFEF 半角全角形）
/// 的 `Script` 多为 `Common` 而非 `Han`，仍沿用旧区间语义保留。
fn needs_cjk_fallback(character: char) -> bool {
    matches!(
        character.script(),
        Script::Han
            | Script::Hiragana
            | Script::Katakana
            | Script::Hangul
            | Script::Bopomofo
            | Script::Yi
            | Script::Tangut
            | Script::Khitan_Small_Script
            | Script::Nushu
    ) || matches!(
        character as u32,
        0x3000..=0x303F | 0xFF00..=0xFFEF
    )
}

/// cosmic-text Metrics 的行高倍数。
const DEFAULT_LINE_HEIGHT_RATIO: f32 = 1.2;

/// 等效无限的整形缓冲宽度。
const INFINITE_BUFFER_WIDTH: f32 = 999_999.0;

impl FontPipeline {
    pub fn shape_run(&mut self, text: &str) -> Vec<ShapedGlyphInfo> {
        self.shape_run_styled(text, false, false)
    }

    /// 按单元的粗体/斜体选择同族字面；字形缓存键包含这两个维度。
    pub fn shape_run_styled(
        &mut self,
        text: &str,
        bold: bool,
        italic: bool,
    ) -> Vec<ShapedGlyphInfo> {
        if text.is_empty() {
            return Vec::new();
        }
        // 整形结果依赖字号/字体/光栅缩放/回退层：单文本键在任一维度
        // 变化时串味（旧字号的 glyph_id 与 x 偏移被复用，“d 像 a”类错字）。
        let shape_key = ShapeKey {
            text: text.to_string(),
            font_size_bits: self.font_size.to_bits(),
            raster_scale_bits: self.raster_scale.to_bits(),
            font_id: self.font_id,
            fallback_generation: self.fallback_generation,
            bold,
            italic,
        };
        if let Some(cached) = self.caches.shape_cache.get(&shape_key) {
            return cached.clone();
        }

        // 布局单位必须是**光栅像素**：`x_offset`/`y_offset` 被 cell_builder 直接加到
        // 四边形原点上，而原点是 `cell_metrics() * raster_scale` 得来的物理像素。
        // 用逻辑字号整形会让高密度屏上的组合标记/ZWJ 叠加层偏移一个 raster_scale 倍。
        // 缓存键本就含 raster_scale，键值语义与此处一致。
        let raster_size = self.font_size * self.raster_scale;
        let metrics =
            cosmic_text::Metrics::new(raster_size, raster_size * DEFAULT_LINE_HEIGHT_RATIO);
        let mut buffer = self.shaping_buffer.take().unwrap_or_else(|| {
            let mut reused_buffer = cosmic_text::Buffer::new_empty(metrics);
            reused_buffer.set_size(Some(INFINITE_BUFFER_WIDTH), None);
            reused_buffer
        });
        buffer.set_metrics(metrics);
        buffer.set_size(Some(INFINITE_BUFFER_WIDTH), None);

        let family_name = self.default_font_name();
        let family = if family_name.is_empty() {
            cosmic_text::Family::Monospace
        } else {
            cosmic_text::Family::Name(&family_name)
        };
        let weight = if bold {
            cosmic_text::Weight::BOLD
        } else {
            cosmic_text::Weight::NORMAL
        };
        let style = if italic {
            cosmic_text::Style::Italic
        } else {
            cosmic_text::Style::Normal
        };
        let attrs = cosmic_text::Attrs::new()
            .family(family)
            .weight(weight)
            .style(style);

        buffer.set_text(text, &attrs, cosmic_text::Shaping::Advanced, None);
        if !self.cjk_fallback_ids.is_empty() {
            // 只对真正的 CJK 段挂回退族：整段 0..len 会让 "hello中文" 的拉丁
            // 部分也走回退整形（多花开销且 IME 命中不了缓存）。
            let mut cjk_ranges: Vec<std::ops::Range<usize>> = Vec::new();
            let mut start: Option<usize> = None;
            for (byte_index, character) in text.char_indices() {
                let is_cjk = needs_cjk_fallback(character);
                if is_cjk {
                    if start.is_none() {
                        start = Some(byte_index);
                    }
                } else if let Some(range_start) = start.take() {
                    cjk_ranges.push(range_start..byte_index);
                }
            }
            if let Some(range_start) = start {
                cjk_ranges.push(range_start..text.len());
            }
            if !cjk_ranges.is_empty() {
                let font_database = self.font_system.db();
                let mut list = cosmic_text::AttrsList::new(&attrs);
                // 倒序挂 span：AttrsList 的重叠区间后者覆盖前者，而光栅侧的回退链
                // （pipeline.rs 的 FALLBACK_HIT）是首个命中即返回。正序会让整形挑中
                // 优先级最低的那个族，与实际光栅的字体不同——叠加层于是按错误的
                // 字形度量定位。倒序后整形侧同样是「首个命中」。
                for &fallback_id in self.cjk_fallback_ids.iter().rev() {
                    if let Some(face) = font_database.face(fallback_id)
                        && let Some((fallback_name, _)) = face.families.first()
                    {
                        for range in &cjk_ranges {
                            list.add_span(
                                range.clone(),
                                &cosmic_text::Attrs::new()
                                    .family(cosmic_text::Family::Name(fallback_name))
                                    .weight(weight)
                                    .style(style),
                            );
                        }
                    }
                }
                for line in &mut buffer.lines {
                    line.set_attrs_list(list.clone());
                }
            }
        }
        buffer.shape_until_scroll(&mut self.font_system, false);

        let result: Vec<ShapedGlyphInfo> = buffer
            .layout_runs()
            .flat_map(|run| run.glyphs.iter())
            .map(|glyph| ShapedGlyphInfo {
                glyph_id: glyph.glyph_id,
                font_id: glyph.font_id,
                x: glyph.x,
                w: glyph.w,
                // 上游 offset 以字号为单位，物理位置为 `x + font_size * x_offset`；
                // 下游直接加到物理像素原点，故在此一次性换算。
                x_offset: glyph.x + glyph.x_offset * glyph.font_size,
                // 上游物理坐标为 `y - font_size * y_offset`。
                y_offset: -glyph.y_offset * glyph.font_size,
            })
            .collect();

        self.shaping_buffer = Some(buffer);
        self.caches.shape_cache.put(shape_key, result.clone());
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn small_pipeline() -> FontPipeline {
        FontPipeline::new(512, 512, 12.0)
    }

    fn advance(glyphs: &[ShapedGlyphInfo]) -> f32 {
        glyphs.iter().map(|glyph| glyph.w).sum()
    }

    #[test]
    fn empty_text_shapes_to_nothing() {
        let mut pipeline = small_pipeline();
        assert!(pipeline.shape_run("").is_empty());
    }

    #[test]
    fn ascii_text_produces_glyphs() {
        let mut pipeline = small_pipeline();
        let glyphs = pipeline.shape_run("Hello");
        assert!(!glyphs.is_empty(), "ASCII 'Hello' must shape to glyphs");
        let mut prev_x = 0.0f32;
        for glyph in &glyphs {
            assert!(glyph.x >= prev_x, "glyph x must not go backwards");
            prev_x = glyph.x;
        }
    }

    /// 整形输出（推进宽度与叠加层偏移）必须是**光栅像素**：cell_builder 把
    /// `x_offset`/`y_offset` 直接加到 `cell_metrics() * raster_scale` 得来的
    /// 物理像素原点上。用逻辑字号整形会让高密度屏上的组合标记/ZWJ 叠加层
    /// 偏移一个 raster_scale 倍。缓存键含 raster_scale，键值语义与此处一致。
    #[test]
    fn shaping_advances_in_raster_pixels() {
        let mut pipeline = small_pipeline();
        pipeline.set_raster_scale(1.0);
        let at_one = advance(&pipeline.shape_run("Hello"));
        pipeline.set_raster_scale(2.0);
        let at_two = advance(&pipeline.shape_run("Hello"));
        assert!(at_one > 0.0, "baseline advance must be non-zero");
        assert!(
            (at_two - 2.0 * at_one).abs() < 0.5 * at_one,
            "advance must double with raster_scale (1x={at_one}, 2x={at_two})",
        );
    }

    #[test]
    fn shaping_results_are_cached() {
        let mut pipeline = small_pipeline();
        let first = pipeline.shape_run("cache me");
        assert!(!first.is_empty());
        let second = pipeline.shape_run("cache me");
        assert_eq!(first, second, "cached shape must equal first shape");
    }

    #[test]
    fn combining_cluster_shapes_to_positioned_glyphs() {
        let mut pipeline = small_pipeline();
        // 组合字符簇须能整形并进入整形缓存（渲染簇路径依赖此）。
        let cluster = "e\u{301}";
        let glyphs = pipeline.shape_run(cluster);
        assert!(!glyphs.is_empty(), "combining cluster must shape to glyphs");
        let cached = pipeline.shape_run(cluster);
        assert_eq!(glyphs, cached, "cluster shape must come from cache");
    }

    /// 多字形簇的后续字形带累计水平位置；叠加层只加字面偏移会全部叠回起点。
    #[test]
    fn multi_glyph_cluster_preserves_advance_position() {
        let mut pipeline = small_pipeline();
        let glyphs = pipeline.shape_run("👩‍👩");
        assert!(
            glyphs.len() > 1,
            "ZWJ 序列必须整形为多个字形，实际 {} 个",
            glyphs.len()
        );
        assert!(
            glyphs[1].x_offset >= glyphs[0].w,
            "后续字形必须落在首字形推进之后，实际 x_offset={} w={}",
            glyphs[1].x_offset,
            glyphs[0].w
        );
    }

    #[test]
    fn cjk_mixed_text_shapes_without_panic() {
        let mut pipeline = small_pipeline();
        let glyphs = pipeline.shape_run("A中B");
        assert!(!glyphs.is_empty(), "mixed text must produce glyphs");
    }

    #[test]
    fn cjk_fallback_covers_cjk_scripts_not_just_legacy_ranges() {
        // 旧硬编码区间漏掉扩展区：库判定须覆盖，拉丁须排除，
        // 兼容符号区沿用旧语义保留。
        for character in [
            '中', 'あ', 'ア', '한', 'ㄅ', 'ꆈ', '𠀋', '𗀀', '𘬀', '𛅰', '々', 'Ａ',
        ] {
            assert!(
                needs_cjk_fallback(character),
                "{character:?} (U+{:04X}) must need CJK fallback",
                character as u32
            );
        }
        for character in ['A', 'é', ' ', 'α'] {
            assert!(
                !needs_cjk_fallback(character),
                "{character:?} must not need CJK fallback"
            );
        }
    }

    #[test]
    fn shape_cache_invalidated_by_font_size_change() {
        const SAMPLE_TEXT: &str = "Hello";
        const SCALED_FONT_SIZE: f32 = 28.0;
        let mut pipeline = small_pipeline();
        let before = pipeline.shape_run(SAMPLE_TEXT);
        assert!(!before.is_empty(), "baseline shape must produce glyphs");
        pipeline.set_font_size_in_place(SCALED_FONT_SIZE);
        let after = pipeline.shape_run(SAMPLE_TEXT);
        assert!(!after.is_empty(), "resized shape must produce glyphs");
        assert!(
            advance(&after) > advance(&before),
            "larger font must advance wider (before={}, after={})",
            advance(&before),
            advance(&after)
        );
        let cached = pipeline.shape_run(SAMPLE_TEXT);
        assert_eq!(after, cached, "resized shape must come from cache");
    }

    #[test]
    fn shape_cache_key_distinguishes_font_size() {
        // 同文本不同字号必须命中不同缓存条目：单文本键在字号切换
        // 时串味（旧字号的 glyph_id 与 x 偏移被复用，“d 像 a”类错字）。
        // set_font_size_in_place 本来就清缓存；这里验证键本身携带维度，
        // 即使不清缓存也不会串味。
        let mut pipeline = small_pipeline();
        let before = pipeline.shape_run("Hello");
        assert!(!before.is_empty());
        let key_small = ShapeKey {
            text: "Hello".to_string(),
            font_size_bits: pipeline.font_size.to_bits(),
            raster_scale_bits: pipeline.raster_scale.to_bits(),
            font_id: pipeline.font_id,
            fallback_generation: pipeline.fallback_generation,
            bold: false,
            italic: false,
        };
        pipeline.font_size = 28.0;
        let key_large = ShapeKey {
            text: "Hello".to_string(),
            font_size_bits: pipeline.font_size.to_bits(),
            raster_scale_bits: pipeline.raster_scale.to_bits(),
            font_id: pipeline.font_id,
            fallback_generation: pipeline.fallback_generation,
            bold: false,
            italic: false,
        };
        assert_ne!(key_small, key_large, "shape key must distinguish font size");
        let after = pipeline.shape_run("Hello");
        assert!(
            advance(&after) > advance(&before),
            "larger font must advance wider without reusing stale shaping"
        );
        assert_eq!(
            pipeline.caches.shape_cache.len(),
            2,
            "both sizes must coexist as distinct entries when the cache is not cleared"
        );
        let _ = key_small;
    }

    /// 同一簇的常规、粗体、斜体整形不得共用缓存条目。
    #[test]
    fn shape_cache_key_distinguishes_style() {
        let mut pipeline = small_pipeline();
        let _ = pipeline.shape_run_styled("e\u{301}", false, false);
        let _ = pipeline.shape_run_styled("e\u{301}", true, false);
        let _ = pipeline.shape_run_styled("e\u{301}", false, true);
        let styles: Vec<(bool, bool)> = pipeline
            .caches
            .shape_cache
            .iter()
            .filter(|(key, _)| key.text == "e\u{301}")
            .map(|(key, _)| (key.bold, key.italic))
            .collect();
        assert_eq!(styles.len(), 3, "三种样式必须各占一条缓存");
        assert!(styles.contains(&(false, false)));
        assert!(styles.contains(&(true, false)));
        assert!(styles.contains(&(false, true)));
    }
}
