# 终端体验抛光 v4 — 详细实施计划 (2026-08-28)

> **Scope**: 4 大 P0 用户感知缺陷的像素级闭环 + 依赖/文档全面优化。方法：research → spec → plan → implement → verify → 4× 双审（Standards + Spec）。对标 26 项目，模拟器 90fps+ 可验，后端确定性，前端像素级。

## 1. 背景与目标

### 1.1 用户原诉（2026-08-28）

- **CJK**：设置页显示 `Noto Sans CJK`（无衬线黑体）实际渲染为宋体（衬线）且模糊，渲染极慢（首屏 jank >300ms）。
- **IME**：输入法弹出/隐藏时“终端页面”动画跳跃、闪烁、文字被横向压扁。
- **切应用**：HOME→最近任务→回前台 终端页闪黑/白 200ms+。
- **滚动**：上下滑动方向错误、不流畅（慢速阈值卡、fling 抖）。

### 1.2 历史声称 vs 实地

`full-closure-v3` 文档声称四项已闭环，但代码审计发现关键修复未落地或回退（见 proposal.md 表）。本次以**地面真值**（代码、日志、录屏）为准重做。

### 1.3 目标（可量化）

| 指标 | 验收值 | 测量 |
|------|--------|------|
| CJK 首屏 14 字 | <400ms，二次 <16ms | log `FALLBACK_HIT` 时间差 + `Choreographer` |
| CJK 无宋体 | 设置首项 == 日志首项，screencap 像素采样直线无衬线 | `font_information()` vs `FALLBACK_HIT Noto Sans CJK` |
| IME Missed frame | 0（曾 74） | `adb logcat Choreographer Missed` |
| IME 无压扁 | `applyGridResize` 每往返恰 1 次，cell.wgsl UV 不拉伸 | log 审计 + 录屏逐帧 |
| 切应用黑帧 | 0/3 次 | `screenrecord` + `RECONFIGURE_SWAPCHAIN` 日志 |
| 滚动方向 | 与 Termux 一致 | 单测 + 手指上滑露历史 |
| 滚动流畅 | p95<11ms, janky<5% | `dumpsys gfxinfo framestats` 300 帧 |
| 后端 | `cargo test --workspace` 997+ pass | CI |
| 前端 | `spotlessCheck detekt` PASS | CI |

## 2. 26 项目像素级对标（提炼自 docs/reference 44 文件与 00-TORVOX-BASELINE）

| 维度 | 对标项目 | 抄得技术 | 落点 |
|------|----------|----------|------|
| 字体回退 | ghostty-android 4-slot TerminalFontStore、CellRun | 独立 bold/italic 槽、字体族精确匹配 | `pipeline.rs set_font_family_for_style` 已有，CJK 需同链路 |
| 字体回退 | moke Nerd 层分桶、Symbol/Emoji 分层 | CJK→Symbol→Nerd→Emoji→db scan 链路 | `cjk.rs` 四层已实现，本次补 token/惩罚/缓存 |
| 字体回退 | warp ASystemFontIterator / Shaping::Basic for CJK | 系统字体发现、shape 分段仅 CJK ranges | `shaping.rs` 已改分段，本次补 locale token |
| 字体回退 | shashlik wgpu is_emulator 双后端 | 字体光栅化在模拟器 GL 回退时不模糊 | `rasterization.rs` hint 保持 |
| 选择/滚动 | termux-app `TerminalView:170-187` 手指上滑→`mTopRow--` | 拖动方向 `offset+=` 同向对照 | `TerminalSurface:1781-1794` 注释显式对照 |
| 选择/滚动 | ghostty-android tapCount、边缘滚动、selectionGeometryKey | 多击不依赖 GestureDetector、边缘 30ms 自循环 | 已有，本次不改 |
| IME | termux-app `adjustNothing + WindowInsetsAnimation.Callback` + Haven `imePadding` | `offset` 放置阶段零重组 + settled 单次重排 | `TerminalScreen.kt` 本次回归 |
| IME | cpmdroid 屏内滚动不缩字号 | IME 时文字不缩放，仅平移 | `offset` 保证 |
| lifecycle | zelland/wgpu `jni_fn + acquire重试 + reconfigure_swapchain` | 原地重配 vs `ERROR_NATIVE_WINDOW_IN_USE_KHR` 重建 | `context.rs` 保留回退 |
| lifecycle | ghostty-android `hide(0)` 同步、`scrollShift GPU blit` | 窗口重建不闪白 | 同上 |
| 渲染 | zelland 行级脏缓存、DirtyBand、CachedInstances | 增量渲染保 90fps+ | `cell_builder.rs` 已有 |

