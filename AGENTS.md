# AGENTS.md

基于 `wgpu`（Vulkan）渲染、`Ghostty` VT 解析（`libghostty-vt`）与 Kotlin + Compose UI 的 Android 终端模拟器。

## 规范优先级

- `docs/specification/` 是行为与架构的规范来源，可能滞后于用户指令，两者冲突时以用户明确指令为准。
- openspec/specs openspec/changes 目录使用 openspec 命令管理，openspec/specs 目录需要保持更新，修改代码后更新实际情况和补充实现细节，openspec 文档仅供参考。`openspec/` 用于保存更改记录和技术规范文档，作为项目的持久记忆层，必须及时更新。不允许 `openspec/specs/upstream-alignment/` 这种模糊不清的分类，`openspec/specs/` 记录现行规范，不记录修改历史，必须保持整洁，措辞严谨和简洁，不保留不必要或过时的内容。`openspec/` 必须只保留必要内容，非必要内容（如“审阅未发现问题”、“测试未发现问题”等无意义内容不得添加），`openspec/changes/archive/` 也允许在不丢失必要信息的前提下进行清理。
- 规范未覆盖的实现细节，按既有行为与参考项目和最佳实践处理，无法确定时先提问。
- 用户提示和文档中的“代码”一词不包括“注释”；“文档”指 Markdown 等文档文件。
- 规范与用户提示不可能面面俱到，不得咬文嚼字，不得以规范文档反驳用户提示。

## 开工前

- 修改任何文件前，阅读 `docs/specification/` 下的全部文档。
- 不使用子代理进行独立审阅

## 受保护文件

以下文件更改时需谨慎，须遵循 `docs/specification/` 和用户要求，不可随意修改：

- `Cargo.toml`
- `build.gradle.kts`
- `settings.gradle.kts`
- `detekt.yml`
- `lint.xml`
- `docs/persistence`

以下文件和目录只允许修正拼写 / 语法错误，或修正格式 / 排版。需用户明确同意（必须明确修改的文件和预期内容）后方可修改实际内容，严禁非法修改，相对必要的修改必须及时询问或写入 `docs/` 中文档，不得隐瞒问题：

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
- 必要时下载 `https://github.com/android/skills` 参考
