## 未被解决的问题

基于 `c384d751`

- 上下滑动滚动 缓慢 卡顿 闪烁
  - 已改：活跃闭锁与 vsync 曾是两路帧节拍（60Hz 上实测平均 ≈72fps，成对呈现后接
    66–77ms 取帧空档）。现于 vsync 链健康时把活跃闭锁当安全网（34ms），链停摆才退回
    17ms 兜底；手势滚动也不再逐事件写 Compose 偏移镜像（原先每个滚动事件重组整页）。
    判据：`RenderLatchCadenceTest`。待真机复测呈现间隔与取帧耗时。
- 文本搜索匹配 和 当前匹配 高亮颜色应该不同
  - 已改：底色原先以交换后的背景为基准混合，而默认主题的选区底色回退为前景色，
    `blend(前景, 前景)` 与 alpha 无关，两者渲染成同一块。现从原背景混合，选中命中
    按全部物理行着色。判据：`other_match_stays_distinct_when_highlight_equals_foreground`、
    `soft_wrapped_selection_colors_every_physical_row`。
- 文本长按选择 朝一个方向拖动指针后如果朝反方向移动越过另一个指针则文本匹配区域将异常变动另一边界异常增加且两个指针和边界不再对齐。
  - 已改：快速路径与边缘滚动各持一份边界，交替时未拖动的一侧被算回旧端点。
    现由 `resolveDragBounds` 统一折叠并写回实时边界。判据：`SelectionStateTest`。
    待真机复测手势。
