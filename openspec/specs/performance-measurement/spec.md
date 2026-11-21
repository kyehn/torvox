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
- 跨应用性能对比的测量手段取舍 MUST 另立一条要求记录，不并入本条。

#### Scenario: 复查不采纳项

- **WHEN** 再次评估「是否需要 input-to-present 关联诊断」
- **THEN** 以本条目给出的边界（现有探针已定位帧内慢段）为起点，MUST NOT 从零开始调研

### Requirement: 跨应用性能对比的帧面类测量手段取舍须记录理由

跨应用性能对比的帧面类测量手段取舍 MUST 逐项记录理由，使复查不必重新调研：

- 宏基准的 `FrameTimingMetric` 与 `StartupTimingMetric` 作跨应用主判据：官方明确不推荐在
  模拟器上跑；目标应用须非 debuggable；两者量的是各目标自身管线的帧，跨应用语义不同；
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

### Requirement: Android 宏基准在模拟器上运行并断言

Android 宏基准（`android/benchmark`）MUST 在模拟器上以连接测试方式运行
（`:benchmark:connectedBenchmarkReleaseAndroidTest`），MUST NOT 只写在文档里而不真跑。

每个用例 MUST 至少断言两件事：交互窗口内**至少出一帧**（帧数为 0 说明被测路径没有
产生任何绘制，测量无意义）、操作**总耗时在预算内**（拦住「慢到不可用」级回归）。
滚动、按键等状态类交互还 MUST 断言功能状态真的发生（粘滞态置起、抽屉节点出现），
MUST NOT 用「没报错」代替「发生了」。

#### Scenario: 滚动不产生帧判红

- **WHEN** 回滚区填满后滚动手势没有产生任何帧
- **THEN** 用例失败并报出呈现帧数为 0，而不是留下一个测了个寂寞的空窗口

#### Scenario: 启动超预算判红

- **WHEN** 冷启动到终端可用的耗时超过预算
- **THEN** 用例失败并报出实测耗时与预算值

### Requirement: 宏基准的阈值校准与日志等级

阈值校准与回归判定 MUST 以执行环境里模拟器的实测为准（当前执行环境为本机 AVD，
API 35 / SwiftShader），MUST NOT 因某条流水线缺席而停摆或降低覆盖。

宏基准 MUST 以启动 extra 把被测进程的日志等级提到 DEBUG：原生呈现汇总是 debug 级，
而 release 构建的缺省等级是 WARN——等级不够时基准永远读不到帧，只能以「没出帧」
判红，测的是日志等级而不是性能。extra MUST NOT 写回用户的日志等级设置。

模拟器宏基准数据 MUST NOT 用于建立物理设备性能结论，只用于数量级回归与功能级
断言（本能力「真机性能结论的判据与环境口径」不变）。

#### Scenario: 日志等级不够则读不到帧

- **WHEN** 被测进程以 release 构建的缺省日志等级启动
- **THEN** 基准读不到 debug 级的呈现汇总，判红的是日志等级而不是性能

#### Scenario: 流水线缺席不停摆

- **WHEN** 某条流水线缺席宏基准的执行环境
- **THEN** 阈值仍以执行环境里的模拟器实测为准，不停摆也不降低覆盖

### Requirement: 宏基准的帧级数据面按绘制主体选源

帧级数据面 MUST 按「谁画了这帧」选源，MUST NOT 用单一数据面覆盖所有被测路径：

- Compose / HWUI 绘制的界面（修饰键栏、抽屉、输入法动画）→ `dumpsys gfxinfo` 的
  帧数、janky 占比与 p50/p90/p95/p99。
- 终端内容由原生渲染线程直绘 SurfaceView 的 Surface，不经过主线程 `ViewRootImpl`
  遍历，HWUI 根本记不到这些帧 → MUST 取原生渲染器的呈现汇总
  （`render_frame: presented N frames`），MUST NOT 用 gfxinfo 的帧数判「终端没出帧」。

呈现帧数是**宿主吞吐**的函数，不是被测逻辑的函数：同一台模拟器在空闲宿主与被并发
任务抢占的宿主上，同一构建、同一手势的实测帧数可差三倍。故帧数下限 MUST 按**最差
实测**校准，MUST NOT 按空闲宿主的峰值设定；同时 MUST 另设「有帧的呈现窗口数」下界
——该判据与宿主速度无关，拦的是滚动中途停摆。

#### Scenario: 终端滚动按原生呈现汇总判帧

