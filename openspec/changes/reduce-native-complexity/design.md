# Design

## Context

### `render_inner` 的形状

函数体本身已经用中文注释分成了四段，段落边界就是天然的切分点：

1. **阶段 0**（切会话后索要一帧）：读一次渲染状态缓存里的会话号，会话不同就
   向目标会话索要重推。
2. **阶段 1**（渲染前准备）：一次持有渲染状态锁，检查 surface、惰性建管线、
   上传字形图集脏区。
3. **阶段 2**（收集单元数据）：只持会话锁，从通道取本帧单元数据；空闲时只记
   事实，不克隆整帧。
4. **阶段 3**（渲染）：持渲染状态锁，按新帧/空闲两条路径绘制。

四段之间的锁序约束是硬性的：**渲染状态锁内绝不嵌套注册表锁与会话锁**。阶段 3
的「新帧」路径因此会在中途显式 `drop` 渲染状态锁去查会话，再重取一次锁应用
结果——这正是它无法随手切成小函数的原因：拆出来的函数必须能把「需要重取锁」
这件事表达清楚。

### `run_inner` 的形状

555 行里，前 170 行是终端初始化与上游回调注册（回滚上限、Kitty 介质、
PTY 回写回调、OSC 52 写回回调、XTWINOPS 尺寸回调、编码器、渲染提取器、
初始主题、Kitty 缓存），后 380 行是事件循环本身。初始化段没有依赖循环里的
任何可变状态，是纯粹的前置装配。

### `pump_sessions` 的形状

156 行分三段：采集本帧要处理的会话句柄 → 逐会话推进（活跃会话全量、后台
会话每帧 2 块）并收割剪贴板读请求与退出事件 → 处理剪贴板读请求并把事件推入
队列。第三段自成一体：它处理的是前一段攒下的 `pending_clipboard_reads`，不碰
会话推进逻辑。

### `MockPty` 的退出态

`MockPtyInner::child_exited` 只被 `set_exited` 写成 `true`，而 `set_exited`
没有任何调用方。于是：

- `MockPty::write` 里的 `BrokenPipe` 分支永不执行；
- `MockPty::read` 里的 `Ok(0)` 分支永不执行；
- `MockPtyHandle::is_exited` 恒返回 `false`。

## Goals / Non-Goals

**Goals:**

- 三个超长函数降到可逐段核对的规模。
- 删掉全仓不可达的代码路径。
- 消除新帧/空闲帧两条路径之间重复的脏行掩码构建。
- 重构前后行为**逐条等价**：锁序、返回值、状态写入时机都不变。

**Non-Goals:**

- 不改 `render_inner` 的空闲重绘门控语义（`should_repaint_idle` 的判据不动）。
- 不改 Kitty 缓存的失效规则，不动 `kitty_cache_invalidation`。
- 不改 VT 线程的命令语义与查询应答时机。
- 不做大文件拆分（`ffi.rs` 6005 行等）：本轮只切函数，文件拆分另议，避免
  一次改动同时动结构与逻辑。
- 不为「凑行数」引入一次性包装类型。

## Decisions

### 一、`render_inner` 按阶段拆成四个函数

拆分边界直接沿用函数体内已有的阶段注释，函数名用阶段原话：

- `prepare_render(session_id) -> bool`：阶段 0 + 阶段 1。返回 `false` 表示
  本帧不渲染（渲染状态缺失、surface 未挂载、图集尺寸未就绪）。
- `collect_frame_data(session_id) -> Result<FrameData, jint>`：阶段 2。`Err(0)`
  表示暂停中或 surface 丢失、`Err(-1)` 表示渲染状态缺失或会话不存在——
  退出码即原先 `render_inner` 的返回值。
- `render_new_frame(...)`：阶段 3 的新帧路径。
- `render_idle_frame(...)`：阶段 3 的空闲路径。

`FrameData` 枚举从函数体内提到模块级：它现在是两个函数的返回类型，
藏在函数体里没法在签名里引用。

**为什么不把阶段 3 整段再切细：**阶段 3 内部必须在「渲染状态锁 → 释放 →
查会话 → 重取渲染状态锁」之间来回。切成函数后，函数要么接收锁守卫（生命
周期与调用点交织），要么把「需要重取锁」的结果作为参数传回。前者把锁的
生命周期藏进签名，后者把控制流拆成两半且需要额外的错误通道——两者都比现在
更难核对。故阶段 3 只按「新帧 / 空闲」两条路径切分，这两条路径各自是直线
流程，切开后每条都在 200 行以内。

### 二、脏行掩码构建抽成一个 helper

