//! 系统字体库的加载与解析。

/// Android 系统字体目录（扫描顺序），含 OEM 放置自定义字体的 `/odm/fonts/` 与
/// `/data/fonts/`（`ASystemFontIterator` 也会枚举这两处）。仅 Android 构建与固定该
/// 列表的宿主测试编译，避免纯宿主构建下成为死代码。
#[cfg(any(target_os = "android", test))]
pub(crate) const FONT_DIRS: &[&str] = &[
    "/system/fonts/",
    "/system/product/fonts/",
    "/system_ext/fonts/",
    "/vendor/fonts/",
    "/product/fonts/",
    "/odm/fonts/",
    "/data/fonts/",
];

#[cfg(target_os = "android")]
static CACHED_FONT_PATHS: std::sync::OnceLock<Vec<std::path::PathBuf>> = std::sync::OnceLock::new();

#[cfg(target_os = "android")]
static CACHED_FONT_DB: std::sync::OnceLock<fontdb::Database> = std::sync::OnceLock::new();

#[cfg(target_os = "android")]
/// GUI 层提供的额外字体路径：由 `set_extra_font_paths()` 写入，`FontPipeline::new()` 读取。
#[cfg(target_os = "android")]
pub(crate) static EXTRA_FONT_PATHS: parking_lot::RwLock<Vec<std::path::PathBuf>> =
    parking_lot::RwLock::new(Vec::new());

#[cfg(target_os = "android")]
pub fn set_extra_font_paths(paths: Vec<std::path::PathBuf>) {
    let mut extra = EXTRA_FONT_PATHS.write();
    *extra = paths;
    log::debug!("FONT_LOAD: set {} extra font paths", extra.len());
}

#[cfg(target_os = "android")]
pub(crate) fn load_font_database() -> fontdb::Database {
    let db = CACHED_FONT_DB.get_or_init(|| {
        let font_paths = CACHED_FONT_PATHS.get_or_init(|| {
            // 优先用 NDK ASystemFontIterator（API 29+，minSdk 33）：它能枚举平台
            // 已知的所有系统字体（含 OEM 路径），无需猜测静态目录表。API 不可用
            // （特殊运行时）或结果为空时回退到 FONT_DIRS 扫描。
            let mut paths = android_font_iterator::enumerate_font_paths();
            if paths.is_empty() {
                log::debug!("FONT_LOAD: ASystemFontIterator empty, falling back to FONT_DIRS scan");
                for dir in FONT_DIRS {
                    let dir_path = std::path::Path::new(dir);
                    if let Ok(entries) = std::fs::read_dir(dir_path) {
                        let mut dir_count = 0usize;
                        for entry in entries.flatten() {
                            if is_font_file(&entry.path()) {
                                dir_count += 1;
                                paths.push(entry.path());
                            }
                        }
                        // 每个存在的目录都记日志（即使为空），便于设备诊断确认已扫描。
                        log::debug!("FONT_LOAD: dir={dir} files={dir_count}");
                    }
                }
            }
            log::debug!("FONT_LOAD: cached {} font paths", paths.len());
            paths
        });

        let mut db = fontdb::Database::new();
        let mut count = 0u32;
        for path in font_paths {
            if db.load_font_file(path).is_ok() {
                count += 1;
            }
        }
        log::debug!("FONT_LOAD: loaded {count} fonts from cached paths");
        db
    });
    db.clone()
}

/// 系统等宽字体文件名，取自 `fonts.xml`（DESIGN 字体节：fonts.xml 是唯一来源，
/// 不得使用任何硬编码字体名）。
///
/// 规范要求「系统不存在 fonts.xml 或其内容无法解析，输出日志并崩溃退出」：
/// 两个候选文件都读不到、都无法解析、或都没给出等宽字体时直接 `abort`。
/// 宿主（非 Android）不参与：那里没有 fonts.xml，见下方 `#[cfg]` 版本。
#[cfg(target_os = "android")]
pub(crate) fn resolve_system_monospace_from_fonts_xml() -> String {
    /// 按序尝试的 fonts.xml 位置：部分 ROM 只提供 fonts_fallback.xml。
    const CANDIDATES: [&str; 2] = ["/system/etc/fonts.xml", "/system/etc/fonts_fallback.xml"];

    let mut last_error = String::new();
    for xml_path in CANDIDATES {
        let content = match std::fs::read_to_string(xml_path) {
            Ok(content) => content,
            Err(error) => {
                last_error = format!("读取 {xml_path} 失败: {error}");
                continue;
            }
        };
        let (monospace, _) = parse_fonts_xml_families(&content);
        if let Some(filename) = monospace.into_iter().next() {
            log::debug!("FONT_XML: monospace target='{filename}'");
            return filename;
        }
        last_error = format!("{xml_path} 未声明等宽字体");
    }
    log::error!("FONT_XML: 无法从系统 fonts.xml 解析等宽字体（{last_error}）");
    std::process::abort();
}

