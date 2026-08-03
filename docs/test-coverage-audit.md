# 测试覆盖与可靠性审计报告

日期：2026-08-02
范围：全仓库（Rust `native/` + Kotlin `android/` + 测试体系）
方法：4 路并行只读调查（Rust 功能 / Rust 测试 / Kotlin 功能与测试 / 测试矩阵）+ 模拟器端到端实测

---

## 1. 测试执行实测结果（emulator-5554，2026-08-02）

| 入口 | 结果 | 用时 |
|---|---|---|
| `:app:connectedDebugAndroidTest`（Cucumber 22 scenario） | ✅ 27/27 通过 | 2m28s |
| `:app:installRelease` | ✅ 成功 | 7s |
| `benchmark:lockClocks` + `:benchmark:connectedReleaseAndroidTest`（3 macrobenchmark） | ✅ 3/3 | 3m9s |
| `:baselineprofile:generateBaselineProfile` | ✅ 成功 | 2s |
| Maestro flows（`--include-tags smoke,e2e`，22 flows） | ✅ **22/22** | 11m35s |

性能基线（模拟器软件渲染环境，模拟器无硬件 GPU）：
- coldStart：median **312.5ms**（min 87 / max 1779）
- warmStart：median **317.5ms**
- terminalOutputTiming：median **381.5ms**
- 注：模拟器不支持 GPU clock lock（`Failed to set property 'ctl.interface_restart'`），数值方差大属预期。

交互动画帧级基线（`InteractionAnimationBenchmark`，FrameTimingMetric，2026-08-03）：
- modifierKeyPressAnimation（CTRL 键 spring 按压动画）：frameCount median **12**（11-13）
- imeShowAnimation（键盘弹出，底栏 220ms tween 上移）：frameCount median **4**（3-4）
- 修复前基线：imeShow frameCount median **0**（`imePadding` 瞬跳，无动画帧）
- 像素级验证（screenrecord 29 帧）：底栏顶边 y **2090→1790→1600→1530→1500**（4 个中间过渡位，
  moved=590px），证明动画逐帧渐进而非瞬跳
- imeHideAnimation 已从 benchmark 移除：系统 `IME_INSETS_HIDE_ANIMATION` 在软件渲染模拟器上
  掉帧超时（FrameTracker force finish），损坏 perfetto trace 导致 UTP 输出插件 EOF；该动画
  帧证据见上方像素级数据（back 键收起时底栏回落同样渐进）

帧耗时指标（同产物，**仅作相对信号**——软件渲染模拟器绝对帧耗时无意义）：
- imeShowAnimation：frameDurationCpuMs P50=**249ms**、P90=744ms；frameOverrunMs P50=283ms
- modifierKeyPressAnimation：frameDurationCpuMs P50=71.5ms、P99=997ms
- 模拟器软件渲染（SwiftShader，2 核）下这些数值不代表真机性能；真机（Mali/Adreno 硬件 GPU）
  帧耗时预期 <16ms。回归检测用 frameCount（动画有无），不用帧耗时绝对值。

---

## 2. Rust 端功能清单（32 文件，生产 ≈14.6k 行 + 测试 ≈18.2k 行）

### 2.1 模块功能

| 模块 | 文件 | 行数 | 功能 |
|---|---|---|---|
| `terminal/ghostty_terminal/` | 7 文件 | 2,636（生产） | VT 引擎：双命令通道（cmd/query）、快照缓存、搜索、DEC 矩形操作、key encode、样式位打包、KGP 图像存储 |
| `terminal/session.rs` | — | 1,128 | 会话编排：状态机 Spawned→Running→Exited→Cleaned、reader 线程、`process_output`、`poll_pty_output`（后台限流）、resize、send_signal、focus_event |
| `terminal/pty.rs` | — | 1,012 | fork+setsid+控制终端+termios（关 IXON/IXOFF、开 IUTF8）、环境构建 |
| `terminal/osc_handler.rs` | — | 618 | OSC 7/8/9/52/777 拦截、跨缓冲区分片、负载上限 1MiB |
| `terminal/output_processor.rs` | — | 218 | BEL 提取、OSC 133 shell integration 标记（四态） |
| `render/` | 16 文件 | 10,219 | wgpu 全管线：Renderer/FrameContext、cell/surface/KGP/模糊 shader、字体（cosmic-text+swash+guillotiere）、CJK 回退、5 类 LRU 缓存、Lavapipe 可测 |
| `android/ffi.rs` | — | 1,487 | 17 个 JNI 导出、会话注册表（RwLock）、`jni_export_guard` 防 panic 逃逸、MCP 回调桥 |
| `android/logging.rs` | — | 148 | logcat + 文件双写 |
| `mcp.rs` | — | 802 | 9 个 MCP 工具，Unix socket + stdio 双传输 |
| `event.rs` | — | 334 | 事件队列（上限 1024，Exit 永不丢） |
| `lock_util.rs` | — | 91 | 毒锁恢复 |

