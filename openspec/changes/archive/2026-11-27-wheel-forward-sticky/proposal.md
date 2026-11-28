# Proposal

## Why

`SurfaceInput.onScroll` 把「本次手势是否已把拖动转发为滚轮」这一标记直接赋成本次
编码的结果（`wheelForwarded = bridge.encodeMouseWheel(...)`），于是标记在一次手势内
可升可降：前一次滚动成功转发、后一次编码失败时，标记被清成 false。

而规范（`key-input-encoding`）要求的是「本次拖动已转发滚轮的手势，其惯性段 MUST
同样属于应用，MUST NOT 让本地回滚接管」。标记被清掉后，本地回滚在手势中途接管，
应用（已收到前几档滚轮）与本地视口同时滚动；手势末尾的惯性段也随之改为滚本地回滚，
用户在 TUI（yazi 等）里看到的是「应用滚一半、本地滚一半」。

编码失败并非只在「应用未启用鼠标上报」时发生：VT 忙时编码拿不到编码器（查询超时）
同样返回空数组。两条失败原因在调用方都表现为 false，故必须靠标记的只升不降来区分
「本次手势尚未转发过」与「本次手势已转发过」。

## What Changes

- 「已转发滚轮」标记在一次手势内只升不降：转发过一次即整段手势归远端
- 判定抽为纯函数 `wheelForwardedState`，可 JVM 单测
- 规范补上「转发标记在手势内只升不降」的场景，使实现细节可追溯

## Capabilities

### New Capabilities

无。

### Modified Capabilities

- `key-input-encoding`：明确转发标记在手势内只升不降（编码失败不得让本地回滚接管）。

## Impact

- `android/app/src/main/java/terminal/emulator/ui/SurfaceInput.kt`：转发标记的更新方式
- `android/app/src/main/java/terminal/emulator/ui/TerminalSurfaceMath.kt`：新增纯函数
  `wheelForwardedState`
- `android/app/src/test/java/terminal/emulator/ui/WheelForwardingTest.kt`：新增回归测试
- 受影响的行为：应用启用鼠标上报时，一次拖动中途的编码失败不再让本地回滚接管
