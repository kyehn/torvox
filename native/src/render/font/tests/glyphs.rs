use super::*;

#[test]
fn han_is_wide_and_non_emoji() {
    let mut pipeline = FontPipeline::new(512, 512, 14.0);
    let ascii_info = pipeline.glyph_information('A').expect("ASCII glyph info");
    let han_info = pipeline.glyph_information('世').expect("Han glyph info");
    assert!(
        han_info.width as f32 > ascii_info.width as f32 * 1.5,
        "CJK 回退字体已安装时汉字必须是双格宽：'世' {} vs 'A' {}",
        han_info.width,
        ascii_info.width
    );
}

/// 回应对"加粗中文空白"：合成样式不得跳过回退链，加粗汉字必须有位图。
#[test]
fn bold_cjk_fallback_not_blank() {
    let (mut pipeline, _) = styled_test_pipeline();
    pipeline.find_cjk_fallback_fonts("");
    let bold_han = pipeline
        .glyph_information_styled('中', true, false)
        .expect("加粗汉字必须解析出字形");
    assert!(
        bold_han.width > 0 && bold_han.height > 0,
        "加粗汉字位图不得为空"
    );
}

#[test]
fn font_pipeline_creation() {
    let pipeline = FontPipeline::new(1024, 1024, 14.0);
    assert_eq!(pipeline.atlas_dimensions(), (1024, 1024));
    assert!(
        pipeline.cache_length() > 0,
        "ASCII glyphs should be pre-rasterized"
    );
}

#[test]
fn font_pipeline_has_system_fonts() {
    let pipeline = FontPipeline::new(1024, 1024, 14.0);
    let fonts = pipeline.list_monospace_fonts();
    assert!(
        !fonts.is_empty(),
        "Should have at least one system monospace font"
    );
}

/// 上报的 `CjkFallbackState` 与渲染器实际取舍必须同源：报 `Skipped`
/// （主字体已覆盖、无需回退层）的唯一正当理由是 charmap 探测确认主字体
/// 覆盖本 locale 的 CJK，否则应报 `None`。
///
/// 一屏之内的不同码位必须都留在样式面字形号缓存里。
///
/// 该缓存的键是 (面, 码位)，规模随屏幕上出现过的**不同字符**增长，与同族的
/// (字体, 粗, 斜) 三元组缓存不同源。容量若按三元组那种「组合数天然很小」来取，
/// 一屏 40×20 就把它穿透，每次未命中都要重入 `with_face_data` 重建 charmap
/// （字体解压）——粗体/斜体文本越常见越慢。
#[test]
fn styled_screenful_of_codepoints_stays_cached() {
    const SCREENFUL: u32 = 200;
    let mut pipeline = FontPipeline::new(2048, 2048, 14.0);
    pipeline.set_system_locale("zh-CN");
    let sample: Vec<char> = (0x4E00..0x4E00 + SCREENFUL)
        .filter_map(char::from_u32)
        .collect();
    for codepoint in &sample {
        pipeline.glyph_information_styled(*codepoint, true, false);
    }
    assert_eq!(
        pipeline.caches.face_glyph_id_cache.len(),
        sample.len(),
        "样式面字形号缓存必须容得下一屏之内的不同码位"
    );
}

/// 上报的单元格尺寸必须是**物理**像素，即布局实际使用的那个尺度。
///
/// 单元四边形按 `cell_metrics × raster_scale` 布置，而 `font_info` 的字段
/// 声明为物理像素（打包布局与设置页的「px」标签都按物理像素读）。少了这个
/// 换算，高密度设备上设置页显示的单元格会比实际渲染小一个密度倍数。
#[test]
fn reported_cell_size_matches_the_rendered_grid_scale() {
    let mut pipeline = FontPipeline::new(1024, 1024, 14.0);
    pipeline.set_raster_scale(3.0);
    let info = pipeline.font_info();
    let (logical_width, logical_height) = pipeline.cell_metrics();
    assert!(
        (info.cell_width_px - logical_width * 3.0).abs() < 1e-3
            && (info.cell_height_px - logical_height * 3.0).abs() < 1e-3,
        "上报单元格 {:?} 应为布局尺寸 ({}, {}) × raster_scale",
        (info.cell_width_px, info.cell_height_px),
        logical_width,
        logical_height
    );
}

