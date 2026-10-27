use super::*;

#[test]
fn load_font_file_valid_ttf_returns_family() {
    let mut pipeline = FontPipeline::new(512, 512, 14.0);
    let font_path = find_test_font();
    let family = pipeline
        .load_font_file(&font_path)
        .expect("有效 TTF 必须返回族名");
    assert!(
        pipeline
            .font_system
            .db()
            .faces()
            .any(|face| face.families.iter().any(|(name, _)| *name == family)),
        "返回的族名 {family} 必须已在字体库中登记"
    );
}

#[test]
fn load_font_file_nonexistent_path_returns_none() {
    let mut pipeline = FontPipeline::new(512, 512, 14.0);
    let result = pipeline.load_font_file(std::path::Path::new("/nonexistent/path/to/font.ttf"));
    assert!(result.is_none(), "should return None for nonexistent path");
}

#[test]
fn load_font_file_empty_file_returns_none() {
    let dir = tempfile::tempdir().unwrap();
    let empty_path = dir.path().join("empty.ttf");
    std::fs::write(&empty_path, []).ok();
    let mut pipeline = FontPipeline::new(512, 512, 14.0);
    let result = pipeline.load_font_file(&empty_path);
    assert!(result.is_none(), "empty file should return None");
}

#[test]
fn load_font_file_corrupt_file_returns_none() {
    let dir = tempfile::tempdir().unwrap();
    let corrupt_path = dir.path().join("corrupt.ttf");
    let garbage: Vec<u8> = (0..256).map(|raw_byte| (raw_byte ^ 0xAB) as u8).collect();
    std::fs::write(&corrupt_path, &garbage).ok();
    let mut pipeline = FontPipeline::new(512, 512, 14.0);
    let result = pipeline.load_font_file(&corrupt_path);
    assert!(result.is_none(), "corrupt file should return None");
}

// ──: 粗/斜体字形合成 ─────────────────────────

/// 可变字体必须走 `wght`/`ital` 轴（DESIGN 字体节：多字重与动态字体）。
///
/// 按能力而非按名字找：开发环境的 `flake.nix` 固定了 `noto-fonts-cjk-sans`
/// （`NotoSansCJK-VF.otf.ttc`，带 `wght` 轴）与 `maple-mono.Normal-NF-CN`
/// （16 个静态字重的族，**无** `fvar` 表，真字重靠同族查询）。两种形态都须正确。
#[test]
fn variable_font_axes_are_applied() {
    let pipeline = FontPipeline::new(512, 512, 24.0);
    let font_database = pipeline.font_system.db();
    let axes_of = |font_id: fontdb::ID| {
        font_database
            .with_face_data(font_id, |font_data, face_index| {
                let font_ref = swash::FontRef::from_index(font_data, face_index as usize)?;
                Some(
                    font_ref
                        .variations()
                        .map(|axis| (axis.tag(), axis.min_value(), axis.max_value()))
                        .collect::<Vec<_>>(),
                )
            })
            .flatten()
    };
    let settings_of = |font_id: fontdb::ID, synthesis: GlyphSynthesis| {
        font_database
            .with_face_data(font_id, |font_data, face_index| {
                let font_ref = swash::FontRef::from_index(font_data, face_index as usize)?;
                Some(super::super::variation_settings(&font_ref, synthesis))
            })
            .flatten()
            .expect("字体可读")
    };

    let (variable_id, variable_axes) = font_database
        .faces()
        .filter_map(|face| axes_of(face.id).map(|axes| (face.id, axes)))
        .find(|(_, axes)| {
            axes.iter()
                .any(|(tag, min, max)| *tag == swash::tag_from_bytes(b"wght") && max > min)
        })
        .expect("开发环境固定了带 wght 轴的可变字体（noto-fonts-cjk-sans）");
    let (_, min_weight, max_weight) = variable_axes
        .iter()
        .find(|(tag, _, _)| *tag == swash::tag_from_bytes(b"wght"))
        .copied()
        .expect("已断言存在 wght 轴");

    // 粗体只设 wght 一条轴，取值被 clamp 到字体声明的范围。
    let bold = settings_of(variable_id, GlyphSynthesis::Bold);
    assert_eq!(bold.len(), 1, "粗体只应设置 wght 一条轴");
    assert_eq!(bold[0].tag, swash::tag_from_bytes(b"wght"));
    assert_eq!(
        bold[0].value,
        VARIATION_WEIGHT_BOLD.min(max_weight).max(min_weight)
    );

    // 粗斜体：字体声明 ital 轴时两条，否则只有 wght。
    let bold_italic = settings_of(variable_id, GlyphSynthesis::BoldItalic);
    let has_ital = variable_axes
        .iter()
        .any(|(tag, _, _)| *tag == swash::tag_from_bytes(b"ital"));
    assert_eq!(
        bold_italic.len(),
        if has_ital { 2 } else { 1 },
        "粗斜体轴数须与字体声明一致"
    );

    // 静态字体（无 fvar，如 Maple Mono / DroidSansMono）不得产出轴设置，
    // 交回轮廓级加粗/剪切。
    let static_id = font_database
        .faces()
        .map(|face| face.id)
        .find(|id| axes_of(*id).is_some_and(|axes| axes.is_empty()))
        .expect("系统等宽字体不是可变字体");
    assert!(
        settings_of(static_id, GlyphSynthesis::BoldItalic).is_empty(),
        "静态字体必须退回轮廓级合成"
    );
}

