# paulrobello/par-term-emu-core-rust 研究报告

## 仓库概况
- 克隆路径：`/tmp/opencode/par-term-emu-core-rust`
- 技术栈：Rust VT engine core, published crate `par-term-emu-core-rust`, includes FFI, Python bindings, web terminal.
- 定位： VT core, rendering state, terminal engine.
- 许可证：MIT
- 目录 extras: web_term, python wrappers, fuzz and tests.

## 全仓库结构与关键代码
- `src/`: core state.
- `crates/` contains additional Rust modules; binding inventory is not expanded in this report.
- `include/cbindgen` C FFI.
- `web-terminal-frontend` web terminal.
- `terminfo`, `shell_integration`.

## 对比本项目
- 本项目 Android Rust + wgpu + Compose + Ghostty VT.
- be direct Rust VT engine as independent component, but single-source Ghostty policy.
- FFI to native renders and external tests/corpus valuable.

## 可直接使用的库/工具
- MIT Rust crate; can embed in Rust Android layer as VT engine alternative.
- C API via cbindgen easier integration.
- Python bindings for fuzz/reference test.

## 可间接借鉴
- VT engine API surface.
- Web terminal frontend interaction.
- Kitty graphics.
- IPC/MCP schemas.
- Tab/mux support.

## 可稍微修改使用的代码/设计
- Rust core can be forked for larger feature coverage, but maintaining FFI and tests adds risk.
- Ghostty already covers wider standard; not adopt unless need.
- MIT generous.

## 测试数据/测试用例
- `tests`, `fuzz`, docs.
- be useful conformance vectors.

## 设计模式
- Library + language bindings.
- Clear separation.
- IPC and test harness.

## 其他结论
- Valuable as both VT engine alternative and Rust API design reference.
- Again single VT source.
- MIT, good legal.

## 四次审阅记录
- 准确性.
- 完整性.
- 对比正确性.
- 文档 consistency.
