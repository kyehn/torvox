# cjk-rendering Specification

## Purpose

CJK 字形在终端中的渲染质量与输入链路要求。记录设备取证结论、根因与已验证的实现细节，作为后续渲染/输入工作的参考（置信度低于 docs/specification/DESIGN.md）。

## Requirements

### Requirement: CJK 回退字体遵循系统 fonts.xml

CJK fallback MUST 遵循 Android fonts.xml：zh-Hans 链首选 `NotoSansCJK-Regular.ttc`
index=2（Noto Sans CJK SC），MUST NOT 回退到 Serif/JP。`FontPipeline::find_cjk_fallback_fonts`
按 fonts.xml 顺序匹配，首个命中即用。

#### Scenario: 简体中文环境选中 CJK SC

- **WHEN** 系统 locale 为 zh-Hans 且需要 CJK 字形回退
- **THEN** 选中 Noto Sans CJK SC，不使用 Noto Serif 或 Noto Sans CJK JP

### Requirement: CJK 字形按位图物理像素 1:1 取样

CJK 字形 SHALL 按 atlas 位图物理像素 1:1 采样，MUST NOT 按
`glyph_advance / quad_size` 比率缩放（等宽西文 advance==cell 时比率恒为 1.0，
掩盖了公式错误；CJK quad 与 advance 不等会导致水平拉伸约 1.197x）。

#### Scenario: 中英文混排字形大小协调

- **WHEN** 同一终端同时显示西文与 CJK 字形
- **THEN** CJK 笔画不增粗，中/文/测/试宽度与 FreeType 无 hint 参考一致（比率 1.000）

### Requirement: IME 提交的 CJK 文本首帧可见

IME commitText 经 TerminalInputEncoder UTF-8 编码、InputBatchBuffer 单线程发送、
bridge.writeToPty 进入 PTY 后，提交的 CJK 文本 MUST 在首帧渲染可见，无需额外点击。
退格按码点 1:1（CJK 一次一个汉字）。

#### Scenario: 拼音提交后立即显示

- **WHEN** 冷启动后一次点击开键盘，经拼音 SPACE 提交“你好”
- **THEN** 终端立即显示“你好”，无需额外点击
