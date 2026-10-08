# 字号区间收敛与备用屏输入法位移

## 上下文

- `fontSizeRangeMaxSp`（Termux 256px 换算）与原生 4.0..100.0 钳位、预览固定门限三处不一致。
- `computeImeSurfaceShift` 对备用屏恒返回键盘高度，隐藏应用顶部并错位触摸。

## 任务

1. `SettingsRepository` 新增 `NATIVE_FONT_SIZE_MAX_SP`、`effectiveFontSizeMaxSp`、`effectiveFontSizeRangeSteps`，注释写明三入口共用。
2. 调节条、手势、预览、存储值应用四处改用有效区间；删除被替代的 tenths 常量（避免死代码）。
3. `computeImeSurfaceShift` 新增 `isAltScreenActive` 参数（默认 false，主屏行为不变），备用屏返回 0。
4. `TerminalScreen` 缓存备用屏状态（键盘定居与会话切换时 IO 查询），位移调用传入。
5. 增补 `FontSizeRangeTest`（有效上界、档位整除、自适应值落界）、`ComputeImeSurfaceShiftTest`（备用屏恒 0）、低密度手势钳制用例；现有用例保持。
6. 单测验证：`./gradlew ':app:testDebugUnitTest' --tests 'terminal.emulator.settings.FontSizeRangeTest' --tests 'terminal.emulator.runtime.ComputeImeSurfaceShiftTest' --tests 'terminal.emulator.ui.TerminalSurfaceLogicTest'` 全绿。
