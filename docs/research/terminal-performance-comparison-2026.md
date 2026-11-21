# 跨终端性能对比与全场景数据采集：无人工干预的方法学

## 摘要

结论先行：

1. **端到端帧率不可跨应用直接比。** 两个现实对照对象（`termux/termux-app`、
   `sylirre/ghostty-android-terminal`）都用 HWUI `Canvas` 绘制终端内容，本项目用 wgpu 直绘
   SurfaceView；同一句「60 fps」在两侧量的不是同一件事。
2. **跨应用统一帧面分三层，各有覆盖面**：显示级是 `dumpsys SurfaceFlinger --frametimeline`
   （逐 display frame 的期望/实际起止与呈现时刻、jank 分类；每个 display frame 下还挂着逐图层
   条目，带 `Owner Pid` 归属，无需预知图层名，对所有应用同一口径）；图层级是
   `dumpsys SurfaceFlinger --latency <layer>`（逐帧 `desired / actual / ready` 纳秒三列，精确名
   匹配**任何**图层，含 BLAST 类型的 SurfaceView 内容层）；应用内是各家的帧统计（本项目终端内容
   直绘 SurfaceView，只有原生呈现汇总能测到）。实测推翻了「`--latency` 覆盖不到 SurfaceView」
   这一原假设：名称精确即可命中本项目终端 SurfaceView 图层，「行全为零」只说明该图层没有内容帧。
3. **采集分七面**：跨应用帧时序、应用内帧时序、进程与系统状态、能耗与热、崩溃与退出、
   日志与制品、深度 trace。每面都有「采集为空即判红」纪律，采集侧不做筛选、不限级。
4. **全场景矩阵 12 类**，每类有固定负载、硬超时、输出上限；超限即终止并记录，不无限等待，
   不为失败场景继续消耗设备资源。
5. **失败归因分四类**：应用归因、系统归因（内存／显示／宿主抢占）、人为与编排归因、采集失败。
   归因依据只能来自 `ApplicationExitInfo` 的 reason、FrameTimeline 的 jank 类型、logcat 与
   dropbox，不靠主观判断；证据不足时记「未定」并保留全部制品。

## 研究方法与坐标

原则与既有研究报告一致：只用一手资料；任何结论都要能回到官方文档、上游源码或本仓源码核实。
本仓约定文档不写提交号，故坐标一律用标签、版本号、符号名或文件路径。

| 对象 | 坐标 | 一手核验手段 |
| --- | --- | --- |
| Android 官方文档 | `topic/performance/benchmarking/macrobenchmark-overview`、`macrobenchmark-metrics`、`topic/performance/vitals`、`tools/dumpsys`、`studio/command-line/adb`、`reference/android/app/ApplicationExitInfo` | 抓取正文并逐条取事实 |
| Perfetto 官方文档 | 上游仓库 `platform/external/perfetto` 的 `docs/` 目录（`data-sources/*.md`） | perfetto.dev 路径已 404，改从上游仓库镜像逐份读原文 |
| AOSP 源码 | `frameworks/native/services/surfaceflinger/SurfaceFlinger.cpp`、`services/surfaceflinger/Layer.cpp`、`services/surfaceflinger/FrameTracker.cpp`、`services/surfaceflinger/FrameTimeline/FrameTimeline.cpp`、`services/surfaceflinger/FrontEnd/RequestedLayerState.cpp`、`frameworks/base/core/java/android/view/ThreadedRenderer.java` | 逐符号读实现 |
| 对照对象源码 | `termux/termux-app` v0.119.0-beta.3；`sylirre/ghostty-android-terminal` main | 仓库树枚举 + 关键文件逐行读 |
| 宏基准库源码 | `androidx/androidx` 的 `benchmark-common`（`Errors.kt`、`BenchmarkState.kt`）与 `benchmark-macro`（`Macrobenchmark.kt`） | 逐符号读实现 |
| 本仓 | `android/benchmark/`、`android/app/src/main/AndroidManifest.xml`、`scripts/test-emulator.nu`、`native/benches/`、`openspec/specs/performance-measurement/spec.md` | 直接读工作树 |

## 一、对比对象的一手事实

| 项目 | 终端内容渲染路径 | VT 核 | 基准资产 | 可核实的官方性能数字 |
| --- | --- | --- | --- | --- |
| `termux/termux-app` v0.119.0-beta.3 | `TerminalView` 继承 `View`（`terminal-view/src/main/java/com/termux/view/TerminalView.java`），`TerminalRenderer` 以 `canvas.drawTextRun` 逐段绘制——HWUI 路径 | JVM 内自研（`terminal-emulator` 模块） | 仓内无 benchmark 模块 | 无 |
| `sylirre/ghostty-android-terminal` | `TerminalView` 继承 `View`（`app/src/main/java/io/github/sylirre/terminal/ui/TerminalView.java`），`canvas.drawText` 逐 run 绘制——同为 HWUI 路径 | 预编译 `libghostty-vt.a`（`app/src/main/cpp/CMakeLists.txt` 的 `add_library(ghostty-vt STATIC IMPORTED)`） | 仓内无 benchmark 模块 | 无 |
| 本项目 | wgpu 直绘 SurfaceView，终端内容不经 HWUI；Compose 外壳走 HWUI | `libghostty-vt` | `android/benchmark` 宏基准模块 + `native/benches` 微基准 | 无（模拟器数据只作数量级） |

两条结论直接决定方法学：

- **对比必须自建 harness。** 两个对照对象既没有基准资产，也没有发布过可核实的性能数字；
  任何引用其自述性能的对比都不成立。
- **端到端 fps 不可直接比。** 三者的终端内容帧归属不同（HWUI 绘制 vs 原生直绘），帧率数字
  不在同一语义上。可比的必须是「系统侧看得见的东西」：呈现时序、进程状态、能耗、退出归因。

另有两个影响可比性的工程事实：Termux release 构建开启代码压缩（`app/build.gradle` 的
`minifyEnabled true`），ghostty-android-terminal 依赖 Zig 构建的预编译原生库与
`arm64chroot` 层；三者的安装包体积、原生库体积与启动路径都不同，故「同场景相对表现」只在其
余条件（设备、负载、网格、字体、刷新率）锁定后才成立。

## 二、可比性边界

