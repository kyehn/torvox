# render-stability Specification

## Purpose

渲染稳定性基线：字形首帧完整性、IME 动画期间输出完整呈现、滚动无撕裂、启动首帧及时呈现。记录设备取证结论、根因与已验证的实现细节（置信度低于 docs/specification/DESIGN.md）。

## Requirements

### Requirement: 字形首帧完整可见且幂等

任意 glyph（含合成斜体/粗体）首次渲染 MUST 完整可见，不得缺失部分字形；重复渲染结果
MUST 与首次一致（幂等）。MUST NOT 以 advance 宽度裁剪合成斜体位图的有效部分。

实现：`render_cell_data` 在实例构建完成、绘制命令提交之前补传脏区
（`take_dirty_rect` → `upload_atlas`），`write_texture` 以调用顺序入队先于本帧绘制命令
执行，保证首帧即按完整字形采样。

#### Scenario: 斜体字形首帧完整

- **WHEN** 终端首次输出合成斜体字形
- **THEN** 首帧即完整显示，无需重复执行才完整；重复渲染逐字节一致且无新脏区

### Requirement: IME 动画期间输出完整呈现

IME 弹出/隐藏动画期间产生的 PTY 新输出，在 IME 稳定后 MUST 完整呈现。渲染暂停 MUST
配对恢复；恢复后首选帧强制重绘。

#### Scenario: 输入法弹出期间输出不丢

- **WHEN** IME 弹出动画期间 PTY 产生新输出
- **THEN** IME 稳定后输出完整呈现，无缺失无错位

### Requirement: 滚动渲染无可见卡顿或撕裂

滚动渲染 MUST NOT 有可见卡顿或撕裂。

#### Scenario: 快速滚动画面连续

- **WHEN** 用户快速滚动终端内容
- **THEN** 画面连续更新，无撕裂残影（见 render-loop-scroll-cadence）

### Requirement: 启动首帧及时呈现

应用启动后终端内容 MUST 在合理时间内呈现，MUST NOT 长时间黑屏。

#### Scenario: 冷启动直接显示 Shell

- **WHEN** 应用冷启动完成
- **THEN** 启动动画结束后直接显示 Shell 与主题背景（见 DESIGN.md Shell 节）

### Requirement: 渲染 surface 失效后自愈

原生 surface MUST NOT 因其原生窗口的 BufferQueue 被遗弃而终身失效：连续 surface 级取
纹理失败 MUST 使缓存的 surface 失效，使下一次挂载走重建慢路径而非原地 reconfigure；宿主
MUST 换新的 `SurfaceView` 以取得新的原生窗口，且请求 MUST 受间隔与次数上限约束。

#### Scenario: 死窗口连续失败后置失效

- **WHEN** 连续两次取纹理返回 `Lost`/`Outdated`（`SURFACE_LOSS_STREAK_LIMIT`）
- **THEN** `surface_invalidated` 置位，MUST 只记一条 error

#### Scenario: 慢机器超时不算失效

- **WHEN** 取纹理因工作线程忙或超时而本帧跳过（`Skipped`）
- **THEN** 连续计数 MUST 清零，MUST NOT 置失效

#### Scenario: 单次 Outdated 不误判

- **WHEN** 取纹理单次返回 `Outdated`（SurfaceFlinger 缩放竞态）且下一次取到纹理
- **THEN** MUST NOT 置失效

#### Scenario: 失效后挂载走重建

- **WHEN** surface 已失效且宿主送来 `ANativeWindow`
- **THEN** `attach_surface` MUST 走重建慢路径（`attach_surface: configured`），
      MUST NOT 记 `RECONFIGURE_SWAPCHAIN`

#### Scenario: 恢复即清零

- **WHEN** 重建成功或 `detachWindow` 释放 surface
- **THEN** 失效位与连续计数 MUST 清零，新 surface 重新接受判定

#### Scenario: 失效位随打包返回上报

- **WHEN** 原生完成一帧渲染
- **THEN** 失效位 MUST 置于打包返回的第 53 位，MUST NOT 挂在渲染计数门下
      （空闲帧也会读取）

#### Scenario: 宿主换视图而非重挂同一窗口

- **WHEN** 宿主观察到失效位为 1
- **THEN** MUST 换掉整个 `SurfaceView` 以取得**新的**原生窗口；
      MUST NOT 仅对同一窗口反复 detach/attach（唤不活被遗弃的 BufferQueue）

#### Scenario: 重建请求限流

- **WHEN** 失效位持续为 1（重建无望）
- **THEN** 请求间隔 MUST ≥ 最小间隔，单会话请求次数 MUST ≤ 上限，超过后停止并告警一次

### Requirement: 帧与循环计时报告保留亚毫秒精度

帧窗口与循环窗口的汇总行 MUST 由纳秒直接换算为亚毫秒精度输出，MUST NOT 以整毫秒
截断的中间值参与换算：真机帧时长集中在 0–2ms，整除后健康状态一律报出
`avg=0ms p95=0ms`，窗口无法区分 0.1ms 与 0.9ms 两种健康基线，`frameTimingTrend`
的「退化到基线 3 倍」与 `FRAME_TIME_WARN_*` 绝对阈值告警在该分辨率下拿不到可读真值。
循环窗口的 `fps` MUST 由 `averageNanos` 直接换算；经截断均值换算的 `fps` 会把同一台
健康设备报成 55/58/62/66/71/76fps 六个值，全部是舍入产物而不含帧率信息。

