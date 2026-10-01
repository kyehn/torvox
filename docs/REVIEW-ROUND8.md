# 第 8 轮全面审查（按缺陷类别全仓横切）

审查日期：2026-09-30
方法：按第 7 轮末尾的建议，放弃「按目录分配审读预算」，改为**按缺陷类别做全仓横切**，两个并行子代理分别扫「吞错点」与「锁/阻塞/线程安全」两类，每类覆盖 `native/src/**` 与 `android/app/src/main/**` 全部文件。关键结论逐条回读源码核实。
本轮只审查，**未改动任何源码**。

---

## 一、为什么换方法

第 6、7 轮连续两次把「连续四轮无新问题」的计数归零，且两次的成因相同：按目录分配审读预算，而每轴的样本量远小于文件总量。第 8 轮改为类别横切 —— 一个类别（如「持锁跨阻塞调用」）天然覆盖此前从未被读过的文件。

结果：**在两个此前重点审过的文件里又各挖出 P0，同时覆盖了第 6、7 轮完全没碰的 `Bridge.kt` 查询面、`InputBatchBuffer`、`NativeQueryPort`、`font_db.rs`、`pty.rs` 原始模式配置等区域。**

---

## 二、新的 P0

> 维护注：N0-10（锁内零期限排空）已修复并验证（`poll_pty_output` 改零期限，输出泵移出渲染分支），对应小节删除；其余编号保持不变。

### N0-11 `CACHED_FONT_DB` 初始化在全局渲染锁内做文件 I/O（约 3 秒）

`native/src/android/ffi.rs:2948-2954` 与 `:3039-3045`：在持有 `RENDER_STATE`（全局渲染状态互斥锁）的情况下构造 `FontPipeline` → `CACHED_FONT_DB.get_or_init`（`native/src/render/font/font_db.rs:56-90`）→ 读 `/system/etc/fonts.xml` + 对每个字面 `load_font_file`。

`ffi.rs:160` 自己的注释承认这一步可达约 3 秒。`font_db.rs:350-363` 还让 `EXTRA_FONT_PATHS` 的**读锁**跨 `fs::read_dir` 整个 I/O 期间保持。

**故障场景**：用户改字号或切换主字体 → 渲染线程被持锁阻塞最长约 3 秒 → 期间 `setScrollYPx`、`setCursorColor`、`getCellWidth/Height`、`render`、`setRenderPaused` 全部排队 → 用户看到的是**终端冻结几秒后突然跳一下**。`DESIGN.md:160` 要求「切换应用返回或从应用设置返回终端应该正常渲染且无进入卡顿、黑屏、闪烁、跳跃」。

违反 `DESIGN.md:20`、`DESIGN.md:160`。

**修法**：字体数据库初始化移到 JNI 入口的锁外（或用 `OnceLock` 之外的一次性预热，在会话创建阶段完成），`EXTRA_FONT_PATHS` 的读锁只保护向量快照，不跨 I/O。

---

## 三、新的 P1

### N1-7 OSC 52 剪贴板写入在渲染暂停时被静默丢弃

`native/src/terminal/ghostty_terminal/internal.rs:542`

```rust
let _ = clipboard_tx.try_send((selection, text));
```

通道是 `bounded(16)`（`public_api.rs:50`），而唯一消费者 `drain_callback_events` 只在渲染路径（`session.rs:509/520`）上被调用。

**故障场景**：用户在 `less` / `tmux` / SSH 会话里按复制 → 剪贴板内容永远不更新。**打开设置页、Surface 被销毁、输入框弹出导致渲染暂停时必现** —— 也就是用户最需要剪贴板的时刻。无日志、无提示。

违反 `DESIGN.md:72`（剪贴板集成）、`DESIGN.md:24`。

### N1-8 JNI 查询面上 14 个无日志的 `getOrNull` / `getOrDefault`

`android/app/src/main/java/terminal/emulator/bridge/Bridge.kt:581-643`

```kotlin
override fun scrollbackLine(row: Int): String? = runCatchingCancellable {
    queryPort.scrollbackLine(row)
}.getOrNull()

override fun scrollbackLength(): Int = runCatchingCancellable { queryPort.scrollbackLength() }.getOrDefault(0)
```

