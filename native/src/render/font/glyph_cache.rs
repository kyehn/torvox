//! 字形的 LRU 缓存：glyph id 与整形结果。独立于 FontPipeline 以便单独测试淘汰策略。
use std::num::NonZeroUsize;

use super::{
    ASCII_UPPER_BOUND, GLYPH_CACHE_CAPACITY, GlyphInfo, GlyphKey, OUTLINE_CACHE_CAPACITY,
    SHAPE_CACHE_CAPACITY, STYLE_FACE_CACHE_CAPACITY, STYLE_GLYPH_ID_CACHE_CAPACITY,
    UNRESOLVABLE_CACHE_CAPACITY,
};
use lru::LruCache;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ShapeKey {
    pub text: String,
    pub font_size_bits: u32,
    pub raster_scale_bits: u32,
    pub font_id: Option<fontdb::ID>,
    pub fallback_generation: u64,
    pub bold: bool,
    pub italic: bool,
}
pub struct GlyphCache {
    pub glyph_cache: LruCache<GlyphKey, GlyphInfo>,
    pub shape_cache: LruCache<ShapeKey, Vec<super::ShapedGlyphInfo>>,
    /// ASCII 快速路径：预分配的 ' '..'~' 字形 id 数组。
    pub ascii_glyph_ids: [Option<swash::GlyphId>; ASCII_UPPER_BOUND as usize],
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
    /// 「库内无面能解析此码位」的负缓存：值恒为 `()`，只用作集合。
    ///
    /// 链条走到全库扫描仍找不到时记入，此后跳过一次「遍历 fontdb 全部 face 并逐个
    /// `with_face_data` 解压」的扫描——罕见汉字每帧一次是实打实的开销。
    ///
    /// 与 [`Self::cjk_glyph_cache`] 同批失效（`clear_identity_caches`，即换字体/
    /// 换 locale/换字号三处），故新增字体后不会留下永久误判。
    pub unresolvable_glyphs: LruCache<char, ()>,
    /// 实际供出过字形的面，按首次命中顺序去重。`FontPipeline::font_info` 据此上报
    /// 「其他被使用字体」——符号族经全库扫描参与渲染，却不在 `cjk_fallback_ids` 里，
    /// 只按回退层上报会让实际渲染所用的面全部隐形。
    ///
    /// 与 `cjk_glyph_cache` 同批失效：换字体/换 locale/换字号后旧结论不再成立。
    /// 元素个数等于「当前库里有面的族数」，设备上常驻 3 个族（见 `FontPipeline::new`），
    /// 用户换族与 locale 补装各自追加，故线性查找的成本可忽略。
    pub used_faces: Vec<fontdb::ID>,
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
        let style_glyph_id_cap = NonZeroUsize::new(STYLE_GLYPH_ID_CACHE_CAPACITY)
            .expect("STYLE_GLYPH_ID_CACHE_CAPACITY > 0");
        let outline_cache_cap =
            NonZeroUsize::new(OUTLINE_CACHE_CAPACITY).expect("OUTLINE_CACHE_CAPACITY > 0");
        let unresolvable_cap = NonZeroUsize::new(UNRESOLVABLE_CACHE_CAPACITY)
            .expect("UNRESOLVABLE_CACHE_CAPACITY > 0");
        Self {
            glyph_cache: LruCache::new(cache_cap),
            shape_cache: LruCache::new(shape_cache_cap),
            ascii_glyph_ids: [None; ASCII_UPPER_BOUND as usize],
            glyph_id_cache: LruCache::new(cache_cap),
            cjk_glyph_cache: LruCache::new(cache_cap),
            style_face_cache: LruCache::new(style_face_cache_cap),
            style_glyph_id_cache: LruCache::new(style_glyph_id_cap),
            outline_cache: LruCache::new(outline_cache_cap),
            unresolvable_glyphs: LruCache::new(unresolvable_cap),
            used_faces: Vec::new(),
        }
    }

    /// 登记一个实际供出过字形的面。只在字形解析成功后调用（见
    /// `FontPipeline::glyph_information_from_font_with_synthesis`），故列表里
    /// 不会混进「试过但没出图」的面。
    pub fn note_used_face(&mut self, face: fontdb::ID) {
        if !self.used_faces.contains(&face) {
            self.used_faces.push(face);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn glyph_cache_new_is_empty() {
        let glyph_cache = GlyphCache::new();
        assert!(glyph_cache.glyph_cache.iter().next().is_none());
    }

    /// 模块存在的理由就是淘汰策略：容量一满就必须真的丢掉最久未用的那条，
    /// 否则字形缓存会随输入无界增长（终端内存的主要消耗方）。
    #[test]
    fn outline_cache_drops_the_least_recently_used_entry_at_capacity() {
        let mut glyph_cache = GlyphCache::new();
        let key_for = |raster_size_bits: u32| {
            (
                fontdb::ID::default(),
                swash::GlyphId::from(42u16),
                raster_size_bits,
            )
        };
        for index in 0..OUTLINE_CACHE_CAPACITY {
            glyph_cache.outline_cache.put(key_for(index as u32), true);
            assert_eq!(glyph_cache.outline_cache.len(), index + 1);
        }
        // `get` 刷新最近使用次序（`peek` 不刷新）；压入新条目时被淘汰的应是
        // 「上一步之前最旧」的那条，也就是 size 1（size 0 已被提到最新）。
        assert!(glyph_cache.outline_cache.get(&key_for(0)).is_some());
        glyph_cache.outline_cache.put(key_for(u32::MAX), true);
        assert_eq!(glyph_cache.outline_cache.len(), OUTLINE_CACHE_CAPACITY);
        assert!(
            glyph_cache.outline_cache.peek(&key_for(0)).is_some(),
            "刚被触碰的条目不得被淘汰"
        );
        assert!(
            glyph_cache.outline_cache.peek(&key_for(1)).is_none(),
            "最久未用的条目必须被淘汰"
        );
    }

    #[test]
    fn outline_cache_key_distinguishes_raster_size() {
        // 同一 (font, gid) 在不同光栅尺寸下结论可能相反（bitmap strike
        // 只在特定尺寸存在）：键必须区分尺寸，否则缩放后沿用旧结论。
        let mut glyph_cache = GlyphCache::new();
        let font = fontdb::ID::default();
        let gid = swash::GlyphId::from(42u16);
        glyph_cache.outline_cache.put((font, gid, 100), true);
        glyph_cache.outline_cache.put((font, gid, 200), false);
        assert_eq!(glyph_cache.outline_cache.len(), 2);
        assert_eq!(
            glyph_cache.outline_cache.get(&(font, gid, 100)),
            Some(&true)
        );
        assert_eq!(
            glyph_cache.outline_cache.get(&(font, gid, 200)),
            Some(&false)
        );
    }
}
