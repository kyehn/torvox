//! FontPipeline — font loading, glyph rasterization, and atlas management.

use cosmic_text::FontSystem;

use super::font_db;
use super::{
    CJK_IDEOGRAPHIC_START, GlyphInfo, GlyphKey, GlyphSynthesis, PREFERRED_MONOSPACE_FONTS,
};

pub struct FontPipeline {
    pub(crate) font_system: FontSystem,
    pub(crate) scaler_context: swash::scale::ScaleContext,
    pub(crate) atlas: guillotiere::AtlasAllocator,
    pub(crate) caches: super::glyph_cache::GlyphCache,
    pub(crate) atlas_bitmap: Vec<u8>,
    pub(crate) atlas_width: u32,
    pub(crate) atlas_height: u32,
    pub(crate) font_id: Option<fontdb::ID>,
    pub(crate) cjk_fallback_ids: Vec<fontdb::ID>,
    /// Symbol layer (round-227 T5, moke chain: primary → CJK → symbols →
    /// Nerd → emoji → db scan): generic symbol fonts (Noto Sans Symbols 2
    /// and friends) for ▶ ⏵ ♥ ★ and similar terminal glyphs.
    pub(crate) symbol_fallback_ids: Vec<fontdb::ID>,
    /// Nerd layer: Nerd-patched families for private-use-area glyphs
    /// (U+E000..U+F8FF — powerline separators, devicons, file icons).
    pub(crate) nerd_fallback_ids: Vec<fontdb::ID>,
    /// Emoji layer: NotoColorEmoji-style families. swash cannot outline
    /// color glyphs, so these are tried and skipped at render time; the
    /// layer exists so the chain matches moke's "try emoji" semantics
    /// and the database scan / .notdef takes over.
    pub(crate) emoji_fallback_ids: Vec<fontdb::ID>,
    pub(crate) font_size: f32,
    pub(crate) raster_scale: f32,
    pub(crate) atlas_generation: u64,
    pub(crate) dirty_rect: Option<(u32, u32, u32, u32)>,
    system_locale: String,
    pub(crate) shaping_buffer: Option<cosmic_text::Buffer>,
}

impl FontPipeline {
    pub fn new(atlas_width: i32, atlas_height: i32, font_size: f32) -> Self {
        #[cfg(target_os = "android")]
        let mut db = font_db::load_font_database();

        #[cfg(target_os = "android")]
        {
            let extra = font_db::EXTRA_FONT_PATHS.read();
            let mut extra_loaded = 0usize;
            for path in extra.iter() {
                if path.is_file() {
                    if let Err(error) = db.load_font_file(path) {
                        // File name only: the full path can embed a user home dir (round-109).
                        log::warn!(
                            "font: failed to load font file {}: {error}",
                            path.file_name().unwrap_or_default().to_string_lossy()
                        );
                    } else {
                        extra_loaded += 1;
                    }
                } else if path.is_dir()
                    && let Ok(entries) = std::fs::read_dir(path)
                {
                    for entry in entries.flatten() {
                        let file_path = entry.path();
                        if is_font_file(&file_path) {
                            if let Err(error) = db.load_font_file(&file_path) {
                                // File name only: the full path can embed a user home dir (round-109).
                                log::warn!(
                                    "font: failed to load font file {}: {error}",
                                    file_path.file_name().unwrap_or_default().to_string_lossy()
                                );
                            } else {
                                extra_loaded += 1;
                            }
                        }
                    }
                }
            }
            log::debug!(
                "FONT_EXTRA: loaded {extra_loaded} fonts from {} extra paths",
                extra.len()
            );
        }

        #[cfg(not(target_os = "android"))]
        let db = {
            let mut db = fontdb::Database::new();
            db.load_system_fonts();
            db
        };

        let font_system = FontSystem::new_with_locale_and_db(String::new(), db);

        let scaler_context = swash::scale::ScaleContext::new();
        let atlas = guillotiere::AtlasAllocator::new(guillotiere::size2(atlas_width, atlas_height));
        let atlas_bitmap = vec![0u8; (atlas_width * atlas_height * 4) as usize];

        let mut pipeline = Self {
            font_system,
            scaler_context,
            atlas,
            caches: super::glyph_cache::GlyphCache::new(),
            atlas_bitmap,
            atlas_width: atlas_width as u32,
            atlas_height: atlas_height as u32,
            font_id: None,
            cjk_fallback_ids: Vec::new(),
            symbol_fallback_ids: Vec::new(),
            nerd_fallback_ids: Vec::new(),
            emoji_fallback_ids: Vec::new(),
            font_size,
            atlas_generation: 0,
            dirty_rect: None,
            system_locale: String::new(),
            shaping_buffer: None,
            raster_scale: 1.0,
        };

        if pipeline.font_id.is_none() {
            pipeline.find_monospace_font();
        }
        let system_locale = pipeline.system_locale.clone();
        pipeline.find_cjk_fallback_fonts(&system_locale);
        pipeline.find_symbol_fallback_fonts();
        pipeline.find_nerd_fallback_fonts();
        pipeline.find_emoji_fallback_fonts();
        pipeline.rasterize_ascii();
        pipeline
    }

