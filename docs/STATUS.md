# 状态

本文件记录尚未解决的问题。修改必须与对应代码改动同批提交：删除一条即表示该问题
已被修复并验证，提交消息里须写明验证方式（用例名、日志证据或设备复现步骤）。

## 未被解决的问题

- 输入法未弹出时，修饰键栏底部存在过高的空区域背景
  （机制：内容整体按 `navigationBars` 与 `safeGestures` 内缩，键栏贴在内容下沿，
  键栏与屏幕底边之间因此留下一条只有背景色的内缩带。修法是把这段内缩纳入键栏**自身**的
  背景而不是留白，但必须同时保证键位仍落在手势区之外——真机上上滑手势会先被系统认领并
  向应用补发 `ACTION_CANCEL`，键位若落回手势区即重现「按下动画不复位」。3 键导航与手势
  导航两种模式下都要验证，故需真机）
  （补充核查：键栏背景 `resolvedTerminalTheme.background` 与外层 Box 的
  `terminalBackground` 是同一条解析链上的同一个值，那条带子的**颜色**与键栏完全一致，
  所以观感上的「空白」来自**高度**而不是配色：带子宽度恰是
  `max(navigationBars, safeGestures)`，也就是系统导航/手势区本身。因此「把背景涂进带子」
  不产生任何可见变化（本就同色）；只剩两条路——让键位进入手势区（会重现
  `ACTION_CANCEL` 回归，禁止），或让终端网格按含带子的高度排版（末行会被导航条压住）。
  两者都需真机像素验证后再定）
- 使用 `kitten icat test.png` 后，输入 clear 回车不会清除图像，图像变大，文本被遮挡，再次盲输入 clear 回车被清除
- 切换应用返回页面闪烁（已按日志定位到「开孔覆盖状态晚一帧」并改为常驻收集；
  仍需真机确认一闪消失）
  （证据：切后台时 `setRenderPaused: paused=true` 后约 20ms 系统拆除 surface
  ——`mali_gralloc` 连续 unregister 交换链缓冲；切回时 `attach_surface: ANativeWindow
  changed` 重建，从拆到首帧 `presented` 间隔约 300ms。这段里 `SurfaceView` 的开孔没有
  缓冲，只能靠组合层临时涂终端底色遮住。该标志由渲染线程在 `onSurfaceDestroyed` 写，
  时机在 `onStop` 之后，而 `collectAsStateWithLifecycle` 在低于 STARTED 时已停止收集，
  切回的 `ON_START` 重启收集与首帧绘制抢跑——首帧按旧值（有缓冲）绘制，开孔透出上一个
  应用，即那一闪。已改为常驻 `collectAsState`（见 `TerminalScreen`）并新增用例
  `SurfaceLossRecoveryInstrumentedTest.backgroundStopKeepsCoverStateAndResumePresentsAgain`
  守住「切回必须重新出帧」。剩余的平台侧代价：surface 被拆到重建这 300ms 只能显示纯底色，
  文字必然短暂消失，要彻底消除需保留最后一帧位图，属另一项改动）
- 侧边面板过宽，布局不合理
  （现状：`SessionDrawer` 的宽度是 `fillMaxWidth(0.84f)` 落在 `ModalDrawerSheet` 的
  宽度上界内（Material3 `DrawerDefaults.MaximumDrawerWidth`，360dp），实际解析为
  0.84 × min(窗口宽, 360dp)，360dp 屏上约 302dp；紧随其后的
  `widthIn(240..320)` 在这种写法下不生效——`fillMaxWidth` 交给下游的是「min = max」
  单点约束。要收窄必须先定一个目标宽度，需真机截图确认是「占屏比例过大」还是「绝对宽度
  过大」，两种成因的改法不同）
- 输入光标遮盖文本时可见性差，和 termux 效果不一致（已按 termux 的
  `invertCursorTextColor` 改为整格反色：背景取不透明光标色、文字取该格原背景色，
  取色在 SGR 反显与搜索高亮之后，见 `native/src/render/cell_builder.rs`；竖线/下划线
  仍是半透明细标记。回归钉：`render::tests::cell_pipeline::cursor_with_text_and_block_style`、
  `cursor_color_custom_values`、`cursor_at_origin`、
  `cursor_cell_not_affected_by_search_highlight` 与
  `render::tests::gpu_core::cursor_rendering_on_visible_cursor`；规约见
  `openspec/specs/cursor-rendering`「方块光标整格反色」。仍需真机确认对比度观感）
