//! FontPipeline：字体加载、字形光栅化与图集管理。

use cosmic_text::FontSystem;

#[cfg(target_os = "android")]
use super::font_db;
use super::{
    ASCII_UPPER_BOUND, CJK_IDEOGRAPHIC_START, GlyphInfo, GlyphKey, GlyphSynthesis,
    NERD_FONT_PRIVATE_USE_END, NERD_FONT_PRIVATE_USE_START,
};

/// 叠加字形（字素簇延续部分）绘制在基础单元四边形之上的位置与配色。
/// 整个 FontInfo 系列仅承载数据，全部显示格式由 Kotlin 的字符串资源完成。
pub(crate) struct OverlayQuad {
    pub origin: [f32; 2],
    pub size: [f32; 2],
    pub foreground: [f32; 4],
    pub background: [f32; 4],
    pub deco: [f32; 4],
    pub flags: f32,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct FontInfo {
    pub active: Option<FontInfoActive>,
    /// CJK 回退状态："fallback"（已列出族名）、"skipped"（主字体已覆盖 CJK）或 "none"。
    pub cjk_state: String,
    /// `cjk_state` 为 "fallback" 时的 CJK 回退族名。
    pub cjk_families: Vec<String>,
    pub cell_width_px: f32,
    pub cell_height_px: f32,
    /// 逻辑字号（sp，Kotlin 侧传入 setFontSizeInPlace 的值）。
    /// 设备像素 = font_size * raster_scale * density。
    pub font_size: f32,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct FontInfoActive {
    pub name: String,
    pub monospaced: bool,
}

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
    pub(crate) font_size: f32,
    pub(crate) raster_scale: f32,
    pub(crate) atlas_generation: u64,
    /// 回退层代际：cjk/symbol/nerd/emoji 任一层重发现即递增。
    /// 整形缓存键携带此代际，回退字体变化时旧整形结果自动失配，
    /// 不依赖显式清缓存（清缓存只处理字号/字体/尺寸维度）。
    pub(crate) fallback_generation: u64,
    pub(crate) dirty_rect: Option<(u32, u32, u32, u32)>,
    system_locale: String,
    pub(crate) shaping_buffer: Option<cosmic_text::Buffer>,
}

impl FontPipeline {
    pub fn new(atlas_width: i32, atlas_height: i32, font_size: f32) -> Self {
        // 渲染侧只常驻 3 个族（fonts.xml 等宽主字体 + 一个符号族 + 一个区域族），
        // 用户投放字体选中时按需装入（见 font_db::load_font_database）。
        #[cfg(target_os = "android")]
        let font_database = super::font_db::load_font_database();

        #[cfg(not(target_os = "android"))]
        let font_database = {
            let mut font_database = fontdb::Database::new();
            font_database.load_system_fonts();
            font_database
        };

        let font_system = FontSystem::new_with_locale_and_db(String::new(), font_database);

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
            font_size,
            atlas_generation: 0,
            fallback_generation: 0,
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
        pipeline.rasterize_ascii();
        pipeline
    }

