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

#### Scenario: 启动屏遮挡到终端首帧呈现

- **WHEN** 冷启动后系统启动屏求值撤下时机（内容视图首次预绘制）
- **THEN** 终端尚未向 Surface 呈现任何一帧时 MUST 继续遮挡（此时 SurfaceView 的开孔区透出黑底）
- **AND** 终端首帧呈现后 MUST 立即撤下
- **AND** 首帧在 5s 上限内始终未呈现时 MUST 撤下，MUST NOT 永久遮挡界面

### Requirement: 启动屏图标为单一图案源的收缩包装

系统启动屏的图标 MUST 是**普通图形**（`@drawable/splash_icon`），MUST NOT 直接给自适应
图标。彩色图案（终端窗口 + 星形 + 点迹）在仓内 MUST 只有一份（`@drawable/logo_art`）：
启动屏与桌面图标前景都只包装引用它，MUST NOT 各自维护路径副本；自适应图标的 monochrome
层（`launcher_monochrome`）是平台 themed icon 的单色简化重绘，不属该份彩色图案的副本。
启动屏包装 MUST 是固定尺寸（96dp）的收缩包装，墨迹半径 MUST 严格小于平台蒙版圆且不小于
其 40%；桌面自适应图标前景（`launcher_foreground`）MUST 以 108dp 固定尺寸 1:1 引用
`logo_art`，外观与直接引用原图案一致。缩小只允许发生在启动屏包装层。

#### Scenario: 平台启动屏几何（动机）

- **WHEN** 平台冷启动绘制启动屏图标
- **THEN** 图标单元 = `starting_surface_icon_size`(160dp) ×
  `splash_icon_no_background_scale_factor`(1.2) = 192dp，默认蒙版圆 = 单元的 50/108
  ≈ 88.9dp；裸 `layer-list`（无固定尺寸子项）会被拉到满单元（放大 1.78×），墨迹半径
  超越蒙版上限，星尖与窗口一角被裁——用户报「图案不完整，被限制到图标形状」的实测根因，
  也是本收缩包装的动机

#### Scenario: 实测口径

- **WHEN** 验证启动屏输出
- **THEN** Robolectric 模型墨迹 ≈0.50×蒙版，平台 emulator 录屏量测 ≈0.87×蒙版，
  两栈均在 [40%, 95%] 区间内；旧实现 ≈1.0× 即被裁

#### Scenario: 启动屏为图形加底色

- **WHEN** 冷启动显示系统启动屏
- **THEN** 只见图形本体画在主题底色上，图标后不出现圆形底板

#### Scenario: 启动屏直接给自适应图标

- **WHEN** `windowSplashScreenAnimatedIcon` 指向 `@mipmap/ic_launcher`
- **THEN** 图案沿桌面蒙版被裁成图标形状，MUST NOT 如此配置

#### Scenario: 与桌面图标同源

- **WHEN** 审查启动屏图标与自适应图标前景
- **THEN** 启动屏与桌面图标前景都只引用同一份的 `logo_art`，仓内无第二份路径副本；
  桌面前景以 108dp 1:1 呈现，启动屏以 96dp 收缩包装呈现

### Requirement: 启动屏主题配置与回归防线

`Theme.Terminal.Splash` 的 `windowSplashScreenAnimatedIcon` MUST 指向
`@drawable/splash_icon`；父主题 MUST 为 `Theme.SplashScreen` 且不声明
`windowSplashScreenIconBackgroundColor`。回归防线 `SplashIconRobolectricTest`
MUST 解析平台几何后断言墨迹与蒙版关系，MUST NOT 只断言资源存在。

#### Scenario: 主题配置

- **WHEN** 审查启动屏主题
- **THEN** 渲染期解析出的 `windowSplashScreenAnimatedIcon` 指向 `splash_icon`，
  底色 alpha 为 0（无图标背板）

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

### Requirement: 切换会话后呈现新会话的整幅内容

累加器与 `last_frame` 缓存只属于产帧的那个会话。切到另一会话后，渲染 MUST NOT 停在
上一会话的像素上等它恰好产生新输出：缓存帧会话与当前会话不一致时，渲染路径 MUST 主动
请求该会话按当前状态重推一帧，且该请求 MUST NOT 被内容去重吞掉（切会话时终端本身毫无
变化）；该帧到达后 MUST 整幅重画。

#### Scenario: 空闲会话被切回

- **WHEN** 会话 A 已有内容且此后无新输出，用户从会话 B 切回 A
- **THEN** 渲染路径 MUST 请求 A 重推一帧，A 的整幅内容 MUST 在无需任何输入的情况下
      呈现（脏掩码覆盖全部行，`presented … bands=1`），MUST NOT 保持 B 的像素

#### Scenario: 重推请求不被内容去重吞掉

