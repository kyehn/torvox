use super::*;
use crate::terminal::ghostty_terminal::CellData;

fn cell_data(
    row: u32,
    col: u32,
    character: char,
    foreground: [f32; 4],
    background: [f32; 4],
    flags: u32,
) -> CellData {
    CellData {
        codepoint: character as u32,
        width: 1,
        grapheme_extra: [0; 7],
        foreground,
        background,
        underline_color: foreground,
        flags,
        row,
        col,
    }
}

fn build(
    cells: &[CellData],
    cursor: CellCursor,
    highlights: &[SearchHighlight],
) -> Vec<CellInstance> {
    let mut font_pipeline = crate::render::font::FontPipeline::new(1024, 1024, 14.0);
    let mut instances = Vec::new();
    let result = build_instances_from_cell_data(
        cells,
        CellInstanceConfig {
            rows: 24,
            cols: 80,
            grid_cell_width: 1024.0 / 80.0,
            grid_cell_height: 1024.0 / 24.0,
            cursor,
            atlas_width: 1024.0,
            atlas_height: 1024.0,
            search_highlights: highlights,
            default_background: [0.0; 4],
        },
        &mut font_pipeline,
        &mut instances,
    );
    assert!(
        result.is_some(),
        "build should succeed with a font pipeline"
    );
    instances
}

/// 无反显/高亮时，普通单元保持自身颜色。
#[test]
fn plain_cell_keeps_colors() {
    let cells = vec![cell_data(
        0,
        0,
        'A',
        [1.0, 0.0, 0.0, 1.0],
        [0.0, 0.0, 1.0, 1.0],
        0,
    )];
    let instances = build(&cells, CellCursor::default(), &[]);
    assert_eq!(instances.len(), 1);
    assert_eq!(instances[0].foreground, [1.0, 0.0, 0.0, 1.0]);
    assert_eq!(instances[0].background, [0.0, 0.0, 1.0, 1.0]);
    assert_eq!(instances[0].flags, 0.0);
}

/// SGR 7 反显交换前景与背景。
#[test]
fn reverse_swaps_foreground_background() {
    let cells = vec![cell_data(
        0,
        0,
        'A',
        [1.0, 0.0, 0.0, 1.0],
        [0.0, 0.0, 1.0, 1.0],
        1 << cell_flags::REVERSE,
    )];
    let instances = build(&cells, CellCursor::default(), &[]);
    assert_eq!(instances[0].foreground, [0.0, 0.0, 1.0, 1.0]);
    assert_eq!(instances[0].background, [1.0, 0.0, 0.0, 1.0]);
}

/// SGR 1/3 bold+italic 标志传到 GPU 实例（shader 样式位）。
#[test]
fn bold_italic_flags_reach_instance() {
    let flags = (1 << cell_flags::BOLD) | (1 << cell_flags::ITALIC);
    let cells = vec![cell_data(
        0,
        0,
        'I',
        [1.0, 1.0, 1.0, 1.0],
        [0.0, 0.0, 0.0, 1.0],
        flags,
    )];
    let instances = build(&cells, CellCursor::default(), &[]);
    assert_eq!(
        instances[0].flags, flags as f32,
        "bold+italic flags must reach the instance"
    );
    assert!(
        instances[0].atlas_size[0] > 0.0 && instances[0].atlas_size[1] > 0.0,
        "styled cell must carry a real atlas quad, not a blank fallback"
    );
}

/// SGR 58 下划线颜色传到 GPU 实例，供 shader 装饰遍使用。
#[test]
fn underline_color_reaches_instance() {
    let mut decorated = cell_data(
        0,
        0,
        'U',
        [1.0, 1.0, 1.0, 1.0],
        [0.0, 0.0, 0.0, 1.0],
        1 << cell_flags::UNDERLINE,
    );
    decorated.underline_color = [1.0, 0.0, 0.0, 1.0];
    let instances = build(&[decorated], CellCursor::default(), &[]);
    assert_eq!(
        instances[0].underline_color,
        [1.0, 0.0, 0.0, 1.0],
        "SGR 58 color must reach the instance deco channel"
    );
}

