# performance-measurement Specification

## ADDED Requirements

### Requirement: 跨应用性能对比的对照面与统一帧面

跨终端性能对比 MUST 以系统侧可观测面为准，MUST NOT 以各家自报的帧率作跨应用结论。帧时序
MUST 取系统侧图层时序：场景前用 `dumpsys SurfaceFlinger --latency-clear <layer>` 清空环形
历史，场景后用 `dumpsys SurfaceFlinger --latency <layer>` 取回逐帧数据。该面量的是合成器把
帧呈现出去的时刻，与帧由哪条管线绘制无关，故是跨应用唯一的统一帧面。

对照对象 MUST 取现实可得的同类终端应用，且 MUST 先核实其是否提供基准资产与可核实的官方性能
数字；两者皆无时对比 MUST 自建 harness，MUST NOT 引用其自述性能数字。

`dumpsys gfxinfo` 的帧统计 MUST 只用于 HWUI 绘制的界面，MUST NOT 作终端内容帧的跨应用判据：
该入口只统计经 ViewRootImpl/RenderThread 绘制的帧，SurfaceView 直绘不经该路径。

#### Scenario: 终端内容帧按图层时序比对

- **WHEN** 同一场景在多个终端应用上采集帧时序
- **THEN** 每个应用的帧时序 MUST 取自系统侧图层时序，MUST NOT 取自各家自报帧率，
      MUST NOT 用 `gfxinfo` 帧数作终端内容帧的跨应用判据

#### Scenario: 采集前清空图层帧历史

- **WHEN** 一个场景开始前采集图层时序
- **THEN** MUST 先清空该图层的帧历史，否则读到的 MUST NOT 含上一个场景的残留帧

#### Scenario: 图层名取不到即判采集失败

- **WHEN** 图层名无法解析或 `--latency` 返回为空
- **THEN** 该次采集 MUST 判为采集失败并中止该场景，MUST NOT 把空结果当作「被测路径没有
      出帧」，MUST NOT 静默跳过该场景

#### Scenario: 对照对象无基准资产时不引用自述数字

- **WHEN** 对照应用既无基准模块又无可核实的官方性能数字
- **THEN** 对比 MUST 由自建 harness 产出，MUST NOT 引用该应用的自述性能数字作为结论

### Requirement: 全场景矩阵与环境固定化

场景矩阵 MUST 覆盖启动三态（冷／温／热）、稳态空载、持续输出、突发输出、长滚动、宽字符行、
交互延迟、输入洪水、前后台切换与资源极端；每个场景 MUST 有固定负载、统一指标、采集面、判据
与硬上限。

同一场景 MUST 给各被测应用完全相同的字节流与网格尺寸；负载不等价时该场景的对比结论无效。
同一场景 MUST 重复多次并以分位数（p95／p99）表达，MUST NOT 以均值或单次结果下结论；被测
应用的执行顺序 MUST 交替以抵消宿主漂移。

每次测量 MUST 记录设备型号、ROM 构建号、API、刷新率、电池状态、热状态与构建标识。刷新率、
性能档位、动画缩放与温度 MUST 在测量前固定并记录实际生效值。

#### Scenario: 负载不等价的对比无效

- **WHEN** 同一场景给各被测应用的字节流或网格尺寸不同
- **THEN** 该场景的对比结论无效，MUST NOT 以该结果作性能结论

#### Scenario: 环境漂移被记录

- **WHEN** 测量在刷新率、电池状态或热状态发生变化后进行
- **THEN** 该次测量 MUST 记录变化后的实际值，MUST NOT 与变化前的数据直接对比

#### Scenario: 同一场景重复多次取分位数

- **WHEN** 一个场景在同一被测应用上重复多次
- **THEN** 结论 MUST 以分位数表达，MUST NOT 以均值或单次结果下结论

### Requirement: 失败归因与采集失败

失败 MUST 按四类归因：应用归因、系统归因、人为与编排归因、采集失败。归因依据 MUST 取自
`ApplicationExitInfo` 的 reason、FrameTimeline 的 jank 类型、logcat 与 dropbox 四源交集；证据
不足时 MUST 记「未定」并保留全部制品，MUST NOT 强行归因，MUST NOT 把归因不清记为无问题。

`REASON_OTHER` MUST 归人为与编排归因——其官方口径是既不是应用的问题也不是应用能操作的。
`REASON_CRASH`、`REASON_CRASH_NATIVE` 与 `REASON_ANR` MUST 归应用归因；
`REASON_LOW_MEMORY` 与 `REASON_MEMORY_LIMITER` MUST 归系统归因。

