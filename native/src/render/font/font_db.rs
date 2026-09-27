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

/// 按序尝试的 fonts.xml 位置：部分 ROM 只提供 `fonts_fallback.xml`。
#[cfg(target_os = "android")]
pub(crate) const FONTS_XML_CANDIDATES: [&str; 2] =
    ["/system/etc/fonts.xml", "/system/etc/fonts_fallback.xml"];

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

/// 追加单条额外字体路径（已存在则跳过）：`loadFontFile` 探测不得覆盖用户目录。
#[cfg(target_os = "android")]
pub fn add_extra_font_path(path: std::path::PathBuf) {
    let mut extra = EXTRA_FONT_PATHS.write();
    if !extra.contains(&path) {
        extra.push(path);
    }
    log::debug!("FONT_LOAD: {} extra font paths", extra.len());
}

/// 渲染侧的精简字体库：只装 fonts.xml 中**一个符号族** + **一个区域族**，
/// 加上主字体（`~/.termux/font.ttf|ttc|otf`；为空时用 fonts.xml 的 monospace 族）。
///
/// DESIGN 字体节要求「遵循 Android 系统 fonts.xml」，而终端实际只用得上这几族：
/// 符号层承载 ▶ ⏵ ♥ ★，区域层承载当前语言的 CJK，其余 200 余个族是
/// WebView/UI 用字，渲染路径永不触及，故不加载。
#[cfg(target_os = "android")]
pub(crate) fn load_font_database() -> fontdb::Database {
    let db = CACHED_FONT_DB.get_or_init(|| {
        let mut db = fontdb::Database::new();
        let mut loaded = 0u32;

        // 主字体优先：用户投放的 `~/.termux/font.ttf|ttc|otf`，否则用 fonts.xml
        // 的 monospace 族（`resolve_system_monospace_from_fonts_xml` 负责缺失时 abort）。
        let primary = user_font_files();
        if primary.is_empty() {
            let target = resolve_system_monospace_from_fonts_xml();
            loaded += load_files(&mut db, &resolve_font_files(std::slice::from_ref(&target)));
        } else {
            loaded += load_files(&mut db, &primary);
        }

        // 一个符号族 + 一个区域族。fonts.xml 缺失时这两项为空：符号缺失只是
        // ▶ ⏵ ♥ ★ 变豆腐块，区域缺失只是 CJK 变豆腐块，都不该让应用不可用。
        let Some(content) = read_fonts_xml() else {
            log::warn!("FONT_LOAD: fonts.xml 不可读，符号与区域回退族为空");
            log::debug!("FONT_LOAD: loaded {loaded} font files");
            return db;
        };
        let symbol = resolve_font_files(&symbol_family_files(&content));
        loaded += load_files(&mut db, &symbol);
        let region = resolve_font_files(&locale_fallback_files(&content, &current_locale()));
        loaded += load_files(&mut db, &region);

        log::debug!(
            "FONT_LOAD: loaded {loaded} font files, {} faces",
            db.faces().count()
        );
        db
    });
    db.clone()
}

/// 按需加载一个字体族：设置页选中但尚未装入的族走这里。
#[cfg(target_os = "android")]
pub(crate) fn load_family(db: &mut fontdb::Database, family: &str) -> bool {
    let Some(files) = family_files(family) else {
        return false;
    };
    load_files(db, &resolve_font_files(files)) > 0
}

/// 字体族索引项：`display_name` 是展示用原始族名，`files` 是 fonts.xml 声明的
/// 文件名（加载时按平台字体目录表解析成路径）。
#[cfg(target_os = "android")]
pub(crate) struct FamilyEntry {
    pub display_name: String,
    pub files: Vec<String>,
}

