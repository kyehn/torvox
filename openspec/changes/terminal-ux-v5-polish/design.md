# Design: terminal-ux-v5-polish

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

构建门禁： build-android-libs.nu → jniLibs/libnative.so (readelf 无 ghostty NEEDED) → build-apk.nu → 90M APK含.so
日志审计： FALLBACK_CANDIDATE -15 vs -47, FALLBACK_HIT Noto Sans CJK, FONT_RASTERIZE_ASCII before/after
```

- **不改核心**：`CJK_SERIF_PENALTY=32` 证明（最坏 Sans -15 > 最好 Serif -17）、token 边界、多数投票、outline 缓存、保序均已正确，v5 仅补门禁与审计
- **验证**：`cargo test` 99%+ pass + `screencap` 无宋体 + `dumpsys` 首帧 <400ms

### 2) IME hybrid v5 (Kotlin: `TerminalScreen.kt` ↔ `TerminalSurface.kt` → Rust)

```
Compose WindowInsets.ime.getBottom(density) ──▶  placement-phase Modifier.offset { IntOffset(0, -getBottom()) }
       │ 动画期零组合：不经 animateDpAsState，不经 LaunchedEffect 驱动动画，仅放置阶段读 inset
       │ settled 判定：LaunchedEffect(rawImeBottomPx) { delay 48ms (16×3); settledImePx = raw; isImeSettled=true; surface.onImeSettled(raw) }
       ▼
TerminalSurface.onApplyWindowInsets: record lastImeBottom, dismiss selection, NO resize
TerminalSurface.onImeSettled: resizeManager.applyGridResize(width, height, settled)
       │ availableHeight = height - imeBottom  (height 已是 Compose padding 后的可见高度，不再减 bar)
       │ recompute cols/rows → runtime.resize → attachSurface(reconfigure_swapchain fast-path)
       ▼
Wgpu context: reconfigure_swapchain live; fallback release→attach on ERROR_NATIVE_WINDOW_IN_USE_KHR (保留)
```

- **v5 唯一改动**：`TerminalScreen.kt` 删除 `animateDpAsState` import 与 `animatedImeBottom/animatedImePx` 状态，`offset` lambda 内联 `WindowInsets.ime.getBottom(density)`，`remember(density)` 缓存计算
- **单 `navigationBarsPadding`**：仅外层 `Box`，`ModifierBarOverlay` 仅 `background + padding(bottom)`（v4 已正确，v5 保持）
- **验证**：`dumpsys gfxinfo` Missed 0，`logcat applyGridResize` 1/IME往返，`RECONFIGURE_SWAPCHAIN` 原地

### 3) 切应用 (Android lifecycle)

```
onPause → finishComposing + hideSoftInput
onResume:
  if (surface.isAttached && width>0) { setRenderPaused(false); resumeRendering() } // 即时
  else postDelayedUnpause(200) // surface 尚未就绪
surfaceCreated: guard width<=0||height<=0 return; attachSurface(reconfigure fast-path)
surfaceDestroyed: onSurfaceDestroyed() + releaseAllGpuSurfaces + lastConfigured 0 + frame_invalidated=true
```

- **产物为首因**：`jniLibs` 空时 `attachSurface` 仍空转，渲染黑帧非逻辑 bug。v5 以填充后 APK 为基线
- **验证**：HOME 往返 3 次 `screencap` 均见 `$` prompt，`logcat` 0 黑

### 4) 滚动 (Gesture)

```
onDown: reset accumulator, stopFling
onScroll distanceY: accumulator += distanceY; lines = floor(accum / cellH).toInt(); accum -= lines*cellH; offset = (offset+lines).coerceIn(0,len); forceRender() vsync
onFling velocityY: flingScroller.fling(0, offset, 0, -velocityY, 0,0,0,len); postOnAnimation(step)
step: curr = clamp(currY,0,len); if changed → offset=curr → onScrollChanged → requestRender()
```

- **保持 Termux parity**：`TerminalView:170-187` `mTopRow -= distanceY/cellH` 与 `offset +=` 同向
- **验证**：`ScrollBehaviorQuantifiedTest` 0 collapses，`seq 1 400` 上滑露历史

## Tests

- 后端：`cjk_priority_tests`、`classification_disjoint`、`font_dirs`、`glyph_cache`、`cargo test --workspace` 99%+ pass
- 前端：`KeyboardJellyInstrumentedTest` miss 0、`ScrollBehaviorQuantifiedTest` vsync、`CursorPixelAcceptanceTest` 无宋体、`EchoGridDumpTest` 逐帧
- 构建：`unzip -l *.apk | grep .so` 非空，`readelf` 无 ghostty NEEDED，`stat libnative.so` 15M-60M
- 日志：`FALLBACK_CANDIDATE -15 vs -47`、`FALLBACK_HIT Noto Sans CJK`、`RECONFIGURE_SWAPCHAIN` 原地、`applyGridResize 1/往返`

## Risks

- 去 spring 后阶梯：系统动画已插值，实测有阶梯可加 `derivedStateOf` 插值
- `reconfigure_swapchain` 不支持：保留 `release+attach` 回退
- 产物过大：`MAXIMUM_SO_SIZE_BYTES 60M` 门禁，release profile 强制
