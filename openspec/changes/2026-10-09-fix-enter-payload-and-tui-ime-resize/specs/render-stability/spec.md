## MODIFIED Requirements

### Requirement: 备用屏下输入法弹出不位移终端 Surface

备用屏（helix/vim 等全屏 TUI）激活时，输入法跟随位移 MUST 为 0：此类应用恒占满
视口，任何位移都会把应用顶部推出屏幕并使视觉行与触摸换算行错位。主屏的位移公式
MUST NOT 因此改变。备用屏状态 MUST 随每帧渲染结果一并上报（与内容下沿同批），MUST NOT 由输入法定居后的独立查询提供：输入法弹出动画期间该状态会翻转
（启动 helix 的同时键盘正收起），查询所得缓存必然滞后于当帧的位移计算。

**位移为 0 MUST 同时以网格收缩兑现「适应窗口大小」**：窗口是 `adjustNothing`，
Surface 尺寸全程不变，故备用屏下 MUST 把输入法遮挡计入网格高度
（`rows = floor((surface − ModifierBar − 输入法遮挡) / cellHeight)`），使全屏 TUI
收到 SIGWINCH 并按可见高度重绘。仅位移为 0 而网格不变会让键盘遮住的末行
（helix/vim 的状态行即在其中）永久不可见。该重排 MUST 防抖到输入法高度稳定后执行
（MUST NOT 逐帧发 SIGWINCH），MUST 只重算网格，MUST NOT 重配交换链。主屏 MUST
不扣输入法遮挡：它靠位移跟随，改网格会带来重排闪烁与底部行丢失。

该扣减 MUST 走运行期的**单一来源**（`TerminalRuntime.imeGridReserve`），三条网格
重算路径——字号/字族变化触发的 `recomputeGridFromFontMetrics`、视图尺寸变化的
`recomputeRowsColsImmediate`（含度量未就绪时的兜底反推）与 `applyGridResize`——
MUST 共用它。任一条漏扣都会让备用屏在改字号或捏合缩放后被撑回被键盘遮住的高度，
且此后无触发点自愈。离开备用屏时 MUST 同样执行一次重排（此时扣减量为 0，网格按
整屏高度复原）：PTY 停留在被输入法缩小后的行数会让 shell 按残缺网格排版且不自愈。
该重排 MUST NOT 因此在主屏暂停渲染——键盘动画期间平台逐帧派发 insets，暂停会被
一直续到动画结束，终端停止上帧。

输入法遮挡高度 MUST 取自平台 insets 派发（`ViewCompat.setOnApplyWindowInsetsListener`
装在终端 Surface 上）。此前实现是两条合成通道——Compose `WindowInsets.ime` 叶节点
与轮询 `rootWindowInsets`，各自与对方取最大值——该组合的真正问题是在 insets 遍历内
写组合状态：写入不保证被观察到（实测每 300ms 写一次，20 次才换来一次重组），
位移因此从未发生。派发是平台自己的分发路径，已挂载视图必然收到。
键盘可见性翻转时关闭选区手柄与菜单的逻辑 MUST 放在该监听器内，MUST NOT 放在
`onApplyWindowInsets` 重写里：框架在视图装了 `OnApplyWindowInsetsListener` 后
只调它而不再调用重写方法，放在重写里即静默失效。

**终端 Surface 的跟随位移 MUST 由视图自身的 `translationY` 承担，MUST NOT 经组合容器
平移**：组合平移要经「重组 → 重新测量 → 重新布局」，而重组只在 Choreographer 帧回调
里跑；主线程每帧阻塞在 `syncAndDrawFrame` 等待渲染线程时，重组滞后可达十几秒（实测
每 300ms 写一次组合状态，20 次才换来一次重组），期间键盘已弹出而终端内容与键栏纹丝
不动。`translationY` 在 insets 派发的同一拍内生效。键栏是组合覆盖层，只能读同一个
`imeInsetFlow` 上移——它 MUST NOT 另取来源，也 MUST NOT 与终端 Surface 平移两次。
两者取值同源故必然一致；生效时刻不同（`translationY` 同一拍内生效，键栏要等一次
重组），键盘动画期间终端先于键栏上移。

遮挡高度的**发布者** MUST 唯一，且离场方 MUST 按发布者（而非按数值）归零：同一
窗口里的新旧 Surface 看到同一个输入法高度，数值天然相同，按值判断会把新 Surface
刚发布的真实高度误当成「自己还是最后一个发布者」而覆盖成 0；此后新 Surface 的
「值变化才发布」门控永不触发，运行期永久停在 0（键栏错位、备用屏网格恢复整屏行数
且不自愈）。

