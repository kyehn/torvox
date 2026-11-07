# Proposal

## Why

当前部分帧（脏带路径）虽然只重绘脏行，但 `render_frame_with_plan` 仍把整份 `instances`
（含干净行的缓存实例）上传到 GPU。在突发输出或滚动期间，脏行通常只占全部行的一小部分，
全量上传浪费 CPU→GPU 带宽与队列时间，成为部分帧路径的隐藏成本。

## What Changes

- 部分帧时，实例上传只覆盖 `FramePatch.bands` 中每个脏带的 `[instance_start, instance_end)`
  切片，不再上传干净行的缓存实例。
- 全量帧与现有路径保持不变，继续上传全部实例。
- 不改变绘制命令、脏带几何与累加器语义；只减少上传字节数。

## Capabilities

### New Capabilities

无。

### Modified Capabilities

- `render-stability`：新增「部分帧只上传脏带实例」要求（上传范围与绘制槽位同坐标系）。

## Impact

- `native/src/render/pass/frame.rs`：`render_frame_with_plan` 的实例上传循环
- `native/src/render/pass/bands.rs`：脏带切片与上传布局复用
- 测试：上传字节数/脏带切片的表驱动用例
- `openspec/specs/render-stability/spec.md`：部分帧上传范围的要求