#### Scenario: 健康窗口报出可读真值

- **WHEN** 真机静置一分钟，帧窗口内每帧渲染均在 2ms 以内
- **THEN** 汇总行的 `avg` 与 `p95` 为非零亚毫秒值（如 `avg=0.42ms p95=0.91ms`），
  而非 `avg=0ms p95=0ms`

#### Scenario: fps 不随舍入跳变

- **WHEN** 同一台设备循环周期稳定在 16ms 上下
- **THEN** 各窗口的 `fps` 取值一致，不出现 55/58/62/66 之间由整除产生的跳变

### Requirement: 备用屏下输入法随窗口自适应（备用屏激活时输入法不位移终端 Surface）

备用屏（helix/vim 等全屏 TUI）激活时，输入法跟随位移 MUST 为 0：此类应用恒占满
视口，任何位移都会把应用顶部推出屏幕并使视觉行与触摸换算行错位。主屏的位移公式
MUST NOT 因此改变。备用屏状态 MUST 随每帧渲染结果一并上报（与光标行、内容下沿
同批），MUST NOT 由输入法定居后的独立查询提供：输入法弹出动画期间该状态会翻转
（启动 helix 的同时键盘正收起），查询所得缓存必然滞后于当帧的位移计算。

**位移为 0 MUST 同时以网格收缩兑现「适应窗口大小」**：窗口是 `adjustNothing`，
Surface 尺寸全程不变，故备用屏下 MUST 把输入法遮挡计入网格高度
（`rows = floor((surface − ModifierBar − 输入法遮挡) / cellHeight)`），使全屏 TUI
收到 SIGWINCH 并按可见高度重绘。仅位移为 0 而网格不变会让键盘遮住的末行
（helix/vim 的状态行即在其中）永久不可见。该重排 MUST 防抖到输入法高度稳定后执行
（MUST NOT 逐帧发 SIGWINCH），MUST 只重算网格，MUST NOT 重配交换链。主屏 MUST
不扣输入法遮挡：它靠位移跟随，改网格会带来重排闪烁与底部行丢失。

该扣减 MUST 是运行期的单一值（`TerminalRuntime.imeGridReserve`），三条网格重算路径
——字号/字族变化触发的 `recomputeGridFromFontMetrics`、视图尺寸变化触发的
`recomputeRowsColsImmediate` 与 `applyGridResize`——MUST 共用它。任一条漏扣都会让
备用屏在改字号或捏合缩放后被撑回被键盘遮住的高度，且此后无触发点自愈。
网格重排 MUST 只在备用屏发生；主屏上 MUST NOT 因此暂停渲染，否则键盘动画期间
逐帧派发的 insets 会把暂停一直续到动画结束，终端停止上帧。

输入法遮挡高度 MUST 取自平台 insets 派发（`ViewCompat.setOnApplyWindowInsetsListener`
装在终端 Surface 上），MUST NOT 取自轮询 `rootView.rootWindowInsets` 或 Compose 的
`WindowInsets.ime` 叶节点：这两处在仪器化环境下均恒为 0（实测键盘高 883px 时
`DecorView.rootWindowInsets` 仍报 0），据此驱动的输入法跟随位移整体失效。
键盘可见性翻转时关闭选区手柄与菜单的逻辑 MUST 放在该监听器内，MUST NOT 放在
`onApplyWindowInsets` 重写里：框架在视图装了 `OnApplyWindowInsetsListener` 后
只调它而不再调用重写方法，放在重写里即静默失效。

**终端 Surface 的跟随位移 MUST 由视图自身的 `translationY` 承担，MUST NOT 经组合容器
平移**：组合平移要经「重组 → 重新测量 → 重新布局」，而重组只在 Choreographer 帧回调
里跑；主线程每帧阻塞在 `syncAndDrawFrame` 等待渲染线程时，重组滞后可达十几秒（实测
每 300ms 写一次组合状态，20 次才换来一次重组），期间键盘已弹出而终端内容与键栏纹丝
不动。`translationY` 在 insets 派发的同一拍内生效。键栏是组合覆盖层，只能读同一个
`imeInsetFlow` 上移——它 MUST NOT 另取来源，也 MUST NOT 与终端 Surface 平移两次。

平移量的每个输入变化 MUST 触发重算：输入法遮挡、视口尺寸（旋转）、单元格度量
（字号与捏合缩放）、内容下沿、备用屏状态。视图 detach MUST 取消上述订阅并复位
平移量与遮挡值：订阅跑在 `viewModelScope` 上（跟宿主而非视图），保留会让旧视图被
协程钉住，且 detach 后的备用屏翻转仍会命中网格重排——此时 `View.postDelayed`
落进 `mRunQueue`、只在重新 attach 时被 drain，被丢弃的旧视图永不 attach，
配对的「恢复渲染」永不执行，共享渲染器被永久暂停（终端全黑）。

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

#### Scenario: 收起输入法后网格复原

- **WHEN** 输入法收起
- **THEN** 网格行数回到弹出前的值

#### Scenario: 主屏位移公式不变

- **WHEN** 终端处于主屏且内容填满网格
- **THEN** 位移仍为整块键盘高度，与备用屏判定无关；`rows`/`cols` 不变，
      上移后底部像素与上移前完全相同
