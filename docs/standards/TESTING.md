# Testing Guide

## Principles

- Tests are specs — no test means no spec
- Only test public API
- One test equals one behavior
- No flaky tests — use deterministic synchronization
- Every test must assert concrete behavior — "does not crash" is not a test; use fixed seeds for any random generation

## Rust Tests

```bash
cargo test --workspace                                # all Rust tests
cargo test -p native                                   # native crate only
cargo test -p native prop_tests                        # property/concurrency tests
```

> 注：`cargo-nextest` 经评估**不引入**（`docs/rejected-technologies.md` §1.8 R 表与
> `docs/dependencies.md`：`check-rust.nu` 依赖 `cargo test` 的
> `--test-threads 1` 串行语义，bench 断言基准模型不匹配 nextest 并行隔离）。

## Test File Locations

All Rust code lives in the single `native` crate, with tests inside each module:

| Module | Tests (in-source) | Integration Tests |
|--------|-------------------|-------------------|
| `native/src/terminal/ghostty_terminal/` | `tests.rs` — grid ops, cell iterator, CellData, snapshot, config | — |
| `native/src/render/` | `tests.rs` — GPU headless, font, shader validation, OCR, screenshot | — |
| `native/src/android/` | (tests guarded by `#[cfg(target_os = "android")]`) | — |
| `native/src/mcp/` | Inline tests — tool listing, tool calls | — |
| `exec-bin` | — | `tests/basic.rs` |

### Property and Concurrency Testing

Property-based testing uses **proptest 1.11**（主力）for state-machine invariants
（`native/src/prop_tests.rs`：OSC 52 roundtrip、任意 escape 流不 panic、网格/滚动不变式、
UTF-8 边界切割），并发调度测试用 **shuttle 0.9**（`shuttle::check_random`：事件队列并发
push/pop、锁序）。工具选型依据见 `docs/rejected-technologies.md` §1.8（loom 不引入——
shuttle 已选定、async 原生；quickcheck 已移除——proptest 覆盖属性测试）。

Benchmarks 用 **criterion 0.8**（`native/benches/cell_builder.rs`、`vt_typing.rs`，
`cargo bench -p native`），统计置信区间 + `--failure-threshold` 回归检测；与测试内的
`Instant` 硬阈值断言基准互补（后者跑在 `cargo test` 中作 CI 快速守卫）。

## Android Tests

```bash
cd android && ./gradlew testDebugUnitTest            # unit tests
cd android && ./gradlew connectedDebugAndroidTest     # instrumented
```

`testDebugUnitTest` includes `NativeBridgeSmokeTest` (JNI round-trip, see
row 1b above). It needs a host-built `libnative.so` — produce one with
`cargo build --package native` (repo-root `target/{debug,release}/libnative.so`).
When absent the test is skipped, never failed. CI builds it via
`test-gradle.nu` before invoking Gradle.

### Six test types and where each lives

The test suite verifies Android behavior with six distinct test types. Use the
right type for the behavior under test — do not collapse them into one.

| # | Type | Location | What it covers |
|---|------|----------|----------------|
| 1 | **Unit** (Rust) | `native/src/terminal/`, `native/src/render/`, `native/src/mcp/` | Pure logic: VT parse, grid/scrollback, OSC, keyboard encode, MCP. Runs on host via `cargo nextest`. |
| 1b | **JNI round-trip** (JVM) | `android/app/src/test/java/terminal/emulator/bridge/NativeBridgeSmokeTest.kt` | Loads the host-built `libnative.so` in the test JVM and drives the real bridge (initSession → feedTerminal → getTitle/getTerminalText → destroySession). Covers the JNI boundary layer pure-Rust tests cannot: JString/jbyteArray conversion, `env.throw_new` paths, and the Kotlin→JNI→VT-thread→ghostty-parser→query round-trip — without an emulator. |
| 2 | **Compose UI** (instrumented) | `android/app/src/androidTest/java/terminal/emulator/ui/*ComposeTest.kt` (e.g. `TerminalScreenComposeTest`) | Compose widget state/interaction on-device. |
| 3 | **OCR screenshot** (emulator) | `native/src/render/tests.rs` + `scripts/test-emulator.nu` (rapidocr) | End-to-end terminal-text visibility on the emulator. |
| 4 | **Maestro** | `android/app/src/androidTest/java/terminal/emulator/ui/*.yaml` flow files (e.g. `SelectionMaestroTest.yaml`) | End-to-end on-device flows driven by Maestro YAML. |
| 5 | **Android UI testing framework** | `android/app/src/androidTest/java/terminal/emulator/ui/*UiAutomatorTest.kt` (e.g. `TerminalUiAutomatorTest`, `SelectionUiAutomatorTest`, `TextSearchUiAutomatorTest`) | Cross-app / system-level interaction via UiAutomator. |
| 6 | **Espresso** | `android/app/src/androidTest/java/terminal/emulator/ui/*EspressoTest.kt` (e.g. `TerminalActivityEspressoTest`, `SelectionEspressoTest`, `TextSearchEspressoTest`) | In-app View-level interaction via Espresso. |

