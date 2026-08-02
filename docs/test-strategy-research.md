# torvox 测试策略调研报告

> 调研时间：2026-08（所有 crates.io 下载量/版本数据为调研当日实时值）
> 调研对象：Rust 测试工具链、Kotlin/JVM 测试栈、模拟器依赖削减、同类终端项目测试分层
> 结论落地：文末给出五层测试金字塔建议（Rust 单测 / Kotlin-JVM(Robolectric) / Compose-Robolectric / Instrumented / E2E-Maestro）

---

## 1. Rust 测试工具

### 1.1 proptest vs quickcheck：当前选择

| 维度 | proptest | quickcheck |
|---|---|---|
| 总下载量 | 1.61 亿 | 6 227 万 |
| 近 90 天下载量 | 4 210 万 | 884 万 |
| 最新版本 / 发布时间 | 1.11.0 / 2026-03-24 | 1.1.0 / 2026-02-10 |
| 维护组织 | proptest-rs（活跃，多次年度发版） | BurntSushi（Andrew Gallant）个人维护，1.0.3（2021）到 1.1.0（2026）间隔 5 年 |
| 核心 API | 策略组合（`Strategy`）+ `prop_test!` 宏，内置 shrinking、fork/超时防护、no_std 支持 | `Arbitrary` trait + `#[quickcheck]` 宏，随机值生成 + shrinking |

- 来源：https://crates.io/crates/proptest 、https://crates.io/crates/quickcheck

**结论与理由**：
- 两者**流行度差距明显**（proptest 近 90 天下载量约为 quickcheck 的 4.8 倍），proptest 生态更活跃、API 更现代（策略可组合、可精细控制收缩与分布，处理"特定输入形态"比 Arbitrary 自动生成强得多）。
- 本项目根 `Cargo.toml` 与 `native/Cargo.toml` 已同时声明 `proptest 1.11`、`quickcheck 1.1` + `quickcheck_macros`，**现状是"双轨"**。
- **建议：以 proptest 为主力**（终端引擎的网格/光标/选区等状态机最适合策略化生成输入序列），quickcheck 保留给少量"类型级 Arbitrary 直给"的简单属性（例如字节流往返），**新测试默认用 proptest**，避免两套心智负担。

### 1.2 loom vs shuttle：并发测试

| 维度 | loom | shuttle |
|---|---|---|
| 总下载量 | 5 682 万 | 475 万 |
| 近 90 天下载量 | 990 万 | 126 万 |
| 最新版本 / 时间 | 0.7.2 / 2024-04-23（此后约两年未发版） | 0.9.1 / 2026-04-21（活跃） |
| 维护方 | tokio-rs | awslabs |
| 机制 | 置换测试（permutation testing）：穷举线程交错，要求被测代码改用 loom 的原子/锁/`LazyLock` 等替代 std | 基于 tokio 的调度随机化 + 确定性回归（`replay`），**原生支持 async**；同步并发也能测 |
| 学习成本 | 高：必须把 `std::sync`/原子替换成 `loom::` 类型，`cfg(loom)` 双写 | 中：`#[test]` 包一层 `shuttle::check`/`run` 即可，被测代码大多无需改动；async 场景直接可用 |

- 来源：https://crates.io/crates/loom 、https://crates.io/crates/shuttle

**结论与理由**：
- **本项目 `native/Cargo.toml` 已引入 `shuttle = "0.9"`，方向正确，应继续作为并发测试主力**。理由：① 引擎与 JNI 桥大量使用 `tokio` + `flume` 通道 + `futures`，shuttle 对 async 原生支持，loom 对 async 支持有限（需 `loom::futures` 且限制多）；② shuttle 无需替换并发原语，改造成本低；③ loom 自 2024 年中起发版停滞，而 shuttle 保持月度级活跃迭代。
- loom 仅在出现"无 async 的 lock-free/手写原子结构"（如未来引入无锁环形缓冲）时按需引入；当前**不建议新增 loom**。
- 适用场景定位：shuttle 用于通道竞争、会话读写并发、渲染任务与输入事件竞态；注意 shuttle 单测跑得慢（随机调度），应放到 CI 的慢速任务里。