#[test]
fn styled_bold_produces_heavier_glyph() {
    let (mut pipeline, _) = styled_test_pipeline();
    let regular = pipeline.glyph_information('A').expect("regular A");
    let bold = pipeline
        .glyph_information_styled('A', true, false)
        .expect("bold A");
    assert!(bold.width > 0 && bold.height > 0, "bold bitmap must exist");
    // 样式位图必须与常规位图实际不同——要么解析到真实粗体字面，
    // 要么由合成做了加粗。
    assert_ne!(
        bold.atlas_x, regular.atlas_x,
        "bold and regular glyphs must not share a cache entry"
    );
}

#[test]
fn styled_italic_shears_glyph() {
    let (mut pipeline, _) = styled_test_pipeline();
    let regular = pipeline.glyph_information('A').expect("regular A");
    let italic = pipeline
        .glyph_information_styled('A', false, true)
        .expect("italic A");
    assert!(
        italic.width > 0 && italic.height > 0,
        "italic bitmap must exist"
    );
    assert_ne!(
        italic.atlas_x, regular.atlas_x,
        "italic and regular glyphs must not share a cache entry"
    );
    // 真实斜体字面或剪切合成都会改变位图内容，直接比较区域字节，
    // 避免像素计数偶然相同导致的误判。
    let bitmap = pipeline.atlas_bitmap();
    let atlas_width = pipeline.atlas_width as usize;
    let regular_alpha = glyph_region_alpha(&regular, bitmap, atlas_width);
    let italic_alpha = glyph_region_alpha(&italic, bitmap, atlas_width);
    assert!(
        regular_alpha.iter().any(|&alpha| alpha > 0),
        "常规字形区域必须有墨水"
    );
    assert!(
        italic_alpha.iter().any(|&alpha| alpha > 0),
        "斜体字形区域必须有墨水"
    );
    assert_ne!(
        regular_alpha, italic_alpha,
        "斜体位图必须与常规不同（真实字面或剪切合成）"
    );
}

#[test]
fn styled_bold_italic_combines_both() {
    let (mut pipeline, _) = styled_test_pipeline();
    let regular = pipeline.glyph_information('A').expect("regular A");
    let bold_italic = pipeline
        .glyph_information_styled('A', true, true)
        .expect("bold-italic A");
    assert!(bold_italic.width > 0 && bold_italic.height > 0);
    assert_ne!(
        bold_italic.atlas_x, regular.atlas_x,
        "bold-italic must have its own cache entry"
    );
}

#[test]
fn styled_glyph_cache_distinguishes_synthesis() {
    let (mut pipeline, _) = styled_test_pipeline();
    let _regular = pipeline.glyph_information('A').expect("regular A");
    let bold = pipeline
        .glyph_information_styled('A', true, false)
        .expect("bold A");
    let italic = pipeline
        .glyph_information_styled('A', false, true)
        .expect("italic A");
    // 再次查询返回缓存中的样式字形（同一图集槽位），绝不返回常规字形。
    let bold_again = pipeline
        .glyph_information_styled('A', true, false)
        .expect("bold A again");
    let italic_again = pipeline
        .glyph_information_styled('A', false, true)
        .expect("italic A again");
    assert_eq!(bold.atlas_x, bold_again.atlas_x);
    assert_eq!(italic.atlas_x, italic_again.atlas_x);
}

