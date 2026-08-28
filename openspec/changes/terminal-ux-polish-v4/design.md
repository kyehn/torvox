# Design: terminal-ux-polish-v4

## Architecture

### 1) CJK (Rust: `cjk.rs` → `pipeline.rs` → `glyph_cache.rs` → `shaping.rs` → Kotlin)

```
system locale ──▶ find_cjk_fallback_fonts (scan_fallback_candidates)
                     │ effective_priority = family_priority(locale_tag, serif_penalty=32)
                     │                  - bitmapPenalty(20) + outlineBonus(10)
                     │                 多数投票 is_vector(3 chars)
                     │ outline_cache LRU 10k
                     ▼
               cjk_fallback_ids[0..3] (priority ordered)
                     │ glyph_information() 逐字符 resolved via try_cjk_outline_fallback
                     │ → cjk_glyph_cache (char → (ID,gid)) → swash scaler once
                     ▼
               cjk_fallback_names() 保序去重归一 (Noto Sans CJK)
                     │ JNI font_info() → Kotlin Settings FontInfoCard
                     ▼
               shaping.rs: Buffer.AttrsList 仅对 CJK ranges 加 fallback families
```

- **Serif 惩罚证明**：设 `P_sans=5, P_serif=5-32=-27, penalty=20, bonus=10`。
  - 最坏 Sans (bitmap) = -15， 最好 Serif (vector) = -17 → Sans 仍胜 2。
  - 同为 vector: Sans 15 vs Serif -17 差 32。
  - 同为 bitmap: -15 vs -47 差 32。故任何组合 Sans 必胜。
- **Token 边界**：`family.split(|c: char| !c.is_alphanumeric()).any(|tok| tok==locale_tag)` 防 `misc→sc`。
- **Outline 多数投票**：对 `['中','日','가']` 分别探针，向量命中数>1 则判向量字体，避免混合位图/矢量误判。

### 2) IME hybrid (Kotlin: `TerminalScreen.kt` ↔ `TerminalSurface.kt` → Rust)

```
Compose WindowInsets.ime.getBottom ──▶ LaunchedEffect 16ms sample loop
       │ state: animated=pan / settled=padding
       │ animating: Modifier.offset { IntOffset(0,-inset) } // placement-phase, 0 recomposition
       │ settled (3 stable frames 48ms): padding(bottom=settled) + surface.onImeSettled(settled)
       ▼
TerminalSurface.onApplyWindowInsets: record lastImeBottom, dismiss selection, NO resize
TerminalSurface.onImeSettled: resizeManager.applyGridResize(width, height, settled)
       │ availableHeight = height - imeBottom  (height 已含 Compose padding, 不再减 bar)
       │ recompute cols/rows → runtime.recomputeGrid → attachSurface(reconfigure_swapchain)
       ▼
Wgpu context: reconfigure_swapchain live; fallback release→attach on ERROR_NATIVE_WINDOW_IN_USE_KHR
```

- 单 `navigationBarsPadding` 在外层 `Box`，`ModifierBarOverlay` 仅 `background + padding(bottom)`。

### 3) 切应用 (Android lifecycle)

```
onPause → finishComposing + hideSoftInput
onResume:
  if (surface.isAttached && width>0) { setRenderPaused(false); resumeRendering() } // 即时
  else postDelayedUnpause(200) // surface 尚未就绪
surfaceCreated: guard width<=0||height<=0 return; attachSurface(reconfigure)
surfaceDestroyed: onSurfaceDestroyed() + releaseAllGpuSurfaces + lastConfigured 0
```

### 4) 滚动 (Gesture)

```
onDown: reset accumulator, stopFling
onScroll distanceY: accumulator += distanceY; lines = floor(accum / cellH); accum -= lines*cellH; offset = (offset+lines).coerceIn(0,len); forceRender() vsync
onFling velocityY: flingScroller.fling(0, offset, 0, -velocityY, 0,0,0,len); postOnAnimation(step)
step: curr = clamp(currY,0,len); if changed → offset=curr → onScrollChanged → requestRender()
```

- 符号对照：Termux `TerminalView.onScroll` 中 `mTopRow -= distanceY/cellH` (更小 Top = 更老)，torvox `offset +=` 等价 (`offset` 与 `-TopRow` 同向)。
- 流畅：floor 对称阈值，vsync 节流，10Hz scrollback 长度缓存。

## Tests

- 后端：`cjk_priority_tests` 5→8 用例（sans>serif×2、locale token 边界 misc 不匹配、投票、fallback 首项一致），`glyph_cache` outline 命中率，`cargo test --workspace 997`.
- 前端：`TerminalSurfaceLogicTest` 扩展方向/累加；Compose `TerminalScreenLayoutTest` 断言 padding vs offset 阶段；仪器化 `KeyboardJellyInstrumentedTest` missFrame 0, `CursorPixelAcceptanceTest` CJK 无宋体, `ScrollBehaviorQuantifiedTest` vsync 间隔。
- 性能：`dumpsys gfxinfo framestats` p95<11ms, `screenrecord` 逐帧无压扁, `Choreographer` avg<8ms.
- 日志：`FALLBACK_CANDIDATE -15 vs -47`, `FALLBACK_HIT Noto Sans CJK`, `RECONFIGURE_SWAPCHAIN`, `applyGridResize once per settle`, `SCROLL offset+lines floor`.

## Risks

- `reconfigure_swapchain` 不支持：回退 `release+attach`（保留日志 WARNING）。
- IME 3 帧过早：可调 4 帧 64ms 仍 <100ms 阈值。
- Serif 罚 32 过大：若出现 Noto Sans CJK 优先级应低于某新 Sans，增加更高优先级的 `KNOWN_FAMILY` 分支即可（单测覆盖）。
