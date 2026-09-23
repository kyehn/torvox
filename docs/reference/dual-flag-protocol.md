# 双标志协议（dual-flag protocol）

渲染线程每帧消费两个独立标志。两者正交：新输出可将视口复位到底部，
非输出变更只重绘、永不复位视口。

## P1-1 `new_output`（滚动复位信号）

- 置位：PTY 摄取路径收到非空块（`OutputProcessor::process`）。
  空块不计数。
- 消费：渲染线程经 `take_new_output` / `consumeNewOutput` 单次读清。
- 形态：旁路标志，不是队列事件——输出洪峰下排队事件会被淹没，
  故与 `pollAll()` 循环并列读取。
- 语义：为真表示本帧有新终端输出，视口可复位到底部。

## P2-1 `dirty`（内容脏信号）

- 置位：变更延迟渲染输入的 JNI 入口
  （`setSearchHighlights` / `clearSearchHighlights` / `setFontSizeInPlace`）。
- 消费：渲染线程在 `render_inner` 内一次 `getAndSet(false)` 交换取走。
- 语义：高亮/字号变更只触发重绘，永不复位视口。
- 同时是 Kotlin 渲染循环的唤醒信号
  （UI 调用方 `notifyRender()` + 兜底 latch 节奏）。
