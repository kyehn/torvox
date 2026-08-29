# 终端体验深度诊断与抛光 v5 — 详细实施计划 (2026-08-29)

> **Scope**: 针对用户 2026-08-29 报告的 4 大 P0 缺陷（CJK 字体错乱/模糊/极慢、IME 动画跳跃闪烁、切应用黑闪、滚动方向错误与卡顿）进行**地面真值诊断 → 26 项目像素级对标 → 可验证修复 → 自动化门禁**的闭环。本计划在 `terminal-ux-polish-v4` 已落地代码基础上，以**模拟器 90fps+ 可复现、后端确定性 997+ pass、前端像素级无回归**为验收硬门槛。
> 方法: `research → spec → plan → implement → verify → 4× 双审（Standards + Spec）`，双审连续 PASS 前不视为完成。

## 1. 背景与目标

### 1.1 用户原诉（2026-08-29 原文）

> 设置显示 cjk 字体 noto sans cjk 实际宋体（模糊）且渲染极其缓慢，输入法弹出/隐藏时 "终端页面" 动画 跳跃 闪烁，切换应用终端页面进入时闪烁，上下滑动方向错误 不流畅。首先通过调试找出问题原因，进行测试包括虚拟机测试

拆解为 4 个可观测缺陷：

| # | 缺陷 | 复现路径 | 用户感知 | 严重度 |
|---|------|----------|----------|--------|
| 1 | CJK 回退字体错乱：设置页显示 `Noto Sans CJK`，实际渲染为宋体（衬线）且模糊，首屏极慢（>300ms jank） | 任一中文字符首次渲染（`echo 中文渲染速度测试`） | 可读性 P0 | P0 |
| 2 | IME 输入法弹出/收起：终端页面跳跃、闪烁、文字被横向压扁 | 键盘 0→320dp 动画期间，输入法切换 | 视觉断裂 P0 | P0 |
| 3 | 切应用黑闪：HOME → 最近任务 → 回前台，终端页闪黑/白 200ms+ | Surface 保留未销毁的往返 | 可用性 P0 | P0 |
| 4 | 滚动方向错误、不流畅（慢速阈值卡、fling 抖） | 单指拖动与 fling | 可用性 P0 | P0 |

### 1.2 历史声称 vs 地面真值（代码审计 2026-08-29）

| 维度 | `full-closure-v3`/`polish-v4` 文档声称 | 2026-08-29 代码实际状态 | 结论 |
|------|----------------------------------------|------------------------|------|
| CJK | 保序去重 + Serif 惩罚 32 + token 边界 + outline LRU 10k + 3字符多数投票 + shaping 分段 | **已落地**：`cjk.rs:15 CJK_SERIF_PENALTY=32`、`locale_token_match`、`scan_fallback_candidates` 多数投票、`glyph_cache.rs:53 outline_cache 10k`、`pipeline.rs:676 cjk_fallback_names` 保序、`shaping.rs` 仅 CJK ranges 加 spans | 代码层已闭环，但**未在真机/模拟器以日志 + screencap + 帧时验证**，用户仍报告复现，需以**测量为准**而非代码为准 |
| IME | hybrid `offset` 放置阶段零重组 + 3帧 settle 单次 `reconfigure_swapchain` | **已落地但存在可验证瑕疵**：`TerminalScreen.kt:499 LaunchedEffect delay 48ms` 正确，但 `animateDpAsState spring 4500/0.9` 每帧触发 `TerminalScreen` 重组（`animatedImeBottom` 在组合阶段读取），`Modifier.offset` 虽为放置阶段但外层 recomposition 仍每帧发生；`onApplyWindowInsets` 仅记 `lastImeBottom` 正确；`applyGridResize` 已去 `modifierBarHeightPx` 双减正确 | 动画期仍有**可测量重组开销**，SwiftShader 上可能贡献 jank；需重构为**placement 内联读取**以零重组 |
| 切应用 | 0-size 守卫 + ON_RESUME 双清 `render_paused` + `reconfigure_swapchain` 原地 | **已落地**：`TerminalSurface.surfaceCreated` 0 守卫、`TerminalScreen ON_RESUME` 双路径、`context.rs attach_surface` fast-path `RECONFIGURE_SWAPCHAIN` | 逻辑正确，但**APK 未含 .so**（`android/app/build/outputs/apk/release/app-release.apk 7.3M`，无 `jniLibs`）导致渲染黑帧；`resumeRendering` 与 `setRenderPaused` 竞争需审计 |
| 滚动 | `floor` 对称 + Termux parity + vsync `postOnAnimation` | **已落地**：`TerminalSurface:1800 floor` 对称、`fling -velocityY` 同向、`doFlingStep` vsync | 方向与 Termux 一致，但用户体感“反向”需以**单测 + 手指上滑露历史**的可视化证据判定，非主观 |

