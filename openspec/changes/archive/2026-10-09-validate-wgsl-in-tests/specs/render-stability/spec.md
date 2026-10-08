## ADDED Requirements

### Requirement: 着色器在测试阶段完成解析与校验

终端单元与 Kitty 图形协议的 WGSL 着色器 MUST 在 `cargo test` 阶段完成解析与校验，
MUST NOT 仅依赖设备上创建管线时才暴露错误。校验 MUST 只用 CPU，MUST NOT 依赖 GPU
适配器，校验失败 MUST 以 naga 的诊断信息作为断言失败信息输出。

着色器源码 MUST 为单一来源：管线创建与校验测试 MUST 引用同一处定义，MUST NOT 各自
内联 `include_str!`。

#### Scenario: 着色器语法错误时测试失败并给出诊断

- **WHEN** 任一着色器无法被 naga 解析或校验
- **THEN** 测试失败，输出该着色器的 naga 诊断信息

#### Scenario: 着色器合法时测试通过

- **WHEN** 两个着色器均通过解析与校验
- **THEN** 测试通过