| 指标 | 跨应用可比 | 依据与口径 | 本项目取数 | 对照对象取数 |
| --- | --- | --- | --- | --- |
| 冷启动到首帧 | 可比 | 同一 `am start -W` + 同一首帧定义 | `am start -W` + SurfaceFlinger 首帧 | 同左 |
| 稳态帧时序 | 可比 | 同一通道同一口径：逐图层 `--latency`（含 BLAST SurfaceView 内容层）与 `--frametimeline` 的图层条目（带 `Owner Pid` 归属） | 终端内容 BLAST 图层的逐帧三列 + 应用窗口图层条目；应用内呈现汇总作第二数据面 | 内容在应用窗口图层，取数同左 |
| 内存占用 | 可比 | `dumpsys meminfo` 的 PSS / RSS | `dumpsys meminfo` | 同左 |
| 能耗 | 仅同设备内可比 | 硬件相关，跨设备无意义 | `batterystats` 按 UID | 同左 |
| 崩溃 / ANR / 异常退出 | 可比 | `ApplicationExitInfo` reason + logcat + dropbox | 同左 | 同左 |
| HWUI 帧统计（`gfxinfo`） | **仅同应用内可比** | 只统计经 ViewRootImpl/RenderThread 绘制的帧；SurfaceView 直绘不经该路径 | 只用于 Compose 界面自检 | 终端内容帧可测，但语义与本项目不同 |
| GPU 计数器 | 仅同设备内可比 | counter ID 由设备描述符给定 | `gpu.counters` | 同左 |
| Perfetto FrameTimeline 应用帧 | **不可作终端内容判据** | 官方文档明确 SurfaceView 不受 FrameTimeline 支持 | 终端内容在 SurfaceView 内，不在其覆盖内 | 走 HWUI 管线，在其覆盖内 |
| 宏基准指标 | 不作跨应用主判据 | 官方明确模拟器不代表真实体验；目标应用须非 debuggable；指标量的是各目标自身管线的帧 | 仅数量级回归 | 不适用 |

`gfxinfo` 的入口在 `ThreadedRenderer.handleDumpGfxInfo`（`dumpGlobalProfileInfo` 加
`WindowManagerGlobal.dumpGfxInfo`），参数 `framestats` / `reset` / `-a` 由 `dumpArgsToFlags`
解析——这条链只覆盖 HWUI 管线。本仓既有规约「宏基准的帧级数据面按绘制主体选源」与之一致。

关于宏基准的 `profileable` 要求有一处常见误解需要纠正：宏基准库的 `NOT-PROFILEABLE` 错误只在
API 29–30 触发（API 31 起 app trace tag 不再要求该标志）。实测本仓 release 与 debug 变体的
合并清单均未声明 `android:profileable`，但本仓 minSdk 31，故这不构成指标采集障碍；官方文档把
`profileable` 列为通用建议，而非当前版本的前置条件。真正约束宏基准作跨应用判据的是前三条。

## 三、跨应用统一的帧面：三层结构与各自的覆盖面

系统侧的帧数据分三层，覆盖面与语义各不相同。AOSP 实现给出确切语义，可据此写采集协议。

### 显示级：SurfaceFlinger 帧时间线

`SurfaceFlinger` 的 dump 参数表把 `--frametimeline` 映射到 `SurfaceFlinger::dumpFrameTimeline`，
转发给 `FrameTimeline::parseArgs`；该函数只认两个开关：`-jank` 与 `-all`（不带开关则无输出）。
实测（emulator-5554，API 35）：

- `-all` 首行是 `Number of display frames : 64`（默认保留 64 个 display frame），其后逐个 display
  frame 给出 `Jank Type`（`AppDeadlineMissed`、`BufferStuffing`、`SurfaceFlingerCpuDeadlineMissed`、
  `SurfaceFlingerGpuDeadlineMissed`、`DisplayHAL`、`PredictionError`、`Unknown`）、
  `Prediction State`、`Present Metadata`、`Start/Finish Metadata`、`Vsync Period`、
  `Present delta`，以及期望与实际的 `Start time` / `End time` / `Present time` 三列对照。
- **每个 display frame 下还挂着该帧涉及的逐图层条目**（`Layer - TX - <name>#<id>`），每条给出
  `Token`、`Is Buffer?`、`Owner Pid`、`Layer ID`、`Present State`、`Prediction State`、
  `Jank Type`、`Present Metadata`、`Finish Metadata`、`Last latch time` 与同一张三列表。源码依据：
  `FrameTimeline::DisplayFrame::dump` 遍历 `mSurfaceFrames` 调 `SurfaceFrame::dump`；而
  `SurfaceFrame` 由 `Layer::createSurfaceFrameForBuffer` 创建，参数含 `mOwnerPid`、`getSequence()`、
  `mName` 与调试名，故**图层条目自带属主归属**。
- `-jank` 给出同一结构的帧序列。
- 图层条目行尾的 `[*]` 是 dumper 给「jank 类型不为 None」的帧加的标记（AOSP
  `SurfaceFrame::dump`：`Easily identify a janky Surface Frame in the dump`），它**不属于图层名**，
  解析时 MUST 去掉。
- 时刻表里的呈现时刻可能是 `N/A`：dump 那一刻还没呈现的那一帧（实测每次 dump 的最后一帧都是
  这样），MUST 记为「无值」，MUST NOT 当作 0 参与计算。
- 图层条目的呈现时刻与它所属 display frame 的呈现时刻相同（实测 73 条图层行零差异），故归属到
  图层的时刻可直接当作该 display frame 的呈现时刻。

这一层**不需要预知图层名**即可给出 display frame 级口径，同时又能按 `Owner Pid` 归属到具体应用的
图层——它是唯一同时具备「同口径」与「属主归属」的通道。量的是「合成器把帧呈现出去的时刻」，
与应用用什么管线绘制无关：两个对照对象的 HWUI 帧与本项目的 wgpu 直绘帧都落在同一条时间轴上。

### 图层级：`--latency` 逐帧三列

- `SurfaceFlinger::dumpStats` 先写 `mScheduler->getPacesetterVsyncPeriod().ns()`（刷新周期，纳秒），
  再 `traverseLegacyLayers` 遍历 `mLegacyLayers`，对 `layer->getName() == name` **精确相等**的图层
  调用 `Layer::dumpFrameStats`。
- `Layer::getName()` 返回 `mName`，构造时是 `printf("%s#%d", args.name, sequence)`；图层名必须带
  `#<id>` 后缀。不带后缀的名字（如 `StatusBar`）永远匹配不上。
- 图层名的权威来源是 `dumpsys SurfaceFlinger --list`：它逐行打印
  `RequestedLayerState{<name> parentId=…}`，其中 `<name>` 就是 `args.name + "#" + sequence`。
  全量 dump 里的 `Layer [id] <label>` 实测与 `--list` 的 `<name>` 同形（同样带 `#<id>`），但
  `--latency` 认的是 `--list` 的精确串。
