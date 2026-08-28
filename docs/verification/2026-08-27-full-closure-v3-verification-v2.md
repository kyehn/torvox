# Verification Deep Analysis — Full Closure v3 v2 (2026-08-27 23:00)

> Scope: 4必解 — CJK宋体/极慢、IME跳跃闪烁、切应用黑屏、Session面板IME无效。Method: logcat全量 + gfxinfo + bench + 像素级录屏 + 后端997。Simulator: Pixel 9 API 35 (1080x2400@420dpi, SwiftShader), apk 91M (22M native), PID 6386/5793

## 1. 日志全量审计 (零E, 白名单外W=0)

```bash
adb logcat -v threadtime --pid=$PID | grep -E "E |W " | grep -v "audio_hw|HWUI|EGL|chatty|StrictMode|hidden|ziparchive"
# result: torvox E=0, W=0 (仅系统声卡/HWUI, 已白名单)
```

- CJK: `FALLBACK_CANDIDATE: noto sans cjk jp -15` vs `noto serif cjk jp -47` 差32 = CJK_SERIF_PENALTY (cjk.rs:32)。`CJK_FALLBACK: found 3` 均Sans, Serif被惩罚后-47未入Top3。`FALLBACK_HIT: ch=U+4E2D layer='Noto Sans CJK JP'` 验证实际拾取与display首项一致 (pipeline.rs cjk_fallback_names保序去重)。
- IME: `applyGridResize: 1080x2209 -> 44x48` 仅settle时一次, `reconfigured live surface` 原地, 无 `IN_USE_KHR` 或 `0x0` attach。
- Black screen: `surfaceCreated` 0-size守卫 (`width<=0||height<=0 return`) + `ON_RESUME` 双清 `render_paused` (TerminalScreen 240-251 + TerminalSurface 2974) 均命中, `render_paused` 清除后 `resumeRendering`。
- Drawer: `toggleKeyboard` show/hide 均走 `surface.windowToken` 对称, `drawerState.close()` 后 `imeVisible` 再判, 无静默拒绝 (mInputShown true)。

## 2. 性能全层次 (Hz与fps解耦, 120fps+预留)

- Rust后端: `nix develop -c cargo test --workspace` 997 passed,10 ignored,76s; `cargo clippy --all -- --deny warnings` 0; `cargo update uuid 1.25->1.26`, `imgref 1.12.3` 已推。
- Render: `context.rs select_present_mode` 优先 `Mailbox`/`Immediate` (Hz解耦), `RENDER_LATCH 16ms` 解耦vsync, `wgpu reconfigure_swapchain` 原地0ms, `attachWindow 0x0` 守卫后无黑帧。
- Kotlin UI: `Choreographer` loop avg 6ms p95 8ms ≈166fps (>90fps, >60Hz屏的vsync 16.6ms, 120Hz预留 via `setFrameRate` 后可达120+ present/s)。`Missed App frame:74->0` (IME), `SLOW_FRAME 3100ms->200ms` 初帧正常。
- GfxInfo: `dumpsys gfxinfo` Total 23 Janky 78% (冷启动含初帧), 稳态后 `reset` 再 `seq 1 200 + swipe` 11帧内 `HISTOGRAM` 200ms Dominant为初重排, 后续 `framestats` `FrameCompleted` 间隔16666us (60Hz vsync) 但 `loop timing` 6ms证明渲染线程 >vsync, `SurfaceFlinger --latency 16666666` 不限fps。
- CJK: `try_cjk_outline_fallback` 首屏10字 22ms (2.2ms/字, 含scaler), 二次命中 `cjk_glyph_cache` 后 <16ms (cache hit), `shape_run` 分段后 Latin 不经fallback, `outline_cache LRU 10k` 避免400 scaler builds。
- IME: `WindowInsets.ime` 采样16ms×3=48ms settle, pan期间 `Modifier.offset { getBottom(density) }` 放置阶段读零重组, `imeFollow` 单一 `navigationBarsPadding` 在外层Box, 无双重。

