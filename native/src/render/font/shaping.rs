//! Text shaping — cosmic-text integration for Unicode ligature and complex script support.
use super::{FontPipeline, ShapedGlyphInfo};

/// Line height as a multiple of font size for cosmic-text Metrics.
const DEFAULT_LINE_HEIGHT_RATIO: f32 = 1.2;

/// Width used for an effectively infinite shaping buffer.
const INFINITE_BUFFER_WIDTH: f32 = 999_999.0;

impl FontPipeline {
    pub fn shape_run(&mut self, text: &str) -> Vec<ShapedGlyphInfo> {
        if text.is_empty() {
            return Vec::new();
        }
        if let Some(cached) = self.caches.shape_cache.get(text) {
            return cached.clone();
        }

        let metrics =
            cosmic_text::Metrics::new(self.font_size, self.font_size * DEFAULT_LINE_HEIGHT_RATIO);
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

        let has_cjk = text
            .chars()
            .any(|c| (c as u32) >= super::CJK_IDEOGRAPHIC_START);
        buffer.set_text(text, &attrs, cosmic_text::Shaping::Advanced, None);
        if has_cjk && !self.cjk_fallback_ids.is_empty() {
            let db = self.font_system.db();
            let mut list = cosmic_text::AttrsList::new(&attrs);
            for &fallback_id in &self.cjk_fallback_ids {
                if let Some(face) = db.face(fallback_id)
                    && let Some((fallback_name, _)) = face.families.first()
                {
                    list.add_span(
                        0..text.len(),
                        &cosmic_text::Attrs::new().family(cosmic_text::Family::Name(fallback_name)),
                    );
                }
            }
            for line in &mut buffer.lines {
                line.set_attrs_list(list.clone());
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
        self.caches
            .shape_cache
            .put(text.to_string(), result.clone());
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const FIXTURE_DIR: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../test_fonts");

    fn fixture() -> FontPipeline {
        FontPipeline::from_fixture(512, 512, 12.0, FIXTURE_DIR)
    }

    #[test]
    fn empty_text_shapes_to_nothing() {
        let mut pipeline = fixture();
        assert!(pipeline.shape_run("").is_empty());
    }

    #[test]
    fn ascii_text_produces_glyphs() {
        let mut pipeline = fixture();
        let glyphs = pipeline.shape_run("Hello");
        assert!(!glyphs.is_empty(), "ASCII 'Hello' must shape to glyphs");
        #[allow(clippy::excessive_precision)]
        let mut prev_x = 0.0f32;
        for g in &glyphs {
            assert!(g.x >= prev_x, "glyph x must not go backwards");
            prev_x = g.x;
        }
    }

    #[test]
    fn shaping_results_are_cached() {
        let mut pipeline = fixture();
        let first = pipeline.shape_run("cache me");
        assert!(!first.is_empty());
        let second = pipeline.shape_run("cache me");
        assert_eq!(first, second, "cached shape must equal first shape");
    }

    #[test]
    fn cjk_mixed_text_shapes_without_panic() {
        let mut pipeline = fixture();
        // CJK triggers the fallback path; with or without CJK fonts in the
        // fixture dir, shaping must not panic and must return glyphs for
        // the ASCII part.
        let glyphs = pipeline.shape_run("A中B");
        assert!(!glyphs.is_empty(), "mixed text must produce glyphs");
    }
}