- `--latency-clear` 走同一个精确比较；不带名字时清空全部图层的帧历史。
- 输出形态随帧统计开关而变：旧帧统计器路径下环形缓冲 128 槽，固定输出 127 行
  （`NUM_FRAME_RECORDS = 128`，减去当前槽），未填的槽是 `0 0 0`；新帧时间线路径下由
  `TimeStats::generateFrameStats` 给出同样形态的 127 行，末尾另有一个空行。

实测判据（emulator-5554，API 35）：

| `--latency <name>` 输出 | 含义 | 处置 |
| --- | --- | --- |
| 只有首行（vsync 周期） | 图层名未匹配：名字不带 `#<id>`、或图层已销毁 | 判**采集失败**，不得当作「没出帧」 |
| 命令无输出（设备 shell 报错） | 经真实 shell 时名称未加引号：`[` `]` `(` `)` 被设备 shell 当作元字符，参数被拆坏 | 判**采集失败**；经 shell MUST 加引号，不经 shell MUST 原样传入（见下） |
| 首行 + 127 行 | 图层已匹配 | 继续按行有效性解析 |
| 首行 + 127 行且全部 `0 0 0` | 图层匹配但没有内容帧记录（该图层不承载内容，或刚被 `--latency-clear` 清空） | 判**采集失败**，不得当作「该场景没出帧」 |
| 首行 + 若干行且 `desired != 0` | 真实帧 | 解析呈现间隔与超出量 |

行的有效性判据取自源码：旧帧统计器 `getStats` 以 `desiredPresentTime == 0` 跳过空槽，
`isFrameValidLocked` 以 `0 < actualPresentTime < INT64_MAX` 判有效帧。解析时按 `desired != 0`
筛出真实帧即可。

**覆盖面实测（推翻先前结论）**：先前依据「BLAST 图层不在 `mLegacyLayers`」断定 `--latency` 覆盖
不到 SurfaceView，实测不成立。本项目运行中的终端 SurfaceView 图层
`SurfaceView[com.termux/terminal.emulator.MainActivity](BLAST)#215` 用精确名调用 `--latency`
返回 127 行、其中 14 行非零；`--latency-clear` 后归零，终端再次输出后重新累积到 3 行；同一应用的
窗口图层 `com.termux/terminal.emulator.MainActivity#210` 返回 32 行非零。真正决定「有没有数据」
的是**该图层是否承载内容帧**：动画 leash、dim、bounds 等图层同样精确匹配，但行全为零。先前
「9 个图层只有 4 个可寻址」的观察，实际是把「行全零」误记为「不可寻址」。

**对本项目的直接后果**：终端内容帧可由系统侧逐图层通道取得（`--latency` 给逐帧
`desired / actual / ready`，`--frametimeline` 的图层条目给属主归属与呈现时刻），不必只依赖应用
自己的呈现汇总；应用自己的呈现汇总仍作为该应用内部的第二数据面（既有规约「宏基准的帧级数据面按
绘制主体选源」不冲突：那条规约讲的是**应用内**断言用哪一面）。

**两条渲染管线的同通道实测**：同一台设备上，HWUI 路径的系统 Settings（进程 1134）的窗口图层
`com.android.settings/com.android.settings.Settings#256` 用 `--latency` 取到 127 行、其中 41 行
非零；同一图层在 `--frametimeline` 里出现 33 条带 `Owner Pid 1134` 且 `Is Buffer? = 1` 的条目。
本项目的 BLAST SurfaceView 层同样命中这两个通道。故 HWUI 与 SurfaceView 两条管线的终端内容都落在
系统侧逐图层通道内，取数口径一致。

**图层名的取法与两类命令通道的转义**：`--list` 在 Android 15 起的每行是
`RequestedLayerState{<名字> <键>=<值> ...}`，名字要从包装里取出来（名字本身可能含空格，如
`Background for SurfaceView[com.termux/terminal.emulator.MainActivity]#270`）；本项目终端内容层
的名字是 `SurfaceView[com.termux/terminal.emulator.MainActivity](BLAST)#269`，其父层
`SurfaceView[com.termux/terminal.emulator.MainActivity]#268` 实测帧行全为零——内容在带
`(BLAST)` 的子层，故选取 MUST 以「承载内容帧」为据，MUST NOT 按图层类型（父子、名字前缀）认定。

转义判据按命令通道分开：命令经真实 shell（`adb shell`）时名称 MUST 加引号，否则 `[` `]` `(` `)`
被当作元字符，设备 shell 直接报 `syntax error: unexpected '('`；命令不经 shell 时 MUST NOT 加
引号。依据是 AOSP `android/app/UiAutomationConnection.java` 的
`executeShellCommandWithStderr`——它执行的是 `Runtime.getRuntime().exec(command)`，**不经 shell**，
按空白切词、引号原样传进参数、管道不生效（`android/app/UiAutomation.java` 的
`executeShellCommand(String)` 转发到此）。故宏基准里用 `UiDevice.executeShellCommand` 采集时，
图层名原样传入且 MUST NOT 含空白；用 `adb shell` 采集时 MUST 加引号。

**丢帧判据只能用呈现间隔**：BLAST 内容帧的「实际呈现减期望呈现」恒为一个刷新周期左右（实测
p50 16.2ms，刷新周期 16.67ms），逐帧拿 `actual` 与 `desired` 比较会把每一帧都判成迟到，故该差值
MUST NOT 作迟到判据。丢帧数 MUST 取呈现间隔跨过的刷新槽位数（`round(间隔 ÷ 刷新周期) - 1`），
半周期以内的抖动（实测间隔 12.2–20.9ms、p50 16.4ms）MUST NOT 记为丢帧。

**丢帧绝对量不能作阈值**：丢帧数是保留窗口内的绝对量，会随宿主负载与窗口内帧数一起变大——同一
构建、同一手势、同一 AVD 实测：宿主轻到中载（负载均值 3.4–4.3）时保留窗口内丢帧 6–32 帧
（呈现 48–56fps），宿主被并发编译任务抢占时丢 41 帧，宿主被 8 个忙循环超额占用（4 核宿主负载
11–13）时丢 89–113 帧（呈现 29–35fps）。按最差实测放宽的阈值会失去拦截力，按空闲实测收紧的阈值
又会在正常负载下误报。可作阈值的是**槽位比例**（呈现频率 ÷ 设备刷新率）：无量纲的相对量，宿主被
抢占时窗口内帧数与窗口长度一起缩小，比例不会跟着塌。同一组场景实测槽位比例：轻到中载 0.80–0.96，
超载 0.49–0.58。

### 应用级：Perfetto FrameTimeline 的覆盖限制

