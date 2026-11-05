//! 基准的**下限断言**：criterion 只做统计，不判定「慢到不能用」。
//!
//! `build-and-validation` 规范要求性能基准也必须给出结果断言——没有断言的基准
//! 在回归时只会安静地变慢，CI 依旧全绿。criterion 的统计区间不适合直接当门禁
//! （机器差异大、偶发调度抖动会让合法提交变红），故这里在正式采样前跑一段
//! **粗粒度**吞吐量测：上限按 CI 实测值（`[floor]` 打印）约 1.5 倍校准，
//! 与 `docs/specification/TESTING.md` 的「基准需要和实际测试结果高度相近」一致，
//! 既能发现明显劣化，又不至于被调度抖动判红。
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
    ///
    /// 正式采样前**先预热一次**且不计入统计：被测路径的首次调用要做一次性初始化
    /// （字形光栅化与图集上传、查询缓存填充），其成本比稳态高两个数量级。不预热时
    /// 它被摊进平均，CI runner（2 核慢 CPU）上 1920 格首调约 13s，把平均抬到
    /// 800ms+——断言报的是初始化成本而不是被测路径的退化，测的根本不是要测的东西。
    ///
    /// 采样后**打印实测均值**：`Floor` 的上限与本地开发机的 CPU 快慢强相关
    /// （同一断言在 M 系列开发机与 CI 2 核 runner 上差一个数量级），上限的校准
    /// 依据必须是 CI 实测值而非本地实测值；没有这行打印，上限定错时只能靠
    /// 失败消息逐轮回溯。
    #[allow(
        clippy::print_stdout,
        reason = "基准须把实测均值打印出来：上限按 CI 实测值校准，没有这行输出无从核对"
    )]
    pub(crate) fn assert_within(&self, iterations: u32, mut operation: impl FnMut()) {
        operation();
        let start = std::time::Instant::now();
        for _ in 0..iterations {
            operation();
        }
        let average_millis = start.elapsed().as_secs_f64() * 1000.0 / f64::from(iterations);
        println!(
            "[floor] {}: {:.3}ms average over {iterations} runs (limit {:.3}ms)",
            self.what, average_millis, self.max_millis,
        );
        assert!(
            average_millis <= self.max_millis,
            "{}：单次平均 {average_millis:.3}ms，超出上限 {:.3}ms（{iterations} 次采样）",
            self.what,
            self.max_millis,
        );
    }
}
