//! CJK 回退字体解析：定位并加载表意文字渲染所需的 CJK 字体。
use super::{FontPipeline, GlyphInfo};

pub(super) const CJK_BITMAP_PENALTY: u8 = 20;
pub(super) const OUTLINE_BONUS: u8 = 10;

/// 族名匹配当前系统 locale 标签的 CJK 回退字体的优先级加成（如简体的 `sc`）。
const CJK_LOCALE_BONUS: i16 = 6;

/// 衬线 CJK 族的惩罚分，使无衬线 CJK 在回退平局时必胜（衬线在无衬线终端字体旁
/// 呈现为宋体，观感突兀）。取 32 可保证即使无衬线是 bitmap、衬线是 vector 也仍然胜出：
/// Sans 最差 5-20=-15 > Serif 最好 5-32+10=-17。
const CJK_SERIF_PENALTY: i16 = 32;

/// 知名 CJK 字体族（Noto Sans/Serif CJK、Source Han、Droid Sans Fallback、WenQuanYi）的优先级。
const CJK_PRIORITY_KNOWN_FAMILY: u8 = 5;
const CJK_PRIORITY_GENERIC_CJK: u8 = 4;
const CJK_PRIORITY_LOCALE_TAG: u8 = 3;
const CJK_PRIORITY_FALLBACK: u8 = 2;

impl FontPipeline {
    pub(crate) fn is_cjk_candidate_family(name: &str) -> bool {
        !(name.contains("emoji")
            || name.contains("color")
            || name.contains("symbol")
            || name.contains("nerd"))
    }

    /// 族名是否属符号回退层（如 Noto Sans Symbols 2 的媒体/几何/杂项符号 ▶ ⏵ ♥ ★，
    /// 终端字体通常缺失）；排除 emoji/color（bitmap）与 nerd 字体（自成一层）。
    pub(crate) fn is_symbol_candidate_family(name: &str) -> bool {
        (name.contains("symbol")
            || name.contains("dingbat")
            || name.contains("icon")
            || name.contains("misc"))
            && !name.contains("emoji")
            && !name.contains("color")
            && !name.contains("nerd")
    }

    /// 族名是否属 Nerd 层（私用区 U+E000 字形：powerline 分隔符、devicons、文件类型图标）。
    pub(crate) fn is_nerd_candidate_family(name: &str) -> bool {
        name.contains("nerd")
    }

    pub(crate) fn is_emoji_candidate_family(name: &str) -> bool {
        name.contains("emoji") || name.contains("color")
    }

    const SYMBOL_TEST_CHARS: [char; 5] =
        ['\u{25b6}', '\u{23f5}', '\u{2665}', '\u{2605}', '\u{25c6}'];

    const NERD_TEST_CHARS: [char; 4] = ['\u{e0a0}', '\u{e0b0}', '\u{f50a}', '\u{f553}'];

    const EMOJI_TEST_CHARS: [char; 2] = ['\u{1f600}', '\u{1f44d}'];