/// alpha >= 128 的搜索高亮先交换前景/背景，再混合背景。
#[test]
fn search_highlight_swaps_and_blends() {
    let cells = vec![cell_data(
        2,
        3,
        'D',
        [1.0, 0.0, 0.0, 1.0],
        [0.0, 0.0, 1.0, 1.0],
        0,
    )];
    let hl = SearchHighlight {
        row: 2,
        start_col: 3,
        end_col_exclusive: 4,
        color: [0xFF, 0xFF, 0x00, 0xFF], // 不透明黄
    };
    let instances = build(&cells, CellCursor::default(), &[hl]);
    // alpha >= 128 → 交换前景/背景，随后 background = blend(background, yellow, alpha=1) = yellow。
    assert_eq!(
        instances[0].foreground,
        [0.0, 0.0, 1.0, 1.0],
        "highlight swaps foreground→background"
    );
    assert_eq!(
        instances[0].background,
        [1.0, 1.0, 0.0, 1.0],
        "blend with opaque yellow"
    );
}

/// alpha 低于 128 的高亮只混合、不交换。
#[test]
fn search_highlight_blends_without_swap() {
    let cells = vec![cell_data(
        0,
        0,
        'E',
        [0.0, 0.0, 0.0, 1.0],
        [1.0, 1.0, 1.0, 1.0],
        0,
    )];
    let hl = SearchHighlight {
        row: 0,
        start_col: 0,
        end_col_exclusive: 1,
        color: [0xFF, 0x00, 0x00, 0x7F], // alpha ~0.5 的红（低于 128 交换阈值）
    };
    let instances = build(&cells, CellCursor::default(), &[hl]);
    assert_eq!(
        instances[0].foreground,
        [0.0, 0.0, 0.0, 1.0],
        "foreground unchanged below alpha 128"
    );
    // background = white * (1-a) + red * a，其中 a = 0x7F/255
    let alpha = GhosttyTerminal::byte_to_float(0x7F);
    let expected = 1.0 - alpha;
    assert!(
        (instances[0].background[0] - 1.0).abs() < 1e-5,
        "red channel keeps base white"
    );
    assert!((instances[0].background[1] - expected).abs() < 1e-5);
    assert!((instances[0].background[2] - expected).abs() < 1e-5);
}

/// 方块光标把背景换成光标色（半透明），
/// 并保留原前景色，使字形仍可读。
#[test]
fn block_cursor_paints_background_keeps_foreground() {
    let cells = vec![cell_data(
        5,
        5,
        'F',
        [1.0, 0.0, 0.0, 1.0],
        [0.0, 0.0, 1.0, 1.0],
        0,
    )];
    let cursor = CellCursor {
        row: 5,
        col: 5,
        visible: true,
        style: CursorStyle::Block,
        color: [0.0, 1.0, 0.0, 1.0],
    };
    let instances = build(&cells, cursor, &[]);
    assert_eq!(
        instances[0].foreground,
        [1.0, 0.0, 0.0, 1.0],
        "block cursor keeps foreground"
    );
    assert_eq!(
        instances[0].background,
        [0.0, 1.0, 0.0, 0.7],
        "background = cursor color * 0.7"
    );
}