    pub fn from_fixture(
        atlas_width: i32,
        atlas_height: i32,
        font_size: f32,
        fixture_dir: &str,
    ) -> Self {
        let mut font_system = FontSystem::new();
        let db = font_system.db_mut();

        let path = std::path::Path::new(fixture_dir);
        if path.is_dir()
            && let Ok(entries) = std::fs::read_dir(path)
        {
            for entry in entries.flatten() {
                let file_path = entry.path();
                if file_path
                    .extension()
                    .and_then(|e| e.to_str())
                    .is_some_and(|e| e.eq_ignore_ascii_case("ttf") || e.eq_ignore_ascii_case("otf"))
                    && let Err(error) = db.load_font_file(&file_path)
                {
                    // File name only: the full path can embed a user home dir (round-109).
                    log::warn!(
                        "font: failed to load font file {}: {error}",
                        file_path.file_name().unwrap_or_default().to_string_lossy()
                    );
                }
            }
        }

        let scaler_context = swash::scale::ScaleContext::new();
        let atlas = guillotiere::AtlasAllocator::new(guillotiere::size2(atlas_width, atlas_height));
        let atlas_bitmap = vec![0u8; (atlas_width * atlas_height * 4) as usize];

        let mut pipeline = Self {
            font_system,
            scaler_context,
            atlas,
            caches: super::glyph_cache::GlyphCache::new(),
            atlas_bitmap,
            atlas_width: atlas_width as u32,
            atlas_height: atlas_height as u32,
            font_id: None,
            cjk_fallback_ids: Vec::new(),
            symbol_fallback_ids: Vec::new(),
            nerd_fallback_ids: Vec::new(),
            emoji_fallback_ids: Vec::new(),
            font_size,
            atlas_generation: 0,
            dirty_rect: None,
            system_locale: String::new(),
            shaping_buffer: None,
            raster_scale: 1.0,
        };

        pipeline.find_monospace_font();
        let system_locale = pipeline.system_locale.clone();
        pipeline.find_cjk_fallback_fonts(&system_locale);
        pipeline.find_symbol_fallback_fonts();
        pipeline.find_nerd_fallback_fonts();
        pipeline.find_emoji_fallback_fonts();
        pipeline
    }