- **WHEN** 断言终端滚动的帧数
- **THEN** 帧数取自原生呈现汇总，gfxinfo 只用于 Compose / HWUI 绘制的界面

#### Scenario: 满载宿主不误判为回归

- **WHEN** 宿主被并发任务抢占，滚动实测帧数降到空闲时的约三分之一
- **THEN** 用例仍通过（帧数下限按最差实测校准），且呈现窗口数下界仍成立

### Requirement: 宏基准实测值可观测且阈值按实测校准

宏基准的实测值 MUST 可观测：每轮迭代把耗时与帧统计写进设备 logcat（`println` 进
instrumentation 输出流、`Log.i("Benchmark")` 进 logcat，两条通道各留一份），连接测试
任务按用例把设备 logcat 落进构建产物
（`androidTest-results/connected/<variant>/<device>/logcat-<类名>-<方法名>.txt`），
MUST NOT 让实测值只留在 IDE 可见的 instrumentation bundle 里。

帧级阈值与耗时预算 MUST 以实测值校准（实测的约 1.5 倍口径，与 Rust 微基准
上限断言同口径），并在常量注释里引用实测出处；实测出处取执行环境里的模拟器
（当前为本机 AVD）；MUST NOT 凭空设定，也不得宽到只拦「卡死级」。
`FrameTimingMetric` 的百分位只进 IDE 结果 bundle，模拟器上可脚本采集
的帧级数据按上一条 Requirement 的分工取（gfxinfo 或原生呈现汇总）。

#### Scenario: 实测值落进构建产物

- **WHEN** 在模拟器上运行宏基准
- **THEN** 连接测试产出的用例 logcat 里出现每轮迭代的实测耗时与帧统计
      （`[bench-measure]` 行），断言预算可据此校准

#### Scenario: 断言预算有实测出处

- **WHEN** 给帧级阈值或耗时设断言阈值
- **THEN** 常量注释引用实测值与运行出处，阈值约为实测的 1.5 倍

### Requirement: 宏基准的设备侧采集通道按执行环境的真实语义构造

宏基准通过 `UiDevice.executeShellCommand` 采集设备侧数据。该入口 MUST 按其真实
语义构造命令：`UiAutomation` 以 `Runtime.exec(String)` 执行，**不经 shell**——
命令按空白切词、引号原样传进参数、管道不生效。故：

- 命令 MUST NOT 含引号、管道、重定向等 shell 语法；需要过滤时 MUST 取回整份输出
  后在本地筛选。
- logcat 的 `-e` 过滤词 MUST 是 POSIX ERE 且不含空白（空白用 `.` 代替）：Java 正则
  的 `\d`/`\w` 在 ERE 里不是字符类，设备侧过滤会把真正的记录全滤掉。

MUST NOT 以「命令没报错」当作「采集到了数据」：采集失败 MUST 表现为空结果，并由
断言判红。

#### Scenario: 过滤词按 ERE 构造

- **WHEN** 采集 logcat 里形如 `session 1 first output received` 的记录
- **THEN** 过滤词写作不含空白的 ERE（如 `first.output.received`），设备侧命中该行

#### Scenario: 采集为空即判红

- **WHEN** 采集通道返回空结果（如命令被 shell 语义毁掉）
- **THEN** 用例失败并指出采集为空，MUST NOT 把空结果当成「被测路径没有发生」

### Requirement: 宏基准的输入前置条件必须可判定

宏基准通过真实输入链路（`input text` / `input keyevent`）驱动终端时，MUST 先判定
「终端已经能吃键」再继续：shell 起来前写入的输入会被 tty 初始化冲掉，命令静默
消失，测量到的却是「什么都没发生」。

终端内的 shell 没有提示符可等（实测 `/system/bin/sh` 在 PTY 里首启无任何输出），
故 MUST 用探针命令诱发一次输出作为就绪信号（幂等、可重发），MUST NOT 用固定
等待时长替代。

就绪上限 MUST 覆盖实测的启动耗时：模拟器上 shell 的首个 PTY 输出实测可达 44s
（2026-11-12 真机日志：19:28:55 spawn，19:29:40 首输出），上限过短会把「还没
开口」误判成「shell 不工作」。上限 MUST NOT 用固定等待时长替代就绪信号本身。

#### Scenario: shell 未就绪判红

- **WHEN** 探针在上限内始终没有回显
- **THEN** 用例失败并报出「shell 未就绪」，MUST NOT 继续测量一个空终端

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
