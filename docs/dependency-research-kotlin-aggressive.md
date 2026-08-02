# torvox Kotlin/Android 依赖调研报告（激进版）

> 调研时间：2026-08。定位：**最先进激进**——允许不稳定版本、最新预览版、激进架构，但每项判断必须落到本项目实际手写代码量，拒绝泛泛而谈。
> 前置文档：`docs/dependency-research-kotlin.md`（保守版）——其结论（lifecycle-runtime-compose 引入、okhttp 推荐、accompanist/kotlinx-datetime 排除等）不重复，本文只覆盖**激进差异点**与**保守版遗漏的新发现**。
> 方法：本地代码盘点（build.gradle.kts、grep 手写实现、源码使用验证）+ 官方仓库 release API / Maven Central / Google Maven 元数据逐库核对。

## 0. 与保守版不同的新发现（重要）

1. **navigation-compose 是死依赖**：`android/app/build.gradle.kts:138` 声明 `navigation-compose:2.9.8`，但全源码树（`src/`）**零 import**——`MainActivity.TerminalNavHost()`（第 688 行）根本不是 NavHost，而是手写 `remember { mutableStateOf(showSettings) }` + Box 覆盖层（~50 行）。保守版称"已在用且必要"是误判。激进结论：**删除显式声明**（`hilt-navigation-compose:1.4.0` 的 `hiltViewModel()` 在用，其传递依赖仍会带入 navigation-compose，删声明只是去掉直依赖与显式版本钉扎）。
2. **Molecule 在 cashapp 不在 square**（`cashapp/molecule`），保守版未评估。
3. **kotlinx-serialization 没有 2.x**：最新为 1.11.0（2026-04-09，基于 Kotlin 2.3.20）。但 1.11.0 新增 `Json { exceptionsWithDebugInfo = false }`——**契合本项目"日志不泄漏用户数据"的既有安全实践**（BootstrapDownloader 注释 round-102/103 明示此原则），这是保守版没有的推荐理由。
4. **Kotlin 2.4.10 / AGP 9.3.1**（`android/build.gradle.kts`），serialization 编译器插件版本随 Kotlin 走，无版本冲突问题。
5. **根构建已预留 baseline profile 插件**：`androidx.baselineprofile:1.5.0-alpha06` + `androidx.benchmark:1.5.0-alpha06` 均以 `apply false` 声明但从未启用——这是"激进但低风险"的现成抓手。
6. **kotlinx-coroutines 没有 2.x 预览**：最新 1.11.0（2026-05-08，Kotlin 2.2.20），且修复了 `shareIn/stateIn` 被 R8 误 GC 的 bug（#4646）——项目 20+ 处 `stateIn`/`_state.update`，值得显式钉版本。
7. **Compose BOM 2026.06.01 就是最新**；androidx.core 1.19.0、lifecycle 2.11.0、activity 1.13.0 全部已是 Google Maven 最新稳定——"升级"方向无剩余空间，激进空间全在**新增库**与**删死代码**。

---

## 1. kotlinx-serialization（替换手写 org.json）→ **激进推荐（升级为 TOP 3）**

- **候选**：`org.jetbrains.kotlinx:kotlinx-serialization-json:1.11.0` + `kotlin("plugin.serialization")`（版本随 Kotlin 2.4.10）。
  - 来源：https://github.com/Kotlin/kotlinx.serialization/releases/tag/v1.11.0
- **理由（比保守版多两条硬理由）**：
  - 手写量确认：`Bridge.kt parseEvent()`（~70 行 `optString/optInt/optJSONArray` 链）+ `ui/ToolbarPreferences.kt`（~50 行 JSONArray/JSONObject 读写）= **~120 行**。
  - **1.11.0 新安全特性**：`Json { exceptionsWithDebugInfo = false }` 让坏 JSON 的异常信息不再内嵌输入原文（含 clipboard 文本、URL 等）。项目已在 BootstrapDownloader 中为"日志不落 token/URL"专门写注释（round-102/103），这个配置直接服务同一原则。保守版只给了"类型安全"，激进版多了"日志脱敏"这一条。
  - 1.11.0 公开 `JsonException/JsonDecodingException/JsonEncodingException`（experimental）——可精确区分"坏 JSON 跳过"与"字段类型不符"，替代现在的 `catch (Exception)` 一刀切。
  - 容错语义可 1:1 复刻：`ignoreUnknownKeys = true`（= 未知字段忽略）+ `coerceInputValues = true`（= 缺字段/非法值走默认值，与 `opt*` 语义对齐）+ 模型字段全部给默认值。
