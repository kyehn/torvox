use super::*;

/// 图集每个像素的字节数（RGBA）。
const BYTES_PER_PIXEL: usize = 4;

fn bearing_fits_inside_cell(glyph: char, label: &str) {
    let mut pipeline = FontPipeline::new(512, 512, 14.0);
    let info = pipeline
        .glyph_information(glyph)
        .unwrap_or_else(|| panic!("'{glyph}' glyph_information"));
    let (_cell_width, cell_height) = pipeline.cell_metrics();
    let ascent = pipeline.ascent_pixels();
    let bearing_y = ascent - info.placement.top as f32;
    let glyph_h = info.placement.height as f32;
    assert!(
        bearing_y >= -cell_height,
        "{label} glyph starts way above cell: bearing_y={} < -cell_height",
        bearing_y
    );
    assert!(glyph_h > 0.0, "{label} glyph has zero height");
    assert!(cell_height > 0.0, "{label} cell has zero height");
}

/// 字体库里是否有能解析「中」的面。
///
/// 判据是字形覆盖而非族名含 "cjk"：用例要的前提是「能解析汉字」，族名启发式
/// 会与真实能力脱节——宿主的等宽 CJK 族名是 Maple Mono Normal NF CN（不含
/// "cjk"），而族名含 "cjk" 的面也可能没有中文字形。
fn has_cjk_coverage(font_database: &fontdb::Database) -> bool {
    font_database
        .faces()
        .any(|face| crate::render::font::font_db::face_covers(font_database, face.id, '中'))
}

fn try_load_cjk_fonts(font_database: &mut fontdb::Database) -> bool {
    if has_cjk_coverage(font_database) {
        return true;
    }
    // 只加载系统字体（开发 shell 的字体由 fontconfig 解析），绝不扫描硬编码的 store 路径。
    font_database.load_system_fonts();
    has_cjk_coverage(font_database)
}

// ──: 分层回退 ─────────────

/// fonts.xml 匹配用例的公共夹具：已加载 CJK 字体的 fontdb 与其中一个
/// TTC 字面的 `(文件名, index)`——后者覆盖 `(filename, index)` 映射路径。
fn cjk_font_db_with_ttc_face() -> (fontdb::Database, String, u32) {
    let mut font_database = fontdb::Database::new();
    assert!(
        try_load_cjk_fonts(&mut font_database),
        "CJK fonts must load (run inside nix develop)"
    );
    let (filename, index) = font_database
        .faces()
        .filter_map(|face| {
            let path = crate::render::font::font_db::face_source_path(face)?;
            let name = path.file_name()?.to_str()?.to_string();
            (path.extension()?.to_str()?.eq_ignore_ascii_case("ttc")).then_some((name, face.index))
        })
        .next()
        .expect("CJK TTC face must exist after try_load_cjk_fonts");
    (font_database, filename, index)
}

/// 经系统字体库定位 Maple Mono 字体（开发 shell 的字体由 fontconfig
/// 解析，未硬编码任何路径）。
fn find_maple_mono_font(font_database: &mut fontdb::Database) -> Option<std::path::PathBuf> {
    font_database.load_system_fonts();
    font_database
        .faces()
        .filter(|face| {
            face.families
                .first()
                .is_some_and(|(name, _)| name.to_lowercase().contains("maple"))
        })
        .filter_map(|face| match &face.source {
            fontdb::Source::File(path) => Some(path.clone()),
            fontdb::Source::SharedFile(path, _) => Some(path.clone()),
            fontdb::Source::Binary(_) => None,
        })
        .next()
}

/// 取字体库中第一个来自文件的字形字体路径。
///
/// 开发环境的 `flake.nix` 用 `FONTCONFIG_FILE` 固定了字体集，直接查已加载的
/// 字体库即可，不再探测文件系统或调用外部进程。
fn find_test_font() -> std::path::PathBuf {
    let pipeline = FontPipeline::new(512, 512, 14.0);
    pipeline
        .font_system
        .db()
        .faces()
        .filter_map(|face| match face.source {
            fontdb::Source::File(ref path) => Some(path.clone()),
            _ => None,
        })
        .find(|path| {
            path.extension()
                .and_then(|extension| extension.to_str())
                .is_some_and(|extension| extension.eq_ignore_ascii_case("ttf"))
        })
        .expect("开发环境固定了系统字体集，其中必有 .ttf 轮廓字体")
}

/// 提取字形在图集区域的覆盖度字节（RGBA 图集的红色通道）。
fn glyph_region_alpha(info: &GlyphInfo, bitmap: &[u8], atlas_width: usize) -> Vec<u8> {
    if info.allocation_id.is_none() || info.width <= 0 || info.height <= 0 {
        return Vec::new();
    }
    let mut region = Vec::with_capacity(info.width as usize * info.height as usize);
    for pixel_row in 0..info.height as usize {
        for pixel_column in 0..info.width as usize {
            let offset = ((info.atlas_y as usize + pixel_row) * atlas_width
                + info.atlas_x as usize
                + pixel_column)
                * BYTES_PER_PIXEL;
            region.push(bitmap[offset]);
        }
    }
    region
}

fn styled_test_pipeline() -> (FontPipeline, String) {
    let mut pipeline = FontPipeline::new(512, 512, 14.0);
    let font_path = find_test_font();
    let family = pipeline
        .load_font_file(&font_path)
        .expect("test font loads");
    assert!(
        pipeline.set_font_family(&family, &[]),
        "test font family selects"
    );
    (pipeline, family)
}

mod fallback;
mod glyphs;
mod loading;