Perfetto 官方文档 `docs/data-sources/frametimeline.md` 明确写着：「SurfaceViews are currently
**not supported**, but will be, in future.」该数据源给出的是每个应用的 Choreographer 帧
（`actual_frame_timeline_slice` 表带 `layer_name` 与 `process.name`），本项目的终端内容在
SurfaceView 内、未经 Choreographer，故**不在其覆盖内**；两个对照对象走 HWUI `View`，在其覆盖内。
文档同时说明 SurfaceFlinger 自身也有 Expected/Actual 两条轨道，但那些是 display frame 级的合成
轨迹（示例中 `layer_name` 为 NULL），**没有图层归属**，无法归属到某个应用的终端内容层。结论：
Perfetto FrameTimeline 可作对照对象的应用帧归因，但**不可作终端内容的跨应用判据**，也不能用它
断言本项目终端内容「没有帧」。

### 系统侧通道的保留窗口

两个系统侧通道都只保留最近一段历史，不是全量账本：

- `--latency` 每图层一个 127 槽的环形缓冲（`NUM_FRAME_RECORDS = 128` 减当前槽），写满即覆盖最旧的帧。
- `--frametimeline` 只保留最近 64 个 display frame。

实测（emulator-5554，API 35，本项目跑 `timeout 6 yes` 持续输出）：3 秒内应用侧呈现 551 帧、
9 秒内 1089 帧；同期 `--latency` 的非零行数封顶在 127、`--frametimeline` 里该图层的条目封顶在
64——两通道同时饱和。

后果：**系统侧通道能给出时序（呈现间隔、超出量、丢帧），不能给出长突发场景的总帧数**。场景矩阵
里「帧数」类指标 MUST 取应用自己的计数（各应用的帧计数口径不同，见既有规约「宏基准的帧级数据面按
绘制主体选源」），或把场景压进保留窗口内再数行数；MUST NOT 把被窗口截断的行数当作场景总帧数。

### 显示级帧时间线的时刻基准

显示级帧时间线的时刻**不是稳定时钟**：dump 里的时刻以「保留窗口里最旧的一帧」为基准。源码依据：
AOSP `FrameTimeline::dumpAll` 取 `baseTime = mDisplayFrames[0]->getBaseTime()`，而
`DisplayFrame::getBaseTime()` 取该帧自身与其图层条目的最小时刻——即保留窗口里最旧那帧的时刻。
窗口滑动时最旧的一帧被挤出，基准随之前移。

实测（2026-11-21 本机 AVD，API 35 模拟器，终端持续输出期间）：

- 相隔 8 秒的两次 `--frametimeline -all`，最新一帧的相对时刻分别是 1048.19ms 与 1050.37ms——
  都是保留窗口的跨度（约 64 帧 × 16.2ms），而不是「8 秒后」。
- 窗口里最旧一帧的相对时刻恒在 0–25ms（它就是基准）。

后果：**MUST NOT 把 dump 里的时刻直接与墙上时钟或场景起止比较**，否则整体偏移一整个保留窗口的
跨度；要按场景时间窗筛选，MUST 先把时刻换算到稳定时钟。

换算办法（已落成可执行采集）：逐图层的 `--latency` 通道给的是**绝对纳秒时刻**（CLOCK_MONOTONIC，
与 `SystemClock.elapsedRealtime` 同域；实测其最新呈现时刻 17573.89s 与设备 `/proc/uptime`
17587.66s 同量级），而两条通道记录的是**同一批物理呈现**。按呈现时刻序列对齐即可换算出基准：
对每一对时刻求差，落在同一档（0.1ms）的差值即同一批事件，取众数档即基准偏移。实测（同机同日）
终端内容层在帧时间线里的 63 个呈现时刻有 62 个与逐图层通道的绝对时刻在 0.5ms 内一一对上，
基准偏移 17658688.22ms。

对齐后即可按场景时间窗筛选，并可用两条通道做一致性校验：同一图层在两条通道里的呈现时刻
MUST 一一对得上（容差 1ms），对不上即取数出了偏差。

### 三层帧面的采集协议

1. 场景开始前 `dumpsys SurfaceFlinger --latency-clear <layer>` 清空该图层的帧历史——不清空则读到
   的是上一个场景的残留帧；不带图层名时清空全部图层。
2. 场景结束后 `dumpsys SurfaceFlinger --latency <layer>` 取回逐帧数据（名称经真实 shell 时 MUST
   加引号，不经 shell 时 MUST 原样传入），先按上表的判据确认图层已匹配且确有帧记录，再按
   `desired != 0` 筛出真实帧。
3. 同时取 `dumpsys SurfaceFlinger --frametimeline -all`：display frame 级取呈现时序与 jank 分类，
   逐图层条目按 `Owner Pid`（由包名查得）归属到被测应用的内容图层。该面的时刻以保留窗口里最旧
   一帧为基准，取场景时间窗前 MUST 先按上节的办法换算基准；换算后 MUST 用同一图层在逐图层通道
   里的呈现时刻做一致性校验（逐个对得上，容差 1ms）。
4. 图层名候选来自 `dumpsys SurfaceFlinger --list`；取不到精确图层名、或取回的输出表明图层未匹配
   或无帧记录时，判**采集失败**并中止该场景，不得静默跳过、也不得以「没采到」当作「没出帧」。
5. 内容图层按「该应用 `Owner Pid` 下、承载内容帧的那条图层」选取：终端内容在本项目落在 BLAST
   SurfaceView 层，在 HWUI 对照对象落在应用窗口层；两者在图层条目里都是 `Is Buffer? = 1`。
6. SurfaceView 内容帧的 jank 判定 MUST 由呈现间隔与刷新周期比较得出：其图层条目的 `Token` 为
   `-1`、`Prediction State` 为 `None`、`Jank Type` 为 `None`、`Present Metadata` 为
   `Unknown Present`——调度器预测不覆盖此类帧。`Start time` 恒为 `N/A`，`Present time` 偶为
   `N/A`（此时以 `Last latch time` 兜底），MUST NOT 把 `N/A` 当作 0 参与计算。
7. 由此可算：呈现间隔（相邻呈现时刻之差）、距 deadline 的超出量、丢帧（呈现间隔超过刷新周期）。
   帧数类指标 MUST 取应用自己的计数（系统侧通道受保留窗口限制，见上节），MUST NOT 把被窗口截断
   的行数当作场景总帧数。

## 四、采集面清单

