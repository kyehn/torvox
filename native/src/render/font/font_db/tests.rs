
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
        .find_map(|face| {
            super::face_source_path(face)
                .filter(|path| path.is_file())
                .map(|path| path.to_path_buf())
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
    let families = super::parse_fonts_xml_document(&probe_fonts_xml(&filename)).families;

    let mut font_database = fontdb::Database::new();
    assert_eq!(
        super::missing_region_fallback_faces(&font_database, &families, "zh-CN"),
        vec![(filename.clone(), 2)],
        "库在 locale 之前定型时区域族必须报为缺失，否则 zh-CN 用户无 CJK 可用"
    );

    font_database
        .load_font_file(&probe)
        .expect("探针字体须可装入");
    assert!(
        super::missing_region_fallback_faces(&font_database, &families, "zh-CN").is_empty(),
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
    let families = super::parse_fonts_xml_document(&format!(
        r#"<familyset version="23"><family lang="zh-Hans">{weights}</family></familyset>"#
    ))
    .families;
    let font_database = fontdb::Database::new();
    assert_eq!(
        super::missing_region_fallback_faces(&font_database, &families, "zh-CN"),
        vec![(filename.to_string(), 2)],
        "同一 TTC 的九个字重声明只补一个文件"
    );
}

/// 非 CJK locale 不补装任何 `lang` 族：否则 en-US 设备会被塞入整本 CJK 字体。
#[test]
fn non_cjk_locale_requests_no_region_fallback() {
    let font_database = fontdb::Database::new();
    let families =
        super::parse_fonts_xml_document(&probe_fonts_xml("NotoSansCJK-Regular.ttc")).families;
    assert!(
        super::missing_region_fallback_faces(&font_database, &families, "en-US").is_empty(),
        "非 CJK locale 不得补装区域回退族"
    );
    assert!(
        super::missing_region_fallback_faces(&font_database, &families, "ja").is_empty(),
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
        super::parse_fonts_xml_document(FONTS_XML_SNIPPET).symbol_files,
        vec!["NotoSansSymbols-Regular-Subsetted.ttf"]
    );
    assert!(
        super::parse_fonts_xml_document("<familyset></familyset>")
            .symbol_files
            .is_empty()
    );
    assert!(
        super::parse_fonts_xml_document("not xml")
            .symbol_files
            .is_empty()
    );
}

/// 区域族按当前语言取 `lang` 匹配项；语言不匹配时为空（不猜）。
#[test]
fn locale_family_follows_system_language() {
    let families = super::parse_fonts_xml_document(FONTS_XML_SNIPPET).families;
    assert_eq!(
        super::locale_fallback_files(&families, "zh-CN"),
        vec!["NotoSansCJK-Regular.ttc"]
    );
    assert_eq!(
        super::locale_fallback_files(&families, "ja"),
        vec!["NotoSansCJK-Regular.ttc"]
    );
    assert!(
        super::locale_fallback_files(&families, "en-US").is_empty(),
        "非 CJK 语言不得回退到 CJK 族"
    );
}

#[test]
fn parse_fonts_xml_monospace_and_lang_blocks() {
    let families = super::parse_fonts_xml_document(FONTS_XML_SNIPPET).families;
    assert_eq!(families.monospace, vec!["DroidSansMono.ttf"]);
    let lang_fallbacks = &families.lang_fallbacks;
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

/// 全部视图同源于一次解析：族结构、符号层、声明文件集与别名表必须互相自洽。
/// 各写一份遍历时任何一处增删过滤条件都会静默分叉，而分叉只体现在真机字体
/// 缺失上——声明文件集少一个族，放宽装库就少装一个文件。
#[test]
fn fonts_xml_document_views_are_self_consistent() {
    let document = super::parse_fonts_xml_document(FONTS_XML_SNIPPET);
    assert_eq!(document.families.monospace, vec!["DroidSansMono.ttf"]);
    assert_eq!(
        document.symbol_files,
        vec!["NotoSansSymbols-Regular-Subsetted.ttf"]
    );
    assert_eq!(
        document.aliases,
        vec![
            (
                "monospace".to_string(),
                vec!["DroidSansMono.ttf".to_string()]
            ),
            ("casual".to_string(), vec!["ComingSoon.ttf".to_string()]),
        ]
    );
    assert_eq!(
        document.declared_files,
        vec![
            "DroidSansMono.ttf",
            "ComingSoon.ttf",
            "NotoSansCJK-Regular.ttc",
            "NotoSansCJK-Regular.ttc",
            "NotoSansSymbols-Regular-Subsetted.ttf",
        ]
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
        super::parse_fonts_xml_document(&xml).families.monospace,
        vec!["DroidSansMono.ttf", "DroidSansMonoBold.ttf"]
    );
}

/// `fallbackFor` 标记的字体属于**另一条** fallback 链，不得并入本族候选：
/// zh-Hans 里 `NotoSerifCJK-Regular.ttc` 与正体同名同权重并列，收进来会让
/// `cjk-rendering` 规格要求中文用 Noto Sans CJK SC（index=2）、
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
    let document = super::parse_fonts_xml_document(&xml);
    let zh_hans = document
        .families
        .lang_fallbacks
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
    let document = super::parse_fonts_xml_document(xml);
    let names: Vec<&str> = document.families.lang_fallbacks[0]
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
        super::parse_fonts_xml_document(xml).families.monospace,
        vec!["CutiveMono.ttf"]
    );
}

/// 真机 测试设备 / Android 13 的 `fonts.xml` 结构：等宽族必须仍以
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
    let document = super::parse_fonts_xml_document(REAL_DEVICE_FONTS_XML);
    assert_eq!(
        document.families.monospace,
        vec!["DroidSansMono.ttf", "CutiveMono.ttf"],
        "主字体必须仍是 DroidSansMono.ttf 打头，否则渲染字形改变"
    );
    let zh_hans = document
        .families
        .lang_fallbacks
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
        document.symbol_files,
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
    let document = super::parse_fonts_xml_document(&xml);
    assert!(
        document.families.monospace.is_empty(),
        "只含 lang 族与无名族时不得产出等宽候选，否则崩溃退出条件被静默绕过"
    );
    assert_eq!(
        document.families.lang_fallbacks.len(),
        2,
        "其余解析不受影响"
    );
}

