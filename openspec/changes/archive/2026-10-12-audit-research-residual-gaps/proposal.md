# Proposal

## Why

`docs/research/` 的 29 份文档已全部通读。其收敛结论
（`27-改进点收敛与openspec落点.md` §1）为「新增 openspec 能力：0」，但该结论本身建立在
「已覆盖 = 规范或在途变更已有对应条款」这一口径上，而口径的判定未经逐条复核。本次把每一条
可落地项拿回仓库现状核对，证实 4 处残留：1 处是 `DESIGN.md` 已声明但未落地的性能要求，
2 处是研究文档点名的实现与文档失真，1 处是既有不采用结论的依据不足。审计基础特性时另发现
2 处 `DESIGN.md` 声明与实现冲突，一并记录并已提交维护者裁定。

### 依据一：行级脏跟踪在 VT 侧从未生效（研究文档 §3.5）

`docs/libghostty.md` §3.5 曾记载「行级脏跟踪不启用：`build_cell_data` 每帧新建
`RenderState`，`rows = 0` 初值使 `update` 走「维度变化 → 全量重建」分支，全部行恒为脏，
实测命中率 0%」。该结论在 `native/src/terminal/ghostty_terminal/internal.rs:1458-1466`
被原样写成代码注释，并以「不做行级缓存——那只会逐帧多一次整网格深拷贝与分配而永不命中」
为由收口。

收口理由只覆盖了**渲染侧**：渲染侧确有行级增量（`native/src/render/cell_builder.rs` 的
`diff_dirty_rows_into`、`build_instances_cached`、`CachedInstances`）。但**抽取侧**并未因此
变得无关紧要：`internal.rs:820-824` 自述每次重建「遍历 ~2444 次 Ghostty FFI 调用」，
而每次重建的触发条件是 `batch_dirty`——即一个渲染帧内有 PTY 输出。持续输出时每帧仍要
把整张网格逐格过一遍跨语言边界，而其中绝大多数行逐字节未变。

本次已用探针实测复核（探针跑完即删，按 `upstream-alignment` 的「判定完成不留红测试」）：
同一 `RenderState` 跨帧复用、首帧写满 24 行、次帧只在第 0 行写一个字符并逐帧标记快照已
消费，脏标记为 `[true, false × 22, true]`——仅写入行与光标离开的末行判脏，其余 22 行
干净；去掉快照已消费标记后 24 行全脏。故行级判定在上游确实可用，0% 命中率是两个条件
同时不成立的合流，详见 `design.md` §1.2。

`docs/specification/DESIGN.md:118` 要求「脏跟踪，跳过干净快照，**跳过逐行复制**，减少突发
输出期间的工作量」。跳过干净快照已实现（`snapshot_needs_rebuild`），跳过逐行复制在抽取
侧从未落地。

### 依据二：振铃回调通道的注释描述了不存在的实现（研究文档 §4）

`docs/libghostty.md` §4 原文：「`Terminal::on_bell` 未接线，而 `commands.rs` 的
`RunConfig` 注释描述了一条 BEL 回调事件通道（含「每次振铃一个空消息」）。注释与实现
不一致，二者需取一：接上回调或删掉该段注释。」

复核确认该不一致仍在，且不止两处：

- `native/src/terminal/ghostty_terminal/commands.rs:197-198` 的文档注释描述
  `bell_tx` 通道，但 `RunConfig` 无此字段，该注释悬空地附着在 `cell_size_px` 上——
  读者会以为单元格像素几何来自一条振铃通道。
- `native/src/terminal/ghostty_terminal/types.rs:257` 称 `EVENT_CHANNEL_CAPACITY`
  承载「剪贴板写入、振铃」，而该常量的唯一使用点是 `public_api.rs:33` 的剪贴板通道。
- `native/src/terminal/session.rs` 的收割路径另有 5 处文档注释（`:525`、`:533`、`:539`、
  `:575`、`:1185`）同样把 BEL 列为回调事件。

`upstream-alignment` 已判定 `on_bell` 不采用（Kotlin 侧零消费者），故取「删注释」一侧。
`STYLE.md`「不得保留死代码」「代码注释和文档保持极简」同样指向删除。

### 依据三：选择手势类上游 API 的不采用依据不完整