/// 族名 → 字体文件。**只在设置页显示字体列表时构建**，渲染路径永不触发：
/// 族名存在字体的 name 表里，只能读完全部声明文件才能得到，实测约 4ms。
/// 按 fonts.xml 文档顺序返回，`~/.termux/fonts` 投放字体追加在后；精确去重，
/// 不排序、不归并（外部库 name 表为准）。
#[cfg(target_os = "android")]
pub(crate) fn family_index() -> &'static Vec<FamilyEntry> {
    static INDEX: std::sync::OnceLock<Vec<FamilyEntry>> = std::sync::OnceLock::new();
    INDEX.get_or_init(|| {
        let mut ordered_names: Vec<String> = Vec::new();
        let mut display_to_files: std::collections::HashMap<String, Vec<String>> =
            Default::default();
        let mut push_family = |family: String, file_label: String| {
            if let std::collections::hash_map::Entry::Vacant(entry) =
                display_to_files.entry(family.clone())
            {
                entry.insert(Vec::new());
                ordered_names.push(family);
            }
            if let Some(files) = display_to_files.get_mut(&family) {
                if !files.contains(&file_label) {
                    files.push(file_label);
                }
            }
        };
        if let Some(content) = read_fonts_xml() {
            for filename in parse_fonts_xml_declared_files(&content) {
                let Some(path) = resolve_font_path(&filename) else {
                    continue;
                };
                let mut db = fontdb::Database::new();
                if db.load_font_file(&path).is_err() {
                    continue;
                }
                for face in db.faces() {
                    for (family, _) in &face.families {
                        push_family(family.clone(), filename.clone());
                    }
                }
            }
        }
        for path in user_font_files() {
            let mut db = fontdb::Database::new();
            if db.load_font_file(&path).is_err() {
                continue;
            }
            let label = path
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or_default()
                .to_string();
            for face in db.faces() {
                for (family, _) in &face.families {
                    push_family(family.clone(), label.clone());
                }
            }
        }
        let entries: Vec<FamilyEntry> = ordered_names
            .into_iter()
            .filter_map(|display_name| {
                display_to_files
                    .remove(&display_name)
                    .map(|files| FamilyEntry {
                        display_name,
                        files,
                    })
            })
            .collect();
        log::debug!("FONT_INDEX: {} families indexed", entries.len());
        entries
    })
}

/// 按族名（忽略大小写）取其字体文件。
#[cfg(target_os = "android")]
pub(crate) fn family_files(family: &str) -> Option<&'static [String]> {
    family_index()
        .iter()
        .find(|entry| entry.display_name.eq_ignore_ascii_case(family))
        .map(|entry| entry.files.as_slice())
}

/// 当前系统语言（BCP 47），由 JNI `setSystemLocale` 经 `set_current_locale` 写入。
/// locale 归渲染层自有：`load_font_database` 在管线创建前读取，不回读 android 层
///（层方向只许 android → render）。不读进程环境变量：Android 上 `LANG` 是
/// `zh_CN.UTF-8`（下划线），与 fonts.xml 的 `zh-Hans` 标签体系对不上。
#[cfg(target_os = "android")]
static CURRENT_LOCALE: parking_lot::RwLock<String> = parking_lot::RwLock::new(String::new());

/// 写入当前系统语言（BCP 47），供管线创建前的区域回退族决策使用。
#[cfg(target_os = "android")]
pub fn set_current_locale(locale: String) {
    *CURRENT_LOCALE.write() = locale;
}

#[cfg(target_os = "android")]
pub(crate) fn current_locale() -> String {
    CURRENT_LOCALE.read().clone()
}

/// fonts.xml 里既无 `name` 也无 `lang` 的族即符号层：实测 emulator 的
/// `NotoSansSymbols-Regular-Subsetted.ttf` 正是声明在这里。
#[cfg(any(target_os = "android", test))]
pub(crate) fn symbol_family_files(xml: &str) -> Vec<String> {
    let Ok(document) = roxmltree::Document::parse(xml) else {
        return Vec::new();
    };
    for family in document
        .root_element()
        .children()
        .filter(|node| node.is_element() && node.tag_name().name() == "family")
    {
        if family.attribute("name").is_some() || family.attribute("lang").is_some() {
            continue;
        }
        let filenames: Vec<String> = family
            .children()
            .filter(|node| node.is_element() && node.tag_name().name() == "font")
            .filter_map(|font| font.text().map(str::trim).map(str::to_string))
            .filter(|text| !text.is_empty())
            .collect();
        if !filenames.is_empty() {
            return filenames;
        }
    }
    Vec::new()
}