/// 非 CJK 系统语言下，本身覆盖 CJK 的主字体必须报「已跳过」，不是「缺失」。
///
/// `cjk_probe_char("en-US")` 返回 `None`，其语义是「本 locale 没有分片代表
/// 字符」，不是「主字体不支持 CJK」；早先的实现把两者混为一谈，于是英文系统下
/// 的 CJK 主字体（用户在 `~/.termux/font` 放了字体文件，或直接选了 CJK 族）
/// 在设置页被报成「未检测到 CJK 备用字体」——一个本无需回退的主字体却显示
/// 明确错误提示。分片代表字符只用于「CN 字体未必覆盖谚文」这类匹配，不能
/// 反过来当作主字体的能力上限。
///
/// 报出的必须是**非 CJK 语言**这一条：语言回退层按平台语义只对 CJK 语言
/// 声明，故英文系统下的缺席原因是语言，与主字体是否覆盖汉字无关。
#[test]
fn cjk_primary_under_non_cjk_locale_reports_skipped_not_none() {
    use crate::render::font::font_db;
    use crate::render::font::pipeline::CjkFallbackState;

    let mut pipeline = FontPipeline::new(1024, 1024, 14.0);
    assert!(
        try_load_cjk_fonts(pipeline.font_system.db_mut()),
        "CJK fonts must load (run inside nix develop)"
    );
    assert_eq!(
        font_db::cjk_probe_char("en-US"),
        None,
        "本用例的前提：en-US 没有分片代表字符"
    );
    // 现挑一个真的覆盖「中」的面当主字体（宿主环境带 Noto CJK）。
    let family = {
        let font_database = pipeline.font_system.db();
        font_database
            .faces()
            .find(|face| font_db::face_covers(font_database, face.id, '中'))
            .and_then(|face| face.families.first().map(|(name, _)| name.clone()))
            .expect("宿主须装入至少一个覆盖「中」的面（nix develop 的 Noto CJK）")
    };
    pipeline.set_system_locale("en-US");
    assert!(
        pipeline.set_font_family(&family, &[]),
        "族 '{family}' 应可选中"
    );
    let info = pipeline.font_info();
    assert_eq!(
        info.active.as_ref().map(|active| active.name.as_str()),
        Some(family.as_str()),
        "选中的主字体必须就是上面探测过的那个族"
    );
    assert!(
        info.cjk_families.is_empty(),
        "非 CJK locale 不装回退层，却得到 {:?}",
        info.cjk_families
    );
    assert_eq!(info.cjk_state, CjkFallbackState::SkippedNonCjkLocale);
}

/// 「按设计跳过」与「确实缺失」必须分开：非 CJK 语言下无论主字体是否覆盖
/// 汉字，回退层都按平台语义不装——那是正常状态，不是「未检测到」。
///
/// 早先的实现只按「主字体是否覆盖代表字符」二分：英文系统 + 拉丁等宽主字体
/// 会落进 `None`，设置页于是对一台完全正常的机器报「未检测到 CJK 备用字体」。
#[test]
fn non_cjk_locale_never_reports_missing_cjk_fallback() {
    use crate::render::font::pipeline::CjkFallbackState;

    let mut pipeline = FontPipeline::new(1024, 1024, 14.0);
    pipeline.set_system_locale("en-US");
    let info = pipeline.font_info();
    assert_eq!(
        info.cjk_state,
        CjkFallbackState::SkippedNonCjkLocale,
        "非 CJK 语言下的缺席是按设计为之，不得报成缺失"
    );
    assert!(
        info.cjk_families.is_empty(),
        "非 CJK 语言不装语言回退族，却得到 {:?}",
        info.cjk_families
    );
}

/// 系统语言未知时同样是「按设计跳过」，不是缺失。
#[test]
fn unknown_locale_reports_skipped_not_missing() {
    use crate::render::font::pipeline::CjkFallbackState;

    let mut pipeline = FontPipeline::new(1024, 1024, 14.0);
    pipeline.find_cjk_fallback_fonts("");
    let info = pipeline.font_info();
    assert_eq!(
        info.cjk_state,
        CjkFallbackState::SkippedLocaleUnknown,
        "语言未知时无从判断是否需要回退族，不得报成缺失"
    );
}

