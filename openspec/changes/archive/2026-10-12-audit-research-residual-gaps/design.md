# Design

## 1. 行级抽取的边界

### 1.1 为什么现状的注释结论不成立

`build_cell_data` 的注释把「上游行级脏跟踪不可用」与「故不做行级缓存」连成一句因果。
后半句只对渲染侧成立：`native/src/render/cell_builder.rs` 的行级增量比较的是**已抽取
的 `CellData`**，它省的是字形图集遍历与实例构造，省不了**抽取本身**。

抽取侧的成本是另一笔账：`internal.rs:820-824` 自述一次重建要「遍历 ~2444 次 Ghostty
FFI 调用」，而重建的触发条件是 `batch_dirty`，即一个渲染帧内有 PTY 输出或命令。终端在
一次 `ls`、`cat`、`make` 输出期间几乎每帧都满足该条件，而每帧真正变化的通常只有提示符
一行或若干行。

### 1.2 根因是 `RenderState` 的位置与其 clean 标记，而非其能力

上游 `RowIteration::dirty()` 判定一行是否脏，依据是 `RenderState` 跨帧持有的上次快照
状态。现状每次 `build_cell_data` 都新建 `RenderState`（`internal.rs:138-165` 的
`create_render_iterators`），新实例 `rows = 0`，`update` 因判定为「维度变化」而重建全部
行，于是脏标记恒为真。

已实测复核（探针跑完即删）：同一 `RenderState` 跨帧复用、首帧写满 24 行、次帧只在第0
行写一个字符，并逐帧调用快照 clean 标记，脏标记为 `[true, false × 22, true]`——第 0 行
因写入变脏、末行因光标离开变脏，其余 22 行判干净。同一探针去掉 clean 标记后，24 行全
部判脏。故行级判定在上游确实可用，现有的 0% 命中率是两个条件同时不满足的合流：

1. `RenderState` 每次重建，跨帧状态无处存放；
2. 没有调用快照 clean 标记，上游的累积脏状态永不重置。

只改其一不产生任何收益。

### 1.3 现有注释的一半是对的

`internal.rs:1745-1746` 写着「每次调用新建，以避开 invariant-param `Terminal` 类型带来的
生命周期问题」。该约束确实存在——上游 `Terminal` 持 `*mut VTable<'alloc, 'cb>`，`'alloc`
不变，把 `&Terminal` 与 `&mut RenderState` 一起跨函数传递且各自省略生命周期会报
「lifetime may not live long enough」。但它是签名写法问题，不是必须每次新建的理由：为
四个参数声明同一具名生命周期（`Terminal<'a,'a>`、`RenderState<'a>`、
`RowIterator<'a>`、`CellIterator<'a>`）即可编译，探针已验证。结论「故每次调用新建」
是把约束的代价当成了约束本身。

### 1.4 干净行从何处来

`CellData` 通道的契约是整帧：`cell_data_tx` 传 `Vec<CellData>`，渲染线程逐帧消费，不接受
行级补丁。因此干净行 MUST 由抽取侧补齐，做法是保留上一帧的 `Vec<CellData>` 与每行区间，
对判定为干净的行整段复制。这与渲染侧 `CachedInstances` 的做法同构，且复制的是本仓自己的
POD，不跨语言。

行区间由 `build_row_ranges` 按 `CellData.row` 划分。抽取侧复用同一函数，
不另写一套行边界推导——宽字符被上游省略导致各行单元数不同，凭 `col` 反推行边界会错位。

### 1.5 缓存失效条件

以下情形 MUST 使行缓存整体失效并退化为全量重建：

| 情形 | 理由 |
| --- | --- |
| 网格行列变化（`Command::Resize`） | 行区间与上游 `RenderState` 的尺寸前提同时改变 |
| 终端重置（`Command::Reset`） | RIS 清空网格与回滚，上游脏状态不可比 |
| 主题变更（`Command::SetTheme`） | 默认前景/背景参与每个单元的解算结果 |
| 选区安装或清除（`SetSelection`/`ClearSelection`） | 反白被烘焙进 `CellData` 颜色 |
| 上游 `RenderState::update` 报错 | 已无法取得可信快照，继续增量只会输出陈旧帧 |
| 任一行抽取中断（`CellIterator::update`/`style` 失败） | 该行已不完整，不得以缓存版本冒充 |