    /// 选定主字体。
    ///
    /// 设备上 `fonts.xml` 是唯一来源（DESIGN 字体节：不得使用任何硬编码字体名）：
    /// 缺失或无法解析由 [`font_db::resolve_system_monospace_from_fonts_xml`] 直接
    /// `abort`；声明的等宽字体未加载或未匹配时按 [`font_db::select_primary_face`]
    /// 的降级梯次选择，仅当库内没有任何可用面（等同 fonts.xml 不可用）才 `abort`。
    fn find_monospace_font(&mut self) {
        #[cfg(target_os = "android")]
        {
            let target_filename = font_db::resolve_system_monospace_from_fonts_xml();
            match font_db::select_primary_face(self.font_system.db(), &target_filename) {
                Some(face_id) => self.font_id = Some(face_id),
                None => {
                    log::error!("FONT_SELECT: fonts.xml 未提供任何可用字体面");
                    std::process::abort();
                }
            }
        }

        // 宿主（单元测试与基准）没有系统 fonts.xml：取 fontdb 首个等宽面。
        // 优先选自身不覆盖 CJK 的面 —— 否则 `find_cjk_fallback_fonts` 会判定
        // 「主字体已支持 CJK」而整层跳过，单元测试就再也验证不到 CJK 回退链路。
        // 判定用字形能力探测（charmap），不是硬编码字体名。全部等宽面都覆盖 CJK
        // 时退回首面。设备上走上面的 fonts.xml 分支，本段不编译。
        #[cfg(not(target_os = "android"))]
        {
            let font_database = self.font_system.db();
            let covers_cjk = |face_id| {
                font_database
                    .with_face_data(face_id, |font_data, face_index| {
                        swash::FontRef::from_index(font_data, face_index as usize)
                            .map(|font| font.charmap().map('中') != 0)
                    })
                    .flatten()
                    .unwrap_or(false)
            };
            let face = font_database
                .faces()
                .find(|face| face.monospaced && !covers_cjk(face.id))
                .or_else(|| font_database.faces().find(|face| face.monospaced));
            if let Some(face) = face {
                let name = face.families.first().map_or("", |(name, _)| name);
                log::debug!("FONT_SELECT: host monospace id={:?} name='{name}'", face.id);
                self.font_id = Some(face.id);
            }
        }
    }

    /// 重建字形图集：重置分配器、清位图、递增代次并重光栅化 ASCII。
    /// 字号、光栅缩放或字体族变化后调用。
    fn reset_atlas(&mut self) {
        self.atlas = guillotiere::AtlasAllocator::new(guillotiere::size2(
            self.atlas_width as i32,
            self.atlas_height as i32,
        ));
        self.atlas_bitmap.fill(0);
        self.atlas_generation = self.atlas_generation.wrapping_add(1);
        self.reset_dirty_rect_full();
        self.rasterize_ascii();
    }

    /// 清空整形/字形标识缓存。字体族或语言变化使码位到字形号的映射失效后调用。
    fn clear_identity_caches(&mut self) {
        self.caches.shape_cache.clear();
        self.caches.glyph_id_cache.clear();
        self.caches.cjk_glyph_cache.clear();
        self.caches.ascii_glyph_ids = [None; 128];
    }

    /// 重新发现 CJK 回退层并重光栅化 ASCII。任何可能改变可用回退字体的
    /// 字体变更后调用。
    fn rediscover_fallback_fonts(&mut self) {
        self.fallback_generation = self.fallback_generation.wrapping_add(1);
        self.cjk_fallback_ids.clear();
        let system_locale = self.system_locale.clone();
        self.find_cjk_fallback_fonts(&system_locale);
    }

    pub fn set_font_family(&mut self, family_name: &str) -> bool {
        self.clear_identity_caches();
        if family_name.is_empty() {
            self.font_id = None;
            self.find_monospace_font();
            self.caches.glyph_cache.clear();
            self.reset_atlas();
            self.rediscover_fallback_fonts();
            return true;
        }
        // 库内没有该族时按需装入：渲染侧只常驻 3 个族（见 font_db::load_font_database），
        // 设置页选中的其余族要到这里才真正加载。
        #[cfg(target_os = "android")]
        if !Self::find_font_by_name(self.font_system.db(), family_name).is_some() {
            let loaded = super::font_db::load_family(self.font_system.db_mut(), family_name);
            log::debug!("FONT_SELECT: 按需装入族 '{family_name}': {loaded}");
        }
        let found = {
            let font_database = self.font_system.db_mut();
            Self::find_font_by_name(font_database, family_name)
        };
        if let Some(id) = found {
            let font_database = self.font_system.db();
            let name = font_database
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
            self.reset_atlas();
            self.rediscover_fallback_fonts();
            return true;
        }
        log::warn!(
            "FONT_DIAG: set_font_family('{}') NOT FOUND in fontdb",
            family_name
        );
        false
    }

