# 修复输入法弹出时稀疏内容被推出屏幕

## Why

实测（release APK，Android 15 x86_64 模拟器 1080x2400 / density 420）：

- 键盘隐藏：提示符 `$ ▉` 在 y=134-163（网格 row 0），键栏文字行 y=2184/2279。
- 点击终端弹出输入法：`dumpsys window` 报 `ime frame=[0,1517][1080,2400]`
  （高 883px），应用按 883 − 导航条 63 = 820px 整体平移「终端 Surface + 键栏」，
  键栏底边 1517 恰与键盘顶边相接（正确），而提示符落到 y = 134 − 820 = **−686**
  出屏；y 81..1364 之间**零墨迹**，键盘上方整片区域纯背景色。
- 内容较多（`seq 1 120`）时 y=128..1307 满屏内容、提示符紧贴键栏顶边，行为正确。

根因：`TerminalScreen` 的位移容器按输入法高度**无条件**平移整个内容盒，而网格
自顶端锚定渲染（`cell_builder` 的 `quad_origin.y = row × cellHeight`）且刻意不随
输入法重排（`TerminalSurface.ResizeManager` 明确「刻意不减去输入法 inset」，
`onImeSettled` 把 Surface 自身 `translationY` 清零）。平移后可见的只有内容盒
底部 `H − imeHeight` 一条，对应网格**末尾**若干行：内容多时末尾有内容（看起来
正常），内容少时内容在顶部行 → 全部出屏，只剩空白。

违反 `docs/specification/TESTING.md`「实际内容较少时输入法弹出时终端无动画、无闪烁、
无变化」与本 spec 场景「终端内容与提示符原位」。既有仪器化用例
`contentFewImePopupBarAboveKeyboardAndNoFlicker` 把稀疏会话内容上抬写成已知代价、
只断言键栏位置，故未能发现。

## What Changes

- 原生侧把「视口内最后一个有内容的行」搭载进 `renderWithNewOutput` 的打包返回
  （bit 49..63，0x7FFF = 视口全空）。数据源是本帧已渲染缓存的 CellData（已含
  视口滚动偏移），不新增发往 VT 线程的同步查询；与光标行同门：空闲帧亦采样。
- `Bridge.RenderResult` 增加该字段；`TerminalRuntime` 发布与既有 `cursorRowFlow`
  同构的 `lastContentRowFlow`（会话切换时随 `cursorRow` 一并重新初始化）。
- 位移改为**同一合成 ime 状态、同一帧 placement** 求两个值：修饰键栏仍
  `-imeBottom`（恒在键盘上方）；终端 Surface 改为
  `−max(0, contentBottomPx − (surfaceHeight − barHeight − imeBottom))`
  ——即「只平移 Termux `adjustResize` 会砍掉的那部分高度」：
  放得下的内容每像素原位（稀疏会话 shift = 0），内容占满网格时 shift 恒等于
  imeBottom（底部不被吞，键盘上方无空隙）。网格行数不变，仍无重排/SIGWINCH。
- 仪器化用例改回实测「内容较少时终端无变化」：弹出前后顶部条带逐像素一致，
  且条带确有墨迹（断言内容可见，而不只是无位移）。

## Non-goals

- 不改网格行数与输入法交互契约：输入法显示/隐藏仍不触发 `resize`/交换链重建。
- 不改 insets 读取通道（叶节点 `WindowImeBottomPx` 与 `rootWindowInsets` 取大者）
  与定居节流语义。
- 不新增「内容下沿」的同步 JNI 查询（逐行 `read_line_text` 扫描）。
- 不处理仅有背景色填充而无字形（SGR 48）行的可见性判定。