- **用在哪 / 减多少**：`Bridge.parseEvent` 事件模型改为 `@Serializable` sealed class（Bell/Notification/Clipboard/Exit/Dialog/PickFile/Toast/OpenUrl/GetClipboard）+ `ToolbarPreferences` 的 `@Serializable ToolbarLayout`。删 **~120 行**。
- **风险（unstable 标注）**：中。① 新增编译器插件（KSP 已有，加 serialization 插件零成本）；② **行为变化是特性而非缺陷**——现在 Rust 侧加字段是隐式容错，切换后 Kotlin 模型必须显式加字段（schema 漂移会在编译期暴露，正是要的类型安全，但需与 Rust 侧 `pollEvent` 输出对齐一次）；③ 现有坏 JSON 容错测试需重写并保留语义；④ `JsonException` 新 API 为 experimental，**不依赖它也可用**（只影响日志分类，不影响主路径）。Release 版 1.11.0 本身稳定（1.10 起 API 稳定化为主）。

## 2. kotlinx-io（替换手写字节缓冲）→ **明确不引入**

- **候选**：`org.jetbrains.kotlinx:kotlinx-io-core:0.9.1`（2026-06-26）。
  - 来源：https://github.com/Kotlin/kotlinx-io/releases/tag/0.9.1
- **理由**：
  - 唯一疑似匹配点是 `runtime/InputBatchBuffer.kt`（~130 行），但它手写的**不是字节缓冲库能替代的东西**：Choreographer 帧回调调度 + 单线程 "PtyWriter" daemon executor + PTY 满时（EAGAIN）xterm 式丢字节背压语义 + 顺序保持。其内部 `ByteBuffer.allocateDirect(capacity)` 只是 ~10 行的薄封装。
  - `LogcatFileWriter.kt`（~200 行）的轮转/大小跟踪/锁是业务逻辑，okio/kotlinx-io 只能简化写缓冲 ~30 行。
  - **unstable 标注**：kotlinx-io 仍是 0.x（0.8→0.9 有 breaking change），0.9.0 才切 Kotlin 2.3。为 30 行收益引入 experimental 依赖，激进方向错了。
- **若 okhttp 引入后 okio 随传递进来**：LogcatFileWriter 用 okio `BufferedSink` 属可选项（还能顺手符合 StrictMode `detectUnbufferedIo` 已开启的现状），但**不为此单独引入任何 IO 库**。

## 3. Molecule（Compose 状态管理激进方案）→ **激进试点（观望，非立即引入）**

- **候选**：`app.cash.molecule:molecule-runtime:2.2.0`（**cashapp/molecule**，非 square；2025-09-24；2.x 已无需专属 Gradle 插件——用官方 `org.jetbrains.kotlin.plugin.compose`，项目已有）。
  - 来源：https://github.com/cashapp/molecule/releases/tag/2.2.0
- **理由（本项目形态高度匹配，但有两处硬伤）**：
  - 匹配点：`TerminalViewModel` 有 **25 个 `stateIn(WhileSubscribed)` 设置流** + 1 个 `TerminalState`（`MutableStateFlow` + 20+ 处 `_state.update`）。Molecule 的 `launchMolecule` 恰好把"多流 → 一个 StateFlow"变成声明式 Compose 函数，**可删 ~100–150 行样板**（25 个 stateIn 声明 + 各自初始值 + 部分 update 合并）。2.2.0 已切换到 AndroidX Compose runtime，minSdk 23 ≤ 项目 33，兼容。
  - 节奏匹配：Molecule 以帧为节奏发射状态，与 `pollAll()` 每帧轮询的现有架构天然同频。
  - 硬伤 1（命令式事件不适用）：bootstrap 进度、poll 结果、selection 拖动、`pastePopupRequest` 等 **20+ 处命令式 `_state.update` 无法被 Molecule 替代**——它们仍是事件驱动写 MutableStateFlow，Molecule 只能接管"设置聚合"层，收益打折一半。
  - 硬伤 2（维护节奏）：2.2.0 发布近一年无新 release；社区小众；@HiltViewModel + Molecule 组合无官方先例。
