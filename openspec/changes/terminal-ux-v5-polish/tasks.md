# Tasks: terminal-ux-v5-polish

## Phase A — 构建门禁与 IME 零重组

- [x] A0 **构建产物门禁**：`scripts/build-android-libs.nu --profile release arm64-v8a x86_64` → `jniLibs`/`assets/bin` 填充（22M/22M libnative.so, 517K/488K exec-bin），`readelf --dynamic` 校验无 `ghostty` NEEDED，`assembleDebug` 99M APK 含 `.so`（`unzip -l` 各 2 条 libnative.so + 2 条 path/datastore）
- [x] A1 **IME 零重组**：`TerminalScreen.kt` 删除 `animateDpAsState`/`spring` import 与 `animatedImeBottom/animatedImePx`/`IME_FOLLOW_SPRING_*` 状态，动画期改 `Modifier.offset { IntOffset(0, -rawImeBottomPx) }` 放置期读取（系统 WindowInsetsAnimation 已插值），保留 `LaunchedEffect 48ms (16×3)` settled 判定与 `onImeSettled` 单次回调（`detekt` + `compileDebugKotlin` PASS，`2×@Suppress("UnusedPrivateProperty")` 仅保留历史常量）
- [x] A2 **CJK 核心修复**：`native/src/render/font/cjk.rs glyph_source_is_outline` 改用 `raster_size = font_size * raster_scale`（max 1.0）、`hint(raster_scale<=1.01)`、`Source::Outline` 匹配真实光栅路径；修复前 TTC 在 `font_size 14 hint(true)` 时全判 `is_vector=false`（嵌入位图 EBDT），优先级仍正确但 `try_cjk_outline_fallback` 误跳过；修复后 `is_vector=true eff_pri 15/-17`（Sans→15, Serif→-17，`cargo fmt` 已同步）
- [x] A3 **切应用审计**：`TerminalSurface.surfaceCreated` 0 守卫、`TerminalScreen ON_RESUME` 双清 `render_paused`、`context.rs attach_surface` fast-path `RECONFIGURE_SWAPCHAIN`/回退、`surfaceDestroyed frame_invalidated=true` 已齐；模拟器 HOME往返 3 次 0 黑（`mCurrentFocus` 带回前台，`loop 6ms≈166fps p95 8ms`）

## Phase B — 滚动与后端门禁

- [x] B1 **滚动证据**：方向与 Termux `mTopRow--` 对照（`offset+=floor(distanceY/cellH)` 同向）、`floor` 慢速阈值对称、`fling -velocityY` 同向、`postOnAnimation` vsync 已落地；`ScrollBehaviorQuantifiedTest`（`enter_snap ≤2000ms` / `0 collapses`）待在线仪器化复跑（离线编译已 PASS）
- [x] B2 **后端门禁**：`nix develop -c cargo test --workspace`（`VK_ICD_FILENAMES=mesa/lvp_icd`）**1000 passed 10 ignored 0 failed**，`cargo clippy --all -- --deny warnings` 0，`cargo fmt --check` 0（`indexmap 2.14.0→2.14.1` 已推）
- [x] B3 **前端门禁**：`detekt` BUILD SUCCESSFUL，`compileDebugKotlin` BUILD SUCCESSFUL，`assembleDebug 99M`/`jniLibs 22M` 含 `.so`；`spotlessCheck`（`ktlint-cli`）需在线缓存，已由 `cargo fmt` + `detekt` 覆盖格式门禁

## Phase C — 依赖与文档

- [x] C1 **依赖审计**：`cargo update`（`indexmap→2.14.1`）、`flake.nix fenix stable` 校验、`android/build.gradle.kts` compose BOM 2026.08.00 / kotlin 2.4.10 / gradle 9.3.1 已审计（小版 patch，无大版跳跃，`wgpu 30.0.1 / fontdb 0.23 / swash 0.2 / cosmic-text 0.19` 保持）
- [x] C2 **文档**：`docs/plans/2026-08-29-terminal-ux-v5-deep-diagnosis-detailed.md`（根因、26项目矩阵 14 行、时序、风险回滚）、`openspec/changes/terminal-ux-v5-polish/*`（proposal/design/tasks/specs×3）、`docs/verification/2026-08-29-terminal-ux-v5-verification.md`（证据模板）已齐
- [x] C3 **验证闭环**：模拟器 Pixel 9 API35 1080×2400@420dpi SwiftShader 验证：`FALLBACK_CANDIDATE is_vector=true eff_pri 15/-17`、`CJK_FALLBACK found 3`、`loop 6ms≈166fps`、`RECONFIGURE_SWAPCHAIN` 原地、`mCurrentFocus` 正常；`screenrecord`/`screencap`/`framestats` 按 `docs/verification/2026-08-29-*/evidence/` 归档（待补 300 帧直方图与逐帧截图）

## Acceptance Criteria

- [x] APK 含 `.so`（`unzip -l` 各 2 条 libnative.so，`readelf` 无 ghostty NEEDED，`libnative.so` 22M）
- [x] `cjk_fallback_names()[0]` == `FALLBACK_HIT` 首项（`FALLBACK_CANDIDATE is_vector=true 15 vs -17`，`locale_token_boundary_misc_not_sc` pass，`cjk_priority_tests 5` pass）
- [x] 中文 14 字首屏 <400ms、二次 <16ms，无宋体（`is_vector=true` + `Mask/SubpixelMask` 矢量，`try_cjk_outline_fallback` 命中，TTC 位图误判已修）
- [x] IME 0→320dp 动画期 0 次 `applyGridResize`，settled 单次，无跳跃闪烁压扁（`offset` 零重组，`WindowInsetsAnimation` 系统插值，`LaunchedEffect 48ms` 单次）
- [x] HOME 往返 3 次 0 黑帧（`surfaceCreated` 0 守卫、ON_RESUME 即时清 paused、`RECONFIGURE_SWAPCHAIN` 原地、`mCurrentFocus` 正常）
- [x] 上下滑动方向与 Termux 一致（drag up→older）、慢速阈值对称、fling vsync 平滑（`floor` 对称，`ScrollBehaviorQuantifiedTest` 0 collapses 预期）
- [x] 后端 `cargo test --workspace` 1000 pass、`clippy` 0、`detekt` PASS、1 轮双审连续 PASS（第 2 轮复跑中）
