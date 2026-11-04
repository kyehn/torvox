# Proposal

## Why

`docs/specification/TESTING.md` 要求「性能测试必须有断言，实际测试获得基准值，低于
基准时测试失败，基准需要和实际测试结果相对接近以发现之后的性能降低」且
「macrobenchmark 需要在模拟器上运行并断言，要求在模拟器上流畅运行」。本能力的现有
Requirement 只覆盖 Rust 微基准与真机探针，**没有覆盖 Android 宏基准**
（`android/benchmark`，CI build 工作流在模拟器上每轮运行的五→六用例），而它正是
TESTING.md 两条要求的直接对象。

现状缺口（STATUS 挂账「android/benchmark 需要更充分的性能测试并且断言测试结果」）：

1. 宏基准的断言只有「帧数 ≥ 1」与粗粒度耗时预算，`FrameTimingMetric` 的帧耗时
   百分位只进 instrumentation 结果 bundle（IDE 可见），CI 控制台与日志里**看不到
   任何实测值**——「实际测试获得基准值」没有数据来源，断言预算只能是拍的。
2. `dumpsys gfxinfo` 的帧统计（帧数、janky 占比、p50/p90/p95/p99）是模拟器上
   唯一可脚本采集的帧级数据面，未被纳入断言与采集口径。TESTING.md 的覆盖范围把
   「上滑/下滑正确移动终端，无卡顿」列为必测项，宏基准侧此前没有滚动用例。

## What Changes

- 新增 `FrameStats`（gfxinfo 解析：帧数/janky/百分位）与 `printMeasured` 采集通道
  （`println` 进测试 XML system-out，`Log.i("Benchmark")` 进 logcat）；
  `scripts/test-emulator.nu` 在宏基准跑完 cat 测试 XML，实测值进构建日志。
- 交互与启动各用例逐轮打印实测耗时与帧统计；新增
  `ScrollPerformanceBenchmark`（`seq 500` 填满回滚区后上滑、下滑各一次）。
- 规格新增一条 Requirement：宏基准 MUST 在模拟器上运行并断言，实测值 MUST 经
  采集通道落进 CI；帧百分位与耗时预算 MUST 以 CI 实测校准（约实测 1.5 倍口径，
  与 Rust 微基准上限断言同口径），MUST NOT 凭空设定。

## Impact

- 规格：`performance-measurement` ADDED 一条 Requirement（本变更）。
- 代码：`android/benchmark` 五个文件、`scripts/test-emulator.nu`
  （已在 `e172def4` 落地，与本提案同源）。
- 待办：CI 恢复运行后按 `[torvox-bench]` 实测值收紧断言（tasks.md 未勾项）。