- **风险（unstable 标注）**：中。2.x API 稳定（2.0 仅构建变更、二进制兼容 1.x），但项目引入新状态范式需重写 TerminalViewModel 设置层 + 全部相关测试（Turbine 仍可测 StateFlow，测试改造可控）。
- **若做**：先在 `SettingsState` 聚合上试点（25 个 stateIn → 1 个 Molecule 流），不动 TerminalState 事件路径；验证一个里程碑后再决定是否扩散。

## 4. Decompose（激进导航/组件化）→ **明确不引入**

- **候选**：`com.arkivanov.decompose:decompose:3.5.0`（2026-03-15 稳定；3.6.0-alpha01 已出，活跃）。
  - 来源：https://github.com/arkivanov/Decompose/releases/tag/3.5.0
- **理由**：
  - **导航面根本不存在**：MainActivity 的"导航"是 `showSettings: Boolean` + Box 覆盖层（~50 行），连 navigation-compose 都没用。Decompose 的 Child Stack/生命周期管理/back 处理对一个"终端 + 设置"两屏应用是纯架构税。
  - `TerminalSurface`（87KB）与 `TerminalScreen`（41KB）的复杂度在渲染与输入语义，不在导航拓扑。Decompose 解决不了任何现有痛点。
  - 引入代价：重写 MainActivity 为 `RootComponent` + 学习曲线 + 与 Hilt 手工接线，收益为零。

## 5. Coil 3（背景图）→ **激进推荐（低风险高确定性）**

- **候选**：`io.coil-kt.coil3:coil-compose:3.5.0`（2026-06-10；**不需要** `coil-network-okhttp`——背景图是本地 content URI）。
  - 来源：https://github.com/coil-kt/coil/releases/tag/3.5.0
- **理由**：
  - 手写点确认：`TerminalViewModel.applyBackgroundImageFromPath()`（~50 行）两遍 BitmapFactory 解码 + 手写 inSampleSize 循环（1920×1080 上限）→ RGBA 喂 native。**已防 OOM 但完全没处理 EXIF 方向**——竖拍照片横置是真实用户痛点，手写修复需 ~30 行 EXIF 解析（ExifInterface）。
  - Coil 直接覆盖：EXIF 自动校正 + downsample（给 ImageRequest size 即替代 inSampleSize 循环）+ 内存缓存 + 磁盘缓存。
  - 集成点清晰：`ImageLoader.execute()` 拿 `Bitmap` 转 RGBA 的现有代码保留，只替换解码段。
- **用在哪 / 减多少**：`applyBackgroundImageFromPath` 解码部分，删 **~45 行**，另免去手写 EXIF 修正的 ~30 行。
- **风险**：低。+1 依赖（R8 规则内置）；`coil-compose` 核心模块无网络组件连带。注意把目标尺寸（1920×1080）显式传给 ImageRequest 以保留现有防 OOM 语义。

## 6. App Startup（初始化聚合）→ **明确不引入**

- **候选**：`androidx.startup:startup-runtime:1.2.0`（2024-09 发布后两年未更新，生态已定型）。
  - 来源：https://dl.google.com/android/maven2/androidx/startup/startup-runtime/maven-metadata.xml
- **理由**：
  - 手写点确认：`TerminalApp.onCreate()`（~50 行）是**强顺序业务初始化**：BootGuard 日志轮转 → BootGuard 检查 → StrictMode → LogcatFileWriter → 后台线程 native init → ANR/Memory/Thermal 监控 → 崩溃处理器 → 10 分钟健康标记。崩溃处理器必须在任何异常前注册、BootGuard 必须先于一切——这是 boot-loop 防护，**顺序即安全语义**。
  - Initializer 化不减少代码：每个 Initializer 一个类 + manifest 声明 + 依赖图配置，把 50 行显式顺序换成分散的隐式图，可读性反而下降。
  - 收益（合并 ContentProvider 省启动时间）在本项目不存在：Hilt 的 ContentProvider 无法被 App Startup 消除；LeakCanary 仅 debug。

