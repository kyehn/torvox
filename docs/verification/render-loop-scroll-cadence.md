# REPORT — 滚动卡顿根因与修复（render loop cadence）

分支：`fix-perf-perf-omp`（worktree `wt-perf-omp`），任务 **fix-render-performance T1**

---

## 一、症状（实测证据）

- **真机 logcat**：滚动瞬间 native 渲染循环掉到 ≈1fps（`loop avg=509ms → 9fps → 恢复 58-66fps`），
  `SLOW_FRAME session=1 render=35.88ms`；但 `frame timing avg=0-1ms`（绘制本身极快）。
- **模拟器复现（同一根因）**：滚动/输出峰值时 `loop timing` 窗口掉到 `avg=414ms p95=500ms max=502ms ≈2fps`；
  `SLOW_FRAME render=36-48ms`（模拟器软件渲染 present 成本，远低于 500ms 的 park）。

**结论**：瓶颈在渲染循环的**调度/等待（latch park）**，不在 GPU 绘制。
`frame avg≈0-16ms`（绘制快）而 `loop avg≈500ms`（等待占大头），两者之差 ≈ **500ms 空闲 latch 停车**。
`500ms` 恰为 `RENDER_LATCH_IDLE_TIMEOUT_NANOS`。

---

## 二、根因

渲染循环此刻度由两个 gate 共同决定，而**两者都只依赖 `lastSignalNanos` 的新鲜度**：

1. **Choreographer vsync 回调**（主线程）：仅当 `now - lastSignalNanos <= 5s` 才 `vsyncRequested=true`。
   一旦 idle clock 超过 `RENDER_IDLE_THRESHOLD_NANOS (5s)`，vsync 停止泵帧。
2. **loop-top latch gate**（渲染线程）：`lastSignalNanos > 5s` 时选择 **500ms** 空闲超时，否则 17ms 活跃超时。

> 5s 无信号后：vsync 不再 pump + 循环停车 500ms。滚动手势只能靠**每次 motion-event 的
> `notifyRender()` unpark** 把循环唤醒。在事件间隔内（模拟器稀疏 motion 事件；或真机主线程被
> 手势 + Compose 重组占满）没有任何唤醒源 → 循环一次 park ~500ms → 滚动帧塌到 ~1-2fps。

**失效点**：决定性的不是 `scrollRemainderPx` 逐帧微调重建（native 是增量 dirty-band，本就走 idle 重绘路径并修缮
`scroll_px_changed` —— `frame avg=0-1ms` 已证明原生滚动重绘极快），而是**循环在新兴场景空转 latch**。

---

## 三、修复（一个小步、一个 commit）

`android/app/src/main/java/terminal/emulator/runtime/TerminalRuntime.kt`

- 新增 `SCROLL_MOTION_WINDOW_NANOS = 250ms`。
- 新增 `SessionEntry.hasScrollMotion()`：`now - lastScrollNanos < 250ms`。
- `setScrollRemainderPx()` 同步戳 `lastScrollNanos`（与 `setScrollOffset()` 对齐；逐像素亚行运动是拖动中最高频信号）。
- vsync 回调：`lastSignalNanos` 新鲜 **或** `hasScrollMotion()` 都继续 `vsyncRequested=true` + pump。
- loop-top gate：空闲 latch 仅在 `idleNanos > 5s **且** !hasScrollMotion()` 时启用；
  滚动运动期间始终走 **17ms 活跃 cadence**。

**效果**：手势移动期间循环保持活跃 cadence（vsync 泵帧 + 17ms latch 兜底），初帧即渲染 → 滚动回到 ~60fps；
手势停止移动 ~250ms 后 `hasScrollMotion()` 变 false，**空闲 latch 按原设计重新生效**（不回归省电）。

**未改动**：native/（`render_inner`、滚动重建路径无需改动，原生滚动重绘本就不贵）；
`Cargo.toml`、`build.gradle.kts`、`docs/specification/`、`README.md`、`AGENTS.md` 均未改。

---

## 四、前后数据（模拟器 logcat `loop timing`，同机同法）

| 场景 | 修复前（baseline） | 修复后 |
|------|--------------------|--------|
| 滚动/活动峰值 | `avg=414ms p95=500ms max=502ms ≈2fps` | `avg=25ms p95=22ms max=385ms ≈40fps` |
| 活动后稳定 | `avg=16ms ≈62fps` | `avg=14-15ms ≈62-71fps` |
| 空闲（5s+ 无输入） | latch 生效 ~2fps | 按构造仍 ~2fps（省电未回归） |

**解读**：修复前滚动窗口的 `p95=500ms` 就是 500ms 空闲 latch 停车；修复后滚动期间 `p95=22ms`，
500ms 停车帧消失。模拟器仍受软件渲染 present 成本（`frame max` 偶发几十～几百 ms）限制，故为 ≈40fps；
真机 `frame avg=0-1ms`（present 极快）按同机制应恢复到满 60fps。

---

## 五、验证

- **`cargo test -p native --lib`（nix develop）**：548 passed；1 failed
  `render::tests::bench_gpu_buffer_upload_throughput` —— 该测试用真实 GPU `Renderer::new_with_no_surface()`
  的 `queue.write_buffer` 做吞吐基准，本环境无 GPU（创建 device 报 `NotSet`）；**本次改动零 Rust 触碰**，
  属环境性失败，与代码无关。
- **Kotlin 编译 + APK 构建**：`nix develop && ./gradlew :app:assembleDebug` → `BUILD SUCCESSFUL in 51s`，
  `app-debug.apk (82.5M)`。
- **真机等价 logcat 对比**：见上表（模拟器 com.termux，安装最新 APK）。
- `git diff` 自审：仅 1 个 Kotlin 文件 +40/-3，无语法/逻辑问题。

---

## 六、文件清单（本 commit）

- `android/app/src/main/java/terminal/emulator/runtime/TerminalRuntime.kt`
- `openspec/changes/fix-render-performance/specs/render-loop-scroll-cadence/spec.md`（新增 delta spec）
- `openspec/changes/fix-render-performance/tasks.md`（更新 T1 状态）

## 后续（未做，属其他任务）
- T2 IME 动画流畅性、T3 退格慢：仍开放。