# STATUS 三项处置证据

对应 `docs/STATUS.md` 的三项。只写结论、用例名与计量，不写原始日志行、时间戳、
PID 与设备标识。

## 一览

| 项 | 根因 | 修复 | 自动化判据 | 剩余 |
| --- | --- | --- | --- | --- |
| 上下滑动滚动 缓慢 卡顿 闪烁 | 两路帧节拍（17ms 闭锁 + vsync）相位漂移，同一显示周期推出两帧，交换链排满后取帧阻塞约 4 个 vsync | 健康 vsync 下闭锁只作安全网（34ms），停摆退回 17ms | JVM `RenderLatchCadenceTest` 两例 | 真机复测呈现间隔 |
| 文本搜索匹配与当前匹配高亮应不同 | 底色以交换后的背景为基准混合，默认主题选区色回退等于前景色，`blend(前景, 前景)` 与 alpha 无关 | 底色从原背景混合；选中命中保留全部物理行 | Rust `other_match_stays_distinct_when_highlight_equals_foreground`、`soft_wrapped_selection_colors_every_physical_row` | 无 |
| 反向越过对侧手柄后另一边界异常增加 | 快速路径与边缘滚动路径各持一份边界，慢速路径只读已提交状态 | 抽出 `resolveDragBounds`，两条路径共用并写回实时边界 | JVM `SelectionStateTest` 三例 | 真机复测手势 |

## 逐项证据

仪器化复跑（x86_64 模拟器，软件 Vulkan）：`SelectionDragQuantifiedTest` 7 例、
`TextSearchEndToEndTest` 10 例、`ScrollBehaviorQuantifiedTest` 3 例、
`SelectionEspressoTest` 9 例，全部通过（0 失败）。

### 一、滚动

- 日志计量：渲染循环窗口平均 ≈72fps（显示刷新 60Hz）；呈现成对出现（3–5ms），
  随后 66–77ms 空档；同帧 `SLOW_RENDER_INNER` 的 `render` 段几乎全部是取帧等待。
  产出快于合成器消费即交换链背压，而不是 GPU 绘制慢。
- 修复后的行为判据：`activeLatchTimeoutNanos(vsyncFresh = true)` 必须大于一个
  60Hz 周期（34ms），`vsyncFresh = false` 时必须等于 17ms 兜底；新鲜度阈值覆盖
  多个显示周期。用例 `activeLatchOutlastsOneVsyncWhenVsyncIsFresh`、
  `vsyncCallbackTimestampMarksTheChainFresh`。
- 另一处已计入的开销：手势滚动曾逐事件写 Compose 可观察的滚动偏移镜像，
  使整个终端页每个滚动事件重组一次（主线程被占住时 vsync 回调迟到）。
  现仅在搜索可见时写，打开搜索时补一次当前偏移。
- 剩余：真机（Mali）复测「相邻两次呈现间隔不低于一个显示周期、无 60ms 以上取帧
  空档」。本环境无设备，仪器化用例未跑。

### 二、搜索高亮

- 根因：`apply_search_highlight` 先交换前景/背景，再以交换后的背景为混合基准。
  主题未定义 `[colors.selection]` 时上游把选区底色回退为前景色（`dracula_plus`
  即如此），于是其余命中的高亮色与混合基准同色，alpha 255 与 160 的像素完全一致。
- 修复：基准恒取**原背景**；交换只决定文字色。当前命中仍是前景色不透明块，
  其余命中是 `lerp(背景, 前景, 0.63)`，同色系、明度不同。
- 判据：`other_match_stays_distinct_when_highlight_equals_foreground` 用默认主题的
  前景/背景取值构造，断言其余命中的底色既不同于当前命中、也不同于未高亮背景；
  旧实现下该用例必然失败（同色）。
- 同源缺陷：选中命中只保留首行，软换行续行落回淡色。`selected` 改为全部物理行，
  判据 `soft_wrapped_selection_colors_every_physical_row`。
- 剩余：无。纯渲染层，不依赖设备。

### 三、选区拖动

- 根因：快速路径不写 Compose 状态（边界只存在于未提交的实时结果），
  慢速路径（边缘滚动）只读已提交状态。两条路径交替时，慢速路径以旧端点为基准，
  未拖动的一侧被算回交叉前的位置。
- 修复：`resolveDragBounds` 统一折叠与钳位，两条路径都以实时边界为基准并写回；
  边缘滚动改走快速路径、按返回值直接重定位两只手柄；删除失去调用方的
  `updateSelectionStart` 与 `syncDragSelectionToNativeThrottled`。
- 判据：`slow path folds onto the pending boundary from the fast path`
  （旧实现得到 `(1,10)` 而非 `(1,2)`，用例必红）、
  `fast path folds onto the boundary written by the slow path`、
  `drag bounds are clamped to the grid on both paths`。
- 剩余：真机复测「快速移动 ←→ 边缘滚动」交替时选区连续。

## 本轮未改动的受保护文件

`docs/specification/` 内容未改动。
