# Tasks: terminal-ux-v5-polish

## Phase A — 构建门禁与 IME 零重组

- [ ] A0 **构建产物门禁**：`scripts/build-android-libs.nu --profile release arm64-v8a x86_64` → `jniLibs`/`assets/bin` 填充，`readelf --dynamic` 校验无 `ghostty` NEEDED，`scripts/build-apk.nu --release --debug` 产出 90M± APK 含 `.so`（门禁：`unzip -l | grep .so` 非空，`stat libnative.so` 15M-60M）
- [ ] A1 **IME 零重组**：`TerminalScreen.kt` 删除 `animateDpAsState`/`spring` import 与 `animatedImeBottom/animatedImePx` 状态，动画期 `Modifier.offset { IntOffset(0, -WindowInsets.ime.getBottom(density)) }` 内联读取，保留 `LaunchedEffect 48ms` settled 判定与 `onImeSettled` 单次回调（`isImeSettled` 切换时 `padding(bottom=settled)`）
- [ ] A2 **CJK 日志审计脚本**：`grep -E "FALLBACK_CANDIDATE|FALLBACK_HIT|CJK_FALLBACK|FONT_RASTERIZE"` 日志断言（`sans -15 vs serif -47`、`Noto Sans CJK` 首项 = display 首项）
- [ ] A3 **切应用审计**：确认 `TerminalSurface.surfaceCreated` 0 守卫、`TerminalScreen ON_RESUME` 双清、`context.rs RECONFIGURE_SWAPCHAIN` 原地、`surfaceDestroyed frame_invalidated`，HOME 往返 3 次 0 黑（填充后 APK 基线）

## Phase B — 滚动与后端门禁

- [ ] B1 **滚动证据**：复跑 `ScrollBehaviorQuantifiedTest`（`enter_snaps_viewport_to_bottom` ≤2000ms，`pty_flood_never_resets_viewport_mid_gesture` 0 collapses），`seq 1 400` 手势录屏，上滑露历史
- [ ] B2 **后端门禁**：`nix develop -c cargo test --workspace` 99%+ pass，`cargo clippy --all -- --deny warnings` 0，`cargo fmt --check` 0
- [ ] B3 **前端门禁**：`nix develop -c gradlew spotlessCheck detekt` BUILD SUCCESSFUL，`assembleRelease` 产出含 `.so`

## Phase C — 依赖与文档

- [ ] C1 **依赖审计**：`cargo update` patch 小版检查（`wgpu 30.x / fontdb 0.23 / swash 0.2 / cosmic-text 0.19`），`flake.nix fenix stable` 校验，`android/build.gradle.kts` compose BOM 2026.08.00 / kotlin 2.4.10 / gradle 8.13 审计（小版 patch 升级，无大版跳跃）
- [ ] C2 **文档**：`docs/plans/2026-08-29-*.md` 详细计划（已产出）、`docs/verification/2026-08-29-terminal-ux-v5-verification.md` 证据模板（已产出雏形）、刷新 `docs/specification/DESIGN.md` 如需增补禁止实现/字体章节
- [ ] C3 **验证闭环**：`scripts/build-apk.nu` + `adb logcat` 5 场景 + `dumpsys gfxinfo framestats` 300 帧 + `screenrecord` 逐帧 + `screencap` 像素采样，按 `docs/verification/2026-08-29-*/evidence/` 归档

## Acceptance Criteria

- [ ] APK 含 `.so`（`unzip -l | grep .so` 1+），`libnative.so` 15M-60M，`readelf` 无 `ghostty` NEEDED
- [ ] `cjk_fallback_names()[0]` == `FALLBACK_HIT` 首项（`FALLBACK_CANDIDATE -15 vs -47`，`locale_token_boundary_misc_not_sc` pass）
- [ ] 中文 14 字首屏 <400ms、二次 <16ms，无宋体（`screencap` + 像素采样，`Mask` 渲染）
- [ ] IME 0→320dp 动画期 0 次 `applyGridResize`，settled 单次，无跳跃闪烁压扁（Missed 0，`offset` 零重组，`WindowInsetsAnimation` 系统插值）
- [ ] HOME 往返 3 次 0 黑帧（`surfaceCreated` 0 守卫、ON_RESUME 即时清 paused、`RECONFIGURE_SWAPCHAIN` 原地）
- [ ] 上下滑动方向与 Termux 一致（drag up→older）、慢速阈值对称、fling vsync 平滑（`ScrollBehaviorQuantifiedTest` 间隔 16.6ms±3ms，0 collapses）
- [ ] 后端 `cargo test --workspace` 99%+ pass、`clippy` 0、`spotlessCheck detekt` PASS、4 轮双审连续 PASS
