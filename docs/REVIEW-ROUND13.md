# 第 13 轮全面审查（并发、内存归属与停机顺序）

审查日期：2026-09-30
基线提交：`b937228`
方法：第 12 轮换到输入/IME 链路与门禁有效性。本轮换第三个横切维度：
**并发、内存归属与停机顺序**。先建立完整线程拓扑（原生 5 条 + Kotlin 11 条协程/线程），
再逐条检查锁序、内存序、通道生命周期、`Arc`/`OwnedFd` 归属与析构顺序。
本轮只审查，**未改动任何源码**。

---

## 一、本轮的四个 P0（全部经回读源码证实）

> 维护注：N0-21（触摸滚轮转发按手势钳制行数）、N0-22（取消先回滚 bridge 再重抛）已修复并验证，对应小节删除；其余编号保持不变。

### N0-19 `pauseRendering` 持 `sessionLock` 做**每会话 3 秒**的阻塞，而主线程在等同一把锁

代码自己的成本模型算错了 3 倍，且「不会 ANR」的结论不成立。

**阻塞链**（逐跳核实）：

1. `pauseRendering`（`TerminalRuntime.kt:2977-2987`）在 `surfaceTransitionExecutor` 上，
   **持 `sessionLock` 遍历全部会话**：

   ```kotlin

   surfaceTransitionExecutor.execute {
       synchronized(sessionLock) {
           sessions.values.forEach { entry ->
               if (entry.running) {
                   renderSupervisor.stopRenderThread(entry)

   ```

2. `stopRenderThread`（`:1539-1548`）**第一件事**就是看门狗的阻塞式停止，
   然后才 join 渲染线程：

   ```kotlin

   internal fun stopRenderThread(entry: SessionEntry): Boolean {
       entry.renderWatchDog?.stop()          // ← runBlocking，最长 2s
       entry.renderWatchDog = null
       entry.running = false
       val thread = entry.renderThreadRef
       ...
       thread?.let { t ->
           t.interrupt()
           t.join(THREAD_JOIN_TIMEOUT_MS)    // ← 1000ms

   ```

3. `RenderWatchDog.stop()`（`monitor/RenderWatchDog.kt:36-44`）确实是阻塞的：

   ```kotlin

   runBlocking {
       withTimeoutOrNull(2000L) { job.cancelAndJoin() }
   }

   ```

   ⇒ **每会话最长 2s + 1s = 3s**。

4. 而 `sessionLock` 的**另一个消费者在主线程**：
   `MainActivity.kt:274` `onDestroy` → `runtime.stopForegroundServiceIfIdle()`
   → `TerminalRuntime.kt:2719-2727` 的 `synchronized(sessionLock)`。
   `onDestroy` 跑在主线程。

**结论**：Activity 销毁时若 `pauseRendering` 正在执行，主线程被阻塞
`3 × 会话数` 秒。2 个会话 = 6s，已超过 Android 的 5s ANR 窗口；
而本仓的 `AnrWatchDog`（`monitor/AnrWatchDog.kt`，`ANR_TIMEOUT_MILLIS = 5_000L`）
在该阈值触发时执行 `BootGuard.exit` → `Process.killProcess` → **销毁全部 shell**。
即「退出应用」这条最普通的路径，在 GPU 恰好挂起时会把所有会话连同用户数据窗口一起杀掉。

**代码注释的两处错误**（`:2964-2976`）：

```kotlin

// stopRenderThread 会 join 渲染线程（每会话最长 1s）；
...
// 注意：每会话的 join 发生在 sessionLock 内（各最长 1s），

```

- 「每会话最长 1s」漏算了 `renderWatchDog.stop()` 的 2s `runBlocking`；
- 「故不 ANR」只对执行器线程成立，**没有考虑主线程经 `stopForegroundServiceIfIdle` 争用同一把锁**。

同类问题还有两处：`startRenderThread`（`:1041`）也在锁内调 `renderWatchDog?.stop()`，
与 `:920-923`「阶段 2 刻意放在锁外以免 join 阻塞会话操作」的既定策略自相矛盾；
`handleSessionExit`（`:479-491`，**运行在渲染线程上**）同样在锁内 `runBlocking`。