/// 方块光标恒为整格矩形（spec cursor-rendering「方块光标按整格几何」）：
/// 不随格内字形变高变矮，也不越过行界。
#[test]
fn block_cursor_quad_is_the_whole_cell() {
    let cell_width = 1024.0 / 80.0;
    let cell_height = 1024.0 / 24.0;
    let cursor = CellCursor {
        row: 0,
        col: 0,
        visible: true,
        style: CursorStyle::Block,
        color: [0.0, 1.0, 0.0, 1.0],
    };
    const FG: [f32; 4] = [1.0, 1.0, 1.0, 1.0];
    const BG: [f32; 4] = [0.0, 0.0, 0.0, 1.0];
    let cases = [
        ("空白", cell_data(0, 0, ' ', FG, BG, 0), 1u32),
        ("码点 0", cell_data(0, 0, '\0', FG, BG, 0), 1),
        ("升部", cell_data(0, 0, 'b', FG, BG, 0), 1),
        ("降部", cell_data(0, 0, 'g', FG, BG, 0), 1),
        ("宽字符", cell_data(0, 0, '中', FG, BG, 0), 2),
    ];
    for (name, mut cell, width) in cases {
        cell.width = width;
        let instances = build(&[cell], cursor, &[]);
        assert_eq!(
            instances[0].quad_origin,
            [0.0, 0.0],
            "{name}: 光标盒顶边即单元顶边"
        );
        assert_eq!(
            instances[0].quad_size,
            [cell_width * width as f32, cell_height],
            "{name}: 光标盒必须等于整格"
        );
    }
}

/// 空单元格（空格）只发背景四边形，绝不发字形实例。
#[test]
fn empty_cell_emits_background_quad() {
    let cells = vec![cell_data(
        0,
        0,
        ' ',
        [0.9, 0.9, 0.9, 1.0],
        [0.1, 0.1, 0.1, 1.0],
        0,
    )];
    let instances = build(&cells, CellCursor::default(), &[]);
    assert_eq!(instances.len(), 1);
    assert_eq!(
        instances[0].atlas_size, [0.0; 2],
        "no glyph UVs for a space"
    );
    assert_eq!(instances[0].background, [0.1, 0.1, 0.1, 1.0]);
}

// 逐字段比较（CellInstance 未派生 PartialEq）。

const TEST_GRID_ROWS: u32 = 24;
const TEST_GRID_COLS: u32 = 80;

/// 缓存构建测试所用的标准 24x80 配置。
fn test_config(cursor: CellCursor) -> CellInstanceConfig<'static> {
    CellInstanceConfig {
        rows: TEST_GRID_ROWS,
        cols: TEST_GRID_COLS,
        grid_cell_width: 1024.0 / TEST_GRID_COLS as f32,
        grid_cell_height: 1024.0 / TEST_GRID_ROWS as f32,
        cursor,
        atlas_width: 1024.0,
        atlas_height: 1024.0,
        search_highlights: &[],
        default_background: [0.0; 4],
    }
}

/// 以标准测试配置执行一次缓存构建。
/// 增量缓存用例的公共夹具：24 行全 `a` 的网格、隐藏光标、1024² 图集与
/// 24×80 缓存，并以一次全脏构建把缓存播种好。
struct SeededCacheBuild {
    cells: Vec<CellData>,
    cursor: CellCursor,
    font_pipeline: crate::render::font::FontPipeline,
    cache: CachedInstances,
    instances: Vec<CellInstance>,
}

fn seeded_cache_build() -> SeededCacheBuild {
    let cells: Vec<CellData> = (0..24)
        .map(|row| cell_data(row, 0, 'a', [1.0, 1.0, 1.0, 1.0], [0.0, 0.0, 0.0, 1.0], 0))
        .collect();
    let cursor = CellCursor {
        row: 0,
        col: 0,
        visible: false,
        style: CursorStyle::Block,
        color: [1.0, 1.0, 1.0, 1.0],
    };
    let mut seeded = SeededCacheBuild {
        cells,
        cursor,
        font_pipeline: crate::render::font::FontPipeline::new(1024, 1024, 14.0),
        cache: CachedInstances::new(24, 80),
        instances: Vec::new(),
    };
    let ok = run_cached_build(
        &seeded.cells,
        seeded.cursor,
        &std::iter::repeat_n(true, 24).collect::<Vec<bool>>(),
        &mut seeded.font_pipeline,
        &mut seeded.cache,
        &mut seeded.instances,
    );
    assert!(ok.is_some(), "initial full build should succeed");
    seeded
}

