# Design

## 问题

`wgpu::SurfaceTexture` 在未呈现的情况下被丢弃时，wgpu（api 层）调
`detail.texture_discard()`；Vulkan 后端把它落到
`wgpu-hal/src/vulkan/swapchain/native.rs` 的 `SurfaceTexture::discard_texture`，
而那里是：

```rust
// TODO: Current implementation no-ops
Ok(())
```

于是那张交换链图像既不呈现、也不归还给交换链。Android 的交换链通常只有两三张图像，
累积几次后取纹理就再也拿不到图像：`vkAcquireNextImageKHR` 卡住 →
渲染线程按 100ms 期限跳过 → 工作线程继续卡在 dequeue → 循环往复，画面永久停在旧帧。

模拟器证据（release 构建，API 35 / SwiftShader）：

- 首帧后 `acquire_texture: get_current_texture timed out after 100ms`；
- 此后同一进程内 `vulkan: dequeueBuffer timed out: Function not implemented (-38)`
  每约 200ms 一条，`Runtime: transient render error code=-1 (consecutive=…, surviving)`
  每约 600ms 一条，计数一路涨到 91；
- 期间画面停在输入到一半的那一帧（`ls /nonex`），强停进程才恢复。

同一形态在 `docs/STATUS.md` 的分析里被记为「取纹理超时 → 画面冻结」，此前只当慢机器
退化处理；上游源码说明它是不可逆的缓冲区泄漏。

## 三条丢弃路径

1. `Renderer::begin_frame`：取得的纹理尺寸与当前配置不符时，原实现「丢弃 output 后返回」，
   注释还声称跳过等于不丢缓冲区——与上游事实相反。
2. `Renderer::render_frame_with_plan`：管线缺失时 `?` 提前返回，`FrameContext` 被丢弃，
   其 `texture` 随之丢弃。
3. 取纹理工作线程：渲染线程超时后已销毁接收端，工作线程 `send` 收到
   `SendError(result)` 并把它连纹理一起丢弃。

## 修复

新增 `release_acquired_texture(device, queue, texture, background)`：把纹理清成背景色
（一个 RenderPass 的 `LoadOp::Clear`），提交后 `queue.present(texture)`。清屏是为了
不把未定义内容闪到屏幕上：换尺寸后新分配的缓冲区内容未定义，直接呈现可能是垃圾像素。

三条路径分别改为：

- 尺寸不符：先归还纹理，再 `surface.configure(...)`。顺序不能反——configure 会销毁旧
  交换链（`release_resources` 会等设备空闲），之后再呈现就是悬垂图像。
- 管线缺失：归还后返回原有的 `GpuError::Surface("No render pipeline")`。
- 迟到结果：工作线程在 `send` 失败且结果带纹理时亲自归还；`lane.finish()` 挪到回送之后，
  使在途记账覆盖「纹理尚未归还」的整个窗口——`release_surface` / `Renderer::drop`
  的排干据此保证交换链销毁前该纹理已被呈现。

工作线程归还时提交到与渲染色线程同一条队列，wgpu 的提交顺序保证两次呈现不会互相插入
到对方的编码中间；队列提交本身是线程安全的。

## 时序约束

- 归还必须在交换链销毁之前完成：排干方（`drain_in_flight_acquires`）等待在途归零，
  而工作线程先回送、后销账，因此排干返回时纹理已呈现（或已被渲染线程接收）。
- 尺寸不符分支的归还在 `configure` 之前：此时旧交换链仍然有效。
- 管线缺失分支的归还在同一帧内完成：surface 仍在本帧的持有期内。

## 不做

- 不删除 100ms 期限；它是 Mali 驱动永久卡死时的唯一保护。
- 不改 present mode、缓冲几何或 `PowerPreference`：这次停帧与它们无关。
- 不把归还做成「记账后统一在下一帧处理」：纹理必须在本帧或 surface 释放前归还，
  否则交换链图像仍处于被占用状态。
