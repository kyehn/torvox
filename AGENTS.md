# AGENTS.md

基于 `wgpu`（Vulkan）渲染、`Ghostty` VT 解析（`libghostty-vt-sys`）与 Kotlin + Compose UI 的 Android 终端模拟器。

## 必须

- 修改任何文件前阅读 `docs/specification/` 下的全部文档。
- openspec/specs 目录保存 项目功能及其他 的详细设计规范文档，使用 openspec 命令管理，需要保持更新和正确，修改前编写对应的 changes（完成后进行归档和删除） 和 specs（修改验证后对文档进行更新实际情况和补充实现细节） 文档，openspec/specs 只是参考文档不是严格规范，置信度较 docs/specification/ 低，以 docs/specification/ 和用户提示为实际标准。
- 用户提示和文档中使用的 “代码” 一词不包括 “注释”。而 “文档” 一词指的是如 Markdown 文件之类

## 受阻时

- 缺少依赖时：优先检查 `flake.nix`，再提问。
- 遇到合并冲突时：停止操作并展示冲突文件。
- 优先修复根因：避免通过删除文件、跳过测试或添加 `#[allow(...)]` 来掩盖问题。

## 禁止修改文件和目录，文件不得修改，目录递归要求并且不得创建/删除/重命名文件

只允许修正拼写 / 语法错误，或修正格式 / 排版，不可更改实际内容，未经询问不可修改错误或其他问题，修复错误或其他阻塞项必须询问用户且避免不必要的修改（修改后重新设置只读属性）。需用户明确同意后方可修改，禁止非法修改。

- `.github/`、`scripts/`、`flake.nix`、`rust-toolchain.toml`、`README.md` `AGENTS.md`、`docs/specification/` `.markdownlint.jsonc` `.semgrepignore` `.cargo/config.toml` `.gitignore`

## 不得轻易更改文件，必须确保符合 docs/specification/ 要求

`Cargo.toml` `build.gradle.kts`  `settings.gradle.kts` `detekt.yml` `lint.xml`
