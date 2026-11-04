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
- **THEN** 用例失败并报出呈现帧数为 0，而不是留下一个测了个寂寞的空窗口

#### Scenario: 启动超预算判红

- **WHEN** 冷启动到终端可用的耗时超过预算
- **THEN** 用例失败并报出实测耗时与预算值

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

帧级阈值与耗时预算 MUST 以 CI 实测值校准（实测的约 1.5 倍口径，与 Rust 微基准
上限断言同口径），并在常量注释里引用实测出处；MUST NOT 凭空设定，也不得宽到只拦
「卡死级」。`FrameTimingMetric` 的百分位只进 IDE 结果 bundle，模拟器上可脚本采集
的帧级数据按上一条 Requirement 的分工取（gfxinfo 或原生呈现汇总）。

#### Scenario: 实测值落进构建产物

- **WHEN** CI 运行宏基准
- **THEN** 连接测试产出的用例 logcat 里出现每轮迭代的实测耗时与帧统计
      （`[bench-measure]` 行），断言预算可据此校准

#### Scenario: 断言预算有实测出处

- **WHEN** 给帧级阈值或耗时设断言阈值
- **THEN** 常量注释引用 CI 实测值与运行出处，阈值约为实测的 1.5 倍

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

#### Scenario: shell 未就绪判红

- **WHEN** 探针在上限内始终没有回显
- **THEN** 用例失败并报出「shell 未就绪」，MUST NOT 继续测量一个空终端