### 2.2 JNI 导出（17 个）

initSession / destroySession / switchSession / getSessionCount / resize / focusEvent / feedPty / writeKey / pollEvent / initLogger / setLogFilePath / attachWindow / detachWindow / setMcpEnabled / clipboardResult / dialogResult / listSessions

### 2.3 MCP 工具（9 个）

terminal_info / clipboard_get / clipboard_set / notify / toast / open_url / send_signal / pick_file / dialog

### 2.4 stub / 死代码 / 未实现（必须知晓）

| 位置 | 状态 |
|---|---|
| `ffi.rs:1175-1233` attachWindow / detachWindow | **ADR-0007 占位 stub**：ANativeWindow 获取即 release，不接入渲染；Kotlin 从不调用 |
| `ffi.rs:1465-1466` listSessions | 死代码："Kotlin currently never calls this" |
| `osc_handler.rs:321-325` OSC 52 **读** | 未实现：收到读请求仅 log 警告 |
| `pty.rs:61` set_pixel_size | 死代码（无生产调用方） |
| `ffi.rs:883-886` writeKey 修饰符编码 | 简化实现（完整 Kitty 键盘协议未用） |
| `context.rs:657` select_present_mode | 非测试构建无调用方（allow dead_code） |

---

## 3. Kotlin 端功能清单（`android/app/src/main/java/terminal/emulator/`）

| 类/文件 | 功能 |
|---|---|
| MainActivity | 启动、权限请求、MCP 对话框/pick_file 接线、广播接收器 |
| TerminalViewModel | 核心 VM：selection CAS 状态机、字体加载、bootstrap URL、状态合并 |
| TerminalRuntime（153KB） | 会话编排：start/stop、渲染线程监视与重启、事件消费、exit 处理 |
| TerminalSurface | SurfaceView + 手势（长按/拖动/双指缩放/边缘滚动）、选择手柄 |
| TerminalScreen | Compose UI 骨架、搜索/选择/粘贴弹层组合 |
| Bridge / NativeBridge | JNI 桥（instance API + 静态 externals）、PollResult 解析 |
| SettingsScreen | 设置项（外观/字体/主题/背景/终端/Shell） |
| SessionDrawer | 会话抽屉（列表/新建/切换/关闭/搜索入口） |
| TextSearchBar | 搜索栏（大小写/模糊/上下跳转/计数） |
| PasteChipOverlay | 粘贴气泡 |
| installer/ | Bootstrap 下载/安装/二阶段（zip-slip 防护、https 强制） |
| service/ | 前台服务 + 通知（OSC 9/777） |
| monitor/ | AnrWatchDog / BootGuard / MemoryMonitor / ThermalMonitor / RenderWatchDog |
| DocumentsProvider | SAF 文件提供器（symlink 方向与 exec-bit 处理） |
| InputBatchBuffer / InputCoalescer / MouseModeTracker / UrlDetector / NerdKeyLabels / TerminalInputEncoder / FontUtils | 纯逻辑工具 |

---

## 4. 测试矩阵：每类测试证明什么、在哪跑

| 入口 | 数量 | 能证明 | 环境 |
|---|---|---|---|
| native Rust 单测 | 1052 | VT/OSC/网格/PTY/会话/字体/渲染逻辑正确性（Lavapipe 渲染 PNG + rapidocr OCR） | 无头 Linux |
| integration-tests lib | 79 | 仓库结构/脚本/CI 配置校验 | 无头 |
| terminal_render_test | 47 | 跨 crate GhosttyTerminal API 行为 | 无头 |
| tool_lint | 15 | 文档/需求结构、typo、machete | 无头 |
| jni_bridge_test | 2 | JNI 符号导出（host JVM 加载 libnative.so） | 无头 + JDK |
| exec-bin | 3 | 存在性/usage | 无头 |
| Kotlin JVM（Robolectric） | 7 | DocumentsProvider SAF、BootstrapInstaller | 无头 JVM |
| Kotlin instrumented | 331（10 @Ignore） | 真实 Activity/渲染/触摸/跨应用 | **模拟器** |
| Cucumber | 64 scenario（42 @wip，22 有效） | 行为验收 | **模拟器** |
| Maestro | 30 flow（脚本跑 22） | 端到端用户旅程 | **模拟器** |
| Macrobenchmark | 3 | 冷/热启动、帧时序 | **模拟器** |
| baselineprofile | 1 | 基线 profile 生成 | **模拟器** |
| **合计** | **≈1640 测试点** | | |

