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
6. **启动对比必须双记墙钟与进程 CPU 时间**：宿主被抢占时墙钟整体变慢而进程自身 CPU 时间
   几乎不变（实测跨应用冷启动五轮：墙钟中位数从 856ms 涨到 2191–2275ms，进程 CPU 时间只从
   730ms 涨到 970–1020ms）。负载只记录不折算，判定用负载无关的相对量。

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
| 启动耗时（系统侧） | `cmd package resolve-activity --brief <包名>`、`am start -W -n <包名>/<类名>` | 启动状态、启动态、实际被拉起的 Activity、TotalTime、WaitTime | 跨应用启动耗时的同口径量法（各应用自报的启动指标跨应用不可比） |
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

### 跨应用启动对比面：系统侧口径与实测形态

启动耗时是跨应用对比里最容易取错口径的量：各应用自报的启动指标量的是各自管线内的耗时
（宏基准的 `StartupTimingMetric` 量的是它自己的探针），跨应用不可比。可比口径只有系统侧的
`am start -W`——它的 TotalTime 与被测应用无关，同一台设备上依次测各应用即得横向可比的数据。
本仓已把该面落成可执行资产（`android/benchmark` 的 `AppLaunch` 与
`CrossAppLaunchBenchmark`），实测语义如下：

- 启动入口由 `cmd package resolve-activity --brief <包名>` 解析：输出首行是匹配信息
  （`priority=0 preferredOrder=0 match=0x108000 specificIndex=-1 isDefault=false`），第二行
  才是 `包名/类名`；未安装的包输出 `No activity found`，据此判失败而不回退到别的入口。
- `am start -W` 的输出给出 `Status`、`LaunchState`、`Activity`、`TotalTime`、`WaitTime`；
  状态不是 `ok` 即判失败——启动没成功时耗时字段没有意义，读成 0 会把「没启动」记成
  「启动很快」。
- 冷启动的准备动作（`am force-stop`）必须落在测量窗口之外：强停自身会在 exit-info 里留下
  一条 `USER REQUESTED / FORCE STOP` 的退出记录，落在窗口内会让该轮按「窗口内进程退出」
  判红。这是本轮实测暴露的：首轮跨应用测量即因这条强停记录判红，把准备动作移到窗口
  外后恢复。
- 权限弹窗会抢走启动：被测应用首次启动时系统弹出通知权限窗，`am start -W` 报出的
  `Activity` 变成权限控制器的 `GrantPermissionsActivity`——量到的不是被测应用的启动耗时。
  测量前须预授权限：授掉包 dump 里未授权的运行时权限，并显式授一次通知权限（目标 SDK
  低于 33 的应用在安装时已被系统自动授予该权限，但首次建通知渠道时系统仍会弹窗，显式
  授予才会让系统记住用户的选择）；授完再读一次包 dump，授不了的权限（受限权限等）记进
  测量行，由「拉起的是不是被测应用」这条判据兜底。
- 首次启动含 dex 优化与首次初始化，故每个包先跑一次不落盘的预热启动，再开始记行。
- 拉起后须校验启动态是 `COLD`（强停后拉起才成立），不是冷启动即判红。
- **总耗时行可能缺失**：AOSP `ActivityManagerShellCommand` 只在 `result.totalTime >= 0` 时
  打印 `TotalTime` 行，系统没测到这次启动时该行不存在——实测「APK 缺 native 库 → 启动即崩」
  的形态是 `Status: ok` / `LaunchState: UNKNOWN (0)` / `Activity: …` / `WaitTime: …` /
  `Complete`，没有 `TotalTime` 行。故总耗时按可空取：缺失时显式判失败，MUST NOT 读成 0
  （读成 0 会把「系统没测到」记成「启动很快」）。
- 启动态 `UNKNOWN` 与启动即崩溃同现：实测强停后立即拉起、间隔 1 秒拉起均为 `UNKNOWN` 且
  无总耗时；恢复 native 库后为 `COLD` 且有总耗时——`UNKNOWN` 是「系统没测到这次启动」的信号。
- **墙钟之外还要记进程 CPU 时间**：宿主被抢占时墙钟整体变慢而进程自身 CPU 时间几乎不变
  （见第六节的实测对照），故每轮记 `/proc/<pid>/stat` 的 utime 与 stime 合计，作为宿主无关
  的工作量口径。
- 实测（2026-11-22 本机 AVD，API 35 模拟器，软件渲染，同一轮内先后测两个包）：本项目
  `com.termux` 五轮系统侧 TotalTime 为 744–920ms（p50 892ms、p95 920ms），系统应用
  `com.android.settings` 为 624–735ms；十轮全部无退出记录、判定为 PASS。模拟器上的绝对量只作
  数量级参考（既有规约同口径），横向关系才是此处要的。