    pub(crate) fn find_cjk_fallback_fonts(&mut self, system_locale: &str) {
        let locale_tag = locale_tag(system_locale);
        if !system_locale.is_empty() && locale_tag.is_empty() {
            log::debug!("CJK_FALLBACK: skipped (non-CJK locale)");
            return;
        }

        if let Some(primary_id) = self.font_id {
            let db = self.font_system.db();
            // 探测 locale 的代表字符：CJK 字体按 locale 分片（CN 字体未必覆盖
            // 谚文音节），故不要求中/日/가同时存在，否则会拒绝匹配的主字体。
            let probe = if locale_tag == "kr" { '가' } else { '中' };
            let primary_supports_cjk = db
                .with_face_data(primary_id, |font_data, face_index| {
                    let font_ref = swash::FontRef::from_index(font_data, face_index as usize)?;
                    Some(font_ref.charmap().map(probe) != 0)
                })
                .flatten()
                .unwrap_or(false);
            if primary_supports_cjk {
                log::debug!("CJK_FALLBACK: skipped (primary font already supports CJK)");
                return;
            }
        }

        const MAX_CJK_FALLBACK_FONTS: usize = 3;
        let mut ids = self.fonts_xml_cjk_fallback_ids(system_locale, MAX_CJK_FALLBACK_FONTS);
        if ids.len() < MAX_CJK_FALLBACK_FONTS {
            // CJK 评分：知名族 + locale 标签 + 通用 `cjk` 标签（见 CJK_PRIORITY_*），再加 locale 加成。
            let test_chars = ['中', '日', '가'];
            let mut scanned = self.scan_fallback_candidates(
                &test_chars,
                Self::is_cjk_candidate_family,
                |family_name| cjk_family_priority(family_name, locale_tag),
                MAX_CJK_FALLBACK_FONTS,
            );
            scanned.retain(|id| !ids.contains(id));
            ids.extend(scanned.into_iter().take(MAX_CJK_FALLBACK_FONTS - ids.len()));
        }
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

    #[cfg(any(target_os = "android", test))]
    fn fonts_xml_cjk_fallback_ids(
        &self,
        system_locale: &str,
        max_results: usize,
    ) -> Vec<fontdb::ID> {
        let xml = std::fs::read_to_string("/system/etc/fonts.xml")
            .or_else(|_| std::fs::read_to_string("/system/etc/fonts_fallback.xml"));
        let Ok(xml) = xml else {
            return Vec::new();
        };
        Self::match_fonts_xml_fallbacks(self.font_system.db(), &xml, system_locale, max_results)
    }

    #[cfg(any(target_os = "android", test))]
    pub(crate) fn match_fonts_xml_fallbacks(
        db: &fontdb::Database,
        xml: &str,
        system_locale: &str,
        max_results: usize,
    ) -> Vec<fontdb::ID> {
        let (_, lang_fallbacks) = super::font_db::parse_fonts_xml_families(xml);
        let langs = super::font_db::locale_fonts_xml_langs(system_locale);
        let locale_tag = locale_tag(system_locale);
        let mut ids = Vec::new();
        for wanted in langs {
            let Some((_, filenames)) = lang_fallbacks
                .iter()
                .find(|(lang, _)| lang.split(',').any(|tag| tag == *wanted))
            else {
                continue;
            };
            for (filename, index) in filenames {
                let same_file: Vec<(fontdb::ID, String, u32)> = db
                    .faces()
                    .filter_map(|face| {
                        let path = match &face.source {
                            fontdb::Source::File(path) => path,
                            fontdb::Source::SharedFile(path, _) => path,
                            fontdb::Source::Binary(_) => return None,
                        };
                        path.file_name()
                            .and_then(|name| name.to_str())
                            .filter(|name| name.eq_ignore_ascii_case(filename))?;
                        let family = face
                            .families
                            .first()
                            .map(|(name, _)| name.to_lowercase())
                            .unwrap_or_default();
                        Some((face.id, family, face.index))
                    })
                    .collect();
                // 精确 (文件名, 索引) 命中优先；否则取首个匹配 locale 标签的同文件面。
                let hit = same_file
                    .iter()
                    .find(|(_, _, face_index)| *face_index == *index)
                    .or_else(|| {
                        same_file
                            .iter()
                            .find(|(_, family, _)| locale_token_match(family, locale_tag))
                    });
                if let Some((id, _, _)) = hit
                    && !ids.contains(id)
                {
                    log::debug!("FONTS_XML_FALLBACK: file='{filename}' index={index} id={id:?}");
                    ids.push(*id);
                }
                if ids.len() >= max_results {
                    return ids;
                }
            }
            if !ids.is_empty() {
                break;
            }
        }
        ids
    }

    #[cfg(not(any(target_os = "android", test)))]
    fn fonts_xml_cjk_fallback_ids(
        &self,
        _system_locale: &str,
        _max_results: usize,
    ) -> Vec<fontdb::ID> {
        Vec::new()
    }

    pub(crate) fn find_symbol_fallback_fonts(&mut self) {
        const MAX_SYMBOL_FALLBACK_FONTS: usize = 2;
        let ids = self.scan_fallback_candidates(
            &Self::SYMBOL_TEST_CHARS,
            Self::is_symbol_candidate_family,
            |family_name| {
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

    fn scan_fallback_candidates(
        &mut self,
        test_chars: &[char],
        family_allowed: impl Fn(&str) -> bool,
        family_priority: impl Fn(&str) -> i16,
        max_results: usize,
    ) -> Vec<fontdb::ID> {
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
                    // 对 test_chars 多数表决：bitmap/vector 混合字体两种字形并存，单探测
                    // `中` 会误判。先用短借用收集 GID，db 借用释放后再探测轮廓缓存。
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

    pub(crate) fn find_glyph_anywhere(&mut self, ch: char) -> Option<(fontdb::ID, u16)> {
        let primary = self.font_id?;
        let db = self.font_system.db();
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
        // Sans 优先：同为 CJK 候选时衬线排后，避免中文落到 NotoSerifCJK。
        // 分数越高越优先（升序排完倒序取），故用加法。
        let locale_snapshot = self.system_locale_tag();
        candidates.sort_by_key(|(id, _)| {
            let family = db
                .face(*id)
                .and_then(|face| face.families.first().map(|(name, _)| name.to_lowercase()))
                .unwrap_or_default();
            i16::from(Self::is_cjk_candidate_family(&family)) * 100
                + cjk_family_priority(&family, locale_tag(&locale_snapshot))
        });
        // 返回第一个真正能渲染的候选：彩色字体（如 Noto Color Emoji）的 charmap 命中
        // 无法被 swash 描边，必须跳过。
        for (id, gid) in candidates.into_iter().rev() {
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

    pub(crate) fn glyph_source_is_outline_cached(
        &mut self,
        font_id: fontdb::ID,
        glyph_id: swash::GlyphId,
    ) -> bool {
        let key = (
            font_id,
            glyph_id,
            super::raster_size_key(self.font_size * self.raster_scale.max(1.0)),
        );
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
        // 须与真实光栅路径（atlas.rs）一致：`raster_size = font_size * raster_scale`、
        // 仅 1:1 时 hint、仅 `Source::Outline`。若改用 `font_size` + `hint(true)` 且不加
        // Source 过滤，NotoSansCJK TTC 在 14sp 会命中内嵌 bitmap strike（is_vector=false），
        // 而图集始终以 hint(false) 按 raster_size 光栅化矢量轮廓，导致高密度屏上
        // `try_cjk_outline_fallback` 跳过全部 CJK。
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
            assert!(!(nerd && cjk), "{name} should not be both nerd and cjk");
            assert!(!(emoji && cjk), "{name} should not be both emoji and cjk");
            assert!(
                !(sym && emoji),
                "{name} should not be both symbol and emoji"
            );
        }
    }

    // ── 大小写：fontdb 返回的族名均为小写 ──────────────────────────────
    #[test]
    fn case_sensitive_nerd() {
        assert!(!FontPipeline::is_nerd_candidate_family("Nerd"));
        assert!(FontPipeline::is_nerd_candidate_family("nerd"));
    }

    // ── 空/极简字符串 ──────────────────────────────────────────────────
    #[test]
    fn empty_string_cjk_is_candidate() {
        assert!(FontPipeline::is_cjk_candidate_family(""));
    }

    #[test]
    fn empty_string_rejects_symbol_nerd_emoji() {
        assert!(!FontPipeline::is_symbol_candidate_family(""));
        assert!(!FontPipeline::is_nerd_candidate_family(""));
        assert!(!FontPipeline::is_emoji_candidate_family(""));
    }
}

/// 把系统 locale 标签映射为 locale 加成所用的 CJK 变体标记
/// （`sc`/`tc`/`jp`/`kr`，非 CJK locale 为空）。
fn locale_tag(system_locale: &str) -> &'static str {
    match system_locale {
        s if s.starts_with("zh-CN") || s.starts_with("zh-Hans") => "sc",
        s if s.starts_with("zh-TW") || s.starts_with("zh-Hant") || s.starts_with("zh-HK") => "tc",
        s if s.starts_with("zh") => "sc",
        s if s.starts_with("ja") => "jp",
        s if s.starts_with("ko") => "kr",
        _ => "",
    }
}

/// `family_name` 是否以独立 token 形式包含 `locale_tag`（按非字母数字切分）；
/// 避免 `misc` 误配 `sc`（含子串 `sc` 但非 token `sc`）。
pub(crate) fn locale_token_match(family_name: &str, locale_tag: &str) -> bool {
    if locale_tag.is_empty() {
        return false;
    }
    family_name
        .split(|c: char| !c.is_ascii_alphanumeric())
        .any(|token| token == locale_tag)
}

/// CJK 回退族优先级（越高越优先）。无衬线 CJK 压过衬线：衬线在无衬线/等宽终端
/// 字体旁呈现为宋体，观感突兀（用户反馈“中文显示为宋体”——NotoSansCJK 与
/// NotoSerifCJK 同在 /system/fonts，原先同分让加载顺序选中了 Serif）。
fn cjk_family_priority(family_name: &str, locale_tag: &str) -> i16 {
    let is_locale_match = locale_token_match(family_name, locale_tag);
    let locale_boost: i16 = if is_locale_match { CJK_LOCALE_BONUS } else { 0 };
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
    base_priority + locale_boost
}

#[cfg(test)]
mod cjk_priority_tests {
    use super::*;

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
        // `misc` 含子串 `sc` 但非 token `sc`，不得加成。
        assert!(!locale_token_match("misc", "sc"));
        assert!(!locale_token_match("misc symbols", "sc"));
        assert!(locale_token_match("noto sans cjk sc", "sc"));
        assert!(locale_token_match("noto sans cjk jp", "jp"));
        assert!(!locale_token_match("noto sans cjk jp", "sc"));
    }

    #[test]
    fn serif_penalty_guards_vector_vs_bitmap() {
        // Sans 最差（bitmap）= 5-20 = -15；Serif 最好（vector）= 5-32+10 = -17 → Sans 仍胜。
        let sans_bitmap = cjk_family_priority("noto sans cjk", "") - CJK_BITMAP_PENALTY as i16;
        let serif_vector = cjk_family_priority("noto serif cjk", "") + OUTLINE_BONUS as i16;
        assert!(
            sans_bitmap > serif_vector,
            "Sans bitmap ({sans_bitmap}) must beat Serif vector ({serif_vector})"
        );
    }

    #[test]
    fn locale_boost_applies() {
        assert!(
            cjk_family_priority("droid sans fallback", "")
                < cjk_family_priority("noto sans cjk jp", "jp")
        );
    }

    #[test]
    fn unknown_family_gets_fallback_priority() {
        assert_eq!(
            cjk_family_priority("some han font", ""),
            CJK_PRIORITY_FALLBACK as i16
        );
    }

    /// 全库扫描的排序分必须让 Sans 压过 Serif：en-US 下 CJK 层被跳过，
    /// 中文只能靠 `find_glyph_anywhere`。
    #[test]
    fn anywhere_scan_score_prefers_sans_over_serif() {
        let score = |family: &str| {
            i16::from(FontPipeline::is_cjk_candidate_family(family)) * 100
                + cjk_family_priority(family, "")
        };
        assert!(
            score("noto sans cjk sc") > score("noto serif cjk sc"),
            "anywhere scan must rank Sans CJK above Serif CJK"
        );
        assert!(
            score("noto sans cjk sc") > score("noto color emoji"),
            "anywhere scan must rank CJK above emoji"
        );
    }
}
