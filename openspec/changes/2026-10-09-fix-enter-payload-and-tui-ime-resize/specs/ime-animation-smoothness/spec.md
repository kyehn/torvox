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