/// 图集重建必须保留合成位：满图重建把合成字形降级为常规位图，
/// 斜体重建成常规体（满图后斜体退化缺失）。
#[test]
fn rebuild_atlas_preserves_synthesis_keyed_entries() {
    let (mut pipeline, _) = styled_test_pipeline();
    let regular = pipeline.glyph_information('A').expect("regular A");
    let glyph_id = pipeline.caches.ascii_glyph_ids['A' as usize].expect("ascii gid cached");
    let font_id = pipeline.font_id.expect("primary font set");
    let italic = pipeline
        .glyph_information_from_font_with_synthesis(font_id, glyph_id, GlyphSynthesis::Italic)
        .expect("synthesized italic A");
    assert!(
        italic.width > 0 && italic.height > 0,
        "synthesized italic bitmap must exist"
    );
    let atlas_width = pipeline.atlas_width as usize;
    let italic_alpha_before = glyph_region_alpha(&italic, pipeline.atlas_bitmap(), atlas_width);
    assert!(
        italic_alpha_before.iter().any(|&alpha| alpha > 0),
        "synthesized italic region must have ink"
    );
    assert!(
        pipeline
            .caches
            .glyph_cache
            .iter()
            .any(|(key, _)| key.synthesis == GlyphSynthesis::Italic.bits()),
        "precondition: italic synthesis entry must be cached"
    );
    let generation = pipeline.atlas_generation();
    pipeline.rebuild_atlas();
    assert!(
        pipeline.atlas_generation() > generation,
        "rebuild must bump the atlas generation"
    );
    assert!(
        pipeline
            .caches
            .glyph_cache
            .iter()
            .any(|(key, _)| key.synthesis == GlyphSynthesis::Italic.bits()),
        "rebuild must preserve synthesis-keyed entries"
    );
    let italic_after = pipeline
        .lookup_glyph(font_id, glyph_id, GlyphSynthesis::Italic)
        .expect("synthesized italic must stay cached after rebuild");
    let bitmap_after = pipeline.atlas_bitmap().to_vec();
    let atlas_width_after = pipeline.atlas_width as usize;
    let italic_alpha_after = glyph_region_alpha(&italic_after, &bitmap_after, atlas_width_after);
    assert_eq!(
        italic_alpha_before, italic_alpha_after,
        "rebuilt synthesized bitmap must match the pre-rebuild content"
    );
    let regular_after = pipeline
        .glyph_information('A')
        .expect("regular A after rebuild");
    let regular_alpha_after = glyph_region_alpha(&regular_after, &bitmap_after, atlas_width_after);
    assert_ne!(
        regular_alpha_after, italic_alpha_after,
        "synthesized italic must still differ from regular after rebuild"
    );
    let _ = regular;
}

/// 回应对“d 有些区域像 a”：相邻小写字母必须命中不同缓存条目与不同位图。
#[test]
fn distinct_lowercase_glyphs_do_not_collide() {
    let (mut pipeline, _) = styled_test_pipeline();
    let glyph_d = pipeline.glyph_information('d').expect("d");
    let glyph_a = pipeline.glyph_information('a').expect("a");
    assert_ne!(
        (glyph_d.atlas_x, glyph_d.atlas_y),
        (glyph_a.atlas_x, glyph_a.atlas_y),
        "d and a must not share a cache entry"
    );
    let bitmap = pipeline.atlas_bitmap();
    let atlas_width = pipeline.atlas_width as usize;
    let alpha_d = glyph_region_alpha(&glyph_d, bitmap, atlas_width);
    let alpha_a = glyph_region_alpha(&glyph_a, bitmap, atlas_width);
    assert!(alpha_d.iter().any(|&alpha| alpha > 0), "d 必须有墨水");
    assert!(alpha_a.iter().any(|&alpha| alpha > 0), "a 必须有墨水");
    assert_ne!(alpha_d, alpha_a, "d 与 a 位图必须不同");
}

