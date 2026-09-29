//! 字形图集：把光栅化后的字形打包进 GPU 纹理。
use swash::scale::{Render, Source};
use swash::zeno::Transform;

use super::{FontPipeline, GlyphInfo, GlyphKey, GlyphSynthesis};

pub(super) const GLYPH_CACHE_EVICTION_DIVISOR: usize = 4;

/// 图集每个像素的字节数（RGBA）。
pub(crate) const ATLAS_BYTES_PER_PIXEL: usize = 4;

/// 图集像素内红色通道偏移。
const ATLAS_RED_OFFSET: usize = 0;

/// 图集像素内绿色通道偏移。
const ATLAS_GREEN_OFFSET: usize = 1;

/// 图集像素内蓝色通道偏移。
const ATLAS_BLUE_OFFSET: usize = 2;

/// 图集像素内透明通道偏移。
const ATLAS_ALPHA_OFFSET: usize = 3;

/// 全不透明 alpha 值。
const OPAQUE_ALPHA: u8 = 255;

/// 斜体剪切斜率：tan(12°)，合成斜体的经典角度。
const ITALIC_SHEAR: f32 = 0.2126;

/// 伪粗体强度（占 em 尺寸的像素比例），4% 与常见渲染器默认一致（FreeType 约 4.5%）。
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

    /// 样式感知字形查询：请求合成 bold/italic 而主字体无匹配面时对 alpha 遮罩做后处理
    /// （bold 加粗、italic 剪切）；若存在匹配面（如 Roboto-Bold.ttf），调用方先用
    /// [FontPipeline::resolve_style_face] 解析出该面并传 [GlyphSynthesis::None]。
    pub(crate) fn glyph_information_from_font_with_synthesis(
        &mut self,
        font_id: fontdb::ID,
        glyph_id: swash::GlyphId,
        synthesis: GlyphSynthesis,
    ) -> Option<GlyphInfo> {
        if let Some(info) = self.lookup_glyph(font_id, glyph_id, synthesis) {
            return Some(info);
        }

        let key = GlyphKey {
            font_id,
            glyph_id,
            raster_size_bits: super::raster_size_key(self.font_size * self.raster_scale),
            synthesis: synthesis.bits(),
        };

        let font_database = self.font_system.db();
        let font_size = self.font_size;
        let raster_size = font_size * self.raster_scale;
        let pair =
            font_database.with_face_data(font_id, |font_data, face_index| -> Option<(_, f32)> {
                let font_ref = swash::FontRef::from_index(font_data, face_index as usize)?;
                // 可变字体优先走 `wght`/`ital` 轴（真字重/真倾斜）；字体未声明该轴
                // 时才退回轮廓级合成。轴必须在 builder 之前设定才生效。
                let variations = super::variation_settings(&font_ref, synthesis);
                let mut scaler = self
                    .scaler_context
                    .builder(font_ref)
                    .size(raster_size)
                    // Hinting 把 TrueType 竖干对齐到像素网格；raster_scale > 1（设备密度）时
                    // 位图已足够大，hinting 只会扭曲字形（模拟器 OCR 在 124px hinting 位图上
                    // 失败），故仅在 1:1 渲染时启用。
                    .hint(self.raster_scale <= 1.01)
                    .variations(variations.clone())
                    .build();
                let image = {
                    let mut render = Render::new(&[Source::Outline]);
                    // 轮廓级合成（swash 原生）：bold 用 embolden()，italic 用仿射剪切，
                    // 在光栅化时应用以保留抗锯齿质量。已由轴表达的部分不重复合成。
                    let axis_has = |tag: &[u8; 4]| {
                        variations
                            .iter()
                            .any(|setting| setting.tag == swash::tag_from_bytes(tag))
                    };
                    if matches!(synthesis, GlyphSynthesis::Bold | GlyphSynthesis::BoldItalic)
                        && !axis_has(b"wght")
                    {
                        render.embolden(raster_size * BOLD_STRENGTH_EM);
                    }
                    if matches!(
                        synthesis,
                        GlyphSynthesis::Italic | GlyphSynthesis::BoldItalic
                    ) && !axis_has(b"ital")
                    {
                        // 剪切 x' = x + y * slope（顶部行向右倾）。
                        render.transform(Some(Transform::new(
                            1.0,
                            0.0,
                            ITALIC_SHEAR,
                            1.0,
                            0.0,
                            0.0,
                        )));
                    }
                    render.render(&mut scaler, glyph_id)
                };
                let upem = font_ref.metrics(&[]).units_per_em as f32;
                let scale = if upem > 0.0 {
                    font_size / upem
                } else {
                    font_size
                };
                // `glyph_metrics(&[])` 不带轴坐标，VF 加粗后前伸宽度会偏小；
                // 轴生效时改用缩放后轮廓的水平范围（swash 未公开带坐标的
                // GlyphMetrics 构造）。等宽网格布局本身不消费该值，仅供诊断。
                let advance_width = if variations.is_empty() {
                    font_ref.glyph_metrics(&[]).advance_width(glyph_id) * scale
                } else {
                    scaler.scale_outline(glyph_id).map_or(0.0, |outline| {
                        let bounds = outline.bounds();
                        (bounds.max.x - bounds.min.x) * scale
                    })
                };
                Some((image, advance_width))
            })?;
        let (image, advance_width) = pair?;

        let image = match image {
            Some(img) => img,
            None => {
                // 光栅无输出：返回 None 让上层回退链继续试下一层字体。
                // 不得缓存零尺寸占位——零尺寸 Some 入库后毒化后续所有帧
                //（快路直接命中返回空白，整批同字形持续消失）。
                return None;
            }
        };

        let width = image.placement.width as i32;
        let height = image.placement.height as i32;

        if width == 0 || height == 0 {
            return None;
        }

        let allocation = match self
            .atlas
            .allocate(guillotiere::size2(width + 1, height + 1))
        {
            Some(a) => a,
            None => {
                let evict_count =
                    (self.caches.glyph_cache.len() / GLYPH_CACHE_EVICTION_DIVISOR).max(1);
                let mut evicted_any = false;
                for _ in 0..evict_count {
                    if let Some((_, evicted)) = self.caches.glyph_cache.pop_lru()
                        && let Some(allocated_id) = evicted.allocation_id
                    {
                        self.atlas.deallocate(allocated_id);
                        evicted_any = true;
                    }
                }
                // 被驱逐的区域会分给后续分配，已缓存的单元实例仍引用旧 UV，
                // 故须重建实例缓存（见 atlas_generation）。
                if evicted_any {
                    self.atlas_generation = self.atlas_generation.wrapping_add(1);
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
        let origin_x = rect.min.x as u32;
        let origin_y = rect.min.y as u32;

        if width > 0 && height > 0 {
            let glyph_width = width as u32;
            let glyph_height = height as u32;
            match &mut self.dirty_rect {
                Some((dirty_x, dirty_y, dirty_width, dirty_height)) => {
                    let combined_max_x = (*dirty_x + *dirty_width).max(origin_x + glyph_width);
                    let combined_max_y = (*dirty_y + *dirty_height).max(origin_y + glyph_height);
                    *dirty_x = (*dirty_x).min(origin_x);
                    *dirty_y = (*dirty_y).min(origin_y);
                    *dirty_width = combined_max_x - *dirty_x;
                    *dirty_height = combined_max_y - *dirty_y;
                }
                None => {
                    self.dirty_rect = Some((origin_x, origin_y, glyph_width, glyph_height));
                }
            }
        }

        match image.content {
            swash::scale::image::Content::Mask => {
                let atlas_width_pixels = self.atlas_width as usize;
                let atlas_height_pixels = self.atlas_height as usize;
                for offset_y in 0..height as usize {
                    let destination_y = origin_y as usize + offset_y;
                    if destination_y >= atlas_height_pixels {
                        break;
                    }
                    for offset_x in 0..width as usize {
                        let source_index = offset_y * width as usize + offset_x;
                        let alpha = image.data.get(source_index).copied().unwrap_or(0);
                        let destination_x = origin_x as usize + offset_x;
                        if destination_x >= atlas_width_pixels {
                            break;
                        }
                        let destination_index = (destination_y * atlas_width_pixels
                            + destination_x)
                            * ATLAS_BYTES_PER_PIXEL;
                        if destination_index + ATLAS_ALPHA_OFFSET < self.atlas_bitmap.len() {
                            self.atlas_bitmap[destination_index + ATLAS_RED_OFFSET] = alpha;
                            self.atlas_bitmap[destination_index + ATLAS_GREEN_OFFSET] = alpha;
                            self.atlas_bitmap[destination_index + ATLAS_BLUE_OFFSET] = alpha;
                            self.atlas_bitmap[destination_index + ATLAS_ALPHA_OFFSET] = alpha;
                        }
                    }
                }
            }
            _ => {
                let atlas_width_pixels = self.atlas_width as usize;
                let atlas_height_pixels = self.atlas_height as usize;
                for offset_y in 0..height as usize {
                    let destination_y = origin_y as usize + offset_y;
                    if destination_y >= atlas_height_pixels {
                        break;
                    }
                    for offset_x in 0..width as usize {
                        let destination_x = origin_x as usize + offset_x;
                        if destination_x >= atlas_width_pixels {
                            break;
                        }
                        let source_index =
                            (offset_y * width as usize + offset_x) * ATLAS_BYTES_PER_PIXEL;
                        let destination_index = (destination_y * atlas_width_pixels
                            + destination_x)
                            * ATLAS_BYTES_PER_PIXEL;
                        if destination_index + ATLAS_ALPHA_OFFSET < self.atlas_bitmap.len()
                            && source_index + ATLAS_ALPHA_OFFSET < image.data.len()
                        {
                            let alpha = image.data[source_index + ATLAS_ALPHA_OFFSET];
                            self.atlas_bitmap[destination_index + ATLAS_RED_OFFSET] = alpha;
                            self.atlas_bitmap[destination_index + ATLAS_GREEN_OFFSET] = alpha;
                            self.atlas_bitmap[destination_index + ATLAS_BLUE_OFFSET] = alpha;
                            self.atlas_bitmap[destination_index + ATLAS_ALPHA_OFFSET] =
                                OPAQUE_ALPHA;
                        }
                    }
                }
            }
        }

        let info = GlyphInfo {
            atlas_x: origin_x as i32,
            atlas_y: origin_y as i32,
            width,
            height,
            placement: image.placement,
            advance_width,
            allocation_id,
        };

        self.caches.glyph_cache.put(key, info.clone());
        // 注意：此处不推进 atlas_generation。新分配只占用空闲区，已有 UV 不变；
        // 代际只在驱逐/重建（真正搬迁 UV）时推进，否则每帧新字形都会误杀增量实例缓存。
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
            let synthesis = GlyphSynthesis::from_bits(key.synthesis);
            self.glyph_information_from_font_with_synthesis(key.font_id, key.glyph_id, synthesis);
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
