## Purpose

输入法弹出/隐藏动画流畅性。取证（Android 15 x86_64 模拟器，guest GPU，logcat
与 OCR 实测）：IME show/hide 全程零 `setRenderPaused` / `attachWindow` /
`applySurfaceResize`——暂停链未参与动画；渲染循环在动画及此后 5s 内跑 17ms
活跃 latch（≈62fps，帧本体 0ms、无新输出），随后回落 500ms idle latch——
热泵属设计内行为。结构性成本是 `TerminalScreen` 组合体逐帧读
`WindowInsets.ime.getBottom()`：键盘动画每帧 insets 变化都整屏重组（终端
Column + 修饰键栏 + 搜索层），主线程动画帧成本随 UI 树规模增长。

## ADDED Requirements

### Requirement: IME 动画期间位移逐帧跟随

输入法弹出/隐藏动画期间，终端区与修饰键栏位移 MUST 随 insets 逐帧更新，
不卡顿、不闪烁、不跳跃、不压扁拉伸；修饰键栏 MUST 位于键盘上方不被遮挡，
键盘隐藏动画期间 MUST 逐帧跟随到 0（不得用 settled 值提前冻结）。

#### Scenario: 键盘弹出修饰键栏越过键盘

- **WHEN** 点击终端弹出输入法
- **THEN** 修饰键栏随动画逐帧上移、完整位于键盘上方，终端内容与提示符原位

#### Scenario: 键盘隐藏修饰键栏回落

- **WHEN** BACK 隐藏输入法
- **THEN** 修饰键栏随动画逐帧下移回到底部，不出现滞留或迟跳

### Requirement: insets 读取隔离（动画帧不整屏重组）

键盘动画逐帧 insets 变化 MUST NOT 触发主组合整屏重组：insets 读取限定在
叶节点（`WindowImeBottomPx`），位移经 snapshotFlow 收集器与 placement 阶段
offset lambda 应用（状态变化只重排布局、不重组终端/修饰键栏子树）。

#### Scenario: 动画帧仅叶节点重组

- **WHEN** 键盘动画期间 insets 每帧变化
- **THEN** 只有 `WindowImeBottomPx` 节点重组并把新值写入状态，终端 Column/
      修饰键栏/搜索层不逐帧重组

#### Scenario: 定居节流语义不变

- **WHEN** insets 值停止变化 `IME_SETTLE_FRAMES × 轮询间隔`（48ms）后
- **THEN** 位移锁定 settled 值并调用 `onImeSettled`，期间值变化按新帧取最新
      （与旧 keyed `LaunchedEffect` 取消-重启语义等价）

### Requirement: IME 动画不触发暂停链与交换链重建

IME 弹出/隐藏动画及定居 MUST NOT 触发 `setRenderPaused`、`attachWindow` 或
交换链重建；渲染循环 cadence 由既有 idle-clock 语义驱动（动画后 5s 活跃、
随后 idle latch 回落），不为此引入额外暂停/恢复对。

#### Scenario: 动画全程无暂停链调用

- **WHEN** 弹出与隐藏输入法各一次并完成定居
- **THEN** logcat 无 `setRenderPaused` / `attachWindow` / `applySurfaceResize`
      记录，循环 cadence 保持 idle → 活跃(≈5s) → idle

### Requirement: 光标可见时终端不上抬

键盘打开但光标行本就在可见区（最小平移为 0）时，终端内容 MUST NOT 整体上抬
（稀疏会话防黑屏）；光标被键盘遮挡时才按 `computeTerminalPanPx` 最小平移抬升
到可见区，平移与修饰键栏预留严格一致。

#### Scenario: 顶部提示符原位

- **WHEN** 提示符位于顶部且键盘弹出
- **THEN** 终端内容保持原位（pan=0），仅修饰键栏上移

#### Scenario: 底部光标抬升

- **WHEN** 光标行被键盘遮挡
- **THEN** 终端区按 `computeTerminalPanPx` 平移恰好使光标行停在修饰键栏上方