### 1.3 cargo-nextest：并行测试执行器

- 数据：总下载 1 170 万，近 90 天 171 万，最新 0.9.140 / 2026-07-05（双周级发版，极活跃）。来源：https://crates.io/crates/cargo-nextest 、官方文档 https://nexte.st/
- **与 cargo test 完全兼容**：测试代码零改动，只换 runner（`cargo nextest run`）；支持 `#[ignore]`、自定义超时、失败重试（`--retries`）、flaky 测试标记、JUnit XML 报告、按测试二进制并行（默认每个二进制一个进程，可配置进程内并行）。
- **结论：强烈建议引入**。本项目是 workspace（native / exec-bin / integration-tests 三成员），integration-tests 里有 wgpu 场景级测试（重、慢），nextest 的并行与超时控制能显著缩短 CI 时间；JUnit 输出便于与既有 Android 侧报告风格统一。唯一注意：依赖环境变量/共享资源的测试需用 `nextest` 的 `serial` 或测试分组配置隔离。

### 1.4 cargo-llvm-cov：覆盖率

- 数据：总下载 689 万，近 90 天 132 万，最新 0.8.7 / 2026-05-13（taiki-e 维护，活跃）。来源：https://crates.io/crates/cargo-llvm-cov
- 能力：基于 LLVM source-based coverage（`-C instrument-coverage`），支持 lcov / html / coveralls / codecov 等输出；workspace 一键 `cargo llvm-cov --workspace`。
- **结论：适合本项目**：Rust 侧（native + integration-tests）用 `cargo llvm-cov --workspace --lcov --output-path lcov.info` 接入 CI，作为覆盖率趋势与"测试金字塔是否倒挂"的度量。注意覆盖 wgpu 渲染路径的测试需要 `test-util` feature 与软件渲染路径（`wgpu` 的 `gles` 后端可无头跑），覆盖率数字应剔除 GPU 分支噪音。
- 另注：本项目已有 `docs/test-coverage-audit.md` 手工审计，llvm-cov 可把审计自动化。

### 1.5 criterion vs iai（含本项目"已有 Instant 计时测试"的现状）

| 维度 | criterion | iai（原版） | iai-callgrind（继任者） |
|---|---|---|---|
| 总下载量 | 2.5 亿 | 401 万 | 133 万 |
| 近 90 天下载量 | 5 258 万 | 48.9 万 | 31.7 万 |
| 最新版本 / 时间 | 0.8.2 / 2026-02-04（活跃） | 0.1.1 / **2021-01-24（已停更）** | 0.16.1 / 2025-07-30（活跃） |
| 机制 | 统计驱动的耗时采样：置信区间、回归阈值（`--nthr`/change detection）、HTML 报告、`cargo bench` 无缝集成、支持 async | 基于 valgrind callgrind 的一次性指令计数 | callgrind 指令/缓存计数，确定性、对噪声免疫，需要系统安装 valgrind |

- 来源：https://crates.io/crates/criterion 、https://crates.io/crates/iai 、https://crates.io/crates/iai-callgrind

**结论与理由**：
- **原版 iai 已停更 5 年，不应再选**；其继任 iai-callgrind 依赖 valgrind，在 Android/CI 容器里安装成本高，且本项目基准对象多为"渲染/解码"这类含外部依赖的路径，指令计数口径收益有限。
- 本项目目前基准手段是 `integration-tests` 里的 `Instant` 计时测试（"已有 Instant 计时测试"）。`Instant` 计时适合**冒烟级回归阈值**（慢 2 倍必挂），但噪声大、无法做统计显著性判断。
- **建议：在 `native` 增加 `[[bench]]`（criterion 0.8）**，覆盖：VT 序列解析吞吐、网格滚动/重排、选区计算、脏区域合并。criterion 提供统计置信区间 + 变更检测，`cargo bench` 开箱即用、与 nextest 并行不冲突；保留 Instant 测试作为 CI 快速守卫（秒级），criterion 作为定期/手动基准（分钟级）。渲染帧率类基准仍留在 instrumented 层（真实 GPU）。

