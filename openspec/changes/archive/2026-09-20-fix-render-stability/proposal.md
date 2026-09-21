## Why

设备取证确认三类渲染缺陷：
1. **斜体字形缺失**：`italic_test.txt` 首行 16 字符（abcdefghijklmnop）渲染时仅 3-5 个可见（OCR `a cd` + `i l`），首次渲染缺失约 70%，重复执行后完整 —— 疑似 atlas 字形首次上传/缓存未命中或渲染触发时序问题。DroidSansMono 无 italic face，走合成 shear 路径，与 shader `glyph_advance_w` 裁剪上限交互异常。
2. **IME 弹出/隐藏期间文本不可见**：输入法弹出时新输出（红色 error、斜体）不可见，隐藏后出现，滑动后部分出现 —— 疑似 `setRenderPaused(true)` 暂停渲染与 IME 动画/pan 链路竞态，恢复后未强制重绘新输出。
3. **滚动卡顿/撕裂与启动黑屏慢**：渲染循环 vsync 门控与新输出消费时序需核查。

## What Changes

- 修复斜体字形首次渲染缺失（atlas 上传/缓存/渲染触发）。
- 修复 IME 弹出/隐藏期间渲染暂停恢复竞态（确保新输出强制呈现）。
- 核查滚动卡顿/撕裂与启动黑屏慢的根因并修复。

## Capabilities

### New Capabilities

- `render-stability`: 字形首帧完整性、IME 期间渲染恢复、滚动流畅度。

### Modified Capabilities

## Impact

- 影响字体渲染（native `render/font/*`）、渲染循环（`TerminalRuntime` render thread）、IME 处理（`TerminalSurface` pan/pause）。