**核心判断**：`polish-v4` 的 4 项代码修复**已进入主分支**，但**缺乏端到端的可复现证据**（日志、录屏、帧时直方图）且存在 1 处可验证的 IME 重组瑕疵 + 1 处构建产物缺失，导致用户仍感知缺陷。本次 v5 以**测量驱动**重做验证与精修。

### 1.3 目标（可量化验收）

| 指标 | 验收值 | 测量方法 | 工具链 |
|------|--------|----------|--------|
| CJK 首屏 14 字 | <400ms（暖机），二次 <16ms | `FALLBACK_HIT` 日志时间差 + `Choreographer` | `adb logcat` + `screencap` |
| CJK 无宋体 | 设置首项 == 日志首项，像素采样无衬线 | `font_information()` vs `FALLBACK_HIT` 家族 + 色直方图 | `adb logcat FALLBACK_CANDIDATE -15 vs -47` + `screencap` 像素采样 |
| CJK 无模糊 | `Mask/SubpixelMask` 矢量渲染，`cellHeight.ceil()` | `rasterization.rs` 逻辑 + 目视 | `screencap` 放大 |
| IME Missed frame | 0（曾 74） | `dumpsys gfxinfo` | `adb shell dumpsys gfxinfo com.termux framestats` |
| IME 无压扁 | `applyGridResize` 每往返恰 1 次，`cell.wgsl` UV 不拉伸 | 日志审计 + 逐帧录屏 | `adb logcat applyGridResize` + `screenrecord` 60fps |
| 切应用黑帧 | 0/3 次 | `screenrecord` + `RECONFIGURE_SWAPCHAIN` 日志 | `adb screencap` + `logcat` |
| 滚动方向 | 与 Termux 一致（上滑露历史） | 单测 + 手指上滑 | `TerminalSurfaceLogicTest` + 手势 |
| 滚动流畅 | p95<11ms, janky<5% | `dumpsys gfxinfo framestats` 300帧 | `adb shell dumpsys gfxinfo` |
| 后端 | `cargo test --workspace` 99%+ pass | CI | `nix develop -c cargo test` |
| 前端 | `spotlessCheck detekt` PASS | CI | `nix develop -c gradle` |
| 构建产物 | APK 含 `.so`，`libnative.so` 20M±，`readelf` 无 `ghostty` 动态依赖 | 构建校验 | `scripts/build-android-libs.nu` + `readelf` |

## 2. 26 项目像素级对标（提炼自 44 文件与 00-TORVOX-BASELINE，第二轮精读）

| 维度 | 对标项目 | 抄得技术 | 落点 | 是否已落地 | v5 精修 |
|------|----------|----------|------|------------|---------|
| 字体回退 | ghostty-android 4-slot TerminalFontStore、CellRun | 独立 bold/italic 槽、字体族精确匹配 | `pipeline.rs set_font_family_for_style` | ✅ | 保持 |
| 字体回退 | moke Nerd 层分桶、Symbol/Emoji 分层 | CJK→Symbol→Nerd→Emoji→db scan 链路 | `cjk.rs` 四层 | ✅ | 保持 |
| 字体回退 | warp ASystemFontIterator / Shaping::Basic for CJK | 系统字体发现、shape 分段仅 CJK ranges | `shaping.rs` 分段 | ✅ | 保持 |
| 字体回退 | shashlik wgpu is_emulator 双后端 | 字体光栅化在模拟器 GL 回退时不模糊 | `rasterization.rs hint(true)` + `wgpu_backend` | ✅ | 审计 `raster_scale` |
| 选择/滚动 | termux-app `TerminalView:170-187` 上滑→`mTopRow--` | 拖动方向 `offset+=` 同向 | `TerminalSurface:1793-1805` | ✅ | 补单测 |
| 选择/滚动 | ghostty-android tapCount、边缘滚动 | 多击不依赖 GestureDetector | 已有 | ✅ | 保持 |
| IME | termux-app `adjustNothing + WindowInsetsAnimation.Callback` + Haven `imePadding` | `offset` 放置阶段零重组 + settled 单次重排 | `TerminalScreen.kt` | ⚠️ 部分 | **v5: 去 `animateDpAsState`，改 placement 内联读取** |
| IME | cpmdroid 屏内滚动不缩字号 | IME 时文字不缩放，仅平移 | `offset` 保证 | ✅ | 保持 |
| lifecycle | zelland/wgpu `jni_fn + acquire重试 + reconfigure_swapchain` | 原地重配 vs `ERROR_NATIVE_WINDOW_IN_USE_KHR` 重建 | `context.rs` | ✅ | 审计 `release_surface` 时序 |
| lifecycle | ghostty-android `hide(0)` 同步、`scrollShift GPU blit` | 窗口重建不闪白 | 已有 | ✅ | 保持 |
| 渲染 | zelland 行级脏缓存、DirtyBand、CachedInstances | 增量渲染保 90fps+ | `cell_builder.rs` | ✅ | 保持 |
| 测试 | fission LiveTest + PNG 截图 | 无 TTY 确定性帧 + 像素级门禁 | `mod.rs` 截图测试 | ✅ | 复用 |

