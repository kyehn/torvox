# cursor-rendering Specification

## Purpose

光标几何。锚点为 termux 的 `TerminalRenderer.java`（`docs/specification/DESIGN.md` 要求
光标「高度、宽度等均参考 Termux」）：其光标矩形为
`canvas.drawRect(left, y - cursorHeight, right, y, mTextPaint)`，其中
`cursorHeight = mFontLineSpacingAndAscent - mFontAscent`、`mFontLineSpacingAndAscent =
mFontLineSpacing + mFontAscent`（`mFontAscent` 为负）⇒ 光标高度恰为 `mFontLineSpacing`
（行高），矩形即整格。

实现：`native/src/render/cell_builder.rs` 的 `append_row_instances`——光标盒就是该函数的
默认 `quad_origin` / `quad_size`，Block 分支只把背景换成光标色；Bar/Underline 在单元盒内
收缩为细标记（空单元格收缩那唯一的四边形，文本格在字形之后追加一个标记四边形），
同样不读字形位图。

## Requirements

### Requirement: 方块光标按整格几何

方块光标 MUST 是整格矩形：宽 = 单元宽 × 字形列跨度，高 = 单元高，顶边 = 单元顶边。
MUST NOT 跟随格内字形位图定尺寸——位图高度随字符变化，同一光标会随内容变高变矮、
越过行界；空单元格取 `'M'` 位图、文本单元格取该字形位图时两者还不相等。
宽度 MUST NOT 用字形 advance 推算（advance 与单元宽不等会留下缝隙）。

#### Scenario: 光标在空单元格与文本单元格上形状一致

- **WHEN** 光标从空白格移到该行任一字符上
- **THEN** 光标盒的顶边与高度不变，恒等于整格

#### Scenario: 光标在降部字形上不越出本行

- **WHEN** 光标停在 `g`、`j`、`中` 等位图高于或低于参考字形的字符上
- **THEN** 光标盒仍限制在本行整格内，不侵入上一行或下一行

### Requirement: 光标标记锚在单元盒

竖线（Bar）与下划线（Underline）光标 MUST 锚在单元盒，MUST NOT 锚在格内字形位图：
竖线的高 = 单元高、顶边 = 单元顶边；下划线的底边 = 单元底边、宽 = 单元宽 × 列跨度。
位图高度随字符变化，空单元格还得另取参考字形，锚位图会让同一光标在空白格与文本格之间
变高变矮。

#### Scenario: 竖线光标在空单元格与文本格上等高

- **WHEN** 同一个竖线光标依次停在空白格、`M` 与降部字形 `g` 上
- **THEN** 三处的顶边与高度相同，恒为整格

#### Scenario: 下划线光标贴单元底边

- **WHEN** 光标停在任一单元格
- **THEN** 下划线横条的下沿与单元底边重合，不侵入下一行

### Requirement: 宽字符光标几何

宽字符（`CellData::width == 2`）上的光标 MUST 覆盖整个字形宽度（两列），
MUST NOT 只覆盖首列。尾部网格列没有独立 `CellData`，光标盒按首条记录的列跨度缩放，
故不存在需要单独处理的间隔单元。

#### Scenario: 光标停在宽字符上

- **WHEN** VT 光标落在某个宽字符的首列
- **THEN** 光标盒横跨该宽字符的两列，右边缘与字形右边缘对齐
