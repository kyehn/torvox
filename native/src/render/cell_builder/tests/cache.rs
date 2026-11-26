use super::*;

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

/// 增量路径（`build_instances_cached`）：只让第 2 行变脏的一帧
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
    let mut cells2 = cells;
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
    let mut cells2 = cells;
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

/// 图集必须装得下**一帧的工作集**，但装不下「工作集 + churn」。
///
/// 上限来自「收敛」而非「驱逐」：这一用例要验的是帧内搬迁之后 UV 仍然一致，
/// 而工作集一旦本身装不进图集，每一遍发射都会把另一个字形挤出去——遍数再多也不
/// 可能收敛（此时正确的行为是 [`build_row_instances_into`] 放弃写缓存并告警，
/// 而不是反复重建）。故取 160：实测 churn 在此推进约 9 次搬迁、第二帧发射再
/// 推进约 3 次，既真的脏到会在帧内搬迁，又留得住本帧工作集。
/// 调大前先看下面的 churn / 帧间代际断言：两者同时成立才说明场景有效。
const THRASHING_ATLAS: f32 = 160.0;

/// 帧内驱逐回归：小 atlas 上混合旧字形（clean 行）与大量新字形
/// （dirty 行，迫使驱逐）的一帧，增量结果中每个实例的 UV 必须与
/// 当前缓存查询一致。驱逐前构建/服务的旧 UV 即 stale（同字不同区
/// 渲染不一致，如 "d 部分区域像 a"），必须被预热 + 全量降级消除。
#[test]
fn incremental_frame_with_midframe_eviction_has_current_uvs() {
    const ATLAS: f32 = THRASHING_ATLAS;
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
    let generation_before_frame1 = font_pipeline.atlas_generation();
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
    let generation_before_churn = font_pipeline.atlas_generation();
    for &character in &churn {
        let _ = font_pipeline.glyph_information(character);
    }
    let generation_after_churn = font_pipeline.atlas_generation();
    // 场景前提：churn 必须真的把第一帧的字形逐出图集。不推进即 ATLAS 太大，
    // 第二帧不会在帧内搬迁，本用例就退化成「干净行照抄」的平凡用例。
    assert!(
        generation_after_churn > generation_before_churn.max(generation_before_frame1),
        "churn set must evict the first frame's glyphs (generation {generation_before_churn} -> {generation_after_churn})"
    );
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
    let generation_before_frame2 = font_pipeline.atlas_generation();
    let ok = build_instances_cached(
        &frame2,
        config,
        &mut font_pipeline,
        &dirty,
        &mut cache,
        &mut instances,
    );
    assert!(ok.is_some(), "incremental build should succeed");
    // 场景前提（之二）：第二帧内部必须有字形搬迁。代际不动即 churn 没有
    // 真的制造帧内驱逐，本用例就没有碰到它要验的那条路径。
    assert!(
        font_pipeline.atlas_generation() > generation_before_frame2,
        "frame 2 must relocate glyphs mid-frame (generation stuck at {generation_before_frame2})"
    );
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

/// 24×80 预热用例的共用夹具：14px 字体决定的单元格尺寸、隐藏光标、空白网格。
///
/// 网格内容由 [character] 逐格决定；前景白、背景黑、无样式位。
fn warm_fixture(
    character: impl Fn(u32, u32) -> char,
) -> (
    crate::render::font::FontPipeline,
    CellInstanceConfig<'static>,
    Vec<CellData>,
) {
    let mut font_pipeline = crate::render::font::FontPipeline::new(1024, 1024, 14.0);
    let (cell_width, cell_height) = font_pipeline.cell_metrics();
    let config = CellInstanceConfig {
        rows: TEST_GRID_ROWS,
        cols: TEST_GRID_COLS,
        grid_cell_width: cell_width,
        grid_cell_height: cell_height,
        cursor: CellCursor {
            row: 0,
            col: 0,
            visible: false,
            style: crate::terminal::ghostty_terminal::CursorStyle::Block,
            color: [1.0; 4],
        },
        atlas_width: 1024.0,
        atlas_height: 1024.0,
        search_highlights: &[],
        default_background: [0.0; 4],
    };
    let mut cells = Vec::with_capacity((TEST_GRID_ROWS * TEST_GRID_COLS) as usize);
    for row in 0..TEST_GRID_ROWS {
        for col in 0..TEST_GRID_COLS {
            cells.push(cell_data(
                row,
                col,
                character(row, col),
                [1.0; 4],
                [0.0, 0.0, 0.0, 1.0],
                0,
            ));
        }
    }
    (font_pipeline, config, cells)
}

/// 增量帧的预热只覆盖脏行。
///
/// 干净行的字形早已在图集里——缓存代际一致即证明它们没被驱逐、UV 也没搬迁——
/// 预热整屏纯属白费：实测整屏预热 68µs，而一行脏行的重建只要几微秒。
/// 判别力在计数：断言预热只访问了脏行的单元数。只断言输出相等只能证明
/// 「结果对」，证不了「没白扫整屏」——预热范围退化成整屏时输出照样全对。
#[test]
fn incremental_frame_warms_only_dirty_rows() {
    let (mut font_pipeline, config, cells) = warm_fixture(|_, _| 'x');
    let mut cache = CachedInstances::new(TEST_GRID_ROWS, TEST_GRID_COLS);
    let mut instances = Vec::new();
    // 播种：首帧没有基线，整屏重建一次。
    assert!(
        build_instances_cached(
            &cells,
            config,
            &mut font_pipeline,
            &vec![true; TEST_GRID_ROWS as usize],
            &mut cache,
            &mut instances,
        )
        .is_some(),
        "播种构建失败"
    );

    // 第二帧：只有第 2 行变脏。
    let mut dirty = vec![false; TEST_GRID_ROWS as usize];
    dirty[2] = true;
    let before = warm_cells_visited();
    assert!(
        build_instances_cached(
            &cells,
            config,
            &mut font_pipeline,
            &dirty,
            &mut cache,
            &mut instances,
        )
        .is_some(),
        "增量构建失败"
    );
    let visited = warm_cells_visited() - before;
    assert!(
        visited <= u64::from(TEST_GRID_COLS),
        "增量帧的预热只应覆盖脏行（实测访问了 {visited} 个单元，全屏是 {} 个）",
        u64::from(TEST_GRID_ROWS) * u64::from(TEST_GRID_COLS)
    );
}

/// 预热遍按字形去重：同一字形在一帧里只请求一次。
///
/// 一屏 1920 单元通常只含几十种字形。逐格请求看着「只是查表」，但每次请求都要
/// 走图集分配器的查找与 LRU 记账；重复字形占绝大多数时，那正是预热遍的主要开销。
/// 判据是**请求数**而非单元数：单元数只说明扫了多少格，请求数才说明做了多少事。
#[test]
fn warm_frame_deduplicates_repeated_glyphs() {
    // 三个不同的字形各自重复满屏。
    let (mut font_pipeline, config, cells) =
        warm_fixture(|row, col| ['x', 'y', 'z'][((row + col) % 3) as usize]);
    let mut cache = CachedInstances::new(TEST_GRID_ROWS, TEST_GRID_COLS);
    let mut instances = Vec::new();
    let before = warm_glyph_requests();
    assert!(
        build_instances_cached(
            &cells,
            config,
            &mut font_pipeline,
            &vec![true; TEST_GRID_ROWS as usize],
            &mut cache,
            &mut instances,
        )
        .is_some(),
        "全量构建失败"
    );
    let requests = warm_glyph_requests() - before;
    assert_eq!(
        requests,
        3,
        "预热遍只应按去重后的字形数发请求（实测 {requests} 次，全屏是 {} 个单元）",
        u64::from(TEST_GRID_ROWS) * u64::from(TEST_GRID_COLS)
    );
}

/// 驱逐场景的网格尺寸：2 行 16 列。
const EVICTION_ROWS: u32 = 2;
const EVICTION_COLS: u32 = 16;

/// 驱逐场景的图集边长（像素）。
const EVICTION_ATLAS: f32 = 64.0;

/// 「预热触发驱逐」的增量帧场景夹具。
///
/// 图集先被塞满（光栅化不再有空间时才会驱逐），帧 1 播种缓存，帧 2 只让
/// 第 1 行变脏且换成图集里没有的字形：预热这些字形必然驱逐旧条目，
/// 缓存行引用的旧 UV 随之失效，增量路径因而必须降级为全量重建。
struct EvictionFixture {
    font_pipeline: crate::render::font::FontPipeline,
    cache: CachedInstances,
    instances: Vec<CellInstance>,
    frame: Vec<CellData>,
    /// 帧 2 构建前预热遍访问过的单元数。
    warm_before: u64,
}

/// 构造 [`EvictionFixture`]。
///
/// 场景前提（图集已满、帧 2 起步时缓存仍有效、帧 2 的预热确实驱逐）
/// 在此就地断言：前提不成立时降级分支根本进不去，失败须在此处报出，
/// 而不是让调用方的断言给出误导性的结论。
fn eviction_fixture() -> EvictionFixture {
    let mut font_pipeline =
        crate::render::font::FontPipeline::new(EVICTION_ATLAS as i32, EVICTION_ATLAS as i32, 14.0);
    // 先把图集塞满：光栅化不再有空间时才会驱逐，驱逐即图集已满的信号。
    let mut atlas_is_full = false;
    let generation = font_pipeline.atlas_generation();
    for character in ('a'..='z').chain('α'..='ω').chain('а'..='я') {
        let _ = font_pipeline.glyph_information(character);
        if font_pipeline.atlas_generation() != generation {
            atlas_is_full = true;
            break;
        }
    }
    assert!(atlas_is_full, "场景前提：图集必须先被塞满（未观察到驱逐）");
    let cursor = CellCursor {
        row: 0,
        col: 0,
        visible: false,
        style: crate::terminal::ghostty_terminal::CursorStyle::Block,
        color: [1.0; 4],
    };
    let config = CellInstanceConfig {
        rows: EVICTION_ROWS,
        cols: EVICTION_COLS,
        grid_cell_width: EVICTION_ATLAS / EVICTION_COLS as f32,
        grid_cell_height: EVICTION_ATLAS / EVICTION_ROWS as f32,
        cursor,
        atlas_width: EVICTION_ATLAS,
        atlas_height: EVICTION_ATLAS,
        search_highlights: &[],
        default_background: [0.0; 4],
    };
    let make_row = |row: u32, characters: &[char]| -> Vec<CellData> {
        characters
            .iter()
            .enumerate()
            .map(|(col, &character)| {
                cell_data(
                    row,
                    col as u32,
                    character,
                    [1.0; 4],
                    [0.0, 0.0, 0.0, 1.0],
                    0,
                )
            })
            .collect()
    };
    // 帧 1：整屏 'x'（只一个字形），全脏重建。
    let frame1: Vec<CellData> = make_row(0, &['x'; 16])
        .into_iter()
        .chain(make_row(1, &['x'; 16]))
        .collect();
    let mut cache = CachedInstances::new(EVICTION_ROWS, EVICTION_COLS);
    let mut instances = Vec::new();
    assert!(
        build_instances_cached(
            &frame1,
            config,
            &mut font_pipeline,
            &vec![true; EVICTION_ROWS as usize],
            &mut cache,
            &mut instances,
        )
        .is_some(),
        "播种构建失败"
    );

    // 帧 2 起步时缓存必须仍然有效（代际一致）：否则增量路径根本进不去，
    // 降级分支也就没被覆盖。
    assert_eq!(
        cache.built_atlas_generation(),
        font_pipeline.atlas_generation(),
        "场景前提：帧 2 起步时缓存必须仍然有效"
    );

    // 帧 2：只有第 1 行变脏，且换成图集里没有的字形（大写希腊字母）。
    // 预热它们必然触发驱逐 → 增量路径必须降级为全量重建。
    let frame: Vec<CellData> = make_row(0, &['x'; 16])
        .into_iter()
        .chain(make_row(
            1,
            &[
                'Α', 'Β', 'Γ', 'Δ', 'Ε', 'Ζ', 'Η', 'Θ', 'Ι', 'Κ', 'Λ', 'Μ', 'Ν', 'Ξ', 'Ο', 'Π',
            ],
        ))
        .collect();
    let mut dirty = vec![false; EVICTION_ROWS as usize];
    dirty[1] = true;
    let generation_before = font_pipeline.atlas_generation();
    let warm_before = warm_cells_visited();
    assert!(
        build_instances_cached(
            &frame,
            config,
            &mut font_pipeline,
            &dirty,
            &mut cache,
            &mut instances,
        )
        .is_some(),
        "增量构建失败"
    );
    assert!(
        font_pipeline.atlas_generation() > generation_before,
        "场景前提：新字形的光栅化必须真的驱逐了图集条目（代际 {generation_before} -> {}）",
        font_pipeline.atlas_generation()
    );
    EvictionFixture {
        font_pipeline,
        cache,
        instances,
        frame,
        warm_before,
    }
}

/// 增量帧的预热触发驱逐时，必须降级为全量重建。
///
/// 预热把字形挤出图集时，缓存行引用的旧 UV 已失效；此时若仍按「干净行照抄
/// 缓存」出帧，画面上就是同一个字符在两个区域画出不同轮廓。降级路径
/// （预热驱逐 → 全量重建）必须把整帧都预热一遍（发射遍才可能是纯查表）。
#[test]
fn incremental_frame_falls_back_to_full_rebuild_when_warm_evicts() {
    let fixture = eviction_fixture();
    // 降级判据：预热触发驱逐后必须把整帧都预热一遍。只预热脏行的话，
    // 干净行的字形仍可能被驱逐，帧内 UV 就会陈旧。
    let warmed = warm_cells_visited() - fixture.warm_before;
    assert!(
        warmed >= u64::from(EVICTION_ROWS * EVICTION_COLS),
        "预热触发驱逐后必须降级为整帧预热（实测只预热了 {warmed} 个单元，整帧是 {} 个）",
        u64::from(EVICTION_ROWS * EVICTION_COLS)
    );
    assert_eq!(
        fixture.cache.built_atlas_generation(),
        fixture.font_pipeline.atlas_generation(),
        "降级为全量重建后，缓存必须记下当前代际"
    );
}

/// 降级后的每个实例的 UV 都必须与当前查询一致：陈旧 UV 即「同字不同区」。
#[test]
fn incremental_frame_uv_matches_atlas_after_eviction() {
    let mut fixture = eviction_fixture();
    let generation_before_verify = fixture.font_pipeline.atlas_generation();
    for (cell, instance) in fixture.frame.iter().zip(fixture.instances.iter()) {
        let character = char::from_u32(cell.codepoint).expect("测试单元码位合法");
        let info = fixture
            .font_pipeline
            .glyph_information(character)
            .expect("帧内字形必须解析得出");
        assert_eq!(
            instance.atlas_offset,
            [
                info.atlas_x as f32 / EVICTION_ATLAS,
                info.atlas_y as f32 / EVICTION_ATLAS
            ],
            "陈旧 UV：{character:?}"
        );
        assert_eq!(
            instance.atlas_size,
            [
                info.width as f32 / EVICTION_ATLAS,
                info.height as f32 / EVICTION_ATLAS
            ],
            "陈旧 UV 尺寸：{character:?}"
        );
    }
    assert_eq!(
        fixture.font_pipeline.atlas_generation(),
        generation_before_verify,
        "校验用的查询必须是纯缓存命中"
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