#[cfg(any(target_os = "android", test))]
type FontsXmlFamilies = (Vec<String>, Vec<(String, Vec<(String, u32)>)>);

/// 解析 `fonts.xml`，产出等宽字体文件名与有序的 `(lang, [(filename, ttc_index)])`
/// 回退条目。纯函数，便于宿主测试喂入真实设备片段；无法解析的输入产出空列表。
#[cfg(any(target_os = "android", test))]
pub(crate) fn parse_fonts_xml_families(xml: &str) -> FontsXmlFamilies {
    let mut monospace = Vec::new();
    let mut lang_fallbacks = Vec::new();
    let document = match roxmltree::Document::parse(xml) {
        Ok(document) => document,
        Err(error) => {
            log::error!("FONT_XML: 解析失败（{error}）");
            return (monospace, lang_fallbacks);
        }
    };
    let root = document.root_element();
    if !matches!(root.tag_name().name(), "familyset" | "fontconfig") {
        log::error!("FONT_XML: 根元素不是 familyset/fontconfig");
        return (monospace, lang_fallbacks);
    }
    for family in root
        .children()
        .filter(|node| node.is_element() && node.tag_name().name() == "family")
    {
        let mut filenames = Vec::new();
        for font in family
            .children()
            .filter(|node| node.is_element() && node.tag_name().name() == "font")
        {
            let Some(filename) = font.text().map(str::trim).filter(|text| !text.is_empty()) else {
                continue;
            };
            let index = font
                .attribute("index")
                .and_then(|value| value.parse().ok())
                .unwrap_or(0);
            filenames.push((filename.to_string(), index));
        }
        if filenames.is_empty() {
            continue;
        }
        if let Some(name) = family.attribute("name") {
            if ["monospace", "sans-serif mono", "serif mono"].contains(&name) {
                monospace.extend(filenames.into_iter().map(|(filename, _)| filename));
            }
        } else if let Some(lang) = family.attribute("lang") {
            lang_fallbacks.push((lang.to_string(), filenames));
        }
    }
    (monospace, lang_fallbacks)
}

/// 按文档顺序把 `fonts.xml` 解析为 `(alias, filenames)` 对。无名字段被跳过。
#[cfg(any(target_os = "android", test))]
pub(crate) fn parse_fonts_xml_aliases(xml: &str) -> Vec<(String, Vec<String>)> {
    let mut aliases = Vec::new();
    let document = match roxmltree::Document::parse(xml) {
        Ok(document) => document,
        Err(_) => return aliases,
    };
    let root = document.root_element();
    if !matches!(root.tag_name().name(), "familyset" | "fontconfig") {
        return aliases;
    }
    for family in root
        .children()
        .filter(|node| node.is_element() && node.tag_name().name() == "family")
    {
        let Some(name) = family
            .attribute("name")
            .map(str::trim)
            .filter(|name| !name.is_empty())
        else {
            continue;
        };
        let mut filenames = Vec::new();
        for font in family
            .children()
            .filter(|node| node.is_element() && node.tag_name().name() == "font")
        {
            if let Some(filename) = font.text().map(str::trim).filter(|text| !text.is_empty()) {
                filenames.push(filename.to_string());
            }
        }
        if !filenames.is_empty() {
            aliases.push((name.to_string(), filenames));
        }
    }
    aliases
}

#[cfg(target_os = "android")]
static FONTS_XML_ALIASES: std::sync::OnceLock<Vec<(String, Vec<String>)>> =
    std::sync::OnceLock::new();

/// 平台 `fonts.xml` 的别名解析结果，每进程读取一次。
#[cfg(target_os = "android")]
pub(crate) fn fonts_xml_aliases() -> &'static [(String, Vec<String>)] {
    FONTS_XML_ALIASES.get_or_init(|| {
        std::fs::read_to_string("/system/etc/fonts.xml")
            .map(|content| parse_fonts_xml_aliases(&content))
            .unwrap_or_default()
    })
}

/// 把系统 locale 标签映射为按优先级排列的 `fonts.xml` `lang` 候选。
/// AOSP 用 `zh-Hans`/`zh-Hant`，旧版本可能用 `zh-CN`。
#[cfg(any(target_os = "android", test))]
pub(crate) fn locale_fonts_xml_langs(locale: &str) -> &'static [&'static str] {
    if locale.starts_with("zh-CN") || locale.starts_with("zh-Hans") || locale == "zh" {
        &["zh-Hans", "zh-CN", "zh", "und-Hani"]
    } else if locale.starts_with("zh-TW")
        || locale.starts_with("zh-Hant")
        || locale.starts_with("zh-HK")
    {
        &["zh-Hant", "zh-TW", "zh-HK", "zh", "und-Hani"]
    } else if locale.starts_with("ja") {
        &["ja"]
    } else if locale.starts_with("ko") {
        &["ko"]
    } else {
        &[]
    }
}