- **WHEN** 会话内容与上次推送逐字节相同（终端毫无变化）而收到重推请求
- **THEN** VT 线程 MUST 失效去重基线并推送该帧

#### Scenario: 无会话可推时不报错

- **WHEN** 缓存帧属于别的会话而当前会话已在注册表中消失
- **THEN** 请求 MUST 静默跳过（返回失败而非 panic），渲染照常返回「本帧无内容」

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

### Requirement: 备用屏下输入法弹出不位移终端 Surface

备用屏（helix/vim 等全屏 TUI）激活时，输入法跟随位移 MUST 为 0：此类应用恒占满
视口，任何位移都会把应用顶部推出屏幕并使视觉行与触摸换算行错位。主屏的位移公式
MUST NOT 因此改变。备用屏状态 MUST 随每帧渲染结果一并上报（与内容下沿同批），MUST NOT 由输入法定居后的独立查询提供：输入法弹出动画期间该状态会翻转
（启动 helix 的同时键盘正收起），查询所得缓存必然滞后于当帧的位移计算。

#### Scenario: 以网格收缩兑现适应窗口大小

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

换活动会话 MUST 把内容下沿与备用屏状态重置为新会话的现值；活动会话归零（最后一个
会话被关闭、启动失败回滚）时 MUST 一并复位为两者的缺省值。渲染循环只在**变化时**
发布这两项，新会话首帧若与它自身的字段相同（如备用屏的初始 `false`）就不会发布，
运行期会一直保留**上一个**会话的值。备用屏标记陈旧为真时，输入法扣减会一直扣着
一份早已撤走的键盘高度（网格永久塌缩且不自愈）、主屏位移恒为 0，且输入法动画会在
主屏上白白领走一次渲染暂停。

遮挡高度的**发布者** MUST 唯一，且离场方 MUST 按发布者（而非按数值）归零：同一
窗口里的新旧 Surface 看到同一个输入法高度，数值天然相同，按值判断会把新 Surface
刚发布的真实高度误当成「自己还是最后一个发布者」而覆盖成 0；此后新 Surface 的
「值变化才发布」门控永不触发，运行期永久停在 0（键栏错位、备用屏网格恢复整屏行数
且不自愈）。

平移量的每个输入变化 MUST 触发重算：输入法遮挡、视口尺寸（旋转，Activity 声明
`configChanges` 不重建）、单元格度量（字号与捏合缩放）、内容下沿、备用屏状态。
订阅 MUST 在每次 attach 时重建（detach 已停掉它们，而视图复用不重走 viewModel 注入）。

渲染暂停 MUST 只有一个计数账本，由运行期持有，全部来源共用：设置页展示、切后台、
Surface 销毁、交换链重配防抖、输入法网格重排防抖。运行期的 `setRenderPaused` 是非计数布尔，任何
来源 MUST NOT 直接写它——先结束的来源会把别人还在等的暂停放掉。

两个防抖窗口重叠时 MUST 计数，并 MUST 在替换/取消一个在途防抖时先归还它持有的
那一次——被 `removeCallbacks` 丢弃的 runnable 永不执行，它领到的暂停也就永不归还，
计数单调增长，共享渲染器被**永久**暂停（终端全黑）。键盘弹出动画期间平台逐帧派发
insets，每个动画帧都会走一遍这条替换路径。

**全部来源都经账本**：设置页、切后台、Surface 销毁、视图的两个防抖。
直接写 `setRenderPaused` 会把别人仍在等的暂停顶掉——在已被系统回收的 BufferQueue
上出帧即永久黑屏。任何一个来源 MUST NOT 直写那个非计数布尔；否则它不被任何持有者
计数，别人归还时顺手把它清掉。

一次来源的**替换**（切后台重复发生、换链防抖被新尺寸替换）MUST 先领新的、再归还
旧的——持有者数经过 0 即会多发一对暂停/恢复，而那一对恢复恰好落在最不该出帧的时刻。

取消视图的两个在途防抖 MUST **只归还本视图领到的那两个凭证**：MUST NOT 整本清账
（那会连带作废设置页那条与本视图无关的暂停），也 MUST NOT 只把凭证字段置空——
清字段不等于归还，一次漏还即让计数永不归零，此后所有领用都不再发出暂停（布尔幂等），
暂停功能就此静默失效。

「刚拿到可用 Surface、必须立刻出一帧」的入口（Surface 重建成功、交换链重配完成）
MUST 只作废**生命周期**持有（Surface 销毁、切后台）；设置页那条 MUST 保留——它是
界面层的独立意图，而终端视图在设置页覆盖层之下仍会收到尺寸变化（设置页自己的
键盘），整本清账会把「进入设置时暂停渲染」废掉。

#### Scenario: 关设置页不放开切后台的暂停

