## MODIFIED Requirements

### Requirement: 禁改区待维护者裁定项台账（只登记，不实施）

`docs/specification/TESTING.md` 要求而现行门禁缺席的四项，全部落在 AGENTS.md
禁改区（`scripts/`、`.github/`、`android/app/build.gradle.kts`）。维护者已裁定
「都不改，只登记」，落地方案记录于此，不擅自实施：本表各项 MUST 维持登记状态，
未经维护者明确同意 MUST NOT 实施。

登记 MUST NOT 只有落地方案而无事实基线：日后获授权实施的人 MUST NOT 需要重新调研
「改之前是什么样」。故每项 MUST 同时登记**实测基线**与**实施时的验收判据**，
使门禁的判定以基线为准而非以「无输出」为准。

#### Scenario: 实测基线与验收判据

- **WHEN** 记录规范要求但尚未落地的门禁项
- **THEN** MUST 给出规范依据、落地方案与当次实测基线，不得以计划代替实测

| 项 | 规范依据 | 落地方案（未实施） |
| --- | --- | --- |
| aislop / jscpd 代码检查门禁 | TESTING.md 环境节 | `scripts/check.nu` 与 CI 增 `npx aislop@latest scan` 与 `npx jscpd@latest --min-lines 8 --min-tokens 60` 两步 |
| rapidocr OCR 自动化 | TESTING.md 环境节 + 外部基准优先级 4 | `scripts/` 增 OCR 判定脚本 + `build.yml` 模拟器步骤调用 |
| CI 产物哈希比对 | vercel-labs-wterm 研究结论 | `build.yml` 增产物 sha256 记录步骤 |
| Android 15+ 16KB 页 `.so` 对齐 | GlobalTechInfo-RedTerm 研究结论 | `android/app/build.gradle.kts` 按 AGP 16KB 对齐指南调整打包 |

实测基线（本轮实跑所得，工具版本可能前进，重跑 MUST 以当日实测覆盖本列）：

- **aislop**（须以 `npx aislop@latest` 调用，本仓 nix 环境未内置该可执行文件）：
  `npx aislop@latest scan --json native/src` 退出码 0，`score` 100、label `Healthy`、
  `coverage.supportedFiles` 113、`unsupportedFiles` 0，`summary` 0 errors /
  1 warning / 0 fixable；唯一 `diagnostics` 为 `complexity/file-too-large`
  （`terminal/ghostty_terminal/internal/input_filter.rs`，1146 行），
  `findingAssessment.byKind` 为 `style-policy` 1 / `confirmed-defect` 0。
  全仓扫描 `npx aislop@latest scan --json` 同为 0 errors / 1 warning，
  `coverage.supportedFiles` 114、`unsupportedFiles` 201、
  `dominantUnsupported` 为 `Kotlin`、`scoreable` 为真。
  **覆盖面限制**：该工具扫不到本仓 Android 侧 Kotlin 代码，
  故本项 MUST NOT 被当作全仓门禁；它只对 Rust 侧有判别力。
  **评分可用性**：该工具仅在 `unsupportedFiles < 10` 或
  `unsupportedFiles ≤ supportedFiles × 3` 时给出分数；本仓当前 201 ≤ 114 × 3，
  全仓扫描亦可评分，但分数基线仍 MUST 取 Rust 子目录扫描。
  **与上一列基线的偏差来源**：上一列记的唯一 `file-too-large` 命中在
  `terminal/ghostty_terminal/public_api.rs`（1123 行），该文件已按职责拆成
  `terminal/ghostty_terminal/public_api/` 目录模块，命中位置随之迁移到
  `internal/input_filter.rs`；命中条数仍为 1（`function-too-long` 仍为 0），
  未上升，故不触发「条数 MUST NOT 上升」的验收判据。`supportedFiles` 由 105 升至 113
  是新增 Rust 源文件所致。
  验收判据：接入后 `confirmed-defect` 与 `errors` MUST 维持 0，
  `file-too-large` / `function-too-long` 的条数 MUST NOT 上升（基线 1 / 0）；
  该工具归类为 style-policy 的项 MUST NOT 阻断构建，
  只有 error 级或 `confirmed-defect` 才阻断。