#[test]
fn parse_fonts_xml_rejects_garbage() {
    assert_eq!(
        super::parse_fonts_xml_document("not xml at all").families,
        super::FontsXmlFamilies::default()
    );
    assert_eq!(
        super::parse_fonts_xml_document("<html></html>").families,
        super::FontsXmlFamilies::default()
    );
    assert_eq!(
        super::parse_fonts_xml_document("").families,
        super::FontsXmlFamilies::default()
    );
}

#[test]
fn fonts_xml_declared_files_include_nameless_family() {
    // emulator（API 35）的符号字体声明在既无 name 也无 lang 的族里，
    // 按 name/lang 过滤会漏掉，终端将失去 ▶ ⏵ ♥ ★。
    let xml = r#"<familyset>
            <family name="monospace"><font>DroidSansMono.ttf</font></family>
            <family lang="zh-Hans"><font index="2">NotoSansCJK-Regular.ttc</font></family>
            <family><font>NotoSansSymbols-Regular-Subsetted.ttf</font></family>
        </familyset>"#;
    assert_eq!(
        super::parse_fonts_xml_document(xml).declared_files,
        vec![
            "DroidSansMono.ttf",
            "NotoSansCJK-Regular.ttc",
            "NotoSansSymbols-Regular-Subsetted.ttf",
        ]
    );
}

#[test]
fn fonts_xml_declared_files_reject_garbage() {
    assert!(
        super::parse_fonts_xml_document("not xml at all")
            .declared_files
            .is_empty()
    );
    assert!(
        super::parse_fonts_xml_document("")
            .declared_files
            .is_empty()
    );
    assert!(
        super::parse_fonts_xml_document("<familyset><family></family></familyset>")
            .declared_files
            .is_empty()
    );
}

#[test]
fn fonts_xml_aliases_in_document_order() {
    let aliases = super::parse_fonts_xml_document(FONTS_XML_SNIPPET).aliases;
    assert_eq!(aliases.len(), 2);
    assert_eq!(aliases[0].0, "monospace");
    assert_eq!(aliases[0].1, vec!["DroidSansMono.ttf"]);
    assert_eq!(aliases[1].0, "casual");
    assert_eq!(aliases[1].1, vec!["ComingSoon.ttf"]);
}

#[test]
fn fonts_xml_aliases_skip_nameless_and_reject_garbage() {
    let xml = FONTS_XML_SNIPPET.replace("<family name=\"casual\">", "<family>");
    let aliases = super::parse_fonts_xml_document(&xml).aliases;
    assert_eq!(aliases.len(), 1);
    assert_eq!(aliases[0].0, "monospace");
    assert!(
        super::parse_fonts_xml_document("not xml at all")
            .aliases
            .is_empty()
    );
    assert!(super::parse_fonts_xml_document("").aliases.is_empty());
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

/// 覆盖探测字符在非 CJK locale 下退到通用汉字，分片 locale 仍取分片字符。
///
/// 它（而不是 `cjk_probe_char`）才是「主字体是否覆盖 CJK」的判据：非 CJK
/// locale 下返回 `None` 会被下游当成「主字体不支持 CJK」，使自带汉字的字体
/// 在设置页被报成缺回退层。
#[test]
fn cjk_coverage_probe_char_falls_back_to_han_for_non_cjk_locales() {
    assert_eq!(super::cjk_coverage_probe_char("zh-CN"), '中');
    assert_eq!(super::cjk_coverage_probe_char("ja-JP"), '中');
    assert_eq!(super::cjk_coverage_probe_char("ko-KR"), '가');
    for locale in ["", "en-US", "de-DE", "und"] {
        assert_eq!(
            super::cjk_coverage_probe_char(locale),
            '中',
            "locale={locale}"
        );
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
    let source_path =
        super::face_source_path(host_database.face(proportional).expect("比例面须存在"))
            .expect("探针面须来自字体文件")
            .to_path_buf();
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