同一形状重复 14 次（`scrollbackLine` / `scrollbackLength` / `cursorViewportPacked` / `isCellEmpty` / `selectAll` / `selectWordAt` / `listFontFamilies` / `clearSearchHighlights` 等）。文件自己的注释（`:576-579`）点名了可达的失败原因：「会话在 id 检查与实际调用之间被销毁」，但**一个字都不记**。

**三个可观察后果**：

1. `scrollbackLine` → `null` → `TerminalSurface.kt:2886` 的 `isWhitespaceCell(null, col) == true` → 用户长按**有内容的文本**时弹出只有「粘贴」的菜单（`DESIGN.md:170` 要求「如果是有内容区域：复制、分享…」）。
2. `selectAll()` → `null` → 全选静默无效（`DESIGN.md:173` 明文要求全选必须能用）。
3. `listFontFamilies()` → `null` → `TerminalViewModel.kt:696` 的 `.orEmpty()` → `:700` 的 `allFonts.first()` → `NoSuchElementException`，在 `:719` 被重抛 → **启动期崩溃**。

违反 `DESIGN.md:24`、`DESIGN.md:170`、`DESIGN.md:173`。

### N1-9 OSC 52 读取应答在两处独立地被转成「空字符串」

`native/src/android/ffi.rs:1976-1979`

```rust
if let Some(tx) = REQUEST_REGISTRY.lock().remove(&(session_id, request_id)) {
    let text_str: String = text.try_to_string(env).unwrap_or_default();
    let _ = tx.send(text_str);
}
```

UTF-16 转换失败 → **空但成功**的 OSC 52 应答；`tx.send` 的结果也被丢弃（无消费者时静默）。

Kotlin 侧同样：`TerminalRuntime.kt:392-400`，`clipboardAccess.clipboardText().orEmpty()` 在捕获异常后 `LogUtil.e(...)` 记了日志（这点没问题），但仍然调用 `NativeBridge.clipboardResult(id, reqId, "")` —— 注释自己承认了这一点。

**故障场景**：远端程序（如 `tmux`/`ssh`）请求读取剪贴板，得到一个「成功但为空」的应答，于是**粘贴出一片空白**，且看起来像是用户真的清空了剪贴板。正确做法是不应答。

违反 `DESIGN.md:72`、`DESIGN.md:24`。

### N1-10 每行的选区查询失败被静默降级为「无高亮」

`native/src/terminal/ghostty_terminal/internal.rs:1495`

```rust
row.selection().ok().flatten()
```

`.ok()` 把「查询出错」与「没有选区」合并成同一个 `None`。结果是：终端持有活动选区时，**那一行不显示反色**，但复制仍然返回完整文本 —— 用户看到「选中了但看不出选中」，且没有任何诊断。

同一形状在 `:2014, 2033, 2049, 2052, 2059`（`.to_ordered(…).ok()?` / `.select_word(…).ok()??` / `.select_all().ok()??`）重复出现，后果是**双击选词与全选间歇性无反应**，错误经 `Query::SelectWordAt` 的 `try_send` 转发时无日志。

违反 `DESIGN.md:170`（被长按文本单元格反色）。

### N1-11 子进程原始模式配置失败是**自述的**静默忽略

`native/src/terminal/pty.rs:651-663`

```rust
/// 异步信号安全：不分配、不调用 `log::warn!`。错误静默忽略（不致命）。
fn configure_raw_mode_child(fd: std::os::unix::io::RawFd) {
    …
    if unsafe { libc::tcgetattr(fd, termios.as_mut_ptr()) } != 0 {
        return; // 不致命
    }
    …
    let _ = unsafe { libc::tcsetattr(fd, libc::TCSANOW, &termios) };
}
```

注释解释了为什么**不能**开 `cfmakeraw`（会破坏 bash readline —— 这个论证是对的），但结论滑到了「所以任何错误都静默忽略」。`tcsetattr` 失败意味着 `IUTF8` 与 `IXON`/`IXOFF` 关闭从未生效 —— **Ctrl+S 会锁死终端显示**，而用户只会看到屏幕冻结，没有任何提示。

