# Proposal

## Why

`logcat-hygiene` 的 2026-11-20 场景写明「设备丢失的成因在 `render-stability` 的设备丢失
场景逐条留档」，但 `render-stability` 只记了 2026-11-19 的两簇丢失，冷启动首帧后 5ms 的
这一簇没有留档。它与 11-19 是不同形态：11-19 发生在切后台/换窗簇里，本次发生在冷启动的
正常渲染路径上（首帧呈现后 5ms，驱动经设备丢失回调异步上报）。没有这条基线，下次再现时
无法判断「又多了一簇」还是「同一簇」，也无法核对有界恢复的耗时。

## What Changes

- `render-stability` 规范补一条场景：登记 2026-11-20 12-45-04 冷启动设备丢失的完整
  时序（首帧 → 丢失 → 换窗 → 重建 → 恢复出帧）、丢失的报告路径（GPU 轮询线程上的
  设备丢失回调）与恢复耗时（261ms，无人工干预）。
- 同场景如实写明：本次丢失的底层根因仍未定位；应用配置（`PowerPreference`／present
  mode／`ANativeWindow_setBuffersGeometry`）仍无证据为成因；恢复是有界缓解而非消除，
  MUST NOT 表述为「已解决」。

## 不做什么

- 不改设备丢失的判定、恢复与换窗实现：本次样本的恢复链已按既有规范跑通，没有可修的
  应用侧环节。
- 不以改 present mode、改缓冲几何或降级日志来「预防」这次丢失：日志里没有任何证据
  指向应用配置，改配置即是无根据的猜测。
- 不动 `docs/STATUS.md`：该文件当前无未解决问题条目，且首行声明禁止修改。

## Capabilities

### New Capabilities

（无）

### Modified Capabilities

- `render-stability`: 「换新窗口触发 GPU 重建并恢复渲染」新增冷启动首帧后设备丢失的
  留档场景与如实记录要求。

## Impact

- Affected specs: `render-stability`（新增一条场景）。
- Affected code: 无（纯规范留档，不改实现）。
- 行为变化：无。
