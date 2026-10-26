# Proposal

## Why

真机 logcat（2026-10-25 采集）显示：每次滚动手势步进都产生**两次**帧提交——先一帧
内容未变的空闲重绘（旧视口配新余量），再一帧 VT 线程推送的滚动后内容。旧帧既多余
（多一次全量提交，软件 Vulkan 上全量帧约 500ms）又错误（画出的视口比实际少滚了
整行数，用户看到滑动慢一拍）。

根因是 `setScrollOffset` 把滚动命令发给 VT 线程后立刻置 `dirty` 标志，强制渲染线程
在**新内容到达之前**用缓存帧重绘一帧。那是「内容去重会吞掉滚动重绘」时代的补丁；
去重缺陷早已修好（滚动命令会失效去重基线并强制推帧），补丁却留下了多余的提交。

## What Changes

- 原生渲染态新增「滚动命令在途」标志：滚动命令发出即置位，新帧就位后清除。
- 滚动命令在途期间，空闲重绘 MUST NOT 发生——屏幕内容尚未反映本次滚动，
  此刻重绘只能用旧视口配新余量，画出的内容错位整行数。
- 移除 `setScrollOffset` 的 `dirty` 置位（滚动后的帧到达即唤醒渲染线程，
  无需强制重绘）。
- 空闲重绘门控抽成纯函数并补表驱动单测，钉住「滚动在途即跳过」这条判据。

## Capabilities

### New Capabilities

无。

### Modified Capabilities

- `render-loop-scroll-cadence`：新增「滚动命令在途期间不得呈现陈旧帧」的要求
  （一次滚动事件只产生一次呈现帧提交）。

## Impact

- `native/src/android/ffi.rs`：`RenderState` 新增滚动在途标志；`setScrollOffset`
  的重绘触发由 `dirty` 改为该标志；`render_inner` 的空闲重绘门控纳入该标志；
  新增纯函数门控与表驱动单测。
- `openspec/specs/render-loop-scroll-cadence/spec.md`：归档时同步新要求。
