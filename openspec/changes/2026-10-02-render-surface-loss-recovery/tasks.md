# 任务

## 1. 取证（已完成）

- [x] 1.1 全量 `:app:connectedDebugAndroidTest` 实测并落档：release profile native 库、
      Android 15 x86_64 模拟器 emulator-5554，161 例 / 27 失败 / 0 跳过 / 22m12s，
      模拟器存活；`BehaviorInstrumentedTest` 10/10 通过（CI run 36861699809 的失败用例已消除）
- [x] 1.2 A/B 对照：同一台机器、同一 AVD、同一用例，dev profile 库复现 CI 失败
      （消息为空 + 设备丢失），release profile 库通过 → emulator 步骤先行部署 release profile 库
- [x] 1.3 logcat 根因取证：pid 5303 内 `attach_surface: configured=1`、
      `RECONFIGURE_SWAPCHAIN=132`、`BufferQueue has been abandoned=6148`、
      `Surface::configure` 错误 6148、`ERROR_SURFACE_LOST_KHR=3074`、`begin_frame failed=3074`
- [x] 1.4 27 个失败逐条落档并分类：A 画面未呈现 8、B 输入未送达 9、C Shell/VT/剪贴板 6、
      D 预置资产缺失 2、E 其他 2，合计与 XML 报告 27 一致

## 2. 可控失效注入（验证手段，也是回归护栏）

- [ ] 2.1 原生侧测试用失效注入：仅在 `#[cfg(test)]` 或显式 debug 开关下让下一次
      `begin_frame` 报 surface 失败，不污染生产路径
- [ ] 2.2 仪器化用例：渲染中注入失效 → 断言宿主发起 surface 重建 → 断言恢复后画面重新有墨迹
- [ ] 2.3 注入用例先在未修复代码上失败（红），再在修复后通过（绿）

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

按基线运行（161 例 / 27 失败）报告顺序逐条列出，每例都必须有结论：
「修复后通过」「另开 change」「前置条件」三者之一，不得悬空。分类标记：A 画面未呈现、
B 输入未送达、C Shell/VT/剪贴板、D 预置资产缺失、E 其他。断言原文与耗时取自
`TEST-test_avd(AVD) - 15.xml`（StickyCtrl 一例消息尾部为网格内容，此处只留首行）。

- [ ] 5.1（C）`terminal.emulator.ScrollBehaviorQuantifiedTest#enter_snaps_viewport_to_bottom_within_budget`（7.818s）
      — 断言「viewport never snapped to bottom after Enter」→ 渲染自愈后复测；仍失败则查滚动沉降与视口贴底
- [ ] 5.2（B）`terminal.emulator.SelectionDragQuantifiedTest#handle_drag_updates_highlight_live_between_steps`（20.653s）
      — 断言「标记必须落格: dragstart dragend dragend dragend」→ 渲染自愈后复测（IME 弹起与输入注入链路）；仍失败则取 input_method 日志单独立项
- [ ] 5.3（B）`terminal.emulator.SelectionDragQuantifiedTest#word_longpress_shows_copy_selectall_without_paste`（18.561s）
      — 断言「标记必须落格: targetword targetword targetword」→ 渲染自愈后复测（IME 弹起与输入注入链路）；仍失败则取 input_method 日志单独立项
- [ ] 5.4（B）`terminal.emulator.SelectionDragQuantifiedTest#paste_only_handle_drag_upgrades_selection_and_grows_D75`（19.164s）
      — 断言「标记必须落格: growme growme growme」→ 渲染自愈后复测（IME 弹起与输入注入链路）；仍失败则取 input_method 日志单独立项
- [ ] 5.5（A）`terminal.emulator.SelectionEspressoTest#selectAllShowsSelectionMenu`（16.001s）
      — 断言「Selection menu must appear after Select All」→ 渲染自愈后复测；用例前置断言改为「先确认画面有墨迹/有手柄」
