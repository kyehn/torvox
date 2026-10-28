//! 字体族的选择、列举与上报。
//!
//! 与 [`super`]（管线本体：图集、光栅化、字形解析）分开的原因：这一组方法
//! 都在**冷路径**上——设置改动、字体列表打开、UI 查询——而管线的其余部分在
//! 每帧的热路径上。两者的变更节奏与关注点都不同，合在一个文件里时，调节
//! 渲染路径的人要在字体族逻辑里翻找。

use super::{CjkFallbackState, FontInfo, FontInfoActive, FontPipeline};
use crate::render::font::font_db;

impl FontPipeline {
    /// 选定主字体。
    ///
    /// 设备上 `fonts.xml` 是唯一来源（DESIGN 字体节：不得使用任何硬编码字体名）：
    /// 缺失或无法解析由 [`font_db::resolve_system_monospace_files`] 崩溃退出；声明的等宽
    /// 字体未加载或未匹配时按 [`font_db::select_primary_face`] 的降级梯次选择，
    /// 仅当库内没有任何可用面才崩溃退出——该情形是字体文件装入失败，不是
    /// `fonts.xml` 的问题，故原因文案不得归因 `fonts.xml`。
    pub(super) fn find_monospace_font(&mut self) {
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
                font_db::face_covers(
                    font_database,
                    face_id,
                    font_db::cjk_coverage_probe_char(&self.system_locale),
                )
            };
            let usable_monospace = |face: &&fontdb::FaceInfo| {
                face.monospaced
                    && font_db::face_covers_latin(font_database, face.id)
                    && !covers_cjk(face.id)
            };
            let face = font_database
                .faces()
                .find(usable_monospace)
                .or_else(|| {
                    font_database.faces().find(|face| {
                        face.monospaced && font_db::face_covers_latin(font_database, face.id)
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
                        font_db::load_family_files(self.font_system.db_mut(), family_files);
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
    /// [`Self::primary_covers_cjk_probe`]——两处若各判一次，
    /// 上报的状态可能与渲染器实际是否装了回退层相反。
    fn primary_supports_cjk(&self) -> bool {
        self.primary_covers_cjk_probe()
    }

    /// 实际供出字形的面里，除主字体与 [Self::cjk_fallback_names] 之外的族名。
    ///
    /// 按 [`super::super::glyph_cache::GlyphCache::used_faces`] 的首次命中顺序去重保留：
    /// 第一个供出字形的面就是渲染真正落到它上面的那个族，顺序有信息量。主字体
    /// （含其粗/斜面，同族）与已单独上报的 CJK 回退族都不在此列，避免设置页同一行
    /// 显示两遍。
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

    /// 字体列表：设备上取进程级族索引，宿主构建列出库内全部等宽族。
    ///
    /// 设备上的渲染侧只常驻 3 个族（见 `new`：主字体 + 符号族 + 区域族），列表却
    /// 必须完整，故那里才触发那次全量枚举。族索引与渲染状态无关：生产路径取列表
    /// 时 MUST 在渲染状态锁外调用 `font_db::family_display_names`（首次构建
    /// 真机实测 127ms，持锁取会把渲染线程冻结同样时长）。
    pub fn list_monospace_fonts(&self) -> Vec<String> {
        #[cfg(target_os = "android")]
        {
            font_db::family_display_names()
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
        for (alias, filenames) in font_db::fonts_xml_aliases() {
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
    /// 按首面取族名会得到空串，用户选字体时无从匹配。
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
