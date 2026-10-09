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

/// 按序尝试的 fonts.xml 位置：部分 ROM 只提供 `fonts_fallback.xml`。宿主非测试构建下
/// 两者都不存在，`read_fonts_xml` 自然返回 `None`，故无需平台分支。
pub(crate) const FONTS_XML_CANDIDATES: [&str; 2] =
    ["/system/etc/fonts.xml", "/system/etc/fonts_fallback.xml"];

#[cfg(target_os = "android")]
static CACHED_FONT_DB: std::sync::OnceLock<fontdb::Database> = std::sync::OnceLock::new();

/// GUI 层提供的额外字体路径：由 `set_extra_font_paths()` 写入，供 `family_index()`
/// 枚举列表与 `load_family()` 按需装入使用；渲染常驻字体库不读它（主字体恒为
/// fonts.xml 的 monospace 族，见 `load_font_database`）。
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

/// 渲染侧的精简字体库：只装 fonts.xml 的 monospace 族（主字体）+ **一个符号族** +
/// **一个区域族**。
///
/// 主字体恒为 fonts.xml 的 monospace 族（DESIGN：主字体为空时取 fonts.xml 的
/// monospace 字体）：用户 `~/.termux/font.{ttf,ttc,otf}` 覆盖经 Kotlin 探测后由
/// `setFontFamily` 按需装入，`~/.termux/fonts` 投放字体只进字体列表、选中时经
/// `load_family` 装入，两者都不在建库时常驻——否则投放字体会抢占 fonts.xml 等宽
/// 主字体令其选择失败（启动 abort），且建库结果依赖 `setExtraFontPaths` 时序。
///
/// DESIGN 字体节要求「遵循 Android 系统 fonts.xml」，而终端实际只用得上这几族：
/// 符号层承载 ▶ ⏵ ♥ ★，区域层承载当前语言的 CJK，其余 200 余个族是
/// WebView/UI 用字，渲染路径永不触及，故不加载。例外见 `widen_to_declared_set`。
#[cfg(target_os = "android")]
pub(crate) fn load_font_database() -> fontdb::Database {
    let font_database = CACHED_FONT_DB.get_or_init(|| {
        let mut font_database = fontdb::Database::new();

        // 主字体：fonts.xml 的 monospace 族**整族**（`resolve_system_monospace_files`
        // 负责 fonts.xml 缺失或不可解析时崩溃退出）。只装族内首个时，该文件一旦不可用
        // 就再无任何面可降级，等于把「某个字体文件缺失」升级成崩溃。
        let mut loaded = load_files(
            &mut font_database,
            &resolve_font_files(resolve_system_monospace_files()),
        );

        // 一个符号族 + 一个区域族。fonts.xml 缺失时这两项为空：符号缺失只是
        // ▶ ⏵ ♥ ★ 变豆腐块，区域缺失只是 CJK 变豆腐块，都不该让应用不可用。
        let Some(content) = read_fonts_xml() else {
            log::warn!("FONT_LOAD: fonts.xml 不可读，符号与区域回退族为空");
            log::debug!("FONT_LOAD: loaded {loaded} font files");
            return font_database;
        };
        let symbol = resolve_font_files(&symbol_family_files(&content));
        loaded += load_files(&mut font_database, &symbol);
        let region = resolve_font_files(&locale_fallback_files(&content, &current_locale()));
        loaded += load_files(&mut font_database, &region);

        if !db_has_monospaced(&font_database) {
            loaded += widen_to_declared_set(&mut font_database, &content);
        }

        log::debug!(
            "FONT_LOAD: loaded {loaded} font files, {} faces",
            font_database.faces().count()
        );
        font_database
    });
    font_database.clone()
}

/// 库内是否已有任一等宽面：主字体选择梯次的建库侧不变量——最小常驻集不含
/// 等宽面时必须放宽装库，否则 `select_primary_face` 无从选择。
#[cfg(any(target_os = "android", test))]
pub(crate) fn db_has_monospaced(font_database: &fontdb::Database) -> bool {
    font_database.faces().any(|face| face.monospaced)
}

/// OEM 精简 ROM 可能声明了等宽字体却不提供文件（或文件损坏），使最小常驻集
/// 没有任何等宽面：放宽装入 fonts.xml 全量声明文件（跳过已在库中的），保证
/// 库内至少有一个可用面，返回新装入的文件数。健康设备永不进入此路径。
#[cfg(target_os = "android")]
fn widen_to_declared_set(font_database: &mut fontdb::Database, content: &str) -> u32 {
    let mut loaded = 0u32;
    for path in resolve_font_files(&parse_fonts_xml_declared_files(content)) {
        let Some(filename) = path
            .file_name()
            .and_then(|name| name.to_str())
            .map(str::to_string)
        else {
            continue;
        };
        if font_file_is_loaded(font_database, &filename) {
            continue;
        }
        loaded += load_files(font_database, std::slice::from_ref(&path));
    }
    log::error!("FONT_LOAD: fonts.xml 等宽字体不可用，放宽到全量声明集（+{loaded} 文件）");
    loaded
}

/// 按需加载一个字体族：设置页选中但尚未装入的族走这里。
#[cfg(target_os = "android")]
pub(crate) fn load_family(font_database: &mut fontdb::Database, family: &str) -> bool {
    let Some(files) = family_files(family) else {
        return false;
    };
    load_files(font_database, &resolve_font_files(files)) > 0
}

