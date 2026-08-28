# Spec: scroll-physics

## Requirement

终端 scrollback 的单指拖动与 fling 必须方向正确（与 Termux 一致）、慢速阈值对称、vsync 平滑 60fps 稳态 / 90fps+ 峰值。

## Design — parity + floor + vsync

- **方向对标**：Termux `TerminalView.java:170-187` 手指上滑 `distanceY>0` → `mTopRow--`（更负=更老历史）。torvox 等价 `scrollOffset += lines`（offset 增大=更老），注释显式对照；`fling` 的 `-velocityY` 与 drag 同向（手指下滑正 velocity → offset 减小→更新）。
- **累加对称**：`accum += distanceY; lines = floor(accum / cellH).toInt(); accum -= lines*cellH`。用 `kotlin.math.floor` 保证正慢速 `0.9→0` 与负慢速 `-0.9→-1` 对称阈值，避免 `toInt(trunc)` 的慢速负向卡顿。
- **节流**：`cellHeight.coerceAtLeast(1f)` 防 0 除；`currentScrollbackLength()` 10Hz 缓存；`isScrolling` 标志去抖；`doFlingStep` 每帧 `computeScrollOffset()` 后 `requestRender()` 以 vsync 节拍 `postOnAnimation`。

## Scenarios

- Given scrollback 已有一屏历史 `scrollOffset=0`（底部），when 手指上滑 1 cell 高度，then `scrollOffset==1`，视口上移一行露出历史行（older）。
- Given 手指下滑 1 cell，when 拖动，then `scrollOffset` 减小至 0 止，不负。
- Given 慢速拖动 `distanceY = 0.4*cellH` 连续 3 帧（累计 1.2），when floor 累加，then 第 3 帧才 `lines=1`，前两帧 0，无抖动；负向同阈值对称。
- Given `fling velocityY=+2000`（手指快速下滑），when `fling(-velocityY)`，then `currY` 向 0 方向惯性滚动，`OverScroller` 每帧 16ms vsync 步长，`dumpsys` 帧间隔 16.6ms±3ms。
- Given alternate screen 激活（vim/less），when 拖动，then 不滚本地 scrollback，转为 `encodeMouseEvent` wheel 上报（已实现）。

## Verification

- 单测 `scrollDirectionParity`（offset+ 与 TopRow-- 同向）、`accumulatorFloorSymmetry`（正负慢速阈值）、`flingSignConsistency`。
- 仪器化 `ScrollBehaviorQuantifiedTest` vsync 间隔 histogram p95<11ms，`Choreographer` avg<8ms。
- 真机/模拟器 `seq 1 200` 后上滑露旧 1-39，下滑回 0 见 `$`，无碎片。