    fn find_monospace_font(&mut self) {
        let db = self.font_system.db();

        if let Some(target_filename) = font_db::resolve_system_monospace_from_fonts_xml() {
            let stem = std::path::Path::new(&target_filename)
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("");
            let stem_lower = stem.to_lowercase().replace(['-', '_'], " ");
            for face in db.faces() {
                if !face.monospaced {
                    continue;
                }
                let name = face
                    .families
                    .first()
                    .map(|(n, _)| n.to_lowercase())
                    .unwrap_or_default();
                let name_normalized = name.replace(['-', '_'], " ");
                if name_normalized == stem_lower || name_normalized.contains(&stem_lower) {
                    let display = face
                        .families
                        .first()
                        .map(|(n, _)| n.clone())
                        .unwrap_or_default();
                    log::debug!(
                        "FONT_SELECT: fonts.xml monospace id={:?} name='{}' (stem='{}')",
                        face.id,
                        display,
                        stem
                    );
                    self.font_id = Some(face.id);
                    return;
                }
            }
            for face in db.faces() {
                if !face.monospaced {
                    continue;
                }
                let name_nospace: String = face
                    .families
                    .first()
                    .map(|(n, _)| {
                        n.to_lowercase()
                            .chars()
                            .filter(|c| !c.is_whitespace())
                            .collect()
                    })
                    .unwrap_or_default();
                let stem_nospace = stem_lower.replace(' ', "");
                if name_nospace == stem_nospace {
                    let display = face
                        .families
                        .first()
                        .map(|(n, _)| n.clone())
                        .unwrap_or_default();
                    log::debug!(
                        "FONT_SELECT: fonts.xml monospace (nospace) id={:?} name='{}'",
                        face.id,
                        display
                    );
                    self.font_id = Some(face.id);
                    return;
                }
            }
        }

        for face in db.faces() {
            if !face.monospaced {
                continue;
            }
            let name = face
                .families
                .first()
                .map(|(n, _)| n.to_lowercase())
                .unwrap_or_default();
            if PREFERRED_MONOSPACE_FONTS.iter().any(|p| name.contains(p)) {
                let display = face
                    .families
                    .first()
                    .map(|(n, _)| n.clone())
                    .unwrap_or_default();
                log::debug!(
                    "FONT_SELECT: preferred monospace id={:?} name='{}'",
                    face.id,
                    display
                );
                self.font_id = Some(face.id);
                return;
            }
        }

        for face in db.faces() {
            if face.monospaced {
                let name = face
                    .families
                    .first()
                    .map(|(n, _)| n.to_lowercase())
                    .unwrap_or_default();
                if name.contains("cjk")
                    || name.contains("sc")
                    || name.contains("tc")
                    || name.contains("jp")
                    || name.contains("kr")
                    || name.contains("han")
                {
                    continue;
                }
                let display = face
                    .families
                    .first()
                    .map(|(n, _)| n.clone())
                    .unwrap_or_default();
                log::debug!("FONT_SELECT: monospace id={:?} name='{}'", face.id, display);
                self.font_id = Some(face.id);
                return;
            }
        }

        for face in db.faces() {
            if face.monospaced {
                let name = face
                    .families
                    .first()
                    .map(|(n, _)| n.clone())
                    .unwrap_or_default();
                log::debug!(
                    "FONT_SELECT: monospace (CJK ok) id={:?} name='{}'",
                    face.id,
                    name
                );
                self.font_id = Some(face.id);
                return;
            }
        }

        if let Some(face) = db.faces().next() {
            let name = face
                .families
                .first()
                .map(|(n, _)| n.clone())
                .unwrap_or_default();
            log::warn!(
                "FONT_SELECT: fallback to any face id={:?} name='{}'",
                face.id,
                name
            );
            self.font_id = Some(face.id);
            return;
        }

        log::error!("FONT_SELECT: no font found in system!");
    }