采集为空 MUST 表现为采集失败并判红，MUST NOT 当作「被测路径没有发生」。

#### Scenario: exit-info reason 归因

- **WHEN** 被测进程在场景中退出
- **THEN** 归因 MUST 由 exit-info 的 reason 给出，应用归因与系统归因 MUST 分列，
      MUST NOT 以「跑挂了」一类概括收尾

#### Scenario: 采集失败不当作没有发生

- **WHEN** 采集命令返回空结果
- **THEN** 该次采集 MUST 判为采集失败并判红，MUST NOT 当作「被测路径没有发生」

#### Scenario: 证据不足记未定

- **WHEN** 四源证据不足以区分应用归因与系统归因
- **THEN** 结论 MUST 记「未定」并保留全部制品，MUST NOT 强行归因、亦 MUST NOT 记为无问题

### Requirement: 极端场景的资源上限

每个场景 MUST 有 wall-clock 上限、输出字节上限与采样间隔下限；超限 MUST 终止该场景并记录
「超限终止」，MUST NOT 继续等待，MUST NOT 继续追加输出。制品落盘 MUST 有配额并轮转清理，
超限制品 MUST 丢弃而非占满设备。

采集 MUST 单被测应用串行执行：同一时刻只跑一个被测应用，以免宿主抢占污染结论。

#### Scenario: 超限即终止

- **WHEN** 场景达到 wall-clock 上限或输出字节上限
- **THEN** 该场景 MUST 被终止并记录「超限终止」，MUST NOT 继续等待或继续追加输出

#### Scenario: 制品配额轮转

- **WHEN** 制品目录达到配额
- **THEN** 旧制品 MUST 被轮转清理，MUST NOT 让设备存储被制品占满

### Requirement: 跨应用性能对比的帧面类测量手段取舍须记录理由

跨应用性能对比的帧面类测量手段取舍 MUST 逐项记录理由，使复查不必重新调研：

- 宏基准的 `FrameTimingMetric` 与 `StartupTimingMetric` 作跨应用主判据：官方明确不推荐在
  模拟器上跑；本仓 release 变体的源码清单未声明 `android:profileable`，指标无采集前提；
  既有规约已把它定位为「模拟器数量级回归」用途。
- `dumpsys gfxinfo` 的帧数作终端内容帧判据：该入口只覆盖经 ViewRootImpl/RenderThread
  绘制的帧，SurfaceView 直绘不在其中。
- `screenrecord` 作判据：编码与分辨率引入偏差，只作人工复核制品。
- 引用对照应用的自述性能数字：对照应用既无基准模块又无可核实的官方性能数字时不得引用。

#### Scenario: 复查跨应用判据类不采纳项

- **WHEN** 再次评估「能否用某家自报的帧率或宏基准指标作跨应用结论」
- **THEN** MUST 以本条目给出的边界为起点：统一帧面是系统侧图层时序，自报帧率与宏基准指标
      不作跨应用主判据

### Requirement: 跨应用性能对比的 trace 与能耗类测量手段取舍须记录理由

跨应用性能对比的 trace 与能耗类测量手段取舍 MUST 逐项记录理由，使复查不必重新调研：

- Perfetto `android.log` 数据源：仅 userdebug 构建可用，正式版真机改用 logcat 全量落盘。
- `gpu.counters` 跨设备对比：counter ID 由设备描述符给定，跨设备不可比。
- `PowerMetric` 跨设备对比：依赖具体硬件与充电状态。

#### Scenario: 复查 trace 与能耗类不采纳项

- **WHEN** 再次评估「能否用 Perfetto 日志数据源或 GPU 计数器作跨设备对比」
- **THEN** MUST 以本条目给出的边界为起点：正式版真机用 logcat 全量落盘替代
      `android.log`，GPU 计数器与功耗指标只在同设备内可比

## MODIFIED Requirements

### Requirement: 不采纳的测量手段须记录理由

已评估后决定不采纳的测量手段 MUST 在本规范中记录理由，使后续复查不必重新调研。

- atrace counter 与 FrameTimeline 关联（input-to-present 诊断）：需要输入侧计数器、
  帧侧计数器与关联脚本三件套且只能在真机运行；帧与循环计时报告已能定位帧内慢段，
  作为真机排障可选项记录，不入规。
- 跨应用性能对比的测量手段取舍 MUST 另立一条要求记录，不并入本条。

#### Scenario: 复查不采纳项

- **WHEN** 再次评估「是否需要 input-to-present 关联诊断」
- **THEN** 以本条目给出的边界（现有探针已定位帧内慢段）为起点，MUST NOT 从零开始调研
