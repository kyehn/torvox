# STATUS 四项处置证据

对应 `docs/STATUS.md` 的四项。只写结论、用例名与计量，不写原始日志行、时间戳、
PID 与设备标识。

## 复核基线

本轮复核基线为 canonical `main`。采集自 `kyehn-patch-1` 的那份 logcat 属**修复前**
的平行世系：该分支与 `main` 无共同祖先，不能把它的行为直接当作 `main` 的现状。
本文只把它当作「修复前现象」的证据引用。

设备信息不外传；下文「真机」一律指 Mali-G57（Vulkan）真机。

## 一览

| 项 | 根因 | 修复 | 自动化判据 | 剩余 |
| --- | --- | --- | --- | --- |
| 上下滑动滚动 缓慢 卡顿 闪烁 | 三个成因叠加：两路帧节拍相位漂移导致同一显示周期推出两帧（交换链背压，取帧等待约 4 个 vsync）；滚动事件逐个写 Compose 可观察的滚动偏移镜像（整页每事件重组一次）；亚行余量非零即强制整帧重绘（静止帧白丢脏带渲染） | 健康 vsync 下活跃闭锁只作安全网；仅在搜索可见时镜像滚动偏移；余量稳定时停止整屏重绘 | JVM `RenderLatchCadenceTest`；Rust `scroll_offset_active_decision_table`、`scroll_gate_composition_*` | 真机复测呈现间隔 |
| 文本长按选择 越侧后另一边界异常增加、手柄与边界不再对齐 | **本轮定位**：Kotlin 侧选区是行主序线性选区，原生侧也按线性语义解释端点（`SELECTION_RECTANGLE = false`），但下发前把端点换成了**包围盒角点**（行列各自取极值）。跨行且起始列大于结束列时，原生区域在首行左侧与末行右侧同时外扩，而手柄仍停在锚点上 | 下发端点改回行主序有序对；拖动途中与最终提交共用同一对端点 | JVM `SelectionStateTest`（端点下发 4 例）、`NormalizeSelectionAnchorsTest` | 真机像素级复测 |
| 文本搜索定位 输入法弹出时可能看不到匹配文本 | 居中只按「总行数 − 输入法行数」折算，漏掉「内容为跟随键盘整体上移」这一项；输入法遮挡超过终端总高度三分之一时，命中行被推到**屏幕上缘之外** | 居中改以**可见带**为基准（输入法遮挡 + 键栏占高 + 内容上移），由可见带正中反解命中行的视口行 | JVM `SearchCenterViewportRowTest`（4 例） | 真机复测 |
| kitten icat 图像 clear 后要清两次才消失 | ED2 撤下放置只推进**放置生成戳**，而旧缓存规则只看**图像存储生成戳**：既不唤醒也不重查，图像留在累加器里，要等下一次整帧重绘才被抹掉 | 单元数据新增放置生成戳；缓存按放置生成戳失效并整帧重绘 | Rust `ed2_removes_kitty_placements_on_first_clear`、`ed2_removes_virtual_kitty_placement_on_first_clear`；仪器化 `clear_erases_image_on_first_ed2` | 无（纯渲染层） |

## 逐项证据

### 一、滚动

- 现象计量（修复前，真机）：滚动帧整幅重绘，单帧约 54ms；相邻呈现出现 3–5ms 的
  成对呈现，随后 60ms 以上空档；同帧慢帧日志的耗时几乎全部落在取帧等待段，
  不是 GPU 绘制段——产出快于合成器消费即交换链背压。
- 修复后的行为判据：
  - `activeLatchOutlastsOneVsyncWhenVsyncIsFresh`（健康 vsync 下活跃闭锁 > 一个显示周期）
    与 `vsyncCallbackTimestampMarksTheChainFresh`（健康判据取自帧回调时间戳）；
  - Rust `scroll_offset_active_decision_table`：余量稳定（非零）时允许部分帧，
    余量变化时才强制整帧；亚 EPSILON 的变化按浮点噪声放行；
  - Rust `scroll_gate_composition_*`：滚动激活时其余四个谓词位各自翻转都压过部分判定。
- 剩余：真机复测「相邻两次呈现间隔不低于一个显示周期、无 60ms 以上取帧空档」。
  本环境无设备，仪器化用例未跑。

### 二、选区

- 根因（本轮定位，与前几轮的「两条路径各持一份边界」不同）：`SelectionState.applyHandleDrag`
  与 `clampSelection` 维护的是**行主序线性**选区；原生 `install_selection_impl` /
  `selection_text_impl` 同样按线性语义解释端点（上游 `Selection::new(start, end, false)`）。
  但 `publishNormalizedSelectionBounds` 与 `syncDragBoundsToNativeThrottled` 在下发前
  取了行列各自的极值，得到**包围盒角点**。
