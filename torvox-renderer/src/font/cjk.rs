use super::{FontPipeline, GlyphInfo};

pub(super) const CJK_BITMAP_PENALTY: u8 = 20;
pub(super) const OUTLINE_BONUS: u8 = 10;

/// Priority boost applied to CJK fallback fonts whose family name matches
/// the current system locale tag (e.g. "sc" for Simplified Chinese).
const CJK_LOCALE_BONUS: i16 = 6;

/// Priority for well-known CJK font families (Noto Sans/Serif CJK, Source Han,
/// Droid Sans Fallback, WenQuanYi).
const CJK_PRIORITY_KNOWN_FAMILY: u8 = 5;
/// Priority for fonts with a generic "cjk" tag in their family name.
const CJK_PRIORITY_GENERIC_CJK: u8 = 4;
/// Priority for fonts with a locale-specific tag (sc, tc, jp, kr).
const CJK_PRIORITY_LOCALE_TAG: u8 = 3;
/// Baseline priority for any other CJK-capable font.
const CJK_PRIORITY_FALLBACK: u8 = 2;

impl FontPipeline {
    pub(crate) fn find_cjk_fallback_fonts(&mut self, system_locale: &str) {
        let locale_tag = match system_locale {
            s if s.starts_with("zh") || s.starts_with("ja") || s.starts_with("ko") => {
                match system_locale {
                    s if s.starts_with("zh-CN") || s.starts_with("zh-Hans") => "sc",
                    s if s.starts_with("zh-TW")
                        || s.starts_with("zh-Hant")
                        || s.starts_with("zh-HK") =>
                    {
                        "tc"
                    }
                    s if s.starts_with("zh") => "sc",
                    s if s.starts_with("ja") => "jp",
                    s if s.starts_with("ko") => "kr",
                    _ => "",
                }
            }
            _ => "",
        };

        if let Some(primary_id) = self.font_id {
            let db = self.font_system.db();
            let primary_supports_cjk = db
                .with_face_data(primary_id, |font_data, face_index| {
                    let font_ref = swash::FontRef::from_index(font_data, face_index as usize)?;
                    let charmap = font_ref.charmap();
                    Some(charmap.map('中') != 0 && charmap.map('日') != 0 && charmap.map('가') != 0)
                })
                .unwrap_or(None)
                .unwrap_or(false);
            if primary_supports_cjk {
                log::debug!("CJK_FALLBACK: skipped (primary font already supports CJK)");
                return;
            }
        }

        let db = self.font_system.db();
        let test_chars = ['中', '日', '가'];
        let mut candidates: Vec<(fontdb::ID, f32, i16)> = Vec::new();

        for face in db.faces() {
            if face.id == self.font_id.unwrap_or_default() {
                continue;
            }
            let family_name = face
                .families
                .first()
                .map(|(n, _)| n.to_lowercase())
                .unwrap_or_default();

            if family_name.contains("emoji")
                || family_name.contains("color")
                || family_name.contains("symbol")
            {
                continue;
            }

            let result = db.with_face_data(face.id, |font_data, face_index| {
                let font_ref = swash::FontRef::from_index(font_data, face_index as usize)?;
                let charmap = font_ref.charmap();
                let metrics = font_ref.metrics(&[]);
                let upem = metrics.units_per_em as f32;
                if upem == 0.0 {
                    return Some(None);
                }
                let scale = self.font_size / upem;
                let mut total_advance = 0.0;
                let mut found = 0u32;
                for &test_char in &test_chars {
                    let gid = charmap.map(test_char);
                    if gid != 0 {
                        let advance = font_ref.glyph_metrics(&[]).advance_width(gid);
                        total_advance += advance * scale;
                        found += 1;
                    }
                }
                if found == 0 {
                    return Some(None);
                }
                let avg_advance = total_advance / found as f32;
                Some(Some(avg_advance))
            });
            if let Some(Some(Some(advance_px))) = result {
                let is_locale_match = !locale_tag.is_empty() && family_name.contains(locale_tag);
                let is_generic_cjk = family_name.contains("cjk");
                let locale_boost = if is_locale_match { CJK_LOCALE_BONUS } else { 0 };

                let base_priority: u8 = if family_name.contains("noto sans sc")
                    || family_name.contains("noto sans tc")
                    || family_name.contains("noto sans hk")
                    || family_name.contains("noto sans jp")
                    || family_name.contains("noto sans kr")
                    || family_name.contains("noto sans cjk")
                    || family_name.contains("noto serif cjk")
                    || family_name.contains("noto sans mono cjk")
                    || family_name.contains("source han")
                    || family_name.contains("droid sans fallback")
                    || family_name.contains("wenquanyi")
                {
                    CJK_PRIORITY_KNOWN_FAMILY
                } else if is_generic_cjk {
                    CJK_PRIORITY_GENERIC_CJK
                } else if family_name.contains("sc")
                    || family_name.contains("tc")
                    || family_name.contains("jp")
                    || family_name.contains("kr")
                {
                    CJK_PRIORITY_LOCALE_TAG
                } else {
                    CJK_PRIORITY_FALLBACK
                };
                let priority = (base_priority as i16).saturating_add(locale_boost as i16);
                let (is_vector, source_quality_penalty): (bool, u8) = {
                    let is_vector = db
                        .with_face_data(face.id, |font_data, face_index| {
                            let font_ref =
                                swash::FontRef::from_index(font_data, face_index as usize)?;
                            let mut scaler = self
                                .scaler_context
                                .builder(font_ref)
                                .size(self.font_size)
                                .hint(true)
                                .build();
                            let charmap = font_ref.charmap();
                            let gid = charmap.map('\u{4e2d}');
                            if gid == 0 {
                                return Some(false);
                            }
                            let image = swash::scale::Render::new(&[]).render(&mut scaler, gid);
                            Some(image.is_some_and(|img| {
                                matches!(
                                    img.content,
                                    swash::scale::image::Content::Mask
                                        | swash::scale::image::Content::SubpixelMask
                                )
                            }))
                        })
                        .unwrap_or(Some(false))
                        .unwrap_or(false);
                    if is_vector {
                        (true, 0u8)
                    } else {
                        (false, CJK_BITMAP_PENALTY)
                    }
                };
                let outline_bonus = if is_vector {
                    OUTLINE_BONUS as i16
                } else {
                    0i16
                };
                let effective_priority =
                    priority.saturating_sub(source_quality_penalty as i16) + outline_bonus;
                log::debug!(
                    "CJK_CANDIDATE: family='{}' base={} locale={} is_vector={} eff_pri={}",
                    family_name,
                    base_priority,
                    locale_boost,
                    is_vector,
                    effective_priority,
                );
                candidates.push((face.id, advance_px, effective_priority));
            }
        }

        candidates.sort_by(|a, b| {
            b.2.cmp(&a.2)
                .then_with(|| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal))
        });