这是注释里写明的、未在规范中声明的 Fallback，`DESIGN.md:24` 明令禁止（「错误静默忽略（不致命）」本身就是自认）。

**修法**：异步信号安全约束只限制 `tcsetattr` 之前不能写日志；`tcsetattr` 失败可以安全地 `_exit` 并向 fd 2 写一行诊断（与 `pty.rs:365-371` 的 `execve` 失败路径一致）。

### N1-12 `TerminalRuntime` 的根协程作用域从不取消，`TerminalViewModel` 没有 `onCleared`

`android/app/src/main/java/terminal/emulator/runtime/TerminalRuntime.kt:244`

```kotlin
private val scope = CoroutineScope(SupervisorJob() + terminal.emulator.util.TerminalDispatchers.inputOutput)
```

全文件**没有任何 `scope.cancel()`**；`TerminalViewModel` 里 `onCleared` 出现次数为 **0**。该作用域持有 `renderMonitorJob`（`:834`）与 `handleDeadRenderThread` 的重启逻辑（`:858`），这些协程强引用 `TerminalRuntime`，而 `TerminalRuntime` 持有 `context`（`:229`）、`clipboardAccess`、`modifierBarHeightPx`。

同时 `:365` 的 `surfaceTransitionExecutor` 从不 `shutdown()` —— **每次屏幕旋转泄漏一个线程**。

违反 `DESIGN.md:20`（低内存友好）、`DESIGN.md:64-66`（Activity 重建 / 进程回收场景）。

> 维护注：N1-13（看门狗任务引用与渲染线程引用补齐易变注解）已修复并验证，对应小节删除；其余编号保持不变。

---

## 四、新的 P2

### 静默降级但值本身有误导性

| 编号 | 位置 | 问题 |
| --- | --- | --- |
| N2-1 | `font_db.rs:536` | `Err(_) => return aliases` —— `/system/etc/fonts.xml` 解析失败时返回**空**别名列表，无日志，静默改变 monospace / CJK 回退解析。`DESIGN.md:96` 明文要求「软件输出日志并崩溃退出，不做复杂处理」。更矛盾的是 **Kotlin 侧确实会崩**（`TerminalViewModel.kt:710`），两层策略相反。 |
| N2-2 | `ffi.rs:2955` | `let _ = render_state.font_pipeline.set_font_family(&family);` 失败时 `loadFontFile` 仍返回该字族名，于是 `Bridge.kt:428-432` 把它缓存为「已应用」且永不复查。`DESIGN.md:102`「不显示不存在的字体」。 |
| N2-3 | `ffi.rs:2503-2507` | `if let Ok(family) = env.new_string(family) { let _ = array.set_element(…); }` —— 转换失败的字族在**非 null 的 `jobjectArray`** 里留下一个空槽，表现为字体选择器里的空行。 |
| N2-4 | `ffi.rs:3029-3036` | `setExtraFontPaths` 中 `array.len(env).unwrap_or(0)` 且跳过失败的 `get_element`，随后仍然 `log::info!("registered extra font paths")` —— **错误值被报告为成功**。 |
| N2-5 | `ffi.rs:2597-2605` | 搜索高亮包 `convert_byte_array(…).ok()` 后 `let Some(prefix) = … else { return Ok(()) }` —— 包格式错误时**上一帧的高亮继续留在屏幕上**，永不清除。 |
| N2-6 | `internal.rs:180` | `scrollback_rows().unwrap_or(0)` 的失败被 `log::debug!` 掩盖（`:67`）—— 表现为「没有回滚」，用户无法向上滚动或搜索历史，且只在 debug 级可见。`title().unwrap_or("")`（`:163`）→ 抽屉里会话没有标题；`mode(…).unwrap_or(false)`（`:173`）→ DECRQM 报告「未设置」而实际已设置。 |
| N2-7 | `internal.rs:830` | RIS（终端复位）时 `let _ = terminal.set_selection(None);` 失败 → 完整复位后旧选区仍然存活，无日志。 |
| N2-8 | `context.rs:483` | `attach_surface` 中 `let _ = self.ensure_frame_texture(...)` 失败被丢弃 —— 而 `:480` 附近的注释明确写着「缺少预分配的帧纹理会导致**静默黑屏**」，丢弃的正是能证明这一点的错误。（另：`context.rs:180` 的 `device.poll` 被丢弃，而 `pass.rs:891` 对同一调用 `log::warn!` —— 级别不一致。） |
| N2-9 | `Bridge.kt:324` | `event.text.ifEmpty { null }` → `TerminalRuntime.kt:1586` 跳过 `setClipboardText`。程序用 `ESC]52;c;BEL` **清空**剪贴板的意图被静默忽略，旧内容保留。 |
| N2-10 | `NativeQueryPort.kt:105` | `catch (_: Exception) { emptyList() }` —— 原生搜索 JSON 解码失败变成「0 个匹配」，与真实的「没找到」不可区分，且**无日志**（对比 `Bridge.kt:307` 对同类错误是有日志的）。 |
| N2-11 | `FontInfoDto.kt:29` | `catch (_: Exception) { null }` —— 字体信息 JSON 格式错误 → 设置页静默显示占位符。 |
| N2-12 | `ClipboardAccess.kt:44, 48` | `catch (_: Exception) { false }` 与 `manager()` 的 `false` —— 剪贴板服务死亡时粘贴按钮永久变灰，无任何诊断。 |
| N2-13 | `InputBatchBuffer.kt:88-97` | `catch (exception: RejectedExecutionException) { }` **空 catch 体**，而字节已在 `:80-82` 从缓冲区取走 → 击键被丢弃。注释称「已调用 close()（视图已分离）」，但 `send()` 是正常操作路径，与 `:112` 的 `close()` 路径语义不同。 |
| N2-14 | `pty.rs:363-371` | `chdir` 失败向 fd 2 写消息后**继续在 `/` 下运行** —— 用户可见，但这是未声明的 Fallback（`DESIGN.md:24`），且 shell 起始目录是错的。 |
| N2-15 | `RenderWatchDog.kt:20, 42` | `10_000_000_000L` 与 `2000L` 是魔数，同文件已有具名常量 `CHECK_INTERVAL_MS`，风格不统一（`AGENTS.md:25`）。 |