/// 当前语言对应的区域回退族（简中 → `zh-Hans` 的 NotoSansCJK）。
#[cfg(any(target_os = "android", test))]
pub(crate) fn locale_fallback_files(xml: &str, locale: &str) -> Vec<String> {
    let (_, lang_fallbacks) = parse_fonts_xml_families(xml);
    for wanted in locale_fonts_xml_langs(locale) {
        if let Some((_, filenames)) = lang_fallbacks
            .iter()
            .find(|(lang, _)| lang.split(',').any(|tag| tag == *wanted))
        {
            return filenames
                .iter()
                .map(|(filename, _)| filename.clone())
                .collect();
        }
    }
    Vec::new()
}

/// 用户投放字体：`~/.termux/font` 目录下的 ttf/ttc/otf。
#[cfg(target_os = "android")]
fn user_font_files() -> Vec<std::path::PathBuf> {
    let mut files = Vec::new();
    for path in EXTRA_FONT_PATHS.read().iter() {
        if path.is_file() {
            files.push(path.clone());
        } else if path.is_dir()
            && let Ok(entries) = std::fs::read_dir(path)
        {
            files.extend(
                entries
                    .flatten()
                    .map(|entry| entry.path())
                    .filter(|file_path| is_font_file(file_path)),
            );
        }
    }
    files
}

/// `fonts.xml` 只给文件名，路径由平台字体目录表解析；解析不到的文件丢弃。
#[cfg(target_os = "android")]
fn resolve_font_files(filenames: &[String]) -> Vec<std::path::PathBuf> {
    filenames
        .iter()
        .filter_map(|filename| resolve_font_path(filename))
        .collect()
}

#[cfg(target_os = "android")]
pub(crate) fn read_fonts_xml() -> Option<String> {
    FONTS_XML_CANDIDATES
        .iter()
        .find_map(|path| std::fs::read_to_string(path).ok())
}

/// CJK 回退读取 fonts.xml：与 `read_fonts_xml` 同一候选顺序，测试目标下直接试读
/// 系统路径（宿主无该文件即为空，不猜测）。
#[cfg(any(target_os = "android", test))]
pub(crate) fn read_fonts_xml_fallback() -> Option<String> {
    #[cfg(target_os = "android")]
    {
        read_fonts_xml()
    }
    #[cfg(not(target_os = "android"))]
    {
        ["/system/etc/fonts.xml", "/system/etc/fonts_fallback.xml"]
            .iter()
            .find_map(|path| std::fs::read_to_string(path).ok())
    }
}

#[cfg(target_os = "android")]
fn load_files(db: &mut fontdb::Database, paths: &[std::path::PathBuf]) -> u32 {
    let mut count = 0u32;
    for path in paths {
        if let Err(error) = db.load_font_file(path) {
            // 只记文件名：完整路径可能带出用户主目录。
            log::warn!(
                "font: failed to load font file {}: {error}",
                path.file_name().unwrap_or_default().to_string_lossy()
            );
        } else {
            count += 1;
        }
    }
    count
}

/// 按平台字体目录表解析 `fonts.xml` 声明的文件名，首个命中即为该字体。
#[cfg(target_os = "android")]
fn resolve_font_path(filename: &str) -> Option<std::path::PathBuf> {
    FONT_DIRS
        .iter()
        .map(|dir| std::path::Path::new(dir).join(filename))
        .find(|path| path.is_file())
}