        const MAX_CJK_FALLBACK_FONTS: usize = 3;
        for (id, _, _) in candidates.iter().take(MAX_CJK_FALLBACK_FONTS) {
            let face = db.face(*id);
            let name = face
                .and_then(|f| f.families.first())
                .map(|(n, _)| n.clone())
                .unwrap_or_default();
            log::debug!("CJK_FALLBACK: selected font id={:?} name='{}'", id, name);
            self.cjk_fallback_ids.push(*id);
        }
        log::debug!(
            "CJK_FALLBACK: found {} fallback fonts (limited to {})",
            self.cjk_fallback_ids.len(),
            MAX_CJK_FALLBACK_FONTS
        );
        if candidates.is_empty() || candidates.iter().all(|c| c.2 <= 0i16) {
            log::debug!(
                "CJK_FALLBACK: no font with vector outlines found; CJK may render as bitmap"
            );
        }
    }

    pub(crate) fn glyph_source_is_outline(
        &mut self,
        font_id: fontdb::ID,
        glyph_id: swash::GlyphId,
    ) -> bool {
        let scaler_context = &mut self.scaler_context;
        let font_size = self.font_size;
        let db = self.font_system.db();
        let result = db.with_face_data(font_id, |font_data, face_index| {
            let font_ref = swash::FontRef::from_index(font_data, face_index as usize)?;
            let mut scaler = scaler_context
                .builder(font_ref)
                .size(font_size)
                .hint(true)
                .build();
            let image = swash::scale::Render::new(&[]).render(&mut scaler, glyph_id);
            Some(image.is_some_and(|img| {
                matches!(
                    img.content,
                    swash::scale::image::Content::Mask | swash::scale::image::Content::SubpixelMask
                )
            }))
        });
        result.unwrap_or(Some(false)).unwrap_or(false)
    }

    pub(crate) fn try_cjk_outline_fallback(&mut self, ch: char) -> Option<GlyphInfo> {
        let glyphs: Vec<(fontdb::ID, swash::GlyphId)> = {
            let db = self.font_system.db();
            self.cjk_fallback_ids
                .iter()
                .filter_map(|&fallback_id| {
                    let gid = db.with_face_data(fallback_id, |font_data, face_index| {
                        let font_ref = swash::FontRef::from_index(font_data, face_index as usize)?;
                        let charmap = font_ref.charmap();
                        let gid = charmap.map(ch);
                        if gid != 0 {
                            Some(gid)
                        } else {
                            None
                        }
                    })??;
                    Some((fallback_id, gid))
                })
                .collect()
        };
        for (fallback_id, fid) in &glyphs {
            if self.glyph_source_is_outline(*fallback_id, *fid) {
                return self.glyph_information_from_font(*fallback_id, ch, *fid);
            }
        }
        None
    }
}