---

## 2. Kotlin/JVM 测试

### 2.1 Robolectric 对 Compose UI 测试的支持现状（核心问题）

**结论先行：`createComposeRule()` / `createAndroidComposeRule<Activity>()` 在 Robolectric 下可以运行，是 Google 官方支持路径；但像素级断言（`captureToImage`）与真实系统能力不可用。本项目的 androidTest Compose 测试**不能全部**迁移到 `src/test`，需要按"语义测试 vs 渲染/系统测试"分流。**

证据链：

1. **Robolectric 4.10（2023-04-11）官方发布说明**：新增 native Android graphics（`@GraphicsMode(NATIVE)`），"interactions with Android graphics classes use real native Android graphics code and are much higher fidelity"——即 JVM 上可跑真实图形代码路径，这是 Compose 测试能在 Robolectric 下工作的基础。来源：https://github.com/robolectric/robolectric/releases/tag/robolectric-4.10

2. **GitHub issue #8071（2023-03-19 开，2025-08 仍 open）**：作者用 `createComposeRule` + `setContent` 在 Robolectric 4.10-alpha 下运行成功，`onRoot().assertIsDisplayed()`、`printToLog` 等语义断言可用；**但 `onRoot().captureToImage()` 在 native graphics 下 2000ms 超时**（`ComposeTimeoutException`）。说明"节点树/语义断言"可用，"位图截取"不可用。来源：https://github.com/robolectric/robolectric/issues/8071

3. **GitHub issue #9738（2024-10）**：Compose 内嵌 `AndroidView(WebView)` + Espresso-Web 在 Robolectric 下不可测（非硬件加速、主线程判定、超时）。来源：https://github.com/robolectric/robolectric/issues/9738

4. **Robolectric 官方 "Best Practices & Limitations"**：明确列出 JVM 与真实 Android 的差异（libcore vs OpenJDK、系统服务行为差异等），并建议"相应测试改用真机跑"。来源：https://robolectric.org/best-practices/

5. **Android 官方 Compose 测试文档**：`ui-test-junit4` 同时是 instrumented 与本地（JVM）测试的标准依赖（`testImplementation` 与 `androidTestImplementation` 均可），`createComposeRule`/`createAndroidComposeRule` 是官方 API；官方样例 nowinandroid 即以 Robolectric 跑 Compose 语义测试。来源：https://developer.android.com/develop/ui/compose/testing

6. **本项目自身**：`android/app/build.gradle.kts` 已配置 `testImplementation` 的 robolectric 4.16.1、compose `ui-test-junit4`、`ui-test-manifest`、roborazzi 1.70.0，且 `src/test` 已有 Robolectric 先例（`DocumentsProviderTest`、`BootstrapInstallerTest`，`robolectric.properties` 固定 `sdk=33`）——基础设施已就绪，只缺 Compose 用例。

**Robolectric 下 Compose 测试的限制清单**：
- `captureToImage()`/像素截图超时（#8071）；Roborazzi 在 Robolectric 下做像素对比同样受此影响（其 Robolectric 模式基于 RNG 软件渲染，复杂 shader/字体光栅化结果与真机不一致）。
- 真实 `wgpu`/GPU 渲染路径不存在（Robolectric 无硬件加速），`Canvas` 类绘制走软件渲染。
- IME 输入法框架、剪贴板系统级行为、跨进程/跨应用交互（UiAutomator 场景）不可测。
- Compose 内嵌 WebView 等真实浏览器内核不可测（#9738）。
- 动画/帧时钟为虚拟时钟，`mainClock` 可手动推进——这是优势（确定性），但也意味着"真实帧时序"验证必须回 instrumented。
- `createAndroidComposeRule` 能启动真实 Activity（Robolectric 支持 ActivityScenario），但局限于单进程、单 Activity 生命周期模拟。

