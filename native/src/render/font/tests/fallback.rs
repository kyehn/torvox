use super::*;
use crate::render::font::cjk::MAX_CJK_FALLBACK_FONTS;

/// 回归钉：样式面判定须随 `clear_identity_caches` 一并失效。
///
/// 「这个族有没有粗/斜面」的结论存在 `style_face_cache` 里，而
/// `set_system_locale` 恰恰会在清缓存之后**补装整个 TTC**（含全套字重与样式）。
/// 只清正缓存的话，补装前算出的「无粗面」会活过那次补装：主字体没变、key 不变、
/// LRU 也不会自然淘汰，于是 `bold=true` 一直走合成加粗而不是真粗面。
#[test]
fn style_face_verdict_is_invalidated_by_clear_identity_caches() {
    let mut pipeline = FontPipeline::new(512, 512, 14.0);
    // 以真实路径填充判定缓存（`resolve_style_face` 在里面记账）。
    let _ = pipeline.glyph_information_styled('A', true, false);
    assert!(
        !pipeline.caches.style_face_cache.is_empty(),
        "前置条件：样式面判定应已被缓存"
    );

    // 换 locale：这正是会补装区域回退面的那条路径。
    pipeline.set_system_locale("zh-CN");
    assert!(
        pipeline.caches.style_face_cache.is_empty(),
        "补装字面后旧的「有无粗面」结论必须作废，否则补装进来的真粗面永不被使用"
    );

    // 换字体同样要作废（另一条调用 `clear_identity_caches` 的路径）。
    // 必须换成**真正不同的族**：`set_font_family` 在解析结果与当前字面相同时是
    // 刻意不去失效的空操作（见其 KDoc），换回同一族本就不该清缓存。
    let _ = pipeline.glyph_information_styled('A', true, false);
    assert!(
        !pipeline.caches.style_face_cache.is_empty(),
        "前置条件：判定重新被缓存"
    );
    let primary_id = pipeline.font_id.expect("管线应有主字体");
    let current_family = pipeline
        .font_system
        .db()
        .face(primary_id)
        .and_then(|face| face.families.first().map(|(name, _)| name.clone()))
        .expect("主字体须给出族名");
    let other = pipeline
        .list_monospace_fonts()
        .into_iter()
        .find(|name| *name != current_family)
        .expect("宿主应有第二个等宽族（run inside nix develop）");
    assert!(pipeline.set_font_family(&other, &[]), "切到 {other} 应成功");
    assert!(
        pipeline.caches.style_face_cache.is_empty(),
        "换主字体同样必须作废样式面判定"
    );
}

/// 回归钉：解析不出的码位只扫一次全库，之后命中负缓存直接跳过。
///
/// `find_glyph_anywhere` 要遍历 fontdb 全部 face 并逐个 `with_face_data` 解压，
/// 设备未装的罕见汉字每帧一次是实打实的开销。判别力：断言第一次解析后负缓存
/// 即有该码位，且它与 `cjk_glyph_cache` 同批失效——换字体后必须重新扫，
/// 否则「新增字体后仍判找不到」会让新字永久不显示。
#[test]
fn unresolvable_codepoint_is_scanned_once_then_cached() {
    let mut pipeline = FontPipeline::new(512, 512, 14.0);
    // U+10FFFD（私用平面）无任何字体覆盖，稳定落在「扫描失败」这一支。
    let rare = char::from_u32(0x10FFFD).expect("valid private-use codepoint");
    pipeline.glyph_information(rare);
    assert!(
        pipeline.caches.unresolvable_glyphs.contains(&rare),
        "扫描失败后必须记入负缓存，否则每帧重扫全库"
    );
    // 同批失效：换字号走 clear_identity_caches，负缓存随之清空。
    pipeline.set_font_size_in_place(16.0);
    assert!(
        !pipeline.caches.unresolvable_glyphs.contains(&rare),
        "负缓存须与 cjk_glyph_cache 同批失效，否则新字体装入后仍判找不到"
    );

    // 缩放同样在失效面上：扫描的接受判据是「**当前光栅尺寸**下 swash 描得出图」
    // （`find_glyph_anywhere` 要求 `width > 0 && height > 0`，而换尺寸会换掉
    // 候选面上的 bitmap strike 与矢量轮廓）。结论绑在尺寸上却不同步失效时，
    // 缩放后个别码位永久显示 .notdef，直到换字体/字号/locale 才恢复——而
    // `glyph_cache` 与 `atlas_generation` 都已换代，症状与缓存不一致极易误判。
    pipeline.glyph_information(rare);
    assert!(
        pipeline.caches.unresolvable_glyphs.contains(&rare),
        "前置：重新落入负缓存"
    );
    pipeline.set_raster_scale(2.0);
    assert!(
        !pipeline.caches.unresolvable_glyphs.contains(&rare),
        "负缓存绑定当前光栅尺寸，改缩放必须随之失效"
    );
}