### 吞错点密度（供后续轮次参考）

| Rust 文件 | 吞错点数 | Kotlin 文件 | 吞错点数 |
| --- | --- | --- | --- |
| `android/ffi.rs` | 79 | `runtime/TerminalRuntime.kt` | 88 |
| `ghostty_terminal/internal.rs` | 71 | `bridge/Bridge.kt` | 28 |
| `terminal/pty.rs` | 21 | `ui/TerminalSurface.kt` | 26 |
| `render/font/mod.rs` | 15 | `TerminalViewModel.kt` | 19 |
| `render/font/font_db.rs` | 13 | `MainActivity.kt` | 15 |
| `render/cell_builder.rs` | 13 | `installer/SecondStageRunner.kt` | 5 |
| `render/font/pipeline.rs` | 12 | 其余 10 个文件 | 各 1–4 |

生产 Kotlin 中 `!!` 出现次数为 **0**（这一点是好的）。`ffi.rs` 与 `TerminalRuntime.kt` 的吞错点密度是次高文件的约 4 倍。

---

## 五、锁与阻塞的完整图景（第 8 轮的另一条轴）

### 锁清单（Rust，22 个原语）

全局：`RENDER_STATE`（`ffi.rs:99`，`std::sync::Mutex`）、`SESSION_REGISTRY`（`ffi.rs:92`，`parking_lot::RwLock`）、`REQUEST_REGISTRY`（`ffi.rs:267`）、事件 FIFO 双锁（`event.rs:50,52`）、6 个 `AtomicU64`/`AtomicBool`、`CACHED_FONT_DB` / 家族索引 / fonts.xml 别名（3 个 `OnceLock`）、`EXTRA_FONT_PATHS`（`font_db.rs:28`）、`CURRENT_LOCALE`（`font_db.rs:197`）、`GlobalGpu`（`context.rs:49`）、`GLOBAL_SURFACE`（`context.rs:842`，**生产零调用，死原语**）、`GPU_BENCH_LOCK`（`render/mod.rs:38`，仅测试）、`pty_write_responses`（`public_api.rs:52`）。
每会话：`Arc<Mutex<Session>>`、5 个闩锁、6 个原子、若干有界通道。

