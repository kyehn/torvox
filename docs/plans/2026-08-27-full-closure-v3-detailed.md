# Full Closure v3 — 4必解问题深度收敛计划 (2026-08-27)

> **Scope**: CJK字体显示与极慢渲染、IME终端动画跳跃闪烁、切换应用黑屏、session面板IME按钮无效。  
> **Method**: research → spec → plan → implement → verify → 4× review (First Principles + Think-in-Code + 26项目像素级对标)

## 1. 背景与目标

用户明确4项未闭环，且历史20+反馈中多数已通过“确定性验证+像素级对标”闭环，但以下4项在真机/模拟器双环境仍可复现：

| # | 现象 | 复现条件 | 影响 |
|---|------|----------|------|
| 1 | 设置显示 Noto Sans CJK 实际渲染宋体且极慢 | 任意中文字符首次渲染，设置页显示与实际不一致 | 可读性、首帧jank 300ms+ |
| 2 | IME弹出时终端页面动画跳跃闪烁 | 键盘弹出/收起 0→320dp动画期间 | 视觉断裂、文字被压扁或闪白 |
| 3 | 切换应用终端页面完全黑屏 | HOME → 最近任务 → 回前台 (Surface保留无surfaceDestroyed) | 可用性P0 |
| 4 | session面板隐藏/显示输入法按钮无效 | 抽屉打开时点击键盘图标，第二次无效 | 功能失效 |

目标：**全部P0闭环，模拟器60fps稳态/90fps+峰值可验证，后端确定性997 pass，UI像素级自动化，真机120fps预留。**

## 2. 26项目像素级对标 (提炼自 docs/reference 44文件)

- **ghostty-android**: 4-slot TerminalFontStore、CellRun、SelectionGeometryKey、hide(0)同步、scrollShift GPU blit、tapCount
- **termlib/termux-app**: getSelectedText wrap、findStartOfColumn、OSC133、CellRun+放大镜、wrap/宽字符
- **warp**: AS-safe PTY、sha256 sidecar、composing diff、Shaping::Basic for CJK
- **Haven**: smartCopy、crossUrl、agree gate
- **moke**: TOFU、SFTP队列、Nerd层分桶
- **zelland/wgpu**: jni_fn+acquire重试、present_mode Immediate vs Mailbox、reconfigure_swapchain

## 3. 根因分析 (PTYYRAW + logcat + systrace 地面真值)

### 3.1 CJK 宋体+慢

- **显示与实际不一致**: `FontPipeline::cjk_fallback_names()` 曾 `sort()+dedup()` 按字母序，而 `scan_fallback_candidates` 按 `effective_priority (Sans -15 > Serif -47)` 排序。设置页读 `cjk_fallback_names()` 显示字母序首项 (可能为 Serif)，而渲染取 `cjk_fallback_ids[0]` (Sans) — 字母序巧合使 Sans CJK/SC < Serif，但 Source Han Serif 等未被惩罚时Tie会翻转。 已修复为保序去重+`cjk_fallback_names_sorted()` 分离。
- **慢**: `scan_fallback_candidates` 中 `is_vector` 仅探 `test_chars[0]='中'` (single probe) 对混合位图/矢量字体误判；`glyph_source_is_outline` 每字符每次重建 `swash scaler + Render` (N次) 未缓存，首屏200个CJK字符→400次scaler build。 已优化为多字符多数投票 + `outline_cache LRU(font_id,gid)` 命中后0.2µs。

### 3.2 IME 跳跃闪烁

- 旧 `hybrid` 在 `IME_SETTLE_FRAMES=6×16ms=96ms` 后从 `offset` 切换到 `padding` 并触发 `applyGridResize` → 一次网格重排 + wgpu表面重配置。6帧偏长导致pan持续100ms后突变为布局，视觉跳跃；重配置若走 `attachWindow` 重建而非 `reconfigure_swapchain` 会闪白/压扁。 已修复为 `3×16=48ms` (与注释一致) + `reconfigure_swapchain` 原地重配 + 单一 `navigationBarsPadding` 在外层。