fn run_cached_build(
    cells: &[CellData],
    cursor: CellCursor,
    mask: &[bool],
    font_pipeline: &mut crate::render::font::FontPipeline,
    cache: &mut CachedInstances,
    instances: &mut Vec<CellInstance>,
) -> Option<()> {
    build_instances_cached(
        cells,
        test_config(cursor),
        font_pipeline,
        mask,
        cache,
        instances,
    )
}

/// 退化输入用例的公共夹具：24 行全 `x` 的网格、隐藏光标与 1024² 字体管线。
/// 缓存与实例由各用例按自身退化组合构造。
fn degraded_grid() -> (Vec<CellData>, CellCursor, crate::render::font::FontPipeline) {
    let cells: Vec<CellData> = (0..24)
        .map(|row| cell_data(row, 0, 'x', [1.0; 4], [0.0, 0.0, 0.0, 1.0], 0))
        .collect();
    let cursor = CellCursor {
        row: 0,
        col: 0,
        visible: false,
        style: CursorStyle::Block,
        color: [1.0, 1.0, 1.0, 1.0],
    };
    let font_pipeline = crate::render::font::FontPipeline::new(1024, 1024, 14.0);
    (cells, cursor, font_pipeline)
}

fn instances_equal(left: &[CellInstance], right: &[CellInstance]) -> bool {
    left.len() == right.len()
        && left
            .iter()
            .zip(right)
            .all(|(left_instance, right_instance)| {
                left_instance.quad_origin == right_instance.quad_origin
                    && left_instance.atlas_offset == right_instance.atlas_offset
                    && left_instance.atlas_size == right_instance.atlas_size
                    && left_instance.foreground == right_instance.foreground
                    && left_instance.background == right_instance.background
                    && left_instance.quad_size == right_instance.quad_size
                    && left_instance.flags == right_instance.flags
                    && left_instance.bearing == right_instance.bearing
            })
}

/// 增量路径（build_instances_cached）：只让第 2 行变脏的一帧
/// 必须产出与完整重建完全一致的实例，其中干净行
/// 从缓存提供（NFR-010：只重绘脏行）。
#[test]
fn cached_incremental_rebuild_matches_full_build() {
    let SeededCacheBuild {
        cells,
        cursor,
        mut font_pipeline,
        mut cache,
        mut instances,
    } = seeded_cache_build();
    let full_frame1 = build(&cells, cursor, &[]);
    assert!(
        instances_equal(&instances, &full_frame1),
        "initial build equals a full build"
    );

    // 第 2 帧：仅第 2 行变化；其余各行必须从
    // 缓存提供，结果仍须等于完整重建。
    let mut cells2 = cells.clone();
    cells2[2] = cell_data(2, 0, 'z', [1.0, 1.0, 1.0, 1.0], [0.0, 0.0, 0.0, 1.0], 0);
    let mut dirty = vec![false; 24];
    dirty[2] = true;
    instances.clear();
    let ok = run_cached_build(
        &cells2,
        cursor,
        &dirty,
        &mut font_pipeline,
        &mut cache,
        &mut instances,
    );
    assert!(ok.is_some(), "incremental build should succeed");
    let full_frame2 = build(&cells2, cursor, &[]);
    assert!(
        instances_equal(&instances, &full_frame2),
        "incremental result must match a full rebuild"
    );
    // 缓存此刻保存第 2 帧的实例供下一帧复用。
    assert!(
        instances_equal(cache.instances(), &instances),
        "cache must be refreshed with the latest instances"
    );
}