**迁移判断（对本项目 androidTest 现状）**：
- **可迁移（语义层）**：设置页/主题切换/搜索栏/Modifier 栏/会话抽屉的"存在性 + 点击 + 状态流转"断言（本项目 `TerminalUiTest` 中大量 `onNodeWithText/performClick/assertIsDisplayed` 类用例）；`mainClock` 驱动光标的 Compose 侧逻辑。
- **不可迁移（必须留 instrumented）**：真实 JNI 符号解析与 `.so` 加载、`SurfaceView`/GL 帧输出、渲染像素验证（Roborazzi emulator 模式）、真实 IME 注入、手势在真实渲染面上的行为、跨应用 SAF/通知交互、LeakCanary 集成、Cucumber BDD 中依赖真实终端的步骤。
- 本项目 `TerminalUiTest.kt`（11.5KB）与 `TouchGestureInstrumentedTest.kt`（14.8KB）等需逐用例分类；预计 androidTest 中约 30-40% 的 Compose 语义断言可迁至 `src/test`，**收益是 CI 上不再需要为这些用例启动模拟器**。

### 2.2 mockk（与 Robolectric 配合）

- 官方定位：Kotlin 首选 mocking 库，DSL（`mockk`/`every`/`verify`），支持协程（`coEvery`/`coVerify`）、object/static/constructor mock；JVM 单测用 `testImplementation("io.mockk:mockk")`，instrumented 用 `mockk-android`。来源：https://mockk.io/
- **适合本项目**：项目已用 mockk 1.14.x。与 Robolectric 的配合要点：**不要 mock Android 框架类**（Robolectric 官方最佳实践明确反对 mock Context/SharedPreferences 等，见 https://robolectric.org/best-practices/），应 mock 自己的边界（`TermuxSession`/`Bootstrap` 服务、`McpClient` 等）；ViewModel/仓库测试中用 mockk 替掉 JNI 桥与真实文件系统，跑在 Robolectric 的 `ApplicationProvider` 环境里。
- 已知坑（官方文档）：inline 函数不可 mock、JDK16+ 上 `mockkStatic`/spy 需开 `--add-opens`（Robolectric 4.16 默认 JVM 参数已处理大部分）；协程 suspend spy 行为异常，建议用 `coEvery` 而非 `spyk` 包 suspend。

### 2.3 kotlinx-coroutines-test（runTest / StandardTestDispatcher）

- 官方能力（1.11.0）：`runTest` 自动跳过 `delay`、虚拟时间推进、60s 兜底超时、子协程未捕获异常上抛；`StandardTestDispatcher`/`UnconfinedTestDispatcher`；`Dispatchers.setMain` 替换主线程调度器。来源：https://github.com/Kotlin/kotlinx.coroutines/blob/master/kotlinx-coroutines-test/README.md
- **适合本项目**：会话状态、搜索、设置持久化等 ViewModel/Flow 逻辑测试。约定：ViewModel 测试一律 `runTest` + `Dispatchers.setMain(StandardTestDispatcher())`；`StandardTestDispatcher` 需显式 `advanceUntilIdle`/`runCurrent`（严格、确定性），`UnconfinedTestDispatcher` 用于不关心调度顺序的快速用例。项目已依赖 1.11.0。

### 2.4 turbine（Flow 断言）

- 官方定位：Flow 测试专用（`awaitItem`/`awaitComplete`/`awaitError`/`ensureAllEventsConsumed`、`testIn` + `turbineScope` 多 Flow 并行），1.2.1。来源：https://github.com/cashapp/turbine
- **适合本项目**：与 coroutines-test 天然配套（turbine 依赖 `UnconfinedTestDispatcher` 语义，官方 README 注明与 `runTest` 配合使用）。用于会话输出流、搜索事件流、主题跟随系统流等断言；注意 turbine 与 mockk 结合时先 `coEvery` 桩好上游 Flow。
- 项目已依赖 turbine 1.2.1，直接开用。

---

## 3. 模拟器依赖减少

### 3.1 Gradle Managed Devices（GMD）

