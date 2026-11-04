# cjk-rendering Specification

## Purpose

CJK 字形在终端中的渲染质量与输入链路要求。本文件是 2026-09-20 那次变更的能力
快照（归档件），记录设备取证结论、根因与已验证的实现细节，作为后续渲染/输入工作的
参考（置信度低于 docs/specification/DESIGN.md）；当前生效的规范见
`openspec/specs/cjk-rendering/spec.md`。

## Requirements

### Requirement: CJK 回退字体按系统字体链选择

CJK fallback MUST 遵循 Android fonts.xml：zh-Hans 链 → `NotoSansCJK-Regular.ttc`
index=2（Noto Sans CJK SC），MUST NOT 回退到 Serif/JP。`FontPipeline::find_cjk_fallback_fonts`
MUST 按 fonts.xml 顺序匹配，首个命中即用。

#### Scenario: 首个命中即采用

- **WHEN** 系统字体链中存在多个可解析汉字的面
- **THEN** 取 fonts.xml 顺序里的首个命中面，MUST NOT 继续向后挑选

### Requirement: CJK 字形按物理像素 1:1 取样

CJK 字形 MUST 按 atlas 位图物理像素 1:1 采样，MUST NOT 按 `glyph_advance / quad_size`
比率缩放。

原因：等宽西文 advance==cell 时比率恒为 1.0，掩盖了公式错误；CJK quad（2 cell=44px）
与 advance（≈36.75px）不等 → 每字形水平拉伸 1.197x → 笔画增粗、与英文大小不协调。

实现：`native/shaders/cell.wgsl` 中 `scaled_x = cell_px.x`，`in_glyph` 按
`glyph_size_px`（位图物理尺寸）裁剪；quad 其余区域走背景分支。垂直方向本就 1:1。

验证：修复后 中/文/测/试 宽度与 FreeType 36.75px 无 hint 参考一致（比率 1.000）。

#### Scenario: 汉字宽度与参考渲染一致

- **WHEN** 渲染中/文/测/试
- **THEN** 各字形宽度与 FreeType 无 hint 参考一致（比率 1.000），MUST NOT 拉伸

### Requirement: IME 输入链按码点送达

IME 提交 MUST 经 `commitText` → `TerminalInputEncoder`（UTF-8）→ `InputBatchBuffer`
单线程发送线程 → `bridge.writeToPty` → PTY。退格 MUST 按码点 1:1（CJK 一次一个汉字）。

首帧可见性：commit 后首帧渲染（Choreographer vsync 链常活 + renderSignaled 唤醒门 +
渲染线程无条件 render）；新字形首帧 atlas 上传（pass.rs take_dirty_rect+upload_atlas）
已保证不空白。真机验证：冷启动 → 一次点击开键盘 → 拼音 → SPACE 提交 → 终端立即显示
“你好”，无需额外点击。

点击行为：`onSingleTapUp` 清空 `suppressUntilNanos`、请求键盘；50ms SUPPRESS_GRACE
仅覆盖窗口焦点变化防抖，MUST NOT 吞常规首输入。

组词预览：AOSP LatinIME 拼音把组词保留在自身候选条（不回调 `setComposingText`，
adb 注入事件下实测），提交后文本立即可见 —— 与 Termux 行为一致。

#### Scenario: 提交后首帧即见文本

- **WHEN** 冷启动后点击开键盘并用拼音提交一次
- **THEN** 终端立即显示提交文本，MUST NOT 需要额外点击

#### Scenario: 退格一次退一个汉字

- **WHEN** 退格键作用于一个已提交的汉字
- **THEN** 该汉字被一次退格删除，MUST NOT 需要两次

### Requirement: CJK 字形渲染成本与西文同档

CJK 字形 MUST 经 `cjk_glyph_cache` 缓存（char→(font_id,gid)），首用光栅一次，后续帧
与西文同路径（同一 atlas / 同一 quad 管线）。稳态帧成本 MUST 与西文基本一致；首帧
成本仅限字体 fallback 解析与单字形光栅，一次摊销。

#### Scenario: 稳态帧成本与西文同档

- **WHEN** 同一汉字在后续帧被重复渲染
- **THEN** 该帧成本与渲染西文基本一致，MUST NOT 每次重新光栅
