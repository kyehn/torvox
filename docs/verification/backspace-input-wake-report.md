# T3 退格/按键空闲唤醒延迟 — 根因与修复报告

分支 `fix-perf-ohmy`（基于 main），worktree `wt-perf-ohmy`。对应
openspec change `fix-render-performance` T3（backspace latency），delta spec
`openspec/changes/fix-render-performance/specs/backspace-input-wake/`。

## 用户现象

终端空闲 5 秒后（渲染循环进入 500ms 空闲 latch、vsync 泵帧停止），按退格
（以及一般按键）后画面更新明显延迟。与滚动卡顿（T1）同一条延迟路径：
渲染循环停车在 500ms 空闲 latch，而非 GPU 绘制或退格处理本身。

## 链路取证（读代码，未操作模拟器）

渲染循环（`TerminalRuntime` renderThread）在无唤醒源时于
`bridge.waitOutput()` 纯 park：idle-clock 新鲜（≤5s）选 17ms 活跃 latch，
过期选 500ms 空闲 latch；**PTY 到达不能提前唤醒 park**（既有行为），
`new_output` 是本机 AtomicBool（`output_processor.rs`），新输出只能由渲染
线程下一次 render 消费，无原生→Kotlin 唤醒。空闲 5s 后 vsync 链不再
poke（idle 钟过期），唯一立即唤醒源是 `SessionEntry.notifyRender()`
（renderSignaled + unpark）。

输入写 PTY 的三条 Kotlin 路径：

| 路径 | 写入方式 | 唤醒渲染循环 |
|---|---|---|
| `TerminalViewModel.writeToPty → runtime.writeToPty`（IME 退格 deleteSurroundingText / setComposingText 增量 / 修饰键栏 BKSP）| `feedPty` + `entry.notifyRender()` | ✅ |
| `Bridge.processKeyEvent`（实体键盘、adb/maestro keyevent、IME `sendKeyEvent` 路由的 KEYCODE_DEL 退格）| `feedPty`/`writeKey`，只走 `onPtyWrite` 延迟打点 | ❌ 不 notify |
| `Bridge.encodeMouseEvent`（鼠标点击）| `Bridge.writeToPty`，同样只打点 | ❌ 不 notify |

## 根因

空闲 >5s 后循环停在 500ms 空闲 latch；退格经 `processKeyEvent`（实体键盘 /
IME sendKeyEvent）或鼠标路径写入 PTY 时**没有 notifyRender**，shell 回显要
等下一次 idle-latch tick（最坏 ≈500ms）才渲染 —— 即"退格慢"。对比：
Gboard 走 deleteSurroundingText 的退格路径已带唤醒（回显 ≤17ms 活跃 latch
上屏），说明瓶颈不是 ghostty 退格处理（frame avg 0-1ms）也不是 IME/编码层
（纯字节映射；commit 批量路径有 ≤50ms handler fallback 兜底，非 500ms 级
延迟）。

## 修复（提交 a383088）

1. **唤醒接线**：`TerminalRuntime` 两处 `bridge.onPtyWrite` 接线
   （createSession 与 start）在延迟打点后追加 `entry.notifyRender()` —— 所有
   PTY 写入路径（硬件键 / IME sendKeyEvent 退格 / 鼠标）统一唤醒渲染循环；
   `runtime.writeToPty` 自身的 notify 不变（幂等双 notify 无害）。
2. **latch 门控提取**：循环内的 latch 选择（`idleNanos > 阈值 && !hasScrollMotion`
   → 500ms idle latch）提取为纯函数 `shouldUseIdleLatch(idleNanos,
   hasScrollMotion, idleThresholdNanos)`，与 `shouldResetScroll` 同风格、
   同注释层级；回显在新 17ms 活跃 latch 内上屏。
3. **空闲回落不变**：输入唤醒只作用于当次写入；停止输入/输出 >5s 后仍回落
   500ms idle latch（T1 语义）—— `notifyRender` 已被既有 `newOutput` 消费 &
   循环 `set(false)`，不会自持活跃循环。

## 单测（新增 `RenderLatchCadenceTest`，3 用例）

- `allFourCombinationsFollowIdleGate`：latch 门控真值表（仅"idle 钟过期 ∧
  无滚动运动"选 idle latch；过期+滚动/新鲜钟均活跃）。
- `clockExactlyAtThresholdStaysActive`：边界——恰在阈值不算过期。
- `notifyRenderRefreshesIdleClockAndRaisesSignal`：唤醒契约——输入写入的
  notifyRender 刷新 idle-clock 且置 renderSignaled，门控随即选活跃 latch。

## 验证（本 worktree 完成部分，模拟器由主 agent 独占）

- `cargo test -p native --lib`：**551 passed; 0 failed**（无 Rust 改动）。
- `:app:testDebugUnitTest`：**602 passed; 0 failed**（含新增 3 用例与
  NativeBridgeSmokeTest，需先 `cargo build -p native` 产出 host
  libnative.so）。
- `:app:spotlessKotlinCheck` / `:app:detekt`：改动文件干净；仅存的
  TerminalScreen.kt / ImePopupPixelInstrumentedTest.kt 违规为 T2 既有，
  未触碰。
- 待主 agent 在模拟器验证：空闲 >5s 后实体键/IME sendKeyEvent 退格的 logcat
  loop 时序（输入→回显应不再出现 ~500ms 级等待）。

## 前/后对比（预期）

| 场景（空闲 >5s 后按键） | 修复前 | 修复后 |
|---|---|---|
| 实体键盘 / adb / IME sendKeyEvent 退格 | 回显等满 500ms idle-latch tick（≈500ms）| 写入即唤醒，回显 ≤17ms 活跃 latch 上屏 |
| Gboard deleteSurroundingText 退格 | 已快（≤~30ms）| 不回退，行为不变 |
| 空闲回落 | 500ms latch | 不变（>5s 无输入/输出回落） |