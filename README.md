# Torvox

基于 `wgpu`（Vulkan）渲染、`Ghostty` VT 解析（`libghostty-vt-sys`）、Rust 引擎与 Kotlin / Compose UI 的 GPU 加速 Android 终端模拟器；兼容 `com.termux`，适配 Termux 生态。

## 技术栈

- **渲染**：`wgpu` / Vulkan，无 CPU / OpenGL 回退
- **终端解析**：`libghostty-vt-sys`（上游 `libghostty-vt`，跟踪 git master）
- **引擎**：Rust（会话、PTY、渲染均由 Rust 管理）
- **界面**：Kotlin + Compose
- **兼容性**：`applicationId = "com.termux"`，AOSP testkey 签名（`android/app/aosp-testkey.p12`）

## 快速开始

环境由 `flake.nix` 声明，请使用 `nix develop` 进入开发环境。

```sh
nix develop
```

详细构建流程见 [`docs/specification/BUILD.md`](docs/specification/BUILD.md)。

## 文档

| 文档 | 说明 |
| --- | --- |
| [`docs/specification/design.md`](docs/specification/design.md) | 架构与产品设计 |
| [`docs/specification/BUILD.md`](docs/specification/BUILD.md) | 构建指南 |
| [`docs/specification/STYLE.md`](docs/specification/STYLE.md) | 样式指南 |
| [`docs/specification/TESTING.md`](docs/specification/TESTING.md) | 测试指南 |
| [`AGENTS.md`](AGENTS.md) | 协作约束与工作流 |

修改前请先阅读 `docs/specification/` 下的全部规范。
