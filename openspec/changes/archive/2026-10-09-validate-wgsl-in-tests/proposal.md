## Why

着色器只在设备上创建管线时才被编译：WGSL 语法或校验错误不会在 `cargo test` 暴露，而是表现为运行期黑屏或无渲染单元，且日志之外无定位信息。本项目有两个着色器（`shaders/cell.wgsl`、`shaders/kitty_graphics.wgsl`），均无任何编译期校验。

`wgpu` 无条件再导出 `wgpu::naga`，校验只需 CPU，不依赖 GPU 适配器，符合 `docs/specification/TESTING.md`「缺少 Mesa lavapipe 时 Vulkan 测试失败而不是跳过」的要求（本测试根本不触及 Vulkan）。

## What Changes

- 着色器源码由两处 `include_str!` 收敛为 `pipeline.rs` 内两个常量，管线创建与校验测试共用同一来源，消除重复。
- 新增测试用 `wgpu::naga` 解析并校验两个着色器模块，校验错误直接作为断言失败信息输出。

## Capabilities

### New Capabilities

无。

### Modified Capabilities

- `render-stability`：着色器在 `cargo test` 阶段完成解析与校验。

## Impact

- `native/src/render/pipeline.rs`：新增着色器源码常量与校验测试。
- 不新增依赖（经 `wgpu::naga` 访问），不新增测试体系。