前三项当前经 `mark_grid_dirty` 与 `batch_dirty` 生效，后两项另行置 `last_cell_data_push =
None`。本变更在这两处既有的失效时机上追加行缓存失效，不另设一套时机。

一项必须排除在失效条件外的是 `Command::SetCellPixelSize`：它只改单元格像素几何，网格
内容不变，当前实现有意不置脏。`CellData` 不承载单元格度量，故行缓存在该命令后继续有效；
把它一并列为失效条件会使每次字号变化都退化为全量重建，与该命令存在的理由相反。

### 1.6 为什么不复用渲染侧的脏判定

渲染侧用 `diff_dirty_rows_into` 做逐行字节比较来定位变化行。若抽取侧照抄，即变成
「先全量抽取再逐行比较」，跨语言调用一次也没省下。故抽取侧的脏判定 MUST 取上游
`RowIteration::dirty`/`next_dirty`，渲染侧的字节比较保持原样——两者服务不同阶段，
不合并。

### 1.7 与「跳过干净快照」的关系

`internal.rs:175` 的 `snapshot_needs_rebuild` 判的是**网格是否变化**（`grid_dirty`），
为假时整帧复用缓存的 `CellSnapshot`，连抽取都不进。行级增量判的是**已进入抽取后哪些行
需要重取**。两者是同一决策的两层，语义不重叠，也不互相取代：网格未变时行级逻辑不执行。

## 2. 注释清理的边界

振铃回调的处理取「删注释」一侧，依据是 `upstream-alignment` 的台账已判定 `on_bell`
不采用。清理范围是全部 7 处描述该通道的注释文本：`commands.rs:197-198`（悬空附着于
`cell_size_px`）、`types.rs:257`（`EVENT_CHANNEL_CAPACITY`），以及 `session.rs:525`、
`:533`、`:539`、`:575`、`:1185`（收割路径的文档注释，均称回调事件含 BEL）。`clipboard_tx`
与 `EVENT_CHANNEL_CAPACITY` 的实际语义（OSC 52 剪贴板写入、有界丢弃）保持不变，只把
「振铃」从描述中去除。

`session.rs:612`、`:1028`、`:1040` 提到的 BEL 是 OSC 52 应答的 ST 终止符，是真实协议
内容，MUST NOT 一并删除。

MUST NOT 顺手接线 `on_bell`：`DESIGN.md` 未声明振铃行为（音效、通知或静默），
`STYLE.md` 要求不实现未声明的功能。

## 3. `terminal-state-regression` 的口径修正

删除语料装置的真实理由不是「`TESTING.md` 不做回归测试」——该文件无此表述——而是
`upstream-alignment` 的「上游测试资产只在不产生自验证时采纳」：语料期望值由本仓实现
自身产出，属自验证，采纳后不净增外部真相。本变更把该理由写为行为条款，并要求断言直接
针对网格输出，使 Purpose 与条款不再互相依赖一份不存在的引用。

## 4. 上游选择手势 API 的权衡

`upstream-alignment` 台账已把 `selection/gesture` 一族列为不采用，原理由只提连击计数。
本次逐项核对了上游接口与本仓拖动路径，结论不变但依据不同。

上游 `Gesture` 提供一个可复用的手势状态机：`PressEvent`/`DragEvent`/`ReleaseEvent`/
`AutoscrollTickEvent`/`DeepPressEvent`，持 `anchor`、`click_count`、`dragged`、
`autoscroll`，按 `Behaviors` 的单击/双击/三击行为分别给出 `Cell`/`Word`/`Line`/`Output`，
每次 `apply` 返回一个 `Selection` 快照，由嵌入方决定是否安装。

优势，逐项对本仓成立：

- 宽字符吸附的**策略**（拖到哪一格就吸附到哪个宽字符的边界）由上游一并给出。本仓的
  宽度事实已取自上游（`wide_char_tail_cols_impl` 读单元宽度），但吸附与归属决策在
  Kotlin 侧（`applyHandleDrag` 加 `clampSelection`）。
- `Behavior::Word` 提供词粒度拖拽（整词扩展而非逐格）。本仓拖动只做逐格——实测
  `applyHandleDrag` 只有锚点比较、交叉翻转与排序，无词扩展。
