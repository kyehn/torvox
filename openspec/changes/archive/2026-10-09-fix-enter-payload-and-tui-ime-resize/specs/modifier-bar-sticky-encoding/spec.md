## ADDED Requirements

### Requirement: 回车族载荷必须是 CR 而非 LF

回车键族（`KEYCODE_ENTER` / `KEYCODE_NUMPAD_ENTER` / `KEYCODE_DPAD_CENTER`）在
无修饰时 MUST 下发 CR(0x0D)，MUST NOT 下发 LF(0x0A)：raw 模式的 TUI 应用
（helix/vim/less）在 raw 模式下把 0x0A 解成 Ctrl+J 即字母 `j`
（crossterm `parse.rs`：*"\n = 0xA, which is also the keycode for Ctrl+J … When we
enter raw mode, we disable that"*），LF 因此表现为「回车变成 j」。带 Ctrl/Alt 的回车
MUST 按 kitty 键盘协议的 modifyOtherKeys 上报 `CSI 27;mod;13~`；仅 Alt 时 MUST 为
ESC 前缀加 CR。

依据：ghostty `src/input/function_keys.zig` 的裸回车条目是 `"\r"`、Alt+回车是
`"\x1b\r"`（modifyKeysNormal）、带 Ctrl 的是 `CSI 27;{mod};13~`；termux
`KeyHandler.getCode` 的裸回车与 Alt+回车同样是 `"\r"` / `"\033\r"`。
（`CSI 13;mod~` 是 xterm 的传统 `modifyFunctionKeys` 形式，与 ghostty 的实际实现不符。）

#### Scenario: 无修饰回车下发 CR

- **WHEN** 用户按回车（硬件键、`KEYCODE_DPAD_CENTER` 或输入法回车）
- **THEN** PTY 收到的字节是 0x0D，在 raw 模式下被应用读作 Enter

#### Scenario: Alt 加回车是 ESC 前缀加 CR

- **WHEN** Alt 粘滞状态下按回车
- **THEN** PTY 收到 `ESC` + 0x0D

#### Scenario: Ctrl 加回车上报 CSI 27

- **WHEN** Ctrl 粘滞状态下按回车
- **THEN** PTY 收到 `CSI 27;5;13~`，与 ghostty 的 modifyOtherKeys 一致

### Requirement: 输入法换行提交归一为 CR

输入法提交文本中的**唯一**换行提交（`"\n"`，多行字段的回车键）MUST 归一为 CR(0x0D)，
修饰键状态 MUST NOT 影响归一结果（IME 只交出一个 `"\n"`，无从得知修饰键）。
多字符提交（拼音候选、滑行输入、输入法内部粘贴）内的换行 MUST 逐字保留：
那是内容本身，逐字换成 CR 会把多行文本粘成一行。

该归一只覆盖**孤立**换行提交。多字符提交在 helix 里仍是 Ctrl+J，直到粘贴路径
真正实现 bracketed paste；本 MUST NOT 被解读为多行提交已修复。

编辑器属性 MUST 声明 `TYPE_TEXT_FLAG_MULTI_LINE`（回车键形态由此决定），
MUST NOT 使用 `IME_ACTION_NONE`：termux 在该处注明它会让屏幕键盘无法输入换行
（termux-app#221）。

#### Scenario: 输入法回车提交归一为 CR

- **WHEN** 输入法以 `commitText("\n")` 提交回车
- **THEN** PTY 收到 0x0D 而不是 0x0A

#### Scenario: 多字符提交内的换行保留

- **WHEN** 输入法提交 `"a\nb"`
- **THEN** PTY 原样收到 `a` LF `b`
