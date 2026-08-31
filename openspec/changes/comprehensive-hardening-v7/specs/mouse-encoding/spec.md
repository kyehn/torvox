## MODIFIED Requirements

### Requirement: SGR 鼠标编码门控与实时尺寸

系统 SHALL 在 `encode_mouse_event` 中通过 `ghostty_mouse_encoder` 校验 mouse mode（1000/1002/1003/1006），非使能时返回空；使能时使用实时 `cellWidth/cellHeight` 将像素坐标转换为 cell 坐标并生成 SGR 序列，支持越界 clamp 与拖拽序列。

#### Scenario: 非 mouse mode 零事件

- **WHEN** 终端未启用 mouse mode 且用户点击
- **THEN** `encode_mouse_event` 返回 `None`，PTY 不写入任何序列

#### Scenario: 越界 clamp

- **WHEN** 触摸坐标为负或超出 `cols*cellWidth`
- **THEN** 编码后的 `col/row` 被 clamp 至 `[0, cols)`/`[0, rows)`，SGR 坐标为 `col+1/row+1`

#### Scenario: 拖拽序列

- **WHEN** 用户按下左键后移动至新 cell 再抬起
- **THEN** 依次产生 `press(0)`、`drag(32)`、`release(3)` 的 SGR 序列，且坐标基于实时 cell 尺寸

#### Scenario: 滚轮

- **WHEN** 用户在 mouse mode 下滚动
- **THEN** 编码为 `button 64/65` 的 SGR 序列，方向正确