**不抄**：`Canvas.drawText` 每格（AGENTS.md 禁止）、`portable-pty`、Java 文件、JNA、`bash/sh` 脚本。

**第二轮精读清单（已读 44→精读 9，新增 3）**：

- `research-termux-app.md` §2-3（选择）、`research-ghostty-android.md` §2（tapCount）、`research-warp.md` §9（composing diff）、`research-shashlik.md` §3（wgpu 双后端）、`research-haven.md` §2（smartCopy）、`research-zelland-wgpu.md` §5（DirtyBand）— 已精读
- **新增**：`research_moke.md` §2（Nerd 层分桶细节，确认 `is_nerd_candidate` 边界）、`research_warp_extra.md` §11（ASystemFontIterator OEM 路径 `/odm/fonts` 验证）、`research_termux_app_extra.md` §9（宽字符吸附与 wrap 拼接，确认 `snapToWideCharBoundary` 已对齐）

## 3. 根因深度分析（First Principles + Think-in-Code + 代码证据 + 测量假设）

### 3.1 CJK 宋体/模糊/极慢（P0）— 代码已修复，需测量验证

#### 3.1.1 历史根因（已修复，留作回归基线）

- **显示与实际不一致**：`pipeline.rs:cjk_fallback_names()` 曾 `sort()+dedup()` 字母序；`cjk_fallback_ids` 按 `effective_priority` 排序。设置页读前者，渲染取后者首项。已修复为保序 `HashSet` 去重，按 `ids` 顺序，`cjk_fallback_names_sorted()` 另提供排序版。
- **Serif 仍胜 Sans**：`CJK_SERIF_PENALTY=4` 时 Sans 位图 worst -15 vs Serif 矢量 best 11，Serif 胜。`contains(locale_tag)` 把 `misc` 判为 `sc`。已修复为 32 + token 边界 `split(!alnum).any(|t| t==tag)`。
- **极慢**：`scan_fallback_candidates` 每候选建 `scaler_context.builder().size().hint(true).build() + Render`，无缓存，`shape_run` 曾整段 `0..len` 加 CJK fallback。已修复为 3字符多数投票 + `outline_cache LRU 10k` + `shape_run` 仅 CJK ranges。
- **模糊**：混合位图/矢量误判导致位图被当矢量放大。多数投票 + `Mask/SubpixelMask` 已修复。

#### 3.1.2 2026-08-29 仍报告复现的假设与验证路径

假设 A（**构建产物缺失**）：`android/app/build/outputs/apk/release/app-release.apk 7.3M` 远小于预期 91M，`jniLibs` 空，`libnative.so` 未打包。无 native 渲染时回退到空白或系统字体，导致“宋体模糊”。**验证**：`unzip -l app-release.apk | grep .so`、`ls jniLibs`、`readelf -d libnative.so`。

假设 B（**模拟器字体 TTC 多面**）：`/system/fonts/NotoSansCJK-Regular.ttc` 含多个 face，首个 face 家族名可能被误判为 Serif。**验证**：`adb shell ls /system/fonts/ | grep -i cjk`、`adb shell dumpsys` + `logcat FALLBACK_CANDIDATE` 审计 `eff_pri -15 vs -47` 是否真正 Sans 胜。

假设 C（**raster_scale 未同步**）：`font_pipeline.set_raster_scale` 与 `renderer.set_raster_scale` 需一致，否则 atlas 位图与 wgsl 投影不一致导致模糊。**验证**：`grep -rn set_raster_scale` 审计调用点，`logcat FONT_RASTERIZE_ASCII`。

