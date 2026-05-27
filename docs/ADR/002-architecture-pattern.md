# ADR 002: 架构模式 — 事件驱动、库优先分层架构

**状态**: 已接受
**日期**: 2026-05-26
**决策者**: 项目负责人

---

## 上下文

架构必须支持:
1. 可嵌入核心 (同一引擎用于 Android 应用、桌面应用和无头测试)
2. 多个并发终端会话 (标签, 分割)
3. 人类和 AI 智能体同为消费者
4. 平台特定渲染 (Android Compose, 桌面 wgpu, 无头)
5. 确定性回放用于测试和调试
6. 亚 5ms 输入到像素延迟

## 决策

**库优先、事件驱动分层架构，严格依赖方向。**

```
┌──────────────────────────────────────────────────┐
│ 消费者 │
│ ┌──────────┐ ┌──────────┐ ┌──────────────────┐ │
│ │torvox-app │ │torvox-tui │ │ Custom Embedding │ │
│ │(Compose) │ │(ratatui) │ │ (你的应用) │ │
│ └─────┬────┘ └────┬─────┘ └────────┬─────────┘ │
│ │ │ │ │
├───────┴───────────┴───────────────┴──────────────┤
│ torvox-gui │
│ (平台特定渲染 + 事件分发) │
├──────────────────────────────────────────────────┤
│ torvox-renderer │
│ (wgpu 29 GPU 字形图集, 实例化四边形, 脏区域) │
├──────────────────────────────────────────────────┤
│ torvox-terminal │
│ (VT 解析器, Grid, PTY 会话, 剪贴板) │
├──────────────────────────────────────────────────┤
│ torvox-core (no_std 兼容) │
│ (VT 状态机, Cell 类型, ANSI 颜色, 事件类型) │
└──────────────────────────────────────────────────┘
```

完整版本锁定见 `docs/ARCHITECTURE.md §技术版本锁定`。

### Crate 依赖方向

```
torvox-core (无依赖, 仅 libcore)
↑
torvox-terminal (依赖 torvox-core + libghostty-vt + nix + serde + postcard)
↑
torvox-renderer (依赖 torvox-core + wgpu + cosmic-text + swash)
↑
torvox-gui (依赖 torvox-renderer + winit 0.30 / SurfaceView)
↑
torvox-app / torvox-tui / 自定义消费者
```

### 事件流

```
PTY → [读取线程] → 原始字节 → [flume SPSC 通道] → VT 解析器
→ [Grid 变更 + DirtyMask 集合] → [crossbeam Notify]
→ [渲染线程] → 字形图集更新 → GPU 提交

用户输入 → [UI 线程] → 按键/鼠标事件 → [boltffi 调用]
→ [InputEngine] → 转义序列编码 → PTY 写入

AI 智能体 → [Unix 域套接字/JSON-RPC] → 结构化命令
→ [会话线程] → 与用户输入相同路径
```

## 理由

### 为什么库优先？

Ghostty 确立了此模式 (libghostty 可嵌入)，Spectra、BossTerm 和 ConnectBot termlib 都独立收敛于它。库优先核心 crate 使以下成为可能：

- **无 UI 测试**: `torvox-core` 可在纯 Rust 中测试——无需模拟器、GPU、Android
- **多前端**: 一个 Android 应用、一个潜在桌面应用、一个无头测试工具
- **确定性回放**: 将录制的 PTY 输出馈入解析器 → 断言 Grid 状态
- **AI 智能体访问**: 驱动 UI 的同一库接口可驱动智能体自动化

### 为什么事件驱动？

终端仿真问题天然映射到事件溯源：
- **输入流** (PTY 字节) → **状态机** (VT 解析器) → **状态差** (Grid 变更)
- **用户动作** (键盘) → **事件** → **处理** → **输出流** (PTY 写入)

这是 WezTerm (Mux 事件系统)、Ghostty (线程安全事件通道) 和 Spectra (异步解析器任务) 使用的模式。它使以下成为可能：
- 确定性回放 (记录事件 → 回放 → 比较状态)
- 结构化智能体交互 (AI 发送与键盘相同的事件)
- 性能分析 (按事件类型测量延迟)

### 为什么严格依赖方向？

终端模拟器架构的头号失败模式是解析器、网格、渲染和 UI 状态之间的循环依赖。每个成熟项目 (Alacritty, WezTerm, Ghostty) 都强制严格分层。Torvox 遵循相同规则：**下层永不导入上层**。

**Termux 的反面教训**: Termux 的 `TerminalEmulator.java` (2617 行单体类) 混合了 VT 解析、屏幕状态和渲染逻辑，导致无法独立测试、无法替换渲染器、无法嵌入其他应用。

**Haven 的反面教训**: Haven 的 libvterm JNI 桥接有 mutex 保护，但回调不能重新进入 Terminal 方法 (会死锁)。这是层间耦合不清晰的结果。

### 为什么不用 libvterm？

Haven 和 ConnectBot termlib 使用 libvterm (C 库) 通过 JNI。我们选择不这样做：

| 因素 | libvterm (C/JNI) | libghostty-vt 0.1 crate (Rust) |
|------|-------------------|------------------------|
| 内存安全 | C 代码, JNI 不安全边界 | Rust 安全, 零 unsafe |
| 跨语言开销 | JNI 调用 + C→Kotlin 复制 | 纯 Rust, 无跨语言边界 |
| 锁要求 | mutex 保护每次访问 | Rust 所有权, 无锁 |
| 死锁风险 | 回调不可重入 (已验证) | 无回调, 无死锁可能 |
| 图集集成 | C 不知道 GPU 图集 | 直接 Rust 类型到 wgpu |
| 测试 | 需要 Android 模拟器 | 纯 Rust 单元测试 |
| 序列化 | 需要自定义 C→Rust 桥 | postcard 1.1 直接序列化 (dev-dep) |

### 为什么用 flume 通道而非 tokio？

PTY 读取线程使用阻塞 I/O（这是必须的），flume SPSC 通道提供：
- 零分配 lock-free 通信
- 有界背压（PTY 输出速度 > 解析速度时丢弃旧数据）
- 无异步运行时依赖（减少 100K+ 行 tokio 依赖）

tokio 仅用于会话级任务调度（超时、健康检查），不用于热路径。

## 后果

**正面**:
- `torvox-core` 在 `no_std` 环境编译 (嵌入式, WASM)
- 每个 crate 独立可测试、可基准测试
- AI 辅助代码生成的清晰所有权边界
- 新前端 (桌面, web) 无需触碰引擎

**负面**:
- 更多 crate 边界样板 (公共类型, 序列化)
- 跨 crate 重构需要更多协调
- "可见"进度前的初始设置开销

**缓解措施**:
- boltffi 类型集中在 `torvox-gui-android/src/bridge.rs`
- Workspace 级 `cargo nextest --workspace` 集成测试
- `torvox-integration-tests` crate 跨边界测试
