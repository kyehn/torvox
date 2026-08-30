# 详细实施计划: reference-adoption-v6

> 日期: 2026-08-30 | 基于 26 参考项目深度研究

## 概述

基于 `docs/reference/` 下 26 个参考项目的研究，实施 4 项高价值功能采纳。

## 阶段 1: 鼠标编码 (P0, ~2 天)

### 目标
实现标准 SGR/1006 鼠标事件编码，支持 vim/htop/mc 等终端应用的鼠标交互。

### 实施步骤

1. **Rust 模块创建** (`native/src/terminal/mouse_encoder.rs`)
   - 定义 `MouseMode` 枚举（None/X10/Button/Any/SGR）
   - 定义 `MouseAction` 枚举（Press/Release/Drag/ScrollUp/ScrollDown）
   - 定义 `MouseButton` 枚举（Left/Right/Middle/None）
   - 实现 `MouseEncoder` 结构体，封装 `ghostty_mouse_encoder` C API
   - 实现 `encode(x, y, action, button, cell_w, cell_h) -> Option<Vec<u8>>`
   - Mode gate: 非 mouse mode 时返回 None

2. **JNI 扩展** (`ffi.rs`)
   - 新增 `sendMouseEvent(x: f32, y: f32, action: i32, button: i32)`
   - 调用 `Session::send_mouse_event()`

3. **Kotlin 集成**
   - `NativeBridge.kt`: 新增 `external fun sendMouseEvent(x: Float, y: Float, action: Int, button: Int)`
   - `TerminalSurface.kt`: 在 mouse mode 激活时，将触摸/滚动事件转发到 mouse encoder

4. **测试**
   - Rust 单测: mode gate (8 tests)、SGR encoding (5 tests)、bounds clamping (3 tests)
   - 模拟器集成测试: `adb shell input mouse` 注入验证

### 验收标准
- [ ] vim 在 mouse mode 下响应鼠标点击
- [ ] 非 mouse mode 时不发送鼠标事件
- [ ] 所有现有测试通过

## 阶段 2: 无障碍朗读 (P0, ~1 天)

### 目标
启用 TalkBack 屏幕阅读器读取终端内容。

### 实施步骤

1. **可见文本提取** (`TerminalRuntime.kt`)
   - 新增 `extractVisibleText()` 方法
   - 从 scrollback + viewport 提取当前可见文本

2. **contentDescription 更新**
   - `TerminalSurface.kt`: 在 `onFrameRendered` 回调中更新 `contentDescription`
   - 仅在内容变化时更新（diff check）

3. **事件 announce**
   - bell 事件: `announceForAccessibility("Bell")`
   - 标题变化: `announceForAccessibility("Title: $newTitle")`

4. **测试**
   - Robolectric: 断言 contentDescription 随输出更新
   - 模拟器: TalkBack 手动验证

### 验收标准
- [ ] TalkBack 朗读当前可见终端内容
- [ ] bell 事件触发 announce
- [ ] 性能: contentDescription 更新不阻塞渲染

## 阶段 3: OSC 133 语义段 (P1, ~2 天)

### 目标
实现 shell integration 的命令输出提取。

### 实施步骤

1. **数据结构** (`output_processor.rs`)
   - 定义 `SemanticSegment { start_col, end_col, segment_type, exit_code }`
   - 定义 `SemanticType { Prompt, CommandInput, CommandOutput, CommandFinished }`

2. **OSC 133 解析**
   - 扩展 `handle_osc133()` 支持 A/B/C/D 事件
   - 按当前 cursor 列记录段起始/结束

3. **JNI 扩展**
   - 新增 `getLastCommandOutput() -> String`
   - 返回最近 `CommandFinished` 之前的 `CommandOutput` 段文本

4. **测试**
   - Rust 单测: OSC 133 序列 → 段生成
   - Kotlin 单测: getLastCommandOutput 返回正确内容

### 验收标准
- [ ] `getLastCommandOutput` 返回正确输出
- [ ] 多行命令正确处理
- [ ] 所有现有测试通过

## 阶段 4: CellRun 游程编码 (P1, ~1 天)

### 目标
减少 JNI 往返，同格式连续 cell 合并传输。

### 实施步骤

1. **CellRun 结构** (`cell_builder.rs`)
   - 定义 `CellRun { start_col, length, fg_color, bg_color, flags }`

2. **游程检测**
   - `build_row_runs()`: 行内检测连续相同格式 cell
   - 合并为 CellRun

3. **增量渲染集成**
   - 在 `build_row_instances_into()` 增量路径中使用 CellRun
   - 减少 CellData 条目数

4. **测试**
   - Rust 单测: 同格式文本 → 游程数减少
   - Benchmark: JNI 调用次数对比

### 验收标准
- [ ] 游程数 < 原始 cell 数（同格式文本）
- [ ] 所有现有测试通过
- [ ] 性能无回退

## 风险与缓解

| 风险 | 影响 | 缓解 |
|------|------|------|
| libghostty-vt mouse encoder API 不完整 | 高 | 检查 bindings.rs，必要时手写编码 |
| TalkBack 性能影响 | 中 | 仅在内容变化时更新 |
| OSC 133 解析错误 | 低 | 非致命错误，日志记录 |
| CellRun 增量路径兼容性 | 中 | 保留原有路径作为 fallback |