#[test]
fn resolve_style_face_prefers_same_family_bold_when_available() {
    let (mut pipeline, _) = styled_test_pipeline();
    let base_id = pipeline.font_id.expect("font selected");
    let base_family = pipeline.font_system.db().face(base_id).and_then(|face| {
        face.families
            .first()
            .map(|(family_name, _)| family_name.clone())
    });
    // 加载系统字体后，该族在本宿主上可能有也可能没有粗体字面。
    // 无论哪种情况契约都成立：解析到的字面同族且不同于基础字面；
    // 完全没有字面时调用方退回合成。
    if let Some(style_id) = pipeline.resolve_style_face(base_id, true, false) {
        assert_ne!(style_id, base_id, "bold face must differ from regular");
        let style_family = pipeline.font_system.db().face(style_id).and_then(|face| {
            face.families
                .first()
                .map(|(family_name, _)| family_name.clone())
        });
        assert_eq!(
            base_family, style_family,
            "style face must share the base family"
        );
    }
    // 解析常规样式得到同族字面（fontdb 查询返回最接近者，除非族内另有
    // 常规字面（如 DejaVuSansCondensed），否则就是基础字面本身——
    // 故此处只断言族不变式）。
    if let Some(plain) = pipeline.resolve_style_face(base_id, false, false) {
        let plain_family = pipeline.font_system.db().face(plain).and_then(|face| {
            face.families
                .first()
                .map(|(family_name, _)| family_name.clone())
        });
        assert_eq!(
            base_family, plain_family,
            "plain-style face must share the base family"
        );
    }
}

#[test]
fn load_font_file_multiple_times_works() {
    let mut pipeline = FontPipeline::new(512, 512, 14.0);
    let font_path = find_test_font();
    let first = pipeline.load_font_file(&font_path);
    let second = pipeline.load_font_file(&font_path);
    assert!(first.is_some(), "first load should succeed");
    assert!(second.is_some(), "second load of same file should succeed");
    assert_eq!(
        first, second,
        "loading same file twice should return same family"
    );
}

#[test]
fn load_font_file_does_not_break_cell_metrics() {
    let mut pipeline = FontPipeline::new(512, 512, 14.0);
    let (cell_width_before, cell_height_before) = pipeline.cell_metrics();
    assert!(
        cell_width_before > 0.0 && cell_height_before > 0.0,
        "initial metrics should be positive"
    );
    let font_path = find_test_font();
    let family = pipeline.load_font_file(&font_path);
    assert!(family.is_some(), "should load test font");
    let (cell_width_after, cell_height_after) = pipeline.cell_metrics();
    assert!(
        (cell_width_before - cell_width_after).abs() < f32::EPSILON,
        "cell width unchanged after load_font_file"
    );
    assert!(
        (cell_height_before - cell_height_after).abs() < f32::EPSILON,
        "cell height unchanged after load_font_file"
    );
}

#[test]
fn load_font_file_loaded_font_can_be_set() {
    let mut pipeline = FontPipeline::new(512, 512, 14.0);
    let family = pipeline
        .load_font_file(&find_test_font())
        .expect("should load test font");
    assert!(
        pipeline.set_font_family(&family, &[]),
        "set_font_family should succeed for loaded font '{family}'"
    );
    let (cw, ch) = pipeline.cell_metrics();
    assert!(cw > 0.0, "cell width positive after setting loaded font");
    assert!(ch > 0.0, "cell height positive after setting loaded font");
}

/// 按族名选中必须用「装入该文件时记住的那份面」。
///
/// fontdb 的库只增不删：同一路径的字体文件被替换后旧面仍留在库里，
/// 按族名扫库取首个会选回旧面，用户看到「换了字体却不生效」。
#[test]
fn set_font_family_uses_the_face_loaded_from_the_file() {
    let mut pipeline = FontPipeline::new(512, 512, 14.0);
    let family = pipeline
        .load_font_file(&find_test_font())
        .expect("应装入测试字体");
    let loaded_face = pipeline.user_font_faces[&family.to_lowercase()];
    assert!(
        pipeline.set_font_family(&family, &[]),
        "装入后按族名选中必须成功"
    );
    assert_eq!(
        pipeline.font_id,
        Some(loaded_face),
        "选中面必须是装入文件时记住的那份"
    );
}

/// 「族是否已在渲染字体库中」的判定：常驻族与装入的族为常驻，未知族非常驻。
///
/// 该判定决定要不要解析族文件列表——解析会构建族索引（真机实测 127ms），故
/// 已在库中的族必须走「不解析」的快径，否则装载字体文件也会把全量枚举拉起来。
#[test]
fn family_resident_reports_only_families_in_the_font_database() {
    let mut pipeline = FontPipeline::new(512, 512, 14.0);
    assert!(
        pipeline.family_resident(""),
        "空族名表示系统等宽字体，恒在库内"
    );
    let family = pipeline
        .load_font_file(&find_test_font())
        .expect("应装入测试字体");
    assert!(pipeline.family_resident(&family), "装入文件后该族必在库内");
    assert!(
        !pipeline.family_resident("__不存在的字体族__"),
        "库内没有的族不得判为常驻"
    );
}