/// 合成运行同样须记入「解析不出」的负缓存。
///
/// `find_glyph_anywhere` 只回答「库里有哪个面能解析此码位」，与样式无关：
/// 合成扫不出结果时同样扫不出。不记入则粗体/斜体文本里的罕见码位每帧都要
/// 遍历 fontdb 全部 face 重扫一次——正是负缓存当初要消掉的开销。
#[test]
fn unresolvable_codepoint_is_cached_for_synthesized_runs() {
    let mut pipeline = FontPipeline::new(512, 512, 14.0);
    // U+10FFFD（私用平面）无任何字体覆盖，稳定落在「扫描失败」这一支。
    let rare = char::from_u32(0x10FFFD).expect("valid private-use codepoint");
    pipeline.glyph_information_styled(rare, true, false);
    assert!(
        pipeline.caches.unresolvable_glyphs.contains(&rare),
        "合成运行同样须记入负缓存，否则粗体罕见码位每帧重扫全库"
    );
}

/// 回退链上的「面 → 字形号」查表必须走缓存。
///
/// 合成运行逐帧重走回退链（`cjk_glyph_cache` 被合成路径跳过），查表不入缓存
/// 则每帧每字都重入 `with_face_data` 重建 charmap——字体文件重新 mmap，
/// 粗体 CJK 文本越常见越慢。
#[test]
fn fallback_chain_face_lookup_is_cached() {
    let mut pipeline = FontPipeline::new(1024, 1024, 14.0);
    assert!(
        try_load_cjk_fonts(pipeline.font_system.db_mut()),
        "宿主须装入覆盖「中」的面（nix develop 的 Noto CJK）"
    );
    // 主字体取一个不覆盖「中」的面、回退层取一个覆盖的面，使解析必然走到回退链。
    // 宿主没有 fonts.xml，回退层须手工装填（安卓上由 fonts.xml 决定）。
    let (primary_face, fallback_face) = {
        let font_database = pipeline.font_system.db();
        let fallback_face = font_database
            .faces()
            .find(|face| font_db::face_covers(font_database, face.id, '中'))
            .map(|face| face.id)
            .expect("宿主须装入覆盖「中」的面");
        let primary_face = font_database
            .faces()
            .find(|face| {
                face.id != fallback_face && !font_db::face_covers(font_database, face.id, '中')
            })
            .map(|face| face.id)
            .expect("宿主须有至少一个不覆盖「中」的面");
        (primary_face, fallback_face)
    };
    pipeline.font_id = Some(primary_face);
    pipeline.cjk_fallback_ids = vec![fallback_face];

    pipeline.glyph_information_styled('中', true, false);
    assert!(
        pipeline
            .caches
            .face_glyph_id_cache
            .contains(&(fallback_face, '中' as u32)),
        "回退链上的查表必须留下 (面, 码位) 条目，否则每帧重建 charmap"
    );
}

