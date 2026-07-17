use super::FontPipeline;

pub(super) const ASCII_START: u32 = 32;
pub(super) const ASCII_END: u32 = 127;

pub(super) const ASCENT_FALLBACK_RATIO: f32 = 0.8;
pub(super) const DESCENT_FALLBACK_RATIO: f32 = 0.2;
pub(super) const CELL_WIDTH_FALLBACK_RATIO: f32 = 0.6;
pub(super) const CELL_HEIGHT_FALLBACK_RATIO: f32 = 1.2;

impl FontPipeline {
    pub fn rasterize_ascii(&mut self) {
        let before = self.cache_length();
        for ch in ASCII_START as u8..ASCII_END as u8 {
            self.glyph_information(ch as char);
        }
        let after = self.cache_length();
        log::debug!(
            "FONT_RASTERIZE_ASCII: before={} after={} font_id={:?}",
            before,
            after,
            self.font_id
        );
    }

    pub fn ascent_pixels(&self) -> f32 {
        if let Some(font_id) = self.font_id {
            let db = self.font_system.db();
            let result = db.with_face_data(font_id, |font_data, face_index| {
                let font_ref = swash::FontRef::from_index(font_data, face_index as usize)?;
                let metrics = font_ref.metrics(&[]);
                let upem = metrics.units_per_em as f32;
                if upem == 0.0 {
                    return None;
                }
                let scale = self.font_size / upem;
                Some(metrics.ascent * scale)
            });
            if let Some(Some(px)) = result {
                return px;
            }
        }
        self.font_size * ASCENT_FALLBACK_RATIO
    }

    pub fn descent_pixels(&self) -> f32 {
        if let Some(font_id) = self.font_id {
            let db = self.font_system.db();
            let result = db.with_face_data(font_id, |font_data, face_index| {
                let font_ref = swash::FontRef::from_index(font_data, face_index as usize)?;
                let metrics = font_ref.metrics(&[]);
                let upem = metrics.units_per_em as f32;
                if upem == 0.0 {
                    return None;
                }
                let scale = self.font_size / upem;
                Some(metrics.descent.abs() * scale)
            });
            if let Some(Some(px)) = result {
                return px;
            }
        }
        self.font_size * DESCENT_FALLBACK_RATIO
    }

    pub fn cell_metrics(&self) -> (f32, f32) {
        if let Some(font_id) = self.font_id {
            let db = self.font_system.db();
            let result = db.with_face_data(font_id, |font_data, face_index| {
                let font_ref = swash::FontRef::from_index(font_data, face_index as usize)?;
                let metrics = font_ref.metrics(&[]);
                let upem = metrics.units_per_em as f32;
                if upem == 0.0 {
                    return None;
                }
                let scale = self.font_size / upem;
                let ascent = metrics.ascent * scale;
                let descent = metrics.descent.abs() * scale;
                let line_gap = metrics.leading.max(0.0) * scale;
                let cell_height = ascent + descent + line_gap;

                let charmap = font_ref.charmap();
                let glyph_metrics = font_ref.glyph_metrics(&[]);

                if self
                    .font_id
                    .is_some_and(|id| db.faces().any(|f| f.id == id && f.monospaced))
                {
                    let glyph_id = charmap.map('m' as u32);
                    let advance = glyph_metrics.advance_width(glyph_id);
                    let cell_width = if advance > 0.0 {
                        advance * scale
                    } else {
                        self.font_size * CELL_WIDTH_FALLBACK_RATIO
                    };
                    return Some((cell_width, cell_height.ceil()));
                }

                let max_advance = ['m', 'W', '0']
                    .iter()
                    .filter_map(|&ch| {
                        let gid = charmap.map(ch as u32);
                        let adv = glyph_metrics.advance_width(gid);
                        if adv > 0.0 { Some(adv * scale) } else { None }
                    })
                    .fold(0.0f32, f32::max);

                let cell_width = if max_advance > 0.0 {
                    max_advance
                } else {
                    self.font_size * CELL_WIDTH_FALLBACK_RATIO
                };

                Some((cell_width, cell_height.ceil()))
            });
            if let Some(Some(m)) = result {
                return m;
            }
        }
        (
            self.font_size * CELL_WIDTH_FALLBACK_RATIO,
            (self.font_size * CELL_HEIGHT_FALLBACK_RATIO).ceil(),
        )
    }
}
