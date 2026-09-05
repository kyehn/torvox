# AGENTS.md

基于 `wgpu`（Vulkan）渲染、`Ghostty` VT 解析（`libghostty-vt-sys`）与 Kotlin + Compose UI 的 GPU 加速 Android 终端模拟器。

## 必须

- 修改前阅读 `docs/specification/` 下的全部规范。
- openspec/specs 目录保存 项目功能及其他 的详细设计规范文档，使用 openspec 命令管理，需要保持更新和正确，只是参考文档，docs/specification/ 为实际标准。

## 禁止

- Java 文件、`portable-pty`、`rust-android-gradle`
- 逐单元格 `Canvas.drawText`、跨 FFI 传递原始字节、`/proc/self/exe`
- 在库 crate 中使用 `anyhow` — 请使用 `thiserror 2`
- 在核心终端数据路径中使用 `unsafe`
- 基于反射的 JNA 绑定 — 如发现残留 JNA 代码请直接删除
- bash / sh 脚本 — 仅可使用 Nushell

---

## 编码规范

- 编写任何文件前阅读 `docs/specification/`。
- 禁止魔数：使用具名常量，并赋予描述性名称。
- 禁止缩写：使用 `config` 而非 `cfg`、`background` 而非 `bg`、`terminal` 而非 `term`。
- 生产代码中禁止 `#[allow]`（测试辅助代码除外）。
- 禁止硬编码 `/data/.*/files` 形式的应用数据路径。
- Nushell 脚本中禁止 `||`（无效语法）。
- Rust：使用 `std::hint::black_box`，而非已弃用的 `criterion::black_box`。
- Kotlin：使用 `SharingStarted.WhileSubscribed(TIMEOUT_MILLIS)`，并以具名常量提供超时时间。

## 受阻时

- 缺少依赖时：优先检查 `flake.nix`，再提问。
- 遇到合并冲突时：停止操作并展示冲突文件。
- 优先修复根因：避免通过删除文件、跳过测试或添加 `#[allow(...)]` 来掩盖问题。

## 禁止修改文件

只允许修正拼写 / 语法错误，或修正格式 / 排版，不可更改实际内容。需用户明确要求后方可修改，禁止非法修改

- `.github/`、`scripts/`、`flake.nix`、`rust-toolchain.toml`、`README.md` `AGENTS.md`、`docs/specification/`
