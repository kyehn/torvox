# Proposal

## Why

三处函数签名把同一组数据拆成散参数逐个传递，字段清单在多处各写一遍：

- 查询应答路径：`process_query` 收 `all_text` 与两个默认色，`drain_queries` 原样
  再传一遍——三者的真正归属是 `CommandState`（`cached_all_text`、
  `default_foreground`、`default_background`）。
- VT 线程主循环：查询通道句柄被克隆一份再逐层传递，而 `RunConfig` 里本就存着
  同一个句柄。
- 渲染路径：`FrameData::New` 的六个字段在 `render_inner` 里拆开、再逐个传给
  `render_new_frame`，同一份字段清单写了两遍。

散参数传法改一处字段便要同步改多处签名，漏一处即编译不过或传错顺序；参数个数
本身也逼近可读性上限。

## What Changes

- 查询应答路径改传 `&mut CommandState`：去掉 `all_text` 与两个默认色三个参数。
- VT 线程主循环不再克隆查询通道句柄，直接用 `RunConfig` 里的那一份；
  `wait_for_event`、`finish_command_batch`、`respond_queries_and_publish`
  少收一个参数。
- 渲染路径把新帧数据成组为 `NewFrame`，`render_new_frame` 由七个参数降为两个。

## Capabilities

### New Capabilities

- 无

### Modified Capabilities

- 无：本次只改函数签名，不改任何可观察行为。

## Impact

- `native/src/terminal/ghostty_terminal/internal/dispatch.rs`、`internal/run.rs`：
  查询应答与主循环的签名。
- `native/src/android/ffi/render/frame.rs`：新帧数据的组织方式。
- 不改 `docs/specification/`、`docs/STATUS.md` 等保护文件。
- 不引入新依赖。
