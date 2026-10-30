//! FontPipeline：字体加载、字形光栅化与图集管理。

use cosmic_text::FontSystem;

use super::{
    ASCII_UPPER_BOUND, CJK_IDEOGRAPHIC_START, GlyphInfo, GlyphKey, GlyphSynthesis,
    NERD_FONT_PRIVATE_USE_END, NERD_FONT_PRIVATE_USE_START,
};

mod families;

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

pub struct FontInfo {
    pub active: Option<FontInfoActive>,
    pub cjk_state: CjkFallbackState,
    /// [`CjkFallbackState::Fallback`] 时列出的 CJK 回退族名。
    pub cjk_families: Vec<String>,
    /// 除主字体与 `cjk_families` 之外实际供出过字形的族名（符号族、区域族等）。
    pub other_families: Vec<String>,
    pub cell_width_px: f32,
    pub cell_height_px: f32,
    /// 逻辑字号（sp，Kotlin 侧传入 setFontSizeInPlace 的值）。
    /// 设备像素 = font_size * raster_scale——`raster_scale` 本身已是「显示密度 ×
    /// 系统字体缩放」，不再另乘 density。
    pub font_size: f32,
}

/// CJK 回退状态。
///
/// 跨语言时按下表的判别序号传递（见 [`crate::android`] 的 `getFontInfo` 打包），
/// 故两侧各有一份枚举而没有互相比对的字符串字面量。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u32)]
pub enum CjkFallbackState {
    /// CJK 语言环境、主字体不覆盖该语言的代表字符，且未能解析到任何 CJK 回退族。
    None = 0,
    /// 主字体不含 CJK，已列出回退族名。
    Fallback = 1,
    /// 主字体已覆盖本 locale 的 CJK 代表字符：按设计不装语言回退族。
    SkippedPrimaryCovers = 2,
    /// 系统语言不是 CJK：按平台语义不装 `lang` 语言回退族。
    SkippedNonCjkLocale = 3,
    /// 系统语言尚未知：无从判断是否需要语言回退族。
    SkippedLocaleUnknown = 4,
}

/// CJK 回退层为何缺席。
///
/// 「按设计不装」与「装了但没找到」是两件事，设置页的提示必须分开：
/// 前者是正常状态（不该报缺失），后者才是真问题。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum CjkFallbackSkipReason {
    /// 系统语言尚未知。
    LocaleUnknown,
    /// 系统语言不是 CJK：平台只为 CJK 语言声明 `lang` 回退族。
    NonCjkLocale,
    /// 主字体已覆盖本 locale 的 CJK 代表字符。
    PrimaryCovers,
}

impl CjkFallbackSkipReason {
    /// 该原因对应的上报状态。
    pub(crate) fn state(self) -> CjkFallbackState {
        match self {
            Self::LocaleUnknown => CjkFallbackState::SkippedLocaleUnknown,
            Self::NonCjkLocale => CjkFallbackState::SkippedNonCjkLocale,
            Self::PrimaryCovers => CjkFallbackState::SkippedPrimaryCovers,
        }
    }
}

#[derive(Clone, Debug)]
pub struct FontInfoActive {
    pub name: String,
    pub monospaced: bool,
}

