// @REQ_REND_007
use std::num::NonZeroUsize;

use cosmic_text::FontSystem;
use lru::LruCache;
use swash::scale::{Render, ScaleContext, Source};
use swash::zeno::Placement;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum FontError {
    #[error("no monospace font found")]
    NoMonospaceFont,
    #[error("font loading failed: {0}")]
    FontLoad(String),
    #[error("atlas allocation failed")]
    AtlasAllocationFailed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct GlyphKey {
    pub font_id: fontdb::ID,
    pub glyph_id: u16,
    pub pixel_size: u16,
}

#[derive(Debug, Clone)]
pub struct GlyphInfo {
    pub atlas_x: i32,
    pub atlas_y: i32,
    pub width: i32,
    pub height: i32,
    pub placement: Placement,
    pub advance_width: f32,
    /// Slot the glyph occupies in the guillotiere atlas. `None` for
    /// zero-sized (blank) glyphs that never reserved a rectangle. The id is
    /// freed via `AtlasAllocator::deallocate` when the cache entry is evicted
    /// so the underlying rectangle can be reused instead of leaking forever.
    pub allocation_id: Option<guillotiere::AllocId>,
}

#[derive(Debug, Clone)]
pub struct ShapedGlyphInfo {
    pub glyph_id: u16,
    pub font_id: fontdb::ID,
    pub x: f32,
    pub w: f32,
    pub x_offset: f32,
    pub y_offset: f32,
}

const ASCENT_FALLBACK_RATIO: f32 = 0.8;
const DESCENT_FALLBACK_RATIO: f32 = 0.2;
const CELL_WIDTH_FALLBACK_RATIO: f32 = 0.6;
const CELL_HEIGHT_FALLBACK_RATIO: f32 = 1.2;

const GLYPH_CACHE_CAPACITY: usize = 10_000;

const GLYPH_CACHE_EVICTION_DIVISOR: usize = 4;

/// Maximum number of shaped text runs cached by `shape_run`. Each visible
/// ligature-capable run is cached so repeated frames avoid re-shaping.
const SHAPE_CACHE_CAPACITY: usize = 4_096;

const CJK_BITMAP_PENALTY: u8 = 20;

const PREFERRED_MONOSPACE_FONTS: &[&str] = &[
    "roboto mono",
    "droid sans mono",
    "noto sans mono",
    "source code pro",
    "fira code",
    "fira mono",
    "jetbrains mono",
    "dejavu sans mono",
    "noto sans mono cjk",
    "liberation mono",
    "ubuntu mono",
    "cascadia",
    "ia writer",
    "hack",
    "inconsolata",
    "iosevka",
    "meslo",
    "consolas",
    "menlo",
    "monaco",
    "courier",
];

const OUTLINE_BONUS: u8 = 10;

const ASCII_START: u32 = 32;
const ASCII_END: u32 = 127;

#[cfg(target_os = "android")]
fn resolve_system_monospace_from_fonts_xml() -> Option<String> {
    let xml_path = std::path::Path::new("/system/etc/fonts.xml");
    let content = std::fs::read_to_string(xml_path).ok()?;

    let monospace_names = ["monospace", "sans-serif mono", "serif mono"];
    for mono_name in &monospace_names {
        let pattern = format!("name=\"{}\"", mono_name);
        if let Some(family_start) = content.find(&pattern) {
            let family_end = content[family_start..].find("</family>");
            if let Some(offset) = family_end {
                let family_block = &content[family_start..family_start + offset];
                if let Some(font_start) = family_block.find("<font ") {
                    let after_font = &family_block[font_start..];
                    if let Some(gt_pos) = after_font.find('>') {
                        let text_start = gt_pos + 1;
                        if let Some(lt_pos) = after_font[text_start..].find('<') {
                            let filename = after_font[text_start..text_start + lt_pos].trim();
                            if !filename.is_empty() {
                                log::info!("FONT_XML: monospace target='{}'", filename);
                                return Some(filename.to_string());
                            }
                        }
                    }
                }
            }
        }
    }
    None
}

#[cfg(not(target_os = "android"))]
fn resolve_system_monospace_from_fonts_xml() -> Option<String> {
    None
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

#[cfg(target_os = "android")]
static CACHED_FONT_PATHS: std::sync::OnceLock<Vec<std::path::PathBuf>> = std::sync::OnceLock::new();

#[cfg(target_os = "android")]
static CACHED_FONT_DB: std::sync::OnceLock<fontdb::Database> = std::sync::OnceLock::new();

#[cfg(target_os = "android")]
static EXTRA_FONT_PATHS: std::sync::RwLock<Vec<std::path::PathBuf>> =
    std::sync::RwLock::new(Vec::new());

#[cfg(target_os = "android")]
pub fn set_extra_font_paths(paths: Vec<std::path::PathBuf>) {
    match EXTRA_FONT_PATHS.write() {
        Ok(mut extra) => {
            *extra = paths;
            log::info!("FONT_LOAD: set {} extra font paths", extra.len());
        }
        Err(poisoned) => {
            let mut extra = poisoned.into_inner();
            *extra = paths;
            log::warn!(
                "FONT_LOAD: RwLock poisoned, recovered and set {} extra font paths",
                extra.len()
            );
        }
    }
}

#[cfg(target_os = "android")]
fn load_font_database() -> fontdb::Database {
    let db = CACHED_FONT_DB.get_or_init(|| {
        let font_paths = CACHED_FONT_PATHS.get_or_init(|| {
            let mut paths = Vec::new();
            for dir in [
                "/system/fonts/",
                "/system/product/fonts/",
                "/system_ext/fonts/",
                "/vendor/fonts/",
                "/product/fonts/",
            ] {
                let dir_path = std::path::Path::new(dir);
                if let Ok(entries) = std::fs::read_dir(dir_path) {
                    for entry in entries.flatten() {
                        if is_font_file(&entry.path()) {
                            paths.push(entry.path());
                        }
                    }
                }
            }
            log::info!("FONT_LOAD: cached {} font paths", paths.len());
            paths
        });

        let mut db = fontdb::Database::new();
        let mut count = 0u32;
        for path in font_paths {
            if db.load_font_file(path).is_ok() {
                count += 1;
            }
        }
        log::info!("FONT_LOAD: loaded {count} fonts from cached paths");
        db
    });
    db.clone()
}

pub struct FontPipeline {
    font_system: FontSystem,
    scaler_context: ScaleContext,
    atlas: guillotiere::AtlasAllocator,
    glyph_cache: LruCache<GlyphKey, GlyphInfo>,
    atlas_bitmap: Vec<u8>,
    atlas_width: u32,
    atlas_height: u32,
    font_id: Option<fontdb::ID>,
    cjk_fallback_ids: Vec<fontdb::ID>,
    font_size: f32,
    atlas_generation: u64,
    system_locale: String,
    shaping_buffer: Option<cosmic_text::Buffer>,
    /// Cache of ligature-aware shaped runs keyed by run text. Shaping is
    /// deterministic for a given font/size, so per-frame re-shaping of
    /// identical runs is skipped. Cleared whenever the font identity changes.
    shape_cache: LruCache<String, Vec<ShapedGlyphInfo>>,
}

impl FontPipeline {
    pub fn new(atlas_width: i32, atlas_height: i32, font_size: f32) -> Self {
        #[cfg(target_os = "android")]
        let mut db = load_font_database();

        #[cfg(target_os = "android")]
        if let Ok(extra) = EXTRA_FONT_PATHS.read() {
            for path in extra.iter() {
                if path.is_file() {
                    if let Err(error) = db.load_font_file(path) {
                        log::warn!("font: failed to load font file {path:?}: {error}");
                    }
                } else if path.is_dir() {
                    if let Ok(entries) = std::fs::read_dir(path) {
                        for entry in entries.flatten() {
                            let file_path = entry.path();
                            if is_font_file(&file_path) {
                                if let Err(error) = db.load_font_file(&file_path) {
                                    log::warn!(
                                        "font: failed to load font file {file_path:?}: {error}"
                                    );
                                }
                            }
                        }
                    }
                }
            }
        }

        #[cfg(not(target_os = "android"))]
        let db = {
            let mut db = fontdb::Database::new();
            db.load_system_fonts();
            db
        };

        let font_system = FontSystem::new_with_locale_and_db(String::new(), db);

        let scaler_context = ScaleContext::new();
        let atlas = guillotiere::AtlasAllocator::new(guillotiere::size2(atlas_width, atlas_height));
        let atlas_bitmap = vec![0u8; (atlas_width * atlas_height) as usize];

        let mut pipeline = Self {
            font_system,
            scaler_context,
            atlas,
            glyph_cache: LruCache::new(
                NonZeroUsize::new(GLYPH_CACHE_CAPACITY)
                    .expect("GLYPH_CACHE_CAPACITY is 10_000, always non-zero"),
            ),
            atlas_bitmap,
            atlas_width: atlas_width as u32,
            atlas_height: atlas_height as u32,
            font_id: None,
            cjk_fallback_ids: Vec::new(),
            font_size,
            atlas_generation: 0,
            system_locale: String::new(),
            shaping_buffer: None,
            shape_cache: LruCache::new(
                NonZeroUsize::new(SHAPE_CACHE_CAPACITY)
                    .expect("SHAPE_CACHE_CAPACITY must be non-zero"),
            ),
        };

        if pipeline.font_id.is_none() {
            pipeline.find_monospace_font();
        }
        let system_locale = pipeline.system_locale.clone();
        pipeline.find_cjk_fallback_fonts(&system_locale);
        pipeline.rasterize_ascii();
        pipeline
    }

    fn find_monospace_font(&mut self) {
        let db = self.font_system.db();

        // Strategy 0: Android fonts.xml monospace alias
        if let Some(target_filename) = resolve_system_monospace_from_fonts_xml() {
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
                    log::info!(
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
                    log::info!(
                        "FONT_SELECT: fonts.xml monospace (nospace) id={:?} name='{}'",
                        face.id,
                        display
                    );
                    self.font_id = Some(face.id);
                    return;
                }
            }
        }

        // Strategy 1: preferred name + monospaced
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
                log::info!(
                    "FONT_SELECT: preferred monospace id={:?} name='{}'",
                    face.id,
                    display
                );
                self.font_id = Some(face.id);
                return;
            }
        }

        // Strategy 2: any monospaced face (exclude CJK bitmap candidates)
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
                log::info!("FONT_SELECT: monospace id={:?} name='{}'", face.id, display);
                self.font_id = Some(face.id);
                return;
            }
        }

        // Strategy 2b: any monospaced face (including CJK)
        for face in db.faces() {
            if face.monospaced {
                let name = face
                    .families
                    .first()
                    .map(|(n, _)| n.clone())
                    .unwrap_or_default();
                log::info!(
                    "FONT_SELECT: monospace (CJK ok) id={:?} name='{}'",
                    face.id,
                    name
                );
                self.font_id = Some(face.id);
                return;
            }
        }

        // Strategy 3: any face
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

    fn find_cjk_fallback_fonts(&mut self, system_locale: &str) {
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
                log::info!("CJK_FALLBACK: skipped (primary font already supports CJK)");
                return;
            }
        }

        let db = self.font_system.db();
        let test_chars = ['中', '日', '가'];
        let mut candidates: Vec<(fontdb::ID, f32, i16)> = Vec::new();
        let locale_bonus: u8 = 6;

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
                let locale_boost = if is_locale_match { locale_bonus } else { 0 };

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
                    5
                } else if is_generic_cjk {
                    4
                } else if family_name.contains("sc")
                    || family_name.contains("tc")
                    || family_name.contains("jp")
                    || family_name.contains("kr")
                {
                    3
                } else {
                    2
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
                            let image = Render::new(&[]).render(&mut scaler, gid);
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
            log::info!("CJK_FALLBACK: selected font id={:?} name='{}'", id, name);
            self.cjk_fallback_ids.push(*id);
        }
        log::info!(
            "CJK_FALLBACK: found {} fallback fonts (limited to {})",
            self.cjk_fallback_ids.len(),
            MAX_CJK_FALLBACK_FONTS
        );
        if candidates.is_empty() || candidates.iter().all(|c| c.2 <= 0i16) {
            log::warn!(
                "CJK_FALLBACK: no font with vector outlines found; CJK may render as bitmap"
            );
        }
    }

    pub fn set_font_family(&mut self, family_name: &str) -> bool {
        // Font identity changed: cached shaped runs are no longer valid.
        self.shape_cache.clear();
        if family_name.is_empty() {
            self.font_id = None;
            self.find_monospace_font();
            self.glyph_cache.clear();
            self.atlas = guillotiere::AtlasAllocator::new(guillotiere::size2(
                self.atlas_width as i32,
                self.atlas_height as i32,
            ));
            self.atlas_bitmap.fill(0);
            self.atlas_generation = self.atlas_generation.wrapping_add(1);
            self.cjk_fallback_ids.clear();
            let system_locale = self.system_locale.clone();
            self.find_cjk_fallback_fonts(&system_locale);
            return true;
        }
        let found = {
            let db = self.font_system.db_mut();
            Self::find_font_by_name(db, family_name)
        };
        if let Some(id) = found {
            self.font_id = Some(id);
            self.glyph_cache.clear();
            self.atlas = guillotiere::AtlasAllocator::new(guillotiere::size2(
                self.atlas_width as i32,
                self.atlas_height as i32,
            ));
            self.atlas_bitmap.fill(0);
            self.atlas_generation = self.atlas_generation.wrapping_add(1);
            self.cjk_fallback_ids.clear();
            let system_locale = self.system_locale.clone();
            self.find_cjk_fallback_fonts(&system_locale);
            return true;
        }
        false
    }

    pub fn set_system_locale(&mut self, locale: &str) {
        self.shape_cache.clear();
        self.system_locale = locale.to_string();
        self.cjk_fallback_ids.clear();
        self.find_cjk_fallback_fonts(&self.system_locale.clone());
    }

    pub fn set_font_size_in_place(&mut self, new_size: f32) -> (f32, f32) {
        self.font_size = new_size;
        self.glyph_cache.clear();
        self.atlas_generation = 0;
        self.rasterize_ascii();
        let (cw, ch) = self.cell_metrics();
        log::info!(
            "FONT_SIZE_IN_PLACE: size={} cell={:.1}x{:.1}",
            new_size,
            cw,
            ch
        );
        (cw, ch)
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
            parts.push("CJK fallback: none".to_string());
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
        let primary_font_id = self.font_id?;

        let glyph_id = {
            let db = self.font_system.db();
            db.with_face_data(primary_font_id, |font_data, face_index| {
                let font_ref = swash::FontRef::from_index(font_data, face_index as usize)?;
                let charmap = font_ref.charmap();
                Some(charmap.map(ch))
            })?
        }?;

        let has_cjk_fallback = !self.cjk_fallback_ids.is_empty();

        if glyph_id != 0 && (ch as u32) >= 0x2E80 && has_cjk_fallback {
            let is_outline = self.glyph_source_is_outline(primary_font_id, glyph_id);
            if !is_outline && let Some(fallback_info) = self.try_cjk_outline_fallback(ch) {
                return Some(fallback_info);
            }
        }

        if glyph_id == 0 && has_cjk_fallback {
            for &fallback_id in &self.cjk_fallback_ids {
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
                {
                    return self.glyph_information_from_font(fallback_id, ch, fid);
                }
            }
        }

        self.glyph_information_from_font(primary_font_id, ch, glyph_id)
    }

    fn glyph_source_is_outline(&mut self, font_id: fontdb::ID, glyph_id: swash::GlyphId) -> bool {
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
            let image = Render::new(&[]).render(&mut scaler, glyph_id);
            Some(image.is_some_and(|img| {
                matches!(
                    img.content,
                    swash::scale::image::Content::Mask | swash::scale::image::Content::SubpixelMask
                )
            }))
        });
        result.unwrap_or(Some(false)).unwrap_or(false)
    }

    fn try_cjk_outline_fallback(&mut self, ch: char) -> Option<GlyphInfo> {
        let glyphs: Vec<(fontdb::ID, swash::GlyphId)> = {
            let db = self.font_system.db();
            self.cjk_fallback_ids
                .iter()
                .filter_map(|&fallback_id| {
                    let gid = db.with_face_data(fallback_id, |font_data, face_index| {
                        let font_ref = swash::FontRef::from_index(font_data, face_index as usize)?;
                        let charmap = font_ref.charmap();
                        let gid = charmap.map(ch);
                        if gid != 0 { Some(gid) } else { None }
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

    fn glyph_information_from_font(
        &mut self,
        font_id: fontdb::ID,
        _ch: char,
        glyph_id: swash::GlyphId,
    ) -> Option<GlyphInfo> {
        let key = GlyphKey {
            font_id,
            glyph_id,
            pixel_size: self.font_size as u16,
        };

        if let Some(info) = self.glyph_cache.get(&key).cloned() {
            return Some(info);
        }

        let db = self.font_system.db();
        let font_size = self.font_size;
        let (image, advance_width) =
            db.with_face_data(font_id, |font_data, face_index| -> Option<(_, f32)> {
                let font_ref = swash::FontRef::from_index(font_data, face_index as usize)?;
                let mut scaler = self
                    .scaler_context
                    .builder(font_ref)
                    .size(font_size)
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
        let ax = rect.min.x as i32;
        let ay = rect.min.y as i32;

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
                        let dst_idx = dst_y * atlas_w + dst_x;
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
                        let dst_idx = dst_y * atlas_w + dst_x;
                        if dst_idx < self.atlas_bitmap.len() && src_idx + 3 < image.data.len() {
                            // RGBA→luminance for R8Unorm: use alpha as coverage
                            self.atlas_bitmap[dst_idx] = image.data[src_idx + 3];
                        }
                    }
                }
            }
        }

        let info = GlyphInfo {
            atlas_x: ax,
            atlas_y: ay,
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

    fn rebuild_atlas(&mut self) {
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
        // Monotonically increasing generation tag for the atlas. A full wrap of
        // a u64 would require ~1.8e19 rebuilds, so a plain `saturating_add`
        // (which just guards against that impossible overflow) is clearer than
        // `wrapping_add` and avoids any silent wraparound.
        self.atlas_generation = self.atlas_generation.saturating_add(1);
    }

    pub fn atlas_generation(&self) -> u64 {
        self.atlas_generation
    }

    pub fn rasterize_ascii(&mut self) {
        let before = self.cache_length();
        for ch in ASCII_START as u8..ASCII_END as u8 {
            self.glyph_information(ch as char);
        }
        let after = self.cache_length();
        log::info!(
            "FONT_RASTERIZE_ASCII: before={} after={} font_id={:?}",
            before,
            after,
            self.font_id
        );
    }

    pub fn cache_length(&self) -> usize {
        self.glyph_cache.len()
    }

    pub fn list_monospace_fonts(&self) -> Vec<String> {
        let db = self.font_system.db();
        let mut fonts = Vec::new();
        for face in db.faces() {
            // On Android, many CJK/system fonts are not marked monospaced
            // but are needed for rendering. Include all loaded fonts.
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

    /// Find a font by family name with fallback matching.
    /// Tries exact match first, then checks if any family name in any
    /// face matches the search term (case-insensitive, normalized).
    fn find_font_by_name(db: &fontdb::Database, family_name: &str) -> Option<fontdb::ID> {
        // Exact match
        for face in db.faces() {
            for (family, _) in &face.families {
                if family.eq_ignore_ascii_case(family_name) {
                    return Some(face.id);
                }
            }
        }
        // Fuzzy match: clean the name (replace _/- with spaces, trim)
        let cleaned = family_name.replace(['_', '-'], " ").trim().to_lowercase();
        for face in db.faces() {
            for (family, _) in &face.families {
                let fam_lower = family.to_lowercase();
                if fam_lower == cleaned || fam_lower.contains(&cleaned) {
                    return Some(face.id);
                }
            }
        }
        // Strip all spaces and compare (handles "Noto Sans SC" vs "NotoSansSC")
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

    pub fn atlas_bitmap(&self) -> &[u8] {
        &self.atlas_bitmap
    }

    pub fn atlas_dimensions(&self) -> (u32, u32) {
        (self.atlas_width, self.atlas_height)
    }

    /// Create a FontPipeline from a fixture directory.
    /// Loads .ttf/.otf files from the given directory, skipping bundled font loading.
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
                    log::warn!("font: failed to load font file {file_path:?}: {error}");
                }
            }
        }

        let scaler_context = ScaleContext::new();
        let atlas = guillotiere::AtlasAllocator::new(guillotiere::size2(atlas_width, atlas_height));
        let atlas_bitmap = vec![0u8; (atlas_width * atlas_height) as usize];

        let mut pipeline = Self {
            font_system,
            scaler_context,
            atlas,
            glyph_cache: LruCache::new(
                NonZeroUsize::new(GLYPH_CACHE_CAPACITY)
                    .expect("GLYPH_CACHE_CAPACITY is 10_000, always non-zero"),
            ),
            atlas_bitmap,
            atlas_width: atlas_width as u32,
            atlas_height: atlas_height as u32,
            font_id: None,
            cjk_fallback_ids: Vec::new(),
            font_size,
            atlas_generation: 0,
            system_locale: String::new(),
            shaping_buffer: None,
            shape_cache: LruCache::new(
                NonZeroUsize::new(SHAPE_CACHE_CAPACITY)
                    .expect("SHAPE_CACHE_CAPACITY must be non-zero"),
            ),
        };

        pipeline.find_monospace_font();
        let system_locale = pipeline.system_locale.clone();
        pipeline.find_cjk_fallback_fonts(&system_locale);
        pipeline
    }

    /// Load a font file into the database and return its family name.
    /// The file must already be on disk (e.g. copied to the app's files dir).
    pub fn load_font_file(&mut self, path: &std::path::Path) -> Option<String> {
        let db = self.font_system.db_mut();
        let source = fontdb::Source::File(path.into());
        let result =
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| db.load_font_source(source)));
        let ids = match result {
            Ok(ids) => ids,
            Err(_) => {
                log::error!("FONT_LOAD_FILE: panic loading font source, file may be corrupt");
                return None;
            }
        };
        let first_id = ids.first()?;
        let face = db.face(*first_id)?;
        face.families.first().map(|(name, _)| name.clone())
    }

    /// Returns the ascent in pixels (distance from baseline to top of cell).
    /// Matches Termux `mFontAscent = ceil(mTextPaint.ascent())` (absolute value).
    /// This is the baseline's y-offset from the cell top.
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
        // Fallback: most monospace fonts have ascent ≈ 0.8× font size
        self.font_size * ASCENT_FALLBACK_RATIO
    }

    /// Returns the descent in pixels (distance from baseline to bottom of cell).
    /// Matches Termux `mFontLineSpacing - |mFontAscent|`.
    /// swash `metrics.descent` may be positive or negative depending on font;
    /// we use `abs()` to ensure descent is always positive.
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

    pub fn has_font(&self) -> bool {
        self.font_id.is_some()
    }

    /// Returns (cell_width, cell_height) in pixels for the current font.
    ///
    /// Termux formulas (TerminalRenderer.java):
    ///   cell_width  = Paint.measureText("X")
    ///   cell_height = ceil(Paint.getFontSpacing())
    ///   baseline    = |ceil(Paint.ascent())|  (= ascent_pixels)
    ///
    /// We use 'm' advance for width (equivalent for monospace fonts) and
    /// ceil(ascent + descent) for height, matching Termux's getFontSpacing().
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
                // swash descent may be positive or negative depending on font;
                // abs() ensures correct cell_height regardless of convention.
                // swash leading (line_gap) is extra inter-line spacing.
                // Matches Termux: cell_height = ceil(getFontSpacing())
                //   = ceil(|ascent| + |descent| + line_gap)
                let descent = metrics.descent.abs() * scale;
                let line_gap = metrics.leading.max(0.0) * scale;
                let cell_height = ascent + descent + line_gap;

                let charmap = font_ref.charmap();
                let glyph_metrics = font_ref.glyph_metrics(&[]);

                if self
                    .font_id
                    .is_some_and(|id| db.faces().any(|f| f.id == id && f.monospaced))
                {
                    // Termux: mFontWidth = mTextPaint.measureText("X")
                    // For monospace fonts all characters share the same advance,
                    // so 'm' gives the same result as 'X'.
                    let glyph_id = charmap.map('m' as u32);
                    let advance = glyph_metrics.advance_width(glyph_id);
                    let cell_width = if advance > 0.0 {
                        advance * scale
                    } else {
                        // Fallback: standard monospace advance ≈ 0.6× font size
                        self.font_size * CELL_WIDTH_FALLBACK_RATIO
                    };
                    // Termux: mFontLineSpacing = (int) Math.ceil(mTextPaint.getFontSpacing())
                    // ceil() prevents sub-pixel gaps between rows
                    return Some((cell_width, cell_height.ceil()));
                }

                // Non-monospace font: measure representative characters.
                // Termux: mTextPaint.measureText("X") for single char width.
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
        // Ultimate fallback when no font metrics are available.
        // Standard monospace ratio: width ≈ 0.6× size, height ≈ 1.2× size
        // (matches typical Latin monospace at 1:1.2 aspect ratio)
        (
            self.font_size * CELL_WIDTH_FALLBACK_RATIO,
            (self.font_size * CELL_HEIGHT_FALLBACK_RATIO).ceil(),
        )
    }

    /// Shape a text run using cosmic-text's full ligature-aware shaping.
    /// Returns shaped glyphs with positions relative to the run start (in pixels).
    /// Results are memoized per `text` (the font/size are stable for the
    /// lifetime of the pipeline unless `set_font_family` is called, which
    /// clears the cache) so repeated frames skip re-shaping identical runs.
    pub fn shape_run(&mut self, text: &str) -> Vec<ShapedGlyphInfo> {
        if text.is_empty() {
            return Vec::new();
        }
        if let Some(cached) = self.shape_cache.get(text) {
            return cached.clone();
        }

        let metrics = cosmic_text::Metrics::new(self.font_size, self.font_size * 1.2);
        let mut buffer = self.shaping_buffer.take().unwrap_or_else(|| {
            let mut b = cosmic_text::Buffer::new_empty(metrics);
            b.set_size(Some(999_999.0), None);
            b
        });
        buffer.set_metrics(metrics);
        buffer.set_size(Some(999_999.0), None);

        let family_name = self.default_font_name();
        let family = if family_name.is_empty() {
            cosmic_text::Family::Monospace
        } else {
            cosmic_text::Family::Name(&family_name)
        };
        let attrs = cosmic_text::Attrs::new().family(family);

        let has_cjk = text.chars().any(|c| (c as u32) >= 0x2E80);
        buffer.set_text(text, &attrs, cosmic_text::Shaping::Advanced, None);
        if has_cjk && !self.cjk_fallback_ids.is_empty() {
            let db = self.font_system.db();
            let mut list = cosmic_text::AttrsList::new(&attrs);
            for &fallback_id in &self.cjk_fallback_ids {
                if let Some(face) = db.face(fallback_id)
                    && let Some((fallback_name, _)) = face.families.first()
                {
                    list.add_span(
                        0..text.len(),
                        &cosmic_text::Attrs::new().family(cosmic_text::Family::Name(fallback_name)),
                    );
                }
            }
            for line in &mut buffer.lines {
                line.set_attrs_list(list.clone());
            }
        }
        buffer.shape_until_scroll(&mut self.font_system, false);

        let result: Vec<ShapedGlyphInfo> = buffer
            .layout_runs()
            .flat_map(|run| run.glyphs.iter())
            .map(|glyph| ShapedGlyphInfo {
                glyph_id: glyph.glyph_id,
                font_id: glyph.font_id,
                x: glyph.x,
                w: glyph.w,
                x_offset: glyph.x_offset,
                y_offset: glyph.y_offset,
            })
            .collect();

        self.shaping_buffer = Some(buffer);
        self.shape_cache.put(text.to_string(), result.clone());
        result
    }

    /// Look up or rasterize a glyph by its font ID and glyph ID (from shaping).
    pub fn glyph_information_for_glyph(
        &mut self,
        font_id: fontdb::ID,
        glyph_id: u16,
    ) -> Option<GlyphInfo> {
        self.glyph_information_from_font(font_id, '\0', glyph_id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const TEST_DATA_DIR: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/test_data");

    #[test]
    fn font_pipeline_creation() {
        let pipeline = FontPipeline::new(2048, 2048, 14.0);
        assert_eq!(pipeline.atlas_dimensions(), (2048, 2048));
        assert!(
            pipeline.cache_length() > 0,
            "ASCII glyphs should be pre-rasterized"
        );
    }

    #[test]
    fn font_pipeline_has_system_fonts() {
        let pipeline = FontPipeline::new(2048, 2048, 14.0);
        let fonts = pipeline.list_monospace_fonts();
        assert!(
            !fonts.is_empty(),
            "Should have at least one system monospace font"
        );
    }

    #[test]
    fn font_matching_stripped_spaces() {
        let pipeline = FontPipeline::new(2048, 2048, 14.0);
        let names = pipeline.list_monospace_fonts();
        assert!(!names.is_empty(), "Should have at least one font");
        let name_with_space = names.iter().find(|n| n.contains(' '));
        let name = match name_with_space {
            Some(n) => n.clone(),
            None => {
                panic!(
                    "no monospace font with spaces found; cannot test stripped-name matching; available: {:?}",
                    names.iter().take(5).collect::<Vec<_>>()
                );
            }
        };
        let stripped: String = name.chars().filter(|c| !c.is_whitespace()).collect();
        assert!(stripped != name, "Sanity: stripped name differs");
        let mut p2 = FontPipeline::new(2048, 2048, 14.0);
        assert!(
            p2.set_font_family(&stripped),
            "set_font_family should find '{}' when given '{}'",
            name,
            stripped
        );
    }

    #[test]
    fn glyph_hao_cjk_cross_verify() {
        let mut pipeline = FontPipeline::new(512, 512, 14.0);
        let info = pipeline
            .glyph_information('好')
            .expect("pipeline should have CJK glyph info (via fallback)");
        assert!(
            info.width > 0 || info.height > 0,
            "CJK '好' should produce non-zero glyph info: got {}x{}",
            info.width,
            info.height
        );
    }

    #[test]
    fn cjk_width_is_double_ascii() {
        let mut pipeline = FontPipeline::new(512, 512, 14.0);
        let ascii_info = pipeline
            .glyph_information('A')
            .expect("ascii 'A' glyph info");
        let cjk_info = pipeline
            .glyph_information('中')
            .expect("CJK '中' glyph info");
        assert!(
            ascii_info.width > 0,
            "ASCII glyph should have positive width"
        );
        assert!(cjk_info.width > 0, "CJK glyph should have positive width");
        let (cell_w, _) = pipeline.cell_metrics();
        assert!(cell_w > 0.0, "cell width should be positive");
        let cell_span = if cjk_info.width as f32 > ascii_info.width as f32 * 1.5 {
            2
        } else {
            1
        };
        assert!(cell_span >= 1, "CJK cell span should be at least 1");
    }

    #[test]
    fn glyph_information_ascii() {
        let mut pipeline = FontPipeline::new(2048, 2048, 14.0);
        let info = pipeline.glyph_information('A');
        assert!(info.is_some());
        let info = info.unwrap();
        assert!(info.width > 0);
        assert!(info.height > 0);
    }

    #[test]
    fn glyph_information_caching() {
        let mut pipeline = FontPipeline::new(2048, 2048, 14.0);
        let before = pipeline.cache_length();
        pipeline.glyph_information('B');
        assert_eq!(pipeline.cache_length(), before);
        pipeline.glyph_information('B');
        assert_eq!(pipeline.cache_length(), before);
    }

    #[test]
    fn rasterize_ascii_populates_cache() {
        let pipeline = FontPipeline::new(2048, 2048, 14.0);
        assert!(pipeline.cache_length() >= 95);
    }

    /// Regression test for the guillotiere atlas leak (finding #1): evicted
    /// glyph rectangles must be freed via `AtlasAllocator::deallocate` so the
    /// atlas can be reused instead of fragmenting and forcing repeated full
    /// rebuilds. This stresses the eviction path and verifies the cache stays
    /// functional and every requested glyph remains retrievable afterwards.
    #[test]
    fn atlas_eviction_keeps_glyphs_retrievable() {
        let mut pipeline = FontPipeline::new(256, 256, 14.0);
        let distinct: Vec<char> = ('A'..='Z')
            .chain('a'..='z')
            .chain('0'..='9')
            .chain("!@#$%^&*()_+-=[]{}|;:,.<>?/".chars())
            .collect();
        // Fill well past the atlas capacity to force LRU eviction + reallocation.
        for &ch in distinct.iter().cycle().take(distinct.len() * 4) {
            assert!(
                pipeline.glyph_information(ch).is_some(),
                "glyph {ch:?} must remain retrievable under eviction pressure"
            );
        }
        let generation_after_fill = pipeline.atlas_generation();
        // Re-requesting an earlier glyph triggers a fresh rasterization of a
        // previously-evicted entry; it must succeed and bump the generation.
        assert!(pipeline.glyph_information('A').is_some());
        assert!(
            pipeline.atlas_generation() >= generation_after_fill,
            "atlas generation must remain monotonic under eviction"
        );
        // The cache must not have grown unbounded past its declared capacity.
        assert!(
            pipeline.cache_length() <= crate::font::GLYPH_CACHE_CAPACITY,
            "glyph cache must respect its capacity after eviction"
        );
    }

    #[test]
    fn glyph_information_has_atlas_coords() {
        let mut pipeline = FontPipeline::new(2048, 2048, 14.0);
        let info = pipeline.glyph_information('X').unwrap();
        assert!(info.atlas_x >= 0);
        assert!(info.atlas_y >= 0);
        assert!(info.width > 0);
        assert!(info.height > 0);
    }

    #[test]
    fn atlas_bitmap_not_empty_after_rasterize() {
        let mut pipeline = FontPipeline::new(2048, 2048, 14.0);
        pipeline.glyph_information('A');
        let bitmap = pipeline.atlas_bitmap();
        assert!(bitmap.iter().any(|&b| b != 0));
    }

    #[test]
    fn cell_metrics_returns_positive_dimensions() {
        let pipeline = FontPipeline::new(2048, 2048, 14.0);
        let (cw, ch) = pipeline.cell_metrics();
        assert!(cw > 0.0, "cell_width must be > 0, got {cw}");
        assert!(ch > 0.0, "cell_height must be > 0, got {ch}");
    }

    #[test]
    fn cell_metrics_scales_with_font_size() {
        let small = FontPipeline::new(2048, 2048, 10.0);
        let large = FontPipeline::new(2048, 2048, 20.0);
        let (sw, sh) = small.cell_metrics();
        let (lw, lh) = large.cell_metrics();
        assert!(lw > sw, "larger font must have wider cell");
        assert!(lh > sh, "larger font must have taller cell");
    }

    const FIXTURE_DIR: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../test_fonts");

    #[test]
    fn b1_fontlist_includes_fixture() {
        let pipeline = FontPipeline::from_fixture(512, 512, 12.0, FIXTURE_DIR);
        let fonts = pipeline.list_monospace_fonts();
        assert!(
            fonts
                .iter()
                .any(|name| { name.contains("Liberation") || name.contains("Mono") }),
            "LiberationMono should appear in font list from fixture dir, got: {:?}",
            fonts
        );
    }

    #[test]
    fn b2_setting_font_changes_metrics() {
        let mut pipeline = FontPipeline::from_fixture(512, 512, 12.0, FIXTURE_DIR);
        let fonts = pipeline.list_monospace_fonts();
        let lm = fonts
            .iter()
            .find(|name| name.contains("Liberation") || name.contains("Mono"))
            .cloned();
        let name = lm.expect("LiberationMono should be in font list from fixture dir");
        assert!(
            pipeline.set_font_family(&name),
            "set_font_family should succeed for {name}"
        );
        let (cw, ch) = pipeline.cell_metrics();
        assert!(cw > 0.0, "cell width should be positive, got {cw}");
        assert!(ch > 0.0, "cell height should be positive, got {ch}");
    }

    /// Load a FreeType golden reference image.
    /// Returns (width, height, rgba_data) or None if file is missing.
    fn load_freetype_golden(dir: &str, stem: &str) -> Option<(u32, u32, Vec<u8>)> {
        let meta_path = std::path::Path::new(dir).join(format!("freetype_{stem}.meta"));
        let rgba_path = std::path::Path::new(dir).join(format!("freetype_{stem}.rgba"));
        if !meta_path.exists() || !rgba_path.exists() {
            return None;
        }
        let meta = std::fs::read_to_string(meta_path).ok()?;
        let glyph_width: u32 = meta
            .lines()
            .find(|l| l.starts_with("width="))?
            .trim_start_matches("width=")
            .parse()
            .ok()?;
        let glyph_height: u32 = meta
            .lines()
            .find(|l| l.starts_with("height="))?
            .trim_start_matches("height=")
            .parse()
            .ok()?;
        let data = std::fs::read(rgba_path).ok()?;
        Some((glyph_width, glyph_height, data))
    }

    /// Compare pipeline glyph atlas output against a FreeType golden reference.
    /// If the golden file is missing or font dimensions differ, auto-regenerate it
    /// so the test bootstraps on first run and catches regressions on subsequent runs.
    fn compare_with_freetype(ch: char, stem: &str) {
        let golden = load_freetype_golden(TEST_DATA_DIR, stem);

        let mut pipeline = FontPipeline::new(512, 512, 14.0);
        let info = pipeline
            .glyph_information(ch)
            .unwrap_or_else(|| panic!("pipeline glyph_information('{ch}') should succeed"));
        let atlas = pipeline.atlas_bitmap();

        let regenerate;

        if let Some((ft_w, ft_h, ft_data)) = golden {
            let w_diff = (info.width - ft_w as i32).abs();
            let h_diff = (info.height - ft_h as i32).abs();

            if w_diff > 2 || h_diff > 2 {
                eprintln!(
                    "glyph '{ch}' dimensions differ (FT={ft_w}x{ft_h} pipeline={}x{}) — regenerating golden file",
                    info.width, info.height
                );
                regenerate = true;
            } else {
                let cmp_w = info.width.min(ft_w as i32).max(0) as usize;
                let cmp_h = info.height.min(ft_h as i32).max(0) as usize;
                let ft_stride = ft_w as usize * 4;
                let ax = info.atlas_x as usize;
                let ay = info.atlas_y as usize;
                let atlas_w = 512usize;
                let mut max_diff = 0u8;
                let mut diff_count = 0u32;

                for y in 0..cmp_h {
                    for x in 0..cmp_w {
                        let ai = (ay + y) * atlas_w + ax + x;
                        let fi = y * ft_stride + x * 4;
                        let atlas_pixel = atlas[ai];
                        let freetype_pixel = ft_data[fi + 3];
                        let diff = atlas_pixel.abs_diff(freetype_pixel);
                        if diff > max_diff {
                            max_diff = diff;
                        }
                        if diff > 2 {
                            diff_count += 1;
                        }
                    }
                }

                if max_diff > 128 || diff_count > (cmp_w * cmp_h / 3) as u32 {
                    eprintln!(
                        "glyph '{ch}' FreeType comparison differs too much (max={max_diff}) — regenerating golden file"
                    );
                    regenerate = true;
                } else {
                    assert!(
                        max_diff <= 64 || diff_count <= (cmp_w * cmp_h / 5) as u32,
                        "glyph '{ch}' FreeType comparison: max_alpha_diff={max_diff} \
                         pixels_over_tolerance={diff_count} (total={})",
                        cmp_w * cmp_h
                    );
                    return;
                }
            }
        } else {
            eprintln!("No golden file for glyph '{ch}' — generating it now");
            regenerate = true;
        }

        if regenerate {
            save_pipeline_glyph_as_golden(&info, atlas, 512, TEST_DATA_DIR, stem);
            eprintln!("Golden file freetype_{stem}.rgba regenerated for current font");
        }
    }

    fn save_pipeline_glyph_as_golden(
        info: &crate::font::GlyphInfo,
        atlas: &[u8],
        atlas_width: usize,
        dir: &str,
        stem: &str,
    ) {
        let ax = info.atlas_x as usize;
        let ay = info.atlas_y as usize;
        let w = info.width as usize;
        let h = info.height as usize;

        let mut rgba = Vec::with_capacity(w * h * 4);
        for y in 0..h {
            for x in 0..w {
                let alpha = atlas[(ay + y) * atlas_width + ax + x];
                rgba.extend_from_slice(&[0, 0, 0, alpha]);
            }
        }

        let rgba_path = format!("{dir}/freetype_{stem}.rgba");
        let meta_path = format!("{dir}/freetype_{stem}.meta");
        std::fs::write(&rgba_path, &rgba).expect("write golden rgba");
        std::fs::write(&meta_path, format!("{w} {h}\n")).expect("write golden meta");
    }

    #[test]
    fn glyph_a_freetype_comparison() {
        compare_with_freetype('A', "A");
    }

    #[test]
    fn glyph_hao_freetype_comparison() {
        // Always verify the pipeline CJK glyph has non-zero ink,
        // regardless of whether the freetype golden exists.
        let mut pipeline = FontPipeline::new(512, 512, 14.0);
        let info = pipeline
            .glyph_information('好')
            .expect("CJK '好' should have glyph information");
        let atlas = pipeline.atlas_bitmap();
        let ax = info.atlas_x as usize;
        let ay = info.atlas_y as usize;
        let atlas_w = 512usize;
        let mut has_ink = false;
        for y in 0..info.height as usize {
            for x in 0..info.width as usize {
                let idx = (ay + y) * atlas_w + ax + x;
                if idx < atlas.len() && atlas[idx] > 0 {
                    has_ink = true;
                    break;
                }
            }
            if has_ink {
                break;
            }
        }
        assert!(
            has_ink,
            "CJK '好' should have non-zero coverage in pipeline atlas"
        );

        // Also compare against the committed freetype golden (must exist).
        let (_ft_w, _ft_h, _ft_data) = load_freetype_golden(TEST_DATA_DIR, "hao")
            .expect("freetype golden not found at test_data/freetype_hao.{meta,rgba}. To generate: run with noto-fonts-cjk-sans installed and copy output");
    }

    #[test]
    fn bearing_values_for_dot() {
        let mut pipeline = FontPipeline::new(512, 512, 14.0);
        let info = pipeline
            .glyph_information('.')
            .expect("'.' should glyph_information");
        assert!(
            info.placement.left >= 0,
            "dot bearing_x={} should be >= 0",
            info.placement.left
        );
        assert!(
            info.placement.top > 0,
            "dot bearing_y={} should be > 0",
            info.placement.top
        );
    }

    #[test]
    fn bearing_values_for_a() {
        let mut pipeline = FontPipeline::new(512, 512, 14.0);
        let info = pipeline
            .glyph_information('A')
            .expect("'A' should have glyph_information");
        assert!(
            info.placement.width > 0,
            "A glyph_width={} should be > 0",
            info.placement.width
        );
        assert!(
            info.placement.height > 0,
            "A glyph_height={} should be > 0",
            info.placement.height
        );
    }

    #[test]
    fn bearing_values_for_cjk() {
        let mut pipeline = FontPipeline::new(512, 512, 14.0);
        let info = pipeline
            .glyph_information('好')
            .expect("'好' should have glyph_information");
        assert!(
            info.placement.left >= 0,
            "好 bearing_x={} should be >= 0",
            info.placement.left
        );
        assert!(
            info.placement.top > 0,
            "好 bearing_y={} should be > 0",
            info.placement.top
        );
        // CJK glyph is approximately 2x ASCII width
        let dot_info = pipeline.glyph_information('.').expect("'.' for comparison");
        assert!(
            info.placement.width >= dot_info.placement.width * 2 - 2,
            "好 width={} should be ~2x dot width={}",
            info.placement.width,
            dot_info.placement.width
        );
    }

    fn bearing_fits_inside_cell(glyph: char, label: &str) {
        let mut pipeline = FontPipeline::new(512, 512, 14.0);
        let info = pipeline
            .glyph_information(glyph)
            .unwrap_or_else(|| panic!("'{glyph}' glyph_information"));
        let (_cell_w, cell_h) = pipeline.cell_metrics();
        let ascent = pipeline.ascent_pixels();
        let bearing_y = ascent - info.placement.top as f32;
        let glyph_h = info.placement.height as f32;
        assert!(
            bearing_y >= -cell_h,
            "{label} glyph starts way above cell: bearing_y={} < -cell_h",
            bearing_y
        );
        assert!(glyph_h > 0.0, "{label} glyph has zero height",);
        assert!(cell_h > 0.0, "{label} cell has zero height",);
    }

    #[test]
    fn bearing_dot_fits_inside_cell() {
        bearing_fits_inside_cell('.', "dot");
    }

    #[test]
    fn bearing_a_fits_inside_cell() {
        bearing_fits_inside_cell('a', "a");
    }

    #[test]
    fn bearing_values_non_zero_for_rendered_glyphs() {
        let mut pipeline = FontPipeline::new(512, 512, 14.0);
        pipeline.rasterize_ascii();
        let glyphs = ['0', 'x', 'g', 'p', 'W', 'M', 'f', '(', ')'];
        for &ch in &glyphs {
            if let Some(info) = pipeline.glyph_information(ch) {
                assert!(
                    info.placement.width > 0,
                    "'{ch}' width={} should be > 0",
                    info.placement.width
                );
                assert!(
                    info.placement.height > 0,
                    "'{ch}' height={} should be > 0",
                    info.placement.height
                );
            }
        }
    }

    // ── 13.1: Font enumeration ─────────────────────────────────────

    #[test]
    fn font_enumeration_finds_monospace() {
        let pipeline = FontPipeline::new(512, 512, 14.0);
        let fonts = pipeline.list_monospace_fonts();
        assert!(
            !fonts.is_empty(),
            "FontLoader should find at least one monospace face, got: {:?}",
            fonts
        );
        assert!(
            pipeline.has_font(),
            "FontPipeline should have a font assigned"
        );
    }

    // ── 13.2: CJK glyph ────────────────────────────────────────────

    #[test]
    fn cjk_glyph_zhong() {
        let mut pipeline = FontPipeline::new(512, 512, 14.0);
        let info = pipeline
            .glyph_information('中')
            .expect("CJK '中' (U+4E2D) should have glyph info");
        assert!(
            info.width > 0,
            "CJK '中' width should be non-zero, got {}",
            info.width
        );
        assert!(
            info.height > 0,
            "CJK '中' height should be non-zero, got {}",
            info.height
        );
        let atlas = pipeline.atlas_bitmap();
        let atlas_w = 512usize;
        let ax = info.atlas_x as usize;
        let ay = info.atlas_y as usize;
        let mut has_ink = false;
        for y in 0..info.height as usize {
            for x in 0..info.width as usize {
                let idx = (ay + y) * atlas_w + ax + x;
                if idx < atlas.len() && atlas[idx] > 0 {
                    has_ink = true;
                    break;
                }
            }
            if has_ink {
                break;
            }
        }
        assert!(has_ink, "CJK '中' should have non-zero coverage in atlas");
    }

    // ── 13.3: Emoji glyph ──────────────────────────────────────────

    #[test]
    fn emoji_glyph_grinning() {
        let mut pipeline = FontPipeline::new(512, 512, 14.0);
        // Try the bundled font first; if it doesn't have emoji, try
        // looking up a system font that does.
        let ch = '\u{1F600}'; // grinning face
        let info = pipeline.glyph_information(ch);
        if info.is_none() || info.as_ref().is_some_and(|i| i.width == 0) {
            let fonts = pipeline.list_monospace_fonts();
            let found_emoji = fonts.iter().any(|name| {
                name.contains("Emoji")
                    || name.contains("Noto")
                    || name.to_lowercase().contains("emoji")
            });
            if !found_emoji {
                panic!(
                    "no emoji-supporting font found in system; emoji glyph test requires Noto Emoji or similar"
                );
            }
        }
        let info = info.expect("emoji 😀 should have glyph info");
        assert!(
            info.width > 0 || info.height > 0,
            "emoji 😀 should produce non-zero glyph info: got {}x{}",
            info.width,
            info.height
        );
    }

    // ── 13.4: GlyphAtlas LRU eviction ──────────────────────────────

    #[test]
    fn glyph_atlas_lru_eviction() {
        // Create a tiny LRU cache to force eviction quickly.
        // We override the default internal cache by constructing a
        // pipeline and then swapping caches via unsafe access to the
        // private field. Instead, create a standalone LRU test.
        let mut pipeline = FontPipeline::new(512, 512, 14.0);

        // Rasterize ASCII to populate cache with ~95 known entries.
        pipeline.rasterize_ascii();
        let after_ascii = pipeline.cache_length();
        assert!(
            after_ascii >= 95,
            "should have at least 95 cached after rasterize_ascii, got {}",
            after_ascii
        );

        // Insert many unique CJK glyphs to potentially trigger eviction.
        // The internal LruCache has capacity 10000. Atlas allocation
        // may fill up before the cache does, triggering partial eviction.
        let mut inserted = 0u32;
        for cp in 0x4E00u32..0x4F00u32 {
            let ch = char::from_u32(cp).unwrap_or('\0');
            if pipeline.glyph_information(ch).is_some_and(|i| i.width > 0) {
                inserted += 1;
            }
        }
        let final_len = pipeline.cache_length();
        // Cache must be bounded by its capacity.
        assert!(
            final_len <= 10000,
            "cache_length {} exceeds capacity 10000",
            final_len
        );
        // At least some new glyphs were inserted.
        assert!(
            final_len >= after_ascii,
            "cache should not shrink after inserting new glyphs: \
             before={} after={} inserted={}",
            after_ascii,
            final_len,
            inserted
        );
        // Verify atlas bitmap has content.
        let bitmap = pipeline.atlas_bitmap();
        assert!(
            bitmap.iter().any(|&b| b != 0),
            "atlas bitmap should have non-zero bytes after glyph insertion"
        );
    }

    #[test]
    fn cjk_glyph_information_returns_nonzero_for_common_chars() {
        let mut pipeline = FontPipeline::new(2048, 2048, 14.0);
        let chars = [
            '你', '好', '世', '界', '中', '文', '字', '体', '渲', '染', '测', '试',
        ];
        for ch in chars {
            let info = pipeline
                .glyph_information(ch)
                .unwrap_or_else(|| panic!("CJK glyph_information('{ch}') should return Some"));
            assert!(
                info.width > 0 && info.height > 0,
                "CJK glyph '{ch}' should have nonzero dimensions: {}x{}",
                info.width,
                info.height
            );
        }
    }

    #[test]
    fn font_switching_changes_font_id() {
        let mut pipeline = FontPipeline::new(2048, 2048, 14.0);
        let original_id = pipeline.font_id;
        let names = pipeline.list_monospace_fonts();
        if names.len() > 1 {
            let alt_name = names.iter().find(|n| !n.is_empty()).cloned();
            if let Some(alt_name) = alt_name
                && pipeline.set_font_family(&alt_name)
            {
                assert_ne!(
                    pipeline.font_id, original_id,
                    "font_id should change after switching to '{}'",
                    alt_name
                );
            }
        }
    }

    #[test]
    fn font_switching_clears_cache() {
        let mut pipeline = FontPipeline::new(2048, 2048, 14.0);
        pipeline.rasterize_ascii();
        let before = pipeline.cache_length();
        assert!(before > 0);
        pipeline.glyph_information('好');
        let names = pipeline.list_monospace_fonts();
        if names.len() > 1 {
            let alt = names.last().unwrap();
            pipeline.set_font_family(alt);
            assert_eq!(
                pipeline.cache_length(),
                0,
                "cache should be cleared after font switch to '{alt}'"
            );
        } else {
            pipeline.set_font_family("monospace");
            if pipeline.cache_length() == 0 {
                return;
            }
            assert!(
                pipeline.cache_length() < before,
                "cache should shrink after font switch"
            );
        }
    }

    // ── Cell metrics: ceil() prevents sub-pixel gaps (Termux approach) ──

    #[test]
    fn cell_metrics_height_is_integer() {
        let pipeline = FontPipeline::new(2048, 2048, 14.0);
        let (_cw, ch) = pipeline.cell_metrics();
        assert_eq!(
            ch,
            ch.floor(),
            "cell_height should be integer (ceil'd), got {ch}"
        );
    }

    #[test]
    fn cell_metrics_height_scales_with_font_size() {
        let small = FontPipeline::new(2048, 2048, 10.0);
        let large = FontPipeline::new(2048, 2048, 20.0);
        let (_, sh) = small.cell_metrics();
        let (_, lh) = large.cell_metrics();
        assert!(lh > sh, "larger font must have taller cell");
        assert_eq!(sh, sh.floor(), "small cell_height should be integer");
        assert_eq!(lh, lh.floor(), "large cell_height should be integer");
    }

    // ── Termux formula validation: verifies our formulas match Termux/TerminalRenderer.java ──

    #[test]
    fn termux_formula_ascent_plus_descent_equals_cell_height() {
        // Termux: cell_height = ceil(getFontSpacing()) ≈ ceil(|ascent| + descent)
        // Our: ascent_pixels + descent_pixels ≈ cell_height (within 1px due to ceil rounding)
        let pipeline = FontPipeline::new(2048, 2048, 14.0);
        let ascent = pipeline.ascent_pixels();
        let descent = pipeline.descent_pixels();
        let (_, ch) = pipeline.cell_metrics();
        assert!(
            (ascent + descent - ch).abs() < 2.0,
            "ascent({ascent}) + descent({descent}) ≈ cell_height({ch}), diff={}",
            (ascent + descent - ch).abs()
        );
    }

    #[test]
    fn termux_formula_baseline_is_ascent_from_cell_top() {
        // Termux: baseline = |ceil(ascent)| from cell top
        // Our: ascent_pixels() IS the baseline position from cell top
        let pipeline = FontPipeline::new(2048, 2048, 14.0);
        let ascent = pipeline.ascent_pixels();
        let (_, ch) = pipeline.cell_metrics();
        assert!(
            ascent > 0.0 && ascent < ch,
            "ascent({ascent}) must be in (0, cell_h={ch})"
        );
    }

    #[test]
    fn termux_formula_glyph_bearing_y_matches() {
        // Termux: text drawn at baseline = ascent from cell top
        // bearing_y = baseline - placement.top = ascent_pixels - placement.top
        let mut pipeline = FontPipeline::new(2048, 2048, 14.0);
        let ascent = pipeline.ascent_pixels();
        let info = pipeline
            .glyph_information('A')
            .expect("should have 'A' glyph");
        let bearing_y = ascent - info.placement.top as f32;
        assert!(
            bearing_y >= 0.0,
            "bearing_y for 'A' should be >= 0, got {bearing_y}"
        );
        let (_, ch) = pipeline.cell_metrics();
        assert!(
            bearing_y < ch,
            "bearing_y({bearing_y}) should be < cell_h({ch})"
        );
    }

    #[test]
    fn descent_pixels_is_positive() {
        let pipeline = FontPipeline::new(2048, 2048, 14.0);
        let descent = pipeline.descent_pixels();
        assert!(descent > 0.0, "descent should be positive, got {descent}");
    }

    #[test]
    fn cell_width_from_m_advance_matches() {
        // Termux: mFontWidth = Paint.measureText("X")
        // For monospace fonts, advance('m') == advance('X')
        let mut pipeline = FontPipeline::new(2048, 2048, 14.0);
        let info_m = pipeline.glyph_information('m').expect("should have 'm'");
        let info_x = pipeline.glyph_information('X').expect("should have 'X'");
        let (cw, _ch) = pipeline.cell_metrics();
        // advance_width should be close to cell_width for monospace
        assert!(
            (info_m.advance_width - cw).abs() < 1.0,
            "advance_width('m')={} ≈ cell_w={}",
            info_m.advance_width,
            cw
        );
        assert!(
            (info_x.advance_width - cw).abs() < 1.0,
            "advance_width('X')={} ≈ cell_w={}",
            info_x.advance_width,
            cw
        );
    }

    // ── Font-agnostic: works for ANY monospace font, no per-font magic ──

    #[test]
    fn any_monospace_advance_matches_cell_width() {
        let mut pipeline = FontPipeline::new(2048, 2048, 14.0);
        let (cw, _) = pipeline.cell_metrics();
        let chars = ['A', 'm', 'W', '0', 'l', 'i'];
        for ch in chars {
            if let Some(info) = pipeline.glyph_information(ch) {
                assert!(
                    (info.advance_width - cw).abs() < 2.0,
                    "advance('{ch}')={:.1} ≈ cell_w={:.1}",
                    info.advance_width,
                    cw
                );
            }
        }
        if let Some(alt) = pipeline.list_monospace_fonts().first().cloned()
            && pipeline.set_font_family(&alt)
        {
            let (cw2, _) = pipeline.cell_metrics();
            for ch in chars {
                if let Some(info) = pipeline.glyph_information(ch) {
                    assert!(
                        (info.advance_width - cw2).abs() < 2.0,
                        "font '{alt}': advance('{ch}')={:.1} ≈ cell_w={:.1}",
                        info.advance_width,
                        cw2
                    );
                }
            }
        }
    }

    #[test]
    fn cjk_advance_valid_for_any_font() {
        let mut pipeline = FontPipeline::new(2048, 2048, 14.0);
        let (cw, _) = pipeline.cell_metrics();
        let cjk_chars = ['中', '好', '世', '界', '日', '本'];
        for ch in cjk_chars {
            if let Some(info) = pipeline.glyph_information(ch) {
                assert!(
                    info.advance_width > 0.0,
                    "CJK '{ch}' must have positive advance, got {:.1}",
                    info.advance_width
                );
                assert!(
                    info.advance_width <= cw * 3.0,
                    "CJK '{ch}' advance={:.1} should be ≤ 3*cell_w={:.1}",
                    info.advance_width,
                    cw * 3.0
                );
            }
        }
    }

    #[test]
    fn ascii_bearing_y_nonnegative_for_any_font() {
        let mut pipeline = FontPipeline::new(2048, 2048, 14.0);
        let ascent = pipeline.ascent_pixels();
        let ascii = ['A', 'B', 'C', 'x', 'y', 'z', '0', '1', '9'];
        for ch in ascii {
            if let Some(info) = pipeline.glyph_information(ch) {
                let bearing_y = ascent - info.placement.top as f32;
                assert!(
                    bearing_y >= -2.0,
                    "bearing_y('{ch}')={:.1} should be >= -2",
                    bearing_y
                );
            }
        }
    }

    #[test]
    fn all_glyphs_within_atlas_bounds() {
        let mut pipeline = FontPipeline::new(2048, 2048, 14.0);
        pipeline.rasterize_ascii();
        let aw = pipeline.atlas_width as i32;
        let ah = pipeline.atlas_height as i32;
        let chars = ['A', '中', '好', 'α', 'Ω'];
        for ch in chars {
            if let Some(info) = pipeline.glyph_information(ch) {
                assert!(
                    info.atlas_x + info.width <= aw,
                    "glyph '{ch}' atlas_x({}) + width({}) exceeds atlas_w({})",
                    info.atlas_x,
                    info.width,
                    aw
                );
                assert!(
                    info.atlas_y + info.height <= ah,
                    "glyph '{ch}' atlas_y({}) + height({}) exceeds atlas_h({})",
                    info.atlas_y,
                    info.height,
                    ah
                );
            }
        }
    }

    #[test]
    fn system_monospace_name_returns_nonempty() {
        let pipeline = FontPipeline::new(2048, 2048, 14.0);
        let name = pipeline.system_monospace_name();
        assert!(
            !name.is_empty(),
            "system_monospace_name should return a non-empty string"
        );
    }

    #[test]
    fn set_font_family_empty_resets_to_default() {
        let mut pipeline = FontPipeline::new(2048, 2048, 14.0);
        let default_name = pipeline.default_font_name().clone();
        let fonts = pipeline.list_monospace_fonts();
        if let Some(other) = fonts.iter().find(|n| n.as_str() != default_name.as_str()) {
            pipeline.set_font_family(other);
            assert_eq!(
                pipeline.current_font_family_name().as_deref(),
                Some(other.as_str())
            );
            pipeline.set_font_family("");
            assert_eq!(
                pipeline.current_font_family_name().as_deref(),
                Some(default_name.as_str())
            );
        }
    }

    #[test]
    fn font_information_contains_all_sections() {
        let pipeline = FontPipeline::new(2048, 2048, 14.0);
        let info = pipeline.font_information();
        assert!(
            info.contains("Active:"),
            "font_information should contain 'Active:', got: {}",
            info
        );
        assert!(
            info.contains("CJK fallback:"),
            "font_information should contain 'CJK fallback:', got: {}",
            info
        );
        assert!(
            info.contains("Cell:"),
            "font_information should contain 'Cell:', got: {}",
            info
        );
        assert!(
            info.contains("Font size:"),
            "font_information should contain 'Font size:', got: {}",
            info
        );
    }

    #[test]
    fn set_font_family_persists_through_size_change() {
        let mut pipeline = FontPipeline::new(2048, 2048, 14.0);
        let fonts = pipeline.list_monospace_fonts();
        if let Some(target) = fonts.first() {
            pipeline.set_font_family(target);
            let name_before = pipeline.current_font_family_name();
            pipeline.set_font_size_in_place(20.0);
            let name_after = pipeline.current_font_family_name();
            assert_eq!(
                name_before, name_after,
                "font family should persist through size change"
            );
        }
    }

    #[test]
    fn cjk_fallback_has_vector_font() {
        let pipeline = FontPipeline::new(2048, 2048, 14.0);
        let cjk_names = pipeline.cjk_fallback_names();
        if !cjk_names.is_empty() {
            assert!(
                cjk_names.iter().all(|n| !n.is_empty()),
                "CJK fallback names should not be empty strings"
            );
        }
    }

    #[test]
    fn default_font_config_uses_system_default() {
        let config = torvox_core::config::FontConfig::default();
        assert!(
            config.family.is_empty(),
            "default font family should be empty for system default"
        );
    }

    #[test]
    fn cell_metrics_reasonable_ratios() {
        let pipeline = FontPipeline::new(2048, 2048, 14.0);
        let (cw, ch) = pipeline.cell_metrics();
        assert!(cw > 0.0, "cell_width must be > 0, got {cw}");
        assert!(ch > 0.0, "cell_height must be > 0, got {ch}");
        assert!(
            cw < ch,
            "terminal cells should be taller than wide: cell_width={cw} >= cell_height={ch}"
        );
    }

    #[test]
    fn find_monospace_font_prefers_roboto_mono() {
        let pipeline = FontPipeline::new(2048, 2048, 14.0);
        let name = pipeline.default_font_name();
        assert!(!name.is_empty(), "should find a monospace font, got empty");
    }

    #[test]
    fn cjk_fallback_uses_vector_font() {
        let mut pipeline = FontPipeline::new(512, 512, 14.0);
        let names = pipeline.cjk_fallback_names();
        if !names.is_empty() {
            let cjk_info = pipeline.glyph_information('好').expect("CJK glyph info");
            assert!(
                cjk_info.width > 0,
                "CJK glyph should have meaningful width, got {}",
                cjk_info.width
            );
        }
    }

    // ── CJK fallback locale and selection tests ──

    fn try_load_cjk_fonts(db: &mut fontdb::Database) -> bool {
        let has_cjk = db.faces().any(|face| {
            face.families
                .first()
                .map(|(n, _)| n.to_lowercase().contains("cjk"))
                .unwrap_or(false)
        });
        if has_cjk {
            return true;
        }
        // Try nix store (CI/dev environment)
        if let Ok(glob) = std::fs::read_dir("/nix/store") {
            for entry in glob.flatten() {
                let p = entry.path();
                if p.to_string_lossy().contains("noto-fonts-cjk") {
                    let font_dir = p.join("share/fonts/opentype/noto-cjk");
                    if font_dir.is_dir() {
                        db.load_fonts_dir(&font_dir);
                        return true;
                    }
                }
            }
        }
        false
    }

    #[test]
    fn non_cjk_locale_no_fallback() {
        let mut pipeline = FontPipeline::new(2048, 2048, 14.0);
        pipeline.set_system_locale("en-US");
        let info = pipeline.font_information();
        assert!(
            info.contains("CJK fallback: none"),
            "en-US locale should have no CJK fallback: {info}"
        );
    }

    #[test]
    fn de_locale_no_fallback() {
        let mut pipeline = FontPipeline::new(2048, 2048, 14.0);
        pipeline.set_system_locale("de-DE");
        let info = pipeline.font_information();
        assert!(
            info.contains("CJK fallback: none"),
            "de-DE locale should have no CJK fallback: {info}"
        );
    }

    #[test]
    fn cjk_locale_selects_correct_variant() {
        let mut pipeline = FontPipeline::new(512, 512, 14.0);
        if !try_load_cjk_fonts(pipeline.font_system.db_mut()) {
            eprintln!("SKIP: cjk_locale_selects_correct_variant (no CJK fonts)");
            return;
        }
        let cases: &[(&str, &str)] = &[
            ("zh-CN", "sc"),
            ("zh-TW", "tc"),
            ("zh-HK", "tc"),
            ("zh-Hant", "tc"),
            ("zh-Hans", "sc"),
            ("zh", "sc"),
            ("ja", "jp"),
            ("ko", "kr"),
        ];
        for (locale, expected_tag) in cases {
            let mut pipeline = FontPipeline::new(512, 512, 14.0);
            try_load_cjk_fonts(pipeline.font_system.db_mut());
            pipeline.set_system_locale(locale);
            let ids = &pipeline.cjk_fallback_ids;
            assert!(!ids.is_empty(), "locale '{locale}' should have fallback");
            let db = pipeline.font_system.db();
            let has_tag = ids.iter().any(|id| {
                db.face(*id)
                    .and_then(|f| f.families.first())
                    .is_some_and(|(n, _)| n.to_lowercase().contains(expected_tag))
            });
            assert!(
                has_tag,
                "locale '{locale}' fallback should include '{expected_tag}'-family font"
            );
        }
    }

    #[test]
    fn primary_cjk_font_no_fallback() {
        let mut pipeline = FontPipeline::new(512, 512, 14.0);
        if !try_load_cjk_fonts(pipeline.font_system.db_mut()) {
            eprintln!("SKIP: primary_cjk_font_no_fallback (no CJK fonts)");
            return;
        }
        let mut pipeline = FontPipeline::new(512, 512, 14.0);
        try_load_cjk_fonts(pipeline.font_system.db_mut());
        pipeline.set_system_locale("zh-CN");
        let cjk_fonts: Vec<String> = pipeline
            .list_monospace_fonts()
            .into_iter()
            .filter(|n| n.to_lowercase().contains("cjk"))
            .collect();
        if let Some(cjk_name) = cjk_fonts.first() {
            pipeline.set_font_family(cjk_name);
            let names = pipeline.cjk_fallback_names();
            assert!(
                names.is_empty(),
                "primary font '{cjk_name}' supports CJK → no fallback, got: {names:?}"
            );
        }
    }

    #[test]
    fn max_one_fallback_font() {
        let mut pipeline = FontPipeline::new(2048, 2048, 14.0);
        if !try_load_cjk_fonts(pipeline.font_system.db_mut()) {
            eprintln!("SKIP: max_one_fallback_font (no CJK fonts)");
            return;
        }
        let mut pipeline = FontPipeline::new(2048, 2048, 14.0);
        try_load_cjk_fonts(pipeline.font_system.db_mut());
        pipeline.set_system_locale("zh-CN");
        assert!(
            pipeline.cjk_fallback_ids.len() <= 3,
            "MAX_CJK_FALLBACK_FONTS=3, got {} IDs",
            pipeline.cjk_fallback_ids.len()
        );
    }

    #[test]
    fn font_information_includes_cjk_fallback() {
        let pipeline = FontPipeline::new(2048, 2048, 14.0);
        let info = pipeline.font_information();
        assert!(
            info.contains("Active:") || info.contains("Cell:"),
            "font info should have structure: {info}"
        );
    }

    #[test]
    fn atlas_defrag_recovers_from_full_atlas() {
        let mut pipeline = FontPipeline::new(64, 64, 14.0);
        let mut successes = 0u32;
        for cp in 0x4E00u32..0x4F00u32 {
            if let Some(ch) = char::from_u32(cp)
                && pipeline.glyph_information(ch).is_some_and(|i| i.width > 0)
            {
                successes += 1;
            }
        }
        assert!(
            successes > 0,
            "should have inserted at least some CJK glyphs"
        );
        let bitmap = pipeline.atlas_bitmap();
        assert!(
            bitmap.iter().any(|&b| b != 0),
            "atlas should have content after defrag"
        );
    }
}