- `bell-character` 未接线（`docs/specification/CONFIG.md` 声明该键，`DESIGN.md` 要求
  实现与 termux-app 一致）
  （现状：全仓没有注册 `on_bell`，`docs/persistence/osc-notifications.md` 因此只能记
  「未接线」。上游 `libghostty-vt` 提供 `Terminal::on_bell`；接线要三处一起改：VT 线程
  注册回调 → `Event::Bell { session_id }` 入队（事件本来就以 JSON 跨 JNI，见
  `native/src/event.rs`）→ Kotlin 侧按三态响应（`vibrate` 默认，振动 50ms、限频 150ms；
  `beep`；`ignore`），且仅在界面可见时生效。Rust 侧本机不具备构建条件（缺 ghostty
  源码与 zig），故未提交无法验证的原生改动）
- docs/persistence 内容需要重新验证和更新
- openspec/specs 目录文档需要根据 docs/specification 重写
- android/benchmark 需要更充分的性能测试并且断言测试结果，使用模拟器检查，优化提高性能
- termux.properties 的所有相关代码必须根据 docs/specification termux-app 重新检查和修改，并且每个键都需要充分测试
- 检查 docs/specification 的修改，对照项目进行修改 修复

## 已判定为非应用缺陷（附判定依据，不再作为待修项）

- **`mali_gralloc: Requested R8 format is not supported with this allocator`**
  应用侧已把窗口缓冲固定为 RGBA_8888（`native/src/android/ffi/render/window.rs`
  的 `ANativeWindow_setBuffersGeometry(ptr, w, h, 1)`），交换链格式为 `Rgba8Unorm`
  （运行日志 `attach_surface: configured ... format=Rgba8Unorm`）。R8 请求发生在
  `vkCreateSwapchainKHR` 内部：Mali WSI 对每个 swapchain image 先探测 R8、失败后
  回退到实际格式（同批日志里随后是 4 次 `register` 成功）。应用侧没有可干预的
  调用点；强行改格式只会换一种缓冲，不消除驱动内部探测。

- **`Not starting debugger since process cannot load the jdwp agent`**
  debuggable 构建启动时 Android 尝试加载 JDWP agent 的系统日志，由
  `ro.debuggable` 与系统镜像的 debug 配置决定；release 构建不会出现。应用无法也
  不应在运行时改变它。

- **`DocumentsProvider: Failed during query ... no such document '*.rollback'`**
  系统文件选择器/编辑器在保存流程中探测回滚文件。AOSP 契约要求
  `DocumentsProvider.queryDocument` 对不存在的文档抛 `FileNotFoundException`；
  改成返回空行会让客户端把不存在的路径当成仍然存在，新建/改名随即撞「路径已存在」
  （`TerminalDocumentsProvider.kt` 的注释记录了该回归）。这是 SAF provider 的预期
  行为，W 级日志由系统打印，应用侧不应为了消日志破坏契约。

- **`SurfaceSyncer: Failed to find sync for id=N`**
  归因：平台。审计的三份运行日志里，该类条目只出现在 `SurfaceView` 的 surface
  建立/拆除前后，id 个数等于该次 attach 中参与同步的 surface 数（1–4 个），紧随
  其后的帧照常呈现、画面无缺失。id 由框架的 `SurfaceControl` 事务分配，应用侧没有
  可写入该 id 的 API；应用能做的只有「不重建 surface」，而重建由窗口与视图生命周期
  决定，不是本仓的代码路径。

- **`Parcel: Expecting binder but got null!`**
  归因：平台。与上一项成对出现：审计中每一次都在 surface attach 前数毫秒内打印，
  是框架打包系统窗口事务时对空 binder 的诊断。事务由平台发起，应用代码里没有构造
  该事务、也没有可传入 binder 的调用点。
