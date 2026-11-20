# Design

## 为什么只补留档，不改实现

2026-11-20 12-45-04 这份日志里的设备丢失，**应用侧能做的每一步都已经按既有规范跑通**：

| 时刻 | 事件 | 规范对应 |
| --- | --- | --- |
| 12:38:40.357 | `GPU device lost`（error，唯一一条） | 「不得以日志手段掩盖设备丢失」：第一次记账是 error |
| 12:38:40.369 | `render skipped`（warn，限频） | 「设备丢失后停止向死设备提交」：后续帧在 GPU 调用前返回 |
| 12:38:40.370 | `surface invalidated (attempt 1/5)` | 「换视图预算只在回到前台时清零」：宿主换新 `SurfaceView` |
| 12:38:40.476 | 新窗口 `attachWindow` | 「换新窗口触发 GPU 重建并恢复渲染」 |
| 12:38:40.538 | `GPU rebuilt after device loss` | 重建 instance/adapter/device 与 `Renderer` |
| 12:38:40.618 | 新 surface 挂载、`setRenderPaused: paused=false` | 恢复出图 |

即：判定（一条 error、余波降级、停止向死设备提交）、换窗（预算限流）、重建（新
instance/device，不复用判死设备）、恢复（会话状态搬运、重新出帧）四条链都命中了规范，
日志里没有任何「规范未覆盖但可修」的环节。恢复耗时 261ms（丢失 → 新 surface 挂载），
全程无人工干预。

## 根因：仍不可归因，且不得猜

日志能确定的只有「驱动在首帧呈现后 5ms 异步上报了设备丢失」：

- 丢失由 **设备丢失回调**上报（`native::render::context::gpu` 的
  `register_device_lost_callback`），该次落在非渲染线程 tid 7885 上；渲染线程 tid 7887
  同期只记了首帧呈现与随后的 `render skipped`，即不是渲染线程的取帧/提交返回错误——
  是驱动主动报的 `VK_ERROR_DEVICE_LOST`，不是应用侧调用序列的返回值。
- 丢失发生在**冷启动的正常渲染路径**上：交换链已配置（`attach_surface: configured
  472x819 format=Rgba8Unorm present=Mailbox latency=2`）、管线已建好、首帧已呈现，
  与 2026-11-19 的切后台/换窗簇形态不同。
- 应用侧配置没有可指认的证据：`PowerPreference`、present mode、
  `ANativeWindow_setBuffersGeometry` 三项在 11-19 与 11-20 两批日志里都没有伴随异常
  的证据；把 present mode 改成 Fifo 或去掉缓冲几何设置都属于无根据的猜测，且会牺牲
  已被规范承认的取舍（Mailbox 的「新帧胜出」与队列几何对齐）。

因此本次只把样本与时序如实登记下来，作为后续再现时的比对基线；规范里既有的
「恢复是有界缓解而非消除，MUST NOT 表述为『已解决』」继续适用。

## 与 `logcat-hygiene` 的分工

`logcat-hygiene` 负责「这一份日志里每条 W/E 是什么、归谁」；`render-stability` 负责
「这类事件的判定与恢复语义」。11-20 的场景在 `logcat-hygiene` 里已写明「成因在
`render-stability` 的设备丢失场景逐条留档」，本 change 补上这条被引用的留档，两侧
口径一致。

## 不做的事

- 不新增恢复路径、不改判定阈值、不改换窗预算：日志未显示这些环节有偏差。
- 不把「丢失 5ms 后自愈」写成「已解决」：根因未定位。
- 不把该样本写进 `logcat-hygiene` 的归因表（那里已登记），避免同一事实两处维护。