| 面 | 采集命令 | 输出 | 判据用法 |
| --- | --- | --- | --- |
| 显示级帧时序 | `dumpsys SurfaceFlinger --frametimeline -all` / `-jank` | 逐 display frame 的期望/实际起止与呈现时刻、jank 分类；每个 display frame 下挂逐图层条目（`Owner Pid`、`Layer ID`、`Is Buffer?`、呈现状态与时刻） | 跨应用呈现时序分位数、jank 归因、按属主归属到内容图层 |
| 图层级帧时序 | `dumpsys SurfaceFlinger --latency <layer>` / `--latency-clear <layer>` | 首行 vsync 周期 + 逐帧三列纳秒（127 个槽位行，末尾一空行） | 对承载内容帧的图层一律有效（含 BLAST SurfaceView 层）；帧数、呈现间隔分位数、超出量分位数 |
| 应用内帧时序 | `dumpsys gfxinfo <pkg> framestats` | HWUI 帧统计与分位数 | 仅 Compose / HWUI 界面（既有规约同口径） |
| 进程状态 | `dumpsys meminfo <pkg>`、`dumpsys procstats`、`/proc/<pid>/status` | PSS / RSS / 匿名与文件占用 | 内存峰值与回收行为 |
| 能耗与热 | `dumpsys batterystats --checkin`、`dumpsys battery`、`dumpsys thermalservice` | 按 UID 统计、热状态、散热设备 | 同设备内能耗对比、热节流归因 |
| 崩溃与退出 | `dumpsys activity exit-info <pkg>`、`dumpsys dropbox --print`、`logcat -b crash,system,main,events,radio` | 退出 reason、崩溃记录、ANR | 归因第一手依据 |
| 日志与制品 | `logcat -b all` 全量落盘（不筛选、不限级）、`screenrecord` | 全量日志、视频 | 日志分析输入；视频只作人工复核制品，不作判据 |
| 深度 trace（可选真机） | Perfetto：`linux.ftrace`、`linux.process_stats`、`android.surfaceflinger.frametimeline`、`battery counters` | 调度、内存计数、帧时间线、功耗 | 归因深挖，不作常规判据 |

Perfetto 数据源的一手约束（取自上游仓库 `docs/data-sources/`）：

- `linux.process_stats`：轮询间隔必须大于 100 毫秒，否则进程本身成为噪声；计数器名为
  `mem.rss`、`mem.virt`、`mem.rss.anon`、`mem.rss.file`、`mem.swap`、`mem.rss.watermark`。
- `android.surfaceflinger.frametimeline`：给出 jank 类型——`AppDeadlineMissed`、`BufferStuffing`
  （应用提交节奏）、`SurfaceFlingerCpuDeadlineMissed`、`SurfaceFlingerGpuDeadlineMissed`、
  `DisplayHAL`、`PredictionError`、`Unknown`。同一份文档明确写着「SurfaceViews are currently
  **not supported**, but will be, in future.」——该数据源不覆盖 SurfaceView 内的帧。
- `android.log`：只在 userdebug 构建可用，正式版真机不可用；改用 logcat 全量落盘。
- ATrace 每事件开销 1–10 微秒（字符串化、JNI、用户态到内核态写标记），因此禁止在渲染热路径
  铺开埋点——既有规约「日志不得在热路径产生开销」同向。
- `gpu.counters` 的 counter ID 由设备描述符 `GpuCounterSpec` 给定，跨设备不可比。

### Perfetto 设备端采集与分析配方

设备端 `perfetto` 命令（官方 `PERFETTO(1)` 文档口径）有两种配置方式：

- **轻量模式**：`adb shell perfetto [ --time TIMESPEC ] [ --buffer SIZE ] [ --size SIZE ]`
  `[ --app APP_NAME ] [ATRACE_CAT | FTRACE_GROUP/FTRACE_NAME]...`。默认时长 10 秒，默认
  环形缓冲 32 MB；`--size` 给文件上限，`--app` 限定应用级 atrace 事件；事件类别随 Android
  版本而变（例：窗口管理类别 `wm`；ftrace 事件 `sched/sched_switch`）。
- **正常模式**：`adb shell perfetto [ --txt ] --config CONFIG_FILE`，配置用 `-` 时从 stdin 读，
  按 `trace_config.proto` 的 `data_sources.config` 组织，每个数据源一个 `name`。官方把 `--txt`
  标注为「不是稳定 API，不建议用于生产」——故采集脚本应优先用轻量模式，或在正常模式下用二进制
  配置。

会话与制品控制：`--out` 指定落盘路径（或 `-` 走 stdout）、`--no-clobber` 不覆盖既有文件、
`--background` / `--background-wait` 后台录制（后者最多等 30 秒确认数据源就绪）、`--query`
查服务状态、`--attach` / `--detach` / `--stop` 把长会话与采集进程解耦、`--clone` /
`--clone-by-name` 造只读克隆、`--add-attribute` 给 trace 打标（进入 `metadata` 表的
`trace_attribute.*` 行）、`--upload` 走框架上报路径。

分析侧：`trace_processor` shell（`./trace_processor trace.perfetto-trace`）打开 PerfettoSQL，
标准表为 `slice` 与 `counter`；trace 文件也可以是含多个 trace 的 ZIP/TAR，会被合并到同一时间轴；
批量分析用 Batch Trace Processor（Python 库，面向上千个 trace、秒级查询）。

对本仓的用法边界：常规判据仍取系统 dump（显示级 `--frametimeline`、图层级 `--latency`、
meminfo 与 exit-info），Perfetto 只在归因深挖时开；轻量模式的 atrace 类别不得进渲染热路径
（每事件 1–10 微秒）。另注：`--frametimeline` 的显示级帧时间线没有清空入口（dump 参数表只把
`-jank` / `-all` 转发给 `FrameTimeline::parseArgs`），它只保留最近 64 个 display frame，故按场景
取数 MUST 用时间窗筛选，不能靠「先清空」。

设备侧已实测可用的通道（emulator-5554，API 35）：`/system/bin/perfetto` 存在；
`dumpsys thermalservice` 提供 `inject-temperature` / `override-status` / `headroom`；
`cmd display set-user-preferred-display-mode` 可锁刷新率；`cmd power set-fixed-performance-mode-enabled`
可开固定性能模式；`dumpsys battery`、`dumpsys activity exit-info`、`dumpsys SurfaceFlinger --list`、
`dumpsys dropbox --print` 均可用；`adb emu` 控制台提供 `avd snapshot` 等设备侧操作。该设备上未装
任何对照应用——对照 APK 必须外部下载，不入库。

## 五、全场景矩阵

同一场景给三个应用**完全相同**的字节流与网格尺寸；负载不等价则对比无效。