### Screenshot / OCR verification

Pixel-exact Compose goldens (Roborazzi) are intentionally NOT used: FR-055
bans committed PNG artifacts. Terminal-text visibility is verified
end-to-end on the emulator: `scripts/test-emulator.nu` captures screenshots
and OCR-verifies the text with `rapidocr` (available in the dev shell).

### RapidOCR Text Verification

RapidOCR (via `rapidocr`) is available in the dev shell for OCR-verifying screenshots on Linux.

Used by `native/src/render/tests.rs` to verify font rendering end-to-end: renders text with swash, saves PNG, OCR-verifies the output.

## Emulator Tests

```bash
nu scripts/test-emulator.nu                         # automated emulator tests
```

Prerequisites (verified on the CI image, API 35 x86_64 emulator):

1. **Native libs must be built first** — `scripts/build-android-libs.nu` copies
   `libnative.so` into `android/app/src/main/jniLibs/`. Without it the APK
   loads no JNI (`dlopen failed: library "libnative.so" not found`) and every
   instrumented test that touches the bridge silently degrades (compose-idle
   tests "pass" vacuously because no render loop exists). Build at least the
   ABI matching the emulator. **Always use the release profile**: a debug
   (dev) `.so` is ~122MB unoptimized code that stalls the emulator loader and
   crashes the device (measured regression) — `build-android-libs.nu` now
   fails the build when a deployed `.so` exceeds `MAXIMUM_SO_SIZE_BYTES`:
   ```bash
   nu scripts/build-android-libs.nu --profile release x86_64
   ```
2. **GPU mode**: `-gpu swiftshader_indirect` crashes the emulator on wgpu
   SPIR-V it cannot parse (`Invalid source language operand: 10`, SIGSEGV
   when the app starts the renderer). Use `-gpu angle_indirect` instead
   (verified stable: app boots, native render loop runs).
3. **Compose idle vs render loop**: with the real native render loop running,
   software-rendered emulators cannot satisfy `waitForIdle` — compose tests
   that wait for global idle time out. Such tests are `@Ignore`d with an
   explicit reason (needs a hardware-accelerated device). They are *not*
   vacuous: they assert real search results / selection handles; the
   `@Ignore` comment states the exact blocker.
4. **POST_NOTIFICATIONS**: `MainActivity` requests it on Android 13+ at
   startup; the system dialog covers the UI and breaks every compose-test
   node lookup. The test APK declares the permission
   (`src/androidTest/AndroidManifest.xml`) and each Activity-launching test
   class applies `GrantPermissionRule` — keep that rule when adding new UI
   tests.

### Instrumented 冻结与质量审计（2026-08）

实测基线：androidTest 共 **338 个 `@Test`**，其中 **27 处 `@org.junit.Ignore`**
冻结（~8%，非 15%——冻结方法多数为多断言大方法，故用例占比高于方法占比）。
冻结的**唯一根因**已在每条 reason 中写明：

> native data path 已接线，但持续渲染循环使软件渲染模拟器无法满足 Compose
> `waitForIdle` → 需要硬件加速设备。

不是功能缺失——被冻结的搜索/选择/剪贴板断言在 Rust 侧有**等价双轨覆盖**
（`search_in_scrollback`/`search_highlight_*`、`selection_range_*`、
`gpu_render_selection_swaps_fg_bg`、`osc52_roundtrip`）。解除冻结的前提是 CI
改用硬件加速设备（KVM/GPU 直通），当前 SwiftShader 环境**不应**解除。

冻结分布（同因同批）：
- `selection/SelectionRoborazziEmulatorTest.kt` 9 处（截图内无选区高亮）
- `ui/TextSearchInstrumentedTest.kt` 4、`TextSearchImeSmartCaseTest.kt` 1（搜索结果 UI）
- `ui/SelectionVisualVerificationTest.kt` 类级、`ui/TextSearchColorVerificationTest.kt`
  类级、`gpu/CursorBehaviorInstrumentedTest.kt` 类级、`gpu/CursorBlinkFrameTest.kt` 类级