    pub fn set_system_locale(&mut self, locale: &str) {
        self.clear_identity_caches();
        self.system_locale = locale.to_string();
        self.cjk_fallback_ids.clear();
        // 回退层变化必须推进代际，否则整形缓存按旧回退 span 摆字
        //（中文字形错位/用了错误 locale 变体）。
        self.fallback_generation = self.fallback_generation.wrapping_add(1);
        // 区域回退族在 locale 到达后补装：字体库在管线创建时已定型，而 spawn 前的
        // locale 调用被 `Bridge.onSession` 的 `sessionId == 0` 守卫丢弃，重建整个
        // 库又会因 `fontdb::ID` 重排而废掉主字体，故只增补缺失的面。
        #[cfg(target_os = "android")]
        super::font_db::load_region_fallback_faces(self.font_system.db_mut(), locale);
        self.find_cjk_fallback_fonts(&self.system_locale.clone());
    }

    pub fn set_font_size_in_place(&mut self, new_size: f32) -> (f32, f32) {
        self.font_size = new_size;
        // 字形身份缓存必须同步失效：ascii_glyph_ids 存的是旧尺寸光栅化
        // 前解析的字形 id，若不清零，d 等字符会命中旧 id 对应的错误位图
        //（“d 在某些区域像 a”类字形混淆）。glyph_cache 以 (font, gid,
        // 尺寸) 为键，清 glyph_cache 不清 ascii 表仍会查到脏 id。
        self.clear_identity_caches();
        self.caches.glyph_cache.clear();
        self.reset_atlas();
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
        self.reset_atlas();
        log::debug!("RASTER_SCALE: scale={:.3}", scale);
    }

    pub fn get_raster_scale(&self) -> f32 {
        self.raster_scale.max(f32::EPSILON)
    }

    /// 为字素簇延续码位（组合标记、emoji ZWJ 组件等）构造叠加四边形。
    /// 叠加与基础实例共享原点、尺寸与配色，UV 与基线偏移同样取自字形度量，
    /// 区别只在于叠加永远前进 0 个单元格。
    pub(crate) fn overlay_glyph_instance(
        &mut self,
        codepoint: u32,
        quad: super::OverlayQuad,
        cell_height: f32,
    ) -> Option<crate::render::CellInstance> {
        let mark_character = char::from_u32(codepoint)?;
        let info = self.glyph_information(mark_character)?;
        self.shaped_overlay_instance(&info, quad, cell_height)
    }

    /// 由已光栅化的整形字形构造叠加四边形（簇整形路径）：计算同上，
    /// 另需调用方事先把整形器位置烘进四边形原点。
    pub(crate) fn shaped_overlay_instance(
        &self,
        info: &super::GlyphInfo,
        quad: super::OverlayQuad,
        cell_height: f32,
    ) -> Option<crate::render::CellInstance> {
        let atlas_width = self.atlas_width as f32;
        let atlas_height = self.atlas_height as f32;
        let raster_scale = self.raster_scale;
        let ascent_pixels = self.ascent_pixels();
        let uv_x = info.atlas_x as f32 / atlas_width;
        let uv_y = info.atlas_y as f32 / atlas_height;
        let uv_w = info.width as f32 / atlas_width;
        let uv_h = info.height as f32 / atlas_height;
        let bearing_x = info.placement.left as f32;
        // 物理像素对物理像素：与主字形路径同式（height 已含光栅缩放）。
        let glyph_height_px = info.height as f32;
        let raw_bearing_y = ascent_pixels * raster_scale - info.placement.top as f32;
        let bearing_y = if glyph_height_px > cell_height {
            (cell_height - glyph_height_px) / 2.0
        } else {
            raw_bearing_y
        };
        Some(crate::render::CellInstance {
            quad_origin: quad.origin,
            atlas_offset: [uv_x, uv_y],
            atlas_size: [uv_w, uv_h],
            foreground: quad.foreground,
            background: quad.background,
            underline_color: quad.deco,
            quad_size: quad.size,
            flags: quad.flags,
            bearing: [bearing_x, bearing_y],
            glyph_advance_width: 0.0,
        })
    }

