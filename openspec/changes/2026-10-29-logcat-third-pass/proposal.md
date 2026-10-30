# Proposal

## Why

第二轮审计把三份设备 logcat 的 753 条 W/E/F 逐类归了因，结论是「应用自身条目已全部有根因与
落地位置」。本轮复核发现两处归因把**上游与驱动的能力缺失**写成了应用侧的选择：

- `W/wgpu_core::instance: Missing downlevel flags: DownlevelFlags(SURFACE_VIEW_FORMATS)`
  被记为「当前渲染路径不使用该能力」。实际上该能力位由 Vulkan 设备是否支持
  `VK_KHR_swapchain_mutable_format` 决定，与应用声明的表面格式列表无关；
- `E/mali_gralloc` 的「Requested R8 format is not supported」与
  「Unrecognized and/or unsupported format」被笼统记为「平台/驱动，无应用开关」，
  但没说清探测由什么触发、为何每次表面重建都来一轮。

这两条归因口径的差别会直接误导后续排障：前者会让人以为应用「关掉某能力」即可消除告警，
后者无法解释「同一台设备上为什么有时有、有时没有」。故本轮把归因落到可复核的上游源码事实上。

同时，CI 的 `build` 工作流在模拟器用例 `ThemeFollowSystemInstrumentedTest` 上失败
（断言「日间主题底色没有出现在终端 Surface 上」）。根因已修（启动期换窗不再丢弃新 Surface、
用例逐轮重取视图），但该工作流在远端已被手动禁用，无法直接复跑；本轮在本地模拟器上以
同一组仪器化用例复跑，并把这次复跑顺带产出的完整 logcat 作为第三轮的计数基线。

基线暴露出两处**应用自身**仍在正常路径上记 warn：

- 安装器执行包安装后脚本时把「实际执行的命令行」记成 warn——那是每次安装必走的轨迹行，
  不是异常；
- 剪贴板应答回写失败一律记 warn——但会话已退出（或 PTY 已随子进程关闭）时，远端询问的
  会话已经不存在，应答无处可去，这是生命周期正常终态。

按 `logcat-hygiene` 规范「应用自身的正常路径不得产生警告与错误」，这两处必须降级。

## What Changes

- 修正归因：渲染能力提示缺失归因于设备未提供 `VK_KHR_swapchain_mutable_format`
  （上游文档亦写明 Vulkan on Android 不支持该能力），应用侧无可用开关；
  设备分配器的格式探测归因于表面重建时驱动枚举全部表面格式并逐个试分配，与适配器枚举、
  与应用声明的格式列表都无关。
- 归因补齐可复核事实：平台与驱动侧条目必须指出「由哪一层决定」并给出上游源码位置或
  扩展名判据，不得以「当前路径不使用该能力」代替根因。
- 应用侧降级两处正常路径日志：安装器的安装后脚本轨迹行改为 info；剪贴板应答回写在会话
  已结束时改为 debug（PTY 已随子进程关闭时同样按会话结束处理，判据与输入路径一致）。
- 新增审计留档 `logcat-audit.md`：本地模拟器复跑的逐标签计数与设备日志的归因修正，
  只保留聚合结论与倍数关系。

## Capabilities

### New Capabilities

无。

### Modified Capabilities

- `logcat-hygiene`：正常退化路径补上「安装后脚本轨迹行」与「会话已结束的剪贴板应答回写」；
  新增「平台与驱动侧条目必须给出可复核的上游事实」的要求。

## Impact

- `native/src/android/ffi/render/clipboard.rs`（应答回写失败分级 + 分类单测）
- `android/app/src/main/java/terminal/emulator/installer/SecondStageRunner.kt`
  （安装后脚本轨迹行降级为 info）
- `openspec/specs/logcat-hygiene/spec.md`
- `openspec/changes/2026-10-29-logcat-third-pass/logcat-audit.md`
- 不改 `docs/specification/` 与 `docs/STATUS.md`。
