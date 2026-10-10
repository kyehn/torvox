## REMOVED Requirements

### Requirement: insets 读取隔离（动画帧不整屏重组）

组合叶节点（`WindowImeBottomPx`）读取、`rootView.rootWindowInsets` 轮询与
`IME_SETTLE_FRAMES` 定居节流一并移除。它们的取值正确，但终点是组合状态写入而这样的
写入不保证被观察到（实测每 300ms 写一次组合状态，20 次才换来一次重组），位移因此从未
发生。替代要求见「位移不经组合往返」。

### Requirement: 光标可见时终端不上抬

按光标行裁剪平移量（`computeTerminalPanPx` 与 `cursorRowFlow`）的机制移除：光标隐藏
或在视口外时无坐标可用，且光标在顶行时其下方内容会被吞。裁剪基准改为渲染帧的视口内容
下沿，替代要求见「内容稀疏时终端不上抬」。

### Requirement: 终端与修饰键栏位移同源同帧

「两个位移值 MUST 在同一帧 placement 中求值」「MUST NOT 挂 SurfaceView 的
`OnApplyWindowInsetsListener`」一并移除：前者与「终端 Surface 的位移由视图属性承担」
不可兼得（要求同帧只会诱使实现把位移搬回组合路径），后者断言的恰是实现确认可靠的
那条通道。保留「同一来源」这一半，见新增的「位移来源唯一且生效时刻可不同」。

### Requirement: 位移源不得自激振荡

「写入布局状态 MUST NOT 位于 insets dispatch 遍历内」移除：终端 Surface 的位移
MUST 在该遍历内同步生效（组合往返在主线程被渲染阻塞时会滞后十几秒）。真正的防振荡
约束改为「不得写会反过来触发 insets 派发的状态」，见新增的「位移写入不得触发新的
insets 派发」。

### Requirement: IME 动画不触发暂停链与交换链重建

「输入法动画 MUST NOT 触发 `setRenderPaused`」的绝对表述移除：备用屏下按可见高度重排
网格 MUST 在防抖窗内暂停渲染，否则新旧行数交替渲染会被用户看见。替代要求见
「输入法动画不重建交换链，主屏不暂停渲染」。

## ADDED Requirements

### Requirement: 位移不经组合往返

键盘动画逐帧 insets 变化 MUST NOT 触发主组合整屏重组，且终端 Surface 的位移 MUST NOT
经组合往返：insets 读取限定在 `TerminalSurface` 的平台 insets 派发回调，位移直接写
视图属性（`translationY`）。

组合叶节点读取（`WindowInsets.ime.getBottom()`）与 `rootView.rootWindowInsets` 轮询
两条通道 MUST NOT 存在：它们的终点是组合状态写入，在重组滞后时不生效。派发是平台自己的
分发路径，已挂载视图必然收到，且回调直接改视图属性时在同一拍内生效。

遮挡高度 MUST 按每一帧派发的最新值更新，MUST NOT 存在「静置 N 帧后冻结」的节流：
冻结会让隐藏动画在静置瞬间跳变。

#### Scenario: 主线程被渲染阻塞时位移仍即时

- **WHEN** 每帧绘制都阻塞在等待渲染线程，且输入法在弹出
- **THEN** 终端 Surface 的平移量在 insets 派发的同一拍内生效，不等组合重组

### Requirement: 位移来源唯一且生效时刻可不同

输入法弹出时终端 Surface 与修饰键栏 MUST 只以**同一个**输入法遮挡高度为唯一位移
来源（`imeInsetFlow`：由 `TerminalSurface` 的平台 insets 派发发布，两者都读它）。
两者 MUST NOT 各自持有独立位移来源（叶节点 vs view 监听、后写覆盖）：两个位移源
取值不一致（如一个跟随 live insets、另一个跟随 settled 值）即表现为持续闪烁。

生效时刻 MUST NOT 要求相同：终端 Surface 由视图属性承担，在 insets 回调内同一拍
生效；键栏由组合 `offset` 承担，要等一次重组。键栏覆盖在 Surface 底部，其高度已由
网格按同一口径预留，故位移不改变 Surface 尺寸，主屏网格不重排。

#### Scenario: 键栏底边恒等于键盘顶边

- **WHEN** 键盘弹出完成定居
- **THEN** 修饰键栏底边像素与键盘顶边像素相等（同屏实测 `y=1516/1517` 相接），
      无空隙无重叠

#### Scenario: 位移源唯一

- **WHEN** 键盘动画期间
- **THEN** insets 仅由 `TerminalSurface` 上的 `OnApplyWindowInsetsListener` 读取并
      发布到 `imeInsetFlow`，终端 Surface 的位移直接写 `translationY`；MUST NOT
      另有组合叶节点读取或 `rootWindowInsets` 轮询

### Requirement: 位移写入不得触发新的 insets 派发

键入/隐藏动画期间位移值 MUST 收敛静止。insets 派发回调内 MUST NOT 写入会反过来
触发该派发的状态（组合/布局状态会经重组再触发一轮派发，构成反馈环）；写**视图属性**
与运行期的 StateFlow 不触发派发，是该回调内允许的写入。

#### Scenario: 定居后位移静止

- **WHEN** 键盘保持打开并持续输出
- **THEN** 连续 8 帧截图中，修饰键栏所在条带逐帧一致；差异只允许出现在系统状态栏
      时钟等无关区域（MUST NOT 出现键栏条带整块跳变）

### Requirement: 动画期间键栏逐帧跟随

键盘动画逐帧 insets 变化 MUST NOT 触发主组合整屏重组，位移 MUST 随每一帧派发更新。

#### Scenario: 动画帧只有键栏重组

- **WHEN** 键盘动画期间 insets 每帧变化
- **THEN** 终端 Surface 的平移量在 insets 回调内同拍写入 `translationY`，
      键栏随 `imeInsetFlow` 重组

#### Scenario: 位移值随每一帧派发更新

- **WHEN** 键盘隐藏动画期间 insets 逐帧减小
- **THEN** 位移逐帧跟到 0，MUST NOT 停在某个中间值

### Requirement: 输入法动画不重建交换链，主屏不暂停渲染

输入法弹出/隐藏动画及定居 MUST NOT 触发 `attachWindow` 或交换链重建（窗口是
`adjustNothing`，Surface 尺寸不变）。渲染循环 cadence 由既有 idle-clock 语义驱动
（动画后 5s 活跃、随后 idle latch 回落）。

**主屏**下的输入法动画 MUST NOT 触发 `setRenderPaused`：那里的遮挡计为 0，网格重排
是 no-op，而键盘动画期间平台逐帧派发 insets，每次都重新起算防抖，暂停会一直挂到动画
结束、终端停止上帧。**备用屏**下按可见高度重排网格 MAY 触发一对配对的
`setRenderPaused`（防抖窗内，48ms），窗内的新旧行数 MUST NOT 被呈现。

#### Scenario: 主屏动画全程无暂停链调用

- **WHEN** 主屏上弹出与隐藏输入法各一次并完成定居
- **THEN** logcat 无 `setRenderPaused` / `attachWindow` / `applySurfaceResize` 记录

#### Scenario: 备用屏重排期间不出帧

- **WHEN** 备用屏下输入法弹出，防抖窗内新旧行数交替
- **THEN** 该窗内不上帧；窗结束后恢复渲染，且恢复 MUST 恰好发生一次