| 编号 | 场景 | 统一负载 | 指标 | 采集面 | 判据 | 硬上限 |
| --- | --- | --- | --- | --- | --- | --- |
| S1 | 冷启动 | 杀进程后 `am start -W` | 启动到首帧、退出归因 | `am start -W`、显示级 frametimeline、exit-info | 首帧在预算内、无异常退出 | 60 秒 |
| S2 | 温启动 | Activity 重启不杀进程 | 首帧、帧数 | 显示级 frametimeline、应用内首帧探针 | 首帧在预算内 | 30 秒 |
| S3 | 热启动 | 进程存活切前台 | 首帧、丢帧数 | 显示级 frametimeline、应用内呈现汇总 | 无丢帧、无 Surface 丢失 | 15 秒 |
| S4 | 稳态空载 | 静置 10 秒 | 帧数、CPU 占用、内存 | 显示级 frametimeline、呈现汇总、meminfo | 空载不产生持续绘制 | 15 秒 |
| S5 | 持续输出 | 连续字节流（逐行） | 吞吐、呈现间隔 p95、内存峰值 | 显示级 frametimeline、呈现汇总、meminfo | 无丢帧、内存有界 | 60 秒 / 8 MB |
| S6 | 突发输出 | 单次大块字节流 | 无损、内存峰值、退出归因 | meminfo、exit-info、logcat | 输出无损、无异常退出 | 30 秒 / 8 MB |
| S7 | 长滚动 | 填满回滚区后拖动 | 呈现间隔 p95、丢帧 | 显示级 frametimeline、呈现汇总 | p95 在预算内、无停摆 | 30 秒 |
| S8 | 宽字符行 | CJK、组合字符、emoji 行 | 排版正确、呈现间隔 p95 | 显示级 frametimeline、呈现汇总、截图复核 | 无错位、无丢帧 | 30 秒 |
| S9 | 交互延迟 | 探针命令 + 计时回显 | 按键到回显间隔 | logcat 时间戳 | 间隔在预算内 | 30 秒 |
| S10 | 输入洪水 | 连续按键事件 | 丢键数、ANR | exit-info、logcat | 无丢键、无 ANR | 30 秒 |
| S11 | 后台前台切换 | 切后台再切回 + 旋转 | 恢复后首帧、丢帧 | 显示级 frametimeline、呈现汇总 | 恢复即出帧、无 Surface 丢失 | 30 秒 |
| S12 | 资源极端 | 低内存、强压、灭屏、Doze | 存活、快速死亡、归因 | exit-info、batterystats、thermalservice | 快速死亡且归因明确 | 60 秒 |

S9 的探针口径沿用既有规约：终端内的 shell 没有提示符可等，必须用幂等的探针命令诱发一次输出作
就绪信号，不得用固定等待时长替代。

矩阵里的「呈现汇总」指各应用自己的帧统计：本项目取原生渲染器的呈现汇总（终端内容直绘
SurfaceView，经 HWUI 的 `gfxinfo` 记不到），对照对象取其 HWUI 帧统计。跨应用呈现时序一律取系统侧
逐图层通道（`--latency` 逐帧三列 + `--frametimeline` 的图层条目并按 `Owner Pid` 归属）。
帧数类指标受系统侧保留窗口限制（见「系统侧通道的保留窗口」），跨应用对比帧数 MUST 用各应用
自己的计数口径。

## 六、环境固定化

- **刷新率**：`cmd display set-user-preferred-display-mode` 锁定，并记录实际生效值。
- **性能档位**：`cmd power set-fixed-performance-mode-enabled true`；关闭三类动画缩放。
- **温度**：`cmd thermalservice inject-temperature` / `override-status` 注入并记录实际热状态；
  热节流会把性能差异伪装成应用差异。
- **电池与充电**：每轮记 `dumpsys battery`；低电量时宏基准类采集会直接报错（官方口径）。
- **屏幕与网络**：统一亮屏或统一灭屏；统一网络状态，避免后台下载制造噪声。
- **字体与网格**：三应用统一字体族与字号；同一列行数由同一 resize 序列产生。
- **每轮元数据**：设备型号、ROM 构建号、API、刷新率、电池、热状态、构建标识（APK 校验和）。
- **宿主 CPU 占用**：模拟器跑在宿主上，宿主被并发任务抢占时测得值整体变慢——实测冷启动
  十轮中位数从 2.0s 涨到 3.5s、终端内容层刷新槽位比例从 0.91 降到 0.59。故每轮 MUST 记录
  宿主负载，判定 MUST 用负载无关的相对量或覆盖该情形的余量，MUST NOT 把宿主抖动当成应用
  回归，也 MUST NOT 用只能拦「卡死级」的宽阈值掩盖它。

## 七、失败归因规则

| 类别 | 判定依据 | 处置 |
| --- | --- | --- |
| A 应用归因 | exit-info reason 为 `REASON_CRASH`、`REASON_CRASH_NATIVE`、`REASON_ANR`；FrameTimeline 判为 `AppDeadlineMissed`；logcat 有本应用异常 | 判失败，修根因 |
| B 系统归因 | reason 为 `REASON_LOW_MEMORY`、`REASON_MEMORY_LIMITER` 且同批其它进程同刻被杀；FrameTimeline 判为 `DisplayHAL`、`SurfaceFlingerGpuDeadlineMissed`、`PredictionError`；dropbox 有 system 侧记录 | 不计为本应用失败，但必须记录证据 |
| C 人为与编排归因 | reason 为 `REASON_USER_REQUESTED`、`REASON_USER_STOPPED`、`REASON_PACKAGE_UPDATED`、`REASON_PACKAGE_STATE_CHANGE`、`REASON_PERMISSION_CHANGE`、`REASON_OTHER`、`REASON_SIGNALED` | 不计为本应用失败 |
| D 采集失败 | 命令返回空或超时（设备未就绪、未解锁、低电量、命令被 shell 语义毁掉） | 判红并标为采集失败，不得当作「被测路径没发生」 |

`REASON_OTHER` 的官方口径是「既不是应用的问题，也不是应用能操作的」，故直接归 C。
证据不足时记「未定」并保留全部制品，不强行归因——这与「不隐藏错误」同向：归因不清不等于
没有问题。

### 进程退出归因通道的实测语义

`dumpsys activity exit-info <包名>` 的实测语义（2026-11-21 本机 AVD，API 35 模拟器；
格式依据 AOSP `ApplicationExitInfo.dump` 与 `AppExitInfoTracker` 源码）：

- 输出按 `ApplicationExitInfo #N` 分块，块内三行分别是时刻、原因与明细；包没有退出历史
  时只有 dump 头（连 `package:` 段都没有），故「无记录」与「命令失败」须分开判。
- 时刻是墙钟本地时间（`yyyy-MM-dd HH:mm:ss.SSS`，设备时区与日历），不是单调时钟；与
  场景起止比较须同取墙钟，两者同机同时区故可直接比。
- 记录在进程死亡后立即可见（实测强停后立即读取即有记录，无轮询延迟）。
- 原因码是公开常量（`REASON_EXIT_SELF` 至 `REASON_PACKAGE_UPDATED`），子原因码是
  `@hide` 常量，只能按 dump 给出的名字读。`REASON_MEMORY_LIMITER` 自 android-37.2 起
  才公开（android-37.0 的 SDK 无此常量），未公开的新原因码归「未知」并判红，不猜语义。
