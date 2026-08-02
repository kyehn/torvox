# torvox Kotlin/Android 依赖调研报告

> 调研时间：2026-08。方法：① 本地代码盘点（`android/app/build.gradle.kts`、grep 手写实现）；② 逐库核对官方仓库/文档页面（版本、维护状态）。聚焦"哪些手写代码可被库替代、哪些库值得引入"。
> 关联文档：`docs/test-strategy-research.md`（测试策略，本次不重复）。

---

## 0. 项目现状速览

**已用依赖**（`android/app/build.gradle.kts`）：Compose BOM 2026.06.01、Hilt 2.60.1（+ hilt-navigation-compose 1.4.0 + hilt-android-testing）、navigation-compose 2.9.8、DataStore-preferences 1.2.1、LeakCanary 3.0-alpha-9（debugImplementation）、Roborazzi 1.70.0（plugin + test + androidTest）、Robolectric 4.16.1、MockK、Turbine、kotlinx-coroutines-test 1.11.0、ArchUnit、Cucumber、ML Kit text-recognition（androidTest）、detekt、PIT。**无 version catalog**（无 `libs.versions.toml`）。

**手写代码盘点**（grep `HttpURLConnection|org.json|ToneGenerator|ClipboardManager` 等）：

| 手写点 | 位置 | 规模 | 可被替代 |
|---|---|---|---|
| JSON 解析（org.json） | `bridge/Bridge.kt` `parseEvent()`（~80 行 opt 链）+ `ui/ToolbarPreferences.kt` | ~120 行 | kotlinx-serialization |
| HTTP 下载（HttpURLConnection） | `installer/BootstrapDownloader.kt`（https 校验、重定向防降级、1GiB 上限、进度回调、连接工厂注入测试） | ~130 行 | okhttp/okio |
| 权限请求 | `MainActivity.kt:401` `requestPermissions(POST_NOTIFICATIONS)`；其余用官方 `ActivityResultContracts` | 1 处 | 官方 API 已够 |
| 系统 UI 控制 | `TerminalSurface.kt`（`WindowInsetsControllerCompat` 显隐）、`enableEdgeToEdge()`、`statusBarsPadding()` | 数处 | 官方 API 已够 |
| 背景图解码 | `TerminalViewModel.kt:681-729`（BitmapFactory 两遍解码 + inSampleSize + RGBA 传 native） | ~45 行 | Coil |
| 剪贴板 / 铃声 | `TerminalRuntime.kt`、`TerminalSurface.kt` 等（ClipboardManager、ToneGenerator） | — | 终端功能语义，无库替代（正常） |
| 日期/时间 | `SimpleDateFormat` ×8 文件（日志/ANR 文件名）、`System.currentTimeMillis` | 轻量 | 见 §6 |

---

## 1. kotlinx-serialization vs 手写 org.json

- **候选**：`org.jetbrains.kotlinx:kotlinx-serialization-json:1.11.0` + `kotlin("plugin.serialization")` 编译器插件（版本与 Kotlin 同步）。
  来源：https://github.com/Kotlin/kotlinx.serialization
- **判断**：**可选（低优先级），当前不急于引入**。
- **理由**：
  - 确认项目**确为手写 org.json**：`Bridge.kt parseEvent()` 用 `optString/optInt/optJSONArray` 解析 Rust native 传来的事件流（带坏 JSON 容错 `try/catch + Log.w`），`ToolbarPreferences.kt` 解析/生成工具栏配置。合计 ~120 行。
  - 手写量小、语义已匹配场景：`opt*` 的"缺字段给默认值"容错恰好适合事件流；org.json 是平台内置零依赖。
  - kotlinx-serialization 优势（类型安全、`Json { ignoreUnknownKeys = true; coerceInputValues = true }` 可复刻容错、R8 规则内置、性能）在此场景收益有限。
- **若引入**：替换 `Bridge.kt` 的事件模型为 `@Serializable` data class + `ToolbarPreferences`，删 ~120 行；代价是新增编译器插件 + 依赖 + 重写两处解析及测试。**触发条件**：Bridge 事件字段继续膨胀、或与 Rust 侧 schema 需同步校验时再引入。

## 2. okhttp/okio vs 手写 HttpURLConnection

- **候选**：`com.squareup.okhttp3:okhttp:5.4.0`（+ BOM；测试用 `mockwebserver3`）。
  来源：https://github.com/lysine-dev/okhttp