**修法**：`RenderWatchDog.stop()` 改为非阻塞 `job.cancel()`（join 交给后台路径），
或把所有 `renderWatchDog?.stop()` 移出 `synchronized(sessionLock)`。
同时修正 `:2964-2976` 的成本注释。

### N0-20 `RENDER_STATE` 跨 **2 秒**阻塞的取纹理往返被持有，且 UI 线程的 `attachSurface` 同形

`render_inner` 在 `ffi.rs:1537` 取全局渲染锁，该 guard 活到函数结束：

```rust

// ── 阶段 3：渲染（持渲染状态锁）────────────────────────────────────
let mut state = render_state_mut();

```

锁内调用 `render_state.renderer.render_cell_data(...)`（`ffi.rs:1721`）
→ `begin_frame()`（`render/context.rs:178`）→ `acquire_texture()`（`context.rs:199`）
→ `acquire_worker_tx().try_send(request)` 后
`response_receiver.recv_timeout(ACQUIRE_TIMEOUT)`（`render/pass.rs:140-145`），
而 `ACQUIRE_TIMEOUT = Duration::from_secs(2)`（`pass.rs:10`）。

**GPU 取纹理工作线程**（`pass.rs:23-33`）的目的是让**渲染线程**不被无限阻塞 ——
它并不阻止**其他线程**进入 `RENDER_STATE`。于是所有 setter
（`setFontFamily` / `setFontSizeInPlace` / `setSearchHighlights` / `clearSearchHighlights` /
`setCursorColor` / `setScrollYPx` / `listFontFamilies` / `getDefaultFontName` / `getCellWidth|Height`）
在一次 2s GPU 卡死期间全部阻塞。

**更严重的是 `attachSurface`**：`ffi.rs:1357` 同样取 `render_state_mut()`
并跨越 `attach_surface` → `warmup()` → `acquire_texture()`（`context.rs:490`）。
而 `attachWindow`/`detachWindow` 是 `SurfaceHolder.Callback`，**由 UI 线程驱动**
（`context.rs:350` 注释自述）。故一次 2s 的 GPU 卡死 = **2s 的主线程冻结**。

与第 12 轮 N0-15/N1-24 叠加：`AnrWatchDog` 在 5s 触发，
而这 2s 阻塞会反复占用主线程，使触发概率上升。

**修法**：与 `render_inner` 已有的三阶段做法一致 —— 在**取锁之前**把纹理取到局部变量
（阶段 1/2/3 已经这样处理过其它资源），或把 `ACQUIRE_TIMEOUT` 降到约 1 个 vsync
并直接丢帧。

> 维护注：N0-21（触摸滚轮转发按手势钳制行数）、N0-22（取消先回滚 bridge 再重抛）已修复并验证，对应小节删除；其余编号保持不变。

## 二、新的 P1

