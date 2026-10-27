# Proposal

## Why

「上下滑动晃动」是 `docs/STATUS.md` 里唯一剩下的未解决问题。逐条排除后根因收敛到
一处结构缺口：**整行滚动是异步的，亚行余量是立即的，两者在跨行边界那一刻不同步。**

- 整行滚动经 VT 的 `scroll_viewport` 生效：命令入队后由 VT 线程处理，处理完才推出
  带新视口的帧。
- 亚行余量经渲染器的投影矩阵生效：`setScrollYPx` 当场改写待用值，下一帧开帧时写进
  uniform。

跨行边界时两者同时变：宿主把行偏移 +1、余量从 0.6h 降到 0.1h。命令还在通道里，
渲染器手上仍是**旧视口**，投影却已按**新余量**平移，画出的位置比实际视口少滚了
整行数。视觉上内容先上跳半行，等 VT 的新视口落地再下跳整行。

已有门控只挡住了一半：空闲路径有「滚动在途」门（`scroll_frame_pending`），**新帧
路径没有**。而新帧路径恰恰是陈旧帧的入口——PTY 正在输出时，VT 会持续推帧，其中
一帧早于滚动命令被处理。渲染线程消费到它就用旧视口配新余量画出来，随后又把它
当成「滚动已落地」，把在途门一并解除。

规范里其实已经写了这条要求（「滚动命令在途期间不得呈现陈旧帧」），但当时的实现
只覆盖了空闲路径，新帧路径没跟上——规范与实现不一致。

另一处同源缺口在宿主侧：行偏移未被原生接受时（会话锁被 PTY 写入占用、命令通道
满），亚行余量仍被下发，于是原生视口是旧的、余量是新的，同样是「旧视口配新余量」。

## What Changes

- 滚动命令带**请求序号**（进程级单调递增），VT 处理后把序号随帧回传；渲染侧比对
  「帧携带的序号」与「宿主已送达的最新序号」，早于在途命令的帧一律不呈现，也不
  解除滚动在途门。
- 宿主侧：行偏移未被原生接受时不再下发亚行余量，余量随行偏移同拍提交。
- 规范补上「陈旧帧的识别口径」与「余量与行偏移同拍提交」两条要求与场景。

## Capabilities

### New Capabilities

无。

### Modified Capabilities

- `render-loop-scroll-cadence`：把「滚动命令在途期间不得呈现陈旧帧」从只覆盖空闲
  路径扩到新帧路径，写明请求序号口径；补上「亚行余量与行偏移同拍提交」。

亚行余量的提交时机归入 `render-loop-scroll-cadence` 而非 `scroll-physics-drift`：
后者钉的是滚动物理本身（取整、边界丢弃、惯性曲线），本次不动；提交时机与「滚动命令
在途期间不得呈现陈旧帧」是同一处错配的两面，放在一起才不会两处描述漂移。

## Impact

- `native/src/terminal/ghostty_terminal/commands.rs`
- `native/src/terminal/ghostty_terminal/public_api.rs`
- `native/src/terminal/ghostty_terminal/types.rs`
- `native/src/terminal/ghostty_terminal/internal/{mod,commands,rendering}.rs`
- `native/src/android/ffi/render/{frame,overlay,mod}.rs`
- `android/app/src/main/java/terminal/emulator/runtime/TerminalRuntime.kt`