    pub fn current_font_family_name(&self) -> Option<String> {
        let font_id = self.font_id?;
        let font_database = self.font_system.db();
        let face_info = font_database.face(font_id)?;
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
        self.font_system
            .db()
            .faces()
            .find(|face| face.monospaced)
            .and_then(|face| face.families.first().map(|(name, _)| name.clone()))
            .unwrap_or_default()
    }

    /// 按优先级返回 CJK 回退族名（顺序同按 effective_priority 排序的
    /// `cjk_fallback_ids`）。去重保留首次出现，通用 CJK 族（含 "cjk" 不含
    /// "serif"）归并为一项 "Noto Sans CJK"，"Noto Serif CJK" 仍独立保留。
    /// 不按字母排序：首元素即实际渲染命中者，必须与 `FALLBACK_HIT` 族一致。
    pub fn cjk_fallback_names(&self) -> Vec<String> {
        let font_database = self.font_system.db();
        let mut seen: std::collections::HashSet<String> = std::collections::HashSet::new();
        let mut ordered: Vec<String> = Vec::new();
        for &id in &self.cjk_fallback_ids {
            if let Some(face) = font_database.face(id)
                && let Some((name, _)) = face.families.first()
            {
                let lower = name.to_lowercase();
                if seen.insert(lower) {
                    ordered.push(name.clone());
                }
            }
        }
        let mut normalized: Vec<String> = Vec::new();
        let mut seen_generic = false;
        for name in ordered {
            let lower = name.to_lowercase();
            let is_generic_cjk = lower.contains("cjk") && !lower.contains("serif");
            if is_generic_cjk {
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

    fn primary_supports_cjk(&self) -> bool {
        let Some(font_id) = self.font_id else {
            return false;
        };
        let font_database = self.font_system.db();
        let Some(face) = font_database.face(font_id) else {
            return false;
        };
        face.families.iter().any(|(name, _)| {
            let lower = name.to_lowercase();
            lower.contains("cjk")
                || lower.contains("chinese")
                || lower.contains("japanese")
                || lower.contains("korean")
                || lower.contains(" sc")
                || lower.contains(" tc")
                || lower.contains(" jp")
                || lower.contains(" kr")
        })
    }

    pub fn font_information(&self) -> String {
        let font_database = self.font_system.db();
        let mut parts = Vec::new();
        if let Some(id) = self.font_id
            && let Some(face) = font_database.face(id)
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
        } else if self.primary_supports_cjk() {
            parts.push("CJK fallback: skipped (primary font supports CJK)".to_string());
        } else {
            parts.push("CJK fallback: none".to_string());
        }
        let (cw, ch) = self.cell_metrics();
        parts.push(format!("Cell: {:.1}x{:.1}px", cw, ch));
        parts.push(format!("Font size: {:.1}px", self.font_size));
        parts.join("\n")
    }

    /// 供 UI 层（JNI）使用的结构化字体信息，仅承载数据，
    /// 显示字符串一律放在 Kotlin 资源里以便本地化。
    pub fn font_info(&self) -> FontInfo {
        let font_database = self.font_system.db();
        let active = self
            .font_id
            .and_then(|id| font_database.face(id))
            .map(|face| {
                let name = face.families.first().map_or("unknown", |(n, _)| n.as_str());
                FontInfoActive {
                    name: name.to_string(),
                    monospaced: face.monospaced,
                }
            });
        let cjk = self.cjk_fallback_names();
        let (cjk_state, cjk_families) = if !cjk.is_empty() {
            ("fallback".to_string(), cjk)
        } else if self.primary_supports_cjk() {
            ("skipped".to_string(), Vec::new())
        } else {
            ("none".to_string(), Vec::new())
        };
        let (cw, ch) = self.cell_metrics();
        FontInfo {
            active,
            cjk_state,
            cjk_families,
            cell_width_px: cw,
            cell_height_px: ch,
            font_size: self.font_size,
        }
    }

    pub fn font_size(&self) -> f32 {
        self.font_size
    }

    /// 在缓存中查已光栅化的字形。
    pub(super) fn lookup_glyph(
        &mut self,
        font_id: fontdb::ID,
        glyph_id: u16,
        synthesis: GlyphSynthesis,
    ) -> Option<GlyphInfo> {
        let key = GlyphKey {
            font_id,
            glyph_id,
            raster_size_bits: super::raster_size_key(self.font_size * self.raster_scale),
            synthesis: synthesis.bits(),
        };
        self.caches.glyph_cache.get(&key).cloned()
    }

    pub fn glyph_information(&mut self, ch: char) -> Option<GlyphInfo> {
        self.glyph_information_with_synthesis(ch, GlyphSynthesis::None)
    }

    /// 在真实样式面上取字形：字形号非 0 且光栅化出非空位图才算命中，
    /// 命中即返回，不做任何合成。
    fn styled_face_glyph(&mut self, style_id: fontdb::ID, ch: char) -> Option<GlyphInfo> {
        let glyph_id = self
            .style_glyph_id(style_id, ch)
            .filter(|&glyph| glyph != 0)?;
        let info = self.glyph_information_from_font_with_synthesis(
            style_id,
            glyph_id,
            GlyphSynthesis::None,
        )?;
        (info.width > 0 && info.height > 0).then_some(info)
    }

    /// 优先用同族的真实粗/斜体面（保持字形清晰且可微调），否则对基底面
    /// 做合成（加粗/倾斜）。
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

        // 同族真实粗/斜体面：命中即用，不合成。
        if let Some(style_id) = self.resolve_style_face(primary_font_id, bold, italic)
            && let Some(info) = self.styled_face_glyph(style_id, ch)
        {
            return Some(info);
        }

        // 2) 退回对基底面（或回退面）做合成。
        let synthesis = match (bold, italic) {
            (true, true) => GlyphSynthesis::BoldItalic,
            (true, false) => GlyphSynthesis::Bold,
            (false, true) => GlyphSynthesis::Italic,
            (false, false) => GlyphSynthesis::None,
        };
        self.glyph_information_with_synthesis(ch, synthesis)
    }

    /// 查找同族内符合请求字重/样式的面：基底面已符合时返回同一 id，
    /// 无匹配面时返回 None（由调用方改用合成）。
    /// 结果记在 `style_face_cache`，使 fontdb 查询每个 (字体, 粗, 斜)
    /// 组合至多执行一次，而非每帧每个样式单元一次。
    pub(crate) fn resolve_style_face(
        &mut self,
        base_id: fontdb::ID,
        bold: bool,
        italic: bool,
    ) -> Option<fontdb::ID> {
        let key = (base_id, bold, italic);
        if let Some(cached) = self.caches.style_face_cache.get(&key) {
            return *cached;
        }
        let font_database = self.font_system.db();
        let base = font_database.face(base_id)?;
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
        let matched = font_database.query(&query);
        let result = match matched {
            Some(id) if id != base_id => Some(id),
            _ => None,
        };
        self.caches.style_face_cache.put(key, result);
        result
    }

    /// 在（可能已加样式的）面上缓存式查 charmap，把 `ch` 映射到字形号，
    /// 避免每次调用都重入 `with_face_data`。
    fn style_glyph_id(&mut self, face_id: fontdb::ID, ch: char) -> Option<swash::GlyphId> {
        let key = (face_id, ch as u32);
        if let Some(&cached) = self.caches.style_glyph_id_cache.get(&key) {
            return Some(cached);
        }
        let gid = self
            .font_system
            .db()
            .with_face_data(face_id, |font_data, face_index| {
                let font_ref = swash::FontRef::from_index(font_data, face_index as usize)?;
                Some(font_ref.charmap().map(ch))
            })
            .flatten()?;
        self.caches.style_glyph_id_cache.put(key, gid);
        Some(gid)
    }

    fn glyph_information_with_synthesis(
        &mut self,
        ch: char,
        synthesis: GlyphSynthesis,
    ) -> Option<GlyphInfo> {
        let primary_font_id = self.font_id?;
        let has_cjk_fallback = !self.cjk_fallback_ids.is_empty();
        let synthesized = synthesis != GlyphSynthesis::None;
        let code_point = ch as u32;

        // ── 快径：ASCII 且字形号已缓存 ──
        if code_point < ASCII_UPPER_BOUND
            && let Some(gid) = self.caches.ascii_glyph_ids[ch as usize]
            && let Some(info) = self.lookup_glyph(primary_font_id, gid, synthesis)
        {
            return Some(info);
        }

        // ── CJK 缓存：已解析过的字跳过 swash 与回退 ──
        // 合成时不用：缓存的 (字体, 字形) 是按常规样式解析的，不适用于样式运行。
        // 不按码点设限：回退与全库扫描的写入本就不设限，符号字形同样受益；
        // 缓存是有界 LRU，不会无界增长。
        if !synthesized
            && has_cjk_fallback
            && let Some(&(cached_font_id, cached_glyph_id)) = self.caches.cjk_glyph_cache.get(&ch)
            && let Some(info) = self.lookup_glyph(cached_font_id, cached_glyph_id, synthesis)
        {
            return Some(info);
        }

        // ── 解析字形号（优先缓存） ──
        let glyph_id = if let Some(&cached) = self.caches.glyph_id_cache.get(&code_point) {
            cached
        } else {
            let gid = {
                let font_database = self.font_system.db();
                font_database.with_face_data(primary_font_id, |font_data, face_index| {
                    let font_ref = swash::FontRef::from_index(font_data, face_index as usize)?;
                    let charmap = font_ref.charmap();
                    Some(charmap.map(ch))
                })?
            }?;
            self.caches.glyph_id_cache.put(code_point, gid);
            gid
        };

        // 缓存 ASCII 字形号供后续快径命中。
        if code_point < ASCII_UPPER_BOUND {
            self.caches.ascii_glyph_ids[ch as usize] = Some(glyph_id);
        }

        // ── 进入开销较大的 CJK 处理前先查 glyph_cache ──
        // 私用区（Nerd Font）字符不走这条快径：缓存里可能是加载 Nerd Font
        // 之前主字体写入的 .notdef（豆腐块）。
        let is_nerd_private_use =
            (NERD_FONT_PRIVATE_USE_START..=NERD_FONT_PRIVATE_USE_END).contains(&code_point);
        if !is_nerd_private_use
            && let Some(info) = self.lookup_glyph(primary_font_id, glyph_id, synthesis)
        {
            if !synthesized && code_point >= CJK_IDEOGRAPHIC_START {
                self.caches
                    .cjk_glyph_cache
                    .put(ch, (primary_font_id, glyph_id));
            }
            return Some(info);
        }

        // ── CJK：先查轮廓再试回退 ──
        // 合成时跳过：轮廓回退面解析的是常规字形，样式运行必须留在基底路径上，
        // 这样合成的加粗/倾斜才能作用到已光栅化的遮罩。
        if !synthesized && glyph_id != 0 && code_point >= CJK_IDEOGRAPHIC_START && has_cjk_fallback
        {
            // cached 版：scaler 构建 + Render 约 20µs/次，不缓存则每字重复探测。
            let is_outline = self.glyph_source_is_outline_cached(primary_font_id, glyph_id);
            if !is_outline && let Some(fallback_info) = self.try_cjk_outline_fallback(ch) {
                return Some(fallback_info);
            }
        }

        // ── 字形号为 0：先走 CJK 回退层，再全库扫描 ──
        // 私用区字符即使主字体映射成功也必须走链：多数字体把 PUA 映射到
        // .notdef（豆腐块），非零字形号并不代表真的有字形。
        if glyph_id == 0 || is_nerd_private_use {
            // 先收集 id：链会借用 self 的字段，与循环内的 &mut self 渲染调用冲突。
            let cjk_fallback_ids: Vec<fontdb::ID> = self.cjk_fallback_ids.clone();
            for fallback_id in cjk_fallback_ids {
                let fallback_glyph = {
                    let font_database = self.font_system.db();
                    font_database.with_face_data(fallback_id, |font_data, face_index| {
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
                    let font_database = self.font_system.db();
                    let face_name = font_database
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
                }
            }
            // 链尾的全库扫描（spec d7：以全字体库扫描结束）。结果进
            // cjk_glyph_cache，故每个字只跑一次。
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
        if !synthesized && code_point >= CJK_IDEOGRAPHIC_START {
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

    /// 字体列表。宿主列出库内全部等宽族；设备上改为按需枚举
    /// `fonts.xml` 声明的完整族集合——渲染侧只常驻 4 个族，列表却必须完整，
    /// 故此处才触发那次枚举（约 4ms），且只在设置页显示列表时被调用。
    pub fn list_monospace_fonts(&self) -> Vec<String> {
        #[cfg(target_os = "android")]
        {
            super::font_db::family_index()
                .iter()
                .map(|entry| entry.display_name.clone())
                .collect()
        }
        #[cfg(not(target_os = "android"))]
        {
            let font_database = self.font_system.db();
            let mut fonts = Vec::new();
            for face in font_database.faces() {
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
    }

    fn find_font_by_name(
        font_database: &fontdb::Database,
        family_name: &str,
    ) -> Option<fontdb::ID> {
        for face in font_database.faces() {
            for (family, _) in &face.families {
                if family.eq_ignore_ascii_case(family_name) {
                    return Some(face.id);
                }
            }
        }
        // fonts.xml 别名（如 sans-serif）精确优先于模糊匹配：别名指向的
        // 文件名在已加载库中直接定位，不加载新文件，避免模糊命中错误字形。
        #[cfg(target_os = "android")]
        if let Some(id) = Self::find_font_by_alias(font_database, family_name) {
            return Some(id);
        }
        let cleaned = family_name.replace(['_', '-'], " ").trim().to_lowercase();
        for face in font_database.faces() {
            for (family, _) in &face.families {
                let fam_lower = family.to_lowercase();
                if fam_lower == cleaned || fam_lower.contains(&cleaned) {
                    return Some(face.id);
                }
            }
        }
        let cleaned_nospace: String = cleaned.chars().filter(|c| !c.is_whitespace()).collect();
        for face in font_database.faces() {
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
            for face in font_database.faces() {
                if face.monospaced {
                    return Some(face.id);
                }
            }
        }
        None
    }

    /// 经 `fonts.xml` 别名定位已加载字体：别名→文件名→库中同名源文件。
    /// 只做精确查找，不加载新文件。
    #[cfg(target_os = "android")]
    fn find_font_by_alias(
        font_database: &fontdb::Database,
        family_name: &str,
    ) -> Option<fontdb::ID> {
        for (alias, filenames) in super::font_db::fonts_xml_aliases() {
            if !alias.eq_ignore_ascii_case(family_name) {
                continue;
            }
            for filename in filenames {
                for face in font_database.faces() {
                    let path = match &face.source {
                        fontdb::Source::File(path) => path,
                        fontdb::Source::SharedFile(path, _) => path,
                        fontdb::Source::Binary(_) => continue,
                    };
                    if path
                        .file_name()
                        .and_then(|name| name.to_str())
                        .is_some_and(|name| name.eq_ignore_ascii_case(filename))
                    {
                        return Some(face.id);
                    }
                }
            }
        }
        None
    }

    pub fn load_font_file(&mut self, path: &std::path::Path) -> Option<String> {
        let font_database = self.font_system.db_mut();
        let source = fontdb::Source::File(path.into());
        let ids = font_database.load_font_source(source);
        let first_id = ids.first()?;
        let face = font_database.face(*first_id)?;
        face.families.first().map(|(name, _)| name.clone())
    }

    pub fn has_font(&self) -> bool {
        self.font_id.is_some()
    }
}
