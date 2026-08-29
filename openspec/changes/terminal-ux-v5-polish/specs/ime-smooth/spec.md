# Spec: ime-smooth + app-switch-continuity (v5 — 零重组)

## Requirement

IME 弹出/收起期间终端内容跟随键盘无跳跃、无闪烁、无文字压扁，切应用往返无黑屏，均可在模拟器像素级验证；**动画期零 recomposition**。

## Design — placement 内联读取 + settled 单次重排

| Phase | Trigger | UI | Grid / Surface |
|-------|---------|----|----------------|
| Animating | `imeInset != settled` | `Modifier.offset { IntOffset(0, -WindowInsets.ime.getBottom(density)) }` **放置阶段内联读 inset，零组合、零重组**（删除 `animateDpAsState spring`） | 不变，0 次 `applyGridResize` |
| Settled | `inset` 连续 `IME_SETTLE_FRAMES=3` 帧不变（16ms 采样） | `padding(bottom=settled)` 单次布局 | 单次 `onImeSettled(settled)` → `applyGridResize` → `reconfigure_swapchain` 原地 |

- `navigationBarsPadding` 仅外层 `Box`
- `onApplyWindowInsets` 仅 `lastImeBottom = inset + clearSelection`，不重排
- `surfaceCreated` 0-size 守卫，`ON_RESUME`：`if (surface.isAttached && width>0) { setRenderPaused(false); resumeRendering() } else postDelayedUnpause(200)`
- Native：`attachSurface` 优先 `reconfigure_swapchain`，仅 `ERROR_NATIVE_WINDOW_IN_USE_KHR` 回退 `release+attach`（保留）

**v5 与 v4 差异**：v4 用 `animateDpAsState spring 4500/0.9` 驱动 `animatedImeBottom`，每帧触发 `TerminalScreen` recomposition；v5 删除该状态，`offset` lambda 内联读取系统 `WindowInsets`（系统侧 `WindowInsetsAnimation` 已插值，无需二次 spring）

## Scenarios

- Given 任一会话，when IME show 0→320dp 动画期，then `rows/cols` 不变、0 次 `applyGridResize` 日志、pan 逐帧跟随且 `TerminalScreen` recomposition 0 次（`Layout Inspector` 或 `logcat Composition` 计数）；3 帧稳定后 padding 生效、单次 `applyGridResize`、prompt 在键盘上方可见
- Given 文本被选中，when 任意 IME inset 到达，then 选择手柄与菜单被清
- Given 终端页可见，when HOME→recents→回前台且 Surface 未销毁，then `surfaceCreated` 不黑、ON_RESUME 立即清暂停，`RECONFIGURE_SWAPCHAIN` 日志，录屏 0 黑帧（以填充后 APK 为基线）
- Given 宽度高度任一为 0 的过早 `surfaceCreated`，when 守卫命中，then 不创建 wgpu surface，不产生 `acquire failed` 黑帧

## Verification

- `adb shell dumpsys gfxinfo com.termux framestats` Total Missed 0（曾 74），`dumpsys gfxinfo` p95<11ms
- `adb logcat --pid=$PID | grep applyGridResize` 每 IME 往返恰 1 次（animating 0 次），`RECONFIGURE_SWAPCHAIN` 原地
- `screenrecord` 60fps 逐帧无压扁，`screencap` 1080x1326 vs 1080x2209 尺寸切换仅在 settled 时发生
