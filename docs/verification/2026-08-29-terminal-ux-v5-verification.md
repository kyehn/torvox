# Verification — terminal-ux-v5-polish (2026-08-29)

> **Scope**: 4 大 P0 — CJK 字体错乱/模糊/极慢、IME 跳跃闪烁、切应用黑闪、滚动方向/卡顿。Method: 后端确定性 + 前端像素级 + 构建产物门禁 + 日志审计。Env: `nix develop` (wvulkan lavapipe), Pixel 9 API35 1080×2400@420dpi SwiftShader（如有真机同测）。

## 1. 构建产物门禁 ✅ / ❌

```bash
./scripts/build-android-libs.nu --profile release arm64-v8a x86_64
ls -lh android/app/src/main/jniLibs/arm64-v8a/libnative.so android/app/src/main/jniLibs/x86_64/libnative.so
readelf --dynamic android/app/src/main/jniLibs/arm64-v8a/libnative.so | grep NEEDED  # 无 ghostty
./scripts/build-apk.nu --release --debug
unzip -l android/app/build/outputs/apk/release/app-release.apk | grep .so  # 1+ 条
unzip -l android/app/build/outputs/apk/debug/app-debug.apk | grep .so  # 1+ 条
stat -c%s android/app/src/main/jniLibs/arm64-v8a/libnative.so  # 15M-60M
```

- 预期：release/apk 各含 `.so`，`libnative.so` 15M-60M，`readelf` 无 `libghostty-vt.so` NEEDED（静态链接）
- 实际：____  已记录于 `evidence/build/`（`unzip -l` 输出、`readelf` 输出、`stat`）

## 2. 后端确定性

```bash
nix develop -c cargo test --workspace  # 99%+ pass
nix develop -c cargo clippy --all -- --deny warnings  # 0
nix develop -c cargo fmt --check  # 0
```

- 预期：`cjk_priority_tests` 5、分类 27、总计 1000± pass
- 实际：____

## 3. CJK 保真

```bash
adb logcat -v threadtime --pid=$PID | grep -E "FALLBACK|FONT_RASTERIZE|CJK_FALLBACK"
# 预期：FALLBACK_CANDIDATE: noto sans cjk -15 vs noto serif cjk -47，FALLBACK_HIT: Noto Sans CJK JP，CJK_FALLBACK: found 3，FONT_RASTERIZE_ASCII before/after
adb shell screencap -p /sdcard/cjk.png && adb pull /sdcard/cjk.png evidence/cjk/
# 预期：screencap 像素采样无宋体衬线（黑体直线收尾）
adb shell dumpsys gfxinfo com.termux framestats | grep HISTOGRAM
# 预期：首屏 14 字 cat <400ms，二次 <16ms
```

- 实际：____  `evidence/cjk/`（日志片段 + screencap + framestats）

## 4. IME 零跳跃（v5 零重组）

```bash
adb shell dumpsys gfxinfo reset; adb shell input tap 540 600; sleep 2; adb shell dumpsys gfxinfo com.termux framestats
# 预期：Total frames Missed 0（曾 74），p95<11ms

adb logcat --pid=$PID | grep -E "applyGridResize|RECONFIGURE_SWAPCHAIN"
# 预期：每次 IME 往返 applyGridResize 恰 1 次（settled 后），RECONFIGURE_SWAPCHAIN 原地，animating 0 次

screenrecord --time-limit 10 /sdcard/ime.mp4 → 逐帧 60fps 无文字横向拉伸
# 预期：cell.wgsl UV 不变，网格尺寸仅在 settled 时切换（1080x2209 → 1080x1326）
```

- 实际：____  `evidence/ime/`（framestats + logcat + screenrecord 逐帧截图）

## 5. 切应用零黑

```bash
for i in 1 2 3; do
  adb shell input keyevent 3; sleep 1
  adb shell am start -n com.termux/.MainActivity; sleep 1
  adb shell screencap -p /sdcard/switch_$i.png; adb pull /sdcard/switch_$i.png evidence/switch/
  adb logcat --pid=$PID | grep -E "surfaceCreated|render_paused|RECONFIGURE|0x0"
done
# 预期：surfaceCreated 0-size 守卫命中时无 0x0 attach，ON_RESUME 立即清 paused，3 张图均见 $ prompt，RECONFIGURE 原地
```

- 实际：____  `evidence/switch/`（3 张 screencap + logcat）

## 6. 滚动方向与流畅

```bash
adb shell dumpsys gfxinfo com.termux framestats | grep -A 5 HISTOGRAM
# 预期：p50≤7ms p95≤11ms janky<5% (Mailbox)
# 手势：seq 1 400 后上滑露旧 1-39，下滑回 0 见 $，screenrecord 逐帧无碎片
# 仪器化：ScrollBehaviorQuantifiedTest 0 collapses，enter_snap ≤2000ms
```

- 实际：____  `evidence/scroll/`（framestats + 录屏 + 仪器化日志 `UX_METRIC`）

## 7. 前端门禁

```bash
nix develop -c ./gradlew spotlessCheck detekt --offline  # BUILD SUCCESSFUL
nix develop -c ./gradlew :app:assembleRelease --offline  # 90M apk 含 .so
```

- 实际：____

## 8. 证据归档

- 结构：`docs/verification/2026-08-29-terminal-ux-v5-verification/evidence/{build,cjk,ime,switch,scroll}/`
- 随 `cargo test`/`gradle` 产出物一起作为 `git commit` 附带验证（或 CI artifact）
