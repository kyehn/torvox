//! Glyph atlas — packing rasterized glyphs into GPU texture.
use swash::scale::{Render, Source};
use swash::zeno::{Placement, Transform};

use super::{FontPipeline, GlyphInfo, GlyphKey, GlyphSynthesis};

pub(super) const GLYPH_CACHE_EVICTION_DIVISOR: usize = 4;

/// Italic shear slope: tan(12°) — the classic synthetic-italic angle.
const ITALIC_SHEAR: f32 = 0.2126;

/// Faux-bold strength as a fraction of the em size (pixels). 4% matches
/// common text-renderer defaults (FreeType's bold strength is ~4.5%).
const BOLD_STRENGTH_EM: f32 = 0.04;

impl FontPipeline {
    pub(crate) fn glyph_information_from_font(
        &mut self,
        font_id: fontdb::ID,
        _ch: char,
        glyph_id: swash::GlyphId,
    ) -> Option<GlyphInfo> {
        self.glyph_information_from_font_with_synthesis(font_id, glyph_id, GlyphSynthesis::None)
    }

    /// Style-aware glyph lookup: when the requested synthesis
    /// is bold/italic and the primary font has no matching face, the alpha
    /// mask is post-processed — embolden for bold, shear for italic. When a
    /// matching bold/italic face exists (e.g. Roboto-Bold.ttf), the caller
    /// resolves it first via [FontPipeline::resolve_style_face] and passes
    /// [GlyphSynthesis::None] with that face id.
    pub(crate) fn glyph_information_from_font_with_synthesis(
        &mut self,
        font_id: fontdb::ID,
        glyph_id: swash::GlyphId,
        synthesis: GlyphSynthesis,
    ) -> Option<GlyphInfo> {
        let key = GlyphKey {
            font_id,
            glyph_id,
            pixel_size: (self.font_size * self.raster_scale) as u16,
            synthesis: synthesis.bits(),
        };

        if let Some(info) = self.caches.glyph_cache.get(&key).cloned() {
            return Some(info);
        }

        let db = self.font_system.db();
        let font_size = self.font_size;
        let raster_size = font_size * self.raster_scale;
        let pair = db.with_face_data(font_id, |font_data, face_index| -> Option<(_, f32)> {
            let font_ref = swash::FontRef::from_index(font_data, face_index as usize)?;
            let mut scaler = self
                .scaler_context
                .builder(font_ref)
                .size(raster_size)
                // Hinting aligns TrueType stems to the pixel grid. With a
                // raster_scale > 1 (device density) the bitmap is large
                // enough that hinting is unnecessary and can distort the
                // glyph shapes: emulator OCR failed on hinted
                // 124px bitmaps); hint only when rendering at 1:1.
                .hint(self.raster_scale <= 1.01)
                .build();
            let image = {
                let mut render = Render::new(&[Source::Outline]);
                // font synthesis at the outline level (swash
                // native) — faux bold via embolden(), synthetic italic via
                // an affine shear, both applied while rasterizing so the
                // anti-aliasing quality is preserved.
                if matches!(synthesis, GlyphSynthesis::Bold | GlyphSynthesis::BoldItalic) {
                    render.embolden(raster_size * BOLD_STRENGTH_EM);
                }
                if matches!(
                    synthesis,
                    GlyphSynthesis::Italic | GlyphSynthesis::BoldItalic
                ) {
                    // Shear x' = x + y * slope (top rows lean right).
                    render.transform(Some(Transform::new(1.0, 0.0, ITALIC_SHEAR, 1.0, 0.0, 0.0)));
                }
                render.render(&mut scaler, glyph_id)
            };
            let upem = font_ref.metrics(&[]).units_per_em as f32;
            let scale = if upem > 0.0 {
                font_size / upem
            } else {
                font_size
            };
            let advance_width = font_ref.glyph_metrics(&[]).advance_width(glyph_id) * scale;
            Some((image, advance_width))
        })?;
        let (image, advance_width) = pair?;

        let image = match image {
            Some(img) => img,
            None => {
                let info = GlyphInfo {
                    atlas_x: 0,
                    atlas_y: 0,
                    width: 0,
                    height: 0,
                    placement: Placement::default(),
                    advance_width,
                    allocation_id: None,
                };
                self.caches.glyph_cache.put(key, info.clone());
                return Some(info);
            }
        };

        // font synthesis is applied at the outline level by
        // the Render builder above (embolden/shear); the image returned here
        // is already styled.
        let width = image.placement.width as i32;
        let height = image.placement.height as i32;

        if width == 0 || height == 0 {
            let info = GlyphInfo {
                atlas_x: 0,
                atlas_y: 0,
                width: 0,
                height: 0,
                placement: image.placement,
                advance_width,
                allocation_id: None,
            };
            self.caches.glyph_cache.put(key, info.clone());
            return Some(info);
        }

        let allocation = match self
            .atlas
            .allocate(guillotiere::size2(width + 1, height + 1))
        {
            Some(a) => a,
            None => {
                let evict_count =
                    (self.caches.glyph_cache.len() / GLYPH_CACHE_EVICTION_DIVISOR).max(1);
                for _ in 0..evict_count {
                    if let Some((_, evicted)) = self.caches.glyph_cache.pop_lru()
                        && let Some(allocated_id) = evicted.allocation_id
                    {
                        self.atlas.deallocate(allocated_id);
                    }
                }
                if let Some(a) = self
                    .atlas
                    .allocate(guillotiere::size2(width + 1, height + 1))
                {
                    a
                } else {
                    log::warn!(
                        "ATLAS_REBUILD: atlas full ({}x{}), rebuilding with {} cached glyphs",
                        self.atlas_width,
                        self.atlas_height,
                        self.caches.glyph_cache.len(),
                    );
                    self.rebuild_atlas();
                    self.atlas
                        .allocate(guillotiere::size2(width + 1, height + 1))?
                }
            }
        };
        let rect = allocation.rectangle;
        let allocation_id = Some(allocation.id);
        let ax = rect.min.x as u32;
        let ay = rect.min.y as u32;

        if width > 0 && height > 0 {
            let gw = width as u32;
            let gh = height as u32;
            match &mut self.dirty_rect {
                Some((dx, dy, dw, dh)) => {
                    let cx2 = (*dx + *dw).max(ax + gw);
                    let cy2 = (*dy + *dh).max(ay + gh);
                    *dx = (*dx).min(ax);
                    *dy = (*dy).min(ay);
                    *dw = cx2 - *dx;
                    *dh = cy2 - *dy;
                }
                None => {
                    self.dirty_rect = Some((ax, ay, gw, gh));
                }
            }
        }

        match image.content {
            swash::scale::image::Content::Mask => {
                let atlas_w = self.atlas_width as usize;
                let atlas_h = self.atlas_height as usize;
                for y in 0..height as usize {
                    let dst_y = ay as usize + y;
                    if dst_y >= atlas_h {
                        break;
                    }
                    for x in 0..width as usize {
                        let src_idx = y * width as usize + x;
                        let alpha = image.data.get(src_idx).copied().unwrap_or(0);
                        let dst_x = ax as usize + x;
                        if dst_x >= atlas_w {
                            break;
                        }
                        let dst_idx = (dst_y * atlas_w + dst_x) * 4;
                        if dst_idx + 3 < self.atlas_bitmap.len() {
                            self.atlas_bitmap[dst_idx] = alpha;
                            self.atlas_bitmap[dst_idx + 1] = alpha;
                            self.atlas_bitmap[dst_idx + 2] = alpha;
                            self.atlas_bitmap[dst_idx + 3] = alpha;
                        }
                    }
                }
            }
            _ => {
                let atlas_w = self.atlas_width as usize;
                let atlas_h = self.atlas_height as usize;
                let bpp = 4;
                for y in 0..height as usize {
                    let dst_y = ay as usize + y;
                    if dst_y >= atlas_h {
                        break;
                    }
                    for x in 0..width as usize {
                        let dst_x = ax as usize + x;
                        if dst_x >= atlas_w {
                            break;
                        }
                        let src_idx = (y * width as usize + x) * bpp;
                        let dst_idx = (dst_y * atlas_w + dst_x) * 4;
                        if dst_idx + 3 < self.atlas_bitmap.len() && src_idx + 3 < image.data.len() {
                            let alpha = image.data[src_idx + 3];
                            self.atlas_bitmap[dst_idx] = alpha;
                            self.atlas_bitmap[dst_idx + 1] = alpha;
                            self.atlas_bitmap[dst_idx + 2] = alpha;
                            self.atlas_bitmap[dst_idx + 3] = 255;
                        }
                    }
                }
            }
        }

        let info = GlyphInfo {
            atlas_x: ax as i32,
            atlas_y: ay as i32,
            width,
            height,
            placement: image.placement,
            advance_width,
            allocation_id,
        };

        self.caches.glyph_cache.put(key, info.clone());
        self.atlas_generation += 1;
        Some(info)
    }

