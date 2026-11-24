## Context

项目规范要求 openspec 现行规范目录整洁、措辞严谨，且安装器诊断 Must 不丢失字符语义。本轮审查以 `code-review-and-quality` 与 `ponytail-audit` 的五轴/优先级框架为准则，对每个子系统做针对性阅读，并用静态与动态检查交叉验证。

## Goals

- 对 terminal/pty/session、渲染/字体、Kotlin runtime/IME/安装器等子系统做最小覆盖式阅读。
- 仅修复有具体错误场景的缺陷，不做无意义修改。
- 每小步提交并推送，保留可核验的测试输出。

## Decisions

- 安装器截断改用既有 `truncateTextAtCharBoundary`，避免新增依赖或重复逻辑。
- 直接删除被禁止的 `upstream-alignment/` 分类及引用行，不新建替代分类。

## Verification

- `cargo clippy --all-targets -- -D warnings`：通过。
- `cargo test`：953 passed，0 failed。
- `testDebugUnitTest`、`detekt`、`spotlessKotlinCheck`：通过。
- 连通测试：`VtCorrectnessInstrumentedTest` 15/15、`terminal.emulator.ui` 144/144、`terminal.emulator.installer` 全部通过。
