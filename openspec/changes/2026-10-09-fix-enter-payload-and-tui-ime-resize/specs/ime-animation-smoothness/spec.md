## MODIFIED Requirements

### Requirement: insets 读取隔离（动画帧不整屏重组）

键盘动画逐帧 insets 变化 MUST NOT 触发主组合整屏重组，且终端 Surface 的位移
MUST NOT 经组合往返：insets 读取限定在 `TerminalSurface` 的平台 insets 派发回调，
位移直接写视图属性（`translationY`），状态变化只重排布局、不重组终端/键栏/搜索层
子树。

组合叶节点读取（`WindowInsets.ime.getBottom()`）与 `rootView.rootWindowInsets` 轮询
两条通道 MUST 一并删除：它们的取值正确但**生效时机**失效——终点是组合状态写入，
而这样的写入不保证被观察到（实测每 300ms 写一次组合状态，20 次才换来一次重组），
位移因此从未发生（`ImePopupPixelInstrumentedTest` 三个用例即以此判红：位移=0）。
派发是平台自己的分发路径，已挂载视图必然收到。

#### Scenario: 动画帧只有键栏重组

- **WHEN** 键盘动画期间 insets 每帧变化
- **THEN** 终端 Surface 的平移量在 insets 回调内同拍写入 `translationY`，
      键栏随 `imeInsetFlow` 重组，终端/搜索层不逐帧重组

#### Scenario: 位移值随每一帧派发更新

- **WHEN** 键盘动画期间 insets 每帧变化
- **THEN** 遮挡高度按每帧的最新值更新，MUST NOT 停留在定居帧的结果上

### Requirement: 终端与修饰键栏位移同源同帧

输入法弹出时终端 Surface 与修饰键栏 MUST 只以**同一个**输入法遮挡高度为唯一位移
来源（`imeInsetFlow`：由 `TerminalSurface` 的 insets 派发发布，两者都读它）。
两者 MUST NOT 各自持有独立位移来源（叶节点 vs view 监听、后写覆盖）：两个位移源
取值不一致（如一个跟随 live insets、另一个跟随 settled 值）即表现为持续闪烁。

两个位移值 MUST NOT 在同一帧 placement 中求值——终端 Surface 由视图属性承担，
键栏由组合 `offset` 承担，二者的生效机制本就不同；要求同帧只会诱使实现把位移
搬回组合路径。取值一致即一致，生效时刻不同（Surface 同一拍、键栏等一次重组）
是这条分工的直接后果。键栏覆盖在 Surface 底部，其高度已由网格按同一口径预留，
故位移不改变 Surface 尺寸，主屏网格不重排、无 SIGWINCH。

#### Scenario: 键栏底边恒等于键盘顶边

- **WHEN** 键盘弹出完成定居
- **THEN** 修饰键栏底边像素与键盘顶边像素相等（同屏实测 `y=1516/1517` 相接），
      无空隙无重叠

#### Scenario: 位移源唯一

- **WHEN** 键盘动画期间
- **THEN** insets 仅由 `TerminalSurface` 上的 `OnApplyWindowInsetsListener` 读取并
      发布到 `imeInsetFlow`，终端 Surface 的位移直接写 `translationY`；MUST NOT
      另有组合叶节点读取或 `rootWindowInsets` 轮询——它们的终点是组合状态写入，
      在重组滞后时不生效

### Requirement: 光标可见时终端不上抬

键盘打开但光标行本就在可见区（最小平移为 0）时，终端内容 MUST NOT 整体上抬
（稀疏会话防黑屏）；内容下沿被键盘遮挡时才按内容裁剪出的最小平移抬升到可见区，
平移与修饰键栏预留严格一致。内容下沿 MUST 取自渲染帧的视口内容，
MUST NOT 取光标行（光标隐藏或滚出视口时无坐标可用，且光标在顶行时其下方内容
会被吞），且 MUST 在渲染计数为 0 的空闲帧同样上报；采样不得阻塞渲染线程。

#### Scenario: 顶部提示符原位

- **WHEN** 提示符位于顶部且键盘弹出
- **THEN** 终端内容保持原位（pan=0），仅修饰键栏上移

#### Scenario: 底部内容抬升

- **WHEN** 视口内容下沿被键盘遮挡
- **THEN** 终端区按内容裁剪平移，末行停在修饰键栏上方

#### Scenario: 空闲帧仍上报内容下沿

- **WHEN** 输出停止后弹出输入法（渲染计数为 0 的空闲帧）
- **THEN** `lastContentRowFlow` 仍为真实内容下沿，内容较多时终端按最小平移上移
