# 第 14 轮专项审查（注释断言 vs 代码行为）

审查日期：2026-09-30
基线提交：`7944461`
方法：第 11–13 轮的四个 P0 全部属于同一缺陷类别 ——
**注释/文档断言的不变量、成本或保证，代码并不提供**。本轮把该类别单独抽出作为审读维度，
对全仓（`*.rs` / `*.kt` / `*.kts` / `*.nu` / `*.wgsl` / `*.xml` / `*.yml`）系统扫描六类断言：
成本声明、否定式保证、回退声明、不变量声明、死代码声明、规范引用。
本轮只审查，**未改动任何源码**。

---

## 一、本轮的核心发现：这些失真已经形成体系

第 11–14 轮共 4 轮，产出 6 个此类 P0。它们不是六个独立笔误，而是**同一套成本模型从未被回读验证**
在三个文件里的六次投影：

| 断言位置 | 断言内容 | 实际 | 差值 |
| --- | --- | --- | --- |
| `TerminalRuntime.kt:2964`、`:2717` | 每会话 stop「最长 1s」 | 3s（`renderWatchDog.stop()` 的 2s `runBlocking` + 1s join） | **3×** |
| `ffi.rs:136`、`:1515` | `scrollback_length()` 阻塞渲染线程「最多 50ms」 | 500ms（`QUERY_TIMEOUT_MS`） | **10×** |
| `ffi.rs:1484`、`:1548` | 空闲路径每帧「省约 32KB」克隆 | 24×80×96B = **≈180KB** | **5.6×** |
| `TerminalRuntime.kt:3341` | 关闭死渲染线程「后台约 20s」 | 6×500ms + 2500ms 退避 + 5×300ms = **≈7.0s** | **2.9×**（方向保守） |
| `internal.rs:482` | 「深缓冲（2 万行）」 | `DEFAULT_SCROLLBACK_LINES = 2000` | **10×** |
| `pass.rs:129-130` | 每帧省 `std::thread::spawn` 的「约 1ms」 | `OnceLock` 省的是**每进程一次**建线程；每帧只剩一次 `sync_channel(1)` 分配（亚微秒） | **量级误置** |

规律：**没有一个数字是随手编的** —— 它们都是**从别处照抄的**（50ms 抄自
`FOCUS_MODE_QUERY_TIMEOUT_MS`，20000 抄自某个历史配置，32KB 抄自 `CellData` 布局改写前的旧尺寸，
1ms 抄自 `thread::spawn` 的真实成本但用错了位置）。
这说明**没有任何机制会验证注释里的数字**，而 `STYLE.md:66` 要求「注释保持极简」的同时
并未要求注释**正确**。这是规范本身的缺口。

---

## 二、新的 P1

### N1-36 `cached_scrollback` 是**只写字段**，其「空闲时复用」的声明从未兑现

`ffi.rs:136` 的字段文档：

```rust
/// 缓存的回滚长度——在 FrameData::New 时更新，空闲时复用，避免同步
/// `scrollback_length()` RPC（VT 线程繁忙时会把渲染线程阻塞最多 50ms）。
cached_scrollback: u32,
```

**全仓三处命中，无一处读取**：

```text
ffi.rs:137   cached_scrollback: u32,                       ← 声明
ffi.rs:194   cached_scrollback: 0,                          ← 初始化
ffi.rs:1560  render_state.cached_scrollback = scrollback;   ← 唯一写入
```

即：每帧都写、从不读。「空闲时复用」是假的。
同时该注释把 `scrollback_length()` 的代价说成「最多 50ms」，实际是
`QUERY_TIMEOUT_MS = 500`（`types.rs:234`，经 `public_api.rs:474` 的
`recv_timeout` 生效）；`50` 是 `session.rs:485` 的 `FOCUS_MODE_QUERY_TIMEOUT_MS`
—— 一个完全不同的常量，被复制到了错误的注释里。

**后果**：
(1) 违反 `STYLE.md:63`（不得保留死代码）；
(2) 「把回滚长度随 `CursorInfo` 一起传」这一设计的**存在理由**被错误陈述 ——
真实的理由是「避免 500ms 阻塞的同步 RPC」，而按注释的 50ms 理解会得出
「同步查询也不贵」的相反结论，未来有人据此重新引入该 RPC 时会按 50ms 预算设计。

**修法**：删除该字段（回滚长度已随 `CursorInfo` 传递，无需缓存），
或为 `FrameData::Idle` 路径补上读取。两种情况都应把 `50ms` 改为 `500ms`。

### N1-37 `TerminalSurface.surfaceDestroyed` 断言的 Surface 释放不变式**既未接线，也不可能成立**

`TerminalSurface.kt:2717-2723` 的注释：