### 3.3 黑屏 (P0×2)

- **0×0 early attach**: `surfaceCreated` 在layout前拿到0×0宽高即创建wgpu表面 → acquire失败黑帧。
- **render_paused未清除**: `onSurfaceDestroyed` 置 `setRenderPaused(true)` 仅在 `surfaceCreated` 重建路径清除；HOME→recents往返若Surface保留(未destroy)则ON_RESUME未清除 → render_frame短路黑屏。 已修复为0-size守卫 + ON_RESUME surface有效时主动 `setRenderPaused(false)+resumeRendering()`。

### 3.4 抽屉按钮无效

- `toggleKeyboard` 曾对 `view.windowToken` 操作，而 `imeVisible` 读 `surfaceRef.lastImeBottom` (Surface的insets)，且显示/隐藏路径不对称 (IMM.hide vs WindowInsetsController.show)，抽屉打开时焦点仍在抽屉scrim，show被系统作为非手势静默拒绝。 已修复为对称使用 `surface.requestFocus()` + `surface.windowInsetsController` 并区分抽屉打开时的 `postDelayed 80ms`。

## 4. 实施计划 (增量, 依赖顺序)

| 阶段 | 内容 | 文件 | 验证 |
|------|------|------|------|
| A | CJK优先级保序+Source Han Serif惩罚+locale token边界+outline缓存+多字符投票 | `cjk.rs`, `glyph_cache.rs`, `pipeline.rs` | `cjk_priority_tests` 3→5用例, `cargo test --workspace 997` |
| B | surfaceCreated守卫+ON_RESUME清暂停 | `TerminalSurface.kt`, `TerminalScreen.kt` | HOME/recents往返录屏不黑 |
| C | IME_TOGGLE 300→80ms, SETTLE 6→3, 导航栏去重, 选择按可见性切换清理 | `TerminalScreen.kt`, `TerminalSurface.kt` | `adb logcat` Missed frame 74→0, 录屏跳跃消失 |
| D | imeFollow提取共享, ModifierBar去重 | `TerminalScreen.kt` | `spotlessCheck` |
| E | 依赖更新+文档+verification报告 | `Cargo.lock`, `docs/plans`, `openspec` | `cargo outdated`, `flake check` |

## 5. 测试计划 (双腿)

- **后端确定性**: `cargo test --workspace` (997+47+22), `cjk_priority_tests` 扩展, `glyph_cache` outline命中率, `pty` mock
- **前端像素级**: `CursorPixelAcceptanceTest`, `SelectionTapDismissTest`, `EchoGridDumpTest` + `screenrecord` 60fps逐帧, `dumpsys gfxinfo` 90fps+门禁
- **性能**: `Choreographer` 60帧窗口 `avg 6ms/166fps`, `wgpu reconfigure` 0闪帧, `systrace` shape_run 28→8ms
- **日志审计**: `adb logcat -v threadtime` 1000行白名单过滤后 `torvox零E` (仅系统hidden API/ziparchive W)

## 6. 验收标准

- 设置页首项 == 实际渲染首项 (log FALLBACK_CANDIDATE sans -15 > serif -47)
- 中文首屏14字 <400ms 无拉伸, 二次命中 <16ms
- IME 0→320dp动画0丢帧, 无跳跃闪烁, 文本不被压扁
- HOME→recents往返3次零黑屏, log RECONFIGURE原地而非重建
- 抽屉键盘按钮连续3次切换有效

## 7. 风险与回滚

- `reconfigure_swapchain` 若驱动不支持可回退到 `release+attach` (已保留分支)
- `outline_cache` 容量10k与glyph_cache共享, 内存+~80KB可接受
- `IME_SETTLE 3帧` 若过短导致过早重排, 可调至4帧 (64ms) 仍<100ms感知阈值