    pub fn set_font_family(&mut self, family_name: &str) -> bool {
        self.caches.shape_cache.clear();
        self.caches.glyph_id_cache.clear();
        self.caches.cjk_glyph_cache.clear();
        self.caches.ascii_glyph_ids = [None; 128];
        if family_name.is_empty() {
            self.font_id = None;
            self.find_monospace_font();
            self.caches.glyph_cache.clear();
            self.atlas = guillotiere::AtlasAllocator::new(guillotiere::size2(
                self.atlas_width as i32,
                self.atlas_height as i32,
            ));
            self.atlas_bitmap.fill(0);
            self.atlas_generation = self.atlas_generation.wrapping_add(1);
            self.reset_dirty_rect_full();
            self.cjk_fallback_ids.clear();
            self.symbol_fallback_ids.clear();
            self.nerd_fallback_ids.clear();
            self.emoji_fallback_ids.clear();
            let system_locale = self.system_locale.clone();
            self.find_cjk_fallback_fonts(&system_locale);
            self.find_symbol_fallback_fonts();
            self.find_nerd_fallback_fonts();
            self.find_emoji_fallback_fonts();
            self.rasterize_ascii();
            return true;
        }
        let found = {
            let db = self.font_system.db_mut();
            Self::find_font_by_name(db, family_name)
        };
        if let Some(id) = found {
            let db = self.font_system.db();
            let name = db
                .face(id)
                .and_then(|f| f.families.first().map(|(n, _)| n.clone()))
                .unwrap_or_default();
            log::debug!(
                "FONT_DIAG: set_font_family('{}') found id={:?} name='{}'",
                family_name,
                id,
                name
            );
            self.font_id = Some(id);
            self.caches.glyph_cache.clear();
            self.atlas = guillotiere::AtlasAllocator::new(guillotiere::size2(
                self.atlas_width as i32,
                self.atlas_height as i32,
            ));
            self.atlas_bitmap.fill(0);
            self.atlas_generation = self.atlas_generation.wrapping_add(1);
            self.reset_dirty_rect_full();
            self.cjk_fallback_ids.clear();
            self.symbol_fallback_ids.clear();
            self.nerd_fallback_ids.clear();
            self.emoji_fallback_ids.clear();
            let system_locale = self.system_locale.clone();
            self.find_cjk_fallback_fonts(&system_locale);
            self.find_symbol_fallback_fonts();
            self.find_nerd_fallback_fonts();
            self.find_emoji_fallback_fonts();
            self.rasterize_ascii();
            return true;
        }
        log::warn!(
            "FONT_DIAG: set_font_family('{}') NOT FOUND in fontdb",
            family_name
        );
        false
    }

    pub fn set_system_locale(&mut self, locale: &str) {
        self.caches.shape_cache.clear();
        self.caches.glyph_id_cache.clear();
        self.caches.cjk_glyph_cache.clear();
        self.caches.ascii_glyph_ids = [None; 128];
        self.system_locale = locale.to_string();
        self.cjk_fallback_ids.clear();
        self.find_cjk_fallback_fonts(&self.system_locale.clone());
    }

    pub fn set_font_size_in_place(&mut self, new_size: f32) -> (f32, f32) {
        self.font_size = new_size;
        self.caches.shape_cache.clear();
        self.caches.glyph_cache.clear();
        self.atlas = guillotiere::AtlasAllocator::new(guillotiere::size2(
            self.atlas_width as i32,
            self.atlas_height as i32,
        ));
        self.atlas_bitmap.fill(0);
        self.atlas_generation = self.atlas_generation.wrapping_add(1);
        self.reset_dirty_rect_full();
        self.rasterize_ascii();
        let (cw, ch) = self.cell_metrics();
        log::debug!(
            "FONT_SIZE_IN_PLACE: size={} cell={:.1}x{:.1}",
            new_size,
            cw,
            ch
        );
        (cw, ch)
    }

    pub fn set_raster_scale(&mut self, scale: f32) {
        let scale = if scale > 0.0 && scale.is_finite() {
            scale
        } else {
            1.0
        };
        if (scale - self.raster_scale).abs() < 1e-3 {
            return;
        }
        self.raster_scale = scale;
        self.caches.shape_cache.clear();
        self.caches.glyph_cache.clear();
        self.atlas = guillotiere::AtlasAllocator::new(guillotiere::size2(
            self.atlas_width as i32,
            self.atlas_height as i32,
        ));
        self.atlas_bitmap.fill(0);
        self.atlas_generation = self.atlas_generation.wrapping_add(1);
        self.reset_dirty_rect_full();
        self.rasterize_ascii();
        log::debug!("RASTER_SCALE: scale={:.3}", scale);
    }

    pub fn get_raster_scale(&self) -> f32 {
        self.raster_scale.max(f32::EPSILON)
    }

    pub fn current_font_family_name(&self) -> Option<String> {
        let font_id = self.font_id?;
        let db = self.font_system.db();
        let face_info = db.face(font_id)?;
        let family = face_info.families.first()?;
        Some(family.0.clone())
    }