- 同一轮里多包并测时，行的迭代序号按场景名连续递增（先测的包占前若干号），故横向比较
  按 `app.package` 分组。

## 五、全场景矩阵

同一场景给三个应用**完全相同**的字节流与网格尺寸；负载不等价则对比无效。

矩阵已落成仓内可执行资产（`android/benchmark` 的 `ScenarioMatrix`）：12 类场景连同统一负载、
指标、采集面、判据、wall-clock 上限、输出字节上限与采样间隔下限都是数据，测量行引用矩阵
编号；`ScenarioBudget` 执行这两道上限，超限即终止场景并记「超限终止」（见第八节）。

| 编号 | 场景 | 统一负载 | 指标 | 采集面 | 判据 | 硬上限 |
| --- | --- | --- | --- | --- | --- | --- |
| S1 | 冷启动 | 杀进程后 `am start -W` | 启动到首帧、退出归因 | `am start -W`、显示级 frametimeline、exit-info | 首帧在预算内、无异常退出 | 60 秒 |
| S2 | 温启动 | Activity 重启不杀进程 | 首帧、帧数 | 显示级 frametimeline、应用内首帧探针 | 首帧在预算内 | 30 秒 |
| S3 | 热启动 | 进程存活切前台 | 切前台耗时、内容层帧数 | `am start -W`、系统侧逐图层、gfxinfo | 切前台在预算内、内容层出帧 | 15 秒 |
| S4 | 稳态空载 | 静置 10 秒 | 帧数、CPU 占用、内存 | 显示级 frametimeline、呈现汇总、meminfo | 空载不产生持续绘制 | 30 秒 |
| S5 | 持续输出 | 设备侧 shell 持续产出（60 批 × 1000 行，批间 0.1 秒） | 吞吐、呈现间隔 p95、刷新槽位占用、进程 CPU 时间、内存峰值 | 呈现汇总、系统侧逐图层、显示级 frametimeline、meminfo、`/proc` 进程统计 | 吞吐在预算内、无停摆、内存有界 | 60 秒 / 8 MB |
| S6 | 突发输出 | 设备侧 shell 一次产出（50 万行，约 3.9MB，无节流） | 消费时长、无损、呈现间隔 p95、内存峰值、退出归因 | 会话标题、呈现汇总、系统侧逐图层、显示级 frametimeline、meminfo、exit-info、logcat | 流末尾被终端消费到、呈现不断档、内存有界、无异常退出 | 30 秒 / 8 MB |
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

### 热启动的取数口径与实测形态

热启动不新建 Activity，宏基准自带的启动等待认不出这次启动：系统侧 `am start -W` 报
「Warning: Activity not started, its current task has been brought to the front」，框架便退化成
固定等两秒再返回（`androidx.benchmark.macro.MacrobenchmarkScope.amStartAndWait` 的
「Unable to safely detect Activity launch, waiting 2s」分支）。实测每次启动都睡满两秒，测得的
「启动耗时」中位数 2838ms，而同一次启动的系统侧总耗时只有 113ms——量到的是固定等待，不是应用。

故热启动走系统侧 `am start -W`：取 `TotalTime` 作切前台耗时，并校验启动态是 `HOT`——启动态不是
`HOT` 说明这轮量到的不是切前台（冷启动、或者系统没测到这次启动），当场判红而不是记成一份
「热启动很慢」的数字。

终端内容层的 Surface 随界面回到前台重新创建：实测按 home 之后内容层图层从图层清单里消失
（`SurfaceView[com.termux/terminal.emulator.MainActivity](BLAST)#204` 不再出现），切回前台后
以新的图层号重建（`#230`）。故内容层帧 MUST 取启动之后的图层：窗口开始前取名会直接失败，旧
图层名在新图层上也不再命中。实测每轮内容层有 3 帧、`gfxinfo` 有 3–5 帧。

实测（2026-11-22 本机 AVD，API 35 模拟器，软件渲染）十一轮（含框架的准备轮次）：切前台耗时
中位 113ms、终端界面可见中位 858ms、总耗时 929–1568ms（中位 971ms）。

### 稳态空载的判据面与实测形态

空载要判的是「不画东西」，而本项目的终端内容直绘 SurfaceView：`gfxinfo` 记不到那些帧，
只看 `gfxinfo` 会把「终端内容还在刷」误判成空载。故空载取三个互相独立的面——系统侧逐
图层的终端内容帧、应用进程的 `gfxinfo` 帧数、`/proc/<pid>/stat` 的进程 CPU 时间，另记
`dumpsys meminfo` 的 PSS 与 RSS。

