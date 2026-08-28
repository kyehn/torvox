# Proposal: terminal-ux-polish-v4 — 4 大用户感知缺陷像素级闭环

## Why

当前 `main` (808e794) 在模拟器与真机均可复现 4 项 P0 用户感知缺陷，且与 2026-08-27 声称已闭环的 `full-closure-v3` 描述不一致（代码未落地或回退）：

| # | 现象 | 复现路径 | 影响 | 2026-08-27 声称 | 当前代码实际 |
|---|------|----------|------|-----------------|--------------|
| 1 | 设置页显示 `Noto Sans CJK` 实际渲染宋体（Serif）且模糊、首屏极慢 | 任一中文字符首次渲染，`CJK_FALLBACK` 日志与 `font_information()` 矛盾 | 可读性 / 首帧 jank >300ms | 保序去重 + Serif 惩罚 + 多字符投票 + outline LRU | 仍 `sort()+dedup()` 字母序 + 惩罚 4（不足 32）+ 单字符 `test_chars[0]` 无缓存 |
| 2 | 输入法弹出/隐藏时 “终端页面” 动画跳跃闪烁、文字被压扁 | 键盘 0→320dp 动画期间，Composition 连续帧 | 视觉断裂 / 掉帧 | hybrid `offset` 放置阶段零重组 + 3 帧 settle 单次 `reconfigure_swapchain` | 直接 `animateDpAsState spring + padding` 每帧触发布局 + 32ms 防抖仍每分段触发 `applyGridResize` → 逐帧重排 + 缩放拉伸 |
| 3 | 切换应用（HOME→最近任务→回前台）终端页闪黑/白屏 200ms+ | Surface 保留未 `surfaceDestroyed` 的往返 | 可用性 P0 | 0-size 守卫 + ON_RESUME `setRenderPaused(false)+resume` + `reconfigure_swapchain` 原地 | `surfaceCreated` 无 0 守卫、ON_RESUME 仅 `postDelayed 200ms`、可能走 `release+attach` 重建 |
| 4 | 上下滑动方向错误、不流畅 | 单指拖动与 fling，回弹卡顿 | 可用性 | accumulator + OverScroller vsync | 方向注释正确但 `fling(-velocityY)` 与 drag 符号存在镜像歧义；`toInt()` 截断导致慢速阈值抖动；无 `floor` 对称 |

用户要求：**深入研究 26 项目像素级优点、产出完整文档与 OpenSpec、自动化验证、模拟器 90fps+、后端确定性 997 pass、前后双审四轮连续 PASS、更新所有依赖、优化所有代码与文档**。

## What Changes

### 1. `cjk-font-fidelity` — 字体保真 + 极速 (P0)

- `pipeline.rs:cjk_fallback_names()` 去掉 `sort()`, 改为按 `cjk_fallback_ids` 优先级保序去重，`cjk_fallback_names_sorted()` 另提供排序版；归一化 `Noto Sans CJK` 仅当 `contains("cjk")` 且 `!contains("serif")` 时合并，防止 Serif 被误归一。
- `cjk.rs:CJK_SERIF_PENALTY` 4→32，保证 Sans 任何位图/矢量组合下必胜 Serif（证明见 design.md）；`cjk_family_priority` 的 locale 匹配改为 token 边界 `split(!isAlphanumeric).any(|t| t==locale_tag)`，避免 `misc` 误匹配 `sc`。
- `scan_fallback_candidates` 的 `is_vector` 单字符探针 → 3 字符多数投票（`['中','日','가']` 各自探针，向量命中数 > 位图命中数则判向量）；`glyph_source_is_outline` 增加 `outline_cache: LruCache<(ID,GlyphId), bool>` 10k，命中 0.2µs，首屏 200 字从 400 次 scaler build 降至 ~3 次。
- `font_db.rs` / `SystemFonts.kt` 对齐：`list_monospace_fonts` 过滤 CJK、系统字体扫描复用 `EXTRA_FONT_PATHS`，设置页首项 == 渲染首项可通过 `log FALLBACK_HIT` 一致性断言。

### 2. `ime-smooth-follow` — 零重排动画 (P0)

- `TerminalScreen.kt`: 回归 spec 的 hybrid。`WindowInsets.ime.getBottom` 由 `LaunchedEffect` 16ms 采样环持有，`IME_SETTLE_FRAMES=3` 连续不变才进入 settled；animating 相用 `Modifier.offset { IntOffset(0, -insetPx) }` 放置阶段读 inset（零重组），settled 相切换为 `padding(bottom = settled)` 并单次回调 `surface.onImeSettled(settledBottom)`。
- `navigationBarsPadding` 仅在外层 `Box` 出现一次，`ModifierBarOverlay` 不再重复。
- `TerminalSurface.kt`: `onApplyWindowInsets` 仅记录 `lastImeBottom` + 清选择，不再直接 `applyGridResize`；`onImeSettled` 单次调用 `resizeManager.applyGridResize(width, height, settledBottom)`；`applyGridResize` 修正公式：`availableHeight = height - imeBottom`（height 已是 Compose padding 后的可用高度，不再减 `modifierBarHeightPx` 避免双减）。
- `surfaceCreated/surfaceChanged`: 0-size 守卫，`reconfigure_swapchain` 分支优先，失败回退 `release+attach`。

