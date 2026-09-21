//! CJK 解析稳态开销：缓存预热后重复解析同一组汉字，度量热路径单字成本。
//!
//! 开发期分析用（`cargo bench -p native --bench cjk_resolve`），
//! 不进门禁：机器字体环境不同，绝对数值不可比，只看同机前后对比。

use criterion::{Criterion, criterion_group, criterion_main};
use native::render::font::FontPipeline;
use std::hint::black_box;

fn cjk_resolve_steady_state(criterion: &mut Criterion) {
    let mut pipeline = FontPipeline::new(512, 512, 16.0);
    assert!(
        pipeline.set_font_family("DejaVu Sans Mono"),
        "宿主必须有主字体"
    );
    pipeline.set_system_locale("");
    let line: Vec<char> = "中文测试字体渲染速度".chars().collect();
    for glyph in &line {
        pipeline.glyph_information(*glyph);
    }
    criterion.bench_function("cjk_resolve_warm_per_line", |holder| {
        holder.iter(|| {
            for glyph in &line {
                black_box(pipeline.glyph_information(black_box(*glyph)));
            }
        })
    });
}

criterion_group!(benches, cjk_resolve_steady_state);
criterion_main!(benches);