---

## 5. 分层可靠性判定

### 5.1 Rust 端可完全证明的（有真实断言、无头可跑）

- ✅ VT 解析/OSC/网格/样式位打包（1000+ 测试）
- ✅ 快照缓存、搜索（含 CJK 边界回归测试）
- ✅ PTY fork/termios/child pid、MockPty
- ✅ 字体 shaping 正确性（65/67，2 个 golden 仅 CI 有）
- ✅ 渲染正确性（Lavapipe + OCR）
- ✅ 事件队列 FIFO、锁恢复、glyph cache 清理
- ✅ MCP 4/9 工具（list_tools / terminal_info / clipboard_set / method_not_found 错误处理）

### 5.2 Rust 端弱证明 / 无证明

| 功能 | 状态 |
|---|---|
| ~~MCP 其余 7 工具~~ | ✅ 已解决（mcp.rs 12 个测试覆盖全部 9 工具，见 §7 #10） |
| render/context.rs、pipeline.rs、font/pipeline.rs、atlas.rs、cjk.rs | 0 单测（cell_builder 已有 10 个纯逻辑测试；pipeline/atlas/cjk 需 GPU/字体） |
| android/ffi.rs | 0 单测（仅 jni_bridge_test 间接验证符号） |
| process_output 通道路径 | 无直接单测（project-health 自认） |
| property/fuzz 测试 | 依赖已声明、**零使用**（proptest/quickcheck/shuttle） |
| 并发多线程 session 访问 | 无 |

### 5.3 Kotlin 端可完全证明的

- ✅ DocumentsProvider SAF 逻辑（3 测试）
- ✅ BootstrapInstaller 解压/原子 rename（4 测试，symlink 需设备）

### 5.4 Kotlin 端零覆盖的高价值纯逻辑（可 JVM 测试）

- TextSearchBar.findMatches
- TerminalInputEncoder
- MouseModeTracker
- UrlDetector
- InputCoalescer
- InputBatchBuffer（已有 forTest 工厂）
- FontUtils
- TerminalViewModel 选择状态机
- SettingsRepository / DataStore
- monitor/ 四件套
- TerminalForegroundService / TerminalNotificationHelper

### 5.5 只能模拟器/真机验证的（已覆盖）

真实 JNI 符号解析、渲染到屏幕（SwiftShader）、IME、权限对话框、系统 UI 交互、剪贴板、通知、前台服务/wakelock、触摸手势、真实 bootstrap 下载、LeakCanary、perfetto 性能。

---

## 6. 问题清单（层级分类）

### P0 —— 与代码实际不符、误导决策

| # | 位置 | 问题 |
|---|---|---|
| 1 | `docs/standards/TESTING.md` | 包路径 `io/term/...` 全部失效（实际 `terminal/emulator`）；声称 Roborazzi goldens 已提交——目录不存在且 FR-055 禁止 PNG；声称 src/test 有 Compose 单测（实际仅 2 个 Robolectric 类）；引用不存在的 `native/tests/` |
| 2 | `docs/traceability.yml` | 82 需求几乎全用 `cargo test --package native` 验证；0 引用 instrumented/maestro（常量定义了却未使用）；引用不存在的 `native/src/render/surface.rs` 路径（surface 已内联进 context.rs；keymap.rs 引用有效）；FR-024 标 instrumented 却配 unit 命令；NFR-019/020/021 `tests: []` |
| 3 | `docs/acceptance.md` | FR-049~053 编号与 srs.md 错位重复；引用不存在的过滤名 `ffi_contract_tests`/`gpu_noop_tests`；声称 native 测试覆盖 JNI/surface lifecycle（实际 ffi.rs 0 测试） |
| 4 | `android/app/build.gradle.kts` 注释 | "No unit tests exist in src/test"——已过时（现有 7 个） |
| 5 | `.github/workflows/android-tests.yml` | 上传不存在的 `src/test/resources/roborazzi/` artifact；recordRoborazziDebug 无 goldens 时空操作 |

