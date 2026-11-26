//! Criterion trend benchmarks: `CellData` -> `CellInstance` conversion.
//!
//! Complements the in-test `bench_build_instances_from_cell_data`
//! assertion benchmark. This one measures the pure CPU conversion path
//! with realistic mixed content (ASCII/CJK/colors/bold/italic):
//!
//! ```text
//! cargo bench --manifest-path native/Cargo.toml -p native --bench cell_builder
//! ```

mod support;

use criterion::{Criterion, criterion_group, criterion_main};
use native::render::cell_builder::CellInstanceConfig;
use native::render::font::FontPipeline;
use native::terminal::ghostty_terminal::CellData;
use std::hint::black_box;

/// 一屏混合内容：ASCII/CJK/颜色/粗斜俱全。
const MIXED_DATA: [(char, u32, [f32; 4], [f32; 4], u32); 8] = [
    ('A', 1, [0.9, 0.9, 0.9, 1.0], [0.1, 0.1, 0.1, 1.0], 0),
    (' ', 1, [0.9, 0.9, 0.9, 1.0], [0.1, 0.1, 0.1, 1.0], 0),
    ('中', 2, [1.0, 0.8, 0.2, 1.0], [0.05, 0.05, 0.1, 1.0], 0),
    ('W', 1, [0.3, 0.8, 1.0, 1.0], [0.1, 0.1, 0.1, 1.0], 1),
    ('i', 1, [0.5, 1.0, 0.5, 1.0], [0.1, 0.1, 0.1, 1.0], 2),
    ('e', 1, [0.9, 0.9, 0.9, 1.0], [0.2, 0.0, 0.0, 1.0], 4),
    ('█', 1, [0.6, 0.6, 0.6, 1.0], [0.15, 0.15, 0.15, 1.0], 0),
    ('~', 1, [0.4, 0.4, 0.4, 1.0], [0.1, 0.1, 0.1, 1.0], 8),
];

/// 增量帧里唯一变脏的行。
const DIRTY_ROW: u32 = 12;

/// 造一屏混合内容的单元数据。
///
/// `shifted_row` 给出「哪一行整体换一种字符」：增量基准用它造出与基线
/// 只差一行的第二帧。
fn mixed_cells(rows: u32, cols: u32, shifted_row: Option<(u32, usize)>) -> Vec<CellData> {
    let count = (rows * cols) as usize;
    (0..count)
        .map(|i| {
            let row = (i / cols as usize) as u32;
            let shift = match shifted_row {
                Some((target_row, shift)) if target_row == row => shift,
                _ => 0,
            };
            let (ch, w, foreground, background, fl) = MIXED_DATA[(i + shift) % MIXED_DATA.len()];
            CellData {
                codepoint: ch as u32,
                width: w,
                grapheme_extra: [0; 7],
                foreground,
                background,
                underline_color: foreground,
                flags: fl,
                row,
                col: (i % cols as usize) as u32,
            }
        })
        .collect()
}

fn build_instances(c: &mut Criterion) {
    let mut font_pipeline = FontPipeline::new(2048, 2048, 14.0);
    let (rows, cols) = (24u32, 80u32);
    let raster_scale = font_pipeline.get_raster_scale();
    let (metric_width, metric_height) = font_pipeline.cell_metrics();
    let grid_cell_width = metric_width * raster_scale;
    let grid_cell_height = metric_height * raster_scale;

    let cell_data = mixed_cells(rows, cols, None);

    let cursor = native::render::CellCursor {
        row: 12,
        col: 40,
        visible: true,
        style: native::terminal::CursorStyle::Block,
        color: [1.0, 1.0, 1.0, 1.0],
    };

    // 断言与采样共用同一份配置：两处各写一份时，字段增减会只改到其中一处，
    // 断言量到的路径与基准量到的路径便不再相同。
    let config = CellInstanceConfig {
        rows,
        cols,
        grid_cell_width,
        grid_cell_height,
        cursor,
        atlas_width: 2048.0,
        atlas_height: 2048.0,
        search_highlights: &[],
        default_background: [0.0; 4],
    };

    // 下限断言：只兜住数量级退化（逐格重建字体、O(n²) 查表），不判绝对快慢。
    //
    // 上限按 **CI 实测**校准（CI runner 2 核，比开发机慢一截）。修掉「全库扫描
    // 结论没被复用」那个缺陷前，本用例稳态就要 750ms（CI 878ms）——那不是构建器
    // 的成本，是同一个码位每格重扫一遍全库。修复后本地实测 0.26ms（[floor] 输出），
    // 按 CI 约 3 倍估 0.8ms，取 25 倍余量 = 20ms：
    //  - 回归到「每格重扫全库」是 740ms 量级，必然判红；
    //  - 25 倍余量足以吸收 CI 调度抖动与字体集差异。
    support::Floor::new("build_instances_from_cell_data", 20.0).assert_within(16, || {
        let mut instances = Vec::new();
        black_box(native::render::build_instances_from_cell_data(
            &cell_data,
            config,
            &mut font_pipeline,
            &mut instances,
        ));
        black_box(instances.len());
    });

    c.bench_function("build_instances_from_cell_data", |b| {
        b.iter(|| {
            let mut instances = Vec::new();
            black_box(native::render::build_instances_from_cell_data(
                &cell_data,
                config,
                &mut font_pipeline,
                &mut instances,
            ));
            black_box(instances.len());
        });
    });
}