> 仅在渲染线程被 join 之后才释放 Android Surface（pauseRendering 跑在 Surface 转换执行器上，
> 其 join 每会话最长 1s）… 执行器的顺序保证 join 已完成

两个断言都不成立：

1. **「每会话最长 1s」** —— 与 N0-19 同一根因，实际 3s（`RenderWatchDog.stop()` 的
   2s `runBlocking` 在 join 之前，`TerminalRuntime.kt:1540` vs `:1548`）。
2. **「执行器的顺序保证 join 已完成」** —— 紧随其后的代码（`:2721-2723`）只把
   `lastConfiguredWidth/Height` 与 `currentSurface` 置空，**并未等待**
   `surfaceTransitionExecutor`；而 `pauseRendering`（`:2977`）只是向该执行器**投递**任务。
   专门为建立该顺序而写的 `TerminalRuntime.runAfterRenderThreadsStopped()`
   （`:3350`）**全仓只有它自己的定义，零调用点**。

> **重要说明**：`runAfterRenderThreadsStopped` 零调用这一点**不是本轮新发现** ——
> `docs/REVIEW.md:126` 与 `docs/REVIEW-ROUND6.md:60`（P1-13）已分别记录，
> 且第 6 轮明确判定为「成立」。**它至今未被修复。**
> 本轮的新增内容是：(a) 同一处错误的 1s 数字在第二个站点重复出现；
> (b) 断言中的「执行器顺序保证」被明确写成了已成立的事实。

**后果**：`currentSurface = null` 与一个可能仍在 `ANativeWindow_fromSurface` / wgpu 内部
运行的渲染线程竞争。该不变式从未被保护。

**修法**：`surfaceDestroyed` 的清理改走 `runAfterRenderThreadsStopped`（顺手让该函数不再是死代码），
并把两处「1s」更正为 3s。

---

## 三、新的 P2（注释失真，低危但会误导后续修改）

| 编号 | 位置 | 断言 vs 实际 |
| --- | --- | --- |
| N2-79 | `ffi.rs:1484`、`:1548` | 「每帧省约 32KB」；`CellData` 为 96B（`types.rs:92` 断言），24×80 = **180KB**，真机网格更大（50×120 ≈ 540KB）。该数字早于 `CellData` 字段集重写。 |
| N2-80 | `pass.rs:129-130` | 「省掉 Android 上每帧约 1ms 的 `std::thread::spawn` 开销」。`OnceLock` 确实省建线程，但建线程是**每进程一次**；每帧仍有一次 `sync_channel(1)` 分配（亚微秒）。把 hang 防护（`pass.rs:121-122` Mali-G57）与省建线程混为一谈，可能诱导后人「优化」掉真正的超时防护。 |
| N2-81 | `TerminalRuntime.kt:3341` | 「后台约 20s」。实算：`shouldCloseDeadRender(attempts, 5) = attempts > 5` ⇒ 6 轮；6×`RENDER_MONITOR_INTERVAL_MS`(500ms) + 退避 (100+200+400+800+1000) + 5×`GRACE_PERIOD_AFTER_RESTART_MS`(300ms) = **≈7.0s**，尚未计入每次最多 1s 的 join。方向保守（高估了被否决方案的代价），但常量错误。 |
| N2-82 | `internal.rs:481-482` | 「深缓冲（2 万行）会被拦腰截断」。`DEFAULT_SCROLLBACK_LINES = 2000`（`session.rs:48`，用于 `:351`），差 10×。该注释是「必须同步解除字节预算」的论证依据，论证对象与实际配置不符。 |
| N2-83 | `TerminalRuntime.kt:1419` / `:1653` / `FrameTimingTrend.kt:6` | 同一指标两套基线：「模拟器基线 ~555ms/帧」与「实测模拟器空闲窗口平均个位数毫秒」不可同真。`FrameTimingTrend` 的绝对阈值（`FRAME_TIME_WARN_P95_NANOS = 1s`、`DEFAULT_ATTENTION_FLOOR_NANOS = 100ms`）是按 555ms 标定的，与当前空闲门控后的实测行为脱节。 |
| N2-84 | `TerminalRuntime.kt:1133` | 「闭锁超时（活跃 16ms / 空闲 500ms）」；`RENDER_LATCH_TIMEOUT_NANOS = 17_000_000L`（`:1642`）。`NativeBridge.kt:118` 写「约 16ms」，常量自身注释（`:1634`）写「17ms」，三处不一致。 |
| N2-85 | `TerminalRuntime.kt:189` | 「空闲终端上自我维持的 60-166fps 循环」；166fps 对应已移除的 8ms 锁存，当前活跃地板是 17ms（≈59fps）。 |
| N2-86 | `session.rs:126-127` | 「在注册表写锁内查询会让所有会话操作最多阻塞 2×QUERY_TIMEOUT_MS」；`switch_session_inner`（`:549-563`）只取写锁 + 一次 `AtomicU64::store`，**不发任何查询 RPC**。 |
| N2-87 | `ffi.rs:1096` | 「`wait_exit_code` 最多忙等 100ms」；实现在 `:1069-1077` 是 `sleep(10ms)` × 10，不是忙等。 |
| N2-88 | `session.rs:480-481` | 「`MAX_CHUNKS_PER_FRAME = 10` 是每会话帧处理的最大 VT 输出块数」；`ffi.rs:1157` 对后台会话传 `PTY_POLL_CHUNKS_PER_FRAME = 2`，10 只用于活动会话的 `drain_output`（`session.rs:543`）。 |
| N2-89 | `ffi.rs:503` | 「`Session::drop`（数十至数百毫秒）」；`join_with_timeout` 在初始超时后重试 3×100ms（`session.rs:664-679`），`Drop` 对**两个**句柄各调一次且前置 50ms sleep ⇒ 最坏约 800ms。 |
| N2-90 | `ffi.rs:1618` vs `:136`/`:1515` | `:1618` 正确写「500ms 超时兜底」，而 `:136`/`:1515` 对同一 RPC 写「50ms」。同一文件内三处对同一超时的三种说法。 |
| N2-91 | `ffi.rs:2045-2048` | 「设备 16MB + 暂存位图 16MB」；2048²×4B = 16.8MB **合计**一个量级，读起来像 32MB。另「约 700 → 约 2800 字形」为无出处的估算。 |
| N2-92 | `TerminalSurface.kt:1206` / `TerminalScreen.kt:76` | 后者注释描述「3 个稳定帧 × 16ms = 48ms」的轮询机制，前者实际是 `Handler.postDelayed(48)`。已被替换的旧机制仍留在注释里。 |
| N2-93 | `TerminalViewModel.kt:805-806` | 两行悬空注释「剪贴板粘贴的上界…也是流式发送它所用的块大小」下方**没有任何常量**（companion object 在 `:807` 结束）。常量实际在 `PasteChunker.kt:46`/`:49` 且是两个不同值。 |
| N2-94 | `render/mod.rs:35-36` | 「串行化 GPU 基准…保证一次只跑一个」被 `#[cfg(test)]` 门控（`:37`），**生产二进制中根本不存在**，但注释读起来像生产不变式。 |
| N2-95 | `ffi.rs:11-14` | 文档化的锁顺序 `SESSION_REGISTRY → Session → exit_code` **遗漏 `RENDER_STATE`**，而 `:1534-1536` 恰恰把 `RENDER_STATE` 称为唯一的方向反转点。 |