`upstream-alignment` 的台账把 `selection/gesture` 一族列为不采用，理由是「DESIGN 禁双击
三击多击选择」。该理由只覆盖连击计数，未覆盖该族真正的主体：拖动手势与边缘自动滚动。
本次核对了上游 `Gesture`、`PressEvent`/`DragEvent`/`AutoscrollTickEvent` 的接口与本仓
拖动路径的实际形态，结论与台账一致但依据不同，逐条记录于 `design.md` §4。

### 依据四：`terminal-state-regression` 的 Purpose 引用了不存在的条款

`openspec/specs/terminal-state-regression/spec.md:5` 与 `:16` 两处写「
`docs/specification/TESTING.md` 明确不进行回归测试」并据此作为不设金标语料的依据。
`docs/specification/TESTING.md` 全文没有「回归测试」「快照」「语料」三处表述，其
对应条款只有「不保留无意义或低价值测试」。该引用是删除语料装置时写入的、与被引用
文件不符的说明。`终端模拟器生态深度研究报告.md` §19、§21 两节均记录过同类误判的纠正
先例（把 xterm.js 语料误判为自验证、把 kotlin-app 单测数误判为 23 文件 145 方法），口径
要求以被引用文件的实际内容为准。

## What Changes

- 新增 `grid-extraction-incremental` 能力：VT 线程持有跨帧复用的 `RenderState`，逐帧标记
  快照已消费，按上游行级脏标记跳过未变行的逐格抽取，干净行自上一帧缓存复制；尺寸变更、
  主题变更与终端重置使缓存失效并退化为全量重建。
- 修改 `upstream-alignment`：已判定不采用的上游回调，其通道不得以注释形式留在代码中；
  选择手势类上游 API 的不采用依据须覆盖拖动路径的跨线程往返成本。
- 修改 `terminal-state-regression`：把「不做回归快照」从 Purpose 里的失实引用改为可执行
  的行为条款，断言覆盖口径与删除语料装置的真实理由一并写明。
- 两处 `DESIGN.md` 声明与实现的冲突（滚动条、连字）已提交维护者裁定并据裁定收窄声明，
  本变更不实现这两项本身。

## Capabilities

### New Capabilities

- `grid-extraction-incremental`：VT 线程把终端状态抽取为 `CellData` 的行级增量口径——
  `RenderState` 的跨帧生命周期、快照已消费标记、脏行判定来源、缓存失效条件，以及
  「跳过逐行复制」不得换来陈旧帧的约束。

### Modified Capabilities

- `upstream-alignment`：不采用的上游回调在代码中的残留形态；选择手势类上游 API 的不采用
  依据。
- `terminal-state-regression`：网格状态断言的覆盖口径与快照装置的存废依据。

## Non-goals

- 不接线 `Terminal::on_bell` 及任何桌面通知、进度上报回调。`upstream-alignment`
  已判定其无 Kotlin 消费者，本变更只清理与之矛盾的注释。
- 不改「跳过干净快照」的网格级缓存（`snapshot_needs_rebuild`）语义，也不改渲染侧的
  `CachedInstances` 行级增量——两者保持原样。
- 不采纳上游 `selection::gesture` 的拖动与自动滚动，不重构 Kotlin 侧选择拖动路径。
- 不实现跨单元连字：`DESIGN.md:138` 的声明与按格渲染结构冲突，已按裁定收窄为单码位整形与
  复杂文种叠加（`design.md` §5.2）。
- 不新增滚动条等未在 `DESIGN.md` 声明的界面元素。
- 不改回滚行数、网格尺寸来源、单元格度量与 JNI 几何口径。
- 除已裁定的声明收窄外不触碰 `docs/specification/`。`DESIGN.md:118` 已是本变更的依据，
  无需新增声明。

## Impact

- Rust：`native/src/terminal/ghostty_terminal/internal.rs`（`RenderState` 移入 VT 线程
  循环、新增快照已消费标记、行缓存与失效判定）、`commands.rs`、`types.rs` 与
  `render/font/shaping.rs`（删除失真注释）。
- 测试：行级抽取须有「跨帧后未变行判干净」「未标记已消费则脏状态不重置」「增量与全量
  一致」「失效后退化为全量重建」等用例，并测量改动前后的单帧抽取耗时；沿用现有
  `cargo test` 与 `native/benches/` 体系，不新增测试框架。
- 规范：新增 `grid-extraction-incremental` 一项；`upstream-alignment` 增补两项
  Requirement（回调通道注释、手势 API 判定口径），`terminal-state-regression` 增补一项。
