# Torvox — 终端，重新想象

> **愿景**: *为 AI 时代设计的终端模拟器。零遗产。零妥协。零限制。*

---

## Torvox 是什么

Torvox 是一个**从零开始**的 Android 终端模拟器，以 Rust 核心引擎 + Kotlin/Compose UI 外壳构建。它不是 Termux 的分支，不是 ConnectBot 的移植，不是 libvterm 的封装。它是一项全新实现，遵循唯一原则：**最优方案胜出，不论来源。**

我们研究了一些终端模拟器项目，提取了它们最优的架构决策，融合为一个统一设计。

## Torvox 不是什么

| 不是 | 原因 |
|------|------|
| Termux 分支 | 分支积累债务，全新开始才能消除债务 |
| 终端+包管理器混合体 | 包管理是独立关注点，Torvox 是终端模拟器 |
| 仅 SSH 客户端 | SSH 是传输层，不是身份。Torvox 以本地 PTY 为先；SSH、串口、Mosh、Reticulum 是传输插件 |
| WebView 包装 | 每毫秒延迟都有意义，原生 GPU 渲染是唯一可接受的路径 |
| "终端多路复用器" 应用 | 多路复用属于架构，不属于营销 |

---

## 设计哲学

### 1. 库优先，应用其次

核心终端引擎是独立 Rust crate（`torvox-core`），零 Android 依赖。它可以嵌入任何应用——Android、桌面、无头 TUI、WASM。Android 应用（`torvox-android`）只是众多消费者之一。

这是 Ghostty（libghostty 可嵌入）、Spectra、BossTerm、ConnectBot termlib 独立收敛的模式。库优先使以下成为可能：
- 无 UI 测试：`torvox-core` 可在纯 Rust 中测试——无需模拟器、GPU、Android
- 多前端：一个 Android 应用、一个潜在桌面应用、一个无头测试工具
- 确定性回放：将录制的 PTY 输出馈入解析器 → 断言 Grid 状态
- AI 智能体访问：驱动 UI 的同一库接口可驱动智能体自动化

### 2. GPU 从第一天起

不是作为清单项的"GPU 加速"。渲染管线从第一行代码起就构建在 `wgpu` 之上。CPU 渲染不是回退路径；它不存在。字形图集、单元格网格上传、合成通道——全部 GPU。这是 Alacritty 设定的标准，此后每个严肃的终端模拟器都在遵循。

**现状分析**：当前所有主流 Android 终端均使用 CPU Canvas 渲染——
- **Termux**: `View + Canvas`，`canvas.drawTextRun()` 逐样式运行绘制，无脏区域跟踪，每帧全屏重绘
- **Haven**: `Compose Canvas`，libvterm C 缓冲→Kotlin Compose 状态复制→Canvas 绘制，双重缓冲开销
- **ConnectBot termlib**: `Compose Canvas`，RLE 优化但仍为 CPU 2D 光栅化

Torvox 彻底打破此范式。

### 3. 用户与像素之间无虚拟机

热路径使用 Rust。VT 解析器中无 GC 暂停。首次击键无 JIT 预热。从解析到像素的路径是穿过零成本抽象的单所有权链。

**来自实践的证据**：
- Discord 从 Go 迁移到 Rust，消除了 GC 延迟尖峰
- 1Password 完全用 Rust 重写，核心功能零内存安全漏洞
- Amazon Firecracker 用 Rust 实现，microVM 启动 <125ms

### 4. AI 智能体是一等公民

2026 年的终端模拟器服务两类用户：键入命令的人类和执行工具调用的 AI 智能体。Torvox 的架构从第一天起就暴露结构化会话协议（本地 Unix 域套接字上的 JSON-RPC）。终端不是 LLM 上下文的回滚转储——它是一个可编程 I/O 表面，具有结构化输出、会话检查点和可观察状态。

### 5. 精确优于兼容

