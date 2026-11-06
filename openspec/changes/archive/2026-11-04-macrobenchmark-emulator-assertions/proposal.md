# Proposal

## Why

`docs/specification/TESTING.md` 要求「性能测试必须有断言，实际测试获得基准值，低于
基准时测试失败，基准需要和实际测试结果相对接近以发现之后的性能降低」且
「macrobenchmark 需要在模拟器上运行并断言，要求在模拟器上流畅运行」。本能力的现有
Requirement 只覆盖 Rust 微基准与真机探针，**没有覆盖 Android 宏基准**
（`android/benchmark`，连接测试在模拟器上每轮运行的五→六用例），而它正是
TESTING.md 两条要求的直接对象。

现状缺口（STATUS 挂账「android/benchmark 需要更充分的性能测试并且断言测试结果」）：

1. 宏基准的断言只有「帧数 ≥ 1」与粗粒度耗时预算，`FrameTimingMetric` 的帧耗时
   百分位只进 instrumentation 结果 bundle（IDE 可见），控制台与日志里**看不到
   任何实测值**——「实际测试获得基准值」没有数据来源，断言预算只能是拍的。
2. `dumpsys gfxinfo` 的帧统计（帧数、janky 占比、p50/p90/p95/p99）是模拟器上
   唯一可脚本采集的帧级数据面，未被纳入断言与采集口径。TESTING.md 的覆盖范围把
   「上滑/下滑正确移动终端，无卡顿」列为必测项，宏基准侧此前没有滚动用例。

## What Changes

- 新增 `FrameStats`（gfxinfo 解析：帧数/janky/百分位）与 `printMeasured` 采集通道
  （`println` 进 instrumentation 输出流、`Log.i("Benchmark")` 进 logcat；连接测试任务
  按用例把设备 logcat 落进构建产物，实测值因此可复核）。
- 交互与启动各用例逐轮打印实测耗时与帧统计；新增
  `ScrollPerformanceBenchmark`（`seq 500` 填满回滚区后上滑、下滑各一次）。
- **修正帧级数据面的分工**：终端内容由原生渲染线程直绘 SurfaceView，gfxinfo
  （HWUI）根本记不到这些帧（实测滚动全程 0 帧），故滚动用例改取原生渲染器的
  呈现汇总（`render_frame: presented N frames`）。
- **修正设备侧采集通道的用法**：`UiDevice.executeShellCommand` 走
  `Runtime.exec(String)`，不经 shell（按空白切词、引号原样传参、管道不生效），
  logcat 过滤词必须是 POSIX ERE 且不含空白。
- **补上输入前置条件的判定**：shell 没有提示符可等，用幂等探针命令诱发一次
  PTY 输出作为「已能吃键」的信号，探针不回显即判红。
- 规格新增五条 Requirement：宏基准 MUST 在模拟器上运行并断言、帧级数据面 MUST 按
  绘制主体选源、实测值 MUST 经采集通道可复核、采集通道 MUST 按执行环境真实语义
  构造、输入前置条件 MUST 可判定；帧级阈值与耗时预算 MUST 以实测校准（约实测
  1.5 倍口径，与 Rust 微基准上限断言同口径），MUST NOT 凭空设定。

## Impact

- 规格：`performance-measurement` ADDED 五条 Requirement（本变更，已同步进
  `openspec/specs/performance-measurement/spec.md`）。
- 代码：`android/benchmark` 下的基准源码；`android/app` 的 `TerminalRuntime` 在首个
  PTY 输出时记一条 INFO 日志（宏基准据此判定 shell 已开口）。
- 待办：无。断言阈值已按本机 AVD 的实测校准并在本地模拟器复核，门禁以本机 AVD
  为准。
