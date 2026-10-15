# vercel-labs/wterm 研究报告

## 仓库概况
- 克隆路径：`/tmp/opencode/wterm`
- 技术栈：Zig + WASM 核（約 26 KB release）+ TypeScript/JavaScript 包，pnpm monorepo + Turbo + Vitest；可选 `@wterm/ghostty` 通过 Zig/WASM 封装 libghostty
- 定位：Web 终端模拟器，DOM 渲染，支持 React/Vue/Svelte
- 许可证：Apache-2.0
- 主要包：`@wterm/core`、`@wterm/dom`、`@wterm/react`、`@wterm/vue`、`@wterm/svelte`、`@wterm/ghostty`、`@wterm/just-bash`、`@wterm/markdown`
- 状态：Vercel Labs 实验，长期维护中

## 全仓库结构与关键代码
- `src/*.zig`：内置 Zig VT 解析器（`parser.zig`、`terminal.zig`、`grid.zig`、`cell.zig`、`scrollback.zig`、`charset.zig`、`unicode_width.zig`、`hyperlink.zig`、`wasm_api.zig`），编译到 `packages/@wterm/core/wasm/wterm.wasm`。
- `packages/@wterm/core/src`：`TerminalCore` 接口、WASM bridge、内存加载、WebSocket transport；强类型测试覆盖 bridge/内存/WASM 加载/传输。
- `packages/@wterm/dom/src`：DOM renderer、输入、选择、搜索、选区、文本捕获、tracked selection、图形层、可访问性。
- `packages/@wterm/ghostty`：通过 Zig/WASM 封装 libghostty 的全功能核心；包含 `zig/src/wasm_api.zig`、绑定 `wasm-bindings.ts`、`ghostty-core.ts`，大量 TS 测试。
- `packages/@wterm/*`：框架绑定、just-bash、markdown。
- `e2e`、`apps/docs`：端到端与文档站。

## 对比本项目
- 本项目为 Android Rust + wgpu/Vulkan + Kotlin/Compose，终端状态以 `libghostty-vt` 为唯一来源，Rust 宿主进程与 PTY。
- wterm 为 Web/WASM 形态，渲染到 DOM；其 Zig 核是自研 VT 解析器（非 Ghostty）的替代形态。
- Zig/WASM 核与 Rust 不能直接嵌入 Android JNI，Zig 从源构建产物也不符合本仓 Rust-only/FFI 契约。
- `@wterm/ghostty` 证明 libghostty 可作 WASM 后端，但本仓已直接使用 `libghostty-vt-sys` Rust 绑定，WASM 层额外。

## 可直接使用的库/工具
- 无 Rust/Kotlin/Android 依赖可直接使用。
- Zig 代码只能作为算法与实现参考；本仓不得 vendoring 或引入片段（STYLE.md/build 无 vendor）。
- Web/WASM PTY transport、重连与背压设计可作为网络终端功能的未来参考（本仓当前无网络终端）。

## 可间接借鉴
- DOM 渲染中的脏行跟踪、部分重绘、帧直接调度（`requestAnimationFrame`）思路可映射到 Compose/WGPU 的快速路径。
- WebSocket 传输的背压与重连设计（`transport.ts`、测试）。
- Kitty 图像、OSC 序列、软换行元数据、搜索 across scrollback 等终端特性矩阵可作需求对照。
- 可访问性、文本捕获、readText() API 设计。
- Ghostty 封装层的内存管理与页面零初始化修复。

## 可稍微修改使用的代码/设计
- TS/JS 层不能直接移植到 Kotlin；需按 Compose 重写。
- Zig 终端解析可作为自研 VT 的反例/参考（本仓不自研 VT），只能用于理解。
- 文档与测试可作验收用例池，但期望值必须按 upstream-alignment 三条准入门槛判定；不直接引入自验证期望。
- `check-wasm`/CI 二进制比对流程可作为本仓 wgpu shader/artifact 一致性的工程参考（不直接适用）。

## 测试数据/测试用例
- Vitest 单测覆盖 bridge、内存、WASM 加载、WS transport（core）；DOM 渲染、输入、剪贴板、搜索、选区、文本捕获、kitty、cursor 等（dom）；ghostty 核心初始化、内存、主题、剪贴板、hyperlink、scrollback、prompt 导航、row metadata、responses、graphics 等（ghostty）。
- e2e Playwright 测试（`e2e/`）。
- 提供 docs 与测试网页作为端到端验证场景。
- 不提供可直接用于本仓 Rust/JNI 的测试数据；所有期望值来自 Web/DOM 行为，需重新判定。

## 设计模式
- Pluggable core：`TerminalCore` 接口 + Zig/WASM 或 Ghostty WASM 两套后端， reflects “终端状态委托上游”的架构模式。
- DOM-first 渲染：把可访问性/原生选择交给浏览器，减少自研 UI。
- Ring buffer scrollback、脏行跟踪、帧调度、soft-wrap 元数据。
- 客户端 WebSocket 传输与背压。
- 文档站 Geistdocs + 公开 Markdown 导出。

## 其他结论
- 本仓 persistent ecosystem 已把 wterm 评级为 C，结论确认：浏览器/WASM 形态无法作为 Android 依赖；VT 解析与渲染都在别处，与本仓单一来源正面冲突。
- 可借鉴的仅产品面与 transport/accessebility 设计；不可直接作为依赖。
- Ghostty 封装层的 WASM 构建固定 Zig 0.15.2，本仓不适用版本矩阵。

## 四次审阅记录
- 准确性：基于实际克隆树与 README/package/src，无编造路径。
- 完整性：覆盖包矩阵、核心实现、transport、测试、模式、对比。
- 对比正确性：明确 Zig/WASM 与 Web/DOM 形态与本仓 Android/Rust/wgpu/Ghostty 边界冲突。
- 文档一致性：仅新增 Markdown，未动受保护文件；与 persistent ecosystem C 评级一致。