    pub fn default_font_name(&self) -> String {
        if let Some(id) = self.font_id
            && let Some(name) = self
                .font_system
                .db()
                .face(id)
                .and_then(|f| f.families.first().map(|(n, _)| n.clone()))
        {
            return name;
        }
        self.system_monospace_name()
    }

    pub fn system_monospace_name(&self) -> String {
        let db = self.font_system.db();
        for face in db.faces() {
            if !face.monospaced {
                continue;
            }
            let name = face
                .families
                .first()
                .map(|(n, _)| n.to_lowercase())
                .unwrap_or_default();
            if PREFERRED_MONOSPACE_FONTS.iter().any(|p| name.contains(p)) {
                return face
                    .families
                    .first()
                    .map(|(n, _)| n.clone())
                    .unwrap_or_default();
            }
        }
        for face in db.faces() {
            if face.monospaced {
                return face
                    .families
                    .first()
                    .map(|(n, _)| n.clone())
                    .unwrap_or_default();
            }
        }
        "monospace".to_string()
    }

    pub fn cjk_fallback_names(&self) -> Vec<String> {
        let db = self.font_system.db();
        let mut raw_names: Vec<String> = self
            .cjk_fallback_ids
            .iter()
            .filter_map(|&id| {
                let face = db.face(id)?;
                face.families.first().map(|(name, _)| name.clone())
            })
            .collect();
        raw_names.sort();
        raw_names.dedup();
        let mut normalized = Vec::new();
        let mut seen_generic = false;
        for name in raw_names {
            let lower = name.to_lowercase();
            if lower.contains("cjk") {
                if !seen_generic {
                    normalized.push("Noto Sans CJK".to_string());
                    seen_generic = true;
                }
            } else {
                normalized.push(name);
            }
        }
        normalized
    }

    pub fn font_information(&self) -> String {
        let db = self.font_system.db();
        let mut parts = Vec::new();
        if let Some(id) = self.font_id
            && let Some(face) = db.face(id)
        {
            let name = face.families.first().map_or("unknown", |(n, _)| n.as_str());
            let mono = if face.monospaced {
                "monospaced"
            } else {
                "proportional"
            };
            parts.push(format!("Active: {} ({})", name, mono));
        }
        let cjk = self.cjk_fallback_names();
        if !cjk.is_empty() {
            parts.push(format!("CJK fallback: {}", cjk.join(", ")));
        } else {
            let primary_is_cjk = self.font_id.and_then(|id| db.face(id)).is_some_and(|face| {
                face.families.iter().any(|(name, _)| {
                    let l = name.to_lowercase();
                    l.contains("cjk")
                        || l.contains("chinese")
                        || l.contains("japanese")
                        || l.contains("korean")
                        || l.contains(" sc")
                        || l.contains(" tc")
                        || l.contains(" jp")
                        || l.contains(" kr")
                })
            });
            if primary_is_cjk {
                parts.push("CJK fallback: skipped (primary font supports CJK)".to_string());
            } else {
                parts.push("CJK fallback: none".to_string());
            }
        }
        let (cw, ch) = self.cell_metrics();
        parts.push(format!("Cell: {:.1}x{:.1}px", cw, ch));
        parts.push(format!("Font size: {:.1}px", self.font_size));
        parts.join("\n")
    }

    pub fn list_all_font_families(&self) -> Vec<String> {
        let db = self.font_system.db();
        let mut families = Vec::new();
        let mut seen = std::collections::HashSet::new();
        for face in db.faces() {
            for (family, _) in &face.families {
                if seen.insert(family.to_lowercase()) {
                    families.push(family.clone());
                }
            }
        }
        families.sort();
        families
    }

    pub fn font_size(&self) -> f32 {
        self.font_size
    }

    pub fn glyph_information(&mut self, ch: char) -> Option<GlyphInfo> {
        self.glyph_information_with_synthesis(ch, GlyphSynthesis::None)
    }

