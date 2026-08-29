//! CJK fallback font resolution — finds and loads CJK fonts for ideograph rendering.
use super::{FontPipeline, GlyphInfo};

pub(super) const CJK_BITMAP_PENALTY: u8 = 20;
pub(super) const OUTLINE_BONUS: u8 = 10;

/// Priority boost applied to CJK fallback fonts whose family name matches
/// the current system locale tag (e.g. "sc" for Simplified Chinese).
const CJK_LOCALE_BONUS: i16 = 6;

/// Penalty subtracted from serif CJK families so sans CJK always wins the
/// fallback tie-break (serif reads as 宋体 next to a sans terminal font).
/// 32 guarantees Sans wins even when Sans is bitmap and Serif is vector:
/// Sans worst (bitmap) 5-20=-15 > Serif best (vector) 5-32+10=-17.
const CJK_SERIF_PENALTY: i16 = 32;

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
    /// Whether a family name is a CJK-capable candidate for the CJK
    /// fallback layer (excludes emoji/color/symbol fonts: they either
    /// cannot be outlined by swash or are dedicated to other layers).
    pub(crate) fn is_cjk_candidate_family(name: &str) -> bool {
        !(name.contains("emoji")
            || name.contains("color")
            || name.contains("symbol")
            || name.contains("nerd"))
    }

    /// Whether a family name belongs to the symbol-fallback layer
    /// (moke: Noto Sans Symbols 2 — media/geometric/misc symbols such as
    /// ▶ ⏵ ♥ ★ that terminal fonts usually lack). Excludes emoji/color
    /// fonts (bitmap) and nerd fonts (their own layer).
    pub(crate) fn is_symbol_candidate_family(name: &str) -> bool {
        (name.contains("symbol")
            || name.contains("dingbat")
            || name.contains("icon")
            || name.contains("misc"))
            && !name.contains("emoji")
            && !name.contains("color")
            && !name.contains("nerd")
    }

    /// Whether a family name belongs to the Nerd layer (moke: Symbols
    /// Nerd Font — private-use-area U+E000 glyphs: powerline separators,
    /// devicons, file-type icons).
    pub(crate) fn is_nerd_candidate_family(name: &str) -> bool {
        name.contains("nerd")
    }

    /// Whether a family name belongs to the emoji layer (moke: emoji
    /// handled via the system fallback; here NotoColorEmoji-style fonts).
    pub(crate) fn is_emoji_candidate_family(name: &str) -> bool {
        name.contains("emoji") || name.contains("color")
    }

    /// Symbol-layer test glyphs (moke symbol fonts cover these blocks).
    const SYMBOL_TEST_CHARS: [char; 5] =
        ['\u{25b6}', '\u{23f5}', '\u{2665}', '\u{2605}', '\u{25c6}'];

    /// Nerd-layer test glyphs (U+E000..U+F8FF private use: powerline
    /// separators, devicons, file-type icons).
    const NERD_TEST_CHARS: [char; 4] = ['\u{e0a0}', '\u{e0b0}', '\u{f50a}', '\u{f553}'];

    /// Emoji-layer test glyphs.
    const EMOJI_TEST_CHARS: [char; 2] = ['\u{1f600}', '\u{1f44d}'];

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
                .flatten()
                .unwrap_or(false);
            if primary_supports_cjk {
                log::debug!("CJK_FALLBACK: skipped (primary font already supports CJK)");
                return;
            }
        }

        const MAX_CJK_FALLBACK_FONTS: usize = 3;
        // CJK scoring: known families + locale tag + generic "cjk" tags
        // (see CJK_PRIORITY_* constants), plus the locale boost.
        let test_chars = ['中', '日', '가'];
        let ids = self.scan_fallback_candidates(
            &test_chars,
            Self::is_cjk_candidate_family,
            |family_name| cjk_family_priority(family_name, locale_tag),
            MAX_CJK_FALLBACK_FONTS,
        );
        self.cjk_fallback_ids = ids.clone();
        if ids.is_empty() {
            log::warn!(
                "WARN FontFallback script=Han fallback missing, tried CJK scan with {} candidates; fallback list empty",
                self.cjk_fallback_ids.len()
            );
        }
        log::debug!(
            "CJK_FALLBACK: found {} fallback fonts (limited to {})",
            self.cjk_fallback_ids.len(),
            MAX_CJK_FALLBACK_FONTS
        );
    }

    /// Resolve the symbol layer (moke chain: after CJK, before Nerd).
    pub(crate) fn find_symbol_fallback_fonts(&mut self) {
        const MAX_SYMBOL_FALLBACK_FONTS: usize = 2;
        let ids = self.scan_fallback_candidates(
            &Self::SYMBOL_TEST_CHARS,
            Self::is_symbol_candidate_family,
            |family_name| {
                // Known symbol families get a small boost; advance
                // similarity does the rest.
                if family_name.contains("noto sans symbols") {
                    2
                } else {
                    0
                }
            },
            MAX_SYMBOL_FALLBACK_FONTS,
        );
        self.symbol_fallback_ids = ids;
        log::debug!(
            "SYMBOL_FALLBACK: found {} symbol fallback fonts",
            self.symbol_fallback_ids.len()
        );
    }

    /// Resolve the Nerd layer (moke chain: after symbols, before emoji).
    pub(crate) fn find_nerd_fallback_fonts(&mut self) {
        const MAX_NERD_FALLBACK_FONTS: usize = 2;
        let ids = self.scan_fallback_candidates(
            &Self::NERD_TEST_CHARS,
            Self::is_nerd_candidate_family,
            |family_name| {
                if family_name.contains("symbols nerd") || family_name.contains("nerd font") {
                    2
                } else {
                    0
                }
            },
            MAX_NERD_FALLBACK_FONTS,
        );
        self.nerd_fallback_ids = ids;
        log::debug!(
            "NERD_FALLBACK: found {} nerd fallback fonts",
            self.nerd_fallback_ids.len()
        );
    }

    /// Resolve the emoji layer (moke: emoji via system chain; here the
    /// color fonts are collected so the lookup at least TRIES them —
    /// swash cannot outline color glyphs, so they are skipped at render
    /// time and the database scan /.notdef takes over).
    pub(crate) fn find_emoji_fallback_fonts(&mut self) {
        const MAX_EMOJI_FALLBACK_FONTS: usize = 1;
        let ids = self.scan_fallback_candidates(
            &Self::EMOJI_TEST_CHARS,
            Self::is_emoji_candidate_family,
            |family_name| {
                if family_name.contains("noto color emoji") {
                    2
                } else {
                    0
                }
            },
            MAX_EMOJI_FALLBACK_FONTS,
        );
        self.emoji_fallback_ids = ids;
        log::debug!(
            "EMOJI_FALLBACK: found {} emoji fallback fonts",
            self.emoji_fallback_ids.len()
        );
    }

    /// Generic layered scan: every face whose family passes
    /// [family_allowed] and whose charmap covers at least one of
    /// [test_chars] becomes a candidate. Candidates are scored by
    /// [family_priority] plus an outline bonus, then ranked by score and
    /// average advance; the top [max_results] IDs are returned.
    ///
    /// Borrow discipline: inside `db.with_face_data` only the
    /// `scaler_context` / `font_size` fields are touched (field-level
    /// borrows), so the db borrow never conflicts with `&mut self`.
    fn scan_fallback_candidates(
        &mut self,
        test_chars: &[char],
        family_allowed: impl Fn(&str) -> bool,
        family_priority: impl Fn(&str) -> i16,
        max_results: usize,
    ) -> Vec<fontdb::ID> {
        // Collect face IDs and family names first with a short-lived immutable borrow;
        // the subsequent per-face outline-cache probes require &mut self and must not
        // overlap the db borrow (field-level borrow discipline).
        let faces: Vec<(fontdb::ID, String)> = {
            let db = self.font_system.db();
            db.faces()
                .filter(|face| face.id != self.font_id.unwrap_or_default())
                .filter_map(|face| {
                    let name = face
                        .families
                        .first()
                        .map(|(n, _)| n.to_lowercase())
                        .unwrap_or_default();
                    if !family_allowed(&name) {
                        return None;
                    }
                    Some((face.id, name))
                })
                .collect()
        };
        let mut candidates: Vec<(fontdb::ID, f32, i16)> = Vec::new();
        let font_size = self.font_size;

        for (face_id, family_name) in faces {
            let result = self
                .font_system
                .db()
                .with_face_data(face_id, |font_data, face_index| {
                    let font_ref = swash::FontRef::from_index(font_data, face_index as usize)?;
                    let charmap = font_ref.charmap();
                    let metrics = font_ref.metrics(&[]);
                    let upem = metrics.units_per_em as f32;
                    if upem == 0.0 {
                        return Some(None);
                    }
                    let scale = font_size / upem;
                    let mut total_advance = 0.0;
                    let mut found = 0u32;
                    for &test_char in test_chars {
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
                let (is_vector, source_quality_penalty): (bool, u8) = {
                    // Majority vote over test_chars: mixed bitmap/vector fonts have some glyphs
                    // vector, some bitmap; single-probe on '中' misclassifies. Count hits.
                    // First collect GIDs with a short-lived db borrow, then probe the outline
                    // cache (which needs &mut self) after the db borrow is released.
                    let probe_gids: Vec<swash::GlyphId> = {
                        let db = self.font_system.db();
                        test_chars
                            .iter()
                            .filter_map(|&probe_char| {
                                db.with_face_data(face_id, |font_data, face_index| {
                                    let font_ref =
                                        swash::FontRef::from_index(font_data, face_index as usize)?;
                                    let charmap = font_ref.charmap();
                                    let gid = charmap.map(probe_char);
                                    if gid == 0 {
                                        return None;
                                    }
                                    Some(gid)
                                })
                                .flatten()
                            })
                            .collect()
                    };
                    let mut outline_hits: u32 = 0;
                    let mut bitmap_hits: u32 = 0;
                    for gid in probe_gids {
                        let is_outline = self.glyph_source_is_outline_cached(face_id, gid);
                        if is_outline {
                            outline_hits += 1;
                        } else {
                            bitmap_hits += 1;
                        }
                    }
                    let is_vector = if outline_hits + bitmap_hits == 0 {
                        false
                    } else {
                        outline_hits > bitmap_hits
                    };
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
                    family_priority(&family_name) - source_quality_penalty as i16 + outline_bonus;
                log::debug!(
                    "FALLBACK_CANDIDATE: family='{}' advance={:.2} is_vector={} eff_pri={}",
                    family_name,
                    advance_px,
                    is_vector,
                    effective_priority,
                );
                candidates.push((face_id, advance_px, effective_priority));
            }
        }

        candidates.sort_by(|&(_, advance_a, pri_a), &(_, advance_b, pri_b)| {
            pri_b.cmp(&pri_a).then_with(|| {
                advance_b
                    .partial_cmp(&advance_a)
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
        });

        candidates
            .iter()
            .take(max_results)
            .map(|(id, _, _)| *id)
            .collect()
    }

    /// Whole-database scan (spec d7 tail of the chain): the first face —
    /// other than the primary — whose charmap maps [ch] to a real glyph.
    /// Render-time failures (color fonts) are handled by the caller. The
    /// result is cached by the caller in `cjk_glyph_cache`, so this only
    /// runs once per character.
    pub(crate) fn find_glyph_anywhere(&mut self, ch: char) -> Option<(fontdb::ID, u16)> {
        let primary = self.font_id?;
        let db = self.font_system.db();
        // Collect candidates first: the db borrow must end before the
        // &mut self render check below.
        let mut candidates: Vec<(fontdb::ID, u16)> = Vec::new();
        for face in db.faces() {
            if face.id == primary {
                continue;
            }
            let gid = db
                .with_face_data(face.id, |font_data, face_index| {
                    let font_ref = swash::FontRef::from_index(font_data, face_index as usize)?;
                    let charmap = font_ref.charmap();
                    let gid = charmap.map(ch);
                    (gid != 0).then_some(gid)
                })
                .flatten();
            if let Some(gid) = gid {
                candidates.push((face.id, gid));
            }
        }
        // Return the first candidate that actually renders (charmap hits
        // in color fonts such as Noto Color Emoji cannot be outlined by
        // swash and must be skipped —).
        for (id, gid) in candidates {
            if let Some(info) = self.glyph_information_from_font(id, ch, gid)
                && info.width > 0
                && info.height > 0
            {
                log::debug!(
                    "FALLBACK_SCAN: char U+{:04X} resolved in font id={:?}",
                    ch as u32,
                    id
                );
                return Some((id, gid));
            }
        }
        None
    }

    /// Cached outline probe: checks `outline_cache` before building a scaler.
    pub(crate) fn glyph_source_is_outline_cached(
        &mut self,
        font_id: fontdb::ID,
        glyph_id: swash::GlyphId,
    ) -> bool {
        let key = (font_id, glyph_id);
        if let Some(&cached) = self.caches.outline_cache.get(&key) {
            return cached;
        }
        let is_outline = self.glyph_source_is_outline(font_id, glyph_id);
        self.caches.outline_cache.put(key, is_outline);
        is_outline
    }

    pub(crate) fn glyph_source_is_outline(
        &mut self,
        font_id: fontdb::ID,
        glyph_id: swash::GlyphId,
    ) -> bool {
        let scaler_context = &mut self.scaler_context;
        // Match the real raster path (atlas.rs): raster_size = font_size * raster_scale,
        // hint only when 1:1, Source::Outline only. Using font_size + hint(true) without
        // Source filter hit embedded bitmap strikes in NotoSansCJK TTC at 14sp (is_vector=false)
        // while the atlas always rasterizes vector outlines at raster_size with hint(false),
        // causing try_cjk_outline_fallback to skip ALL CJK on high-density screens.
        let raster_size = self.font_size * self.raster_scale.max(1.0);
        let hint = self.raster_scale <= 1.01;
        let db = self.font_system.db();
        let result = db.with_face_data(font_id, |font_data, face_index| {
            let font_ref = swash::FontRef::from_index(font_data, face_index as usize)?;
            let mut scaler = scaler_context
                .builder(font_ref)
                .size(raster_size)
                .hint(hint)
                .build();
            let image = swash::scale::Render::new(&[swash::scale::Source::Outline])
                .render(&mut scaler, glyph_id);
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
        // Check cache first — skip swash iteration for already-resolved CJK chars
        if let Some(&(cached_font_id, cached_glyph_id)) = self.caches.cjk_glyph_cache.get(&ch) {
            let result = self.glyph_information_from_font(cached_font_id, ch, cached_glyph_id);
            if result.is_some() {
                return result;
            }
        }
        let glyphs: Vec<(fontdb::ID, swash::GlyphId)> = {
            let db = self.font_system.db();
            self.cjk_fallback_ids
                .iter()
                .filter_map(|&fallback_id| {
                    let result = db.with_face_data(fallback_id, |font_data, face_index| {
                        let font_ref = swash::FontRef::from_index(font_data, face_index as usize)?;
                        let charmap = font_ref.charmap();
                        let gid = charmap.map(ch);
                        if gid != 0 { Some(gid) } else { None }
                    })?;
                    let gid = result?;
                    Some((fallback_id, gid))
                })
                .collect()
        };
        for (fallback_id, fid) in &glyphs {
            if self.glyph_source_is_outline_cached(*fallback_id, *fid) {
                let result = self.glyph_information_from_font(*fallback_id, ch, *fid);
                if result.is_some() {
                    // Cache the successful CJK resolution
                    self.caches.cjk_glyph_cache.put(ch, (*fallback_id, *fid));
                    return result;
                }
            }
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── is_cjk_candidate_family ────────────────────────────────────────
    #[test]
    fn cjk_candidate_noto_sans_cjk() {
        assert!(FontPipeline::is_cjk_candidate_family("noto sans cjk sc"));
    }

    #[test]
    fn cjk_candidate_source_han() {
        assert!(FontPipeline::is_cjk_candidate_family("source han sans sc"));
    }

    #[test]
    fn cjk_candidate_wenquanyi() {
        assert!(FontPipeline::is_cjk_candidate_family("wenquanyi micro hei"));
    }

    #[test]
    fn cjk_candidate_liberation_mono() {
        // Regular Latin font is also a valid CJK candidate (may have CJK glyphs)
        assert!(FontPipeline::is_cjk_candidate_family("liberation mono"));
    }

    #[test]
    fn cjk_reject_emoji() {
        assert!(!FontPipeline::is_cjk_candidate_family("noto color emoji"));
    }

    #[test]
    fn cjk_reject_symbol() {
        assert!(!FontPipeline::is_cjk_candidate_family(
            "noto sans symbols 2"
        ));
    }

    #[test]
    fn cjk_reject_nerd() {
        assert!(!FontPipeline::is_cjk_candidate_family(
            "jetbrainsmono nerd font mono"
        ));
    }

    #[test]
    fn cjk_reject_color() {
        assert!(!FontPipeline::is_cjk_candidate_family("openmoji color"));
    }

    // ── is_symbol_candidate_family ─────────────────────────────────────
    #[test]
    fn symbol_candidate_noto_symbols() {
        assert!(FontPipeline::is_symbol_candidate_family(
            "noto sans symbols 2"
        ));
    }

    #[test]
    fn symbol_candidate_dingbats() {
        assert!(FontPipeline::is_symbol_candidate_family("dingbats"));
    }

    #[test]
    fn symbol_candidate_misc() {
        assert!(FontPipeline::is_symbol_candidate_family("misc symbols"));
    }

    #[test]
    fn symbol_reject_emoji() {
        assert!(!FontPipeline::is_symbol_candidate_family("emoji symbols"));
    }

    #[test]
    fn symbol_reject_nerd() {
        assert!(!FontPipeline::is_symbol_candidate_family("nerd symbols"));
    }

    #[test]
    fn symbol_reject_color() {
        assert!(!FontPipeline::is_symbol_candidate_family("color symbols"));
    }

    #[test]
    fn symbol_reject_regular_font() {
        assert!(!FontPipeline::is_symbol_candidate_family("liberation mono"));
    }

    // ── is_nerd_candidate_family ───────────────────────────────────────
    #[test]
    fn nerd_candidate_jetbrains() {
        assert!(FontPipeline::is_nerd_candidate_family(
            "jetbrainsmono nerd font mono"
        ));
    }

    #[test]
    fn nerd_candidate_firacode() {
        assert!(FontPipeline::is_nerd_candidate_family("firacode nerd font"));
    }

    #[test]
    fn nerd_reject_regular() {
        assert!(!FontPipeline::is_nerd_candidate_family("jetbrains mono"));
    }

    #[test]
    fn nerd_reject_symbol() {
        assert!(!FontPipeline::is_nerd_candidate_family("noto symbols"));
    }

    // ── is_emoji_candidate_family ──────────────────────────────────────
    #[test]
    fn emoji_candidate_noto_color_emoji() {
        assert!(FontPipeline::is_emoji_candidate_family("noto color emoji"));
    }

    #[test]
    fn emoji_candidate_openmoji() {
        assert!(FontPipeline::is_emoji_candidate_family("openmoji color"));
    }

    #[test]
    fn emoji_reject_regular() {
        assert!(!FontPipeline::is_emoji_candidate_family("liberation mono"));
    }

    #[test]
    fn emoji_reject_symbol() {
        assert!(!FontPipeline::is_emoji_candidate_family("noto symbols 2"));
    }

    // ── Classification boundary: one name can only belong to ONE layer ──
    #[test]
    fn classification_disjoint() {
        let families = [
            "noto sans cjk sc",
            "noto sans symbols 2",
            "jetbrainsmono nerd font mono",
            "noto color emoji",
            "liberation mono",
            "source han sans sc",
        ];
        for name in &families {
            let cjk = FontPipeline::is_cjk_candidate_family(name);
            let sym = FontPipeline::is_symbol_candidate_family(name);
            let nerd = FontPipeline::is_nerd_candidate_family(name);
            let emoji = FontPipeline::is_emoji_candidate_family(name);
            // Nerd+emoji should be disjoint from CJK
            assert!(!(nerd && cjk), "{name} should not be both nerd and cjk");
            assert!(!(emoji && cjk), "{name} should not be both emoji and cjk");
            // Symbol+emoji should be disjoint
            assert!(
                !(sym && emoji),
                "{name} should not be both symbol and emoji"
            );
        }
    }

    // ── Case sensitivity: family names from fontdb are lowercase ───────
    #[test]
    fn case_sensitive_nerd() {
        // fontdb returns lowercase names, so matching is case-sensitive
        assert!(!FontPipeline::is_nerd_candidate_family("Nerd"));
        assert!(FontPipeline::is_nerd_candidate_family("nerd"));
    }

    // ── Empty / minimal strings ────────────────────────────────────────
    #[test]
    fn empty_string_cjk_is_candidate() {
        // Empty name contains none of the exclusion keywords, so it IS a CJK candidate
        assert!(FontPipeline::is_cjk_candidate_family(""));
    }

    #[test]
    fn empty_string_rejects_symbol_nerd_emoji() {
        // Empty name has no matching keywords for other layers
        assert!(!FontPipeline::is_symbol_candidate_family(""));
        assert!(!FontPipeline::is_nerd_candidate_family(""));
        assert!(!FontPipeline::is_emoji_candidate_family(""));
    }
}

/// Returns true when `family_name` contains `locale_tag` as a standalone token
/// (split on non-alphanumeric). Prevents `misc` matching `sc` (`misc` contains
/// the substring `sc` but not the token `sc`).
pub(crate) fn locale_token_match(family_name: &str, locale_tag: &str) -> bool {
    if locale_tag.is_empty() {
        return false;
    }
    family_name
        .split(|c: char| !c.is_ascii_alphanumeric())
        .any(|token| token == locale_tag)
}

/// CJK fallback family priority (higher wins). Sans CJK families outrank
/// serif CJK: serif renders as 宋体/SimSun-style, visually jarring next to a
/// sans/mono terminal font (user report "中文显示为宋体" — both
/// NotoSansCJK and NotoSerifCJK ship in /system/fonts and the old equal
/// priority let load order pick Serif).
fn cjk_family_priority(family_name: &str, locale_tag: &str) -> i16 {
    let is_locale_match = locale_token_match(family_name, locale_tag);
    let locale_boost = if is_locale_match { CJK_LOCALE_BONUS } else { 0 };
    let base_priority: i16 = if family_name.contains("noto sans sc")
        || family_name.contains("noto sans tc")
        || family_name.contains("noto sans hk")
        || family_name.contains("noto sans jp")
        || family_name.contains("noto sans kr")
        || family_name.contains("noto sans cjk")
        || family_name.contains("noto sans mono cjk")
        || family_name.contains("source han")
        || family_name.contains("droid sans fallback")
        || family_name.contains("wenquanyi")
    {
        CJK_PRIORITY_KNOWN_FAMILY as i16
    } else if family_name.contains("noto serif cjk") {
        CJK_PRIORITY_KNOWN_FAMILY as i16 - CJK_SERIF_PENALTY
    } else if family_name.contains("cjk") {
        CJK_PRIORITY_GENERIC_CJK as i16
    } else if family_name.contains("sc")
        || family_name.contains("tc")
        || family_name.contains("jp")
        || family_name.contains("kr")
    {
        CJK_PRIORITY_LOCALE_TAG as i16
    } else {
        CJK_PRIORITY_FALLBACK as i16
    };
    base_priority + locale_boost as i16
}

#[cfg(test)]
mod cjk_priority_tests {
    use super::*;

    /// Sans CJK must outrank serif CJK regardless of locale (宋体 complaint).
    #[test]
    fn sans_cjk_outranks_serif_cjk() {
        assert!(
            cjk_family_priority("noto sans cjk", "") > cjk_family_priority("noto serif cjk", "")
        );
        assert!(
            cjk_family_priority("noto sans cjk sc", "sc")
                > cjk_family_priority("noto serif cjk sc", "sc")
        );
    }

    #[test]
    fn locale_token_boundary_misc_not_sc() {
        // "misc" contains substring "sc" but not token "sc" — must NOT boost.
        assert!(!locale_token_match("misc", "sc"));
        assert!(!locale_token_match("misc symbols", "sc"));
        assert!(locale_token_match("noto sans cjk sc", "sc"));
        assert!(locale_token_match("noto sans cjk jp", "jp"));
        assert!(!locale_token_match("noto sans cjk jp", "sc"));
    }

    #[test]
    fn serif_penalty_guards_vector_vs_bitmap() {
        // Worst Sans (bitmap) = 5-20 = -15; best Serif (vector) = 5-32+10 = -17 → Sans still wins.
        let sans_bitmap = cjk_family_priority("noto sans cjk", "") - CJK_BITMAP_PENALTY as i16;
        let serif_vector = cjk_family_priority("noto serif cjk", "") + OUTLINE_BONUS as i16;
        assert!(
            sans_bitmap > serif_vector,
            "Sans bitmap ({sans_bitmap}) must beat Serif vector ({serif_vector})"
        );
    }

    /// Locale-matching families get the boost on top of their base priority.
    #[test]
    fn locale_boost_applies() {
        assert!(
            cjk_family_priority("droid sans fallback", "")
                < cjk_family_priority("noto sans cjk jp", "jp")
        );
    }

    /// Unknown CJK-capable families still qualify at fallback priority.
    #[test]
    fn unknown_family_gets_fallback_priority() {
        assert_eq!(
            cjk_family_priority("some han font", ""),
            CJK_PRIORITY_FALLBACK as i16
        );
    }
}