- **jscpd**：`jscpd --min-lines 8 --min-tokens 60` 对 `native/src` 与
  `android/app/src/main` 实测退出码 0，2 处克隆 / 23 行重复 / 0.03%，涉 256 个文件
  （2026-11-14 重测：克隆数、重复行数与重复率与上一列基线同值，文件数随新增源文件而变）。
  测量取自已提交的树快照，不取工作区状态（工作区可能同时存在他人进行中的改动）。
  前一列基线为 2 处克隆 / 26 行重复 / 0.04% / 115 个文件：其第二处
  `native/src/terminal/ghostty_terminal/internal/mod.rs` 内 `process_query` 与
  `drain_queries` 的同形十参形参表已随该模块重组消失；基线记录之后新增的代码引入
  了另外三处克隆（JNI 导出的注册表查找骨架、会话测试夹具、Kitty 放置撤除用例的
  送图与清屏流程），已分别收敛为 `with_session_entry`、`mock_pty::mock_session` 与
  `mock_pty::mock_arc_session`、`ed2_clears_direct_kitty_placement`。
  现存两处残留逐条判定如下：
  ① `android/app/src/main/res/drawable/launcher_monochrome.xml` 与 `logo_art.xml`
  的同形 `pathData`（14 行），不修的具体理由是 Android drawable 不支持跨文件
  引用几何，单色版必须自带路径。
  ② `android/app/src/main/java/terminal/emulator/ui/SettingsAppearanceSection.kt` 与
  `SettingsLogLevelSection.kt` 的同形包声明与 import 段（9 行），不修的具体理由是
  同包同层级的两个界面文件各自按需 import 同一组 Compose API，合并即产生
  只为转发符号的中间层（违 `STYLE.md` 的一次性抽象禁令），属天然相似。
  验收判据：重复率 MUST NOT 高于该基线；新增克隆 MUST 逐处给出
  「不修」的具体理由（被测行为不同、脚手架样板、或传状态必需的形参列表），
  MUST NOT 以「先记着」放行。
- **rapidocr**：`flake.nix` 引导调用 `scripts/download-rapidocr-models.nu` 把模型
  下载到 `/tmp/.rapidocr-models`，模型环境已就绪；该下载与其他 shellHook 初始化步骤
  （AOSP testkey、`/tmp/alacritty-theme` 克隆）共享同一把排他锁，多个 shell 并发进入
  时 MUST 串行执行，MUST NOT 并发写同一 `/tmp` 路径；但
  `scripts/test-emulator.nu` 跑 `connectedDebugAndroidTest`、
  `connectedBenchmarkReleaseAndroidTest` 与 `connectedBenchmarkNixAndroidTest`，
  仓内无任何 OCR 消费方，即
  `TESTING.md` 的「拉截图 → 跑 rapidocr → 断言」闭环仍只存在于人工步骤。
  验收判据：实施后 MUST 有程序化断言比对识别文本，MUST NOT 只落截图或只打印识别结果。
- **CI 产物哈希比对** 与 **16KB 页对齐**：本轮未实测（MUST NOT 以推定数字入表）。
  前者实施判据以产物 `sha256` 记录步骤存在且随每次构建产出为准；
  后者须以 AGP 官方指南与实际打包产物校验为准，MUST NOT 以本仓既有配置反推。

`code-health` 要求的审计留档（静态形状扫描与依赖活跃度）同记在本节，事实与判定如下。

**静态形状扫描**（`footgun-scan` 的 32 个模式，覆盖 Kotlin/Compose 源码；Rust 侧由
`cargo clippy --all-targets -- --deny warnings` 与 `aislop` 覆盖，本工具不适用）：

- 参数：`python3 <footgun-scan>/scripts/scan.py --all <目录>`，逐行扫描受版本控制的文件；
  目录取 `android/app/src/main`、`android/app/src/test` 与 `android/app/src/androidTest`。
- 当日实测（2026-11-14）：主源码集 117 个文件 / 9 处命中，单测源码集 134 个文件 /
  1 处命中，仪器测试源码集 87 个文件 / 0 处命中；全仓 363 个文件 / 10 处命中，全部为
  `look` 级形状提示，无 `likely` 级命中。

