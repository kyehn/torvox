# Torvox

> GPU 加速 Android 终端模拟器。Rust 核心 + Kotlin/Compose UI。

---

所有文档位于 `docs/`。AI 智能体上下文位于仓库根目录 `AGENTS.md`。

## 技术栈

| 层 | 技术 |
|----|------|
| 核心引擎 | Rust (stable, Edition 2024) |
| GPU 渲染 | wgpu 29 (Vulkan) |
| FFI | UniFFI |
| Android UI | Kotlin + Compose |
| 最低 SDK | Android 13 (API 33, Vulkan 1.3) |

完整版本锁定见 `docs/ARCHITECTURE.md §技术版本锁定`。

## 文档索引

| 文件 | 用途 |
|------|------|
| `AGENTS.md` | AI 智能体首要上下文 |
| `docs/VISION.md` | 项目愿景和设计哲学 |
| `docs/ARCHITECTURE.md` | 技术架构, crate 结构, 数据流 |
| `docs/SPECIFICATION.md` | 标准合规性和性能目标 |
| `docs/ROADMAP.md` | 开发里程碑 |
| `docs/DEVELOPMENT.md` | 构建和开发工作流, CI/CD |
| `docs/ADR/` | 架构决策记录 |