### P1 —— 测试虚空通过/名实不符

| # | 位置 | 问题 |
|---|---|---|
| 6 | Bridge.kt stub（ADR-0007） | `searchAllInScrollback→null`、`isCellEmpty→true`、`getTerminalText→null`、`listFontFamilies→null` → 搜索/选择链路全部"虚空通过"；cucumber 搜索 7/7、选择 4/5 全 @wip |
| 7 | `UiAutomatorTest.searchTypingShowsResultCount` 等 | 空转测试未标 @Ignore，与同断言已 @Ignore 的测试不一致 |
| 8 | `AppStartupBenchmark`（原 BridgeMicrobenchmark） | 已改名：明确为 app 启动/帧宏基准；bridge 调用级测量由 Rust bench + JNI 集成测试承担 |
| 9 | `StageHSelectionEspressoTest` 注释 | 承认 New Session/session switching 触发已知 wgpu GPU-surface hang |
| 10 | `TextSearchEndToEndTest` | OCR 依赖宿主 `rapidocr` 二进制，缺失时断言静默跳过 |
| 11 | `AppInstrumentedTest`/`BehaviorInstrumentedTest` 若干 | 反断言"不显示 Nerd/OSC133 切换"引用已删除功能 |
| 12 | `performance.md` | 自认 4 个测试在 Lavapipe 上因 fp16 精度失败靠放宽阈值掩盖，与 TESTING.md "No flaky tests" 矛盾 |

### P2 —— 数量/描述过时、低优先

| # | 位置 | 问题 |
|---|---|---|
| 13 | project-health.md / review-status.md | "1046 tests"/"1045 unit tests" 过时（实际 ≈1198） |
| 14 | maestro/suites/ | 3 个套件文件不被任何脚本使用（脚本已注释说明） |
| 15 | build.gradle.kts | 未使用的 mockk/turbine/archunit 依赖；PIT 变异任务注册未接入 CI |
| 16 | cucumber feature 与 steps 脱节 | terminal-font "Invalid font does not crash" 无 @wip 但 steps 空实现 |

---

## 7. 修复建议（优先级）

### 立即（P0 文档修正）
1. 重写 TESTING.md 路径与事实
2. 修正 traceability.yml 验证映射（接入 instrumented/maestro 常量）
3. 修正 acceptance.md 编号错位
4. 更新 build.gradle.kts 注释 + android-tests.yml artifact

### 短期（P1 测试真实化）
5. Bridge stub 决策：ADR-0007 落地前，将虚空测试统一 @Ignore 并注释"待 bridge 落地"；或实现 `searchAllInScrollback` 的 Rust→JNI 通路
6. ~~空转测试补齐 @Ignore~~ ✅ SelectionVisualVerificationTest 类级 @Ignore（KDoc 声明但无注解，虚空测试真实运行）
7. ~~BridgeMicrobenchmark 改名~~ ✅ 已改 AppStartupBenchmark（26467d2 后续提交）；bridge 调用级测量由 Rust bench 套件承担，设备端 JNI 微基准记录为 follow-up
8. performance.md 4 个阈值测试改为条件跳过而非放宽

### 中期（P2 覆盖补齐）
9. ~~Kotlin 纯逻辑单测~~ ✅ 已落地（FontUtils/SettingsRepository/AnrWatchDog 本轮 +14；Encoder/MouseModeTracker/findMatches/UrlDetector/InputBatchBuffer/NerdKeyLabels/Coalescer/PasteChunker 此前 +52）
10. ~~Rust 覆盖补齐~~ ✅ cell_builder 10 测试、MCP 7 工具全测（12 测试）、OSC 52 读实现（FR-036 完整链路 + 5 测试）
11. ~~文档数量同步~~ ✅ project-health/review-status 已更新为 1071 Rust + JVM

### 长期（架构）
12. ADR-0007 落地（attachWindow 接入渲染）后，激活搜索/选择/渲染全链路测试
13. ~~property/fuzz 测试启用~~ ✅ prop_tests.rs 已存在（osc52 roundtrip/arbitrary + 并发队列）
14. PIT 变异测试接入 CI

---

## 8. 附：git 状态

- 最近提交：`c694056`（fix: emulator E2E）
- 工作区干净，与 `origin/main` 同步
- 作者/提交者统一 `jane <jane@computer.local>`