| 位置 | 命中形状 | 判定 | 依据 |
| --- | --- | --- | --- |
| `FontManager.kt` 的 `refreshFontList` | `job-relaunched-without-cancel` | 驳回 | 陷阱指认的形态是「赋值前不取消仍在跑的旧作业」。此处的启动点在同一条语句序列上带并发闩 `fontListLoaded \|\| fontLoadJob?.isActive == true`：旧作业仍活跃时根本不启动第二个；`fontListLoaded` 是 `@Volatile`，跨 IO 线程与主线程可见；字段只有一个赋值点，唯一调用方在主线程（设置页打开字体选择框那一刻）。旧作业已结束时无「仍在跑的旧作业」可取消 |
| `NativeBridge.kt` 的类初始化块（`System.loadLibrary("native")`） | `native-load` | 驳回 | 陷阱讲的是 JNA：同名库可能被系统副本抢先加载，故须打印实际打开的路径。本仓走 Android 的 JNI `System.loadLibrary`，装载路径由应用自身的 native 库目录决定，不存在同名系统副本兜底装载；失败路径把平台链接器给出的 `UnsatisfiedLinkError` 消息全文记入日志；构建侧保证 `.so` 打进 APK 的 `jniLibs`（`BUILD.md` 明确不设计运行时兜底） |
| `NativeBridgeSmokeTest.kt` 的 `loadNativeLibrary` | `native-load` | 驳回 | 走 `System.load(绝对路径)`：路径是常量（宿主编译产物），不经名字解析，缺失时 `System.load` 自身抛错，不存在「同名副本被抢先加载」的场景 |
| `BootstrapInstallService.kt` 的 `install` | `run-blocking` | 驳回 | `runBlocking` 在 `install()` 内，而 `install()` 由 `onStartCommand` 里显式 `Thread { … }.start()` 调用，全程在后台线程，不在主线程 |
| `RenderSupervisor.kt` 的 `startRenderThread` | `run-blocking` | 驳回 | `runBlocking` 在渲染线程的 `Thread { … }` 体（专用渲染线程）内，不在主线程 |
| `RenderWatchDog.kt` 的 `start` | `job-relaunched-without-cancel` | 驳回 | 「检查是否活跃」与「启动」在同一个 `synchronized(fireLock)` 临界区，两个并发调用者不会各跑一个 2s 轮询循环；`stop()` 推进代次并取消后清空字段；代次单调递增使陈旧循环无权回调 |
| `ModifierBar.kt` 的按键手势 `pointerInput(Unit)` | `pointer-input-unit` | 驳回 | 陷阱指认的是「以 Unit 为键时块内捕获的普通值不再更新」。此处块内读到的可变值全部经 `rememberUpdatedState`（按键几何、手势区、点击、重复、次要动作），没有裸捕获；以 Unit 为键是刻意选择：改用 lambda 实例作键会在长按期间的任何重组上取消并重启手势协程，静默杀死自动重复 |
| `SettingsAppearanceSection.kt` 的 `FontSizeSlider` | `slider-value-range` | 驳回 | 传入滑块的值是 `sliderFontSize`，初值经 `SettingsRepository.clampFontSize` 收敛，区间收紧时由 `LaunchedEffect(fontSizeUpperBound)` 再次收敛；区间上界与收敛入口同源（`SettingsRepository.fontSizeMaxSp`），故值恒在区间内 |
| `SettingsComponents.kt` 的 `SettingsSliderRow` | `slider-value-range` | 驳回 | 通用滑块行只透传调用方给的区间与值，唯一调用方即上条，值已在区间内 |
| `SettingsScreen.kt` 的设置页遮罩层 `pointerInput(Unit)` | `pointer-input-unit` | 驳回 | 块体是 `detectTapGestures(onTap = {})`，不读任何状态，无可失效的捕获值 |

**依赖活跃度**（直接依赖的下载量与最近发布）：

- crates.io 侧（口径同 `code-health`：累计下载量 ≥ 100 万、近 90 天下载量 ≥ 10 万、
  最近发布在 12 个月内；事实取 crates.io crate 元数据接口的 `downloads`、
  `recent_downloads` 与最新非撤回版本的 `created_at`），采样于 2026-11-14：

| crate | 累计下载 | 近 90 天 | 最近发布 | 判定 |
| --- | --- | --- | --- | --- |
| base64 | 1,966,914,514 | 525,319,336 | 2026-08-04 | 保留 |
| bytemuck | 475,531,146 | 142,627,622 | 2026-07-19 | 保留 |
| cosmic-text | 12,311,205 | 5,040,766 | 2026-04-22 | 保留（上游阻断：需 fontdb 0.23） |
| flume | 292,039,169 | 81,786,289 | 2025-12-08 | 保留 |
| foldhash | 826,077,404 | 340,488,435 | 2025-08-23 | 保留 |
| fontdb | 46,003,933 | 17,173,480 | 2026-07-29 | 保留（与 cosmic-text 同版本要求） |
| pollster | 48,575,613 | 21,836,244 | 2026-07-10 | 保留 |
| etagere | 10,167,806 | 4,450,841 | 2026-03-18 | 保留 |
| jni | 246,541,292 | 85,150,843 | 2026-03-16 | 保留 |
| libc | 1,976,289,422 | 486,586,000 | 2026-10-16 | 保留 |
| log | 1,531,065,096 | 392,713,342 | 2026-10-30 | 保留 |
| lru | 442,790,173 | 131,595,588 | 2026-10-18 | 保留 |
| nix | 978,529,563 | 250,044,617 | 2026-05-11 | 保留 |
| parking_lot | 1,276,525,387 | 283,846,814 | 2026-10-21 | 保留 |
| raw-window-handle | 135,190,490 | 44,606,428 | 2024-05-17 | 保留（生态基线类型，见下） |
| roxmltree | 105,796,268 | 40,792,343 | 2026-10-24 | 保留 |
| serde | 1,705,161,054 | 430,394,306 | 2026-07-18 | 保留 |
| serde_json | 1,611,830,375 | 432,551,167 | 2026-10-11 | 保留 |
| swash | 18,893,111 | 7,889,958 | 2026-07-17 | 保留 |
| thiserror | 1,849,817,042 | 528,582,996 | 2026-09-23 | 保留 |
| toml | 1,138,979,644 | 317,041,580 | 2026-11-06 | 保留 |
| unicode-script | 63,172,017 | 23,214,424 | 2026-11-02 | 保留 |
| wgpu | 50,340,268 | 18,915,738 | 2026-11-12 | 保留 |
| wgpu-types | 52,488,031 | 19,846,342 | 2026-11-12 | 保留 |
| criterion | 339,537,495 | 80,151,633 | 2026-02-04 | 保留（dev） |
| proptest | 245,654,555 | 76,495,907 | 2026-10-10 | 保留（dev） |
| shuttle | 7,517,082 | 2,498,430 | 2026-11-05 | 保留（dev） |
| tempfile | 995,301,049 | 254,563,200 | 2026-03-11 | 保留（dev） |