就绪信号用「终端内容层真的画出了第一帧」，不拿固定等待当就绪；进静置窗口前另留三秒让
启动余波（输入法弹出动画、shell 首次提示符）散尽，量到的才是稳态。

实测（2026-11-22 本机 AVD，API 35 模拟器，软件渲染）三轮：静置十秒内界面帧 0 帧、终端
内容层 0 帧、进程 CPU 时间 150ms（三轮一致）、PSS 66–71 MB、RSS 166–175 MB。

本场景是普通仪器测试而不是宏基准：宏基准的帧类指标在「一帧都没出」的窗口里没有样本，
框架会因「读不到任何指标」直接报错；空载要的恰恰是「没有帧」这个事实。

### 持续输出的判据面与实测形态

持续输出量的是「字节流一直在来」这条路径：负载由设备侧 shell 自己产出（`for` 循环里
逐批 `seq`，批末写一条完成标记），产能不依赖宿主注入速度，各应用拿到同一串字节；批间
停固定时间，输出在整个窗口里持续到达——一次性灌进来的大块字节流属于突发输出场景。

批次是否真的跑完由完成标记计数判定：呈现帧数不为零证明不了字节流完整（终端吞掉输出、
shell 半途死掉时也会出帧）。每批标记的到达时刻同时给出负载窗口与吞吐。

判据取三个互相独立的面：呈现汇总的窗口数与最长停摆（拦「输出还在跑、终端不画了」），
系统侧逐图层的内容层呈现间隔 p95 与刷新槽位占用（拦「画得越来越慢」），以及进程 CPU
时间与 PSS 增长（拦「越跑越费」）。内存峰值只能在负载进行中采：负载结束后内存可能
回落，故等完成标记的轮询按固定间隔顺带采样。显示级帧时间线按属主归属到被测应用，与
逐图层通道的呈现时刻交叉校验，任一条系统侧通道缺失即判红。

shell 就绪与内容层首次出帧属于准备动作，放在测量窗口之外：shell 首输出在模拟器上实测
可达 44 秒，留在窗口里会把场景的 wall-clock 上限吃掉，判红的原因就变成了准备慢而不是
应用慢。

通知权限弹窗会盖住应用：被盖住时输入法不弹出、终端拿不到输入焦点，场景会死在「输入法
未弹出」而不是被测行为上。空载场景更隐蔽——弹窗盖住时界面停画，静置窗口里的零帧是遮挡
造成的，不是应用的空闲表现。故这两个场景测量前都显式授予通知权限，空载场景另核对焦点
在被测应用上。

设备自身不健康时同样会卡在输入法前置条件上：实测模拟器的 SystemUI 无响应后输入法服务
不再出面，场景报的还是「输入法未弹出」。判红的原因落在设备而不是应用，编排须先确认
设备健康。

实测（2026-11-22 本机 AVD，API 35 模拟器，软件渲染）：宿主轻载时负载窗口 7.0 秒、
吞吐 40.9–41.8KB/s、呈现在 12–19 个窗口里、内容层间隔 p95 118–120ms、槽位占用
0.23–0.28、进程 CPU 880–950ms、PSS 增长 16.5–19.7MB；宿主被抢占时（steal 占比 0.41）
窗口拉长到 10.4 秒、吞吐降到 28.2KB/s、p95 涨到 150ms、进程 CPU 涨到 3710ms。门禁阈值
按最差实测定，不能拿空闲宿主的数字当门禁。

复核（2026-11-23 本机 AVD，宿主未抢占，steal 占比 0.03）：负载窗口 7.48 秒、吞吐
39.2KB/s、呈现在 14 个窗口里、最长停摆 620ms、终端内容层 127 帧、呈现间隔 p95 134ms、
刷新槽位占用 0.288、显示级帧时间线归属到 64 帧终端内容、进程 CPU 1380ms、PSS 增长
13.4MB。

### 突发输出的判据面与实测形态

突发输出量的是「整段字节流一次到达」这条路径：负载由设备侧 shell 一次产出（`seq` 逐行
加行尾哨兵），产能不依赖宿主注入速度，中间没有节流；与持续输出的区别在节奏，不在字节
来源。

无损要证明「终端把整段流消费到了末尾」，帧数证明不了：流被截断时终端照样出帧。证据链是
两个哨兵，分工明确：

- 生产者写完全部字节后打的完成标记（`log -t` 写进 logcat）证明字节都进了 PTY；
- 流末尾的会话标题哨兵（OSC 2）出现在会话抽屉里证明终端按序解析到了流的末尾——中途丢
  字节时标题不会出现。

