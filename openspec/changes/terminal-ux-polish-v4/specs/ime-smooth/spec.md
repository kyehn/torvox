# Spec: ime-smooth + app-switch-continuity

## Requirement

IME 弹出/收起期间终端内容跟随键盘无跳跃、无闪烁、无文字压扁；切应用往返无黑屏；均可在模拟器像素级验证。

## Design — hybrid pan-then-reflow + lifecycle guards

| Phase | Trigger | UI | Grid / Surface |
|-------|---------|----|----------------|
| Animating | `imeInset != settled` | `Modifier.offset { IntOffset(0, -insetPx) }` placement-phase 读 inset，零重组 | 不变，0 次 `applyGridResize` |
| Settled | `inset` 连续 `IME_SETTLE_FRAMES=3` 帧不变（16ms 采样） | `padding(bottom=settled)` 单次布局 | 单次 `onImeSettled(settled)` → `applyGridResize` → `reconfigure_swapchain` 原地 |

- `navigationBarsPadding` 仅外层 `Box`。
- `onApplyWindowInsets` 仅 `lastImeBottom = inset + clearSelection`，不重排。
- `surfaceCreated` 0-size 守卫 `if (width<=0||height<=0) return`。
- `ON_RESUME`：`if (surface.isAttached && width>0) { setRenderPaused(false); resumeRendering() } else postDelayedUnpause(200)`。
- Native：`attachSurface` 优先 `reconfigure_swapchain`，仅 `ERROR_NATIVE_WINDOW_IN_USE_KHR` 回退 `release+attach`。

## Scenarios

- Given 任一会话，when IME show 0→320dp 动画期，then `rows/cols` 不变、0 次 `applyGridResize` 日志、pan 逐帧跟随；3 帧稳定后 padding 生效、单次 `applyGridResize`、prompt 在键盘上方可见（短会话 top 行与长会话 bottom 行均满足）。
- Given 文本被选中，when 任意 IME inset 到达，then 选择手柄与菜单被清（`onApplyWindowInsets` 侧清），不出现滞后错位弹窗。
- Given 终端页可见，when HOME→recents→回前台且 Surface 未销毁，then `surfaceCreated` 不黑、ON_RESUME 立即清暂停，`RECONFIGURE_SWAPCHAIN` 日志，录屏 0 黑帧。
- Given 旋转/折叠等真 `surfaceDestroyed+Created`，when 新 surface 尺寸有效，then `attachSurface` 原地或重建，3 次往返零黑屏。
- Given 宽度高度任一为 0 的过早 `surfaceCreated`（layout 前），when 守卫命中，then 不创建 wgpu surface，不产生 `acquire failed` 黑帧。

## Verification

- `KeyboardJellyInstrumentedTest` Missed frame 0 (was 74)，`dumpsys gfxinfo framestats` p95<11ms，`screenrecord` 逐帧对比无压扁。
- `adb logcat` 每 IME 往返 `applyGridResize` 恰好 1 次、`RECONFIGURE_SWAPCHAIN` 原地、`0x0 early attach` 0 次。
- HOME 往返 3 次 `screencap` 均见 `$` prompt，`render_paused` 已清。
