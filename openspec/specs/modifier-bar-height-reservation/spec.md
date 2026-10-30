# modifier-bar-height-reservation Specification

## Purpose

键盘弹出与隐藏时终端可用高度变化，网格重算、内容平移与键栏占位必须同源：
`runtime.modifierBarHeightPx` 同时供 `TerminalScreen` 的高度预留、`TerminalSurface`
的可用高度（`applyGridResize`）与 `computeImeSurfaceShift` 的平移量使用。任一处
另算即出现平移与行数差半行，表现为底行被吞或内容跳动。

## Requirements

### Requirement: 预留高度由布局数据算出

`modifierBarHeightPx` MUST 由 `extra-keys` 的实际行数与 `terminal-toolbar-height`
的单键高度算出，MUST NOT 用写死的行数常量。

行数 MUST 与「实际画出的行数」用同一判据（`extraKeyRowCount` 与
`buildExtraKeySlots` 共用 `isRenderableKey`）。两者一旦分家，键栏就会盖住终端
底部若干行——它盖在 Surface 之上，被盖的行既看不见也点不到，日志里没有异常。

解析出零行时 MUST 返回 0 且不画键栏。

#### Scenario: 输入法弹出底部行可见

- **WHEN** 输入法弹出且光标在底部行
- **THEN** 底部行恰好停在键栏上方，不多不少，不被遮挡

#### Scenario: 自定义三行布局

- **WHEN** `extra-keys` 给出三行且全部可渲染
- **THEN** 预留高度是三行，终端行数按同一高度扣减

#### Scenario: 只有不可渲染键的一行

- **WHEN** 某行只写了 `FN`（未实现，不渲染）
- **THEN** 该行不计入行数，也不占高度

#### Scenario: 键栏被清空

- **WHEN** `extra-keys = []`
- **THEN** 预留高度为 0，键栏不出现在界面上
