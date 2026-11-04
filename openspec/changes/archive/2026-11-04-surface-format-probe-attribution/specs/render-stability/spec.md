## ADDED Requirements

### Requirement: 交换链格式必须是能承载颜色的格式

交换链格式 MUST 至少有三个颜色分量。应用 MUST 从适配器报告的表面格式里**指名**取
8 位 RGBA 族（`Rgba8Unorm` 优先），退化时也只能退到三通道以上的格式；单通道与双通道
格式 MUST NOT 被选中——它们渲染不出颜色，且 R8（`AHARDWAREBUFFER_FORMAT_R8_UNORM`，
0x38）在部分设备的分配器上根本申请不到。设备只报告这类格式时 MUST 报错，MUST NOT
带着不可分配的格式去配置交换链。

#### Scenario: 设备把 R8 排在首位

- **WHEN** 适配器报告的表面格式以 `R8Unorm` 开头且同时含 `Rgba8Unorm`
- **THEN** 交换链格式 MUST 取 `Rgba8Unorm`，MUST NOT 取 `R8Unorm`

#### Scenario: 只剩单通道格式

- **WHEN** 适配器只报告单通道或双通道格式
- **THEN** 配置 MUST 报错退出，MUST NOT 选中该格式去申请窗口缓冲
