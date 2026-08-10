# Round 230 — §3.7 UX/外圍 Gap Analysis

> 日期：2026-08-10 | 基准：round-229 (577279bc)

## 1. 用户范围（排除项）

### §3.7 明确排除
- 隐私黑屏覆盖
- 标签条/顶部栏（保持侧边栏）
- tmux 集成 / SSH 客户端 / SFTP 断点续传 / mosh
- 保存会话
- Split Panes 分屏 / Block 模型 / AI 集成
- 悬浮窗终端 / 开机脚本
- proot 发行版（bootstrap 范畴）
- 日志写入文件

### P0 全部排除
- bootstrap sha256检查、MCP同意弹窗、OpenGL、隐私黑屏

### P1 全部排除
- 会话保存、jni_fn、SSH+TOFU、输出导出到文件、粘贴确认对话框

## 2. §3.7 待实现项分析

| # | 功能 | 现状 | 工作量 | 参考 |
|---|------|------|--------|------|
| 1 | 响铃防抖 | ✅ BellHandler 150ms debounce + 4 modes 存在 | **小** — Samsung NPE guard around vibrate() call + dispose 生命周期 | bell/BellHandler.kt |
| 2 | logPrivate | ⚠️ LogUtil 4068B 分块完成，但无 private API | **小** — 添加 `logPrivate()` debug-only 方法 | research-termux-kotlin.md §7.3 |
| 3 | 新输出复位滚动位置 | ❌ 缺失 — 用户滚动浏览历史时新输出不回到底部 | **中** — VT loop new output → auto-reset scrollOffset | research-small-repos.md §4.5 |
| 4 | 设置UI enabledWhen门控 | ❌ 用 `if` 条件渲染代替 disabled，用户无法感知依赖关系 | **中** — SettingsRow 组件添加 `enabled` 参数，灰化而非隐藏 | research-ghostty-android-extra.md §6.2 |
| 5 | 键盘弹出"屏内滚动不缩字号" | ✅ adjustNothing + grid resize（行数减少但字号不变）| **N/A** — 已满足"不缩字号"需求，grid resize 是最佳方案 | docs/lessons/07-ime-pixel-stable.md |
| 6 | 节流持久化 | N/A — 无此设置项 | **跳过** | research-small-repos.md §4.5 |
| 7 | OSC 9;4 进度环 | ❌ OSC 9 notification 完整，但 9;4 进度完全缺失 | **大** — Rust osc_handler.rs + OutputSnapshot + PollEvent + UI | research-ghostty-android-extra.md §5-5 |
| 8 | 快捷键录制 | ❌ 无任何快捷键录制基础设施 | **大** — ShortcutBinding + CaptureDialog + KeyShortcutHandler 三件套 | research-mid-repos-b.md §5.1 |
| 9 | 自定义主题编辑器 | ⚠️ 主题选择器+保存/删除完成，但无颜色编辑 | **大** — ColorPickerDialog + working-copy + dirty 模型 | research-ghostty-android-extra.md §5-P2 |

## 3. 实施优先级

### Phase 1 — 快速胜利（~2h）
1. **T1 响铃防抖**: vibrate() try/catch + bellHandler.dispose() 生命周期
2. **T2 logPrivate**: LogUtil 添加 logPrivate/debugSensitive 方法

### Phase 2 — 核心UX（~4h）
3. **T3 新输出复位滚动**: VT thread → event channel → Kotlin auto-scroll
4. **T4 设置enabledWhen**: SettingsRow/SliderRow 添加 enabled 参数

### Phase 3 — 复杂功能（~8h）
5. **T5 OSC 9;4**: Rust parse → OutputSnapshot → JNI poll → Kotlin NotificationBar
6. **T6 快捷键录制**: 3 个新 Kotlin 文件 + settings UI
7. **T7 主题编辑器**: ColorPickerDialog + working-copy + dirty 模型

## 4. 技术设计要点

### T3 新输出复位滚动
- 路径: `internal.rs` VT loop `Command::Write` 处理 → 新增 `is_new_data` flag → `ffi.rs` poll event → Kotlin `TerminalRuntime` → `scrollOffset = 0` + `bridge.setScrollOffset(0)`
- 参考: cpmdroid `TerminalView.kt:530-540` (`userScrollUp = 0`)
- 边界: 仅当用户处于滚动浏览状态时复位；实时屏底部时无需操作

### T4 enabledWhen
- `SettingsComponents.kt` 四个 Row 组件添加 `enabled: Boolean = true`
- `enabled = false` 时: Switch 不响应点击, Slider 不响应滑动, Selector 灰化
- 不改变条件渲染逻辑（仍然可以 hide），disabled 状态用于逻辑上应该存在但暂不可交互的控件
- 受影响: cursorSpeed ↔ cursorBlink, bgBlur/alpha ↔ bgImage, day/night ↔ themeMode

### T5 OSC 9;4
- Rust: `osc_handler.rs` 添加 `OscEvent::Progress(ProgressEvent { state: u8, value: u8 })`
- session: `OutputSnapshot.progress` → `poll_progress()` → JNI `poll_progress`
- Kotlin: `PollEvent.Progress` + 进度条 UI（可选，默认 notification）
- 状态: 0=remove, 1=indeterminate, 2=normal, 3=error, 4=indeterminate(error)

### T6 快捷键录制
- `ShortcutBinding.kt`: data class (ctrl/shift/alt/ctrl+shift/etc + keyCode), serialize "CTRL|SHIFT|54"
- `ShortcutCaptureDialog.kt`: Compose Dialog, onPreviewKeyEvent 捕获组合键
- `KeyShortcutHandler.kt`: dispatch 快捷键到现有 actions (paste/new_session/close/switch)
- 与现有 `handleLayoutAwareHardwareKey` 集成

### T7 主题编辑器
- `ThemeEditor.kt`: working-copy of TerminalTheme, dirty flag
- `ColorPickerDialog.kt`: Compose Dialog, SV field + hue bar + hex input
- `updating[]` 防回环守卫（参考 ghostty-android ThemeActivity）
- 保存: 写入 UserThemeStore, 可选覆盖已有

## 5. 不需要实现的确认
| 项 | 原因 |
|---|---|
| 键盘弹出缩字号 | adjustNothing + grid resize 已实现，字号不变 |
| 节流持久化 | 无此设置项，N/A |
