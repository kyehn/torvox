# Proposal

## Why

真机 logcat 里，应用自身仍在一批「正常路径」上留下 warn 与 error：文档提供者对缺失
文档抛异常被框架记成 W 级「Failed during query」（15 条）；主线程的 vsync 帧回调链在
应用不可见时被逐帧停/起，既刷屏又空耗 CPU；设备 GPU 分配器在创建交换链时按 R8 单通道
格式申请缓冲，被本机分配器拒绝并记为 error（24 条）。另有三条未决问题（新会话概率无
响应、修饰键栏全面屏手势触发动画）同源：都是「判定与实际状态不一致」。

## What Changes

- 文档提供者的缺失文档查询改为回 `null` 游标而不是抛 `FileNotFoundException`：框架的
  `DocumentsProvider#query` 捕获该异常后本就回 `null`，客户端看到的结果逐位相同，差异
  只有一条被框架记成 warn 的日志。
- vsync 帧回调链的停链复核改用同一套停链判据：应用不可见时不再被「会话仍活跃」立刻
  拉起，消除逐帧停/起循环与逐帧日志。
- 交换链创建路径上，窗口缓冲格式与交换链格式保持一致，并避免无谓的表面重建，减少设备
  分配器在每次重建时按不支持的单通道格式申请缓冲。
- 输入法连接失活时的光标锚点请求不再由应用侧主动触发。
- 修饰键栏在全面屏手势区内的按压反馈不再启动动画。
- 新会话的首帧必须真正产出：生成后立即补齐网格与像素尺寸，首帧未产出时按会话存活
  规约定位到层级并恢复。

## Capabilities

### Modified Capabilities

- `documents-provider`：缺失文档与缺失目录的查询响应方式。
- `logcat-hygiene`：新增「文档缺失查询」「帧回调链停启」「输入法连接失活」「设备分配器
  拒绝单通道缓冲」四类场景的定级判据。
- `session-liveness`：新会话首帧产出的判据与恢复动作。
- `modifier-bar-gesture-cancel`：全面屏手势区内的按压反馈不得启动。
- `render-stability`：交换链格式与窗口缓冲格式的一致性。

## Impact

- `android/app/src/main/java/terminal/emulator/TerminalDocumentsProvider.kt`
- `android/app/src/main/java/terminal/emulator/runtime/TerminalRuntime.kt`
- `android/app/src/main/java/terminal/emulator/ui/ModifierBar.kt`
- `native/src/render/context/surface.rs`
- `native/src/android/ffi/render/window.rs`