- **判断**：**推荐（中优先级）**。
- **理由**：
  - 确认项目手写 `BootstrapDownloader.kt`（~130 行）：https 强制校验、301/302 重定向降级到 http 的防护、1GiB 硬上限、2% 步进进度回调、超时管理、`internalConnectionFactory` 注入供测试。
  - okhttp 直接覆盖：连接池/HTTP2、失败重试、TLS 1.3/ALPN、gzip、超时。**可删 ~100 行**并去掉手工 socket 生命周期管理。
  - 安全语义必须保留：okhttp 默认会跟随跨协议重定向，需 `followRedirects(false)` + 手动校验 `Location` 仍为 https（或校验最终 URL），维持现有"防降级"防线。下载 zip 会被解压执行 `postinst`，属安全敏感路径，**替换需配套重测**（可用 MockWebServer3 取代注入工厂，现有 `internalConnectionFactory` 可退役）。
  - ⚠️ 维护权变化：2025 年起 OkHttp 由 Square 移交 **lysine-dev** 社区维护（坐标不变 `com.squareup.okhttp3`），5.4.0 稳定、Android 生态地位不变，可放心用。
  - okio 作为传递依赖随附，未来若写日志 IO（`LogcatFileWriter`）可顺带受益，但**不为日志单独引入 okio**。

## 3. accompanist（权限 / 系统 UI）

- **候选**：`com.google.accompanist:accompanist-permissions`（0.37.x 支持 Compose 1.7+）。
  来源：https://github.com/google/accompanist
- **判断**：**明确不引入**。
- **理由**：
  - **System UI Controller 模块已废弃并移除**（官方指引迁移到 edge-to-edge）——项目已用 `enableEdgeToEdge()` + `WindowInsetsControllerCompat` 手写显隐 + `statusBarsPadding()`，正是官方路线，无需库。
  - permissions 模块仍在，但项目仅 1 个运行时权限（POST_NOTIFICATIONS，`MainActivity` 手写 `requestPermissions` 一处），且其余已用官方 `ActivityResultContracts`（OpenDocument/GetContent）——官方 API 已完全覆盖，accompanist 无增量价值。

## 4. collectAsStateWithLifecycle

- **候选**：`androidx.lifecycle:lifecycle-runtime-compose:2.11.0`（与现有 lifecycle 2.11.0 同版本）。
  来源：https://developer.android.com/topic/libraries/architecture/lifecycle
- **判断**：**推荐（最高优先级，最便宜）**。
- **理由**：
  - 确认项目**全部使用 `collectAsState()`**（~30 处：TerminalScreen ~10、SettingsScreen ~20、MainActivity、SessionDrawer），未引入 `lifecycle-runtime-compose`。
  - 官方最佳实践明确："Use `collectAsStateWithLifecycle` for data"（Lifecycle 2.7.0+ 引入，文档 2026-04 更新仍强调）。
  - 收益：后台/离屏（导航切走）时停止 Flow 收集与重组，省电省资源；机械替换 ~30 处，零行为风险。**注意**：native 渲染由前台服务 + Rust 引擎线程驱动，不依赖 Compose 重组，替换后后台渲染不受影响——正是期望行为。

## 5. Hilt vs 手动 DI

- **判断**：**已在用，无需动作**。
- **现状证据**：`@HiltAndroidApp`（TerminalApp）、`@AndroidEntryPoint`（MainActivity）、`@HiltViewModel`（TerminalViewModel）、`@Inject`（runtime/settings 各层）、`hilt-navigation-compose` + `hilt-android-testing` + KSP 全链就绪。手动 DI 不存在。
  来源：https://dagger.dev/hilt/ （项目内 `com.google.dagger:hilt-android:2.60.1` 为 2026 年现行版本）

## 6. kotlinx-datetime / kotlinx-coroutines

- **kotlinx-coroutines**：**已在用**（核心库经传递依赖提供；测试直接依赖 1.11.0）。
  小建议：`implementation("org.jetbrains.kotlinx:kotlinx-coroutines-android:1.11.0")` 显式声明核心依赖（依赖可见性最佳实践），非必须。
- **kotlinx-datetime**：**明确不引入**。
  来源：https://github.com/Kotlin/kotlinx-datetime
  - 项目日期用途仅两类：`SimpleDateFormat` 格式化（日志/ANR/备份文件名）×8 文件、`System.currentTimeMillis` 计算——均为本地纯展示，无解析/时区换算需求。
  - kotlinx-datetime 0.8.0 官方标注 **experimental**（README 明示 API 可变）；JVM 上底层就是 java.time。minSdk 33 ≥ 26，真需要改进时**直接用 java.time 零依赖**即可，不引入 kotlinx-datetime。

## 7. LeakCanary

- **判断**：**已在用**（`debugImplementation("com.squareup.leakcanary:leakcanary-android:3.0-alpha-9")`，3.x 自动安装无需 Application 改动）。
  来源：https://github.com/square/leakcanary