/// 增量帧基准：一行变脏时的构建耗时。
///
/// 生产路径每来一次输出就走这条（脏掩码 + 行级实例缓存）：此时只有脏行需要
/// 重建，干净行原样复制缓存里的实例。若构建器仍逐帧遍历整屏做预热，
/// 增量帧的耗时就由「预热整屏」而不是「重建脏行」决定——这是生产路径上
/// 占比最大的一段固定开销。
fn build_instances_incremental(c: &mut Criterion) {
    let mut font_pipeline = FontPipeline::new(2048, 2048, 14.0);
    let (rows, cols) = (24u32, 80u32);
    let raster_scale = font_pipeline.get_raster_scale();
    let (metric_width, metric_height) = font_pipeline.cell_metrics();
    let grid_cell_width = metric_width * raster_scale;
    let grid_cell_height = metric_height * raster_scale;

    let cells_a = mixed_cells(rows, cols, None);
    let cells_b = mixed_cells(rows, cols, Some((DIRTY_ROW, 1)));

    let cursor = native::render::CellCursor {
        row: DIRTY_ROW,
        col: 40,
        visible: true,
        style: native::terminal::CursorStyle::Block,
        color: [1.0, 1.0, 1.0, 1.0],
    };
    let config = CellInstanceConfig {
        rows,
        cols,
        grid_cell_width,
        grid_cell_height,
        cursor,
        atlas_width: 2048.0,
        atlas_height: 2048.0,
        search_highlights: &[],
        default_background: [0.0; 4],
    };

    let mut cache = native::render::cell_builder::CachedInstances::new(rows, cols);
    let mut instances = Vec::new();
    let mut dirty = vec![false; rows as usize];
    dirty[DIRTY_ROW as usize] = true;

    // 播种：首帧没有基线，整屏重建一次（生产首帧同样如此）。
    assert!(
        native::render::cell_builder::build_instances_cached(
            &cells_a,
            config,
            &mut font_pipeline,
            &vec![true; rows as usize],
            &mut cache,
            &mut instances,
        )
        .is_some(),
        "增量基准的播种构建失败"
    );

    let mut flip = false;
    // 下限断言：只兜数量级崩塌（增量帧退化成整屏重建是 0.13ms 量级，
    // 退化成逐格重扫字体库是毫秒量级）。阈值按实测留两个数量级余量，
    // 不因机器差异判红；「增量帧不得遍历整屏」由单测的预热计数钉住。
    support::Floor::new("build_instances_cached_one_dirty_row", 1.0).assert_within(16, || {
        build_one_dirty_row(
            &cells_a,
            &cells_b,
            config,
            &dirty,
            &mut font_pipeline,
            &mut cache,
            &mut instances,
            &mut flip,
        );
    });

    c.bench_function("build_instances_cached_one_dirty_row", |b| {
        b.iter(|| {
            build_one_dirty_row(
                &cells_a,
                &cells_b,
                config,
                &dirty,
                &mut font_pipeline,
                &mut cache,
                &mut instances,
                &mut flip,
            );
        });
    });
}

/// 交替用两份只差一行的单元数据构建：每轮只有 [`DIRTY_ROW`] 变脏。
fn build_one_dirty_row(
    cells_a: &[CellData],
    cells_b: &[CellData],
    config: CellInstanceConfig<'_>,
    dirty: &[bool],
    font_pipeline: &mut FontPipeline,
    cache: &mut native::render::cell_builder::CachedInstances,
    instances: &mut Vec<native::render::CellInstance>,
    flip: &mut bool,
) {
    let cells = if *flip { cells_b } else { cells_a };
    *flip = !*flip;
    black_box(native::render::cell_builder::build_instances_cached(
        cells,
        config,
        font_pipeline,
        dirty,
        cache,
        instances,
    ));
}

criterion_group!(benches, build_instances, build_instances_incremental);
criterion_main!(benches);
