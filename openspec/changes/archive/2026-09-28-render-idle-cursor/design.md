# Design

## Context

`renderWithNewOutput` 已把光标采样移出 `count > 0` 门，空闲帧同样上报，见 `proposal.md - Why`。此前实现随后对会话调用 `render_cursor()`；该方法经通用查询通道实现，上限为 `QUERY_TIMEOUT_MS`（500ms），见 `native/src/terminal/ghostty_terminal/types.rs:241` 与 `public_api.rs:445`。`render_inner` 已有避免同步查询的先例：回滚长度搭载在 `CursorInfo` 上，而不是调用 `scrollback_length()`，见 `native/src/android/ffi.rs:1497`。

## Goals / Non-Goals

**Goals:**

- 上报坐标与本帧实际渲染的 `CursorInfo` 同源。
- 光标采样不新增发往 VT 线程的同步查询。

**Non-Goals:**

- 不改变 `render_inner` 空闲返回 0 的语义。
- 不新增 JNI 入口。
- 不调整通用查询通道的 500ms 上限。

## Decisions

- 光标行取自 `render_inner` 刚更新或复用的 `RENDER_STATE.last_frame.CursorInfo`，而不是会话的 `render_cursor()`。
  - 新帧成功时缓存即本帧；空闲帧复用同一缓存；隐藏或视口外仍为未知。
  - 备选缩小通用查询超时不可行：VT 循环按 50ms 排空查询，见 `internal.rs:667`，过短超时只会制造未知回退，并影响其他查询。
- 保留 `getCursorViewportPacked` 的直接查询路径，只用于诊断，不进入每帧上报。
- 抽出纯函数映射缓存光标到上报位，并用单元测试锁定未知、可见与隐藏三种情形；行为仍由现有 `ImePopupPixelInstrumentedTest` 覆盖。

## Risks / Trade-offs

- [Risk] 缓存可能与直接查询存在同一帧内的瞬时差 → Mitigation：跟随平移本就应与 GPU 绘制帧一致，直接查询反而可能读到更新但尚未绘制的坐标。
- [Risk] 渲染状态是全局的 → Mitigation：渲染调用本身按请求会话更新缓存，Kotlin 渲染循环只调用活动会话，不存在并发渲染竞争。
- [Risk] 文档曾把超时写成 200μs → Mitigation：本设计以 `QUERY_TIMEOUT_MS` 为准，并同步修正相关注释。

## Migration Plan

- 无数据迁移；回滚即还原本提交，行为由目标仪器测试验证。
