# modifier-bar-height-reservation Specification

## Purpose

键栏高度预留与实际渲染总高一致，网格与平移不差半行。

## ADDED Requirements

### Requirement: 预留高度等于实际总高

`MODIFIER_BAR_HEIGHT_DP` MUST 等于 `BUTTON_HEIGHT_DP` 两行实际总高（72dp），
使网格预留行数与 IME 平移量严格一致。

#### Scenario: 输入法弹出底部行可见

- **WHEN** 输入法弹出且光标在底部行
- **THEN** 底部行恰好停在键栏上方，不多不少，不被遮挡