- **WHEN** 应用在后台（切后台已持有一次暂停）时用户关闭设置页
- **THEN** 渲染保持暂停，`setRenderPaused(false)` 不被调用

#### Scenario: 取消在途防抖须归还而非只置空凭证

- **WHEN** 防抖窗口内触发强制出帧，视图随即取消在途防抖
- **THEN** 两个在途防抖各自归还其持有的那一次，账本计数归零且设置页的持有不受影响

#### Scenario: 强制出帧只作废生命周期持有

- **WHEN** Surface 重建成功而此前仍有切后台或 Surface 销毁的暂停持有
- **THEN** 这两个生命周期持有被作废，渲染恢复，随后新领的暂停仍能发出；设置页
      仍在展示时则保持暂停

#### Scenario: 设置页暂停先于防抖归还

- **WHEN** 设置页关闭时仍有交换链防抖在途
- **THEN** 渲染保持暂停，直到该防抖也归还

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

#### Scenario: 备用屏会话关闭后新会话不继承备用屏状态

- **WHEN** helix 处于备用屏且输入法展开时关闭该会话，由另一会话接替
- **THEN** 新会话的网格按整屏高度计算（不扣任何输入法遮挡），键盘弹出时仍随键盘上移

#### Scenario: 选区在键盘弹出后不留陈旧弹窗

- **WHEN** 选中文本后弹出输入法
- **THEN** 选区手柄与上下文菜单被关闭，不留在弹出前的位置

#### Scenario: 主屏位移公式不变

- **WHEN** 终端处于主屏且内容填满网格
- **THEN** 位移仍为整块键盘高度，与备用屏判定无关；`rows`/`cols` 不变，
      上移后底部像素与上移前完全相同

### Requirement: 换 Kitty 图集即失效累加器

替换或清空 Kitty 图像图集 MUST 使下一帧走全量自包含重绘（累加器失效位）。图集内容
变了，而未重绘的行上留的是上一张图的残片；清空时后果更明显——图像消失后实例表为空，
部分帧重新放行，而图像原先压住的那些行若并不脏（kitty 图常叠在空白单元格上），
此后每帧只画脏带，旧图像像素在屏幕上永久残留。

#### Scenario: 删除图像后不残留像素

- **WHEN** 某张 kitty 图像覆盖的行本身并不脏，图像随后被删除
- **THEN** 删除后的下一帧为全量重绘，旧图像像素不再留在屏幕上

### Requirement: surface 挂载失败必须让出属主并判死

任一挂载失败路径（`ANativeWindow_fromSurface` 返回空、`ANativeWindow_setBuffersGeometry`
非零、`create_surface_unsafe` / `get_capabilities` / `configure` 失败）MUST 同时做两
件事：把当前 surface 属主清零，并把 surface 标记为已判死。缺任一条的后果相同且不可
自愈：属主残留时它随后的 `detachWindow` 仍能通过归属校验并清掉状态，而新窗口无人挂载；
而渲染对「无 surface」返回 0，与空闲帧不可区分，宿主的换窗口看门狗因此永远不触发。

#### Scenario: 几何设置失败也走同一条自愈路径

- **WHEN** 挂载时交换链几何设置失败
- **THEN** 属主被清零且 surface 判死上报，宿主据此换新窗口重新挂载

### Requirement: Surface 尺寸钳制到设备上限

交换链的可配置尺寸 MUST 由唯一一个纯函数计算：先按跨渲染器固定的
`RENDER_SCALE` 缩放，再钳制到 `[1, device.limits().max_texture_dimension_2d]`。
`attach_surface` 与 `reconfigure_swapchain` 两条挂载路径 MUST 共用该函数，
MUST NOT 各写一份缩放/钳制逻辑——补了钳制的那条路径会给另一条留下缺口，而
该缺口在 wgpu 30 无返回值、只经 `get_current_texture` 的 Lost 状态异步上报，
表现为永久黑屏且没有直接错误。

钳制 MUST 在宿主单测里以判定表直接判定（不经 GPU）：缩放、下限 1、上限钳制
三个不变量各有用例；改动钳制逻辑 MUST 让对应用例判红。

#### Scenario: 超限窗口尺寸被钳制而非交给 configure 失败

- **WHEN** 窗口缩放后尺寸超过 `max_texture_dimension_2d`
- **THEN** 配置尺寸等于该上限，surface 正常出帧，MUST NOT 以超限尺寸 configure

#### Scenario: 两条挂载路径共用同一计算

- **WHEN** 复查 `attach_surface` 与 `reconfigure_swapchain` 的尺寸来源
- **THEN** 两者都调用同一纯函数，MUST NOT 出现第二份缩放/钳制实现

#### Scenario: 下限为一像素

- **WHEN** 传入尺寸为 0（空窗口／尚未布局）
- **THEN** 结果两维都不小于 1，与 wgpu 对零尺寸 surface 的要求一致
