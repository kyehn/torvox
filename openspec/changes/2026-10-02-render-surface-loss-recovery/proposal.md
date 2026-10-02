# 渲染 Surface 丢失后自愈：消除仪器化测试集体黑屏

## Why

`:app:connectedDebugAndroidTest` 全量套件实测（release profile native 库，Android 15
x86_64 模拟器 emulator-5554，161 例 / 27 失败 / 0 跳过 / 22m12s，模拟器全程存活）：

```text
tests="161" failures="27" errors="0" skipped="0"    terminal.emulator.* 共 37 个类
terminal.emulator.BehaviorInstrumentedTest tests="10" failures="0"   （含 CI 失败用例）
```

27 个失败按断言性质分成 5 类（逐条取自
`android/app/build/outputs/androidTest-results/connected/debug/TEST-test_avd(AVD) - 15.xml`）：

| 类别 | 数量 | 用例与断言原文 |
| --- | --- | --- |
| A 画面未呈现 | 8 | `diag.CursorPixelAcceptanceTest#cursorBlockMatchesRenderCursorCell` → `T0-boot: 光标格必须变亮`；`diag.SgrColorPixelAcceptanceTest#sgrRedTextProducesRedPixels` → `SGR 红色文本必须产生红色像素 (前=0 最大红=0)`；`diag.SgrItalicPixelAcceptanceTest#sgrItalicTextProducesDistinctGlyphPixels` → `斜体字形像素必须与正体不同 (差分=0)`；`ImePopupPixelInstrumentedTest#contentManyImePopupMovesUpBottomIdentical` → `位移=0 差异=0`；`VisualInlineVerificationTest#verifyWordSelectionPositions` → `Expected >=2 selection handles, found 0`；`#verifyUrlSelectionPositions` → `Expected >=2 handles for URL, found 0`；`SelectionEspressoTest#selectAllShowsSelectionMenu` → `Selection menu must appear after Select All`；`#copyActionPlacesTextOnClipboard` → `复制 action must be present` |
| B 输入未送达 | 9 | `ImePopupPixelInstrumentedTest#contentFewImePopupTerminalStaysPutAndVisible` → `标记必须落格: IME_FEW_88202`；`#imeCommitChineseTextGridded` → `中文提交必须落格`；`MultiTapSelectionInstrumentedTest#doubleTapSelectsWordAndCopyFillsClipboard` → `IME 必须弹起（20s 未可见）`；`#tripleTapSelectsLine` → `标记必须落格: MTAPA_35270 MTAPB_35270`；`SelectionDragQuantifiedTest` 3 例 → `标记必须落格: dragstart…/targetword…/growme…`；`ShellResponseLatencyTest` 2 例 → `burst never fully appeared` / `marker UXMARK1 never appeared on screen within 4000ms` |
| C Shell/VT/剪贴板 | 6 | `ScrollBehaviorQuantifiedTest#enter_snaps_viewport_to_bottom_within_budget` → `viewport never snapped to bottom after Enter`；`ShellPtyInstrumentedTest#shellBellReportsEvent` → `BEL 振铃事件必须上报: 16`；`VtCorrectnessInstrumentedTest#bellEventIsReportedViaVtFeed` → `BEL 振铃事件必须上报: 32`；`StickyCtrlInterruptInstrumentedTest#stickyCtrlPlusCInterruptsRunningCommand` → `CTRL+c 必须产生真实 ^C 中断（rc=130 未出现）`；`PasteButtonInstrumentedTest#pasteMenuTypesClipboardIntoShell` → `shell 回显链必须健康 (探针=Q571)`；`Osc52ClipboardInstrumentedTest#osc52_sequence_sets_system_clipboard` → `clipboard never received OSC52 marker OSC52_ALIVE_56409 (got: )` |
| D 预置资产缺失 | 2 | `installer.BootstrapCompatibilityTest` → `bootstrap failed`；`installer.TermuxBootstrapRealTerminalTest#termuxBootstrap_shell_runs_real_commands_with_asserted_output` → `bootstrap zip must be staged first: adb push <termux bootstrap-x86_64.zip> /sdcard/Download/…` |
| E 其他 | 2 | `ui.FontSizeReflowInstrumentedTest#fontSizeChangeReflowsGridAndScalesCellHeight` → `字号增大后列数必须收缩 (前=33 后=33)`；`ui.SessionDrawerInstrumentedTest#addSwitchAndCloseSession` → `必须切回首个会话` |