**不抄**：termux `Canvas.drawText` 每格（AGENTS.md 禁止）、portable-pty、Java 文件、JNA。

## 3. 根因深度分析（First Principles + Think-in-Code + 代码证据）

### 3.1 CJK 宋体/模糊/极慢（P0）

#### 3.1.1 显示与实际不一致

- **证据**：`pipeline.rs:cjk_fallback_names()` `raw_names.sort()+dedup()` 字母序；`cjk_fallback_ids` 按 `effective_priority` 排序（Sans 5 vs Serif 1）。设置页读前者，渲染取后者首项。巧合时 Sans < Serif 字母序藏 bug，但 Source Han Serif 等未被惩罚时 Tie 翻转即现宋体。
- **深因**：`sort` 破坏优先级；归一化 `Noto Sans CJK` 把 Serif 与 Sans 合并为同一显示名，无法区分。
- **修复**：去 `sort`，保序 `HashSet` 去重，按 `ids` 顺序；归一化仅对 `cjk && !serif` 合并，Serif 保持独立；新增 `cjk_fallback_names_sorted()` 供需排序的单测/对比。

#### 3.1.2 Serif 仍胜 Sans

- **证据**：`CJK_SERIF_PENALTY=4` 时 Sans 位图 worst -15 vs Serif 矢量 best -17? 计算：Sans 5-20=-15，Serif (5-4)=1+10=11 → Serif 胜 26。用户 “misc symbols” 等含 `sc` 子串误夺 localeBoost 6。
- **深因**：罚 4 只在同为位图时够（差 4），跨位图/矢量不够；`contains(locale_tag)` 把 `misc` 判为 `sc`。
- **修复**：罚 32（证明见 design.md），token 边界 `split(!alnum).any(|t| t==tag)`。

#### 3.1.3 极慢

- **证据**：`scan_fallback_candidates` 每候选字体对 `test_chars[0]='中'` 单探针建 `scaler_context.builder().size().hint(true).build() + Render`；100 候选×1 =100 builds；`glyph_source_is_outline` 每字符每回退字体再建一次，首屏 200 字×2 回退×1 =400 builds。无缓存，`shape_run` 曾整段 `0..len` 加 CJK fallback 族成本高。
- **修复**：3 字符多数投票（各探针走 `outline_cache`），命中后 0.2µs；`outline_cache LRU 10k`；`shape_run` 已改仅 CJK ranges 加 spans（本次保留）。

#### 3.1.4 模糊

- 候选：混合位图/矢量误判导致位图字体被当矢量光栅化放大→模糊；或 `hint(true)` 在小字号下子像素不对齐。
- 修复后：多数投票正确分类位图字体被罚 20，矢量胜；atlas 采用 `Mask/SubpixelMask` 渲染，`cellHeight.ceil()` 保证行高整数，wgsl 采样 `linear` 但 glyph 已像素对齐。

### 3.2 IME 跳跃闪烁（P0）

#### 3.2.1 旧 pipeline

- `TerminalScreen.kt` `animateDpAsState spring + padding(bottom)` 每帧触发布局 `measure`；`TerminalSurface.kt` `onApplyWindowInsets` 32ms 防抖仍每段触发 `applyGridResize → recomputeGrid → attachSurface` → PTY resize + swapchain 重配 → 文字被非等比缩放（旧 buffer 拉伸）。
- `navigationBarsPadding` 在外层 `Box` 与 `ModifierBarOverlay Box` 双算，`availableHeight = height - ime - bar(80dp)` 双减使结算网格过小 1×键盘高度。

#### 3.2.2 跳跃/闪烁机制