pub struct FontPipeline {
    pub(crate) font_system: FontSystem,
    pub(crate) scaler_context: swash::scale::ScaleContext,
    pub(crate) atlas: etagere::AtlasAllocator,
    pub(crate) caches: super::glyph_cache::GlyphCache,
    pub(crate) atlas_bitmap: Vec<u8>,
    pub(crate) atlas_width: u32,
    pub(crate) atlas_height: u32,
    pub(crate) font_id: Option<fontdb::ID>,
    pub(crate) cjk_fallback_ids: Vec<fontdb::ID>,
    /// CJK 回退层按设计缺席的原因；`None` 表示「不是按设计缺席」——
    /// 要么已装上回退族，要么确实找不到（那是真问题，由状态 `None` 上报）。
    pub(crate) cjk_fallback_skip_reason: Option<CjkFallbackSkipReason>,
    pub(crate) font_size: f32,
    /// 主字体单元格度量缓存。`fontdb::with_face_data` 每次都会重新 mmap 字体文件，
    /// 而渲染循环每帧要读多次；缓存后只在字体族或字号变化时重算。
    pub(crate) cell_width: f32,
    pub(crate) cell_height: f32,
    pub(crate) raster_scale: f32,
    pub(crate) atlas_generation: u64,
    /// 回退层代际：cjk/symbol/nerd/emoji 任一层重发现即递增。
    /// 整形缓存键携带此代际，回退字体变化时旧整形结果自动失配，
    /// 不依赖显式清缓存（清缓存只处理字号/字体/尺寸维度）。
    pub(crate) fallback_generation: u64,
    pub(crate) dirty_rect: Option<(u32, u32, u32, u32)>,
    system_locale: String,
    pub(crate) shaping_buffer: Option<cosmic_text::Buffer>,
    /// 族名（小写）→ 用户选文件装入并选中的面。
    ///
    /// fontdb 的库只增不删：同一路径的字体文件被替换后旧面仍留在库里，
    /// 按族名扫库取首个会选回旧面（用户看到「换了字体却不生效」）。
    pub(crate) user_font_faces: std::collections::HashMap<String, fontdb::ID>,
}

/// 字形位图在单元格内的竖向定位：返回 `(bearing_y, raw_bearing_y)`。
///
/// 两处消费必须逐字段一致：主字形路径（`cell_builder` 的 `append_row_instances`）与
/// 叠加层（本文件的 `shaped_overlay_instance`）各自算一遍，分叉会让组合记号/ZWJ 表情
/// 与它的基字形错开——而这类偏移没有任何既有用例覆盖（整形的用例断言的是 advance，
/// 不是 bearing）。故收成一处，由两边各自调用。
///
/// - `bearing_y`：常规取「基线减放置顶部」；位图高于单元格时退化为竖向居中。
///   CJK 回退面常出现 `placement.top > ascent`，此时 bearing_y 为负，正是设计意图。
/// - `raw_bearing_y`：未作居中回退的原值。Bar/Underline 光标标记靠它对齐字形盒顶边。
///
/// 单位统一为物理像素：`glyph_h_px` 已含光栅缩放，`ascent_px * raster_scale` 同。
pub(crate) fn glyph_bearing_y(
    ascent_px: f32,
    raster_scale: f32,
    placement_top: f32,
    glyph_h_px: f32,
    cell_h_px: f32,
) -> (f32, f32) {
    let raw_bearing_y = ascent_px * raster_scale - placement_top;
    let bearing_y = if glyph_h_px > cell_h_px {
        (cell_h_px - glyph_h_px) / 2.0
    } else {
        raw_bearing_y
    };
    (bearing_y, raw_bearing_y)
}