**处置**：三假设均以日志与产物校验为准，v5 不改 CJK 核心逻辑（已正确），仅补**构建门禁**与**日志审计自动化**。

### 3.2 IME 跳跃闪烁（P0）— 1 处可验证瑕疵

#### 3.2.1 当前 pipeline（代码审计 2026-08-29）

- `TerminalScreen.kt:496 rawImeBottomPx = WindowInsets.ime.getBottom(density)` 每帧 composition 读取
- `LaunchedEffect(rawImeBottomPx)` 48ms 防抖后 `settledImePx = rawImeBottomPx` + `surface.onImeSettled`
- `animateDpAsState spring 4500/0.9` 每帧更新 `animatedImeBottom` → `animatedImePx` → 触发 `TerminalScreen` 整树 recomposition（`animatedImeBottom` 在组合阶段读取），`Modifier.offset` 虽为放置阶段但外层已重组
- `onApplyWindowInsets` 仅记 `lastImeBottom` + 清 selection，正确
- `applyGridResize(height - imeBottom)` 已去双减，正确

#### 3.2.2 跳跃/闪烁机制（假设，需测量）

- **Pan 期重组开销**：`TerminalScreen` 每帧重组（~41ms on SwiftShader）→ `Choreographer Missed` 曾 74 帧。即使 `offset` 不触发布局，**组合阶段**仍执行大量 `remember`/`LaunchedEffect` 检查。
- **Settled 突变**：48ms 后 `padding` 生效 + `applyGridResize` → `reconfigure_swapchain`。若 Pan 期卡顿，Settled 切量突变视觉跳跃。

#### 3.2.3 v5 修复

- **去 `animateDpAsState`**：动画期 `offset` 改为**内联读取** `WindowInsets.ime.getBottom` 于 placement lambda 内，零组合、零重组。系统侧 `WindowInsetsAnimation` 已提供插值，无需 Compose spring 二次插值。
- **保留 LaunchedEffect 48ms settled**：`rawImeBottomPx` 仍用于 settled 判定，但**不驱动动画**；`isImeSettled` 切换时单次 `padding` + `onImeSettled`。
- **验证**：`dumpsys gfxinfo framestats` Missed 0，`logcat applyGridResize` 往返 1 次，`screenrecord` 逐帧无压扁。

### 3.3 切应用闪黑（P0）— 构建产物为首因

- **0×0 early attach**：`surfaceCreated width<=0||height<=0 return` 已加
- **render_paused 未清**：`ON_RESUME` 双路径已加
- **剩余风险**：`attach_surface` fast-path `reconfigure_swapchain` 需确保 `surfaceConfig` width/height 与 `projection_width` 同步；`release_surface` 在 `surfaceDestroyed` 后 `frame_invalidated=true` 已加
- **v5 审计**：`context.rs:488 RECONFIGURE_SWAPCHAIN` 日志 + `ffi.rs ANativeWindow_setBuffersGeometry` + `TerminalSurface.surfaceDestroyed` 置 0

**首要修复**：重建 `jniLibs` 与 `assets/bin`，确保 APK 含 `libnative.so`（`scripts/build-android-libs.nu` + `scripts/build-apk.nu` 门禁）。

### 3.4 滚动方向错误/不流畅（P0）— 需可视化证据

#### 3.4.1 方向

- Android `GestureDetector.onScroll distanceY = previousY - currentY`，Termux `mTopRow -= distanceY/cellH`（上滑>0→更小 Top→更老）。torvox `offset += floor(distanceY/cellH)` 等价。注释已正确，`fling -velocityY` 同向正确。

用户感知的“反向”可能源于**对“终端历史在上”的心智 vs 列表历史在下**的差异。**处置**：以 `ScrollBehaviorQuantifiedTest` + `seq 1 400` 手势录屏为准，保留 Termux parity，不改方向，但补**单测与文档显式对照**。

#### 3.4.2 卡顿

- `floor` 已对称，比 `toInt(trunc)` 修复慢速负向卡顿
- `OverScroller postOnAnimation` 已有
- **v5 审计**：补 `cellHeight.coerceAtLeast(1f)` 防 0 除（已有），`currentScrollbackLength()` 10Hz 缓存（已有），`forceRender()` vsync 节流（已有）
- **性能目标**：`dumpsys gfxinfo framestats` 300 帧 p50≤7ms p95≤11ms janky<5%（`Mailbox` present_mode），`Choreographer` loop 6ms ≈166fps