- 边缘自动滚动的上下方向与节流由上游给出，本仓自行管理计时器。
- 与 DESIGN「以 Ghostty 为终端状态单一来源」一致：选区语义不再散在两层。

劣势，逐项都落在同一处：

1. **线程亲和**。上游全库 `!Send`，且手势调用「not concurrency-safe and must be serialized
   with terminal mutations」。手势必须经 VT 线程执行。
2. **位置以像素为单位**。`set_position(x, y)` 需配 `Geometry`，需新增像素到单元的映射入口。
3. **每个 MOVE 一次跨线程往返**。本仓现在的拖动路径在 Kotlin 内算完边界（`dragMove`），
   并以 `syncDragBoundsToNativeThrottled` 节流推给原生，松手才提交精确边界；手柄重定位
   直接用返回值，不经 Compose 重组。若改为每次 MOVE 经 `DragEvent::apply` 走 VT 线程，
   则每次 MOVE 多一次带超时的 JNI 查询往返，与 DESIGN「可灵活拖动，流畅不卡顿」直接
   冲突，且丢掉现有的重组规避。
4. **接不上长按时序**。`Behavior` 没有「不选」的变体，长按选词靠嵌入方把 `PressEvent`
   压到长按超时后才 `apply`；单次 `apply` 的时序控制仍在本仓，收益被抵消一部分。
5. **内存**。`Gesture` 析构会暂时泄露少量 `TrackedGridRef` 内存至终端析构，需显式
   `reset`。

结论：四项优势都落在**选区语义**（吸附、词粒度、边界、自动滚动），其中词边界已经经
`Terminal::select_word` 走上游、宽度事实也已经取自上游，本仓只在策略层重写；而这些
语义收益都要以「每次 MOVE 一次带超时的跨线程往返」为代价，与 DESIGN「可灵活拖动，
流畅不卡顿」直接冲突。故维持不采用，但把依据从「DESIGN 禁多击」改为「拖动路径的往返
成本 + 语义优势已在现有实现中部分覆盖」，否则后来者会以「多击不是主体」为由重新评估。

若日后要采纳，正确形态是**只把边缘自动滚动交出去**（低频定时事件，不在每个 MOVE 上），
拖动与吸附仍留在 Kotlin 侧；本变更不做此拆分。

## 5. `DESIGN.md` 声明但实现缺失的两项

审计基础特性时发现两处声明与实现的差距。两处均已提交维护者裁定并据裁定更新 `DESIGN.md`
措辞，本变更不实现这两项本身。

### 5.1 滚动条

`DESIGN.md:176` 把「清除滚动条」写进侧边面板「重置终端」按钮的职责，但仓库内不存在
滚动条：Kotlin 侧无该组件，原生侧亦无（`Terminal::scrollbar` 未接线）。该按钮实际清零的
是滚动偏移与亚行余量（`resetActiveTerminal`）。若该句指的是滚动状态则现状成立；若指
可见滚动条则属缺失，而滚动条本身未被 `DESIGN.md` 作为界面元素声明。裁定：该句指滚动状态，
`DESIGN.md` 已改为「清除回滚与滚动状态」，不引入可见滚动条。

### 5.2 连字

`DESIGN.md:138` 要求「支持连字」，`native/src/render/font/shaping.rs` 的模块文档也写着
「接入 cosmic-text 以支持连字与复杂文种」。实测两者均不成立：

- `cell_builder.rs:610` 与 `:805` 只在单元含多个码位（即字素簇）时才调 `shape_run`，
  单码位单元直接走 `glyph_information(ch)`。整形的输入永远不跨相邻单元，而连字至少要
  两个相邻字符一起整形，故连字在现有按格渲染的结构下无法表达。
- 即便跨单元整形，`cosmic_text::Attrs::new()` 的 font features 为空，未开启 `liga`/
  `dlig`，也不会形成连字。

这不是补一个开关的事：跨单元连字字形会横跨多个网格列，与行区间划分、选区端点、命中
测试、光标对齐全部冲突，且与 `DESIGN.md` 的宽字符吸附规则语义交叠。裁定：收窄声明为
「单码位整形与复杂文种叠加」，不做跨单元连字，`DESIGN.md` 已按此更新。

`shaping.rs` 模块文档的「以支持连字」表述已按此裁定改为单码位整形与复杂文种叠加。
