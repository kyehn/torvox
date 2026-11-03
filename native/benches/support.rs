//! 基准的**下限断言**：criterion 只做统计，不判定「慢到不能用」。
//!
//! `build-and-validation` 规范要求性能基准也必须给出结果断言——没有断言的基准
//! 在回归时只会安静地变慢，CI 依旧全绿。criterion 的统计区间不适合直接当门禁
//! （机器差异大、偶发调度抖动会让合法提交变红），故这里在正式采样前跑一段
//! **粗粒度**吞吐量测：只兜住数量级的崩塌（例如误引入逐字体重建、掉进 O(n²)），
//! 阈值取实测值的十分之一以下，正常机器不会命中。
//!
//! 与 `native/src/render/tests/cell_pipeline.rs` 的 `bench_*` 用例互补：那边是
//! CI 门禁的硬阈值，这边是 `cargo bench` 自身的兜底。

/// 单次操作耗时上限（毫秒）。
///
/// 用「单次耗时」而非「每秒次数」表达，读起来与基准名同尺度；也避免除零。
pub(crate) struct Floor {
    /// 断言里打印的名字。
    pub(crate) what: &'static str,
    /// 单次操作允许的最长耗时（毫秒）。
    pub(crate) max_millis: f64,
}

impl Floor {
    pub(crate) const fn new(what: &'static str, max_millis: f64) -> Self {
        Self { what, max_millis }
    }

    /// 跑 `iterations` 次取平均，超预算即 panic。
    ///
    /// 取平均而非取最大值：单次抖动（GC、调度、频率调节）不该判红，数量级退化会。
    pub(crate) fn assert_within(&self, iterations: u32, mut operation: impl FnMut()) {
        let start = std::time::Instant::now();
        for _ in 0..iterations {
            operation();
        }
        let average_millis = start.elapsed().as_secs_f64() * 1000.0 / f64::from(iterations);
        assert!(
            average_millis <= self.max_millis,
            "{}：单次平均 {average_millis:.3}ms，超出上限 {:.3}ms（{iterations} 次采样）",
            self.what,
            self.max_millis,
        );
    }
}
