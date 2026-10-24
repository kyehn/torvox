## Why

`50b09fd7`（「删除未发现本项目可解问题的上游测试资产」）在删除 VT 语料时，连带删掉了
`native/src/render/pipeline.rs` 里的 `CELL_SHADER`/`KGP_SHADER` 常量与 naga 校验测试，
并把主规范 `render-stability` 的「着色器在测试阶段完成解析与校验」一并移除。该删除没有
替代项：当前两个 WGSL 着色器只在设备创建管线时才被解析，语法或校验错误会表现为运行期
黑屏/无渲染，且 `cargo test` 不会失败。

## What Changes

- 恢复 `CELL_SHADER`/`KGP_SHADER` 两个单一来源常量，管线创建与校验测试共用。
- 恢复用 `wgpu::naga` 的 CPU 解析、校验与入口点断言，失败信息携带 naga 诊断。
- 把该要求写回 `render-stability` 常驻规范（经本变更 delta 归档）。

## Capabilities

### New Capabilities

无。

### Modified Capabilities

- `render-stability`：着色器在 `cargo test` 阶段完成解析与校验。

## Impact

- `native/src/render/pipeline.rs`：恢复常量与测试。
- `openspec/specs/render-stability/spec.md`：归档时补回丢失的要求。
- 不新增依赖（经 `wgpu::naga` 访问），不改变渲染行为。
