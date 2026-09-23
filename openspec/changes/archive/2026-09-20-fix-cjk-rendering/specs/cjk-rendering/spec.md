# cjk-rendering — Spec

## 概述

CJK 字形在终端中的渲染质量与输入链路要求。本 spec 记录设备取证结论、根因与已验证的实现
细节，作为后续渲染/输入工作的参考（置信度低于 docs/specification/DESIGN.md）。

## 能力

`cjk-rendering`: CJK 字形光栅、取样与 IME 输入链路的行为要求。

### 字体选择

- CJK fallback 遵循 Android fonts.xml：zh-Hans 链 → `NotoSansCJK-Regular.ttc` index=2
  （Noto Sans CJK SC），不回退到 Serif/JP。
- `FontPipeline::find_cjk_fallback_fonts` 按 fonts.xml 顺序匹配，首个命中即用。

### 字形取样 1:1（发虚修复）

- SHALL：CJK 字形按 atlas 位图物理像素 1:1 采样，不得按
  `glyph_advance / quad_size` 比率缩放。
- 原因：等宽西文 advance==cell 时比率恒为 1.0，掩盖了公式错误；CJK quad（2 cell=44px）
  与 advance（≈36.75px）不等 → 每字形水平拉伸 1.197x → 笔画增粗、与英文大小不协调。
- 实现：`native/shaders/cell.wgsl` 中 `scaled_x = cell_px.x`，`in_glyph` 按
  `glyph_size_px`（位图物理尺寸）裁剪；quad 其余区域走背景分支。垂直方向本就 1:1。
- 验证：修复后 中/文/测/试 宽度与 FreeType 36.75px 无 hint 参考一致（比率 1.000）。

### IME 输入链

- 输入触发：IME commitText → TerminalInputEncoder UTF-8 → InputBatchBuffer 单线程发送
  线程 → bridge.writeToPty → PTY。退格按码点 1:1（CJK 一次一个汉字）。
- 首帧可见性：commit 后首帧渲染（Choreographer vsync 链常活 + renderSignaled 唤醒门 +
  渲染线程无条件 render）；新字形首帧 atlas 上传（pass.rs take_dirty_rect+upload_atlas）
  已保证不空白。真机验证：冷启动 → 一次点击开键盘 → 拼音 → SPACE 提交 → 终端立即显示
  “你好”，无需额外点击。
- 点击行为：onSingleTapUp 清空 suppressUntilNanos、请求键盘；50ms SUPPRESS_GRACE 仅覆盖
  窗口焦点变化防抖，不吞常规首输入。
- 组词预览：AOSP LatinIME 拼音把组词保留在自身候选条（不回调 setComposingText，
  adb 注入事件下实测），提交后文本立即可见 —— 与 Termux 行为一致。

### 性能

- CJK 字形经 `cjk_glyph_cache` 缓存（char→(font_id,gid)），首用光栅一次，后续帧与西文
  同路径（同一 atlas / 同一 quad 管线）。稳态帧成本与西文基本一致；首帧成本仅为字体
  fallback 解析 + 单字形光栅，一次摊销。