- 结论：28 个 crates.io 直接依赖全部通过门槛（最低者为 `shuttle` 累计 751 万、近 90 天
  249 万）。唯一的豁免是 `raw-window-handle`：最近发布停在 2024-05-17，它只出现在类型
  边界上（0.6.x 是 wgpu/窗口句柄的事实标准），按「生态基线类型」豁免「最近发布」一项。
- 上游 git 依赖 `libghostty-vt`（`Uzaaft/libghostty-rs`，钉 rev）不在 crates.io，按本规范
  首条 Requirement 的「跟踪上游 master rev」口径核对。
- Maven 侧（Google Maven 与 Maven Central）：两仓库的公开接口只发布版本与时间戳，不发布
  下载量（`repo1.maven.org` 与 `dl.google.com` 的 `maven-metadata.xml` 只含 `lastUpdated`；
  `search.maven.org` 的 solr 接口在本环境不可达，Central Portal 站点受 Cloudflare 拦截），
  故采信口径的「累计下载量」「近 90 天下载量」两项对 Maven 侧不可测，MUST NOT 以估算或
  记忆补写。Maven 侧以「最近发布」核对，事实取各构件 POM 响应头的 `Last-Modified`：
  47 个直接依赖中 40 个落在 12 个月内；落在 12 个月之外的 7 个按「生态基线类型」豁免
  「最近发布」一项，理由如下：

| 构件 | 最近发布 | 豁免理由 |
| --- | --- | --- |
| `junit:junit:4.13.2` | 2021-02-13 | 生态基线类型：JUnit 4 是事实标准，仓内只用它的断言与运行器接口 |
| `androidx.profileinstaller:profileinstaller:1.4.1` | 2024-10-02 | 生态基线类型：Android Gradle 插件配套的安装期产物，不承担运行期逻辑 |
| `androidx.compose.material:material-icons-extended:1.7.8` | 2025-02-12 | 生态基线类型：图标资源包，版本由 Compose BOM 统一管理，不由本仓直接选型 |
| `androidx.concurrent:concurrent-futures:1.3.0` | 2025-07-16 | 生态基线类型：为对齐 AGP 传递依赖而由 Gradle 强制固定的版本 |
| `app.cash.turbine:turbine:1.2.1` | 2025-06-11 | 生态基线类型：仅测试用的流断言夹具 |
| `com.slack.lint:slack-lint-checks:0.11.1` | 2025-10-10 | 生态基线类型：仅 lint 工具链 |
| `androidx.core:core-splashscreen:1.2.0` | 2025-11-05 | 生态基线类型：启动屏 API 的稳定版，接口面已冻结 |

#### Scenario: 禁改区项不被静默实施

- **WHEN** 本轮或后续复查接触到上述四项
- **THEN** MUST 维持登记状态并按维护者裁定执行，MUST NOT 以「规范要求」为由擅改
      禁改区文件

#### Scenario: 门禁的覆盖面限制被如实记录

- **WHEN** 引用 aislop 的结果作为本仓代码质量结论
- **THEN** MUST 同时写明它不覆盖本仓 Kotlin 代码（`dominantUnsupported` 为
      `Kotlin`），MUST NOT 据其 0 error 声称全仓无问题
- **AND** 全仓扫描因覆盖面闸门而 `score` 为 `null`、label 为 `not scored` 时，
      MUST 写明该事实并以 Rust 子目录扫描的分数作基线，
      MUST NOT 把未评分当作满分或据「not scored」断言质量已达标

#### Scenario: 接入门禁时以基线而非无输出判合格

- **WHEN** 日后获授权把 aislop 或 jscpd 接入 `scripts/check.nu` 或 CI
- **THEN** 判定 MUST 以上表的实测基线为阈值（重复率不升、`confirmed-defect` 维持 0），
      MUST NOT 以「命令无输出」或「退出码 0」当作已通过