/// 字体族索引项：`display_name` 是展示用原始族名，`files` 是字体文件定位串。
/// 系统族存 `fonts.xml` 里的裸文件名（加载时经平台字体目录表解析）；用户投放族存
/// 绝对路径——它不在任何字体目录内，只留文件名会在按需装入时解析到别的文件或解析不到。
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
            let files = display_to_files.entry(family.clone()).or_insert_with(|| {
                ordered_names.push(family.clone());
                Vec::new()
            });
            if !files.contains(&file_label) {
                files.push(file_label);
            }
        };
        if let Some(content) = read_fonts_xml() {
            for filename in parse_fonts_xml_declared_files(&content) {
                let Some(path) = resolve_font_path(&filename) else {
                    continue;
                };
                let mut font_database = fontdb::Database::new();
                if font_database.load_font_file(&path).is_err() {
                    continue;
                }
                for face in font_database.faces() {
                    for (family, _) in &face.families {
                        push_family(family.clone(), filename.clone());
                    }
                }
            }
        }
        for path in user_font_files() {
            let mut font_database = fontdb::Database::new();
            if font_database.load_font_file(&path).is_err() {
                continue;
            }
            // 用户投放字体存绝对路径：它不在任何 FONT_DIRS 内，按裸文件名解析只会
            // 落空（同名系统字体时更会装错文件），使 `font.ttf` 永远选不中。
            let label = path.to_string_lossy().into_owned();
            for face in font_database.faces() {
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
///
/// 只作**建库时的提示**：locale 可能晚于建库才到达（spawn 前的调用被
/// `Bridge.onSession` 的 `sessionId == 0` 守卫丢弃），届时由
/// `FontPipeline::set_system_locale` 经 `load_region_fallback_faces` 增补，
/// 两条路径以本值为准。
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

/// 当前语言对应的区域回退条目 `(文件名, ttc_index)`（简中 → `zh-Hans` 的 NotoSansCJK）。
/// 保留 `ttc_index` 供面级匹配使用；按 locale 优先级取首个匹配的 `lang` 块。
#[cfg(any(target_os = "android", test))]
pub(crate) fn locale_fallback_entries(xml: &str, locale: &str) -> Vec<(String, u32)> {
    let (_, lang_fallbacks) = parse_fonts_xml_families(xml);
    for wanted in locale_fonts_xml_langs(locale) {
        if let Some((_, filenames)) = lang_fallbacks
            .iter()
            .find(|(lang, _)| lang.split(',').any(|tag| tag == *wanted))
        {
            return filenames.clone();
        }
    }
    Vec::new()
}

/// 当前语言对应的区域回退族文件名（简中 → `zh-Hans` 的 NotoSansCJK）。
#[cfg(any(target_os = "android", test))]
pub(crate) fn locale_fallback_files(xml: &str, locale: &str) -> Vec<String> {
    locale_fallback_entries(xml, locale)
        .into_iter()
        .map(|(filename, _)| filename)
        .collect()
}

/// 库内是否已有该字体文件。装入一个 TTC 即含其全部面，故按文件名判定即可，
/// 无需逐面比较索引。
#[cfg(any(target_os = "android", test))]
pub(crate) fn font_file_is_loaded(font_database: &fontdb::Database, filename: &str) -> bool {
    font_database.faces().any(|face| {
        let path = match &face.source {
            fontdb::Source::File(path) | fontdb::Source::SharedFile(path, _) => path,
            fontdb::Source::Binary(_) => return false,
        };
        path.file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| name.eq_ignore_ascii_case(filename))
    })
}

/// 区域回退族中库内尚缺的 `(文件名, ttc_index)`。
///
/// 字体库在管线创建时构建，而 locale 由 JNI 在其之后才到达：spawn 前的调用被
/// `Bridge.onSession` 的 `sessionId == 0` 守卫丢弃，随后的重放又晚于库定型。
/// 故区域族必须能在 locale 到达后按本函数的结果补装，而不是指望它建库时就在。
/// 纯函数（库 + XML + locale → 缺失项），便于宿主单测覆盖该时序。
#[cfg(any(target_os = "android", test))]
pub(crate) fn missing_region_fallback_faces(
    font_database: &fontdb::Database,
    xml: &str,
    system_locale: &str,
) -> Vec<(String, u32)> {
    let mut missing: Vec<(String, u32)> = Vec::new();
    for (filename, index) in locale_fallback_entries(xml, system_locale) {
        // 真机 `zh-Hans` 块把同一 TTC 按 weight 100..900 重复声明九次
        // （对照设备 /system/etc/fonts.xml）。装一个文件即含全部面，
        // 故按文件名去重、只补首个。
        if font_file_is_loaded(font_database, &filename) {
            continue;
        }
        if missing
            .iter()
            .any(|(present, _)| present.eq_ignore_ascii_case(&filename))
        {
            continue;
        }
        missing.push((filename, index));
    }
    missing
}

/// 把当前 locale 的区域回退族补装进**活动**字体库，返回新装入的面 id。
///
/// 只增补不重建整个库：`fontdb::ID` 是库内序号，重建会让主字体的 `font_id`
/// 指向另一个面。已在库中的文件跳过，重复调用只有一次比对开销。
#[cfg(target_os = "android")]
pub(crate) fn load_region_fallback_faces(
    font_database: &mut fontdb::Database,
    system_locale: &str,
) -> Vec<fontdb::ID> {
    let Some(content) = read_fonts_xml() else {
        log::warn!("FONT_LOAD: fonts.xml 不可读，区域回退族无法补装");
        return Vec::new();
    };
    let missing = missing_region_fallback_faces(font_database, &content, system_locale);
    let mut loaded = Vec::new();
    for path in resolve_font_files(
        &missing
            .into_iter()
            .map(|(filename, _)| filename)
            .collect::<Vec<String>>(),
    ) {
        // `load_font_source` 直接返回新面 id，解析失败时为空 vec。
        let ids = font_database.load_font_source(fontdb::Source::File(path));
        if ids.is_empty() {
            log::warn!("FONT_LOAD: 区域回退族装入失败（无法解析）");
        }
        loaded.extend(ids);
    }
    if !loaded.is_empty() {
        log::debug!(
            "FONT_LOAD: locale='{system_locale}' 补装 {} 个区域回退族面",
            loaded.len()
        );
    }
    loaded
}

/// 用户投放字体：`~/.termux/font` 目录下的 ttf/ttc/otf。仅供 `family_index()`
/// 枚举列表使用；渲染常驻字体库不装它们（选中时经 `load_family` 按需装入）。
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
fn resolve_font_files(filenames: &[impl AsRef<str>]) -> Vec<std::path::PathBuf> {
    filenames
        .iter()
        .filter_map(|filename| resolve_font_entry(filename.as_ref()))
        .collect()
}

/// 族索引项的文件定位串 → 实际路径：绝对路径原样使用（用户投放字体不在任何
/// `FONT_DIRS` 内），裸文件名才走平台字体目录表。
///
/// 绝不能对绝对路径也走目录表：用户把字体命名为某个系统字体的文件名时，
/// 会静默装入系统的那一份，用户选的族与实际渲染的不是同一个字体。
#[cfg(any(target_os = "android", test))]
pub(crate) fn resolve_font_entry(entry: &str) -> Option<std::path::PathBuf> {
    let path = std::path::Path::new(entry);
    if path.is_absolute() {
        return path.is_file().then(|| path.to_path_buf());
    }
    resolve_font_path(entry)
}

pub(crate) fn read_fonts_xml() -> Option<String> {
    FONTS_XML_CANDIDATES
        .iter()
        .find_map(|path| std::fs::read_to_string(path).ok())
}

#[cfg(target_os = "android")]
fn load_files(font_database: &mut fontdb::Database, paths: &[std::path::PathBuf]) -> u32 {
    let mut count = 0u32;
    for path in paths {
        if let Err(error) = font_database.load_font_file(path) {
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
#[cfg(any(target_os = "android", test))]
fn resolve_font_path(filename: &str) -> Option<std::path::PathBuf> {
    FONT_DIRS
        .iter()
        .map(|dir| std::path::Path::new(dir).join(filename))
        .find(|path| path.is_file())
}

/// 字体致命退出的唯一出口（`DESIGN.md:16` 的「输出日志并崩溃退出」）：先输出一行
/// `FONT_FATAL` 错误日志，再 `abort`。
///
/// 用 `log` 门面而非直写 `__android_log_write`：层方向禁止 `render` 依赖 `android`
/// （semgrep `no-android-in-render`），而 Android logger 已由 `JNI_OnLoad` 安装
/// （早于任何 JNI 方法），故该行必然落 logcat。`reason` MUST 陈述真实原因，不得归因
/// 到别处——现场通常只剩 strip 过的 tombstone，这一行是唯一的诊断入口。
#[cfg(target_os = "android")]
pub(crate) fn fatal(reason: &str) -> ! {
    log::error!(target: "FONT_FATAL", "{reason}");
    std::process::abort()
}

/// 等宽字体文件名列表，进程内解析一次：建库与 `find_monospace_font` 共用同一结果。
///
/// `fonts.xml` 在进程生命周期内不变，而 `set_font_family("")` 会反复进入
/// `find_monospace_font`，每次重读重解析整份 XML 都落在渲染线程上。
#[cfg(target_os = "android")]
static MONOSPACE_XML_FILES: std::sync::OnceLock<Vec<String>> = std::sync::OnceLock::new();

/// 缓存的等宽字体文件名，按 `fonts.xml` 声明顺序。
///
/// 整族而非首个：族内通常声明多个 `<font>`，只装第一个时它一旦不可用（OEM 改名、
/// 分区迁移、`FONT_DIRS` 未覆盖），库里便没有任何面而崩溃——那不是 fonts.xml
/// 「无法解析」，不该走 [`fatal`]。首个仍是 `find_monospace_font` 的匹配目标。
#[cfg(target_os = "android")]
pub(crate) fn resolve_system_monospace_files() -> &'static [String] {
    MONOSPACE_XML_FILES.get_or_init(resolve_system_monospace_from_fonts_xml)
}

/// 系统等宽字体文件名列表，取自 `fonts.xml`（DESIGN 字体节：fonts.xml 是唯一来源，
/// 不得使用任何硬编码字体名）。
///
/// 规范要求「系统不存在 fonts.xml 或其内容无法解析，输出日志并崩溃退出」：
/// 候选文件都读不到、都无法解析、或都没给出等宽字体时经 [`fatal`] 退出。
/// 宿主（非 Android）不参与：那里没有 fonts.xml。
#[cfg(target_os = "android")]
fn resolve_system_monospace_from_fonts_xml() -> Vec<String> {
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
        if !monospace.is_empty() {
            log::debug!("FONT_XML: monospace targets={monospace:?}");
            return monospace;
        }
        last_error = format!("{xml_path} 未声明等宽字体");
    }
    fatal(&format!(
        "无法从系统 fonts.xml 解析等宽字体（{last_error}）"
    ))
}

/// 面是否覆盖基本拉丁（探测字符 'm'，与单元格度量所用字符一致）：终端主字体的
/// 最低可用门槛——emoji 字体（如 Noto Color Emoji Flags）会被 fontdb 标记为等宽
/// 却没有任何拉丁字形，选中它们主字体只剩豆腐块。判定用字形能力探测（charmap），
/// 不是硬编码字体名。
#[cfg(any(target_os = "android", test))]
fn face_covers_latin(font_database: &fontdb::Database, face_id: fontdb::ID) -> bool {
    font_database
        .with_face_data(face_id, |font_data, face_index| {
            swash::FontRef::from_index(font_data, face_index as usize)
                .map(|font| font.charmap().map('m') != 0)
        })
        .flatten()
        .unwrap_or(false)
}

/// 主字体面选择梯次（fonts.xml 是默认主字体的唯一来源）：
/// 1. 声明的等宽文件按词干匹配——fonts.xml 给文件名、fontdb 给家族名，先用
///    「分隔符归一 + 精确相等」，再退「去空白后精确相等」，覆盖 Droid Sans Mono
///    这类家族名带空格而文件名不带的差异；
/// 2. 库内首个覆盖拉丁的等宽面：声明文件被 OEM 改包装（家族名与词干对不上）或
///    放宽装库后声明文件本就不存在时，仍拿到可用的等宽字体——emoji 这类无拉丁
///    字形的「等宽」面必须跳过；
/// 3. 库内首个覆盖拉丁的面：全库无可用等宽面时的最后手段——单元格度量按该字体
///    计算，终端降级可用胜过启动即崩溃；
/// 4. 库内首个面：连拉丁字体都没有（仅剩符号/emoji 字体）也先让终端与设置页
///    可用，用户仍可在设置中改选字体；
/// 5. 库为空返回 `None`，由调用方按「fonts.xml 不可用」输出日志并崩溃。
#[cfg(any(target_os = "android", test))]
pub(crate) fn select_primary_face(
    font_database: &fontdb::Database,
    target_filename: &str,
) -> Option<fontdb::ID> {
    let stem = std::path::Path::new(target_filename)
        .file_stem()
        .and_then(|stem| stem.to_str())
        .unwrap_or_default();
    let stem_lower = stem.to_lowercase().replace(['-', '_'], " ");
    let stem_nospace = stem_lower.replace(' ', "");
    let family_lower = |face: &fontdb::FaceInfo| {
        face.families
            .first()
            .map_or("", |(name, _)| name)
            .to_lowercase()
    };
    if let Some(face_id) = font_database
        .faces()
        .filter(|face| face.monospaced)
        .find(|face| family_lower(face).replace(['-', '_'], " ") == stem_lower)
        .or_else(|| {
            font_database.faces().find(|face| {
                face.monospaced
                    && family_lower(face)
                        .chars()
                        .filter(|character| !character.is_whitespace())
                        .collect::<String>()
                        == stem_nospace
            })
        })
        .map(|face| face.id)
    {
        log::debug!("FONT_SELECT: fonts.xml monospace id={face_id:?} stem='{stem}'");
        return Some(face_id);
    }
    if let Some(face) = font_database
        .faces()
        .find(|face| face.monospaced && face_covers_latin(font_database, face.id))
    {
        log::warn!(
            "FONT_SELECT: fonts.xml 声明的等宽字体 {target_filename} 未匹配，改用库内等宽面 '{}'",
            family_lower(face)
        );
        return Some(face.id);
    }
    if let Some(face) = font_database
        .faces()
        .find(|face| face_covers_latin(font_database, face.id))
    {
        log::error!(
            "FONT_SELECT: 库内无可用等宽面，降级使用首个拉丁面 '{}'",
            family_lower(face)
        );
        return Some(face.id);
    }
    if let Some(face) = font_database.faces().next() {
        log::error!(
            "FONT_SELECT: 库内无拉丁字体，降级使用首个可用面 '{}'",
            family_lower(face)
        );
        return Some(face.id);
    }
    None
}

type FontsXmlFamilies = (Vec<String>, Vec<(String, Vec<(String, u32)>)>);

/// 解析 `fonts.xml`，产出等宽字体文件名与有序的 `(lang, [(filename, ttc_index)])`
/// 回退条目。纯函数，便于宿主测试喂入真实设备片段；无法解析的输入产出空列表。
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
        // 常规字重（weight=400 或不写）排在族内最前：调用方按序取前若干个，
        // AOSP 把 Thin/Light/DemiLight 排在 Regular 之前，直接按文档序取会先拿到
        // 超细字重——spec 要求的却是常规字重的那个面。
        let mut regular_filenames = Vec::new();
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
            // `fallbackFor="serif"` 之类是**另一条** fallback 链专用的字体：同一族里
            // 常与正文字体同名同权重并列（如 zh-Hans 的 NotoSerifCJK-Regular.ttc）。
            // 终端主链按字体文件取面，收进来就会让 serif 面与正体竞争同一字符。
            if font.attribute("fallbackFor").is_some() {
                log::debug!(
                    "FONT_XML: skip fallbackFor={} font='{filename}'",
                    font.attribute("fallbackFor").unwrap_or_default()
                );
                continue;
            }
            let is_regular = font
                .attribute("weight")
                .and_then(|value| value.parse().ok())
                .unwrap_or(400)
                == 400;
            if is_regular {
                regular_filenames.push((filename.to_string(), index));
            } else {
                filenames.push((filename.to_string(), index));
            }
        }
        filenames.splice(0..0, regular_filenames);
        if filenames.is_empty() {
            continue;
        }
        if let Some(name) = family.attribute("name") {
            // AOSP 的族名是 `sans-serif-monospace`/`serif-monospace`（连字符），
            // 与真机 fonts.xml 一致（见本文件测试里逐条摘录的片段）。
            if ["monospace", "sans-serif-monospace", "serif-monospace"].contains(&name) {
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

/// 系统 locale → CJK 变体标记（`sc`/`tc`/`jp`/`kr`，非 CJK 为 `None`）。
/// 区域字体只有简繁之分，故其余 `zh` 子标签（如 `zh-SG`）归简中。
pub(crate) fn locale_cjk_variant(locale: &str) -> Option<&'static str> {
    if locale.starts_with("zh-CN") || locale.starts_with("zh-Hans") {
        Some("sc")
    } else if locale.starts_with("zh-TW")
        || locale.starts_with("zh-Hant")
        || locale.starts_with("zh-HK")
    {
        Some("tc")
    } else if locale.starts_with("zh") {
        Some("sc")
    } else if locale.starts_with("ja") {
        Some("jp")
    } else if locale.starts_with("ko") {
        Some("kr")
    } else {
        None
    }
}

/// 把系统 locale 标签映射为按优先级排列的 `fonts.xml` `lang` 候选。
/// AOSP 用 `zh-Hans`/`zh-Hant`，旧版本可能用 `zh-CN`。
pub(crate) fn locale_fonts_xml_langs(locale: &str) -> &'static [&'static str] {
    match locale_cjk_variant(locale) {
        Some("sc") => &["zh-Hans", "zh-CN", "zh", "und-Hani"],
        Some("tc") => &["zh-Hant", "zh-TW", "zh-HK", "zh", "und-Hani"],
        Some("jp") => &["ja"],
        Some("kr") => &["ko"],
        // `DESIGN.md:155` 限定区域字体取「本区域」，故非 CJK 系统语言下候选必须为空：
        // 放开会让英文系统也预装 CJK 族，违反该条。终端里出现非本区域文字即无回退，
        // 属规范现状而非缺陷——改它要先改 155（保护文件）。
        _ => &[],
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

    /// 宿主字体库里取一个真实字体文件（绝对路径）作为区域族探针。
    /// 区域族的「是否已在库内」按文件名判定，故任一字体文件即可。
    /// 依赖 dev shell 的 fontconfig（见 flake.nix 的 `FONTCONFIG_FILE`），
    /// 无字体时自然失败，不做环境检查也不跳过。
    fn probe_font_file() -> std::path::PathBuf {
        let mut font_database = fontdb::Database::new();
        font_database.load_system_fonts();
        font_database
            .faces()
            .find_map(|face| match &face.source {
                fontdb::Source::File(path) | fontdb::Source::SharedFile(path, _) => {
                    path.is_file().then(|| path.clone())
                }
                fontdb::Source::Binary(_) => None,
            })
            .expect("宿主字体库须有可加载的字体文件（run inside nix develop）")
    }

    /// 区域族的探针 XML：一个 `zh-Hans` 块声明探针文件。
    fn probe_fonts_xml(filename: &str) -> String {
        format!(
            r#"<familyset version="23"><family lang="zh-Hans"><font weight="400" style="normal" index="2">{filename}</font></family></familyset>"#
        )
    }

    /// 设备上的真实时序：字体库在 locale 到达之前就已定型（spawn 前的 locale
    /// 调用被 `Bridge.onSession` 的 `sessionId == 0` 守卫丢弃），区域族此时
    /// 必须被报为「缺失」以便事后补装；补装后不得再报，否则会重复装入。
    #[test]
    fn region_family_missing_until_loaded() {
        let probe = probe_font_file();
        let filename = probe
            .file_name()
            .and_then(|name| name.to_str())
            .expect("探针字体须有文件名")
            .to_string();
        let xml = probe_fonts_xml(&filename);

        let mut font_database = fontdb::Database::new();
        assert_eq!(
            super::missing_region_fallback_faces(&font_database, &xml, "zh-CN"),
            vec![(filename.clone(), 2)],
            "库在 locale 之前定型时区域族必须报为缺失，否则 zh-CN 用户无 CJK 可用"
        );

        font_database
            .load_font_file(&probe)
            .expect("探针字体须可装入");
        assert!(
            super::missing_region_fallback_faces(&font_database, &xml, "zh-CN").is_empty(),
            "补装后不得再报缺失，否则重复调用会重复装入同一个 TTC"
        );
    }

    /// 真机 `zh-Hans` 块把同一 TTC 按 weight 100..900 重复声明九次。装一个文件
    /// 即含其全部面，故缺失项必须按文件名去重，只补首个。
    #[test]
    fn region_family_deduplicates_repeated_weights() {
        let filename = "NotoSansCJK-Regular.ttc";
        let weights: String = (100..=900)
            .step_by(100)
            .map(|weight| {
                format!(r#"<font weight="{weight}" style="normal" index="2">{filename}</font>"#)
            })
            .collect();
        let xml = format!(
            r#"<familyset version="23"><family lang="zh-Hans">{weights}</family></familyset>"#
        );
        let font_database = fontdb::Database::new();
        assert_eq!(
            super::missing_region_fallback_faces(&font_database, &xml, "zh-CN"),
            vec![(filename.to_string(), 2)],
            "同一 TTC 的九个字重声明只补一个文件"
        );
    }

    /// 非 CJK locale 不补装任何 `lang` 族：否则 en-US 设备会被塞入整本 CJK 字体。
    #[test]
    fn non_cjk_locale_requests_no_region_fallback() {
        let font_database = fontdb::Database::new();
        let xml = probe_fonts_xml("NotoSansCJK-Regular.ttc");
        assert!(
            super::missing_region_fallback_faces(&font_database, &xml, "en-US").is_empty(),
            "非 CJK locale 不得补装区域回退族"
        );
        assert!(
            super::missing_region_fallback_faces(&font_database, &xml, "ja").is_empty(),
            "CJK locale 须报出待补装的区域族，与上方 en-US 构成对照"
        );
    }

    /// 已装入判定按文件名且忽略大小写：库内来源路径的目录前缀无关。
    #[test]
    fn font_file_loaded_matching_ignores_directory_and_case() {
        let probe = probe_font_file();
        let filename = probe
            .file_name()
            .and_then(|name| name.to_str())
            .expect("探针字体须有文件名");
        let mut font_database = fontdb::Database::new();
        assert!(!super::font_file_is_loaded(&font_database, filename));
        font_database
            .load_font_file(&probe)
            .expect("探针字体须可装入");
        assert!(super::font_file_is_loaded(&font_database, filename));
        assert!(super::font_file_is_loaded(
            &font_database,
            &filename.to_uppercase()
        ));
        assert!(!super::font_file_is_loaded(
            &font_database,
            "NoSuchFont-Regular.ttc"
        ));
    }

    /// 用户投放字体（`~/.termux/font.ttf`）存的是绝对路径，按需装入时必须原样
    /// 解析：它不在任何平台字体目录内，只留文件名的旧实现永远解析不到，
    /// `font.ttf 存在即默认` 因此在设备上完全失效。
    #[test]
    fn absolute_font_entry_resolves_without_font_dir_lookup() {
        let probe = probe_font_file();
        let entry = probe.to_string_lossy().into_owned();
        assert!(
            probe.is_absolute(),
            "probe font must be referenced by absolute path, got {probe:?}"
        );
        assert_eq!(
            super::resolve_font_entry(&entry).as_deref(),
            Some(probe.as_path()),
            "绝对路径必须原样解析，不得退回平台字体目录表"
        );
    }

    /// 反向断言：裸文件名仍走目录表解析（宿主无 `/system/fonts/` 即解析不到），
    /// 绝对路径分支没有把该语义一并改掉。
    #[test]
    fn bare_file_name_still_resolves_through_font_dirs() {
        assert_eq!(
            super::resolve_font_entry("NoSuchFont-Regular.ttf"),
            None,
            "裸文件名只经 FONT_DIRS 解析，宿主上无此目录即为空"
        );
    }

    /// 绝对路径指向不存在的文件时不得回落目录表：回落会装到同名的系统字体，
    /// 表现为「选中的族和渲染的不是同一个字体」。
    #[test]
    fn missing_absolute_entry_does_not_fall_back_to_font_dirs() {
        let missing = std::path::Path::new("/nonexistent-user-font-dir/font.ttf");
        assert_eq!(
            super::resolve_font_entry(&missing.to_string_lossy()),
            None,
            "绝对路径不存在时必须为空，不得按文件名回落"
        );
    }

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

    /// monospace 族声明多个字体时必须**全部**留下。建库若只装族内首个，该文件一旦
    /// 不可用（OEM 改名、分区迁移、`FONT_DIRS` 未覆盖）就再无任何面可降级，
    /// 直接走 `find_monospace_font` 的 `fatal` —— 真机 SIGABRT 即由此而来。
    #[test]
    fn parse_fonts_xml_monospace_family_keeps_every_declared_font() {
        let xml = FONTS_XML_SNIPPET.replace(
            r#"<family name="monospace">
        <font weight="400" style="normal">DroidSansMono.ttf</font>
    </family>"#,
            r#"<family name="monospace">
        <font weight="400" style="normal">DroidSansMono.ttf</font>
        <font weight="700" style="normal">DroidSansMonoBold.ttf</font>
    </family>"#,
        );
        assert_ne!(xml, FONTS_XML_SNIPPET, "片段替换未命中，测试会恒真");
        assert_eq!(
            super::parse_fonts_xml_families(&xml).0,
            vec!["DroidSansMono.ttf", "DroidSansMonoBold.ttf"]
        );
    }

    /// `fallbackFor` 标记的字体属于**另一条** fallback 链，不得并入本族候选：
    /// zh-Hans 里 `NotoSerifCJK-Regular.ttc` 与正体同名同权重并列，收进来会让
    /// serif 面与正体竞争同一字符（`DESIGN.md:159` 要求中文用 Noto Sans CJK SC
    /// 而非 Noto Serif）。
    #[test]
    fn parse_fonts_xml_skips_fallback_for_fonts() {
        let xml = FONTS_XML_SNIPPET.replace(
            r#"<font weight="400" style="normal" index="2" postScriptName="NotoSansCJKJP-Regular">
            NotoSansCJK-Regular.ttc
        </font>"#,
            r#"<font weight="400" style="normal" index="2" postScriptName="NotoSansCJKJP-Regular">
            NotoSansCJK-Regular.ttc
        </font>
        <font weight="400" style="normal" index="2" fallbackFor="serif"
              postScriptName="NotoSerifCJKJP-Regular">NotoSerifCJK-Regular.ttc
        </font>"#,
        );
        assert_ne!(xml, FONTS_XML_SNIPPET, "片段替换未命中，测试会恒真");
        let (_, lang_fallbacks) = super::parse_fonts_xml_families(&xml);
        let zh_hans = lang_fallbacks
            .iter()
            .find(|(lang, _)| lang == "zh-Hans")
            .unwrap();
        assert!(
            !zh_hans.1.iter().any(|(name, _)| name.contains("Serif")),
            "fallbackFor=serif 的字体泄漏进 zh-Hans 候选: {:?}",
            zh_hans.1
        );
    }

    /// 常规字重排在族内最前：调用方按序取前若干个，AOSP 把 Thin/Light/DemiLight
    /// 排在 Regular 之前，按文档序取会先拿到超细字重而非 spec 要求的常规字重面。
    #[test]
    fn parse_fonts_xml_orders_regular_weight_first() {
        let xml = r#"<?xml version="1.0" encoding="utf-8"?>
<familyset version="23">
    <family lang="zh-Hans">
        <font weight="100" style="normal" index="2">NotoSansCJK-Thin.ttc</font>
        <font weight="400" style="normal" index="2">NotoSansCJK-Regular.ttc</font>
    </family>
</familyset>"#;
        let (_, lang_fallbacks) = super::parse_fonts_xml_families(xml);
        let names: Vec<&str> = lang_fallbacks[0]
            .1
            .iter()
            .map(|(name, _)| name.as_str())
            .collect();
        assert_eq!(
            names,
            vec!["NotoSansCJK-Regular.ttc", "NotoSansCJK-Thin.ttc"]
        );
    }

    /// AOSP 的等宽族名用连字符；只认空格写法会漏掉 `serif-monospace`。
    #[test]
    fn parse_fonts_xml_accepts_hyphenated_monospace_family_names() {
        let xml = r#"<?xml version="1.0" encoding="utf-8"?>
<familyset version="23">
    <family name="serif-monospace">
        <font weight="400" style="normal">CutiveMono.ttf</font>
    </family>
</familyset>"#;
        assert_eq!(
            super::parse_fonts_xml_families(xml).0,
            vec!["CutiveMono.ttf"]
        );
    }

    /// 真机 ZTE P720S20 / Android 13 的 `fonts.xml` 结构：等宽族必须仍以
    /// DroidSansMono 打头（主字体选择据此 stem 匹配，改动顺序即改渲染字形），
    /// 而 zh-Hans 必须以常规字重的 Sans 面打头且不含 `fallbackFor="serif"` 的 Serif 面。
    ///
    /// 片段逐条摘自真机 `fonts.xml`：等宽族只一个 `DroidSansMono.ttf`，`serif-monospace`
    /// 是**独立**的具名族（不是别名），zh-Hans 里 Sans 与 Serif 同名同权重并列、
    /// 且 Thin/Light/DemiLight 排在 Regular 之前。
    ///
    /// 这份结构还决定真机 SIGABRT 会不会发生：真机 14 个族共声明 37 个文件，其中 7 个
    /// 在 `/system/fonts/` 里并不存在（`DancingScript-Regular.ttf` 与 OEM 的
    /// RedMagic/ICN ZDigit 系列）。但等宽族声明非空，所以
    /// `resolve_system_monospace_files` 绝不会走「fonts.xml 不可用」的 fatal；
    /// 历史崩溃来自「声明的文件装不进库就按 fonts.xml 解析失败处理」的旧口径，
    /// 已由 2026-10-01 的降级梯次变更修掉。本用例把「等宽目标非空」这个前置条件钉住。
    #[test]
    fn real_device_fonts_xml_keeps_primary_first_and_drops_serif() {
        const REAL_DEVICE_FONTS_XML: &str = r#"<?xml version="1.0" encoding="utf-8"?>
<familyset version="23">
    <family name="monospace">
        <font weight="400" style="normal">DroidSansMono.ttf</font>
    </family>
    <alias name="sans-serif-monospace" to="monospace" />
    <family name="serif-monospace">
        <font weight="400" style="normal" postScriptName="CutiveMono-Regular">CutiveMono.ttf</font>
    </family>
    <family lang="zh-Hans">
        <font weight="100" style="normal" index="2">NotoSansCJK-Thin.ttc</font>
        <font weight="300" style="normal" index="2">NotoSansCJK-DemiLight.ttc</font>
        <font weight="400" style="normal" index="2">NotoSansCJK-Regular.ttc</font>
        <font weight="400" style="normal" index="2" fallbackFor="serif">NotoSerifCJK-Regular.ttc</font>
    </family>
    <family lang="und-Zsym">
        <font weight="400" style="normal">NotoSansSymbols-Regular-Subsetted2.ttf</font>
    </family>
    <family>
        <font weight="400" style="normal">NotoSansSymbols-Regular-Subsetted.ttf</font>
    </family>
</familyset>"#;
        let (monospace, lang_fallbacks) = super::parse_fonts_xml_families(REAL_DEVICE_FONTS_XML);
        assert_eq!(
            monospace,
            vec!["DroidSansMono.ttf", "CutiveMono.ttf"],
            "主字体必须仍是 DroidSansMono.ttf 打头，否则渲染字形改变"
        );
        let zh_hans = lang_fallbacks
            .iter()
            .find(|(lang, _)| lang == "zh-Hans")
            .expect("真机声明 zh-Hans 族");
        let names: Vec<&str> = zh_hans.1.iter().map(|(name, _)| name.as_str()).collect();
        assert_eq!(
            names[0], "NotoSansCJK-Regular.ttc",
            "常规字重必须排族内首位（真机把 Thin/DemiLight 排在它之前）"
        );
        assert!(
            !names.iter().any(|name| name.contains("Serif")),
            "fallbackFor=\"serif\" 的面泄漏进 zh-Hans 候选: {names:?}"
        );
        assert_eq!(
            super::symbol_family_files(REAL_DEVICE_FONTS_XML),
            vec!["NotoSansSymbols-Regular-Subsetted.ttf"],
            "真机的符号层同样声明在无 name/lang 的族里"
        );
    }

    /// 未声明等宽族的 `fonts.xml` 不产出主字体候选：`resolve_system_monospace_files`
    /// 据此崩溃退出。把该触发条件钉在单测里，任何放宽解析的改动都会立刻暴露。
    #[test]
    fn parse_fonts_xml_without_monospace_family_yields_no_candidate() {
        let xml = FONTS_XML_SNIPPET.replace(
            r#"<family name="monospace">
        <font weight="400" style="normal">DroidSansMono.ttf</font>
    </family>"#,
            "",
        );
        let (monospace, lang_fallbacks) = super::parse_fonts_xml_families(&xml);
        assert!(
            monospace.is_empty(),
            "只含 lang 族与无名族时不得产出等宽候选，否则崩溃退出条件被静默绕过"
        );
        assert_eq!(lang_fallbacks.len(), 2, "其余解析不受影响");
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

    /// 变体判定是 CJK 回退探针与区域字体候选的共同来源：任一侧单独改动都会
    /// 让二者对同一 locale 得出不同结论（`zh-SG` 曾被判为非 CJK 而取不到候选）。
    #[test]
    fn locale_cjk_variant_matches_aosp_locale_shapes() {
        assert_eq!(super::locale_cjk_variant("zh-CN"), Some("sc"));
        assert_eq!(super::locale_cjk_variant("zh-Hans-CN"), Some("sc"));
        assert_eq!(super::locale_cjk_variant("zh-SG"), Some("sc"));
        assert_eq!(super::locale_cjk_variant("zh-TW"), Some("tc"));
        assert_eq!(super::locale_cjk_variant("zh-Hant-HK"), Some("tc"));
        assert_eq!(super::locale_cjk_variant("ja-JP"), Some("jp"));
        assert_eq!(super::locale_cjk_variant("ko-KR"), Some("kr"));
        for locale in ["", "en-US", "und", "ZH-CN"] {
            assert_eq!(super::locale_cjk_variant(locale), None, "locale={locale}");
        }
    }

    /// `zh-SG` 等其余简中子标签曾取不到区域候选：变体判定与候选表此前各写一份。
    #[test]
    fn locale_fonts_xml_langs_follows_cjk_variant() {
        assert_eq!(
            super::locale_fonts_xml_langs("zh-SG"),
            &["zh-Hans", "zh-CN", "zh", "und-Hani"]
        );
        assert!(super::locale_fonts_xml_langs("").is_empty());
    }

    /// 宿主字体库与其中首个等宽面、首个比例面：主字体选择梯次测试的探针。
    /// 依赖 dev shell 的字体（同 `probe_font_file`），缺字体时自然失败。
    fn host_faces() -> (fontdb::Database, fontdb::ID, fontdb::ID) {
        let mut font_database = fontdb::Database::new();
        font_database.load_system_fonts();
        let monospace = font_database
            .faces()
            .find(|face| face.monospaced)
            .map(|face| face.id)
            .expect("宿主字体库须有等宽面（run inside nix develop）");
        let proportional = font_database
            .faces()
            .find(|face| !face.monospaced)
            .map(|face| face.id)
            .expect("宿主字体库须有比例面（run inside nix develop）");
        (font_database, monospace, proportional)
    }

    /// 梯次第 1 级：声明词干命中时必须选回声明族（去空白比较覆盖
    /// DroidSansMono.ttf 与 Droid Sans Mono 的差异），而不是其它等宽面。
    #[test]
    fn select_primary_face_prefers_declared_stem_match() {
        let (font_database, monospace, _) = host_faces();
        let family = font_database
            .face(monospace)
            .and_then(|face| face.families.first())
            .map(|(name, _)| name.clone())
            .expect("等宽面须有族名");
        // 由族名构造必然命中「去空白」比较的文件名。
        let target = format!(
            "{}.ttf",
            family
                .chars()
                .filter(|character| !character.is_whitespace())
                .collect::<String>()
        );
        let selected = super::select_primary_face(&font_database, &target).expect("须选中面");
        let selected_face = font_database.face(selected).expect("选中面须存在");
        assert!(selected_face.monospaced, "词干匹配必须落在等宽面上");
        assert_eq!(
            selected_face.families.first().map(|(name, _)| name),
            Some(&family),
            "词干命中必须选回声明族，不得退到其它等宽面"
        );
    }

    /// 梯次第 2 级：词干不匹配时退到库内任一等宽面（OEM 改包装/改名），不崩溃。
    #[test]
    fn select_primary_face_falls_back_to_any_monospaced_face() {
        let (font_database, _, _) = host_faces();
        let selected = super::select_primary_face(&font_database, "NoSuchDeclaredFont.ttf")
            .expect("库内有等宽面时须选中");
        assert!(
            font_database
                .face(selected)
                .expect("选中面须存在")
                .monospaced,
            "词干不匹配时必须退到库内等宽面"
        );
    }

    /// 梯次第 3 级：全库无等宽面时退到首个可用面——终端降级可用胜过启动即崩溃。
    #[test]
    fn select_primary_face_degrades_to_first_face_without_monospaced() {
        let (host_database, _, proportional) = host_faces();
        let source_path = match &host_database
            .face(proportional)
            .expect("比例面须存在")
            .source
        {
            fontdb::Source::File(path) | fontdb::Source::SharedFile(path, _) => path.clone(),
            fontdb::Source::Binary(_) => panic!("探针面须来自字体文件"),
        };
        let mut font_database = fontdb::Database::new();
        font_database
            .load_font_file(&source_path)
            .expect("探针字体须可装入");
        assert!(
            !super::db_has_monospaced(&font_database),
            "探针文件须只含比例面，否则测不到首面降级"
        );
        let expected = font_database.faces().next().map(|face| face.id);
        assert_eq!(
            super::select_primary_face(&font_database, "NoSuchDeclaredFont.ttf"),
            expected,
            "无等宽面时必须退到库内首个可用面"
        );
    }

    /// 梯次第 4 级：库为空返回 None，由调用方按「fonts.xml 不可用」abort。
    #[test]
    fn select_primary_face_returns_none_for_empty_database() {
        let font_database = fontdb::Database::new();
        assert_eq!(
            super::select_primary_face(&font_database, "DroidSansMono.ttf"),
            None,
            "库为空必须返回 None"
        );
    }

    /// 放宽装库的触发条件：库内是否存在等宽面。
    #[test]
    fn db_has_monospaced_reflects_face_flags() {
        assert!(
            !super::db_has_monospaced(&fontdb::Database::new()),
            "空库必无等宽面"
        );
        let (font_database, _, _) = host_faces();
        assert!(
            super::db_has_monospaced(&font_database),
            "宿主库含等宽面时必须为真"
        );
    }

    /// 拉丁探测：宿主文本字体（等宽与比例）都必须覆盖 'm'；梯次的第 2/3 级
    /// 依赖它跳过 emoji 这类被标记为等宽却无拉丁字形的面。
    #[test]
    fn face_covers_latin_for_host_text_faces() {
        let (font_database, monospace, proportional) = host_faces();
        assert!(
            super::face_covers_latin(&font_database, monospace),
            "等宽文本面必须覆盖拉丁"
        );
        assert!(
            super::face_covers_latin(&font_database, proportional),
            "比例文本面必须覆盖拉丁"
        );
    }
}