- `SelectionEspressoTest.kt` / `SelectionUiAutomatorTest.kt` / `BehaviorInstrumentedTest.kt` 各 1
- BDD：`terminal-search.feature` 3 场景 + `terminal-selection.feature` 4 场景
  标 `@wip`（含"复制到剪贴板"场景），由 `CucumberOptionsClass` 的
  `tags = "not @wip"` 排除——与上述 @Ignore 同因。

固定 `Thread.sleep` 等待分布（flaky 风险源）：`BehaviorInstrumentedTest` 13、
`SelectionRoborazziEmulatorTest` 21、`FontSwitchInstrumentedTest` 12、
`VisualInlineVerificationTest` 9、`TestUtils.kt` 7、`InlineScreenshotVerificationTest` 7、
`SettingsInstrumentedTest` 5、`TextSearchColorVerificationTest` 5，其余 9 个文件各 1-3。
改进方向：换 `waitUntil` 条件轮询（`TestUtils.waitForSession` 已是范例）；
swiftshader 1.8fps 下 sleep 语义不可靠，轮询需以渲染帧数（截图前后对比）而非墙钟为准。

纯动作/弱断言测试（保留但标注）：`ModifierBarTest` 4 个 `*_clickable`、部分 Roborazzi
截图测试（`captureRoboImage` 即 golden 断言，不是弱断言）、墓碑式 `assertNotNull(activity)`
测试（`TerminalUiTest` 等）——**无崩溃 smoke 价值**，但不承担行为证明；
新增断言时优先验证状态流转（如 `ModifierBarTest.ctrl_toggle_cycles` 的 selected 语义）。

---

## Traceability

### Requirement-to-Test Mapping

Every functional requirement (FR-xxx) and non-functional requirement (NFR-xxx) in
`docs/requirements/`（StrictDoc .sdoc） must be traceable to at least one test. The traceability matrix is
maintained in `docs/traceability.yml`.

### Verification Methods

| Method | Description | CI Command |
|--------|-------------|------------|
| **unit** | Rust unit/integration test | `cargo test --workspace` |
| **doctest** | Rust doc-test (executable examples in `///` comments) | `cargo test --doc` |
| **property** | Property-based test (proptest, `native/src/prop_tests.rs`) | `cargo test -p native prop_tests` |
| **fuzz** | Fuzz target | (not used — VT parsing inherited from libghostty-vt upstream; no fuzz target) |
| **lint** | Lint/static analysis check | `cargo clippy --all -- --deny warnings` |
| **android-unit** | Android unit test (Robolectric) | `./gradlew testDebugUnitTest` |
| **ocr-screenshot** | Emulator screenshot + OCR | `nu scripts/test-emulator.nu` (rapidocr) |
| **instrumented** | Android instrumented test | `./gradlew connectedDebugAndroidTest` |
| **maestro** | Maestro E2E flow | `maestro test <flow.yaml>` |
| **ui-automator** | UiAutomator cross-app test | Via instrumented test suite |
| **espresso** | Espresso in-app interaction test | Via instrumented test suite |
| **emulator** | Full emulator E2E test | `nu scripts/test-emulator.nu` |
| **tool-lint** | External tool quality check | `cargo test -p integration-tests --test tool_lint` |
| **docs-validate** | Documentation structural validation | `cargo test -p integration-tests --test tool_lint -- docs_*` |

### Adding Tests for New Requirements

When adding a new requirement to `docs/requirements/`（StrictDoc .sdoc）:

1. Determine which verification method(s) apply
2. Add or update test(s) in the appropriate test directory
3. Update `docs/traceability.yml` with the new requirement-to-test mapping
4. Run the relevant test command and confirm it passes

### SRS ID Checks

The following structural checks ensure traceability integrity:

- Every `FR-\d{3}` / `NFR-\d{3}` in `docs/requirements/`（StrictDoc .sdoc） follows the format
- Every referenced requirement in `docs/traceability.yml` exists in `docs/requirements/`（StrictDoc .sdoc）
- Every acceptance criterion in `docs/acceptance.md` references a valid requirement ID
- Every ADR in `docs/adr/` references at least one requirement ID

These checks run as part of `tool_lint.rs` (see `cargo test -p integration-tests --test tool_lint`).