会话标题要在抽屉打开时读：抽屉打开时应用强制刷新会话标题，读到的是当下的标题而不是打开
前的旧值。标题哨兵会被提示符覆盖——mkshrc 的提示符每次都带 OSC 0 上报当前目录名，故测量
前先把提示符换成不含转义的普通提示符，否则哨兵留不住，判据会一直判红。

送达字节数按 PTY 实际吐出的字节算：行规程把每个换行展开成 CRLF，故送达量是行内容字节
加两倍行数（50 万行实测 3888908 字节）。

判据取四面：无损（两个哨兵）、呈现（呈现汇总的窗口数与最长停摆、系统侧逐图层的呈现间隔
p95 与刷新槽位占用、显示级帧时间线按属主归属并与逐图层通道交叉校验）、资源（负载进行中
采的 PSS/RSS 峰值与进程 CPU 时间）、归因（窗口内退出即判红）。内存峰值只能在负载进行中
采：负载结束后内存可能回落。

回滚区按配置封顶（默认 2000 行），故突发不涨内存：实测灌完 50 万行后回滚行数停在
1120–1510 行。

实测（2026-11-22 本机 AVD，API 35 模拟器，软件渲染）三轮：送达字节 3888908、消费时长
1402–1668ms、呈现在 3–4 个窗口里、最长停摆 506–515ms、终端内容层 93–110 帧、呈现间隔
p95 19–20ms（最大 84–102ms）、刷新槽位占用 0.94–0.96、显示级帧时间线归属到 62–63 帧
终端内容、进程 CPU 1220–1500ms、PSS 增长 2.4–7.9MB、RSS 峰值 175–177MB、标题哨兵可见。

突发与持续输出的形状不同：突发时终端基本满帧（槽位占用 0.94–0.96、间隔中位 16ms），持
续输出时槽位占用只有 0.23–0.28、间隔 96–134ms——后者是字节流还没到，不是终端画得慢。故
两场景的呈现窗口数下限分别取 2 与 8，不共用一个阈值。

## 六、环境固定化

- **刷新率**：设备只有一个显示模式时，刷新率没有别的取值，天然固定；有多个模式时用
  `cmd display set-user-preferred-display-mode` 锁定，并记录实际生效值。
- **性能档位**：`cmd power set-fixed-performance-mode-enabled true`；三类动画缩放
  （`window_animation_scale`、`transition_animation_scale`、`animator_duration_scale`）
  显式定成 1.0 并记录实际生效值。
- **温度**：`cmd thermalservice override-status` 注入并记录实际热状态；热节流会把性能
  差异伪装成应用差异。
- **电池与充电**：每轮记 `dumpsys battery`；低电量时宏基准类采集会直接报错（官方口径）。
- **屏幕与网络**：统一亮屏或统一灭屏；统一网络状态，避免后台下载制造噪声。
- **字体与网格**：三应用统一字体族与字号；同一列行数由同一 resize 序列产生。
- **每轮元数据**：设备型号、ROM 构建号、API、刷新率、电池、热状态、构建标识（APK 校验和）。
- **宿主 CPU 占用**：模拟器跑在宿主上，宿主被并发任务抢占时测得值整体变慢——实测冷启动
  十轮中位数从 2.0s 涨到 3.5s、终端内容层刷新槽位比例从 0.91 降到 0.59。故每轮 MUST 记录
  宿主负载，判定 MUST 用负载无关的相对量或覆盖该情形的余量，MUST NOT 把宿主抖动当成应用
  回归，也 MUST NOT 用只能拦「卡死级」的宽阈值掩盖它。负载取设备能读到的宿主抢占时间占比
  （`/proc/stat` 的 steal 列），并同时记被测进程的 CPU 时间（见下节）。

### 环境固定化的实测形态

本仓已把环境固定落成可执行资产（`android/benchmark` 的 `EnvironmentPinning`，由各基准
用例的 `EnvironmentPinningRule` 在用例前固定、用例后复原），实测语义如下（2026-11-21 本机
AVD，API 35 模拟器）：

- 刷新率：活动显示只有一个显示模式（`modeId 1`，320×640，60.000004Hz），刷新率没有别的
  取值，故不下发首选模式命令——实测该命令在本镜像上会抛
  `SecurityException: Package android does not belong to 2000`（写全局设置时调用方归属被拒），
  且命令报错后模式仍被写进全局设置。有多个模式时下发首选模式并核对实际生效值，不符即判红。
- 动画缩放：`settings put global` 三项后读回核对，实测生效值 1.0／1.0／1.0。定成 1.0 而不是 0：
  Compose 的动效时长基准取 `animator_duration_scale`，置 0 会让本仓的交互动画宏基准测不到
  任何动画（帧数为 0 的假通过）。