- [ ] 5.6（A）`terminal.emulator.SelectionEspressoTest#copyActionPlacesTextOnClipboard`（10.762s）
      — 断言「复制 action must be present」→ 渲染自愈后复测；用例前置断言改为「先确认画面有墨迹/有手柄」
- [ ] 5.7（B）`terminal.emulator.ShellResponseLatencyTest#rapid_command_stream_preserves_order_and_completes`（9.294s）
      — 断言「burst never fully appeared」→ 渲染自愈后复测（IME 弹起与输入注入链路）；仍失败则取 input_method 日志单独立项
- [ ] 5.8（B）`terminal.emulator.ShellResponseLatencyTest#shell_echo_latency_meets_emulator_budget`（8.055s）
      — 断言「marker UXMARK1 never appeared on screen within 4000ms」→ 渲染自愈后复测（IME 弹起与输入注入链路）；仍失败则取 input_method 日志单独立项
- [ ] 5.9（A）`terminal.emulator.diag.CursorPixelAcceptanceTest#cursorBlockMatchesRenderCursorCell`（17.195s）
      — 断言「T0-boot: 光标格必须变亮」→ 渲染自愈后复测；用例前置断言改为「先确认画面有墨迹/有手柄」
- [ ] 5.10（A）`terminal.emulator.diag.SgrColorPixelAcceptanceTest#sgrRedTextProducesRedPixels`（15.304s）
      — 断言「SGR 红色文本必须产生红色像素 (前=0 最大红=0)」→ 渲染自愈后复测；用例前置断言改为「先确认画面有墨迹/有手柄」
- [ ] 5.11（A）`terminal.emulator.diag.SgrItalicPixelAcceptanceTest#sgrItalicTextProducesDistinctGlyphPixels`（7.558s）
      — 断言「斜体字形像素必须与正体不同 (差分=0)」→ 渲染自愈后复测；用例前置断言改为「先确认画面有墨迹/有手柄」
- [ ] 5.12（D）`terminal.emulator.installer.BootstrapCompatibilityTest#null`（0.000s）
      — 断言「bootstrap failed」→ 前置条件：emulator 步骤前 push bootstrap 资产；另立测试基建项
- [ ] 5.13（D）`terminal.emulator.installer.TermuxBootstrapRealTerminalTest#termuxBootstrap_shell_runs_real_commands_with_asserted_output`（2.181s）
      — 断言「bootstrap zip must be staged first: adb push \<termux bootstrap-x86_64.zip\> /sdcard/Download/termux-bootstrap-x86_64.zip」→ 前置条件：emulator 步骤前 push bootstrap 资产；另立测试基建项
- [ ] 5.14（E）`terminal.emulator.ui.FontSizeReflowInstrumentedTest#fontSizeChangeReflowsGridAndScalesCellHeight`（3.574s）
      — 断言「字号增大后列数必须收缩 (前=33 后=33)」→ 单独诊断：字号生效链路与网格列数重算
- [ ] 5.15（A）`terminal.emulator.ui.ImePopupPixelInstrumentedTest#contentManyImePopupMovesUpBottomIdentical`（43.836s）
      — 断言「内容较多时弹出输入法终端内容必须上移 (位移=0 差异=0)」→ 渲染自愈后复测；用例前置断言改为「先确认画面有墨迹/有手柄」
- [ ] 5.16（B）`terminal.emulator.ui.ImePopupPixelInstrumentedTest#contentFewImePopupTerminalStaysPutAndVisible`（47.994s）
      — 断言「标记必须落格: IME_FEW_88202」→ 渲染自愈后复测（IME 弹起与输入注入链路）；仍失败则取 input_method 日志单独立项
- [ ] 5.17（B）`terminal.emulator.ui.ImePopupPixelInstrumentedTest#imeCommitChineseTextGridded`（18.376s）
      — 断言「中文提交必须落格」→ 渲染自愈后复测（IME 弹起与输入注入链路）；仍失败则取 input_method 日志单独立项
