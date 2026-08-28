# Proposal: full-closure-v3 (4必解问题收敛)

## Why

4项P0/P1未闭环：CJK显示与实际不一致且慢、IME动画跳跃闪烁、切换应用黑屏、抽屉IME按钮无效。历史20+反馈已通过确定性验证闭环，本迭代聚焦4项的深度根因与像素级修复。

## What Changes

- **CJK**: `cjk_fallback_names`保序、`Source Han Serif`惩罚、`locale token边界`、`outline_cache`、多字符投票。
- **IME**: `IME_TOGGLE 80ms`、`SETTLE 3帧(48ms)`、`navigationBarsPadding`单点、`imeFollow`共享、`wasVisible`可见性切换清理。
- **黑屏**: `surfaceCreated 0-size守卫`、`ON_RESUME清render_paused`、`reconfigure_swapchain`原地。
- **抽屉**: `surface.requestFocus()+surface.windowInsetsController`对称token。

## Capabilities

### New Capabilities
- `cjk-font-fidelity` — 设置与渲染一致且首屏<400ms。
- `ime-smooth-follow` — 0丢帧无跳跃闪烁。
- `app-switch-continuity` — HOME往返零黑屏。
- `drawer-ime-toggle` — 连续3次有效。

### Modified Capabilities
- `font-fallback` — 保序与惩罚扩展。
- `ime-animation` — 96ms→48ms settle。
- `surface-lifecycle` — 0-size与暂停守卫。

## Impact

- 4项P0闭环，997 pass，60fps稳态，90fps+峰值可验证。
