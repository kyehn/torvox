## MODIFIED Requirements

### Requirement: 备用屏下输入法随窗口自适应（备用屏激活时输入法不位移终端 Surface）

备用屏（helix/vim/less 等全屏 TUI）激活时，网格高度 MUST 把输入法遮挡计入，
使 `rows = floor((surface − ModifierBar − 输入法遮挡) / cellHeight)`：
窗口是 `adjustNothing` 时 Surface 尺寸不变，备用屏应用按整屏行数布局，
键盘遮住的末行（含状态行）将永久不可见且应用收不到 SIGWINCH 而不会重排。
该重排 MUST 防抖到输入法高度稳定后执行（MUST NOT 逐帧发 SIGWINCH），
并 MUST 只重算网格、MUST NOT 重配交换链。

该扣减 MUST 走运行期的**单一来源**（`TerminalRuntime.imeGridReserve`），三条网格
重算路径——字号/字族变化的 `recomputeGridFromFontMetrics`、视图尺寸变化的
`recomputeRowsColsImmediate`（含度量未就绪时的兜底反推）与 `applyGridResize`——
MUST 共用它。任一条漏扣都会让备用屏在改字号或捏合缩放后被撑回被键盘遮住的高度，
且此后无触发点自愈。离开备用屏时 MUST 同样执行一次重排（此时扣减量为 0，网格按整屏
高度复原）：PTY 停留在被输入法缩小后的行数会让 shell 按残缺网格排版且不自愈。
网格重排 MUST NOT 因此在主屏暂停渲染——键盘动画期间平台逐帧派发 insets，暂停会被
一直续到动画结束，终端停止上帧。

输入法遮挡高度 MUST 取自平台 insets 派发（`ViewCompat.setOnApplyWindowInsetsListener`
装在终端 Surface 上）。此前实现是两条合成通道——Compose `WindowInsets.ime` 叶节点
与轮询 `rootWindowInsets`，各自与对方取最大值——该组合的真正问题是在 insets 遍历内
写组合状态：写入不保证被观察到（实测每 300ms 写一次，20 次才换来一次重组），
位移因此从未发生。派发是平台自己的分发路径，已挂载视图必然收到。
键盘可见性翻转时关闭选区手柄与菜单的逻辑 MUST 放在该监听器内，MUST NOT 放在
`onApplyWindowInsets` 重写里：框架在视图装了 `OnApplyWindowInsetsListener` 后
只调它而不再调用重写方法，放在重写里即静默失效。

同一场景下终端 Surface 的平移量 MUST 为 0（备用屏内容顶对齐渲染，平移会破坏
触摸行与视觉行的对位）。主屏 MUST 保持既有位移语义且 MUST NOT 因输入法改变
`rows`/`cols`：位移量按内容下沿裁剪，平移后底部像素与平移前完全相同。

主屏的跟随位移 MUST 由终端 Surface 自身的 `translationY` 承担，MUST NOT 经组合
容器平移：组合平移要经「重组 → 重新测量 → 重新布局」，而重组只在 Choreographer
帧回调里跑；主线程每帧阻塞在 `syncAndDrawFrame` 等待渲染线程时，重组滞后可达十几秒
（实测每 300ms 写一次组合状态，20 次才换来一次重组），期间键盘已弹出而终端内容与
键栏纹丝不动。键栏是组合覆盖层，MUST 读同一个 `imeInsetFlow` 上移，MUST NOT 另取
来源，也 MUST NOT 与终端 Surface 平移两次。两者取值同源故必然一致；生效时刻不同
（`translationY` 在同一拍内生效，键栏要等一次重组），键盘动画期间终端先于键栏上移。

平移量的每个输入变化 MUST 触发重算：输入法遮挡、视口尺寸（旋转，Activity 声明
`configChanges` 不重建）、单元格度量（字号与捏合缩放）、内容下沿、备用屏状态。
视图 detach MUST 取消上述订阅并复位平移量与遮挡高度，且 MUST 同时归零运行期的
`imeInsetFlow`：订阅跑在 `viewModelScope` 上（跟宿主而非视图），保留会让旧视图被
协程钉住，且 detach 后的备用屏翻转仍会命中网格重排——此时 `View.postDelayed`
落进 `mRunQueue`、只在重新 attach 时被 drain，被丢弃的旧视图永不 attach，
配对的「恢复渲染」永不执行，共享渲染器被永久暂停（终端全黑）。只清视图内的字段
会让新视图的首次派发算出 0、与它自己的字段相等而被门控挡下，运行期永久保留旧键盘
高度。

#### Scenario: 主线程被渲染阻塞时位移仍即时

- **WHEN** 每帧绘制都阻塞在等待渲染线程，且输入法在弹出
- **THEN** 终端 Surface 的平移量在 insets 派发的同一拍内生效，不等组合重组

#### Scenario: 备用屏弹键盘后网格收缩并触发重排

- **WHEN** 全屏 TUI 运行中弹出输入法且高度稳定
- **THEN** 网格行数收缩到可见高度能容纳的行数，列数不变，应用收到 SIGWINCH 后重绘

#### Scenario: 键盘已展开时启动 TUI 也重排

- **WHEN** 输入法已展开，随后从 shell 启动全屏 TUI
- **THEN** 网格立即收缩到可见高度

#### Scenario: 键盘保持展开时离开备用屏复原行数

- **WHEN** 全屏 TUI 运行中输入法仍展开，应用退出到主屏
- **THEN** 网格行数立即回到整屏高度容纳的行数，不停留在被输入法缩小后的值

#### Scenario: 收起输入法后网格复原

- **WHEN** 输入法收起
- **THEN** 网格行数回到弹出前的值

#### Scenario: 主屏不因输入法改变网格

- **WHEN** 主屏 shell 会话中弹出输入法
- **THEN** `rows`/`cols` 不变，终端内容按内容下沿裁剪平移，底部像素与平移前相同

#### Scenario: 备用屏改字号后仍按可见高度排

- **WHEN** 备用屏下输入法已展开，随后改变字号
- **THEN** 网格仍等于可见高度容纳的行数，不被撑回被键盘遮住的高度

#### Scenario: 旋转后平移量按新视口高度重算

- **WHEN** 输入法已展开时旋转设备（同一视图实例尺寸改变）
- **THEN** 平移量按新的视口高度重算，不停留在旧高度的结果上

#### Scenario: 换视图后不残留旧键盘高度

- **WHEN** 键盘展开时 Surface 判死导致视图被替换
- **THEN** 新视图接管后键栏与网格都不带旧键盘高度，首次 insets 派发即可自愈

#### Scenario: 选区在键盘弹出后不留陈旧弹窗

- **WHEN** 选中文本后弹出输入法
- **THEN** 选区手柄与上下文菜单被关闭，不留在弹出前的位置