- 官方：`testOptions.managedDevices.localDevices` 配置 ATD 镜像（`aosp-atd`，API 30+），无头、精简系统、无需 Android Studio、Gradle 直接拉起；`managedDevices`（云设备）走 Firebase Test Lab（需 Firebase 项目与付费计费计划）。来源：https://developer.android.com/studio/test/gradle-managed-devices
- **对本项目**：CI（GitHub Actions）上用 `localDevices` + `aosp-atd` 跑"必须真机但不需要完整系统"的 instrumented 子集（渲染帧输出、手势注入、JNI 加载），比全量 Google APIs 镜像快、内存占用低；云设备在本项目场景无必要（见 3.3）。注意 ATD 镜像缺少部分系统应用，UiAutomator 跨应用用例需完整镜像——因此 GMD 应只承载"轻 instrumented"，重用例仍走本地/CI 模拟器。

### 3.2 Robolectric 与 instrumented 的分工边界

**必须留在真机/模拟器的清单**（Robolectric 明确不支持或行为不一致，证据见 2.1 节与 https://robolectric.org/best-practices/）：

1. **真实 JNI 符号解析 / native 库加载**：`libnative.so`（`jni` crate 绑定）、libghostty-vt-sys 的 Zig 产物加载与 `System.loadLibrary` 行为、`dlopen` 失败路径。
2. **渲染到屏幕**：wgpu Surface/GPU 管线、`SurfaceView`/`GLSurfaceView` 帧提交、帧率与 vsync、硬件加速位图合成。
3. **像素级视觉验证**：Roborazzi emulator 截图、`captureToImage`、字体光栅化差异（Robolectric 软件渲染与真机字形不一致）。
4. **真实 IME**：软键盘弹出/收起、`InputConnection` 双向编辑、候选栏、物理键盘映射。
5. **真实系统服务行为**：剪贴板跨应用、通知、SAF/`DocumentsProvider` 与系统文件选择器交互（注意：项目 `DocumentsProviderTest` 已证明 provider 逻辑本身可 Robolectric 化，但"系统 UI 调用 provider"仍是 instrumented）。
6. **跨应用交互**：分享面板、外部 intent 唤起、UiAutomator 驱动的系统 UI。
7. **生命周期真实化**：进程被杀重建、后台回收、多 Activity 栈。

**Robolectric 该接管的**：一切"框架 API 影子可用且断言的是逻辑"的测试——ContentProvider/Repository、Preferences/设置持久化、安装引导（BootstrapInstallerTest 已迁移）、Compose 语义交互（见 2.1）、ViewModel/Flow（见 2.3/2.4）。

### 3.3 Firebase Test Lab 是否需要

- 定位：云端物理/虚拟设备矩阵（多 OEM、多 API 版本、多语言），与 GMD `managedDevices` 集成；需要 Firebase 项目 + Google Cloud 计费（Blaze），按使用计费。来源：https://developer.android.com/studio/test/gradle-managed-devices 、https://firebase.google.com/pricing
- **结论：本项目（个人 fork）不需要**。理由：① 设备矩阵的价值在"兼容性回归"，个人 fork 的核心风险是功能正确性而非 OEM 差异，ATD + 本地模拟器已覆盖；② 每次运行产生云计费，个人维护成本不可控；③ 现有 androidTest 以功能/手势/渲染为主，无多设备矩阵诉求。**升级路径**：若未来发布正式版、需要 API 26→36 矩阵验证，再启用 GMD 云设备（配置成本低，`managedDevices` 段与 `localDevices` 同构）。

---

## 4. 同类终端项目测试分层（调研实证）

### 4.1 Termux（termux/termux-app）——Android 终端标杆

- 结构：`app/src/test`（极少，仅少量 JVM 用例）、`app/src/androidTest`（较大）、`termux-shared/src` **只有 androidTest + main，无 src/test**。
- CI：`.github/workflows/run_tests.yml` 仅 `./gradlew test`（JVM 单测），**androidTest 不进 CI**（需设备，靠开发者手工跑）。
- 来源：https://github.com/termux/termux-app （目录结构与 workflow 实测）
- **启示**：Termux 的 JVM 单测覆盖率极低、instrumented 不进 CI，属于"验证靠手工"的老派模式；torvox 不应模仿其测试投入，而应把 Termux 缺的自动化补齐。

