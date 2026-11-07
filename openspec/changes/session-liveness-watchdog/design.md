# 设计

## 背景与现状

当前会话挂起的可观测面只有 `RenderWatchDog`：

- 它以 `entry.frameMarks`（`startNanos` / `doneNanos`）为输入，每 2s 检查一次。
- 判定条件是 `startNanos > doneNanos` 且空闲超过 10s 且 `isRunning()` 为真。
- 命中后只把 `entry.renderThreadExited` 置真，由 `checkSessions` 以指数退避重启渲染线程。

这条路径有两个盲区：

1. **层级盲区**：`frameMarks.startNanos > doneNanos` 只说明“整帧没跑完”，不能区分是
   渲染线程卡在 `bridge.render()`、`pollAll()`、还是 Kotlin 侧事件派发；VT 线程与
   PTY 读取线程的停滞完全不可见。
2. **恢复盲区**：渲染线程重启不能解决 VT 线程或 PTY 读取线程挂起；若挂起发生在
   原生侧，重启渲染线程只是反复把挂起会话判死，最终 `closeDeadSession` 关掉用户的 shell。

现有代码已经具备部分基础：

- `RenderWake` 是“代数 + Condvar”的事件唤醒，输出到达与 Kotlin 信号共用它。
- `SessionEntry` 持有 `render_wake`、`new_output`、`alt_screen_active` 等原子镜像，
  渲染线程不必取会话锁即可读取关键状态。
- `Session::take_reported_exit` 已把“退出码到手才上报”作为不变量，避免伪造 0。
- `join_with_timeout` 已为线程回收提供有界等待，避免拆卸被阻塞。

因此改进应复用这些机制，而不是另起一套线程模型。

## 设计目标

1. **可定位**：挂起时能报告层级（渲染 / VT / PTY 读取 / Kotlin 生命周期）与最近进展
   时间，而不是一个布尔“挂了”。
2. **可恢复**：渲染层挂起可重建渲染线程；VT 或 PTY 读取层挂起必须关闭并重建会话，
   不能只重启渲染线程。
3. **不破坏不变量**：PTY 字节一致性、背压、退出上报、会话锁纪律保持不变。
4. **可观测**：诊断信息进入现有日志与会话表，不引入新权限、通知或后台服务。
5. **可测试**：层级判定与恢复决策必须是纯函数，可 JVM/Rust 单测。

## 分层存活模型

### 三层代数

每个会话维护三个单调递增的“进展代数”，分别由对应线程在关键点推进：

| 层级 | 代数 | 推进点 | 含义 |
| --- | --- | --- | --- |
| PTY 读取 | `ptyReadGeneration` | 读取线程每次成功 `read()` 或通道发送完成后 | PTY 读取链路仍活 |
| VT | `vtGeneration` | VT 线程每次完成 `receive_cell_data` / 状态更新后 | VT 解析链路仍活 |
| 渲染 | `renderGeneration` | 渲染线程每次完成 `renderWithNewOutput()` 后 | 渲染链路仍活 |

代数只增不减，允许回绕（`wrapping_add`），比较时按“是否变化”而非绝对大小。

### 最后进展时间

每个代数配套一个 `lastProgressNanos`，由对应线程在推进代数时以 `System.nanoTime()`
（Kotlin 侧）或 `Instant::now()`（Rust 侧）写入。看门狗只读这些时间，不取会话锁。

### 层级判定

看门狗每 2s 读取三个代数与时间，按以下优先级判定：

1. 若 `renderGeneration` 超过阈值未推进，且 `vtGeneration` 与 `ptyReadGeneration` 近期推进：
   **渲染层挂起**。
2. 若 `vtGeneration` 超过阈值未推进，且 `ptyReadGeneration` 近期推进：
   **VT 层挂起**。
3. 若 `ptyReadGeneration` 超过阈值未推进：**PTY 读取层挂起**。
4. 若三层都未推进但会话仍 `running`：**Kotlin 生命周期层挂起**（渲染线程可能已死或未启动）。

阈值按层级不同：渲染 10s、VT 15s、PTY 20s，避免把正常空闲误判为挂起。空闲会话的
`lastSignalNanos` 与滚动窗口必须作为“近期推进”的补充证据：空闲但未挂起时，渲染层
代数不必每帧推进，但 VT/PTY 层在有输出时必须推进。

