# 任务

## 一、堵住应用侧的格式选择漏洞

- [x] `select_surface_format`：退化链只接受三通道以上的格式，单通道与双通道格式
      一律不选；设备只报告这类格式时报错，不再带着不可分配的格式去配置交换链。
- [x] 单测：设备把 R8 排在首位时取 `Rgba8Unorm`；只剩单通道或双通道格式时报错；
      RGBA8 族缺席时退到多通道格式。

## 二、把设备侧告警的归因钉成可复核的事实

- [x] 真机取证：R8 申请全部落在 `attachWindow` 与 `attach_surface: configured` 之间，
      即厂商 Vulkan 驱动创建交换链的窗口期；每次表面重建对应一组申请，紧随其后是
      缓冲注册成功与 `format=Rgba8Unorm` 的交换链建立，帧照常呈现——失败的是驱动
      自己的格式试分配，不是应用的呈现缓冲。
- [x] 上游判据：wgpu-hal 30.0.1 `src/vulkan/swapchain/native.rs` 的
      `create_swapchain` 用 `image_format(original_format)`，应用请求的是
      `VK_FORMAT_R8G8B8A8_UNORM`；AOSP `libs/gui/Surface.cpp` 的
      `dispatchSetBuffersGeometry` 与 `query(NATIVE_WINDOW_FORMAT)`、
      `libs/gui/BufferQueueProducer.cpp` 的 `allocFormat` 决定窗口缓冲格式的来路。
- [x] 规格：`logcat-hygiene` 的「设备分配器的格式探测告警」场景补齐上述事实。

## 三、取证可读性

- [x] 仪器化取证规则把 `mali_gralloc` 与 `gralloc4` 纳入失败报告的日志尾部标签表，
      使「窗口缓冲申请被拒」在设备失败报告里可见。

## 四、验证

- [x] `cargo test`：原生 791 个用例全过（含新增 2 个格式选择用例）。
- [x] 应用自身在该设备上的日志无 error：只有已限频的慢帧诊断，表面未判死、帧照常
      呈现。

## 遗留（需真机复测）

- 真机复跑：确认 R8 告警条目数与表面重建次数仍是固定倍数，且画面正常。本仓环境
  只有 Mesa lavapipe（主机软件 Vulkan）与 CI 的 SwiftShader 模拟器，二者都不是 Mali
  驱动，故该复测不在本变更的完成条件内；本轮以用户提供的真机 logcat 取证代替，
  应用侧的格式选择漏洞已由单测钉死。
