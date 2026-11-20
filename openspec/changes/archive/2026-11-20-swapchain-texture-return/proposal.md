# Proposal

## Why

模拟器实测到终端**永久停帧**：首帧呈现后取纹理超时（100ms 期限），随后画面停在旧帧，
帧循环每 600ms 报一次 `transient render error code=-1`，同时
`vulkan: dequeueBuffer timed out` 反复出现，几十秒内毫无恢复，只有强停进程才结束。

根因在 wgpu-hal 的上游实现：Vulkan 后端对「未呈现即被丢弃」的交换链纹理，
`discard_texture` 是空实现（上游 TODO，`wgpu-hal-30.0.2/src/vulkan/swapchain/native.rs`
的 `SurfaceTexture::discard_texture`）。丢弃一张这样的纹理既不呈现、也不归还，
那张交换链图像从此不可再用。交换链总共只有两三张图像，累积几次后
`vkAcquireNextImageKHR` 永远等不到可用图像——终端就此停帧，且没有任何自愈路径。

本仓库有三条路径会把已取到的纹理直接丢弃：

- `begin_frame` 的尺寸不符分支（注释还写着「跳过也保证不被丢弃而不 present」，与上游事实相反）；
- `render_frame_with_plan` 的「管线缺失」提前返回；
- 取纹理工作线程在渲染线程已超时放弃后回送结果失败，纹理随 `SendError` 被丢弃。

## What Changes

- `render-stability` 新增要求：**取到的交换链纹理必须呈现归还**，并写明三条路径的约束
  （尺寸不符、管线缺失、迟到结果）与「释放 surface 前排干」的时序前提。
- 实现：新增 `release_acquired_texture`（清成背景色后呈现）并在上述三条路径调用；
  工作线程的 `lane.finish()` 移到回送之后，使在途记账覆盖「纹理尚未归还」的窗口。
- 回归测试：`returned_texture_is_cleared_to_the_background` 断言归还前纹理被清成背景色。

## 不做什么

- 不放宽或取消取纹理的 100ms 期限：它是 Mali 驱动 `vkAcquireNextImageKHR` 永久卡死时的
  唯一保护，停帧的成因是纹理没归还，不是期限太短。
- 不通过降级日志、过滤 tag 或改日志等级来掩盖停帧：失败仍按既有闸门如实记录。
- 不 vendor 或打补丁改 wgpu-hal：上游实现不可改，应用侧只能保证不再丢弃未呈现纹理。
- 不动 `docs/STATUS.md`：该文件首行声明禁止修改。

## Capabilities

### New Capabilities

（无）

### Modified Capabilities

- `render-stability`: 新增「取到的交换链纹理必须呈现归还」要求与其场景。

## Impact

- Affected specs: `render-stability`（新增一条要求）。
- Affected code: `native/src/render/pass/acquire.rs`（归还助手、工作线程迟到结果）、
  `native/src/render/context/renderer.rs`（尺寸不符分支）、
  `native/src/render/pass/frame.rs`（管线缺失分支）、
  `native/src/render/pass/tests.rs`（回归测试）、`native/src/render/tests/mod.rs`
  （测试设备助手改为 `pub(crate)` 供兄弟模块复用）。
- 行为变化：取到但本帧不用的纹理改为「清成背景色后呈现」，屏幕最多闪一帧背景色，
  交换链不再丢图像。
