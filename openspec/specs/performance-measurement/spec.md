# performance-measurement Specification

## Purpose

本仓的性能结论来自两条互不替代的路径：CI 内的微基准与测试（`cargo bench --quick`、
Rust 用例、JVM 用例）与真机上的帧时序探针（渲染循环的 avg/p95/max 报告、慢帧分段
诊断）。两者的能力边界必须写明：合成负载可以证明回归，但不能替代物理设备结论。
本能力记录判据口径、环境可比性要求、负载口径，以及已评估后不采纳的测量手段。

## Requirements

### Requirement: 真机性能结论的判据与环境口径

真机性能结论 MUST 以 p95 / p99 / missed-deadline 百分比表达，MUST NOT 以中位数或
单次帧时长下结论。两个 revision 的对比 MUST 同设备、同刷新率、同字体、同尺寸，
并交替顺序执行；每次测量 MUST 记录设备、构建与热状态。模拟器数据 MUST NOT 用于
建立物理设备性能结论，只能用于数量级回归。

#### Scenario: 同一 harness 对比两个 revision

- **WHEN** 在同一台真机上交替运行两个 revision 的同一测量
- **THEN** 两者同刷新率、同字体、同网格尺寸，且每次运行的设备/构建/热状态被记录

#### Scenario: 模拟器数据只作数量级参考

- **WHEN** 在软件 Vulkan 模拟器上测得帧率
- **THEN** 该结果 MUST NOT 作为物理设备结论，只能用于发现数量级回归

#### Scenario: 帧时序报告保留分布而非均值

- **WHEN** 渲染循环结算一个计时窗口
- **THEN** 报告包含 avg、p95、max 与 fps，慢帧诊断按 500ms 限频输出

### Requirement: 基准负载口径固定为合成负载

`cargo bench` 的基准负载 MUST 为可复现的合成负载，MUST NOT 引入需现场采集的
真机 PTY 语料——语料进仓库违反「不得 vendor 可自动获取的测试数据」，现场采集则
每次对比的设备与时机都不同，丧失可比性。键入路径的正确性由 Rust / JVM 用例承担，
基准只测同一路径的性能。

#### Scenario: 基准不依赖设备侧资源

- **WHEN** 在任意开发机上运行 `cargo bench --bench cell_builder --bench vt_typing -- --quick`
- **THEN** 基准只读仓库内构造的负载，不需要连接设备或外部语料

#### Scenario: 突发输出有无损用例兜底

- **WHEN** 评估是否需要真实 PTY 重放负载
- **THEN** 以「键入路径已被用例覆盖 + 突发无损用例在位」为准，不新增采集型负载

### Requirement: CI 下限断言先预热再采样

CI 性能门禁的下限断言（`native/benches/support.rs` 的 `Floor::assert_within`）MUST 在
正式采样前先执行一次被测操作并**不计入**统计：被测路径的首次调用包含一次性初始化
（字形光栅化与图集上传、查询缓存填充），其成本比稳态高两个数量级，摊进平均会把
「初始化成本」误报成「路径退化」。阈值 MUST 相对**预热后**的稳态设定；不设预热的
断言在慢机上永远误报，等于没有门禁。上限 MUST 按 CI 实测均值校准（每轮采样后打印
`[floor]` 实测行，取其实测的约 1.5 倍），与 `docs/specification/TESTING.md` 的
「基准需要和实际测试结果高度相近以发现之后的性能降低」一致。

#### Scenario: 首调成本不判红

- **WHEN** CI runner 上首次运行 `build_instances_from_cell_data` 下限断言
- **THEN** 初始化被预热吸收，采样均值反映稳态转换成本，不因首调光栅化失败

#### Scenario: 数量级退化仍然判红

- **WHEN** 转换路径被改成逐格重建字体或 O(n²) 查表
- **THEN** 稳态均值超出上限，断言失败并报出实测均值与采样数

### Requirement: 不采纳的测量手段须记录理由

已评估后决定不采纳的测量手段 MUST 在本规范中记录理由，使后续复查不必重新调研。

- atrace counter 与 FrameTimeline 关联（input-to-present 诊断）：需要输入侧计数器、
  帧侧计数器与关联脚本三件套且只能在真机运行；帧与循环计时报告已能定位帧内慢段，
  作为真机排障可选项记录，不入规。

#### Scenario: 复查不采纳项

- **WHEN** 再次评估「是否需要 input-to-present 关联诊断」
- **THEN** 以本条目给出的边界（现有探针已定位帧内慢段）为起点，MUST NOT 从零开始调研
