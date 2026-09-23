# Fix Render Performance

## Why

用户主诉：上下滚动卡顿/撕裂/底部残留、输入法动画不流畅、退格慢、总体性能差。

## What Changed

- 滚动时渲染循环掉帧：真机 logcat 实测 loop avg=509ms ≈1fps（scroll）→ 恢复 58-66fps，SLOW_FRAME render=35.88ms
- frame avg=0-1ms（绘制本身极快）→ 瓶颈在循环等待/滚动重建路径，非 GPU 绘制
- 输入法弹出/隐藏动画：取证确认暂停链（setRenderPaused/attachWindow/applySurfaceResize）全程零调用；
  insets 逐帧值读取从主组合体移入叶节点 `WindowImeBottomPx`，位移经 snapshotFlow 收集器 +
  placement 阶段 offset 应用——动画帧只重组单节点、只重排位移布局，主组合不逐帧重组；
  循环 cadence 与定居语义不变（详见 specs/ime-animation-smoothness）
- 退格（backspace）处理的帧延迟与编码链路待优化

## Impact

- android/app/src/main/java/terminal/emulator/runtime/TerminalRuntime.kt（渲染循环 timing/latch）
- android/app/src/main/java/terminal/emulator/ui/TerminalScreen.kt（IME insets 处理：叶节点观察者 +
  snapshotFlow 收集器 + 布局期 offset）
- native/src/render/（滚动/重建路径、pass.rs）
- 输入法链路（IME 动画 + 退格编码）

## Tasks

T1: 滚动卡顿根因与修复（native + runtime 循环）
T2: IME 动画流畅性（onImeInsetsChanged/暂停链）
T3: 退格慢（编码/发送/渲染链路）
