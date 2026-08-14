# Glossary

本表收录 Torvox 文档与代码中反复出现的核心术语，定义以本仓库文档为权威。
采用 [The Good Docs Project 术语表模板](https://gitlab.com/tgdp/templates/-/tree/main/glossary)。
新增术语时同步维护本表。

| Term | Abbreviation | Definition | Source |
| --- | --- | --- | --- |
| Architecture Decision Record | ADR | 记录一次架构决策的背景、决策与后果的文档，采用 Nygard 格式，由 `adrs` 门禁管理 | [docs/adr/README.md](adr/README.md) |
| arc42 | — | 本项目架构文档采用的分节模板（12 节），由 `check-docs.py` 校验节标题 | [docs/architecture.md](architecture.md) |
| CellData | — | 80 字节 `bytemuck` Pod，跨 FFI 传输的渲染单元数据结构 | [docs/architecture.md](architecture.md) |
| Functional Requirement / Non-Functional Requirement | FR / NFR | 以 `FR-NNN` / `NFR-NNN` 编号的需求条目，权威定义在 `docs/requirements/*.sdoc` | [docs/srs.md](srs.md) |
| GridSnapshot | — | 查询路径使用的网格状态快照（与渲染路径的 CellData 分离） | [docs/architecture.md](architecture.md) |
| ghostty_terminal | — | 终端状态的唯一权威：VT 解析、滚动、光标、选择全部由 vendored Ghostty 库持有 | [ADR-0002](adr/0002-ghostty-source-of-truth.md) |
| Java Native Interface | JNI | Rust 与 Kotlin 的直连桥（`jni` crate），`ffi.rs` 是唯一导出点 | [ADR-0003](adr/0003-bridge-and-rendering-strategy.md) |
| libghostty-vt | — | 通过 `libghostty-vt-sys` vendored 的 Ghostty VT 解析库，本项目的终端状态来源 | [ADR-0002](adr/0002-ghostty-source-of-truth.md) |
| Model Context Protocol | MCP | 嵌入 `native/` 的 JSON-RPC 2.0 工具服务器（tower-mcp），经 Unix socket 或 stdio 提供终端工具 | [ADR-0005](adr/0005-mcp-embed.md) |
| Nushell | — | 项目唯一允许的脚本语言；禁止 bash/sh | [docs/standards/STYLE.md](standards/STYLE.md) |
| Pseudo Terminal | PTY | POSIX 伪终端；`pty.rs` 是唯一允许使用 fork `unsafe` 的位置 | [docs/architecture.md](architecture.md) |
| Render Thread | — | 每会话 4 线程之一：wgpu 渲染循环（也负责 MCP 事件泵） | [ADR-0004](adr/0004-thread-model.md) |
| Session | — | 一个终端会话：PTY + 渲染器 + 4 线程（PTY 读、输入写、进程等待、渲染） | [docs/architecture.md](architecture.md) |
| StrictDoc | — | 需求管理工具，`.sdoc` 是需求 ID 的权威来源 | [docs/srs.md](srs.md) |
| TextureView | — | Android 渲染视图组件；不再使用 SurfaceView + `setZOrderOnTop` 方案 | [ADR-0003](adr/0003-bridge-and-rendering-strategy.md) |
| wgpu / Vulkan | — | GPU 渲染后端：wgpu 底层使用 Vulkan，无物理 GPU 时经 Mesa Lavapipe 软件渲染 | [docs/architecture.md](architecture.md) |