### 3. `app-switch-continuity` — 切应用无闪 (P0)

- `TerminalSurface.surfaceCreated`: `if (width<=0||height<=0) return` 守卫，防止 0×0 `ANativeWindow` 绑定黑帧。
- `TerminalScreen` `ON_RESUME`: 若 `surfaceRef.isAttachedToWindow && surface.width>0` 立即 `setRenderPaused(false); resumeRendering()`，仅当 surface 尚未就绪时 `postDelayedUnpause(200)`。
- `native/src/render/context.rs` / `ffi.rs`: `attachSurface` 优先 `reconfigure_swapchain` 原地，`ERROR_NATIVE_WINDOW_IN_USE_KHR` 时才 `release + attach`，日志 `RECONFIGURE_SWAPCHAIN` vs `RECREATE`.

### 4. `scroll-physics` — 方向与流畅 (P0)

- 方向：核对 Termux `TerminalView.java:170-187` `distanceY >0`→`mTopRow--`（更负=更老）与 torvox `scrollOffset +` 一致性，保留但增加对照注释与单测 `scrollDirectionParity`；`fling` 的 `-velocityY` 保持与 drag 同向（finger DOWN→newest），补充 `absoluteAdapter` 避免符号翻转。
- 流畅：`scrollAccumulatorPx` 从 `Float + toInt(trunc)` 改为 `floorDiv` 对称（正 `floor(0.9)=0` 负 `floor(-0.9)=-1` 需用 `kotlin.math.floor(accum/ch).toInt()` 保证慢速对称阈值）；`cellHeight` 用 `coerceAtLeast(1f)` 防 0 除；`OverScroller` 每帧 `postOnAnimation` 步长后 `requestRender()` 节流至 vsync（16ms）；`currentScrollbackLength()` throttling 10Hz 已有，补充 `scrollState` 的 `isScrolling` 去抖。

### 5. 依赖与文档 (P1)

- `cargo update`: `wgpu 30.x → 最新 30.x patch`, `fontdb 0.23→0.23.x`, `swash 0.2`, `cosmic-text 0.19→0.19.x`, `jni 0.21` 保持；`flake.nix` `fenix` stable 保持，`gradle 8.13 / kotlin 2.1 / compose BOM 2026.08.00` 审计最新小版。
- 文档：新增 `docs/plans/2026-08-28-terminal-ux-polish-v4-detailed.md`（本变更的完整实施计划，含 26 项目对标矩阵）、`docs/verification/2026-08-28-terminal-ux-polish-v4-verification.md`（自动化证据模板）、刷新 `docs/specification/design.md` 的禁止实现与字体/IME 章节。

## Capabilities

### New Capabilities

- `cjk-font-fidelity` — 设置首项 == 渲染首项、中文 14 字首屏 <400ms、二次 <16ms、无宋体。
- `ime-smooth-follow` — IME 动画期 0 次 `applyGridResize`、零文字压扁、Missed frame 0。
- `app-switch-continuity` — HOME 往返 3 次零黑帧、零闪白。
- `scroll-physics` — 滚动方向与 Termux 一致、慢速拖动阈值对称、fling vsync 平滑 60/90fps。

### Modified Capabilities

- `font-fallback` (`cjk.rs`/`pipeline.rs`/`glyph_cache.rs`) — 保序、惩罚 32、token 边界、outline 缓存、多数投票。
- `ime-animation` (`TerminalScreen.kt`/`TerminalSurface.kt`) — 48ms settle、offset 放置阶段、单次重排。
- `surface-lifecycle` (`TerminalSurface.kt`/`context.rs`) — 0-size 守卫、双清暂停、原地重配。
- `gesture-scroll` (`TerminalSurface.kt`) — 符号对照注释、floor 累加、vsync 节流。

## Impact

- **用户感知**：4 项 P0 全部闭环，可在无真机网络的模拟器上 90fps+ 验证（`dumpsys gfxinfo framestats` + `screenrecord` 逐帧）。
- **兼容性**：无破坏性 API；`font_info` JSON 字段 `font_size_px`→`font_size` 已在 808e794 改过，本变更保持。
- **风险**：`reconfigure_swapchain` 在部分模拟器驱动不支持时回退路径已保留；Serf 罚 32 若未来出现更高优先级的 CJK 字体可通过 `cjk_family_priority` 单测调整；IME 3 帧 settle 若过短导致过早重排可调至 4 帧。
- **验证**：`nix develop -c cargo test --workspace` 997 pass、`cargo clippy --deny warnings`、`gradle spotlessCheck detekt`、`adb` 4 场景录屏 + 日志审计（见 tasks.md）。
