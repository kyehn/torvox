## Why

网格与 IME 平移共用的键栏预留高度为常量 80dp，但键栏实际总高 72dp（36dp×2 行，零间距）：网格少算约半行、平移多减约半行，输入法弹出时底部常差半行被遮、上滑浏览时顶部被多顶半行。

## What Changes

- `MODIFIER_BAR_HEIGHT_DP` 由 80 改为 72，与 `BUTTON_HEIGHT_DP` 两行实际总高一致。
- 具名常量注释说明来源，防回退。

## Capabilities

### New Capabilities

- `modifier-bar-height-reservation`: 键栏预留高度与实际总高一致。

### Modified Capabilities

无。

## Impact

- `TerminalRuntime.kt` 一处常量；网格行数与 IME 平移自动对齐。