---

## 四、经核实**正确**的断言（记录以免后续轮次重复怀疑）

以下类别已系统核对并成立：

- **成本常量成立**：`TerminalRuntime.kt:1829`「等待至多 3 秒」（60×50ms，`：1830-1834` 精确）；
  `RENDER_HANG_TIMEOUT_NANOS` 10s 与看门狗默认一致；`CLIPBOARD_ANSWER_TIMEOUT` 2s 与 `:1187` 一致；
  `EXIT_CODE_WAIT_TIMEOUT_MS` 100ms 与 10×10ms 轮询一致；
  `event.rs:12` `OVERFLOW_WARN_INTERVAL` 1s 与 `:98`「每秒至多一次」一致；
  `ffi.rs:225` `PTY_POLL_CHUNKS_PER_FRAME = 2` 与 `:1156` 一致；
  `context.rs:15` `GPU_DRAIN_POLL_QUANTUM` 16ms 与「一帧 60Hz 量级」一致；
  `ffi.rs:133`「`dirty_mask` 约 100-300 字节 × 120fps」（`Vec<bool>` 长度 = 行数）成立，
  且 `:1685-1687`/`:1775-1777` 的 `clear()+resize()` 复用真实存在。
- **否定式保证成立**：`ffi.rs:14`「绝不在持有 `Session` 锁时获取 `EVENT_QUEUE`」——
  `collect_session_events` 在 `:1145` 返回，事件队列只在 `:1214`/`:1218` 被触碰，
  此时注册表 guard 已在 `:1168` 释放；
  `ffi.rs:1534-1536` 的方向反转自述准确，且对应的正向站点
  （`setSelection` `:2726-2730`、`setTheme` `:2792-2798`、`setScrollOffset` `:3189-3194`、
  `getGridRowsColsPacked` `:3136-3144`）确实都先 `drop()` 再取 `RENDER_STATE`；
  `event.rs:72-75`「绝不丢弃 `Exit`」—— `:81-94` 的淘汰扫描确实在全 `Exit` 时回退为丢弃**新**事件。