- 温度：`cmd thermalservice override-status 0` 后读 `dumpsys thermalservice` 核对，实测生效
  热状态为 0（NONE）。
- 固定性能模式：`cmd power set-fixed-performance-mode-enabled true` 在本镜像上被接受，但
  `dumpsys power` 里没有该状态的字段（实测 dump 全文无 fixed／performance 相关行），故只记
  「不可核对」，不谎称已固定——设备提供了核对通道时读出核对，读到「未开」即判红。
- 复原：用例结束后解除热状态覆盖、关掉固定性能模式、还原用户首选显示模式与三项动画缩放的
  原值（原值为「未设置」即删掉该项），不把设备留在被改过的状态里。
- 固定后的实际生效值随每次测量的环境快照落进测量行（`animation_scales` 与
  `fixed_performance_mode` 字段）。

### 宿主负载与进程 CPU 时间的实测形态

宿主负载与进程 CPU 时间已落成可执行资产（`android/benchmark` 的 `HostLoad` 与
`ProcessCpuTime`，由 `measureScenario` 在场景窗口前后各取一次快照并随测量行落盘），实测
语义如下（2026-11-22 本机 AVD，API 35 模拟器，4 核宿主）：

- 负载口径用宿主抢占时间（`/proc/stat` 的 steal 计数）占 CPU 总时间的比例表达：设备自身
  读不到宿主负载，但读得到「自己没拿到多少 CPU」。取 cpu 汇总行而非逐核行（逐核行是各 vCPU
  的账，汇总行才是全体合计）；汇总行末尾的 guest 与 guest_nice 两列已计入 user，只取前八列。
- 抢占判定阈值定 10%：实测四档——模拟器空闲且宿主安静时 steal 约 0.1%；宿主安静但模拟器
  活跃（负载 0.3–0.9）时 1.9%–5.2%；宿主被 3 个忙循环占用（负载 1.7–4.6）时 44%–46%；被
  8 个忙循环占用（负载 0.9–5.3）时 38%–43%。10% 是安静上限的两倍，又远低于任何抢占态的
  下限；先前取的 1% 会把全部正常轮次误判为抢占。
- **只记录，不折算**：测试进程内的定长 CPU 循环作速度参照已被实测否掉（见第十一节）；按
  steal 占比折算同样不成立——steal 记的是全部 vCPU 的被抢时间，空闲的 vCPU 被抢走并不拖慢
  被测进程，按占比缩放会把被测进程算得比实际更快。故判定只用负载无关的相对量（进程 CPU
  时间、槽位比例），负载只作事后归因的证据。
- 进程 CPU 时间取 `/proc/<pid>/stat` 的 utime/stime（进程自创建起的累计 CPU 时间）与
  starttime（进程创建时刻），配合 `/proc/uptime` 得进程年龄；滴答频率取 100 Hz（Android 上
  USER_HZ 恒为 100）。字段序号从最后一个右括号之后起算（utime 11、stime 12、starttime 19），
  因为第二列的进程名可能含空格与括号。
- 实测对照（同一构建、同一场景、跨应用冷启动五轮）：宿主安静时 steal 占比 0.019–0.052、
  墙钟中位数 856ms、进程 CPU 时间中位数 730ms；宿主被 3 个忙循环占用时 steal 0.436–0.458、
  墙钟中位数 2275ms、进程 CPU 时间中位数 1020ms；被 8 个忙循环占用时 steal 0.382–0.431、
  墙钟中位数 2191ms、进程 CPU 时间中位数 970ms。墙钟涨 150%–165% 而进程 CPU 时间只涨
  30%–40%——这正是「双记」的根据：墙钟是用户感受，CPU 时间是应用自身的工作量。
- 负载采集失败不覆盖原始失败：读不出 `/proc/stat` 时把读失败挂成 suppressed，判定仍记失败。

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
| `metrics` | 启动时长、首帧时长、帧数与呈现间隔分位数、超出量分位数、内存 PSS／RSS、能耗、进程 CPU 时间与进程年龄 |
| `host_load` | 宿主抢占占比、设备忙碌占比与窗口时长（抢占占比高即该轮被宿主抢占） |
| `exit` | exit-info reason 与归因类别 |
| `artifacts` | 制品名、路径、字节数、校验和 |
| `verdict` | 判定、归因、备注 |

日志纪律：采集侧全量落盘、不筛选、不限级；按 tag 或关键字的过滤只发生在分析侧。

### 落地格式的实测形态

