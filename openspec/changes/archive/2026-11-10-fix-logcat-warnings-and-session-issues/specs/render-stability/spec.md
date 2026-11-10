# render-stability Specification

## ADDED Requirements

### Requirement: 交换链格式必须与窗口缓冲格式一致

宿主挂载 Android 窗口时 MUST 以窗口缓冲格式 `RGBA_8888`
（`WINDOW_FORMAT_RGBA_8888` = `AHARDWAREBUFFER_FORMAT_R8G8B8A8_UNORM`）设置缓冲
几何；交换链 MUST 选用与之同格式的表面格式（Vulkan 上的
`VK_FORMAT_R8G8B8A8_UNORM`，即 wgpu 的 `Rgba8Unorm`）。两处 MUST 取自同一处定义，
MUST NOT 各写一份字面量。

两处不一致时，缓冲队列按另一种格式分配缓冲，厂商 Vulkan 驱动会试分配单通道格式
（Mali 报 `Requested R8 format is not supported with this allocator`），分配失败被
记为 error；即便侥幸分配成功，渲染进单通道目标也只有灰度一个通道，颜色是错的。

单通道与双通道格式 MUST NOT 被选为交换链格式：颜色渲染不出来，且部分设备的分配器
根本分配不出来。

#### Scenario: 窗口缓冲格式与交换链格式同源

- **WHEN** 宿主挂载窗口并创建交换链
- **THEN** 窗口缓冲格式 MUST 为 `RGBA_8888`，交换链 MUST 为 `Rgba8Unorm`，
      两者 MUST 取自同一处定义

#### Scenario: 不得选用单通道交换链格式

- **WHEN** 适配器报告的候选格式里排在前面的单通道格式
- **THEN** 交换链 MUST NOT 选用它，MUST 退到多通道格式；退化 MUST 记一条日志

#### Scenario: 分配器不得因格式被拒收

- **WHEN** 设备分配器不支持被申请的缓冲格式
- **THEN** 该情形 MUST NOT 出现——申请格式 MUST 是分配器支持的格式