- **备注**：所用为 **3.0 alpha 线**（2.14 为上一稳定线）。仅 debug 生效、不进 release，风险可控；建议跟踪 3.0 正式版发布后升级，当前无需动作。

## 8. Roborazzi

- **判断**：**已在用**（插件 + test + androidTest 三处 1.70.0，与 Robolectric 4.16.1 配套）。
  来源：https://github.com/takahirom/roborazzi
- **备注**：仓库活跃（2026 年持续发版），建议随版本升级保持与 Robolectric 兼容，无需其他动作。

## 9. Coil（背景图片）

- **候选**：`io.coil-kt.coil3:coil-compose:3.5.0`（**不需要** `coil-network-okhttp`——背景图为本地 content URI，避免连带 okhttp）。
  来源：https://github.com/coil-kt/coil
- **判断**：**可选（低优先级）**。
- **理由**：
  - 现状：`TerminalViewModel.applyBackgroundImageFromPath()` 手写 BitmapFactory 两遍解码（inJustDecodeBounds → inSampleSize 防 48MP OOM）→ RGBA 字节缓冲 → native bridge（blur/alpha 在 Rust 侧）。~45 行，已处理 OOM 但**未处理 EXIF 方向**（竖拍照片会横置，真实痛点）。
  - Coil 收益：EXIF 方向自动校正、内存缓存、downsample 更稳；`ImageLoader.execute()` 仍可拿 Bitmap 转 RGBA 喂 native，集成点清晰。
  - 成本：+1 依赖（R8 友好）。缓存收益有限（背景图每会话加载一次），主要价值是 EXIF 与删 ~45 行。
- **结论**：不阻塞、可做可不做；若做，用 `coil-compose` 核心模块即可。

## 10. navigation-compose

- **判断**：**已在用且必要**（`navigation-compose:2.9.8` + `hilt-navigation-compose:1.4.0`，`MainActivity` 内 `TerminalNavHost()` 管理 Terminal/Settings 导航，Hilt 集成完成）。
  来源：https://developer.android.com/develop/ui/compose/navigation
- **备注**：无引入必要；若未来导航图复杂化可评估 Navigation 3，当前 2.9.8 与 Compose BOM 2026.06.01 配套稳定。

---

## 值得引入清单（按优先级）

| # | 动作 | 依赖 | 收益 | 风险/成本 |
|---|---|---|---|---|
| 1 | 引入 lifecycle-runtime-compose，`collectAsState()` → `collectAsStateWithLifecycle()`（~30 处） | `androidx.lifecycle:lifecycle-runtime-compose:2.11.0` | 官方推荐；后台/离屏停止收集与重组，省电；机械替换 | 极低（同版本族，无 API 破坏） |
| 2 | BootstrapDownloader 换 okhttp | `com.squareup.okhttp3:okhttp:5.4.0`（+ mockwebserver3 测试） | 删 ~100 行；连接池/重试/TLS 1.3；测试可弃注入工厂 | 中（安全敏感路径：必须保留 https 防降级语义，需重测） |
| 3 | （可选）背景图解码换 Coil | `io.coil-kt.coil3:coil-compose:3.5.0` | 删 ~45 行；EXIF 方向修正、缓存 | 低（新增 1 依赖；仅核心模块） |
| 4 | （可选）org.json → kotlinx-serialization | `kotlinx-serialization-json:1.11.0` + serialization 插件 | 删 ~120 行、类型安全（Bridge 事件模型重构时再动） | 中（新增编译器插件；等 Bridge schema 变化触发） |
| 5 | （顺手）显式声明 `kotlinx-coroutines-android:1.11.0` | 同上版本 | 依赖可见性清晰 | 零 |

## 明确不引入清单

| 库 | 原因 |
|---|---|
| **accompanist（permissions / systemuicontroller）** | SystemUI Controller 已废弃移除；权限仅 1 处且官方 `ActivityResultContracts` 已覆盖；edge-to-edge 已落地 |
| **kotlinx-datetime** | 官方 experimental；项目日期用法（SimpleDateFormat 格式化文件名）收益趋零；真需改进用 java.time（minSdk 33） |
| **Hilt 替代 / 手动 DI** | Hilt 全链已在用（App/Activity/ViewModel/Repository 注入完备） |
| **LeakCanary** | 已在用（debugImplementation）；仅需跟踪 3.0 正式版 |
| **Roborazzi** | 已在用（1.70.0，test + androidTest） |
| **navigation-compose** | 已在用（2.9.8 + Hilt 集成） |
| **okio（单独引入）** | 仅随 okhttp/Coil 传递获得即可，日志 IO 不值得为它单独加依赖 |
| **coil-network-okhttp** | 背景图是本地 content URI，不需要网络模块（避免连带依赖） |