/// 断言码位不由主字体解析，而是由回退面解析出真实字形（非 `.notdef`）。
fn assert_resolved_via_fallback_face(
    pipeline: &mut FontPipeline,
    character: char,
    primary: fontdb::ID,
) -> GlyphInfo {
    let info = pipeline
        .glyph_information(character)
        .expect("码位必须解析出字形");
    assert!(
        info.width > 0 && info.height > 0,
        "{character:?} 必须光栅化出非空位图，实得 {info:?}"
    );
    let resolved = pipeline
        .caches
        .cjk_glyph_cache
        .get(&character)
        .copied()
        .expect("解析结果必须入缓存");
    assert_ne!(
        resolved.0, primary,
        "{character:?} 必须由回退面解析，而非主字体"
    );
    assert_ne!(
        resolved.1, 0,
        "{character:?} 必须解析到真实字形，而非 .notdef"
    );
    info
}

#[test]
fn symbol_glyph_resolves_via_database_scan() {
    // U+25B6 (▶) 不在 Liberation Mono 中，却在宿主的 DejaVu Sans 中。
    // 以 Liberation Mono 为主字体时，分层链末端是全数据库扫描（spec d7），
    // 必须在非主字体中解析出该字形。
    let mut pipeline = FontPipeline::new(512, 512, 14.0);
    let names = pipeline.list_monospace_fonts();
    assert!(
        names.iter().any(|name| name.contains("Liberation")),
        "Liberation Mono must be present (run inside nix develop)"
    );
    assert!(
        pipeline.set_font_family("Liberation Mono", &[]),
        "switch to Liberation Mono"
    );
    let primary = pipeline.font_id.expect("primary font");
    let info = assert_resolved_via_fallback_face(&mut pipeline, '▶', primary);
    // 同一符号再次解析必须稳定一致：缓存读路径不得丢失解析或引入毒化。
    let reread = pipeline
        .glyph_information('▶')
        .expect("symbol glyph reread");
    assert_eq!(
        reread.width, info.width,
        "symbol reread must match first resolution"
    );
}

/// 汉字必须由回退面解析出真实字形：等宽西文主字体不覆盖它，
/// 解析结果若是主字体的 `.notdef`，屏幕上就是豆腐块。
///
/// 「双格宽」不由字体层决定：单元宽度取自上游 VT 的宽度表，本仓只按该宽度
/// 推进列号（终端层 `cell_iterator_cjk_double_width` 钉住）。字形位图的墨迹
/// 宽度是字体自身度量，与单元宽度无关，故不得拿它与 ASCII 位图宽度作比较。
#[test]
fn han_resolves_via_fallback_face_to_a_real_glyph() {
    let mut pipeline = FontPipeline::new(512, 512, 14.0);
    assert!(
        try_load_cjk_fonts(pipeline.font_system.db_mut()),
        "宿主须装入覆盖「世」的面（nix develop 的 Noto CJK）"
    );
    assert!(
        pipeline.set_font_family("Liberation Mono", &[]),
        "switch to Liberation Mono"
    );
    let primary = pipeline.font_id.expect("primary font");
    assert_resolved_via_fallback_face(&mut pipeline, '世', primary);
}

