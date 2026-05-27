# ADR 001: 语言选择 — Rust 核心 + Kotlin 外壳

**状态**: 已接受
**日期**: 2026-05-26
**决策者**: 项目负责人

---

## 上下文

终端模拟器必须:
1. 以 I/O 线速率解析 VT 转义序列 (可能 >1GB/s)
2. 以 60-120 FPS 渲染字形到 GPU
3. 安全管理 PTY 子进程
4. 集成 Android 平台 API (生命周期, 服务, 剪贴板, 通知)
5. 同时支持人类和 AI 智能体交互模式

候选方案: 纯 Rust, 纯 Kotlin/JVM, 混合 Rust+Kotlin。

## 决策

**混合方案: Rust 用于终端引擎, Kotlin 用于 Android UI 外壳。**

```
┌─────────────────────────────────────┐
│ Kotlin / Jetpack Compose (UI)       │
│ - TerminalView (SurfaceView 宿主)   │
│ - 设置, 导航, 叠加层               │
│ - Android 生命周期, 服务            │
├─────────────────────────────────────┤
│ boltffi Bridge                      │
├─────────────────────────────────────┤
│ Rust Core                           │
│ - VT 解析器 (libghostty-vt crate)   │
│ - PTY I/O (nix forkpty)            │
│ - 单元格网格 + 回滚缓冲            │
│ - 字体管线 (cosmic-text/swash)     │
│ - GPU 渲染器 (wgpu 29)             │
│ - 会话持久化 (postcard, dev-dep)    │
└─────────────────────────────────────┘
```

完整版本锁定见 `docs/ARCHITECTURE.md §技术版本锁定`。

## 理由

### 为什么不是纯 Rust？

| 关注点 | 评估 |
|--------|------|
| **Android UI** | Jetpack Compose 是唯一现代、良好支持的 Android UI 框架。Rust UI 选项 (GPUI, egui, Druid) 没有 Android 生产级方案。 |
| **平台 API** | Android 生命周期、前台服务、SAF、剪贴板、生物识别——全部需要 Kotlin/Java API。纯 Rust JNI 桥接增加的复杂度等于混合方案。 |
| **迭代速度** | Kotlin UI 代码迭代显著更快。UI 层频繁变更；核心引擎很少变更。 |

### 为什么不是纯 Kotlin/JVM？

| 关注点 | 评估 |
|--------|------|
| **JVM 上的 VT 解析** | 热 VT 解析器路径中的 GC 暂停导致可见卡顿。每个主要 JVM 终端 (JediTerm, BossTerm) 都依赖原生代码处理 PTY 和 VT。 |
| **无 GPU 文本管线** | Kotlin/JVM 没有 `cosmic-text`/`swash` 等价物。Compose Canvas `drawText` 是 CPU 逐单元格渲染——Termux 的瓶颈。 |
| **内存开销** | JVM 基线 50-100MB。终端模拟器应该空闲 <10MB。 |
| **PTY 安全性** | JNI 中的 fork/exec 是已知的 CVE 级漏洞来源。Rust 类型系统消除了整个 PTY 相关漏洞类别。 |

### 混合方案胜出

| 因素 | 纯 Rust | 纯 Kotlin | 混合 (选择) |
|------|---------|-----------|-------------|
| VT 解析性能 | ✅ 优秀 | ⚠️ GC 依赖 | ✅ 优秀 |
| GPU 文本渲染 | ✅ wgpu + cosmic-text | ❌ 无管线 | ✅ Rust 处理 |
| Android UI 集成 | ❌ 不成熟 | ✅ 最佳 | ✅ Kotlin 处理 |
| PTY 安全性 | ✅ 内存安全 | ⚠️ JNI 风险 | ✅ Rust 处理 |
| 构建复杂度 | ⚠️ Android 交叉编译 | ✅ 原生 | ⚠️ 双工具链 |
| 人体工学 | ⚠️ 陡峭 | ✅ 熟悉 | ⚠️ 双语言 |

构建复杂度成本是一次性设置投资。性能和安全收益是永久的。

## 来自实践的证据

完整研究数据见 `docs/VISION.md`。关键验证:

| 项目 | 技术栈 | 结论 |
|------|--------|------|
| **ZeroAI** | Rust + Kotlin + UniFFI | ✅ UniFFI 生产验证 |
| **Haven/IronRDP** | Rust (UniFFI) + Kotlin | ✅ 大规模 UniFFI 验证 |
| **Bun 重写** | AI 辅助 Rust 1M 行/9 天 | ✅ AI 辅助 Rust 开发可行 |
| **1Password** | C→Rust 完整重写 | ✅ 零内存安全漏洞 |

## 后果

**正面**:
- 热路径同类最佳性能
- 解析/PTY 层内存安全
- 完整 Android 平台 API 访问
- 可嵌入核心 crate (桌面, WASM, 无头)
- Rust 生态系统测试 (属性测试, 模糊测试)

**负面**:
- 双构建系统 (Cargo + Gradle)
- boltffi 边界需要仔细类型设计
- 需精通双语言
- 交叉编译 CI 复杂性

**缓解措施**:
- boltffi 类型安全 Kotlin ↔ Rust 绑定
- CI 矩阵: `cargo nextest` + `./gradlew test` 每 PR 强制执行
- `torvox-gui-android/src/bridge.rs` 跨边界共享 boltffi 类型
- `cargo-ndk v4` 脚本自动化交叉编译