/// 宿主环境没有系统 fonts.xml：调用方据此跳过 fonts.xml 分支（见
/// [`super::pipeline::FontPipeline::find_monospace_font`] 的 `cfg` 分派）。
#[cfg(not(target_os = "android"))]
pub(crate) fn resolve_system_monospace_from_fonts_xml() -> Option<String> {
    None
}

#[cfg(target_os = "android")]
pub(crate) fn is_font_file(entry: &std::path::Path) -> bool {
    entry
        .extension()
        .and_then(|ext| ext.to_str())
        .is_some_and(|ext| {
            ext.eq_ignore_ascii_case("ttf")
                || ext.eq_ignore_ascii_case("otf")
                || ext.eq_ignore_ascii_case("ttc")
        })
}

#[cfg(test)]
mod tests {
    use super::FONT_DIRS;

    /// 扫描列表须含 `ASystemFontIterator` 会枚举的 OEM 目录（/odm/fonts/、
    /// /data/fonts/），以免漏掉放在 /system/fonts 之外的 OEM 自定义字体。
    #[test]
    fn font_dirs_include_oem_paths() {
        assert!(
            FONT_DIRS.contains(&"/odm/fonts/"),
            "OEM font dir /odm/fonts/ must be scanned"
        );
        assert!(
            FONT_DIRS.contains(&"/data/fonts/"),
            "OEM font dir /data/fonts/ must be scanned"
        );
        // Android 基础路径必须保留。
        for required in [
            "/system/fonts/",
            "/system/product/fonts/",
            "/system_ext/fonts/",
            "/vendor/fonts/",
            "/product/fonts/",
        ] {
            assert!(FONT_DIRS.contains(&required), "{required} must be scanned");
        }
        // 不得重复。
        let mut sorted = FONT_DIRS.to_vec();
        sorted.sort_unstable();
        sorted.dedup();
        assert_eq!(
            sorted.len(),
            FONT_DIRS.len(),
            "FONT_DIRS must not contain duplicates"
        );
    }

    /// 目录须以最标准的路径优先（同名字体存在于多处时靠前者胜出）。
    #[test]
    fn font_dirs_start_with_system_fonts() {
        assert_eq!(FONT_DIRS[0], "/system/fonts/");
    }

    /// 仿 AOSP 结构的最小片段（对照真实 API 35 模拟器文件）：带属性的 monospace、
    /// 无属性 `<font>`，以及共用同一 TTC 但 `index` 不同的 `lang` 块。
    const FONTS_XML_SNIPPET: &str = r#"<?xml version="1.0" encoding="utf-8"?>
<familyset version="23">
    <family name="monospace">
        <font weight="400" style="normal">DroidSansMono.ttf</font>
    </family>
    <family name="casual">
        <font>ComingSoon.ttf</font>
    </family>
    <family lang="zh-Hans">
        <font weight="400" style="normal" index="2" postScriptName="NotoSansCJKJP-Regular">
            NotoSansCJK-Regular.ttc
        </font>
    </family>
    <family lang="ja">
        <font weight="400" style="normal" index="0" postScriptName="NotoSansCJKJP-Regular">
            NotoSansCJK-Regular.ttc
        </font>
    </family>
</familyset>"#;

    #[test]
    fn parse_fonts_xml_monospace_and_lang_blocks() {
        let (monospace, lang_fallbacks) = super::parse_fonts_xml_families(FONTS_XML_SNIPPET);
        assert_eq!(monospace, vec!["DroidSansMono.ttf"]);
        assert_eq!(lang_fallbacks.len(), 2);
        assert_eq!(lang_fallbacks[0].0, "zh-Hans");
        assert_eq!(
            lang_fallbacks[0].1,
            vec![("NotoSansCJK-Regular.ttc".to_string(), 2)]
        );
        assert_eq!(lang_fallbacks[1].0, "ja");
        assert_eq!(
            lang_fallbacks[1].1,
            vec![("NotoSansCJK-Regular.ttc".to_string(), 0)]
        );
    }

    #[test]
    fn parse_fonts_xml_rejects_garbage() {
        assert_eq!(
            super::parse_fonts_xml_families("not xml at all"),
            (Vec::new(), Vec::new())
        );
        assert_eq!(
            super::parse_fonts_xml_families("<html></html>"),
            (Vec::new(), Vec::new())
        );
        assert_eq!(
            super::parse_fonts_xml_families(""),
            (Vec::new(), Vec::new())
        );
    }