- [ ] 5.18（B）`terminal.emulator.ui.MultiTapSelectionInstrumentedTest#doubleTapSelectsWordAndCopyFillsClipboard`（22.996s）
      — 断言「IME 必须弹起（20s 未可见）」→ 先定位「IME 未弹起」原因（可能与渲染失效同源），再复测
- [ ] 5.19（B）`terminal.emulator.ui.MultiTapSelectionInstrumentedTest#tripleTapSelectsLine`（23.791s）
      — 断言「标记必须落格: MTAPA_35270 MTAPB_35270」→ 渲染自愈后复测（IME 弹起与输入注入链路）；仍失败则取 input_method 日志单独立项
- [ ] 5.20（C）`terminal.emulator.ui.Osc52ClipboardInstrumentedTest#osc52_sequence_sets_system_clipboard`（17.333s）
      — 断言「clipboard never received OSC52 marker OSC52_ALIVE_56409 (got: )」→ 与已实证的既有失败 `NativeBridgeSmokeTest#feedTerminal OSC52 …` 同族，另开剪贴板 change
- [ ] 5.21（C）`terminal.emulator.ui.PasteButtonInstrumentedTest#pasteMenuTypesClipboardIntoShell`（19.334s）
      — 断言「shell 回显链必须健康 (探针=Q571)」→ Shell/VT 项另开 change（本 change 解耦）
- [ ] 5.22（E）`terminal.emulator.ui.SessionDrawerInstrumentedTest#addSwitchAndCloseSession`（15.382s）
      — 断言「必须切回首个会话」→ 单独诊断：会话切换与 surface attach/detach 的关系（可能与本 change 相关）
- [ ] 5.23（C）`terminal.emulator.ui.ShellPtyInstrumentedTest#shellBellReportsEvent`（20.253s）
      — 断言「BEL 振铃事件必须上报: 16」→ Shell/VT 项另开 change（本 change 解耦）
- [ ] 5.24（C）`terminal.emulator.ui.StickyCtrlInterruptInstrumentedTest#stickyCtrlPlusCInterruptsRunningCommand`（23.942s）
      — 断言「CTRL+c 必须产生真实 ^C 中断（rc=130 未出现）, 实际尾部: 4795」→ 网格确有内容（消息尾部含提示符），属 Shell/VT，另开 change
- [ ] 5.25（A）`terminal.emulator.ui.VisualInlineVerificationTest#verifyUrlSelectionPositions`（30.010s）
      — 断言「Expected >=2 handles for URL, found 0」→ 渲染自愈后复测；用例前置断言改为「先确认画面有墨迹/有手柄」
- [ ] 5.26（A）`terminal.emulator.ui.VisualInlineVerificationTest#verifyWordSelectionPositions`（28.793s）
      — 断言「Expected >=2 selection handles, found 0」→ 渲染自愈后复测；用例前置断言改为「先确认画面有墨迹/有手柄」
- [ ] 5.27（C）`terminal.emulator.ui.VtCorrectnessInstrumentedTest#bellEventIsReportedViaVtFeed`（15.292s）
      — 断言「BEL 振铃事件必须上报: 32」→ Shell/VT 项另开 change（本 change 解耦）

## 6. 验证与文档

- [ ] 6.1 `cargo fmt` / `clippy` / `test`、`testDebugUnitTest`、`spotless`、`detekt`、
      `lintDebug` / `lintVitalRelease`、`semgrep`、`markdownlint`
- [ ] 6.2 真机/模拟器实测：人为制造 surface 失效（2.x 注入或系统回收）后自愈，
      记录恢复耗时与日志
- [ ] 6.3 全量 `:app:connectedDebugAndroidTest` 复跑，逐例回填第 5 节结论，
      失败数从 27 降到剩余未修项的真实数量，且 `abandoned` 之后不再持续 `count=-1`
- [ ] 6.4 更新 `openspec/specs/render-stability/spec.md`，完成后归档本 change
