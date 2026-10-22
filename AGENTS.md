# AGENTS.md

基于 `wgpu`（Vulkan）渲染、`Ghostty` VT 解析（`libghostty-vt`）与 Kotlin + Compose UI 的 Android 终端模拟器。

## 规范优先级

- `docs/specification/` 是行为与架构的规范来源，可能滞后于用户指令，两者冲突时以用户明确指令为准。
- openspec/specs openspec/changes 目录使用 openspec 命令管理，openspec/specs 目录需要保持更新，修改代码后更新实际情况和补充实现细节，openspec 文档仅供参考。
- 规范未覆盖的实现细节，按既有行为与参考项目和最佳实践处理，无法确定时先提问。
- 用户提示和文档中的“代码”一词不包括“注释”；“文档”指 Markdown 等文档文件。
- 规范与用户提示不可能面面俱到，不得咬文嚼字，不得以规范文档反驳用户提示。

## 开工前

- 修改任何文件前，阅读 `docs/specification/` 下的全部文档。
- 不使用子代理进行独立审阅

## 受保护文件

以下文件更改时需谨慎，须遵循 `docs/specification/` 和用户要求：

- `Cargo.toml`
- `build.gradle.kts`
- `settings.gradle.kts`
- `detekt.yml`
- `lint.xml`
- `docs/persistence`

以下文件只允许修正拼写 / 语法错误，或修正格式 / 排版。需用户明确同意后方可修改实际内容：

- `.github/`
- `scripts/`
- `flake.nix`
- `rust-toolchain.toml`
- `README.md`
- `AGENTS.md`
- `docs/specification/`
- `docs/STATUS.md`
- `.markdownlint.jsonc`
- `.semgrepignore`
- `.cargo/config.toml`
- `.gitignore`
- `native/build.rs`
- `openspec/config.yaml`

## 受阻时

- 遇到已尝试所有方案仍然无法解决的问题时停止操作并提供说明。
- 必要时下载参考 `https://github.com/android/skills`
