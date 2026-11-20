# Proposal

## Why

上一轮修复（`2026-11-20-swapchain-texture-return`）确立了硬约束：**已取到的交换链纹理
必须呈现归还**——Vulkan 后端丢弃未呈现纹理是空实现，丢一张即永久损失一张交换链图像，
累积后终端永久停帧。

该修复覆盖了工作线程「回送失败」的路径（渲染线程已销毁接收端 → `SendError` → 工作线程
自行归还），但**没有覆盖回送成功的窄窗口**：

- 渲染线程 `recv_timeout(100ms)` 超时返回后，接收端要活到 `acquire_texture` 返回才析构；
- 工作线程若恰在这个窗口里把纹理送进通道，`send` 成功（接收端仍在），纹理就躺在通道缓冲里；
- 函数返回 → 接收端析构 → 通道缓冲析构 → `SurfaceTexture` 未呈现即被丢弃 →
  Vulkan 的 `discard_texture` 空实现 → **那张交换链图像永不归还**。

结果是：迟到纹理**既没被工作线程归还，也没被渲染线程归还**，正是上一轮修复要杜绝的
「永久停帧」成因，只是概率低（窗口只有几百纳秒，且要求工作线程恰好在超时后返回）。

## What Changes

- `render-stability` 的「取到的交换链纹理必须呈现归还」要求补齐第三条路径的精确口径：
  迟到结果 MUST 恰好由一方归还，「放弃」与「回送」MUST 互斥。
- 实现：新增 `ResultHandoff` 交接协议——渲染线程放弃与工作线程回送在同一把锁内互斥。
  - 渲染线程先放弃：工作线程拿到结果后自行归还，不再回送；
  - 工作线程先回送：渲染线程在锁内看到「已回送」，从通道取回结果并自行归还。
- 回归测试：三条用例钉住「结果至多被一方取走、且必被取走」。

## 不做什么

- 不放宽取纹理的 100ms 期限：它是 Mali 驱动 `vkAcquireNextImageKHR` 永久卡死时的唯一保护。
- 不改 `release_acquired_texture` 的语义（清成背景色后呈现）：那是上一轮已定的归还方式。
- 不动 `docs/STATUS.md`：该文件首行声明禁止修改。

## Capabilities

### New Capabilities

（无）

### Modified Capabilities

- `render-stability`: 细化「取到的交换链纹理必须呈现归还」里迟到结果一条的归还责任归属。

## Impact

- Affected specs: `render-stability`（一条要求的第三条路径与新增场景）。
- Affected code: `native/src/render/pass/acquire.rs`（交接协议、工作线程回送、渲染线程超时路径）、
  `native/src/render/pass/tests.rs`（回归测试）。
- 行为变化：无可见变化。修复的是「迟到纹理无人归还」这一低概率路径，
  命中时终端不再因交换链图像耗尽而永久停帧。
