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
- 使用 `kitten icat test.png` 后，输入 clear 回车不会清除图像，图像变大，文本被遮挡，再次盲输入 clear 回车被清除
- 切换应用返回页面闪烁
- 侧边面板过宽，布局不合理
  （现状：`SessionDrawer` 的宽度是 `fillMaxWidth(0.84f)` 落在 `ModalDrawerSheet` 的
  360dp 上界内，实际解析为 0.84 × min(窗口宽, 360dp)，360dp 屏上约 302dp；紧随其后的
  `widthIn(240..320)` 在这种写法下不生效——`fillMaxWidth` 交给下游的是「min = max」
  单点约束。要收窄必须先定一个目标宽度，需真机截图确认是「占屏比例过大」还是「绝对宽度
  过大」，两种成因的改法不同）
- 输入光标遮盖文本时可见性差，和 termux 效果不一致
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