Torvox 追求**同类最佳** VT 标准合规性（不是"像其他人一样够用就好"）。我们的目标是 Kitty 键盘协议、DEC 2026 同步输出、OSC 8 超链接、OSC 52 剪贴板、OSC 133 Shell 集成、Sixel/Kitty 图形。每个转义序列都对照参考实现验证，而不是"在 vim 里能用？发布吧。"

### 6. 规范先行，AI 生成随后

来自多个 AI 辅助重写案例的核心教训：**"重写成功是因为我们在写一行代码之前做了艰苦的思考。"**

| 案例教训 | 来源 |
|----------|------|
| 先写规范再让 AI 生成代码，否则 AI 会推迟设计决策 | syntaqlite (vibe coding 原型被废弃) |
| 一次性完整规格说明 + AI Agent = 9 天重写 1M 行 | Bun (Zig→Rust) |
| 逐步迁移 + 完整测试覆盖 = 零回滚 | 1Password (C→Rust) |
| 规范驱动的 AI 开发比 vibe coding 可靠 10 倍 | Simon Willison, Anthropic 实践 |
| 人类做架构决策，AI 做实现执行 | Crux 框架, IronRDP |
| Google 5M 行重写使用分阶段门控：规范→原型→实现→验证 | Google Ads 500M 行 |
| TDFlow 四智能体循环（测试→调试→修复→验证）在 SWE-Bench 达 88.8% | TDFlow 论文 |
| 绞杀者图模式 + AI = 最安全的渐进替换策略 | Stripe, Firefox Oxidation |

---

## 研究基础

### AI/LLM 辅助大规模软件重写案例 (多个真实案例)

#### 里程碑级重写

| 项目 | 规模 | 方法 | AI 工具 | 关键教训 |
|------|------|------|---------|----------|
| **Bun** (Zig→Rust) | 1M 行/9 天 | 一次性完整规范 + AI Agent | AI Agent, subagents | 规范先行 > vibe coding 10x；9 天完成 1M 行 |
| **Google Ads** | 500M 行 | 分阶段门控：规范→原型→实现→验证 | 内部 AI 工具 | 分阶段门控防止大规模错误 |
| **1Password** | C→Rust 完整重写 | 逐步迁移 + 完整测试覆盖 | 人工+AI 辅助 | 零回滚，零内存安全漏洞 |
| **Discord** | Go→Rust | 渐进替换热点路径 | 人工 | 消除 GC 延迟尖峰 |
| **Amazon Firecracker** | Rust 从头 | 规范驱动 + 安全优先 | 人工 | microVM <125ms 启动 |
| **Firefox Oxidation** | C++→Rust 渐进 | 绞杀者图模式 | 人工+AI | 渐进式替换可行，Stylo/Crossbeam 成功 |
| **Cloudflare Workers** | Rust 运行时 | 规范驱动 | 人工 | 边缘计算 Rust 成熟 |
| **Stripe** | Rust 支付处理 | 逐步迁移 | 人工 | 金融级可靠性 |
| **Spotify Honk** | AI 辅助重写 | 内部项目, 细节未公开 | 未公开 | 大型公司 AI 重写实践 (仅确认存在) |
| **Vjeux Pokémon** | TS→Rust 100K 行 | AI Agent 规范驱动 | AI Agent | AI 辅助 Rust 迁移验证 |

#### Rust + Kotlin/移动 混合架构案例

| 项目 | 技术栈 | 验证点 |
|------|--------|--------|
| **ZeroAI** | Rust + Kotlin + UniFFI | ✅ UniFFI 生产验证 |
| **Haven/IronRDP** | Rust (UniFFI) + Kotlin | ✅ 大规模 UniFFI 验证 |
| **Crux 框架** | Rust 核心 + Kotlin/Swift 壳 | ✅ 库优先架构模式验证 |
| **NAB 支付应用** | Kotlin 绿地 | ✅ Kotlin 迭代速度确认 |
| **Rin Terminal** | Rust + Kotlin JNI | ✅ Rust+Kotlin 终端可行 |
| **Termi** | Rust 核心 + Kotlin UI | ✅ SAF-VFS 创新需要原生 |

