//! Glyph cache — LRU caches for glyph IDs and shaped runs.
//!
//! Separated from FontPipeline so cache eviction strategies can be
//! unit-tested independently.
use std::num::NonZeroUsize;

use super::{GLYPH_CACHE_CAPACITY, GlyphInfo, GlyphKey};
use lru::LruCache;

/// Collection of LRU caches for fast glyph re-lookup.
///
/// All four caches are evicted together when `clear()` is called
/// (e.g. on font family change).
pub struct GlyphCache {
    /// Full glyph-info cache (keyed by glyph key → rasterized info).
    pub glyph_cache: LruCache<GlyphKey, GlyphInfo>,
    /// Cache for shaped runs (keyed by text string).
    pub shape_cache: LruCache<String, Vec<super::ShapedGlyphInfo>>,
    /// ASCII fast-path: pre-allocated array of glyph IDs for ' '..'~'.
    pub ascii_glyph_ids: [Option<swash::GlyphId>; 128],
    /// Non-ASCII glyph ID lookups (codepoint → glyph_id in primary font).
    pub glyph_id_cache: LruCache<u32, swash::GlyphId>,
    /// CJK glyph resolution (char → final font_id + glyph_id).
    pub cjk_glyph_cache: LruCache<char, (fontdb::ID, swash::GlyphId)>,
}

impl Default for GlyphCache {
    fn default() -> Self {
        Self::new()
    }
}

impl GlyphCache {
    pub fn new() -> Self {
        let cache_cap = NonZeroUsize::new(GLYPH_CACHE_CAPACITY).expect("GLYPH_CACHE_CAPACITY > 0");
        let shape_cache_cap = NonZeroUsize::new(1024).expect("1024 > 0");
        Self {
            glyph_cache: LruCache::new(cache_cap),
            shape_cache: LruCache::new(shape_cache_cap),
            ascii_glyph_ids: [None; 128],
            glyph_id_cache: LruCache::new(cache_cap),
            cjk_glyph_cache: LruCache::new(cache_cap),
        }
    }

    /// Clear all caches (called when font family or system locale changes).
    pub fn clear(&mut self) {
        self.glyph_cache.clear();
        self.shape_cache.clear();
        self.ascii_glyph_ids = [None; 128];
        self.glyph_id_cache.clear();
        self.cjk_glyph_cache.clear();
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
    fn glyph_cache_default_works() {
        let gc: GlyphCache = Default::default();
        assert!(gc.glyph_cache.iter().next().is_none());
    }
}
