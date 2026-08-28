# Verification — terminal-ux-polish-v4 (2026-08-28)

> **Scope**: 4 大 P0 — CJK 宋体/模糊/极慢、IME 跳跃闪烁、切应用黑屏、滚动方向/卡顿。Method: 后端确定性 + 前端像素级 + 性能 + 日志审计。Env: `nix develop` (wgpu lavapipe), Pixel 9 API35 1080×2400@420dpi SwiftShader（如有真机同测）。

## 1. 后端确定性

```bash
nix develop -c cargo test --workspace -- --test-threads=1  # 997+ pass, 10 ignored
nix develop -c cargo clippy --all -- --deny warnings       # 0
nix develop -c cargo fmt --check                            # 0
```

- `cjk_priority_tests` 8 用例: `sans>serif`, `locale_token_boundary_misc_not_sc`, `serif_penalty_guards_vector_vs_bitmap`, `locale_boost`, `unknown_fallback`, plus original 3.
- `glyph_cache::outline_cache_evicts_with_clear` pass.
- `pipeline cjk_fallback_names` 保序：`ordered == ids order`, `sorted != ordered` 时显示仍保序。

## 2. CJK 保真

```bash
adb logcat -v threadtime --pid=$PID | grep FALLBACK
# 预期:
# FALLBACK_CANDIDATE: family='noto sans cjk jp' advance=... is_vector=true eff_pri=11
# FALLBACK_CANDIDATE: family='noto serif cjk jp' advance=... is_vector=true eff_pri=-21  (penalty 32)
# CJK_FALLBACK: found 3 fallback fonts
# FALLBACK_HIT: ch=U+4E2D layer='Noto Sans CJK JP'  (与 cjk_fallback_names()[0] 一致)
adb shell dumpsys gfxinfo com.termux framestats | grep HISTOGRAM
# 首屏 14 字 cat: <400ms (第二次 <16ms), no Missed 300ms
```

- `screencap` 像素采样：黑体笔画直线收尾无衬线三角（宋体特征），色直方图对比 Sans vs Serif。
- `font_information()` JSON `cjk_families[0]` == log 首项。

## 3. IME 零跳跃

```bash
adb shell dumpsys gfxinfo reset; adb shell input tap 540 600; sleep 1; adb shell dumpsys gfxinfo com.termux framestats
# Total frames during IME show: Missed 0, p95<11ms

adb logcat --pid=$PID | grep -E "applyGridResize|RECONFIGURE"
# 每次 IME 往返 applyGridResize 恰 1 次（settled 后），RECONFIGURE_SWAPCHAIN 原地，重建 0
# animating 期 0 次 applyGridResize（offset 阶段）
screenrecord --time-limit 10 /sdcard/ime.mp4 →逐帧 60fps 无文字横向拉伸（cell.wgsl UV 不变）
```

- `Modifier.offset` 放置阶段零 `measure`，`navigationBarsPadding` 单点验证：`dumpsys window windows | grep mNavigationBar` 仅外层。

## 4. 切应用零黑

```bash
for i in 1 2 3; do
  adb shell input keyevent 3; sleep 1
  adb shell am start -n com.termux/.MainActivity; sleep 1
  adb shell screencap -p /sdcard/switch_$i.png
  adb logcat --pid=$PID | grep -E "surfaceCreated|render_paused|RECONFIGURE|0x0"
done
# 断言: surfaceCreated 0-size 守卫命中时无 0x0 attach, ON_RESUME 立即清 paused, 3 张图均见 $ prompt, RECONFIGURE 原地
```

## 5. 滚动方向与流畅

```bash
adb shell dumpsys gfxinfo com.termux framestats | grep -A 5 HISTOGRAM
# p50≤7ms p95≤11ms janky<5% (Mailbox)
```

- 手指上滑 1 cell → `scrollOffset==1` 视口上移露历史（`seq 1 200` 后上滑见旧 1-39，下滑回 0 见 $）。
- 慢速 0.4*cellH×3 → 第 3 帧才滚动（floor 对称），负向同阈值。
- Fling `velocityY=+2000` 下滑 → 惯性向 0 回弹，`Choreographer` avg 6ms。

## 6. 前端门禁

```bash
nix develop -c gradle spotlessCheck detekt --offline  # BUILD SUCCESSFUL
nix develop -c gradle :app:assembleDebug --offline    # 91M apk, libnative.so 22M
```

## 7. 依赖

- `cargo update -p wgpu -p fontdb -p swash -p cosmic-text` patch 小版检查，`flake check` pass。
- `gradle libs.versions` compose BOM 2026.08.00 / kotlin 2.1 / gradle 8.13 审计。

## 8. 证据归档

- 日志片段、screencap 前后对比（CJK 中文、IME 展开/收起、切应用）、framestats 直方图、screenrecord 逐帧截图，按 `docs/verification/2026-08-28-.../evidence/` 落盘。
- 本文档随 `cargo test`/`gradle` 产出物一起作为 `git commit` 附带验证。
