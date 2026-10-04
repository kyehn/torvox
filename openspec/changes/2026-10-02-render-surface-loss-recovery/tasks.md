# 任务

## 1. 取证（已完成）

- [x] 1.1 全量 `:app:connectedDebugAndroidTest` 实测并落档：release profile native 库、
      Android 15 x86_64 模拟器 emulator-5554，161 例 / 27 失败 / 0 跳过 / 22m12s，
      模拟器存活；`BehaviorInstrumentedTest` 10/10 通过（CI run 的失败用例已消除）
- [x] 1.2 A/B 对照：同一台机器、同一 AVD、同一用例，dev profile 库复现 CI 失败
      （消息为空 + 设备丢失），release profile 库通过 → emulator 步骤先行部署 release profile 库
- [x] 1.3 logcat 根因取证：pid 5303 内 `attach_surface: configured=1`、
      `RECONFIGURE_SWAPCHAIN=132`、`BufferQueue has been abandoned=6148`、
      `Surface::configure` 错误 6148、`ERROR_SURFACE_LOST_KHR=3074`、`begin_frame failed=3074`
- [x] 1.4 27 个失败逐条落档并分类：A 画面未呈现 8、B 输入未送达 9、C Shell/VT/剪贴板 6、
      D 预置资产缺失 2、E 其他 2，合计与 XML 报告 27 一致

## 2. 可控失效注入（验证手段，也是回归护栏）

- [x] 2.1 失效注入开关：`Renderer::set_surface_loss_injected_for_test` 经
      `NativeBridge.setSurfaceLossInjected` 暴露（`render/context.rs:619`），
      `begin_frame` 逐帧判该标志。取舍：不是 `#[cfg(test)]`，而是运行时开关——
      仪器化用例跑的是 release profile 库，`cfg(test)` 开关在设备上不存在
- [x] 2.2 仪器化用例 `SurfaceLossRecoveryInstrumentedTest` 已落地：注入 → 宿主换
      `SurfaceView` → 恢复后画面有墨迹 → 新命令落格
- [ ] 2.3 注入用例「先红后绿」的证据需模拟器复跑（本地无 AVD/system-image，
      仪器化套件不可本地执行；证据以 build workflow 全量运行结果为准）

## 3. 原生侧失效缓存与状态位

- [x] 3.1 `Renderer`：新增 surface 失效标记；`attach_surface` 快路径条件收紧为
      「已配置且未失效」（`render/context.rs:368`）
- [x] 3.2 `begin_frame` / acquire 的 surface 级失败置失效（幂等、每帧最多记一次）；
      `render_paused` 与「本帧无内容」不置失效（`render/pass.rs:339-349`、`context.rs:343-345`）
- [x] 3.3 `renderWithNewOutput` 打包返回新增失效状态位（复用既有返回通道，零额外 JNI）；
      函数文档位段注释同步
- [x] 3.4 `detachWindow` 语义不变；失效位随 RenderState 清除
- [x] 3.5 Rust 单测（`surface_loss_transition` 纯函数）：单次失败不置失效、连续置失效、
      跳过帧清零计数、置位幂等；「失效后 attach 走重建路径」需真实窗口，由 2.2 的
      仪器化注入用例覆盖

## 4. 宿主重建 surface

- [x] 4.1 观察方是 `TerminalRuntime`（不是 `TerminalSurface`：渲染循环在 runtime），
      裁决后递增 `surfaceRecreateRequests`；`TerminalScreen` 以 `key(计数)` 换掉整个
      `SurfaceView`（比切 visibility 更确定：旧视图必被拆除并 `surfaceDestroyed` 释放
      wgpu surface），新视图的 `surfaceCreated` 交付新的 `ANativeWindow`
- [x] 4.2 限流：状态位回落为 0（即开始出帧）或距上次请求 ≥ 最小间隔才允许再次请求；
      连续失败达上限后停止并打一条 error
- [x] 4.3 `RenderResult` 解包新增字段；不改 ime 位移裁剪与网格锚定
- [x] 4.4 Kotlin 单测：0→1 触发一次请求、回落前不重复、间隔与上限生效

## 5. 逐例跟踪：基线的 27 个失败用例

基线为 161 例 / 27 失败（`TEST-test_avd(AVD) - 15.xml`）。下表按最新一次全量运行
（build run ，commit 302a9e16，15 失败）逐条给出结论：通过者关闭，
仍失败者保留并记录**新断言**（基线断言已被更贴近根因的断言取代，例如「屏幕无墨迹」
换成「标记必须落格」）。仍失败项的根因定位需要模拟器复跑——本机无 AVD/system-image，
仪器化套件不可本地执行。