---

## Test Pyramid & Coverage Snapshot

（吸收自 `docs/test-strategy-research.md` 与 `docs/test-coverage-audit.md`，
原文已删除；未落地项的登记见 `docs/rejected-technologies.md` §2。）

### 五层测试金字塔（用例数量占比）

```text
        ▲  E2E (maestro)  ~5%    ← 贵、慢、只保关键旅程（发布前门槛）
       ▲  Instrumented   ~10%    ← 真机/模拟器：渲染/JNI/IME/像素/跨应用
      ▲  Compose-Robolectric ~15% ← JVM：Compose 语义交互（无像素断言）
     ▲  Kotlin JVM (Robolectric) ~25% ← JVM：仓库/会话/设置/Provider
    ▲  Rust 单测层（native + integration-tests）~45% ← 最快、最多、无头
```

| 层 | 放什么 | 工具 |
|----|--------|------|
| Rust 单测（~45%） | VT 解析/OSC/网格状态机 proptest、shuttle 并发、doc 测试、Lavapipe 渲染 + OCR（`native/src/render/tests.rs`） | proptest / shuttle / criterion bench 互补 |
| Kotlin JVM（~25%） | DocumentsProvider、BootstrapInstaller、设置持久化、搜索索引、ViewModel/Flow | Robolectric 4.16 + mockk + turbine + kotlinx-coroutines-test |
| Compose-Robolectric（~15%） | Compose 语义交互（节点存在/点击/状态流转，`mainClock` 驱动）；**不写** `captureToImage`/像素断言（Robolectric 下超时） | `ui-test-junit4`（testImplementation）+ Robolectric |
| Instrumented（~10%） | 真实 JNI/.so 加载、wgpu 帧输出、Roborazzi 截图像素、真实 IME、手势、Cucumber、LeakCanary | Espresso / UiAutomator / Roborazzi / Cucumber |
| E2E（~5%） | 启动→会话→命令→搜索→设置→主题→选择/复制粘贴关键旅程 | maestro（`scripts/test-emulator.nu`，`--include-tags smoke,e2e` 22 flows） |

### 覆盖率基线（审计时点）

约 2400 测试点：Rust 运行用例 1455（`cargo test --workspace -- --list` 实测，含
integration-tests 与 tool_lint 21；静态 `#[test]` 计数 1445 由 `tool_lint`
`rust_test_count_within_baseline` 守护，偏差 >25% 即失败）+ Kotlin JVM 573（`testDebugUnitTest`
XML 实测）+ Kotlin instrumented 338 @Test（其中 27 处 `@Ignore` 冻结，见下节）
flow + Macrobenchmark 3 + baselineprofile 1

- 分层判定：VT/OSC/网格/PTY/字体/渲染逻辑可无头证明；真实 JNI 符号、
渲染到屏幕、IME、系统服务协同只能 instrumented/真机证明（原 `docs/test-coverage-audit.md`
§5 的判定结论，已吸收至本条）。

### Kotlin 覆盖率门禁（kover）

`org.jetbrains.kotlinx.kover` 0.9.9 已接入（root + app 插件）。报告命令（nix develop 内）：

```bash
cd android && ./gradlew :app:koverXmlReportDebug -Dorg.gradle.internal.test.results.binary.enabled=false
# 报告：app/build/reports/kover/reportDebug.xml（HTML：koverHtmlReportDebug）
```

- 覆盖对象：`testDebugUnitTest`（JVM/Robolectric 单测层）；`androidTest`（instrumented）
代码不在统计内——UI/runtime 的低百分比不代表无覆盖，需与 `docs/standards/TESTING.md`
设备测试配合解读。
- 审计时点（2026-08）：单测层总行覆盖 26.5%（12292/46420）。低点：`runtime` 14%、
`ui` 13%、`bridge` 34%——其中大量逻辑由 instrumented 测试覆盖；高点是纯逻辑包
`util` 100%、`input` 89%、`bell` 87%、`service` 83%、`installer` 76%、`monitor` 73%。
- 故意不设最低阈值：UI/渲染大量以设备测试证明，按百分比设门禁会误导；后续若
逐包提取纯逻辑（如 `runtime`/`bridge` 中的决策函数），可对特定包设下限。

### Instrumented 方法论与研究结论（2026-08）

对"哪些功能没有测试、如何减少模拟器依赖"的深度研究结论（含已验证的工具事实）：