    pub(super) fn rebuild_atlas(&mut self) {
        let entries: Vec<(GlyphKey, GlyphInfo)> = self
            .caches
            .glyph_cache
            .iter()
            .map(|(&k, v)| (k, v.clone()))
            .collect();
        self.atlas = guillotiere::AtlasAllocator::new(guillotiere::size2(
            self.atlas_width as i32,
            self.atlas_height as i32,
        ));
        self.atlas_bitmap.fill(0);
        self.caches.glyph_cache.clear();
        for (key, _old_info) in &entries {
            self.glyph_information_from_font(key.font_id, '\0', key.glyph_id);
        }
        self.atlas_generation = self.atlas_generation.saturating_add(1);
        self.reset_dirty_rect_full();
    }

    pub fn atlas_generation(&self) -> u64 {
        self.atlas_generation
    }

    pub fn take_dirty_rect(&mut self) -> Option<(u32, u32, u32, u32)> {
        self.dirty_rect.take()
    }

    pub fn reset_dirty_rect_full(&mut self) {
        self.dirty_rect = Some((0, 0, self.atlas_width, self.atlas_height));
    }

    pub fn cache_length(&self) -> usize {
        self.caches.glyph_cache.len()
    }

    pub fn atlas_bitmap(&self) -> &[u8] {
        &self.atlas_bitmap
    }

    pub fn atlas_dimensions(&self) -> (u32, u32) {
        (self.atlas_width, self.atlas_height)
    }
}