平移量的每个输入变化 MUST 触发重算：输入法遮挡、视口尺寸（旋转，Activity 声明
`configChanges` 不重建）、单元格度量（字号与捏合缩放）、内容下沿、备用屏状态。
订阅 MUST 在每次 attach 时重建（detach 已停掉它们，而视图复用不重走 viewModel 注入）。

两个防抖窗口（交换链重配与输入法网格重排）MUST NOT 共用一个非计数的渲染暂停
布尔：两者窗重叠时先结束的那个会把暂停清掉，另一个仍在等稳定尺寸，陈旧缓冲被拉伸。
MUST 用计数，并 MUST 在替换/取消一个在途防抖时先归还它持有的那一次——被
`removeCallbacks` 丢弃的 runnable 永不执行，它领到的暂停也就永不归还，计数单调
增长，共享渲染器被**永久**暂停（终端全黑）。键盘弹出动画期间平台逐帧派发 insets，
每个动画帧都会走一遍这条替换路径。

视图 detach MUST 取消上述订阅与两个在途防抖、复位平移量与遮挡高度，且 MUST
按发布者归零运行期的 `imeInsetFlow`：订阅跑在 `viewModelScope` 上（跟宿主而非
视图），保留会让旧视图被协程钉住，且 detach 后的备用屏翻转仍会命中网格重排——
此时 `View.postDelayed` 落进 `mRunQueue`、只在重新 attach 时被 drain，被丢弃的
旧视图永不 attach，配对的「恢复渲染」永不执行。只清视图内的字段会让新视图的首次
派发算出 0、与它自己的字段相等而被门控挡下，运行期永久保留旧键盘高度。

#### Scenario: 键盘动画期间渲染不永久停摆

- **WHEN** 备用屏下输入法弹出动画期间平台逐帧派发 insets，防抖被连续替换数十次
- **THEN** 动画结束后渲染恢复；停留期间终端不黑屏（计数归零后 `setRenderPaused(false)`
  必须被调用）

#### Scenario: 换 Surface 后新视图的遮挡高度不被旧视图清掉

- **WHEN** 键盘展开时 Surface 被判死而重建，新 Surface 已发布真实遮挡高度，
      旧 Surface 随后 detach
- **THEN** 运行期仍持有新 Surface 发布的高度，键栏位置与网格扣减量正确

#### Scenario: 主线程被渲染阻塞时位移仍即时

- **WHEN** 每帧绘制都阻塞在等待渲染线程，且输入法在弹出
- **THEN** 终端 Surface 的平移量在 insets 派发的同一拍内生效，不等组合重组

#### Scenario: 旋转后平移量按新视口高度重算

- **WHEN** 输入法已展开时旋转设备（Activity 不重建，同一视图实例尺寸改变）
- **THEN** 平移量按新的视口高度重算，不停留在旧高度的结果上

#### Scenario: 备用屏改字号后仍按可见高度排

- **WHEN** 备用屏下输入法已展开，随后改变字号
- **THEN** 网格仍等于可见高度容纳的行数，不被撑回被键盘遮住的高度

#### Scenario: 备用屏弹出输入法后网格收缩并触发重排

- **WHEN** helix 处于备用屏且输入法弹出并稳定
- **THEN** 网格行数收缩到可见高度能容纳的行数、列数不变，helix 重绘后状态行可见

#### Scenario: 备用屏弹出输入法不隐藏顶部

- **WHEN** helix 处于备用屏且输入法弹出，网格内容填满视口
- **THEN** 终端 Surface 位移为 0，helix 首行仍在屏幕内可见

#### Scenario: 键盘已展开时启动 TUI 也重排

- **WHEN** 主屏 shell 会话中输入法已展开，随后启动 helix
- **THEN** 网格立即收缩到可见高度，helix 按新行数布局

#### Scenario: 键盘保持展开时离开备用屏复原行数

- **WHEN** helix 处于备用屏且输入法仍展开，应用退出到主屏
- **THEN** 网格行数立即回到整屏高度容纳的行数，不停留在被输入法缩小后的值

#### Scenario: 收起输入法后网格复原

- **WHEN** 输入法收起
- **THEN** 网格行数回到弹出前的值

#### Scenario: 换视图后不残留旧键盘高度

- **WHEN** 键盘展开时 Surface 判死导致视图被替换
- **THEN** 新视图接管后键栏与网格都不带旧键盘高度，首次 insets 派发即可自愈

#### Scenario: 选区在键盘弹出后不留陈旧弹窗

- **WHEN** 选中文本后弹出输入法
- **THEN** 选区手柄与上下文菜单被关闭，不留在弹出前的位置

#### Scenario: 主屏位移公式不变

- **WHEN** 终端处于主屏且内容填满网格
- **THEN** 位移仍为整块键盘高度，与备用屏判定无关；`rows`/`cols` 不变，
      上移后底部像素与上移前完全相同