## 4. 实施计划（依赖顺序，增量可独立验证，每步附证据）

| 阶段 | 内容 | 文件 | 验证 |
|------|------|------|------|
| A0 | **构建门禁修复**：`scripts/build-android-libs.nu` → `jniLibs`/`assets/bin` 填充，`readelf --dynamic` 校验，`scripts/build-apk.nu` 产出 90M+ APK | `scripts/*`, `android/app/src/main/jniLibs` | `unzip -l app-*.apk \| grep .so` 非空，`stat libnative.so` 20M± |
| A1 | **IME 零重组重构**：`TerminalScreen.kt` 去 `animateDpAsState`，`offset` 改 placement 内联读取 `WindowInsets.ime.getBottom`，保留 `LaunchedEffect 48ms` settled | `TerminalScreen.kt` | `dumpsys gfxinfo` Missed 0，`logcat applyGridResize` 1/往返 |
| A2 | **CJK 日志审计自动化**：补 `logcat` 校验脚本，`FALLBACK_CANDIDATE -15 vs -47` 断言 | `scripts/test-emulator.nu`, `docs/verification` | CI 日志审计 PASS |
| A3 | **切应用审计**：确认 `RECONFIGURE_SWAPCHAIN` 原地，`0x0` early attach 0 次，`render_paused` 已清 | `TerminalScreen`, `TerminalSurface`, `context.rs` | 3次往返 0黑，`screencap` 均见 `$` |
| B1 | **滚动证据**：`ScrollBehaviorQuantifiedTest` 复跑，`seq 1 400` 手势录屏 | `ScrollBehaviorQuantifiedTest.kt` | vsync 16.6ms±3ms |
| C1 | **依赖更新**：`cargo update` patch 小版，`flake.nix` `fenix stable` 校验，`android/build.gradle.kts` compose BOM 2026.08.00 / kotlin 2.4.10 / gradle 8.13 审计 | `Cargo.lock`, `flake.nix`, `android/build.gradle.kts` | `cargo clippy --deny warnings` 0，`spotlessCheck` PASS |
| D1 | **文档与 openspec**：本计划、openspec v5、verification 模板 | `docs/plans/*`, `openspec/changes/*`, `docs/verification/*` | 4轮双审连续 PASS |

**分支**：`main` 直接增量（改动面 <5 文件，风险低）
**预计**：编码 0.5 天、自动化 0.5 天、双审 4 轮（含修复）合计 <1 天

## 5. 测试计划（双腿：后端确定性 + 前端像素级 + 性能 + 日志审计）

### 5.1 后端确定性

- `nix develop -c cargo test --workspace` 预期 1000± pass（`cjk_priority_tests` 5、`is_cjk_candidate` 27、`outline_cache`、CellInstance 不变量、`font_dirs` 等）
- `nix develop -c cargo clippy --all -- --deny warnings` 0，`cargo fmt --check` 0
- `outline_cache` 命中率：首屏后二次 hit >99%（log 采样 `FALLBACK_HIT` + `glyph_source_is_outline_cached`）

### 5.2 前端像素级（模拟器 Pixel 9 API35 1080×2400@420dpi SwiftShader，真机复测同流程）

- `CursorPixelAcceptanceTest` CJK 无宋体（像素采样色差 + 日志一致）
- `ScrollBehaviorQuantifiedTest`：`enter_snaps_viewport_to_bottom` ≤2000ms，`pty_flood_never_resets_viewport_mid_gesture` 0 collapses，floor 对称
- `KeyboardJellyInstrumentedTest` Missed 0
- `EchoGridDumpTest` + `screenrecord` 60fps 逐帧：IME 期间文字不压扁、切应用 0 黑
- `dumpsys gfxinfo framestats` 300 帧 p50≤7ms p95≤11ms janky<5%（`Mailbox`）
- `Maestro flows/launch.yaml, font-change-rerender.yml` 回归（如可用）

### 5.3 日志审计（自动化脚本）

```bash
adb logcat -v threadtime --pid=$PID | grep -E "FALLBACK|RECONFIGURE|applyGridResize|render_paused|SCROLL|FONT_RASTERIZE"
# 断言：FALLBACK_CANDIDATE sans -15 vs serif -47, FALLBACK_HIT Noto Sans CJK, RECONFIGURE_SWAPCHAIN 原地, applyGridResize 1/IME往返, render_paused cleared on RESUME, FONT_RASTERIZE_ASCII before/after
```

### 5.4 性能（Hz 与 fps 解耦）