### 4.2 Ghostty（ghostty-org/ghostty）——终端引擎测试标杆

- 结构（Zig）：
  - **内嵌 test 块**：每个源文件内 `test "..."`（Parser/Screen/Terminal 等核心模块），`zig build test` 全量跑；
  - **snapshot 测试体系** `src/terminal/snapshot/`：`screen.zig`(78KB)/`terminal.zig`(65KB)/`page.zig`(51KB)/`grid.zig`(30KB) 等重型测试文件 + `testdata/` 数据目录 + 自定义二进制快照格式（`.ksy` Kaitai 描述 + `verify-kaitai.py` 校验器）——对终端状态做**结构化快照对比**，而非像素对比；
  - **benchmark 目录** `src/benchmark/`：性能基准。
- 来源：https://github.com/ghostty-org/ghostty/tree/main/src/terminal/snapshot
- **启示**：终端模拟器最有价值的测试资产是"引擎状态快照"（VT 序列 → 网格/光标/滚动的确定性比对）。torvox 的 Rust 侧（native crate）应建立类似的结构化快照：`integration-tests/tests/` 里对 `libghostty-vt` 输出 + 自有网格状态做 golden 比对（本项目 `integration-tests` crate 已具备雏形，含 wgpu 场景测试）。

### 4.3 Haven（GlassHaven/Haven）——Android 终端（Compose 架构）

- 结构：多模块（`app` / `core` / `feature-*` / `termlib`(submodule) / **`integration-tests` 独立模块**），Compose UI + 自研 termlib；集成测试独立成 Gradle 模块承载。
- 来源：https://github.com/GlassHaven/Haven
- **启示**：与 torvox 的"native 引擎 + Compose 前端 + 独立 integration-tests"架构同构；集成测试独立模块是合理做法——torvox 已有 Rust `integration-tests` crate，可考虑 Android 侧是否也需要独立 `:integration-tests` Gradle 模块承接跨模块用例（当前 androidTest 承担此角色，够用则不必拆分）。

### 4.4 mightty（frixaco/mightty）——Rust + gpui 终端

- 结构：Rust workspace，`dev-dependencies` 仅 `gpui`（`test-support` feature）——即用 **gpui 自带场景测试框架**（`gpui::test` 驱动 UI 场景），无 proptest/quickcheck/nextest 自定义；终端核心直接复用上游 ghostty（submodule `src/ghostty`），测试资产也继承上游。
- 来源：https://github.com/frixaco/mightty （Cargo.toml 实测）
- **启示**：终端项目的"测试自研程度"与"自研核心代码量"成正比。torvox 自研了渲染器（wgpu + cosmic-text）与 JNI 桥，故需比 mightty 更重的自建测试（这正是本报告第 6 节金字塔的依据）。

---

## 5. 总结对照表（工具 × 项目现状 × 建议）

| 工具 | 本项目现状 | 建议 | 主要来源 |
|---|---|---|---|
| proptest | 已依赖 1.11 | **主力属性测试** | https://crates.io/crates/proptest |
| quickcheck | 已依赖 1.1 | 保留，新用例默认 proptest | https://crates.io/crates/quickcheck |
| loom | 未使用 | **不引入**（发版停滞、async 弱） | https://crates.io/crates/loom |
| shuttle | 已依赖 0.9 | **并发测试主力**（async 原生） | https://crates.io/crates/shuttle |
| cargo-nextest | 未使用 | **引入**（CI 并行 + JUnit + retry） | https://nexte.st/ |
| cargo-llvm-cov | 未使用 | **引入**（Rust 覆盖率自动化） | https://crates.io/crates/cargo-llvm-cov |
| criterion | 未使用 | **引入** `[[bench]]`（统计基准） | https://crates.io/crates/criterion |
| iai / iai-callgrind | 未使用 | **不引入**（原版停更；callgrind 依赖 valgrind） | https://crates.io/crates/iai-callgrind |
| Robolectric + Compose | 已配置 4.16.1 + ui-test-junit4 | **大举使用**（语义层迁移） | https://robolectric.org/ 、https://github.com/robolectric/robolectric/issues/8071 |
| mockk | 已依赖 1.14 | 继续（不 mock 框架类） | https://mockk.io/ |
| kotlinx-coroutines-test | 已依赖 1.11 | 继续（runTest + StandardTestDispatcher） | https://github.com/Kotlin/kotlinx.coroutines/blob/master/kotlinx-coroutines-test/README.md |
| turbine | 已依赖 1.2.1 | 继续（Flow 断言标配） | https://github.com/cashapp/turbine |
| GMD（localDevices/ATD） | 未配置 | **引入**（CI 轻量 instrumented） | https://developer.android.com/studio/test/gradle-managed-devices |
| Firebase Test Lab | 未使用 | **不引入**（个人 fork 性价比低） | https://firebase.google.com/pricing |

