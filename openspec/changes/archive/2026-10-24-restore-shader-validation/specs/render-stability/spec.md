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
- **THEN** 测试通过，且入口点集合恰为 `vs_main` 与 `fs_main`

实现细节：

- 着色器源码为 `render/pipeline.rs` 的 `CELL_SHADER` 与 `KGP_SHADER` 两个常量，
  由 `include_str!` 编译期内联；管线创建与校验测试共用，不存在第二处内联。
- 校验经 `wgpu::naga`（`wgpu` 无条件再导出 `wgpu::naga`）完成，无需新增依赖，也无需
  GPU 适配器，因此在无 lavapipe 的环境下同样执行而非跳过。
- 校验分两步且分别断言：先 `wgsl::parse_str` 解析，再以
  `Validator::new(ValidationFlags::all(), Capabilities::default())` 校验 IR；两步失败
  均以 `emit_to_string` 的 naga 诊断作为断言信息。