/// 同一路径的文件被替换后重新装入：按族名选中必须改用新装入的那份面。
#[test]
fn reloading_a_file_switches_family_selection_to_the_new_face() {
    let mut pipeline = FontPipeline::new(512, 512, 14.0);
    let font_path = find_test_font();
    let family = pipeline
        .load_font_file(&font_path)
        .expect("首次装入测试字体");
    let first_face = pipeline.user_font_faces[&family.to_lowercase()];
    let reloaded_family = pipeline
        .load_font_file(&font_path)
        .expect("再次装入测试字体");
    let second_face = pipeline.user_font_faces[&reloaded_family.to_lowercase()];
    assert_ne!(
        first_face, second_face,
        "再次装入同一文件必须产生新的面（字体库只增不删）"
    );
    assert!(
        pipeline.set_font_family(&reloaded_family, &[]),
        "重新装入后按族名选中必须成功"
    );
    assert_eq!(
        pipeline.font_id,
        Some(second_face),
        "选中面必须是重新装入的那份"
    );
}

#[test]
fn load_font_file_unicode_path() {
    let dir = tempfile::tempdir().unwrap();
    let target = dir.path().join("测试-font.ttf");
    std::fs::copy(find_test_font(), &target).expect("copy test font to unicode path");
    let mut pipeline = FontPipeline::new(512, 512, 14.0);
    let family = pipeline.load_font_file(&target);
    assert!(family.is_some(), "should load font from unicode path");
    assert!(!family.unwrap().is_empty(), "family should not be empty");
}

#[test]
fn load_font_file_after_set_font_family() {
    let mut pipeline = FontPipeline::new(512, 512, 14.0);
    let fonts = pipeline.list_monospace_fonts();
    if let Some(first) = fonts.first() {
        assert!(
            pipeline.set_font_family(first, &[]),
            "set font family {first}"
        );
    }
    let result = pipeline.load_font_file(&find_test_font());
    assert!(
        result.is_some(),
        "load after set_font_family should succeed"
    );
}

/// 真机日志：每次改字号后 Kotlin 侧会对**同一**字族再调两次 `setFontFamily`
/// （设置应用与会话状态发射两条路径），而每次都清图集、重光栅、重探回退层，
/// 渲染帧被拖到 100–180ms，VT 线程的单元帧通道随之溢出丢帧。重设为当前字面
/// 必须是空操作：图集代际不变、已光栅的字形仍在。
#[test]
fn set_font_family_to_current_face_keeps_atlas() {
    let mut pipeline = FontPipeline::new(512, 512, 14.0);
    let family = pipeline
        .list_monospace_fonts()
        .into_iter()
        .next()
        .expect("宿主须有等宽字体");
    assert!(pipeline.set_font_family(&family, &[]), "首次选中 {family}");
    assert!(pipeline.glyph_information('A').is_some());
    let generation = pipeline.atlas_generation();
    let cached = pipeline.caches.glyph_cache.len();

    assert!(
        pipeline.set_font_family(&family, &[]),
        "重设同一字族仍报告找到"
    );
    assert_eq!(
        pipeline.atlas_generation(),
        generation,
        "重设同一字面不得重建图集"
    );
    assert_eq!(
        pipeline.caches.glyph_cache.len(),
        cached,
        "重设同一字面不得丢弃已光栅的字形"
    );
}

#[test]
fn glyph_cache_key_distinguishes_subpixel_raster_sizes() {
    // 36.75px 与 36.22px 截断同为 36：旧 `as u16` 键共用同一条目，
    // 缩放后取到错误尺寸位图（模糊/字形不对）。键必须精确区分。
    let big = 14.0f32 * 2.625;
    let small = 13.8f32 * 2.625;
    assert_eq!(big as u16, small as u16, "用例前提：截断后同键");
    assert_ne!(
        super::super::raster_size_key(big),
        super::super::raster_size_key(small),
        "不同光栅尺寸必须命中不同缓存条目"
    );
    assert_eq!(
        super::super::raster_size_key(big),
        super::super::raster_size_key(big),
        "同尺寸必须命中同一条目"
    );
}
