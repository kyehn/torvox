# Spec: scroll-physics (v5 — 证据化)

## Requirement

终端 scrollback 的单指拖动与 fling 必须方向正确（与 Termux 一致）、慢速阈值对称、vsync 平滑 60fps 稳态 / 90fps+ 峰值，以**可视化证据**而非主观判定。

## Design — 保持 v4，补证据

- **方向对标**：Termux `TerminalView.java:170-187` 手指上滑 `distanceY>0` → `mTopRow--`（更负=更老历史）。torvox 等价 `scrollOffset += floor(distanceY/cellH)`（`offset` 与 `-TopRow` 同向），注释显式对照；`fling -velocityY` 与 drag 同向
- **累加对称**：`accum += distanceY; lines = floor(accum / cellH).toInt(); accum -= lines*cellH`，`kotlin.math.floor` 保证正 `0.9→0` 与负 `-0.9→-1` 对称
- **节流**：`cellHeight.coerceAtLeast(1f)` 防 0 除；`currentScrollbackLength()` 10Hz 缓存；`isScrolling` 去抖；`doFlingStep` 每帧 `computeScrollOffset()` 后 `requestRender()` 以 vsync 节拍 `postOnAnimation`

## Scenarios

- Given scrollback 已有一屏历史 `scrollOffset=0`，when 手指上滑 1 cell 高度，then `scrollOffset==1`，视口上移一行露出历史行（older），以 `seq 1 400` 后上滑见旧 1-39 录屏为证
- Given 手指下滑 1 cell，when 拖动，then `scrollOffset` 减小至 0，不负
- Given 慢速拖动 `distanceY = 0.4*cellH` 连续 3 帧（累计 1.2），when floor 累加，then 第 3 帧才 `lines=1`，前两帧 0，无抖动；负向同阈值对称（单测覆盖）
- Given `fling velocityY=+2000`（手指快速下滑），when `fling(-velocityY)`，then `currY` 向 0 方向惯性滚动，`OverScroller` 每帧 16ms vsync 步长
- Given alternate screen 激活（vim/less），when 拖动，then 不滚本地 scrollback，转为 `encodeMouseEvent` wheel 上报

## Verification

- 单测：方向与累加对称（`TerminalSurfaceLogicTest` 扩展）
- 仪器化：`ScrollBehaviorQuantifiedTest` `enter_snaps_viewport_to_bottom ≤2000ms`，`pty_flood_never_resets_viewport_mid_gesture` 0 collapses，maxOffset>0
- 手势录屏：`seq 1 400` 后上滑露旧 1-39，下滑回 0 见 `$`，无碎片
- 帧时：`dumpsys gfxinfo framestats` 300 帧 p50≤7ms p95≤11ms janky<5%（`Mailbox`）