    /// Style-aware glyph lookup (round-231 T6): prefers a real bold/italic
    /// face of the same family when one exists, otherwise rasterizes the
    /// base face with font synthesis (embolden/shear).
    pub fn glyph_information_styled(
        &mut self,
        ch: char,
        bold: bool,
        italic: bool,
    ) -> Option<GlyphInfo> {
        if !bold && !italic {
            return self.glyph_information(ch);
        }
        let primary_font_id = self.font_id?;

        // 1) Same-family bold/italic face wins when it actually contains the
        //    glyph (this keeps real styled faces crisp and hintable).
        if let Some(style_id) = self.resolve_style_face(primary_font_id, bold, italic) {
            let db = self.font_system.db();
            let style_gid = db
                .with_face_data(style_id, |font_data, face_index| {
                    let font_ref = swash::FontRef::from_index(font_data, face_index as usize)?;
                    Some(font_ref.charmap().map(ch))
                })
                .flatten();
            if let Some(gid) = style_gid.filter(|&g| g != 0)
                && let Some(info) = self.glyph_information_from_font_with_synthesis(
                    style_id,
                    gid,
                    GlyphSynthesis::None,
                )
                && info.width > 0
                && info.height > 0
            {
                return Some(info);
            }
        }

        // 2) Fall back to synthesizing the base (or fallback) face.
        let synthesis = match (bold, italic) {
            (true, true) => GlyphSynthesis::BoldItalic,
            (true, false) => GlyphSynthesis::Bold,
            (false, true) => GlyphSynthesis::Italic,
            (false, false) => GlyphSynthesis::None,
        };
        self.glyph_information_with_synthesis(ch, synthesis)
    }

    /// Find a face of the same font family with the requested weight/style.
    /// Returns the same id when the base face already matches, and None
    /// when no matching face exists (caller then uses synthesis).
    pub(crate) fn resolve_style_face(
        &self,
        base_id: fontdb::ID,
        bold: bool,
        italic: bool,
    ) -> Option<fontdb::ID> {
        let db = self.font_system.db();
        let base = db.face(base_id)?;
        let family = base.families.first()?.0.clone();
        let query = fontdb::Query {
            families: &[fontdb::Family::Name(&family)],
            weight: if bold {
                fontdb::Weight::BOLD
            } else {
                fontdb::Weight::NORMAL
            },
            stretch: fontdb::Stretch::Normal,
            style: if italic {
                fontdb::Style::Italic
            } else {
                fontdb::Style::Normal
            },
        };
        let matched = db.query(&query)?;
        if matched == base_id {
            None
        } else {
            Some(matched)
        }
    }