**1. 三侧全覆盖矩阵已建立，设备域零覆盖清单**（R=Rust / K=Kotlin JVM / A=androidTest）：

| 功能域 | R | K | A | 备注 |
|--------|---|---|---|------|
| 终端渲染/像素 | ✅ 极强 | 辅助 | ✅ | Rust Lavapipe 像素断言冗余于 A |
| 输入编码 | ✅ | ✅ 最强 | ✅ | A 侧无结果断言 |
| 搜索/高亮/OCR | ✅ 强 | ✅ | ⚠️ 冻结 | A 冻结有 R 双轨 |
| 选择/剪贴板 | ✅ 强 | ✅ | ⚠️ 冻结 | OSC52 往返在 R 侧 |
| 会话管理 | ✅ 强 | ✅ | ✅ | |
| Bootstrap 安装 | — | ✅ | ✅ 最扎实 | 唯一无 R 侧的功能域 |
| 主题/字体/修饰键/粘贴 | 部分 | ✅ | ⚠️ 部分弱 | A 侧多为 smoke |
| 性能监控 | — | ✅ 唯一 | — | 无设备/集成验证 |
| 快捷键 | — | ✅ 15 | — | |
| **零覆盖** | — | — | — | `ShortcutCaptureDialog`、`ThemeEditorDialog`/`ColorPickerDialog`（UI）|

设备域**三侧零覆盖**（按风险）：`shortcut/ShortcutCaptureDialog.kt`（快捷键→终端
e2e）、`ui/theme/ThemeEditorDialog.kt` + `ColorPickerDialog.kt`（主题编辑 UI）、
`installer/BootstrapInstallService.kt`（前台服务执行链路）、`TerminalApp.kt`
（初始化/全局生命周期）、`TestBackdoorReceivers.kt`（生产内测试后门本身）、
`SettingsComponents.kt`（纯展示，可豁免）。这些属于 androidTest 域，无法 JVM 化。

**2. Kover 不支持 instrumented 覆盖率**（kover 0.9.9 官方文档：
"instrumentation tests executing on the Android device are not supported yet"）——
**没有免费的"Kotlin 全量合并矩阵"**。替代路径：JaCoCo 采集 `connectedDebugAndroidTest`
（AGP 9 需单独集成，未接）；现状解读方式为 kover（unit）+ 上表的静态矩阵（A 侧）。

**3. 减少模拟器依赖的现成杠杆**（按性价比排序）：
- **Roborazzi 可运行在 JVM**（Robolectric + `ui-test-junit4`）：纯 Compose 结构测试
  （ModifierBar、布局、主题）与 Roborazzi golden 可放 `src/test/` 免模拟器跑。
  尚未试点——试点前置条件：目标 Composable 构造路径不触 JNI（`ModifierBar` 满足）。
- **testBalloon 1.0.1 已在依赖但未启用**（`app/build.gradle.kts` 第 249/285 行）：
  可把部分 instrumented 逻辑测试降到 JVM；启用前需研究其注解/运行器用法。
- **后端 e2e 无需模拟器**：native MCP（`terminal_info`/`last_command_output`/
  `send_signal` 等 12 工具）协议层已由 `tower_mcp::testing::TestClient` 全测
  （`native/src/mcp/mod.rs` tests）；真实 session 接线在 Kotlin `TerminalRuntime`，
  属模拟器域——「应用暴露接口 + 后端分离」已实现，缺口只是 Kotlin 接线层。
- **GMD/ATD**（Gradle Managed Devices + Automated Test Device）：并行分片 +
  快照加速 + 一致性，需 AGP DSL 配置 + CI 硬件；`scripts/` 只读，未接。
- **TestBackdoorReceivers**（`terminal.emulator.*` 广播，RECEIVER_NOT_EXPORTED）：
  已是 instrumented/Maestro 的确定性控制面；新增 e2e 场景优先走它而非坐标点击。

**4. CI 模拟器渲染是当前最大环境债**：swiftshader 1.8fps 是软件渲染 1080x2400 的
硬件极限；Compose `waitForIdle` 依赖持续渲染循环 → 27 个 @Ignore 的根因。
解除冻结的唯一路径是硬件加速设备（KVM/GPU 直通），不是改测试。

## Benchmarks & Performance Thresholds

（吸收自 `docs/performance.md` 与 `docs/project-health.md` §5，原文已删除；
基准命令与阈值断言以 bench 代码为准，本节为运行方式与典型值记录。）

