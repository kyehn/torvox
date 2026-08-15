//! Glyph rasterization — converting font outlines to coverage bitmaps.
use super::FontPipeline;

pub(super) const ASCII_START: u32 = 32;
pub(super) const ASCII_END: u32 = 127;

pub(super) const ASCENT_FALLBACK_RATIO: f32 = 0.8;
pub(super) const DESCENT_FALLBACK_RATIO: f32 = 0.2;
pub(super) const CELL_WIDTH_FALLBACK_RATIO: f32 = 0.6;
pub(super) const CELL_HEIGHT_FALLBACK_RATIO: f32 = 1.2;

/// Cap the line-gap contribution to a cell so pathological font `leading`
/// values (Droid Sans Mono on Android ships a very large one) cannot inflate
/// the cell height far beyond the glyph metrics. Maximum 25% of
/// ascent+descent, mirroring Termux/Ghostty row-height behavior.
///
/// No longer used in the cell-height computation (row height is
/// now ascent+descent exactly, like Termux/Ghostty/Kitty); retained for the
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
                // Standard terminal row height is ascent+descent
                // WITHOUT the font's line gap. Droid Sans Mono ships a huge
                // leading (~2000 units); even capped at 25% it inflated the
                // cell to 1.465em, leaving ~2x the glyph height of empty
                // space between rows (reported as "row spacing way too
                // large"). Termux/Ghostty/Kitty all use (ascent+descent)
                // as the row height; the line gap belongs between paragraphs,
                // not inside every terminal row.
                let cell_height = ascent + descent;

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