- Pan 期逐帧重排：每帧 `rows` 变 → `cellHeight` 变 → GPU 需重建实例 → 41ms 帧（SwiftShader）→ 卡顿。
- Settled 期突变：96ms(6帧) pan 后突然 padding→网格突变，视觉跳跃；`attachWindow` 重建 vs `reconfigure_swapchain` 原地差异导致闪白/压扁（`ERROR_NATIVE_WINDOW_IN_USE_KHR`）。

#### 3.2.3 本次

- 回归 spec hybrid：Compose `offset { IntOffset(0,-animatedPx) }` 放置阶段读 inset，零 `measure`，`height` 不变→`onSizeChanged` 不触发；`onApplyWindowInsets` 仅记录 `lastImeBottom`，`IME_RESIZE_DEBOUNCE 48ms` 单次 `applyGridResize(height-ime)`（减 bar 去掉）→ `reconfigure_swapchain` 原地。

### 3.3 切应用闪黑（P0）

- **0×0 early attach**：`surfaceCreated` 在 `width/height=0` 时即 `attachSurface` → wgpu `acquire` 失败黑帧。
- **render_paused 未清**：`onSurfaceDestroyed` 置 `true` 仅在 `surfaceCreated` 重建路径清；HOME→recents 保留 Surface 时 `ON_RESUME` 未清→短路。
- **修复**：`surfaceCreated/surfaceChanged` 0-size 守卫；`ON_RESUME` 双路径：`isAttached && width>0` 立即 `setRenderPaused(false)+resumeRendering()`，否则 `postDelayed 200`。

### 3.4 滚动方向错误/不流畅（P0）

#### 3.4.1 方向

- Android `GestureDetector.onScroll distanceY = previousY - currentY`，Termux `mTopRow -= distanceY/cellH`（上滑>0→更小 Top→更老）。torvox `offset += lines` 等价（offset=-Top），注释已正确，但 `fling(-velocityY)` 符号需与 drag 同向验证（finger DOWN 正 velocity→ offset 减小→更新）。
- 用户感知的“反向”可能源于对“终端历史在上”的心智 vs 列表历史在下的差异；保留 Termux  parity 但补单测与注释。

#### 3.4.2 卡顿

- `scrollAccumulatorPx / ch).toInt()` 对正 0.9→0 但负 -0.9→0 不对称，慢速负向永远不滚；改为 `floor` 对称。
- `OverScroller` 每帧 `postOnAnimation` 已有，但 `computeScrollOffset` 后未 `forceRender` 节流；补 vsync 阈值。

## 4. 实施计划（依赖顺序，增量可独立验证）

| 阶段 | 内容 | 文件 | 验证 |
|------|------|------|------|
| A1 | `CJK_SERIF_PENALTY 32` + `locale_token_match` + 单测 3→5 | `cjk.rs` | `cjk_priority_tests 5→8` |
| A2 | `outline_cache 10k` + 3 字符多数投票 | `cjk.rs`, `glyph_cache.rs` | log `FALLBACK_CANDIDATE` 仍 -15 vs -47 但 Serif 必败 |
| A3 | `cjk_fallback_names` 保序去重 + `sorted()` | `pipeline.rs` | `font_information()` 首项==渲染首项 |
| A4 | 模拟器 cat 中文 14 字首屏 <400ms | `shaping.rs` | `screencap` 无宋体 |
| B1 | `TerminalScreen` offset 化 + 去双 navBars + 48ms | `TerminalScreen.kt` | Missed 0 |
| B2 | `TerminalSurface` applyGridResize 去 bar + 0 守卫 | `TerminalSurface.kt` | `applyGridResize` 每往返 1 次 |
| C1 | `ON_RESUME` 双清 + `reconfigure` 优先 | `TerminalScreen.kt`, `context.rs` | 3 次往返 0 黑 |
| D1 | `floor` 累加 + vsync | `TerminalSurface.kt` | `ScrollBehaviorQuantifiedTest` |
| E | `cargo update` + `flake check` + 文档 | `Cargo.lock`, `flake.nix`, `gradle` | `clippy`/`spotlessCheck` |

## 5. 测试计划（双腿：后端确定性 + 前端像素级）

### 5.1 后端确定性