Kotlin：`sessionLock`、`monitorLock`、`InputBatchBuffer.lock`、`BootGuard.LOCK`、`BootstrapOrchestrator.processLock`、`LatencyProbe`。`synchronized` 只出现在 5 个文件里（共 41 处），无 `ReentrantLock` / `Mutex.withLock`（除一处协程互斥量）。

### 锁序图与环

已确认的环只有两个，都是 `ffi.rs` 里的 `RENDER_STATE ↔ SESSION_REGISTRY ↔ Session`（第 6 轮 P0-1 / P0-2 / N1）。本轮额外验证了一个**必要条件为假**的事实，可以排除第三类环：

> `native/src/android/ffi.rs` 中 `call_method` / `call_static_method` 出现次数为 **0**，整文件只有一处 `std::thread::spawn`（`:1183`）。因此**任何 JNI 导出都不可能在持有 `RENDER_STATE` / `SESSION_REGISTRY` / `Session` 时回调进 Kotlin**（Java 侧的重入）。
> `EXTRA_FONT_PATHS` / `CURRENT_LOCALE` 的写入方（`ffi.rs:2947, 2998, 3037`）都在取 `render_state_mut()` **之前**释放。
> 字体管线与 wgpu 队列**自身不加锁** —— `FontPipeline` 完全由 `RENDER_STATE` 保护。

结论：**第三类死锁不存在**。`K1(sessionLock) → L1 → L2 → L3` 是一条线性链，构成的不是死锁而是**阻塞车队**：任一持 `RENDER_STATE` 达 2 秒的路径（第 6 轮 N0-3 / 本轮 N0-11）都会冻结全部 41 个 `sessionLock` 站点。

### 持锁跨阻塞调用的完整清单

| 位置 | 持锁 | 等待 | 最坏 | 用户可见 |
| --- | --- | --- | --- | --- |
| `ffi.rs:1130/1156` | 注册表读锁 + 会话锁 | `flush()` 的 `recv_timeout` | **5s × (1 + 后台会话数)** | 本轮 N0-10：全应用冻结 |
| `ffi.rs:1351→1357` | `RENDER_STATE` | `attach_surface`：能力查询 + `configure` + 预热 + `acquire_texture`（2s；通道满时**无上界**，`pass.rs:154-158`） | 2s / 无上界 | 从 **UI 线程**进入（`TerminalSurface.kt:2659` `surfaceCreated`）→ 界面冻结 → 触发 `AnrWatchDog` → `BootGuard.exit` → **杀进程丢全部会话** |
| `ffi.rs:1522` | `RENDER_STATE` | `begin_frame` + `submit` + `present` | 2s / 无上界 | `setScrollYPx` / `getCellWidth` / `setCursorColor` 排队 |
| `ffi.rs:2948-2954, 3039-3045` | `RENDER_STATE` | 字体数据库初始化（文件 I/O） | ~3s（`ffi.rs:160` 自述） | 本轮 N0-11：改字号冻结终端 |
| `ffi.rs:2520-2529` | `RENDER_STATE` | `env.new_string`（JVM 分配/GC） | 不定 | 唯一在持 `RENDER_STATE` 时回调 JVM 的导出（对比 `ffi.rs:2494/2544/2046-2047/3048` 都先 drop） |
| `ffi.rs:2866` | `RENDER_STATE` | `reset_atlas`：2048² 位图清零 + 全 ASCII 重光栅 | 10–100ms/次 | 双指缩放每帧调用 → 卡顿 |
| `TerminalRuntime.kt:2502` | `sessionLock` | `attachSurface` → 上述 2s GPU | 2s+ | 冻结创建/切换/关闭/暂停渲染 |
| `TerminalRuntime.kt:507-526` 等 | `sessionLock` | `join(1s)` ×2 + `RenderWatchDog.stop()` 的 `runBlocking(2s)` + `syncGridDimensions` → `RENDER_STATE` | **约 4s 累计** | 与同文件 `:476-477 / :2511-2518 / :935-939` 自述的「join 绝不可在锁内」直接矛盾 |
| `TerminalRuntime.kt:3288-3290` | `sessionLock` | `getActiveSessionTitle` → 注册表锁 → 会话锁 → `title()` 的 500ms VT RPC | 500ms | `MainActivity.onDestroy:274` 在**主线程**上跑它 |
| `TerminalSurface.kt:2090` | 无（但要取会话锁） | `focusChange` → `focus_event` → 50ms RPC；且要与每滚动帧调用一次的 `setScrollOffset`（**写锁**，`ffi.rs:3124`）抢注册表锁 | 50ms + 排队 | 焦点切换掉帧 |