- **不变量声明成立**：`TerminalRuntime.kt:124-129`（`restartScheduled` 期间不 release/close）；
  `:1031-1035`（每个 `startRenderThread` 调用方都持 `sessionLock`，`entry.closing` 不可被竞争）；
  `:320-322`（`modifierBarHeightPx` 确有三个消费方）；
  `TerminalSurface.kt:2196`（「`scrollbackLength - scrollOffset` 唯一来源」——两条路径都走 `currentViewportTopGrid`）。
- **回退声明真实可达**：`pty.rs:741-745` 的 `getpgid` 失败直接 kill 分支、
  `session.rs:720-723` 同进程组自 kill 守卫、`internal.rs:570` 的 bell 丢弃，均为真实分支。
- **规范引用无漂移**：`session.rs:47`「PROHIBITED 禁止终端回滚行数设置」、
  `TerminalRuntime.kt:1594`「DESIGN Shell 节」指向的章节确实如此表述；
  `REFERENCE.md` 合并后残留的 `DESIGN.md:N` 行号引用未发现漂移。
- **本轮未发现新的死代码方向**：`cached_scrollback`（N1-36）是唯一的只写字段；
  第 11 轮已归档的 `row_cache`、N2-32/33/37、N2-48、N2-62 经复核描述仍准确，无新增同类。

---

## 五、需要用户裁决的问题

1. **规范缺口：没有任何机制要求注释里的数字正确。** 本轮六个失真全部是照抄别处的数字。
   `STYLE.md:66` 只要求「注释保持极简」。是否要在 `STYLE.md` 增加一条
   「注释中的数值常量必须与代码常量同源（引用常量名而非复述数值）」？
   （`docs/specification/` 是保护文件，需授权。）
2. **`runAfterRenderThreadsStopped` 零调用已跨三轮未修**（`REVIEW.md:126`、
   `REVIEW-ROUND6.md` P1-13、本轮 N1-37）。它同时是死代码与一个未兑现不变式的证据。
   是否连同 N1-37 一起在下一轮修复？
3. **`AnrWatchDog`**（第 9 轮 D3 / 第 11 轮 Q6 / 第 12 轮 Q6 / 第 13 轮 Q1，四次未决）。

---

## 六、修复顺序建议

1. **N1-36 `cached_scrollback`** —— 删字段（连同两处错误注释）。一行级改动，同时消掉死代码与错误的设计理由陈述。
2. **N1-37 Surface 释放不变式** —— 让 `runAfterRenderThreadsStopped` 有调用方，顺带消除跨三轮的未修项。
3. **成本数字批量更正**（N2-79 ~ N2-84）—— 这些数字是后续容量与性能判断的依据，
   错误数字比没有数字更危险。优先更正被**用作论证**的三处：
   `internal.rs:482`（字节预算解除的论证）、`TerminalRuntime.kt:3341`（方案取舍的论证）、
   `ffi.rs:136`/`:1515`（RPC 预算的依据）。
4. **N2-90 同文件内三处超时不一致** —— 应统一引用 `QUERY_TIMEOUT_MS` 的常量名。
5. 其余 N2 项。

---

## 七、收敛状态

- 本轮**不是**「无新问题」的一轮：新增 **2 个 P1、17 个 P2**（其中 6 个 P2 属「注释失真」类），
  外加 **6 组经核实的正确断言**。
- 「连续五次无新问题」计数**第一次归零**，从第 14 轮重新开始。
- 累计（第 6–14 轮）：**19 个 P0、47 个 P1、约 126 个 P2/P3**。
- **本轮最重要的产出不是任何单条发现，而是第一节那张表**：
  它给出了「注释失真」这一类缺陷的**量化形态**（3×、10×、5.6×、2.9×、10×、量级误置），
  并指出其成因是**数字被跨文件复制而无人回读**。
  这使得该类缺陷今后可以被一次性系统清理，而不是每轮再发现几处。
- 累计九轮的结论已经完全一致，且证据强度逐轮上升：
  1. 缺陷密度高到任何抽样都会漏 P0（九轮九次验证，每次都产出新 P0）；
  2. 至少 13 个 P0 的修法是「几行改动消除一个永久错误状态」；
  3. **「连续五次无新问题」这个验收标准只有在已修完的代码上才有意义** ——
     在未修的代码上它衡量的是抽样运气，不是代码质量。
  建议停止审查、改为修复；修完后重跑同维度复审，那时若确实连续五轮无新问题，才构成有效的收敛证据。
- 本轮任务限定「不实际修改代码」，上述内容以文档形式留存，等待授权后按第六节顺序执行。