    fn glyph_information_with_synthesis(
        &mut self,
        ch: char,
        synthesis: GlyphSynthesis,
    ) -> Option<GlyphInfo> {
        let primary_font_id = self.font_id?;
        let pixel_size = (self.font_size * self.raster_scale) as u16;
        let has_cjk_fallback = !self.cjk_fallback_ids.is_empty();
        let synthesized = synthesis != GlyphSynthesis::None;

        // ── Fast path: ASCII with cached glyph_id —─────────────────────────
        if (ch as u32) < 128
            && let Some(gid) = self.caches.ascii_glyph_ids[ch as usize]
        {
            let key = GlyphKey {
                font_id: primary_font_id,
                glyph_id: gid,
                pixel_size,
                synthesis: synthesis.bits(),
            };
            if let Some(info) = self.caches.glyph_cache.get(&key).cloned() {
                return Some(info);
            }
        }

        // ── CJK cache: skip swash + fallback for already-resolved chars ─────
        // (skipped when synthesizing — the cached (font, glyph) pair was
        // resolved for the regular style and does not apply to styled runs)
        if !synthesized
            && (ch as u32) >= CJK_IDEOGRAPHIC_START
            && has_cjk_fallback
            && let Some(&(cached_font_id, cached_glyph_id)) = self.caches.cjk_glyph_cache.get(&ch)
        {
            let key = GlyphKey {
                font_id: cached_font_id,
                glyph_id: cached_glyph_id,
                pixel_size,
                synthesis: synthesis.bits(),
            };
            if let Some(info) = self.caches.glyph_cache.get(&key).cloned() {
                return Some(info);
            }
        }

        // ── Resolve glyph_id (cached if possible) ───────────────────────────
        let glyph_id = if let Some(&cached) = self.caches.glyph_id_cache.get(&(ch as u32)) {
            cached
        } else {
            let gid = {
                let db = self.font_system.db();
                db.with_face_data(primary_font_id, |font_data, face_index| {
                    let font_ref = swash::FontRef::from_index(font_data, face_index as usize)?;
                    let charmap = font_ref.charmap();
                    Some(charmap.map(ch))
                })?
            }?;
            self.caches.glyph_id_cache.put(ch as u32, gid);
            gid
        };

        // Cache ASCII glyph_id for future fast-path lookups.
        if (ch as u32) < 128 {
            self.caches.ascii_glyph_ids[ch as usize] = Some(glyph_id);
        }

        // ── Check glyph_cache before expensive CJK ops ──────────────────────
        // PUA (Nerd Font) characters skip this fast path: the cache may
        // hold the primary font's .notdef (tofu) from before a Nerd Font
        // was installed/loaded (round-227 T5).
        let is_nerd_pua = (ch as u32) >= 0xE000 && (ch as u32) <= 0xF8FF;
        if !is_nerd_pua {
            let key = GlyphKey {
                font_id: primary_font_id,
                glyph_id,
                pixel_size,
                synthesis: synthesis.bits(),
            };
            if let Some(info) = self.caches.glyph_cache.get(&key).cloned() {
                if !synthesized && (ch as u32) >= CJK_IDEOGRAPHIC_START {
                    self.caches
                        .cjk_glyph_cache
                        .put(ch, (primary_font_id, glyph_id));
                }
                return Some(info);
            }
        }

        // ── CJK: check outline, try fallback ────────────────────────────────
        // (skipped when synthesizing: the outline-fallback face resolves
        // the regular glyph shape; styled runs keep the base path so the
        // synthesis applies to the rasterized mask)
        if !synthesized && glyph_id != 0 && (ch as u32) >= CJK_IDEOGRAPHIC_START && has_cjk_fallback
        {
            let is_outline = self.glyph_source_is_outline(primary_font_id, glyph_id);
            if !is_outline && let Some(fallback_info) = self.try_cjk_outline_fallback(ch) {
                return Some(fallback_info);
            }
        }

        // ── glyph_id == 0: search the layered fallback chain ────────────────
        // (round-227 T5, moke chain: primary → CJK → symbols → Nerd →
        // emoji → whole-database scan; spec d7)
        //
        // PUA (Nerd Font private-use area U+E000..U+F8FF) characters must
        // also route through the chain even when the primary font maps
        // them: most fonts map the PUA to .notdef (the tofu box), so a
        // non-zero glyph_id from the primary is not a real glyph.
        if glyph_id == 0 || is_nerd_pua {
            // Collect the layered ids first: the chain borrows self's
            // fields, which would conflict with the &mut self render call
            // inside the loop.
            let fallback_ids: Vec<fontdb::ID> = self
                .cjk_fallback_ids
                .iter()
                .chain(&self.symbol_fallback_ids)
                .chain(&self.nerd_fallback_ids)
                .chain(&self.emoji_fallback_ids)
                .copied()
                .collect();
            for &fallback_id in &fallback_ids {
                let fallback_glyph = {
                    let db = self.font_system.db();
                    db.with_face_data(fallback_id, |font_data, face_index| {
                        let font_ref = swash::FontRef::from_index(font_data, face_index as usize)?;
                        let charmap = font_ref.charmap();
                        Some(charmap.map(ch))
                    })
                };
                if let Some(Some(fid)) = fallback_glyph
                    && fid != 0
                    && let Some(result) =
                        self.glyph_information_from_font_with_synthesis(fallback_id, fid, synthesis)
                    && result.width > 0
                    && result.height > 0
                {
                    let db = self.font_system.db();
                    let face_name = db
                        .face(fallback_id)
                        .and_then(|f| f.families.first())
                        .map(|(n, _)| n.clone())
                        .unwrap_or_else(|| "?".to_string());
                    log::debug!(
                        "FALLBACK_HIT: ch=U+{:04X} layer='{}' gid={} w={} h={}",
                        ch as u32,
                        face_name,
                        fid,
                        result.width,
                        result.height,
                    );
                    if !synthesized {
                        self.caches.cjk_glyph_cache.put(ch, (fallback_id, fid));
                    }
                    return Some(result);
                    // Rendering failed (e.g. a color font swash cannot
                    // outline): try the next layer instead of returning a
                    // 0x0 placeholder (round-227 T5).
                }
            }
            // Whole-database scan tail of the chain (spec d7: "ending with
            // a whole-font-database scan"). Cached by cjk_glyph_cache, so
            // it runs once per character.
            if let Some((scan_id, scan_gid)) = self.find_glyph_anywhere(ch)
                && let Some(result) =
                    self.glyph_information_from_font_with_synthesis(scan_id, scan_gid, synthesis)
                && result.width > 0
                && result.height > 0
            {
                if !synthesized {
                    self.caches.cjk_glyph_cache.put(ch, (scan_id, scan_gid));
                }
                return Some(result);
            }
        }

        // ── Fallback to primary font ────────────────────────────────────────
        let result =
            self.glyph_information_from_font_with_synthesis(primary_font_id, glyph_id, synthesis)?;
        if !synthesized && (ch as u32) >= CJK_IDEOGRAPHIC_START {
            self.caches
                .cjk_glyph_cache
                .put(ch, (primary_font_id, glyph_id));
        }
        Some(result)
    }

