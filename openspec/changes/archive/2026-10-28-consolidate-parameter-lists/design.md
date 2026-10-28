# 设计

## 事实一：查询路径的三个参数本就属于 `CommandState`

`CommandState` 已把「与终端借用无关的可变状态」收在一处（见 `internal/mod.rs`
的说明）：`cached_all_text`（全量文本缓存）、`default_foreground`、
`default_background` 都在其中。查询应答路径却把它们拆成三个参数逐层传递，
其中 `all_text` 传的是 `&mut state.cached_all_text`——即调用方先把字段的引用
取出来再传下去。

改为直接传 `&mut CommandState`：字段归属回到唯一的持有者，调用点不再重复
「取字段引用」这套写法。借用检查不受影响——`state` 与 `terminal`、`extractors`、
`query_context` 是互不相干的对象，字段级借用（`state.cached_all_text` 可变借用
与 `state.default_foreground` 复制）本就允许。

## 事实二：查询通道句柄的克隆已无必要

`run_inner` 曾克隆 `config.query_receiver`，注释写明是为了「使下面的命令循环仍能
借用 config，无需部分移动」。如今 `config` 全程按引用传递（`&config`），不再有
部分移动，克隆的唯一作用变成把同一个句柄在主循环里多传几层。

`flume::Receiver` 是 `Arc` 句柄，克隆与借用等价；`flume::Selector::recv` 收的是
`&Receiver`，生命周期绑定在选择器上，直接借 `config.query_receiver` 即可。

## 事实三：新帧数据在两处各写一份字段清单

`FrameData::New` 的六个字段在 `collect_frame_data` 组装、在 `render_inner` 拆开、
再逐个传给 `render_new_frame`。字段增删要改三处。

改为 `NewFrame` 结构体：`FrameData::New(NewFrame)`，`render_new_frame` 只收
`session_id` 与整组数据，函数入口一次解构出局部变量，函数体其余部分不变。

## 取舍：为什么不把所有参数塞进一个大结构体

`Terminal`、`RenderExtractors`、`QueryContext` 之间存在借用关系（`Search` 的
生命周期挂在终端的分配器上），无法收进同一个结构体——这正是 `CommandState` 只
收「与终端借用无关的量」的原因。本次只收敛真正同源的那几组参数，不做跨生命
周期的强行打包。

## 验证

- 行为不变的重构：`cargo test --all-targets --no-fail-fast` 全绿（743 例）。
- `cargo clippy --all-targets -- --deny warnings` 与 `cargo fmt --all -- --check` 通过。