/// 系统等宽字体文件名，取自 `fonts.xml`（DESIGN 字体节：fonts.xml 是唯一来源，
/// 不得使用任何硬编码字体名）。
///
/// 规范要求「系统不存在 fonts.xml 或其内容无法解析，输出日志并崩溃退出」：
/// 候选文件都读不到、都无法解析、或都没给出等宽字体时直接 `abort`。
/// 宿主（非 Android）不参与：那里没有 fonts.xml，见下方 `#[cfg]` 版本。
#[cfg(target_os = "android")]
pub(crate) fn resolve_system_monospace_from_fonts_xml() -> String {
    let mut last_error = String::new();
    for xml_path in FONTS_XML_CANDIDATES {
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

/// `fonts.xml` 声明的全部字体文件名（去重前，按文档顺序）。
///
/// 覆盖**每个** `<family>` 下的 `<font>`，包括既无 `name` 也无 `lang` 的族：
/// 实测 emulator（API 35）的 `NotoSansSymbols-Regular-Subsetted*.ttc` 正声明在无名族里，
/// 终端的 ▶ ⏵ ♥ ★ 依赖它，按 name/lang 过滤会漏掉。
#[cfg(any(target_os = "android", test))]
pub(crate) fn parse_fonts_xml_declared_files(xml: &str) -> Vec<String> {
    let mut filenames = Vec::new();
    let Ok(document) = roxmltree::Document::parse(xml) else {
        log::error!("FONT_XML: 解析失败");
        return filenames;
    };
    for family in document
        .root_element()
        .children()
        .filter(|node| node.is_element() && node.tag_name().name() == "family")
    {
        for font in family
            .children()
            .filter(|node| node.is_element() && node.tag_name().name() == "font")
        {
            if let Some(filename) = font.text().map(str::trim).filter(|text| !text.is_empty()) {
                filenames.push(filename.to_string());
            }
        }
    }
    filenames
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
    <family>
        <font>NotoSansSymbols-Regular-Subsetted.ttf</font>
    </family>
</familyset>"#;

    /// 符号层取自既无 `name` 也无 `lang` 的族：实测 emulator（API 35）的
    /// `NotoSansSymbols-Regular-Subsetted.ttf` 正声明在那里，按 name/lang 过滤会漏掉，
    /// 终端将失去 ▶ ⏵ ♥ ★。
    #[test]
    fn symbol_family_comes_from_nameless_family() {
        assert_eq!(
            super::symbol_family_files(FONTS_XML_SNIPPET),
            vec!["NotoSansSymbols-Regular-Subsetted.ttf"]
        );
        assert!(super::symbol_family_files("<familyset></familyset>").is_empty());
        assert!(super::symbol_family_files("not xml").is_empty());
    }

    /// 区域族按当前语言取 `lang` 匹配项；语言不匹配时为空（不猜）。
    #[test]
    fn locale_family_follows_system_language() {
        assert_eq!(
            super::locale_fallback_files(FONTS_XML_SNIPPET, "zh-CN"),
            vec!["NotoSansCJK-Regular.ttc"]
        );
        assert_eq!(
            super::locale_fallback_files(FONTS_XML_SNIPPET, "ja"),
            vec!["NotoSansCJK-Regular.ttc"]
        );
        assert!(
            super::locale_fallback_files(FONTS_XML_SNIPPET, "en-US").is_empty(),
            "非 CJK 语言不得回退到 CJK 族"
        );
    }

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
    fn parse_fonts_xml_declared_files_includes_nameless_family() {
        // emulator（API 35）的符号字体声明在既无 name 也无 lang 的族里，
        // 按 name/lang 过滤会漏掉，终端将失去 ▶ ⏵ ♥ ★。
        let xml = r#"<familyset>
            <family name="monospace"><font>DroidSansMono.ttf</font></family>
            <family lang="zh-Hans"><font index="2">NotoSansCJK-Regular.ttc</font></family>
            <family><font>NotoSansSymbols-Regular-Subsetted.ttf</font></family>
        </familyset>"#;
        assert_eq!(
            super::parse_fonts_xml_declared_files(xml),
            vec![
                "DroidSansMono.ttf",
                "NotoSansCJK-Regular.ttc",
                "NotoSansSymbols-Regular-Subsetted.ttf",
            ]
        );
    }

    #[test]
    fn parse_fonts_xml_declared_files_rejects_garbage() {
        assert!(super::parse_fonts_xml_declared_files("not xml at all").is_empty());
        assert!(super::parse_fonts_xml_declared_files("").is_empty());
        assert!(
            super::parse_fonts_xml_declared_files("<familyset><family></family></familyset>")
                .is_empty()
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
