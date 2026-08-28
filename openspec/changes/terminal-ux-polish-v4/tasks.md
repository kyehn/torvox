# Tasks: terminal-ux-polish-v4

## Phase A — CJK 保真与极速 (Rust)

- [ ] A1 `cjk.rs` 将 `CJK_SERIF_PENALTY` 4→32，`cjk_family_priority` 的 locale 匹配改为 token 边界 `split(|c: char| !isAlphanumeric).any(|t| t==locale)`；新增 `token_match` 辅助 + 单测 `locale_token_boundary`
- [ ] A2 `cjk.rs` `scan_fallback_candidates` 的 `is_vector` 单字符探针改为 3 字符多数投票；`glyph_source_is_outline` 增加 `outline_cache: LruCache<(ID,u16),bool>` 10k，命中路径校验
- [ ] A3 `pipeline.rs` 重写 `cjk_fallback_names()` 为保序去重（按 `cjk_fallback_ids` 顺序，Seen HashSet），归一化仅对 `contains("cjk") && !contains("serif")` 合并；新增 `cjk_fallback_names_sorted()` 排序版；`font_information()/font_info()` 调用保序版
- [ ] A4 `glyph_cache.rs` 新增 `outline_cache` 字段、`clear()` 同步清理、`new()` 初始化 10k
- [ ] A5 `shaping.rs` 确保仅对 CJK ranges 加 fallback families（已做），补充 `cjk_locale_selects_correct_variant` 5→8 locale 用例；`cargo test --workspace` 997 断言

## Phase B — IME 零重排动画 (Kotlin)

- [ ] B1 `TerminalScreen.kt` 回归 hybrid：`LaunchedEffect` 16ms 采样环持有 `WindowInsets.ime.getBottom`，`IME_SETTLE_FRAMES=3`；animating= `Modifier.offset { IntOffset(0, -insetPx) }`（placement 阶段），settled= `padding(bottom=settled)`；`navigationBarsPadding` 仅外层 Box
- [ ] B2 `TerminalScreen.kt` 新增 `onImeSettled` 回调路径，settled 时单次调用 `surfaceRef.onImeSettled(settledBottom)`； animating 期 0 次 `applyGridResize` 断言（log 审计）
- [ ] B3 `TerminalSurface.kt` `onApplyWindowInsets` 去掉直接 `applyGridResize` 防抖，仅记录 `lastImeBottom` + 清 selection；新增 `onImeSettled(settledBottom)` → `resizeManager.applyGridResize(width,height,settled)`
- [ ] B4 `resizeManager.applyGridResize` 修正公式 `availableHeight = height - imeBottom`（不再减 `modifierBarHeightPx`），`recomputeRowsColsImmediate` 兼容 `cellWidth==0` stub
- [ ] B5 `surfaceCreated/surfaceChanged` 0-size 守卫 + `reconfigure_swapchain` 优先日志 `RECONFIGURE_SWAPCHAIN`，失败回退 `RECREATE`

## Phase C — 切应用无闪 (Kotlin + Rust)

- [ ] C1 `TerminalSurface.surfaceCreated` 0-size 守卫 `if (width<=0||height<=0) return`
- [ ] C2 `TerminalScreen` `ON_RESUME` 双路径：`if (surface.isAttached && width>0) { setRenderPaused(false); resumeRendering() } else postDelayedUnpause(200)`
- [ ] C3 `native/src/render/context.rs` / `ffi.rs` 审计 `attachSurface` 原地 `reconfigure` 优先，`ERROR_NATIVE_WINDOW_IN_USE_KHR` 才 `release+attach`，日志可审计

## Phase D — 滚动方向与流畅 (Kotlin)

- [ ] D1 `TerminalSurface.gestureListener.onScroll` 的 `scrollAccumulatorPx` 从 `toInt(trunc)` 改 `floor(accum/cellH).toInt()` 对称；增加注释对照 Termux `TerminalView:170-187`；补充 `scrollDirectionParity` 单测
- [ ] D2 `onFling` 保持 `-velocityY` 与 drag 同向，增加 `coerceIn` 与 `postOnAnimation` vsync 步长校验，`isScrolling` 状态正确翻转
- [ ] D3 `doFlingStep` 的 `computeScrollOffset` 后 `requestRender()` 节流至 vsync（16ms 阈值），`currentScrollbackLength()` 10Hz 缓存已有验证

## Phase E — 依赖、文档与验证

- [ ] E1 `cargo update` / `nix develop -c cargo update -p wgpu -p fontdb -p swash -p cosmic-text` 小版升级；`flake.nix` `fenix stable` 校验；`android/gradle/libs.versions` compose BOM / gradle / kotlin 审计
- [ ] E2 文档：`docs/plans/2026-08-28-terminal-ux-polish-v4-detailed.md`（含 26 项目矩阵、根因、方案、时间线）、`docs/verification/2026-08-28-terminal-ux-polish-v4-verification.md`（证据模板）、刷新 `docs/specification/design.md`
- [ ] E3 自动化：`scripts/build-apk.nu` + `scripts/setup-emulator.nu` + `maestro/ flows` 回归；`dumpsys gfxinfo framestats` 90fps+ 门禁；`cargo test --workspace` / `spotlessCheck` / `detekt`

## Acceptance Criteria

- [ ] `cjk_fallback_names()[0]` == 实际 `FALLBACK_HIT` 首项日志（Sans 胜 Serif，-15 vs -47，locale token 边界 misc 不命中）
- [ ] 中文 14 字首屏 <400ms（首屏暖机），二次命中 <16ms（outline 缓存），无宋体（screencap + 像素采样）
- [ ] IME 0→320dp 动画期 0 次 `applyGridResize`，settled 后单次，无跳跃闪烁文字压扁（screenrecord 逐帧 + Missed frame 0）
- [ ] HOME 往返 3 次零黑帧（`surfaceCreated` 0 守卫命中，`RECONFIGURE_SWAPCHAIN` 日志，原地重配）
- [ ] 上下滑动方向与 Termux 一致（drag up→older）、慢速阈值对称、fling vsync 平滑（`ScrollBehaviorQuantifiedTest` 间隔 16.6ms±3ms）
- [ ] 后端 `cargo test --workspace` 997 pass（或等价最新数）、`cargo clippy --deny warnings` 0、`gradle spotlessCheck detekt` PASS
- [ ] 4 轮双审（Standards + Spec）连续 PASS