## 7. kotlinx-datetime（激进重新评估）→ **明确不引入；激进替代：java.time 零依赖替换**

- **候选**：`org.jetbrains.kotlinx:kotlinx-datetime:0.8.0`（2026-05-07 稳定；Instant/Clock 已迁入 Kotlin stdlib）。
  - 来源：https://github.com/Kotlin/kotlinx-datetime/releases/tag/v0.8.0
- **理由**：
  - 8 个文件（MainActivity/TerminalApp/AnrWatchDog/BootGuard/ThermalMonitor/LogcatFileWriter 等）的 `SimpleDateFormat` 用途全是**本地纯展示时间戳**（日志/ANR/备份文件名），无解析、无时区换算。0.8.0 的 `parseOrNull`/`LocalIsoWeekDate` 等新 API 全部用不上。
  - **更激进且零依赖**：用 java.time `DateTimeFormatter` 替换。minSdk 33 ≥ 26，java.time 全量可用。额外收益：**`SimpleDateFormat` 非线程安全**（LogcatFileWriter.dateFormat 虽在锁内，但每个调用点各自 new 实例是浪费），`DateTimeFormatter` 线程安全可做单例。
  - 结论：激进方向是"删依赖思维"——不引 kotlinx-datetime，做 8 文件机械替换（每处减 ~2 行 import + 实例化），零风险。

## 8. okhttp 5（BootstrapDownloader）→ **激进推荐（已决定，TOP 1）**

- **候选**：`com.squareup.okhttp3:okhttp:5.4.0`（Maven Central 确认最新，2026-06-08；5.0.0 起为稳定线，坐标不变）。
  - 来源：https://repo1.maven.org/maven2/com/squareup/okhttp3/okhttp/maven-metadata.xml
- **理由**：保守版已详述（连接池/HTTP2/重试/TLS 1.3/gzip/超时，删 ~100 行，mockwebserver3 退役 `internalConnectionFactory`）。激进版补充确认：**5.x API 与 4.x 相同**（OkHttpClient/Request/call.execute()），无迁移陷阱。
- **用在哪 / 减多少**：`installer/BootstrapDownloader.kt`，删 **~100 行**（手工 socket 生命周期、连接工厂、超时管理）。
- **风险**：中（安全敏感路径）。**必须保留**：① `followRedirects(false)` + 手动校验最终 URL 协议为 https（okhttp 默认跟随跨协议重定向，与 HttpURLConnection 同样的防降级语义要显式做）；② 1GiB 硬上限与进度回调是业务代码，保留。维护权已移交 lysine-dev 社区（2025 年起），坐标与稳定性不受影响。测试用 `mockwebserver3` 替换注入工厂，现有防降级/超限/进度用例全部重跑。

## 9. 其他值得的（含保守版遗漏项）

### 9.1 kotlinx-coroutines：无 2.x，但 1.11.0 值得显式声明
- 最新 1.11.0（2026-05-08）。**不存在 2.x 预览**（用户假设不成立）。
  - 来源：https://github.com/Kotlin/kotlinx.coroutines/releases/tag/1.11.0
- 动作：`implementation("org.jetbrains.kotlinx:kotlinx-coroutines-android:1.11.0")` 显式声明（现仅测试依赖钉了 1.11.0，主依赖走传递）。1.11.0 修复 `stateIn/shareIn` 协程被 R8 误 GC（#4646）——项目 25 处 stateIn 是直接受益者。**零风险**。

### 9.2 Compose BOM / Material3 / androidx.core / lifecycle：全部已是最新，无动作
- Compose BOM 2026.06.01 = Google Maven 最新（2026-07-01 更新确认）；core-ktx 1.19.0、lifecycle 2.11.0、activity 1.13.0 均为最新稳定。
  - 来源：https://dl.google.com/android/maven2/androidx/compose/compose-bom/maven-metadata.xml