    #[test]
    fn parse_fonts_xml_aliases_in_document_order() {
        let aliases = super::parse_fonts_xml_aliases(FONTS_XML_SNIPPET);
        assert_eq!(aliases.len(), 2);
        assert_eq!(aliases[0].0, "monospace");
        assert_eq!(aliases[0].1, vec!["DroidSansMono.ttf"]);
        assert_eq!(aliases[1].0, "casual");
        assert_eq!(aliases[1].1, vec!["ComingSoon.ttf"]);
    }

    #[test]
    fn parse_fonts_xml_aliases_skips_nameless_and_rejects_garbage() {
        let xml = FONTS_XML_SNIPPET.replace("<family name=\"casual\">", "<family>");
        let aliases = super::parse_fonts_xml_aliases(&xml);
        assert_eq!(aliases.len(), 1);
        assert_eq!(aliases[0].0, "monospace");
        assert!(super::parse_fonts_xml_aliases("not xml at all").is_empty());
        assert!(super::parse_fonts_xml_aliases("").is_empty());
    }

    #[test]
    fn locale_fonts_xml_langs_matches_aosp_tags() {
        assert_eq!(
            super::locale_fonts_xml_langs("zh-CN"),
            &["zh-Hans", "zh-CN", "zh", "und-Hani"]
        );
        assert_eq!(
            super::locale_fonts_xml_langs("zh-TW"),
            &["zh-Hant", "zh-TW", "zh-HK", "zh", "und-Hani"]
        );
        assert_eq!(super::locale_fonts_xml_langs("ja"), &["ja"]);
        assert_eq!(super::locale_fonts_xml_langs("ko"), &["ko"]);
        assert!(super::locale_fonts_xml_langs("en-US").is_empty());
    }
}

/// NDK `ASystemFontIterator` 绑定（android/font.h + android/system_fonts.h，API 29+，
/// minSdk 33）：枚举平台已知的所有系统字体，避免静态目录表漏掉 OEM 位置与新分区。
#[cfg(target_os = "android")]
mod android_font_iterator {
    use std::ffi::{CStr, c_char};
    use std::path::PathBuf;

    /// 不透明迭代器句柄。
    #[repr(C)]
    pub(super) struct ASystemFontIterator {
        _private: [u8; 0],
    }

    /// `ASystemFontIterator_next` 返回的不透明字体句柄。
    #[repr(C)]
    pub(super) struct AFont {
        _private: [u8; 0],
    }

    // SAFETY: 这些是 system_fonts.h/font.h 声明的稳定 NDK C 函数；所有指针均为同一
    // API 族产生并消费的不透明句柄。
    #[link(name = "android")]
    unsafe extern "C" {
        fn ASystemFontIterator_open() -> *mut ASystemFontIterator;
        fn ASystemFontIterator_next(iterator: *mut ASystemFontIterator) -> *mut AFont;
        fn ASystemFontIterator_close(iterator: *mut ASystemFontIterator);
        fn AFont_getFontFilePath(font: *const AFont) -> *const c_char;
        fn AFont_close(font: *mut AFont);
    }

    /// 枚举所有系统字体的绝对路径；API 失败或未安装字体时返回空 vec，由调用方回退到
    /// 静态 `FONT_DIRS` 扫描。
    pub(super) fn enumerate_font_paths() -> Vec<PathBuf> {
        let mut paths = Vec::new();
        // SAFETY: open() 或返回有效迭代器或返回 null；迭代器经 ASystemFontIterator_close()
        // 恰好关闭一次；next() 返回的每个 AFont 均经 AFont_close() 关闭；
        // AFont_getFontFilePath() 返回的路径指针按头文件契约仅在其所属 AFont 存活期间读取。
        unsafe {
            let iterator = ASystemFontIterator_open();
            if iterator.is_null() {
                log::warn!("FONT_LOAD: ASystemFontIterator_open failed");
                return paths;
            }
            loop {
                let font = ASystemFontIterator_next(iterator);
                if font.is_null() {
                    break;
                }
                let path_ptr = AFont_getFontFilePath(font);
                if !path_ptr.is_null() {
                    let cstr = CStr::from_ptr(path_ptr);
                    if let Ok(path) = cstr.to_str() {
                        paths.push(PathBuf::from(path));
                    } else {
                        log::warn!("FONT_LOAD: non-UTF-8 font path skipped");
                    }
                }
                AFont_close(font);
            }
            ASystemFontIterator_close(iterator);
        }
        paths
    }
}