- `pss`／`rss` 是系统侧按单位舍入的显示值（`0.00`、`50MB`），不是精确字节数；进程在
  死亡前没被采样时为 `0.00`。

## 八、极端场景的快速失败与资源上限

- 场景级 wall-clock 上限（见矩阵表）→ 超时即终止被测进程并取 exit-info，不无限等待。
- 输出字节上限（文本 8 MB、trace 32 MB）→ 超限终止并记「超限终止」，不继续追加。
- 采样下限：`process_stats` 轮询不小于 100 毫秒；`gfxinfo` 每场景取一次；日志按 buffer 分文件落盘。
- 存储配额：每轮制品目录有配额并轮转清理，trace 超限即丢弃，不占满设备。
- 并发上限：同一时刻只跑一个被测应用；对照基线与应用交替执行，避免宿主抢占互相污染。
- 失败快速传播：adb 命令自身带超时；仪器测试侧用 `UiDevice.executeShellCommand` 时命令不含引号、
  管道与重定向（既有规约「命令 MUST NOT 含引号、管道、重定向等 shell 语法」）。

## 九、无人工干预的编排

```text
for device in targets:
    prepare(device)            # 解锁、刷新率、固定性能、温度、动画、网络、输入法
    for app in [本项目, termux, ghostty-android]:
        install(app)           # 外部下载的 APK，不入库；与被测应用撞包名时先卸载再装
        reset_state(app)
        for scenario in matrix:
            for iteration in 1..N:
                reset_measurement(device, app)   # 杀进程、清日志、清图层帧历史、记 exit-info 基线
                run_scenario(app, scenario, watchdog=limit)
                collect(device, app, scenario)
                persist_row(jsonl)
                classify(row)
    teardown(device)
```

**包名撞名的处置**：上游 Termux 的包名是 `com.termux`，与本项目的 `applicationId` 相同；同设备
安装实测报 `INSTALL_FAILED_VERSION_DOWNGRADE`（对照构建版本码更低），版本码更高时会直接替换被测
应用。故同一台设备上 MUST 按应用轮换安装（装 A 跑完所有场景 → 卸载 → 装 B），或给对照构建改包名
后并存；两种做法都 MUST 记录 APK 校验和与安装顺序，因为卸载重装会清空应用状态。

- **顺序**：同场景下应用顺序交替（ABAB）以抵消宿主漂移；每个场景重复 N 次，取 p95 / p99，不取均值。
- **就绪判定**：每个场景先确认应用可交互（探针命令有回显），再开始计时。
- **落地位置**：本仓已有 `android/benchmark`（`com.android.test` 模块，宏基准与仪器化采集面）与
  `scripts/test-emulator.nu`（设备侧编排）。跨设备多应用编排需要新增设备侧脚本，而 `scripts/`
  属受保护目录，故本报告只登记建议位置，不擅自落地。

## 十、数据落地格式

每轮产出一份 JSONL（每行一次测量）与一个制品目录。JSONL 只放摘要与制品指针，原始制品按
`run_id / scenario / iteration` 分目录存放并附校验和。

| 字段 | 含义 |
| --- | --- |
| `run_id` | 本轮唯一标识 |
| `device` | 序列号、型号、API、ROM 构建号、刷新率、电池、热状态 |
| `app` | 包名、版本、APK 校验和、应用类别（本项目／对照） |
| `scenario` | 场景编号、迭代序号、起止时间、是否超限终止 |
| `metrics` | 启动时长、首帧时长、帧数与呈现间隔分位数、超出量分位数、内存 PSS／RSS、能耗 |
| `exit` | exit-info reason 与归因类别 |
| `artifacts` | 制品名、路径、字节数、校验和 |
| `verdict` | 判定、归因、备注 |

日志纪律：采集侧全量落盘、不筛选、不限级；按 tag 或关键字的过滤只发生在分析侧。

## 十一、不采纳的测量手段

| 手段 | 不采纳理由 |
| --- | --- |
| 宏基准的 `FrameTimingMetric` / `StartupTimingMetric` 作跨应用主判据 | 官方明确不推荐在模拟器上跑；目标应用须非 debuggable；两者量的是各目标自身管线的帧，跨应用语义不同；既有规约已把它定位为「模拟器数量级回归」用途 |
| `dumpsys gfxinfo` 的帧数作终端内容帧判据 | 只统计 HWUI 管线帧，SurfaceView 直绘不在其中（既有规约同口径） |
| Perfetto `android.log` 数据源 | 仅 userdebug 构建可用，正式版真机不可用；改用 logcat 全量落盘 |
| `gpu.counters` 跨设备对比 | counter ID 由设备描述符给定，跨设备不可比 |
| `PowerMetric` 跨设备对比 | 依赖具体硬件与充电状态 |
| atrace counter 与 FrameTimeline 关联 | 既有规约已登记为不采纳（真机排障可选项） |
| `screenrecord` 作判据 | 编码与分辨率引入偏差，只作人工复核制品 |
| 引用任一对照对象的自述性能数字 | 两者均未发布可核实的性能数字 |
| Perfetto `android.surfaceflinger.frametimeline` 作终端内容帧的跨应用判据 | 官方文档明确 SurfaceView 不受 FrameTimeline 支持：本项目终端内容在 SurfaceView 内不在其覆盖内，对照对象走 HWUI 管线在其覆盖内——用它比较等于比较两套不同的数据面 |
| Perfetto FrameTimeline 的 SurfaceFlinger 侧轨道作终端内容判据 | 该轨道的切片是 display frame 级合成轨迹（示例中 `layer_name` 为 NULL），没有图层归属，无法归属到某个应用的终端内容层 |
| 测试进程内的定长 CPU 循环作宿主速度参照、据以归一化启动耗时 | 实测宿主超额占用时该参照只慢 28%–70%（21ms → 28–37ms）且同一轮次内自身抖动近一倍，而冷启动中位数慢了 78%–84%（2011ms → 3468–3694ms）：启动是多线程且含 I/O 的延迟敏感过程，参照是单线程吞吐敏感的，二者不成比例，按参照折算会把宿主慢误判成应用慢 |

## 十二、与本仓既有资产的关系

- 已有：`android/benchmark`（`AppStartupBenchmark`、`ScrollPerformanceBenchmark`、
  `InteractionAnimationBenchmark`、`BenchmarkSupport`、`FrameStats`、`PresentedFrames`、
  `SurfaceLayerFrames`）、`native/benches`（`cell_builder`、`vt_typing`、`cjk_resolve`、
  `support`）、`scripts/test-emulator.nu`。