### 生命周期与可见性

- `TerminalSurface.kt:2659-2667 surfaceCreated`（UI 线程）一次背景→前台转换要取 `RENDER_STATE` **三次**（`attachSurface` + `recomputeGrid` 的 `getCellWidth`/`getCellHeight` + `setRenderPaused(false)`）；`surfaceDestroyed` 再取 N 次（每会话一次 `setRenderPaused`）。这就是 `DESIGN.md:160`「返回无卡顿/黑屏/跳跃」被破坏的直接机制。
- `RenderWatchDog` / `AnrWatchDog` 的 `watchJob`、`SessionEntry.renderThreadRef` 是非 volatile 的普通 `var`（本轮 N1-13）。
- `Dispatchers.IO` 用在真正的阻塞工作上（`TerminalRuntime.kt:244` 等）——正确。`Dispatchers.Default` 只跑 `delay` 循环（`RenderWatchDog.kt:28`）——浪费但无害。**全仓无 `GlobalScope`**（好）。

---

## 六、修复顺序（本轮增量）

1. **N0-10 + N0-7 一起**（`flush()` 与输出通道移出 `pollEvent` 锁区）—— 目前最高杠杆的一处改动，同时消除「注册表冻结 5s ×N」与「全应用 shell 冻结」。
2. **N0-11**（字体数据库初始化移出 `RENDER_STATE`）—— 消除改字号时的数秒冻结。
3. **N1-12**（`scope.cancel()` + `onCleared` + `surfaceTransitionExecutor.shutdown()`）—— 消除每次旋转的线程与协程泄漏。
4. **N1-7 / N1-8 / N1-9 / N1-10**（剪贴板写入丢弃、14 个无日志查询降级、OSC 52 空应答、选区高亮降级）—— 都是「静默改变用户可见行为」，四处的修法一致：要么记 error 后按不可用处理，要么按 `DESIGN.md:24` 直接崩溃。
5. **N1-11**（`tcsetattr` 失败记诊断并退出）—— 一行 `_exit` + 一行 fd 2 写入。
6. **N1-13**（`@Volatile`）—— 两处。
7. **N2-1**（`font_db.rs` 解析失败按 `DESIGN.md:96` 崩溃退出，并与 Kotlin 层对齐）—— 当前两层策略相反，任何一层改动都会暴露另一层。
8. 其余 N2 项与第 6、7 轮清单。

---

## 七、收敛状态

- 本轮**不是**「无新问题」的一轮。新增 **2 个 P0、7 个 P1、15 个 P2**。
- 「连续四次无新问题」计数**第三次归零**，从第 8 轮重新开始。
- 三次归零的规律已经稳定：**每换一个审读维度（前五轮按文件、中间两轮按目录、本轮按类别），就立刻在「此前没被那个维度覆盖到」的位置产出 P0**。这说明缺陷密度高到任何抽样都必然漏掉 P0，而不是某一轮做得不够认真。
- **给下一轮的建议**：不要再换维度（维度已穷尽），改为**修复优先**。第 6、7、8 三轮共记录 **8 个 P0 / 22 个 P1**，其中至少 6 个 P0 是「几行改动即可消除永久错误状态」（`join` → `join_with_timeout`；`take_kitty_placements` 移出 `RENDER_STATE`；`Result(errors.isEmpty(), …)`；`flush()` 移出锁区；两侧 canonical 化；复用已有的路径谓词）。继续审查的边际收益已经低于先把这些改掉。
- 由于本轮任务限定「不实际修改代码」，上述建议以文档形式留存，等待授权后执行。
