//! CJK 回退字体：只取 `/system/etc/fonts.xml` 的 `lang` 回退条目。
//!
//! 字体库内容取自 fonts.xml 声明的文件集（见 `font_db::load_font_database`），
//! 按文档顺序排列，因此「按库顺序找字形」天然就是平台给出的优先级，
//! 不需要任何族名子串启发式或打分工。
use super::{FontPipeline, GlyphInfo};

/// CJK 回退层上限：fonts.xml 的 `lang` 回退条目按语言给出，超出无意义。
const MAX_CJK_FALLBACK_FONTS: usize = 3;

impl FontPipeline {
    pub(crate) fn find_cjk_fallback_fonts(&mut self, system_locale: &str) {
        if system_locale.is_empty() {
            log::debug!("CJK_FALLBACK: skipped (locale not yet known)");
            return;
        }
        let locale_tag = locale_tag(system_locale);
        if locale_tag.is_empty() {
            log::debug!("CJK_FALLBACK: skipped (non-CJK locale)");
            return;
        }

        if let Some(primary_id) = self.font_id {
            let font_database = self.font_system.db();
            // 探测 locale 的代表字符：CJK 字体按 locale 分片（CN 字体未必覆盖
            // 谚文音节），故不要求中/日/가同时存在，否则会拒绝匹配的主字体。
            let probe = if locale_tag == "kr" { '가' } else { '中' };
            let primary_supports_cjk = font_database
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

        let fonts_xml = super::font_db::read_fonts_xml();
        self.cjk_fallback_ids = fonts_xml.as_deref().map_or_else(Vec::new, |xml| {
            Self::match_fonts_xml_fallbacks(
                self.font_system.db(),
                xml,
                system_locale,
                MAX_CJK_FALLBACK_FONTS,
            )
        });
        if self.cjk_fallback_ids.is_empty() {
            #[cfg(any(target_os = "android", test))]
            let has_pending_region = fonts_xml.as_deref().is_some_and(|xml| {
                !super::font_db::missing_region_fallback_faces(
                    self.font_system.db(),
                    xml,
                    system_locale,
                )
                .is_empty()
            });
            #[cfg(not(any(target_os = "android", test)))]
            let has_pending_region = false;
            if has_pending_region {
                log::debug!("CJK_FALLBACK: 区域回退族尚未补装，等待 locale 补装");
            } else {
                log::warn!("CJK_FALLBACK: fonts.xml 未提供匹配当前语言的回退字体");
            }
        }
        log::debug!(
            "CJK_FALLBACK: found {} fallback fonts",
            self.cjk_fallback_ids.len()
        );
    }

    pub(crate) fn match_fonts_xml_fallbacks(
        font_database: &fontdb::Database,
        xml: &str,
        system_locale: &str,
        max_results: usize,
    ) -> Vec<fontdb::ID> {
        let (_, lang_fallbacks) = super::font_db::parse_fonts_xml_families(xml);
        let langs = super::font_db::locale_fonts_xml_langs(system_locale);
        let mut ids = Vec::new();
        for wanted in langs {
            let Some((_, filenames)) = lang_fallbacks
                .iter()
                .find(|(lang, _)| lang.split(',').any(|tag| tag == *wanted))
            else {
                continue;
            };
            for (filename, index) in filenames {
                let same_file: Vec<(fontdb::ID, u32)> = font_database
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
                        Some((face.id, face.index))
                    })
                    .collect();
                // 纯 fonts.xml 精确命中：仅 (文件名, 索引) 一致才算，不做族名猜测。
                let hit = same_file
                    .iter()
                    .find(|(_, face_index)| *face_index == *index)
                    .map(|(id, _)| *id);
                if let Some(id) = hit
                    && !ids.contains(&id)
                {
                    log::debug!("FONTS_XML_FALLBACK: file='{filename}' index={index} id={id:?}");
                    ids.push(id);
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

    pub(crate) fn find_glyph_anywhere(&mut self, ch: char) -> Option<(fontdb::ID, u16)> {
        let primary = self.font_id?;
        let font_database = self.font_system.db();
        let mut candidates: Vec<(fontdb::ID, u16)> = Vec::new();
        for face in font_database.faces() {
            if face.id == primary {
                continue;
            }
            let gid = font_database
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
        // 库顺序即 fonts.xml 文档顺序，平台自身的优先级，不重排。
        // 彩色字体（如 Noto Color Emoji）的 charmap 命中无法被 swash 描边，
        // 逐个跳过直到第一个能真正渲染的候选。
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

    pub(crate) fn glyph_source_is_outline_cached(
        &mut self,
        font_id: fontdb::ID,
        glyph_id: swash::GlyphId,
    ) -> bool {
        let key = (
            font_id,
            glyph_id,
            super::raster_size_key(self.font_size * self.raster_scale),
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
        // 须与真实光栅路径（atlas.rs）逐项一致：`raster_size = font_size * raster_scale`、
        // 仅 1:1 时 hint、仅 `Source::Outline`。若改用 `font_size` + `hint(true)` 且不加
        // Source 过滤，NotoSansCJK TTC 在 14sp 会命中内嵌 bitmap strike（is_vector=false），
        // 而图集始终以 hint(false) 按 raster_size 光栅化矢量轮廓，导致高密度屏上
        // `try_cjk_outline_fallback` 跳过全部 CJK。
        // 此处曾额外 `.max(1.0)`，于是 `raster_scale < 1`（Kotlin 允许 0.5f..4f；
        // 例如 mdpi 的 1.0 density 配系统小字号 0.85 fontScale）时探测尺寸与图集尺寸不同，
        // 「是否内嵌 bitmap strike」的结论在两个尺寸之间翻转，CJK 回退随之误判。
        let raster_size = self.font_size * self.raster_scale;
        let hint = self.raster_scale <= 1.01;
        let font_database = self.font_system.db();
        let result = font_database.with_face_data(font_id, |font_data, face_index| {
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
            let font_database = self.font_system.db();
            self.cjk_fallback_ids
                .iter()
                .filter_map(|&fallback_id| {
                    let result =
                        font_database.with_face_data(fallback_id, |font_data, face_index| {
                            let font_ref =
                                swash::FontRef::from_index(font_data, face_index as usize)?;
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

/// 系统 locale → CJK 变体标记（`sc`/`tc`/`jp`/`kr`，非 CJK locale 为空）。
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
