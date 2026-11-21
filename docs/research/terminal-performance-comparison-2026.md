# 跨终端性能对比与全场景数据采集：无人工干预的方法学

## 摘要

结论先行：

1. **端到端帧率不可跨应用直接比。** 两个现实对照对象（`termux/termux-app`、
   `sylirre/ghostty-android-terminal`）都用 HWUI `Canvas` 绘制终端内容，本项目用 wgpu 直绘
   SurfaceView；同一句「60 fps」在两侧量的不是同一件事。
2. **唯一可跨应用统一比较的帧面是 SurfaceFlinger 的逐帧呈现时序**：
   `dumpsys SurfaceFlinger --latency <layer>`。AOSP 源码给出确切语义——首行是 vsync 周期
   （纳秒），其后每行一帧三列 `desired / actual / ready`（纳秒）。它对任何应用的图层都成立，
   不区分帧由谁绘制。
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
| AOSP 源码 | `frameworks/native/services/surfaceflinger/SurfaceFlinger.cpp`、`services/surfaceflinger/Layer.cpp`、`frameworks/base/core/java/android/view/ThreadedRenderer.java` | 逐符号读实现 |
| 对照对象源码 | `termux/termux-app` v0.119.0-beta.3；`sylirre/ghostty-android-terminal` main | 仓库树枚举 + 关键文件逐行读 |
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
| 稳态帧时序 | 可比 | 同一图层时序口径（呈现间隔、距 deadline 超出量） | `--latency` | `--latency` |
| 内存占用 | 可比 | `dumpsys meminfo` 的 PSS / RSS | `dumpsys meminfo` | 同左 |
| 能耗 | 仅同设备内可比 | 硬件相关，跨设备无意义 | `batterystats` 按 UID | 同左 |
| 崩溃 / ANR / 异常退出 | 可比 | `ApplicationExitInfo` reason + logcat + dropbox | 同左 | 同左 |
| HWUI 帧统计（`gfxinfo`） | **仅同应用内可比** | 只统计经 ViewRootImpl/RenderThread 绘制的帧；SurfaceView 直绘不经该路径 | 只用于 Compose 界面自检 | 终端内容帧可测，但语义与本项目不同 |
| GPU 计数器 | 仅同设备内可比 | counter ID 由设备描述符给定 | `gpu.counters` | 同左 |
| 宏基准指标 | 不作跨应用主判据 | 官方明确模拟器不代表真实体验；本仓 release 变体的源码清单未声明 `android:profileable` | 仅数量级回归 | 不适用 |

`gfxinfo` 的入口在 `ThreadedRenderer.handleDumpGfxInfo`（`dumpGlobalProfileInfo` 加
`WindowManagerGlobal.dumpGfxInfo`），参数 `framestats` / `reset` / `-a` 由 `dumpArgsToFlags`
解析——这条链只覆盖 HWUI 管线。本仓既有规约「宏基准的帧级数据面按绘制主体选源」与之一致。

## 三、跨应用统一的帧面：SurfaceFlinger 逐帧时序

AOSP 实现给出确切语义，可据此写采集协议：

- `SurfaceFlinger` 的 dump 参数表把 `--latency` 映射到 `SurfaceFlinger::dumpStats`；`dumpStats`
  先写出 `mScheduler->getPacesetterVsyncPeriod().ns()`（刷新周期，纳秒），再对**同名图层**调用
  `layer->dumpFrameStats(result)`。
- `Layer::dumpFrameStats` 在新帧统计开关下逐帧写 `desired<TAB>actual<TAB>ready`（纳秒）；
  旧开关下走旧帧统计器输出同义数据。
- `--latency-clear` 映射到 `SurfaceFlinger::clearStats`，清空该图层（或全部图层）的帧历史。

采集协议：

1. 场景开始前 `dumpsys SurfaceFlinger --latency-clear <layer>` 清空环形历史——不清空则读到的是
   上一个场景的残留帧。
