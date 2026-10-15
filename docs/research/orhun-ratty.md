# orhun/ratty 研究报告

## 仓库概况
- 克隆路径：`/tmp/opencode/ratty`
- 技术栈：Rust + Bevy + Ratatui + portable-pty + wgpu (Bevy wgpu backend)；GPU 渲染终端
- 定位：GPU 渲染终端 + inline 3D 与特效（3D mode、Mobius、Kitty 图像）
- 许可证：MIT（根 LICENSE 明示）。
- 主要模块：terminal、runtime、present、rendering、camera、screen、scene、keyboard/mouse、plugin、rgp、kitty

## 全仓库结构与关键代码
- `src/terminal.rs`：终端状态与 Ratatui 集成。
- `src/runtime.rs`：运行时事件/更新循环。
- `src/present.rs`/`rendering.rs`/`screen.rs`：渲染管线、present shader `terminal_present.wgsl`。
- `src/scene/mobius.rs`：3D mode。
- `src/kitty.rs`/`inline.rs`/`rgp.rs`：Kitty 图像协议与 inline 3D。
- `src/keyboard.rs`/`mouse.rs`：输入。
- `src/plugin.rs`：插件系统。
- `src/bin/ratty-mcp.rs`：MCP server 二进制。
- flake/nix packaging。

## 对比本项目
- 本项目 Android Rust + wgpu/Vulkan + Kotlin/Compose，终端状态 Ghostty。
- ratty 使用 Ratatui + Bevy 进行 GPU 渲染，wgpu 与本仓同栈，可作 GPU 渲染与 2D present shader 参考。
- 终端状态由 Ratatui (VT 解析不同)，与本仓 Ghostty 单一来源冲突；不能替换 VT 核。
- desktop Bevy 依赖巨大，不适合 Android min SDK/包体积，但 shader/rendering approaches 可借鉴。

## 可直接使用的库/工具
- `portable-pty` Rust crate 可直接在 Android? PTY fork/exec differs; portable-pty on Unix uses standard PTY; Android Linux supports standard PTY syscall semantics, but Android application process management differs. Not required.
- Bevy/wgpu 渲染栈 too heavy for Android terminal; wgpu itself already used by本仓.
- `ratatui` not suitable for GPU cell grid with Ghostty.

## 可间接借鉴
- wgpu present shader and rendering pipeline patterns.
- Kitty graphics protocol implementation via `kitty.rs`.
- window warp / 3D presentation as R&D.
- font/atlas? better inspect not now.
- plugin/MCP integration.

## 可稍微修改使用的代码/设计
- Present shader `terminal_present.wgsl` and rendering systems be adapted to wgpu backend, but本仓 has optimized cell rendering already.
- portable-pty can be replaced evaluation for Android, but Termux-style pty (forkpty) preferred.
- Tests? no strong.

## 测试数据/测试用例
- No significant unit tests seen; demo-heavy. Limited test assets.

## 设计模式
- Bevy ECS architecture for terminal UI.
- State resource for terminal; camera/present separation.
- Inline graphics via Kitty protocol.

## 其他结论
- Ratty is novelty/3D terminal; high product risk. GPU rendering reference only.
- WGSL present shader can inform本仓 renderer but no direct reuse.

## 四次审阅记录
- 准确性：基于 source and Cargo.
- 完整性：涵盖 repo, comparison, libraries, indirect, tests, patterns, conclusions.
- 对比正确性：未建议 adopt Ratatui VT; stresses desktop only.
- 文档一致性：仅新增 md.
