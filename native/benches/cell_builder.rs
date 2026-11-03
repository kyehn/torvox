//! Criterion trend benchmarks: CellData -> CellInstance conversion.
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

fn build_instances(c: &mut Criterion) {
    let mut font_pipeline = FontPipeline::new(2048, 2048, 14.0);
    let (rows, cols) = (24u32, 80u32);
    let count = (rows * cols) as usize;
    let raster_scale = font_pipeline.get_raster_scale();
    let (metric_width, metric_height) = font_pipeline.cell_metrics();
    let grid_cell_width = metric_width * raster_scale;
    let grid_cell_height = metric_height * raster_scale;

    let mixed_data = [
        ('A', 1, [0.9, 0.9, 0.9, 1.0], [0.1, 0.1, 0.1, 1.0], 0),
        (' ', 1, [0.9, 0.9, 0.9, 1.0], [0.1, 0.1, 0.1, 1.0], 0),
        ('中', 2, [1.0, 0.8, 0.2, 1.0], [0.05, 0.05, 0.1, 1.0], 0),
        ('W', 1, [0.3, 0.8, 1.0, 1.0], [0.1, 0.1, 0.1, 1.0], 1),
        ('i', 1, [0.5, 1.0, 0.5, 1.0], [0.1, 0.1, 0.1, 1.0], 2),
        ('e', 1, [0.9, 0.9, 0.9, 1.0], [0.2, 0.0, 0.0, 1.0], 4),
        ('█', 1, [0.6, 0.6, 0.6, 1.0], [0.15, 0.15, 0.15, 1.0], 0),
        ('~', 1, [0.4, 0.4, 0.4, 1.0], [0.1, 0.1, 0.1, 1.0], 8),
    ];

    let cell_data: Vec<CellData> = (0..count)
        .map(|i| {
            let (ch, w, foreground, background, fl) = mixed_data[i % mixed_data.len()];
            CellData {
                codepoint: ch as u32,
                width: w,
                grapheme_extra: [0; 7],
                foreground,
                background,
                underline_color: foreground,
                flags: fl,
                row: (i / cols as usize) as u32,
                col: (i % cols as usize) as u32,
            }
        })
        .collect();

    let cursor = native::render::CellCursor {
        row: 12,
        col: 40,
        visible: true,
        style: native::terminal::CursorStyle::Block,
        color: [1.0, 1.0, 1.0, 1.0],
    };

    // 下限断言：只兜住数量级退化（逐格重建字体、O(n²) 查表），不判绝对快慢。
    // 上限必须按 **CI 实测**校准：CI runner（2 核慢 CPU）上全量重建 1920 格的
    // 稳态实测约 750ms，与开发机相差一个数量级，按本地体感拍的 50ms 永远判红。
    // 2s = 实测的 ~2.7 倍余量，仍能挡住 O(n²)（那将是 50 倍以上的崩塌）。
    // CI 实测 878ms（[floor] 输出），1.5 倍余量；超出说明网格重建明显劣化。
    support::Floor::new("build_instances_from_cell_data", 1300.0).assert_within(16, || {
        let mut instances = Vec::new();
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
            black_box(native::render::build_instances_from_cell_data(
                &cell_data,
                config,
                &mut font_pipeline,
                &mut instances,
            ));
            black_box(instances.len());
        })
    });
}

criterion_group!(benches, build_instances);
criterion_main!(benches);