2. 场景结束后 `dumpsys SurfaceFlinger --latency <layer>` 取回逐帧数据。
3. 图层名候选来自 `dumpsys SurfaceFlinger --list`（实测输出为逐层的协议调试串，层名需从中解析）；
   最终以 `--latency` 是否返回逐帧数据为准。取不到精确图层名时判**采集失败**并中止该场景，
   不得静默跳过、也不得以「没采到」当作「没出帧」。
4. 由此可算：呈现间隔（相邻 `actual` 之差）、距 deadline 的超出量（`actual - desired`）、
   丢帧（呈现间隔超过刷新周期）与帧数。

为什么它是跨应用的统一面：它量的是「合成器把帧呈现出去的时刻」，与应用用什么管线绘制无关。
两个对照对象的 HWUI 帧与本项目的 wgpu 直绘帧都落在同一条时间轴上。

## 四、采集面清单

| 面 | 采集命令 | 输出 | 判据用法 |
| --- | --- | --- | --- |
| 跨应用帧时序 | `dumpsys SurfaceFlinger --latency <layer>` / `--latency-clear <layer>` | 首行 vsync 周期 + 逐帧三列纳秒 | 帧数、呈现间隔分位数、超出量分位数 |
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
  `DisplayHAL`、`PredictionError`、`Unknown`。
- `android.log`：只在 userdebug 构建可用，正式版真机不可用；改用 logcat 全量落盘。
- ATrace 每事件开销 1–10 微秒（字符串化、JNI、用户态到内核态写标记），因此禁止在渲染热路径
  铺开埋点——既有规约「日志不得在热路径产生开销」同向。
- `gpu.counters` 的 counter ID 由设备描述符 `GpuCounterSpec` 给定，跨设备不可比。

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
| S1 | 冷启动 | 杀进程后 `am start -W` | 启动到首帧、退出归因 | `--latency`、exit-info | 首帧在预算内、无异常退出 | 60 秒 |
| S2 | 温启动 | Activity 重启不杀进程 | 首帧、帧数 | `--latency` | 首帧在预算内 | 30 秒 |
| S3 | 热启动 | 进程存活切前台 | 首帧、丢帧数 | `--latency` | 无丢帧、无 Surface 丢失 | 15 秒 |
| S4 | 稳态空载 | 静置 10 秒 | 帧数、CPU 占用、内存 | `--latency`、meminfo | 空载不产生持续绘制 | 15 秒 |
| S5 | 持续输出 | 连续字节流（逐行） | 吞吐、呈现间隔 p95、内存峰值 | `--latency`、meminfo | 无丢帧、内存有界 | 60 秒 / 8 MB |
| S6 | 突发输出 | 单次大块字节流 | 无损、内存峰值、退出归因 | meminfo、exit-info、logcat | 输出无损、无异常退出 | 30 秒 / 8 MB |
| S7 | 长滚动 | 填满回滚区后拖动 | 呈现间隔 p95、丢帧 | `--latency` | p95 在预算内、无停摆 | 30 秒 |
| S8 | 宽字符行 | CJK、组合字符、emoji 行 | 排版正确、呈现间隔 p95 | `--latency`、截图复核 | 无错位、无丢帧 | 30 秒 |
| S9 | 交互延迟 | 探针命令 + 计时回显 | 按键到回显间隔 | logcat 时间戳 | 间隔在预算内 | 30 秒 |
| S10 | 输入洪水 | 连续按键事件 | 丢键数、ANR | exit-info、logcat | 无丢键、无 ANR | 30 秒 |
| S11 | 后台前台切换 | 切后台再切回 + 旋转 | 恢复后首帧、丢帧 | `--latency` | 恢复即出帧、无 Surface 丢失 | 30 秒 |
| S12 | 资源极端 | 低内存、强压、灭屏、Doze | 存活、快速死亡、归因 | exit-info、batterystats、thermalservice | 快速死亡且归因明确 | 60 秒 |

