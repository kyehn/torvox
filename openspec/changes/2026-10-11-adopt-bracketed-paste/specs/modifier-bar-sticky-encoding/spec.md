## MODIFIED Requirements

### Requirement: 输入法换行提交归一为 CR

输入法提交文本中的**唯一**换行提交（`"\n"`，多行字段的回车键）MUST 归一为 CR(0x0D)，
修饰键状态 MUST NOT 影响归一结果（IME 只交出一个 `"\n"`，无从得知修饰键）。
多字符提交（拼音候选、滑行输入、输入法内部粘贴）内的换行 MUST 逐字保留：
那是内容本身，逐字换成 CR 会把多行文本粘成一行。

该归一只覆盖**孤立**换行提交。多字符提交在 helix 里仍是 Ctrl+J：本要求只管输入法
提交路径，输入法交出的既成文本不进粘贴编码路径（见 `key-input-encoding`
的「输入法提交文本不经按键编码路径」与本规范同源裁定），故 bracketed paste 的
实现不改变此处行为，MUST NOT 被解读为多行提交已修复。

编辑器属性 MUST 声明 `TYPE_TEXT_FLAG_MULTI_LINE`（回车键形态由此决定），
MUST NOT 使用 `IME_ACTION_NONE`：termux 在该处注明它会让屏幕键盘无法输入换行
（termux-app#221）。

#### Scenario: 输入法回车提交归一为 CR

- **WHEN** 输入法以 `commitText("\n")` 提交回车
- **THEN** PTY 收到 0x0D 而不是 0x0A

#### Scenario: 多字符提交内的换行保留

- **WHEN** 输入法提交 `"a\nb"`
- **THEN** PTY 原样收到 `a` LF `b`

#### Scenario: 剪贴板多行内容与输入法多字符提交行为不同

- **WHEN** bracketed 模式下分别粘贴 `"a\nb"` 与由输入法提交 `"a\nb"`
- **THEN** 粘贴路径发出 bracketed 包裹且换行为 LF，输入法路径不发包裹且换行为 LF
