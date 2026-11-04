# Proposal

## Why

真机（Mali-G57 / Unisoc T606）logcat 里仍有 `mali_gralloc` 的 R8 格式拒绝告警
（`Requested R8 format is not supported with this allocator` 与
`ERROR: Unrecognized and/or unsupported format 0x38 and usage 0xb00`）。这条告警一直
挂在「未被解决的问题」上，缺的是两件事：

1. **应用侧还留着一个能选中 R8 的口子**：交换链格式虽已指名要 `Rgba8Unorm`，但退化链
   末尾是「任意格式」——设备只报告单通道格式时，应用会把 `R8Unorm`（0x38，恰是设备
   分配器拒绝的那个格式）当交换链格式去申请窗口缓冲，申请必然失败，画面无法呈现。
2. **归因缺可复核的上游事实**：现有结论只有「应用侧无开关」，没有说清探测发生在哪
   一层、应用请求的到底是什么格式，于是每次复现都要重新猜一遍。

本轮真机日志给出了两个可复核的事实：R8 申请全部落在 `attachWindow` 与
`attach_surface: configured` 之间（厂商 Vulkan 驱动建交换链的窗口期），且每次申请
失败之后紧接着就有缓冲注册成功、交换链以 `format=Rgba8Unorm` 建立、帧照常呈现——
即失败的是驱动自己的格式试分配，不是应用的呈现缓冲。

## What Changes

- `select_surface_format` 的退化链只接受三通道以上的格式：单通道与双通道格式一律
  不选；设备只报告这类格式时报错退出，不再把 `R8Unorm` 当交换链格式。
- `logcat-hygiene` 的「设备分配器的格式探测告警」场景补齐可复核的上游事实：探测由
  厂商驱动在创建交换链时发起、被拒的是驱动自己的格式试分配、应用请求的是 8 位
  RGBA 族、条目数与表面重建次数成固定倍数、应用侧无开关。
- 仪器化取证规则把设备图形分配器的标签纳入失败报告的日志尾部，使「窗口缓冲申请被
  拒」在设备失败报告里可见。

## Impact

- 规格：`render-stability` ADDED 一条 Requirement（交换链格式必须能承载颜色）；
  `logcat-hygiene` MODIFIED 一条 Requirement（设备分配器的格式探测告警归因）。
- 代码：`native/src/render/context/surface.rs` 的 `select_surface_format`；
  `android/app/src/androidTest/java/terminal/emulator/TerminalLogcatRule.kt` 的取证
  标签表。
- 不改窗口格式与表面生命周期：应用已把窗口格式置为 `PixelFormat.RGBA_8888`，且
  表面重建次数已是最小（尺寸未变时走原地 reconfigure，不重建交换链）。