两条路径的掩码构建有共同骨架：清空 → 按行数扩容 → 标脏光标行（新帧还多标
上一帧的光标行）→ 标脏高亮行 → 整屏标脏。差异只有两点：上一帧光标行只有
新帧路径有；整屏标脏由 `full_repaint` 标志决定，两条路径都有。

故 helper 形如
`mark_dirty_rows(dirty_mask, rows, cursor_row, previous_cursor_row, current_highlights, last_drawn_highlights, full_repaint)`
：上一帧光标行以 `Option<u32>` 传入，`None` 表示没有上一帧（新帧路径在会话
切换后的首帧就是这种情况，空闲路径恒为 `None`）；高亮的当前集合与已绘制集合
一并传入，行号由 helper 内部按「新增 + 消失」求差集。掩码的分配（`clear` +
`resize`）留在各路径内，因为它依赖的是各自持有的行数来源（`rows` 与
`cached_rows`）。

**为什么不抽成返回 `Vec<bool>`：**掩码是 `render_state.dirty_mask` 的原地
写入，渲染器随后以 `Some(&render_state.dirty_mask)` 读取它。返回新向量会
多一次每帧的分配与拷贝，与「高性能、低内存」相悖。

### 三、`pump_sessions` 的剪贴板段独立成函数

抽出 `handle_clipboard_reads(pending_clipboard_reads, pending_events)`，它只
需要「待处理的读请求」与「事件收集器」两样东西，不碰会话推进状态。洪泛削减
的每帧上限（`MAX_FLOOD_SHED_ANSWERS_PER_FRAME`）仍在这个函数里就地计数，
因为它统计的就是本函数内的削减次数。

**为什么不抽事件推入：**`pending_events` 推入队列必须在注册表锁与会话锁之外，
而抽取后主泵里那一段本来就只剩三行，抽出反而多一层间接。

### 四、`run_inner` 的初始化段独立成函数

抽出 `configure_terminal(config: &RunConfig) -> Option<Terminal>`，返回
`None` 表示初始化失败（`Terminal::new` 失败、`Encoders::new` 失败、
`RenderExtractors::new` 失败），调用方据此退出 VT 线程。

**为什么不把事件循环也切细：**事件循环里的 `while let Some(command)` 分派
要同时改动十来个可变状态（`terminal`、`extractors`、`cached_snapshot`、
`grid_dirty`、`cached_all_text`、`last_cell_data_push`、两个默认色、批内
脏标记、应答通道）。把它们打包成一个结构体传给分派函数，等于把「命令处理」
重写成另一套形状——改动面远大于收益，且命令语义（每条命令的失效时机）是
本仓最不该在重构中动的地方。故本轮只抽初始化段。

**为什么不抽「渲染提取器 + 编码器」的创建：**它们与 `Terminal` 的生命周期
绑定，但返回值类型各不相同（`Encoders`、`RenderExtractors`），打包进返回值
需要引入一个一次性结构体。留在主函数里各占几行，收益低于引入的间接成本。

### 五、删除 `MockPty` 的退出态

`set_exited` 无调用方 → `child_exited` 恒假 → 两处分支不可达 → 连同
`is_exited` 一并删除。删除后 `MockPty` 仍能表达「子进程已退出」吗？不能——
但它现在也不能（恒假）。退出态在替身上从未被使用过，真实的退出语义由
`Session::request_exit` / `Session::is_exited` 承担。

## Risks / Trade-offs

- **拆分引入行为漂移**：缓解方式是每拆一处就把拆出的函数体与原函数体逐行
  比对（锁的获取/释放点、返回值、状态写入顺序），并跑全量 Rust 单测。
- **`FrameData` 提到模块级后变成 crate 内可见的类型**：它只在本文件内使用，
  用私有的模块级枚举即可，不进 `pub` API。
- **脏行掩码 helper 的参数变多**（6 个）：这 6 个参数在两条路径里都已有同名
  局部量，helper 反而让两处调用点读起来一致。参数数量本身由 clippy 的
  `too_many_arguments` 规则覆盖（该项目已按规范在 `Cargo.toml` 里整体
  放行该风格规则）。
- **VT 线程初始化段抽出后，错误日志的时机不变**：所有 `log::error!` 仍在
  同一位置发出，线程仍在同一时刻退出。
- **`pump_sessions` 抽出的函数需要 `pending_events` 可变引用**：签名里多一个
  `&mut Vec<Event>` 参数，这是必要的，事件要由主泵统一推入队列。

## Migration Plan

- 无迁移。纯重构，行为由既有单测与逐段比对保证。
- 回滚即还原各函数拆分（每个拆分点一个提交）。