| 编号 | 位置 | 问题 |
| --- | --- | --- |
| N1-30 | `ghostty_terminal/mod.rs:45-53` + `types.rs:227` | `Terminate` 用 `try_send` 投递，**通道满（1024）时直接丢弃**。VT 线程只在 `internal.rs:708` 的 `RecvTimeoutError::Disconnected` 才退出，而 flume 只在**缓冲区排空后**才报 Disconnected。持续输出时（每帧最多 10 个 `Write`、每个 8KiB）缓冲区经常接近满，`join_with_timeout` 350ms 后放弃 → **VT 线程被 detach，仍持有 2000 行回滚 + 64MiB Kitty 存储**，其 `Drop`（`ghostty_termill_free`）在任意更晚的时刻才跑。 |
| N1-31 | `session.rs:271` + `:706-750` | 读取线程用**阻塞** `output_tx.send` 写入 128 槽通道。`Session::drop` 置 `exited` 后 join，但线程阻塞在 `send` 上观察不到；通道仍连接（`Session` 同时持有 `output_tx` 与 `output_rx`，字段要等 `Drop` 返回才析构）⇒ `join_with_timeout` 必然超时，线程被 detach，**panic 永不上报**，`dup` 出的 master fd 与 8KiB 缓冲继续存活。`poll(…, 100ms)` 超时只在线程处于 poll 时有效。 |
| N1-32 | `ffi.rs:506-531` + `:1181-1199` | `destroySession` 在 shell 与三条会话线程仍存活时即返回 `JNI_TRUE`：`poll_event_inner` 把 `Arc<Mutex<Session>>` 克隆进剪贴板应答线程与 `pending_exits`，`Session::drop`（kill + join，最长约 1.1s）要等**最后一个克隆**析构，即最长 `CLIPBOARD_ANSWER_TIMEOUT = 2s` 之后。无 UAF（正是该克隆保证了安全），但生命周期契约被破坏：Kotlin 拿到 `true` 后可 `initSession` 替代会话，而旧 shell 仍在跑。 |
| N1-33 | `ffi.rs:2008-2011` | `REQUEST_REGISTRY` 的 `parking_lot` guard **跨越 JNI 调用**。edition 2024 下 `if let` 的 scrutinee 临时量在 `if let` **块结束**才析构，故 guard 覆盖 `text.try_to_string(env)`（`GetStringUTFChars`，会在 JVM 内分配并可能触发 GC safepoint）与 `tx.send`。无环（该表无其他持有者），但在设计声明「只短暂持有」的那条路径上是潜在隐患。同文件的 `listFontFamilies`（`ffi.rs:2526`）与 `getFontInfo`（`ffi.rs:2576`）已用 `drop(state)` 正确处理。 |
| N1-34 | `TerminalRuntime.kt:2441-2456` | 并发移除的回滚**无条件** `bridge.close()`，绕过了其它所有关闭路径都遵守的 `renderThreadPossiblyAlive` UAF 守卫（`:2769-2785` 明确为此跳过 `close()`）。若并发 `closeSession(nextId)` 因渲染线程挂起而跳过了 `close()`，这条回滚就把原生会话销毁在仍存活的线程脚下。`Bridge.close()` 幂等（`Bridge.kt:150-161`），所以双重 close 无害 —— **危险的是第一次**。 |
| N1-35 | `TerminalRuntime.kt:2601-2612` | `switchSession` 阶段 2 的 `while` 循环里 `delay(RENDER_INITIAL_RETRY_DELAY_MS)`，被 `:2611` 的 `catch (exception: Exception)` 吞掉 `CancellationException`（同文件 `:2051`/`:2191`/`:2329`/`:2460` 都有显式重抛）。吞掉后阶段 3（`:2620-2702`）仍在**已取消的协程**上执行 `startRenderThread`、`NativeBridge.switchSession`、`resize`、`focusEvent`，随后函数「成功」返回，调用方 `TerminalViewModel.switchSession`（`:1370-1388`）照常写 `_state`。Activity 销毁触发 ViewModel 清理时仍会改动运行时状态。 |

---

## 三、新的 P2