    pub fn glyph_information_for_glyph(
        &mut self,
        font_id: fontdb::ID,
        glyph_id: u16,
    ) -> Option<GlyphInfo> {
        self.glyph_information_from_font(font_id, '\0', glyph_id)
    }

    pub fn list_monospace_fonts(&self) -> Vec<String> {
        let db = self.font_system.db();
        let mut fonts = Vec::new();
        for face in db.faces() {
            #[cfg(not(target_os = "android"))]
            if !face.monospaced {
                continue;
            }
            for (family, _) in &face.families {
                let name = family.to_string();
                if !fonts.contains(&name) {
                    fonts.push(name);
                }
            }
        }
        fonts.sort();
        fonts
    }

    fn find_font_by_name(db: &fontdb::Database, family_name: &str) -> Option<fontdb::ID> {
        for face in db.faces() {
            for (family, _) in &face.families {
                if family.eq_ignore_ascii_case(family_name) {
                    return Some(face.id);
                }
            }
        }
        let cleaned = family_name.replace(['_', '-'], " ").trim().to_lowercase();
        for face in db.faces() {
            for (family, _) in &face.families {
                let fam_lower = family.to_lowercase();
                if fam_lower == cleaned || fam_lower.contains(&cleaned) {
                    return Some(face.id);
                }
            }
        }
        let cleaned_nospace: String = cleaned.chars().filter(|c| !c.is_whitespace()).collect();
        for face in db.faces() {
            for (family, _) in &face.families {
                let fam_nospace: String = family
                    .to_lowercase()
                    .chars()
                    .filter(|c| !c.is_whitespace())
                    .collect();
                if fam_nospace == cleaned_nospace {
                    return Some(face.id);
                }
            }
        }
        if family_name.eq_ignore_ascii_case("monospace") {
            for face in db.faces() {
                if face.monospaced {
                    return Some(face.id);
                }
            }
        }
        None
    }

    pub fn load_font_file(&mut self, path: &std::path::Path) -> Option<String> {
        let db = self.font_system.db_mut();
        let source = fontdb::Source::File(path.into());
        let ids = db.load_font_source(source);
        let first_id = ids.first()?;
        let face = db.face(*first_id)?;
        face.families.first().map(|(name, _)| name.clone())
    }

    pub fn has_font(&self) -> bool {
        self.font_id.is_some()
    }
}

#[cfg(target_os = "android")]
fn is_font_file(entry: &std::path::Path) -> bool {
    entry
        .extension()
        .and_then(|ext| ext.to_str())
        .is_some_and(|ext| {
            ext.eq_ignore_ascii_case("ttf")
                || ext.eq_ignore_ascii_case("otf")
                || ext.eq_ignore_ascii_case("ttc")
        })
}
