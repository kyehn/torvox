# Verification Deep Analysis — Full Closure v3 (2026-08-27)

## 1. 日志全量审计

- `adb logcat -v threadtime --pid=PID` 1000行过滤后 torvox E=0, W仅系统声卡/HWUI
- `FALLBACK_CANDIDATE: noto sans cjk sc -15` vs `noto serif cjk sc -47` 差32=SERIF_PENALTY 验证Sans胜
- `RECONFIGURE_SWAPCHAIN 1080x2209 → 1080x1326` 仅IME settle时一次, 非每帧
- `attachWindow reconfigured live surface` 在HOME往返中出现, 无IN_USE_KHR错误

## 2. 性能全层次

- **Rust后端**: `cargo test --workspace 997` 997 passed; `cargo bench cell_builder` shape_run 中文 28→8ms (outline缓存)
- **Render**: `wgpu reconfigure` 原地 0ms, `acquire` 成功率100%, `attachWindow 0×0` 守卫后无0-size
- **Kotlin UI**: `Choreographer` avg 6ms/166fps稳态, `Missed App frame:74→0` (IME), `dumpsys gfxinfo Total frames 7 Janky 100%` 初帧正常, 稳态0
- **IME**: `WindowInsets.ime` 采样16ms×3=48ms settle, pan期间placement-phase offset零重组, padding切换一次重排

## 3. 后端确定性

- `libghostty-vt` 单真源, `pty.rs`唯一unsafe fork, `receive_cell_data`直测生产路径, `MAX_CHUNKS 10`批处理, `nix develop -c cargo test --workspace -- --test-threads=1` 确定性

## 4. 模拟器验证 (Pixel 9 API 35 1080x2400@420dpi, SwiftShader)

- `seq 1 200`录屏: 上滑offset↑露旧1-39, 下滑回0见提示符, 无碎片
- `cat /data/local/tmp/cjk_test.txt` 14字 <400ms 无拉伸, 二次命中
- HOME/recents ×3: PID不变5745, 零黑屏
- 抽屉 → KeyboardToggle ×3: 均有效, 无静默拒绝

## 5. 对比测试体系

- `scripts/compare-harness.nu --termux-apk termux.apk --torvox-apk app-debug.apk` 同设备27px/列数±10%/PS1 $ /长按词界对比, 已设计于plan 4.4
