# termlib 深度研究报告

对 [connectbot/termlib](https://github.com/connectbot/termlib) 的深度调研结论，目的是回答两个问题：本项目有什么可参考之处；本项目的 macrobenchmark 基准是否有意义。调研方式为克隆仓库并通读源码、文档与测试体系。

## 概况

ConnectBot 出品的 Jetpack Compose 终端组件库，供宿主 App 嵌入终端模拟功能。版本 0.3.x，活跃开发中。

- VT 解析：vendored `libvterm`（C，MIT），经 JNI 调用；JNI 边界为 `Terminal.cpp`（约 1200 行），以 `CallbackCache` 全局引用管理回调，用 64 KiB cell 缓冲加 4 KiB header 的批量传输协议跨越边界
- 渲染：Android Canvas 两阶段绘制（先全部背景再全部前景），复杂文种走平台 ICU shaping，双向文本为 terminal-wg level-1 隐式 LTR 模型
- 数据归属：网格快照归属 Kotlin 侧，经 `StateFlow` 发布给 Compose，draw phase 读取——终端渲染帧因此跑在 Compose/Choreographer/RenderThread 管线内
- 配套能力：OSC 133 shell integration 语义导航与无障碍 Review Mode、Kitty/iTerm2 内联图像、Unicode 17 宽度表以 pin 数据加生成脚本加 `--check` 校验保证可复现

## 性能测量体系

termlib 最值得研究的是其四层性能测量体系，层层分离关注点：

### 第一层：macrobenchmark 宏基准

`macrobenchmark` 模块基于 `MacrobenchmarkRule` + `FrameTimingMetric` + 自定义 bounded `PerfettoConfig`（只含 sched/gfx/view/input/dalvik 与 FrameTimeline 数据源；默认宽配置在部分 Android 构建上启动卡 30 秒）。`CompilationMode.Full()` 全量 AOT，`killProcess()` 每轮冷启，20 秒测量窗口加 3 秒预热。

### 第二层：真实工作负载

`test-app` 的 `FrameBenchmarkActivity` 以 60 Hz 定时喂入真实终端负载：`idle`、彩色 Unicode `text`、真 PTY 捕获的 `cacafire`、512×512 `png`/`rgba` 图像替换、Kitty `animation`、`input`（真实按键与 12000 字符 IME 提交并伴随尺寸交替）、`shaping`/`shaping-static`（阿拉伯/印度系文字）。同一活动保持一个微小独立心跳绘制，用于在终端内容停滞时暴露响应性。

### 第三层：host JVM 微基准

`BoundaryBenchmarkTest` 以 `TERMLIB_BENCHMARK=1` 环境变量显式开启（`assumeTrue` 门控，常规 CI 直接跳过），测小载荷 JNI 调用、渲染滚动与 OSC 序列的 ns/op 与分配 bytes/op，输出中位数。真 PTY 捕获（`capture-cacafire.py` 记录 read 时间戳与字节数）按 60 Hz 重组帧边界重放，保真突发输出的帧级压力。

### 第四层：设备端基准

`AndroidRenderBenchmarkTest`（opt-in）测软件 Canvas 渲染 ns/frame 与分配字节；`AndroidMemoryTest` 在旋转 resize 循环中断言 scratch 缓冲复用并采样 Java/native 双堆，检测内存随 Activity 重建泄漏。

### 方法论原则（benchmark/README.md 明文）

- 模拟器数据不能建立物理设备性能结论，须显式 `--emulator` 覆盖并在验收中排除
- 判据用 p95、p99、missed-deadline 百分比而非中位数；无效捕获（Perfetto 丢数、时长偏差超 10%）以非零退出码拒绝并剔除
- 修订对比规程：同一 harness 对两个 revision 构建、同设备同刷新率同字体同尺寸、交替顺序确认；建议真机验收线为 99% 帧达标、text 输入到呈现 p95 在两个刷新间隔内
- `conditions.json` 记录设备/构建/热状态，保证可比性可追溯

### trace 关联诊断

输入线程 `Trace.setCounter("terminal.benchmark.tick")`，绘制钩子回读快照首 8 个 cell 中的数字打 `terminal.draw.tick`，分析脚本将两者与 Perfetto FrameTimeline surface token 关联，得出 input-to-record 与 input-to-present 延迟及内容滞后（以 reader tick 计）；缺失或合并的更新不得计为零延迟。这套诊断能把"输出何时到达屏幕"从帧时长中独立出来。

## 与本项目的架构对比

| 维度 | termlib | 本项目 |
| --- | --- | --- |
| VT 解析 | libvterm（C，JNI 回调） | libghostty-vt（Rust 绑定，跟踪 master） |
| 终端渲染 | Canvas，Compose 帧管线内 | wgpu/Vulkan，Rust 自有线程直绘 ANativeWindow（SurfaceView） |
| 帧节拍 | Choreographer vsync | 17 ms 活跃 latch / 500 ms idle latch |
| 网格归属 | Kotlin 快照 | Rust 侧，Kotlin 无网格 |

由渲染架构差异得出两条推论：

1. `FrameTimingMetric` 基于主线程 Choreographer doFrame 统计。termlib 的终端渲染在其覆盖范围内，是全覆盖核心工具；本项目的终端内容由 Rust 线程直接向 ANativeWindow 呈现，不产生主线程 doFrame，该指标只覆盖 Compose UI 层（修饰键栏动画、IME insets 动画、会话抽屉），测不到终端内容渲染帧。
2. termlib 一切依赖"渲染在 Compose 管线内"的手法（display list 视口裁剪、draw phase 读取、帧时钟合帧），对本项目终端内容渲染不适用；对 Compose UI 层适用。

## 可参考点

### 直接可参考（方法论，与渲染架构无关）

1. 真 PTY 捕获重放：记录 read 时间戳与字节数、按 60 Hz 重组帧边界重放。可用于将 `vt_typing` 基准从合成负载升级为真实负载，直接提升回归检出能力。
2. opt-in 微基准门控：环境变量加 `assumeTrue` 显式开启，输出中位数与分配字节，不进常规 CI。
3. 测量规程明文化：p95/p99/missed-deadline 判据、无效捕获非零退出、同 harness 同设备交替顺序、`conditions.json` 记录环境。真机基准操作规程应照此成文。
4. bounded PerfettoConfig：trace 采集直接采用窄配置写法，规避宽配置在部分系统构建上的启动停滞。
5. 增量与全新 Surface 逐像素一致性断言（`TerminalRedrawTest`：`changed.sameAs(fresh)`）：现成的测试范式，可补强 render-stability 规格中"幂等渲染"的验收。
6. 旋转 resize 循环加双堆采样断言 scratch 复用（`AndroidMemoryTest`）：对应 Activity 重建场景，可作 Rust 与 JNI 侧内存不增长断言的样板。
7. JNI 回调所有权修复史（break JNI callback ownership cycles）：本项目 `ffi.rs` 事件通道可对照自查同类全局引用泄漏。
8. benchmark/README.md 明文写清每层测量能测什么、不能测什么、模拟器结论的边界——测量能力边界文档化本身值得学。

### 需改造后适用（映射到 Rust 侧）

- trace 关联诊断移植：输入侧在喂 PTY/VT 时打序号 counter，Rust 渲染循环每帧把已消费序号写入 atrace counter（或等价探针），真机 Perfetto 与 FrameTimeline 关联即可得出 input-to-present 延迟。这是 termlib 体系中价值最高但需要移植的一条。
- 终端输出负载基准：termlib 用 `FrameTimingMetric` 测；本项目的等效物是渲染循环既有 logcat 时序窗口统计加真机 Perfetto，而非 `FrameTimingMetric`。

### 不适用（明确排除，防误参考）

- `PackedCells`/`TerminalSnapshot` 等 Kotlin 网格结构：本项目设计上无 Kotlin 网格。
- OSC 133 无障碍导航与 Review Mode：规格未要求，不做不必要功能（且无障碍朗读在禁止清单中）。
- Unicode 宽度表生成脚本：由 libghostty-vt 承担。
- Canvas box-drawing 几何绘制、bidi level-1 渲染细节：wgpu 侧与 libghostty-vt 的职责。
- Magnifier 放大镜选区：文本选择须遵循 Termux 设计。

### 本项目已有的对应能力

参考是补强而非补课。以下能力 termlib 反而没有：

- baselineprofile 闭环：生成器（真机 UiAutomator 走真实路径）→ `saveInSrc` 合并 → `CompilationMode.Partial(baselineProfileMode = Require)` 验证 profile 真实生效，缺失即报错
- criterion Rust 微基准（`cell_builder`、`vt_typing`、`cjk_resolve`），CI 以 `--quick` 跑通
- Cucumber instrumentation UI 测试与 OCR 像素验收
- 渲染循环自建时序探针（活跃/idle latch 窗口统计、`SLOW_FRAME`）

## macrobenchmark 评估

结论：有意义，但边界明确——启动时序、Compose UI 层动画、baseline profile 验证三者有效；对终端内容渲染性能本身意义有限。现有基准选型已踩在正确范围内，不建议照搬 termlib 式帧负载基准，维持 CI 只编译不运行的现状。

1. AppStartupBenchmark 是刚需。DESIGN 明确要求启动速度（启动动画结束后直接显示 Shell 与主题背景）。`StartupTimingMetric` 是宏基准中最稳的指标；COLD 加 `Partial(Require)` 与 baselineprofile 模块构成闭环——profile 缺失直接报错，验证其真实生效，termlib 恰恰没有这一层。模拟器上绝对值无意义，但冷启动的数量级回归（native 库膨胀、启动路径新增检查）仍可暴露。
2. InteractionAnimationBenchmark 有效。修饰键弹簧动画与 IME insets 逐帧跟随确实经主线程 doFrame，`FrameTimingMetric` 采得到，对应 ime-animation-smoothness 规格的"不卡顿"要求。选型正确。
3. 终端内容渲染架构上测不到。终端帧由 Rust 线程 wgpu 直绘 ANativeWindow，不产生主线程 doFrame，`FrameTimingMetric` 统计载荷不覆盖该路径；termlib 的 `TerminalFrameBenchmark`（text/cacafire 负载加 `FrameTimingMetric`）是为 Canvas/Compose 架构设计的，照搬得到空数据或误导数据。本领域正确工具已存在：criterion 微基准、渲染循环时序探针、可选 trace 关联。
4. CI 只编译不运行是对的。swiftshader 软件渲染模拟器的帧率不能作回归判据（现有基准注释已声明，termlib README 持同文结论）；`check.nu` 仅 `compileBenchmarkReleaseKotlin` 成本合适。上 CI 门禁的唯一候选是 `StartupTimingMetric` 相对回归检测，需要真机稳定环境，当前不现实。

## 行动建议

优先级从高到低，均不依赖 macrobenchmark 库：

1. 真机基准操作规程文档：同设备同条件交替顺序、p95/p99 判据、环境记录，作为后续所有真机性能工作的前提。
2. 真实 PTY 捕获重放：捕获脚本加 `vt_typing` 重放模式，替换合成负载。
3. Rust 帧循环 trace counter 与 FrameTimeline 关联：评估 input-to-present 延迟，覆盖 termlib 体系中 `FrameTimingMetric` 对本项目不可达的部分。

维持现状不动项：三个宏基准测试、CI 编译位、baselineprofile 闭环；不新增终端渲染的 `FrameTimingMetric` 负载基准。
