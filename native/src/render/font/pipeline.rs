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
    pub(crate) atlas: guillotiere::AtlasAllocator,
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

    /// 选定主字体。
    ///
    /// 设备上 `fonts.xml` 是唯一来源（DESIGN 字体节：不得使用任何硬编码字体名）：
    /// 缺失或无法解析由 [`super::font_db::resolve_system_monospace_files`] 崩溃退出；声明的等宽
    /// 字体未加载或未匹配时按 [`super::font_db::select_primary_face`] 的降级梯次选择，
    /// 仅当库内没有任何可用面才崩溃退出——该情形是字体文件装入失败，不是
    /// `fonts.xml` 的问题，故原因文案不得归因 `fonts.xml`。
    fn find_monospace_font(&mut self) {
        #[cfg(target_os = "android")]
        {
            // 取族内首个作匹配目标：库已装入整族，首个缺失时由下面的降级梯次接管。
            let target_filename = font_db::resolve_system_monospace_files()
                .first()
                .map_or_else(
                    || font_db::fatal("fonts.xml 等宽族解析为空（解析阶段已退出，此处不可达）"),
                    String::as_str,
                );
            match font_db::select_primary_face(self.font_system.db(), target_filename) {
                Some(face_id) => self.font_id = Some(face_id),
                None => {
                    font_db::fatal("fonts.xml 声明的字体文件无一能装入字体库（缺失、损坏或不可读）")
                }
            }
        }

        // 宿主（单元测试与基准）没有系统 fonts.xml：取 fontdb 首个「能当终端字体用」
        // 的等宽面。除等宽外还要覆盖拉丁：宿主环境里 Noto Color Emoji 之类的符号面
        // 也自称等宽（且不覆盖 CJK），按枚举顺序先撞上它就会把整条管线钉在 emoji
        // 上——单元格度量按方形 emoji 算，ASCII 只有零星十来个有字形，六个字体用例
        // 随宿主字体安装顺序时红时绿。判据用字形能力探测（charmap），不是字体名。
        // 优先选自身不覆盖 CJK 的面 —— 否则 `find_cjk_fallback_fonts` 会判定
        // 「主字体已支持 CJK」而整层跳过，单元测试就再也验证不到 CJK 回退链路。
        // 全部等宽面都覆盖 CJK 时退回首面。设备上走上面的 fonts.xml 分支，本段不编译。
        #[cfg(not(target_os = "android"))]
        {
            let font_database = self.font_system.db();
            let covers_cjk = |face_id| {
                super::font_db::face_covers(
                    font_database,
                    face_id,
                    super::font_db::cjk_coverage_probe_char(&self.system_locale),
                )
            };
            let usable_monospace = |face: &&fontdb::FaceInfo| {
                face.monospaced
                    && super::font_db::face_covers_latin(font_database, face.id)
                    && !covers_cjk(face.id)
            };
            let face = font_database
                .faces()
                .find(usable_monospace)
                .or_else(|| {
                    font_database.faces().find(|face| {
                        face.monospaced && super::font_db::face_covers_latin(font_database, face.id)
                    })
                })
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

    /// 选定主字面；字族不在库中返回 false。
    ///
    /// `family_files` 是该族的字体文件定位串：字族不在渲染字体库中时按需装入它们。
    /// 列表由调用方解析并给出——解析要走族索引（首次构建真机实测 127ms），而管线
    /// 住在渲染状态里，它自己解析就等于在渲染状态锁内解析，渲染线程会被冻结同样
    /// 时长。宿主构建的字体库已含系统全部字体，无需装入，故传空切片。
    ///
    /// 解析结果与当前字面相同时是空操作：Kotlin 的设置应用、字体应用与会话
    /// 状态发射三条路径都会对同一字族反复调用（真机每次改字号后紧跟两次），
    /// 无条件清图集会让每次调用都整库重光栅并重探回退层。
    pub fn set_font_family(&mut self, family_name: &str, family_files: &[String]) -> bool {
        let previous = self.font_id;
        if family_name.is_empty() {
            self.find_monospace_font();
        } else {
            // 用户选文件装入的族用它记住的面：同一路径的字体文件被替换后旧面
            // 仍在库里，扫库取首个会选回旧面。
            let mut face_id = self.user_font_face(family_name);
            if face_id.is_none() {
                // 库内没有该族时按需装入：渲染侧只常驻 3 个族（见 font_db::load_font_database），
                // 设置页选中的其余族要到这里才真正加载。
                #[cfg(target_os = "android")]
                if !family_files.is_empty()
                    && Self::find_font_by_name(self.font_system.db(), family_name).is_none()
                {
                    let loaded =
                        super::font_db::load_family_files(self.font_system.db_mut(), family_files);
                    log::debug!("FONT_SELECT: 按需装入族 '{family_name}': {loaded}");
                }
                // 宿主构建不按需装入：字体库已含系统全部字体，族文件列表恒为空
                // （见 font_db::family_files）。
                #[cfg(not(target_os = "android"))]
                let _ = family_files;
                face_id = Self::find_font_by_name(self.font_system.db(), family_name);
            }
            let Some(id) = face_id else {
                log::warn!("FONT_DIAG: set_font_family('{family_name}') NOT FOUND in fontdb");
                return false;
            };
            log::debug!("FONT_DIAG: set_font_family('{family_name}') found id={id:?}");
            self.font_id = Some(id);
        }
        if self.font_id != previous {
            self.clear_identity_caches();
            self.caches.glyph_cache.clear();
            self.reset_atlas();
            self.rediscover_fallback_fonts();
        }
        self.refresh_cell_metrics();
        true
    }

    /// 字族是否已在渲染字体库中：命中则选中它无需族文件。
    ///
    /// 空族名表示「系统等宽字体」，恒在库内。判定口径与 [`Self::set_font_family`]
    /// 的取面顺序一致（用户选文件装入的面优先，其次按族名扫库）：两者若分叉，
    /// 会出现「判定为常驻却选不中」。
    pub fn family_resident(&self, family_name: &str) -> bool {
        if family_name.is_empty() {
            return true;
        }
        self.user_font_face(family_name).is_some()
            || Self::find_font_by_name(self.font_system.db(), family_name).is_some()
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

    #[cfg(test)]
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
                .and_then(|family| family.families.first().map(|(name, _)| name.clone()))
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
    /// `cjk_fallback_ids`），去重保留首次出现。
    ///
    /// 报**真实族名**：设置页把它当「设备上装了哪些回退字体」显示，用户按名去找，
    /// 归一化成一个并不存在的名字（如把 `Noto Sans CJK SC`/`TC`/`JP` 合成
    /// "Noto Sans CJK"）既对不上用户按名查找的结果，也与本函数 KDoc 承诺的
    /// 「首元素即实际渲染命中者」相悖。不按字母排序：首元素必须是渲染真正命中的族。
    pub fn cjk_fallback_names(&self) -> Vec<String> {
        let font_database = self.font_system.db();
        let mut seen: std::collections::HashSet<String> = std::collections::HashSet::new();
        let mut ordered: Vec<String> = Vec::new();
        for &id in &self.cjk_fallback_ids {
            if let Some(face) = font_database.face(id)
                && let Some((name, _)) = face.families.first()
                && seen.insert(name.to_lowercase())
            {
                ordered.push(name.clone());
            }
        }
        ordered
    }

    /// 主字体是否覆盖本 locale 的 CJK 代表字符。与回退层取舍同源，见
    /// [`Self::primary_covers_cjk_probe`]——两处若各判一次，上报的状态可能与
    /// 渲染器实际是否装了回退层相反。
    fn primary_supports_cjk(&self) -> bool {
        self.primary_covers_cjk_probe()
    }

    /// 实际供出字形的面里，除主字体与 [Self::cjk_fallback_names] 之外的族名。
    ///
    /// 按 [`GlyphCache::used_faces`] 的首次命中顺序去重保留：第一个供出字形的面
    /// 就是渲染真正落到它上面的那个族，顺序有信息量。主字体（含其粗/斜面，同族）
    /// 与已单独上报的 CJK 回退族都不在此列，避免设置页同一行显示两遍。
    fn other_used_family_names(&self) -> Vec<String> {
        let font_database = self.font_system.db();
        let primary_family = self
            .font_id
            .and_then(|id| font_database.face(id))
            .and_then(|face| face.families.first().map(|(name, _)| name.as_str()));
        let cjk_families = self.cjk_fallback_names();
        let mut seen: std::collections::HashSet<String> = cjk_families
            .iter()
            .map(|name| name.to_lowercase())
            .collect();
        let mut ordered = Vec::new();
        for &id in &self.caches.used_faces {
            let Some(name) = font_database
                .face(id)
                .and_then(|face| face.families.first().map(|(name, _)| name.clone()))
            else {
                continue;
            };
            if primary_family.is_some_and(|primary| primary.eq_ignore_ascii_case(&name))
                || !seen.insert(name.to_lowercase())
            {
                continue;
            }
            ordered.push(name);
        }
        ordered
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
            (CjkFallbackState::Fallback, cjk)
        } else if let Some(reason) = self.cjk_fallback_skip_reason {
            // 按设计不装：不是缺失，不该报「未检测到」。
            (reason.state(), Vec::new())
        } else if self.primary_supports_cjk() {
            (CjkFallbackState::SkippedPrimaryCovers, Vec::new())
        } else {
            (CjkFallbackState::None, Vec::new())
        };
        // 上报物理像素：`cell_metrics` 给的是逻辑像素，而单元四边形按
        // `cell_metrics × raster_scale` 布置（见 `pass.rs`）。两者不同尺度时，
        // 设置页按物理像素显示的「单元格」就会与实际渲染差一个密度倍数。
        let (cw, ch) = self.cell_metrics();
        let raster_scale = self.get_raster_scale();
        FontInfo {
            active,
            cjk_state,
            cjk_families,
            other_families: self.other_used_family_names(),
            cell_width_px: cw * raster_scale,
            cell_height_px: ch * raster_scale,
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
            if !synthesized {
                self.caches.unresolvable_glyphs.put(ch, ());
            }
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

    /// 字体列表：设备上取进程级族索引，宿主构建列出库内全部等宽族。
    ///
    /// 设备上的渲染侧只常驻 3 个族（见 `new`：主字体 + 符号族 + 区域族），列表却
    /// 必须完整，故那里才触发那次全量枚举。族索引与渲染状态无关：生产路径取列表
    /// 时 MUST 在渲染状态锁外调用 `font_db::family_display_names`（首次构建
    /// 真机实测 127ms，持锁取会把渲染线程冻结同样时长）。
    pub fn list_monospace_fonts(&self) -> Vec<String> {
        #[cfg(target_os = "android")]
        {
            super::font_db::family_display_names()
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
        let cleaned_nospace: String = cleaned
            .chars()
            .filter(|character| !character.is_whitespace())
            .collect();
        for face in font_database.faces() {
            for (family, _) in &face.families {
                let fam_nospace: String = family
                    .to_lowercase()
                    .chars()
                    .filter(|character| !character.is_whitespace())
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

    /// 装入字体文件到当前字体库，返回文件中首个有族名的面的族名。
    ///
    /// 该面被记住（见 `user_font_faces`）：按族名选中时用它，
    /// 而不是扫库取首个——同一路径的文件被替换后旧面仍在库里。
    ///
    /// 族名取自**首个有族名的**面：TTC 的首面可能没有族名（无名子集），
    /// 按首个面取族名会得到空串，用户选字体时无从匹配。
    pub fn load_font_file(&mut self, path: &std::path::Path) -> Option<String> {
        let font_database = self.font_system.db_mut();
        let source = fontdb::Source::File(path.into());
        let ids = font_database.load_font_source(source);
        let (face_id, family) = ids.iter().find_map(|face_id| {
            let face = font_database.face(*face_id)?;
            let family = face.families.first().map(|(name, _)| name.clone())?;
            Some((*face_id, family))
        })?;
        self.user_font_faces.insert(family.to_lowercase(), face_id);
        Some(family)
    }

    /// 用户选文件装入的族对应的面。
    ///
    /// 面 id 是库内序号（slotmap 键）：库被整体替换后旧 id 指向别的面或失效，
    /// 故取用前确认它仍在库里。
    fn user_font_face(&self, family_name: &str) -> Option<fontdb::ID> {
        let face_id = *self.user_font_faces.get(&family_name.to_lowercase())?;
        self.font_system
            .db()
            .face(face_id)
            .is_some()
            .then_some(face_id)
    }

    #[cfg(test)]
    pub fn has_font(&self) -> bool {
        self.font_id.is_some()
    }
}