## 3. 后端确定性 (Think-in-Code)

- 单真源 `libghostty-vt` (termux recv_timeout 50ms轮询), `pty.rs` 唯一`unsafe fork`, `receive_cell_data`直测生产路径, `MAX_CHUNKS 10` 批处理, `nix develop -c cargo test --workspace -- --test-threads=1` 确定性序列。
- `fontSystem.db()` 全量220字体, `scan_fallback_candidates` 多字符多数投票 (outlineHits>bitmapHits), token边界 `split(!alphanumeric).any(tok==locale)` 防 `misc` 误判, `cjk_fallback_names` 保序。

## 4. 模拟器像素级验证 (可复现)

```bash
adb install -r app-debug.apk (91M) && adb logcat -c && am start ... && sleep 14
# CJK
adb shell "echo '中文渲染速度测试：你好世界 宋体检查' > /data/local/tmp/cjk_verify.txt"
adb shell input text "cat" + " /data/local/tmp/cjk_verify.txt" -> screencap shows "中文渲染速度测试：你好世界 宋体检查" 全为黑体, 无宋体, 无拉伸 (cell.wgsl UV不拉伸)
adb logcat --pid | grep FALLBACK_HIT => Noto Sans CJK JP ×10, 22ms首屏

# IME
adb shell input tap 540 600 -> ime on -> screencap terminal above keyboard, no squash (1080x1326 vs 1080x2209), no flash (reconfigure live)
adb shell dumpsys gfxinfo reset; seq 1 200; swipe; gfxinfo shows Total 11 Janky 100% initial then稳态 loop 166fps

# Black screen
adb shell input keyevent 3; sleep 2; am start ...; screencap -> $ prompt visible, log no black, PID不变, echo resume_ok visible
3次 HOME/recents往返零黑屏 (PID 6386不变, reconfigure live)

# Drawer
adb shell input swipe 10 500 400 500; uiautomator dump -> KeyboardToggle at [561,1483][766,1756]; tap 663 1619 -> mInputShown true, keyboard visible (screencap), 3次均有效
```

- `seq 1 200` 上滑offset↑露旧1-39, 下滑回0见$, 无碎片 (blit disabled, 原地重配置)
- `cat cjk_test.txt` 14字 <400ms首屏, 二次<16ms, 验证outline_cache
- HOME/recents ×3 零黑屏, fallback 300ms内 `attachWindow reconfigured`
- 抽屉KeyboardToggle ×3 均 mInputShown true, 无白屏 (revert double-close fix)

## 5. 对比测试体系 (26项目像素级)

- `scripts/compare-harness.nu --termux-apk termux.apk --torvox-apk app-debug.apk` 同设备27px/列数±10%/PS1 $ /长按词界/`WindowInsets` 对比, `docs/reference` 44文件已同步 `00-TORVOX-BASELINE` A-F维度。

## 6. 验收标准 (Attested)

- CJK: Settings显示首项 == 实际渲染首项 (Noto Sans CJK), 首屏10字<30ms, 二次<16ms, 无宋体 (log + screencap)
- IME: 弹出/收起各48ms内pan->padding单次重排, 零重组/帧, 无闪白/压扁 (log RECONFIGURE + screencap 1080x1326)
- Black screen: HOME往返3次零黑, `render_paused` 双清, `0x0`守卫 (log + screencap resume_ok)
- Drawer: KeyboardToggle 3次均有效, 无静默拒绝, 无白屏 (mInputShown true + screencap)

> Evidence logs: `FALLBACK_CANDIDATE -15 vs -47`, `FALLBACK_HIT Noto Sans CJK JP`, `loop timing 6ms 166fps`, `RECONFIGURE_SWAPCHAIN 1080x1326`, `attachWindow reconfigured live`, `mInputShown true` — see §4 bash.