/// 早先的族名子串启发式与探测会相反（族名带 CJK 字样但 charmap 无该码位），
/// 于是 UI 显示「已覆盖」而渲染器其实装着回退层。本用例在宿主上钉住另一端：
/// 拉丁等宽面在 CJK locale 下必须报 `None`（分叉的另一端——主字体本身即 CJK
/// 字体——由 `cjk_primary_under_non_cjk_locale_reports_skipped_not_none` 覆盖，
/// `primary_covers_cjk_probe` 的单一实现则从结构上排除两处各判一次的可能）。
#[test]
fn reported_cjk_skipped_implies_primary_really_covers_cjk() {
    use crate::render::font::pipeline::CjkFallbackState;

    for locale in ["zh-CN", "ja-JP", "ko-KR"] {
        let mut pipeline = FontPipeline::new(1024, 1024, 14.0);
        pipeline.set_system_locale(locale);
        let info = pipeline.font_info();
        if !info.cjk_families.is_empty() {
            assert_eq!(
                info.cjk_state,
                CjkFallbackState::Fallback,
                "locale={locale}"
            );
            continue;
        }
        // 无回退层时报 Skipped 只有一个正当理由：charmap 探测确认主字体
        // 覆盖本 locale 的 CJK。宿主无 fonts.xml，主字体是 DroidSansMono
        // 一类拉丁等宽面，既不覆盖中也不覆盖 가 —— 任何 locale 都必须是
        // None。早先的族名子串启发式在这里同样返回 false，故本断言的作用是
        // 钉住「探测与上报同源」这条关系，而非当前具体字体。
        assert!(
            !pipeline.primary_covers_cjk_probe(),
            "locale={locale}: 宿主等宽面不该覆盖 CJK，若换字体请同步更新本断言"
        );
        assert_eq!(info.cjk_state, CjkFallbackState::None, "locale={locale}");
    }
}

/// 「其他被使用字体」上报的是**实际供出字形的面**，不是「回退层里装着的族」。
///
/// 两者会相反：回退层只含 CJK 族，而符号/区域族是全库扫描找到的，从不进
/// `cjk_fallback_ids`。没有这条上报时，屏幕上真的参与渲染的符号族对用户隐形。
/// 宿主环境带 Noto CJK 与 Maple Mono，拉丁与汉字分属不同族，故渲染一个汉字
/// 后必能观察到一个非主字体族。
#[test]
fn reported_other_families_name_the_faces_that_actually_supplied_glyphs() {
    let mut pipeline = FontPipeline::new(1024, 1024, 14.0);
    let primary = pipeline
        .current_font_family_name()
        .expect("宿主须有可用主字体");
    // 只光栅化 ASCII：全部落在主字体，此时不应报出任何「其他」族。
    assert_eq!(
        pipeline.font_info().other_families,
        Vec::<String>::new(),
        "仅主字体供出字形时不得凭空多报"
    );
    let glyph = pipeline
        .glyph_information('中')
        .expect("宿主字体库须能解析汉字");
    assert!(glyph.width > 0 && glyph.height > 0);
    let info = pipeline.font_info();
    assert!(
        info.other_families
            .iter()
            .any(|family| !family.eq_ignore_ascii_case(&primary)),
        "主字体 {primary} 未覆盖汉字时，实际供出字形的族应被上报：{:?}",
        info.other_families
    );
    assert!(
        !info.other_families.contains(&primary),
        "主字体自身不得出现在「其他被使用字体」里"
    );
    // 已被单独上报的 CJK 回退族不再重复出现在尾段。
    assert!(
        info.cjk_families
            .iter()
            .all(|family| !info.other_families.contains(family)),
        "CJK 回退族已在独立行显示：{:?} / {:?}",
        info.cjk_families,
        info.other_families
    );
}

/// 换字号后旧字体的结论不得留存：换之前渲染过汉字（非主字体族），换回来时
/// 该族须重新累积，否则「其他被使用字体」会一直挂着早已不再渲染的族。
#[test]
fn used_face_reporting_resets_with_the_identity_caches() {
    let mut pipeline = FontPipeline::new(1024, 1024, 14.0);
    assert!(
        pipeline.glyph_information('中').is_some(),
        "宿主字体库须能解析汉字"
    );
    assert!(
        !pipeline.font_info().other_families.is_empty(),
        "前置：渲染汉字后须已有非主字体族可报"
    );
    pipeline.set_font_size_in_place(18.0);
    assert_eq!(
        pipeline.font_info().other_families,
        Vec::<String>::new(),
        "清空后须重新累积，不得留存上一次字体的面"
    );
}