---

## 6. 五层测试金字塔建议

> 占比按"用例数量"估算，依据：本项目 androidTest 约 60+ 用例、Rust 侧现有测试基数、maestro 30 个 flow；目标是 CI 全量可自动跑（Rust 层 + Kotlin JVM 层 + Compose-Robolectric 层在无模拟器环境下完成，instrumented 层在 GMD/ATD 上完成，E2E 为发布前门槛）。

```
        ▲  E2E (maestro)  ~5%    ← 贵、慢、只保关键旅程
       ▲  Instrumented   ~10%    ← 真机/模拟器：渲染/JNI/IME/像素
      ▲  Compose-Robolectric ~15% ← JVM：Compose 语义交互（无像素）
     ▲  Kotlin JVM (Robolectric) ~25% ← JVM：仓库/会话/设置/Provider
    ▲  Rust 单测层（native + integration-tests）~45% ← 最快、最多
```

### 第 1 层：Rust 单测层（native + integration-tests）——占比约 45%

- **放什么**：VT 解析与网格状态机的 proptest 属性测试（滚动/换行/光标边界/选区矩形不变量）；libghostty-vt 输出与自有状态的 golden/结构化快照（仿 Ghostty snapshot 思路）；shuttle 并发测试（通道竞争、会话读写、渲染任务竞态）；doc 测试；`integration-tests` crate 中无 GPU 的场景测试（用 `test-util` feature + wgpu 软件后端）。
- **能证明什么**：终端核心逻辑正确性与并发安全——这是本项目的"心脏"，全部可在无 Android、无 GPU 的 CI 上跑。
- **工具**：proptest 1.11 / quickcheck（存量）、shuttle 0.9、cargo-nextest（并行执行）、cargo-llvm-cov（覆盖率）。

### 第 2 层：Kotlin JVM（Robolectric）层——占比约 25%

- **放什么**：`DocumentsProvider`（已迁移的 `DocumentsProviderTest` 范式）；`BootstrapInstaller`（`BootstrapInstallerTest` 已迁移）；设置持久化/Preferences；会话管理（SessionManager）；搜索索引逻辑；主题跟随系统的状态机；仓库层（mockk 替 JNI 桥与文件系统）。
- **能证明什么**：Android 框架 API 交互逻辑（ContentProvider 契约、SharedPreferences、生命周期回调）在 JVM 上的正确性，秒级反馈。
- **工具**：Robolectric 4.16（`@RunWith(RobolectricTestRunner::class)`，`robolectric.properties` 固定 sdk=33）、mockk、junit4。

### 第 3 层：Compose-Robolectric 层——占比约 15%