/// 图集陈旧（重建或字形驱逐会搬迁 UV）：按更旧图集代际
/// 构建的缓存绝不可提供干净行——该帧必须等于完整
/// 重建，否则陈旧 UV 会渲染出错或空白，
/// 直到各行碰巧重新变脏。
#[test]
fn cached_stale_atlas_generation_forces_full_rebuild() {
    let SeededCacheBuild {
        cells,
        cursor,
        mut font_pipeline,
        mut cache,
        mut instances,
    } = seeded_cache_build();

    // 模拟缓存填充之后的图集重建/驱逐。
    cache.atlas_generation = cache.atlas_generation.wrapping_add(1);

    // 第 2 帧改了第 2 行却声称无脏行：代际为当前时那行陈旧
    // 内容会从缓存被提供；代际陈旧时则必须整帧
    // 改为重建。
    let mut cells2 = cells.clone();
    cells2[2] = cell_data(2, 0, 'z', [1.0, 1.0, 1.0, 1.0], [0.0, 0.0, 0.0, 1.0], 0);
    let clean = vec![false; 24];
    instances.clear();
    let ok = run_cached_build(
        &cells2,
        cursor,
        &clean,
        &mut font_pipeline,
        &mut cache,
        &mut instances,
    );
    assert!(ok.is_some(), "rebuild after atlas change should succeed");
    let full_frame2 = build(&cells2, cursor, &[]);
    assert!(
        instances_equal(&instances, &full_frame2),
        "stale atlas generation must force a full rebuild, not serve cached rows"
    );
}

