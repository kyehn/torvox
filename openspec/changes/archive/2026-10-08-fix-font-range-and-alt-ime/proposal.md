## Why

两处与输入相关的体验缺陷，均有代码级实证：

1. 字号调节条范围与实际可设置范围不一致：调节条上界为 `fontSizeRangeMaxSp`（Termux 256px 换算，低密度设备可达 256sp），而原生 `setFontSizeInPlace` 只接受 4.0..100.0，超限静默丢弃；手势预览门限又是固定的 4..100sp。三处各自为政——低密度设备上滑块可拖到原生直接丢弃的值（设置与渲染脱节），高密度设备上预览与滑块上界不同。
2. 备用屏 TUI（如 helix）遇输入法弹出时顶部被隐藏：`computeImeSurfaceShift` 对填满视口的内容恒返回整块键盘高度，备用屏应用永远占满视口，故每次弹键盘都把应用顶部推出屏幕；且位移后的视觉行与触摸行错位（触摸按未位移坐标换算）。

## What Changes

- 字号三入口收敛到单一来源 `effectiveFontSizeMaxSp`（Termux 像素上限与原生 100sp 钳位的较小者）：调节条、手势、预览、存储值应用全部钳到同一区间；原生钳位保留为最终兜底。像素上限换算改用完整 sp→px 系数（显示密度 × 系统字体缩放），与推给原生 `setRasterScale` 的值同源；该系数的钳位区间与原生接受区间（0.5..=8.0）一致，避免合法系数被静默丢弃。
- 备用屏激活时输入法跟随位移为 0（顶部保持可见、触摸对齐恢复）；主屏行为零变化。备用屏状态随每帧渲染结果一并上报（打包位 54，与光标行、内容下沿同批采样，读取 VT 线程写入的原子量），不再由输入法定居后的独立阻塞查询提供。

## Capabilities

### New Capabilities

无。

### Modified Capabilities

- `font-selection`：用户可选字号上界明确为有效上界（Termux 上限与原生钳位取小）。
- `render-stability`：备用屏下输入法弹出不再位移终端 Surface。

## Impact

- `settings/SettingsRepository.kt`：新增原生上界常量与有效上界/档数函数。
- `ui/SettingsScreen.kt`：调节条改用有效上界。
- `ui/TerminalSurface.kt`：`zoomFontSize` 改用有效上界。
- `runtime/TerminalRuntime.kt`：预览门限与存储值应用钳到有效区间；移除被替代的 tenths 常量。
- `runtime/TerminalRuntime.kt`：`computeImeSurfaceShift` 新增备用屏参数。
- `ui/TerminalScreen.kt`：订阅逐帧发布的备用屏状态并传入位移计算。
- 对应单测随之增补；插桩语义（主屏）不变。