**运行命令**：`cargo test --features test-util -- bench`（`#[bench]` 门控，默认 debug
profile；release profile CPU 密集路径约 5-50× 提升）。

**阈值双层机制**：本地（非 CI）断言严格阈值（typing < 3 ms/keystroke、bulk > 8k cells/s、
scroll > 800/500 snaps/s），真回归即失败；CI（并行测试负载 + 软件 Vulkan 争抢）用约 5×
低于本地单跑的防抖下限，只拦数量级回归。GPU 吞吐阈值两层均生效（buffer upload 在
Lavapipe 下也是 CPU 拷贝有界）。

| 基准 | 典型值（Lavapipe x86 debug） | 阈值 | 说明 |
|------|------------------------------|------|------|
| `bench_typing_latency` | 0.2 ms/keystroke | < 6 ms | `\n` 终止写 + flush |
| `bench_bulk_output_throughput` | 17.72 MB/s（1.9M cells/s） | > 4 kB/s | 4×64KB 写 |
| `bench_scroll_throughput` | 55-56 snaps/s | > 800 | 3 个滚动偏移（0/5/50） |
| `bench_cell_data_vs_grid_snapshot` | 1.47× faster | ≥ 0.5× | CellData 不得慢于 GridSnapshot |
| `bench_build_instances_from_cell_data` | 920 fps（release 5107 fps） | > 200 fps | 1920 cells × 100 迭代 |
| `bench_gpu_buffer_upload_throughput` | 40+ GB/s | > 500 MB/s | 裸 wgpu buffer 写带宽 |
| `bench_cpu_end_to_end_pipeline` | 1362 fps | — | VT 写 → CellData → CellInstance |
| `bench_cjk_glyph_cache_effectiveness` | 6.8M→0.17M ops（-97.5%） | — | 缓存消除 swash 查表 |

**GPU 环境**：需 Vulkan 设备或 Mesa Lavapipe（软件 Vulkan），`VK_ICD_FILENAMES` 指向
Lavapipe `.json` ICD（由 `nix develop` shell 提供）；无设备时 GPU 依赖测试提前返回（不 panic）。

**已知环境回归**：`gpu_render_colored_text` / `vt_color_background_blue` /
`vt_color_foreground_red` / `vt_color_reset` 在 Lavapipe fp16 blend 下颜色精度 0.9→0.8
（真机 Adreno/Mali 通过），阈值已放宽，见 `docs/rejected-technologies.md` §3.1 D28。

**模拟器性能基线**（emulator-5554，1080x2400, 420dpi, SwiftShader）：
1.8fps 是软件渲染 1080x2400（约 8.3M 像素/帧）的硬件极限，**不是 app 缺陷**：
`build_instances` 在 native bench 为 475+ fps（CJK 缓存后 release 5107 fps），
`grid_dirty` 门控确保 idle 不重绘。模拟器帧率测试的价值是**回归检测**：app 侧
逻辑变慢会进一步降低帧率；稳定 1.8fps = 无 app 侧性能回归。真机（Adreno/Mali）
帧率预期 60fps+，受限于 `pollAll` + `waitOutput`（约 500ms 轮询间隔）而非渲染管线。
后续优化方向：渲染线程轮询改事件驱动（`notifyRender` 已存在，验证其覆盖所有路径）；
CellData 增量传输（dirty 行）——SwiftShader 下无意义，GPU 设备可考虑。
（吸收自原 `docs/media/selection/emulator-performance.md`，原文已删除。）

**渲染路径性能特征**（CellData fast path，生产渲染路径）：0 FFI calls/cell（GridSnapshot
路径 8+ 次）、80B `bytemuck::Pod`、flume 通道（无锁、bounded 256）、
`build_instances_from_cell_data` O(n) 转换 + grapheme cluster stacking；GPU readback
（截图测试 / GPU 计算校验）走 `device.on_processed()` 通道 100ms 超时，无忙等。

**已知测试缺口**（原 `docs/project-health.md` §3，需要基建投入而非快速修复）：
`process_output` 无直接单测（VT 输出通道处理，集成测试间接覆盖）；`save_session` /
`restore_session` 无持久化往返测试（小范围，格式由 Ghostty 控制）；GPU pipeline 创建
（`pipeline.rs` 0 单测、`context.rs` 1 个 Send/Sync、`cell_builder.rs` 10 个）由 GPU 集成
测试端到端覆盖。PIT 变异测试已注册未接 CI（`docs/rejected-technologies.md` §2.2 D26）。
