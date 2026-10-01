# ime-animation-smoothness Specification

## Purpose

输入法弹出/隐藏动画流畅性。取证（Android 15 x86_64 模拟器，guest GPU，logcat
与 OCR 实测）：IME show/hide 全程零 `setRenderPaused` / `attachWindow` /
`applySurfaceResize`——暂停链未参与动画；渲染循环在动画及此后 5s 内跑 17ms
活跃 latch（≈62fps，帧本体 0ms、无新输出），随后回落 500ms idle latch——
热泵属设计内行为。结构性成本是 `TerminalScreen` 组合体逐帧读
`WindowInsets.ime.getBottom()`：键盘动画每帧 insets 变化都整屏重组（终端
Column + 修饰键栏 + 搜索层），主线程动画帧成本随 UI 树规模增长。

## Requirements

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
- **THEN** 只有 `WindowImeBottomPx` 节点重组并把新值写入状态，位移容器/
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

### Requirement: 终端与修饰键栏同属一个位移容器

输入法弹出时终端 Surface 与修饰键栏 MUST 处于同一个平移容器内，由**同一次**
offset 施加**同一个**位移值。两者 MUST NOT 各自持有独立位移来源：双位移在屏幕上
互压即表现为内容重叠；两个位移源取值不一致（如一个跟随 live insets、另一个跟随
settled 值）即表现为持续闪烁。键栏覆盖在 Surface 底部，其高度已由网格按同一口径
预留，故合并不改变 Surface 尺寸，网格不重排、无 SIGWINCH。

#### Scenario: 键栏底边恒等于键盘顶边

- **WHEN** 键盘弹出完成定居
- **THEN** 修饰键栏底边像素与键盘顶边像素相等（同屏实测 `y=1516/1517` 相接），
      终端末行紧贴键栏顶边，无空隙无重叠

#### Scenario: 位移源唯一

- **WHEN** 键盘动画期间
- **THEN** insets 仅由 `WindowImeBottomPx` 叶节点读取并写入 `imeBottomPx`，
      位移容器与键栏同读该状态；MUST NOT 再挂 SurfaceView 的
      `OnApplyWindowInsetsListener`——它在 insets dispatch 遍历中读到尚未更新的
      `ime=0`，写入布局状态又触发新一轮 dispatch，形成自激振荡

### Requirement: 位移源不得自激振荡

键入/隐藏动画期间位移值 MUST 收敛静止。写入布局状态 MUST NOT 位于会反过来触发
该写入的回调内（insets dispatch 遍历），否则构成反馈环。

#### Scenario: 定居后位移静止

- **WHEN** 键盘保持打开并持续输出
- **THEN** 连续 8 帧截图中，修饰键栏所在条带逐帧一致；差异只允许出现在系统状态栏
      时钟等无关区域（MUST NOT 出现键栏条带整块跳变）