S9 的探针口径沿用既有规约：终端内的 shell 没有提示符可等，必须用幂等的探针命令诱发一次输出作
就绪信号，不得用固定等待时长替代。

## 六、环境固定化

- **刷新率**：`cmd display set-user-preferred-display-mode` 锁定，并记录实际生效值。
- **性能档位**：`cmd power set-fixed-performance-mode-enabled true`；关闭三类动画缩放。
- **温度**：`cmd thermalservice inject-temperature` / `override-status` 注入并记录实际热状态；
  热节流会把性能差异伪装成应用差异。
- **电池与充电**：每轮记 `dumpsys battery`；低电量时宏基准类采集会直接报错（官方口径）。
- **屏幕与网络**：统一亮屏或统一灭屏；统一网络状态，避免后台下载制造噪声。
- **字体与网格**：三应用统一字体族与字号；同一列行数由同一 resize 序列产生。
- **每轮元数据**：设备型号、ROM 构建号、API、刷新率、电池、热状态、构建标识（APK 校验和）。

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
        install(app)           # 外部下载的 APK，不入库
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
| 宏基准的 `FrameTimingMetric` / `StartupTimingMetric` 作跨应用主判据 | 官方明确不推荐在模拟器上跑；本仓 release 变体的源码清单未声明 `android:profileable`，指标无采集前提；既有规约已把它定位为「模拟器数量级回归」用途 |
| `dumpsys gfxinfo` 的帧数作终端内容帧判据 | 只统计 HWUI 管线帧，SurfaceView 直绘不在其中（既有规约同口径） |
| Perfetto `android.log` 数据源 | 仅 userdebug 构建可用，正式版真机不可用；改用 logcat 全量落盘 |
| `gpu.counters` 跨设备对比 | counter ID 由设备描述符给定，跨设备不可比 |
| `PowerMetric` 跨设备对比 | 依赖具体硬件与充电状态 |
| atrace counter 与 FrameTimeline 关联 | 既有规约已登记为不采纳（真机排障可选项） |
| `screenrecord` 作判据 | 编码与分辨率引入偏差，只作人工复核制品 |
| 引用任一对照对象的自述性能数字 | 两者均未发布可核实的性能数字 |

## 十二、与本仓既有资产的关系

- 已有：`android/benchmark`（`AppStartupBenchmark`、`ScrollPerformanceBenchmark`、
  `InteractionAnimationBenchmark`、`BenchmarkSupport`、`FrameStats`、`PresentedFrames`）、
  `native/benches`（`cell_builder`、`vt_typing`、`cjk_resolve`、`support`）、
  `scripts/test-emulator.nu`。
- 缺：跨应用对比面；SurfaceFlinger 逐帧时序采集；exit-info 归因采集；场景矩阵与制品落盘规范。
- 与既有规约一致性：本报告不改「模拟器只作数量级」「帧级数据面按绘制主体选源」「采集为空即
  判红」「设备侧命令不含 shell 语法」「不得用固定等待替代就绪信号」等既有口径，只在其上补
  跨应用一层。

## 十三、结论

跨终端性能对比在本项目是**可做**的，但前提是选对观测面：帧时序走 SurfaceFlinger 的图层时序，
而不是各家自报的帧率；进程状态、能耗、退出归因走系统 dump，而不是各家自报的内存与耗电。
对照对象没有任何可复用的基准资产，对比必须自建 harness；本仓已有的宏基准模块与设备侧编排是
落点，跨应用扩展受 `scripts/` 受保护约束，须维护者明确同意后实施。失败归因必须落到
exit-info reason、FrameTimeline jank 类型、logcat 与 dropbox 四源交集，证据不足记「未定」而非
猜测；极端场景要有硬超时与输出上限，宁可快速死亡并留下证据，也不在失败场景上继续消耗设备。