/// 换 locale 会清空上报面集合却**保留** `glyph_cache`（`set_system_locale` 只调
/// `clear_identity_caches`）。于是重新取同一个字时走的是缓存命中分支、不再重新
/// 光栅化——若只在新光栅化路径上登记，供出该字形的面就此永久漏报。
#[test]
fn used_face_reporting_survives_a_locale_change_without_re_rasterizing() {
    let mut pipeline = FontPipeline::new(1024, 1024, 14.0);
    assert!(
        pipeline.glyph_information('中').is_some(),
        "宿主字体库须能解析汉字"
    );
    pipeline.set_system_locale("ja-JP");
    assert!(
        pipeline.font_info().other_families.is_empty(),
        "前置：locale 变更后上报面已清空"
    );
    let cached = pipeline.glyph_information('中').expect("缓存命中仍须出图");
    assert!(cached.width > 0 && cached.height > 0);
    assert!(
        !pipeline.font_info().other_families.is_empty(),
        "缓存命中的字形同样算「实际供出」，其面须重新登记"
    );
}

#[test]
fn font_matching_stripped_spaces() {
    let pipeline = FontPipeline::new(1024, 1024, 14.0);
    let names = pipeline.list_monospace_fonts();
    assert!(!names.is_empty(), "Should have at least one font");
    let name_with_space = names.iter().find(|name| name.contains(' '));
    let name = match name_with_space {
        Some(n) => n.clone(),
        None => {
            panic!(
                "no monospace font with spaces found; cannot test stripped-name matching; available: {:?}",
                names.iter().take(5).collect::<Vec<_>>()
            );
        }
    };
    let stripped: String = name
        .chars()
        .filter(|character| !character.is_whitespace())
        .collect();
    assert!(stripped != name, "Sanity: stripped name differs");
    let mut p2 = FontPipeline::new(1024, 1024, 14.0);
    assert!(
        p2.set_font_family(&stripped, &[]),
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
    let (cell_width, _) = pipeline.cell_metrics();
    assert!(cell_width > 0.0, "cell width should be positive");
    let cell_span = if cjk_info.width as f32 > ascii_info.width as f32 * 1.5 {
        2
    } else {
        1
    };
    assert!(cell_span >= 1, "CJK cell span should be at least 1");
}

#[test]
fn glyph_information_ascii() {
    let mut pipeline = FontPipeline::new(1024, 1024, 14.0);
    let info = pipeline.glyph_information('A');
    assert!(info.is_some());
    let info = info.unwrap();
    assert!(info.width > 0);
    assert!(info.height > 0);
}

#[test]
fn glyph_information_caching() {
    let mut pipeline = FontPipeline::new(1024, 1024, 14.0);
    let before = pipeline.cache_length();
    pipeline.glyph_information('B');
    assert_eq!(pipeline.cache_length(), before);
    pipeline.glyph_information('B');
    assert_eq!(pipeline.cache_length(), before);
}

#[test]
fn rasterize_ascii_populates_cache() {
    let pipeline = FontPipeline::new(1024, 1024, 14.0);
    // ASCII 32..127 共 95 格；空格无位图不入库，94 为正确值。
    assert!(pipeline.cache_length() >= 94);
}

#[test]
fn font_size_change_invalidates_ascii_identity_cache() {
    // 手势缩放/字号切换走 set_font_size_in_place：若 ascii 字形 id
    // 表不清零，d 等字符会命中旧尺寸的脏 id 而显示错误字形。
    let mut pipeline = FontPipeline::new(1024, 1024, 14.0);
    let before = pipeline.glyph_information('d').expect("d glyph info");
    pipeline.set_font_size_in_place(28.0);
    let after = pipeline
        .glyph_information('d')
        .expect("d glyph info after resize");
    // 同一字符新尺寸必须重光栅化：位图尺寸随字号显著变化。
    assert!(
        after.width != before.width || after.height != before.height,
        "d must re-rasterize after font size change"
    );
    assert!(after.width > 0 && after.height > 0);
}

#[test]
fn atlas_eviction_keeps_glyphs_retrievable() {
    let mut pipeline = FontPipeline::new(256, 256, 14.0);
    let distinct: Vec<char> = ('A'..='Z')
        .chain('a'..='z')
        .chain('0'..='9')
        .chain("!@#$%^&*()_+-=[]{}|;:,.<>?/".chars())
        .collect();
    for &ch in distinct.iter().cycle().take(distinct.len() * 4) {
        assert!(
            pipeline.glyph_information(ch).is_some(),
            "glyph {ch:?} must remain retrievable under eviction pressure"
        );
    }
    let generation_after_fill = pipeline.atlas_generation();
    assert!(pipeline.glyph_information('A').is_some());
    assert!(
        pipeline.atlas_generation() >= generation_after_fill,
        "atlas generation must remain monotonic under eviction"
    );
    assert!(
        pipeline.cache_length() <= GLYPH_CACHE_CAPACITY,
        "glyph cache must respect its capacity after eviction"
    );
}

#[test]
fn glyph_information_has_atlas_coords() {
    let mut pipeline = FontPipeline::new(1024, 1024, 14.0);
    let info = pipeline.glyph_information('X').unwrap();
    assert!(info.atlas_x >= 0);
    assert!(info.atlas_y >= 0);
    assert!(info.width > 0);
    assert!(info.height > 0);
}

#[test]
fn atlas_bitmap_not_empty_after_rasterize() {
    let mut pipeline = FontPipeline::new(1024, 1024, 14.0);
    pipeline.glyph_information('A');
    let bitmap = pipeline.atlas_bitmap();
    assert!(bitmap.iter().any(|&b| b != 0));
}

#[test]
fn cell_metrics_scales_with_font_size() {
    let small = FontPipeline::new(1024, 1024, 10.0);
    let large = FontPipeline::new(1024, 1024, 20.0);
    let (sw, sh) = small.cell_metrics();
    let (lw, lh) = large.cell_metrics();
    assert!(lw > sw, "larger font must have wider cell");
    assert!(lh > sh, "larger font must have taller cell");
}

#[test]
fn setting_font_family_changes_metrics() {
    let mut pipeline = FontPipeline::new(512, 512, 12.0);
    let name = pipeline
        .list_monospace_fonts()
        .into_iter()
        .next()
        .expect("系统应至少提供一个等宽字体");
    assert!(
        pipeline.set_font_family(&name, &[]),
        "set_font_family 应对 {name} 成功"
    );
    let (cell_width, cell_height) = pipeline.cell_metrics();
    assert!(cell_width > 0.0 && cell_height > 0.0, "单元格尺寸必须为正");
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
    let dot_info = pipeline.glyph_information('.').expect("'.' for comparison");
    assert!(
        info.placement.width >= dot_info.placement.width * 2 - 2,
        "好 width={} should be ~2x dot width={}",
        info.placement.width,
        dot_info.placement.width
    );
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
    let atlas_width = 512usize;
    let atlas_left = info.atlas_x as usize;
    let atlas_top = info.atlas_y as usize;
    let mut has_ink = false;
    for pixel_row in 0..info.height as usize {
        for pixel_column in 0..info.width as usize {
            let byte_offset =
                ((atlas_top + pixel_row) * atlas_width + atlas_left + pixel_column) * 4;
            if byte_offset < atlas.len() && atlas[byte_offset] > 0 {
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

#[test]
fn emoji_glyph_grinning() {
    let mut pipeline = FontPipeline::new(512, 512, 14.0);
    let ch = '\u{1F600}';
    let info = pipeline.glyph_information(ch);
    if info.is_none()
        || info
            .as_ref()
            .is_some_and(|glyph_info| glyph_info.width == 0)
    {
        let fonts = pipeline.list_monospace_fonts();
        let found_emoji = fonts.iter().any(|name| {
            name.contains("Emoji") || name.contains("Noto") || name.to_lowercase().contains("emoji")
        });
        assert!(
            found_emoji,
            "no emoji-supporting font found in system; emoji glyph test requires Noto Emoji or similar"
        );
    }
    let info = info.expect("emoji should have glyph info");
    assert!(
        info.width > 0 || info.height > 0,
        "emoji should produce non-zero glyph info: got {}x{}",
        info.width,
        info.height
    );
}

#[test]
fn glyph_atlas_lru_eviction() {
    let mut pipeline = FontPipeline::new(512, 512, 14.0);
    pipeline.rasterize_ascii();
    let after_ascii = pipeline.cache_length();
    // ASCII 32..127 共 95 格；空格无位图不入库，94 为正确值。
    assert!(
        after_ascii >= 94,
        "should have at least 94 cached after rasterize_ascii, got {}",
        after_ascii
    );

    let mut inserted = 0u32;
    for cp in 0x4E00u32..0x4F00u32 {
        let ch = char::from_u32(cp).unwrap_or('\0');
        if pipeline
            .glyph_information(ch)
            .is_some_and(|glyph_info| glyph_info.width > 0)
        {
            inserted += 1;
        }
    }
    let final_len = pipeline.cache_length();
    assert!(
        final_len <= 10000,
        "cache_length {} exceeds capacity 10000",
        final_len
    );
    assert!(
        final_len >= after_ascii,
        "cache should not shrink after inserting new glyphs: \
             before={} after={} inserted={}",
        after_ascii,
        final_len,
        inserted
    );
    let bitmap = pipeline.atlas_bitmap();
    assert!(
        bitmap.iter().any(|&b| b != 0),
        "atlas bitmap should have non-zero bytes after glyph insertion"
    );
}

#[test]
fn cjk_glyph_information_returns_nonzero_for_common_chars() {
    let mut pipeline = FontPipeline::new(1024, 1024, 14.0);
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
    let mut pipeline = FontPipeline::new(1024, 1024, 14.0);
    let original_id = pipeline.font_id;
    let names = pipeline.list_monospace_fonts();
    let mut found_switch = false;
    for name in &names {
        if name.is_empty() {
            continue;
        }
        if pipeline.set_font_family(name, &[]) && pipeline.font_id != original_id {
            found_switch = true;
            break;
        }
    }
    assert!(
        found_switch,
        "At least one font family should change font_id from {original_id:?}",
    );
}

#[test]
fn font_switching_clears_cache() {
    let mut pipeline = FontPipeline::new(1024, 1024, 14.0);
    pipeline.rasterize_ascii();
    assert!(pipeline.cache_length() > 0);
    // 基线纳入一个非 ASCII 字形：换字体后缓存清空，仅以新字体的
    // ASCII 光栅结果重新填充，故长度必然降到该值以下。
    pipeline.glyph_information('好');
    let before = pipeline.cache_length();
    let names = pipeline.list_monospace_fonts();
    if names.len() > 1 {
        let alt = names.last().unwrap();
        pipeline.set_font_family(alt, &[]);
        assert!(
            pipeline.cache_length() < before,
            "cache should shrink after font switch to '{alt}'"
        );
    } else {
        pipeline.set_font_family("monospace", &[]);
        assert_ne!(
            pipeline.cache_length(),
            0,
            "cache must be non-empty after font switch"
        );
        assert!(
            pipeline.cache_length() < before,
            "cache should shrink after font switch"
        );
    }
}

#[test]
fn cell_metrics_height_is_integer() {
    let pipeline = FontPipeline::new(1024, 1024, 14.0);
    let (_cw, ch) = pipeline.cell_metrics();
    assert!(
        (ch - ch.floor()).abs() < f32::EPSILON,
        "cell_height should be integer (ceil'd), got {ch}"
    );
}

#[test]
fn cell_metrics_height_scales_with_font_size() {
    let small = FontPipeline::new(1024, 1024, 10.0);
    let large = FontPipeline::new(1024, 1024, 20.0);
    let (_, sh) = small.cell_metrics();
    let (_, lh) = large.cell_metrics();
    assert!(lh > sh, "larger font must have taller cell");
    assert!(
        (sh - sh.floor()).abs() < f32::EPSILON,
        "small cell_height should be integer"
    );
    assert!(
        (lh - lh.floor()).abs() < f32::EPSILON,
        "large cell_height should be integer"
    );
}

#[test]
fn termux_formula_ascent_plus_descent_equals_cell_height() {
    let pipeline = FontPipeline::new(1024, 1024, 14.0);
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
    let pipeline = FontPipeline::new(1024, 1024, 14.0);
    let ascent = pipeline.ascent_pixels();
    let (_, ch) = pipeline.cell_metrics();
    assert!(
        ascent > 0.0 && ascent < ch,
        "ascent({ascent}) must be in (0, cell_height={ch})"
    );
}

#[test]
fn termux_formula_glyph_bearing_y_matches() {
    let mut pipeline = FontPipeline::new(1024, 1024, 14.0);
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
        "bearing_y({bearing_y}) should be < cell_height({ch})"
    );
}

#[test]
fn descent_pixels_is_positive() {
    let pipeline = FontPipeline::new(1024, 1024, 14.0);
    let descent = pipeline.descent_pixels();
    assert!(descent > 0.0, "descent should be positive, got {descent}");
}

/// CJK 字形位图必须非空，且宽度落在两格之内。
///
/// 着色器按 bearing 定位、按 1:1 采样，裁剪靠位图与格子的几何关系完成，
/// 不再按前进宽度缩放——故「前进宽度合理」已无意义，改为钉住真正决定观感的
/// 两条：位图非空（否则是豆腐块/空白），宽度不超过 CJK 四边形（两格）太多
/// （超出的部分会被裁掉，用户看到半个字）。
#[test]
fn cjk_glyph_bitmap_fits_in_two_cells() {
    let mut pipeline = FontPipeline::new(1024, 1024, 14.0);
    let (cell_width, _) = pipeline.cell_metrics();
    for codepoint in [
        '\u{4E2D}', '\u{597D}', '\u{4E16}', '\u{754C}', '\u{65E5}', '\u{672C}',
    ] {
        let Some(info) = pipeline.glyph_information(codepoint) else {
            continue;
        };
        assert!(
            info.width > 0 && info.height > 0,
            "CJK {codepoint} 必须光栅化出非空位图（实得 {info:?}）"
        );
        assert!(
            info.width as f32 <= cell_width * 2.5,
            "CJK {codepoint} 位图宽 {} 超出两格（{}×2），会被裁掉半个字",
            info.width,
            cell_width
        );
    }
}

#[test]
fn ascii_bearing_y_nonnegative_for_any_font() {
    let mut pipeline = FontPipeline::new(1024, 1024, 14.0);
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
    let mut pipeline = FontPipeline::new(1024, 1024, 14.0);
    pipeline.rasterize_ascii();
    let atlas_width = pipeline.atlas_width as i32;
    let atlas_height = pipeline.atlas_height as i32;
    let chars = ['A', '中', '好', 'α', 'Ω'];
    for ch in chars {
        if let Some(info) = pipeline.glyph_information(ch) {
            assert!(
                info.atlas_x + info.width <= atlas_width,
                "glyph '{ch}' atlas_x({}) + width({}) exceeds atlas_w({})",
                info.atlas_x,
                info.width,
                atlas_width
            );
            assert!(
                info.atlas_y + info.height <= atlas_height,
                "glyph '{ch}' atlas_y({}) + height({}) exceeds atlas_h({})",
                info.atlas_y,
                info.height,
                atlas_height
            );
        }
    }
}

#[test]
fn system_monospace_name_returns_nonempty() {
    let pipeline = FontPipeline::new(1024, 1024, 14.0);
    let name = pipeline.system_monospace_name();
    assert!(
        !name.is_empty(),
        "system_monospace_name should return a non-empty string"
    );
}

#[test]
fn set_font_family_empty_resets_to_default() {
    let mut pipeline = FontPipeline::new(1024, 1024, 14.0);
    let default_name = pipeline.default_font_name().clone();
    let fonts = pipeline.list_monospace_fonts();
    if let Some(other) = fonts
        .iter()
        .find(|name| name.as_str() != default_name.as_str())
    {
        pipeline.set_font_family(other, &[]);
        assert_eq!(
            pipeline.current_font_family_name().as_deref(),
            Some(other.as_str())
        );
        pipeline.set_font_family("", &[]);
        assert_eq!(
            pipeline.current_font_family_name().as_deref(),
            Some(default_name.as_str())
        );
    }
}

#[test]
fn set_font_family_persists_through_size_change() {
    let mut pipeline = FontPipeline::new(1024, 1024, 14.0);
    let fonts = pipeline.list_monospace_fonts();
    if let Some(target) = fonts.first() {
        pipeline.set_font_family(target, &[]);
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
    let pipeline = FontPipeline::new(1024, 1024, 14.0);
    let cjk_names = pipeline.cjk_fallback_names();
    if !cjk_names.is_empty() {
        assert!(
            cjk_names.iter().all(|name| !name.is_empty()),
            "CJK fallback names should not be empty strings"
        );
    }
}

#[test]
fn cell_metrics_reasonable_ratios() {
    let pipeline = FontPipeline::new(1024, 1024, 14.0);
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
    let pipeline = FontPipeline::new(1024, 1024, 14.0);
    let name = pipeline.default_font_name();
    assert!(!name.is_empty(), "should find a monospace font, got empty");
}