/// 帧内驱逐回归：小 atlas 上混合旧字形（clean 行）与大量新字形
/// （dirty 行，迫使驱逐）的一帧，增量结果中每个实例的 UV 必须与
/// 当前缓存查询一致。驱逐前构建/服务的旧 UV 即 stale（同字不同区
/// 渲染不一致，如 "d 部分区域像 a"），必须被预热 + 全量降级消除。
#[test]
fn incremental_frame_with_midframe_eviction_has_current_uvs() {
    const ATLAS: f32 = 128.0;
    const ROWS: u32 = 24;
    const COLS_PER_ROW: u32 = 8;
    let pool: Vec<char> = ('A'..='Z')
        .chain('a'..='z')
        .chain('0'..='9')
        .chain("!@#$%^&*()_+-=[]{}|;:,.<>?/~`".chars())
        .collect();
    // 帧工作集（95 ASCII）适配 atlas；帧间 churn 逐出旧条目，
    // 第二帧预热/发射中必再次驱逐，覆盖“帧内驱逐致 stale”路径。
    assert!(pool.len() >= 90, "frame working set must be substantial");
    let make_cell_data = |row: u32, col: u32, character: char, bold: bool| {
        cell_data(
            row,
            col,
            character,
            [1.0, 1.0, 1.0, 1.0],
            [0.0, 0.0, 0.0, 1.0],
            if bold { 1 << cell_flags::BOLD } else { 0 },
        )
    };
    let cursor = CellCursor {
        row: 0,
        col: 0,
        visible: false,
        style: CursorStyle::Block,
        color: [1.0, 1.0, 1.0, 1.0],
    };
    let config = CellInstanceConfig {
        rows: ROWS,
        cols: 80,
        grid_cell_width: 1024.0 / 80.0,
        grid_cell_height: 1024.0 / 24.0,
        cursor,
        atlas_width: ATLAS,
        atlas_height: ATLAS,
        search_highlights: &[],
        default_background: [0.0; 4],
    };
    let mut font_pipeline =
        crate::render::font::FontPipeline::new(ATLAS as i32, ATLAS as i32, 14.0);
    let mut instances = Vec::new();
    let mut cache = CachedInstances::new(ROWS, 80);
    // Frame 1：全 dirty 播种缓存。
    let mut frame1 = Vec::new();
    for row in 0..ROWS {
        for column in 0..COLS_PER_ROW {
            let character = pool[((row * COLS_PER_ROW + column) as usize) % pool.len()];
            frame1.push(make_cell_data(
                row,
                column,
                character,
                (row + column) % 7 == 0,
            ));
        }
    }
    let all_dirty = vec![true; ROWS as usize];
    let ok = build_instances_cached(
        &frame1,
        config,
        &mut font_pipeline,
        &all_dirty,
        &mut cache,
        &mut instances,
    );
    assert!(ok.is_some(), "seed build should succeed");
    // 帧间 churn：直接光栅化 300+ 帧外字形，逐出第一帧条目
    // （DejaVu Sans Mono 覆盖希腊/西里尔/拉丁扩展-A/箭头，
    // 主字体直命中，无慢速全库扫描）。
    // 下一帧预热必须重新载入它们，再次驱逐——stale 场景。
    let churn: Vec<char> = ('\u{0391}'..='\u{03C9}')
        .chain('\u{0410}'..='\u{044F}')
        .chain('\u{0100}'..='\u{017F}')
        .chain('\u{2190}'..='\u{21FF}')
        .collect();
    assert!(churn.len() > 300, "churn set must exceed atlas headroom");
    for &character in &churn {
        let _ = font_pipeline.glyph_information(character);
    }
    // Frame 2：偶数行不变（clean），奇数行换新字形（dirty，迫使驱逐）。
    let mut frame2 = Vec::new();
    for row in 0..ROWS {
        for column in 0..COLS_PER_ROW {
            let index = (row * COLS_PER_ROW + column) as usize;
            let character = if row % 2 == 0 {
                pool[index % pool.len()]
            } else {
                pool[(index + 45) % pool.len()]
            };
            frame2.push(make_cell_data(
                row,
                column,
                character,
                (row + column) % 7 == 0,
            ));
        }
    }
    let mut dirty = vec![false; ROWS as usize];
    for (row, dirty_entry) in dirty.iter_mut().enumerate() {
        *dirty_entry = row % 2 == 1;
    }
    instances.clear();
    let ok = build_instances_cached(
        &frame2,
        config,
        &mut font_pipeline,
        &dirty,
        &mut cache,
        &mut instances,
    );
    assert!(ok.is_some(), "incremental build should succeed");
    assert_eq!(
        instances.len(),
        frame2.len(),
        "each non-empty cell yields exactly one instance (no cursor/highlights)"
    );
    // 逐实例断言 UV 与当前查询一致；验证查询本身必须零变更
    // （纯命中），否则断言失去意义。
    let generation_before_verify = font_pipeline.atlas_generation();
    for (cell, instance) in frame2.iter().zip(instances.iter()) {
        let character = char::from_u32(cell.codepoint).expect("test cells are valid");
        let bold = (cell.flags >> cell_flags::BOLD) & 1 == 1;
        let info = font_pipeline.glyph_information_styled(character, bold, false);
        let info = info.expect("frame glyphs must resolve");
        assert!(
            info.width > 0 && info.height > 0,
            "glyph {character:?} must have a bitmap"
        );
        assert_eq!(
            instance.atlas_offset,
            [info.atlas_x as f32 / ATLAS, info.atlas_y as f32 / ATLAS],
            "stale UV for {character:?} (row {}, col {})",
            cell.row,
            cell.col
        );
        assert_eq!(
            instance.atlas_size,
            [info.width as f32 / ATLAS, info.height as f32 / ATLAS],
            "stale UV size for {character:?} (row {}, col {})",
            cell.row,
            cell.col
        );
    }
    assert_eq!(
        font_pipeline.atlas_generation(),
        generation_before_verify,
        "verification lookups must be pure cache hits"
    );
}

/// 退化输入（缓存网格尺寸陈旧，或脏标记短于网格）
/// 必须回退为完整重建，而不是
/// 提供陈旧行。
#[test]
fn cached_degraded_input_falls_back_to_full_rebuild() {
    let (cells, cursor, mut font_pipeline) = degraded_grid();
    let full = build(&cells, cursor, &[]);

    // 陈旧缓存：按 12 行网格构建，而网格已有 24 行。
    let mut cache = CachedInstances::new(12, 80);
    let mut instances = Vec::new();
    let ok = run_cached_build(
        &cells,
        cursor,
        &[true; 24],
        &mut font_pipeline,
        &mut cache,
        &mut instances,
    );
    assert!(ok.is_some());
    assert!(
        instances_equal(&instances, &full),
        "stale cache must force a full rebuild"
    );

    // 脏标记短于网格（仅 10 行）。
    let mut cache2 = CachedInstances::new(24, 80);
    instances.clear();
    let ok = run_cached_build(
        &cells,
        cursor,
        &[true; 10],
        &mut font_pipeline,
        &mut cache2,
        &mut instances,
    );
    assert!(ok.is_some());
    assert!(
        instances_equal(&instances, &full),
        "a too-short dirty mask must force a full rebuild"
    );
}