/// `.notdef` 入缓存后不得短路回退链（spec d7 场景 3 的次序变体）。
///
/// 主字体未覆盖的码位最终会止于 `.notdef`，而 `.notdef` 是一个非空的空心方框，
/// 于是它会被当作正常字形写进字形缓存。此后**任何**主字体未覆盖的码位都会在
/// 快径上命中这条缓存并直接返回豆腐块，跳过 CJK 回退层与全库扫描——同一屏幕上
/// 「先出现的未知码位」会决定后续所有未知码位的命运。故快径必须与回退支路
/// 一样先排除字形号 0。
#[test]
fn cached_notdef_does_not_shortcut_the_fallback_chain() {
    let mut pipeline = FontPipeline::new(512, 512, 14.0);
    assert!(
        pipeline
            .list_monospace_fonts()
            .iter()
            .any(|name| name.contains("Liberation")),
        "Liberation Mono must be present (run inside nix develop)"
    );
    assert!(
        pipeline.set_font_family("Liberation Mono", &[]),
        "switch to Liberation Mono"
    );
    // 毒化：先查一个宿主任何字体都没有的码位，让 `.notdef` 进字形缓存。
    let rare = char::from_u32(0x10FFFD).expect("valid private-use codepoint");
    let notdef = pipeline
        .glyph_information(rare)
        .expect(".notdef must still produce a glyph");
    assert!(
        notdef.width > 0 && notdef.height > 0,
        "前提：.notdef 可光栅化且非空（实得 {notdef:?}），否则本用例无从验证"
    );
    // 随后查询一个必须由回退面解析的符号：若被 `.notdef` 短路，这里会是豆腐块。
    pipeline
        .glyph_information('\u{25B6}')
        .expect("symbol glyph");
    let resolved = pipeline
        .caches
        .cjk_glyph_cache
        .get(&'\u{25B6}')
        .expect("symbol must be cached");
    assert_ne!(
        resolved.1, 0,
        "先查过未知码位后，符号仍须由回退面解析出真实字形，不能被缓存的 .notdef 短路"
    );
}

#[test]
fn private_use_glyph_renders_notdef_without_panic() {
    // U+E0A0（powerline 分隔符 PUA）在宿主任何字体中都不存在：回退链必须
    // 止于 .notdef 而不 panic（spec d7 场景 3）。
    let mut pipeline = FontPipeline::new(512, 512, 14.0);
    let info = pipeline.glyph_information('\u{e0a0}');
    assert!(info.is_some(), ".notdef fallback must return a glyph");
}

#[test]
fn emoji_glyph_gets_an_outline_when_color_font_cannot_outline() {
    // Noto Color Emoji 覆盖 😀，而 swash 无法为彩色字形生成轮廓；本仓经系统链
    // 兜出可用度量。断言的是「确实拿到非空度量 + 垂直不越格 + 后续普通字形
    // 不受污染」，而不是「没有 panic」——后者不构成行为断言（TESTING：「不崩溃」
    // 不是有效断言）。
    let mut pipeline = FontPipeline::new(512, 512, 14.0);
    let emoji = pipeline
        .glyph_information('😀')
        .expect("彩色 emoji 必须经系统链兜出可用度量，而不是被静默跳过");
    assert!(
        emoji.placement.width > 0 && emoji.placement.height > 0,
        "彩色 emoji 的轮廓不得为空：{:?}",
        emoji.placement
    );
    let (_cell_width, cell_height) = pipeline.cell_metrics();
    let ascent = pipeline.ascent_pixels();
    let bearing_y = ascent - emoji.placement.top as f32;
    assert!(
        bearing_y >= -cell_height,
        "彩色 emoji 越出格子上沿：bearing_y={} < -cell_height={}",
        bearing_y,
        cell_height
    );
    let after = pipeline.glyph_information('A');
    assert!(
        after.is_some(),
        "彩色 emoji 的查询不得污染同一 pipeline 上的后续字形"
    );
}

#[test]
fn fallback_names_report_all_layers() {
    // cjk_fallback_names 是仅含 CJK 的视图；分层字段由构造保证全部填充。
    let pipeline = FontPipeline::new(512, 512, 14.0);
    let cjk = pipeline.cjk_fallback_names();
    assert!(
        cjk.iter().all(|name| !name.is_empty()),
        "CJK fallback names must not be empty strings"
    );
}

