# modifier-bar-height-reservation Specification

## Purpose

键盘弹出与隐藏时终端可用高度变化，网格重算、内容平移与键栏占位必须同源：
`runtime.modifierBarHeightPx` 同时供 `TerminalScreen` 的高度预留、`TerminalSurface`
的可用高度（`applyGridResize`）与 `computeTerminalPanPx` 的平移量使用。任一处
另算即出现平移与行数差半行，表现为底行被吞或内容跳动。

## Requirements

### Requirement: 预留高度等于实际总高

`MODIFIER_BAR_HEIGHT_DP` MUST 等于 `BUTTON_HEIGHT_DP` 两行实际总高（72dp），
使网格预留行数与 IME 平移量严格一致。

#### Scenario: 输入法弹出底部行可见

- **WHEN** 输入法弹出且光标在底部行
- **THEN** 底部行恰好停在键栏上方，不多不少，不被遮挡