- Rust 渲染线程 `loop timing 6ms ≈166fps`（`Choreographer` log），`wgpu present_mode Mailbox` Hz 解耦，真机 120Hz 预留 `setFrameRate`
- `shape_run` 分段后 Latin 不经 CJK fallback，`outline_cache` 后首屏 22ms→二次 <16ms

### 5.5 构建产物门禁

```bash
./scripts/build-android-libs.nu --profile release arm64-v8a x86_64
readelf --dynamic android/app/src/main/jniLibs/arm64-v8a/libnative.so | grep NEEDED # 无 ghostty
./scripts/build-apk.nu --release --debug
unzip -l android/app/build/outputs/apk/release/app-release.apk | grep .so # 非空
unzip -l android/app/build/outputs/apk/debug/app-debug.apk | grep .so # 非空
stat -c%s android/app/src/main/jniLibs/arm64-v8a/libnative.so # 15M-60M
```

## 6. 验收标准（与 openspec 一致，量化，可自动化）

- [ ] 设置首项 == 渲染首项（`cjk_fallback_names()[0]` 文本 == `FALLBACK_HIT` 家族，`FALLBACK_CANDIDATE -15 vs -47`）
- [ ] `misc symbols` 不获 `sc` 加分（`locale_token_boundary_misc_not_sc` pass）
- [ ] 中文 14 字首屏 <400ms、二次 <16ms，无宋体衬线（screencap + 色直方图，`Mask` 渲染）
- [ ] IME 0→320dp 动画 0 次 `applyGridResize` 期间、settled 单次，无跳跃/压扁（Missed 0，`offset` 零重组）
- [ ] HOME 往返 3 次 0 黑帧（`surfaceCreated` 0 守卫命中、ON_RESUME 即时清暂停、`RECONFIGURE_SWAPCHAIN` 原地）
- [ ] 滚动上滑露历史、下滑回底，慢速 0.4*cellH×3 帧阈值对称，fling vsync 16.6ms±3ms（`ScrollBehaviorQuantifiedTest` PASS）
- [ ] 后端 `cargo test --workspace` 99%+ pass、`clippy` 0、`spotlessCheck detekt` PASS、4 轮双审连续 PASS
- [ ] 构建产物门禁 PASS（APK 含 .so，`readelf` 无 ghostty NEEDED）

## 7. 风险与回滚

| 风险 | 缓解 | 回滚 |
|------|------|------|
| `reconfigure_swapchain` 驱动不支持 | 保留 `release+attach` 回退+`RECREATE` 日志 | `git revert` 单文件 |
| IME 去 spring 后 stepped 报告显阶梯 | 系统 `WindowInsetsAnimation` 已插值，实测如有阶梯可加 `derivedStateOf` 插值 | 改常量 |
| 构建产物过大（>60M） | `MAXIMUM_SO_SIZE_BYTES` 门禁，release profile 强制 | 切 profile |
| `floor` 改变积累语义 | 单测覆盖正负 | 切回 `toInt` |

## 8. 文档与 Openspec 产出清单

| 文档 | 路径 | 职责 |
|------|------|------|
| 本计划 | `docs/plans/2026-08-29-terminal-ux-v5-deep-diagnosis-detailed.md` | 详细诊断、26项目矩阵、根因、方案、时间线 |
| Openspec Proposal | `openspec/changes/terminal-ux-v5-polish/proposal.md` | 4 缺陷收敛提案 |
| Openspec Design | `openspec/changes/terminal-ux-v5-polish/design.md` | 架构、时序、测试 |
| Openspec Tasks | `openspec/changes/terminal-ux-v5-polish/tasks.md` | 任务与验收 |
| Openspec Specs | `openspec/changes/terminal-ux-v5-polish/specs/*` | cjk/ime/scroll 契约 |
| Verification | `docs/verification/2026-08-29-terminal-ux-v5-verification.md` | 端到端证据模板 |

## 9. 26 项目二次精读清单（已读 44→精读 9）

- `research-termux-app.md` §2-3（选择）、`research-ghostty-android.md` §2（tapCount）、`research-warp.md` §9（composing diff）、`research-shashlik.md` §3（wgpu 双后端）、`research-haven.md` §2（smartCopy）、`research-zelland-wgpu.md` §5（DirtyBand）、`research_moke.md` §2（Nerd 分桶）、`research_warp_extra.md` §11（OEM 路径）、`research_termux_app_extra.md` §9（宽字符）——精读笔记见 `docs/reference-projects.md` 1.1 增补（待建）。