/// 主字体覆盖 CJK 后回退层必须被清空。
///
/// 回退层只在主字体画不出 CJK 时才有意义：留着它既让每个码位多试几个面，
/// 又让设置页上报的「回退字体」与实际渲染不符。切换主字体即重新发现回退层
/// （`rediscover_fallback_fonts`），这条路径必须把旧回退层清掉。
///
/// 判据是「该面能否解析 CJK 探测字」而不是族名含 "cjk"：宿主里覆盖 CJK 的
/// 等宽族名是 Maple Mono Normal NF CN，按族名子串筛一个都找不到——旧实现用
/// `if let Some` 把「找不到」静默跳过，用例从未真正断言。
#[test]
fn primary_cjk_font_clears_fallback_layer() {
    let mut pipeline = FontPipeline::new(512, 512, 14.0);
    assert!(
        try_load_cjk_fonts(pipeline.font_system.db_mut()),
        "CJK fonts must load (run inside nix develop)"
    );
    pipeline.set_system_locale("zh-CN");
    let probe = font_db::cjk_coverage_probe_char("zh-CN");
    let (cjk_face, fallback_face) = {
        let font_database = pipeline.font_system.db();
        let cjk_face = font_database
            .faces()
            .find(|face| face.monospaced && font_db::face_covers(font_database, face.id, probe))
            .expect("宿主须装入覆盖 CJK 的等宽族（nix develop 的 Maple Mono Normal NF CN）");
        let fallback_face = font_database
            .faces()
            .find(|face| {
                face.id != cjk_face.id && font_db::face_covers(font_database, face.id, probe)
            })
            .expect("宿主须装入第二个覆盖 CJK 的面");
        (cjk_face.id, fallback_face.id)
    };
    // 宿主没有 fonts.xml，回退层须手工装填（安卓上由 fonts.xml 决定）。
    pipeline.cjk_fallback_ids = vec![fallback_face];
    assert!(
        !pipeline.cjk_fallback_names().is_empty(),
        "前置条件：回退层已装填"
    );
    let family = pipeline
        .font_system
        .db()
        .face(cjk_face)
        .and_then(|face| face.families.first().map(|(name, _)| name.clone()))
        .expect("覆盖 CJK 的面须给出族名");
    assert!(
        pipeline.set_font_family(&family, &[]),
        "切到 '{family}' 应成功"
    );
    assert!(
        pipeline.primary_covers_cjk_probe(),
        "主字体须覆盖 CJK 探测字，否则回退层本就不该被清空"
    );
    assert!(
        pipeline.cjk_fallback_names().is_empty(),
        "主字体覆盖 CJK 后回退层必须清空，上报状态与实际渲染才一致"
    );
}