| 编号 | 位置 | 问题 |
| --- | --- | --- |
| N2-65 | `ffi.rs:465-470` | `ACTIVE_SESSION_ID.compare_exchange(0, id, Acquire, Relaxed)` 的**成功**序用 `Acquire`，不带 release 语义，因而这次 RMW 不发布此前的 `registry.insert(id, entry)`。同族写入 `switch_session_inner` 用的是 `store(id, Release)`（`ffi.rs:560`）。已证伪为实际 bug（所有消费者先取 `SESSION_REGISTRY.read()`，RwLock 提供 happens-before），但顺序不一致且误导。 |
| N2-66 | `ffi.rs:2552-2559` | `getDefaultFontName` **持 `RENDER_STATE`** 调 `env.new_string()`，而两个直接邻居 `listFontFamilies`（`:2526`）与 `getFontInfo`（`:2576`）都先 `drop(state)`。`NewStringUTF` 可能取 JVM safepoint。 |
| N2-67 | `ffi.rs:1175-1179` | `Event::ClipboardRead` 会被满队列淘汰（`event.rs:81-93` 淘汰最老的非 `Exit` 事件，正是它）。有界（槽位由 `cancel_request` 自清理，`ffi.rs:1190`），但子应用收到的是**空剪贴板**而非自己的答案。 |
| N2-68 | `session.rs:730-748` | `Session::drop` 可阻塞 JNI 调用方约 1.1s（50ms `TRAILING_EXIT_GRACE` + 2×(50+3×100)ms + VT 线程 350ms）。有界且在 `Dispatchers.IO` 上，但计入 `destroySession` 的最坏延迟。 |
| N2-69 | `internal.rs:698-706` | VT 线程在**每个 50ms 空闲 tick** 都重建整幅 `CellData`（`build_cell_data`，约 2400 次 FFI），即使 `batch_dirty` 为假。`last_push` 的 memcmp 去重（`internal.rs:1222-1232`）只抑制**发送**不抑制**构建**。既耗 CPU 也扩大 N0-21 的窗口（查询只在批次边界被服务）。 |
| N2-70 | `TerminalRuntime.kt:2145`、`:2218` | 两处整体赋值 `_state.value = RuntimeState()` 绕过全文件其余位置的 `_state.update {}` CAS 约定。`:2218` 位于**失败之后**的 catch，而失败可能发生在渲染线程已启动（`:2176`）或其他协程已调过 `resize()` 之后，并发写会被静默丢弃。 |
| N2-72 | `TerminalForegroundService.kt:26`、`:149` | `PARTIAL_WAKE_LOCK` 以 `acquire(WAKE_LOCK_TIMEOUT_MS = 30 分钟)` 获取，而重获只在 `onStartCommand`（`:87`）与 `onTaskRemoved`（`:171`）。`TerminalRuntime` 只在会话增删时 `updateSessionCount`，故**空闲超过 30 分钟的会话**在灭屏后 CPU 被挂起、终端静默停止。`:24-26` 注释声称「仍存活会话会在下一个 start tick 重获」，但服务已在运行时 `onStartCommand` 不会被重新调用。 |
| N2-73 | `TerminalViewModel.kt:1199-1203` | 防抖后的引导 URL 写入在 ViewModel 清除时**静默丢弃**（`debounce` 只在收集器存活期间保留待写值，300ms 窗口内取消无日志无 flush）。`bootstrapUrlEdited`/`replayCache`（`:1102-1107`）只保护内存读，不保护持久化。 |
| N2-74 | `TerminalRuntime.kt:479-491` | `handleSessionExit` **运行在渲染线程上**（由 `:1118`、`:1333` 调用），却在持 `sessionLock` 的情况下执行 `entry.renderWatchDog?.stop()` 的 2s `runBlocking`。**每次 shell 退出**都会让渲染线程与所有 `sessionLock` 操作停顿最多 2s。 |
| N2-75 | `TerminalRuntime.kt:3123`、`:3131-3246` | `activateReplacementSession` 的文档写明「必须在持有 `sessionLock` 时调用」，其内部却做两次有界 join（`:3171`、`:1058` 各最长 1s）加 `renderWatchDog.stop()`（`:1041`，最长 2s）。`:2491-2702` 的「阶段 2 放锁外」策略未覆盖阶段 3。 |
| N2-76 | `TerminalRuntime.kt:1317-1329` | 活动会话的渲染循环为**后台**会话调用 `handleSessionExit`，其中 `:502` 的 `entry.bridge?.close()` 会内联执行 `destroySession`（kill + join 子进程线程，注释称「~100ms+」），直接拖慢当前帧并推迟 `pollAll`。`checkSessions`（`:858`）走 `scope.launch` 避免了这一点，渲染循环路径没有。 |
| N2-77 | `context.rs:497` vs `:874` | `release_surface` 直接丢弃 `Arc<wgpu::Surface>`，而 `release_gpu_surface` 会把它停放到 `GLOBAL_SURFACE` —— 后者在生产中**无调用方**。`GLOBAL_SURFACE` 因此永不初始化，`ERROR_NATIVE_WINDOW_IN_USE_KHR` 黑屏规避路径（`context.rs:396-400` 注释所述）实际被绕过。 |

> 维护注：N2-78（字体信息刷新移出主线程收集器）已修复并验证，对应行删除。

---

## 四、本轮的正面结论（经核实为健康）

1. **锁图无环**。全部 30+ 个 `render_state_mut()` / `RENDER_STATE.lock()` 调用点逐个检查，
   除 `render_inner` 阶段 3 外都在取 `RENDER_STATE` 前释放了注册表与会话锁。
   唯一的方向反转（`ffi.rs:1534-1536` 注释所述）安全，因为它取的是**读**锁，
   且没有任何 `SESSION_REGISTRY` 写者在持有写锁时等待 `RENDER_STATE`
   —— `destroy_session_inner`（`:506-523`）、`switch_session_inner`（`:554-561`）、
   `reset_terminal`（`:578-591`）、`set_scroll_offset`（`:3160-3198`）、
   `set_theme`（`:2792-2793`）全部先释放下层锁。
