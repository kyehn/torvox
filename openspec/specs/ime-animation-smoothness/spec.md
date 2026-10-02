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

### Requirement: 终端与修饰键栏位移同源同帧

输入法弹出时终端 Surface 与修饰键栏 MUST 只以**同一个**合成 ime 状态为唯一位移来源，
且两个位移值 MUST 在**同一帧 placement** 中求值。两者 MUST NOT 各自持有独立位移来源
（叶节点 vs view 监听、后写覆盖）：两个位移源取值不一致（如一个跟随 live insets、
另一个跟随 settled 值）即表现为持续闪烁；写入布局状态位于 insets dispatch 遍历内
即构成自激振荡。键栏覆盖在 Surface 底部，其高度已由网格按同一口径预留，故位移
不改变 Surface 尺寸，网格不重排、无 SIGWINCH。

#### Scenario: 键栏底边恒等于键盘顶边

- **WHEN** 键盘弹出完成定居
- **THEN** 修饰键栏底边像素与键盘顶边像素相等（同屏实测 `y=1516/1517` 相接），
      无空隙无重叠

#### Scenario: 位移源唯一

- **WHEN** 键盘动画期间
- **THEN** insets 仅由 `WindowImeBottomPx` 叶节点与 `rootWindowInsets` 轮询取大者写入
      `imeBottomPx`，位移经 placement 期 offset/布局 lambda 应用；MUST NOT 再挂
      SurfaceView 的 `OnApplyWindowInsetsListener`——它在 insets dispatch 遍历中读到
      尚未更新的 `ime=0`，写入布局状态又触发新一轮 dispatch，形成自激振荡

### Requirement: 终端位移按内容下沿裁剪

输入法弹出时终端 Surface 的位移量 MUST 等于「键盘遮住且放不下的内容高度」，
即 `max(0, contentBottomPx − (surfaceHeightPx − barHeightPx − imeBottomPx))` 且
MUST NOT 超过 `imeBottomPx`，MUST NOT 无条件取整个 imeBottom。网格自顶端锚定渲染，
键盘遮挡的是网格**末尾**行：无条件按 imeBottom 平移会把稀疏会话（提示符在首行）
整体推出屏幕上边界，终端区表现为全空（实测提示符由 y=134 落到 y=−686）。内容下沿
MUST 取「视口内最后一个有内容的行」下沿（空格/制表/NUL 不计内容），MUST NOT 取
光标行——光标隐藏或滚出视口时无坐标可用，且光标在顶行时其下方内容会被吞。

该裁剪等价于 Termux `adjustResize` 会砍掉的那部分高度：稀疏会话 shift = 0
（内容每像素原位），内容占满网格时 shift 恒等于 imeBottom（末行紧贴键栏顶边）。

#### Scenario: 内容较少时终端不移动

- **WHEN** 会话只有少量内容（如首行提示符）时点击终端弹出输入法
- **THEN** 终端 Surface 位移为 0，提示符像素位置与弹出前完全相同且位于键盘上方可见

#### Scenario: 内容较多时底部不被吞

- **WHEN** 内容占满网格时点击终端弹出输入法
- **THEN** 终端 Surface 位移恒等于 imeBottom，末行紧贴键栏顶边、弹出前后底部像素相同

#### Scenario: 下沿取自渲染帧而非光标

- **WHEN** 应用光标隐藏（`tput civis`）或视口已滚入回滚区
- **THEN** 内容下沿仍取自渲染帧的视口内容（有内容时 shift = imeBottom），
      MUST NOT 因无光标坐标而退化为整体平移或零位移

### Requirement: 位移源不得自激振荡

键入/隐藏动画期间位移值 MUST 收敛静止。写入布局状态 MUST NOT 位于会反过来触发
该写入的回调内（insets dispatch 遍历），否则构成反馈环。

#### Scenario: 定居后位移静止

- **WHEN** 键盘保持打开并持续输出
- **THEN** 连续 8 帧截图中，修饰键栏所在条带逐帧一致；差异只允许出现在系统状态栏
      时钟等无关区域（MUST NOT 出现键栏条带整块跳变）
