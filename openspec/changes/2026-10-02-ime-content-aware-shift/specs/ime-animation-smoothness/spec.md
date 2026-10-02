## MODIFIED Requirements

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
      `imeBottomPx`，位移经 placement 期 offset lambda 应用；MUST NOT 再挂 SurfaceView 的
      `OnApplyWindowInsetsListener`——它在 insets dispatch 遍历中读到尚未更新的
      `ime=0`，写入布局状态又触发新一轮 dispatch，形成自激振荡

## ADDED Requirements

### Requirement: 终端位移按内容下沿裁剪

输入法弹出时终端 Surface 的位移量 MUST 等于「键盘遮住且放不下的内容高度」，
即 `max(0, contentBottomPx − (surfaceHeight − barHeight − imeBottom))`，
MUST NOT 无条件取整个 imeBottom。网格自顶端锚定渲染，键盘遮挡的是网格**末尾**行：
无条件按 imeBottom 平移会把稀疏会话（提示符在首行）整体推出屏幕上边界，
终端区表现为全空（实测提示符由 y=134 落到 y=−686）。内容下沿 MUST 取「视口内最后一个
有内容的行」下沿（空格/制表/NUL 不计内容），MUST NOT 取光标行——光标隐藏或
滚出视口时无坐标可用，且光标在顶行时其下方内容会被吞。

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