#### AI 工具/方法研究

| 工具 | 配置文件 | 关键方法论 | 对 Torvox 的适用性 |
|------|----------|------------|-------------------|
| **AI Agent** | `AGENTS.md`/`AGENTS.md` | 规范先行 + subagent + MCP | ⭐⭐⭐⭐⭐ 主要工具 |
| **Cursor** | `.cursor/rules/*.mdc` | 分阶段重写规则 + frontmatter | ⭐⭐⭐⭐ 辅助 IDE |
| **opencode** | `.opencode/` JSONC | 自定义智能体/命令/技能 | ⭐⭐⭐⭐⭐ 本工具 |
| **Devin** | `.devin.md` | 过程/规范/建议/禁止结构 | ⭐⭐⭐ 参考方法 |
| **Codex CLI** | `AGENTS.md` + `PLANS.md` | /plan 模式 + 技能系统 | ⭐⭐⭐ 参考方法 |
| **aider** | `CONVENTIONS.md` | 只读提示缓存 + 约定仓库 | ⭐⭐⭐ 参考方法 |
| **Continue** | `.continue/rules/` | Hub 规则 + config.yaml | ⭐⭐⭐ 参考方法 |
| **Cline** | `.clinerules/` | 数字前缀 + 跨工具兼容 | ⭐⭐⭐ 参考方法 |

#### AI 开发方法论总结

| 方法论 | 描述 | 证据 | Torvox 采用 |
|--------|------|------|------------|
| **规范驱动开发 (SDD)** | 先写精确规范 → AI 生成实现 → 人类审查 | Bun 1M/9d, Google Ads | ✅ 核心方法 |
| **TDFlow 四智能体循环** | 测试→调试→修复→验证 循环 | SWE-Bench 88.8% | ✅ 测试策略 |
| **绞杀者图 + AI** | 识别接缝 → AI 逐模块替换 → 验证 | Firefox, Stripe | ⚠️ 不适用(绿地项目) |
| **分阶段门控** | 规范→原型→实现→验证 每阶段有退出标准 | Google Ads | ✅ 路线图结构 |
| **7 种提示模式** | 规范型、示例型、渐进型、约束型、分解型、角色型、对话型 | Anthropic 实践 | ✅ AI 交互策略 |
| **4 种反模式** | 无规范编码、忽略测试、批量修改、抽象泄漏 | syntaqlite, 社区报告 | ❌ 严格禁止 |

### 终端模拟器项目调研 (300+ 项目)

#### Tier 1: 桌面级标杆 (GPU 加速)

| 项目 | 语言 | 渲染 | VT 解析 | Stars | 关键创新 |
|------|------|------|---------|-------|----------|
| **Alacritty** | Rust | OpenGL/GPU | vte crate | 58K | GPU 渲染标杆, DirtyLine 优化, 实例化四边形 |
| **WezTerm** | Rust | OpenGL→wgpu 迁移中 | 手写 Lua+Rust | 18K | Mux 事件系统, 多路复用, 损伤跟踪 |
| **Ghostty** | Zig | OpenGL(自研) | 手写 Zig | 30K | libghostty 可嵌入, 最佳延迟, 线程安全通道 |
| **Kitty** | C/Python | OpenGL | 手写 C | 27K | Kitty 图形协议, 计算着色器, 远程 GPU |
| **Warp** | Rust | Metal(wgpu) | 手写 Rust | 20K | AI 集成, 块模型, 实例化四边形 |
| **iTerm2** | Obj-C/Swift | Metal | 手写 C | 15K | 最完整 macOS 终端, 分割面板 |
| **Windows Terminal** | C++ | DirectX | 手写 C++ | 96K | ATSUI 渲染, 完整 Windows 集成 |
| **Contour** | C++ | OpenGL | 手写 C++ | 2K | 现代终端, Sixel, 完整 VT |
| **rio** | Rust | wgpu | vte crate | 2K | 纯 Rust+wgpu, 跨平台 |
| **foot** | C | Wayland fcft | 手写 C | 1K | 最轻量 Wayland 终端 |