- 本轮落地：`SurfaceLayerFrames` 把图层级通道做成了可执行资产——按 `--list` 取图层名（从
  `RequestedLayerState{...}` 包装里取名字）、逐个候选查帧记录并取承载内容帧的那条、场景前
  `--latency-clear` 清空历史、场景后解析逐帧三列并按呈现间隔折算丢帧、呈现频率与槽位比例；
  滚动宏基准据此断言终端内容层在滚动窗口内确有帧记录，且槽位比例不低于 0.35。阈值用相对量而不用
  绝对丢帧数（见上节「丢帧绝对量不能作阈值」）：实测（2026-11-21 本机 AVD，API 35 模拟器，
  软件渲染）轻到中载宿主槽位比例 0.80–0.96、超载宿主 0.49–0.58，阈值取最差实测的约 1.5 倍余量。
  该采集按图层名里的应用标识归属到本应用（宏基准只测本应用）；跨应用对比的通用归属走显示级帧
  时间线的 `Owner Pid` 条目（已落成可执行采集，见下条）。
- 本轮落地：启动宏基准的判定口径改为「各轮中位数对预算、最差一轮对硬上限」，并把启动耗时
  分成首帧段（`startActivityAndWait` 走 `am start -W` 的系统侧口径）与终端界面可见段
  （UiAutomator 检测口径）分别记录。此前是逐轮硬判，宿主被抢占时整轮变慢即判红（实测宿主
  超额占用时冷启动中位数 3.5s、逐轮峰值 4.5s，原预算 3.4s）；改后同一构建在同一宿主上通过。
  分段记录还分清了「应用慢」与「检测慢」：宿主超载时首帧段从约 1.0s 涨到约 2.9s，检测段基本
  不变。宿主速度参照归一化经实测不成立，不采纳（见第十一节）。
- 本轮落地：显示级帧时间线也做成了可执行资产——`FrameTimelineFrames` 解析 `--frametimeline -all`
  的逐 display frame 与逐图层条目（`Owner Pid`、`Layer ID`、`Is Buffer?`、呈现状态与呈现时刻），
  按属主归属到被测应用的内容图层；并按「保留窗口里最旧一帧为基准」的事实把相对时刻换算到稳定
  时钟——与逐图层通道的绝对时刻按呈现时刻序列对齐（实测 63 个呈现时刻有 62 个在 0.5ms 内对上）。
  滚动宏基准据此断言：场景时间窗内有 display frame、终端内容层被归属到本应用、两条系统侧通道记录
  的呈现时刻逐个对得上（容差 1ms）。实测该宏基准三轮各归属到 63 帧终端内容。
- 本轮落地：进程退出归因也做成了可执行资产——`ExitInfoRecords` 解析 `dumpsys activity exit-info`
  的分块格式（`ApplicationExitInfo #N` 的块内三行一组），按 reason 码分四类归因（应用故障、环境、
  预期、未知），并按场景时间窗筛选退出记录；宏基准的场景执行包在「窗口内退出即判红」与失败归因
  里——冷启动、温启动、滚动与三组交互动画每轮记录窗口内退出数与归因，场景本身失败时补挂退出归因。
  端到端验证用「APK 内缺 native 库 → 应用启动即崩」构造真实崩溃：冷启动宏基准按预期判红并给出
  `APP CRASH(EXCEPTION)` 的原因、子原因与描述；恢复 native 库后重新通过。
- 缺：跨应用对比面；场景矩阵与制品落盘规范；跨设备多应用编排（落点在受保护的
  `scripts/`）。
- 与既有规约一致性：本报告不改「模拟器只作数量级」「帧级数据面按绘制主体选源」「采集为空即
  判红」「设备侧命令不含 shell 语法」「不得用固定等待替代就绪信号」等既有口径，只在其上补
  跨应用一层。

## 十三、结论

跨终端性能对比在本项目是**可做**的，但前提是选对观测面：呈现时序走系统侧——逐图层用
`dumpsys SurfaceFlinger --latency`（名称精确到 `<base>#<id>` 且加引号），display frame 级用
`dumpsys SurfaceFlinger --frametimeline`（并按 `Owner Pid` 归属到内容图层），而不是各家自报的
帧率；进程状态、能耗、退出归因走系统 dump，而不是各家自报的内存与耗电。
本项目终端内容直绘 SurfaceView，不进 HWUI 的 `gfxinfo`，也不在 Perfetto FrameTimeline 的覆盖内
（官方明确 SurfaceView 不支持），但**在系统侧逐图层通道内**：实测终端 SurfaceView（BLAST）图层
用精确名即可取到逐帧三列，`--latency-clear` 后归零、终端再次输出后重新累积；其 jank 判定 MUST 由
呈现间隔与刷新周期比较得出，因为 SurfaceView 帧不参与调度器预测（`Jank Type` 恒为 `None`）。
两个系统侧通道都只保留最近一段历史（`--latency` 每图层 127 行、`--frametimeline` 最近 64 个
display frame），故它们给的是**时序**而非长突发场景的总帧数；帧数 MUST 取各应用自己的计数口径。
显示级帧时间线的时刻以保留窗口里最旧一帧为基准，窗口滑动时基准前移，故它**不是稳定时钟**：
实测相隔 8 秒的两次 dump，最新一帧的相对时刻都是约 1.05 秒（保留窗口的跨度）。按场景时间窗筛选
MUST 先换算基准——逐图层通道给的是绝对纳秒时刻，两条通道记录同一批物理呈现，按呈现时刻序列
对齐即可换算出（实测偏移 17658688.22ms，63 个时刻里 62 个在 0.5ms 内对上），换算后 MUST 用两条
通道的呈现时刻逐个校验。
对照对象没有任何可复用的基准资产，对比必须自建 harness；本仓已有的宏基准模块与设备侧编排是
落点，图层级通道已在本仓落成可执行采集（`android/benchmark` 的 `SurfaceLayerFrames`），跨应用
扩展受 `scripts/` 受保护约束，须维护者明确同意后实施。失败归因必须落到
exit-info reason、FrameTimeline jank 类型、logcat 与 dropbox 四源交集，证据不足记「未定」而非
猜测；极端场景要有硬超时与输出上限，宁可快速死亡并留下证据，也不在失败场景上继续消耗设备。
进程退出归因通道也已落成可执行采集（`android/benchmark` 的 `ExitInfoRecords`）：dump 的分块格式、
墙钟时刻语义与公开 reason 码范围都经实测确认，场景时间窗内出现进程退出即判红并附归因——端到端
验证用「APK 内缺 native 库 → 启动即崩」构造出真实崩溃，冷启动宏基准给出 `APP CRASH(EXCEPTION)`
归因，而不是笼统的「跑挂了」。
