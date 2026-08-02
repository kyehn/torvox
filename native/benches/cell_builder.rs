//! Criterion trend benchmarks: CellData -> CellInstance conversion.
//!
//! Complements the in-test `bench_build_instances_from_cell_data`
//! assertion benchmark. This one measures the pure CPU conversion path
//! with realistic mixed content (ASCII/CJK/colors/bold/italic):
//!
//! ```text
//! cargo bench -p native --bench cell_builder
//! ```

use criterion::{Criterion, criterion_group, criterion_main};
use native::render::font::FontPipeline;
use native::terminal::ghostty_terminal::CellData;
use std::hint::black_box;

fn build_instances(c: &mut Criterion) {
    let mut font_pipeline = FontPipeline::new(1024, 1024, 14.0);
    let (rows, cols) = (24u32, 80u32);
    let count = (rows * cols) as usize;

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
            let (ch, w, fg, bg, fl) = mixed_data[i % mixed_data.len()];
            CellData {
                codepoint: ch as u32,
                width: w,
                grapheme_extra: [0; 7],
                fg_color: fg,
                bg_color: bg,
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
        color: None,
    };

    c.bench_function("build_instances_from_cell_data", |b| {
        b.iter(|| {
            let mut instances = Vec::new();
            black_box(native::render::build_instances_from_cell_data(
                &cell_data,
                rows,
                cols,
                cursor,
                &mut font_pipeline,
                1024.0,
                1024.0,
                None,
                None,
                &[],
                &mut instances,
            ));
            black_box(instances.len());
        })
    });
}

criterion_group!(benches, build_instances);
criterion_main!(benches);
