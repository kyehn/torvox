//! 字形的 LRU 缓存：glyph id 与整形结果。独立于 FontPipeline 以便单独测试淘汰策略。
use std::num::NonZeroUsize;

use super::{
    GLYPH_CACHE_CAPACITY, GlyphInfo, GlyphKey, OUTLINE_CACHE_CAPACITY, SHAPE_CACHE_CAPACITY,
    STYLE_FACE_CACHE_CAPACITY,
};
use lru::LruCache;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ShapeKey {
    pub text: String,
    pub font_size_bits: u32,
    pub raster_scale_bits: u32,
    pub font_id: Option<fontdb::ID>,
    pub fallback_generation: u64,
}
pub struct GlyphCache {
    pub glyph_cache: LruCache<GlyphKey, GlyphInfo>,
    pub shape_cache: LruCache<ShapeKey, Vec<super::ShapedGlyphInfo>>,
    /// ASCII 快速路径：预分配的 ' '..'~' 字形 id 数组。
    pub ascii_glyph_ids: [Option<swash::GlyphId>; 128],
    pub glyph_id_cache: LruCache<u32, swash::GlyphId>,
    /// CJK 字形解析（字符 → 最终 font_id + glyph_id）。
    pub cjk_glyph_cache: LruCache<char, (fontdb::ID, swash::GlyphId)>,
    /// 同族样式面解析（基础字体、bold、italic）→ 胜出面 id，需合成时为 None。
    /// 缓存可避免逐帧逐单元重跑 fontdb 的族/字重/样式查询：实测样式字形查询
    /// 约 20µs/单元，普通文本仅 0.2µs。
    pub style_face_cache: LruCache<(fontdb::ID, bool, bool), Option<fontdb::ID>>,
    /// 样式面 charmap 查询（面, 码点 → glyph id），避免逐帧重入 `with_face_data`
    /// （字体解压与 charmap 构建）。
    pub style_glyph_id_cache: LruCache<(fontdb::ID, u32), swash::GlyphId>,
    /// 轮廓来源缓存（font id, glyph id, 光栅尺寸 → 是否轮廓）。
    /// swash scaler 构建 + Render 约 20µs/次，缓存后 CJK 解析降至约 0.2µs，
    /// 首屏构建次数从 400 降到约 3。键必须带光栅尺寸：embedded bitmap 只在
    /// 特定尺寸存在，同样 (font, gid) 在不同字号/缩放下结论可能相反。
    pub outline_cache: LruCache<(fontdb::ID, swash::GlyphId, u32), bool>,
}

impl Default for GlyphCache {
    fn default() -> Self {
        Self::new()
    }
}

impl GlyphCache {
    pub fn new() -> Self {
        let cache_cap = NonZeroUsize::new(GLYPH_CACHE_CAPACITY).expect("GLYPH_CACHE_CAPACITY > 0");
        let shape_cache_cap =
            NonZeroUsize::new(SHAPE_CACHE_CAPACITY).expect("SHAPE_CACHE_CAPACITY > 0");
        let style_face_cache_cap =
            NonZeroUsize::new(STYLE_FACE_CACHE_CAPACITY).expect("STYLE_FACE_CACHE_CAPACITY > 0");
        let outline_cache_cap =
            NonZeroUsize::new(OUTLINE_CACHE_CAPACITY).expect("OUTLINE_CACHE_CAPACITY > 0");
        Self {
            glyph_cache: LruCache::new(cache_cap),
            shape_cache: LruCache::new(shape_cache_cap),
            ascii_glyph_ids: [None; 128],
            glyph_id_cache: LruCache::new(cache_cap),
            cjk_glyph_cache: LruCache::new(cache_cap),
            style_face_cache: LruCache::new(style_face_cache_cap),
            style_glyph_id_cache: LruCache::new(style_face_cache_cap),
            outline_cache: LruCache::new(outline_cache_cap),
        }
    }

    pub fn clear(&mut self) {
        self.glyph_cache.clear();
        self.shape_cache.clear();
        self.ascii_glyph_ids = [None; 128];
        self.glyph_id_cache.clear();
        self.cjk_glyph_cache.clear();
        self.style_face_cache.clear();
        self.style_glyph_id_cache.clear();
        self.outline_cache.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn glyph_cache_new_is_empty() {
        let gc = GlyphCache::new();
        assert!(gc.glyph_cache.iter().next().is_none());
    }

    #[test]
    fn glyph_cache_clear_resets_ascii() {
        let mut gc = GlyphCache::new();
        gc.ascii_glyph_ids[65] = Some(42);
        assert!(gc.ascii_glyph_ids[65].is_some());
        gc.clear();
        assert!(gc.ascii_glyph_ids[65].is_none());
    }

    #[test]
    fn style_face_cache_evicts_with_clear() {
        let mut gc = GlyphCache::new();
        gc.style_face_cache
            .put((fontdb::ID::default(), true, false), None);
        gc.style_glyph_id_cache
            .put((fontdb::ID::default(), 65), swash::GlyphId::from(42u16));
        assert!(gc.style_face_cache.len() == 1);
        assert!(gc.style_glyph_id_cache.len() == 1);
        gc.clear();
        assert!(gc.style_face_cache.is_empty());
        assert!(gc.style_glyph_id_cache.is_empty());
    }

    #[test]
    fn outline_cache_evicts_with_clear() {
        let mut gc = GlyphCache::new();
        gc.outline_cache.put(
            (fontdb::ID::default(), swash::GlyphId::from(42u16), 0),
            true,
        );
        assert_eq!(gc.outline_cache.len(), 1);
        gc.clear();
        assert_eq!(gc.outline_cache.len(), 0);
    }

    #[test]
    fn outline_cache_key_distinguishes_raster_size() {
        // 同一 (font, gid) 在不同光栅尺寸下结论可能相反（bitmap strike
        // 只在特定尺寸存在）：键必须区分尺寸，否则缩放后沿用旧结论。
        let mut gc = GlyphCache::new();
        let font = fontdb::ID::default();
        let gid = swash::GlyphId::from(42u16);
        gc.outline_cache.put((font, gid, 100), true);
        gc.outline_cache.put((font, gid, 200), false);
        assert_eq!(gc.outline_cache.len(), 2);
        assert_eq!(gc.outline_cache.get(&(font, gid, 100)), Some(&true));
        assert_eq!(gc.outline_cache.get(&(font, gid, 200)), Some(&false));
    }
}