本仓已把上表的落盘格式做成可执行资产（`android/benchmark` 的 `MeasurementArtifacts` 与
`MeasurementStore`），实测形态如下：

- 每轮一个目录：`<run_id>/rows.jsonl` 与 `<run_id>/artifacts/`；`run_id` 是「场景名 + 轮次
  起始时刻」，目录落在测试输出目录里，宿主侧随测试结果一并回收（实测落在
  `android/benchmark/build/outputs/connected_android_test_additional_output/` 下）。
- 行的字段与上表一致：`run_id`、`device`（型号、ROM 构建号、API、刷新率、电池、热状态）、
  `app`（包名与版本标识）、`scenario`（场景名、迭代序号、起止时刻、耗时）、`exit`（退出归因）、
  `artifacts`（制品名、路径、字节数、SHA-256）与 `verdict`（判定与备注）。
- 行数记的是实际发生的测量，**不必等于框架的迭代数**：宏基准框架在温／热启动前会多跑一轮
  准备轮次（该轮不计入框架的测量迭代，实测温启动 11 行对应 10 个 trace），故行里另记框架
  自己的迭代序号并用 `measured` 标出准备轮次。
- 制品配额按一轮 64 MB：腾不出空间时轮转清理最旧的制品；单件就超配额时丢弃该件（返回空），
  不让设备存储被制品占满。
- 宿主负载与进程 CPU 时间随行落盘：`host_load` 对象记 steal 占比、忙碌占比与窗口时长，
  `metrics` 里记 `process_cpu_millis` 与 `process_age_millis`；负载采集失败时不覆盖原始失败
  ——读失败挂成 suppressed，判定仍记失败。

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
| 按宿主抢占占比（steal）折算启动耗时或帧率 | steal 记的是全部 vCPU 的被抢时间，空闲的 vCPU 被抢走并不拖慢被测进程，按占比缩放会把被测进程算得比实际更快；负载只作事后归因的证据，判定用进程 CPU 时间、槽位比例这类负载无关的相对量 |

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
- 本轮落地：测量环境也做成了可执行资产——`DeviceEnvironment` 在每次测量前采集设备型号、ROM
  构建号、API、活动显示的刷新率、电池电量与状态、电池温度、热状态与被测应用的版本标识，随实测
  行一起落进构建产物（`[bench-env]` 行）；任何一项读不到即判红，不记一份缺项的环境。环境目前
  只被记录、未被固定：刷新率锁定、性能档位与温度注入仍缺（见第九节的设备准备清单）。
- 本轮落地：测量结果也落成了可执行资产——`MeasurementArtifacts` 与 `MeasurementStore` 按本报告
  的「数据落地格式」实现：每轮一份 JSONL 行文件与一个制品目录，制品附字节数与 SHA-256；行里带
  环境快照、退出归因、判定与备注，目录落在测试输出目录里，宿主侧随测试结果一并回收（实测落在
  `android/benchmark/build/outputs/connected_android_test_additional_output/`）。制品配额一轮 64 MB，
  腾不出空间时轮转清理最旧的制品，单件超配额即丢弃。
- 本轮落地：跨应用启动对比面——`AppLaunch` 解析系统侧 `am start -W` 与
  `cmd package resolve-activity --brief` 的输出，`CrossAppLaunchBenchmark` 以仪器参数
  `targetPackages` 驱动逐包测冷启动：解析启动入口、预授权限、强停后拉起、校验拉起的是
  被测应用且启动态为 `COLD`、逐轮落盘系统侧 TotalTime 与 WaitTime 并给 p50/p95 汇总；
  被测包的标识与指标随行落盘，横向比较按 `app.package` 分组。实测两个包各五轮全部通过
  （见第四节「跨应用启动对比面」）。
- 本轮落地：环境固定化——`EnvironmentPinning` 把刷新率、三类动画缩放、热状态与固定性能模式
  钉住并核对实际生效值，各基准用例的 `EnvironmentPinningRule` 在用例前固定、用例后复原；
  固定后的实际生效值随环境快照落进测量行。设备只有一个显示模式时不下发首选模式命令，
  固定性能模式在无核对通道的设备上只记「不可核对」（见第六节）。
- 本轮落地：宿主负载与进程 CPU 时间——`HostLoad` 解析 `/proc/stat` 的 cpu 汇总行给出 steal
  占比并按四档实测标定 10% 的抢占阈值，`ProcessCpuTime` 解析 `/proc/<pid>/stat` 的
  utime/stime/starttime 配合 `/proc/uptime` 得进程 CPU 时间与年龄；`measureScenario` 在场景
  窗口前后各取一次 CPU 时间快照，随测量行落盘 `host_load` 对象与 `process_cpu_millis`／
  `process_age_millis` 指标。负载只记录不折算：判定用负载无关的相对量，负载只作事后归因的
  证据（见第六节与第十一节）。