A 类 8 例全部是「零像素 / 零手柄 / 位移=0」，同一次运行的 logcat 给出唯一根因链
（app 进程 pid 5303，09:19:02 → 09:41:05）：

```text
attach_surface: configured                                          ← 全程仅 1 次
attach_surface: RECONFIGURE_SWAPCHAIN (fast path, existing surface)  ← 132 次
E BufferQueueProducer: SurfaceView[com.termux/…]#1(BLAST Consumer)1 query: BufferQueue has been abandoned
E vulkan: NATIVE_WINDOW_MIN_UNDEQUEUED_BUFFERS query failed: No such device (-19)
E wgpu_hal::vulkan::swapchain::native: get_physical_device_surface_capabilities: ERROR_SURFACE_LOST_KHR
E native::render::context: GPU_UNCAPTURED_ERROR: Validation { … "In Surface::configure"
      Caused by: Surface does not support the adapter's queue family }
E native::android::ffi: render: frame failed: surface creation failed: begin_frame failed
W Runtime: SLOW_FRAME session=1 render=39.659880 count=-1 newOutput=false scrollOffset=0
```

同一 pid 统计：`abandoned=6148`、`Surface::configure` 错误 `=6148`、
`ERROR_SURFACE_LOST_KHR=3074`、`begin_frame failed=3074`。即：**SurfaceView 的
BufferQueue 在套件开始后不久被遗弃，此后 22 分钟里每一帧都失败**（`count=-1`），
终端恒为空白。

根因在代码：`render/context.rs:368-372` 的 `attach_surface` 只要 `self.surface.is_some()`
就走原地 `reconfigure_swapchain` 快路径，**永不重建 wgpu Surface**；
`render/pass.rs:157-160` 对 `Lost` / `Outdated` 同样只 reconfigure。已死的
ANativeWindow 无法靠 reconfigure 复活，于是该 surface 在进程内**终身不可用**——
真机上 SurfaceFlinger 重启、屏幕热插拔也会造成同样结果（永久黑屏，直到杀进程）。

后果不止于测试：`docs/specification/TESTING.md` 覆盖范围内的像素/选区/输入法验收
用例目前拿不到任何信号（退化为一律 `位移=0 差异=0`，无法区分「实现对」与
「什么都没画」）。B 类 9 例是否同源（渲染失效导致输入法弹起动画/输入注入失败）
需在渲染恢复后复测确认，见 design.md 第 4 节。

## What Changes

- **原生侧：surface 级失败即失效缓存**。`begin_frame` / acquire 出现 surface 级失败
  （`Surface::configure` 失败、`ERROR_SURFACE_LOST_KHR`、`begin_frame failed`）时，
  把 `Renderer.surface` / `surface_config` 置为失效，使下一次 `attach_window` 走重建
  慢路径而不是快路径；重建仍失败则保持失效并继续上报，不做无意义的每帧重建。
- **原生侧：新增可观测的失效信号**。`renderWithNewOutput` 的打包返回增加
  surface 不可用状态位（复用既有返回通道，零额外 JNI 调用），供宿主决定是否换 surface。
- **宿主侧：收到失效信号后重建 surface**（摘下再挂回同一个 `SurfaceView`，
  强制 `SurfaceHolder` 交付新的 ANativeWindow）→ `surfaceCreated` →
  `attach_window` → 重建慢路径 → 渲染恢复。恢复路径必须有最小间隔与连续次数上限，
  避免抖动。
- **仪器化验收**：`diag/*PixelAcceptanceTest`、`VisualInlineVerificationTest`、
  `ImePopupPixelInstrumentedTest` 等像素用例先断言画面确实呈现、再断言行为，
  杜绝「什么都没画也算通过」。

## Non-goals

- 不改 SurfaceView 架构（不换成应用自建 `SurfaceTexture`）：更大改动，
  SurfaceView + 重建是当前架构内的最小可行修复。
- 不改 ime 位移裁剪、网格锚定、渲染节拍等既有语义（见 `ime-animation-smoothness`、
  `render-loop-scroll-cadence`）。
- 不处理 D 类（`installer.*`：需预置 `termux-bootstrap-x86_64.zip`，测试自身已如此声明）
  与 C 类中的 OSC52 剪贴板族（`Osc52ClipboardInstrumentedTest` 与
  `NativeBridgeSmokeTest#feedTerminal OSC52 …` 同族，后者已实证为改动前既有失败）。
- 不放宽任何断言、不跳过用例来「消除」失败。