- `nix develop -c cargo test --workspace` 997+ pass（`cjk_priority_tests 8`, `is_cjk_candidate` 类, `CellInstance` 不变量）
- `cargo clippy --all -- --deny warnings` 0，`cargo fmt --check`
- `outline_cache` 命中率：首屏后二次 hit >99%（log 采样）

### 5.2 前端像素级（模拟器 Pixel 9 API35 1080×2400@420dpi SwiftShader）

- `CursorPixelAcceptanceTest` CJK 无宋体（像素采样色差 + 日志一致）
- `ScrollBehaviorQuantifiedTest` floor 对称（上/下 各 1 cell 阈值）
- `KeyboardJellyInstrumentedTest` Missed frame 0
- `EchoGridDumpTest` + `screenrecord` 60fps 逐帧：IME 期间文字不压扁、切应用 0 黑
- `dumpsys gfxinfo framestats` 300 帧 p50≤7ms p95≤11ms janky<5%（`Mailbox` present_mode）
- Maestro `flows/launch.yaml, font-change-rerender.yml, resize.yaml` 回归

### 5.3 日志审计

```bash
adb logcat -v threadtime --pid=$PID | grep -E "FALLBACK|RECONFIGURE|applyGridResize|render_paused|SCROLL"
# 断言：FALLBACK_CANDIDATE sans -15 vs serif -47, FALLBACK_HIT Noto Sans CJK JP, RECONFIGURE_SWAPCHAIN 原地, applyGridResize 1/IME往返, render_paused cleared on RESUME
```

### 5.4 性能（Hz 与 fps 解耦）

- Rust 渲染线程 `loop timing 6ms ≈166fps`（Choreographer log），`wgpu present_mode Mailbox` Hz 解耦，真机 120Hz 预留 `setFrameRate`.
- `shape_run` 分段后 Latin 不经 CJK fallback，`outline_cache` 后首屏 22ms→二次 <16ms。

## 6. 验收标准（与 tasks.md 一致，量化）

- 设置首项 == 渲染首项（`cjk_fallback_names()[0]` 文本 == `FALLBACK_HIT` 家族）
- `misc symbols` 不获 `sc` 加分（`locale_token_boundary_misc_not_sc` pass）
- 中文 14 字首屏 <400ms、二次 <16ms，无宋体衬线（screencap + 色直方图）
- IME 0→320dp 动画 0 次 `applyGridResize` 期间、settled 单次，无跳跃/压扁（Missed 0）
- HOME 往返 3 次 0 黑帧（`surfaceCreated` 0 守卫命中、ON_RESUME 即时清暂停）
- 滚动上滑露历史、下滑回底，慢速 0.4*cellH×3 帧阈值对称，fling vsync 16.6ms±3ms
- 后端 997 pass、`clippy` 0、`spotlessCheck detekt` PASS、4 轮双审连续 PASS

## 7. 风险与回滚

| 风险 | 缓解 | 回滚 |
|------|------|------|
| `reconfigure_swapchain` 驱动不支持 | 保留 `release+attach` 回退+`RECREATE` 日志 | `git revert` 单文件 |
| IME 3 帧过短过早重排 | 可调 4 帧 64ms 仍 <100ms 感知阈 | 改常量 |
| Serif 罚 32 过大误杀新 Sans | `KNOWN_FAMILY` 可扩展更高优先级分支 | 单测调参 |
| `floor` 改变积累语义 | 单测覆盖正负 | 切回 `toInt` |

## 8. 时间与分支

- 分支：`main` 直接增量（风险低，改动面 <6 文件）
- 预计：编码 1 轮、自动化 1 轮、双审 4 轮（含修复）合计 <1 天内可达 4×PASS。

## 9. 26 项目二次精读清单（已读 44→精读 6）

- `research-termux-app.md` §2-3（选择）、`research-ghostty-android.md` §2（tapCount）、`research-warp.md` §9（composing diff）、`research-shashlik.md` §3（wgpu 双后端）、`research-haven.md` §2（smartCopy）、`research-zelland-wgpu.md` §5（DirtyBand）——精读笔记见 `docs/reference-projects.md` 1.1 增补。
