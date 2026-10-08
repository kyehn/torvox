## Why

回归快照机制已实现但从未用于回归：`native/src/terminal/snapshot_test.rs` 的 `capture_snapshot` 与 `diff` 除自身单元测试外无任何调用者，`testdata/` 下 `.seq` 语料为零。该模块的文档注释声明「与 `.seq` 输入文件并排存为 JSON 文件」，说明语料是设计的一部分却未落地，违反 `docs/specification/STYLE.md` 「不得保留死代码」。

后果是终端状态断言全部为逐字段手写比较（`input_output_tests.rs` 23 例、`tests.rs` 116 例），每条只覆盖作者当时想到的一个字段；网格尺寸、行序、回滚顺序、样式等整体状态没有任何一处被整体锁定，`docs/specification/TESTING.md` 列出的「旧行必须按顺序进入回滚」「输入回显与光标」「简体中文显示宽度」「不同字重与颜色文本」等条目均无整体快照对照。

现有 JSON 表示为逐单元序列化，24×80 网格单例约 1920 个对象，直接作为语料不可用。

## What Changes

- 快照表示改为紧凑形态：屏幕按行存文本并去除行尾空白，非默认属性单独列出，回滚区按行存文本。断言粒度不变（仍覆盖字符、前景色、背景色、粗体、斜体、下划线、反显），但语料可被人在代码评审中直接阅读。
- 新增语料驱动回归测试：扫描 `native/src/terminal/testdata/` 下每个 `.seq`，写入终端后采集快照，与同名 `.json` 逐项比对，不一致时输出 `diff` 的逐单元差异与应写入的完整 JSON。
- 语料首个批次覆盖 `TESTING.md` 已声明但无整体对照的终端行为：shell 首行不被吞、回滚区行序、中文双宽占位、输入回显与光标、超长行换行、字重与颜色文本。

语料缺失或内容不符一律判定失败并给出应写入内容，不提供跳过、忽略或静默重写。

## Capabilities

### New Capabilities

- `terminal-state-regression`：终端网格整体状态由语料快照回归锁定，快照表示为紧凑可评审形态。

### Modified Capabilities

无。

## Impact

- `native/src/terminal/snapshot_test.rs`：快照表示改为紧凑形态，新增语料驱动测试。
- `native/src/terminal/testdata/`：新增 `.seq` 输入与 `.json` 期望文件。
- 不新增 crate、不新增依赖、不新增测试体系，全部进入现有 `cargo test`。