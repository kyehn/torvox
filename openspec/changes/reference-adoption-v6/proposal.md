# Proposal: reference-adoption-v6 — 26 参考项目精华采纳

## Why

基于 `docs/reference/` 下 26 个参考项目的深度研究，识别出 4 项高价值功能缺口与多项工程改进。这些功能在 ghostty-android、zelland、warp、termlib 等成熟项目中已验证，torvox 当前缺失或实现不完整。

核心差距：
1. **鼠标编码**（P0）：vim/htop/mouse-mode 终端应用的鼠标支持完全缺失，libghostty-vt 的 `ghostty_mouse_encoder` API 已暴露未使用
2. **无障碍朗读**（P0）：TalkBack 无法读取终端内容，SurfaceView 自绘无文本节点
3. **OSC 133 语义段**（P1）：shell integration 的命令输出提取（getLastCommandOutput）是 termux/termlib 标准功能
4. **CellRun 游程编码**（P1）：减少 JNI 往返，同格式连续 cell 合并传输

## What Changes

### 1. `mouse-encoding` — 标准鼠标事件编码（P0）

**来源**：zelland `terminal.rs:41-175`（ghostty_mouse_encoder C FFI）、termlib `MouseModeTracker`

**实现**：
- Rust 侧：`native/src/terminal/mouse_encoder.rs` 新模块
  - `encode_mouse_event(x, y, action, button, cell_w, cell_h)` → `Option<Vec<u8>>`
  - 使用 `ghostty_mouse_encoder_new` + `setopt_from_terminal` + `encode`
  - mouse mode 门控：非 1000/1002/1003 模式时返回 None
- JNI 新增：`sendMouseEvent(x, y, action, button)`
- Kotlin 侧：`TerminalSurface.kt` 在 mouse mode 激活时发送滚动/点击事件

**测试**：
- Rust 单测：mode 门控（8 测试）、SGR 序列生成、越界 clamp
- Kotlin 单测：MouseModeTracker 已有（8 测试）+ 集成测试
- 模拟器验证：`adb shell input mouse` 注入 → PTY 收到 SGR 序列

### 2. `a11y-overlay` — 无障碍朗读支持（P0）

**来源**：termlib `AccessibilityOverlay.kt`

**实现**：
- `TerminalSurface.kt`：设置 `contentDescription` 为当前可见文本快照
- 关键事件 announce：bell、标题变化时 `announceForAccessibility`
- 性能：仅在内容变化时更新（帧级 `onFrameRendered` 回调）

**测试**：
- Robolectric：断言 `contentDescription` 随输出更新
- 模拟器：TalkBack 开启 → 朗读验证

### 3. `osc133-semantic` — OSC 133 语义段 + getLastCommandOutput（P1）

**来源**：termlib `SemanticType.kt` + `OscParser.kt:275-340`

**实现**：
- Rust 侧：`output_processor.rs` 维护 `Vec<SemanticSegment>`（行→段列表）
  - A/B/C/D 事件按当前 cursor 列记录
  - D 时记录 exit code
- JNI 新增：`getLastCommandOutput()` → 返回最近完成命令的输出文本
- 测试：OSC 133 序列 → 段生成 → getLastCommandOutput 内容断言

### 4. `cell-run-cache` — CellRun 游程编码（P1）

**来源**：termlib `CellRun.kt:25-74`

**实现**：
- `cell_builder.rs`：行内检测连续相同格式 cell → 合并游程
- CellData 结构支持"同格式游程"标记
- 减少 CellData 条目数（同格式连续 cell 共享样式）

**测试**：
- Rust 单测：格式相同连续文本 → 游程数减少断言
- 性能：JNI 往返次数减少 benchmark

## Capabilities

### New Capabilities
- `mouse-encoding` — 标准 SGR/1006/X10 鼠标事件编码
- `a11y-overlay` — TalkBack 终端内容朗读
- `osc133-get-last-output` — shell integration 命令输出提取
- `cell-run-encoding` — 同格式 cell 游程合并

### Modified Capabilities
- `terminal-input` — 增加鼠标事件路径
- `terminal-render` — 增加 CellRun 优化路径

## Acceptance Criteria

| ID | Criterion | Verification |
|----|-----------|-------------|
| C1 | vim/htop 鼠标点击在 mouse mode 1000 下工作 | 模拟器 + `adb shell input mouse` |
| C2 | 非 mouse mode 时鼠标事件不发送 | Rust 单测 mode gate |
| C3 | TalkBack 朗读当前可见终端内容 | 模拟器手动验证 |
| C4 | `getLastCommandOutput` 返回正确输出 | Rust + Kotlin 单测 |
| C5 | CellRun 游程数 < 原始 cell 数（同格式文本） | Rust 单测断言 |
| C6 | 所有现有测试通过 | `cargo test` + `gradle test` |
