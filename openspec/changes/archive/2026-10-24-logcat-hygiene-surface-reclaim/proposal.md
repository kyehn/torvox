# Proposal

## Why

三份 logcat 逐条归因后，应用自身剩下的唯一 W/E 来源是一条自愈链：平台收走原生窗口
（BufferQueue 被遗弃）后，wgpu 重新配置 surface 必然失败，渲染帧随之取不到纹理，
宿主换新窗口重挂载即恢复。

这条链功能上正常（一次错误即自愈），但它把**平台事件**按**应用缺陷**的等级记了下来：
一次切后台就刷一条 E 级加一条 W 级。`logcat-hygiene` 规范要求「应用自身的正常路径
不得产生警告与错误」——正常退化路径必须记 debug/info。闸门一旦被正常噪声占住，
排障时无法再区分「应用缺陷」与「环境噪声」。

上游事实（wgpu 30.0.1）：wgpu-core 在 `surface.get_capabilities()` 失败时返回
`UnsupportedQueueFamily`（描述为「不支持该适配器的队列族」），并经
`handle_error_nolabel(..., "Surface::configure")` 上抛，故该错误的描述里必然含
`Surface::configure`。这给了判别依据：**错误种类 + 两个上游固定文案**，
而不是「看起来像」。

## What Changes

- 新增 `GpuError::SurfaceUnavailable` 变体与 `GpuError::is_recoverable()`：
  判据是**错误类型**而非文案——只有「本帧没有可呈现的纹理」这一类可恢复。
- 渲染暂停、surface 尚未挂载、取帧失败三条无纹理路径改用该变体；
  JNI 层按可恢复性分级：可恢复记 debug，真异常（管线缺失、单元转换失败、attach 失败）
  仍记 error，不降级。
- 未捕获 GPU 错误按来源分级：仅当**校验错误**同时含上游固定文案 `Surface::configure`
  与「不支持该适配器的队列族」时视为「窗口被收回」记 debug；自家配置错误
  （格式不支持、尺寸非法等）与其它错误一律仍记 error。
- 宿主侧瞬时渲染错误分级：首次记 info（正常退化），连续出现才记 warn
  （新窗口迟迟不来才是异常）；原有 500ms 限频逻辑不变。
- 顺带修掉 `exec_failure_session_exits_with_errno_encoding` 的偶发失败：退出标志由读线程
  （PTY EOF）与等待线程（拿到退出码）两条路径置位且无序，用例改为轮询到「码可上报」，
  与生产路径（逐帧轮询）同口径。

## Capabilities

### New Capabilities

无。

### Modified Capabilities

- `logcat-hygiene`：正常退化路径补上「平台收走原生窗口」；新增「未捕获 GPU 错误必须
  按来源分级」与「宿主侧瞬时渲染错误必须按连续次数分级」的要求。

## Impact

- `native/src/render/mod.rs`（`GpuError::SurfaceUnavailable`、`is_recoverable()`、分类单测）
- `native/src/render/pass.rs`（三条无纹理路径改用新变体）
- `native/src/render/context.rs`（`gpu_error_is_surface_lost()`、`log_gpu_error()` 分级、分类单测）
- `native/src/android/ffi.rs`（`log_frame_failure()` 按可恢复性分级）
- `android/app/src/main/java/terminal/emulator/runtime/TerminalRuntime.kt`
  （瞬时渲染错误分级 + `transientRenderErrorIsWarning`）
- `android/app/src/test/java/terminal/emulator/runtime/TransientRenderErrorSeverityTest.kt`（新增）
- `native/src/terminal/session.rs`（退出码上报用例改为轮询到可上报）
- 不改 `docs/specification/` 与 `docs/STATUS.md`。