## 恢复策略

| 层级 | 动作 | 理由 |
| --- | --- | --- |
| 渲染 | 重启渲染线程（现有逻辑） | 渲染线程无状态，重启代价最低 |
| VT | 关闭会话并重建 | VT 状态由 Rust 持有，无法安全热重建 |
| PTY 读取 | 关闭会话并重建 | 读取线程持有 PTY fd 与通道，无法安全热重建 |
| Kotlin 生命周期 | 不自动恢复，只记录并等待用户操作 | 避免在 Activity 重建/切后台期间误杀 |

重建会话时必须：

- 先断开输出通道，使阻塞的发送端立刻返回（复用 `request_exit` 的通道换新）。
- 以有界等待回收读取/等待线程（复用 `join_with_timeout`）。
- 通过现有 `handleSessionExit` 路径清理 Kotlin 侧条目与原生会话。
- 保留用户可见的“会话已结束/需要新建”语义，不静默重建。

## 诊断信息

`SessionEntry` 新增只读诊断结构（Kotlin 侧）：

```kotlin
data class SessionLivenessDiagnosis(
    val hungLayer: HungLayer?, // RENDER / VT / PTY_READ / LIFECYCLE / null
    val lastRenderProgressNanos: Long,
    val lastVtProgressNanos: Long,
    val lastPtyReadProgressNanos: Long,
    val diagnosisNanos: Long,
)
```

- `hungLayer == null` 表示未挂起。
- 诊断信息只在挂起时更新，避免空闲会话产生噪声。
- 日志仍按 500ms 限频，避免挂起期间刷屏。

## 推进点落盘

### Rust 侧

- PTY 读取线程：每次成功读取或通道发送完成后推进 `ptyReadGeneration` 与时间。
- VT 线程：每次完成输出处理并更新终端状态后推进 `vtGeneration` 与时间。
- 代数与时间必须以原子量存储，渲染线程与看门狗只读，不取会话锁。

### Kotlin 侧

- 渲染线程：每次 `renderWithNewOutput()` 返回后推进 `renderGeneration` 与时间。
- 看门狗：只读三个代数与时间，不写入。

## 与现有机制的关系

- `RenderWake` 继续作为事件唤醒源，不被诊断逻辑替代。
- `RenderWatchDog` 保留，但其 `onHangDetected` 改为“记录层级并交由会话管理器”，
  不再直接把渲染线程标死作为唯一动作。
- `checkSessions` 的重启逻辑只对渲染层挂起生效；VT/PTY 层挂起走会话重建路径。
- `RenderPauseLedger` 与后台/切前台逻辑不受影响：挂起诊断不参与暂停/恢复门控。

## 测试策略

- 纯函数：层级判定（给定三个代数与时间，输出 `HungLayer?`）必须有 JVM 单测，
  覆盖空闲、渲染挂起、VT 挂起、PTY 挂起、三层都停滞五种情况。
- 纯函数：恢复决策（给定 `HungLayer`，输出 `RestartRenderThread` / `RebuildSession` /
  `RecordOnly`）必须有单测。
- Rust 侧：代数推进的原子性与时间写入顺序必须有单元测试。
- 不得用 `@Ignore`、环境检查或跳过；缺少 Rust 产物时相关测试必须失败。

## 风险与缓解

- **误判空闲为挂起**：通过分层阈值与空闲时钟证据降低；空闲会话的 VT/PTY 代数
  在无输出时允许不推进，但渲染代数仍需在 vsync/超时时推进。
- **诊断读取竞争**：所有代数与时间用原子量，不取会话锁；看门狗只读。
- **恢复风暴**：重建会话后必须重置诊断代数，避免立即再次触发；同一会话 60s 内
  最多自动重建一次，超过则记录并等待用户。
- **真机差异**：阈值按现有 10s 渲染挂起经验设定，最终以真机 p95/p99 校准。

## 非目标

- 不实现进程级 watchdog 或整应用重启。
- 不改变 VT 引擎或 PTY 实现。
- 不新增用户可见的“健康度”UI，仅记录诊断。
