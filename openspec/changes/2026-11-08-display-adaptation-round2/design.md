# Design

## 复核结论（第二轮）

| 发现 | 本轮核对位置 | 状态 |
| --- | --- | --- |
| 密度变化不重推渲染契约 | `MainActivity.onConfigurationChanged` 仍只比 uiMode 与 fontScale；`configChanges` 无 `density` | 未修复，上一轮判定成立 |
| 启动下界按显示尺寸推导 | `TerminalRuntime.start()` 仍用 `displayMetrics` 算 300–600 / 250–500 像素下界 | 未修复 |
| 帧回调链无停止路径 | `vsyncFrameCallback.doFrame` 末尾仍无条件自投；只有 `ensureVsyncChainStarted`，无 `stop` | 未修复 |
| 配置声明少四项 | `AndroidManifest.xml` 仍为六项 | 未修复 |
| 注释引用的闪相位字段 | 全仓无 `pending_flash_phase` | 文档漂移未改 |

## 参考项目复核

- termux-app：`configChanges` 八项（含本项目六项），无 `fontScale`；本项目
  在其基础上加 `fontScale` 是为系统字体缩放，属合理差异。
- ghostty-android-terminal：`configChanges` 九项（含 `uiMode` 与 `density`），
  与 termux-app 一致。
- Drosh：未声明 `configChanges`，走默认重建；帧循环 `start()`/`stop()`
  成对，`doFrame` 内先判 `running`。本项目可照此补停止路径。
- termlib 与 AndroidTerminal：组件库或 Compose 消费状态流，不声明 Activity
  配置；不提供基准。

## 设计决定

与上一轮一致，不重新设计：密度用 `densityDpi` 比较并经 `applySettings()`
重推；帧回调链按活跃度启停（Drosh 式成对启停）；启动下界删除，只保留
非正尺寸拒绝；配置声明补齐四项。实现步骤登记在任务中，等授权。

## 明确不作处置

- 不做 `setFrameRate` 固定帧率声明；不读 `isPowerSaveMode` 分支渲染；
  不建第二套节拍。平台刷新率上限已自动限制显示节拍。
- 不处置「终端会话概率卡死」（`session-liveness` 已有主规约，属另一变更）。
- 不处置代码规模与大文件治理（`code-health` 已有主规约，属另一变更）。

## 验证

- `openspec validate --all --strict` 通过。
- 每一轮复核均重新逐文件读取全部被跟踪源码与文档，覆盖率清单可复算。