| # | 用例 | 分类 | 最新运行结论 |
| --- | --- | --- | --- |
| 5.1 | `ScrollBehaviorQuantifiedTest#enter_snaps_viewport_to_bottom_within_budget` | C | **通过**，基线失败未复现 |
| 5.2 | `SelectionDragQuantifiedTest#handle_drag_updates_highlight_live_between_steps` | B | 仍失败：`标记必须落格: dragstart …` |
| 5.3 | `SelectionDragQuantifiedTest#word_longpress_shows_copy_selectall_without_paste` | B | 仍失败：`标记必须落格: targetword …` |
| 5.4 | `SelectionDragQuantifiedTest#paste_only_handle_drag_upgrades_selection_and_grows_D75` | B | 仍失败：`标记必须落格: growme …` |
| 5.5 | `SelectionEspressoTest#selectAllShowsSelectionMenu` | A | **通过** |
| 5.6 | `SelectionEspressoTest#copyActionPlacesTextOnClipboard` | A | **通过** |
| 5.7 | `ShellResponseLatencyTest#rapid_command_stream_preserves_order_and_completes` | B | 仍失败：burst 标记未完整出现 |
| 5.8 | `ShellResponseLatencyTest#shell_echo_latency_meets_emulator_budget` | B | 仍失败：`UXMARK1` 未在预算内落格 |
| 5.9 | `CursorPixelAcceptanceTest#cursorBlockMatchesRenderCursorCell` | A | 仍失败：`T0-boot: 光标格必须变亮` |
| 5.10 | `SgrColorPixelAcceptanceTest#sgrRedTextProducesRedPixels` | A | **通过** |
| 5.11 | `SgrItalicPixelAcceptanceTest#sgrItalicTextProducesDistinctGlyphPixels` | A | 仍失败：`斜体字形像素必须与正体不同 (差分=0)` |
| 5.12 | `BootstrapCompatibilityTest#null` | D | **通过**（bootstrap 资产已就位） |
| 5.13 | `TermuxBootstrapRealTerminalTest#…asserted_output` | D | **通过**（bootstrap 资产已就位） |
| 5.14 | `FontSizeReflowInstrumentedTest#fontSizeChangeReflowsGridAndScalesCellHeight` | E | **通过**：列数随字号收缩正确 |
| 5.15 | `ImePopupPixelInstrumentedTest#contentManyImePopupMovesUpBottomIdentical` | A | **通过** |
| 5.16 | `ImePopupPixelInstrumentedTest#contentFewImePopupTerminalStaysPutAndVisible` | B | 仍失败：`内容较少时弹出输入法终端必须无变化 (差分=2426)` |
| 5.17 | `ImePopupPixelInstrumentedTest#imeCommitChineseTextGridded` | B | **通过**（CI 模拟器装有中文输入法，基线的环境阻塞不成立） |
| 5.18 | `MultiTapSelectionInstrumentedTest#doubleTapSelectsWordAndCopyFillsClipboard` | B | **通过** |
| 5.19 | `MultiTapSelectionInstrumentedTest#tripleTapSelectsLine` | B | **通过** |
| 5.20 | `Osc52ClipboardInstrumentedTest#osc52_sequence_sets_system_clipboard` | C | **通过** |
| 5.21 | `PasteButtonInstrumentedTest#pasteMenuTypesClipboardIntoShell` | C | 仍失败：`shell 回显链必须健康 (探针=Q571)` |
| 5.22 | `SessionDrawerInstrumentedTest#addSwitchAndCloseSession` | E | 仍失败：`必须切回首个会话` |
| 5.23 | `ShellPtyInstrumentedTest#shellBellReportsEvent` | C | **通过** |
| 5.24 | `StickyCtrlInterruptInstrumentedTest#stickyCtrlPlusCInterruptsRunningCommand` | C | 仍失败：`CTRL+c 必须产生真实 ^C 中断 (rc=130 未出现)` |
| 5.25 | `VisualInlineVerificationTest#verifyUrlSelectionPositions` | A | **通过** |
| 5.26 | `VisualInlineVerificationTest#verifyWordSelectionPositions` | A | 仍失败：`Expected >=2 selection handles, found 0` |
| 5.27 | `VtCorrectnessInstrumentedTest#bellEventIsReportedViaVtFeed` | C | **通过** |

新出现（不在基线 27 例内）：`SelectionEspressoTest#partialSelectShowsSelectionMenu`、
`diag.SelectionTapDismissTest#tapDismissesSelectionWithoutPhantomLongPress`、
`ui.SurfaceLossRecoveryInstrumentedTest#persistentSurfaceLossTriggersHostRebuildAndKeepsRendering`
（`标记必须落格: PRE_INK_MARKER`）——三条与 B 类同族（注入/落格链路），归入第 5 节同一批
定位。

仍失败 12 例的共同前置：多数用例靠「标记必须落格」判定，而失败信息里同一网格还留着
**其他用例的残留输出**（StickyCtrl 的尾部含 `SELL_ALL_A_…`、`PPPPPP…`）——仪器化
套件跨用例共用同一会话，先前的输出污染断言。定位前先确认这一点是否即根因。

## 6. 验证与文档

- [x] 6.1 本地门禁全绿（`nix develop` 内逐项执行，等价 CI 的 check 工作流）：
      `scripts/check-rust.nu`（fmt/clippy/machete/semgrep/test/rustdoc/markdownlint/
      bench，0 失败）与 gradle 侧 `detekt spotlessCheck dokkaGenerate lintDebug
      lintVitalRelease assembleDebugAndroidTest testDebugUnitTest
      benchmark:compileBenchmarkReleaseKotlin
      baselineprofile:compileNonMinifiedReleaseKotlin`（506 例单测全过）
- [ ] 6.2 真机/模拟器实测：人为制造 surface 失效（2.x 注入或系统回收）后自愈，
      记录恢复耗时与日志 —— 需模拟器/真机（本机无 AVD）
- [ ] 6.3 全量 `:app:connectedDebugAndroidTest` 复跑，逐例回填第 5 节结论，
      失败数从 27 降到剩余未修项的真实数量，且 `abandoned` 之后不再持续 `count=-1`
      —— 第 5 节已按 run 逐条回填（27 例中 15 例通过、12 例仍失败），
      仍失败项待模拟器复跑后收口
- [ ] 6.4 更新 `openspec/specs/render-stability/spec.md`，完成后归档本 change
