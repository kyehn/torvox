//! 文本整形：接入 cosmic-text 以支持连字与复杂文种。
use super::{FontPipeline, ShapedGlyphInfo, glyph_cache::ShapeKey};

/// cosmic-text Metrics 的行高倍数。
const DEFAULT_LINE_HEIGHT_RATIO: f32 = 1.2;

/// 等效无限的整形缓冲宽度。
const INFINITE_BUFFER_WIDTH: f32 = 999_999.0;

impl FontPipeline {
    pub fn shape_run(&mut self, text: &str) -> Vec<ShapedGlyphInfo> {
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
            let mut b = cosmic_text::Buffer::new_empty(metrics);
            b.set_size(Some(INFINITE_BUFFER_WIDTH), None);
            b
        });
        buffer.set_metrics(metrics);
        buffer.set_size(Some(INFINITE_BUFFER_WIDTH), None);

        let family_name = self.default_font_name();
        let family = if family_name.is_empty() {
            cosmic_text::Family::Monospace
        } else {
            cosmic_text::Family::Name(&family_name)
        };
        let attrs = cosmic_text::Attrs::new().family(family);

        buffer.set_text(text, &attrs, cosmic_text::Shaping::Advanced, None);
        if !self.cjk_fallback_ids.is_empty() {
            // 只对真正的 CJK 段挂回退族：整段 0..len 会让 "hello中文" 的拉丁
            // 部分也走回退整形（多花开销且 IME 命中不了缓存）。
            let mut cjk_ranges: Vec<std::ops::Range<usize>> = Vec::new();
            let mut start: Option<usize> = None;
            for (idx, ch) in text.char_indices() {
                let cp = ch as u32;
                let is_cjk = matches!(
                    cp,
                    0x1100..=0x11FF
                        | 0x3000..=0x303F
                        | 0x3040..=0x309F
                        | 0x30A0..=0x30FF
                        | 0x3100..=0x312F
                        | 0x3400..=0x4DBF
                        | 0x4E00..=0x9FFF
                        | 0xAC00..=0xD7AF
                        | 0xF900..=0xFAFF
                        | 0xFE30..=0xFE4F
                        | 0xFF00..=0xFFEF
                );
                if is_cjk {
                    if start.is_none() {
                        start = Some(idx);
                    }
                } else if let Some(s) = start.take() {
                    cjk_ranges.push(s..idx);
                }
            }
            if let Some(s) = start {
                cjk_ranges.push(s..text.len());
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
                                    .family(cosmic_text::Family::Name(fallback_name)),
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
                x_offset: glyph.x_offset,
                y_offset: glyph.y_offset,
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
        let advance = |glyphs: &[ShapedGlyphInfo]| glyphs.iter().map(|g| g.w).sum::<f32>();
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

    #[test]
    fn cjk_mixed_text_shapes_without_panic() {
        let mut pipeline = small_pipeline();
        let glyphs = pipeline.shape_run("A中B");
        assert!(!glyphs.is_empty(), "mixed text must produce glyphs");
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
        let before_advance: f32 = before.iter().map(|shaped| shaped.w).sum();
        let after_advance: f32 = after.iter().map(|shaped| shaped.w).sum();
        assert!(
            after_advance > before_advance,
            "larger font must advance wider (before={before_advance}, after={after_advance})"
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
        };
        pipeline.font_size = 28.0;
        let key_large = ShapeKey {
            text: "Hello".to_string(),
            font_size_bits: pipeline.font_size.to_bits(),
            raster_scale_bits: pipeline.raster_scale.to_bits(),
            font_id: pipeline.font_id,
            fallback_generation: pipeline.fallback_generation,
        };
        assert_ne!(key_small, key_large, "shape key must distinguish font size");
        let after = pipeline.shape_run("Hello");
        let before_advance: f32 = before.iter().map(|shaped| shaped.w).sum();
        let after_advance: f32 = after.iter().map(|shaped| shaped.w).sum();
        assert!(
            after_advance > before_advance,
            "larger font must advance wider without reusing stale shaping"
        );
        assert_eq!(
            pipeline.caches.shape_cache.len(),
            2,
            "both sizes must coexist as distinct entries when the cache is not cleared"
        );
        let _ = key_small;
    }
}
