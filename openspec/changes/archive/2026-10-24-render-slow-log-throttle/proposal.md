# Proposal

## Why

`logcat-hygiene` 规范早已要求「逐帧诊断限频：两次输出间隔不小于 500ms」，宿主侧的
慢帧诊断（`SLOW_FRAME` / `SLOW_LOOP`）也一直守着这条线。但原生侧的同类诊断
——`SLOW_RENDER_INNER`（帧内分段）、`SLOW_RENDER_PHASE`（渲染分段）、
`SLOW_CELL_BUILD`（单元构建）、`SLOW_RENDER_SAMPLE`（采样段）——**逐帧都记**：
条件是「本帧超过 30ms」，没有时间闸门。

证据来自三份 logcat 审计：其中一份里 `SLOW_RENDER_INNER` 单类就出了 542 条，
最小间隔 100ms，与规范要求的 500ms 下限差 5 倍。同期取纹理相关的逐帧告警
（工作线程忙/panic/超时）与「尺寸不符」告警同样没有闸门，在 GPU 卡住或重配未生效时
会逐帧刷屏。

限频缺失的实际代价不是「日志多」：慢帧洪流会把真正的告警挤出可读范围——
排障时最先被淹没的正是最该看的那条。

## What Changes

- 新增 `render::slow_log::SlowLogThrottle`：进程内单调时钟 + CAS 推进窗口的限频闸门。
  首次放行、窗口内压住、时钟回跳不放行（回跳按「未到窗口」处理，宁可少记不误放）。
- 四类原生慢帧诊断与四类取纹理告警接入闸门，**每类各持一个闸门**——
  共用计数器会让先到的诊断吞掉后到的（规范明令禁止）。
- 升级信号不受影响：连续跳过计数到上限仍照旧记 error，取纹理超时仍逐帧重试。
- 规范补齐：该限频要求 MUST 同时覆盖宿主侧与原生侧，并写明首条必须可见、
  限频不得吞掉升级信号。

## Capabilities

### New Capabilities

无。

### Modified Capabilities

- `logcat-hygiene`：逐帧诊断限频要求的适用范围（宿主侧 + 原生侧）、首条可见、
  限频不得吞掉升级信号。

## Impact

- `native/src/render/slow_log.rs`（新增：限频闸门及其单元测试）
- `native/src/render/mod.rs`（挂载新模块）
- `native/src/render/pass.rs`（渲染分段、单元构建、取纹理三类告警接入闸门）
- `native/src/render/context.rs`（尺寸不符告警接入闸门）
- `native/src/android/ffi.rs`（帧内分段、采样段接入闸门）
- 不改 `docs/specification/` 与 `docs/STATUS.md`。