- 本轮落地：场景矩阵与资源上限——`ScenarioMatrix` 把本节的 12 类场景落成可执行数据（统一
  负载、指标、采集面、判据、wall-clock 上限、输出字节上限与采样间隔下限），`ScenarioBudget`
  执行两道上限：超限即强停被测应用并记「超限终止」，不继续等待。现有冷启动／温启动／滚动与
  跨应用启动基准按矩阵编号（S1／S2／S7）接入，落盘行据此带 `scenario.matrix_id`（实测冷启动
  行带 `matrix_id: S1`、判定 PASS）。
- 本轮落地：启动口径的健壮性——`am start -W` 的 `TotalTime` 行只在 AOSP `result.totalTime >= 0`
  时打印，启动即崩溃时该行缺失且启动态为 `UNKNOWN`，故总耗时按可空取、缺失显式判红；
  退出归因按「应用故障 → 环境 → 其它」排序，准备动作的强停记录不再盖掉真正的崩溃原因。
- 本轮落地：热启动场景 S3——热启动不新建 Activity，框架的启动等待会退化成固定两秒，故该场景
  改走系统侧 `am start -W` 并校验启动态为 `HOT`；终端内容层的 Surface 随界面回到前台重建，
  内容层帧取启动之后的新图层（按 home 后旧图层已从图层清单消失），实测每轮内容层 3 帧、
  `gfxinfo` 3–5 帧，总耗时中位 971ms（见第五节「热启动的取数口径与实测形态」）。
- 本轮落地：稳态空载场景 S4——`SteadyStateBenchmark` 以「终端内容层画出第一帧」为就绪信号，
  静置十秒后取系统侧逐图层的内容帧、应用进程的 `gfxinfo` 帧数、`/proc/<pid>/stat` 的进程
  CPU 时间与 `dumpsys meminfo` 的 PSS／RSS；三个面任一出现持续绘制或持续占用即判红。实测
  三轮：界面帧 0 帧、内容层 0 帧、进程 CPU 时间 150ms、PSS 66–71 MB（见第五节「稳态空载的
  判据面与实测形态」）。
- 本轮落地：S5／S6 与 S8–S12 的场景执行器——`SustainedOutputLoad`／`BurstOutputLoad`／
  `WideCharacterLoad` 生产设备侧负载并按 PTY 的 CRLF 展开记账字节，`OutputScenarioFrames`
  收拢输出类场景共用的三通道取数与六项判据，`OutputLoadCompletion` 统一负载完成判定，
  `SessionTitleProbe` 负责标题哨兵（测量前把提示符换成不含转义的普通提示符，否则哨兵会被
  每次改写标题覆盖）。S8–S12 的执行器分别是 `WideCharacterBenchmark`、`InteractionLatencyBenchmark`、
  `InputFloodBenchmark`、`BackgroundForegroundBenchmark`、`ResourceStressBenchmark`。
- 本轮落地：S8–S12 的阈值按实测校准（本机 AVD，API 35 模拟器，软件渲染）——S8 负载实测
  10.8–11.5 秒、呈现窗口 22 个、PSS 增长 6922–14585KB、进程 CPU 6310–7170ms，故负载上限
  取 24 秒、窗口数下限取 8、PSS 上限取 23MB、CPU 上限取 11.5 秒。
- 本轮落地：恢复与旋转的首帧取数口径——`SurfaceLayerFrames` 的 `firstPresentMillisAfter` 与
  `awaitContentLayerPresentAfter` 取「重建确认之后」的首个呈现（旧图层的帧历史里留着重建
  之前的帧，算进去会让恢复看起来是零延迟）；旋转的计时起点取「系统确认转到横屏」的时刻
  （`dumpsys window` 的 `mCurrentRotation=ROTATION_90`），不是发起旋转的时刻——本应用在
  清单里声明了 `configChanges`，旋转不重建 Activity，SurfaceView 的 Surface 也不重建，
  变的是表面尺寸。等待出帧的上限（8 秒）与首帧判据的上限（5 秒）分开：前者判「没出帧」，
  后者判「慢」。
- 本轮落地：S12 的屏幕状态复原按当前状态判断——电源键是切换语义，施加压力时按过一次后屏幕
  状态与原值相反，当前状态与原值不同才再按一次，按原值盲按会把设备留在被改过的状态里。
