# Proposal

## Why

对刚落地的 OSC 9 / OSC 777 桌面通知做代码审查时发现两处会随通知洪泛放大的问题：

1. 运行期的通知排空是无界循环（`TerminalRuntime.drainNotifications`）：取到一条通知就
   再取一条，只要通知的产出快于消费，这个循环就永不结束——轮询协程再也回不到它的等待
   点，投递速率随之失控（Toast 连发）。事件队列的排空有每帧上限（`MAX_EVENTS_PER_POLL`），
   通知队列却没有同口径的约束。
2. 原生侧轮询入口每被调用一次就推进一次会话（`poll_notification_event_inner` 先推进再
   取），而排空循环每取一条通知就调一次入口：一次收割被放大成「每条通知一次全量会话
   推进」。推进要取会话锁并解析 VT 输出，与渲染线程的热路径争锁。

两处都只影响通知路径，但通知是远端程序可无限触发的输入（`printf '\e]9;x\a'` 一秒能
刷出上百条），因此按「洪泛下不失控」的口径收口。

## What Changes

- 通知排空改为有界：每轮最多排空固定条数，其余留在原生队列（原生队列本身有界，最旧的
  会被淘汰并告警）。
- 原生侧轮询入口改为「先取已入队的通知，取不到才推进会话」：收割只在队列为空时发生，
  不再为每条通知推进一次会话。
- 规约 `desktop-notification` 增两条要求：排空有界、轮询入口只在无通知可取时推进会话。

## Capabilities

### Modified Capabilities

- `desktop-notification`：增「通知排空必须有界」与「轮询入口只在无通知可取时推进会话」
  两条要求。

## Impact

- 原生侧：`native/src/android/ffi/wake.rs` 的 `poll_notification_event_inner` 调整取用与
  推进的先后。
- Android 侧：`runtime/` 抽出可单测的通知排空函数并加上限；`TerminalRuntime` 改为调用它。
- 不改依赖、不改协议行为：通知的采集、分派形态与点击回跳均不变。
