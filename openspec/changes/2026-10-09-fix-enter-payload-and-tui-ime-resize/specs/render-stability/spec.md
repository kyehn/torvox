## MODIFIED Requirements

### Requirement: 备用屏下输入法随窗口自适应（备用屏激活时输入法不位移终端 Surface）

备用屏（helix/vim/less 等全屏 TUI）激活时，网格高度 MUST 把输入法遮挡计入，
使 `rows = floor((surface − ModifierBar − 输入法遮挡) / cellHeight)`：
窗口是 `adjustNothing` 时 Surface 尺寸不变，备用屏应用按整屏行数布局，
键盘遮住的末行（含状态行）将永久不可见且应用收不到 SIGWINCH 而不会重排。
该重排 MUST 防抖到输入法高度稳定后执行（MUST NOT 逐帧发 SIGWINCH），
并 MUST 只重算网格、MUST NOT 重配交换链。

输入法遮挡高度 MUST 取自平台 insets 派发（MUST NOT 取自轮询
`rootView.rootWindowInsets` 或 Compose 的 `WindowInsets.ime` 叶节点——两者在
仪器化环境下均恒为 0，实测键盘高 883px 时 DecorView 仍报 0）。备用屏状态 MUST
取运行期逐帧发布的流值；键盘已展开时启动 TUI MUST 触发一次重排。

同一场景下终端 Surface 的平移量 MUST 为 0（备用屏内容顶对齐渲染，平移会破坏
触摸行与视觉行的对位）。主屏 MUST 保持既有位移语义且 MUST NOT 因输入法改变
`rows`/`cols`：位移量按内容下沿裁剪，平移后底部像素与平移前完全相同。

主屏的跟随位移 MUST 由终端 Surface 自身的 `translationY` 承担，MUST NOT 经组合
容器平移：组合平移要经「重组 → 重新测量 → 重新布局」，而重组只在 Choreographer
帧回调里跑；主线程每帧阻塞在 `syncAndDrawFrame` 等待渲染线程时，重组滞后可达十几秒
（实测每 300ms 写一次组合状态，20 次才换来一次重组），期间键盘已弹出而终端内容与
键栏纹丝不动。键栏是组合覆盖层，MUST 读同一个 `imeInsetFlow` 上移，MUST NOT 另取
来源，也 MUST NOT 与终端 Surface 平移两次。

#### Scenario: 主线程被渲染阻塞时位移仍即时

- **WHEN** 每帧绘制都阻塞在等待渲染线程，且输入法在弹出
- **THEN** 终端 Surface 的平移量在 insets 派发的同一拍内生效，不等组合重组

#### Scenario: 备用屏弹键盘后网格收缩并触发重排

- **WHEN** 全屏 TUI 运行中弹出输入法且高度稳定
- **THEN** 网格行数收缩到可见高度能容纳的行数，列数不变，应用收到 SIGWINCH 后重绘

#### Scenario: 键盘已展开时启动 TUI 也重排

- **WHEN** 输入法已展开，随后从 shell 启动全屏 TUI
- **THEN** 网格立即收缩到可见高度

#### Scenario: 收起输入法后网格复原

- **WHEN** 输入法收起
- **THEN** 网格行数回到弹出前的值

#### Scenario: 主屏不因输入法改变网格

- **WHEN** 主屏 shell 会话中弹出输入法
- **THEN** `rows`/`cols` 不变，终端内容按内容下沿裁剪平移，底部像素与平移前相同