- 判别力：选区 `(10,5) → (12,2)` 时，包围盒角点是 `(10,2) → (12,5)`。原生按线性语义
  解释后覆盖首行第 2 列至行尾、末行行首至第 5 列，而正确区域是首行第 5 列至行尾、
  末行行首至第 2 列——首行左侧与末行右侧各多出三格；控制柄仍按有序端点画在
  `(10,5)` 与 `(12,2)`，于是「另一边界异常增加且两个指针和边界不再对齐」。
  同一缺陷的另一面：取文走的是行主序归一，**复制出的文本与屏幕高亮不是同一块**。
- 判据：
  - JVM `SelectionStateTest`：`endpoints stay linear when the start column is to the right`、
    `endpoints stay linear within one row`、`endpoints keep the forward order`、
    `drag bounds are the native endpoints verbatim`；
  - 既有 `NormalizeSelectionAnchorsTest` 钉住行主序归一本身。
- 先前已有的修复（保留）：两条拖动路径共用同一份实时边界（快速路径与边缘滚动交替时
  未拖动的一侧不回弹）；长按反向越过锚点后远端边界保持锚定；触点折算的参考点在整次
  拖动内锁定。
- 剩余：真机像素级复测「拖到起始列更靠右的位置后，高亮首行的左缘等于拖动锚点列，
  且复制文本与高亮区域一致」。本环境无设备。

### 三、搜索

- 根因：输入法弹出时主屏**不改网格**（改网格会重排闪烁），而是把终端 Surface 整体
  `translationY = -shift` 上移跟随键盘。命中行的屏幕纵坐标是 `视口行 × 单元高 − shift`，
  屏幕上的可见带是 `[0, Surface 高 − 键栏占高 − 输入法遮挡]`。
  原实现只按「总行数 − 输入法行数」取半屏，漏掉 `shift`：输入法遮挡超过总高度
  三分之一时命中行的屏幕纵坐标为负——被推到上缘之外，仍看不到。
- 判据：JVM `SearchCenterViewportRowTest`：
  - 无输入法时命中行落在视口正中；
  - 输入法遮挡 40px、内容上移 40px 时命中行的屏幕纵坐标仍在可见带内；
  - 输入法遮挡 200px（超过总高三分之一）时命中行仍在可见带内——只按行数折算会得到
    第 5 行、屏幕纵坐标 −120（上看不到）；
  - 单元高度带小数（20.4px）时按真实值折算：截断成整数会让折算量放大两行，
    命中行偏离可见带正中 41px；
  - 单元高度为 0 时不产生偏移。
- 同时删除只被测试引用、生产代码从未调用的 `centerSearchRow`（其断言值与实现不符，
  是基线上唯一失败的单测），以及随之不再需要的 `visibleSearchRows`。
- 剩余：真机复测「输入法弹出后按搜索定位，命中文本落在键盘上方的可见区域」。

### 四、kitty

- 根因：ED2 撤下放置推进的是**放置生成戳**（`image_placements_generation`）；
  **图像存储生成戳**（`Graphics::generation`）只在图像内容增删时变化。旧缓存规则只看
  存储戳，于是既不唤醒渲染线程也不重查放置，图像留在累加器里，要等下一次整帧重绘
  才被抹掉——现象正是「第一次 clear 文本被清、图像还在，第二次 clear 图像才消失」。
- 实测（本轮复核，虚拟放置通道）：撤下放置前后**存储生成戳不变**（2 → 2），
  **放置生成戳变化**（1 → 2）。直接放置通道（`a=T`）不同：图像条目随放置一并删除，
  存储生成戳也变化（3 → 5）——两条通道的共同信号只有放置生成戳。
- 判据：
  - Rust `ed2_removes_virtual_kitty_placement_on_first_clear`：断言存储生成戳不变、
    放置生成戳变化、放置集合立即为空；
  - Rust `ed2_removes_kitty_placements_on_first_clear`：断言放置生成戳变化、放置集合为空；
  - 仪器化 `clear_erases_image_on_first_ed2`：一次 ED2 后图像像素不再出现在屏幕上。
- 本轮同时订正了与实测相反的用例注释（原文写成「上游对虚拟放置的撤除同样会推进存储
  生成戳」）。
- 剩余：无。纯渲染层，不依赖设备。

## 本轮未改动的受保护文件

`docs/specification/`、`docs/STATUS.md` 内容未改动。
