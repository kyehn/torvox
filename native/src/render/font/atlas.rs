//! 字形图集：把光栅化后的字形打包进 GPU 纹理。
use swash::scale::{Render, Source};
use swash::zeno::Transform;

use super::{FontPipeline, GlyphInfo, GlyphKey, GlyphSynthesis};

pub(super) const GLYPH_CACHE_EVICTION_DIVISOR: usize = 4;

/// 图集每个像素的字节数（RGBA）。
pub(crate) const ATLAS_BYTES_PER_PIXEL: usize = 4;

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
    /// [`FontPipeline::resolve_style_face`] 解析出该面并传 [`GlyphSynthesis::None`]。
    ///
    /// 本函数是字形真正出图的唯一收口，故在此登记供出字形的面（供 `font_info`
    /// 上报「其他被使用字体」）；登记只在返回 `Some` 的两条路径上发生。
    pub(crate) fn glyph_information_from_font_with_synthesis(
        &mut self,
        font_id: fontdb::ID,
        glyph_id: swash::GlyphId,
        synthesis: GlyphSynthesis,
    ) -> Option<GlyphInfo> {
        if let Some(info) = self.lookup_glyph(font_id, glyph_id, synthesis) {
            self.caches.note_used_face(font_id);
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
        // 外层 None = 读字体表失败（向上抛）；内层 None = 光栅无输出，此时返回
        // None 让上层回退链继续试下一层字体。不得缓存零尺寸占位——零尺寸 Some
        // 入库后毒化后续所有帧（快路直接命中返回空白，整批同字形持续消失）。
        let image = font_database.with_face_data(font_id, |font_data, face_index| {
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
                render.transform(Some(Transform::new(1.0, 0.0, ITALIC_SHEAR, 1.0, 0.0, 0.0)));
            }
            render.render(&mut scaler, glyph_id)
        })??;

        let width = image.placement.width as i32;
        let height = image.placement.height as i32;

        if width == 0 || height == 0 {
            return None;
        }

        let allocation = if let Some(allocation) =
            self.atlas.allocate(etagere::size2(width + 1, height + 1))
        {
            allocation
        } else {
            let evict_count = (self.caches.glyph_cache.len() / GLYPH_CACHE_EVICTION_DIVISOR).max(1);
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
            if let Some(retry_allocation) =
                self.atlas.allocate(etagere::size2(width + 1, height + 1))
            {
                retry_allocation
            } else {
                log::warn!(
                    "ATLAS_REBUILD: atlas full ({}x{}), rebuilding with {} cached glyphs",
                    self.atlas_width,
                    self.atlas_height,
                    self.caches.glyph_cache.len(),
                );
                self.rebuild_atlas();
                self.atlas.allocate(etagere::size2(width + 1, height + 1))?
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

        // 掩码图（普通字形）与彩色图（上游输出 RGBA）都归一到「透明通道写灰度、
        // 彩色图 alpha 恒不透明」的同一形状，故复制循环只保留一条路径。
        let is_mask = matches!(image.content, swash::scale::image::Content::Mask);
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
                let destination_index =
                    (destination_y * atlas_width_pixels + destination_x) * ATLAS_BYTES_PER_PIXEL;
                let source_index = if is_mask {
                    offset_y * width as usize + offset_x
                } else {
                    (offset_y * width as usize + offset_x) * ATLAS_BYTES_PER_PIXEL
                };
                let alpha = image.data.get(source_index).copied().unwrap_or(0);
                let pixel = if is_mask {
                    [alpha, alpha, alpha, alpha]
                } else {
                    let alpha = image
                        .data
                        .get(source_index + ATLAS_ALPHA_OFFSET)
                        .copied()
                        .unwrap_or(0);
                    [alpha, alpha, alpha, OPAQUE_ALPHA]
                };
                if destination_index + ATLAS_ALPHA_OFFSET >= self.atlas_bitmap.len() {
                    continue;
                }
                self.atlas_bitmap[destination_index..destination_index + ATLAS_BYTES_PER_PIXEL]
                    .copy_from_slice(&pixel);
            }
        }

        let info = GlyphInfo {
            atlas_x: origin_x as i32,
            atlas_y: origin_y as i32,
            width,
            height,
            placement: image.placement,
            allocation_id,
        };

        self.caches.glyph_cache.put(key, info.clone());
        self.caches.note_used_face(font_id);
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
        self.atlas = etagere::AtlasAllocator::new(etagere::size2(
            self.atlas_width as i32,
            self.atlas_height as i32,
        ));
        self.atlas_bitmap.fill(0);
        self.caches.glyph_cache.clear();
        // 重建后逐个字形重新入图集，每次入图都登记脏区：先清掉旧脏区，让脏区正好
        // 等于重新落位的字形包围盒，而不是整幅图集（2048×2048×4B = 16MB 的白传）。
        self.dirty_rect = None;
        for (key, _old_info) in &entries {
            let synthesis = GlyphSynthesis::from_bits(key.synthesis);
            self.glyph_information_from_font_with_synthesis(key.font_id, key.glyph_id, synthesis);
        }
        // 与上面驱逐路径同为 wrapping_add：代际是「能否复用已缓存 UV」的闸门，
        // 判据是 `!=`。saturating_add 一旦停在 u64::MAX 就不再变化，于是上一代的
        // 代际会与它判为相等、缓存里的旧 UV 被当成仍然有效；回绕则永不与前一值相同。
        self.atlas_generation = self.atlas_generation.wrapping_add(1);
    }

    /// 图集代际：图集内容每次变化即推进，供增量上传门控。
    pub fn atlas_generation(&self) -> u64 {
        self.atlas_generation
    }

    /// 取走本次的脏区 `(x, y, w, h)`；无脏区时为 None。
    pub fn take_dirty_rect(&mut self) -> Option<(u32, u32, u32, u32)> {
        self.dirty_rect.take()
    }

    /// 把脏区重置为「已缓存字形的包围盒」（下次只上传这一块）。
    ///
    /// 管线重建后 GPU 侧的图集纹理是新建的空纹理，CPU 侧的字形像素却还在：此时
    /// 必须把这些像素重新传上去，否则终端只剩背景、无文字。按整幅图集上传是
    /// 2048×2048×4B = 16MB，而实际用到的字形通常只占图集的一小块（ASCII 一百来个
    /// 字形约几十 KB）。故取缓存字形的包围盒，只把这块传上去。
    ///
    /// 返回是否需要上传：无缓存字形时图集本来就是空的，没有像素可传。
    pub fn reset_dirty_rect_to_cached_glyphs(&mut self) -> bool {
        let Some((min_x, min_y, width, height)) = self.cached_glyph_bounds() else {
            self.dirty_rect = None;
            return false;
        };
        self.dirty_rect = Some((min_x, min_y, width, height));
        true
    }

    /// 已缓存字形在图集内的包围盒 `(x, y, w, h)`；无缓存字形时 None。
    ///
    /// 包围盒而非逐字形上传：图集分配器把字形散落在各处，逐个上传要发很多次
    /// `write_texture`，而一次包围盒上传即可覆盖全部存活字形。
    fn cached_glyph_bounds(&self) -> Option<(u32, u32, u32, u32)> {
        let mut min_x = u32::MAX;
        let mut min_y = u32::MAX;
        let mut max_x = 0u32;
        let mut max_y = 0u32;
        let mut found = false;
        for (_, info) in &self.caches.glyph_cache {
            if info.width <= 0 || info.height <= 0 {
                continue;
            }
            let x = u32::try_from(info.atlas_x).unwrap_or(0);
            let y = u32::try_from(info.atlas_y).unwrap_or(0);
            let right = x.saturating_add(u32::try_from(info.width).unwrap_or(0));
            let bottom = y.saturating_add(u32::try_from(info.height).unwrap_or(0));
            min_x = min_x.min(x);
            min_y = min_y.min(y);
            max_x = max_x.max(right);
            max_y = max_y.max(bottom);
            found = true;
        }
        if !found {
            return None;
        }
        Some((min_x, min_y, max_x - min_x, max_y - min_y))
    }

    /// 字形缓存条目数。
    pub fn cache_length(&self) -> usize {
        self.caches.glyph_cache.len()
    }

    /// 图集位图（RGBA8 覆盖率）。
    pub fn atlas_bitmap(&self) -> &[u8] {
        &self.atlas_bitmap
    }

    /// 图集尺寸（宽, 高）。
    pub fn atlas_dimensions(&self) -> (u32, u32) {
        (self.atlas_width, self.atlas_height)
    }
}