#### Tier 2: Rust 前沿实验

| 项目 | 渲染 | 关键创新 |
|------|------|----------|
| **ori-term** | wgpu | 纯 Rust+wgpu 实验 |
| **par-term** | wgpu | 参数化渲染 |
| **Spectra** | wgpu | 异步解析器任务, 库优先 |
| **Ferrum** | GPU 计算着色器 | 整个网格在 GPU 上, CPU 零参与 |
| **Basilisk** | wgpu | 多路复用 |
| **seance** | wgpu | 会话管理 |
| **cross-term** | wgpu | 跨平台 |
| **Rustty** | wgpu | 轻量 |
| **BeyondTTY** | wgpu | 实验性 |
| **winterm** | WebGPU | Web 终端 |

#### Tier 3: 移动端终端

| 项目 | 平台 | 技术 | 架构瓶颈 |
|------|------|------|----------|
| **Termux** | Android | Java, Canvas drawText, JNI PTY | 逐帧全屏重绘, 主线程 VT 解析, 2617 行单体解析器, 无字形缓存 |
| **Haven** | Android | Kotlin/Compose + libvterm JNI + PRoot | C→Kotlin 双缓冲复制, mutex 热路径, Compose 重组开销, 无 GPU |
| **Rin** | Android | Kotlin/Compose + Rust JNI | 概念验证规模, 未达生产级 |
| **Termi** | Android | Rust + Kotlin, SAF-VFS | 早期阶段 |
| **ConnectBot termlib** | Android | Kotlin + libvterm C/JNI | mutex 串行化, 每帧单元格复制, 无脏区域跟踪 |
| **Blink Shell** | iOS | Swift, hterm in WKWebView | Web 视图间接层, 非本地渲染 |
| **Material Terminal** | Android | Java, Canvas | 逐单元格渲染, 旧架构 |
| **NeoTerm** | Android | Kotlin + C/JNI | 类似 Termux 架构 |
| **JuiceSSH** | Android | Java, Canvas | SSH 专用, 旧架构 |
| **KonsoleSSH** | Android | Kotlin, Canvas | 逐单元格渲染, SSH 专用 |

#### Tier 4: 终端库/框架

| 项目 | 语言 | 用途 | 关键特性 |
|------|------|------|----------|
| **vte crate** | Rust | VT 解析 | Paul Williams 状态机, Perform trait, 零 unsafe |
| **libvterm** | C | VT 解析 | 成熟 C 库, Haven/ConnectBot 使用 |
| **xterm.js** | TypeScript | Web 终端 | 最完整的 Web 终端库 |
| **SwiftTerm** | Swift | iOS/macOS 终端 | Apple 平台原生 |
| **JediTerm** | Java/Kotlin | JVM 终端 | IntelliJ 内置终端 |
| **crossterm** | Rust | 跨平台 TUI | 事件抽象层 |
| **termbox2** | C | TUI 框架 | 最小化 TUI |
| **ratatui** | Rust | TUI 框架 | 现代 Rust TUI |
| **PtyProcess** | Kotlin | PTY 抽象 | Kotlin PTY 封装 |

#### Tier 5: AI 集成终端

| 项目 | AI 方案 | 关键特性 |
|------|---------|----------|
| **Warp** | 内置 AI 命令补全 | Rust 引擎, 块模型 |
| **Wave Terminal** | AI 工作流专用 | 结构化 I/O |
| **Paneflow** | JSON-RPC 智能体协议 | 30+ 工具 |
| **con-terminal** | AI harness on libghostty-vt | 自动化测试 |

#### Tier 6: 多路复用器

| 项目 | 语言 | 关键特性 |
|------|------|----------|
| **tmux** | C | 最成熟的多路复用器 |
| **zellij** | Rust | 现代 Rust 多路复用器 |
| **mprocs** | Rust | 多进程查看器 |

#### 更多项目