2. **没有线程活过它引用的数据**。每条 detach 后的线程都持有或 `Arc` 化了它访问的一切；
   代价只是延迟回收（N1-30/N1-31）与丢失 panic 上报，不是 UAF。
3. **fd 归属正确**：`pty.rs:559-561` 的 `try_clone` 产生**同一个 open file description**，
   故 `session.rs:197`（dup 之前）设置的 `O_NONBLOCK` 是共享的；
   `Pty::read` / `PtyPair::read`（`pty.rs:529`、`:584`）**在生产中从不被调用**
   （只有读取线程自己的 dup `File`，`session.rs:259`），故读不会交错。
   丢弃 `PtyPair` 安全：读取线程靠 `exited` 标志 + 100ms poll 超时解除阻塞，
   而非靠 fd 关闭（关闭只会产生 `EIO`）。
4. **内存序全部正确配对**：`exited`（`session.rs:231` vs `:255,262,284,325,708`）、
   `alt_screen_active`（`public_api.rs:463` vs `internal.rs:1456-1461`）、
   `cell_size_px`（`internal.rs:562` vs `:786,813`）、`panicked`（`public_api.rs:201` vs `:98`）、
   `new_output`（`output_processor.rs:41 AcqRel` vs `:47 Release`）、
   `grid_dirty`（`session.rs:396` vs `:404,410`）。
5. **`CellData` / `KittyPlacementFrame` 经 flume 有界通道 move 传递**，
   不存在半发布；`CursorInfo`（含 `kitty_generation`、`scrollback_length`）同批 move。
6. **JNI 无重入**：`jni_export_guard!` + `EnvUnowned::with_env`（`ffi.rs:43-49`）
   把 `Env` 限制在闭包内；**没有任何 spawn 出的原生线程触碰 `Env`**
   （应答、读取、waitpid、VT、gpu-acquire 五条线程全部 Env-free）。
7. **`push_cell_data` 的去重基线只在成功后推进**（`internal.rs:1235-1238`），
   被丢弃的帧会重试而不会永久丢失；接收端只在 `GhosttyTerminal` 析构时死亡，
   而那也会一并 drop `cmd_tx` 使循环退出，故不会空转。
8. **所有对不可信终端输出可达的容器都有界**：`COMMAND_CHANNEL_CAPACITY=1024`、
   `QUERY_CHANNEL_CAPACITY=256`、`CELL_DATA_CHANNEL_CAPACITY=4`、
   `EVENT_CHANNEL_CAPACITY=16`、`OUTPUT_CHANNEL_BOUND=128`、`MAX_QUEUED_EVENTS=1024`、
   `MAX_NAVIGABLE_MATCHES=50_000`、Kitty 存储 64MiB、
   `set_scrollback_max_lines(2000)` + `set_scrollback_max_bytes(None)`。
9. **剪贴板应答线程的 `Arc` 克隆是正确的**（`ffi.rs:1181`）：
   它保证 `PtyPair` 与 master fd 在 ≤2s 等待期间存活且有效，
   `answer_clipboard_read` 写回前重新检查 `is_exited()`（`session.rs:597`）。
10. **`sessionLock` 从不跨 `bridge.render()` 持有**：`switchSessionInternal` 刻意把阶段 2
    放在锁外（`:2491-2702`），`activateReplacementSession` 也从不调 render。
11. **无裸 `runCatching`**：唯一助手 `runCatchingCancellable`（`RunCatching.kt:13-19`）
    正确重抛 `CancellationException`，两处调用点都是非挂起路径。
12. **`LatencyProbe` 拆分正确**：`@Volatile` 时间戳供写侧，`synchronized(this)` 保护环形缓冲与计数。
13. **`entry.closing` 标志闭合了孤儿渲染线程的 TOCTOU**：每个 `startRenderThread` 调用方都持
    `sessionLock`，而 `closeSession` 在阶段 1（`:2736-2744`）释放锁**之前**就置了 `closing`，
    故 stop 与 `bridge.close()` 之间不可能出现新线程。
14. **前台服务唤醒锁的获取/释放正确配对**（`:87`/`:171` vs `:161`），且 `setReferenceCounted(false)`。
15. **DataStore 无主线程磁盘 I/O**：`data` 在其内部作用域加载，map 中只做内存 `Preferences` 查询；
    `settings` StateFlow 用具名常量的 `SharingStarted.WhileSubscribed(TIMEOUT_MILLIS)`（`:802`、`:834`）。