- Material3：BOM 内 1.4.x 稳定；1.5.0-alpha25 为最新预览（2026-07-29）。grep 确认项目使用的组件（AlertDialog/Button/Slider/Switch/OutlinedTextField/ModalNavigationDrawer/SnackbarHostState/LinearProgressIndicator/IconButton）**全部是稳定 API，无过时用法**（仅 2 处 `@OptIn(ExperimentalMaterial3Api)` 属正常）。1.5 alpha 无必须特性，不追。
  - 来源：https://dl.google.com/android/maven2/androidx/compose/material3/material3/maven-metadata.xml

### 9.3 Baseline Profile（新激进项，插件已预留）→ **激进推荐（收益真实、风险低-中）**
- 现状：根 `android/build.gradle.kts` 已 `apply false` 声明 `androidx.baselineprofile:1.5.0-alpha06` + `androidx.benchmark:1.5.0-alpha06`，但 app 模块未启用、无 `baseline-prof.txt`、无 `profileinstaller` 依赖。
- 理由：项目冷启动链长（BootGuard 日志 IO + StrictMode + 监控器 + Compose 首帧 + **material-icons-extended 全量图标库**——典型的 Baseline Profile 受益者）。官方数据冷启动可减 15–30%。
- 动作：app 模块 apply `androidx.baselineprofile` 插件 + `releaseImplementation("androidx.profileinstaller:profileinstaller")` + 生成 baseline-prof.txt（手写规则或 `generateBaselineProfile` 跑一次真机）。
- **风险（unstable 标注）**：低-中。插件本身 1.5.0-alpha06 是 alpha（不稳定标注）；需要一台真机/模拟器跑 profile 生成；profileinstaller 进 release 有 ~30KB 成本。但根构建已预留插件说明项目本有此意图，属"激进且顺手"。

### 9.4 终端模拟器专门库（termux）→ **明确不引入**
- termux-app 提供 `terminal-emulator` / `terminal-view` / `termux-shared` 三模块。
  - 来源：https://github.com/termux/termux-app
- 理由：
  - `terminal-view` 是 **Java 实现的终端模拟器 + 渲染器**，与本项目"Rust（Ghostty）引擎 + native 渲染"架构**根本冲突**——引入等于换引擎。
  - `termux-shared` 是 termux-app 内部模块，**未作为独立 Maven artifact 发布**（仅 jitpack 拼装），且与 termux-app 包结构深度耦合。
  - 本项目 `applicationId = com.termux` 复刻的是**协议与数据兼容**（bootstrap 流程、extra keys 布局语义），这些已手写完成（BootstrapDownloader/BootstrapInstaller/ModifierBar）。无 Java 侧可复用点。

---

## 激进引入清单（按收益/风险比排序）

| # | 动作 | 依赖/版本 | 收益 | 风险（unstable 标注） |
|---|---|---|---|---|
| 1 | BootstrapDownloader → okhttp | `com.squareup.okhttp3:okhttp:5.4.0`（+ mockwebserver3 测试） | 删 ~100 行；连接池/重试/TLS 1.3；退役注入工厂 | 中：安全敏感，必须保留 https 防降级语义并重测 |
| 2 | org.json → kotlinx-serialization | `kotlinx-serialization-json:1.11.0` + serialization 插件（随 Kotlin 2.4.10） | 删 ~120 行；事件模型类型安全；`exceptionsWithDebugInfo=false` 日志脱敏（1.11 新特性） | 中：Rust↔Kotlin schema 需对齐一次；容错测试重写；JsonException 新 API experimental（不用也无碍） |
| 3 | 背景图 → Coil 3 | `io.coil-kt.coil3:coil-compose:3.5.0` | 删 ~45 行 + **EXIF 方向修正**（真实痛点）；downsample 替代手写循环 | 低：仅核心模块；需显式传目标尺寸保 OOM 语义 |
| 4 | 删除 navigation-compose 死依赖 | 移除 `androidx.navigation:navigation-compose:2.9.8` 声明 | 去掉直依赖与版本钉扎（源码零使用） | 极低：hilt-navigation-compose 传递依赖仍满足 `hiltViewModel()` |
| 5 | SimpleDateFormat → java.time | 零依赖 | 8 文件机械替换；`DateTimeFormatter` 线程安全；删 import 与实例化 | 极低 |
| 6 | 显式声明 coroutines | `kotlinx-coroutines-android:1.11.0` | 钉版本；获 stateIn R8 GC 修复（#4646） | 极低 |
| 7 | 启用 Baseline Profile | `androidx.baselineprofile:1.5.0-alpha06`（已预留）+ profileinstaller | 冷启动 -15~30%（material-icons-extended 受益） | **unstable（插件 alpha）**：需真机生成 profile；release +30KB |
| 8 | Molecule 试点 | `app.cash.molecule:molecule-runtime:2.2.0` | 25 个 stateIn 设置流 → 声明式聚合，删 ~100–150 行样板 | **unstable（2.2.0 近一年无新 release、社区小众）**：命令式事件（20+ 处 _state.update）无法接管，只做设置层；先试点后扩散 |