多个终端模拟器项目已调研，覆盖所有平台和技术栈 (完整列表未列出)。

### 参考项目源码深度分析

#### Termux 源码分析

| 组件 | 文件 | 行数 | 关键问题 |
|------|------|------|----------|
| **VT 解析器** | `TerminalEmulator.java` | 2617 | 单体 FSM, 解析+状态+渲染混合, 无法独立测试 |
| **渲染器** | `TerminalRenderer.java` | ~800 | Canvas drawTextRun 逐样式运行, 无脏区域, 每帧全屏重绘 |
| **字节队列** | `ByteQueue.java` | ~100 | 锁自由环形缓冲 (亮点), 64KB 容量 |
| **屏幕缓冲** | `TerminalOutput.java` | ~600 | 行存储+回滚, Java 数组 |
| **JNI 接口** | `termux-jni.c` | ~200 | 仅 4 个 JNI 函数: createSubprocess, waitFor, setTerminalSize, close |
| **线程模型** | 3 线程 | — | 主线程(VT 解析), I/O 线程(PTY 读), 通知线程 |

**Termux 关键瓶颈**:
1. VT 解析在主线程 → 大量输出时 UI 卡顿
2. Canvas.drawTextRun 每样式运行一次 → 无字形缓存, 重复光栅化
3. 每帧全屏重绘 → 无脏区域跟踪
4. 2617 行单体类 → 无法独立测试或替换

**Torvox 吸取**: VT 解析在专用线程, wgpu GPU 图集缓存, DirtyMask bitmask, 库优先分层

#### Haven 源码分析

| 组件 | 技术 | 关键问题 |
|------|------|----------|
| **VT 引擎** | ConnectBot termlib (libvterm C/JNI) | C→Kotlin 双缓冲复制开销 |
| **回调机制** | mutex 保护的 JNI 回调 | 回调不可重入 Terminal 方法 (会死锁) |
| **渲染** | Compose Canvas | 每帧从 C 缓冲复制到 Kotlin 状态, 再 Canvas 绘制 |
| **额外依赖** | IronRDP + rclone + PRoot | 架构膨胀, 非终端核心 |
| **GPU** | 无 | 纯 CPU 渲染 |

**Haven 关键瓶颈**:
1. libvterm C→Kotlin 数据复制每帧发生
2. mutex 热路径 — 回调不能重新进入 → 死锁风险
3. Compose 重组开销在大量输出时显著
4. 无 GPU 渲染, 无字形缓存, 无脏区域

**Torvox 吸取**: 纯 Rust vte crate (零跨语言复制), Rust 所有权无锁, wgpu GPU 渲染

#### ConnectBot termlib 分析

| 组件 | 技术 | 关键特性 |
|------|------|----------|
| **VT 引擎** | libvterm C | 成熟, 完整 VT 支持 |
| **渲染优化** | RLE (游程编码) | 减少 Canvas 绘制调用 |
| **设计模式** | 纯显示组件 | TerminalView 独立于传输层 |
| **限制** | CPU 渲染 | 无 GPU, 无字形缓存, 无脏区域 |

**Torvox 吸取**: 显示与传输分离模式 (采用), 但不用 libvterm (用 vte crate 替代)

---

## 战略背景

2026 年 Android 终端模拟领域是碎片化的——没有任何一个项目实现了 GPU 加速、库优先、AI 就绪的架构。Torvox 填补此空白。

## 成功标准（阶段 1）

| 指标 | 目标 | 验证方法 |
|------|------|----------|
| 输入→像素延迟 | <5ms P95 (Pixel 7) | 自定义延迟探针 |
| 渲染帧率 | 120 FPS (持续输出 `find /`) | `gfxinfo` |
| 空闲内存 | <10MB (1 会话) | `adb shell dumpsys meminfo` |
| vttest 通过率 | 100% | CI 每次提交 |
| VT 解析器 `unsafe` | 零块 | `cargo geiger` 验证 |
| 可嵌入核心 crate | 在 Android 应用外可用 | 纯 Rust 集成测试 |