- 本轮落地：共用辅助逻辑的单测——`ScenarioTargetTest` 覆盖被测包参数解析（空值、空串与只有
  分隔符都退回默认包），`OutputScenarioFramesTest` 逐项覆盖输出类场景的六项帧面判据与两通道
  一致性判据。
- 本轮落地：模拟器实测形态——S7–S12 六个场景在本机 AVD 上逐个跑通（S7 需带
  `androidx.benchmark.suppressErrors=EMULATOR`，与 Gradle 的连接测试一致），S8 的三轮实测
  负载耗时 10.8–11.5 秒、呈现窗口 22 个、进程 CPU 6310–7170ms，判定均在预算内。缺 native
  库的 APK 启动即崩（`native library not loaded: theme catalog unavailable`），退出归因给
  `APP CRASH(EXCEPTION)`——构建顺序仍是先出 `.so` 再出 APK。
- 缺：跨应用帧面端到端跑通（对照应用尚未装到设备上按本仓口径取帧）；跨设备多应用编排
  （落点在受保护的 `scripts/`）。
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
测量环境也已落成可执行采集（`android/benchmark` 的 `DeviceEnvironment`）：每次测量前采集设备型号、
ROM 构建号、API、刷新率、电池状态与温度、热状态与被测应用版本标识，随实测行落盘，读不到任何
一项即判红——环境漂移因此可被事后核对。但环境目前只被记录、未被固定，跨应用对比前仍须按第九节
的设备准备清单把刷新率、性能档位与温度固定下来。
测量结果的落盘也已落成可执行资产（`android/benchmark` 的 `MeasurementArtifacts`）：每轮一份
JSONL 行文件与一个制品目录，行里带环境、退出归因与判定，制品附字节数与校验和，目录随测试结果
回到宿主；实测宏基准跑完一轮即产出 33 行（冷启动 10、温启动 11、滚动 3、三组交互动画各 3）。
行数记实际发生的测量，不等于框架的迭代数——温／热启动前框架多跑的那一轮准备轮次在行里被标为
`measured=false`。
跨应用启动对比面也已落成可执行资产（`android/benchmark` 的 `AppLaunch` 与
`CrossAppLaunchBenchmark`）：启动耗时取系统侧 `am start -W` 的 TotalTime——各应用自报的启动
指标跨应用不可比，只有系统口径同源可比。实测暴露并已修掉两处会污染数据的坑：冷启动的准备
动作（强停）自身留下的 exit-info 记录落在测量窗口内会让该轮判红；权限弹窗会抢走启动，
`am start -W` 报出的是弹窗自己的 Activity。两者分别以「准备动作移出窗口」与「测量前预授权限
并校验拉起的是被测应用且启动态为 COLD」处置。实测同轮内两个包各五轮：本项目 p50 873ms、
系统应用 p50 689ms（模拟器上只作数量级参考），十轮全部无退出记录。
环境固定化也已落成可执行资产（`android/benchmark` 的 `EnvironmentPinning`）：刷新率、三类
动画缩放、热状态与固定性能模式在测量前钉住并核对实际生效值，钉不住即判红；固定后的实际值
随环境快照落进测量行，用例结束后复原设备原值。实测确认两处平台事实：模拟器只有一个显示
模式（刷新率天然固定，不必下发首选模式命令），固定性能模式在本镜像上没有核对通道（只记
「不可核对」，不谎称已固定）。动画缩放定成 1.0 而非 0：置 0 会让交互动画宏基准测不到动画，
变成帧数为 0 的假通过。
宿主负载与进程 CPU 时间也已落成可执行采集（`android/benchmark` 的 `HostLoad` 与
`ProcessCpuTime`）：每轮在场景窗口前后各取一次 `/proc/stat` 快照，steal 占比按四档实测标定
10% 阈值（安静上限 5.2% 的两倍，抢占态下限 38% 的数分之一），并记 `/proc/<pid>/stat` 的
utime/stime 得进程 CPU 时间。实测抢占时墙钟中位数从 856ms 涨到 2191–2275ms，而进程 CPU
时间只从 730ms 涨到 970–1020ms——墙钟是用户感受，CPU 时间是应用自身的工作量，两者必须
同记；负载只记录不折算（steal 记的是全部 vCPU 的被抢时间，按占比缩放会把被测进程算得比
实际更快）。
启动口径也补了一处硬约束：`am start -W` 的 `TotalTime` 行只在系统测到启动时才打印，启动
即崩溃时输出的是 `LaunchState: UNKNOWN (0)` 且没有总耗时行，故总耗时按可空取、缺失显式判红
——读成 0 会把「系统没测到」记成「启动很快」。退出归因同时按「应用故障 → 环境 → 其它」
排序，准备动作的强停记录不再盖掉真正的崩溃原因。
