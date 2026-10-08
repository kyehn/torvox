# 字号区间收敛与备用屏输入法位移

## 上下文

- `fontSizeRangeMaxSp`（Termux 256px 换算）与原生 4.0..100.0 钳位、预览固定门限三处不一致；
  且像素上限换算原先只按显示密度，漏掉系统字体缩放。
- `computeImeSurfaceShift` 对备用屏恒返回键盘高度，隐藏应用顶部并错位触摸。

## 任务

- [x] `SettingsRepository` 新增 `NATIVE_FONT_SIZE_MAX_SP`、`effectiveFontSizeMaxSp`、`effectiveFontSizeRangeSteps`，注释写明三入口共用。
- [x] 字号上下限与像素换算统一按完整 sp→px 系数（显示密度 × 系统字体缩放），与原生 `setRasterScale` 同源；该系数钳位区间与原生接受区间一致。
- [x] 调节条、手势、预览、存储值应用四处改用有效区间；移除零调用的档数函数（避免死代码）。
- [x] `computeImeSurfaceShift` 新增 `isAltScreenActive` 参数（默认 false，主屏行为不变），备用屏返回 0。
- [x] 备用屏状态由原生渲染结果打包位随帧上报（与光标行、内容下沿同批），替代输入法定居后的独立阻塞查询。
- [x] `TerminalScreen` 订阅运行期逐帧发布的备用屏状态（`altScreenActiveFlow`）并传入位移计算。
- [x] 回车键族（ENTER / NUMPAD_ENTER / DPAD_CENTER）统一纳入编码器 `enterKeyCodes`，并让贴底判定共用；避免 DPAD_CENTER 落入字符猜测分支。
- [x] 增补 `FontSizeRangeTest`（有效上界、档位整除、系统字体缩放致上界收紧、自适应值落界）、`ComputeImeSurfaceShiftTest`（备用屏恒 0）、`RenderResultPackingTest`（备用屏打包位互不重叠）、`TerminalInputEncoderTest`（回车键族）等用例。
- [x] 单测验证：`:app:testDebugUnitTest` 相关用例全绿；`cargo clippy --workspace --all-targets -- --deny warnings` 零警告。
- [x] 按 code-review 双轴审查修复：光栅钳位区间对齐、删除不可达死分支、补齐文件末尾换行、移除死代码与重复实现。
- [x] 更新 `openspec/specs` 的 font-selection 与 render-stability，并按 openspec 约定补本 change 的 specs 增量目录。