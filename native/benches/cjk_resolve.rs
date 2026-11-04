//! CJK 解析稳态开销：缓存预热后重复解析同一组汉字，度量热路径单字成本。
//!
//! 开发期分析用（`cargo bench --manifest-path native/Cargo.toml -p native --bench cjk_resolve`）：
//! 机器字体环境不同，绝对数值不可比，只看同机前后对比。**仍带下限断言**——
//! 热路径单字解析本该是纳秒级，超过 1ms/行说明缓存已失效或每字都在全库扫描。

mod support;

use criterion::{Criterion, criterion_group, criterion_main};
use native::render::font::FontPipeline;
use std::hint::black_box;

fn cjk_resolve_steady_state(criterion: &mut Criterion) {
    let mut pipeline = FontPipeline::new(512, 512, 16.0);
    assert!(
        pipeline.set_font_family("DejaVu Sans Mono", &[]),
        "宿主必须有主字体"
    );
    pipeline.set_system_locale("");
    let line: Vec<char> = "中文测试字体渲染速度".chars().collect();
    for glyph in &line {
        pipeline.glyph_information(*glyph);
    }
    // 上限按 **CI 实测**校准。这条基准的稳态成本原先是 18.7ms/行（CI [floor] 输出），
    // 那是「解析结论没被复用」那个缺陷的代价——同一行汉字每解析一个字就重扫一遍
    // 全库。修复后本地实测 0.0005ms/行（[floor] 输出），按 CI 约 3 倍估 0.002ms，
    // 取 500 倍余量 = 1ms：
    //  - 回归到「每字重扫全库」是 18.7ms 量级，必然判红；
    //  - 500 倍余量足以吸收 CI 调度抖动。
    support::Floor::new("cjk_resolve_warm_per_line", 1.0).assert_within(256, || {
        for glyph in &line {
            black_box(pipeline.glyph_information(black_box(*glyph)));
        }
    });
    criterion.bench_function("cjk_resolve_warm_per_line", |holder| {
        holder.iter(|| {
            for glyph in &line {
                black_box(pipeline.glyph_information(black_box(*glyph)));
            }
        });
    });
}

criterion_group!(benches, cjk_resolve_steady_state);
criterion_main!(benches);