## 明确不引入

| 库 | 原因 |
|---|---|
| **Decompose 3.5.0 / Navigation 3（1.2.0-alpha）** | 导航实为手写布尔 overlay（~50 行），两屏应用；navigation-compose 都没用，架构税无收益 |
| **kotlinx-io 0.9.1** | 0.x experimental（0.8→0.9 breaking）；InputBatchBuffer 手写核心是帧调度/executor/背压语义非缓冲；LogcatFileWriter 仅 ~30 行收益 |
| **kotlinx-datetime 0.8.0** | 8 处用法全是本地时间戳格式化；java.time（minSdk 33）零依赖且线程安全，更激进 |
| **App Startup 1.2.0** | TerminalApp 初始化是强顺序安全语义（BootGuard/崩溃处理器），Initializer 化不减少代码反而分散；1.2.0 两年未更新 |
| **termux terminal-view / termux-shared** | Java 终端渲染器与 Rust 引擎架构冲突；termux-shared 非独立 artifact |
| **Material3 1.5.0-alpha25** | 项目组件全为稳定 API 无过时用法；alpha 无必须特性 |
| **coil-network-okhttp** | 背景图为本地 content URI，不需要网络模块（避免连带 okhttp 依赖） |
| **accompanist / Navigation 3 / kotlinx-coroutines 2.x** | 保守版已排除 / 导航面不存在 / 不存在 2.x（最新 1.11.0） |

## 附：来源 URL 汇总

- kotlinx-serialization：https://github.com/Kotlin/kotlinx.serialization/releases/tag/v1.11.0
- kotlinx-io：https://github.com/Kotlin/kotlinx-io/releases/tag/0.9.1
- Molecule：https://github.com/cashapp/molecule/releases/tag/2.2.0 · https://repo1.maven.org/maven2/app/cash/molecule/molecule-runtime/maven-metadata.xml
- Decompose：https://github.com/arkivanov/Decompose/releases/tag/3.5.0
- Coil：https://github.com/coil-kt/coil/releases/tag/3.5.0
- okhttp：https://repo1.maven.org/maven2/com/squareup/okhttp3/okhttp/maven-metadata.xml · https://github.com/lysine-dev/okhttp
- kotlinx-coroutines：https://github.com/Kotlin/kotlinx.coroutines/releases/tag/1.11.0
- kotlinx-datetime：https://github.com/Kotlin/kotlinx-datetime/releases/tag/v0.8.0
- androidx.startup：https://dl.google.com/android/maven2/androidx/startup/startup-runtime/maven-metadata.xml
- Compose BOM：https://dl.google.com/android/maven2/androidx/compose/compose-bom/maven-metadata.xml
- Material3：https://dl.google.com/android/maven2/androidx/compose/material3/material3/maven-metadata.xml
- Navigation 3：https://dl.google.com/android/maven2/androidx/navigation3/navigation3-runtime/maven-metadata.xml
- androidx.core：https://dl.google.com/android/maven2/androidx/core/core-ktx/maven-metadata.xml
- lifecycle-runtime-compose：https://dl.google.com/android/maven2/androidx/lifecycle/lifecycle-runtime-compose/maven-metadata.xml
- termux-app：https://github.com/termux/termux-app
