## ADDED Requirements

### Requirement: Surface 尺寸钳制到设备上限

交换链的可配置尺寸 MUST 由唯一一个纯函数计算：先按跨渲染器固定的
`RENDER_SCALE` 缩放，再钳制到 `[1, device.limits().max_texture_dimension_2d]`。
`attach_surface` 与 `reconfigure_swapchain` 两条挂载路径 MUST 共用该函数，
MUST NOT 各写一份缩放/钳制逻辑——补了钳制的那条路径会给另一条留下缺口，而
该缺口在 wgpu 30 无返回值、只经 `get_current_texture` 的 Lost 状态异步上报，
表现为永久黑屏且没有直接错误。

钳制 MUST 在宿主单测里以判定表直接判定（不经 GPU）：缩放、下限 1、上限钳制
三个不变量各有用例；改动钳制逻辑 MUST 让对应用例判红。

#### Scenario: 超限窗口尺寸被钳制而非交给 configure 失败

- **WHEN** 窗口缩放后尺寸超过 `max_texture_dimension_2d`
- **THEN** 配置尺寸等于该上限，surface 正常出帧，MUST NOT 以超限尺寸 configure

#### Scenario: 两条挂载路径共用同一计算

- **WHEN** 复查 `attach_surface` 与 `reconfigure_swapchain` 的尺寸来源
- **THEN** 两者都调用同一纯函数，MUST NOT 出现第二份缩放/钳制实现

#### Scenario: 下限为一像素

- **WHEN** 传入尺寸为 0（空窗口／尚未布局）
- **THEN** 结果两维都不小于 1，与 wgpu 对零尺寸 surface 的要求一致