/// 回退层上限必须生效：fonts.xml 的 `lang` 回退条目可能很多，全装会让每个
/// 码位多试几个面。截断时保留 xml 顺序的前若干个（首位即实际命中者）。
#[test]
fn cjk_fallback_layer_respects_the_cap() {
    let (font_database, _, _) = cjk_font_db_with_ttc_face();
    // 库中互不相同的 (文件名, index)：fonts.xml 只按这两项精确匹配。
    let mut entries: Vec<(String, u32)> = Vec::new();
    for face in font_database.faces() {
        let Some(path) = crate::render::font::font_db::face_source_path(face) else {
            continue;
        };
        let Some(filename) = path.file_name().and_then(|name| name.to_str()) else {
            continue;
        };
        let entry = (filename.to_string(), face.index);
        if !entries.contains(&entry) {
            entries.push(entry);
        }
    }
    assert!(
        entries.len() > MAX_CJK_FALLBACK_FONTS,
        "宿主须装入多于 {MAX_CJK_FALLBACK_FONTS} 个可精确匹配的面，实得 {}",
        entries.len()
    );
    let fonts_xml = entries
        .iter()
        .map(|(filename, index)| {
            format!(r#"<font weight="400" style="normal" index="{index}">{filename}</font>"#)
        })
        .collect::<String>();
    let xml = format!(
        r#"<familyset version="23"><family lang="zh-Hans">{fonts_xml}</family></familyset>"#
    );
    let ids = FontPipeline::match_fonts_xml_fallbacks(
        &font_database,
        &crate::render::font::font_db::parse_fonts_xml_document(&xml).families,
        "zh-CN",
        MAX_CJK_FALLBACK_FONTS,
    );
    assert_eq!(
        ids.len(),
        MAX_CJK_FALLBACK_FONTS,
        "回退层必须截断到上限：{ids:?}"
    );
    let first_filename = &entries[0].0;
    let first_index = entries[0].1;
    let expected_first = font_database
        .faces()
        .find(|face| {
            face.index == first_index
                && crate::render::font::font_db::face_source_path(face)
                    .and_then(|path| path.file_name().and_then(|name| name.to_str()))
                    .is_some_and(|name| name == first_filename.as_str())
        })
        .map(|face| face.id)
        .expect("首个条目对应的面必须存在");
    assert_eq!(ids[0], expected_first, "截断必须保留 xml 顺序的前若干个");
}

#[test]
fn fonts_xml_index_match_resolves_exact_face() {
    let (font_database, filename, index) = cjk_font_db_with_ttc_face();
    let xml = format!(
        r#"<familyset version="23"><family lang="zh-Hans"><font weight="400" style="normal" index="{index}">{filename}</font></family></familyset>"#
    );
    let ids = FontPipeline::match_fonts_xml_fallbacks(
        &font_database,
        &crate::render::font::font_db::parse_fonts_xml_document(&xml).families,
        "zh-CN",
        3,
    );
    assert_eq!(ids.len(), 1, "exact (filename, index) hit expected");
    let face = font_database.face(ids[0]).expect("matched face exists");
    assert_eq!(face.index, index, "TTC index must match fonts.xml");
    let matched_name = crate::render::font::font_db::face_source_path(face)
        .expect("matched face must be file-backed")
        .file_name()
        .and_then(|name| name.to_str())
        .expect("file-backed face has a name");
    assert_eq!(matched_name, filename);
}

#[test]
fn fonts_xml_missing_file_falls_back_to_scan() {
    // 文件名未知：无精确命中，调用方改由扫描结果填充。
    let (font_database, _, _) = cjk_font_db_with_ttc_face();
    let xml = r#"<familyset version="23"><family lang="zh-Hans"><font index="2">NoSuchFont-Regular.ttc</font></family></familyset>"#;
    let ids = FontPipeline::match_fonts_xml_fallbacks(
        &font_database,
        &crate::render::font::font_db::parse_fonts_xml_document(xml).families,
        "zh-CN",
        3,
    );
    assert!(
        ids.is_empty(),
        "unknown file must yield no exact hit: {ids:?}"
    );
}

#[test]
fn fonts_xml_duplicate_entries_deduplicated() {
    // 真机形态：zh-Hans 链内同一文件多 weights 同 index 重复出现，
    // 首位命中后后续重复必须去重（真机 Sans index=2 出现 9 次只取 1 个）。
    let (font_database, filename, index) = cjk_font_db_with_ttc_face();
    let xml = format!(
        r#"<familyset version="23"><family lang="zh-Hans"><font weight="100" style="normal" index="{index}">{filename}</font><font weight="400" style="normal" index="{index}">{filename}</font><font weight="900" style="normal" index="{index}">{filename}</font></family></familyset>"#
    );
    let ids = FontPipeline::match_fonts_xml_fallbacks(
        &font_database,
        &crate::render::font::font_db::parse_fonts_xml_document(&xml).families,
        "zh-CN",
        3,
    );
    assert_eq!(
        ids.len(),
        1,
        "repeated (filename, index) entries must deduplicate: {ids:?}"
    );
    let face = font_database.face(ids[0]).expect("matched face exists");
    assert_eq!(face.index, index, "TTC index must match fonts.xml");
}

#[test]
fn fonts_xml_result_order_follows_xml_order() {
    // 真机形态：zh-Hans 链 Sans 在前 Serif 在后，返回顺序必须跟随
    // xml 顺序（首位胜出），而非数据库加载顺序。
    let mut font_database = fontdb::Database::new();
    assert!(
        try_load_cjk_fonts(&mut font_database),
        "CJK fonts must load (run inside nix develop)"
    );
    let mut faces: Vec<(String, u32)> = Vec::new();
    for face in font_database.faces() {
        let path = match &face.source {
            fontdb::Source::File(path) => path,
            fontdb::Source::SharedFile(path, _) => path,
            fontdb::Source::Binary(_) => continue,
        };
        let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
            continue;
        };
        let is_ttc = path
            .extension()
            .and_then(|extension| extension.to_str())
            .is_some_and(|extension| extension.eq_ignore_ascii_case("ttc"));
        if is_ttc && !faces.iter().any(|(_, index)| *index == face.index) {
            faces.push((name.to_string(), face.index));
        }
        if faces.len() == 2 {
            break;
        }
    }
    assert_eq!(
        faces.len(),
        2,
        "TTC must expose two distinct faces for order test"
    );
    let (first_filename, first_index) = &faces[0];
    let (second_filename, second_index) = &faces[1];
    let xml = format!(
        r#"<familyset version="23"><family lang="zh-Hans"><font weight="400" style="normal" index="{first_index}">{first_filename}</font><font weight="400" style="normal" index="{second_index}">{second_filename}</font></family></familyset>"#
    );
    let ids = FontPipeline::match_fonts_xml_fallbacks(
        &font_database,
        &crate::render::font::font_db::parse_fonts_xml_document(&xml).families,
        "zh-CN",
        3,
    );
    assert_eq!(ids.len(), 2, "both entries must resolve: {ids:?}");
    assert_eq!(
        font_database.face(ids[0]).expect("first face exists").index,
        *first_index,
        "first result must follow the first xml entry"
    );
    assert_eq!(
        font_database
            .face(ids[1])
            .expect("second face exists")
            .index,
        *second_index,
        "second result must follow the second xml entry"
    );
}