/// 回归钉：bearing 判定的两条分支与 raw 值须各自正确。
///
/// 主字形路径与叠加层都调它：错一分，组合记号/ZWJ 表情会与基字形错开，
/// 而整形的用例断言的是 advance 不是 bearing，故此处显式钉住三个性质——
/// ① 位图不高于单元格时用基线减 top；② 高于单元格时退化为居中（可为负）；
/// ③ raw 值永不做居中回退（Bar/Underline 光标标记靠它对齐字形盒顶边）。
#[test]
fn glyph_bearing_y_matches_the_centering_fallback_contract() {
    // ① 常规：ascent 14px、scale 1、top 3 → 11。
    assert_eq!(glyph_bearing_y(14.0, 1.0, 3.0, 10.0, 20.0).0, 11.0);
    // ② 高于单元格：以 30px 高的位图进 20px 单元格 → (20-30)/2 = -5。
    let (bearing, raw) = glyph_bearing_y(14.0, 1.0, 3.0, 30.0, 20.0);
    assert_eq!(
        (bearing, raw),
        (-5.0, 11.0),
        "居中回退须与 raw 值分道：方块光标要的是 raw"
    );
    // ③ CJK 回退面 top > ascent：raw 为负，仍不触发居中（位图不高于单元）。
    let (cjk_bearing, cjk_raw) = glyph_bearing_y(14.0, 1.0, 20.0, 12.0, 20.0);
    assert_eq!((cjk_bearing, cjk_raw), (-6.0, -6.0));
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
        let atlas = etagere::AtlasAllocator::new(etagere::size2(atlas_width, atlas_height));
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
            cjk_fallback_skip_reason: None,
            font_size,
            cell_width: 0.0,
            cell_height: 0.0,
            atlas_generation: 0,
            fallback_generation: 0,
            dirty_rect: None,
            system_locale: String::new(),
            shaping_buffer: None,
            raster_scale: 1.0,
            user_font_faces: std::collections::HashMap::new(),
        };

        if pipeline.font_id.is_none() {
            pipeline.find_monospace_font();
        }
        pipeline.refresh_cell_metrics();
        // 初始 locale 取进程级值并走 `set_system_locale`（而不是只赋值）：重建管线
        // （`loadFontFile` / `setExtraFontPaths` 替换整条管线）与首次创建共用同一条
        // 「locale 到达」路径——区域回退族补装 + 回退层发现。少了它，重建后的管线
        // 从 "" 起步：探测字符缺失、主字体覆盖 CJK 也被报成「未检测到 CJK 备用字体」，
        // 且必须等下一次 locale 事件（`setSystemLocale`）才恢复，用户看到的是
        // 「设置页显示错误、过一会儿又对」。
        let initial_locale = Self::initial_system_locale();
        if !initial_locale.is_empty() {
            pipeline.set_system_locale(&initial_locale);
        }
        pipeline.rasterize_ascii();
        pipeline
    }

    /// 管线创建时的初始 locale：Android 取进程级值（JNI `setSystemLocale` 写入，
    /// [`super::font_db::current_locale`]），其余目标为空串（locale 由调用方显式设置）。
    fn initial_system_locale() -> String {
        #[cfg(target_os = "android")]
        {
            super::font_db::current_locale()
        }
        #[cfg(not(target_os = "android"))]
        {
            String::new()
        }
    }

    /// 重建字形图集：重置分配器、清位图、递增代次并重光栅化 ASCII。
    /// 字号、光栅缩放或字体族变化后调用。
    fn reset_atlas(&mut self) {
        self.atlas = etagere::AtlasAllocator::new(etagere::size2(
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
        // 负缓存与 cjk_glyph_cache 同批失效：二者都依赖「当前库有哪些面」，换字体、
        // 换 locale（会补装区域回退面）、换字号都必须让「找不到」这一结论重来。
        self.caches.unresolvable_glyphs.clear();
        // 样式面判定必须一并失效：它的值形如 `Some(face_id)` / `None`（这个族有没有
        // 粗/斜面），而 `set_system_locale` 在同一路径上经
        // `load_region_fallback_faces` **补装整个 TTC**（含全套字重与样式）。
        // 只清正缓存会让补装前算出的「无粗面」结论活过那次补装：主字体没变、
        // key 不变，LRU 也不会自然淘汰，于是 `bold=true` 一直走合成加粗而非真粗面。
        self.caches.style_face_cache.clear();
        self.caches.used_faces.clear();
        self.caches.ascii_glyph_ids = [None; ASCII_UPPER_BOUND as usize];
    }

    /// 重新发现 CJK 回退层并重光栅化 ASCII。任何可能改变可用回退字体的
    /// 字体变更后调用。
    fn rediscover_fallback_fonts(&mut self) {
        self.fallback_generation = self.fallback_generation.wrapping_add(1);
        self.cjk_fallback_ids.clear();
        let system_locale = self.system_locale.clone();
        self.find_cjk_fallback_fonts(&system_locale);
    }

    /// 当前系统 locale（CJK 判定按它分片，见 `cjk_probe_char`）。
    pub(crate) fn system_locale(&self) -> &str {
        &self.system_locale
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
        self.refresh_cell_metrics();
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
        // 负缓存的成立条件是「当前光栅尺寸下全库扫描描得出图」，故与光栅尺寸同批失效。
        self.caches.unresolvable_glyphs.clear();
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
        let (atlas_offset, atlas_size, bearing_x) = info.atlas_uv(atlas_width, atlas_height);
        // 物理像素对物理像素：与主字形路径共用同一个判定（height 已含光栅缩放）。
        let glyph_height_px = info.height as f32;
        // 叠加层只需要 bearing_y；raw 值方块光标才用，故显式忽略。
        let (bearing_y, _) = glyph_bearing_y(
            ascent_pixels,
            raster_scale,
            info.placement.top as f32,
            glyph_height_px,
            cell_height,
        );
        Some(crate::render::CellInstance {
            quad_origin: quad.origin,
            atlas_offset,
            atlas_size,
            foreground: quad.foreground,
            background: quad.background,
            underline_color: quad.deco,
            quad_size: quad.size,
            flags: quad.flags,
            bearing: [bearing_x, bearing_y],
        })
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
            .face_glyph_id(style_id, ch)
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
    ///
    /// 样式面与 CJK 回退链共用此查表：两者的口径相同（某个面能否解析某个码位），
    /// 而回退链在合成运行下逐帧重走，不入缓存则每帧每字都重建一次 charmap。
    pub(super) fn face_glyph_id(
        &mut self,
        face_id: fontdb::ID,
        ch: char,
    ) -> Option<swash::GlyphId> {
        let key = (face_id, ch as u32);
        if let Some(&cached) = self.caches.face_glyph_id_cache.get(&key) {
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
        self.caches.face_glyph_id_cache.put(key, gid);
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
        // 字形号 0 一律不走快径：`.notdef` 是非空的空心方框，一旦入缓存，
        // 「主字体没有这个码位」就会被快径当成已解析，整条回退链被跳过。
        if code_point < ASCII_UPPER_BOUND
            && let Some(gid) = self.caches.ascii_glyph_ids[ch as usize]
            && gid != 0
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
        // 字形号 0 同理不走：`.notdef` 会带着非空位图进缓存（空心方框），此后
        // 每个主字体未覆盖的码位都在这里命中它并直接返回豆腐块，CJK 回退层与
        // 全库扫描全被跳过——「先出现的未知码位」会决定后续所有未知码位的命运。
        // 这与下面两条回退支路对字形号 0 的处理是同一件事，此处只是先前漏了。
        if !is_nerd_private_use
            && glyph_id != 0
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
                // 查表走缓存：合成运行逐帧重走此链，不入缓存则每帧每字都要
                // 重入 `with_face_data` 重建 charmap（字体文件重新 mmap）。
                if let Some(fid) = self.face_glyph_id(fallback_id, ch)
                    && fid != 0
                    && let Some(result) =
                        self.glyph_information_from_font_with_synthesis(fallback_id, fid, synthesis)
                    && result.width > 0
                    && result.height > 0
                {
                    let font_database = self.font_system.db();
                    let face_name = font_database
                        .face(fallback_id)
                        .and_then(|family| family.families.first())
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
            //
            // 已知解析不出的码位直接跳过扫描：`find_glyph_anywhere` 要遍历 fontdb
            // 全部 face 并逐个 `with_face_data` 解压字体，罕见汉字（设备未装对应
            // CJK 字体）每次脏行重建都要付一次。跳过后照常落到下方主字体回退，由
            // .notdef 出图——与扫描失败时的可见结果完全一致，只是省掉了扫描。
            if !self.caches.unresolvable_glyphs.contains(&ch)
                && let Some((scan_id, scan_gid)) = self.find_glyph_anywhere(ch)
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
            // 负缓存与合成无关：`find_glyph_anywhere` 只回答「库里有哪个面能解析
            // 此码位」，与样式无关，合成运行扫不出结果时同样扫不出。故合成也记入，
            // 否则粗体/斜体文本里的罕见码位每帧都要重扫全库。
            self.caches.unresolvable_glyphs.put(ch, ());
        }

        // ── 回退到主字体 ────────────────────────────────────────────────────
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
}
