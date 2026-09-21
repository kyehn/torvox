# fix-startup-grid-resize Proposal

## Why

用户报 IME 弹出时终端底部空白/内容溢出、滚到顶部内容不可见。实机取证（release APK，pid 6347，1080×2209）定位到更底层的根因：**冷启动时网格永久停在会话 spawn 的默认 24×80，直到旋转等外部尺寸事件才恢复**。

证据链：

1. 冷启动日志 `TerminalSurface: applySurfaceResize: surface not valid yet, deferring` —— `onSizeChanged` 先于 `surfaceCreated`/`surfaceChanged` 触发（布局阶段 SurfaceHolder 尚无有效 Surface），`applyResizeNormal` 在该分支 `return`，尺寸被永久丢弃，且 `lastConfiguredWidth/Height` 未写入。
2. 日志 `Session::spawn ... rows=24, cols=80`，此后无任何网格重算：`Bridge.recomputeGrid` 是 ADR-0007 桩（仅打日志），真实路径是 `applyGridResize`（Kotlin）与 `recomputeGridFromFontMetrics`（依赖 `pendingSurfaceWidth/Height`）。
3. `pendingSurfaceWidth/Height` 只由 `runtime.attachSurface(w,h)` 与 `startRuntime(surface,w,h)` 写入；冷启动会话由 `ensureDefaultSession()` 先建、Surface 重挂载路径又跳过了 `applyGridResize`，故 `recomputeGridFromFontMetrics` 因 `surfaceW/H == 0` 恒早退。
4. 实机对照：pid 6347 全程只有旋转触发的 `applyGridResize: 1080 x 2209 -> 45x48 (was 17x108)`，冷启动段无 `applyGridResize`；pid 1713（时序不同）启动段正常得到 45x48。
5. 网格 24 行 ≈ 1160px，屏幕高 2209px → 下半屏约 1000px 空白。IME 弹出时终端内容止于 y≈1200 正是 24 行网格的底边，"底部溢出/顶部内容看不见"是该空白的直接表现（并非 pan 公式双重扣减）。

## What Changes

### R1: Surface 生效前到达的尺寸不再丢弃

`TerminalSurface.ResizeManager` 新增暂存字段 `pendingRetryWidth/pendingRetryHeight`：`applyResizeNormal` 检测到 `!surface.isValid` 时记录尺寸（而非仅打日志返回）；新增 `applyPendingSurfaceResize()`，在 `surfaceCreated` 末尾重放（走 `applySurfaceResizeNow`，幂等：尺寸未变时空操作）。

**文件**: `TerminalSurface.kt` — `ResizeManager.applyResizeNormal` / `applyPendingSurfaceResize` / `surfaceCreated`

### R2: 会话已运行时重挂载 Surface 后必须重算网格

`surfaceCreated` 的 `isRunning` 分支已 `attachSurface(w,h)`（使 `pendingSurface` 尺寸权威）并写入 `lastConfiguredWidth/Height`，导致后续 `applySurfaceResize` 全部早退、`applyGridResize` 永不执行。在该分支显式调用 `resizeManager.applyGridResize(width, height)`。

**文件**: `TerminalSurface.kt` — `surfaceCreated`（isRunning 分支）

### R3: 单元格度量首次可用时重算网格

`syncGridDimensions` 在 `cellWidth/cellHeight` 从 0 变为有效值时调用 `recomputeGridFromFontMetrics()`：启动序列中该同步可能早于 native 字体度量就绪，此前的早退没有任何补偿路径。

**文件**: `TerminalRuntime.kt` — `syncGridDimensions`

## Verification

```bash
cd android && ./gradlew ':app:testDebugUnitTest' \
  --tests 'terminal.emulator.ui.ScrollDistanceTest' \
  --tests 'terminal.emulator.ui.GridToScreenTest' \
  --tests 'terminal.emulator.runtime.InputBatchBufferTest'
```

实机（release APK，冷启动）：

```bash
adb logcat -d | grep -E "applyGridResize|recomputeGridFromFontMetrics"
```

期望：冷启动段出现 `applyGridResize: 1080 x 2209 cell=(22.05,44.63) -> 45x48`，且截图文本铺满整屏（不再有下半屏空白）。