- **放什么**：从 androidTest 迁出的纯语义用例：设置页/主题切换/搜索栏/Modifier 栏/会话抽屉的"节点存在 + 点击 + 状态流转"；`mainClock` 虚拟时钟驱动的光标/闪烁逻辑；`createAndroidComposeRule` 启动真实 Activity 的轻量旅程（启动→首页→设置往返）。
- **能证明什么**：Compose UI 的结构与交互行为正确（语义树层面），不验证像素。
- **边界**：不写 `captureToImage`/像素断言（#8071 超时）；不写 WebView（#9738）；不写 GPU 内容（Robolectric 无硬件加速）。
- **工具**：`androidx.compose.ui:ui-test-junit4`（testImplementation）、`ui-test-manifest`、Robolectric 4.16、roborazzi 仅用于结构输出（`printToLog`），不用于像素。

### 第 4 层：Instrumented 层——占比约 10%

- **放什么**（对应 3.2 节清单）：真实 JNI/`.so` 加载与 JNI 桥往返；wgpu 真实渲染帧输出；Roborazzi emulator 截图像素验证；真实 IME 输入；手势在渲染面上的行为（`TouchGestureInstrumentedTest`）；Cucumber BDD 冒烟（`terminal-launch`/`terminal-commands` 等核心 feature）；LeakCanary 集成；跨应用 SAF/通知。
- **能证明什么**：真机上才存在的东西——符号解析、GPU 合成、输入法、系统服务协同。
- **运行方式**：CI 用 GMD `localDevices`（aosp-atd）跑轻量子集；重用例（UiAutomator、完整镜像）在发布前本地/CI 模拟器跑；本层不进"每次 push 全量"。
- **工具**：Espresso、UiAutomator、Roborazzi（emulator 模式）、Cucumber-Android、LeakCanary。

### 第 5 层：E2E（maestro）层——占比约 5%

- **放什么**：现有 `maestro/flows/` 30 个 flow 中的关键旅程收敛为发布门槛集：启动→会话→输入命令→搜索→设置→主题切换→会话持久化→选择/复制粘贴。
- **能证明什么**：真实设备上的端到端用户体验旅程完整可用（含系统键盘、剪贴板、真实渲染）。
- **运行方式**：发布前/每晚在真实模拟器或设备上跑（maestro 支持 GMD 设备）；不进每次 push。
- **工具**：maestro（https://docs.maestro.dev/）。

### 落地优先级（建议实施顺序）

1. **P0**：Rust 层接 cargo-nextest + cargo-llvm-cov；Compose-Robolectric 层先迁 5-10 个 androidTest 语义用例打通（`TerminalUiTest` 中与 GPU 无关部分）。
2. **P1**：shuttle 用例覆盖会话/通道并发；criterion bench 落地 VT 解析与网格重排；GMD `localDevices` 接入 CI 跑轻 instrumented。
3. **P2**：maestro 收敛发布门槛集；评估是否引入独立 Android `integration-tests` 模块（参照 Haven）。

---

## 附：调研来源汇总

- crates.io（各 crate 数据页，2026-08 实时）：proptest / quickcheck / loom / shuttle / cargo-nextest / cargo-llvm-cov / criterion / iai / iai-callgrind
- nextest 官方：https://nexte.st/
- Robolectric：官方文档 https://robolectric.org/ 、Best Practices & Limitations https://robolectric.org/best-practices/ 、4.10 Release https://github.com/robolectric/robolectric/releases/tag/robolectric-4.10 、Issue #8071 https://github.com/robolectric/robolectric/issues/8071 、Issue #9738 https://github.com/robolectric/robolectric/issues/9738
- Android 官方：Compose 测试 https://developer.android.com/develop/ui/compose/testing 、GMD https://developer.android.com/studio/test/gradle-managed-devices 、Firebase 定价 https://firebase.google.com/pricing
- mockk：https://mockk.io/ ；kotlinx-coroutines-test：https://github.com/Kotlin/kotlinx.coroutines/blob/master/kotlinx-coroutines-test/README.md ；turbine：https://github.com/cashapp/turbine
- 同类项目：Termux https://github.com/termux/termux-app （.github/workflows/run_tests.yml 与源码树实测）、Ghostty https://github.com/ghostty-org/ghostty （src/terminal/snapshot/ 实测）、Haven https://github.com/GlassHaven/Haven （模块结构实测）、mightty https://github.com/frixaco/mightty （Cargo.toml 实测）