#[test]
fn maple_mono_primary_skips_cjk_fallback() {
    // Maple Mono NF CN 自带 CJK 字形：作为主字体时必须直接覆盖 CJK，
    // 不产生回退层（spec：跳过路径）。CJK 与拉丁文走同一缓存，
    // CJK 渲染速度与拉丁文持平（无逐字形回退扫描）。
    let mut maple_db = fontdb::Database::new();
    let font_path = find_maple_mono_font(&mut maple_db)
        .expect("Maple Mono must be present (run inside nix develop)");
    let mut pipeline = FontPipeline::new(512, 512, 14.0);
    let family = pipeline
        .load_font_file(&font_path)
        .expect("maple mono loads");
    assert!(
        pipeline.set_font_family(&family, &[]),
        "maple mono selectable as primary"
    );
    pipeline.set_system_locale("zh-CN");
    assert!(
        pipeline.cjk_fallback_names().is_empty(),
        "CJK-capable primary must skip fallback, got: {:?}",
        pipeline.cjk_fallback_names()
    );
    let latin = pipeline.glyph_information('A').expect("latin resolves");
    let cjk = pipeline.glyph_information('中').expect("CJK resolves");
    assert!(latin.width > 0 && cjk.width > 0);
    // 第二遍必须命中缓存（不再重复回退扫描）。
    let latin_again = pipeline.glyph_information('A').expect("latin cached");
    let cjk_again = pipeline.glyph_information('中').expect("CJK cached");
    assert_eq!(latin.width, latin_again.width);
    assert_eq!(cjk.width, cjk_again.width);
}

#[test]
fn atlas_defrag_recovers_from_full_atlas() {
    let mut pipeline = FontPipeline::new(64, 64, 14.0);
    let mut successes = 0u32;
    for cp in 0x4E00u32..0x4F00u32 {
        if let Some(ch) = char::from_u32(cp)
            && pipeline
                .glyph_information(ch)
                .is_some_and(|glyph_info| glyph_info.width > 0)
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