/// 回归：与网格不再匹配的缓存（如仅列数变化的 resize，行数
/// 不变）叠加部分脏标记时，仍须产出完整重建。新建的空缓存
/// 看起来「兼容」（rows/cols 字段相同），从中提供「干净」行
/// 会复制 0 个实例，于是把这些行从本帧丢掉。调用方
/// （pass.rs）负责在重建缓存时换成全 true 的脏标记；本测试
/// 钉住这一退化组合，使其永远不会静默通过。
#[test]
fn stale_cache_with_partial_mask_must_not_drop_rows() {
    let (cells, cursor, mut font_pipeline) = degraded_grid();
    let full = build(&cells, cursor, &[]);

    // 模拟 resize 帧：缓存按 24x40 构建，而网格
    // 现为 24x80（仅列数变化）。为 24x80 重建的空缓存
    // 在尺寸上「兼容」，却不含任何行数据。
    let mut cache = CachedInstances::new(24, 80);
    let mut instances = Vec::new();
    // 部分标记：只有第 2 行脏（仅一行字节变化时单元 diff
    // 路径的产出）。
    let mut mask = vec![false; 24];
    mask[2] = true;
    let ok = run_cached_build(
        &cells,
        cursor,
        &mask,
        &mut font_pipeline,
        &mut cache,
        &mut instances,
    );
    assert!(ok.is_some());
    // 每一行都必须在场：空缓存提供「干净」行时
    // 23 个干净行只会产出 0 个实例。
    assert_eq!(
        instances.len(),
        full.len(),
        "partial mask on an empty cache must not drop rows (got {} vs {})",
        instances.len(),
        full.len()
    );
    assert!(
        instances_equal(&instances, &full),
        "result must equal a full rebuild on cache mismatch"
    );
}
/// 竖线光标在空单元格必须是左侧细竖条，而非整格方块。
#[test]
fn bar_cursor_on_empty_cell_is_thin_vertical() {
    let cursor = CellCursor {
        row: 0,
        col: 5,
        visible: true,
        style: crate::terminal::ghostty_terminal::CursorStyle::Bar,
        color: [1.0, 1.0, 1.0, 1.0],
    };
    let cells = vec![cell_data(0, 5, '\0', [1.0; 4], [0.0; 4], 0)];
    let instances = build(&cells, cursor, &[]);
    assert_eq!(instances.len(), 1, "empty bar cursor emits one quad");
    let cell_width = 1024.0 / 80.0;
    let cell_height = 1024.0 / 24.0;
    let width = instances[0].quad_size[0];
    let height = instances[0].quad_size[1];
    let origin_y = instances[0].quad_origin[1];
    assert!(
        width <= cell_width * 0.5,
        "bar width {width} must stay thin within cell {cell_width}"
    );
    assert!(
        width >= CURSOR_MARKER_MINIMUM_THICKNESS - 0.01,
        "bar width {width} must stay visible"
    );
    assert!(
        height >= CURSOR_MARKER_MINIMUM_THICKNESS - 0.01,
        "bar height {height} must stay visible"
    );
    assert!(
        origin_y >= 0.0 && origin_y + height <= cell_height + 0.5,
        "bar spans y [{origin_y}, {}] but row 0 ends at {cell_height}",
        origin_y + height
    );
    assert!(
        (instances[0].quad_origin[0] - 5.0 * cell_width).abs() <= 0.5,
        "bar must sit at the cell left edge"
    );
}
/// 下划线光标在空单元格必须是底部细横条，而非整格方块。
#[test]
fn underline_cursor_on_empty_cell_is_thin_horizontal() {
    let cursor = CellCursor {
        row: 0,
        col: 7,
        visible: true,
        style: crate::terminal::ghostty_terminal::CursorStyle::Underline,
        color: [1.0, 1.0, 1.0, 1.0],
    };
    let cells = vec![cell_data(0, 7, '\0', [1.0; 4], [0.0; 4], 0)];
    let instances = build(&cells, cursor, &[]);
    assert_eq!(instances.len(), 1, "empty underline cursor emits one quad");
    let cell_width = 1024.0 / 80.0;
    let cell_height = 1024.0 / 24.0;
    let width = instances[0].quad_size[0];
    let height = instances[0].quad_size[1];
    assert!(
        (width - cell_width).abs() <= 0.5,
        "underline width {width} must span the cell {cell_width}"
    );
    assert!(
        height <= cell_height * 0.5,
        "underline height {height} must stay thin within cell {cell_height}"
    );
    let bottom = instances[0].quad_origin[1] + height;
    assert!(
        bottom <= cell_height + 0.5,
        "underline bottom {bottom} must stay inside row 0 ({cell_height})"
    );
}
/// 空白网格不再为每格发射背景四边形，实例数只随有效字符增长。
#[test]
fn blank_default_cells_do_not_emit_instances() {
    let mut cells = vec![cell_data(0, 0, 'A', [1.0; 4], [0.0; 4], 0)];
    for col in 1..80 {
        cells.push(cell_data(0, col, '\0', [1.0; 4], [0.0; 4], 0));
    }
    let instances = build(&cells, CellCursor::default(), &[]);
    assert_eq!(
        instances.len(),
        1,
        "only the glyph cell should emit an instance"
    );
}

