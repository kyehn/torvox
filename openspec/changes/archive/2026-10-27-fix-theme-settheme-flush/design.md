# Design

## Context

JNI `setTheme` 与 VT 命令队列异步：旧实现先标脏再等 VT 推送，导致渲染线程用旧单元数据
重绘。根因是时序，不是颜色数据本身（VT 侧 `apply_theme` 已正确重着色已有单元）。

## Goals / Non-Goals

- Goals：保证主题切换后首帧即为新主题底色，不留混屏。
- Non-Goals：不改变主题目录、颜色解析与日夜跟随逻辑。

## Decisions

- 在 JNI `setTheme` 中，`session.terminal().set_theme(entry)` 后调用 `flush()`，阻塞至 VT
  处理完成并推送新单元数据。
- flush 完成后再更新 `render_state`（背景色、主题、实例缓存、脏标志、整幅重绘）。

## Risks / Trade-offs

- `flush()` 最多阻塞 5s；主题切换为低频操作，可接受。
- 若 flush 超时，仍标脏，避免渲染永久停滞。
