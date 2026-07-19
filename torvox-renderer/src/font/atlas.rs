//! Glyph atlas — packing rasterized glyphs into GPU texture.
use swash::scale::{Render, Source};
use swash::zeno::Placement;

use super::{FontPipeline, GlyphInfo, GlyphKey};

pub(super) const GLYPH_CACHE_EVICTION_DIVISOR: usize = 4;

impl FontPipeline {
    pub(crate) fn glyph_information_from_font(
        &mut self,
        font_id: fontdb::ID,
        _ch: char,
        glyph_id: swash::GlyphId,
    ) -> Option<GlyphInfo> {
        let key = GlyphKey {
            font_id,
            glyph_id,
            pixel_size: (self.font_size * self.raster_scale) as u16,
        };

        if let Some(info) = self.glyph_cache.get(&key).cloned() {
            return Some(info);
        }

        let db = self.font_system.db();
        let font_size = self.font_size;
        let raster_size = font_size * self.raster_scale;
        let (image, advance_width) =
            db.with_face_data(font_id, |font_data, face_index| -> Option<(_, f32)> {
                let font_ref = swash::FontRef::from_index(font_data, face_index as usize)?;
                let mut scaler = self
                    .scaler_context
                    .builder(font_ref)
                    .size(raster_size)
                    .hint(false)
                    .build();
                let image = Render::new(&[Source::Outline]).render(&mut scaler, glyph_id);
                let upem = font_ref.metrics(&[]).units_per_em as f32;
                let scale = if upem > 0.0 {
                    font_size / upem
                } else {
                    font_size
                };
                let advance_width = font_ref.glyph_metrics(&[]).advance_width(glyph_id) * scale;
                Some((image, advance_width))
            })??;

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
                self.glyph_cache.put(key, info.clone());
                return Some(info);
            }
        };

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
            self.glyph_cache.put(key, info.clone());
            return Some(info);
        }

        let allocation = match self
            .atlas
            .allocate(guillotiere::size2(width + 1, height + 1))
        {
            Some(a) => a,
            None => {
                let evict_count = (self.glyph_cache.len() / GLYPH_CACHE_EVICTION_DIVISOR).max(1);
                for _ in 0..evict_count {
                    if let Some((_, evicted)) = self.glyph_cache.pop_lru()
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
                        self.glyph_cache.len(),
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
                        if dst_idx < self.atlas_bitmap.len() {
                            self.atlas_bitmap[dst_idx] = alpha;
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
                        if dst_idx < self.atlas_bitmap.len() && src_idx + 3 < image.data.len() {
                            self.atlas_bitmap[dst_idx] = image.data[src_idx + 3];
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

        self.glyph_cache.put(key, info.clone());
        self.atlas_generation += 1;
        Some(info)
    }

    pub(super) fn rebuild_atlas(&mut self) {
        let entries: Vec<(GlyphKey, GlyphInfo)> = self
            .glyph_cache
            .iter()
            .map(|(k, v)| (*k, v.clone()))
            .collect();
        self.atlas = guillotiere::AtlasAllocator::new(guillotiere::size2(
            self.atlas_width as i32,
            self.atlas_height as i32,
        ));
        self.atlas_bitmap.fill(0);
        self.glyph_cache.clear();
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
        self.glyph_cache.len()
    }

    pub fn atlas_bitmap(&self) -> &[u8] {
        &self.atlas_bitmap
    }

    pub fn atlas_dimensions(&self) -> (u32, u32) {
        (self.atlas_width, self.atlas_height)
    }
}