16. **`ThermalMonitor.lastStatus` 是 `@Volatile`** 且回调由单线程执行器串行化，读改写安全。
17. **`MemoryMonitor` 的三个可变字段只有一个写者**（`Dispatchers.Default` 轮询协程），
    `onTrimMemory` 只记日志。

---

## 五、需要用户裁决的问题

1. **`AnrWatchDog` 仍是这一切的放大器**（第 9 轮 D3、第 11 轮 Q6、第 12 轮 Q6 三次未决）。
   N0-19 表明主线程一次 6s 阻塞即可触发它并销毁全部会话。
   在 N0-19 修好之前，先修 `AnrWatchDog` 的阈值/判据是否更划算？
2. **锁序是否应提升进 `docs/specification/`**：`grep -rl "锁顺序|lock order|线程模型" docs/ openspec/`
   零命中 —— 唯一的锁序声明是 `ffi.rs:13-14` 的模块注释。
   建议把锁序与线程拓扑写进 `DESIGN.md`，使并发约束可对照规范审查而非对照源码。
3. **`GLOBAL_SURFACE` 停放路径（N2-77）**：`release_gpu_surface` 会把 `Surface` 停放以规避
   `ERROR_NATIVE_WINDOW_IN_USE_KHR` 黑屏（`context.rs:396-400` 注释所述），
   但生产走的是直接丢弃的 `release_surface`，而前者无调用方。是漏接线还是该路径已废弃？
4. **取消时是否应先关闭再重抛**（N0-22）？代码注释说泄漏「由调用方拆除路径关闭」，
   而该路径不存在。倾向前者。

---

## 六、修复顺序建议

1. **N0-19 `RenderWatchDog.stop()` 阻塞化** —— 一处改动，同时消除主线程 3N 秒阻塞、
   渲染线程上的 2s `runBlocking`（N2-74）与两处锁内 `stop()`。
2. **N0-22 取消时先关闭再重抛** —— 两处 `catch` 调换顺序，堵住原生会话 + 子进程泄漏。
3. **N0-20 取纹理移出 `RENDER_STATE`** —— 与 `render_inner` 既有的三阶段做法同形。
4. **N0-21 UI 线程查询加短期限** —— 复用一个已有的 `*_with_timeout` 写法。
5. **N1-30 / N1-31 停机可中断** —— 同一个根因（`join_with_timeout` 无法打断阻塞原语），
   一处修法可同时解决两条。
6. **N1-34 / N1-35** —— 守卫复用与 `CancellationException` 重抛。
7. **N2-71 `@Volatile` 缺失**、**N2-70 CAS 绕过**、**N2-66 锁内建串** —— 各自一行。
8. 其余 N2 项。

---

## 七、收敛状态

- 本轮**不是**「无新问题」的一轮：新增 **4 个 P0、6 个 P1、14 个 P2**，
  外加 **17 组经核实的正面结论**。
- 「连续五次无新问题」计数**第一次归零**，从第 13 轮重新开始。
- 第 6–13 轮八轮全部产出新 P0。本轮维度是**并发与停机顺序**，
  产出 N0-19/N0-20/N0-21/N0-22 —— 且四个都属于「文档化的成本模型与实际行为不符」这一类：
  `pauseRendering` 的「每会话 1s」实际 3s、「故不 ANR」忽略了主线程争用同一把锁；
  取纹理工作线程的「不让渲染线程阻塞」被当成了「不让任何人阻塞」。
  这提示下一轮应专门审**注释断言 vs 代码行为**的偏差（与第 8 轮 N0-1 同源但已成体系）。
- 累计（第 6–13 轮）：**19 个 P0、45 个 P1、约 109 个 P2/P3**。
- 仍然建议先修复再复审。N0-19 尤其关键：它使「退出应用」这条最普通的路径
  在 GPU 挂起时会杀掉全部会话，直接违反 `DESIGN.md:192`（shell 崩溃保留现场）与
  `PROHIBITED.md:10`（禁止会话持久化/恢复 ⇒ 会话本就不可恢复）。
- 本轮任务限定「不实际修改代码」，上述内容以文档形式留存，等待授权后按第六节顺序执行。
