## ADDED Requirements

### Requirement: Android 宏基准在模拟器上运行并断言

Android 宏基准（`android/benchmark`）MUST 在 CI 的模拟器上运行（`build` 工作流的
`:benchmark:connectedBenchmarkReleaseAndroidTest`），MUST NOT 只留在本地手动执行。

每个用例 MUST 至少断言两件事：交互窗口内**至少出一帧**（帧数为 0 说明被测路径没有
产生任何绘制，测量无意义）、操作**总耗时在预算内**（拦住「慢到不可用」级回归）。
滚动、按键等状态类交互还 MUST 断言功能状态真的发生（粘滞态置起、抽屉节点出现），
MUST NOT 用「没报错」代替「发生了」。

模拟器宏基准数据 MUST NOT 用于建立物理设备性能结论，只用于数量级回归与功能级
断言（本能力「真机性能结论的判据与环境口径」不变）。

#### Scenario: 滚动不产生帧判红

- **WHEN** 回滚区填满后滚动手势没有产生任何帧
- **THEN** 用例失败并报出 `frames=0`，而不是留下一个测了个寂寞的空窗口

#### Scenario: 启动超预算判红

- **WHEN** 冷启动到终端可用的耗时超过预算
- **THEN** 用例失败并报出实测耗时与预算值

### Requirement: 宏基准实测值可观测且阈值按实测校准

宏基准的实测值 MUST 可观测：每轮迭代把耗时与帧统计写进测试输出（`println` 进
instrumentation 输出流，随测试 XML 的 system-out 进构建日志；`Log.i("Benchmark")`
进 logcat），CI 脚本 MUST 在宏基准结束后输出测试 XML，MUST NOT 让实测值只留在
IDE 可见的 instrumentation bundle 里。

帧耗时百分位与耗时预算的断言阈值 MUST 以 CI 实测值校准（实测的约 1.5 倍口径，
与 Rust 微基准上限断言同口径），并在常量注释里引用实测出处；MUST NOT 凭空设定，
也不得宽到只拦「卡死级」。帧级数据面取 `dumpsys gfxinfo` 的帧数、janky 占比与
p50/p90/p95/p99：`FrameTimingMetric` 的百分位只进 IDE 结果 bundle，模拟器上唯一
可脚本采集的帧级数据是 gfxinfo。

#### Scenario: 实测值落进 CI 日志

- **WHEN** CI 运行宏基准
- **THEN** 构建日志里出现每轮迭代的实测耗时与帧统计（`[torvox-bench]` 行），
      断言预算可据此校准

#### Scenario: 断言预算有实测出处

- **WHEN** 给帧耗时百分位或耗时设断言阈值
- **THEN** 常量注释引用 CI 实测值与运行出处，阈值约为实测的 1.5 倍