/// 空白格只有与清除色一致且无光标/装饰时才可省略背景四边形。
#[test]
fn blank_cell_quad_skipped_only_when_clear_color_and_undecorated() {
    let background = [0.1, 0.2, 0.3, 1.0];
    assert!(!blank_cell_needs_quad(0, false, background, background));
    assert!(blank_cell_needs_quad(0, true, background, background));
    assert!(blank_cell_needs_quad(
        1 << cell_flags::UNDERLINE,
        false,
        background,
        background
    ));
    assert!(blank_cell_needs_quad(
        0,
        false,
        [0.1, 0.2, 0.4, 1.0],
        background
    ));
}

/// 非空单元格竖线光标：字形保持原文色并追加一枚标记。
#[test]
fn bar_cursor_on_glyph_cell_emits_glyph_plus_marker() {
    let cursor = CellCursor {
        row: 0,
        col: 0,
        visible: true,
        style: crate::terminal::ghostty_terminal::CursorStyle::Bar,
        color: [1.0, 1.0, 1.0, 1.0],
    };
    let cells = vec![cell_data(
        0,
        0,
        'A',
        [1.0, 0.0, 0.0, 1.0],
        [0.0, 0.0, 0.0, 1.0],
        0,
    )];
    let instances = build(&cells, cursor, &[]);
    assert_eq!(
        instances.len(),
        2,
        "bar cursor on a glyph emits the glyph plus one marker"
    );
    assert_eq!(
        instances[0].foreground,
        [1.0, 0.0, 0.0, 1.0],
        "glyph keeps its own foreground under a bar cursor"
    );
    assert_eq!(
        instances[0].background,
        [0.0, 0.0, 0.0, 1.0],
        "glyph keeps its own background under a bar cursor"
    );
    assert_eq!(
        instances[1].atlas_size,
        [0.0, 0.0],
        "marker is a solid color quad without glyph content"
    );
    assert!(
        instances[1].quad_size[0] <= instances[0].quad_size[0] * 0.5,
        "marker must stay thin relative to the glyph quad"
    );
}
